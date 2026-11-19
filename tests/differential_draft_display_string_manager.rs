#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/DisplayStringManager.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::display_string_manager::DisplayStringManager` against `DisplayStringManager` (#215).
//!
//! # Method
//!
//! `DisplayStringManager::DisplayStringManager` to the end of `DisplayStringManager.cpp` (constructor, destructor, `link`, `unLink`) is cut
//! verbatim (build.rs) into `differential/wrap_draft_display_string_manager.cpp`; the strings are the two link fields of `DisplayString`
//! in an arena of 8. A case is a random sequence of `link(i)` and `unLink(i)` of arena strings, **any** strings in **any** state: upstream's
//! asserts (not linked, in the list) are compiled out of release, and with all eight strings in one arena every pointer the cut follows is a
//! valid object, so the sequence is defined (a string linked twice makes a loop, as upstream does). After each op the head and every string's `next`
//! and `prev` (an arena index, -1 for NULL) are compared; 20,000 sequences of 1 to 30 ops from a fixed seed.
//!
//! Not covered: `game_client_message_dispatcher` (GameClientDispatch.cpp: a comparison chain on message numbers, no state).
//!
//! # Prediction (written before the first run, 2026-11-19)
//!
//! 0 mismatches (the draft reads as the same statements as the cut).

mod support;

use rts_lockstep_engine::drafts::logic_common::display_string_manager::{DisplayStringLinks, DisplayStringManager};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dsm_run(ops: *const i32, nops: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn idx(o: Option<usize>) -> i32 {
    o.map(|i| i as i32).unwrap_or(-1)
}

#[test]
fn display_string_manager_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_D5A1);
    let mut c = Check::new("DisplayStringManager");
    for case in 0..20_000 {
        let n = 1 + g.below(30);
        let mut ops: Vec<i32> = Vec::new();
        for _ in 0..n {
            ops.extend([(g.below(5) < 2) as i32, g.below(8) as i32]);
        }
        let mut cpp = vec![0i32; 1000];
        let len = unsafe { ref_dsm_run(ops.as_ptr(), ops.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut m = DisplayStringManager::new();
        let mut links = [DisplayStringLinks::default(); 8];
        let mut rust: Vec<i32> = Vec::new();
        for op in ops.chunks(2) {
            if op[0] == 0 {
                m.link(&mut links, op[1] as usize);
            } else {
                m.un_link(&mut links, op[1] as usize);
            }
            rust.push(idx(m.string_list));
            for l in &links {
                rust.push(idx(l.next));
                rust.push(idx(l.prev));
            }
        }
        rust.push(idx(m.current_checkpoint));
        c.eq(rust, cpp, || format!("case {case}: ops {ops:?}"));
    }
    finish(c);
}
