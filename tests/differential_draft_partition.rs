#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp (PartitionData::getControllingPlayerIndex,
//   friend_setShroudednessPrevious, PartitionContactList::resetContactList)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::partition_manager` against upstream's (#215).
//!
//! # Method
//!
//! The three functions are cut verbatim (build.rs, drafts_guilogic3) into `differential/wrap_draft_partition.cpp`, with the
//! `DISABLE_INVALID_PREVENTION` define PartitionManager.cpp:109 has (so friend_setShroudednessPrevious's `#ifndef` block is not
//! compiled). 10,000 random cases each:
//!
//! - `getControllingPlayerIndex`: a player with an index of 0 to 15, or none (upstream `throw ERROR_BUG`; the wrapper reports -999, the
//!   draft's `None`).
//! - `friend_setShroudednessPrevious`: 1 to 40 calls with a player index 0 to 15 and a status from the six `ObjectShroudStatus`
//!   values; compared: both arrays, all 16 entries.
//! - `resetContactList`: 0 to 60 nodes, some of them held by hash sockets (0 to 31 by a random mask, and always the last socket of 5,381);
//!   compared: nodes deleted, sockets still set, list head.
//!
//! Excluded: a player index outside 0..16 (upstream asserts in debug and indexes out of range in release).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: three small functions.
//!
//! # Results
//!
//! 2026-11-19: 10,000 cases per function, **0 mismatches** in all three. The first run did not get that far: the process aborted ("Rust cannot catch foreign
//! exceptions"), because the wrapper caught `int` and upstream throws `ERROR_BUG`, an enumerator of an unnamed enum; the wrapper now catches everything (a wrapper fix, not a
//! draft fix). Planted bugs, 8 of 8 caught (mismatches of 10,000): 15698 527 (index 0 taken for no player); 15699 10,000 (ever-seen inverted); 15700 8,018 (always seen);
//! 15701 9,935 (previous status not stored); 15702 9,846 (only 32 sockets cleared); 15703 9,846 (nodes not cleared); 15704 9,846 (sockets not cleared); 15705 9,382 (ever-seen
//! tests the wrong status).

mod support;

use rts_lockstep_engine::common::game_common::ObjectShroudStatus as S;
use rts_lockstep_engine::drafts::sweep_gui_logic::partition_manager as pm;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_pm_ctl(has_player: i32, index: i32) -> i32;
    fn ref_pm_shroud(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_pm_reset(nodes: i32, mask: u32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const STATUSES: [S; 6] = [S::Invalid, S::Clear, S::PartialClear, S::Fogged, S::Shrouded, S::InvalidButPreviousValid];

#[test]
fn partition() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_9A27);
    let mut ctl = Check::new("PartitionData::getControllingPlayerIndex");
    let mut sh = Check::new("PartitionData::friend_setShroudednessPrevious");
    let mut rs = Check::new("PartitionContactList::resetContactList");
    for i in 0..10_000 {
        let (has, idx) = (g.below(6) != 0, g.below(16) as i32);
        let want = pm::get_controlling_player_index(if has { Some(idx) } else { None }).unwrap_or(-999);
        ctl.eq(want, unsafe { ref_pm_ctl(has as i32, idx) }, || format!("case {i}: has {has} index {idx}"));

        let n = 1 + g.below(40) as usize;
        let mut prog = Vec::new();
        let mut prev = [S::Invalid; 16];
        let mut seen = [false; 16];
        for _ in 0..n {
            let (p, st) = (g.below(16) as usize, g.below(6) as usize);
            prog.extend([p as i32, st as i32]);
            pm::friend_set_shroudedness_previous(&mut prev, &mut seen, p, STATUSES[st]);
        }
        let mut want: Vec<i32> = prev.iter().map(|&s| s as i32).collect();
        want.extend(seen.iter().map(|&b| b as i32));
        let mut out = vec![0i32; 64];
        let k = unsafe { ref_pm_shroud(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), 64) };
        out.truncate(k as usize);
        sh.eq(want, out, || format!("case {i}: {prog:?}"));

        let nodes = g.below(61) as i32;
        let mask = if g.below(4) == 0 { 0 } else { g.u32() };
        let mut v: Vec<u32> = (0..nodes as u32).collect();
        let mut heads: Vec<Option<usize>> = vec![None; 5381];
        for k in 0..(nodes as usize).min(32) {
            if (mask >> k) & 1 == 1 {
                heads[k] = Some(k);
            }
        }
        if nodes > 0 {
            heads[5380] = Some(nodes as usize - 1);
        }
        pm::reset_contact_list(&mut v, &mut heads);
        let want = vec![nodes - v.len() as i32, heads.iter().filter(|h| h.is_some()).count() as i32, v.is_empty() as i32];
        let mut out = [0i32; 3];
        unsafe { ref_pm_reset(nodes, mask, out.as_mut_ptr()) };
        rs.eq(want, out.to_vec(), || format!("case {i}: nodes {nodes} mask {mask:#x}"));
    }
    if planted() {
        for c in [ctl, sh, rs] {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in [ctl, sh, rs] {
        c.finish();
    }
}
