// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/line3d.rs` (`Line3DClass`) against upstream `WW3D2/line3d.cpp`: the
//! constructor, both `Scale` overloads, `Get_Obj_Space_Bounding_Sphere` / `_Box`, both `Reset` overloads, `Re_Color`, `Set_Opacity` and `Get_Num_Polys`, cut
//! VERBATIM (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_line.cpp). Stand-ins: the class around the cut (line3d.h's members over a reduced
//! `RenderObjClass`: `Transform`, `Set_Transform`, a counted `Invalidate_Cached_Bounding_Volumes`, no container) and `ShaderClass`'s two presets as ids;
//! `Matrix3D::Obj_Look_At`, `Vector3`, `Vector4`, `SphereClass`, `AABoxClass` are the real headers. `Render` is not cut (its vertex fill is interleaved with DX8 dynamic
//! buffers): its vertices are compared through `render_data`, its colour against `DX8Wrapper::Convert_Color(Color)`. `Clone`, `operator=` (a `WWASSERT(0)`) and
//! `Class_ID` (needs rendobj.h's enum) are not covered.
//!
//! # Method
//!
//! 20,000 lines (fixed seed, `DRAFT_CASES` overrides): a start and end on a half-integer grid (so zero lengths and axis-aligned look-ats occur) or random, a width, a colour,
//! an opacity (below, at and above 1), then 0 to 6 operations (`Scale`, `Scale(x, y, z)`, both `Reset`s, `Re_Color`, `Set_Opacity`, `Set_Sort_Level` with values beyond a `char`).
//! After the constructor and every operation, compared as bits: the shader, sort level, transform (12 words), the 8 vertices, the converted colour, the bounding sphere
//! and box, `Get_Num_Polys`, and the number of cache invalidations (the draft leaves those to the owner, so that one word is dropped).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::line3d::*;
use rts_lockstep_engine::drafts::ww3d::shader_presets::PRESET_ALPHA_SOLID_SHADER;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_line_run(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

fn point(g: &mut Gen, grid: bool) -> Vector3 {
    if grid {
        Vector3::new((g.below(9) as f32 - 4.0) * 0.5, (g.below(9) as f32 - 4.0) * 0.5, (g.below(9) as f32 - 4.0) * 0.5)
    } else {
        Vector3::new(g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0))
    }
}

fn dump(l: &Line3DClass) -> Vec<u32> {
    let (shader, tm, verts) = l.render_data();
    let mut w = vec![if shader == PRESET_ALPHA_SOLID_SHADER { 1 } else { 2 }, l.get_sort_level() as u32];
    w.extend(tm.to_array().iter().map(|x| b(*x)));
    for (p, _) in &verts {
        w.extend([b(p.x), b(p.y), b(p.z)]);
    }
    w.push(verts[0].1);
    let (c, r) = l.get_obj_space_bounding_sphere();
    w.extend([b(c.x), b(c.y), b(c.z), b(r)]);
    let bx = l.get_obj_space_bounding_box();
    w.extend([b(bx.center.x), b(bx.center.y), b(bx.center.z), b(bx.extent.x), b(bx.extent.y), b(bx.extent.z)]);
    w.push(l.get_num_polys() as u32);
    w
}

#[test]
fn line3d_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_06);
    let mut chk = Check::new("Line3DClass (constructor + operations)");
    let mut out = vec![0u32; 20_000];
    for case in 0..support_sweep::cases() {
        let grid = g.below(2) == 0;
        let start = point(&mut g, grid);
        let end = if g.below(8) == 0 { start } else { point(&mut g, grid) };
        let width = if g.below(2) == 0 { g.below(5) as f32 * 0.5 + 0.5 } else { g.uniform(0.1, 3.0) };
        let (r, gr, bl) = (g.uniform(0.0, 1.0), g.uniform(0.0, 1.0), g.uniform(0.0, 1.0));
        let opacity = [0.0f32, 0.5, 0.99999994, 1.0, 1.5][g.below(5) as usize];
        let mut input = vec![b(start.x), b(start.y), b(start.z), b(end.x), b(end.y), b(end.z), b(width), b(r), b(gr), b(bl), b(opacity)];
        let mut line = Line3DClass::new(&start, &end, width, r, gr, bl, opacity);
        let mut expect = dump(&line);
        let nops = g.below(7);
        input.push(nops);
        for _ in 0..nops {
            let kind = g.below(7);
            let mut a = [0u32; 7];
            match kind {
                0 => {
                    let s = if g.below(5) == 0 { 0.0 } else { g.uniform(0.1, 3.0) };
                    a[0] = b(s);
                    line.scale(s);
                }
                1 => {
                    let (x, y, z) = (g.uniform(0.1, 3.0), g.uniform(0.1, 3.0), g.uniform(0.1, 3.0));
                    a[..3].copy_from_slice(&[b(x), b(y), b(z)]);
                    line.scale_xyz(x, y, z);
                }
                2 | 3 => {
                    let s = point(&mut g, grid);
                    let e = if g.below(8) == 0 { s } else { point(&mut g, grid) };
                    a[..6].copy_from_slice(&[b(s.x), b(s.y), b(s.z), b(e.x), b(e.y), b(e.z)]);
                    if kind == 2 {
                        line.reset(&s, &e);
                    } else {
                        let nw = g.uniform(0.2, 4.0);
                        a[6] = b(nw);
                        line.reset_with_width(&s, &e, nw);
                    }
                }
                4 => {
                    let (r, gr, bl) = (g.uniform(0.0, 1.0), g.uniform(0.0, 1.0), g.uniform(0.0, 1.0));
                    a[..3].copy_from_slice(&[b(r), b(gr), b(bl)]);
                    line.re_color(r, gr, bl);
                }
                5 => {
                    let o = [0.0f32, 0.5, 0.99999994, 1.0, 1.5][g.below(5) as usize];
                    a[0] = b(o);
                    line.set_opacity(o);
                }
                _ => {
                    let lv = [0i32, 1, 5, 127, 128, 255, 256, 300, -1][g.below(9) as usize];
                    a[0] = lv as u32;
                    line.set_sort_level(lv);
                }
            }
            input.push(kind);
            input.extend(a);
            expect.extend(dump(&line));
        }
        let n = unsafe { ref_w6_line_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
        // the C++ dump ends each state with the invalidation count, which the draft leaves to the owner: drop that word
        let per = 2 + 12 + 24 + 1 + 4 + 6 + 1;
        let mut cpp = Vec::new();
        for st in out[..n].chunks(per + 1) {
            cpp.extend_from_slice(&st[..per]);
        }
        chk.eq(expect, cpp, || format!("case {case} start {start:?} end {end:?} width {width} colour {r} {gr} {bl} opacity {opacity} ops {nops}"));
    }
    support_sweep::finish(vec![chk]);
}
