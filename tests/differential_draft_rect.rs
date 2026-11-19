// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dlogic) of the draft `src/drafts/ww3d/rect.rs` (`RectClass`) against upstream
//! `WWMath/rect.h`, included UNCHANGED by differential/wrap_draft_rect.cpp (header-only; `Vector2` is the upstream object
//! already linked).
//!
//! # Method
//!
//! 20,000 cases of every member (20 operations: both constructors, `Set`, `Width`, `Height`, `Center`, `Extent`, the four
//! corners, `Scale(float)`, `Scale(Vector2)`, `Inverse_Scale`, `Scale_Relative_Center`, `+=`/`-=` a `Vector2`, `Inflate`,
//! union `+=`, `Contains`, `Snap_To_Units`, `/=`, `*=`, `==` and `!=`), from a fixed seed. Rectangles are random, degenerate
//! (zero or negative width), sharing edges with the other rectangle or the point (so the `>=`/`<=` boundaries and the
//! `MIN`/`MAX` ties are reached), or drawn from the mixed generator with NaN, infinity and subnormals. Every output float is
//! compared by bits.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches, except `Snap_To_Units`: `(int)(x + 0.5f)` is undefined in C++ for NaN and out-of-range values (x86 gives
//! `INT_MIN`), and Rust's `as i32` saturates. I expect misses there on the mixed inputs, which are then excluded (below).
//!
//! # Excluded inputs
//!
//! `Snap_To_Units` only where all four `edge / unit + 0.5f` quotients are finite and within +-2^30 (the cast is defined).
//! `Scale_Relative_Center`, `Scale` and the rest have no excluded input.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::ww3d::rect::RectClass;
use rts_lockstep_engine::ww::vector2::Vector2;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dww3dl_rect_run(op: i32, p: *const f32, out: *mut f32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn r4(r: &RectClass) -> Vec<f32> {
    vec![r.left, r.top, r.right, r.bottom]
}

fn v2(v: &Vector2) -> Vec<f32> {
    vec![v.x, v.y]
}

const NAMES: [&str; 20] = [
    "RectClass(Vector2, Vector2)", "Set(l,t,r,b)", "Width", "Height", "Center", "Extent", "Upper_Left..Lower_Left", "Scale(float)",
    "Scale(Vector2)", "Inverse_Scale", "Scale_Relative_Center", "operator += (Vector2)", "operator -= (Vector2)", "Inflate",
    "operator += (RectClass)", "Contains", "Snap_To_Units", "operator /= (float)", "operator *= (float)", "operator ==, !=",
];

fn coord(g: &mut Gen, kind: u32) -> f32 {
    match kind {
        0 => g.mixed(),
        1 => (g.below(7) as f32) - 3.0,
        _ => g.uniform(-100.0, 100.0),
    }
}

#[test]
fn rect_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_13_02);
    let mut chk: Vec<Check> = NAMES.iter().map(|n| Check::new(&format!("RectClass::{n}"))).collect();
    let mut snap_excluded = 0u64;
    for case in 0..cases() {
        let kind = if g.below(12) == 0 { 0 } else if g.below(3) == 0 { 1 } else { 2 };
        let mut p = [0.0f32; 11];
        for v in p.iter_mut() {
            *v = coord(&mut g, kind);
        }
        // rectangle b shares edges with a; the point v sits on an edge of a
        if g.below(3) == 0 {
            p[4] = p[0];
        }
        if g.below(3) == 0 {
            p[7] = p[3];
        }
        if g.below(4) == 0 {
            p[6] = p[2];
        }
        match g.below(4) {
            0 => p[8] = p[0],
            1 => p[8] = p[2],
            _ => {}
        }
        match g.below(4) {
            0 => p[9] = p[1],
            1 => p[9] = p[3],
            _ => {}
        }
        if g.below(8) == 0 {
            p[10] = 0.0;
        }
        let a = RectClass::new(p[0], p[1], p[2], p[3]);
        let b = RectClass::new(p[4], p[5], p[6], p[7]);
        let v = Vector2::new(p[8], p[9]);
        let k = p[10];
        for op in 0..20usize {
            let mut out = [0.0f32; 8];
            let n = unsafe { ref_dww3dl_rect_run(op as i32, p.as_ptr(), out.as_mut_ptr()) } as usize;
            let mut x = a;
            let mut e: Vec<f32> = match op {
                0 => r4(&RectClass::from_corners(&Vector2::new(p[0], p[1]), &Vector2::new(p[2], p[3]))),
                1 => {
                    let mut r = RectClass::default();
                    r.set(p[0], p[1], p[2], p[3]);
                    r4(&r)
                }
                2 => vec![a.width()],
                3 => vec![a.height()],
                4 => v2(&a.center()),
                5 => v2(&a.extent()),
                6 => [v2(&a.upper_left()), v2(&a.lower_right()), v2(&a.upper_right()), v2(&a.lower_left())].concat(),
                7 => {
                    x.scale(k);
                    r4(&x)
                }
                8 => {
                    x.scale_v(&v);
                    r4(&x)
                }
                9 => {
                    x.inverse_scale(&v);
                    r4(&x)
                }
                10 => {
                    x.scale_relative_center(k);
                    r4(&x)
                }
                11 => {
                    x.translate(&v);
                    r4(&x)
                }
                12 => {
                    // operator -= (const Vector2 &): the draft has one `translate` (the `+=`); `-=` is translate of the negation
                    x.translate(&Vector2::new(-v.x, -v.y));
                    r4(&x)
                }
                13 => {
                    x.inflate(&v);
                    r4(&x)
                }
                14 => {
                    x.union_with(&b);
                    r4(&x)
                }
                15 => vec![if a.contains(&v) { 1.0 } else { 0.0 }],
                16 => {
                    let q = [p[0] / v.x + 0.5, p[2] / v.x + 0.5, p[1] / v.y + 0.5, p[3] / v.y + 0.5];
                    if q.iter().any(|t| !(t.abs() < 1.0e9)) {
                        snap_excluded += 1;
                        continue;
                    }
                    x.snap_to_units(&v);
                    r4(&x)
                }
                17 => {
                    x.div_scale(k);
                    r4(&x)
                }
                18 => {
                    x.scale(k);
                    r4(&x)
                }
                _ => vec![if a == b { 1.0 } else { 0.0 }, if a != b { 1.0 } else { 0.0 }],
            };
            assert_eq!(e.len(), n, "op {op}: output length");
            e.truncate(n);
            let what = || format!("case {case} op {op} p {p:?}");
            chk[op].f32s(&e, &out[..n], what);
        }
    }
    println!("Snap_To_Units inputs excluded (undefined cast): {snap_excluded}");
    finish(chk);
}
