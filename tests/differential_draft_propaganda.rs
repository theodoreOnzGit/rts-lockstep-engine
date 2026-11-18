#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaCenterBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::propaganda_center_behavior::PropagandaCenterBehavior` against `PropagandaCenterBehavior` (#215).
//!
//! # Method
//!
//! The constructor, `onDelete`, `update` and `onRemoving` are cut (build.rs) into `differential/wrap_draft_propaganda.cpp`, with one
//! mechanical edit (VC6's for-scope: `update` reads its loop variable `it` after the loop; the copy declares it before) and stand-ins
//! for everything the cut calls (listed there: the prison base with recorded calls, the objects, the game logic's `findObjectByID` and
//! `getFrame`, the player and team). The stand-ins and the draft's `PropagandaWorld` record the same events as words, and after each
//! operation the behaviour's state is recorded (subject, start frame, the brainwashed list newest first). The draft does not run the
//! base class (its caller does), so the test records the base's call where upstream makes it: `PrisonBehavior::update` and
//! `onDelete` before the derived body, `onRemoving` after it. 20,000 sequences of 1 to 40 operations (update, onRemoving, onDelete)
//! from a fixed seed, each with a fresh environment: the frame (edge values and counting up, including the unsigned wrap), which of the
//! objects 1 to 8 exist, which have AI, whether the centre has a controlling player, whether a door is available, and 0 to 4 contained
//! objects; the brainwash duration from 0, 1, small and large values.
//!
//! The upstream file is under `#ifdef ALLOW_SURRENDER`, which the shipped game does not define; it is compiled here as if defined.
//! Not covered: the module data's INI key, `Snapshot` (xfer), and the prison base (`prison_behavior.rs`, mostly not compiled).
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 20,000 sequences (315,231 updates), **0 mismatches** (prediction held). Planted bugs, 9 of 9 caught (of 20,000 sequences): 11521 2,945;
//! 11522 2,773; 11523 13,146; 11524 13,594; 11525 6,140; 11526 1,524; 11527 17,367; 11528 17,485; 11529 15,281.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11521 `>` for `>=` in the duration test; 11522 the list pushed at the back; 11523 no "already in the list" test; 11524
//! no `setSurrendered`; 11525 `onRemoving` keeps the start frame; 11526 `onDelete` restores units that no longer exist; 11527 the start
//! frame not recorded when a subject is assigned; 11528 the duration test in signed arithmetic; 11529 the subject cleared after the exit.

mod support;

use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::drafts::logic_common::prison_behavior::PrisonBehaviorModuleData;
use rts_lockstep_engine::drafts::logic_common::propaganda_center_behavior::{PropagandaCenterBehavior, PropagandaCenterBehaviorModuleData, PropagandaWorld};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_propaganda_run(duration: u32, ops: *const u32, nops: i32, out: *mut u32) -> i32;
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

struct World {
    log: Vec<u32>,
    frame: u32,
    exist: u32,
    ai: u32,
    has_player: bool,
    door: bool,
    contained: Vec<ObjectId>,
}

impl PropagandaWorld for World {
    fn get_frame(&self) -> u32 {
        self.frame
    }
    fn object_exists(&self, id: ObjectId) -> bool {
        id >= 1 && id <= 8 && self.exist & (1 << (id - 1)) != 0
    }
    fn restore_original_team(&mut self, obj: ObjectId) {
        self.log.extend([10, obj as u32]);
    }
    fn reserve_door_for_exit(&mut self, obj: ObjectId) -> bool {
        self.log.extend([11, obj as u32]);
        self.door
    }
    fn exit_object_via_door(&mut self, obj: ObjectId) {
        self.log.extend([14, obj as u32]);
    }
    fn set_temporary_team_to_owners_default(&mut self, _us: ObjectId, obj: ObjectId) -> bool {
        if self.has_player {
            self.log.extend([12, obj as u32]);
        }
        self.has_player
    }
    fn clear_surrendered(&mut self, obj: ObjectId) {
        if obj >= 1 && obj <= 8 && self.ai & (1 << (obj - 1)) != 0 {
            self.log.extend([13, obj as u32]);
        }
    }
    fn first_contained(&self, _us: ObjectId) -> Option<ObjectId> {
        self.contained.first().copied()
    }
}

fn frame(g: &mut Gen, last: u32) -> u32 {
    match g.below(6) {
        0 => 0,
        1 => u32::MAX - g.below(4),
        2 => 0x7FFF_FFFE + g.below(4),
        3 => last.wrapping_add(g.below(8)),
        4 => last.wrapping_add(g.below(300)),
        _ => g.below(1000),
    }
}

#[test]
fn propaganda_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B2A1);
    let mut c = Check::new("PropagandaCenterBehavior");
    let (mut updates, mut exits, mut restores) = (0u32, 0u32, 0u32);
    for case in 0..20_000 {
        let duration = match g.below(6) {
            0 => 0,
            1 => 1,
            2 => g.below(10),
            3 => u32::MAX,
            4 => 0x8000_0000,
            _ => g.below(300),
        };
        let nops = 1 + g.below(40) as usize;
        let mut ops: Vec<u32> = Vec::new();
        let mut last = g.below(100);
        for _ in 0..nops {
            let kind = match g.below(10) {
                0..=6 => 0,
                7..=8 => 1,
                _ => if g.below(3) == 0 { 2 } else { 0 },
            };
            last = frame(&mut g, last);
            let n = g.below(5);
            ops.extend([kind, last, if g.below(5) == 0 { g.below(256) } else { 0xFF - g.below(4) }, g.below(256), (g.below(8) != 0) as u32, (g.below(3) != 0) as u32, n]);
            for _ in 0..4 {
                ops.push(1 + g.below(8));
            }
            ops.push(1 + g.below(8));
        }
        let mut cpp = vec![0u32; nops * 64];
        let len = unsafe { ref_propaganda_run(duration, ops.as_ptr(), nops as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut b = PropagandaCenterBehavior::new(PropagandaCenterBehaviorModuleData { prison: PrisonBehaviorModuleData::default(), brainwash_duration: duration });
        let mut w = World { log: Vec::new(), frame: 0, exist: 0, ai: 0, has_player: true, door: false, contained: Vec::new() };
        for k in 0..nops {
            let o = &ops[12 * k..12 * k + 12];
            w.frame = o[1];
            w.exist = o[2];
            w.ai = o[3];
            w.has_player = o[4] != 0;
            w.door = o[5] != 0;
            w.contained = o[7..7 + o[6] as usize].iter().map(|&x| x as ObjectId).collect();
            match o[0] {
                0 => {
                    w.log.push(20); // PrisonBehavior::update()
                    b.update(&mut w, 100);
                    updates += 1;
                }
                1 => {
                    b.on_removing(o[11] as ObjectId);
                    w.log.extend([22, o[11]]); // then PrisonBehavior::onRemoving
                }
                _ => {
                    w.log.push(21); // PrisonBehavior::onDelete()
                    b.on_delete(&mut w);
                }
            }
            w.log.extend([30, b.brainwashing_subject_id as u32, b.brainwashing_subject_start_frame, b.brainwashed_list.len() as u32]);
            w.log.extend(b.brainwashed_list.iter().map(|&x| x as u32));
        }
        exits += w.log.windows(2).filter(|p| p[0] == 14).count() as u32;
        restores += w.log.windows(2).filter(|p| p[0] == 10).count() as u32;
        let _ = INVALID_ID;
        c.eq(w.log, cpp, || format!("case {case}: duration {duration} ops {ops:?}"));
    }
    println!("(propaganda: {updates} updates, about {exits} exit-looking events, about {restores} restore-looking events)");
    finish(c);
}
