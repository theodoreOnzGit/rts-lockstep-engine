#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::selection_xlat` against `SelectionTranslator::selectFriends` and `killThemKillThemAll` (#215, group lc3).
//!
//! # Method
//!
//! The two functions are cut verbatim by build.rs (`sel_xlat.inc`) into `differential/wrap_draft_selection_xlat.cpp`; its stand-ins are listed
//! there (the class with its two members, drawables, `CanSelectDrawable`, `TheInGameUI`, `TheGameText`, the message). A case: a selection limit
//! (0, 1..5, or negative), a starting select count, up to 8 drawables (4 templates, object or none, the two answers of `CanSelectDrawable`), and a
//! program of up to 12 calls (selectFriends with or without a message and with dragSelecting, killThemKillThemAll including a NULL drawable). Compared
//! after each call: the result, the message's ids, the warning flag, the messages shown, the select calls and count, and the per-template map.
//! 20,000 cases from a fixed seed.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches: the draft reads as a line-for-line translation.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 cases** (prediction held). Planted bugs, 7 of 7 caught (of 20,000 cases): 19861 5,439; 19862 4,393; 19863 7,329; 19864 5,710; 19865 4,296; 19866 8,119; 19867 5,669.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19861 the limit test `>= 0` for `> 0`; 19862 `>` for `>=` on the count; 19863 the warning is shown every time; 19864 selectFriends
//! passes dragSelecting FALSE; 19865 the template is counted only for a drawable with an object; 19866 selectFriends never appends the object id;
//! 19867 killThemKillThemAll proceeds for a drawable with no object.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::selection_xlat::{SelectFriendsHost, SelectFriendsState};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_sel_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Draw {
    tmpl: i32,
    obj: i32,
    can: [bool; 2],
}

struct Host {
    max: i32,
    count: i32,
    draws: Vec<Draw>,
    messages: Vec<i32>,
    select_calls: i32,
}

impl SelectFriendsHost for Host {
    fn can_select_drawable(&self, draw: u32, drag_selecting: bool) -> bool {
        self.draws[draw as usize - 1].can[drag_selecting as usize]
    }
    fn max_select_count(&self) -> i32 {
        self.max
    }
    fn select_count(&self) -> i32 {
        self.count
    }
    fn message_max_selection_size(&mut self, max: i32) {
        self.messages.push(max);
    }
    fn select_drawable(&mut self, _draw: u32) {
        self.count += 1;
        self.select_calls += 1;
    }
    fn drawable_template(&self, draw: u32) -> Vec<u8> {
        vec![self.draws[draw as usize - 1].tmpl as u8]
    }
    fn drawable_object_id(&self, draw: u32) -> Option<u32> {
        let o = self.draws[draw as usize - 1].obj;
        if o == 0 {
            None
        } else {
            Some(o as u32)
        }
    }
}

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

#[test]
fn selection_xlat_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5E1);
    let mut c = Check::new("selectFriends / killThemKillThemAll");
    for case in 0..20_000 {
        let max = [0, 0, 1, 2, 3, 5, -1][g.below(7) as usize];
        let count = g.below(6) as i32;
        let nd = 1 + g.below(8) as usize;
        let mut prog: Vec<i32> = vec![max, count, nd as i32];
        let mut draws = Vec::new();
        for _ in 0..nd {
            let d = Draw { tmpl: g.below(4) as i32, obj: if g.below(4) == 0 { 0 } else { 1 + g.below(1000) as i32 }, can: [g.below(4) != 0, g.below(4) != 0] };
            prog.extend([d.tmpl, d.obj, d.can[0] as i32, d.can[1] as i32]);
            draws.push(d);
        }
        let nops = 1 + g.below(12) as usize;
        prog.push(nops as i32);
        let mut ops = Vec::new();
        for _ in 0..nops {
            let kind = g.below(2) as i32;
            let mut d = 1 + g.below(nd as u32) as i32;
            if kind == 1 && g.below(8) == 0 {
                d = 0;
            }
            let op = [kind, d, g.below(3).min(1) as i32, g.below(2) as i32];
            prog.extend(op);
            ops.push(op);
        }
        let mut cpp = vec![0i32; 4000];
        let len = unsafe { ref_lc3_sel_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut host = Host { max, count, draws, messages: Vec::new(), select_calls: 0 };
        let mut st = SelectFriendsState::default();
        let mut rust: Vec<i32> = Vec::new();
        for &[kind, d, has_msg, drag] in &ops {
            let mut ids: Vec<u32> = Vec::new();
            let msg = if has_msg != 0 { Some(&mut ids) } else { None };
            let r = if kind == 0 {
                st.select_friends(&mut host, d as u32, msg, drag != 0)
            } else {
                st.kill_them_kill_them_all(&mut host, if d == 0 { None } else { Some(d as u32) }, msg)
            };
            rust.push(r as i32);
            rust.push(ids.len() as i32);
            rust.extend(ids.iter().map(|&i| i as i32));
            rust.push(st.displayed_max_warning as i32);
            rust.push(host.messages.len() as i32);
            rust.extend(host.messages.iter().copied());
            rust.push(host.select_calls);
            rust.push(host.count);
            for t in 0..4u8 {
                rust.push(st.select_count_map.get(&vec![t]).copied().unwrap_or(0));
            }
        }
        c.eq(rust, cpp, || format!("case {case}: max {max}, count {count}, ops {ops:?}"));
    }
    finish(c);
}
