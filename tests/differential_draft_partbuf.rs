// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of the draft `src/drafts/sweep_ww3d/part_buf.rs`
//! (`ParticleBufferClass::Get_Color_Key_Frames`, `Get_Opacity_Key_Frames`, `Get_Size_Key_Frames`, `Get_Rotation_Key_Frames`,
//! `Get_Frame_Key_Frames`, `Get_Blur_Time_Key_Frames`, `Get_Line_Texture_Mapping_Mode`, `Get_Subdivision_Level`) against upstream
//! `WW3D2/part_buf.cpp` cut VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_partbuf.cpp). Stand-ins: the buffer's
//! key-frame fields (part_buf.h's types, `unsigned int` times and `MaxAge`), `ParticlePropertyStruct<T>` (part_emt.h, copied),
//! and a line renderer holding a mapping mode and a subdivision level.
//!
//! # Method
//!
//! 20,000 cases per kind (fixed seed, `DRAFT_CASES` overrides): 1 to 6 key frames (times ascending from 0, in milliseconds;
//! sometimes beyond `MaxAge`, and `MaxAge` and times above 2^24 so that `float` conversion rounds), values and deltas of every
//! sign (the last delta zero about a third of the time, and deltas absent), random `Rand` values. Output: `Start`, `Rand`,
//! `NumKeyFrames`, the key times, the values. The two line-renderer queries are tried with and without a renderer.
//!
//! # Excluded inputs
//!
//! `NumXKeyFrames == 0` reads `XKeyFrameValues[0]` and `XKeyFrameDeltas[-1]` (a NULL / out-of-bounds read upstream): except for
//! rotation without values (the one case upstream guards: `RotationKeyFrameValues ? ... : 0`, with no deltas), it is not generated.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! One MISS found while reading, fixed in the draft before the first run: `MaxAge` and the key times are `unsigned int`, and
//! `float time_delta = MaxAge - XKeyFrameTimes[index - 1]` is an UNSIGNED subtraction (wrapping when a key time exceeds `MaxAge`)
//! converted to float; the draft took `MaxAge` as `f32` and subtracted in float. The generator reaches both differences (times
//! beyond `MaxAge`; values above 2^24). Plant 13895 (a saturating subtraction) is the probe for the wrap. Everything else 0
//! mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::part_buf::*;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{b, cases, feq, finish};

extern "C" {
    fn ref_dsww3d_pb_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = unsafe { ref_dsww3d_pb_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn val(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => g.special(),
        1 => 0.0,
        2 => (g.below(5) as f32) - 2.0,
        _ => g.uniform(-5.0, 5.0),
    }
}

fn flat<const N: usize>(p: &ParticleProperty<[f32; N]>) -> Vec<u32> {
    let mut w: Vec<u32> = b(&p.start);
    w.extend(b(&p.rand));
    w.push(p.num_key_frames as u32);
    w.extend(b(&p.key_times));
    for v in &p.values {
        w.extend(b(v));
    }
    w
}

fn flat1(p: &ParticleProperty<f32>) -> Vec<u32> {
    let mut w = vec![p.start.to_bits(), p.rand.to_bits(), p.num_key_frames as u32];
    w.extend(b(&p.key_times));
    w.extend(b(&p.values));
    w
}

#[test]
fn particle_key_frames_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0b01);
    let names = ["Get_Color_Key_Frames", "Get_Opacity_Key_Frames", "Get_Size_Key_Frames", "Get_Rotation_Key_Frames", "Get_Frame_Key_Frames", "Get_Blur_Time_Key_Frames", "line renderer queries"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        for kind in 0..7usize {
            if kind == 6 {
                let has = g.below(2);
                let (mode, level) = (g.below(3), g.below(6));
                let cpp = run(&[6, has, mode, level]);
                let rust = vec![get_line_texture_mapping_mode((has != 0).then_some(mode as i32)) as u32, get_subdivision_level((has != 0).then_some(level as i32)) as u32];
                feq(&mut c[6], rust, cpp, || format!("case {case}"));
                continue;
            }
            let rotation_empty = kind == 3 && g.below(8) == 0;
            let n = if rotation_empty { 0 } else { 1 + g.below(6) as usize };
            let big = g.below(6) == 0;
            let mut times = vec![0u32];
            for _ in 1..n {
                let step = if big { 1 + g.below(9_000_000) } else { 1 + g.below(3000) };
                times.push(times.last().unwrap() + step);
            }
            times.truncate(n);
            let last_t = times.last().copied().unwrap_or(0);
            let max_age = match g.below(5) {
                0 => g.below(last_t.max(1)), // below the last key time: the subtraction wraps
                1 => last_t,
                2 => last_t + g.below(40_000_000),
                _ => last_t + g.below(5000),
            };
            let dim = if kind == 0 { 3 } else { 1 };
            let values: Vec<f32> = (0..n * dim).map(|_| val(&mut g)).collect();
            let has_deltas = !rotation_empty && g.below(5) != 0;
            let mut deltas: Vec<f32> = (0..n * dim).map(|_| val(&mut g)).collect();
            if has_deltas && n > 0 && g.below(3) == 0 {
                for d in 0..dim {
                    deltas[(n - 1) * dim + d] = 0.0;
                }
            }
            if has_deltas && kind == 0 && n > 0 && g.below(4) == 0 {
                // only one colour component of the last delta nonzero
                let k = g.below(3) as usize;
                for d in 0..3 {
                    deltas[(n - 1) * 3 + d] = if d == k { val(&mut g) } else { 0.0 };
                }
            }
            let rnd: Vec<f32> = (0..dim).map(|_| val(&mut g)).collect();
            let mut inp = vec![kind as u32, n as u32, u32::from(has_deltas), u32::from(!rotation_empty), max_age];
            inp.extend(b(&rnd));
            inp.extend(times.iter().copied());
            inp.extend(b(&values));
            inp.extend(b(&deltas));
            let cpp = run(&inp);
            let rust = if kind == 0 {
                let v3: Vec<[f32; 3]> = values.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
                let d3: Vec<[f32; 3]> = deltas.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
                flat(&get_color_key_frames(&times, &v3, has_deltas.then_some(&d3[..]), [rnd[0], rnd[1], rnd[2]], max_age))
            } else {
                let d = has_deltas.then_some(&deltas[..]);
                let r = rnd[0];
                flat1(&match kind {
                    1 => get_opacity_key_frames(&times, &values, d, r, max_age),
                    2 => get_size_key_frames(&times, &values, d, r, max_age),
                    3 => get_rotation_key_frames(&times, if rotation_empty { &[] } else { &values }, d, r, max_age),
                    4 => get_frame_key_frames(&times, &values, d, r, max_age),
                    _ => get_blur_time_key_frames(&times, &values, d, r, max_age),
                })
            };
            feq(&mut c[kind], rust, cpp, || format!("case {case}: n {n} max_age {max_age} times {times:?}"));
        }
    }
    finish(c);
}
