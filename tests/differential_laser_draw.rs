#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DLaserDraw.cpp and its header
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LaserUpdate.cpp (getCurrentLaserRadius over the draw module)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/DrawModule.cpp and Module.cpp (the bases' xfer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Color.cpp (GameGetColorComponentsReal)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #168: `W3DLaserDraw` against the upstream text compiled verbatim (`differential/wrap_laser_draw.cpp`, which
//! lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! - **The beams** ([`W3DLaserDraw`] with a [`LaserUpdate`] of the test's, the ground the port's `terrain_cell` hash over
//!   eight heights on both sides): 20,000 random cases. Each makes the module from random data (0 to 4 beams, 0 to 5
//!   segments, widths of -1 to 10, random ARGB colours, scroll rates of -2 to 2, tiling on or off, a texture of 0 to 511
//!   by 0 to 511 texels or none, arcs of 0 to 50 (or none, or negative), overlap ratios of -0.1 to 0.2, tiling scalars of 0
//!   to 3; one case in 200 has 65,536 beams of 65,536 segments, whose count wraps: undefined upstream, counted) and runs 1
//!   to 30 steps: `doDrawModule` 40 %, the update's start and end moved 15 % (the same point now and then: a zero-length
//!   arc), `setDirty` 10 %, the width scale 10 %, `loadPostProcess` 5 %, `getCurrentLaserRadius` 8 % (the draw module
//!   first in the drawable's list, second after a draw module without the laser interface, or absent), `getLaserTemplateWidth`
//!   5 %, the update taken away or given back 7 % (`findClientUpdateModule` finding none). Positions are within 100 of the
//!   origin, 1 in 10 from `Gen::mixed` (NaN, infinities and subnormals included). Compared, every word as bits: after the
//!   constructor and after each step the update's dirty flag, the module's `m_selfDirty`, and for every line what it
//!   was given (texture, shader, width, colour, UV rate, mapping mode, visibility, scene membership, tile factor, points);
//!   each radius and template width. **Changed 2026-11-14 (#169):** the wrapper's lines are the REAL
//!   `SegmentedLineClass` (segline.cpp verbatim on the verbatim renderer, `differential/wrap_segline_obj.cpp`, reached
//!   through `differential/laser_segl_api.h`), so each line's words are read through its getters (scene membership,
//!   texture, shader bits, width, colour, UV rate, mapping mode, visibility, tile factor, points, the render object's
//!   bits) against the translated [`rts_lockstep_engine::formats::w3d::segline::SegmentedLine`]'s; and 5 % of the steps
//!   (from the update's 7 %, now 3 %, and `getLaserTemplateWidth`'s, now 4 %) render the scene's lines at a WW3D clock
//!   moving on by up to 200 ms (1 in 20 back by up to 500) with a view turned about x and moved: every chunk (vertices,
//!   triangles, shader, material, sorting, texture) compared, the whole chain from `W3DLaserDraw`'s set calls to the
//!   drawn chunks.
//! - **The save** (`xfer`): a fresh module saved by both (status and bytes), then 20,000 loads by both of the 4 saved
//!   bytes as they are (a quarter), cut short (a quarter), with one byte set to 0, 2 or a random value (a quarter), or
//!   with random bytes after (a quarter): the status and `m_selfDirty` compared.
//!
//! # Predictions (written before the first run, `issue168_predictions.md`)
//!
//! 0 mismatches, max ULP 0. Undefined: only the wrapping cases (about 0.5 %).
//!
//! # Results (2026-11-12)
//!
//! - First run: the C++ link failed (`LaserUpdateModuleData`'s constructor was not cut: added, `ld_laser_data.inc`).
//! - Second run: 5 of 20,000 sequences mismatched, every differing word a NaN on both sides with different bits (a tile
//!   factor or a length of NaN points: the C++ `sqrtf` is `sqrtss`, which keeps the input NaN's sign, the Rust's petir
//!   `sqrt` gives a positive quiet NaN). A miss of the harness's, not the translation's: the comparison now counts NaNs on
//!   both sides as equal, as `Check::f32` does.
//! - **The beams:** 20,000 sequences, 311,757 steps (125,145 `doDrawModule`), 32,172,780 words compared (9,031 NaN, 536
//!   of them NaN on both sides with different bits); 98 undefined upstream (the lines' count wraps; predicted about 0.5
//!   %: 0.49 %); **0 mismatches, max ULP 0**.
//! - **The save:** 4 bytes (`[1, 1, 1, 1]`) by both; 20,000 loads (11,760 whole, 8,240 failing): **0 mismatches**.
//!
//! Planted bugs, mismatching cases of 20,000 sequences (of 20,001 save and load comparisons for 9651): 9631 3,402; 9632
//! 9,993; 9633 9,918; 9634 8,907; 9635 4,590; 9636 13,026; 9637 12,412; 9638 17,323; 9639 11,363; 9640 1,189; 9641
//! 2,955; 9642 4,541; 9643 4,422; 9644 4,443; 9645 3,902; 9646 6,757; 9647 2,612; 9648 8,538; 9649 3,524; 9650 2,057;
//! 9651 11,731; 9652 8,031; 9653 13,383; 9654 8,538; 9655 4,461; 9656 13,419; 9657 2,955; 9658 4,421; 9659 3,902;
//! 9660 3,217. **30 of 30 caught.**
//!
//! # Results (2026-11-14, #169: the real `SegmentedLineClass` behind the wrapper, and the render step)
//!
//! - Predicted 0 mismatches and the 30 plants still caught, some catch counts moving (`issue169b_predictions.md` P2).
//! - **The beams:** 20,000 sequences, 310,095 steps (124,234 `doDrawModule`, 15,498 scene renders: 58,640 chunks,
//!   234,560 vertices), 34,268,488 words compared (174 NaN on both sides with different bits); 82 undefined upstream
//!   (the lines' count wraps); **0 mismatches, max ULP 0**. **The save:** 4 bytes, 20,000 loads: **0 mismatches**.
//! - Planted bugs, mismatching cases of 20,000 (of 20,001 save and load comparisons for 9651): 9631 3,258; 9632 10,148;
//!   9633 10,074; 9634 9,061; 9635 4,434; 9636 13,281; 9637 11,756; 9638 17,081; 9639 11,382; 9640 1,164; 9641 2,883;
//!   9642 4,395; 9643 4,280; 9644 4,304; 9645 3,792; 9646 6,750; 9647 2,239; 9648 8,372; 9649 2,481; 9650 1,915; 9651
//!   11,731; 9652 7,851; 9653 13,398; 9654 8,372; 9655 4,319; 9656 13,433; 9657 2,883; 9658 4,287; 9659 3,792; 9660
//!   2,018. **30 of 30 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9631 | one beam's red is not times the inner alpha |
//! | 9632 | several beams' scale is `i / numBeams` |
//! | 9633 | several beams' red is all times the inner alpha |
//! | 9634 | the UV rate is `(ScrollRate, 0)` |
//! | 9635 | the tiled mapping is set without a texture |
//! | 9636 | the texture's aspect is height / width |
//! | 9637 | the template width is the whole outer width |
//! | 9638 | `m_selfDirty` is not cleared by a draw |
//! | 9639 | the update's dirty flag is not cleared by a draw |
//! | 9640 | a one-segment line arcs |
//! | 9641 | the first segment's start takes the overlap too |
//! | 9642 | a segment's end ratio is its start's |
//! | 9643 | the half length is the whole length |
//! | 9644 | `sin`, not `cos`, raises the arc |
//! | 9645 | the arc skims 1 above the ground, not 2 |
//! | 9646 | without an arc, the end takes the start's z |
//! | 9647 | one beam's width ignores the width scale |
//! | 9648 | several beams all take the outer width |
//! | 9649 | the tile factor leaves out the aspect ratio |
//! | 9650 | the tile factor is set without a texture |
//! | 9651 | the save leaves out `DrawModule`'s version |
//! | 9652 | `loadPostProcess` does not mark the module to draw |
//! | 9653 | the colour's green is its blue byte |
//! | 9654 | a segment's beams are drawn into each other's lines (reversed) |
//! | 9655 | the arc's middle is the start |
//! | 9656 | the constructor leaves its lines visible |
//! | 9657 | the overlap shortens a segment's end |
//! | 9658 | the end point's raise is measured from the line's end |
//! | 9659 | the arc does not skim the ground |
//! | 9660 | the tile factor's length leaves out z |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::laser_update::{LaserHost, LaserObject, LaserUpdate, LaserUpdateModuleData};
use rts_lockstep_engine::client::particle_sys::ParticleSystemId;
use rts_lockstep_engine::client::w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::game_type::DrawableId;
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::seglinerenderer::SegLineChunk;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::random::Random3;
use support::{Check, Gen};

/// (#169) The chunks and vertices the Rust's renders gave (the run's counts).
static CHUNKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VERTICES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

fn cases() -> u64 {
    std::env::var("LASER_DRAW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The port's `terrain_cell` (object_cuts.rs) over the case's eight heights, as the wrapper's `TerrainLogic`.
fn ground(h: &[f32; 8], x: f32, y: f32) -> f32 {
    h[rts_lockstep_engine::logic::world::object_cuts::terrain_cell(x, y)]
}

/// `LaserUpdate::getCurrentLaserRadius`'s reads: the drawable (there) and its first laser draw module's width.
struct RadiusHost(Option<f32>);

impl LaserHost for RadiusHost {
    fn logic_frame(&self) -> u32 {
        unreachable!()
    }
    fn has_drawable(&self) -> bool {
        true
    }
    fn drawable_position(&self, _: DrawableId) -> Option<Coord3D> {
        unreachable!()
    }
    fn drawable_bone_transform(&self, _: DrawableId, _: &[u8]) -> Option<Matrix3D> {
        unreachable!()
    }
    fn drawable_object_effectively_dead(&self, _: DrawableId) -> bool {
        unreachable!()
    }
    fn destroy_own_drawable(&mut self) {
        unreachable!()
    }
    fn local_player_index(&self) -> Option<i32> {
        unreachable!()
    }
    fn shrouded_status(&self, _: &LaserObject, _: i32) -> ObjectShroudStatus {
        unreachable!()
    }
    fn find_template(&self, _: &[u8]) -> bool {
        unreachable!()
    }
    fn create_particle_system(&mut self, _: &[u8]) -> Option<ParticleSystemId> {
        unreachable!()
    }
    fn set_particle_system_position(&mut self, _: ParticleSystemId, _: &Coord3D) {
        unreachable!()
    }
    fn destroy_particle_system_by_id(&mut self, _: ParticleSystemId) {
        unreachable!()
    }
    fn set_drawable_position(&mut self, _: &Coord3D) {
        unreachable!()
    }
    fn laser_template_width(&self) -> Option<f32> {
        self.0
    }
}

fn pos(g: &mut Gen) -> f32 {
    if g.below(10) == 0 {
        g.mixed()
    } else {
        g.uniform(-100.0, 100.0)
    }
}

/// One case's words (the wrapper's layout).
fn random_case(g: &mut Gen) -> Vec<i32> {
    let mut w = Vec::new();
    let clock = &mut 0u32;
    let (beams, segments) = if g.below(200) == 0 { (65_536, 65_536) } else { (g.below(5), g.below(6)) };
    w.push(beams as i32);
    w.push(segments as i32);
    let width = |g: &mut Gen| if g.below(8) == 0 { 0.0 } else { g.uniform(-1.0, 10.0) };
    w.push(b(width(g)));
    w.push(b(width(g)));
    w.push(g.i32());
    w.push(g.i32());
    w.push(b(if g.below(3) == 0 { 0.0 } else { g.uniform(-2.0, 2.0) }));
    w.push(g.below(2) as i32);
    let texture = g.below(3) != 0;
    w.push(texture as i32);
    w.push(if g.below(30) == 0 { 0 } else { 1 + g.below(511) as i32 });
    w.push(if g.below(30) == 0 { 0 } else { 1 + g.below(511) as i32 });
    w.push(b(match g.below(4) {
        0 => 0.0,
        1 => g.uniform(-10.0, 0.0),
        _ => g.uniform(0.0, 50.0),
    }));
    w.push(b(if g.below(3) == 0 { 0.0 } else { g.uniform(-0.1, 0.2) }));
    w.push(b(g.uniform(0.0, 3.0)));
    for _ in 0..8 {
        w.push(b(g.uniform(-5.0, 20.0)));
    }
    w.push((g.below(10) != 0) as i32);
    w.push(match g.below(4) {
        0 | 1 => 0,
        2 => 1,
        _ => 2,
    });
    let start = [pos(g), pos(g), g.uniform(-10.0, 30.0)];
    let end = if g.below(20) == 0 { start } else { [pos(g), pos(g), g.uniform(-10.0, 30.0)] };
    w.extend(start.iter().chain(end.iter()).map(|&v| b(v)));
    w.push(g.below(2) as i32);
    w.push(b(g.uniform(0.0, 1.5)));
    assert_eq!(w.len(), 32);
    for _ in 0..1 + g.below(30) {
        let mut s = [0i32; 8];
        match g.below(100) {
            0..=39 => s[0] = 0,
            40..=54 => {
                s[0] = 1;
                let start = [pos(g), pos(g), g.uniform(-10.0, 30.0)];
                let end = if g.below(20) == 0 { start } else { [pos(g), pos(g), g.uniform(-10.0, 30.0)] };
                for (k, v) in start.iter().chain(end.iter()).enumerate() {
                    s[1 + k] = b(*v);
                }
            }
            55..=64 => {
                s[0] = 2;
                s[1] = g.below(2) as i32;
            }
            65..=74 => {
                s[0] = 3;
                s[1] = b(g.uniform(0.0, 1.5));
            }
            75..=79 => s[0] = 4,
            80..=87 => s[0] = 5,
            88..=91 => s[0] = 6,
            92..=94 => {
                s[0] = 7;
                s[1] = (g.below(3) != 0) as i32;
            }
            _ => {
                // (#169) the scene renders the lines: the clock moves on (now and then back: the offset's wrapping
                // difference), the view a translation and a turn about x
                s[0] = 8;
                *clock = if g.below(20) == 0 { clock.wrapping_sub(g.below(500)) } else { clock.wrapping_add(g.below(200)) };
                s[1] = *clock as i32;
                s[2] = b(g.uniform(-50.0, 50.0));
                s[3] = b(g.uniform(-50.0, 50.0));
                s[4] = b(g.uniform(-400.0, -50.0));
                let a = g.uniform(-3.0, 3.0);
                s[5] = b(rts_lockstep_engine::ww::wwmath::cos(a));
                s[6] = b(rts_lockstep_engine::ww::wwmath::sin(a));
            }
        }
        w.extend(s);
    }
    w
}

/// (#169) Each line's words: the scene membership, then the translated `SegmentedLineClass`'s getters (the wrapper's
/// `ls_state`).
fn line_words(out: &mut Vec<i32>, m: &W3DLaserDraw) {
    for l in &m.lines {
        let c = l.get_color();
        let r = l.get_uv_offset_rate();
        out.extend([
            m.in_scene as i32,
            l.get_texture().is_some() as i32,
            l.get_shader().0 as i32,
            b(l.get_width()),
            b(c.x),
            b(c.y),
            b(c.z),
            b(r.x),
            b(r.y),
            l.get_texture_mapping_mode() as i32,
            l.is_visible() as i32,
            b(l.get_texture_tile_factor()),
            l.get_num_points(),
        ]);
        for k in 0..l.get_num_points() as u32 {
            let p = l.get_point_location(k);
            out.extend([b(p.x), b(p.y), b(p.z)]);
        }
        out.push(l.bits as i32);
    }
}

/// (#169) A chunk's words (wrap_segline.cpp's `log_chunk`).
fn chunk_words(out: &mut Vec<i32>, c: &SegLineChunk) {
    out.extend(['C' as i32, c.sorting as i32, c.vertices.len() as i32]);
    for v in &c.vertices {
        out.extend([b(v.position.x), b(v.position.y), b(v.position.z), v.diffuse as i32, b(v.u), b(v.v)]);
    }
    out.push(c.triangles.len() as i32);
    for t in &c.triangles {
        out.extend(t.map(i32::from));
    }
    out.extend([c.shader.0 as i32, if c.prelit_diffuse { 0 } else { 1 }, c.texture.is_some() as i32]);
}

/// The Rust's words for a case (the wrapper's records).
fn rust_case(w: &[i32]) -> Vec<i32> {
    let data = W3DLaserDrawModuleData {
        num_beams: w[0] as u32,
        segments: w[1] as u32,
        inner_beam_width: r(w[2]),
        outer_beam_width: r(w[3]),
        inner_color: w[4],
        outer_color: w[5],
        scroll_rate: r(w[6]),
        tile: w[7] != 0,
        texture_name: if w[8] != 0 { b"Beam.tga".to_vec() } else { Vec::new() },
        arc_height: r(w[11]),
        segment_overlap_ratio: r(w[12]),
        tiling_scalar: r(w[13]),
        ..Default::default()
    };
    let mut heights = [0.0f32; 8];
    for (k, h) in heights.iter_mut().enumerate() {
        *h = r(w[14 + k]);
    }
    let mut has_update = w[22] != 0;
    let list = w[23];
    let mut out = Vec::new();
    let Ok(mut m) = W3DLaserDraw::new(Arc::new(data), Some((w[9] as u32, w[10] as u32)), 0) else {
        out.push(999);
        return out;
    };
    let mut u = LaserUpdate::new(LaserUpdateModuleData::default());
    u.start_pos = Coord3D { x: r(w[24]), y: r(w[25]), z: r(w[26]) };
    u.end_pos = Coord3D { x: r(w[27]), y: r(w[28]), z: r(w[29]) };
    u.set_dirty(w[30] != 0);
    u.current_width_scalar = r(w[31]);
    let state = |out: &mut Vec<i32>, u: &LaserUpdate, m: &W3DLaserDraw| {
        out.extend([u.dirty as i32, m.self_dirty as i32, m.lines.len() as i32]);
        line_words(out, m);
    };
    out.extend([200, m.has_texture as i32, b(m.texture_aspect_ratio)]);
    state(&mut out, &u, &m);
    for s in w[32..].chunks_exact(8) {
        match s[0] {
            0 => m.do_draw_module(if has_update { Some(&mut u) } else { None }, &mut |x, y| ground(&heights, x, y)),
            1 => {
                u.start_pos = Coord3D { x: r(s[1]), y: r(s[2]), z: r(s[3]) };
                u.end_pos = Coord3D { x: r(s[4]), y: r(s[5]), z: r(s[6]) };
            }
            2 => u.set_dirty(s[1] != 0),
            3 => u.current_width_scalar = r(s[1]),
            4 => m.load_post_process(),
            5 => {
                let host = RadiusHost(if list == 2 { None } else { Some(m.get_laser_template_width()) });
                out.extend([9, b(u.get_current_laser_radius(&host).expect("a drawable"))]);
            }
            6 => out.extend([8, b(m.get_laser_template_width())]),
            7 => has_update = s[1] != 0,
            _ => {
                let view = Matrix3D::new(1.0, 0.0, 0.0, r(s[2]), 0.0, r(s[5]), -r(s[6]), r(s[3]), 0.0, r(s[6]), r(s[5]), r(s[4]));
                out.push(7);
                let chunks = m.render_lines(s[1] as u32, &view, &mut Random3::default()).expect("a laser's lines are defined");
                for c in &chunks {
                    CHUNKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    VERTICES.fetch_add(c.vertices.len() as u64, std::sync::atomic::Ordering::Relaxed);
                    chunk_words(&mut out, c);
                }
                out.push('E' as i32);
            }
        }
        out.push(100);
        state(&mut out, &u, &m);
    }
    out
}

fn run_cpp(input: &[i32]) -> Vec<i32> {
    // (#169) the wrapper's lines are the real SegmentedLineClass's (differential/wrap_segline_obj.cpp)
    unsafe { cpp::ref_laser_segl_bind() };
    let mut out = vec![0i32; 1 << 22];
    let n = unsafe { cpp::ref_laser_draw_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

#[test]
fn the_beams_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("W3DLaserDraw sequences");
    let mut g = Gen::new(0x168_0001);
    let (mut undefined, mut steps, mut draws, mut lines_drawn, mut nan_words, mut nan_only) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut max_ulp = 0u64;
    let mut renders = 0u64;
    for k in 0..cases() {
        let w = random_case(&mut g);
        let mut rust = rust_case(&w);
        let mut cc = run_cpp(&w);
        // NaNs on both sides compare equal whatever their sign or payload, as `Check::f32` counts them (every float word is
        // a float's bits; the integer words, flags, counts and tags, are never NaN patterns)
        if rust.len() == cc.len() {
            for (a, bb) in rust.iter_mut().zip(cc.iter_mut()) {
                if a != bb && r(*a).is_nan() && r(*bb).is_nan() {
                    nan_only += 1;
                    *a = 0x7FC0_0000;
                    *bb = 0x7FC0_0000;
                }
            }
        }
        if rust == [999] {
            undefined += 1;
        }
        steps += (w.len() as u64 - 32) / 8;
        draws += w[32..].chunks_exact(8).filter(|s| s[0] == 0).count() as u64;
        renders += w[32..].chunks_exact(8).filter(|s| s[0] == 8).count() as u64;
        lines_drawn += rust.len() as u64;
        nan_words += rust.iter().filter(|&&x| f32::from_bits(x as u32).is_nan()).count() as u64;
        if rust != cc {
            if rust.len() == cc.len() {
                for (a, bb) in rust.iter().zip(&cc) {
                    let (fa, fb) = (r(*a), r(*bb));
                    if a != bb && fa.is_finite() && fb.is_finite() {
                        max_ulp = max_ulp.max(support::ulp_distance(fa, fb));
                    }
                }
            }
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            let lo = at.saturating_sub(4);
            if !planted() {
                println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 16).min(rust.len())], &cc[lo.min(cc.len())..(at + 16).min(cc.len())]);
            }
        }
        c.eq(rust, cc, || format!("case {k}: {w:?}"));
    }
    println!(
        "W3DLaserDraw: {} cases, {steps} steps ({draws} doDrawModule, {renders} scene renders: {} chunks, {} vertices), {lines_drawn} words compared, {nan_words} NaN words ({nan_only} NaN on both sides with different bits); {undefined} undefined upstream (lines wrap); max ULP among differing finite words {max_ulp}",
        cases(),
        CHUNKS.load(std::sync::atomic::Ordering::Relaxed),
        VERTICES.load(std::sync::atomic::Ordering::Relaxed)
    );
    finish(c);
}

fn xfer_path(kind: &str) -> String {
    format!("{}/laser_draw_xfer_{kind}_{}.bin", std::env::temp_dir().display(), std::process::id())
}

fn status(r: Result<(), XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32 + 1,
    }
}

fn fresh() -> W3DLaserDraw {
    W3DLaserDraw::new(Arc::new(W3DLaserDrawModuleData::default()), None, 0).expect("one line")
}

fn cpp_xfer(mode: i32, path: &str) -> Vec<i32> {
    let cpath = std::ffi::CString::new(path).expect("path");
    unsafe { cpp::ref_laser_segl_bind() };
    let mut out = vec![0i32; 16];
    let n = unsafe { cpp::ref_laser_draw_xfer(mode, cpath.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

#[test]
fn the_save_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("W3DLaserDraw xfer");
    // the save
    let mut m = fresh();
    let mut x = rts_lockstep_engine::common::xfer_save::XferSave::new();
    x.open(b"module").expect("open");
    let st = status(m.xfer(&mut x));
    let saved = x.bytes().to_vec();
    let path = xfer_path("save");
    let cc = cpp_xfer(0, &path);
    let cbytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    c.eq((st, saved.clone()), (cc[0], cbytes), || "the save".into());
    println!("the save: {} bytes {saved:?}", saved.len());
    // the loads
    let mut g = Gen::new(0x168_0002);
    let (mut whole, mut failed) = (0u64, 0u64);
    for k in 0..cases() {
        let mut bytes = saved.clone();
        match g.below(4) {
            0 => {}
            1 => bytes.truncate(g.below(bytes.len() as u32) as usize),
            2 => {
                let at = g.below(bytes.len() as u32) as usize;
                bytes[at] = match g.below(3) {
                    0 => 0,
                    1 => 2,
                    _ => g.below(256) as u8,
                };
            }
            _ => {
                for _ in 0..1 + g.below(8) {
                    bytes.push(g.below(256) as u8);
                }
            }
        }
        let mut m = fresh();
        let mut ld = rts_lockstep_engine::common::xfer_load::XferLoad::new(bytes.clone());
        let st = match ld.open(b"module") {
            Ok(()) => status(m.xfer(&mut ld)),
            Err(e) => e as i32 + 1,
        };
        // (both modules fresh: m_selfDirty TRUE, which the load itself leaves alone)
        let rust = vec![st, m.self_dirty as i32];
        let path = xfer_path("load");
        std::fs::write(&path, &bytes).expect("write the save");
        let cc = cpp_xfer(1, &path);
        let _ = std::fs::remove_file(&path);
        if st == 0 {
            whole += 1;
        } else {
            failed += 1;
        }
        c.eq(rust, cc, || format!("load {k}: {bytes:?}"));
    }
    println!("loads: {whole} whole, {failed} failing");
    finish(c);
}
