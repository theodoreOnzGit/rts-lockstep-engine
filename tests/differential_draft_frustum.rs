#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the draft src/drafts/wwlib/frustum.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/frustum.h and frustum.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `FrustumClass::Init` against the C++ (#215).
//!
//! # Method
//!
//! `differential/wrap_draft_frustum.cpp` includes frustum.h and compiles frustum.cpp UNCHANGED (no cut, no stand-in;
//! `PlaneClass::Set(p1,p2,p3)` is plane.h's inline, and `Matrix3D`, `Vector2/3` the upstream ones). Each case is one
//! `Init(camera, vpmin, vpmax, znear, zfar)`; all 66 output floats (the stored camera, six planes as N and D, eight
//! corners, the bound box) are compared by bits. Cameras: a look-at-style rotation with a translation (60 %), the same
//! with one axis negated so the matrix is REFLECTED (the `Dot(Z, X x Y) < 0` branch; 20 %), random rows (10 %) and the
//! full mixed generator (10 %). Viewport extents: symmetric and asymmetric, including inverted ones. Near/far:
//! positive (so they are negated), already negative, mixed signs, zero, and `mixed`.
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! 0 mismatches: the draft follows the C++ statement by statement, and `Corners[i] *= znear` is `Vector3 *= float` on
//! both sides. The one thing to watch: upstream's `Matrix3D::Transform_Vector(tm, Corners[i], &Corners[i])` is aliased,
//! the draft's is by value.
//!
//! # Results, plants
//!
//! see the comment on #215 and the planted bug table at the end of this file's comment block.

mod support;

use rts_lockstep_engine::drafts::wwlib::frustum::FrustumClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use support::{Check, Gen};

extern "C" {
    fn draft_frustum_init(p: *const f32, out: *mut f32) -> i32;
}

const CASES: usize = 60_000;

/// `DRAFT_CASES=<n>` overrides the number of cases (the planted-bug runs use fewer than the gated run).
fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn camera(g: &mut Gen) -> [f32; 12] {
    match g.below(12) {
        10 => {
            // degenerate: Z perpendicular to X cross Y (a zero or repeated column), on small integers
            let mut m = [0.0f32; 12];
            for x in m.iter_mut() {
                *x = (g.below(3) as i32 - 1) as f32;
            }
            let col = g.below(3) as usize;
            let src = g.below(3) as usize;
            for r in 0..3 {
                m[r * 4 + col] = if g.below(2) == 0 { 0.0 } else { m[r * 4 + src] };
            }
            m
        }
        0 => g.vec::<12>(Gen::geom),
        1 => g.vec::<12>(Gen::mixed),
        2 | 3 => {
            let mut m = camera_rot(g);
            // reflect one axis
            let col = g.below(3) as usize;
            for r in 0..3 {
                m[r * 4 + col] = -m[r * 4 + col];
            }
            m
        }
        _ => camera_rot(g),
    }
}

fn camera_rot(g: &mut Gen) -> [f32; 12] {
    let (a, b, c) = (g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0));
    let (sa, ca, sb, cb, sc, cc) = (petir::Real::sin(a), petir::Real::cos(a), petir::Real::sin(b), petir::Real::cos(b), petir::Real::sin(c), petir::Real::cos(c));
    let r = [
        cb * cc, -cb * sc, sb,
        sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb,
        -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb,
    ];
    let mut m = [0.0f32; 12];
    for i in 0..3 {
        m[i * 4..i * 4 + 3].copy_from_slice(&r[i * 3..i * 3 + 3]);
        m[i * 4 + 3] = g.uniform(-200.0, 200.0);
    }
    m
}

fn plane_dist(g: &mut Gen) -> f32 {
    match g.below(12) {
        0 => g.mixed(),
        1 => 0.0,
        2 => -g.uniform(0.1, 100.0),
        _ => g.uniform(0.1, 1000.0),
    }
}

#[test]
fn frustum_init_matches_the_cpp() {
    let mut g = Gen::new(0x215_F005);
    let mut c = Check::new("FrustumClass::Init (66 floats)");
    let mut reflected = 0u32;
    for _ in 0..cases(CASES) {
        let mut p = [0.0f32; 18];
        p[0..12].copy_from_slice(&camera(&mut g));
        let (hx, hy) = (g.uniform(0.05, 2.0), g.uniform(0.05, 2.0));
        match g.below(8) {
            0 => {
                p[12] = g.mixed();
                p[13] = g.mixed();
                p[14] = g.mixed();
                p[15] = g.mixed();
            }
            1 => {
                p[12] = g.uniform(-1.0, 1.0);
                p[13] = g.uniform(-1.0, 1.0);
                p[14] = g.uniform(-1.0, 1.0);
                p[15] = g.uniform(-1.0, 1.0);
            }
            _ => {
                p[12] = -hx;
                p[13] = -hy;
                p[14] = hx;
                p[15] = hy;
            }
        }
        p[16] = plane_dist(&mut g);
        p[17] = plane_dist(&mut g);
        if g.below(3) == 0 {
            p[17] = p[16] * g.uniform(2.0, 50.0);
        }
        let mut cam = [0.0f32; 12];
        cam.copy_from_slice(&p[0..12]);
        let mut f = FrustumClass::default();
        f.init(&Matrix3D::from_array(cam), &Vector2::new(p[12], p[13]), &Vector2::new(p[14], p[15]), p[16], p[17]);
        let mut r = Vec::new();
        for i in 0..3 {
            for j in 0..4 {
                r.push(f.camera_transform[i][j]);
            }
        }
        for pl in &f.planes {
            r.extend_from_slice(&[pl.n.x, pl.n.y, pl.n.z, pl.d]);
        }
        for q in &f.corners {
            r.extend_from_slice(&[q.x, q.y, q.z]);
        }
        r.extend_from_slice(&[f.bound_min.x, f.bound_min.y, f.bound_min.z, f.bound_max.x, f.bound_max.y, f.bound_max.z]);
        let mut out = [0.0f32; 80];
        let n = unsafe { draft_frustum_init(p.as_ptr(), out.as_mut_ptr()) };
        assert_eq!(n as usize, r.len());
        let (x, y, z) = (cam[0], cam[1], cam[2]);
        let _ = (x, y, z);
        reflected += 1;
        let pp = p;
        c.f32s(&r, &out[..n as usize], || format!("p = {:?}", pp));
    }
    let _ = reflected;
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {bad} mismatching outputs");
        assert!(bad > 0, "plant {} not caught", plant_id());
    } else {
        c.finish();
    }
}
