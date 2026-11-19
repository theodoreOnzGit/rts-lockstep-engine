// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of the draft `src/drafts/sweep_ww3d/camera.rs` (`CameraClass`'s
//! `Transform_To_View_Space`, `Rotate_To_View_Space`, `Get_Near_Clip_Bounding_Box`, `Get_Horizontal_FOV`, `Get_Vertical_FOV`,
//! `Get_Aspect_Ratio`, `Get_D3D_Projection_Matrix`, `Convert_Old`, `Compute_Projected_Sphere_Radius`) against upstream
//! `WW3D2/camera.cpp` cut VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_camera.cpp). Stand-in: a `CameraClass`
//! with the fields the nine functions read and an `Update_Frustum` that counts its calls (the draft takes the call as a closure,
//! so the call count of each function is compared too).
//!
//! # Method
//!
//! 20,000 cameras (fixed seed, `DRAFT_CASES` overrides), each a random inverse transform, near-clip box, view plane, aspect,
//! clip distances (with `ZNear == ZFar`, negative and special values among them), projection type and projection matrix, then
//! 1 to 8 random operations. Every float is compared by bits; the number of `Update_Frustum` calls is compared.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches. Places I am unsure of: `Get_D3D_Projection_Matrix`'s `float oozdiff = 1.0 / (ZFar - ZNear)` is a `double`
//! division narrowed on assignment (the draft does the same through `f64`); `Compute_Projected_Sphere_Radius`'s
//! `Matrix4x4 * Vector4` sums left to right (the draft does too); `WWMath::Atan2` is the `double` `atan2` narrowed.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::camera::{CameraState, ProjectionType};
use rts_lockstep_engine::drafts::wwlib::matrix3::Matrix3x3;
use rts_lockstep_engine::drafts::wwlib::obbox::OBBoxClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::cell::Cell;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{b, cases, feq, finish};

extern "C" {
    fn ref_dsww3d_cam_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = unsafe { ref_dsww3d_cam_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn val(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => g.special(),
        1 => 0.0,
        2 => (g.below(5) as f32) - 2.0,
        _ => g.uniform(-30.0, 30.0),
    }
}

fn v3b(v: &Vector3) -> Vec<u32> {
    b(&[v.x, v.y, v.z])
}

#[test]
fn camera_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0c01);
    let names = [
        "Transform_To_View_Space", "Rotate_To_View_Space", "Get_Near_Clip_Bounding_Box", "Get_Horizontal_FOV", "Get_Vertical_FOV",
        "Get_Aspect_Ratio", "Get_D3D_Projection_Matrix", "Convert_Old", "Compute_Projected_Sphere_Radius",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let m: [f32; 12] = std::array::from_fn(|_| val(&mut g));
        let inv = Matrix3D { row: [Vector4::new(m[0], m[1], m[2], m[3]), Vector4::new(m[4], m[5], m[6], m[7]), Vector4::new(m[8], m[9], m[10], m[11])] };
        let center = Vector3::new(val(&mut g), val(&mut g), val(&mut g));
        let extent = Vector3::new(val(&mut g), val(&mut g), val(&mut g));
        let basis_w: [f32; 9] = std::array::from_fn(|_| val(&mut g));
        let basis = Matrix3x3 { row: [Vector3::new(basis_w[0], basis_w[1], basis_w[2]), Vector3::new(basis_w[3], basis_w[4], basis_w[5]), Vector3::new(basis_w[6], basis_w[7], basis_w[8])] };
        let vmin = [val(&mut g), val(&mut g)];
        let vmax = [val(&mut g), val(&mut g)];
        let aspect = val(&mut g);
        let znear = match g.below(6) {
            0 => 1.0,
            1 => 0.1,
            _ => g.uniform(0.0, 20.0),
        };
        let zfar = match g.below(8) {
            0 => znear,
            1 => 1000.0,
            2 => val(&mut g),
            _ => znear + g.uniform(0.001, 5000.0),
        };
        let ortho = g.below(3) == 0;
        let proj: [[f32; 4]; 4] = std::array::from_fn(|_| std::array::from_fn(|_| val(&mut g)));
        let st = CameraState {
            camera_inv_transform: inv,
            near_clip_bbox: OBBoxClass { basis, center, extent },
            view_plane_min: vmin,
            view_plane_max: vmax,
            aspect_ratio: aspect,
            z_near: znear,
            z_far: zfar,
            projection: if ortho { ProjectionType::Ortho } else { ProjectionType::Perspective },
            projection_transform: proj,
        };
        let mut inp: Vec<u32> = b(&m);
        inp.extend(v3b(&center));
        inp.extend(v3b(&extent));
        inp.extend(b(&basis_w));
        inp.extend(b(&[vmin[0], vmin[1], vmax[0], vmax[1], aspect, znear, zfar]));
        inp.push(u32::from(ortho));
        for r in &proj {
            inp.extend(b(r));
        }
        let nops = 1 + g.below(8);
        inp.push(nops);
        let mut rust: Vec<Vec<u32>> = Vec::new();
        let mut which: Vec<usize> = Vec::new();
        for _ in 0..nops {
            let op = g.below(9) as usize;
            inp.push(op as u32);
            let calls = Cell::new(0u32);
            let upd = || calls.set(calls.get() + 1);
            let mut w: Vec<u32>;
            match op {
                0 | 1 => {
                    let v = Vector3::new(val(&mut g), val(&mut g), val(&mut g));
                    inp.extend(v3b(&v));
                    let r = if op == 0 { st.transform_to_view_space(upd, &v) } else { st.rotate_to_view_space(upd, &v) };
                    w = v3b(&r);
                    w.push(calls.get());
                }
                2 => {
                    let bx = st.get_near_clip_bounding_box(upd);
                    w = v3b(&bx.center);
                    w.extend(v3b(&bx.extent));
                    for r in &bx.basis.row {
                        w.extend(v3b(r));
                    }
                    w.push(calls.get());
                }
                3 => w = vec![st.get_horizontal_fov().to_bits()],
                4 => w = vec![st.get_vertical_fov().to_bits()],
                5 => w = vec![st.get_aspect_ratio().to_bits()],
                6 => {
                    let t = st.get_d3d_projection_matrix(upd);
                    w = t.iter().flat_map(|r| b(r)).collect();
                    w.push(calls.get());
                }
                7 => {
                    let mut v = Vector3::new(val(&mut g), val(&mut g), val(&mut g));
                    inp.extend(v3b(&v));
                    CameraState::convert_old(&mut v);
                    w = v3b(&v);
                }
                _ => {
                    let (d, r) = (val(&mut g), val(&mut g));
                    inp.extend(b(&[d, r]));
                    w = vec![st.compute_projected_sphere_radius(d, r).to_bits()];
                }
            }
            rust.push(w);
            which.push(op);
        }
        let cpp = run(&inp);
        // split the C++ output per operation by the Rust word counts
        let mut at = 0;
        for (i, w) in rust.into_iter().enumerate() {
            let end = (at + w.len()).min(cpp.len());
            feq(&mut c[which[i]], w, cpp[at..end].to_vec(), || format!("case {case} op {}", which[i]));
            at += end - at;
        }
        if at != cpp.len() {
            c[0].eq(at, cpp.len(), || format!("case {case}: output length"));
        }
    }
    finish(c);
}
