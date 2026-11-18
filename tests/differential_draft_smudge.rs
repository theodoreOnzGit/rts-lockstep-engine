#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/Smudge.cpp (and Include/GameClient/Smudge.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::smudge::SmudgePool` against `SmudgeManager` and `SmudgeSet` (#215).
//!
//! # Method
//!
//! Smudge.cpp from the static free list to the end is cut verbatim (build.rs) into `differential/wrap_draft_smudge.cpp`, on upstream's
//! own `dllist.h` and the header read unchanged; the draft's `VecDeque`s stand for the intrusive lists. 10,000 sequences of 1 to 120
//! operations from a fixed seed: add a set, remove a used set, add a smudge to a used set, remove one of its smudges, reset the manager,
//! reset one set. An operation names its target by position in the current used list, so each side resolves it in its own state. After
//! every operation the whole state is compared: the used sets, the free sets, the free smudge list (shared by every set), each set's
//! count and used smudges, as the numbers the objects were created in.
//!
//! Not covered: `~SmudgeManager` (upstream's loop removes the head of the free SET list instead of the free smudge list: undefined once
//! it is empty; the draft records and does not reproduce it), and the hardware-support flag and the smudge count of the last frame
//! (plain fields).
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 10,000 sequences, **0 mismatches** (prediction held). Planted bugs, 8 of 8 caught (of 10,000 sequences): 11530 6,453; 11531 7,239; 11532
//! 5,787; 11533 7,572; 11534 8,386; 11535 7,922; 11536 8,704; 11537 6,940.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11530 a freed set goes to the free list's tail; 11531 a reset set goes to the free list's head; 11532 a freed smudge
//! goes to the free list's tail; 11533 a reset set's smudges go to the free list's tail; 11534 a new smudge always, never recycled; 11535
//! the used count not decremented on removal; 11536 a recycled set is not moved to the used list; 11537 the manager `reset` stops after the
//! first set.

mod support;

use rts_lockstep_engine::drafts::logic_common::smudge::SmudgePool;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_smudge_run(ops: *const u32, nops: i32, out: *mut u32) -> i32;
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

fn state(p: &SmudgePool, out: &mut Vec<u32>) {
    let list = |l: &std::collections::VecDeque<usize>, out: &mut Vec<u32>| {
        out.push(l.len() as u32);
        out.extend(l.iter().map(|&x| x as u32));
    };
    list(&p.used_smudge_set_list, out);
    list(&p.free_smudge_set_list, out);
    list(&p.free_smudge_list, out);
    for s in p.sets.iter() {
        out.push(s.used_smudge_count as u32);
        list(&s.used_smudge_list, out);
    }
}

#[test]
fn smudge_pool_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5D6E);
    let mut c = Check::new("SmudgeManager / SmudgeSet");
    for case in 0..10_000 {
        let nops = 1 + g.below(120) as usize;
        let mut ops: Vec<u32> = Vec::new();
        for _ in 0..nops {
            let kind = match g.below(20) {
                0..=3 => 0,
                4..=5 => 1,
                6..=11 => 2,
                12..=15 => 3,
                16 => 4,
                _ => 5,
            };
            ops.extend([kind, g.below(1000), g.below(1000)]);
        }
        let mut cpp = vec![0u32; 1 << 20];
        let n = unsafe { ref_smudge_run(ops.as_ptr(), nops as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(n);

        let mut p = SmudgePool::new();
        let mut rust: Vec<u32> = Vec::new();
        for k in 0..nops {
            let (kind, j, q) = (ops[3 * k], ops[3 * k + 1] as usize, ops[3 * k + 2] as usize);
            let used: Vec<usize> = p.used_smudge_set_list.iter().copied().collect();
            let set = if used.is_empty() { None } else { Some(used[j % used.len()]) };
            match (kind, set) {
                (0, _) => {
                    p.add_smudge_set();
                }
                (1, Some(s)) => p.remove_smudge_set(s),
                (2, Some(s)) => {
                    p.add_smudge_to_set(s);
                }
                (3, Some(s)) => {
                    let sm: Vec<usize> = p.sets[s].used_smudge_list.iter().copied().collect();
                    if !sm.is_empty() {
                        p.remove_smudge_from_set(s, sm[q % sm.len()]);
                    }
                }
                (4, _) => p.reset(),
                (5, Some(s)) => p.set_reset(s),
                _ => {}
            }
            state(&p, &mut rust);
        }
        c.eq(rust == cpp, true, || format!("sequence {case}: ops {ops:?}"));
    }
    finish(c);
}
