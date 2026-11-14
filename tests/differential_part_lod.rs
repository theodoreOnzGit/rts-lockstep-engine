#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_buf.cpp, part_buf.h, part_emt.cpp, part_emt.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #65 (batch b): the particle buffer's predictive LOD ([`ParticleBuffer::prepare_lod`],
//! [`ParticleBuffer::calculate_cost_value_arrays`], the LOD level, bias and cost/value accessors, the static screen sizes
//! [`LodMaxScreenSizes`]), the LOD decimation of what is drawn, and the emitter's hiding, release and removal, against
//! part_buf.cpp and part_emt.cpp compiled verbatim (`differential/wrap_part_lod.cpp`, which lists the stand-ins and the
//! words).
//!
//! # Methodology
//!
//! 20,000 random sequences. Each seeds the two generators, makes an emitter with the constructor (`ParticleEmitterClass(
//! emit_rate, ...)`) from random arguments (rate 0.5 to 30 per second, 0 now and then; bursts of 1 to 3, 0 now and
//! then; each randomizer class; colour, opacity, size, rotation, frame and blur-time properties with 0 to 2 keyframes and
//! random ranges or none; acceleration or none; lifetimes 0.05 to 4 s; a texture or none; one of two preset shaders or
//! random bits; a particle cap and a buffer cap sometimes; ping-pong positions half the time; every render mode, 1 in 30
//! an unknown one; every frame mode; line properties for every line-mode emitter), adds it to a scene and shows it, then
//! runs 10 to 40 operations: frames (40 %: 0 to 50 ms mostly, 50 to 400 ms, now and then 3 to 8 s, with the emitter
//! moving and turning), `Prepare_LOD` with a random camera (position, viewport, view plane; degenerate ones now and
//! then), `Increment_LOD`, `Decrement_LOD`, `Set_LOD_Level(-3..19)`, `Set_LOD_Bias` (negative and NaN now and then),
//! `Set_LOD_Max_Screen_Size(-1..18, ...)`, the LOD queries, `Set_Hidden`, `Set_Visible`, `Set_Animation_Hidden`,
//! `Set_Invisible`, `Stop`, `Start`, `Reset`, `Remove_Buffer_From_Scene`, the emitter removed from and added to the scene,
//! the emitter destroyed (its buffer kept by the scene), the buffer hidden, `Scale`, `Set_One_Time_Burst`,
//! `Enable_Remove_On_Complete`, `Buffer_Scene_Not_Needed`, `Set_Emission_Rate`. A frame runs the scene's frame: the
//! emitter's `On_Frame_Update` (if in the scene), the buffer's `On_Frame_Update` and `Render` (if in the scene), then
//! the scene's release of what registered (`Post_Render_Processing`). Compared, word for word (floats as bits): after
//! creation and after every operation every emitter field, every buffer field (the queue's new particles, the ring's
//! particles: positions, velocities, time stamps, group IDs; the indices, bounding box, threshold, LOD count, bias,
//! projected area, all 17 costs and 18 values, the cached bounding sphere and its valid bit, the line renderer's width
//! and noise), the release registrations; per frame what the renderer was given (every point, line run or line);
//! per `Prepare_LOD` the cost handed to the optimizer; per query each answer.
//!
//! An operation the Rust reports undefined upstream (a LOD array read past its end after `Set_LOD_Level(17)` or
//! `Decrement_LOD` to 17, a screen size of level 17) ends the sequence before that operation: the C++ is not run on it.
//!
//! # Predictions
//!
//! `issue65b_predictions.md` (written before the first run): see the results.
//!
//! # Results (2026-11-14)
//!
//! - First run that compiled (after a test error: the frame's `dt` was not put in the input, and the C++ aborted
//!   reading past it): **5 mismatching sequences of 20,000, all a NaN's sign bit** in `ProjectedArea` (a degenerate
//!   view plane makes `0 * inf`; after `0.9f * NaN + 0.1f * x` the Rust's NaN had the sign bit clear, the C++'s set).
//!   Rust leaves a NaN's sign and payload unspecified; floats now compare NaN equal to NaN, as `differential_segline`
//!   does. Then: **0 mismatching sequences of 20,000**; 89,951,959 words (1,144 NaN-only), 183,209 frames (9,677 of
//!   an orphaned buffer), 1,058,742 points or line points drawn, 71,210 frames drawn with a threshold of 1 to 15,
//!   44,289 `Prepare_LOD` (833 raised the level), 2,989 frames with a release; **4,288 sequences end at upstream
//!   undefined** (21 %; predicted 15-35 %).
//! - The corrected visibility bits (`Is_Not_Hidden_At_All` without `IS_VISIBLE`, not visible at start): the old Rust
//!   (and the old stand-in) put back mismatch in all 20,000 sequences (the bits are compared after every operation).
//! - **Plants**, mismatching sequences of 20,000: 10116 3,908; 10117 3,893; 10118 19,322; 10119 78; 10120 441; 10121
//!   19,322; 10122 3,085; 10123 8,650; 10124 1,267; 10125 11,467; 10126 2,863; 10127 2,545; 10128 1,627; 10129 2,344;
//!   10130 8,223; 10131 3,495; 10132 8,223; 10133 6,876; 10134 407; 10135 945. **20 of 20 caught.** 10124 and 10129
//!   first crashed the C++ (the Rust, planted, skipped an undefined read the C++ then made): the C++ driver now guards
//!   the LOD reads and answers `'U'`. 10134 was first planted as the visual state updated at threshold 16, which draws
//!   nothing and was caught by 0: replanted as not updated at 15.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 10116 | the line mode's cost factor is `2 * MaxNum` (not `2 * MaxNum - 1`) |
//! | 10117 | the prism's cost factor is 4 polygons per particle |
//! | 10118 | every cost is the small nonzero cost |
//! | 10119 | a screen size equal to the area counts as smaller |
//! | 10120 | with every level clamped, the minimum level is 0 |
//! | 10121 | the benefit factor divides by the polygon count, not its square |
//! | 10122 | the LOD bias is left out of the value |
//! | 10123 | `Increment_LOD` does nothing |
//! | 10124 | `Decrement_LOD` stops at the lowest level (16) |
//! | 10125 | `Get_Value` reads the post-increment value |
//! | 10126 | `Set_LOD_Level` bounds to 16 |
//! | 10127 | `Set_LOD_Bias` keeps a negative bias |
//! | 10128 | the bounding sphere is never cached |
//! | 10129 | a hidden buffer prepares its LOD |
//! | 10130 | the height factor divides by the view plane's width |
//! | 10131 | the projected area takes the bounding sphere's always |
//! | 10132 | the area filter's weights are swapped |
//! | 10133 | `Generate_APT` keeps a slot only above the threshold |
//! | 10134 | the visual state is not updated when the threshold is 15 |
//! | 10135 | `Render_Line` decimates by the slot modulo 8 |

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::part_buf::{LodCamera, ParticleBuffer, ParticleDraw, Props, Ww3dClock};
use rts_lockstep_engine::formats::w3d::part_emt::{EmitterArgs, ParticleEmitter, ParticleRandom};
use rts_lockstep_engine::formats::w3d::part_ldr::Vector3Randomizer;
use rts_lockstep_engine::formats::w3d::shader::{ShaderClass, PRESET_ADDITIVE_SPRITE_SHADER, PRESET_ALPHA_SPRITE_SHADER};
use rts_lockstep_engine::formats::w3d::w3d_file::W3dEmitterLinePropertiesStruct;
use rts_lockstep_engine::formats::w3d::{W3dError, W3dResult};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::random::{Random3, Random4};
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::Gen;

fn planted() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn cases() -> u64 {
    std::env::var("PART_LOD_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
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

/// The emitter, or its buffer once the emitter is destroyed (and whether the scene still has the buffer).
enum Holder {
    Alive(ParticleEmitter),
    Orphan(ParticleBuffer, bool),
}

impl Holder {
    fn buffer(&mut self) -> &mut ParticleBuffer {
        match self {
            Holder::Alive(e) => &mut e.buffer,
            Holder::Orphan(b, _) => b,
        }
    }
    fn buffer_in_scene(&self) -> bool {
        match self {
            Holder::Alive(e) => e.buffer_in_scene,
            Holder::Orphan(_, s) => *s,
        }
    }
}

/// wrap_part_lod.cpp's log_state.
fn state_words(o: &mut Vec<u32>, h: &Holder, clock: &Ww3dClock, releases: u32) {
    let b = match h {
        Holder::Alive(e) => {
            wu(o, 1);
            wu(o, e.emit_rate);
            wu(o, e.burst_size);
            wu(o, e.one_time_burst_size);
            wu(o, e.one_time_burst as u32);
            wv3(o, e.base_vel);
            wf(o, e.outward_vel);
            wf(o, e.vel_inherit_factor);
            wu(o, e.emit_remain);
            for x in [e.prev_q.x, e.prev_q.y, e.prev_q.z, e.prev_q.w] {
                wf(o, x);
            }
            wv3(o, e.prev_orig);
            for x in [e.active, e.first_time, e.buffer_scene_needed] {
                wu(o, x as u32);
            }
            wu(o, e.particles_left as u32);
            for x in [e.is_complete, e.remove_on_complete, e.is_in_scene] {
                wu(o, x as u32);
            }
            wu(o, e.group_id as u32);
            for x in [e.is_invisible, e.hidden, e.animation_hidden, e.visible, e.is_not_hidden_at_all()] {
                wu(o, x as u32);
            }
            &e.buffer
        }
        Holder::Orphan(b, _) => {
            wu(o, 0);
            b
        }
    };
    for x in [h.buffer_in_scene(), b.hidden, b.animation_hidden, b.visible] {
        wu(o, x as u32);
    }
    wu(o, b.new_particle_queue_start);
    wu(o, b.new_particle_queue_end);
    wu(o, b.new_particle_queue_count as u32);
    let mut q = b.new_particle_queue_start as usize;
    for _ in 0..b.new_particle_queue_count {
        let np = &b.new_particle_queue[q];
        wv3(o, np.position);
        wv3(o, np.velocity);
        wu(o, np.time_stamp);
        wu(o, np.group_id as u32);
        q += 1;
        if q == b.max_num as usize {
            q = 0;
        }
    }
    wu(o, b.render_mode as u32);
    wv3(o, b.accel);
    wu(o, b.has_accel as u32);
    wu(o, b.max_age);
    wu(o, b.last_update_time);
    wu(o, b.is_emitter_dead as u32);
    wf(o, b.max_size);
    for x in [b.max_num, b.start, b.end, b.new_end, b.non_new_num as u32, b.new_num as u32] {
        wu(o, x);
    }
    wv3(o, b.bounding_box.center);
    wv3(o, b.bounding_box.extent);
    wu(o, b.bounding_box_dirty as u32);
    wf(o, b.size_random);
    wu(o, b.has_emitter as u32);
    wu(o, b.current_group_id as u32);
    let (sub1_end, sub2_start) =
        if b.start < b.end || (b.start == b.end && b.non_new_num == 0) { (b.end, b.end) } else { (b.max_num, 0) };
    let pp = if b.ping_pong_position { (clock.frame_count & 1) as usize } else { 0 };
    let pos = b.position[pp].as_ref().expect("Position");
    for i in (b.start..sub1_end).chain(sub2_start..b.end) {
        let i = i as usize;
        wv3(o, pos[i]);
        wv3(o, b.velocity[i]);
        wu(o, b.time_stamp[i]);
        wu(o, b.group_id[i] as u32);
    }
    wu(o, b.decimation_threshold);
    wu(o, b.lod_count);
    wf(o, b.lod_bias);
    wf(o, b.projected_area);
    for c in b.cost {
        wf(o, c);
    }
    for v in b.value {
        wf(o, v);
    }
    wu(o, b.bounding_volumes_valid as u32);
    if b.bounding_volumes_valid {
        wv3(o, b.cached_bounding_sphere.center);
        wf(o, b.cached_bounding_sphere.radius);
    }
    if let Some(lr) = &b.line_renderer {
        wf(o, lr.width);
        wf(o, lr.noise_amplitude);
    }
    wu(o, releases);
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

#[derive(Default)]
struct Tally {
    ended_undefined: u64,
    creation_undefined: u64,
    frames: u64,
    drawn: u64,
    decimated_frames: u64,
    prepares: u64,
    level_raised: u64,
    releases: u64,
    orphan_frames: u64,
}

/// One random sequence: the input words and the Rust's output words.
fn build_case(g: &mut Gen, t: &mut Tally) -> Option<(Vec<u32>, Vec<u32>)> {
    let mut input = Vec::new();
    let mut o = Vec::new();
    let (s3a, s3b, s4) = (g.u32(), g.u32(), g.u32());
    let mut sync = g.below(100_000);
    input.extend([s3a, s3b, s4, sync]);
    let mut rng = ParticleRandom { v3: Random3::new(s3a, s3b), buf: Random4::new(s4), lod_max_screen_sizes: Default::default() };
    let mut clock = Ww3dClock { sync_time: sync, frame_time: 0, frame_count: 0 };
    let emit_rate = if g.below(20) == 0 { 0.0 } else { g.uniform(0.5, 30.0) };
    let burst_size = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
    input.extend([emit_rate.to_bits(), burst_size]);
    let pos_rnd = gen_rand(g, &mut input);
    let base_vel = Vector3::new(g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0));
    input.extend([base_vel.x.to_bits(), base_vel.y.to_bits(), base_vel.z.to_bits()]);
    let vel_rnd = gen_rand(g, &mut input);
    let out_vel = if g.below(2) == 0 { 0.0 } else { g.uniform(-5.0, 5.0) };
    let inherit = if g.below(2) == 0 { 0.0 } else { g.uniform(-1.0, 2.0) };
    input.extend([out_vel.to_bits(), inherit.to_bits()]);
    let max_age = g.uniform(0.05, 4.0);
    let color = gen_v(g, &mut input, max_age);
    let opacity = gen_f(g, &mut input, (0.0, 1.0), 0.5, (-0.2, 1.2), max_age);
    let size = gen_f(g, &mut input, (0.0, 3.0), 1.0, (-1.0, 6.0), max_age);
    let rotation_p = gen_f(g, &mut input, (-3.0, 3.0), 1.0, (-3.0, 3.0), max_age);
    let orient_rnd = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 1.0) };
    input.push(orient_rnd.to_bits());
    let frames = gen_f(g, &mut input, (0.0, 16.0), 5.0, (0.0, 16.0), max_age);
    let blur = gen_f(g, &mut input, (0.0, 0.2), 0.06, (0.0, 0.2), max_age);
    let accel = if g.below(2) == 0 { Vector3::new(0.0, 0.0, 0.0) } else { Vector3::new(g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0), g.uniform(-30.0, 5.0)) };
    input.extend([accel.x.to_bits(), accel.y.to_bits(), accel.z.to_bits(), max_age.to_bits(), 0.0f32.to_bits()]);
    let has_tex = g.below(2);
    let shader = match g.below(3) {
        0 => PRESET_ADDITIVE_SPRITE_SHADER,
        1 => PRESET_ALPHA_SPRITE_SHADER,
        _ => ShaderClass(g.u32() & 0x000F_FFFF),
    };
    let max_particles = if g.below(4) == 0 { g.below(40) as i32 } else { 0 };
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
    let args = EmitterArgs {
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
    };
    let mut e = match ParticleEmitter::new(args, &mut rng, &clock) {
        Ok(e) => e,
        Err(_) => {
            t.creation_undefined += 1;
            return None;
        }
    };
    let mut pos = Vector3::new(g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(0.0, 20.0));
    let vel = Vector3::new(g.uniform(-0.05, 0.05), g.uniform(-0.05, 0.05), 0.0);
    let spin = g.uniform(-0.003, 0.003);
    let mut angle = g.uniform(-3.0, 3.0);
    let mut m = rotation(g, angle);
    m.set_translation(&pos);
    push_m(&mut input, &m);
    e.set_transform(m);
    e.notify_added();
    e.set_hidden(false);
    let mut h = Holder::Alive(e);
    let mut releases = 0u32;
    wu(&mut o, 'C' as u32);
    state_words(&mut o, &h, &clock, releases);
    let nops = 10 + g.below(31);
    for _ in 0..nops {
        let alive = matches!(h, Holder::Alive(_));
        let op = loop {
            let k = g.below(1000);
            let op = match k {
                0..=399 => 0,
                400..=499 => 1,
                500..=549 => 2,
                550..=609 => 3,
                610..=659 => 4,
                660..=689 => 5,
                690..=729 => 6,
                730..=789 => 7,
                790..=819 => 8,
                820..=839 => 9,
                840..=859 => 10,
                860..=879 => 11,
                880..=894 => 12,
                895..=909 => 13,
                910..=919 => 14,
                920..=929 => 15,
                930..=939 => 16,
                940..=949 => 17,
                950..=959 => 18,
                960..=979 => 19,
                980..=984 => 20,
                985..=989 => 21,
                990..=994 => 22,
                995..=996 => 23,
                _ => 24,
            };
            if alive || [0, 1, 2, 3, 4, 5, 6, 7, 19].contains(&op) {
                break op;
            }
        };
        let in_mark = input.len();
        let out_mark = o.len();
        input.push(op);
        wu(&mut o, op);
        let r: W3dResult<()> = (|| {
            match op {
                0 => {
                    let dt = match g.below(20) {
                        0 | 1 => 0,
                        2..=13 => 1 + g.below(50),
                        14..=18 => 50 + g.below(350),
                        _ => 3000 + g.below(5000),
                    };
                    input.push(dt);
                    sync = sync.wrapping_add(dt);
                    clock = Ww3dClock { sync_time: sync, frame_time: dt, frame_count: clock.frame_count + 1 };
                    pos += vel * dt as f32;
                    angle += spin * dt as f32;
                    if g.below(40) == 0 {
                        pos.x += g.uniform(-100.0, 100.0);
                    }
                    let mut m = rotation(g, angle);
                    m.set_translation(&pos);
                    push_m(&mut input, &m);
                    t.frames += 1;
                    let before = releases;
                    let mut list = (false, false);
                    if let Holder::Alive(e) = &mut h {
                        e.set_transform(m);
                        if e.is_in_scene && e.on_frame_update() {
                            releases += 1;
                            list.0 = true;
                        }
                    }
                    if h.buffer_in_scene() {
                        let d = match &mut h {
                            Holder::Alive(e) => {
                                if e.buffer_on_frame_update(&clock, &mut rng)? {
                                    releases += 1;
                                    list.1 = true;
                                }
                                let at = (e.get_position(), e.is_stopped());
                                e.buffer.render(&clock, Some(at))?
                            }
                            Holder::Orphan(b, _) => {
                                t.orphan_frames += 1;
                                if b.on_frame_update() {
                                    releases += 1;
                                    list.1 = true;
                                }
                                b.render(&clock, None)?
                            }
                        };
                        let thr = h.buffer().decimation_threshold;
                        if thr > 0 && thr < 16 {
                            t.decimated_frames += 1;
                        }
                        t.drawn += draw_words(&mut o, &d);
                    } else {
                        wu(&mut o, 'N' as u32);
                    }
                    wu(&mut o, releases - before);
                    // Post_Render_Processing: each registered object removed (no container)
                    if list.0 {
                        if let Holder::Alive(e) = &mut h {
                            if e.is_in_scene {
                                e.notify_removed();
                            }
                        }
                    }
                    if list.1 {
                        match &mut h {
                            Holder::Alive(e) => e.buffer_in_scene = false,
                            Holder::Orphan(_, s) => *s = false,
                        }
                    }
                    if list.0 || list.1 {
                        t.releases += 1;
                    }
                }
                1 => {
                    let cam = LodCamera {
                        position: if g.below(30) == 0 {
                            Vector3::new(0.0, 0.0, 0.0)
                        } else {
                            Vector3::new(g.uniform(-200.0, 200.0), g.uniform(-200.0, 200.0), g.uniform(-50.0, 200.0))
                        },
                        viewport_min: if g.below(5) == 0 { Vector2::new(g.uniform(0.0, 100.0), g.uniform(0.0, 100.0)) } else { Vector2::new(0.0, 0.0) },
                        viewport_max: Vector2::new(g.uniform(400.0, 1600.0), g.uniform(300.0, 1200.0)),
                        view_plane_min: Vector2::new(-g.uniform(0.3, 1.5), -g.uniform(0.3, 1.5)),
                        view_plane_max: if g.below(40) == 0 { Vector2::new(0.0, 0.0) } else { Vector2::new(g.uniform(0.3, 1.5), g.uniform(0.3, 1.5)) },
                    };
                    let mut cam = cam;
                    if g.below(40) == 0 {
                        cam.view_plane_max = Vector2::new(-cam.view_plane_min.x, -cam.view_plane_min.y);
                        cam.view_plane_min = cam.view_plane_max;
                    }
                    for x in [cam.position.x, cam.position.y, cam.position.z, cam.viewport_min.x, cam.viewport_min.y, cam.viewport_max.x, cam.viewport_max.y] {
                        input.push(x.to_bits());
                    }
                    for x in [cam.view_plane_min.x, cam.view_plane_min.y, cam.view_plane_max.x, cam.view_plane_max.y] {
                        input.push(x.to_bits());
                    }
                    let sizes = rng.lod_max_screen_sizes;
                    let lvl0 = h.buffer().get_lod_level();
                    match h.buffer().prepare_lod(&cam, &sizes, &clock)? {
                        Some(c) => {
                            wu(&mut o, 1);
                            wf(&mut o, c);
                        }
                        None => wu(&mut o, 0),
                    }
                    t.prepares += 1;

                    if h.buffer().get_lod_level() != lvl0 {
                        t.level_raised += 1;
                    }
                }
                2 => h.buffer().increment_lod(),
                3 => h.buffer().decrement_lod(),
                4 => {
                    let l = g.below(23) as i32 - 3;
                    input.push(l as u32);
                    h.buffer().set_lod_level(l);
                }
                5 => {
                    let f = match g.below(10) {
                        0 => -g.uniform(0.0, 1.0),
                        1 => f32::NAN,
                        _ => g.uniform(0.0, 3.0),
                    };
                    input.push(f.to_bits());
                    match &mut h {
                        Holder::Alive(e) => e.set_lod_bias(f),
                        Holder::Orphan(b, _) => b.set_lod_bias(f),
                    }
                }
                6 => {
                    let l = g.below(20) as i32 - 1;
                    let f = match g.below(6) {
                        0 => 0.0,
                        1 => f32::MAX,
                        2 => f32::INFINITY,
                        _ => g.uniform(0.0, 60_000.0),
                    };
                    input.extend([l as u32, f.to_bits()]);
                    rng.lod_max_screen_sizes.set(l, f)?;
                }
                7 => {
                    let l = g.below(20) as i32 - 1;
                    input.push(l as u32);
                    let b = h.buffer();
                    let (c, v, pv) = (b.get_cost()?, b.get_value()?, b.get_post_increment_value()?);
                    let (lv, n) = (b.get_lod_level(), b.get_lod_count());
                    let s = rng.lod_max_screen_sizes.get(l)?;
                    wf(&mut o, c);
                    wf(&mut o, v);
                    wf(&mut o, pv);
                    wu(&mut o, lv as u32);
                    wu(&mut o, n as u32);
                    wf(&mut o, s);
                    if let Holder::Alive(e) = &h {
                        wf(&mut o, e.get_cost());
                        wf(&mut o, e.get_value());
                        wf(&mut o, e.get_post_increment_value());
                        wu(&mut o, e.get_lod_level() as u32);
                        wu(&mut o, e.get_lod_count() as u32);
                    }
                }
                19 => {
                    let on = g.below(2);
                    input.push(on);
                    h.buffer().set_hidden(on != 0);
                }
                _ => {
                    let Holder::Alive(e) = &mut h else { unreachable!() };
                    match op {
                        8 | 9 | 10 | 11 => {
                            let on = g.below(2);
                            input.push(on);
                            match op {
                                8 => e.set_hidden(on != 0),
                                9 => e.set_visible(on != 0),
                                10 => e.set_animation_hidden(on != 0),
                                _ => e.set_invisible(on != 0),
                            }
                        }
                        12 => e.stop(),
                        13 => e.start(),
                        14 => e.reset(),
                        15 => e.remove_buffer_from_scene(),
                        16 => {
                            if e.is_in_scene {
                                e.notify_removed();
                            }
                        }
                        17 => e.notify_added(),
                        18 => {
                            let Holder::Alive(e) = std::mem::replace(&mut h, Holder::Orphan(ParticleBuffer::new(&dummy_params(), &mut Random4::new(1), &clock, &Default::default())?, false)) else {
                                unreachable!()
                            };
                            let s = e.buffer_in_scene;
                            h = Holder::Orphan(e.into_buffer(), s);
                        }
                        20 => {
                            let f = g.uniform(0.0, 3.0);
                            input.push(f.to_bits());
                            e.scale(f);
                        }
                        21 => {
                            let n = g.below(6) as i32;
                            input.push(n as u32);
                            e.set_one_time_burst(n);
                        }
                        22 => {
                            let on = g.below(2);
                            input.push(on);
                            e.enable_remove_on_complete(on != 0);
                        }
                        23 => e.buffer_scene_not_needed(),
                        _ => {
                            let f = g.uniform(0.5, 40.0);
                            input.push(f.to_bits());
                            e.set_emission_rate(f)?;
                        }
                    }
                }
            }
            Ok(())
        })();
        match r {
            Ok(()) => state_words(&mut o, &h, &clock, releases),
            Err(W3dError::Undefined(_)) | Err(W3dError::NonTerminating) => {
                input.truncate(in_mark);
                o.truncate(out_mark);
                t.ended_undefined += 1;
                break;
            }
            Err(e) => panic!("unexpected {e:?}"),
        }
    }
    Some((input, o))
}

/// A throwaway buffer for the swap in operation 18 (never compared).
fn dummy_params() -> rts_lockstep_engine::formats::w3d::part_buf::BufferParams {
    rts_lockstep_engine::formats::w3d::part_buf::BufferParams {
        buffer_size: 2,
        color: Props::default(),
        opacity: Props::default(),
        size: Props::default(),
        rotation: Props::default(),
        orient_rnd: 0.0,
        frame: Props::default(),
        blurtime: Props::default(),
        accel: Vector3::default(),
        max_age: 1.0,
        future_start: 0.0,
        texture: None,
        shader: PRESET_ADDITIVE_SPRITE_SHADER,
        pingpong: false,
        render_mode: 0,
        frame_mode: 0,
        line_props: None,
    }
}

/// *Results (2026-11-14):* 20,000 sequences, 0 mismatching, 4,288 ended at upstream undefined (see the module docs).
#[test]
fn part_lod_matches_upstream() {
    let mut g = Gen::new(0x65B_10D);
    let n = cases();
    let mut t = Tally::default();
    let (mut words, mut mismatched, mut compared, mut nan_only) = (0u64, 0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    let mut out = vec![0u32; 1 << 24];
    for case in 0..n {
        let Some((input, rust)) = build_case(&mut g, &mut t) else { continue };
        let len = unsafe { cpp::ref_part_lod_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        assert!(len <= out.len(), "output buffer too small: {len}");
        let cw = &out[..len];
        compared += 1;
        words += rust.len().min(len) as u64;
        // floats are compared as bits, a NaN equal to any NaN (Rust leaves a NaN's sign and payload unspecified)
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
                    rust.get(i.saturating_sub(2)..(i + 3).min(rust.len())),
                    cw.get(i.saturating_sub(2)..(i + 3).min(len))
                ));
            }
        }
    }
    let line = format!(
        "part_lod: {n} sequences, {compared} compared ({} not made: undefined at creation), {words} words ({nan_only} NaN-only), {} frames ({} \
         of an orphaned buffer), {} points or line points drawn, {} frames drawn with a threshold of 1 to 15, {} \
         Prepare_LOD ({} raised the level), {} frames with a release, {} ended at upstream undefined; mismatching \
         sequences {mismatched}",
        t.creation_undefined, t.frames, t.orphan_frames, t.drawn, t.decimated_frames, t.prepares, t.level_raised, t.releases, t.ended_undefined
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
