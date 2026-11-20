// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/streak_render.rs` (`StreakRendererClass`: `new`, `init`, the setters, `render_streak`, with
//! `subdivision_util` inside it) against upstream `WW3D2/streakRender.cpp` compiled whole over a recording device (build.rs `drafts_ww3d6`,
//! differential/wrap_draft_w6_streak.cpp and differential/shim_draft_ww3d6/streak_prelude.h, a copy of the segmented-line renderer's recorder). The one named edit of the cut
//! makes `RenderStreak`'s read of the uninitialised `new_direction` (a parallel merge of a parallel intersection, as in `seglinerenderer.cpp`, GitHub #185) start from zero, the
//! value the draft documents, and counts the renders that ran that branch. `DX8Wrapper::Convert_Color_Clamp` is cut verbatim from `dx8wrapper.h` over a transcription of
//! `Clamp_Color`'s CMOV asm; `Convert_Color` is the mesh library's x87 transcription; `VectorProcessorClass::Transform`, `Matrix3D::mul` and the shared
//! `Vector3Randomizer::Randomizer` are upstream's own, linked from other libraries.
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). Each reseeds the shared `Random3Class`, then (90 %) `Init`s the renderer from random line properties (each of the four
//! flags, the mapping mode 0 to 3, subdivision level 0 to 7, noise 0 to 3, merge abort factor 1.5, 0, negative or 0 to 3), (85 %) sets a texture or none, a random shader word,
//! a width, a colour and an opacity, then runs 1 to 4 `RenderStreak`s: a view that is a random rotation putting the line 5 to 80 in front of the eye (now and then behind it), a
//! transform that is the identity or another rotation, 1 to 40 points (1 in 20 lines 120 to 300, several chunks) of a random walk of steps 0.02 to 5 with turns of every size,
//! 1 point in 20 repeating the one before, an RGBA per point plus one (upstream reads `colors[MIN(i/2, point_cnt)]`, one past the points, with end caps), a width per point plus one, and
//! random personalities. Compared, word for word (floats as bits, NaN equal to NaN): per chunk the sorting flag, every vertex (position, packed colour, U, V), every triangle,
//! the shader, material and texture; after every operation the renderer's width, noise, merge abort factor, subdivision level, bits and the shared generator's seed and index.
//!
//! # Excluded inputs
//!
//! The renders that ran the undefined read are compared too, with zero as its value on both sides (counted and printed); that value is the draft's, not upstream's.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! Not 0. `Set_Current_Subdivision_Level` in `streakRender.h` stores the level and then `SubdivisionLevel = 0` ("Streak renderer does not work for non-zero subdivisions"), where
//! the draft's `set_current_subdivision_level` keeps the level: I expect every sequence that `Init`s with a non-zero level to differ. Everything else: 0.
//!
//! # Results (2026-11-20)
//!
//! **First run: the draft panicked** (`index out of bounds` at `radius = widths[sidx]`): with a non-zero subdivision level the draft subdivides the points and then indexes `widths`
//! (one per input point) with the subdivided segment index, which upstream never does because `Set_Current_Subdivision_Level` forces the level to 0. **Prediction held** (a
//! failure for every sequence that `Init`s with a non-zero level). Fixed in the draft: `set_current_subdivision_level` stores 0. **After the fix: 0 mismatching sequences of
//! 20,000**: 49,949 renders, 51,733 chunks; 11,347 sequences contain a render that ran the undefined `new_direction` read (compared with zero on both sides). The
//! packed colours go through `DX8Wrapper::Convert_Color_Clamp` (verbatim), so `dynamesh.rs`'s `convert_color_clamp` is compared too, on colours in -0.2..1.2.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching sequences of 20,000)
//!   19141: 19,885; 19142: 16,589; 19143: 19,885; 19144: 19,758; 19145: 19,626; 19146: 18,869; 19147: 2,273; 19148: 14,661; 19149: 19,885
//!
//! **9 of 9 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::streak_render::StreakRendererClass;
use rts_lockstep_engine::formats::w3d::seglinerenderer::SegLineChunk;
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::random::Random3;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_streak_seq(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn wf(out: &mut Vec<u32>, f: f32) {
    out.push(f.to_bits());
}

fn chunk_words(out: &mut Vec<u32>, c: &SegLineChunk) {
    out.push('C' as u32);
    out.push(u32::from(c.sorting));
    out.push(c.vertices.len() as u32);
    for v in &c.vertices {
        wf(out, v.position.x);
        wf(out, v.position.y);
        wf(out, v.position.z);
        out.push(v.diffuse);
        wf(out, v.u);
        wf(out, v.v);
    }
    out.push(c.triangles.len() as u32);
    for t in &c.triangles {
        for i in t {
            out.push(u32::from(*i));
        }
    }
    out.push(c.shader.0);
    out.push(if c.prelit_diffuse { 0 } else { 1 });
    out.push(u32::from(c.texture.is_some()));
}

fn rotation(g: &mut Gen) -> Matrix3D {
    let mut q = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
    let l = (q.iter().map(|x| x * x).sum::<f32>()).max(1e-6);
    let k = 1.0 / rts_lockstep_engine::ww::wwmath::sqrt(l);
    for x in &mut q {
        *x *= k;
    }
    build_matrix3d(&Quaternion::new(q[0], q[1], q[2], q[3]))
}

fn state_words(out: &mut Vec<u32>, r: &StreakRendererClass, g: &Random3) {
    wf(out, r.width);
    wf(out, r.noise_amplitude);
    wf(out, r.merge_abort_factor);
    out.push(r.subdivision_level);
    out.push(r.bits);
    out.push(g.seed as u32);
    out.push(g.index as u32);
}

/// word equality with NaN equal to NaN
fn same(a: &[u32], b: &[u32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || (f32::from_bits(*x).is_nan() && f32::from_bits(*y).is_nan()))
}

#[test]
fn streak_render_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_09);
    let mut chk = Check::new("StreakRendererClass (Init, setters, RenderStreak)");
    let mut out = vec![0u32; 4_000_000];
    let (mut undefined_renders, mut renders, mut chunks_total) = (0u64, 0u64, 0u64);
    for case in 0..support_sweep::cases() {
        let mut input = Vec::new();
        let mut rust = Vec::new();
        let (s3a, s3b) = (g.u32() % 1000, g.u32() % 1000);
        input.extend([s3a, s3b]);
        let mut gen3 = Random3::new(s3a, s3b);
        let mut r = StreakRendererClass::new();
        let mut ops: Vec<u32> = Vec::new();
        if g.below(10) < 9 {
            ops.push(0);
        }
        if g.below(100) < 85 {
            ops.push(1);
        }
        for _ in 0..1 + g.below(4) {
            ops.push(2);
        }
        let mut flags_used = 0u32;
        let mut level_used = 0u32;
        for op in ops {
            rust.push(op);
            input.push(op);
            match op {
                0 => {
                    let mut flags = 0;
                    if g.below(10) < 7 {
                        flags |= 1;
                    }
                    if g.below(10) < 3 {
                        flags |= 2;
                    }
                    if g.below(10) < 2 {
                        flags |= 4;
                    }
                    if g.below(10) < 2 {
                        flags |= 8;
                    }
                    flags |= g.below(4) << 24;
                    let level = if g.below(2) == 0 { 0 } else { g.below(8) };
                    let noise = if g.below(3) == 0 { 0.0 } else { g.uniform(0.0, 3.0) };
                    let abort = match g.below(4) {
                        0 => 1.5,
                        1 => 0.0,
                        2 => -g.uniform(0.0, 2.0),
                        _ => g.uniform(0.0, 3.0),
                    };
                    input.extend([flags, level, noise.to_bits(), abort.to_bits()]);
                    r.init(flags, level, noise, abort);
                    flags_used = flags;
                    level_used = level;
                }
                1 => {
                    let tex = g.below(2);
                    let shader = g.u32();
                    let width = g.uniform(0.0, 6.0);
                    let c = [g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2)];
                    input.extend([tex, shader, width.to_bits(), c[0].to_bits(), c[1].to_bits(), c[2].to_bits(), c[3].to_bits()]);
                    r.texture = if tex != 0 { Some(b"streak".to_vec()) } else { None };
                    r.shader = ShaderClass(shader);
                    r.width = width;
                    r.color = Vector3::new(c[0], c[1], c[2]);
                    r.opacity = c[3];
                }
                _ => {
                    let mut view = rotation(&mut g);
                    let centre = Vector3::new(g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(-10.0, 10.0));
                    let depth = if g.below(20) == 0 { g.uniform(-30.0, 5.0) } else { g.uniform(5.0, 80.0) };
                    let c = view.mul_vector3(&centre);
                    view.row[0].w = -c.x + g.uniform(-3.0, 3.0);
                    view.row[1].w = -c.y + g.uniform(-3.0, 3.0);
                    view.row[2].w = -c.z - depth;
                    let transform = if g.below(2) == 0 {
                        Matrix3D::identity()
                    } else {
                        let mut t = rotation(&mut g);
                        t.row[0].w = g.uniform(-2.0, 2.0);
                        t.row[1].w = g.uniform(-2.0, 2.0);
                        t.row[2].w = g.uniform(-2.0, 2.0);
                        t
                    };
                    input.extend(view.to_array().iter().map(|f| f.to_bits()));
                    input.extend(transform.to_array().iter().map(|f| f.to_bits()));
                    let n = if g.below(20) == 0 { 120 + g.below(181) } else { 1 + g.below(40) } as usize;
                    input.push(n as u32);
                    let mut pts = Vec::with_capacity(n);
                    let mut p = centre;
                    let mut dir = Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0));
                    let turn = [0.05f32, 0.3, 1.0, 3.0][g.below(4) as usize];
                    let step = [0.02f32, 0.3, 1.5, 5.0][g.below(4) as usize];
                    for i in 0..n {
                        if i > 0 && g.below(20) != 0 {
                            dir = dir + Vector3::new(g.uniform(-turn, turn), g.uniform(-turn, turn), g.uniform(-turn, turn));
                            p = p + dir * (step * g.uniform(0.2, 1.0));
                        }
                        pts.push(p);
                        input.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
                    }
                    let mut cols = Vec::new();
                    for _ in 0..=n {
                        let c = Vector4::new(g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2));
                        cols.push(c);
                        input.extend([c.x.to_bits(), c.y.to_bits(), c.z.to_bits(), c.w.to_bits()]);
                    }
                    let mut widths = Vec::new();
                    for _ in 0..=n {
                        let w = g.uniform(0.0, 3.0);
                        widths.push(w);
                        input.push(w.to_bits());
                    }
                    let m = 4 * n + 8;
                    let pers: Vec<u32> = (0..m).map(|_| g.u32()).collect();
                    input.push(m as u32);
                    input.extend(pers.iter().copied());
                    let chunks = r.render_streak(&view, &transform, &pts, &cols, &widths, &pers, &mut gen3);
                    chunks_total += chunks.len() as u64;
                    renders += 1;
                    for c in &chunks {
                        chunk_words(&mut rust, c);
                    }
                    rust.push('E' as u32);
                }
            }
            state_words(&mut rust, &r, &gen3);
        }
        let _ = (flags_used, level_used);
        let total = unsafe { ref_w6_streak_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        assert!(total <= out.len(), "case {case}: output buffer too small ({total})");
        undefined_renders += u64::from(out[total - 1] != 0);
        let cpp = &out[..total - 1];
        chk.compared += 1;
        if !same(&rust, cpp) {
            chk.mismatches += 1;
            if chk.examples.len() < 6 {
                let at = rust.iter().zip(cpp).position(|(a, b)| a != b);
                chk.examples.push(format!("case {case}: lengths rust {} c++ {}, first differing word {at:?}: rust {:?} c++ {:?}; Init level {level_used} flags {flags_used:#x}", rust.len(), cpp.len(), at.map(|i| rust[i]), at.map(|i| cpp[i])));
            }
        }
    }
    println!("renders {renders}, chunks {chunks_total}, sequences with a render that ran the undefined read: {undefined_renders}");
    for e in &chk.examples {
        println!("EXAMPLE {e}");
    }
    support_sweep::finish(vec![chk]);
}
