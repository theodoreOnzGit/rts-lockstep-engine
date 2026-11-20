#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::game_client` against `GameClient::addTextBearingDrawable`, `flushTextBearingDrawables` and `removeFromRayEffects` (#215, group lc3).
//!
//! # Method
//!
//! The three functions and upstream's `RayEffectSystem` (the callee of `removeFromRayEffects`) are cut verbatim by build.rs into
//! `differential/wrap_draft_game_client.cpp` (stand-ins listed there). A case is a program of up to 40 operations (add a drawable to the text list or NULL, flush,
//! add a ray effect, remove from the ray effects, drawables 0 (NULL) to 12; so the same drawable is added twice, and removed when absent). After each operation:
//! the drawables whose `drawUIText` ran, in order, the text list's length, and the ray table (128 id and start x pairs). 20,000 programs from a fixed seed.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 programs** (prediction held). Planted bugs, 5 of 5 caught (of 20,000 programs): 19881 12,760; 19882 14,997; 19883 12,760;
//! 19884 9,766; 19885 9,869.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19881 add pushes at the front; 19882 flush leaves the list; 19883 flush draws tail to head; 19884 removeFromRayEffects does nothing;
//! 19885 add ignores a drawable already listed.

mod support;

use rts_lockstep_engine::drafts::logic_common::ray_effect::RayEffectSystem;
use rts_lockstep_engine::drafts::sweep_common_client::game_client::{remove_from_ray_effects, TextBearingDrawables};
use rts_lockstep_engine::common::base_type::Coord3D;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_gc_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
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

#[test]
fn game_client_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_6C1);
    let mut c = Check::new("GameClient text list and ray removal");
    for case in 0..20_000 {
        let n = 1 + g.below(40);
        let mut ops: Vec<[i32; 4]> = Vec::new();
        for _ in 0..n {
            let kind = [0, 0, 0, 1, 2, 2, 3, 3][g.below(8) as usize];
            ops.push([kind, g.below(13) as i32, g.below(100) as i32, g.below(100) as i32]);
        }
        let prog: Vec<i32> = ops.iter().flatten().copied().collect();
        let mut cpp = vec![0i32; 80_000];
        let len = unsafe { ref_lc3_gc_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut tb = TextBearingDrawables::default();
        let mut rays = RayEffectSystem::new();
        let mut rust = Vec::new();
        for o in &ops {
            let id = if o[1] == 0 { None } else { Some(o[1]) };
            let mut log: Vec<i32> = Vec::new();
            match o[0] {
                0 => tb.add_text_bearing_drawable(id.map(|v| v as u32)),
                1 => tb.flush_text_bearing_drawables(&mut |d| log.push(d as i32)),
                2 => {
                    let s = Coord3D { x: o[2] as f32, y: 0.0, z: 0.0 };
                    let e = Coord3D { x: o[3] as f32, y: 0.0, z: 0.0 };
                    rays.add_ray_effect(id, Some(&s), Some(&e));
                }
                _ => remove_from_ray_effects(&mut rays, id),
            }
            rust.push(log.len() as i32);
            rust.extend(log);
            rust.push(tb.list.len() as i32);
            for d in rays.effect_data.iter() {
                rust.push(d.draw.unwrap_or(0));
                rust.push(d.start_loc.x as i32);
            }
        }
        c.eq(rust, cpp, || format!("case {case}: ops {ops:?}"));
    }
    finish(c);
}
