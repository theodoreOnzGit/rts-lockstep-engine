// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_emt.cpp, part_emt.h
//   (ParticleEmitterClass: the constructor, Create_From_Definition,
//   Restart, Notify_Added, Notify_Removed, Scale, On_Frame_Update, Reset,
//   Start, Stop, Is_Stopped, Set_Position_Randomizer,
//   Set_Velocity_Randomizer, Set_Base_Velocity, Set_Outwards_Velocity,
//   Set_Velocity_Inheritance_Factor, Emit, Create_New_Particles,
//   Initialize_Particle, Set_Name, Update_On_Visibilty, Set_Hidden,
//   Set_Animation_Hidden, Set_Emission_Rate, Set_Burst_Size,
//   Set_One_Time_Burst, the inline accessors; Copy_Emitter_Property_Struct)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_buf.cpp
//   (ParticleBufferClass::On_Frame_Update)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/gcd_lcm.cpp
//   (Greatest_Common_Divisor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, part_emt by Naty Hoffman and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ParticleEmitterClass` (GitHub #49): the render object a W3D emitter
//! definition ([`super::part_ldr`]) makes, which emits particles into its
//! [`ParticleBuffer`] over W3D sync time.
//!
//! # As upstream does it
//!
//! - **`Create_From_Definition`**: the definition's texture (null for an
//!   empty name) and shader, its keyframes copied
//!   (`Copy_Emitter_Property_Struct`), its randomizers cloned, and the
//!   constructor: the emission period `EmitRate = (unsigned)(1000 / rate)`
//!   ms (1000 for a rate of 0 or less), a burst size of 0 made 1, the
//!   velocities per millisecond (`* 0.001`), the velocity randomizer scaled
//!   by 0.001, a lifetime of 0 or less made 1 s, and a buffer of
//!   `BurstSize * rate * (lifetime + 1)` particles, capped by the maximum
//!   emissions and at least 2. The acceleration is per ms^2 (`/ 1e6`).
//! - **Emission** (`Emit`, called by the buffer's `On_Frame_Update`):
//!   `Create_New_Particles` adds the frame time (taken modulo the time to
//!   fill the buffer when it is over 100 emission periods) to `EmitRemain`
//!   and emits a burst every `EmitRate` ms, each at its own moment: the
//!   transform is slerped (`Cached_Slerp`) and lerped between the last
//!   frame's and this one's, and the time stamp is the sync time minus what
//!   remains. A one-time burst replaces one burst's size; a positive
//!   `MaxParticles` counts down and completes the emitter.
//! - **`Initialize_Particle`**: the creation-volume point rotated into the
//!   world; the velocity randomizer's vector plus the outward velocity
//!   along the point's direction (along x for the origin) plus the base
//!   velocity, rotated, plus the emitter's own velocity times the
//!   inheritance factor.
//! - **Visibility**: a visible emitter in the scene starts
//!   (`Update_On_Visibilty`); a hidden or invisible one stops, and its
//!   particles live on. `Start` counts a new group ID (line mode draws each
//!   group as its own line).
//!
//! # Upstream quirks that are kept
//!
//! - `Cached_Slerp` divides by theta, not its sine (see
//!   [`crate::ww::quat::cached_slerp`]).
//! - The emitter's velocity for inheritance is taken over the whole frame
//!   and the global is only set when the factor is non-zero.
//! - Emission stops at the burst that completes the emitter, even mid-frame.
//! - The future start time is passed to the buffer, which never uses it.
//!
//! # Where this differs from upstream
//!
//! - **The scene**: the caller drives each frame ([`ParticleEmitter::frame`]:
//!   the emitter's `On_Frame_Update`, then the buffer's, which emits, then
//!   the buffer's `Render`), with the WW3D clock ([`Ww3dClock`]) and the two
//!   random generators ([`ParticleRandom`]), process globals upstream.
//! - **Undefined upstream** ([`W3dUndefined`]): a definition with no
//!   creation volume or velocity randomizer (an unknown class: the null
//!   pointer is cloned and scaled, [`W3dUndefined::NullPointer`]); a float
//!   converted to an integer that cannot hold it (the emission period, the
//!   maximum emissions, the buffer size: [`W3dUndefined::FloatToInt`]);
//!   keyframes the 32-bit original cannot copy
//!   ([`W3dUndefined::Allocation`]); an emission period of 0 (a rate over
//!   1000) without a particle count, on which `Create_New_Particles` loops
//!   forever ([`W3dError::NonTerminating`]).

use super::mesh::convert_shader;
use super::part_buf::{f2i, f2u, BufferParams, ParticleBuffer, ParticleDraw, Props, Ww3dClock};
use super::part_ldr::{ParticleEmitterDef, ParticleProperty, Vector3Randomizer};
use super::shader::ShaderClass;
use super::{W3dError, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::{build_quaternion, cached_slerp, slerp_setup, Quaternion};
use crate::ww::random::{Random3, Random4};
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// The two random generators the particle code draws from (process globals
/// upstream): `Vector3Randomizer::Randomizer` (v3_rnd.cpp, a `Random3Class`
/// of seeds 0, 0) and part_buf.cpp's `rand_gen` (a `Random4Class` of seed
/// 4357).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParticleRandom {
    pub v3: Random3,
    pub buf: Random4,
}

/// `Greatest_Common_Divisor(a, b)` (gcd_lcm.cpp, Euclid).
pub fn greatest_common_divisor(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        greatest_common_divisor(b, a % b)
    }
}

/// `Copy_Emitter_Property_Struct(dest, src)` with each value converted.
fn copy_props<T: Copy + PartialEq + Default, U>(src: &ParticleProperty<T>, size: u64, conv: impl Fn(T) -> U) -> W3dResult<Props<U>> {
    let n = src.num_key_frames;
    let mut out = Props { start: conv(src.start), rand: conv(src.rand), key_times: Vec::new(), values: Vec::new() };
    if n > 0 {
        if u64::from(n) * 4 > WIN32_MAX_ALLOCATION || u64::from(n) * size > WIN32_MAX_ALLOCATION {
            return Err(W3dUndefined::Allocation.into());
        }
        // memcpy from the definition's arrays (a null one is read)
        let (Some(t), Some(v)) = (src.key_times.as_ref(), src.values.as_ref()) else {
            return Err(W3dUndefined::NullPointer.into());
        };
        out.key_times.reserve(n as usize);
        out.values.reserve(n as usize);
        for i in 0..n as usize {
            out.key_times.push(t.get(i));
            out.values.push(conv(v.get(i)));
        }
    }
    Ok(out)
}

/// The constructor's arguments (`ParticleEmitterClass(emit_rate, ...)`).
#[derive(Clone, Debug, PartialEq)]
pub struct EmitterArgs {
    pub emit_rate: f32,
    pub burst_size: u32,
    pub pos_rnd: Option<Vector3Randomizer>,
    pub base_vel: Vector3,
    pub vel_rnd: Option<Vector3Randomizer>,
    pub out_vel: f32,
    pub vel_inherit_factor: f32,
    pub color: Props<Vector3>,
    pub opacity: Props<f32>,
    pub size: Props<f32>,
    pub rotation: Props<f32>,
    pub orient_rnd: f32,
    pub frames: Props<f32>,
    pub blur_times: Props<f32>,
    pub accel: Vector3,
    pub max_age: f32,
    pub future_start: f32,
    pub texture: Option<Vec<u8>>,
    pub shader: ShaderClass,
    pub max_particles: i32,
    pub max_buffer_size: i32,
    pub pingpong: bool,
    pub render_mode: i32,
    pub frame_mode: i32,
    pub line_props: Option<super::w3d_file::W3dEmitterLinePropertiesStruct>,
}

/// `class ParticleEmitterClass`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleEmitter {
    /// `EmitRate`: the emission period, ms.
    pub emit_rate: u32,
    pub burst_size: u32,
    pub one_time_burst_size: u32,
    pub one_time_burst: bool,
    pub pos_rand: Option<Vector3Randomizer>,
    /// `BaseVel`, per ms.
    pub base_vel: Vector3,
    pub vel_rand: Option<Vector3Randomizer>,
    /// `OutwardVel`, per ms.
    pub outward_vel: f32,
    pub vel_inherit_factor: f32,
    pub emit_remain: u32,
    pub prev_q: Quaternion,
    pub prev_orig: Vector3,
    pub active: bool,
    pub first_time: bool,
    pub buffer_scene_needed: bool,
    pub particles_left: i32,
    pub max_particles: i32,
    pub is_complete: bool,
    pub name: Option<Vec<u8>>,
    pub remove_on_complete: bool,
    pub is_in_scene: bool,
    pub group_id: u8,
    pub buffer: ParticleBuffer,
    /// The buffer added to the scene (`Buffer->Add(Scene)`).
    pub buffer_in_scene: bool,
    pub is_invisible: bool,
    /// The render object's transform (`Get_Transform()`).
    pub transform: Matrix3D,
    /// `RenderObjClass`'s visibility bits: `Set_Visible`, `Set_Hidden`,
    /// `Set_Animation_Hidden`.
    pub visible: bool,
    pub hidden: bool,
    pub animation_hidden: bool,
}

impl ParticleEmitter {
    /// `ParticleEmitterClass(emit_rate, burst_size, ...)` at the clock's
    /// sync time (the buffer's last update), drawing the buffer's
    /// randomizer tables from `rng.buf`.
    pub fn new(a: EmitterArgs, rng: &mut ParticleRandom, clock: &Ww3dClock) -> W3dResult<ParticleEmitter> {
        let emit_rate = if a.emit_rate > 0.0f32 { f2u(1000.0f32 / a.emit_rate)? } else { 1000 };
        let burst_size = if a.burst_size != 0 { a.burst_size } else { 1 };
        let max_age = if a.max_age > 0.0f32 { a.max_age } else { 1.0f32 };
        let mut vel_rand = a.vel_rnd;
        // VelRand->Scale(0.001f), unconditionally
        match vel_rand.as_mut() {
            Some(v) => v.scale(0.001f32),
            None => return Err(W3dUndefined::NullPointer.into()),
        }
        // the buffer's size: emission rate, burst size and lifetime, capped
        let mut max_num = f2i(burst_size as f32 * a.emit_rate * (max_age + 1.0f32))?;
        if a.max_particles > 0 {
            max_num = max_num.min(a.max_particles);
        }
        if a.max_buffer_size > 0 {
            max_num = max_num.min(a.max_buffer_size);
        }
        max_num = max_num.max(2); // max_num of 1 causes problems
        let bp = BufferParams {
            buffer_size: max_num as u32,
            color: a.color,
            opacity: a.opacity,
            size: a.size,
            rotation: a.rotation,
            orient_rnd: a.orient_rnd,
            frame: a.frames,
            blurtime: a.blur_times,
            accel: a.accel / 1_000_000.0f32,
            max_age,
            future_start: a.future_start,
            texture: a.texture,
            shader: a.shader,
            pingpong: a.pingpong,
            render_mode: a.render_mode,
            frame_mode: a.frame_mode,
            line_props: a.line_props,
        };
        let buffer = ParticleBuffer::new(&bp, &mut rng.buf, clock)?;
        Ok(ParticleEmitter {
            emit_rate,
            burst_size,
            one_time_burst_size: 1,
            one_time_burst: false,
            pos_rand: a.pos_rnd,
            base_vel: a.base_vel * 0.001f32,
            vel_rand,
            outward_vel: a.out_vel * 0.001f32,
            vel_inherit_factor: a.vel_inherit_factor,
            emit_remain: 0,
            prev_q: Quaternion::IDENTITY,
            prev_orig: Vector3::new(0.0, 0.0, 0.0),
            active: false,
            first_time: true,
            buffer_scene_needed: true,
            particles_left: a.max_particles,
            max_particles: a.max_particles,
            is_complete: false,
            name: Some(b"ParticleEmitter".to_vec()),
            remove_on_complete: true,
            is_in_scene: false,
            group_id: 0,
            buffer,
            buffer_in_scene: false,
            is_invisible: false,
            transform: Matrix3D::IDENTITY,
            visible: true,
            hidden: false,
            animation_hidden: false,
        })
    }

    /// `ParticleEmitterClass::Create_From_Definition(definition)` (the
    /// asset manager's `Get_Activate_Fog_On_Load()` false: no fog).
    pub fn create_from_definition(def: &ParticleEmitterDef, rng: &mut ParticleRandom, clock: &Ww3dClock) -> W3dResult<ParticleEmitter> {
        // the texture: none for an empty name
        let name = def.texture_filename().ok_or(W3dUndefined::StringOverrun)?;
        let texture = if name.is_empty() { None } else { Some(name) };
        let shader = ShaderClass(convert_shader(&def.info_v2.shader));
        let v3 = |c: [f32; 3]| Vector3::new(c[0], c[1], c[2]);
        let color = copy_props(&def.color_keyframes, 12, v3)?;
        let opacity = copy_props(&def.opacity_keyframes, 4, |x| x)?;
        let size = copy_props(&def.size_keyframes, 4, |x| x)?;
        let rotation = copy_props(&def.rotation_keyframes, 4, |x| x)?;
        let frames = copy_props(&def.frame_keyframes, 4, |x| x)?;
        let blur_times = copy_props(&def.blur_time_keyframes, 4, |x| x)?;
        // Get_Creation_Volume() and Get_Velocity_Random() clone their randomizers
        let Some(pos_rnd) = def.creation_volume else { return Err(W3dUndefined::NullPointer.into()) };
        let Some(vel_rnd) = def.velocity_randomizer else { return Err(W3dUndefined::NullPointer.into()) };
        let info = &def.info;
        let args = EmitterArgs {
            emit_rate: info.emission_rate,
            burst_size: def.info_v2.burst_size,
            pos_rnd: Some(pos_rnd),
            base_vel: Vector3::new(info.velocity.x, info.velocity.y, info.velocity.z),
            vel_rnd: Some(vel_rnd),
            out_vel: def.info_v2.outward_vel,
            vel_inherit_factor: def.info_v2.vel_inherit,
            color,
            opacity,
            size,
            rotation,
            orient_rnd: def.initial_orientation_random,
            frames,
            blur_times,
            accel: Vector3::new(info.acceleration.x, info.acceleration.y, info.acceleration.z),
            max_age: info.lifetime,
            future_start: def.extra_info.future_start_time,
            texture,
            shader,
            // Get_Max_Emissions() is a float, passed as an int
            max_particles: f2i(info.max_emissions)?,
            max_buffer_size: 0,
            pingpong: false,
            render_mode: def.render_mode(),
            frame_mode: def.frame_mode(),
            line_props: Some(def.line_properties),
        };
        let mut e = ParticleEmitter::new(args, rng, clock)?;
        // Pass the name along to the emitter
        e.set_name(def.name.clone());
        Ok(e)
    }

    /// `Set_Name(pname)`.
    pub fn set_name(&mut self, name: Option<Vec<u8>>) {
        self.name = name;
    }

    /// `Set_Transform(m)`.
    pub fn set_transform(&mut self, m: Matrix3D) {
        self.transform = m;
    }

    /// `Get_Position()`.
    pub fn get_position(&self) -> Vector3 {
        self.transform.get_translation()
    }

    /// `Is_Not_Hidden_At_All()`.
    pub fn is_not_hidden_at_all(&self) -> bool {
        self.visible && !self.hidden && !self.animation_hidden
    }

    /// `Set_Hidden(onoff)`.
    pub fn set_hidden(&mut self, onoff: bool) {
        self.hidden = onoff;
        self.update_on_visibility();
    }

    /// `Set_Visible(onoff)`.
    pub fn set_visible(&mut self, onoff: bool) {
        self.visible = onoff;
        self.update_on_visibility();
    }

    /// `Set_Animation_Hidden(onoff)`.
    pub fn set_animation_hidden(&mut self, onoff: bool) {
        self.animation_hidden = onoff;
        self.update_on_visibility();
    }

    /// `Set_Invisible(onoff)` (no visibility update, as upstream).
    pub fn set_invisible(&mut self, onoff: bool) {
        self.is_invisible = onoff;
    }

    /// `Update_On_Visibilty()`: start or stop with the visibility.
    pub fn update_on_visibility(&mut self) {
        if self.is_not_hidden_at_all() && !self.is_invisible && self.is_stopped() && self.is_in_scene {
            self.start();
        } else if (!self.is_not_hidden_at_all() || self.is_invisible) && !self.is_stopped() {
            self.stop();
        }
    }

    /// `Notify_Added(scene)`.
    pub fn notify_added(&mut self) {
        if !self.first_time {
            self.active = true;
        }
        self.is_in_scene = true;
    }

    /// `Notify_Removed(scene)`.
    pub fn notify_removed(&mut self) {
        self.active = false;
        self.is_in_scene = false;
    }

    /// `Reset()`.
    pub fn reset(&mut self) {
        self.active = true;
        self.prev_q = build_quaternion(&self.transform);
        self.prev_orig = self.transform.get_translation();
        self.particles_left = self.max_particles;
        self.emit_remain = 0;
        self.is_complete = false;
    }

    /// `Start()`.
    pub fn start(&mut self) {
        self.active = true;
        self.prev_q = build_quaternion(&self.transform);
        self.prev_orig = self.transform.get_translation();
        if self.is_complete {
            self.particles_left = self.max_particles;
            self.is_complete = false;
        }
        // to keep track of particles so that the line segments can start and stop properly
        self.group_id = self.group_id.wrapping_add(1);
        self.buffer.set_current_group_id(self.group_id);
    }

    /// `Restart()`.
    pub fn restart(&mut self) {
        self.start();
    }

    /// `Stop()`.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// `Is_Stopped()`.
    pub fn is_stopped(&self) -> bool {
        !self.active
    }

    /// `Set_Emission_Rate(rate)`.
    pub fn set_emission_rate(&mut self, rate: f32) -> W3dResult<()> {
        self.emit_rate = if rate > 0.0f32 { f2u(1000.0f32 / rate)? } else { 1000 };
        Ok(())
    }

    /// `Set_Burst_Size(size)`.
    pub fn set_burst_size(&mut self, size: i32) {
        self.burst_size = if size != 0 { size as u32 } else { 1 };
    }

    /// `Set_One_Time_Burst(size)`.
    pub fn set_one_time_burst(&mut self, size: i32) {
        self.one_time_burst_size = if size != 0 { size as u32 } else { 1 };
        self.one_time_burst = true;
    }

    /// `Set_Position_Randomizer(rand)`.
    pub fn set_position_randomizer(&mut self, r: Option<Vector3Randomizer>) {
        self.pos_rand = r;
    }

    /// `Set_Velocity_Randomizer(rand)` (scaled to ms).
    pub fn set_velocity_randomizer(&mut self, mut r: Option<Vector3Randomizer>) {
        if let Some(v) = r.as_mut() {
            v.scale(0.001f32);
        }
        self.vel_rand = r;
    }

    /// `Set_Base_Velocity(base_vel)`.
    pub fn set_base_velocity(&mut self, base_vel: Vector3) {
        self.base_vel = base_vel * 0.001f32;
    }

    /// `Set_Outwards_Velocity(out_vel)`.
    pub fn set_outwards_velocity(&mut self, out_vel: f32) {
        self.outward_vel = out_vel * 0.001f32;
    }

    /// `Set_Velocity_Inheritance_Factor(inh_factor)`.
    pub fn set_velocity_inheritance_factor(&mut self, f: f32) {
        self.vel_inherit_factor = f;
    }

    /// `Get_Emission_Rate()`.
    pub fn get_emission_rate(&self) -> f32 {
        1000.0f32 / self.emit_rate as f32
    }

    /// `Scale(scale)`.
    pub fn scale(&mut self, scale: f32) {
        if let Some(p) = self.pos_rand.as_mut() {
            p.scale(scale);
        }
        self.base_vel *= scale;
        if let Some(v) = self.vel_rand.as_mut() {
            v.scale(scale);
        }
        self.outward_vel *= scale;
        self.buffer.scale(scale);
    }

    /// `Is_Complete()`.
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// `On_Frame_Update()` (the emitter's): the first time it runs, the
    /// buffer joins the scene and the previous transform is taken.
    pub fn on_frame_update(&mut self) {
        if self.active && !self.is_complete && self.first_time {
            if self.buffer_scene_needed {
                if self.is_in_scene {
                    self.buffer_in_scene = true;
                    self.buffer_scene_needed = false;
                } else {
                    return;
                }
            }
            self.buffer_scene_needed = false;
            self.prev_q = build_quaternion(&self.transform);
            self.prev_orig = self.transform.get_translation();
            self.first_time = false;
        }
        // (a complete emitter in the scene with RemoveOnComplete asks to be released: the caller's)
    }

    /// `Emit()`.
    pub fn emit(&mut self, clock: &Ww3dClock, rng: &mut ParticleRandom) -> W3dResult<()> {
        let curr_quat = build_quaternion(&self.transform);
        let curr_orig = self.transform.get_translation();
        if self.active && !self.is_complete {
            self.create_new_particles(&curr_quat, &curr_orig, clock, rng)?;
        }
        // (either branch ends with the current transform as the previous)
        self.prev_q = curr_quat;
        self.prev_orig = curr_orig;
        Ok(())
    }

    /// `Create_New_Particles(curr_quat, curr_orig)`.
    fn create_new_particles(&mut self, curr_quat: &Quaternion, curr_orig: &Vector3, clock: &Ww3dClock, rng: &mut ParticleRandom) -> W3dResult<()> {
        let mut frametime = clock.frame_time;
        // the time modulo the time it takes to fill the buffer, when the loop would run over 100 times
        if frametime > 100u32.wrapping_mul(self.emit_rate) {
            let buf_size = self.buffer.get_buffer_size();
            let gcd = greatest_common_divisor(buf_size, self.burst_size);
            let bursts = buf_size / gcd;
            let cycle_time = self.emit_rate.wrapping_mul(bursts);
            if cycle_time > 1 {
                frametime %= cycle_time;
            } else {
                frametime = 1;
            }
        }
        self.emit_remain = self.emit_remain.wrapping_add(frametime);
        let fl_frametime = frametime as f32;
        let mut alpha = 1.0f32 - (self.emit_remain as f32 / fl_frametime);
        let d_alpha = self.emit_rate as f32 / fl_frametime;
        let slerp_info = slerp_setup(&self.prev_q, curr_quat);
        // the emitter's world-space velocity (InheritedWorldSpaceEmitterVel)
        let inherited = if self.vel_inherit_factor != 0.0f32 {
            (*curr_orig - self.prev_orig) * (self.vel_inherit_factor / fl_frametime)
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        };
        while self.emit_remain > self.emit_rate {
            if self.emit_rate == 0 && self.particles_left <= 0 {
                // EmitRemain -= 0 for ever
                return Err(W3dError::NonTerminating);
            }
            self.emit_remain -= self.emit_rate;
            alpha += d_alpha;
            let quat = cached_slerp(&self.prev_q, curr_quat, alpha, &slerp_info);
            let orig = Vector3::lerp(&self.prev_orig, curr_orig, alpha);
            let age = clock.sync_time.wrapping_sub(self.emit_remain);
            let mut burst_size = self.burst_size;
            if self.one_time_burst {
                burst_size = self.one_time_burst_size;
                self.one_time_burst = false;
            }
            if self.particles_left > 0 {
                if burst_size > self.particles_left as u32 {
                    burst_size = self.particles_left as u32;
                    self.particles_left = 0;
                } else {
                    self.particles_left -= burst_size as i32;
                }
                if self.particles_left <= 0 {
                    self.is_complete = true;
                }
            }
            for _ in 0..burst_size {
                self.initialize_particle(age, &quat, &orig, &inherited, &mut rng.v3);
            }
            if self.is_complete {
                break;
            }
        }
        Ok(())
    }

    /// `Initialize_Particle(newpart, timestamp, quat, orig)` on the buffer's
    /// next queue slot.
    fn initialize_particle(&mut self, timestamp: u32, quat: &Quaternion, orig: &Vector3, inherited: &Vector3, r: &mut Random3) {
        let rand_pos = match &self.pos_rand {
            Some(p) => p.get_vector(r),
            None => Vector3::new(0.0, 0.0, 0.0),
        };
        let position = quat.rotate_vector(&rand_pos) + *orig;
        let mut rand_vel = match &self.vel_rand {
            Some(v) => v.get_vector(r),
            None => Vector3::new(0.0, 0.0, 0.0),
        };
        // Add outwards velocity to emitterspace velocity
        if self.outward_vel != 0.0f32 {
            let pos_l2 = rand_pos.length2();
            let outwards = if pos_l2 != 0.0f32 {
                rand_pos * (self.outward_vel * wwmath::inv_sqrt(pos_l2))
            } else {
                Vector3::new(self.outward_vel, 0.0, 0.0)
            };
            rand_vel += outwards;
        }
        rand_vel += self.base_vel;
        let velocity = *inherited + quat.rotate_vector(&rand_vel);
        let group_id = self.group_id;
        let np = self.buffer.add_uninitialized_new_particle();
        np.time_stamp = timestamp;
        np.position = position;
        np.velocity = velocity;
        np.group_id = group_id;
    }

    /// One frame of the scene, in the order this translation drives it: the
    /// emitter's `On_Frame_Update`; then, once the buffer is in the scene,
    /// its `On_Frame_Update` (which calls `Emit`) and its `Render`. `None`
    /// before the buffer joins the scene.
    pub fn frame(&mut self, clock: &Ww3dClock, rng: &mut ParticleRandom) -> W3dResult<Option<ParticleDraw>> {
        self.on_frame_update();
        if !self.buffer_in_scene {
            return Ok(None);
        }
        // ParticleBufferClass::On_Frame_Update: Emitter->Emit()
        self.emit(clock, rng)?;
        let at = (self.get_position(), self.is_stopped());
        Ok(Some(self.buffer.render(clock, Some(at))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::part_buf::W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES;
    use crate::formats::w3d::shader::PRESET_ADDITIVE_SPRITE_SHADER;

    fn args() -> EmitterArgs {
        EmitterArgs {
            emit_rate: 10.0,
            burst_size: 2,
            pos_rnd: Some(Vector3Randomizer::SolidBox([0.0; 3])),
            base_vel: Vector3::new(0.0, 0.0, 1000.0),
            vel_rnd: Some(Vector3Randomizer::SolidBox([0.0; 3])),
            out_vel: 0.0,
            vel_inherit_factor: 0.0,
            color: Props { start: Vector3::new(1.0, 1.0, 1.0), ..Props::default() },
            opacity: Props { start: 1.0, ..Props::default() },
            size: Props { start: 1.0, ..Props::default() },
            rotation: Props::default(),
            orient_rnd: 0.0,
            frames: Props::default(),
            blur_times: Props::default(),
            accel: Vector3::default(),
            max_age: 1.0,
            future_start: 0.0,
            texture: None,
            shader: PRESET_ADDITIVE_SPRITE_SHADER,
            max_particles: 0,
            max_buffer_size: 0,
            pingpong: false,
            render_mode: W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES,
            frame_mode: 0,
            line_props: None,
        }
    }

    /// Predicted before the run: 10 per second is a period of 100 ms and a
    /// buffer of 2 * 10 * 2 = 40; in the scene and visible it starts (group
    /// 1); the first frame (at 0 ms) puts the buffer in the scene and emits
    /// nothing (frame time 0); a frame of 250 ms emits 2 bursts of 2
    /// (EmitRemain 250 -> 150 -> 50), stamped 250 - 150 = 100 and 250 - 50
    /// = 200; rising at 1 unit per ms from the origin, at sync 250 the older
    /// pair is at z 150 and the younger at 50; 50 ms remain. A maximum of 3
    /// emissions completes in that frame, the second burst cut to 1.
    ///
    /// *Result (2026-11-07), first run:* held.
    #[test]
    fn emits_bursts_over_sync_time() {
        let mut rng = ParticleRandom::default();
        let c0 = Ww3dClock::default();
        let mut e = ParticleEmitter::new(args(), &mut rng, &c0).unwrap();
        assert_eq!((e.emit_rate, e.buffer.get_buffer_size()), (100, 40));
        e.notify_added();
        e.update_on_visibility();
        assert_eq!((e.active, e.group_id), (true, 1));
        match e.frame(&c0, &mut rng).unwrap() {
            Some(ParticleDraw::Points(p)) => assert!(p.points.is_empty()),
            d => panic!("{d:?}"),
        }
        let c1 = Ww3dClock { sync_time: 250, frame_time: 250, frame_count: 1 };
        match e.frame(&c1, &mut rng).unwrap() {
            Some(ParticleDraw::Points(p)) => {
                assert_eq!(p.points.len(), 4);
                let z: Vec<f32> = p.points.iter().map(|p| p.position.z).collect();
                assert_eq!(z, vec![150.0, 150.0, 50.0, 50.0]);
            }
            d => panic!("{d:?}"),
        }
        assert_eq!(e.emit_remain, 50);
        let mut a = args();
        a.max_particles = 3;
        let mut e = ParticleEmitter::new(a, &mut rng, &c0).unwrap();
        e.notify_added();
        e.update_on_visibility();
        e.frame(&c0, &mut rng).unwrap();
        e.frame(&c1, &mut rng).unwrap();
        assert!(e.is_complete());
        assert_eq!(e.buffer.get_particle_count(), 3);
    }
}
