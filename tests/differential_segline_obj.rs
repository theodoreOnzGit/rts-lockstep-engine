#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/segline.cpp and segline.h (SegmentedLineClass)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/seglinerenderer.cpp (the renderer it drives)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h, rendobj.cpp (the render object parts it uses)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #169, #65: `SegmentedLineClass` ([`SegmentedLine`]) against segline.cpp compiled verbatim on the verbatim
//! renderer (`differential/wrap_segline_obj.cpp`, which documents the words), from the set calls to the rendered
//! chunks.
//!
//! # Methodology
//!
//! 20,000 random sequences (`SEGLINE_OBJ_CASES`). Each reseeds the randomizers' shared generator (`Random3Class`, seeds
//! below 1000), makes a line at a random WW3D sync time, and runs 1 to 40 operations: `Set_Points` (0 to 9 points, so
//! the "fewer than two" assertion path is taken; a random walk about the case's centre, now and then a repeated point,
//! 1 point in 30 from `Gen::mixed`, NaN and infinities included) 12 %; `Set_Point_Location` 6 % and `Delete_Point` 5 %
//! (indices up to two past the end); `Get_Point_Location` 4 %; `Add_Point` 6 %; the texture, shader (the additive preset,
//! the additive sprite preset or random bits), width (-2 to 10), colour and opacity 8 %; the noise (-3 to 3), merge abort
//! factor, subdivision levels (0 to 9), mapping mode (0 to 2; 1 in 25 a fourth, undefined upstream at the next render,
//! where the comparison of that sequence stops), tile factor (-5 to 60), UV rate and the four flags 8 %;
//! `Render` 20 % (the clock on by up to 400 ms, now and then back; a random rotation and the centre 5 to 80 in front of
//! the eye, 1 in 20 behind); `Prepare_LOD` 6 % (a screen size of 0 to 1); `Increment_LOD` 6 %; `Decrement_LOD` 4 %;
//! `Set_LOD_Level` 4 % (-3 to 9); `Reset_Line` 3 %; the render object's visible, hidden, animation-hidden and
//! force-visible bits and collision type 4 %; the line replaced by its copy 2 % and by a fresh line assigned from it 2
//! %; the transform (a rotation and a translation) 2 %. Compared, every word as bits (NaNs on both sides equal): each
//! operation's records (the point read, the LOD request and cost, each chunk's vertices, triangles, shader, material,
//! sorting and texture) and after each operation the whole state (points, every getter, polygons, cost, value,
//! post-increment value, LOD level and count, the bounding box and sphere, the bits, the renderer's clock and UV offset,
//! and the shared generator's seed and index).
//!
//! # Predictions (written before the first run, `issue169b_predictions.md`)
//!
//! 0 mismatches, max ULP 0 (NaN-sign differences on both sides counted equal).
//!
//! # Results (2026-11-14)
//!
//! - First run: the link failed (the stand-ins `RenderObjClass::AT_MIN_LOD`/`AT_MAX_LOD` clashed with the particle
//!   buffer's of the same names): the stand-ins are renamed `RefSegLineObj*`.
//! - Second run: the C++ aborted ("double free or corruption"): upstream's copy constructor and `operator=` share the
//!   points' array (`SimpleDynVecClass` has neither); the driver now detaches the source's array before releasing it
//!   (`detach_points`), see the module docs of `formats::w3d::segline`.
//! - Third run: about 30 of the first 250 sequences differed in the vertices' U: a fourth mapping mode, which only
//!   `SegmentedLineClass`'s setter can store, reads the renderer's `u_values` and `base_tex_v` uninitialised upstream; the
//!   Rust renderer treated it as tiled. It now reports it undefined, and the sequence is compared up to that render.
//! - Fourth run: 20,000 sequences, 406,087 operations, 81,149 renders, 46,120 chunks, 1,820,793 vertices, 6,153 renders
//!   drawing from the generator, 528 sequences stopped at an undefined render, 38,269,878 words compared (733 NaN on
//!   both sides with different bits); **0 mismatches, max ULP 0**.
//! - Planted bugs, mismatching sequences of 20,000: 10176 4,337; 10177 2,605; 10178 3,404; 10179 4,017; 10180 4,075;
//!   10181 7,804; 10182 7,306; 10183 8,414; 10184 11,227; 10185 19,984; 10186 10,949; 10187 9,685; 10188 8,170; 10189
//!   285; 10190 6,857; 10191 1,684; 10192 6,243; 10193 9,233; 10194 3,288; 10195 1,314. **20 of 20 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 10176 | `Set_Points` takes a single point |
//! | 10177 | `Set_Point_Location` cannot reach the last point |
//! | 10178 | `Get_Point_Location` past the list answers the last point |
//! | 10179 | `Delete_Point` moves the last point into the hole |
//! | 10180 | `Set_Width` does not clamp at 0 |
//! | 10181 | `Set_Noise_Amplitude` keeps the sign |
//! | 10182 | `Set_Subdivision_Levels` caps at 6 |
//! | 10183 | `Get_Num_Polys` multiplies by the level, not 2^level |
//! | 10184 | `Render` also needs the visibility bit |
//! | 10185 | the sphere's radius is the extent's x |
//! | 10186 | the box is grown by the whole width |
//! | 10187 | the midpoints' box adds the noise once, not twice |
//! | 10188 | the midpoints' box leaves out the last segment |
//! | 10189 | `Prepare_LOD` does not clamp the level |
//! | 10190 | `Increment_LOD` does not step |
//! | 10191 | `Decrement_LOD` stops at 1 |
//! | 10192 | `Get_Value`'s benefit divides by the polygons, not their square |
//! | 10193 | `Get_Post_Increment_Value` does not double the polygons |
//! | 10194 | `Set_LOD_Level` does not clamp at 0 |
//! | 10195 | the copy keeps the source's render-object bits |

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::segline::{LodRequest, SegmentedLine};
use rts_lockstep_engine::formats::w3d::seglinerenderer::SegLineChunk;
use rts_lockstep_engine::formats::w3d::shader::{ShaderClass, PRESET_ADDITIVE_SHADER, PRESET_ADDITIVE_SPRITE_SHADER};
use rts_lockstep_engine::formats::w3d::{W3dError, W3dUndefined};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::random::Random3;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

/// The wrappers' records are global buffers: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("SEGLINE_OBJ_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn wf(out: &mut Vec<u32>, f: f32) {
    out.push(f.to_bits());
}

fn fw(w: u32) -> f32 {
    f32::from_bits(w)
}

fn chunk_words(out: &mut Vec<u32>, c: &SegLineChunk) {
    out.extend(['C' as u32, u32::from(c.sorting), c.vertices.len() as u32]);
    for v in &c.vertices {
        out.extend([v.position.x.to_bits(), v.position.y.to_bits(), v.position.z.to_bits(), v.diffuse, v.u.to_bits(), v.v.to_bits()]);
    }
    out.push(c.triangles.len() as u32);
    for t in &c.triangles {
        out.extend(t.map(u32::from));
    }
    out.extend([c.shader.0, if c.prelit_diffuse { 0 } else { 1 }, u32::from(c.texture.is_some())]);
}

/// The state words after each operation (the wrapper's `line_state`).
fn state_words(out: &mut Vec<u32>, l: &SegmentedLine, g: &Random3) {
    out.push(l.get_num_points() as u32);
    for p in &l.point_locations {
        out.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
    }
    out.push(u32::from(l.get_texture().is_some()));
    out.push(l.get_shader().0);
    wf(out, l.get_width());
    let c = l.get_color();
    out.extend([c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]);
    wf(out, l.get_opacity());
    wf(out, l.get_noise_amplitude());
    wf(out, l.get_merge_abort_factor());
    out.push(l.get_subdivision_levels());
    out.push(l.get_texture_mapping_mode());
    wf(out, l.get_texture_tile_factor());
    let r = l.get_uv_offset_rate();
    out.extend([r.x.to_bits(), r.y.to_bits()]);
    out.extend([l.is_merge_intersections(), l.is_freeze_random(), l.is_sorting_disabled(), l.are_end_caps_enabled()].map(u32::from));
    out.push(l.get_num_polys() as u32);
    wf(out, l.get_cost());
    wf(out, l.get_value());
    wf(out, l.get_post_increment_value());
    out.push(l.get_lod_level() as u32);
    out.push(l.get_lod_count() as u32);
    let b = l.get_obj_space_bounding_box();
    for f in [b.center.x, b.center.y, b.center.z, b.extent.x, b.extent.y, b.extent.z] {
        wf(out, f);
    }
    let s = l.get_obj_space_bounding_sphere();
    for f in [s.center.x, s.center.y, s.center.z, s.radius] {
        wf(out, f);
    }
    out.push(l.bits);
    out.push(l.line_renderer.last_used_sync_time);
    wf(out, l.line_renderer.current_uv_offset.x);
    wf(out, l.line_renderer.current_uv_offset.y);
    out.push(g.seed as u32);
    out.push(g.index as u32);
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

fn point(g: &mut Gen, at: &mut Vector3) -> Vector3 {
    if g.below(30) == 0 {
        return Vector3::new(g.mixed(), g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0));
    }
    if g.below(15) != 0 {
        let step = [0.02f32, 0.5, 3.0, 10.0][g.below(4) as usize];
        *at = *at + Vector3::new(g.uniform(-step, step), g.uniform(-step, step), g.uniform(-step, step));
    }
    *at
}

fn push_v(input: &mut Vec<u32>, p: &Vector3) {
    input.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
}

/// Counts over the run.
#[derive(Default)]
struct Counts {
    ops: u64,
    renders: u64,
    chunks: u64,
    verts: u64,
    random_draws: u64,
}

/// One random case: the input words, the Rust's output words, and whether the Rust stopped at a render undefined
/// upstream (the input then ends at that render).
fn build_case(g: &mut Gen, n: &mut Counts) -> (Vec<u32>, Vec<u32>, bool) {
    let mut input = Vec::new();
    let mut out = Vec::new();
    let (s3a, s3b) = (g.u32() % 1000, g.u32() % 1000);
    let mut sync = g.u32() % 100_000;
    input.extend([s3a, s3b, sync]);
    let mut gen3 = Random3::new(s3a, s3b);
    let mut l = SegmentedLine::new(sync);
    let centre = Vector3::new(g.uniform(-100.0, 100.0), g.uniform(-100.0, 100.0), g.uniform(-20.0, 20.0));
    let mut at = centre;
    for _ in 0..1 + g.below(40) {
        n.ops += 1;
        let k = g.below(100);
        let op = match k {
            0..=11 => 0,
            12..=17 => 1,
            18..=21 => 2,
            22..=27 => 3,
            28..=32 => 4,
            33..=40 => 5,
            41..=48 => 6,
            49..=68 => 7,
            69..=74 => 8,
            75..=80 => 9,
            81..=84 => 10,
            85..=88 => 11,
            89..=91 => 12,
            92..=95 => 13,
            96..=97 => if k == 96 { 14 } else { 15 },
            _ => 16,
        };
        input.push(op);
        out.push(op);
        let count = l.get_num_points() as u32;
        match op {
            0 => {
                let np = g.below(10);
                input.push(np);
                let mut pts = Vec::new();
                at = centre;
                for _ in 0..np {
                    let p = point(g, &mut at);
                    push_v(&mut input, &p);
                    pts.push(p);
                }
                l.set_points(&pts);
            }
            1 => {
                let idx = g.below(count + 2);
                let p = point(g, &mut at);
                input.push(idx);
                push_v(&mut input, &p);
                l.set_point_location(idx, &p);
            }
            2 => {
                let idx = g.below(count + 2);
                input.push(idx);
                let p = l.get_point_location(idx);
                out.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
            }
            3 => {
                let p = point(g, &mut at);
                push_v(&mut input, &p);
                l.add_point(&p);
            }
            4 => {
                let idx = g.below(count + 2);
                input.push(idx);
                l.delete_point(idx);
            }
            5 => {
                let tex = g.below(2);
                let shader = match g.below(3) {
                    0 => PRESET_ADDITIVE_SHADER,
                    1 => PRESET_ADDITIVE_SPRITE_SHADER,
                    _ => ShaderClass(g.u32()),
                };
                let width = if g.below(8) == 0 { 0.0 } else { g.uniform(-2.0, 10.0) };
                let c = [g.uniform(-0.5, 1.5), g.uniform(-0.5, 1.5), g.uniform(-0.5, 1.5), g.uniform(-0.2, 1.2)];
                input.extend([tex, shader.0, width.to_bits(), c[0].to_bits(), c[1].to_bits(), c[2].to_bits(), c[3].to_bits()]);
                l.set_texture(if tex != 0 { Some(b"segline".to_vec()) } else { None });
                l.set_shader(shader);
                l.set_width(width);
                l.set_color(&Vector3::new(c[0], c[1], c[2]));
                l.set_opacity(c[3]);
            }
            6 => {
                let noise = if g.below(3) == 0 { 0.0 } else { g.uniform(-3.0, 3.0) };
                let abort = match g.below(4) {
                    0 => 1.5,
                    1 => 0.0,
                    2 => -g.uniform(0.0, 2.0),
                    _ => g.uniform(0.0, 3.0),
                };
                let levels = g.below(10);
                // (1 in 25 a fourth mode, which only SegmentedLineClass's setter can store: undefined at the next render)
                let mode = if g.below(25) == 0 { 3 } else { g.below(3) };
                let tile = g.uniform(-5.0, 60.0);
                let (u, v) = (g.uniform(-2000.0, 2000.0), g.uniform(-2000.0, 2000.0));
                let flags = g.below(16);
                input.extend([noise.to_bits(), f32::to_bits(abort), levels, mode, tile.to_bits(), u.to_bits(), v.to_bits(), flags]);
                l.set_noise_amplitude(noise);
                l.set_merge_abort_factor(abort);
                l.set_subdivision_levels(levels);
                l.set_texture_mapping_mode(mode);
                l.set_texture_tile_factor(tile);
                l.set_uv_offset_rate(&Vector2::new(u, v));
                l.set_merge_intersections(flags & 1 != 0);
                l.set_freeze_random(flags & 2 != 0);
                l.set_disable_sorting(flags & 4 != 0);
                l.set_end_caps(flags & 8 != 0);
            }
            7 => {
                n.renders += 1;
                sync = if g.below(20) == 0 { sync.wrapping_sub(g.below(1000)) } else { sync.wrapping_add(g.below(401)) };
                input.push(sync);
                let mut view = rotation(g);
                let depth = if g.below(20) == 0 { g.uniform(-30.0, 5.0) } else { g.uniform(5.0, 80.0) };
                let c = view.mul_vector3(&centre);
                view.row[0].w = -c.x + g.uniform(-3.0, 3.0);
                view.row[1].w = -c.y + g.uniform(-3.0, 3.0);
                view.row[2].w = -c.z - depth;
                input.extend(view.to_array().iter().map(|f| f.to_bits()));
                let before = gen3.clone();
                match l.render(sync, &view, &mut gen3) {
                    Ok(chunks) => {
                        for c in &chunks {
                            n.chunks += 1;
                            n.verts += c.vertices.len() as u64;
                            chunk_words(&mut out, c);
                        }
                    }
                    // undefined upstream: a mapping mode other than the three (the levels are at most 7 and
                    // Render_Seg_Line needs two points, so nothing else); compared up to here
                    Err(e) => {
                        assert!(matches!(e, W3dError::Undefined(W3dUndefined::UninitialisedRead)), "{e:?}");
                        return (input, out, true);
                    }
                }
                if gen3 != before {
                    n.random_draws += 1;
                }
                out.push('E' as u32);
            }
            8 => {
                let s = g.uniform(0.0, 1.0);
                input.push(s.to_bits());
                match l.prepare_lod(s) {
                    LodRequest::AddObject => out.push(1),
                    LodRequest::AddCost(c) => {
                        out.push(2);
                        wf(&mut out, c);
                    }
                }
            }
            9 => l.increment_lod(),
            10 => l.decrement_lod(),
            11 => {
                let lod = g.below(13) as i32 - 3;
                input.push(lod as u32);
                l.set_lod_level(lod);
            }
            12 => {
                sync = sync.wrapping_add(g.below(401));
                input.push(sync);
                l.reset_line(sync);
            }
            13 => {
                let w = [g.below(2), u32::from(g.below(5) == 0), u32::from(g.below(5) == 0), g.below(2), g.below(256)];
                input.extend(w);
                l.set_visible(w[0] != 0);
                l.set_hidden(w[1] != 0);
                l.set_animation_hidden(w[2] != 0);
                l.set_force_visible(w[3] != 0);
                l.set_collision_type(w[4]);
            }
            14 => l = SegmentedLine::copy(&l),
            15 => {
                // (the fresh line's renderer, and so the clock it was made at, is replaced whole by the assignment)
                let mut fresh = SegmentedLine::new(0);
                fresh.assign(&l);
                l = fresh;
            }
            _ => {
                let mut t = rotation(g);
                t.row[0].w = g.uniform(-5.0, 5.0);
                t.row[1].w = g.uniform(-5.0, 5.0);
                t.row[2].w = g.uniform(-5.0, 5.0);
                input.extend(t.to_array().iter().map(|f| f.to_bits()));
                l.transform = t;
            }
        }
        state_words(&mut out, &l, &gen3);
    }
    (input, out, false)
}

fn run_cpp(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 1 << 22];
    let n = unsafe { cpp::ref_segline_obj_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len(), "the C++ wrote more words than the buffer");
    out.truncate(n as usize);
    out
}

#[test]
fn segmented_lines_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("SegmentedLineClass sequences");
    let mut g = Gen::new(0x169_0001);
    let mut n = Counts::default();
    let (mut words, mut nan_only, mut max_ulp, mut undefined) = (0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let (input, mut rust, stopped) = build_case(&mut g, &mut n);
        let mut cc = run_cpp(&input);
        if stopped {
            // the C++ went on to draw from uninitialised values: compared up to the undefined render's code
            undefined += 1;
            cc.truncate(rust.len());
        }
        if rust.len() == cc.len() {
            for (a, b) in rust.iter_mut().zip(cc.iter_mut()) {
                if a != b && fw(*a).is_nan() && fw(*b).is_nan() {
                    nan_only += 1;
                    *a = 0x7FC0_0000;
                    *b = 0x7FC0_0000;
                }
            }
            for (a, b) in rust.iter().zip(&cc) {
                if a != b && fw(*a).is_finite() && fw(*b).is_finite() {
                    max_ulp = max_ulp.max(support::ulp_distance(fw(*a), fw(*b)));
                }
            }
        }
        words += rust.len() as u64;
        if rust != cc && !planted() {
            let at = rust.iter().zip(&cc).position(|(a, b)| a != b).unwrap_or(rust.len().min(cc.len()));
            let lo = at.saturating_sub(4);
            println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 12).min(rust.len())], &cc[lo.min(cc.len())..(at + 12).min(cc.len())]);
        }
        c.eq(rust, cc, || format!("case {k}: {input:?}"));
    }
    println!(
        "SegmentedLineClass: {} sequences, {} operations, {} renders, {} chunks, {} vertices, {} renders drawing from the generator, {undefined} sequences stopped at a render undefined upstream (a fourth mapping mode), {words} words compared ({nan_only} NaN on both sides with different bits); max ULP among differing finite words {max_ulp}",
        cases(),
        n.ops,
        n.renders,
        n.chunks,
        n.verts,
        n.random_draws
    );
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}
