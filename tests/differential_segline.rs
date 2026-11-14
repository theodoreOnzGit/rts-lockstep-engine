#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/seglinerenderer.cpp and seglinerenderer.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #65: `SegLineRendererClass` ([`SegLineRenderer`]) against seglinerenderer.cpp compiled verbatim over a
//! recording device (`differential/wrap_segline.cpp`, which lists the stand-ins and the words).
//!
//! # Methodology
//!
//! 20,000 random sequences. Each reseeds the randomizers' shared `Random3Class`, makes the renderer at a random sync
//! time, then (90 %) `Init`s it from random line properties (each of the four flags, the mapping mode 0 to 3 (3 is not
//! a mode: kept as it was), subdivision level 0 to 7 (half of them 0), noise 0 to 3 (0 a third of the time), merge
//! abort factor 1.5, 0, negative or 0 to 3, tile factor -5 to 60, U and V rates of -2000 to 2000 per second), (85 %)
//! sets a texture or none, a random shader word, a width of 0 to 6, a colour of -0.2 to 1.2 and an opacity, then runs 1
//! to 6 operations: `Render` (70 %), `Scale` (10 %), `Reset_Line` (10 %), `Set_Texture_Tile_Factor` (10 %). A render
//! advances the clock 0 to 400 ms; the view is a random rotation (`build_matrix3d` of a random unit quaternion) with a
//! translation that puts the line 5 to 80 in front of the eye (now and then behind it); the transform is the identity
//! or another such matrix; the line is 1 to 40 points (1 in 20 lines 120 to 300: several chunks) of a random walk of
//! steps 0.02 to 5 with turns of every size, 1 point in 20 repeating the one before (the 0.001 nudge) and colours
//! (70 %) of -0.2 to 1.2. Compared, word for word (floats as bits, NaN equal to NaN): per chunk the sorting flag, every
//! vertex (position, packed colour, U, V), every triangle's indices, the shader, material and texture; after every
//! operation the renderer's state and the shared generator's seed and index.
//!
//! ~~A render where the Rust reports upstream undefined (a parallel merge of a parallel intersection reads an
//! uninitialised direction) ends the sequence: the C++ words up to that render are compared, the rest not.~~
//! **Corrected 2026-11-14 (#185):** the owner's decision on GitHub #185 (option (b)) gives that read a value on both
//! sides: `new_direction` starts as the current intersection's `Direction` (the Rust in
//! `formats::w3d::seglinerenderer`; the C++ by one asserted edit to the verbatim cut, `build.rs`), a deliberate
//! deviation from upstream, whose value is undefined. The filter is removed: every sequence is compared to its end, and
//! a render the Rust still reports undefined is counted and mismatches (its output is shorter than the C++'s).
//! Because the old filter also stopped drawing the sequence's remaining operations from the generator, the 20,000
//! sequences after the first that hit the read are not the same ones as before #185 (plant 10147 restores both the skip
//! and the old stream).
//!
//! # Predictions (written before the first run, `issue65_predictions.md`)
//!
//! At least one mismatch on the first run, in the merge path; then 0 mismatches. Undefined: under 5 % of the
//! sequences that merge. Every plant caught.
//!
//! # Results (2026-11-14)
//!
//! - First run (after the C++ build was fixed: seglinerenderer.cpp reaches `w3d_file.h` through `meshgeometry.h`, which
//!   the prelude replaces; the prelude now includes it): **0 mismatching sequences of 20,000**; ~~171,364,292 words
//!   compared (0 NaN-only), 91,961 chunks, 18,904,757 vertices~~ (**Corrected 2026-11-14 (#185):** these were measured
//!   with the sequences cut at the undefined read; the run after #185 is below). The prediction of a first-run mismatch
//!   missed.
//! - ~~**Undefined upstream: 8,520 sequences**, every one of them among the 14,527 that merge intersections (59 %;
//!   predicted under 5 %: a large miss). Upstream's parallel merge of a parallel intersection reads the uninitialised
//!   `new_direction` that often on these lines (short segments relative to the width, nearly straight runs); the words
//!   before that render are compared, the rest not.~~ **Corrected 2026-11-14 (#185):** the read now has the owner's
//!   value on both sides and nothing is cut (below). The 8,520 of 14,527 stands as the pre-#185 measurement (predicted
//!   under 5 %: a large miss).
//! - **Plants**, mismatching sequences of 20,000: 10096 11,292; 10097 3,133; 10098 746; 10099 6,748; 10100 13,676;
//!   10101 2,721; 10102 1,490; 10103 885; 10104 7,195; 10105 767; 10106 2,030; 10107 2,046; 10108 3,068; 10109 3,691;
//!   10110 39; 10111 12,222; 10112 9,444; 10113 1,896; 10114 12,235; 10115 3,764. **20 of 20 caught.** 10097 was first
//!   planted as a chunk size halved once more, which at level 7 made 1-point chunks and panicked the Rust (caught, but not
//!   measured); replanted as one point longer. (Measured before #185, on the cut sequences.)
//! - **#185 (2026-11-14).** Predictions (`issue185_predictions.md`, before the runs): the 8,520 sequences reach the end;
//!   0 undefined; 0 mismatching; about 300 million words (230 to 400 million); plant 10147 caught in exactly 8,520.
//!   **Results:** **0 mismatching sequences of 20,000**; **345,222,657 words** compared (0 NaN-only), 200,374 chunks,
//!   38,166,767 vertices; **0 undefined** (no other undefined case is reachable here: levels 0 to 7, 1 point or more);
//!   14,538 merging. The substituted branch is taken 2,163,857 times in **8,525 sequences** (counted once with a
//!   temporary trace, since removed), all now compared to their end. They are not the same 8,520 as before: the stream
//!   moved (see the methodology). **Plant 10147 caught in 8,520 sequences**, its stream exactly the old one (8,520
//!   undefined of 14,527 merging, 91,961 chunks, 18,904,757 vertices; each Rust output stops at the read). Prediction
//!   misses: 8,525 sequences, not 8,520 (the stream moved, which was not foreseen); 345 million words against about 300
//!   million (inside the range given).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 10096 | the V offset is not wrapped to 0..1 |
//! | 10097 | a chunk is one point longer |
//! | 10098 | the tiled V restarts at each chunk |
//! | 10099 | a fold does not switch the edges |
//! | 10100 | the parallel factor is 0.99 |
//! | 10101 | the backward merge test is left out |
//! | 10102 | a merge weighs the next point by its own count |
//! | 10103 | the merge abort test is left out |
//! | 10104 | the average plane is always the sum |
//! | 10105 | a merge does not take the next fold |
//! | 10106 | the last intersection is not copied after the merge pass |
//! | 10107 | the noise's X and Y are swapped |
//! | 10108 | the noise is not halved per level |
//! | 10109 | the second half of a split takes the midpoint colour |
//! | 10110 | the merge's copy takes `Parallel` too |
//! | 10111 | a strip step's second triangle is wound the other way |
//! | 10112 | the bottom vertex of a strip step takes the top U |
//! | 10113 | a top fan vertex leaves out the V offset |
//! | 10114 | the vertices are reversed |
//! | 10115 | the tile factor is clamped at 40 |
//! | 10147 | #185: a parallel merge of a parallel intersection is skipped again (the Rust reports it undefined) |

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::seglinerenderer::{SegLineChunk, SegLineRenderer};
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use rts_lockstep_engine::formats::w3d::w3d_file::W3dEmitterLinePropertiesStruct;
use rts_lockstep_engine::formats::w3d::W3dError;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::random::Random3;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::Gen;

fn planted() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn cases() -> u64 {
    std::env::var("SEGLINE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

/// One output word, and whether it is a float (compared NaN-equal).
type Word = (u32, bool);

fn wf(out: &mut Vec<Word>, f: f32) {
    out.push((f.to_bits(), true));
}
fn wu(out: &mut Vec<Word>, u: u32) {
    out.push((u, false));
}

fn state_words(out: &mut Vec<Word>, r: &SegLineRenderer, g: &Random3) {
    wf(out, r.width);
    wf(out, r.noise_amplitude);
    wf(out, r.texture_tile_factor);
    wf(out, r.merge_abort_factor);
    wu(out, r.subdivision_level);
    wu(out, r.bits);
    wu(out, r.last_used_sync_time);
    wf(out, r.current_uv_offset.x);
    wf(out, r.current_uv_offset.y);
    wf(out, r.uv_offset_delta_per_ms.x);
    wf(out, r.uv_offset_delta_per_ms.y);
    wu(out, g.seed as u32);
    wu(out, g.index as u32);
}

fn chunk_words(out: &mut Vec<Word>, c: &SegLineChunk) {
    wu(out, 'C' as u32);
    wu(out, u32::from(c.sorting));
    wu(out, c.vertices.len() as u32);
    for v in &c.vertices {
        wf(out, v.position.x);
        wf(out, v.position.y);
        wf(out, v.position.z);
        wu(out, v.diffuse);
        wf(out, v.u);
        wf(out, v.v);
    }
    wu(out, c.triangles.len() as u32);
    for t in &c.triangles {
        for i in t {
            wu(out, u32::from(*i));
        }
    }
    wu(out, c.shader.0);
    wu(out, if c.prelit_diffuse { 0 } else { 1 });
    wu(out, u32::from(c.texture.is_some()));
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

/// One random sequence: the input words, and the Rust's output words (with the index of the input operation where
/// the Rust found upstream undefined, if it did).
struct Case {
    input: Vec<u32>,
    rust: Vec<Word>,
    /// Output words before the render that was undefined, if one was (counted; since #185 not a filter: the sequence
    /// is still compared to the C++'s end, and the shorter Rust output mismatches).
    undefined_at: Option<usize>,
    merged: bool,
    chunks: u64,
    verts: u64,
}

fn build_case(g: &mut Gen) -> Case {
    let mut input = Vec::new();
    let mut rust = Vec::new();
    let s3a = g.u32() % 1000;
    let s3b = g.u32() % 1000;
    let mut sync = g.u32() % 100_000;
    input.extend([s3a, s3b, sync]);
    let mut gen3 = Random3::new(s3a, s3b);
    let mut r = SegLineRenderer::new(sync);
    let mut undefined_at = None;
    let (mut nchunks, mut nverts) = (0u64, 0u64);
    let mut ops: Vec<u32> = Vec::new();
    if g.below(10) < 9 {
        ops.push(0);
    }
    if g.below(100) < 85 {
        ops.push(1);
    }
    for _ in 0..1 + g.below(6) {
        let k = g.below(10);
        ops.push(match k {
            0..=6 => 2,
            7 => 3,
            8 => 4,
            _ => 5,
        });
    }
    for op in ops {
        let out_before = rust.len();
        wu(&mut rust, op);
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
                let tile = g.uniform(-5.0, 60.0);
                let (u, v) = (g.uniform(-2000.0, 2000.0), g.uniform(-2000.0, 2000.0));
                input.extend([flags, level, noise.to_bits(), f32::to_bits(abort), tile.to_bits(), u.to_bits(), v.to_bits()]);
                r.init(&W3dEmitterLinePropertiesStruct {
                    flags,
                    subdivision_level: level,
                    noise_amplitude: noise,
                    merge_abort_factor: abort,
                    texture_tile_factor: tile,
                    u_per_sec: u,
                    v_per_sec: v,
                    reserved: [0; 9],
                });
            }
            1 => {
                let tex = g.below(2);
                let shader = g.u32();
                let width = g.uniform(0.0, 6.0);
                let c = [g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2)];
                input.extend([tex, shader, width.to_bits(), c[0].to_bits(), c[1].to_bits(), c[2].to_bits(), c[3].to_bits()]);
                r.texture = if tex != 0 { Some(b"segline".to_vec()) } else { None };
                r.shader = ShaderClass(shader);
                r.width = width;
                r.color = Vector3::new(c[0], c[1], c[2]);
                r.opacity = c[3];
            }
            2 => {
                sync = sync.wrapping_add(g.below(401));
                input.push(sync);
                // the view: a rotation, and the line's centre put 5 to 80 in front of the eye (-Z), now and then behind
                let mut view = rotation(g);
                let centre = Vector3::new(g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(-10.0, 10.0));
                let depth = if g.below(20) == 0 { g.uniform(-30.0, 5.0) } else { g.uniform(5.0, 80.0) };
                let c = view.mul_vector3(&centre);
                view.row[0].w = -c.x + g.uniform(-3.0, 3.0);
                view.row[1].w = -c.y + g.uniform(-3.0, 3.0);
                view.row[2].w = -c.z - depth;
                let transform = if g.below(2) == 0 {
                    Matrix3D::identity()
                } else {
                    let mut t = rotation(g);
                    t.row[0].w = g.uniform(-2.0, 2.0);
                    t.row[1].w = g.uniform(-2.0, 2.0);
                    t.row[2].w = g.uniform(-2.0, 2.0);
                    t
                };
                input.extend(view.to_array().iter().map(|f| f.to_bits()));
                input.extend(transform.to_array().iter().map(|f| f.to_bits()));
                let n = if g.below(20) == 0 { 120 + g.below(181) } else { 1 + g.below(40) } as usize;
                let has_cols = g.below(10) < 7;
                input.push(n as u32);
                input.push(u32::from(has_cols));
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
                if has_cols {
                    for _ in 0..n {
                        let c = Vector4::new(g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2));
                        cols.push(c);
                        input.extend([c.x.to_bits(), c.y.to_bits(), c.z.to_bits(), c.w.to_bits()]);
                    }
                }
                match r.render(sync, &view, &transform, &pts, if has_cols { Some(&cols) } else { None }, &mut gen3) {
                    Ok(chunks) => {
                        for c in &chunks {
                            chunk_words(&mut rust, c);
                            nchunks += 1;
                            nverts += c.vertices.len() as u64;
                        }
                        wu(&mut rust, 'E' as u32);
                    }
                    Err(W3dError::Undefined(_)) => {
                        undefined_at = Some(out_before);
                        break;
                    }
                    Err(e) => panic!("unexpected {e:?}"),
                }
            }
            3 => {
                let f = g.uniform(0.0, 3.0);
                input.push(f.to_bits());
                r.scale(f);
            }
            4 => {
                sync = sync.wrapping_add(g.below(401));
                input.push(sync);
                r.reset_line(sync);
            }
            _ => {
                let f = g.uniform(-10.0, 70.0);
                input.push(f.to_bits());
                r.set_texture_tile_factor(f);
            }
        }
        state_words(&mut rust, &r, &gen3);
    }
    let merged = r.is_merge_intersections();
    Case { input, rust, undefined_at, merged, chunks: nchunks, verts: nverts }
}

/// *Results (2026-11-14):* 20,000 sequences, 0 mismatching, ~~8,520 ended at upstream's uninitialised read~~
/// **Corrected 2026-11-14 (#185):** 345,222,657 words, every sequence compared to its end, 0 undefined (see the module
/// docs).
#[test]
fn segline_renderer_matches_upstream() {
    let mut g = Gen::new(0x5E6_11AE_65);
    let n = cases();
    let (mut words, mut mismatched, mut undefined, mut undefined_merging, mut merging) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut chunks, mut vertices, mut nan_only) = (0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    let mut out = vec![0u32; 1 << 22];
    for case in 0..n {
        let c = build_case(&mut g);
        let len = unsafe { cpp::ref_segline_seq(c.input.as_ptr(), c.input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        assert!(len <= out.len(), "output buffer too small");
        let cpp_words = &out[..len];
        if c.merged {
            merging += 1;
        }
        // #185: an undefined render no longer ends the comparison (the filter that did is removed): every sequence is
        // compared to its end, so a Rust that stops at an undefined render is a mismatch (its output is shorter).
        if c.undefined_at.is_some() {
            undefined += 1;
            if c.merged {
                undefined_merging += 1;
            }
        }
        let mut bad = cpp_words.len() != c.rust.len();
        for (i, (rw, cw)) in c.rust.iter().zip(cpp_words).enumerate() {
            words += 1;
            if rw.0 != *cw {
                if rw.1 && f32::from_bits(rw.0).is_nan() && f32::from_bits(*cw).is_nan() {
                    nan_only += 1;
                    continue;
                }
                bad = true;
                if examples.len() < 6 {
                    examples.push(format!("case {case} word {i}: rust {:#010x} c++ {:#010x}", rw.0, cw));
                }
                break;
            }
        }
        chunks += c.chunks;
        vertices += c.verts;
        if bad {
            mismatched += 1;
            if examples.len() < 6 && cpp_words.len() != c.rust.len() {
                examples.push(format!("case {case}: rust {} words, c++ {} words (undefined at {:?})", c.rust.len(), cpp_words.len(), c.undefined_at));
            }
        }
    }
    let line = format!(
        "segline: {n} sequences, {words} words compared ({nan_only} NaN-only), {chunks} chunks, {vertices} vertices, {undefined} undefined \
         ({undefined_merging} of {merging} merging), mismatching sequences {mismatched}"
    );
    if planted() != 0 {
        println!("MEASURED plant {} {line}", planted());
        for e in &examples {
            println!("    e.g. {e}");
        }
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    println!("DIFF {line}");
    assert_eq!(mismatched, 0, "{line}; first: {examples:#?}");
}
