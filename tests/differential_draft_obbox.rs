#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the draft src/drafts/wwlib/obbox.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/obbox.h and obbox.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `OBBoxClass` and its intersection tests against the C++ (#215).
//!
//! # Method
//!
//! `differential/wrap_draft_obbox.cpp` includes obbox.h and compiles obbox.cpp UNCHANGED. One stand-in: `WWMath::
//! Random_Float()` (wwmath.cpp:78, not compiled) returns the next of 16 values the test sets, and the Rust closure
//! draws the same values in the same order, so `Init_Random` is compared on its draw order too. Matrix3x3, Matrix3D and
//! Quaternion are the upstream objects (matrix3.cpp unchanged, matrix3d.cpp, quat.cpp); `Build_Matrix3`
//! is quat.cpp's. Operations (14): both constructors, `==`, `!=` (upstream's is `(C!=) || (E!= && B==)`),
//! `Init_From_Box_Points` (8 points: the corners of a rotated, scaled box, in shuffled order, plus random points), `Init_Random`,
//! `Project_To_Axis`, `Volume`, `Compute_Point`, `Compute_Axis_Aligned_Extent`, `Transform`, `Oriented_Boxes_Intersect`,
//! `Oriented_Boxes_Collide` and `Oriented_Box_Intersects_Tri` (the last three also exercise the `_On_Axis` helpers).
//! Boxes: a rotation basis (orthonormal to rounding) three quarters of the time, otherwise a random matrix; the second
//! box is placed near the first so that about half of the pairs overlap; identical boxes, touching boxes (equal to
//! rounding), zero and negative extents, and NaN/infinity are included. Outputs are compared by bits (the booleans as 0/1).
//! Not covered: `OBBoxClass(points, n)` (upstream `assert(0)` and an `#if 0` body; with NDEBUG it does nothing).
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! 0 mismatches. The place I expect one: `Oriented_Box_Intersects_Tri`, where upstream passes `A[i]` BY REFERENCE and the
//! `_On_Axis` function negates it, so the later cross products use the negated axis; the draft copies `A[i]`. A negated
//! axis gives an exactly negated cross product and the callee negates it back, so I predict no visible effect.
//!
//! # Results, plants
//!
//! see the comment on #215 and the table at the end of this comment block.

mod support;

use rts_lockstep_engine::drafts::wwlib::matrix3::Matrix3x3;
use rts_lockstep_engine::drafts::wwlib::obbox::{oriented_box_intersects_tri, oriented_boxes_collide, oriented_boxes_intersect, OBBoxClass};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::tri::TriClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn draft_obb_randoms(r: *const f32, n: i32);
    fn draft_obb(op: i32, p: *const f32, out: *mut f32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());
const CASES: usize = 40_000;

const NAMES: [&str; 14] = [
    "OBBox(center,extent)", "OBBox(center,extent,basis)", "OBBox ==", "OBBox !=", "Init_From_Box_Points", "Init_Random",
    "Project_To_Axis", "Volume", "Compute_Point", "Compute_Axis_Aligned_Extent", "OBBox::Transform",
    "Oriented_Boxes_Intersect", "Oriented_Boxes_Collide", "Oriented_Box_Intersects_Tri",
];

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn v3(a: &[f32]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}
fn basis(a: &[f32]) -> Matrix3x3 {
    Matrix3x3::from_elements(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8])
}
fn box_of(a: &[f32]) -> OBBoxClass {
    OBBoxClass::with_basis(&v3(&a[9..12]), &v3(&a[12..15]), &basis(&a[0..9]))
}
fn out_box(b: &OBBoxClass) -> Vec<f32> {
    let mut v = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            v.push(b.basis[i][j]);
        }
    }
    v.extend_from_slice(&[b.center.x, b.center.y, b.center.z, b.extent.x, b.extent.y, b.extent.z]);
    v
}

fn rotation(g: &mut Gen) -> [f32; 9] {
    let (a, b, c) = (g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0));
    let r = petir::Real::sin;
    let k = petir::Real::cos;
    let (sa, ca, sb, cb, sc, cc) = (r(a), k(a), r(b), k(b), r(c), k(c));
    [
        cb * cc, -cb * sc, sb,
        sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb,
        -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb,
    ]
}

fn extent(g: &mut Gen) -> [f32; 3] {
    match g.below(20) {
        0 => [g.mixed(), g.mixed(), g.mixed()],
        1 => [0.0, g.uniform(0.0, 3.0), g.uniform(0.0, 3.0)],
        2 => [g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0)],
        _ => [g.uniform(0.1, 5.0), g.uniform(0.1, 5.0), g.uniform(0.1, 5.0)],
    }
}

fn make_box(g: &mut Gen, center: [f32; 3]) -> [f32; 15] {
    let mut b = [0.0f32; 15];
    if g.below(4) == 0 {
        for x in b[0..9].iter_mut() {
            *x = g.uniform(-1.5, 1.5);
        }
    } else if g.below(8) == 0 {
        b[0] = 1.0;
        b[4] = 1.0;
        b[8] = 1.0;
    } else {
        b[0..9].copy_from_slice(&rotation(g));
    }
    b[9..12].copy_from_slice(&center);
    b[12..15].copy_from_slice(&extent(g));
    b
}

fn case(g: &mut Gen) -> [f32; 100] {
    let mut p = [0.0f32; 100];
    let c0 = [g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0)];
    let b0 = make_box(g, c0);
    let reach = g.uniform(0.5, 9.0);
    let c1 = [c0[0] + g.uniform(-reach, reach), c0[1] + g.uniform(-reach, reach), c0[2] + g.uniform(-reach, reach)];
    let mut b1 = make_box(g, c1);
    match g.below(12) {
        0 => b1 = b0, // identical (for == and !=, and total overlap)
        1 => {
            b1[0..9].copy_from_slice(&b0[0..9]); // same basis, differing elsewhere
        }
        2 => {
            b1[9..15].copy_from_slice(&b0[9..15]); // same center and extent, other basis
        }
        3 => {
            b1[12..15].copy_from_slice(&b0[12..15]);
        }
        _ => {}
    }
    p[0..15].copy_from_slice(&b0);
    p[15..30].copy_from_slice(&b1);
    for i in 30..36 {
        p[i] = g.uniform(-6.0, 6.0);
    }
    p[36] = if g.below(10) == 0 { g.mixed() } else { g.uniform(0.0, 2.0) };
    // an axis: unit, a box axis, or arbitrary
    let ax = match g.below(4) {
        0 => [b0[0], b0[3], b0[6]],
        1 => [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)],
        2 => [g.mixed(), g.mixed(), g.mixed()],
        _ => {
            let mut v = Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0));
            v.normalize();
            [v.x, v.y, v.z]
        }
    };
    p[37..40].copy_from_slice(&ax);
    // a triangle near the first box, with its true normal most of the time
    let near = |g: &mut Gen| [c0[0] + g.uniform(-8.0, 8.0), c0[1] + g.uniform(-8.0, 8.0), c0[2] + g.uniform(-8.0, 8.0)];
    let (t0, t1, t2) = (near(g), near(g), near(g));
    p[40..43].copy_from_slice(&t0);
    p[43..46].copy_from_slice(&t1);
    p[46..49].copy_from_slice(&t2);
    let mut tri = TriClass { n: Vector3::new(0.0, 0.0, 0.0), v: [v3(&t0), v3(&t1), v3(&t2)] };
    if g.below(10) == 0 {
        tri.n = Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0));
    } else {
        tri.compute_normal();
    }
    p[49] = tri.n.x;
    p[50] = tri.n.y;
    p[51] = tri.n.z;
    // 8 points: the corners of a rotated box, shuffled, or random
    if g.below(5) == 0 {
        for i in 52..76 {
            p[i] = g.uniform(-10.0, 10.0);
        }
    } else {
        let r = rotation(g);
        let e = [g.uniform(0.2, 4.0), g.uniform(0.2, 4.0), g.uniform(0.2, 4.0)];
        let c = [g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0)];
        let mut corners = [[0.0f32; 3]; 8];
        for k in 0..8 {
            let s = [if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 }];
            for i in 0..3 {
                corners[k][i] = c[i] + r[i * 3] * e[0] * s[0] + r[i * 3 + 1] * e[1] * s[1] + r[i * 3 + 2] * e[2] * s[2];
            }
        }
        // shuffle
        for i in (1..8).rev() {
            let j = g.below(i as u32 + 1) as usize;
            corners.swap(i, j);
        }
        for k in 0..8 {
            p[52 + k * 3..55 + k * 3].copy_from_slice(&corners[k]);
        }
    }
    for i in 76..79 {
        p[i] = if g.below(2) == 0 { [-1.0, 1.0, 0.0, 0.5][g.below(4) as usize] } else { g.uniform(-1.5, 1.5) };
    }
    // a Matrix3D: a rotation and translation, or random
    if g.below(3) == 0 {
        for i in 79..91 {
            p[i] = g.geom();
        }
    } else {
        let r = rotation(g);
        for i in 0..3 {
            p[79 + i * 4..82 + i * 4].copy_from_slice(&r[i * 3..i * 3 + 3]);
            p[82 + i * 4] = g.uniform(-30.0, 30.0);
        }
    }
    p[91] = g.uniform(0.0, 1.0);
    p[92] = p[91] + g.uniform(0.0, 3.0);
    if g.below(10) == 0 {
        p[91] = g.mixed();
        p[92] = g.mixed();
    }
    p
}

fn rust_op(op: usize, p: &[f32; 100], rnd: &[f32; 16]) -> Vec<f32> {
    let b0 = box_of(&p[0..15]);
    let b1 = box_of(&p[15..30]);
    let v0 = v3(&p[30..33]);
    let v1 = v3(&p[33..36]);
    let axis = v3(&p[37..40]);
    let b = |x: bool| vec![if x { 1.0 } else { 0.0 }];
    match op {
        0 => out_box(&OBBoxClass::new(&v3(&p[9..12]), &v3(&p[12..15]))),
        1 => out_box(&b0),
        2 => b(b0.eq_box(&b1)),
        3 => b(b0.ne_box(&b1)),
        4 => {
            let pts: Vec<Vector3> = (0..8).map(|i| v3(&p[52 + i * 3..55 + i * 3])).collect();
            let mut x = b0;
            x.init_from_box_points(&pts);
            out_box(&x)
        }
        5 => {
            let mut i = 0usize;
            let mut x = b0;
            x.init_random(p[91], p[92], &mut || {
                let r = rnd[i & 15];
                i += 1;
                r
            });
            out_box(&x)
        }
        6 => vec![b0.project_to_axis(&axis)],
        7 => vec![b0.volume()],
        8 => {
            let r = b0.compute_point(&[p[76], p[77], p[78]]);
            vec![r.x, r.y, r.z]
        }
        9 => {
            let r = b0.compute_axis_aligned_extent();
            vec![r.x, r.y, r.z]
        }
        10 => {
            let mut a = [0.0f32; 12];
            a.copy_from_slice(&p[79..91]);
            out_box(&OBBoxClass::transform(&Matrix3D::from_array(a), &b0))
        }
        11 => b(oriented_boxes_intersect(&b0, &b1)),
        12 => b(oriented_boxes_collide(&b0, &v0, &b1, &v1, p[36])),
        13 => {
            let tri = TriClass { n: v3(&p[49..52]), v: [v3(&p[40..43]), v3(&p[43..46]), v3(&p[46..49])] };
            b(oriented_box_intersects_tri(&b0, &tri))
        }
        _ => unreachable!(),
    }
}

#[test]
fn obbox_matches_the_cpp() {
    let mut g = Gen::new(0x215_0BB0);
    let mut checks: Vec<Check> = NAMES.iter().map(|n| Check::new(n)).collect();
    let mut trues = [0u64; 14];
    for _ in 0..CASES {
        let p = case(&mut g);
        let mut rnd = [0.0f32; 16];
        for r in rnd.iter_mut() {
            *r = (g.below(0x1000) as f32) / 4095.0;
        }
        let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
        for op in 0..14 {
            unsafe { draft_obb_randoms(rnd.as_ptr(), 16) };
            let r = rust_op(op, &p, &rnd);
            let mut out = [0.0f32; 40];
            let n = unsafe { draft_obb(op as i32, p.as_ptr(), out.as_mut_ptr()) };
            assert_eq!(n as usize, r.len(), "op {op}");
            if r.len() == 1 && r[0] == 1.0 {
                trues[op] += 1;
            }
            let pp = &p;
            checks[op].f32s(&r, &out[..n as usize], || format!("p = {:?}", &pp[..]));
        }
    }
    println!("booleans true: ==:{} !=:{} intersect:{} collide:{} tri:{} of {CASES}", trues[2], trues[3], trues[11], trues[12], trues[13]);
    let planted = plant_id() != 0;
    let mut bad = 0u64;
    for c in checks {
        if planted {
            bad += c.report();
        } else {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED {} mismatching outputs", bad);
        assert!(bad > 0, "plant {} not caught", plant_id());
    }
}
