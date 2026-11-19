// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/coltest.rs` (`AABoxCollisionTestClass`,
//! `OBBoxCollisionTestClass`) against upstream `WW3D2/coltest.cpp` and `coltest.h`, compiled UNCHANGED
//! (differential/wrap_draft_coltest.cpp; NDEBUG, so `Rotate`'s debug cross-check is out as in the shipped game). Matrix3D,
//! OBBoxClass and AABoxClass are the upstream objects already linked. No stand-ins.
//!
//! # Method
//!
//! 20,000 cases per operation from a fixed seed (`DRAFT_CASES` overrides), 11 operations: both box tests' constructors,
//! `Cull(box)`, `Cull(min, max)`, `Translate`, `Rotate` (all four rotation types), `Transform`, the two transformed
//! constructors of `OBBoxCollisionTestClass` (from an OBB test and from an AABox test), and the collision type each carries.
//! Boxes and moves are geometry-scale values (a share of `mixed`, with NaN, infinity and subnormals); the Matrix3D is a
//! random rotation with translation, the four exact quarter-turns, the identity, or a random 3x4; `Cull` boxes are placed
//! so that a face of the box lies exactly on the sweep box (the `>` and `<` boundaries), and the box extents are sometimes
//! zero or negative. Every output float is compared by bits.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. The two places I would look first: `AABoxCollisionTestClass::Transform`, which uses
//! `Transform_Center_Extent_AABox` and `mulVector3Array` (draft Matrix3D methods tested elsewhere), and
//! `OBBoxCollisionTestClass(AABox test, tm)`, where upstream assigns `Box.Basis = tm`.
//!
//! # Not covered
//!
//! `Cast_To_Triangle` of both classes (CollisionMath, not in the draft) and `RayCollisionTestClass` (reused from the port).
//! `CastResultStruct *Result` is a pointer shared by the tests; the draft shares an `Arc`, which has no C++ output to compare.
//!
//! # Results (2026-11-19)
//!
//! **0 mismatches on the first run**: 20,000 cases of each of the 11 operations (320,000 to 500,000 words each for the state
//! outputs), max ULP 0, 18 NaN-payload-only words in `OBBoxCollisionTestClass(OBB test, tm)`. **Prediction held.**
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching output words, all operations it affects)
//!   14679: 294,105; 14680: 1,116; 14681: 1,104; 14682: 57,825
//!   14683: 5,006; 14684: 5,054; 14685: 4,993; 14686: 43,799
//!   14687: 115,739; 14688: 70,354; 14689: 29,439; 14690: 95,011
//!   14691: 1,402
//!
//! **13 of 13 caught.**

mod support;

use rts_lockstep_engine::client::pick_drawable::CastResult;
use rts_lockstep_engine::drafts::ww3d::coltest::*;
use rts_lockstep_engine::formats::w3d::segline::AABox;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::{Arc, Mutex, RwLock};
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_coltest(op: i32, p: *const f32, out: *mut f32) -> i32;
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

fn v3(a: &[f32]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn res() -> Arc<RwLock<CastResult>> {
    Arc::new(RwLock::new(CastResult::default()))
}

fn out_a(t: &AABoxCollisionTestClass) -> Vec<f32> {
    vec![
        t.box_.center.x, t.box_.center.y, t.box_.center.z, t.box_.extent.x, t.box_.extent.y, t.box_.extent.z, t.move_.x, t.move_.y, t.move_.z,
        t.sweep_min.x, t.sweep_min.y, t.sweep_min.z, t.sweep_max.x, t.sweep_max.y, t.sweep_max.z, t.base.collision_type as f32,
    ]
}

fn out_o(t: &OBBoxCollisionTestClass) -> Vec<f32> {
    let mut v = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            v.push(t.box_.basis[i][j]);
        }
    }
    v.extend_from_slice(&[
        t.box_.center.x, t.box_.center.y, t.box_.center.z, t.box_.extent.x, t.box_.extent.y, t.box_.extent.z, t.move_.x, t.move_.y, t.move_.z,
        t.sweep_min.x, t.sweep_min.y, t.sweep_min.z, t.sweep_max.x, t.sweep_max.y, t.sweep_max.z, t.base.collision_type as f32,
    ]);
    v
}

fn val(g: &mut Gen, wild: bool) -> f32 {
    if wild { g.mixed() } else if g.below(6) == 0 { (g.below(9) as f32) - 4.0 } else { g.uniform(-30.0, 30.0) }
}

fn ext(g: &mut Gen, wild: bool) -> f32 {
    if wild { g.mixed() } else {
        match g.below(10) {
            0 => 0.0,
            1 => -g.uniform(0.0, 3.0),
            2 => (g.below(5) as f32) * 0.5,
            _ => g.uniform(0.0, 8.0),
        }
    }
}

fn matrix(g: &mut Gen, wild: bool) -> [f32; 12] {
    match g.below(8) {
        0 => [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        1 => [0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        2 => [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        3 => [0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        4 | 5 => {
            // a rotation with a translation
            let (a, b, c) = (g.uniform(-3.2, 3.2), g.uniform(-3.2, 3.2), g.uniform(-3.2, 3.2));
            let (sa, ca, sb, cb, sc, cc) = (a.sin(), a.cos(), b.sin(), b.cos(), c.sin(), c.cos());
            [
                cb * cc, -cb * sc, sb, g.uniform(-10.0, 10.0),
                sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb, g.uniform(-10.0, 10.0),
                -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb, g.uniform(-10.0, 10.0),
            ]
        }
        _ => {
            let mut m = [0.0f32; 12];
            for x in m.iter_mut() {
                *x = if wild { g.mixed() } else { g.uniform(-2.0, 2.0) };
            }
            m
        }
    }
}

const NAMES: [&str; 11] = [
    "AABoxCollisionTestClass(box, move, res, type)", "AABoxCollisionTestClass::Cull(box)", "AABoxCollisionTestClass::Cull(min, max)",
    "AABoxCollisionTestClass::Translate", "AABoxCollisionTestClass::Rotate", "AABoxCollisionTestClass::Transform",
    "OBBoxCollisionTestClass(obbox, move, res, type)", "OBBoxCollisionTestClass(OBB test, tm)", "OBBoxCollisionTestClass(AABox test, tm)",
    "OBBoxCollisionTestClass::Cull(box)", "OBBoxCollisionTestClass::Cull(min, max)",
];

#[test]
fn coltest_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_14_02);
    let mut chk: Vec<Check> = NAMES.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let wild = g.below(16) == 0;
        let mut p = [0.0f32; 53];
        for i in 0..3 {
            p[i] = val(&mut g, wild); // center
            p[3 + i] = ext(&mut g, wild); // extent
            p[6 + i] = if g.below(6) == 0 { 0.0 } else { val(&mut g, wild) }; // move
            p[48 + i] = val(&mut g, wild); // translation
        }
        let m = matrix(&mut g, wild);
        p[21..33].copy_from_slice(&m);
        // an OBB: a rotation basis or a random one
        let ob = matrix(&mut g, wild);
        for i in 0..3 {
            for j in 0..3 {
                p[33 + i * 3 + j] = ob[i * 4 + j];
            }
        }
        for i in 0..3 {
            p[42 + i] = val(&mut g, wild);
            p[45 + i] = ext(&mut g, wild);
        }
        p[51] = g.below(4) as f32;
        p[52] = if g.below(3) == 0 { 0x02 as f32 } else { g.below(0x10000) as f32 };

        let abox = AABox { center: v3(&p[0..3]), extent: v3(&p[3..6]) };
        let mv = v3(&p[6..9]);
        let at = AABoxCollisionTestClass::new(&abox, &mv, res(), p[52] as i32);
        let obb = ObBoxClass {
            center: v3(&p[42..45]),
            extent: v3(&p[45..48]),
            basis: [[p[33], p[34], p[35]], [p[36], p[37], p[38]], [p[39], p[40], p[41]]],
        };
        let ot = OBBoxCollisionTestClass::new(&obb, &mv, res(), p[52] as i32);
        let tm = Matrix3D::from_array(m);
        // the second box (Cull): near the sweep box, with faces on its faces half the time
        let (smin, smax) = if case % 2 == 0 { (at.sweep_min, at.sweep_max) } else { (ot.sweep_min, ot.sweep_max) };
        let mut c2 = [val(&mut g, wild), val(&mut g, wild), val(&mut g, wild)];
        let mut e2 = [ext(&mut g, wild), ext(&mut g, wild), ext(&mut g, wild)];
        for k in 0..3 {
            let (lo, hi) = ([smin.x, smin.y, smin.z][k], [smax.x, smax.y, smax.z][k]);
            match g.below(6) {
                0 => c2[k] = hi + e2[k], // min face on the sweep max
                1 => c2[k] = lo - e2[k], // max face on the sweep min
                2 => {
                    c2[k] = (lo + hi) * 0.5;
                    e2[k] = 0.0;
                }
                _ => {}
            }
        }
        p[9..12].copy_from_slice(&c2);
        p[12..15].copy_from_slice(&e2);
        let b2 = AABox { center: v3(&c2), extent: v3(&e2) };
        let mn = Vector3::new(c2[0] - e2[0], c2[1] - e2[1], c2[2] - e2[2]);
        let mx = Vector3::new(c2[0] + e2[0], c2[1] + e2[1], c2[2] + e2[2]);
        p[15..18].copy_from_slice(&[mn.x, mn.y, mn.z]);
        p[18..21].copy_from_slice(&[mx.x, mx.y, mx.z]);

        for op in 0..11usize {
            let mut out = [0.0f32; 32];
            let n = unsafe { ref_dpw_coltest(op as i32, p.as_ptr(), out.as_mut_ptr()) } as usize;
            let cpp = &out[..n];
            let rust: Vec<f32> = match op {
                0 => out_a(&at),
                1 => vec![f32::from(at.cull(&b2))],
                2 => vec![f32::from(at.cull_min_max(&mn, &mx))],
                3 => {
                    let mut t = at.clone();
                    t.translate(&v3(&p[48..51]));
                    out_a(&t)
                }
                4 => {
                    let mut t = at.clone();
                    let r = match p[51] as i32 {
                        0 => RotationType::RotateNone,
                        1 => RotationType::RotateZ90,
                        2 => RotationType::RotateZ180,
                        _ => RotationType::RotateZ270,
                    };
                    t.rotate(r);
                    out_a(&t)
                }
                5 => {
                    let mut t = at.clone();
                    t.transform(&tm);
                    out_a(&t)
                }
                6 => out_o(&ot),
                7 => out_o(&OBBoxCollisionTestClass::from_obbox_test_transformed(&ot, &tm)),
                8 => out_o(&OBBoxCollisionTestClass::from_aabox_test_transformed(&at, &tm)),
                9 => vec![f32::from(ot.cull(&b2))],
                _ => vec![f32::from(ot.cull_min_max(&mn, &mx))],
            };
            assert_eq!(rust.len(), cpp.len(), "{}: output length", NAMES[op]);
            chk[op].f32s(&rust, cpp, || format!("op {op} p {p:?}"));
        }
    }
    finish(chk);
}
