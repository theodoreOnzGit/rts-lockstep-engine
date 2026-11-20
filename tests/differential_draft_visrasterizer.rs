// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/visrasterizer.rs` (`IDBufferClass`, `VisPolyClass`,
//! `VisRasterizerClass`) against upstream `WW3D2/visrasterizer.cpp` / `.h`, the declarations and implementation cut VERBATIM
//! (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_vis.cpp). The 4 `WWMath::Float_To_Long` calls name the MSVC `fistp` branch (the shipped
//! game's; the Linux `wwmath.h` truncates), as in `bwrender`. Stand-ins: `CameraClass` (scripted view matrix, a perspective projection, six frustum planes),
//! `CollisionMath::Overlap_Test` (the test's INSIDE flag), `TriIndex` (three shorts); the buffers are zeroed after `Set_Resolution` (the game's allocator
//! zero-fills). `VectorProcessorClass::Transform` is upstream's own, linked from another library.
//!
//! # Method
//!
//! 20,000 cases per part (fixed seed, `DRAFT_CASES` overrides). Part 1, `IDBufferClass` alone: a buffer of 2 to 40 by 2 to 40 pixels, ids, occluder or
//! non-occluder mode, one- or two-sided, then 1 to 8 operations from `Render_Triangle` (vertices in the -1..1 square and a little beyond, a share on pixel
//! boundaries, depths from -0.99 to 1 with exact ties, degenerate, repeated and collinear vertices), `Clear`, `Reset_Pixel_Counter` and a second `Set_Resolution`.
//! Part 2, `VisRasterizerClass::Render_Triangles` (so `VisPolyClass::Clip`, the transform, the projection and the triangle fan): 3 to 12 vertices, 1 to 6
//! triangles, the clip path (vertices on both sides of a frustum, model transforms) and the no-clip path (the INSIDE flag, vertices in front of the camera).
//! Compared: every return value, `Get_Pixel_Counter`, the resolution, and the whole ID buffer through `Get_Pixel_Row`.
//!
//! # Excluded inputs
//!
//! - `Clear` for a buffer wider than tall: upstream clears `ResWidth * ResWidth` entries of a `ResWidth * ResHeight` buffer (writes past the end). The draft caps it at the
//!   buffer. A taller buffer is compared (both clear only the first `ResWidth^2` entries).
//! - Vertices that project outside +-1e4 or to NaN (the C++ float to int conversions of such values are undefined): the rasteriser tests keep depth in front of the camera.
//! - The 1/z buffer cannot be read from the draft (private), so it is compared through its effect on the ID buffer and the counters.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches. The draft transcribes the scan converter line by line; the places to watch are `Clear` for tall buffers and `float_to_long` (default mode).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::visrasterizer::*;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_vis_idbuf(input: *const u32, out: *mut u32) -> i32;
    fn ref_w6_vis_raster(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

struct TestCam {
    view: Matrix3D,
    planes: [PlaneClass; 6],
    inside: bool,
}

impl VisCamera for TestCam {
    fn get_view_matrix(&self) -> Matrix3D {
        self.view
    }
    fn project_camera_space_point(&self, p: &Vector3) -> Vector3 {
        let inv = 1.0f32 / -p.z;
        Vector3::new(p.x * inv, p.y * inv, 1.0f32 + 1.0f32 / p.z)
    }
    fn get_view_space_frustum_planes(&self) -> [PlaneClass; 6] {
        self.planes
    }
    fn frustum_contains_box(&self, _center: &Vector3, _extent: &Vector3) -> bool {
        self.inside
    }
}

fn xy(g: &mut Gen, res: i32) -> f32 {
    match g.below(8) {
        0..=2 => (g.below((res * 2 + 5) as u32) as f32 - 2.0) / res as f32 - 1.0, // pixel boundaries (and a little outside)
        3 => [-1.0f32, 0.0, 1.0][g.below(3) as usize],
        _ => g.uniform(-1.3, 1.3),
    }
}

fn zed(g: &mut Gen) -> f32 {
    match g.below(4) {
        0 => [-0.5f32, 0.0, 0.5][g.below(3) as usize],
        _ => g.uniform(-0.99, 1.0),
    }
}

fn setup(g: &mut Gen) -> (i32, i32, u32, u32, u32, u32, Vec<u32>) {
    let w = 2 + g.below(39) as i32;
    let h = if g.below(3) == 0 { w } else { 2 + g.below(39) as i32 };
    let back = g.below(4);
    let front = if g.below(5) == 0 { back } else { 10 + g.below(5) };
    let mode = g.below(3).min(1);
    let two = g.below(3) / 2;
    (w, h, back, front, mode, two, vec![w as u32, h as u32, back, front, mode, two])
}

fn read_rows(r: &IDBufferClass, w: i32, h: i32) -> Vec<u32> {
    let mut v = vec![w as u32, h as u32];
    for y in 0..h {
        v.extend_from_slice(&r.get_pixel_row(y, 0, w)[..w as usize]);
    }
    v
}

#[test]
fn visrasterizer_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_04);
    let names = ["IDBufferClass (Render_Triangle, Clear, counters)", "VisRasterizerClass::Render_Triangles (clip path)", "VisRasterizerClass::Render_Triangles (no-clip path)"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let mut out = vec![0u32; 20_000];
    for case in 0..support_sweep::cases() {
        // --- Part 1
        {
            let (mut w, mut h, back, front, mode, two, mut input) = setup(&mut g);
            let mut r = IDBufferClass::new();
            r.set_resolution(w, h);
            r.backface_id = back;
            r.frontface_id = front;
            r.render_mode = if mode == 1 { ModeType::NonOccluderMode } else { ModeType::OccluderMode };
            r.two_sided_rendering_enabled = two == 1;
            let nops = 1 + g.below(8);
            input.push(nops);
            let mut expect: Vec<u32> = Vec::new();
            for _ in 0..nops {
                let op = match g.below(12) {
                    0 if w <= h => 1,
                    1 => 2,
                    2 => 3,
                    _ => 0,
                };
                let mut f = [0.0f32; 9];
                match op {
                    0 => {
                        for k in 0..3 {
                            f[3 * k] = xy(&mut g, w);
                            f[3 * k + 1] = xy(&mut g, h);
                            f[3 * k + 2] = zed(&mut g);
                        }
                        match g.below(8) {
                            0 => {
                                f[3] = f[0];
                                f[4] = f[1];
                            }
                            1 => {
                                f[7] = f[1];
                                f[5] = f[2];
                            }
                            2 => {
                                f[6] = f[0];
                                f[8] = f[2];
                            }
                            _ => {}
                        }
                        let v = |k: usize| Vector3::new(f[3 * k], f[3 * k + 1], f[3 * k + 2]);
                        expect.push(r.render_triangle(&v(0), &v(1), &v(2)) as u32);
                        expect.push(r.pixel_counter as u32);
                    }
                    1 => r.clear(),
                    2 => r.pixel_counter = 0,
                    _ => {
                        w = 2 + g.below(39) as i32;
                        h = 2 + g.below(39) as i32;
                        f[0] = f32::from_bits(w as u32);
                        f[1] = f32::from_bits(h as u32);
                        r.set_resolution(w, h);
                    }
                }
                input.push(op);
                input.extend(f.iter().map(|x| b(*x)));
            }
            let k = unsafe { ref_w6_vis_idbuf(input.as_ptr(), out.as_mut_ptr()) } as usize;
            let (rw, rh) = r.get_resolution();
            expect.extend(read_rows(&r, rw, rh));
            c[0].eq(expect, out[..k].to_vec(), || format!("case {case} {w}x{h} ids {back}/{front} mode {mode} two {two} ops {nops}"));
        }
        // --- Part 2
        for part in 0..2 {
            let (w, h, back, front, mode, two, mut input) = setup(&mut g);
            let inside = part == 1;
            input.push(inside as u32);
            let (model, view) = if inside || g.below(2) == 0 {
                (Matrix3D::identity(), Matrix3D::identity())
            } else {
                let ang = g.uniform(-0.5, 0.5);
                let (s, cs) = (ang.sin(), ang.cos());
                (Matrix3D::from_array([cs, 0.0, s, g.uniform(-3.0, 3.0), 0.0, 1.0, 0.0, g.uniform(-3.0, 3.0), -s, 0.0, cs, g.uniform(-3.0, 3.0)]), Matrix3D::identity())
            };
            let mut planes = [
                PlaneClass { n: Vector3::new(1.0, 0.0, 1.0), d: 0.0 },
                PlaneClass { n: Vector3::new(-1.0, 0.0, 1.0), d: 0.0 },
                PlaneClass { n: Vector3::new(0.0, 1.0, 1.0), d: 0.0 },
                PlaneClass { n: Vector3::new(0.0, -1.0, 1.0), d: 0.0 },
                PlaneClass { n: Vector3::new(0.0, 0.0, 1.0), d: -0.5 },
                PlaneClass { n: Vector3::new(0.0, 0.0, -1.0), d: 100.0 },
            ];
            if g.below(5) == 0 {
                for p in planes.iter_mut().take(4) {
                    p.n.x += g.uniform(-0.2, 0.2);
                    p.n.y += g.uniform(-0.2, 0.2);
                    p.d += g.uniform(-0.2, 0.2);
                }
            }
            input.extend(model.to_array().iter().map(|x| b(*x)));
            input.extend(view.to_array().iter().map(|x| b(*x)));
            for p in &planes {
                input.extend([b(p.n.x), b(p.n.y), b(p.n.z), b(p.d)]);
            }
            let nv = 3 + g.below(10) as usize;
            input.push(nv as u32);
            let mut verts = Vec::new();
            for _ in 0..nv {
                let z = if inside { -g.uniform(1.0, 30.0) } else { g.uniform(-30.0, 3.0) };
                let spread = if inside { -z } else { 12.0 };
                let v = Vector3::new(g.uniform(-1.0, 1.0) * spread, g.uniform(-1.0, 1.0) * spread, z);
                input.extend([b(v.x), b(v.y), b(v.z)]);
                verts.push(v);
            }
            let nt = 1 + g.below(6) as usize;
            input.push(nt as u32);
            let mut tris = Vec::new();
            for _ in 0..nt {
                let t = [g.below(nv as u32), g.below(nv as u32), g.below(nv as u32)];
                input.extend(t);
                tris.push(t);
            }
            let cam = TestCam { view, planes, inside };
            let mut r = VisRasterizerClass::new();
            r.set_resolution(w, h);
            r.id_buffer.backface_id = back;
            r.id_buffer.frontface_id = front;
            r.id_buffer.render_mode = if mode == 1 { ModeType::NonOccluderMode } else { ModeType::OccluderMode };
            r.id_buffer.two_sided_rendering_enabled = two == 1;
            r.set_model_transform(&model, Some(&cam));
            let zero = Vector3::new(0.0, 0.0, 0.0);
            let res = r.render_triangles(&cam, &verts, nv, &tris, nt, (&zero, &zero));
            let mut expect = vec![res as u32, r.id_buffer.pixel_counter as u32];
            for y in 0..h {
                expect.extend_from_slice(&r.id_buffer.get_pixel_row(y, 0, w)[..w as usize]);
            }
            let k = unsafe { ref_w6_vis_raster(input.as_ptr(), out.as_mut_ptr()) } as usize;
            c[1 + part].eq(expect, out[..k].to_vec(), || format!("case {case} {w}x{h} verts {verts:?} tris {tris:?} model {model:?} planes {planes:?}"));
        }
    }
    support_sweep::finish(c);
}
