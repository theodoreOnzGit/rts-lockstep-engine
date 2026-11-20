// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/bwrender.rs` (`BWBuffer`, `BWRenderClass`, the
//! `BW_Render` alias) against upstream `WW3D2/bwrender.cpp` compiled whole (build.rs `drafts_ww3d6`,
//! differential/wrap_draft_w6_bw.cpp). One mechanical edit: the 10 `WWMath::Float_To_Long` calls name the MSVC x86 `fistp` branch
//! (the shipped game's; the Linux build of `wwmath.h` truncates). `VectorProcessorClass::MulAdd` is cut verbatim from `vp.cpp`.
//! `bw_render.cpp` (`BW_Render`) is the same code under the old name (`diff` shows only the class name, include and header comments).
//! Stand-in: `memset`, which records a clear that would leave the pixel buffer (a wild write upstream) and writes nothing.
//!
//! # Method
//!
//! 20,000 programs (fixed seed, `DRAFT_CASES` overrides): a square buffer of scale 5 to 64 (mostly powers of two), an initial
//! fill, then 1 to 6 operations from `Fill`, `Set_Vertex_Locations` (3 to 40 vertices in -1..1 space and a little beyond, a share of
//! them placed so that the scaled coordinate lands on an integer or a half-integer, where `fistp` rounds to even), `Render_Triangles`
//! and `Render_Triangle_Strip` (random indices, degenerate and repeated, both windings) and a direct `Buffer::Set_H_Line` (any
//! start, end and row, clipped or not). Compared: every pixel of the buffer, the scaled vertices (bits) and the wild flag.
//!
//! # Excluded inputs
//!
//! A program for which the C++ stand-in `memset` is asked to clear outside the buffer is dropped (counted and printed): upstream
//! writes wild there (`end_x < start_x` after clipping gives a huge `size_t`), the draft clamps it to nothing.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches: the draft copies the control flow and uses the verified `float_to_long_fistp`.
//!
//! # Results (2026-11-20)
//!
//! **0 mismatches** over 19,236 programs (pixels and scaled vertices each); 764 programs dropped for a wild `memset` in the reference. **Prediction held.**
//! `bw_render.cpp` (`BW_Render`) differs from `bwrender.cpp` only by the class name, the include and header comments (checked with `diff`).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching comparisons, both checks)
//!   19061: 5,515; 19062: 4,039; 19063: 6,375; 19064: 7,975; 19065: 6,073; 19066: 3,404; 19067: 3,518; 19068: 1,584; 19069: 5,660; 19070: 3,219
//!   (an earlier 19065, `r <= 0` in `cull`, was an equivalent mutant (0 mismatches: a zero-area triangle draws nothing either way) and was replaced by a sign slip.)
//!
//! **10 of 10 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::bwrender::*;
use rts_lockstep_engine::ww::vector2::Vector2;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_bw_run(input: *const u32, out_bytes: *mut u8, out_words: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn coord(g: &mut Gen, scale: i32) -> f32 {
    let hs = scale as f32 * 0.5;
    match g.below(10) {
        0..=3 => {
            let t = (g.below(2 * scale as u32 * 2 + 16) as f32) * 0.5 - 4.0;
            (t - hs) / hs
        }
        4..=7 => g.uniform(-1.2, 1.2),
        8 => [-1.0f32, 0.0, 1.0][g.below(3) as usize],
        _ => g.uniform(-3.0, 3.0),
    }
}

#[test]
fn bwrender_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_01);
    let mut pixels = Check::new("BWRenderClass pixels");
    let mut verts = Check::new("Set_Vertex_Locations vertices");
    let mut dropped = 0u64;
    for case in 0..support_sweep::cases() {
        let scale = [5, 6, 7, 8, 16, 16, 32, 32, 32, 48, 64, 64][g.below(12) as usize];
        let init = g.below(256);
        let mut input: Vec<u32> = vec![scale as u32, init, 0];
        let mut r = BWRenderClass::new(vec![init as u8; (scale * scale) as usize], scale);
        let mut nverts = 0usize;
        let mut nops = 0u32;
        let want = 1 + g.below(6);
        let mut last_verts: Vec<Vector2> = Vec::new();
        let mut have_verts = false;
        while nops < want || !have_verts {
            let op = if !have_verts { 1 } else { g.below(10) };
            nops += 1;
            match op {
                0 => {
                    let c = g.below(256);
                    input.extend([0, c]);
                    r.fill(c as u8);
                }
                1 => {
                    let n = 3 + g.below(38) as usize;
                    input.extend([1, n as u32]);
                    let mut vs = Vec::new();
                    for _ in 0..n {
                        let (x, y) = (coord(&mut g, scale), coord(&mut g, scale));
                        input.extend([x.to_bits(), y.to_bits()]);
                        vs.push(Vector2::new(x, y));
                    }
                    r.set_vertex_locations(vs);
                    last_verts = r.vertices.clone();
                    nverts = n;
                    have_verts = true;
                }
                2..=6 | 7 => {
                    let strip = op == 7;
                    let n = if strip { g.below(14) as usize } else { 3 * g.below(8) as usize + g.below(3) as usize * (g.below(4) / 3) as usize };
                    let mut idx = Vec::new();
                    for _ in 0..n {
                        idx.push(g.below(nverts as u32));
                    }
                    if g.below(5) == 0 && n >= 3 {
                        idx[1] = idx[0];
                    }
                    input.extend([if strip { 3 } else { 2 }, n as u32]);
                    input.extend(idx.iter().copied());
                    if strip {
                        r.render_triangle_strip(&idx);
                    } else {
                        r.render_triangles(&idx);
                    }
                }
                _ => {
                    let sx = g.below(scale as u32 + 12) as i32 - 6;
                    let ex = g.below(scale as u32 + 12) as i32 - 6;
                    let y = g.below(scale as u32 + 12) as i32 - 6;
                    input.extend([4, sx as u32, ex as u32, y as u32]);
                    r.pixel_buffer.set_h_line(sx, ex, y);
                }
            }
        }
        input[2] = nops;
        let mut out_bytes = vec![0u8; (scale * scale) as usize];
        let mut out_words = vec![0u32; 2 + 2 * 64];
        let n = unsafe { ref_w6_bw_run(input.as_ptr(), out_bytes.as_mut_ptr(), out_words.as_mut_ptr()) } as usize;
        if out_words[0] != 0 {
            dropped += 1;
            continue;
        }
        let _ = &last_verts;
        pixels.eq(r.pixel_buffer.buffer.clone(), out_bytes, || format!("case {case} scale {scale} input {input:?}"));
        let mut rv: Vec<u32> = vec![r.vertices.len() as u32];
        for v in &r.vertices {
            rv.extend([v.x.to_bits(), v.y.to_bits()]);
        }
        verts.eq(rv, out_words[1..n].to_vec(), || format!("case {case} scale {scale}"));
    }
    println!("programs dropped for a wild memset in the C++ reference: {dropped}");
    support_sweep::finish(vec![pixels, verts]);
}
