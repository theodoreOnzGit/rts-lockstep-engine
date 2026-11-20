// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of `DecalPolyClass` (`reset`, `add_vertex`, `clip`) in the draft `src/drafts/ww3d/decalmsh.rs` against upstream
//! `WW3D2/decalmsh.cpp`'s `DecalPolyClass`, the class and its three members cut VERBATIM (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_decal.cpp). `PlaneClass`
//! (`In_Front`, `Compute_Intersection`), `Vector3::Lerp` and `SimpleDynVecClass` are the real headers; no stand-ins. The rest of decalmsh.rs (`DecalMeshClass`/`Create_Decal`/
//! `Render`/`Delete_Decal`, `MultiFixedPool...`) needs `MeshClass`, `MeshModelClass` and the DX8 buffers and is NOT covered here.
//!
//! # Method
//!
//! 20,000 polygons (fixed seed, `DRAFT_CASES` overrides) of 0 to 10 vertices (positions on a half-integer grid, so that vertices lie exactly on a plane, or random; normals random),
//! clipped by a chain of 1 to 4 planes (axis-aligned with grid offsets, or random), the result of each clip being the input of the next, as `Create_Decal` ping-pongs its two
//! polygons. Compared as bits: the vertex count after each clip, every position and every normal.
//!
//! # Excluded inputs
//!
//! An edge that crosses the plane with `Compute_Intersection` returning without writing `alpha` (`den == 0`, parallel): upstream's `alpha` is an uninitialised local (the draft uses 0).
//! It cannot occur for a segment whose end points are on opposite sides of the plane.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches: the draft's `lerp` is the same expression as `Vector3::Lerp`.
//!
//! # Results (2026-11-20)
//!
//! **0 mismatches** over 20,000 polygons clipped by chains of 1 to 4 planes. **Prediction held.**
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!   19131: 1,006; 19132: 1,291; 19133: 8,537; 19134: 12,943; 19135: 12,996; 19136: 12,991
//!
//! **6 of 6 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::decalmsh::DecalPolyClass;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_decal_clip(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

fn coord(g: &mut Gen, grid: bool) -> f32 {
    if grid {
        (g.below(17) as f32 - 8.0) * 0.5
    } else {
        g.uniform(-5.0, 5.0)
    }
}

#[test]
fn decalpoly_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_08);
    let mut chk = Check::new("DecalPolyClass::Clip (chains of planes)");
    let mut out = vec![0u32; 20_000];
    for case in 0..support_sweep::cases() {
        let grid = g.below(2) == 0;
        let nv = g.below(11) as usize;
        let np = 1 + g.below(4) as usize;
        let planes: Vec<PlaneClass> = (0..np)
            .map(|_| {
                let n = match g.below(5) {
                    0 => Vector3::new(1.0, 0.0, 0.0),
                    1 => Vector3::new(0.0, 1.0, 0.0),
                    2 => Vector3::new(0.0, 0.0, -1.0),
                    _ => Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)),
                };
                PlaneClass { n, d: if g.below(2) == 0 { (g.below(9) as f32 - 4.0) * 0.5 } else { g.uniform(-3.0, 3.0) } }
            })
            .collect();
        let mut input = vec![np as u32];
        for p in &planes {
            input.extend([b(p.n.x), b(p.n.y), b(p.n.z), b(p.d)]);
        }
        input.push(nv as u32);
        let mut poly = DecalPolyClass::default();
        for _ in 0..nv {
            let pos = Vector3::new(coord(&mut g, grid), coord(&mut g, grid), coord(&mut g, grid));
            let nrm = Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0));
            input.extend([b(pos.x), b(pos.y), b(pos.z), b(nrm.x), b(nrm.y), b(nrm.z)]);
            poly.add_vertex(&pos, &nrm);
        }
        let k = unsafe { ref_w6_decal_clip(input.as_ptr(), out.as_mut_ptr()) } as usize;
        let mut rust = Vec::new();
        for pl in &planes {
            let mut dest = DecalPolyClass::default();
            poly.clip(pl, &mut dest);
            rust.push(dest.verts.len() as u32);
            for (v, n) in dest.verts.iter().zip(&dest.vert_norms) {
                rust.extend([b(v.x), b(v.y), b(v.z), b(n.x), b(n.y), b(n.z)]);
            }
            poly = dest;
        }
        chk.eq(rust, out[..k].to_vec(), || format!("case {case} {nv} vertices {np} planes {planes:?}"));
    }
    support_sweep::finish(vec![chk]);
}
