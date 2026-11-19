#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp (getZFriction, isCurrentlyOverlapped,
//   wasPreviouslyOverlapped)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::physics_update` against upstream's (#215).
//!
//! # Method
//!
//! The three functions and the two friction limits are cut verbatim (build.rs, drafts_guilogic3) into
//! `differential/wrap_draft_phys.cpp`. 40,000 random cases for `getZFriction` (module-data friction and extra friction drawn
//! from NaN, infinities, signed zeros, the limits 0.01 and 0.99 exactly and their neighbours by one ULP, and uniform values in
//! [-1, 2]); 40,000 for each overlap test (small ids so that the object id equals the overlap id often, ids -1 and 0, and
//! `NULL`). Floats compared by bits.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: two clamps and two id comparisons.
//!
//! # Results
//!
//! 2026-11-19: 40,000 cases per function, **0 mismatches** in all three, first run (prediction met). Planted bugs, 8 of 8 caught (mismatches of 40,000):
//! 15661 21,501 (`z - extra`); 15662 16,899 (extra friction ignored); 15663 9,372 (no lower clamp); 15664 9,372 (clamps to 0, not 0.01); 15665 15,366 (no upper
//! clamp); 15666 15,366 (clamps to 1.0, not 0.99); 15667 9,825 (the wrong overlap id compared); 15668 8,022 (`NULL` counts as overlapped).

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::physics_update as pu;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_phys_zfriction(z: f32, extra: f32) -> f32;
    fn ref_phys_overlap(which: i32, cur: i32, prev: i32, has_obj: i32, obj_id: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn friction(g: &mut Gen) -> f32 {
    let lo = f32::from_bits(0.01f32.to_bits() - 1);
    let hi = f32::from_bits(0.99f32.to_bits() + 1);
    match g.below(12) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => f32::NEG_INFINITY,
        3 => 0.0,
        4 => -0.0,
        5 => 0.01,
        6 => 0.99,
        7 => lo,
        8 => hi,
        9 => 0.8,
        _ => g.uniform(-1.0, 2.0),
    }
}

fn id(g: &mut Gen) -> i32 {
    g.below(6) as i32 - 1
}

#[test]
fn physics() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_9A51);
    let mut z = Check::new("PhysicsBehavior::getZFriction");
    let mut cur = Check::new("PhysicsBehavior::isCurrentlyOverlapped");
    let mut prev = Check::new("PhysicsBehavior::wasPreviouslyOverlapped");
    for _ in 0..40_000 {
        let (a, b) = (friction(&mut g), friction(&mut g));
        z.f32(pu::get_z_friction(a, b), unsafe { ref_phys_zfriction(a, b) }, || format!("z {a:e} extra {b:e}"));
        let (c, p, has, o) = (id(&mut g), id(&mut g), g.below(5) != 0, id(&mut g));
        let obj = if has { Some(o) } else { None };
        let cpp = unsafe { ref_phys_overlap(0, c, p, has as i32, o) } != 0;
        cur.eq(pu::is_currently_overlapped(c, obj), cpp, || format!("cur {c} prev {p} obj {obj:?}"));
        let cpp = unsafe { ref_phys_overlap(1, c, p, has as i32, o) } != 0;
        prev.eq(pu::was_previously_overlapped(p, obj), cpp, || format!("cur {c} prev {p} obj {obj:?}"));
    }
    if planted() {
        for c in [z, cur, prev] {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in [z, cur, prev] {
        c.finish();
    }
}
