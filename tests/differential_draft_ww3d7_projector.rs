// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d7) of the draft `src/drafts/ww3d/projector.rs` (ProjectorClass) against upstream WW3D2
//! `projector.h` / `projector.cpp`, cut verbatim by build.rs `drafts_ww3d7` (differential/wrap_draft_ww3d7_projector.cpp lists the cuts and the
//! stand-ins; matrixmapper.cpp is upstream's, compiled under renamed classes).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) programs from a fixed seed: 1..=8 operations (`Set_Transform` of a random 3x4 matrix, `Set_Perspective_Projection`,
//! `Set_Ortho_Projection`, `Compute_Texture_Coordinate`), the arguments from `Gen::mixed` (ordinary magnitudes, unit range, small integers,
//! log-uniform, raw bit patterns, special values) and, for the field of view, mostly 0.05..3.0 radians. Before the first operation and after each:
//! `Transform` (12 floats), `Projection` (16), `LocalBoundingVolume` (6), `WorldBoundingVolume` (center, extent, basis: 15), the mapper's type. Floats
//! are compared by bits (NaN payload-only differences are counted apart).
//!
//! # Excluded inputs
//! `zfar <= 0.1` (and NaN): upstream's `WWASSERT(zfar > znear)` in `Init_Perspective` and `Init_Ortho` is compiled out and the divisions give inf and NaN,
//! but the draft's `assert!` in src/drafts/wwlib/matrix4.rs fires (a deviation of that draft, not of the projector). The projector passes `znear = 0.1`.
//!
//! # Predictions (2026-11-20, before the first run)
//! 0 mismatches. Doubt: `tan(hfov) * 0.5f` is the C library's double `tan` (the test support routes it to petir's, as for the matrix4 draft).
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default (20,000), fixed seed: **0 mismatches** in 5,461,503 floats and 20,000 mapper types; max ULP 0; 1,259 floats are NaN in
//! both with different payloads (counted apart, as `Check` does). The prediction held, including the `tan` doubt (the support's `tan` is petir's on both sides).
//! The first run panicked in `Matrix4x4::init_ortho`'s `assert!(zfar > znear)` (draft in wwlib/matrix4.rs, upstream's `WWASSERT` is compiled out), so
//! the generator keeps `zfar > 0.1`: the exclusion above. Two getters were added to the draft for this test (`get_projection`, `get_local_bounding_volume`:
//! upstream's members are protected; the derived classes read them).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching floats of 5,461,503, or mapper types of 20,000)
//! 19491 (fov arguments swapped in `Init_Perspective`) 72,418; 19492 (`tan(hfov/2)`) 143,884; 19493 (single-precision `tan`) 17,288 (max 2 ULP); 19494 (sign of
//! the centre z) 102,211; 19495 (extent z without the 0.5) 71,504; 19496 (ortho centre x from the difference) 65,303; 19497 (ortho extent y from x) 72,112; 19498
//! (`Set_Transform` does not update the world box) 336,082; 19499 (local extents x and y swapped in the world box) 144,334; 19500 (ortho sets the perspective
//! type) 13,268 (types). **10 of 10 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::projector::ProjectorClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw7_proj(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 100_000];
    let n = unsafe { ref_dw7_proj(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper failed");
    out.truncate(n as usize);
    out
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in &chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        for e in c.examples.iter().take(2) {
            println!("EX {}: {}", c.name, &e[..e.len().min(1200)]);
        }
        bad += c.mismatches;
    }
    if !planted {
        assert_eq!(bad, 0, "mismatches against the C++");
    } else {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn dump(p: &ProjectorClass) -> Vec<f32> {
    let mut v = Vec::new();
    for r in &p.get_transform().row {
        v.extend([r.x, r.y, r.z, r.w]);
    }
    for r in &p.get_projection().row {
        v.extend([r.x, r.y, r.z, r.w]);
    }
    let l = p.get_local_bounding_volume();
    v.extend([l.center.x, l.center.y, l.center.z, l.extent.x, l.extent.y, l.extent.z]);
    let b = p.get_bounding_volume();
    v.extend([b.center.x, b.center.y, b.center.z, b.extent.x, b.extent.y, b.extent.z]);
    for r in &b.basis {
        v.extend(*r);
    }
    v
}

/// `zfar` of `Init_Perspective` and `Init_Ortho`, whose `znear` the projector always passes as 0.1: upstream's `WWASSERT(zfar > znear)` is compiled
/// out, the draft's `assert!` (src/drafts/wwlib/matrix4.rs) is not, so only `zfar > 0.1` (infinity included) is generated.
fn zfar(g: &mut Gen) -> f32 {
    loop {
        let z = g.mixed();
        if z > 0.1 {
            return z;
        }
    }
}

fn fov(g: &mut Gen) -> f32 {
    if g.below(4) == 0 {
        g.mixed()
    } else {
        g.uniform(0.05, 3.0)
    }
}

fn case(g: &mut Gen, c: &mut Check, ty: &mut Check, k: u64) {
    let mut inp = vec![0u32];
    let nops = 1 + g.below(8);
    let mut ops: Vec<(u32, Vec<f32>)> = Vec::new();
    for _ in 0..nops {
        let kind = g.below(4);
        let args: Vec<f32> = match kind {
            0 => (0..12).map(|_| g.mixed()).collect(),
            1 => vec![fov(g), fov(g), g.mixed(), zfar(g)],
            2 => vec![g.mixed(), g.mixed(), g.mixed(), g.mixed(), g.mixed(), zfar(g)],
            _ => (0..3).map(|_| g.mixed()).collect(),
        };
        inp.push(kind);
        inp.extend(args.iter().map(|f| f.to_bits()));
        ops.push((kind, args));
    }
    inp[0] = nops;
    let cpp = run(&inp);
    let mut p = ProjectorClass::new();
    let mut mine: Vec<f32> = dump(&p);
    let mut types = vec![p.peek_mapper().get_type() as u32];
    for (kind, a) in &ops {
        match kind {
            0 => {
                let m = Matrix3D {
                    row: [Vector4 { x: a[0], y: a[1], z: a[2], w: a[3] }, Vector4 { x: a[4], y: a[5], z: a[6], w: a[7] }, Vector4 { x: a[8], y: a[9], z: a[10], w: a[11] }],
                };
                p.set_transform(&m);
            }
            1 => p.set_perspective_projection(a[0], a[1], a[2], a[3]),
            2 => p.set_ortho_projection(a[0], a[1], a[2], a[3], a[4], a[5]),
            _ => {
                let r = p.compute_texture_coordinate(&Vector3::new(a[0], a[1], a[2]));
                mine.extend([r.x, r.y, r.z]);
            }
        }
        mine.extend(dump(&p));
        types.push(p.peek_mapper().get_type() as u32);
    }
    // the generic reader: dump 0, then per op ([3 coordinates if kind 3], dump)
    let mut at = 0usize;
    let read_dump = |at: &mut usize, cf: &mut Vec<f32>, ct: &mut Vec<u32>| {
        for _ in 0..(12 + 16 + 6 + 15) {
            cf.push(f32::from_bits(cpp[*at]));
            *at += 1;
        }
        ct.push(cpp[*at]);
        *at += 1;
    };
    let mut cf: Vec<f32> = Vec::new();
    let mut ct: Vec<u32> = Vec::new();
    read_dump(&mut at, &mut cf, &mut ct);
    for (kind, _) in &ops {
        if *kind == 3 {
            for _ in 0..3 {
                cf.push(f32::from_bits(cpp[at]));
                at += 1;
            }
        }
        read_dump(&mut at, &mut cf, &mut ct);
    }
    assert_eq!(at, cpp.len(), "the wrapper's output has the layout of the test");
    assert_eq!(cf.len(), mine.len());
    c.f32s(&mine, &cf, || format!("case {k} ops {ops:?}"));
    ty.eq(types, ct, || format!("case {k} ops {ops:?}"));
}

#[test]
fn ww3d7_projector_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_17_04);
    let mut c = Check::new("ProjectorClass floats");
    let mut ty = Check::new("ProjectorClass mapper type");
    for k in 0..cases() {
        case(&mut g, &mut c, &mut ty, k);
    }
    finish(vec![c, ty]);
}
