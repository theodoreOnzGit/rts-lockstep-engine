#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the draft src/drafts/wwlib/vehiclecurve.rs (and the Curve3DClass base it uses, curve.rs)
// against (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit
// 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vehiclecurve.h, vehiclecurve.cpp, curve.h, curve.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `VehicleCurveClass` (turn arcs through a series of points) against the C++ (#215).
//!
//! # Method
//!
//! `differential/wrap_draft_vehiclecurve.cpp` is compiled with vehiclecurve.cpp and curve.cpp UNCHANGED (clang++ as the
//! other W3D builds; stand-ins only for `PersistFactoryClass`'s constructor and destructor, which register a save/load
//! factory this test never uses). Each case is a PROGRAM of 5 to 30 operations from a fixed seed, run on both sides:
//! a new curve (radius 0, 1 to 25, 100, or negative), `Add_Key` (points random, repeated, collinear, or in a tight
//! zigzag; times increasing, repeated, or out of order), `Set_Key`, `Remove_Key`, `Clear_Keys`, `Set_Looping`, the
//! queries (`Key_Count`, `Get_Start_Time`, `Get_End_Time`, `Get_Key`) and `Evaluate` at times before the first key, at a
//! key, inside a segment (at 20 fractions), at a segment boundary of the line/arc split, and past the last key. Every
//! result float is compared by bits: the point, `Get_Current_Sharpness`'s value and position, `Get_Last_Eval_Time`.
//! Evaluate is only called on a curve with at least one key (upstream reads `Keys[0]` unchecked) and never with NaN.
//! The free functions `Find_Tangent`, `Get_Angle_Delta`, `Find_Turn_Arc`, `Find_Tangents` and `Update_Arc_List` run
//! inside `Evaluate`. Not covered: `Save`, `Load`, `Load_Variables`, `Get_Factory` (chunk I/O, another test), and
//! `Initialize_Arc` (declared, defined nowhere).
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! 0 mismatches except that I expect the draft's `Find_Tangent` (`::sqrt` on a `double`) to agree in value with the
//! interposed `sqrt` (exact rounding), so no miss from it. A possible miss: the arc-angle wraps (`WWMath::Wrap` on
//! `float` vs the draft's) near multiples of 2 pi.
//!
//! # Results (2026-11-18)
//!
//! **0 mismatches on the first run**: 40,000 programs, 37,724 of them with an `Evaluate` (about 250,000 result groups of
//! 8 floats), 2,141,823 floats compared, max ULP 0, no NaN. **Prediction held.** One style point fixed in the draft:
//! `Find_Tangent`'s `::sqrt` was the `f64` method; it is now `petir::Real::sqrt` (the same value, sqrt is exactly rounded).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: mismatching floats of 2,141,823:
//!
//! | id | planted bug | floats |
//! |---|---|---|
//! | 11299 | `Find_Tangent` `dist > radius` | 2,907 |
//! | 11300 | the clockwise test inverted | 306,403 |
//! | 11301 | `Get_Angle_Delta` counter-clockwise `angle1 >= angle2` | 11,129 |
//! | 11302 | `Find_Turn_Arc` `delta1 <= delta2` | 332 |
//! | 11303 | `point_out.y` loses its minus | 148,072 |
//! | 11304 | the long-way-round check at 180 degrees | 38 |
//! | 11305 | `time1` without the straight part | 81,474 |
//! | 11306 | `seg_time <= time2` | 7,353 |
//! | 11307 | `m_LastTime` of the line part uses `time1` | 85,143 |
//! | 11308 | `Set_Key` does not mark the arcs dirty | 9,041 |
//! | 11309 | the first arc's sharpness over 20 degrees | 1,046 |
//!
//! **11 of 11 caught** (11304 the fewest, 38 floats: the 200-degree check needs a very tight turn).

mod support;

use rts_lockstep_engine::drafts::wwlib::vehiclecurve::VehicleCurveClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

extern "C" {
    fn draft_vc_run(prog: *const f32, n: i32, out: *mut f32) -> i32;
}

const CASES: usize = 40_000;

/// `DRAFT_CASES=<n>` overrides the number of cases (the planted-bug runs use fewer than the gated run).
fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

struct Prog {
    ops: Vec<f32>,
}

struct Model {
    times: Vec<f32>,
}

fn gen_program(g: &mut Gen) -> Vec<f32> {
    let mut p: Vec<f32> = Vec::new();
    let radius = match g.below(8) {
        0 => 0.0,
        1 => -g.uniform(0.5, 10.0),
        2 => 100.0,
        _ => g.uniform(1.0, 25.0),
    };
    p.extend_from_slice(&[0.0, radius]);
    let mut times: Vec<f32> = Vec::new(); // the model of the keys' times, in order
    let mut keys = 0usize;
    let style = g.below(4);
    let nops = 5 + g.below(26);
    let mut t_next = g.uniform(0.0, 5.0);
    let mut last = Vector3::new(0.0, 0.0, 0.0);
    for _ in 0..nops {
        match g.below(20) {
            0..=7 => {
                // Add_Key
                let pt = match style {
                    0 => Vector3::new(g.uniform(-100.0, 100.0), g.uniform(-100.0, 100.0), g.uniform(-10.0, 10.0)),
                    1 => Vector3::new(last.x + g.uniform(-30.0, 30.0), last.y + g.uniform(-30.0, 30.0), last.z),
                    2 => Vector3::new(last.x + 10.0, last.y + if keys % 2 == 0 { 3.0 } else { -3.0 }, g.uniform(0.0, 2.0)),
                    _ => Vector3::new(g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-1.0, 1.0)),
                };
                let pt = if g.below(12) == 0 { last } else { pt };
                last = pt;
                let t = match g.below(10) {
                    0 => t_next, // repeated time
                    1 => g.uniform(0.0, 20.0), // out of order
                    _ => {
                        t_next += g.uniform(0.2, 3.0);
                        t_next
                    }
                };
                p.extend_from_slice(&[1.0, pt.x, pt.y, pt.z, t]);
                let mut idx = 0;
                while idx < times.len() && times[idx] < t {
                    idx += 1;
                }
                times.insert(idx, t);
                keys += 1;
            }
            8 if keys > 0 => {
                let i = g.below(keys as u32);
                p.extend_from_slice(&[2.0, i as f32, g.uniform(-100.0, 100.0), g.uniform(-100.0, 100.0), g.uniform(-10.0, 10.0)]);
            }
            9 if keys > 0 => {
                let i = g.below(keys as u32);
                p.extend_from_slice(&[3.0, i as f32]);
                times.remove(i as usize);
                keys -= 1;
            }
            10 if g.below(8) == 0 => {
                p.push(4.0);
                times.clear();
                keys = 0;
            }
            11 => p.extend_from_slice(&[7.0, g.below(2) as f32]),
            12 => p.push(6.0),
            13 if keys > 0 => p.extend_from_slice(&[8.0, g.below(keys as u32) as f32]),
            _ if keys > 0 => {
                // Evaluate
                let t = match g.below(8) {
                    0 => times[0] - g.uniform(0.0, 3.0),
                    1 => times[keys - 1] + g.uniform(0.0, 3.0),
                    2 => times[g.below(keys as u32) as usize],
                    3 if keys > 1 => {
                        // a boundary of the straight-line part inside a segment: scan some fractions
                        let i = g.below(keys as u32 - 1) as usize;
                        times[i] + (times[i + 1] - times[i]) * (g.below(21) as f32 / 20.0)
                    }
                    _ => {
                        let lo = times[0];
                        let hi = times[keys - 1];
                        g.uniform(lo, hi.max(lo + 0.001))
                    }
                };
                p.extend_from_slice(&[5.0, t]);
            }
            _ => {}
        }
    }
    let _ = Model { times: Vec::new() };
    let _ = Prog { ops: Vec::new() };
    p
}

fn run_rust(prog: &[f32]) -> Vec<f32> {
    let mut out = Vec::new();
    let mut vc = VehicleCurveClass::with_radius(0.0);
    let mut i = 0;
    while i < prog.len() {
        let op = prog[i] as i32;
        i += 1;
        match op {
            0 => {
                vc = VehicleCurveClass::with_radius(prog[i]);
                i += 1;
            }
            1 => {
                out.push(vc.add_key(&Vector3::new(prog[i], prog[i + 1], prog[i + 2]), prog[i + 3]) as f32);
                i += 4;
            }
            2 => {
                vc.set_key(prog[i] as i32, &Vector3::new(prog[i + 1], prog[i + 2], prog[i + 3]));
                i += 4;
            }
            3 => {
                vc.remove_key(prog[i] as i32);
                i += 1;
            }
            4 => vc.clear_keys(),
            5 => {
                let v = vc.evaluate(prog[i]);
                i += 1;
                let (sh, sp) = vc.get_current_sharpness();
                out.extend_from_slice(&[v.x, v.y, v.z, sh, sp.x, sp.y, sp.z, vc.get_last_eval_time()]);
            }
            6 => {
                out.push(vc.curve.key_count() as f32);
                out.push(vc.curve.get_start_time());
                out.push(vc.curve.get_end_time());
                out.push(if vc.curve.is_looping() { 1.0 } else { 0.0 });
            }
            7 => {
                vc.curve.set_looping(prog[i] != 0.0);
                i += 1;
            }
            8 => {
                let (p, t) = vc.curve.get_key(prog[i] as i32);
                i += 1;
                out.extend_from_slice(&[p.x, p.y, p.z, t]);
            }
            _ => unreachable!(),
        }
    }
    out
}

#[test]
fn vehicle_curve_matches_the_cpp() {
    let mut g = Gen::new(0x215_0EC1);
    let mut c = Check::new("VehicleCurveClass programs (Evaluate, keys, queries)");
    let mut evals = 0u64;
    let mut cases_with_eval = 0u64;
    for _ in 0..cases(CASES) {
        let prog = gen_program(&mut g);
        let r = run_rust(&prog);
        let mut out = vec![0.0f32; prog.len() * 8 + 16];
        let n = unsafe { draft_vc_run(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr()) };
        assert_eq!(n as usize, r.len(), "program {:?}", prog);
        let n_eval = prog.windows(1).filter(|w| w[0] == 5.0).count() as u64;
        if n_eval > 0 {
            cases_with_eval += 1;
        }
        evals += (r.len() / 8) as u64;
        let pp = prog.clone();
        c.f32s(&r, &out[..n as usize], || format!("program {:?}", pp));
    }
    println!("programs {CASES}, with an Evaluate {cases_with_eval}, result groups about {evals}");
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {bad} mismatching outputs");
        assert!(bad > 0, "plant {} not caught", plant_id());
    } else {
        c.finish();
    }
}
