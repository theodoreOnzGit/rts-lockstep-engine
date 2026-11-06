// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the particle systems and the FX lists (#46; `client::particle_sys` and
//! `client::fx_list`, translated in #43/#44) against upstream's own `ParticleSys.cpp`, `INIParticleSys.cpp` and
//! `FXList.cpp` (commit 0a05454d), compiled by `build.rs` with the real `ParticleSys.h`, `FXList.h`, `INI.h` and
//! `BaseType.h` and driven by `differential/wrap_particle_sys.cpp` (which lists every stand-in: an attachment-less
//! Drawable and Object, frames, flat ground, the global data and LOD limits, and for the FX lists the audio, display,
//! view, scorch, ray-effect and tracer requests, logged). The INI is read by the real `INI::load` (wrap_ini.cpp's
//! `ref_ini_load`, whose `ParticleSystem` and `FXList` blocks reach the real handlers).
//!
//! No game files are used: every `ParticleSystem` and `FXList` block is **synthetic**.
//!
//! # Methodology
//!
//! - **The particle INI** (`particle_system_ini_matches_upstream`, 5,000 cases): random files of one to six
//!   `ParticleSystem` blocks, each a random subset of the fields in a random order (some names repeated, so a block
//!   reads into an earlier one; the union fields over one another), with valid and invalid values (unknown names,
//!   words for numbers, missing tokens, colours out of 0..255, a distribution name), loaded by both sides; compared:
//!   the outcome (success, or the thrown status and message) and every field of every template (in name order).
//! - **The runs** (`particle_systems_run_as_upstream`, 5,000 cases): one to five valid templates covering every
//!   velocity type (`ORTHO`, `SPHERICAL`, `HEMISPHERICAL`, `CYLINDRICAL`, `OUTWARD`, and none) and every volume type
//!   (`POINT`, `LINE`, `BOX`, `SPHERE`, `CYLINDER`, and none), hollow volumes, every shader, wind ping-pong and
//!   circular, slaves (chains, with offsets), per-particle attached systems, ground alignment and emission above
//!   ground only, particles turned to their emitter, keyframes, gravity, drift; the particle cap (`MaxParticleCount`
//!   0, 4, 12, 40 or 400), the field cap and the LOD priority limits. Each case seeds the client generator with the
//!   same state on both sides (`RandomValues::client` and `ref_set_state(1, ..)`, so `GameClientRandomValue` draws
//!   the same numbers), creates systems (some with slaves, some attached to an object), positions, turns and attaches
//!   them (to objects, which may have a drawable of their own, and to drawables), tints them, and runs 30 to 90
//!   frames, between which it moves, stops, starts and destroys systems, moves, shrouds and removes the objects and
//!   drawables, changes the ground, the settings and the multipliers, makes more systems, and sometimes repeats a
//!   logic frame (no update). After every frame: the manager's counts, every system (its whole `ParticleSystemInfo`,
//!   attachments, transforms, delays, lifetimes, coefficients, positions, slave, master and control particle, flags)
//!   and every particle (all of `ParticleInfo` and of `Particle`: position, velocity, size, angle, alpha, colour and
//!   their rates and target keys, lifetime left, the system it controls), the per-priority overall lists, and the
//!   client generator's six words. Every `f32` is compared as bits; two NaNs count as equal (`support` explains why).
//!   `computeParticleVelocity`'s `static Coord3D newVel` outlives the C++ manager: `ref_psys_reset` zeroes it (one
//!   call through the default branch), as a new Rust manager starts.
//! - **The FX INI** (`fx_list_ini_matches_upstream`, 5,000 cases): random files of `FXList` blocks of every nugget
//!   (`Sound`, `RayEffect`, `Tracer`, `LightPulse`, `ViewShake`, `TerrainScorch`, `ParticleSystem`, `FXListAtBonePos`)
//!   with random fields, some damaged (a word for every value, a missing `End`); compared: the outcome and every list
//!   and nugget field. `FXListAtBonePos`'s list: INI.cpp's `parseFXList` resolves through the global `FXListStore`,
//!   in this binary the Armor build's stand-in (a pointer per name), so the C++ keeps the name, as the Rust does.
//! - **The FX plays** (`fx_lists_play_as_upstream`, 2,000 cases): templates and FX lists, then 10 to 50 steps of
//!   `doFXPos` (with or without a transform, a secondary, a speed and a radius; some primaries in shroud) and
//!   `doFXObj` (objects or none, fogged or not) of a random list (or `None`, or a missing one), and particle updates;
//!   after each: the requests (sound, light pulse, view shake, scorch, tracer, ray effect, with every argument), the
//!   particle systems the `ParticleSystem` nuggets made (the dump above) and the client generator.
//! - **Undefined upstream** (reported by the Rust in `ParticleSystemManager::undefined`, see its module docs): a key
//!   past the eighth, a float cast to an unsigned or int out of range. The generator makes them sometimes (all eight
//!   keys with frames, a negative lifetime or delay); a case stops being compared at the frame where the Rust reports
//!   one (before the frame's update when it is a new system's initial delay), and is counted.
//! - **Planted bugs**: `L9_PLANT=<id>` (`logic::modules::plant`) switches one on in the Rust: 4401 (the hollow box
//!   stops instead of falling through into `LINE`), 4402 (the green clamp tests green, not upstream's red), 4403 (no
//!   ping-pong minimum change), 4404 (`removeOldestParticles` takes the newest), 4405 (`OUTWARD` without a volume
//!   zeroes the velocity instead of keeping the static), 4406 (no slave offset), 4407 (the inter-frame emission
//!   adjustment counted from 1), and the three defects this test found, put back: 4408 (`cosf` in `float`), 4409
//!   (`tintAllColors` dividing once), 4410 (a slave's particle listed under its master's priority); 4411 (an alpha
//!   key's distribution `CONSTANT`, the INI test); for the FX lists 4412 (the random scorch one short), 4413 (the
//!   ricochet angle from y and x swapped), 4414 (the nugget's `cosf` in `float`, put back), 4415 (`InitialDelay`'s
//!   default 0, not -1, the FX INI test). Each must make its test fail.
//!
//! # Predictions (written before the first run)
//!
//! Expected 0 mismatches, except where reading the two side by side (before the run) found differences:
//! 1. The system dump: upstream's `ParticleSystem` constructor does not copy the template's `m_systemLifetime`,
//!    `m_initialDelay` and `m_slaveSystemName` (they keep `ParticleSystemInfo()`'s 0, the zeroed pool's `CONSTANT 0 0`
//!    and the empty string); the Rust clones the whole template. Unobservable in the run, but the dump differs.
//! 2. A slave's particles: `Particle::Particle` lists a particle under its OWN system's priority
//!    (`system->getPriority()`), and `removeParticle` unlists it under that; the Rust lists a slave particle under
//!    the master's priority (`createParticle`'s argument). The overall lists differ, and so may what
//!    `removeOldestParticles` removes, when a slave's priority differs from its master's.
//! 3. `CYLINDRICAL` velocity and the `CYLINDER` volume call `cos`/`sin` on a `Real`: C semantics (as the MSVC headers),
//!    the double functions, rounded back; the Rust calls `WWMath::Cos`/`Sin` (`cosf`). A last-bit difference in some
//!    positions and velocities.
//! 4. `tintAllColors`: `RGBColor::setFromInt` already divides by 255, and upstream divides by 255 again; the Rust
//!    divides once.
//! 5. `createSlaveSystem` caches the slave template (`m_slaveTemplate`) on first use; the Rust looks the name up each
//!    time. Only visible if a later INI load renames a template's slave after a system of it was made (not
//!    generated: the templates are loaded once per case), so no mismatch expected from it here.
//! 6. (The FX tests, added after the particle tests were green.) 0 mismatches in the reader and the requests. The
//!    `ParticleSystem` nugget's `radius * cos(angle)` is the double `cos` as in 3; noticed while reading `FXList.cpp`
//!    before the first FX run but **not written down until after it** (so not counted as a prediction).
//!
//! # Results (2026-11-06, `cargo +1.97.1 test --release --features differential --test differential_particle_sys`)
//!
//! - **First runs** (200 cases, then 40): the INI comparison was clean from the start (438 loads, 0 mismatches). The
//!   runs: prediction 1 held (words 37 and 124 of the system dump: `m_systemLifetime`, `m_initialDelay`, and the
//!   slave name's length); after fixing it, 27 mismatches in 809 frames: 2 (the overall lists), 3 (a one-ulp
//!   difference in a cylinder position) and 4 (tinted colours ~255 times too bright) as predicted, and one where the
//!   two sides' initial delays differed at frame 0: a negative `InitialDelay` (undefined upstream: the Rust saturates
//!   to 0, the C++ build gave 0xFFFFFFFF), which the test had not yet checked before comparing frame 0. All fixed in
//!   the Rust (see `client::particle_sys`), the undefined case now skipped.
//! - **5,000 runs: one more mismatch**, not predicted: a particle's velocity with `OUTWARD` and no volume type (the
//!   `static newVel` kept from the previous call) in case 2726. Not a translation defect: the C++ static outlived the
//!   case, after an earlier case had stopped on an undefined initial delay made between frames (the two sides then ran
//!   that frame differently, one emitting). Fixed in the harness: `ref_psys_reset` zeroes the static, and a case stops
//!   before the update when a new system's delay is undefined.
//! - **Then: 0 mismatches.** Particle INI: 5,000 cases, 7,419 loads (2,660 succeeded, 4,759 threw), 9,031 templates.
//!   Runs: 5,000 cases, 273,169 frames, 1,315,744 system-frames and 3,308,292 particle-frames compared (at most 1,222
//!   particles at once; 44,501 frames at the cap); system-frames with a slave 119,852, controlled by a particle
//!   245,286, ping-pong 351,902, circular 311,763, hollow box 108,230, on an object 137,885, on a drawable 19,788,
//!   transformed 530,423, destroyed 149,391; 578 cases stopped where upstream is undefined (471 a float cast, 107 a key
//!   past the eighth).
//! - **FX lists: first run** 0 mismatches in the reader (7,478 loads) and in 54,623 steps of requests; 202 in the
//!   particle systems the nuggets made, all one ulp in a position: 6, the nugget's `cos`. Fixed in the Rust
//!   (`client::fx_list`). **Then 0**: FX INI 5,000 cases, 7,478 loads (5,737 succeeded, 1,741 threw), 33,595 nuggets;
//!   FX plays 2,000 cases, 58,634 steps, 19,382 requests (4,728 sounds, 3,601 light pulses, 3,620 shakes, 3,733
//!   scorches, 1,309 tracers, 2,391 ray effects), 5,949 particle systems made (246 attached to an object), 18 cases
//!   stopped where upstream is undefined.
//! - **Planted bugs: 15 of 15 caught** (4401: 695 frames differ; 4402: 2,310; 4403: 913; 4404: 884; 4405: 147; 4406:
//!   141; 4407: 722; 4408: 1,547; 4409: 1,439; 4410: 1,226; 4411: 5,864 template dumps; 4412: 177 request logs; 4413:
//!   19; 4414: 202; 4415: 2,290). A first 4401 (the hollow box's fifth side at `halfSize.x`) was NOT caught: the box
//!   falls through into `LINE`, which overwrites all three coordinates, so that quirk is unobservable; it was replaced.
//!
//! # What is not compared
//!
//! Save and load (`xfer`), `preloadAssets`, `findParentTemplate` and the debug display; `DRAWABLE`, `SMUDGE` and
//! `VOLUME_PARTICLE` rendering (the client draws quads); `createSlaveSystem`'s cached slave template (prediction 5);
//! `FXListAtBonePos` played at bones (the stand-ins have none), and its list resolved when read (see above); the
//! tracer's transform; `doFXPos` without a primary (upstream reads the cell of a NULL position).

#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use rts_lockstep_engine::client::fx_list::*;
use rts_lockstep_engine::client::particle_sys::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{IniResult, Thrown};
use rts_lockstep_engine::common::random_value::{seed_random, GameClientRandomVariable, RandomValues, DISTRIBUTION_TYPE_NAMES};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::Gen;

// ---------------------------------------------------------------------------
// Word comparison
// ---------------------------------------------------------------------------

/// Compares word dumps: equal words, or two words that are both NaN as `f32` (see `support`).
struct Words {
    name: &'static str,
    compared: u64,
    mismatches: u64,
    nan_only: u64,
    examples: Vec<String>,
}

impl Words {
    fn new(name: &'static str) -> Self {
        Words { name, compared: 0, mismatches: 0, nan_only: 0, examples: Vec::new() }
    }
    /// True when equal.
    fn cmp(&mut self, rust: &[u32], cpp: &[u32], ctx: impl Fn() -> String) -> bool {
        self.compared += 1;
        let n = rust.len().max(cpp.len());
        let mut nan_only = false;
        for i in 0..n {
            let (a, b) = (rust.get(i), cpp.get(i));
            if a == b {
                continue;
            }
            if let (Some(&a), Some(&b)) = (a, b) {
                if f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan() {
                    nan_only = true;
                    continue;
                }
            }
            self.mismatches += 1;
            if self.examples.len() < 6 {
                let lo = i.saturating_sub(4);
                let show = |v: &[u32]| v[lo.min(v.len())..(i + 6).min(v.len())].iter().map(|w| format!("{w:#x}")).collect::<Vec<_>>().join(" ");
                let c = ctx();
                let c = if std::env::var("PSYS_FULL").is_ok() { c } else { c.lines().next().unwrap_or("").to_string() };
                self.examples.push(format!("word {i} of {}/{}: rust [{}] c++ [{}] ({c})", rust.len(), cpp.len(), show(rust), show(cpp)));
            }
            return false;
        }
        if nan_only {
            self.nan_only += 1;
        }
        true
    }
    fn finish(self) {
        println!("DIFF {:<44} compared {:>9}  mismatches {:>6}  (NaN-payload-only {})", self.name, self.compared, self.mismatches, self.nan_only);
        assert!(self.compared > 0, "{}: nothing compared", self.name);
        assert_eq!(self.mismatches, 0, "{}: {} of {} differ; first: {:#?}", self.name, self.mismatches, self.compared, self.examples);
    }
}

// ---------------------------------------------------------------------------
// The dumps (wrap_particle_sys.cpp's dumpInfo, ref_psys_template_dump, ref_psys_dump)
// ---------------------------------------------------------------------------

fn f(v: &mut Vec<u32>, x: f32) {
    v.push(x.to_bits());
}
fn c3(v: &mut Vec<u32>, c: &Coord3D) {
    f(v, c.x);
    f(v, c.y);
    f(v, c.z);
}
fn s(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    v.extend(b.iter().map(|&x| u32::from(x)));
}
fn rv(v: &mut Vec<u32>, r: &GameClientRandomVariable) {
    v.push(r.distribution as u32);
    f(v, r.low);
    f(v, r.high);
}
fn mat(v: &mut Vec<u32>, m: &Matrix3D) {
    for r in &m.row {
        f(v, r.x);
        f(v, r.y);
        f(v, r.z);
        f(v, r.w);
    }
}

fn dump_info(v: &mut Vec<u32>, i: &ParticleSystemInfo) {
    v.push(u32::from(i.is_one_shot));
    v.push(i.shader_type as u32);
    v.push(i.particle_type as u32);
    s(v, &i.particle_type_name);
    for r in [&i.angle_z, &i.angular_rate_z, &i.angular_damping, &i.vel_damping, &i.lifetime] {
        rv(v, r);
    }
    v.push(i.system_lifetime);
    for r in [&i.start_size, &i.start_size_rate, &i.size_rate, &i.size_rate_damping] {
        rv(v, r);
    }
    v.push(i.volume_particle_depth);
    for k in &i.alpha_key {
        rv(v, &k.var);
        v.push(k.frame);
    }
    for k in &i.color_key {
        f(v, k.color.red);
        f(v, k.color.green);
        f(v, k.color.blue);
        v.push(k.frame);
    }
    for r in [&i.color_scale, &i.burst_delay, &i.burst_count, &i.initial_delay] {
        rv(v, r);
    }
    c3(v, &i.drift_velocity);
    f(v, i.gravity);
    s(v, &i.slave_system_name);
    c3(v, &i.slave_pos_offset);
    s(v, &i.attached_system_name);
    v.push(i.emission_velocity_type as u32);
    v.push(i.priority as u32);
    for r in &i.emission_velocity {
        rv(v, r);
    }
    v.push(i.emission_volume_type as u32);
    for &x in &i.emission_volume {
        f(v, x);
    }
    for b in [i.is_emission_volume_hollow, i.is_ground_aligned, i.is_emit_above_ground_only, i.is_particle_up_towards_emitter] {
        v.push(u32::from(b));
    }
    v.push(i.wind_motion as u32);
    for x in [
        i.wind_angle,
        i.wind_angle_change,
        i.wind_angle_change_min,
        i.wind_angle_change_max,
        i.wind_motion_start_angle,
        i.wind_motion_start_angle_min,
        i.wind_motion_start_angle_max,
        i.wind_motion_end_angle,
        i.wind_motion_end_angle_min,
        i.wind_motion_end_angle_max,
    ] {
        f(v, x);
    }
    v.push(u32::from(i.wind_motion_moving_to_end_angle));
}

fn rust_template_dump(m: &ParticleSystemManager) -> Vec<u32> {
    let mut v = vec![m.templates.len() as u32];
    for (name, t) in &m.templates {
        s(&mut v, name);
        dump_info(&mut v, &t.info);
    }
    v
}

fn cpp_words(f: impl Fn(*mut u32, c_int) -> c_int) -> Vec<u32> {
    let mut buf = vec![0u32; 1 << 16];
    loop {
        let n = f(buf.as_mut_ptr(), buf.len() as c_int);
        if n >= 0 {
            buf.truncate(n as usize);
            return buf;
        }
        buf = vec![0u32; (-n) as usize];
    }
}

fn cpp_template_dump() -> Vec<u32> {
    cpp_words(|p, n| unsafe { cpp::ref_psys_template_dump(p, n) })
}

fn rust_dump(m: &ParticleSystemManager) -> Vec<u32> {
    let mut v = vec![
        m.particle_count,
        m.unique_system_id,
        m.systems.len() as u32,
        m.last_logic_frame_update.unwrap_or(u32::MAX),
        m.field_particle_count,
        m.systems.len() as u32,
    ];
    for sy in &m.systems {
        v.push(sy.id);
        s(&mut v, &sy.template);
        dump_info(&mut v, &sy.info);
        v.push(sy.attached_to_drawable as u32);
        v.push(sy.attached_to_object as u32);
        mat(&mut v, &sy.local_transform);
        mat(&mut v, &sy.transform);
        v.extend([sy.burst_delay_left, sy.delay_left, sy.start_timestamp, sy.system_lifetime_left, sy.personality_store]);
        f(&mut v, sy.accumulated_size_bonus);
        c3(&mut v, &sy.vel_coeff);
        f(&mut v, sy.count_coeff);
        f(&mut v, sy.delay_coeff);
        f(&mut v, sy.size_coeff);
        c3(&mut v, &sy.pos);
        c3(&mut v, &sy.last_pos);
        v.push(sy.slave.unwrap_or(0));
        v.push(sy.master.unwrap_or(0));
        match sy.control_particle {
            Some((cs, p)) => v.extend([1, cs, p]),
            None => v.extend([0, 0, 0]),
        }
        for b in [sy.is_local_identity, sy.is_identity, sy.is_forever, sy.is_stopped, sy.is_destroyed, sy.is_first_pos, sy.is_saveable, sy.skip_parent_xfrm] {
            v.push(u32::from(b));
        }
        v.push(sy.particles.len() as u32);
        for p in &sy.particles {
            let i = &p.info;
            v.push(p.personality);
            c3(&mut v, &i.vel);
            c3(&mut v, &i.pos);
            c3(&mut v, &i.emitter_pos);
            for x in [i.vel_damping, i.angle_z, i.angular_rate_z, i.angular_damping] {
                f(&mut v, x);
            }
            v.push(i.lifetime);
            for x in [i.size, i.size_rate, i.size_rate_damping] {
                f(&mut v, x);
            }
            for k in &i.alpha_key {
                f(&mut v, k.value);
                v.push(k.frame);
            }
            for k in &i.color_key {
                f(&mut v, k.color.red);
                f(&mut v, k.color.green);
                f(&mut v, k.color.blue);
                v.push(k.frame);
            }
            f(&mut v, i.color_scale);
            f(&mut v, i.wind_randomness);
            v.push(u32::from(i.particle_up_towards_emitter));
            c3(&mut v, &p.accel);
            c3(&mut v, &p.last_pos);
            v.push(p.lifetime_left);
            v.push(p.create_timestamp);
            f(&mut v, p.alpha);
            f(&mut v, p.alpha_rate);
            v.push(p.alpha_target_key as u32);
            for x in [p.color.red, p.color.green, p.color.blue, p.color_rate.red, p.color_rate.green, p.color_rate.blue] {
                f(&mut v, x);
            }
            v.push(p.color_target_key as u32);
            v.push(u32::from(p.is_culled));
            v.push(p.system_under_control.unwrap_or(0));
        }
    }
    for pr in priority::PARTICLE_PRIORITY_LOWEST..priority::NUM_PARTICLE_PRIORITIES {
        let mut list: Vec<(u64, u32, u32)> = Vec::new();
        for sy in &m.systems {
            for p in &sy.particles {
                if p.overall_priority == pr {
                    list.push((p.overall_seq, sy.id, p.personality));
                }
            }
        }
        list.sort();
        v.push(list.len() as u32);
        for (_, id, pers) in list {
            v.extend([id, pers]);
        }
    }
    v
}

fn cpp_dump() -> Vec<u32> {
    cpp_words(|p, n| unsafe { cpp::ref_psys_dump(p, n) })
}

// ---------------------------------------------------------------------------
// INI loading on both sides
// ---------------------------------------------------------------------------

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
}

/// The Rust outcome, or `None` where the Rust reports the input undefined upstream.
fn rust_outcome(r: IniResult<()>) -> Option<Outcome> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return None,
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
            Some(Thrown::Other) => (5, Vec::new()),
        },
    };
    Some(Outcome { status, detail })
}

/// `INI::load` of `data` on the C++ side, into `TheParticleSystemManager`.
fn cpp_load(name: &str, data: &[u8]) -> Outcome {
    let cname = cstr(name);
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const c_char, data.len() as c_int);
        cpp::ref_ini_load(cname.as_ptr(), 0, log.as_mut_ptr(), log.len() as c_int, &mut log_len, &mut crc, msg.as_mut_ptr() as *mut c_char, msg.len() as c_int, &mut code)
    };
    assert_eq!(log_len, 0, "the real block handler logs nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail }
}

// ---------------------------------------------------------------------------
// The INI generator
// ---------------------------------------------------------------------------

const RV_FIELDS: &[&str] = &[
    "AngleZ",
    "AngularRateZ",
    "AngularDamping",
    "VelocityDamping",
    "Lifetime",
    "Size",
    "StartSizeRate",
    "SizeRate",
    "SizeRateDamping",
    "ColorScale",
    "BurstDelay",
    "BurstCount",
    "InitialDelay",
    "VelOrthoX",
    "VelOrthoY",
    "VelOrthoZ",
    "VelSpherical",
    "VelHemispherical",
    "VelCylindricalRadial",
    "VelCylindricalNormal",
    "VelOutward",
    "VelOutwardOther",
];
const REAL_FIELDS: &[&str] = &[
    "Gravity",
    "VolSphereRadius",
    "VolCylinderRadius",
    "VolCylinderLength",
    "WindAngleChangeMin",
    "WindAngleChangeMax",
    "WindPingPongStartAngleMin",
    "WindPingPongStartAngleMax",
    "WindPingPongEndAngleMin",
    "WindPingPongEndAngleMax",
];
const COORD_FIELDS: &[&str] = &["SlavePosOffset", "DriftVelocity", "VolLineStart", "VolLineEnd", "VolBoxHalfSize"];
const BOOL_FIELDS: &[&str] = &["IsOneShot", "IsHollow", "IsGroundAligned", "IsEmitAboveGroundOnly", "IsParticleUpTowardsEmitter"];
const STRING_FIELDS: &[&str] = &["ParticleName", "SlaveSystem", "PerParticleAttachedSystem"];

fn num(g: &mut Gen, lo: f32, hi: f32) -> String {
    match g.below(10) {
        0 => format!("{}", (g.i32() % 10)),
        1 => format!("{:.2}", g.uniform(lo, hi)),
        _ => format!("{}", g.uniform(lo, hi)),
    }
}

/// A value for a random variable: `lo hi [distribution]`.
fn rv_text(g: &mut Gen, lo: f32, hi: f32) -> String {
    let a = num(g, lo, hi);
    let b = num(g, lo, hi);
    match g.below(8) {
        0 => format!("{a} {b} {}", DISTRIBUTION_TYPE_NAMES[g.below(6) as usize]),
        1 => format!("{a} {a}"),
        _ => format!("{a} {b}"),
    }
}

fn names_value(g: &mut Gen, names: &[&str]) -> String {
    names[g.below(names.len() as u32) as usize].to_string()
}

/// A damaged value: a word, nothing, a colour out of range, an unknown name.
fn bad(g: &mut Gen) -> String {
    match g.below(6) {
        0 => "abc".into(),
        1 => String::new(),
        2 => "R:300 G:0 B:0 5".into(),
        3 => "BOGUS".into(),
        4 => "1.5 x".into(),
        _ => "R:1 G:2".into(),
    }
}

/// One field line of a random kind (for the INI comparison).
fn any_field(g: &mut Gen, names: &[String]) -> String {
    let damaged = g.below(60) == 0;
    let (key, val) = match g.below(12) {
        0 | 1 => (RV_FIELDS[g.below(RV_FIELDS.len() as u32) as usize].to_string(), rv_text(g, -20.0, 60.0)),
        2 => (REAL_FIELDS[g.below(REAL_FIELDS.len() as u32) as usize].to_string(), num(g, -10.0, 10.0)),
        3 => {
            let c = COORD_FIELDS[g.below(COORD_FIELDS.len() as u32) as usize].to_string();
            (c, format!("X:{} Y:{} Z:{}", num(g, -9.0, 9.0), num(g, -9.0, 9.0), num(g, -9.0, 9.0)))
        }
        4 => (BOOL_FIELDS[g.below(BOOL_FIELDS.len() as u32) as usize].to_string(), ["Yes", "No", "yes", "NO"][g.below(4) as usize].to_string()),
        5 => {
            let k = STRING_FIELDS[g.below(3) as usize].to_string();
            let v = if g.below(4) == 0 { String::new() } else if g.below(2) == 0 && !names.is_empty() { names[g.below(names.len() as u32) as usize].clone() } else { format!("tex{}.tga", g.below(9)) };
            (k, v)
        }
        6 => (format!("Alpha{}", 1 + g.below(8)), format!("{} {} {}", num(g, 0.0, 1.0), num(g, 0.0, 1.0), g.below(60))),
        7 => (format!("Color{}", 1 + g.below(8)), format!("R:{} G:{} B:{} {}", g.below(256), g.below(256), g.below(256), g.below(60))),
        8 => {
            let (k, list): (&str, &[&str]) = match g.below(6) {
                0 => ("Priority", PARTICLE_PRIORITY_NAMES),
                1 => ("Shader", PARTICLE_SHADER_TYPE_NAMES),
                2 => ("Type", PARTICLE_TYPE_NAMES),
                3 => ("VelocityType", EMISSION_VELOCITY_TYPE_NAMES),
                4 => ("VolumeType", EMISSION_VOLUME_TYPE_NAMES),
                _ => ("WindMotion", WIND_MOTION_NAMES),
            };
            (k.to_string(), names_value(g, list))
        }
        9 => ("SystemLifetime".into(), format!("{}", g.below(50))),
        10 if g.below(10) == 0 => ("UnknownField".into(), "1".into()),
        10 => ("BurstCount".into(), rv_text(g, 0.0, 5.0)),
        _ => ("Lifetime".into(), rv_text(g, 0.0, 40.0)),
    };
    let val = if damaged { bad(g) } else { val };
    format!("  {key} = {val}\n")
}

fn gen_ini_file(g: &mut Gen) -> String {
    let names: Vec<String> = (0..1 + g.below(4)).map(|i| format!("Sys{i}")).collect();
    let mut s = String::new();
    for _ in 0..1 + g.below(6) {
        let name = names[g.below(names.len() as u32) as usize].clone();
        s.push_str(&format!("ParticleSystem {name}\n"));
        for _ in 0..g.below(30) {
            s.push_str(&any_field(g, &names));
        }
        if g.below(40) != 0 {
            s.push_str("End\n");
        }
        s.push('\n');
    }
    s
}

// ---------------------------------------------------------------------------
// The template generator for the runs (valid, with a share of undefined values)
// ---------------------------------------------------------------------------

fn gen_template(g: &mut Gen, idx: usize, count: usize, out: &mut String) {
    let mut f = |k: &str, v: String| out.push_str(&format!("  {k} = {v}\n"));
    f("Priority", names_value(g, &PARTICLE_PRIORITY_NAMES[1..]));
    f("IsOneShot", if g.below(4) == 0 { "Yes".into() } else { "No".into() });
    let shader = if g.below(30) == 0 { "NONE" } else { ["ADDITIVE", "ALPHA", "ALPHA_TEST", "MULTIPLY"][g.below(4) as usize] };
    f("Shader", shader.into());
    f("Type", names_value(g, &PARTICLE_TYPE_NAMES[1..]));
    f("ParticleName", format!("tex{}.tga", g.below(5)));
    f("AngleZ", rv_text(g, -3.0, 3.0));
    f("AngularRateZ", rv_text(g, -0.3, 0.3));
    f("AngularDamping", rv_text(g, 0.8, 1.0));
    f("VelocityDamping", rv_text(g, 0.8, 1.0));
    if g.below(2) == 0 {
        f("Gravity", num(g, -0.2, 0.1));
    }
    // undefined upstream sometimes: a negative lifetime or delay
    let ub = g.below(80) == 0;
    f("Lifetime", if ub { "-5 3".into() } else { format!("{} {}", 1 + g.below(20), 1 + g.below(40)) });
    f("SystemLifetime", format!("{}", if g.below(3) == 0 { 0 } else { 1 + g.below(40) }));
    f("Size", rv_text(g, 0.5, 8.0));
    if g.below(3) == 0 {
        f("StartSizeRate", rv_text(g, -1.0, 2.0));
    }
    f("SizeRate", rv_text(g, -0.5, 0.5));
    f("SizeRateDamping", rv_text(g, 0.8, 1.0));
    // keyframes: in frame order, sometimes all eight (a key past the eighth: undefined upstream)
    let all_alpha = g.below(25) == 0;
    let mut frame = 0;
    let na = if all_alpha { 8 } else { 1 + g.below(5) };
    for k in 0..na {
        let fr = if k == 0 { 0 } else { frame + 1 + g.below(10) };
        frame = fr;
        f(&format!("Alpha{}", k + 1), format!("{} {} {}", num(g, -0.2, 1.2), num(g, -0.2, 1.2), fr));
    }
    let all_color = g.below(25) == 0;
    let mut frame = 0;
    let nc = if all_color { 8 } else { 1 + g.below(5) };
    for k in 0..nc {
        let fr = if k == 0 { 0 } else if g.below(12) == 0 { frame } else { frame + 1 + g.below(10) };
        frame = fr;
        f(&format!("Color{}", k + 1), format!("R:{} G:{} B:{} {}", g.below(256), g.below(256), g.below(256), fr));
    }
    f("ColorScale", rv_text(g, -20.0, 20.0));
    f("BurstDelay", if g.below(80) == 0 { "-3 -1".into() } else { format!("{} {}", g.below(4), g.below(6)) });
    f("BurstCount", format!("{} {}", g.below(4), 1 + g.below(5)));
    f("InitialDelay", if g.below(80) == 0 { "-2 -1".into() } else if g.below(3) == 0 { format!("{} {}", g.below(5), g.below(8)) } else { "0 0".into() });
    f("DriftVelocity", format!("X:{} Y:{} Z:{}", num(g, -0.3, 0.3), num(g, -0.3, 0.3), num(g, -0.3, 0.6)));
    let vt = g.below(EMISSION_VELOCITY_TYPE_NAMES.len() as u32) as usize;
    f("VelocityType", EMISSION_VELOCITY_TYPE_NAMES[vt].into());
    for k in ["VelOrthoX", "VelOrthoY", "VelOrthoZ", "VelSpherical", "VelHemispherical", "VelCylindricalRadial", "VelCylindricalNormal", "VelOutward", "VelOutwardOther"] {
        if g.below(3) != 0 {
            f(k, rv_text(g, -3.0, 3.0));
        }
    }
    let vol = g.below(EMISSION_VOLUME_TYPE_NAMES.len() as u32) as usize;
    f("VolumeType", EMISSION_VOLUME_TYPE_NAMES[vol].into());
    match vol as i32 {
        volume_type::LINE => {
            f("VolLineStart", format!("X:{} Y:{} Z:{}", num(g, -9.0, 9.0), num(g, -9.0, 9.0), num(g, -2.0, 2.0)));
            f("VolLineEnd", format!("X:{} Y:{} Z:{}", num(g, -9.0, 9.0), num(g, -9.0, 9.0), num(g, -2.0, 2.0)));
        }
        volume_type::BOX => {
            f("VolBoxHalfSize", format!("X:{} Y:{} Z:{}", num(g, 0.0, 9.0), num(g, 0.0, 9.0), num(g, 0.0, 9.0)));
            if g.below(3) == 0 {
                f("VolLineEnd", format!("X:{} Y:{} Z:{}", num(g, -9.0, 9.0), num(g, -9.0, 9.0), num(g, -2.0, 2.0)));
            }
        }
        volume_type::SPHERE => f("VolSphereRadius", num(g, 0.0, 9.0)),
        volume_type::CYLINDER => {
            f("VolCylinderRadius", num(g, 0.0, 9.0));
            f("VolCylinderLength", num(g, 0.0, 9.0));
        }
        _ => {}
    }
    f("IsHollow", if g.below(2) == 0 { "Yes".into() } else { "No".into() });
    f("IsGroundAligned", if g.below(3) == 0 { "Yes".into() } else { "No".into() });
    f("IsEmitAboveGroundOnly", if g.below(4) == 0 { "Yes".into() } else { "No".into() });
    f("IsParticleUpTowardsEmitter", if g.below(4) == 0 { "Yes".into() } else { "No".into() });
    f("WindMotion", ["Unused", "PingPong", "Circular", "Unused"][g.below(4) as usize].into());
    if g.below(2) == 0 {
        f("WindAngleChangeMin", num(g, 0.0, 0.3));
        f("WindAngleChangeMax", num(g, 0.2, 0.6));
        f("WindPingPongStartAngleMin", num(g, 0.0, 0.5));
        f("WindPingPongStartAngleMax", num(g, 0.4, 1.0));
        f("WindPingPongEndAngleMin", num(g, 4.0, 5.5));
        f("WindPingPongEndAngleMax", num(g, 5.0, 6.3));
    }
    // slaves and attached systems only name later templates (no cycle)
    if idx + 1 < count && g.below(3) == 0 {
        f("SlaveSystem", format!("T{}", idx + 1 + g.below((count - idx - 1) as u32) as usize));
        f("SlavePosOffset", format!("X:{} Y:{} Z:{}", num(g, -3.0, 3.0), num(g, -3.0, 3.0), num(g, -3.0, 3.0)));
    }
    if idx + 1 < count && g.below(4) == 0 {
        f("PerParticleAttachedSystem", format!("T{}", idx + 1 + g.below((count - idx - 1) as u32) as usize));
    }
}

fn gen_templates(g: &mut Gen, count: usize) -> String {
    let mut s = String::new();
    for i in 0..count {
        s.push_str(&format!("ParticleSystem T{i}\n"));
        gen_template(g, i, count, &mut s);
        s.push_str("End\n\n");
    }
    s
}

// ---------------------------------------------------------------------------
// The scene, on both sides
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Obj {
    pos: [f32; 3],
    t: [f32; 12],
    shroud: i32,
    has_drawable: bool,
    dt: [f32; 12],
    radius: f32,
}
#[derive(Clone, Copy)]
struct Draw {
    pos: [f32; 3],
    t: [f32; 12],
    obscured: bool,
}

#[derive(Default)]
struct Host {
    objects: BTreeMap<u32, Obj>,
    drawables: BTreeMap<u32, Draw>,
    ground: f32,
}

fn m12(t: &[f32; 12]) -> Matrix3D {
    Matrix3D::new(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11])
}
fn co(p: &[f32; 3]) -> Coord3D {
    Coord3D { x: p[0], y: p[1], z: p[2] }
}

impl ParticleHost for Host {
    fn drawable(&self, id: ObjectId) -> Option<Attachment> {
        self.drawables.get(&(id as u32)).map(|d| Attachment { transform: m12(&d.t), position: co(&d.pos), shrouded: d.obscured })
    }
    fn object(&self, id: ObjectId, _: i32) -> Option<Attachment> {
        // OBJECTSHROUD_FOGGED is 3
        self.objects.get(&(id as u32)).map(|o| Attachment { transform: m12(if o.has_drawable { &o.dt } else { &o.t }), position: co(&o.pos), shrouded: o.shroud >= 3 })
    }
    fn ground_height(&self, _: f32, _: f32) -> f32 {
        self.ground
    }
}

impl FxHost for Host {
    /// The stand-in partition manager's rule: clear where x >= -40.
    fn cell_clear_for_local_player(&self, pos: &Coord3D) -> bool {
        pos.x >= -40.0
    }
    /// `getShroudedStatus > OBJECTSHROUD_PARTIAL_CLEAR` (2).
    fn object_fogged_for_local_player(&self, obj: ObjectId) -> bool {
        self.objects.get(&(obj as u32)).is_some_and(|o| o.shroud > 2)
    }
    fn object_position_and_transform(&self, obj: ObjectId) -> Option<(Coord3D, Matrix3D)> {
        self.objects.get(&(obj as u32)).map(|o| (co(&o.pos), m12(&o.t)))
    }
    fn object_bounding_circle_radius(&self, obj: ObjectId) -> f32 {
        self.objects[&(obj as u32)].radius
    }
    fn layer_height_for_destination(&self, _: &Coord3D) -> f32 {
        self.ground
    }
    /// The stand-in drawables have no bones.
    fn bone_positions(&self, _: ObjectId, _: &[u8], _: i32) -> Vec<(Coord3D, Matrix3D)> {
        Vec::new()
    }
}

/// A transform: a turn about z (and sometimes x), a translation, sometimes a scale.
fn gen_matrix(g: &mut Gen) -> [f32; 12] {
    let a = g.uniform(-3.2, 3.2);
    let (s, c) = (a.sin(), a.cos());
    let k = if g.below(5) == 0 { g.uniform(0.5, 2.0) } else { 1.0 };
    let b = if g.below(4) == 0 { g.uniform(-1.0, 1.0) } else { 0.0 };
    let (sb, cb) = (b.sin(), b.cos());
    [c * k, -s * cb, s * sb, g.uniform(-50.0, 50.0), s, c * cb * k, -c * sb, g.uniform(-50.0, 50.0), 0.0, sb, cb * k, g.uniform(-5.0, 20.0)]
}

fn set_object(h: &mut Host, g: &mut Gen, id: u32) {
    let o = Obj {
        pos: [g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(-2.0, 10.0)],
        t: gen_matrix(g),
        shroud: g.below(6) as i32,
        has_drawable: g.below(2) == 0,
        dt: gen_matrix(g),
        radius: g.uniform(0.0, 30.0),
    };
    h.objects.insert(id, o);
    let mut v = Vec::with_capacity(27);
    v.extend_from_slice(&o.pos);
    v.extend_from_slice(&o.t);
    v.extend_from_slice(&o.dt);
    v.push(o.radius);
    let iv = [o.shroud, i32::from(o.has_drawable)];
    unsafe { cpp::ref_psys_scene(0, id, v.as_ptr(), iv.as_ptr()) };
}

fn set_drawable(h: &mut Host, g: &mut Gen, id: u32) {
    let d = Draw { pos: [g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(-2.0, 10.0)], t: gen_matrix(g), obscured: g.below(5) == 0 };
    h.drawables.insert(id, d);
    let mut v = Vec::with_capacity(15);
    v.extend_from_slice(&d.pos);
    v.extend_from_slice(&d.t);
    let iv = [i32::from(d.obscured)];
    unsafe { cpp::ref_psys_scene(1, id, v.as_ptr(), iv.as_ptr()) };
}

/// One system operation (ref_psys_op's codes) on both sides.
fn op(m: &mut ParticleSystemManager, h: &Host, g: &mut Gen, id: u32) {
    let code = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17][g.below(18) as usize];
    let mut v = [0.0f32; 12];
    let mut iv = 0i32;
    match code {
        0 | 9 => v[..3].copy_from_slice(&[g.uniform(-30.0, 30.0), g.uniform(-30.0, 30.0), g.uniform(-3.0, 10.0)]),
        1 => v = gen_matrix(g),
        2..=4 => v[0] = g.uniform(-3.2, 3.2),
        5 => iv = [0, 1, 2, 3][g.below(4) as usize],
        6 | 12 => iv = [0, 1, 2, 3][g.below(4) as usize],
        10 => v[..3].copy_from_slice(&[[1.0, 0.5, 2.0][g.below(3) as usize], [1.0, 0.5, 2.0][g.below(3) as usize], g.uniform(0.5, 2.0)]),
        13 => iv = g.below(5) as i32,
        14 | 15 => v[0] = g.uniform(0.0, 12.0),
        16 => iv = g.u32() as i32 & 0xffffff,
        17 => iv = g.below(2) as i32,
        _ => {}
    }
    let target = if code == 12 { iv as u32 } else { id };
    let r = unsafe { cpp::ref_psys_op(code, target, v.as_ptr(), iv) };
    if code == 11 {
        m.destroy_particle_system_by_id(id);
        return;
    }
    if code == 12 {
        if h.objects.contains_key(&(iv as u32)) {
            m.destroy_attached_systems(iv);
        }
        return;
    }
    let Some(sy) = m.find_particle_system_mut(id) else {
        assert_eq!(r, 1, "the C++ has system {id} and the Rust has not");
        return;
    };
    assert_eq!(r, 0, "the Rust has system {id} and the C++ has not");
    match code {
        0 => sy.set_position(&Coord3D { x: v[0], y: v[1], z: v[2] }),
        1 => sy.set_local_transform(&m12(&v)),
        2 => sy.rotate_local_transform_x(v[0]),
        3 => sy.rotate_local_transform_y(v[0]),
        4 => sy.rotate_local_transform_z(v[0]),
        5 => sy.attach_to_drawable(if h.drawables.contains_key(&(iv as u32)) { iv } else { 0 }),
        6 => sy.attach_to_object(if h.objects.contains_key(&(iv as u32)) { iv } else { 0 }),
        7 => sy.start(),
        8 => sy.stop(),
        9 => sy.vel_coeff = Coord3D { x: v[0], y: v[1], z: v[2] },
        10 => {
            sy.count_coeff = v[0];
            sy.delay_coeff = v[1];
            sy.size_coeff = v[2];
        }
        13 => sy.set_initial_delay(iv as u32),
        14 => sy.set_emission_volume_sphere_radius(v[0]),
        15 => sy.set_emission_volume_cylinder_radius(v[0]),
        16 => sy.info.tint_all_colors(iv),
        17 => sy.skip_parent_xfrm = iv != 0,
        _ => unreachable!(),
    }
}

fn settings_both(g: &mut Gen, m: &mut ParticleSystemManager) -> ParticleSettings {
    let st = ParticleSettings {
        use_fx: g.below(30) != 0,
        particle_scale: [1.0, 1.0, 0.5, 1.7][g.below(4) as usize],
        max_particle_count: [0, 4, 12, 40, 400, 400][g.below(6) as usize],
        max_field_particle_count: [0, 2, 30][g.below(3) as usize],
        min_dynamic_particle_priority: if g.below(4) == 0 { 1 + g.below(13) as i32 } else { 1 },
        min_dynamic_particle_skip_priority: if g.below(4) == 0 { 1 + g.below(13) as i32 } else { 1 },
        particle_skipped: g.below(5) == 0,
    };
    m.field_particle_count = g.below(40);
    unsafe {
        cpp::ref_psys_settings(
            i32::from(st.use_fx),
            st.particle_scale,
            st.max_particle_count,
            st.max_field_particle_count,
            st.min_dynamic_particle_priority,
            st.min_dynamic_particle_skip_priority,
            i32::from(st.particle_skipped),
            m.field_particle_count,
        )
    };
    st
}

fn cpp_client_state() -> [u32; 6] {
    let mut s = [0u32; 6];
    unsafe { cpp::ref_get_state(1, s.as_mut_ptr()) };
    s
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// Random `ParticleSystem` INI files through both readers. See the module docs.
#[test]
fn particle_system_ini_matches_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("PSYS_INI_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(5000);
    let mut g = Gen::new(0x4600_0001);
    let mut outcomes = Words::new("ParticleSystem INI outcome");
    let mut dumps = Words::new("ParticleSystem INI templates");
    let mut statuses: BTreeMap<i32, u64> = BTreeMap::new();
    let (mut undefined, mut templates) = (0u64, 0u64);
    for case in 0..cases {
        unsafe { cpp::ref_psys_reset() };
        let mut m = ParticleSystemManager::new();
        for round in 0..1 + g.below(2) {
            let text = gen_ini_file(&mut g);
            let Some(rust) = rust_outcome(m.load_text(b"p.ini", text.as_bytes())) else {
                undefined += 1;
                break;
            };
            let c = cpp_load("p.ini", text.as_bytes());
            *statuses.entry(rust.status).or_default() += 1;
            let ctx = || format!("case {case} round {round}, text:\n{text}");
            let mut a = vec![rust.status as u32];
            a.extend(rust.detail.iter().map(|&b| u32::from(b)));
            let mut b = vec![c.status as u32];
            b.extend(c.detail.iter().map(|&b| u32::from(b)));
            outcomes.cmp(&a, &b, ctx);
            dumps.cmp(&rust_template_dump(&m), &cpp_template_dump(), ctx);
        }
        templates += m.templates.len() as u64;
    }
    println!("COVERAGE INI: {cases} cases, outcomes by status {statuses:?}, {templates} templates compared at the end, {undefined} undefined (not compared)");
    outcomes.finish();
    dumps.finish();
    assert!(statuses.get(&0).copied().unwrap_or(0) > u64::from(cases) / 4, "enough successful loads");
    assert!(statuses.len() >= 2, "failures too");
}

/// Random systems run frame by frame on both sides. See the module docs.
#[test]
fn particle_systems_run_as_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("PSYS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(5000);
    let mut g = Gen::new(0x4600_0002);
    let mut frames_ck = Words::new("ParticleSystem run (state per frame)");
    let mut rng_ck = Words::new("ParticleSystem run (client random)");
    let mut tpl_ck = Words::new("ParticleSystem run (templates)");
    let (mut undefined, mut frames, mut particles, mut max_particles, mut systems, mut capped) = (0u64, 0u64, 0u64, 0u32, 0u64, 0u64);
    let mut kinds: BTreeMap<&'static str, u64> = BTreeMap::new();
    // system-frames: [with a slave, controlled by a particle, wind ping-pong, wind circular, hollow box, attached to an
    // object, attached to a drawable, not at the identity, destroyed]
    let mut seen = [0u64; 9];
    let mut velocity_types = [0u64; 6];
    let mut volume_types = [0u64; 6];
    for case in 0..cases {
        unsafe { cpp::ref_psys_reset() };
        let mut m = ParticleSystemManager::new();
        let mut h = Host::default();
        let n = 1 + g.below(5) as usize;
        let text = gen_templates(&mut g, n);
        let r = rust_outcome(m.load_text(b"t.ini", text.as_bytes())).expect("defined");
        assert_eq!(r.status, 0, "case {case}: the templates load\n{text}");
        assert_eq!(cpp_load("t.ini", text.as_bytes()).status, 0);
        tpl_ck.cmp(&rust_template_dump(&m), &cpp_template_dump(), || format!("case {case}\n{text}"));
        for t in m.templates.values() {
            velocity_types[t.info.emission_velocity_type as usize] += 1;
            volume_types[t.info.emission_volume_type as usize] += 1;
        }
        let mut st = settings_both(&mut g, &mut m);
        let mut state = [0u32; 6];
        seed_random(g.u32(), &mut state);
        let mut random = RandomValues::default();
        random.client = state;
        unsafe { cpp::ref_set_state(1, state.as_ptr()) };
        for id in 1..=3 {
            if g.below(3) != 0 {
                set_object(&mut h, &mut g, id);
            }
            if g.below(3) != 0 {
                set_drawable(&mut h, &mut g, id);
            }
        }
        if g.below(3) == 0 {
            h.ground = g.uniform(-3.0, 5.0);
            unsafe { cpp::ref_psys_scene(4, 0, [h.ground].as_ptr(), [0].as_ptr()) };
        }
        let create = |m: &mut ParticleSystemManager, h: &Host, g: &mut Gen, random: &mut RandomValues, frame: u32| -> u32 {
            let name = if g.below(20) == 0 { "Missing".to_string() } else { format!("T{}", g.below(n as u32)) };
            let slaves = g.below(5) != 0;
            let attach = if g.below(3) == 0 { 1 + g.below(3) } else { 0 };
            let cname = cstr(&name);
            let c = unsafe { cpp::ref_psys_create(cname.as_ptr(), i32::from(slaves), frame, attach) };
            let r = if attach != 0 {
                // createAttachedParticleSystemID: attached when the object exists
                let id = m.create_attached_particle_system_id(name.as_bytes(), if h.objects.contains_key(&attach) { attach as ObjectId } else { 0 }, slaves, random, frame);
                id
            } else {
                m.create_particle_system(name.as_bytes(), slaves, random, frame).unwrap_or(INVALID_PARTICLE_SYSTEM_ID)
            };
            assert_eq!(r, c, "case {}: created ids", case);
            r
        };
        let mut ids = Vec::new();
        for _ in 0..1 + g.below(4) {
            let id = create(&mut m, &h, &mut g, &mut random, 0);
            if id != 0 {
                ids.push(id);
            }
        }
        for _ in 0..g.below(4) {
            if let Some(&id) = ids.get(g.below(ids.len().max(1) as u32) as usize) {
                op(&mut m, &h, &mut g, id);
            }
        }
        let ctx0 = |f: u32| format!("case {case} frame {f}, templates:\n{text}");
        if let Some(u) = m.undefined {
            *kinds.entry(u).or_default() += 1;
            undefined += 1;
            continue;
        }
        if !frames_ck.cmp(&rust_dump(&m), &cpp_dump(), || ctx0(0)) || !rng_ck.cmp(&random.client, &cpp_client_state(), || ctx0(0)) {
            continue;
        }
        let (mut logic, mut client) = (0u32, 0u32);
        for frame in 1..=30 + g.below(61) {
            // between frames: the world and the systems change
            if g.below(6) == 0 {
                let id = 1 + g.below(3);
                match g.below(5) {
                    0 => set_object(&mut h, &mut g, id),
                    1 => set_drawable(&mut h, &mut g, id),
                    2 => {
                        h.objects.remove(&id);
                        unsafe { cpp::ref_psys_scene(2, id, [0.0].as_ptr(), [0].as_ptr()) };
                    }
                    3 => {
                        h.drawables.remove(&id);
                        unsafe { cpp::ref_psys_scene(3, id, [0.0].as_ptr(), [0].as_ptr()) };
                    }
                    _ => {
                        h.ground = g.uniform(-3.0, 5.0);
                        unsafe { cpp::ref_psys_scene(4, 0, [h.ground].as_ptr(), [0].as_ptr()) };
                    }
                }
            }
            if g.below(5) == 0 && !ids.is_empty() {
                let id = ids[g.below(ids.len() as u32) as usize];
                op(&mut m, &h, &mut g, id);
            }
            if g.below(12) == 0 {
                let id = create(&mut m, &h, &mut g, &mut random, client);
                if id != 0 {
                    ids.push(id);
                }
                // an initial delay out of range: the two sides would run differently from here
                if let Some(u) = m.undefined {
                    *kinds.entry(u).or_default() += 1;
                    undefined += 1;
                    break;
                }
            }
            if g.below(40) == 0 {
                st = settings_both(&mut g, &mut m);
            }
            if g.below(15) != 0 {
                logic += 1;
            }
            client += 1 + g.below(2);
            m.update(&h, &mut random, &st, logic, client);
            unsafe { cpp::ref_psys_update(logic, client) };
            if let Some(u) = m.undefined {
                *kinds.entry(u).or_default() += 1;
                undefined += 1;
                break;
            }
            frames += 1;
            let ctx = || ctx0(frame);
            let ok = frames_ck.cmp(&rust_dump(&m), &cpp_dump(), ctx) && rng_ck.cmp(&random.client, &cpp_client_state(), ctx);
            if !ok && std::env::var("PSYS_FULL").is_ok() {
                println!("RNG equal: {}; last_new_vel {:?}", random.client == cpp_client_state(), m.last_new_vel);
                for sy in &m.systems {
                    println!("system {} {:?} slave {:?} master {:?} vt {} vol {} particles {}", sy.id, String::from_utf8_lossy(&sy.template), sy.slave, sy.master, sy.info.emission_velocity_type, sy.info.emission_volume_type, sy.particles.len());
                    for p in &sy.particles {
                        println!("   p{} vel {:?} pos {:?}", p.personality, p.info.vel, p.info.pos);
                    }
                }
            }
            particles += u64::from(m.particle_count);
            max_particles = max_particles.max(m.particle_count);
            systems += m.systems.len() as u64;
            for sy in &m.systems {
                let i = &sy.info;
                for (k, b) in [
                    sy.slave.is_some(),
                    sy.control_particle.is_some(),
                    i.wind_motion == wind_motion::WIND_MOTION_PING_PONG,
                    i.wind_motion == wind_motion::WIND_MOTION_CIRCULAR,
                    i.is_emission_volume_hollow && i.emission_volume_type == volume_type::BOX,
                    sy.attached_to_object != 0,
                    sy.attached_to_drawable != 0,
                    !sy.is_identity,
                    sy.is_destroyed,
                ]
                .into_iter()
                .enumerate()
                {
                    seen[k] += u64::from(b);
                }
            }
            if st.max_particle_count > 0 && m.particle_count >= st.max_particle_count as u32 {
                capped += 1;
            }
            if !ok {
                break;
            }
        }
    }
    println!(
        "COVERAGE runs: {cases} cases, {frames} frames compared, {systems} system-frames, {particles} particle-frames (at most {max_particles} at once), {capped} frames at the cap; velocity types {velocity_types:?}, volume types {volume_types:?}; system-frames with a slave, controlled, ping-pong, circular, hollow box, on an object, on a drawable, transformed, destroyed: {seen:?}; {undefined} cases stopped where upstream is undefined: {kinds:?}"
    );
    assert!(seen.iter().all(|&n| n > 100), "every feature is exercised: {seen:?}");
    tpl_ck.finish();
    frames_ck.finish();
    rng_ck.finish();
}

// ---------------------------------------------------------------------------
// FXList.cpp (#46): the reader, and the nuggets' requests, client random draws and particle systems
// ---------------------------------------------------------------------------

/// wrap_particle_sys.cpp's `ref_fx_dump`: every list in name order, its nuggets.
fn rust_fx_dump(store: &FxListStore) -> Vec<u32> {
    let mut v = vec![store.fxmap.len() as u32];
    for (name, l) in &store.fxmap {
        s(&mut v, name);
        v.push(l.nuggets.len() as u32);
        for n in &l.nuggets {
            match n {
                FxNugget::Sound { sound_name } => {
                    v.push(1);
                    s(&mut v, sound_name);
                }
                FxNugget::RayEffect { template_name, primary_offset, secondary_offset } => {
                    v.push(2);
                    s(&mut v, template_name);
                    c3(&mut v, primary_offset);
                    c3(&mut v, secondary_offset);
                }
                FxNugget::Tracer(t) => {
                    v.push(3);
                    s(&mut v, &t.tracer_name);
                    s(&mut v, &t.bone_name);
                    for x in [t.speed, t.decay_at, t.length, t.width, t.color.red, t.color.green, t.color.blue, t.probability] {
                        f(&mut v, x);
                    }
                }
                FxNugget::LightPulse(l) => {
                    v.push(4);
                    for x in [l.color.red, l.color.green, l.color.blue, l.radius, l.bounding_circle_pct] {
                        f(&mut v, x);
                    }
                    v.extend([l.increase_frames, l.decrease_frames]);
                }
                FxNugget::ViewShake { shake } => v.extend([5, *shake as u32]),
                FxNugget::TerrainScorch { scorch, radius } => {
                    v.extend([6, *scorch as u32]);
                    f(&mut v, *radius);
                }
                FxNugget::ParticleSystem(p) => {
                    v.push(7);
                    s(&mut v, &p.name);
                    v.push(p.count as u32);
                    c3(&mut v, &p.offset);
                    rv(&mut v, &p.radius);
                    rv(&mut v, &p.height);
                    rv(&mut v, &p.delay);
                    for x in [p.rotate_x, p.rotate_y, p.rotate_z] {
                        f(&mut v, x);
                    }
                    for b in [p.orient_to_object, p.attach_to_object, p.create_at_ground_height, p.use_callers_radius, p.ricochet] {
                        v.push(u32::from(b));
                    }
                }
                FxNugget::FxListAtBonePos { fx, bone_name, orient_to_bone } => {
                    // the name (the C++ reads it back from the Armor build's stand-in store: see wrap_particle_sys.cpp)
                    v.push(8);
                    s(&mut v, fx.as_deref().unwrap_or(b""));
                    s(&mut v, bone_name);
                    v.push(u32::from(*orient_to_bone));
                }
            }
        }
    }
    v
}

fn cpp_fx_dump() -> Vec<u32> {
    cpp_words(|p, n| unsafe { cpp::ref_fx_dump(p, n) })
}

/// The requests as wrap_particle_sys.cpp's stand-ins log them.
fn request_words(reqs: &[FxRequest]) -> Vec<u32> {
    let mut v = Vec::new();
    for r in reqs {
        match r {
            FxRequest::Sound { name, position } => {
                v.push(1);
                s(&mut v, name);
                v.push(u32::from(position.is_some()));
                if let Some(p) = position {
                    c3(&mut v, p);
                }
            }
            FxRequest::LightPulse { position, color, radius, increase_frames, decrease_frames } => {
                v.push(2);
                c3(&mut v, position);
                for x in [color.red, color.green, color.blue, *radius] {
                    f(&mut v, x);
                }
                v.extend([*increase_frames, *decrease_frames]);
            }
            FxRequest::ViewShake { position, shake } => {
                v.push(3);
                c3(&mut v, position);
                v.push(*shake as u32);
            }
            FxRequest::TerrainScorch { position, radius, scorch } => {
                v.push(4);
                c3(&mut v, position);
                f(&mut v, *radius);
                v.push(*scorch as u32);
            }
            // (the C++ tracer does not know the secondary position: it is in the tracer's transform, not compared)
            FxRequest::Tracer { template, primary, speed, length, width, color, frames, .. } => {
                v.push(5);
                s(&mut v, template);
                c3(&mut v, primary);
                for x in [*speed, *length, *width, color.red, color.green, color.blue] {
                    f(&mut v, x);
                }
                v.push(*frames as u32);
            }
            FxRequest::RayEffect { template, source, target } => {
                v.push(6);
                s(&mut v, template);
                c3(&mut v, source);
                c3(&mut v, target);
            }
        }
    }
    v
}

fn cpp_fx_log() -> Vec<u32> {
    cpp_words(|p, n| unsafe { cpp::ref_fx_log(p, n) })
}

/// `FXList.ini` text: blocks `FXList FX<i>` of random nuggets. `FXListAtBonePos` names a list already made (or the
/// one being read), `None` or an unknown name (the C++ keeps the name too, through the Armor build's stand-in store:
/// see wrap_particle_sys.cpp).
fn gen_fx_file(g: &mut Gen, templates: usize, damaged: bool) -> String {
    let mut s = String::new();
    let mut made: Vec<String> = Vec::new();
    for _ in 0..1 + g.below(5) {
        let name = format!("FX{}", g.below(4));
        if !made.contains(&name) {
            made.push(name.clone());
        }
        s.push_str(&format!("FXList {name}\n"));
        for _ in 0..g.below(7) {
            let bad = damaged && g.below(30) == 0;
            // a damaged nugget has its values replaced by a word
            let fld = |s: &mut String, k: &str, v: String| s.push_str(&format!("    {k} = {}\n", if bad { "x".to_string() } else { v }));
            match g.below(8) {
                0 => {
                    s.push_str("  Sound\n");
                    fld(&mut s, "Name", format!("Snd{}", g.below(3)));
                }
                1 => {
                    s.push_str("  RayEffect\n");
                    fld(&mut s, "Name", format!("Ray{}", g.below(3)));
                    fld(&mut s, "PrimaryOffset", format!("X:{} Y:{} Z:{}", num(g, -5.0, 5.0), num(g, -5.0, 5.0), num(g, -5.0, 5.0)));
                    fld(&mut s, "SecondaryOffset", format!("X:{} Y:{} Z:{}", num(g, -5.0, 5.0), num(g, -5.0, 5.0), num(g, -5.0, 5.0)));
                }
                2 => {
                    s.push_str("  Tracer\n");
                    if g.below(2) == 0 {
                        fld(&mut s, "TracerName", format!("Tr{}", g.below(3)));
                    }
                    fld(&mut s, "BoneName", "Muzzle".into());
                    fld(&mut s, "Speed", num(g, -50.0, 600.0));
                    fld(&mut s, "DecayAt", num(g, 0.0, 2.0));
                    fld(&mut s, "Length", num(g, 0.0, 30.0));
                    fld(&mut s, "Width", num(g, 0.0, 3.0));
                    fld(&mut s, "Color", format!("R:{} G:{} B:{}", g.below(256), g.below(256), g.below(256)));
                    fld(&mut s, "Probability", num(g, 0.0, 1.2));
                }
                3 => {
                    s.push_str("  LightPulse\n");
                    fld(&mut s, "Color", format!("R:{} G:{} B:{}", g.below(256), g.below(256), g.below(256)));
                    fld(&mut s, "Radius", num(g, 0.0, 50.0));
                    if g.below(2) == 0 {
                        fld(&mut s, "RadiusAsPercentOfObjectSize", format!("{}%", g.below(300)));
                    }
                    fld(&mut s, "IncreaseTime", format!("{}", g.below(500)));
                    fld(&mut s, "DecreaseTime", format!("{}", g.below(900)));
                }
                4 => {
                    s.push_str("  ViewShake\n");
                    fld(&mut s, "Type", ["SUBTLE", "NORMAL", "STRONG", "SEVERE", "CINE_EXTREME", "CINE_INSANE"][g.below(6) as usize].into());
                }
                5 => {
                    s.push_str("  TerrainScorch\n");
                    fld(&mut s, "Type", ["SCORCH_1", "SCORCH_2", "SCORCH_3", "SCORCH_4", "SHADOW_SCORCH", "RANDOM"][g.below(6) as usize].into());
                    fld(&mut s, "Radius", num(g, 0.0, 30.0));
                }
                6 => {
                    s.push_str("  ParticleSystem\n");
                    let t = if g.below(10) == 0 { "Missing".to_string() } else { format!("T{}", g.below(templates.max(1) as u32)) };
                    fld(&mut s, "Name", t);
                    fld(&mut s, "Count", format!("{}", g.below(4)));
                    if g.below(2) == 0 {
                        fld(&mut s, "Offset", format!("X:{} Y:{} Z:{}", num(g, -5.0, 5.0), num(g, -5.0, 5.0), num(g, -5.0, 5.0)));
                    }
                    if g.below(2) == 0 {
                        fld(&mut s, "Radius", rv_text(g, 0.0, 10.0));
                    }
                    if g.below(2) == 0 {
                        fld(&mut s, "Height", rv_text(g, -2.0, 5.0));
                    }
                    if g.below(2) == 0 {
                        fld(&mut s, "InitialDelay", rv_text(g, -50.0, 300.0));
                    }
                    for k in ["RotateX", "RotateY", "RotateZ"] {
                        if g.below(3) == 0 {
                            fld(&mut s, k, num(g, -180.0, 180.0));
                        }
                    }
                    for k in ["OrientToObject", "Ricochet", "AttachToObject", "CreateAtGroundHeight", "UseCallersRadius"] {
                        if g.below(2) == 0 {
                            fld(&mut s, k, ["Yes", "No"][g.below(2) as usize].into());
                        }
                    }
                }
                _ => {
                    s.push_str("  FXListAtBonePos\n");
                    let fx = match g.below(4) {
                        0 => "None".to_string(),
                        1 => "Unknown".to_string(),
                        _ => made[g.below(made.len() as u32) as usize].clone(),
                    };
                    fld(&mut s, "FX", fx);
                    fld(&mut s, "BoneName", "Bone".into());
                    fld(&mut s, "OrientToBone", ["Yes", "No"][g.below(2) as usize].into());
                }
            }
            if !(bad && g.below(2) == 0) {
                s.push_str("  End\n");
            }
        }
        s.push_str("End\n\n");
    }
    s
}

/// `INI::load` of an FX file on the C++ side (into `TheFXListStore`).
fn cpp_fx_load(data: &[u8]) -> Outcome {
    cpp_load("fx.ini", data)
}

/// Random `FXList` files through both readers: the outcome and every list. See the module docs.
#[test]
fn fx_list_ini_matches_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("PSYS_FX_INI_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(5000);
    let mut g = Gen::new(0x4600_0003);
    let mut outcomes = Words::new("FXList INI outcome");
    let mut dumps = Words::new("FXList INI lists");
    let mut statuses: BTreeMap<i32, u64> = BTreeMap::new();
    let mut nuggets = 0u64;
    for case in 0..cases {
        unsafe {
            cpp::ref_psys_reset();
            cpp::ref_fx_reset();
        }
        let mut store = FxListStore::new();
        for round in 0..1 + g.below(2) {
            let text = gen_fx_file(&mut g, 3, true);
            let rust = rust_outcome(store.load_text(b"fx.ini", text.as_bytes())).expect("the generator writes defined input");
            let c = cpp_fx_load(text.as_bytes());
            *statuses.entry(rust.status).or_default() += 1;
            let ctx = || format!("case {case} round {round}, text:\n{text}");
            let mut a = vec![rust.status as u32];
            a.extend(rust.detail.iter().map(|&b| u32::from(b)));
            let mut b = vec![c.status as u32];
            b.extend(c.detail.iter().map(|&b| u32::from(b)));
            outcomes.cmp(&a, &b, ctx);
            dumps.cmp(&rust_fx_dump(&store), &cpp_fx_dump(), ctx);
        }
        nuggets += store.fxmap.values().map(|l| l.nuggets.len() as u64).sum::<u64>();
    }
    println!("COVERAGE FX INI: {cases} cases, outcomes by status {statuses:?}, {nuggets} nuggets in the stores at the end");
    outcomes.finish();
    dumps.finish();
    assert!(statuses.get(&0).copied().unwrap_or(0) > u64::from(cases) / 2, "enough successful loads");
}

/// FX lists played (`doFXPos`, `doFXObj`) between particle updates: every request, the client random state and the
/// particle systems they make. See the module docs.
#[test]
fn fx_lists_play_as_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("PSYS_FX_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(2000);
    let mut g = Gen::new(0x4600_0004);
    let mut req_ck = Words::new("FXList play (requests)");
    let mut sys_ck = Words::new("FXList play (particle systems)");
    let mut rng_ck = Words::new("FXList play (client random)");
    let mut list_ck = Words::new("FXList play (lists)");
    let (mut plays, mut requests, mut made, mut undefined) = (0u64, 0u64, 0u64, 0u64);
    let mut kinds = [0u64; 7];
    for case in 0..cases {
        unsafe {
            cpp::ref_psys_reset();
            cpp::ref_fx_reset();
        }
        let mut m = ParticleSystemManager::new();
        let mut store = FxListStore::new();
        let mut h = Host::default();
        let n = 1 + g.below(4) as usize;
        let text = gen_templates(&mut g, n);
        assert_eq!(rust_outcome(m.load_text(b"t.ini", text.as_bytes())).expect("defined").status, 0);
        assert_eq!(cpp_load("t.ini", text.as_bytes()).status, 0);
        let fx = gen_fx_file(&mut g, n, false);
        let r = rust_outcome(store.load_text(b"fx.ini", fx.as_bytes())).expect("defined");
        assert_eq!(r.status, 0, "case {case}: the FX lists load\n{fx}");
        assert_eq!(cpp_fx_load(fx.as_bytes()).status, 0);
        list_ck.cmp(&rust_fx_dump(&store), &cpp_fx_dump(), || format!("case {case}\n{fx}"));
        let st = settings_both(&mut g, &mut m);
        let mut state = [0u32; 6];
        seed_random(g.u32(), &mut state);
        let mut random = RandomValues::default();
        random.client = state;
        unsafe { cpp::ref_set_state(1, state.as_ptr()) };
        for id in 1..=3 {
            if g.below(4) != 0 {
                set_object(&mut h, &mut g, id);
            }
        }
        if g.below(2) == 0 {
            h.ground = g.uniform(-3.0, 5.0);
            unsafe { cpp::ref_psys_scene(4, 0, [h.ground].as_ptr(), [0].as_ptr()) };
        }
        let names: Vec<String> = (0..4).map(|i| format!("FX{i}")).chain(["None".to_string(), "Missing".to_string()]).collect();
        let (mut logic, mut client) = (1u32, 1u32);
        for step in 0..10 + g.below(40) {
            let name = names[g.below(names.len() as u32) as usize].clone();
            let cname = cstr(&name);
            let mut reqs = Vec::new();
            let ctx = || format!("case {case} step {step} ({name}), FX lists:\n{fx}\ntemplates:\n{text}");
            match g.below(5) {
                0 | 1 => {
                    let p = [if g.below(6) == 0 { -60.0 } else { g.uniform(-30.0, 30.0) }, g.uniform(-30.0, 30.0), g.uniform(-2.0, 8.0)];
                    let mtx = (g.below(2) == 0).then(|| gen_matrix(&mut g));
                    let speed = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 40.0) };
                    let sec = (g.below(3) != 0).then(|| [g.uniform(-30.0, 30.0), g.uniform(-30.0, 30.0), g.uniform(-2.0, 8.0)]);
                    let radius = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 20.0) };
                    let mm = mtx.map(|t| m12(&t));
                    let mut play = FxPlay { random: &mut random, particles: &mut m, frame: client, requests: &mut reqs };
                    store.do_fx_pos(name.as_bytes(), &h, &mut play, Some(&co(&p)), mm.as_ref(), speed, sec.map(|q| co(&q)).as_ref(), radius);
                    unsafe {
                        cpp::ref_fx_pos(
                            cname.as_ptr(),
                            p.as_ptr(),
                            mtx.as_ref().map_or(std::ptr::null(), |t| t.as_ptr()),
                            speed,
                            sec.as_ref().map_or(std::ptr::null(), |q| q.as_ptr()),
                            radius,
                            logic,
                            client,
                        )
                    };
                }
                2 | 3 => {
                    let (a, b) = (g.below(4), g.below(4));
                    let some = |id: u32| (id != 0 && h.objects.contains_key(&id)).then_some(id as ObjectId);
                    let mut play = FxPlay { random: &mut random, particles: &mut m, frame: client, requests: &mut reqs };
                    store.do_fx_obj(name.as_bytes(), &h, &mut play, some(a), some(b));
                    unsafe { cpp::ref_fx_obj(cname.as_ptr(), a, b, logic, client) };
                }
                _ => {
                    logic += 1;
                    client += 1;
                    m.update(&h, &mut random, &st, logic, client);
                    unsafe { cpp::ref_psys_update(logic, client) };
                }
            }
            plays += 1;
            requests += reqs.len() as u64;
            for r in &reqs {
                kinds[match r {
                    FxRequest::Sound { .. } => 0,
                    FxRequest::LightPulse { .. } => 1,
                    FxRequest::ViewShake { .. } => 2,
                    FxRequest::TerrainScorch { .. } => 3,
                    FxRequest::Tracer { .. } => 4,
                    FxRequest::RayEffect { .. } => 5,
                }] += 1;
            }
            if let Some(u) = m.undefined {
                let _ = u;
                undefined += 1;
                break;
            }
            let ok = req_ck.cmp(&request_words(&reqs), &cpp_fx_log(), ctx) && sys_ck.cmp(&rust_dump(&m), &cpp_dump(), ctx) && rng_ck.cmp(&random.client, &cpp_client_state(), ctx);
            if !ok {
                break;
            }
        }
        made += u64::from(m.unique_system_id);
        kinds[6] += m.systems.iter().filter(|s| s.attached_to_object != 0).count() as u64;
    }
    println!(
        "COVERAGE FX play: {cases} cases, {plays} steps, {requests} requests (sound, light pulse, shake, scorch, tracer, ray; systems attached at the end: {kinds:?}), {made} particle systems made; {undefined} cases stopped where upstream is undefined"
    );
    list_ck.finish();
    req_ck.finish();
    sys_ck.finish();
    rng_ck.finish();
    assert!(kinds.iter().all(|&k| k > 0), "every request kind: {kinds:?}");
}
