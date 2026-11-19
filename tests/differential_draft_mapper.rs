// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of the draft `src/drafts/sweep_ww3d/mapper.rs`
//! (`GridTextureMapperClass::Set_Frame_Per_Second`, `initialize`, `update_temporal_state`, `calculate_uv_offset`) against
//! upstream `WW3D2/mapper.cpp` cut VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_mapper.cpp). Stand-ins:
//! the class's fields (mapper.h, unchanged) without the `TextureMapperClass` base, and `WW3D::Get_Sync_Time()`, which returns a
//! value the test sets before each call (the draft takes the time as an argument).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides): a frame rate (zero, positive, negative, exact `1000/n` values
//! whose f32 and f64 quotients differ, and random ones), a grid side `2^log2` (log2 0 to 6), `LastFrame` (0 for the default
//! `side^2`, or any value including ones above `side^2`), `Offset` (above and below `LastFrame`), a start time near 0 or near
//! the `u32` wrap, then 1 to 12 operations: `Set_Frame_Per_Second`, `update_temporal_state` (time steps from 0 to a few
//! seconds, sometimes a jump backwards, which wraps), `calculate_uv_offset`. After every operation all nine state words are
//! compared, so the constructor's `initialize` call is covered too.
//!
//! # Excluded inputs
//!
//! `|fps| > 1000` gives `MSPerFrame == 0` and `Remainder / MSPerFrame` divides by zero (undefined in C++, a panic in the draft);
//! `|fps| < 0.001` makes `1000 / |fps|` exceed `u32::MAX` (the cast is undefined in C++); `log2 > 15` overflows `grid_width^2`.
//! None is generated. `fps` NaN and infinity are not generated either.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! One MISS expected: `MSPerFrame = (unsigned int)(1000.0f / fabs(fps))`: `fabs` takes a `float` and returns a `double` here
//! (C `fabs`, the shim's math.h keeps the C++ overloads out), so the quotient is a DOUBLE division; the draft divides in `f32`,
//! which differs at truncation boundaries (fps = `1000/n` rounded to f32). Everything else 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::mapper::GridTextureMapperClass;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, feq, finish};

extern "C" {
    fn ref_dsww3d_map_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = unsafe { ref_dsww3d_map_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn fps(g: &mut Gen) -> f32 {
    let sign = if g.below(2) == 0 { 1.0 } else { -1.0 };
    match g.below(10) {
        0 => 0.0,
        1 | 2 => sign * (1000.0 / (1 + g.below(1000)) as f32),
        3 => sign * [1.0, 15.0, 30.0, 60.0, 1000.0, 0.5, 7.0, 24.0][g.below(8) as usize],
        4 => sign * g.uniform(0.001, 5.0),
        _ => sign * g.uniform(0.5, 1000.0),
    }
}

fn state(m: &GridTextureMapperClass) -> Vec<u32> {
    vec![m.sign as u32, m.ms_per_frame, m.oo_grid_width.to_bits(), m.grid_width_log2, m.last_frame, m.offset, m.remainder, m.current_frame, m.last_used_sync_time]
}

#[test]
fn grid_mapper_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0a01);
    let names = ["initialize (constructor path)", "Set_Frame_Per_Second", "update_temporal_state", "calculate_uv_offset"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let f = fps(&mut g);
        let log2 = g.below(7);
        let side2 = 1u32 << (2 * log2);
        let last = if g.below(3) == 0 { 0 } else { 1 + g.below(side2 + 10) };
        let eff_last = if last == 0 { side2 } else { last };
        let offset = if g.below(3) == 0 { g.below(3 * eff_last + 1) } else { g.below(eff_last) };
        let mut sync = if g.below(3) == 0 { u32::MAX - g.below(3000) } else { g.below(100000) };
        let mut inp = vec![f.to_bits(), log2, last, offset, sync];
        let mut m = GridTextureMapperClass { last_frame: last, offset, ..Default::default() };
        m.initialize(f, log2, sync);
        let mut rust: Vec<(usize, Vec<u32>)> = vec![(0, state(&m))];
        let nops = 1 + g.below(12);
        inp.push(nops);
        for _ in 0..nops {
            match g.below(4) {
                0 => {
                    let nf = fps(&mut g);
                    sync = sync.wrapping_add(g.below(2000));
                    inp.extend([0, nf.to_bits(), sync]);
                    m.set_frame_per_second(nf, sync);
                    rust.push((1, state(&m)));
                }
                1 | 2 => {
                    sync = match g.below(8) {
                        0 => sync,
                        1 => sync.wrapping_sub(g.below(5000)),
                        2 => sync.wrapping_add(g.below(4_000_000)),
                        _ => sync.wrapping_add(g.below(3000)),
                    };
                    inp.extend([1, sync]);
                    m.update_temporal_state(sync);
                    rust.push((2, state(&m)));
                }
                _ => {
                    inp.push(2);
                    let (u, v) = m.calculate_uv_offset();
                    let mut w = vec![u.to_bits(), v.to_bits()];
                    w.extend(state(&m));
                    rust.push((3, w));
                }
            }
        }
        let cpp = run(&inp);
        let mut at = 0;
        for (k, w) in rust {
            let end = (at + w.len()).min(cpp.len());
            feq(&mut c[k], w, cpp[at..end].to_vec(), || format!("case {case} fps {f} log2 {log2} last {last} offset {offset}"));
            at = end;
        }
        if at != cpp.len() {
            c[0].eq(at, cpp.len(), || format!("case {case}: output length"));
        }
    }
    finish(c);
}
