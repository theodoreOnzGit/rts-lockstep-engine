// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of the draft `src/drafts/sweep_ww3d/meshmdlio.rs` (`MeshModelSortState`:
//! `get_sort_flags_pass`, `get_sort_flags`, `compute_static_sort_levels`, `modify_for_overbright`, `post_process_fog`) against
//! upstream `WW3D2/meshmdlio.cpp` (`MeshModelClass::{get_sort_flags, get_sort_flags, compute_static_sort_levels,
//! modify_for_overbright, post_process_fog}`) and `WW3D2/shader.cpp` (`ShaderClass::{Enable_Fog, Get_SS_Category}`), all cut
//! VERBATIM (build.rs `drafts_ww3d4`, differential/wrap_draft_dw4_mmio.cpp). The real shader.h is used (class renamed). Stand-ins:
//! the model / material description classes hold only what the cuts touch (see the wrapper), `Report_Unable_To_Fog` is empty (its body
//! is `#ifdef WWDEBUG`), texture and vertex-material objects are ids / UV sources.
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides). Each case builds a material description (0 to 4 passes, 0 to 8 polygons,
//! 1 to 8 vertices; shaders are random bit patterns, field-structured patterns, and the two special two-pass templates of
//! `post_process_fog`, the emissive map and the shiny mask, often with one field perturbed; per-polygon shader arrays on some passes;
//! textures, texture arrays, UV sources and vertex-material arrays on passes 0 and 1), then runs ONE of the five functions and
//! compares the result, the sort level, and every shader word, array, texture id and UV source afterwards.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! One MISS expected: in the emissive-map branch upstream, when pass 1 has no vertex-material array, calls
//! `DefMatDesc->Material[1]->Get_UV_Source(0);` and DISCARDS the result (`uv_source` stays 0); the draft assigns it. Everything else 0.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::meshmdlio::*;
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_mmio_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn mk(dst: u32, src: u32, grad: u32, sec: u32, tex: u32, alpha: u32, fog: u32, other: u32) -> u32 {
    (dst << 5) | (src << 14) | (grad << 10) | (sec << 13) | (tex << 16) | (alpha << 18) | (fog << 8) | (other & 0x0FFF_0000 & !(1 << 16) & !(1 << 18)) | (other & 0x1F)
}

fn shader(g: &mut Gen) -> u32 {
    match g.below(3) {
        0 => g.u32() & 0x0FFF_FFFF,
        _ => mk(g.below(8), g.below(4), g.below(8), g.below(2), g.below(2), g.below(2), g.below(4), g.u32()),
    }
}

fn perturb(g: &mut Gen, w: u32) -> u32 {
    match g.below(8) {
        0 => w ^ (7 << 5),
        1 => w ^ (3 << 14),
        2 => w ^ (7 << 10),
        3 => w ^ (1 << 13),
        4 => w ^ (1 << 16),
        _ => w,
    }
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = unsafe { ref_dw4_mmio_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn dump(s: &MeshModelSortState, result: u32) -> Vec<u32> {
    let d = &s.def_mat_desc;
    let mut o = vec![result, s.sort_level as u32];
    for p in 0..4 {
        o.push(d.shader[p].0);
        match &d.shader_array[p] {
            Some(a) => {
                o.push(1);
                o.extend(a.iter().map(|x| x.0));
            }
            None => o.push(0),
        }
    }
    for p in 0..2 {
        o.push(d.texture0[p].unwrap_or(0) as u32);
        match &d.texture_array0[p] {
            Some(a) => {
                o.push(1);
                o.extend(a.iter().map(|x| x.unwrap_or(0) as u32));
            }
            None => o.push(0),
        }
        o.push(d.material_uv_source0[p] as u32);
        match &d.material_array_uv_source0[p] {
            Some(a) => {
                o.push(1);
                o.extend(a.iter().map(|&x| x as u32));
            }
            None => o.push(0),
        }
    }
    o
}

#[test]
fn mesh_model_sort_and_fog_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d404);
    let names = ["get_sort_flags(pass)", "get_sort_flags()", "compute_static_sort_levels", "modify_for_overbright", "post_process_fog"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let op = g.below(5) as usize;
        let mut pass_count = if g.below(3) == 0 { 2 } else { g.below(5) as usize };
        if op == 4 && g.below(2) == 0 {
            pass_count = 2;
        }
        let polys = g.below(9) as usize;
        let verts = 1 + g.below(8) as usize;
        let sort_level = [0, 10, 15, 20, 7][g.below(5) as usize];
        let arg = g.below(4);
        let mut shaders: Vec<u32> = (0..4).map(|_| shader(&mut g)).collect();
        let template = if op == 4 && pass_count == 2 { g.below(3) } else { 3 };
        match template {
            0 => {
                shaders[0] = mk(0, 1, 1, 0, 0, g.below(2), g.below(4), g.u32());
                shaders[1] = mk(2, 2, g.below(8), g.below(2), 1, g.below(2), g.below(4), g.u32());
                shaders[0] = perturb(&mut g, shaders[0]);
                shaders[1] = perturb(&mut g, shaders[1]);
            }
            1 => {
                shaders[0] = mk(0, 1, g.below(8), g.below(2), g.below(2), g.below(2), g.below(4), g.u32());
                shaders[1] = mk([4, 5, 1, 0][g.below(4) as usize], 1, g.below(8), g.below(2), g.below(2), g.below(2), g.below(4), g.u32());
                shaders[1] = perturb(&mut g, shaders[1]);
            }
            _ => {}
        }
        let mut arrays: Vec<Option<Vec<ShaderClass>>> = Vec::new();
        for p in 0..4 {
            // template cases mostly have no arrays, the other cases often do
            let has = if template < 3 { g.below(8) == 0 } else { g.below(3) == 0 };
            let _ = p;
            arrays.push(has.then(|| (0..polys).map(|_| ShaderClass(shader(&mut g))).collect()));
        }
        let tex0: Vec<Option<usize>> = (0..2).map(|_| if g.below(3) == 0 { None } else { Some(1 + g.below(1000) as usize) }).collect();
        let tex_arrays: Vec<Option<Vec<Option<usize>>>> = (0..2)
            .map(|_| (g.below(2) == 0).then(|| (0..polys).map(|_| if g.below(4) == 0 { None } else { Some(1 + g.below(1000) as usize) }).collect()))
            .collect();
        let mat_uv: Vec<i32> = (0..2).map(|_| g.below(6) as i32).collect();
        let mat_arrays: Vec<Option<Vec<i32>>> = (0..2).map(|_| (g.below(2) == 0).then(|| (0..verts).map(|_| g.below(6) as i32).collect())).collect();

        let mut inp = vec![op as u32, pass_count as u32, polys as u32, verts as u32, sort_level as u32, arg];
        for p in 0..4 {
            inp.push(shaders[p]);
            match &arrays[p] {
                Some(a) => {
                    inp.push(1);
                    inp.extend(a.iter().map(|x| x.0));
                }
                None => inp.push(0),
            }
        }
        for p in 0..2 {
            inp.push(tex0[p].unwrap_or(0) as u32);
            match &tex_arrays[p] {
                Some(a) => {
                    inp.push(1);
                    inp.extend(a.iter().map(|x| x.unwrap_or(0) as u32));
                }
                None => inp.push(0),
            }
            inp.push(mat_uv[p] as u32);
            match &mat_arrays[p] {
                Some(a) => {
                    inp.push(1);
                    inp.extend(a.iter().map(|&x| x as u32));
                }
                None => inp.push(0),
            }
        }
        let mut s = MeshModelSortState {
            def_mat_desc: MatDescView {
                pass_count,
                shader: shaders.iter().map(|&w| ShaderClass(w)).collect(),
                shader_array: arrays,
                texture0: tex0,
                texture_array0: tex_arrays,
                material_array_uv_source0: mat_arrays,
                material_uv_source0: mat_uv,
            },
            vertex_count: verts,
            poly_count: polys,
            sort_level,
            name: String::from("ref"),
        };
        let mut result = 0;
        match op {
            0 => result = s.get_sort_flags_pass(arg as usize),
            1 => result = s.get_sort_flags(),
            2 => s.compute_static_sort_levels(),
            3 => s.modify_for_overbright(),
            _ => s.post_process_fog(),
        }
        let rust = dump(&s, result);
        let cpp = run(&inp);
        c[op].eq(rust, cpp, || format!("case {case}: {inp:?}"));
    }
    finish(c);
}
