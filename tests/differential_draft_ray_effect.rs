#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/RayEffect.cpp (and Include/GameClient/RayEffect.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::ray_effect::RayEffectSystem` against `RayEffectSystem` (#215).
//!
//! # Method
//!
//! RayEffect.cpp from `class RayEffectSystem *TheRayEffects = NULL;` to the end is cut verbatim (build.rs) into
//! `differential/wrap_draft_ray_effect.cpp` (header unchanged; the stand-ins are listed there: the `SubsystemInterface` base, and
//! a drawable being the pointer made of its id). 3,000 sequences of 20 to 600 operations from a fixed seed: add (drawables 1 to 40
//! or 1 to 200, so the 128-entry table fills and overflows), delete, get (the answer is written over a sentinel, so "left alone" is
//! visible), reset, and the NULL arguments of add, get and delete. The locations are any float bits (NaN payloads, zero signs, infinities):
//! they are only copied. Compared: every `get` result, and the whole table (128 entries) every 16th operation and at the end.
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches (pure bookkeeping on arrays).
//!
//! # Results
//!
//! 2026-11-18: 3,000 sequences (about 900,000 operations), **0 mismatches** (prediction held). Planted bugs, 8 of 8 caught (of 3,000 sequences): 11503
//! 3,000; 11504 1,952; 11505 2,137; 11506 178; 11507 2,810; 11508 2,880; 11509 2,268; 11510 2,998.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11503 a free slot is searched from the end; 11504 `delete` clears every entry of the drawable; 11505 `delete` also
//! zeroes the locations; 11506 `add` does not look for a free slot but overwrites slot 0 when the table is full; 11507 `get` copies only
//! the start location; 11508 `add` of a drawable already present updates its entry; 11509 `reset` leaves the locations; 11510
//! `get` with a missing drawable clears the data.

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::drafts::logic_common::ray_effect::{RayEffectData, RayEffectSystem};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_ray_run(ops: *const u32, nops: i32, out: *mut u32) -> i32;
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

fn put(d: &RayEffectData, out: &mut Vec<u32>) {
    out.push(d.draw.map(|x| x as u32).unwrap_or(0));
    for v in [d.start_loc.x, d.start_loc.y, d.start_loc.z, d.end_loc.x, d.end_loc.y, d.end_loc.z] {
        out.push(v.to_bits());
    }
}

#[test]
fn ray_effect_system_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_7A7E);
    let mut c = Check::new("RayEffectSystem");
    for case in 0..3_000 {
        let nops = 20 + g.below(581) as usize;
        let max_id = if g.below(2) == 0 { 40 } else { 200 };
        let mut ops: Vec<u32> = Vec::new();
        for _ in 0..nops {
            let kind = match g.below(20) {
                0..=7 => 0,
                8..=10 => 1,
                11..=13 => 2,
                14 => 3,
                15 => 4,
                16 => 5,
                17 => 6,
                18 => 7,
                _ => 8,
            };
            // reset is rare so that the table fills
            let kind = if kind == 3 && g.below(8) != 0 { 2 } else { kind };
            ops.push(kind);
            ops.push(1 + g.below(max_id));
            for _ in 0..6 {
                ops.push(if g.below(4) == 0 { g.u32() } else { g.mixed().to_bits() });
            }
        }
        let mut cpp = vec![0u32; (nops / 16 + 2) * 128 * 7 + nops * 7 + 16];
        let n = unsafe { ref_ray_run(ops.as_ptr(), nops as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(n);

        let mut sys = RayEffectSystem::new();
        let mut rust: Vec<u32> = Vec::new();
        let dump = |s: &RayEffectSystem, out: &mut Vec<u32>| {
            for d in s.effect_data.iter() {
                put(d, out);
            }
        };
        for k in 0..nops {
            let o = &ops[8 * k..8 * k + 8];
            let id = o[1] as u64;
            let f = |i: usize| f32::from_bits(o[i]);
            let start = Coord3D { x: f(2), y: f(3), z: f(4) };
            let end = Coord3D { x: f(5), y: f(6), z: f(7) };
            match o[0] {
                0 => sys.add_ray_effect(Some(id as _), Some(&start), Some(&end)),
                1 => sys.delete_ray_effect(Some(id as _)),
                2 | 7 => {
                    let mut d = RayEffectData {
                        draw: Some(0xDEAD as _),
                        start_loc: Coord3D { x: 7.0, y: 8.0, z: 9.0 },
                        end_loc: Coord3D { x: 17.0, y: 18.0, z: 19.0 },
                    };
                    sys.get_ray_effect_data(if o[0] == 2 { Some(id as _) } else { None }, &mut d);
                    put(&d, &mut rust);
                }
                3 => sys.reset(),
                4 => sys.add_ray_effect(None, Some(&start), Some(&end)),
                5 => sys.add_ray_effect(Some(id as _), None, Some(&end)),
                6 => sys.add_ray_effect(Some(id as _), Some(&start), None),
                _ => sys.delete_ray_effect(None),
            }
            if k % 16 == 15 {
                dump(&sys, &mut rust);
            }
        }
        dump(&sys, &mut rust);
        c.eq(rust == cpp, true, || format!("sequence {case} of {nops} ops differs"));
    }
    finish(c);
}
