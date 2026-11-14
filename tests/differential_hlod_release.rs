#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp, hlod.h, scene.cpp, part_emt.cpp, part_buf.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #65 (batch c): an HLOD's particle emitters in a scene over frames ([`RenderModel::update_emitters`]'s scene
//! order), their release from the HLOD ([`RenderModel::post_render_processing`], [`RenderModel::remove_sub_object`]),
//! the HLOD's removal ([`RenderModel::remove_from_scene`]) and the buffers left drawing ([`RenderModel::orphans`]),
//! against hlod.cpp's `Remove_Sub_Object`, `Notify_Added` and `Notify_Removed` and scene.cpp's `SimpleSceneClass`
//! list functions and `Post_Render_Processing` compiled verbatim with the real emitter and buffer
//! (`differential/wrap_hlod_release.cpp`, which lists the stand-ins and the words).
//!
//! # Methodology
//!
//! 20,000 random sequences. Each seeds the two generators, makes 1 to 4 emitters with the constructor from random
//! arguments (as `differential_part_lod`: every randomizer class, keyframed properties, every render mode; a particle
//! cap of 1 to 15 for half of them, so that they complete), puts some in the HLOD's LOD and the rest in its additional
//! models, adds the HLOD to a scene (the world, `RTS3DScene`, which never processes releases, or a simple scene, which
//! does: half each) and shows the emitters, then runs 20 to 60 operations: frames (80 %: 0 to 60 ms mostly, up to
//! 400 ms, now and then 1 to 4 s, the HLOD moving), the HLOD removed (once, now and then as the first operation),
//! `Enable_Remove_On_Complete` and `Set_Hidden` on a live emitter. A frame is the scene's: the transforms, the
//! visibility check (the render list as it is), the update list's loop, the visible buffers' `Render`, then (simple
//! scene) `Post_Render_Processing`. Compared word for word (floats as bits, a NaN equal to any NaN): after creation and
//! after every operation the sub-object list (ids) with each emitter's state (active, complete, in the scene,
//! particles left, emission remainder, first time, buffer needed, remove-on-complete, listed for release, its buffer
//! in the scene, its particle counts, dead emitter), the orphaned buffers (id, held, counts, dead emitter, complete,
//! listed) and the release list in order; per frame what every drawn buffer drew (every point, line run or line) and
//! the release list a simple scene processed.
//!
//! A sequence whose Rust reports an input upstream is undefined on stops before that operation (the C++ is not run on
//! it); one whose emitter cannot be made (undefined at creation) is not compared.
//!
//! # Predictions
//!
//! `issue65c_predictions.md` (written before the first run): see the results.
//!
//! # Results (2026-11-14)
//!
//! - First run that compiled (300 sequences): **4 mismatching**, every one a point's position: the buffers' `Emit` ran in
//!   a different order, so they drew the shared `Random3Class` in a different order. The Rust ran the buffers in
//!   sub-object order; upstream's update list holds them in the order they *joined* it (each at the head), and an
//!   emitter hidden before its first frame is not active, so its buffer joins later, nearer the head. Fixed in the Rust
//!   (`EmitterSub::buffer_joined`: later joins first, same-update joins in sub-object order). Then 2,000: 0; then
//!   **20,000 sequences, 0 mismatching**: 130,199,808 words (0 NaN-only), 716,734 frames, 7,886,557 points or line
//!   points drawn (1,505,725 by orphaned buffers, in 230,381 frames), 24,240 releases processed in a simple scene (7,979
//!   of the 9,954 simple-scene sequences have one), 13,945 HLODs removed (4,705 emitters held by the release list),
//!   0 ended at upstream undefined, 0 undefined at creation.
//! - **Plants**, mismatching sequences of 20,000: 10136 12,814; 10137 12,474; 10138 1,808; 10139 4,947; 10140 18,514;
//!   10141 276; 10142 8,025; 10143 7,784; 10144 3,228; 10145 8,026; 10146 1,857. **11 of 11 caught**; the weakest,
//!   10141 (the removed HLOD's emitters not stopped), needs a held emitter whose buffer then emits.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 10136 | a dead emitter's complete buffer never registers for release |
//! | 10137 | the buffers run in reverse sub-object order |
//! | 10138 | the emitters run in sub-object order (not reverse) |
//! | 10139 | an emitter already listed for release is listed again |
//! | 10140 | a buffer added this frame is drawn |
//! | 10141 | removing the HLOD does not stop its emitters (`Notify_Removed`) |
//! | 10142 | the world scene processes releases |
//! | 10143 | a released dead buffer stays in the scene |
//! | 10144 | an emitter in the release list is destroyed with its HLOD (not held) |
//! | 10145 | a release request goes to the list's tail |
//! | 10146 | a destroyed emitter's buffer not in the scene is orphaned |

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::part_buf::{ParticleDraw, Props, Ww3dClock};
use rts_lockstep_engine::formats::w3d::part_emt::{EmitterArgs, ParticleEmitter, ParticleRandom};
use rts_lockstep_engine::formats::w3d::part_ldr::{ParticleEmitterDef, Vector3Randomizer};
use rts_lockstep_engine::formats::w3d::render::{EmitterSub, Orphaned, RenderModel, SceneKind};
use rts_lockstep_engine::formats::w3d::shader::{ShaderClass, PRESET_ADDITIVE_SPRITE_SHADER, PRESET_ALPHA_SPRITE_SHADER};
use rts_lockstep_engine::formats::w3d::w3d_file::W3dEmitterLinePropertiesStruct;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::random::{Random3, Random4};
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::Gen;

fn planted() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn cases() -> u64 {
    std::env::var("HLOD_RELEASE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn wu(o: &mut Vec<u32>, x: u32) {
    o.push(x);
}
fn wf(o: &mut Vec<u32>, f: f32) {
    o.push(f.to_bits());
}
fn wv3(o: &mut Vec<u32>, v: Vector3) {
    wf(o, v.x);
    wf(o, v.y);
    wf(o, v.z);
}
fn wv4(o: &mut Vec<u32>, v: Vector4) {
    wf(o, v.x);
    wf(o, v.y);
    wf(o, v.z);
    wf(o, v.w);
}

/// wrap_part_lod.cpp's log_render; the number of points or lines drawn.
fn draw_words(o: &mut Vec<u32>, d: &ParticleDraw) -> u64 {
    match d {
        ParticleDraw::Points(p) => {
            wu(o, 'P' as u32);
            wu(o, p.quads as u32);
            wu(o, p.frame_row_column_count_log2 as u32);
            wu(o, p.points.len() as u32);
            for s in &p.points {
                wu(o, s.index);
                wv3(o, s.position);
                wv4(o, s.diffuse);
                wf(o, s.size);
                wu(o, s.orientation as u32);
                wu(o, s.frame as u32);
            }
            p.points.len() as u64
        }
        ParticleDraw::Lines { width, runs } => {
            wu(o, 'L' as u32);
            wf(o, *width);
            wu(o, runs.len() as u32);
            let mut n = 0;
            for r in runs {
                wu(o, r.points.len() as u32);
                for (p, c) in r.points.iter().zip(&r.colors) {
                    wv3(o, *p);
                    wv4(o, *c);
                }
                n += r.points.len() as u64;
            }
            n
        }
        ParticleDraw::LineGroup(g) => {
            wu(o, 'G' as u32);
            wu(o, g.prism as u32);
            wu(o, g.lines.len() as u32);
            for l in &g.lines {
                wu(o, l.index);
                wv3(o, l.head);
                wv3(o, l.tail);
                wv4(o, l.head_diffuse);
                wv4(o, l.tail_diffuse);
                wf(o, l.size);
                wf(o, l.ucoord);
            }
            g.lines.len() as u64
        }
        ParticleDraw::Nothing => {
            wu(o, 'X' as u32);
            0
        }
    }
}

fn rotation(g: &mut Gen, angle: f32) -> Matrix3D {
    let h = angle * 0.5;
    let tilt = 0.3f32;
    let q = Quaternion::new(
        rts_lockstep_engine::ww::wwmath::sin(tilt) * rts_lockstep_engine::ww::wwmath::cos(h),
        0.0,
        rts_lockstep_engine::ww::wwmath::sin(h),
        rts_lockstep_engine::ww::wwmath::cos(h) * rts_lockstep_engine::ww::wwmath::cos(tilt),
    );
    let _ = g;
    build_matrix3d(&q)
}

fn push_m(input: &mut Vec<u32>, m: &Matrix3D) {
    input.extend(m.to_array().iter().map(|f| f.to_bits()));
}

fn gen_rand(g: &mut Gen, input: &mut Vec<u32>) -> Vector3Randomizer {
    let cls = g.below(4);
    let (a, b, c) = (g.uniform(0.0, 5.0), g.uniform(0.0, 5.0), g.uniform(0.0, 5.0));
    input.extend([cls, a.to_bits(), b.to_bits(), c.to_bits()]);
    match cls {
        0 => Vector3Randomizer::SolidBox([a, b, c]),
        1 => Vector3Randomizer::SolidSphere(a),
        2 => Vector3Randomizer::HollowSphere(a),
        _ => Vector3Randomizer::SolidCylinder { extent: a, radius: b },
    }
}

fn key_times(g: &mut Gen, n: u32, age: f32) -> Vec<f32> {
    let mut t = 0.0f32;
    (0..n)
        .map(|_| {
            t += g.uniform(0.01, age * 0.6);
            t
        })
        .collect()
}

fn gen_f(g: &mut Gen, input: &mut Vec<u32>, start: (f32, f32), rand: f32, vals: (f32, f32), age: f32) -> Props<f32> {
    let s = g.uniform(start.0, start.1);
    let r = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, rand) };
    let n = g.below(3);
    let times = key_times(g, n, age);
    let values: Vec<f32> = (0..n).map(|_| g.uniform(vals.0, vals.1)).collect();
    input.extend([s.to_bits(), r.to_bits(), n]);
    for (t, v) in times.iter().zip(&values) {
        input.extend([t.to_bits(), v.to_bits()]);
    }
    Props { start: s, rand: r, key_times: times, values }
}

fn gen_v(g: &mut Gen, input: &mut Vec<u32>, age: f32) -> Props<Vector3> {
    let s = Vector3::new(g.uniform(0.0, 1.0), g.uniform(0.0, 1.0), g.uniform(0.0, 1.0));
    let r = if g.below(2) == 0 { Vector3::new(0.0, 0.0, 0.0) } else { Vector3::new(g.uniform(0.0, 0.5), g.uniform(0.0, 0.5), g.uniform(0.0, 0.5)) };
    let n = g.below(3);
    let times = key_times(g, n, age);
    let values: Vec<Vector3> = (0..n).map(|_| Vector3::new(g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2))).collect();
    input.extend([s.x.to_bits(), s.y.to_bits(), s.z.to_bits(), r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), n]);
    for (t, v) in times.iter().zip(&values) {
        input.extend([t.to_bits(), v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
    }
    Props { start: s, rand: r, key_times: times, values }
}


/// One emitter's constructor arguments, the words wrap_hlod_release.cpp reads (as differential_part_lod's).
fn gen_emitter(g: &mut Gen, input: &mut Vec<u32>) -> EmitterArgs {
    let emit_rate = if g.below(20) == 0 { 0.0 } else { g.uniform(0.5, 30.0) };
    let burst_size = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
    input.extend([emit_rate.to_bits(), burst_size]);
    let pos_rnd = gen_rand(g, input);
    let base_vel = Vector3::new(g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0));
    input.extend([base_vel.x.to_bits(), base_vel.y.to_bits(), base_vel.z.to_bits()]);
    let vel_rnd = gen_rand(g, input);
    let out_vel = if g.below(2) == 0 { 0.0 } else { g.uniform(-5.0, 5.0) };
    let inherit = if g.below(2) == 0 { 0.0 } else { g.uniform(-1.0, 2.0) };
    input.extend([out_vel.to_bits(), inherit.to_bits()]);
    let max_age = g.uniform(0.05, 2.0);
    let color = gen_v(g, input, max_age);
    let opacity = gen_f(g, input, (0.0, 1.0), 0.5, (-0.2, 1.2), max_age);
    let size = gen_f(g, input, (0.0, 3.0), 1.0, (-1.0, 6.0), max_age);
    let rotation_p = gen_f(g, input, (-3.0, 3.0), 1.0, (-3.0, 3.0), max_age);
    let orient_rnd = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 1.0) };
    input.push(orient_rnd.to_bits());
    let frames = gen_f(g, input, (0.0, 16.0), 5.0, (0.0, 16.0), max_age);
    let blur = gen_f(g, input, (0.0, 0.2), 0.06, (0.0, 0.2), max_age);
    let accel = if g.below(2) == 0 { Vector3::new(0.0, 0.0, 0.0) } else { Vector3::new(g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0), g.uniform(-30.0, 5.0)) };
    input.extend([accel.x.to_bits(), accel.y.to_bits(), accel.z.to_bits(), max_age.to_bits(), 0.0f32.to_bits()]);
    let has_tex = g.below(2);
    let shader = match g.below(3) {
        0 => PRESET_ADDITIVE_SPRITE_SHADER,
        1 => PRESET_ALPHA_SPRITE_SHADER,
        _ => ShaderClass(g.u32() & 0x000F_FFFF),
    };
    let max_particles = if g.below(2) == 0 { 1 + g.below(15) as i32 } else { 0 };
    let max_buffer_size = if g.below(10) < 7 { 2 + g.below(127) as i32 } else { 0 };
    let pingpong = g.below(2);
    let render_mode = if g.below(30) == 0 { 7 } else { g.below(5) as i32 };
    let frame_mode = g.below(5) as i32;
    input.extend([has_tex, shader.0, max_particles as u32, max_buffer_size as u32, pingpong, render_mode as u32, frame_mode as u32]);
    let has_lp = render_mode == 2 || g.below(2) == 0;
    let lp = W3dEmitterLinePropertiesStruct {
        flags: g.u32() & 0x0300_000F,
        subdivision_level: g.below(4),
        noise_amplitude: g.uniform(0.0, 2.0),
        merge_abort_factor: 1.5,
        texture_tile_factor: g.uniform(0.0, 4.0),
        u_per_sec: 0.0,
        v_per_sec: 0.0,
        reserved: [0; 9],
    };
    input.extend([
        has_lp as u32,
        lp.flags,
        lp.subdivision_level,
        lp.noise_amplitude.to_bits(),
        lp.merge_abort_factor.to_bits(),
        lp.texture_tile_factor.to_bits(),
        lp.u_per_sec.to_bits(),
        lp.v_per_sec.to_bits(),
    ]);
    EmitterArgs {
        emit_rate,
        burst_size,
        pos_rnd: Some(pos_rnd),
        base_vel,
        vel_rnd: Some(vel_rnd),
        out_vel,
        vel_inherit_factor: inherit,
        color,
        opacity,
        size,
        rotation: rotation_p,
        orient_rnd,
        frames,
        blur_times: blur,
        accel,
        max_age,
        future_start: 0.0,
        texture: if has_tex != 0 { Some(b"fx.tga".to_vec()) } else { None },
        shader,
        max_particles,
        max_buffer_size,
        pingpong: pingpong != 0,
        render_mode,
        frame_mode,
        line_props: if has_lp { Some(lp) } else { None },
    }
}

/// wrap_hlod_release.cpp's log_state.
fn state_words(o: &mut Vec<u32>, m: &RenderModel) {
    wu(o, 'S' as u32);
    wu(o, m.emitters.len() as u32);
    for e in &m.emitters {
        let em = e.emitter.as_ref().expect("made");
        let b = &em.buffer;
        wu(o, e.id);
        for x in [em.active, em.is_complete, em.is_in_scene] {
            wu(o, x as u32);
        }
        wu(o, em.particles_left as u32);
        wu(o, em.emit_remain);
        for x in [em.first_time, em.buffer_scene_needed, em.remove_on_complete, e.in_release_list, em.buffer_in_scene] {
            wu(o, x as u32);
        }
        wu(o, b.non_new_num as u32);
        wu(o, b.new_num as u32);
        wu(o, b.is_emitter_dead as u32);
    }
    wu(o, 'O' as u32);
    wu(o, m.orphans.len() as u32);
    for x in &m.orphans {
        let b = x.buffer();
        wu(o, x.id);
        wu(o, matches!(x.obj, Orphaned::Held(_)) as u32);
        wu(o, b.non_new_num as u32);
        wu(o, b.new_num as u32);
        wu(o, b.is_emitter_dead as u32);
        wu(o, b.is_complete() as u32);
        wu(o, x.in_release_list as u32);
    }
    wu(o, 'R' as u32);
    wu(o, m.release_list.len() as u32);
    for &id in &m.release_list {
        wu(o, id);
    }
}

#[derive(Default)]
struct Tally {
    creation_undefined: u64,
    ended_undefined: u64,
    simple: u64,
    frames: u64,
    drawn: u64,
    processed: u64,
    sequences_released: u64,
    orphan_frames: u64,
    orphan_drawn: u64,
    held: u64,
    removed_hlods: u64,
    drained: u64,
}

/// One random sequence: the input words and the Rust's output words.
fn build_case(g: &mut Gen, t: &mut Tally) -> Option<(Vec<u32>, Vec<u32>)> {
    let mut input = Vec::new();
    let mut o = Vec::new();
    let (s3a, s3b, s4) = (g.u32(), g.u32(), g.u32());
    let mut sync = g.below(100_000);
    let simple = g.below(2) == 1;
    let scene = if simple { SceneKind::Simple } else { SceneKind::World };
    let n = 1 + g.below(4);
    let nlod = g.below(n + 1);
    input.extend([s3a, s3b, s4, sync, simple as u32, n, nlod]);
    let mut rng = ParticleRandom { v3: Random3::new(s3a, s3b), buf: Random4::new(s4), lod_max_screen_sizes: Default::default() };
    let mut clock = Ww3dClock { sync_time: sync, frame_time: 0, frame_count: 0 };
    let mut made = Vec::new();
    let mut bad = false;
    for _ in 0..n {
        let args = gen_emitter(g, &mut input);
        match ParticleEmitter::new(args, &mut rng, &clock) {
            Ok(e) => made.push(e),
            Err(_) => bad = true,
        }
    }
    if bad {
        t.creation_undefined += 1;
        return None;
    }
    let mut pos = Vector3::new(g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(0.0, 20.0));
    let vel = Vector3::new(g.uniform(-0.05, 0.05), g.uniform(-0.05, 0.05), 0.0);
    let spin = g.uniform(-0.003, 0.003);
    let mut angle = g.uniform(-3.0, 3.0);
    let mut tm = rotation(g, angle);
    tm.set_translation(&pos);
    push_m(&mut input, &tm);
    // the HLOD added to the scene (Notify_Added for each sub-object), then each emitter shown
    let mut model = RenderModel {
        name: b"H".to_vec(),
        is_hlod: true,
        htree: None,
        sub_objects: Vec::new(),
        not_drawn: Vec::new(),
        emitters: Vec::new(),
        orphans: Vec::new(),
        release_list: Vec::new(),
        updates: 0,
    };
    for (k, mut e) in made.into_iter().enumerate() {
        e.set_transform(tm);
        e.notify_added();
        model.emitters.push(EmitterSub {
            name: b"E".to_vec(),
            bone_index: 0,
            def: ParticleEmitterDef::new(),
            emitter: Some(e),
            draw: None,
            failed: None,
            id: k as u32,
            in_release_list: false,
            buffer_joined: 0,
        });
    }
    for e in model.emitters.iter_mut() {
        e.emitter.as_mut().expect("made").set_hidden(false);
    }
    if simple {
        t.simple += 1;
    }
    wu(&mut o, 'C' as u32);
    state_words(&mut o, &model);
    let nops = 20 + g.below(41);
    let mut alive = true;
    let mut released_any = false;
    for step in 0..nops {
        let op = loop {
            let k = g.below(100);
            let op = match k {
                0..=79 => 0,
                80..=82 => 1,
                83..=89 => 2,
                _ => 3,
            };
            let op = if step == 0 && g.below(30) == 0 { 1 } else { op };
            if op == 0 || (alive && (op == 1 || !model.emitters.is_empty())) {
                break op;
            }
        };
        let in_mark = input.len();
        let out_mark = o.len();
        input.push(op);
        wu(&mut o, op);
        match op {
            0 => {
                let dt = match g.below(20) {
                    0 => 0,
                    1..=13 => 1 + g.below(60),
                    14..=18 => 60 + g.below(340),
                    _ => 1000 + g.below(3000),
                };
                input.push(dt);
                sync = sync.wrapping_add(dt);
                clock = Ww3dClock { sync_time: sync, frame_time: dt, frame_count: clock.frame_count + 1 };
                pos += vel * dt as f32;
                angle += spin * dt as f32;
                let mut m = rotation(g, angle);
                m.set_translation(&pos);
                push_m(&mut input, &m);
                model.update_emitters(&m, &clock, &mut rng);
                if model.emitters.iter().any(|e| e.failed.is_some()) || model.orphans.iter().any(|x| x.failed.is_some()) {
                    input.truncate(in_mark);
                    o.truncate(out_mark);
                    t.ended_undefined += 1;
                    break;
                }
                t.frames += 1;
                let mut draws: Vec<(u32, &ParticleDraw)> = Vec::new();
                for e in &model.emitters {
                    if let Some(d) = &e.draw {
                        draws.push((e.id, d));
                    }
                }
                let mut orphan_drew = false;
                for x in &model.orphans {
                    if let Some(d) = &x.draw {
                        draws.push((x.id, d));
                        orphan_drew = true;
                    }
                }
                draws.sort_by_key(|d| d.0);
                wu(&mut o, 'D' as u32);
                wu(&mut o, draws.len() as u32);
                for (id, d) in draws {
                    wu(&mut o, id);
                    let k = draw_words(&mut o, d);
                    t.drawn += k;
                    if model.orphans.iter().any(|x| x.id == id) {
                        t.orphan_drawn += k;
                    }
                }
                if orphan_drew {
                    t.orphan_frames += 1;
                }
                wu(&mut o, 'P' as u32);
                let before = model.release_list.clone();
                let done = model.post_render_processing(scene);
                if simple {
                    assert_eq!(done, before, "processed what was listed");
                    wu(&mut o, before.len() as u32);
                    for id in before {
                        wu(&mut o, id);
                    }
                    if !done.is_empty() {
                        t.processed += done.len() as u64;
                        released_any = true;
                    }
                } else {
                    wu(&mut o, 0);
                }
                t.drained += model.orphans.iter().filter(|x| x.is_drained()).count() as u64;
            }
            1 => {
                model = model.remove_from_scene();
                alive = false;
                t.removed_hlods += 1;
                t.held += model.orphans.iter().filter(|x| matches!(x.obj, Orphaned::Held(_))).count() as u64;
            }
            _ => {
                let k = g.below(model.emitters.len() as u32);
                let on = g.below(2);
                input.extend([k, on]);
                let em = model.emitters[k as usize].emitter.as_mut().expect("made");
                if op == 2 {
                    em.enable_remove_on_complete(on != 0);
                } else {
                    em.set_hidden(on != 0);
                }
            }
        }
        state_words(&mut o, &model);
    }
    if released_any {
        t.sequences_released += 1;
    }
    Some((input, o))
}

/// *Results (2026-11-14):* 20,000 sequences, 0 mismatching (see the module docs).
#[test]
fn hlod_release_matches_upstream() {
    let mut g = Gen::new(0x65C_4E1);
    let n = cases();
    let mut t = Tally::default();
    let (mut words, mut mismatched, mut compared, mut nan_only) = (0u64, 0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    let mut out = vec![0u32; 1 << 24];
    for case in 0..n {
        let Some((input, rust)) = build_case(&mut g, &mut t) else { continue };
        let len = unsafe { cpp::ref_hlod_release_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        assert!(len <= out.len(), "output buffer too small: {len}");
        let cw = &out[..len];
        compared += 1;
        words += rust.len().min(len) as u64;
        let nan = |x: u32| f32::from_bits(x).is_nan();
        let mut same = cw.len() == rust.len();
        for (a, b) in rust.iter().zip(cw) {
            if a != b {
                if nan(*a) && nan(*b) {
                    nan_only += 1;
                } else {
                    same = false;
                }
            }
        }
        if !same {
            mismatched += 1;
            if examples.len() < 6 {
                let i = rust.iter().zip(cw).position(|(a, b)| a != b && !(nan(*a) && nan(*b))).unwrap_or(rust.len().min(len));
                examples.push(format!(
                    "case {case} word {i} of {} (c++ {len}): rust {:?} c++ {:?}",
                    rust.len(),
                    rust.get(i.saturating_sub(4)..(i + 4).min(rust.len())),
                    cw.get(i.saturating_sub(4)..(i + 4).min(len))
                ));
            }
        }
    }
    let line = format!(
        "hlod_release: {n} sequences, {compared} compared ({} not made: undefined at creation; {} in a simple scene), {words} \
         words ({nan_only} NaN-only), {} frames, {} points or line points drawn ({} by orphaned buffers, in {} frames), \
         {} releases processed ({} sequences with one), {} HLODs removed ({} emitters held), {} drained orphan-frames, \
         {} ended at upstream undefined; mismatching sequences {mismatched}",
        t.creation_undefined, t.simple, t.frames, t.drawn, t.orphan_drawn, t.orphan_frames, t.processed, t.sequences_released, t.removed_hlods, t.held,
        t.drained, t.ended_undefined
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
