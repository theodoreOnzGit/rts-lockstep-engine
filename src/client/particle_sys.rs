// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp
//     (ParticleInfo::ParticleInfo; Particle::computeAlphaRate, computeColorRate, Particle::Particle,
//     Particle::~Particle, applyForce, update, doWindMotion, isInvisible; ParticleSystemInfo::ParticleSystemInfo;
//     ParticleSystem::ParticleSystem, ~ParticleSystem, setMaster, setSlave, start, stop, destroy, getPosition,
//     setPosition, setLocalTransform, rotateLocalTransformX/Y/Z, attachToDrawable, attachToObject,
//     computePointOnUnitSphere, computeParticleVelocity, computeParticlePosition, createParticle,
//     generateParticleInfo, update, updateWindMotion, addParticle, removeParticle, mergeRelatedParticleSystems,
//     setControlParticle; ParticleSystemTemplate's m_fieldParseTable, parseRandomKeyframe, parseRGBColorKeyframe,
//     createSlaveSystem; ParticleSystemManager's constructor, reset, update, createParticleSystem,
//     createAttachedParticleSystemID, findParticleSystem, destroyParticleSystemByID, findTemplate, newTemplate,
//     destroyAttachedSystems, addParticle, removeParticle, friend_addParticleSystem, friend_removeParticleSystem,
//     removeOldestParticles; angleBetween)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ParticleSys.h
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIParticleSys.cpp (INI::parseParticleSystemDefinition)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Michael S. Booth, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The particle systems (GitHub #43, #44): `ParticleSystem.ini`'s templates, the systems made from them and their
//! particles, as `ParticleSys.cpp` runs them. Plain Rust, no renderer: the Bevy client draws what
//! [`ParticleSystemManager::systems`] holds.
//!
//! # As upstream reads it
//!
//! - **The templates.** `ParticleSystem name` blocks (`INI::parseParticleSystemDefinition`): a template of that name is
//!   found or made (`newTemplate`) and its fields read (`m_fieldParseTable`). A second block of the same name reads
//!   into the first, over what it had.
//! - **The unions.** `m_emissionVelocity` and `m_emissionVolume` are unions: `VelOrthoX`, `VelSpherical`,
//!   `VelHemispherical`, `VelCylindricalRadial` and `VelOutward` are the same random variable, as are `VelOrthoY`,
//!   `VelCylindricalNormal` and `VelOutwardOther`; `VolLineStart`, `VolBoxHalfSize`, `VolSphereRadius` and
//!   `VolCylinderRadius`/`VolCylinderLength` share the line's first words. The last line read wins
//!   ([`ParticleSystemInfo::emission_velocity`], [`ParticleSystemInfo::emission_volume`]).
//! - **A system** (`ParticleSystem::ParticleSystem`) copies its template, draws its wind angles from the client random,
//!   turns `ColorScale` from 0..255 to 0..1, and makes its slave system (`SlaveSystem`) if asked.
//! - **Its update** (`ParticleSystem::update`, once per logic frame by `ParticleSystemManager::update`): the initial
//!   delay; the wind; the parent's transform (attached drawable or object, or the particle controlling it); then, unless
//!   destroyed, out of lifetime, shrouded, stopped or a slave, a burst when the burst delay is out (each particle's
//!   position from the emission volume, its velocity from the emission velocity type, both through the transform,
//!   then its damping, angles, lifetime, size, keyframes, colour scale and wind randomness; a per-particle attached
//!   system; the slave's particle); then every particle's update (gravity, velocity damping, drift, wind, rotation,
//!   size, alpha and colour keyframes, colour scale, lifetime, invisibility); then its own lifetime.
//! - **The manager** creates systems with ever-increasing ids, keeps them in creation order, and caps the particles at
//!   `MaxParticleCount`, removing the oldest of lower priorities to make room.
//!
//! # Upstream quirks that are kept
//!
//! - `computeParticlePosition`'s hollow `BOX` has no `break`: it falls through into `LINE`, which draws once more and
//!   places the particle on the line whose start is the box's half size (the union). Its fifth side places `x` at
//!   `halfSize.y`.
//! - `computeParticleVelocity`'s `OUTWARD` with a volume type it does not handle (none) leaves the velocity of the
//!   previous call (`static Coord3D newVel`, shared by every system): [`ParticleSystemManager::last_new_vel`].
//! - `Particle::update`'s colour clamp tests `red < 0` to zero `green`.
//! - A colour or alpha key past the eighth (`m_colorTargetKey` reaching 8 when all eight keys have frames) reads
//!   beyond the array upstream (undefined): here it reads as a key at frame 0 (no rate), see
//!   [`Particle::color_key_at`].
//!
//! # Where this differs from upstream
//!
//! - Systems and particles are owned by the manager ([`ParticleSystemManager::systems`], each system's
//!   [`ParticleSystem::particles`] in its list order), not linked by pointers; a particle is named by its system and
//!   its personality. The global per-priority lists `removeOldestParticles` reads are each particle's creation order
//!   ([`Particle::overall_seq`]) and the priority it was listed under.
//! - What the system reads of the world (an attached drawable's or object's transform, position and shroud, the
//!   ground height) comes through [`ParticleHost`]; `TheGlobalData` and `TheGameLODManager`'s particle limits through
//!   [`ParticleSettings`]; the client frame (`TheGameClient->getFrame()`) and the logic frame are given to
//!   [`ParticleSystemManager::update`].
//! - The random draws are the client's own generator (`GameClientRandomValue`, [`RandomValues`]), never the logic's.
//! - `DRAWABLE` particles (particles that are drawables) and `SMUDGE`/`VOLUME_PARTICLE` rendering are not translated;
//!   such systems run, and the client draws their particles as quads.
//! - Saving and loading (`xfer`, `loadPostProcess`) is not translated.

use std::collections::BTreeMap;

use crate::common::base_type::{real_to_int, Coord2D, Coord3D, RGBColor};
use crate::common::game_type::ObjectId;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::random_value::{DistributionType, GameClientRandomVariable, RandomValues};
use crate::common::trig;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `MAX_KEYFRAMES`.
pub const MAX_KEYFRAMES: usize = 8;
/// `MAX_SIZE_BONUS`.
pub const MAX_SIZE_BONUS: f32 = 50.0;
/// `PI`.
const PI: f32 = std::f32::consts::PI;
/// `TWO_PI`.
const TWO_PI: f32 = 2.0 * std::f32::consts::PI;
/// `INVALID_PARTICLE_SYSTEM_ID`.
pub const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemId = 0;
/// `DEFAULT_VOLUME_PARTICLE_DEPTH`.
pub const DEFAULT_VOLUME_PARTICLE_DEPTH: u32 = 0;
/// `OPTIMUM_VOLUME_PARTICLE_DEPTH`.
pub const OPTIMUM_VOLUME_PARTICLE_DEPTH: u32 = 6;

/// `enum ParticleSystemID`.
pub type ParticleSystemId = u32;

/// `enum ParticlePriorityType` (`INVALID_PRIORITY` 0, `WEAPON_EXPLOSION` = `PARTICLE_PRIORITY_LOWEST` 1, ...,
/// `ALWAYS_RENDER` = `PARTICLE_PRIORITY_HIGHEST` 13).
pub mod priority {
    pub const INVALID_PRIORITY: i32 = 0;
    pub const PARTICLE_PRIORITY_LOWEST: i32 = 1;
    pub const WEAPON_EXPLOSION: i32 = 1;
    pub const SCORCHMARK: i32 = 2;
    pub const DUST_TRAIL: i32 = 3;
    pub const BUILDUP: i32 = 4;
    pub const DEBRIS_TRAIL: i32 = 5;
    pub const UNIT_DAMAGE_FX: i32 = 6;
    pub const DEATH_EXPLOSION: i32 = 7;
    pub const SEMI_CONSTANT: i32 = 8;
    pub const CONSTANT: i32 = 9;
    pub const WEAPON_TRAIL: i32 = 10;
    pub const AREA_EFFECT: i32 = 11;
    pub const CRITICAL: i32 = 12;
    pub const ALWAYS_RENDER: i32 = 13;
    pub const PARTICLE_PRIORITY_HIGHEST: i32 = 13;
    pub const NUM_PARTICLE_PRIORITIES: i32 = 14;
}

/// `ParticleShaderType` (`INVALID_SHADER` 0, `ADDITIVE`, `ALPHA`, `ALPHA_TEST`, `MULTIPLY`).
pub mod shader_type {
    pub const INVALID_SHADER: i32 = 0;
    pub const ADDITIVE: i32 = 1;
    pub const ALPHA: i32 = 2;
    pub const ALPHA_TEST: i32 = 3;
    pub const MULTIPLY: i32 = 4;
}

/// `ParticleType` (`INVALID_TYPE` 0, `PARTICLE`, `DRAWABLE`, `STREAK`, `VOLUME_PARTICLE`, `SMUDGE`).
pub mod particle_type {
    pub const INVALID_TYPE: i32 = 0;
    pub const PARTICLE: i32 = 1;
    pub const DRAWABLE: i32 = 2;
    pub const STREAK: i32 = 3;
    pub const VOLUME_PARTICLE: i32 = 4;
    pub const SMUDGE: i32 = 5;
}

/// `EmissionVelocityType` (`INVALID_VELOCITY` 0, `ORTHO`, `SPHERICAL`, `HEMISPHERICAL`, `CYLINDRICAL`, `OUTWARD`).
pub mod velocity_type {
    pub const INVALID_VELOCITY: i32 = 0;
    pub const ORTHO: i32 = 1;
    pub const SPHERICAL: i32 = 2;
    pub const HEMISPHERICAL: i32 = 3;
    pub const CYLINDRICAL: i32 = 4;
    pub const OUTWARD: i32 = 5;
}

/// `EmissionVolumeType` (`INVALID_VOLUME` 0, `POINT`, `LINE`, `BOX`, `SPHERE`, `CYLINDER`).
pub mod volume_type {
    pub const INVALID_VOLUME: i32 = 0;
    pub const POINT: i32 = 1;
    pub const LINE: i32 = 2;
    pub const BOX: i32 = 3;
    pub const SPHERE: i32 = 4;
    pub const CYLINDER: i32 = 5;
}

/// `WindMotion` (`WIND_MOTION_INVALID` 0, `WIND_MOTION_NOT_USED`, `WIND_MOTION_PING_PONG`, `WIND_MOTION_CIRCULAR`).
pub mod wind_motion {
    pub const WIND_MOTION_INVALID: i32 = 0;
    pub const WIND_MOTION_NOT_USED: i32 = 1;
    pub const WIND_MOTION_PING_PONG: i32 = 2;
    pub const WIND_MOTION_CIRCULAR: i32 = 3;
}

/// `ParticleShaderTypeNames`.
pub const PARTICLE_SHADER_TYPE_NAMES: &[&str] = &["NONE", "ADDITIVE", "ALPHA", "ALPHA_TEST", "MULTIPLY"];
/// `ParticleTypeNames`.
pub const PARTICLE_TYPE_NAMES: &[&str] = &["NONE", "PARTICLE", "DRAWABLE", "STREAK", "VOLUME_PARTICLE", "SMUDGE"];
/// `EmissionVelocityTypeNames`.
pub const EMISSION_VELOCITY_TYPE_NAMES: &[&str] = &["NONE", "ORTHO", "SPHERICAL", "HEMISPHERICAL", "CYLINDRICAL", "OUTWARD"];
/// `EmissionVolumeTypeNames`.
pub const EMISSION_VOLUME_TYPE_NAMES: &[&str] = &["NONE", "POINT", "LINE", "BOX", "SPHERE", "CYLINDER"];
/// `ParticlePriorityNames`.
pub const PARTICLE_PRIORITY_NAMES: &[&str] = &[
    "NONE",
    "WEAPON_EXPLOSION",
    "SCORCHMARK",
    "DUST_TRAIL",
    "BUILDUP",
    "DEBRIS_TRAIL",
    "UNIT_DAMAGE_FX",
    "DEATH_EXPLOSION",
    "SEMI_CONSTANT",
    "CONSTANT",
    "WEAPON_TRAIL",
    "AREA_EFFECT",
    "CRITICAL",
    "ALWAYS_RENDER",
];
/// `WindMotionNames`.
pub const WIND_MOTION_NAMES: &[&str] = &["NONE", "Unused", "PingPong", "Circular"];

/// `struct Keyframe`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keyframe {
    pub value: f32,
    pub frame: u32,
}

/// `struct RGBColorKeyframe`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RgbColorKeyframe {
    pub color: RGBColor,
    pub frame: u32,
}

/// `ParticleSystemInfo::RandomKeyframe`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RandomKeyframe {
    pub var: GameClientRandomVariable,
    pub frame: u32,
}

/// A random variable as zeroed memory holds it (`CONSTANT`, 0, 0): upstream's templates come from a zeroing pool
/// (`MemoryPool::allocateBlockImplementation`) and `ParticleSystemInfo()` leaves these fields alone.
pub const ZERO_VAR: GameClientRandomVariable = GameClientRandomVariable { distribution: DistributionType::Constant, low: 0.0, high: 0.0 };

impl Default for RandomKeyframe {
    fn default() -> Self {
        RandomKeyframe { var: ZERO_VAR, frame: 0 }
    }
}

/// `class ParticleSystemInfo`: what a template holds and a system copies.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystemInfo {
    /// `m_isOneShot`.
    pub is_one_shot: bool,
    /// `m_shaderType` ([`shader_type`]).
    pub shader_type: i32,
    /// `m_particleType` ([`particle_type`]).
    pub particle_type: i32,
    /// `m_particleTypeName`: the texture for `PARTICLE` and `STREAK`, the thing template for `DRAWABLE`.
    pub particle_type_name: Vec<u8>,
    pub angle_z: GameClientRandomVariable,
    pub angular_rate_z: GameClientRandomVariable,
    pub angular_damping: GameClientRandomVariable,
    pub vel_damping: GameClientRandomVariable,
    /// `m_lifetime` (frames).
    pub lifetime: GameClientRandomVariable,
    /// `m_systemLifetime` (frames; 0: forever).
    pub system_lifetime: u32,
    pub start_size: GameClientRandomVariable,
    pub start_size_rate: GameClientRandomVariable,
    pub size_rate: GameClientRandomVariable,
    pub size_rate_damping: GameClientRandomVariable,
    /// `m_volumeParticleDepth`.
    pub volume_particle_depth: u32,
    pub alpha_key: [RandomKeyframe; MAX_KEYFRAMES],
    pub color_key: [RgbColorKeyframe; MAX_KEYFRAMES],
    /// `m_colorScale` (0..255 in a template, 0..1 in a system).
    pub color_scale: GameClientRandomVariable,
    pub burst_delay: GameClientRandomVariable,
    pub burst_count: GameClientRandomVariable,
    pub initial_delay: GameClientRandomVariable,
    pub drift_velocity: Coord3D,
    pub gravity: f32,
    pub slave_system_name: Vec<u8>,
    pub slave_pos_offset: Coord3D,
    pub attached_system_name: Vec<u8>,
    /// `m_emissionVelocityType` ([`velocity_type`]).
    pub emission_velocity_type: i32,
    /// `m_priority` ([`priority`]).
    pub priority: i32,
    /// `m_emissionVelocity`, the union's three random variables: `[0]` is `ortho.x`, `spherical.speed`,
    /// `hemispherical.speed`, `cylindrical.radial` and `outward.speed`; `[1]` is `ortho.y`, `cylindrical.normal` and
    /// `outward.otherSpeed`; `[2]` is `ortho.z`.
    pub emission_velocity: [GameClientRandomVariable; 3],
    /// `m_emissionVolumeType` ([`volume_type`]).
    pub emission_volume_type: i32,
    /// `m_emissionVolume`, the union's six words: `line.start` (x, y, z) then `line.end`; `box.halfSize` is `[0..3]`,
    /// `sphere.radius` `[0]`, `cylinder.radius` `[0]` and `cylinder.length` `[1]`.
    pub emission_volume: [f32; 6],
    pub is_emission_volume_hollow: bool,
    pub is_ground_aligned: bool,
    pub is_emit_above_ground_only: bool,
    pub is_particle_up_towards_emitter: bool,
    /// `m_windMotion` ([`wind_motion`]).
    pub wind_motion: i32,
    pub wind_angle: f32,
    pub wind_angle_change: f32,
    pub wind_angle_change_min: f32,
    pub wind_angle_change_max: f32,
    pub wind_motion_start_angle: f32,
    pub wind_motion_start_angle_min: f32,
    pub wind_motion_start_angle_max: f32,
    pub wind_motion_end_angle: f32,
    pub wind_motion_end_angle_min: f32,
    pub wind_motion_end_angle_max: f32,
    pub wind_motion_moving_to_end_angle: bool,
}

impl Default for ParticleSystemInfo {
    /// `ParticleSystemInfo()`, over zeroed memory.
    fn default() -> Self {
        ParticleSystemInfo {
            is_one_shot: false,
            shader_type: shader_type::INVALID_SHADER,
            particle_type: particle_type::INVALID_TYPE,
            particle_type_name: Vec::new(),
            angle_z: ZERO_VAR,
            angular_rate_z: ZERO_VAR,
            angular_damping: ZERO_VAR,
            vel_damping: ZERO_VAR,
            lifetime: ZERO_VAR,
            system_lifetime: 0,
            start_size: ZERO_VAR,
            start_size_rate: ZERO_VAR,
            size_rate: ZERO_VAR,
            size_rate_damping: ZERO_VAR,
            volume_particle_depth: DEFAULT_VOLUME_PARTICLE_DEPTH,
            alpha_key: [RandomKeyframe::default(); MAX_KEYFRAMES],
            color_key: [RgbColorKeyframe::default(); MAX_KEYFRAMES],
            color_scale: ZERO_VAR,
            burst_delay: ZERO_VAR,
            burst_count: ZERO_VAR,
            initial_delay: ZERO_VAR,
            drift_velocity: Coord3D::default(),
            gravity: 0.0,
            slave_system_name: Vec::new(),
            slave_pos_offset: Coord3D::default(),
            attached_system_name: Vec::new(),
            emission_velocity_type: velocity_type::INVALID_VELOCITY,
            priority: priority::PARTICLE_PRIORITY_LOWEST,
            emission_velocity: [ZERO_VAR; 3],
            emission_volume_type: volume_type::INVALID_VOLUME,
            emission_volume: [0.0; 6],
            is_emission_volume_hollow: false,
            is_ground_aligned: false,
            is_emit_above_ground_only: false,
            is_particle_up_towards_emitter: false,
            wind_motion: wind_motion::WIND_MOTION_NOT_USED,
            wind_angle: 0.0,
            wind_angle_change: 0.15,
            wind_angle_change_min: 0.15,
            wind_angle_change_max: 0.45,
            wind_motion_start_angle: 0.0,
            wind_motion_start_angle_min: 0.0,
            wind_motion_start_angle_max: PI / 4.0,
            wind_motion_end_angle: TWO_PI - (PI / 4.0),
            wind_motion_end_angle_min: TWO_PI - (PI / 4.0),
            wind_motion_end_angle_max: TWO_PI,
            wind_motion_moving_to_end_angle: true,
        }
    }
}

impl ParticleSystemInfo {
    /// `m_emissionVolume.line.start`.
    pub fn line_start(&self) -> Coord3D {
        Coord3D { x: self.emission_volume[0], y: self.emission_volume[1], z: self.emission_volume[2] }
    }
    /// `m_emissionVolume.line.end`.
    pub fn line_end(&self) -> Coord3D {
        Coord3D { x: self.emission_volume[3], y: self.emission_volume[4], z: self.emission_volume[5] }
    }
    /// `m_emissionVolume.box.halfSize`.
    pub fn box_half_size(&self) -> Coord3D {
        self.line_start()
    }

    /// `tintAllColors(tintColor)`: every key but the first scaled by the colour's components over 255.
    pub fn tint_all_colors(&mut self, tint_color: i32) {
        let r = ((tint_color >> 16) & 0xff) as f32;
        let g = ((tint_color >> 8) & 0xff) as f32;
        let b = (tint_color & 0xff) as f32;
        for key in &mut self.color_key[1..] {
            key.color.red *= r / 255.0;
            key.color.green *= g / 255.0;
            key.color.blue *= b / 255.0;
        }
    }
}

/// `class ParticleSystemTemplate`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystemTemplate {
    /// `m_name`.
    pub name: Vec<u8>,
    pub info: ParticleSystemInfo,
}

/// `parseRandomKeyframe`: `low high frame`.
fn parse_random_keyframe(ini: &mut Ini<'_>) -> IniResult<RandomKeyframe> {
    let low = Ini::scan_real(&ini.get_next_token(None)?)?;
    let high = Ini::scan_real(&ini.get_next_token(None)?)?;
    let frame = Ini::scan_unsigned_int(&ini.get_next_token(None)?)?;
    let mut var = ZERO_VAR;
    var.set_range(low, high, DistributionType::Uniform);
    Ok(RandomKeyframe { var, frame })
}

/// `parseRGBColorKeyframe`: `R:r G:g B:b frame`.
fn parse_rgb_color_keyframe(ini: &mut Ini<'_>) -> IniResult<RgbColorKeyframe> {
    let color = ini.parse_rgb_color()?;
    let frame = ini.parse_unsigned_int()?;
    Ok(RgbColorKeyframe { color, frame })
}

macro_rules! rv {
    ($name:literal, $($f:tt)+) => {
        FieldParse::new($name, |ini, t: &mut ParticleSystemTemplate, _| ini.parse_game_client_random_variable(&mut t.info.$($f)+), UserData::None)
    };
}
macro_rules! keyframes {
    ($kind:ident, $parse:ident, $($n:literal => $i:literal),*) => {
        [$(FieldParse::new($n, |ini, t: &mut ParticleSystemTemplate, _| { t.info.$kind[$i] = $parse(ini)?; Ok(()) }, UserData::None)),*]
    };
}

/// `ParticleSystemTemplate::m_fieldParseTable`.
pub fn particle_system_field_parse_table() -> Vec<FieldParse<ParticleSystemTemplate>> {
    type T = ParticleSystemTemplate;
    let mut v: Vec<FieldParse<T>> = vec![
        FieldParse::new("Priority", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.priority = x), UserData::Names(PARTICLE_PRIORITY_NAMES)),
        FieldParse::new("IsOneShot", |ini, t: &mut T, _| ini.parse_bool().map(|x| t.info.is_one_shot = x), UserData::None),
        FieldParse::new("Shader", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.shader_type = x), UserData::Names(PARTICLE_SHADER_TYPE_NAMES)),
        FieldParse::new("Type", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.particle_type = x), UserData::Names(PARTICLE_TYPE_NAMES)),
        FieldParse::new("ParticleName", |ini, t: &mut T, _| ini.parse_ascii_string().map(|x| t.info.particle_type_name = x), UserData::None),
        rv!("AngleZ", angle_z),
        rv!("AngularRateZ", angular_rate_z),
        rv!("AngularDamping", angular_damping),
        rv!("VelocityDamping", vel_damping),
        FieldParse::new("Gravity", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.gravity = x), UserData::None),
        FieldParse::new("SlaveSystem", |ini, t: &mut T, _| ini.parse_ascii_string().map(|x| t.info.slave_system_name = x), UserData::None),
        FieldParse::new("SlavePosOffset", |ini, t: &mut T, _| ini.parse_coord3d().map(|x| t.info.slave_pos_offset = x), UserData::None),
        FieldParse::new("PerParticleAttachedSystem", |ini, t: &mut T, _| ini.parse_ascii_string().map(|x| t.info.attached_system_name = x), UserData::None),
        rv!("Lifetime", lifetime),
        FieldParse::new("SystemLifetime", |ini, t: &mut T, _| ini.parse_unsigned_int().map(|x| t.info.system_lifetime = x), UserData::None),
        rv!("Size", start_size),
        rv!("StartSizeRate", start_size_rate),
        rv!("SizeRate", size_rate),
        rv!("SizeRateDamping", size_rate_damping),
    ];
    v.extend(keyframes!(alpha_key, parse_random_keyframe, "Alpha1" => 0, "Alpha2" => 1, "Alpha3" => 2, "Alpha4" => 3, "Alpha5" => 4, "Alpha6" => 5, "Alpha7" => 6, "Alpha8" => 7));
    v.extend(keyframes!(color_key, parse_rgb_color_keyframe, "Color1" => 0, "Color2" => 1, "Color3" => 2, "Color4" => 3, "Color5" => 4, "Color6" => 5, "Color7" => 6, "Color8" => 7));
    v.extend([
        rv!("ColorScale", color_scale),
        rv!("BurstDelay", burst_delay),
        rv!("BurstCount", burst_count),
        rv!("InitialDelay", initial_delay),
        FieldParse::new("DriftVelocity", |ini, t: &mut T, _| ini.parse_coord3d().map(|x| t.info.drift_velocity = x), UserData::None),
        FieldParse::new("VelocityType", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.emission_velocity_type = x), UserData::Names(EMISSION_VELOCITY_TYPE_NAMES)),
        rv!("VelOrthoX", emission_velocity[0]),
        rv!("VelOrthoY", emission_velocity[1]),
        rv!("VelOrthoZ", emission_velocity[2]),
        rv!("VelSpherical", emission_velocity[0]),
        rv!("VelHemispherical", emission_velocity[0]),
        rv!("VelCylindricalRadial", emission_velocity[0]),
        rv!("VelCylindricalNormal", emission_velocity[1]),
        rv!("VelOutward", emission_velocity[0]),
        rv!("VelOutwardOther", emission_velocity[1]),
        FieldParse::new("VolumeType", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.emission_volume_type = x), UserData::Names(EMISSION_VOLUME_TYPE_NAMES)),
        FieldParse::new("VolLineStart", |ini, t: &mut T, _| ini.parse_coord3d().map(|c| t.info.emission_volume[0..3].copy_from_slice(&[c.x, c.y, c.z])), UserData::None),
        FieldParse::new("VolLineEnd", |ini, t: &mut T, _| ini.parse_coord3d().map(|c| t.info.emission_volume[3..6].copy_from_slice(&[c.x, c.y, c.z])), UserData::None),
        FieldParse::new("VolBoxHalfSize", |ini, t: &mut T, _| ini.parse_coord3d().map(|c| t.info.emission_volume[0..3].copy_from_slice(&[c.x, c.y, c.z])), UserData::None),
        FieldParse::new("VolSphereRadius", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.emission_volume[0] = x), UserData::None),
        FieldParse::new("VolCylinderRadius", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.emission_volume[0] = x), UserData::None),
        FieldParse::new("VolCylinderLength", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.emission_volume[1] = x), UserData::None),
        FieldParse::new("IsHollow", |ini, t: &mut T, _| ini.parse_bool().map(|x| t.info.is_emission_volume_hollow = x), UserData::None),
        FieldParse::new("IsGroundAligned", |ini, t: &mut T, _| ini.parse_bool().map(|x| t.info.is_ground_aligned = x), UserData::None),
        FieldParse::new("IsEmitAboveGroundOnly", |ini, t: &mut T, _| ini.parse_bool().map(|x| t.info.is_emit_above_ground_only = x), UserData::None),
        FieldParse::new("IsParticleUpTowardsEmitter", |ini, t: &mut T, _| ini.parse_bool().map(|x| t.info.is_particle_up_towards_emitter = x), UserData::None),
        FieldParse::new("WindMotion", |ini, t: &mut T, ud| ini.parse_index_list(ud.names()).map(|x| t.info.wind_motion = x), UserData::Names(WIND_MOTION_NAMES)),
        FieldParse::new("WindAngleChangeMin", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_angle_change_min = x), UserData::None),
        FieldParse::new("WindAngleChangeMax", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_angle_change_max = x), UserData::None),
        FieldParse::new("WindPingPongStartAngleMin", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_motion_start_angle_min = x), UserData::None),
        FieldParse::new("WindPingPongStartAngleMax", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_motion_start_angle_max = x), UserData::None),
        FieldParse::new("WindPingPongEndAngleMin", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_motion_end_angle_min = x), UserData::None),
        FieldParse::new("WindPingPongEndAngleMax", |ini, t: &mut T, _| ini.parse_real().map(|x| t.info.wind_motion_end_angle_max = x), UserData::None),
    ]);
    v
}

/// `INI::parseParticleSystemDefinition`: the template of that name, found or made, read.
fn parse_particle_system_definition(ini: &mut Ini<'_>, m: &mut ParticleSystemManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    m.new_template(&name);
    let table = particle_system_field_parse_table();
    let t = m.templates.get_mut(&name).expect("made");
    ini.init_from_ini(t, &table)
}

/// The block table of `ParticleSystem.ini`.
pub const PARTICLE_SYSTEM_BLOCKS: &[BlockParse<ParticleSystemManager>] = &[BlockParse { token: "ParticleSystem", parse: parse_particle_system_definition }];

/// `class ParticleInfo`: what a new particle starts with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParticleInfo {
    pub vel: Coord3D,
    pub pos: Coord3D,
    pub emitter_pos: Coord3D,
    pub vel_damping: f32,
    pub angle_z: f32,
    pub angular_rate_z: f32,
    pub angular_damping: f32,
    pub lifetime: u32,
    pub size: f32,
    pub size_rate: f32,
    pub size_rate_damping: f32,
    pub alpha_key: [Keyframe; MAX_KEYFRAMES],
    pub color_key: [RgbColorKeyframe; MAX_KEYFRAMES],
    pub color_scale: f32,
    pub wind_randomness: f32,
    pub particle_up_towards_emitter: bool,
}

/// `class Particle`.
#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub info: ParticleInfo,
    /// `m_personality`: its place in its system's emission order.
    pub personality: u32,
    pub accel: Coord3D,
    pub last_pos: Coord3D,
    pub lifetime_left: u32,
    pub create_timestamp: u32,
    pub alpha: f32,
    pub alpha_rate: f32,
    pub alpha_target_key: usize,
    pub color: RGBColor,
    pub color_rate: RGBColor,
    pub color_target_key: usize,
    pub is_culled: bool,
    /// `m_systemUnderControl`: the system this particle carries (`PerParticleAttachedSystem`).
    pub system_under_control: Option<ParticleSystemId>,
    /// Its place in the manager's overall list (`m_overallNext`), and the priority it was listed under.
    pub overall_seq: u64,
    pub overall_priority: i32,
}

impl Particle {
    /// `Particle(system, info)` (the lists are the manager's: [`ParticleSystemManager::create_particle`]).
    fn new(info: &ParticleInfo, frame: u32) -> Particle {
        let mut p = Particle {
            info: info.clone(),
            personality: 0,
            accel: Coord3D::default(),
            last_pos: Coord3D::default(),
            lifetime_left: info.lifetime,
            create_timestamp: frame,
            alpha: info.alpha_key[0].value,
            alpha_rate: 0.0,
            alpha_target_key: 1,
            color: info.color_key[0].color,
            color_rate: RGBColor::default(),
            color_target_key: 1,
            is_culled: false,
            system_under_control: None,
            overall_seq: 0,
            overall_priority: 0,
        };
        p.compute_alpha_rate();
        p.compute_color_rate();
        p
    }

    /// `m_alphaKey[i]`, a key past the eighth reading as frame 0 (see the module docs).
    fn alpha_key_at(&self, i: usize) -> Keyframe {
        self.info.alpha_key.get(i).copied().unwrap_or_default()
    }

    /// `m_colorKey[i]`, a key past the eighth reading as frame 0 (see the module docs).
    pub fn color_key_at(&self, i: usize) -> RgbColorKeyframe {
        self.info.color_key.get(i).copied().unwrap_or_default()
    }

    /// `computeAlphaRate()`.
    fn compute_alpha_rate(&mut self) {
        let t = self.alpha_key_at(self.alpha_target_key);
        if t.frame == 0 {
            self.alpha_rate = 0.0;
            return;
        }
        let p = self.alpha_key_at(self.alpha_target_key - 1);
        let delta = t.value - p.value;
        let time = t.frame.wrapping_sub(p.frame);
        self.alpha_rate = delta / time as f32;
    }

    /// `computeColorRate()`.
    fn compute_color_rate(&mut self) {
        let t = self.color_key_at(self.color_target_key);
        if t.frame == 0 {
            self.color_rate = RGBColor::default();
            return;
        }
        let p = self.color_key_at(self.color_target_key - 1);
        let time = t.frame.wrapping_sub(p.frame) as f32;
        self.color_rate.red = (t.color.red - p.color.red) / time;
        self.color_rate.green = (t.color.green - p.color.green) / time;
        self.color_rate.blue = (t.color.blue - p.color.blue) / time;
    }

    /// `applyForce(force)`.
    pub fn apply_force(&mut self, force: &Coord3D) {
        self.accel.x += force.x;
        self.accel.y += force.y;
        self.accel.z += force.z;
    }

    /// `getPosition()`.
    pub fn get_position(&self) -> Coord3D {
        self.info.pos
    }

    /// `update()` at client frame `frame`, in system `sys` (its drift, wind and shader), with `system_pos` the
    /// system's position plus its attachment's (`doWindMotion`'s): false when the particle dies.
    // (the clamps keep upstream's if-chains, the green one testing red)
    #[allow(clippy::manual_clamp)]
    fn update(&mut self, sys: &ParticleSystemInfo, system_pos: Coord3D, frame: u32) -> bool {
        let i = &mut self.info;
        i.vel.x += self.accel.x;
        i.vel.y += self.accel.y;
        i.vel.z += self.accel.z;
        i.vel.x *= i.vel_damping;
        i.vel.y *= i.vel_damping;
        i.vel.z *= i.vel_damping;
        let drift = sys.drift_velocity;
        i.pos.x += i.vel.x + drift.x;
        i.pos.y += i.vel.y + drift.y;
        i.pos.z += i.vel.z + drift.z;
        if sys.wind_motion != wind_motion::WIND_MOTION_NOT_USED {
            self.do_wind_motion(sys.wind_angle, system_pos);
        }
        let i = &mut self.info;
        i.angle_z += i.angular_rate_z;
        i.angular_rate_z *= i.angular_damping;
        if i.particle_up_towards_emitter {
            let up_vec = Coord2D { x: 0.0, y: 1.0 };
            let emitter_dir = Coord2D { x: i.pos.x - i.emitter_pos.x, y: i.pos.y - i.emitter_pos.y };
            i.angle_z = angle_between(&up_vec, &emitter_dir) + PI;
        }
        i.size += i.size_rate;
        i.size_rate *= i.size_rate_damping;

        if sys.shader_type != shader_type::ADDITIVE {
            self.alpha += self.alpha_rate;
            let t = self.alpha_key_at(self.alpha_target_key);
            if self.alpha_target_key < MAX_KEYFRAMES && t.frame != 0 {
                if frame.wrapping_sub(self.create_timestamp) >= t.frame {
                    self.alpha = t.value;
                    self.alpha_target_key += 1;
                    self.compute_alpha_rate();
                }
            } else {
                self.alpha_rate = 0.0;
            }
            if self.alpha < 0.0 {
                self.alpha = 0.0;
            } else if self.alpha > 1.0 {
                self.alpha = 1.0;
            }
        }

        self.color.red += self.color_rate.red;
        self.color.green += self.color_rate.green;
        self.color.blue += self.color_rate.blue;
        let t = self.color_key_at(self.color_target_key);
        if self.color_target_key < MAX_KEYFRAMES && t.frame != 0 {
            if frame.wrapping_sub(self.create_timestamp) >= t.frame {
                // can't set, because of colorscale
                self.color_target_key += 1;
                self.compute_color_rate();
            }
        } else {
            self.color_rate = RGBColor::default();
        }
        let cs = self.info.color_scale;
        self.color.red += cs;
        self.color.green += cs;
        self.color.blue += cs;
        if self.color.red < 0.0 {
            self.color.red = 0.0;
        } else if self.color.red > 1.0 {
            self.color.red = 1.0;
        }
        // (upstream tests red here)
        if self.color.red < 0.0 {
            self.color.green = 0.0;
        } else if self.color.green > 1.0 {
            self.color.green = 1.0;
        }
        if self.color.blue < 0.0 {
            self.color.blue = 0.0;
        } else if self.color.blue > 1.0 {
            self.color.blue = 1.0;
        }
        self.accel = Coord3D::default();

        if self.lifetime_left != 0 {
            self.lifetime_left -= 1;
            if self.lifetime_left == 0 {
                return false;
            }
        }
        !self.is_invisible(sys.shader_type)
    }

    /// `doWindMotion()`, with the system's wind angle and `system_pos` its position plus its attachment's.
    fn do_wind_motion(&mut self, wind_angle: f32, system_pos: Coord3D) {
        let v = Coord3D { x: self.info.pos.x - system_pos.x, y: self.info.pos.y - system_pos.y, z: self.info.pos.z - system_pos.z };
        let full_force_distance = 75.0f32;
        let no_force_distance = 200.0f32;
        let dist_from_wind = v.length();
        if dist_from_wind < no_force_distance {
            let mut wind_force_strength = 2.0 * self.info.wind_randomness;
            if dist_from_wind > full_force_distance {
                wind_force_strength *= 1.0 - ((dist_from_wind - full_force_distance) / (no_force_distance - full_force_distance));
            }
            self.info.pos.x += trig::cos(wind_angle) * wind_force_strength;
            self.info.pos.y += trig::sin(wind_angle) * wind_force_strength;
        }
    }

    /// `isInvisible()` under the system's shader.
    pub fn is_invisible(&self, shader: i32) -> bool {
        match shader {
            shader_type::ADDITIVE => {
                if self.color_key_at(self.color_target_key).frame == 0 && (self.color.red + self.color.green + self.color.blue) <= 0.06 {
                    return true;
                }
                false
            }
            shader_type::ALPHA => self.alpha < 0.02,
            shader_type::ALPHA_TEST => false,
            shader_type::MULTIPLY => {
                if self.color_key_at(self.color_target_key).frame == 0 && (self.color.red * self.color.green * self.color.blue) > 0.95 {
                    return true;
                }
                false
            }
            // should never get here - if we do, data is incorrect
            _ => true,
        }
    }
}

/// `angleBetween(vecA, vecB)`.
fn angle_between(a: &Coord2D, b: &Coord2D) -> f32 {
    if !(a.length() != 0.0 && b.length() != 0.0) {
        return 0.0;
    }
    let length_a = a.length();
    let length_b = b.length();
    let dot_product = a.x * b.x + a.y * b.y;
    let cos_theta = dot_product / (length_a * length_b);
    if dot_product == 0.0 {
        if b.x > 0.0 {
            return PI;
        }
        return 0.0;
    }
    let theta = trig::acos(cos_theta);
    if b.x > 0.0 {
        return theta;
    }
    -theta
}

/// What a system is attached to, as its update reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attachment {
    /// The drawable's (or object's) `getTransformMatrix()`.
    pub transform: Matrix3D,
    /// `getPosition()`.
    pub position: Coord3D,
    /// For a drawable, `getFullyObscuredByShroud()`; for an object, `getShroudedStatus(localPlayerIndex) >=
    /// OBJECTSHROUD_FOGGED`.
    pub shrouded: bool,
}

/// What `ParticleSystem::update` reads of the world.
pub trait ParticleHost {
    /// `TheGameClient->findDrawableByID(id)` (`None`: destroyed).
    fn drawable(&self, id: ObjectId) -> Option<Attachment>;
    /// `TheGameLogic->findObjectByID(id)`: its drawable's transform if it has one, else its own; shrouded for
    /// `local_player_index` (`None`: destroyed).
    fn object(&self, id: ObjectId, local_player_index: i32) -> Option<Attachment>;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height(&self, x: f32, y: f32) -> f32;
}

/// A host with nothing attached and flat ground at 0.
pub struct NoParticleHost;

impl ParticleHost for NoParticleHost {
    fn drawable(&self, _: ObjectId) -> Option<Attachment> {
        None
    }
    fn object(&self, _: ObjectId, _: i32) -> Option<Attachment> {
        None
    }
    fn ground_height(&self, _: f32, _: f32) -> f32 {
        0.0
    }
}

/// `TheGlobalData`'s and `TheGameLODManager`'s particle settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleSettings {
    /// `m_useFX`.
    pub use_fx: bool,
    /// `m_particleScale`.
    pub particle_scale: f32,
    /// `m_maxParticleCount`.
    pub max_particle_count: i32,
    /// `m_maxFieldParticleCount`.
    pub max_field_particle_count: i32,
    /// `getMinDynamicParticlePriority()`.
    pub min_dynamic_particle_priority: i32,
    /// `getMinDynamicParticleSkipPriority()`.
    pub min_dynamic_particle_skip_priority: i32,
    /// `isParticleSkipped()`.
    pub particle_skipped: bool,
}

impl Default for ParticleSettings {
    /// `GlobalData()`'s defaults (`m_maxParticleCount` 0: no particle is made until the game data sets it), and no
    /// dynamic LOD limit.
    fn default() -> Self {
        ParticleSettings {
            use_fx: true,
            particle_scale: 1.0,
            max_particle_count: 0,
            max_field_particle_count: 30,
            min_dynamic_particle_priority: priority::PARTICLE_PRIORITY_LOWEST,
            min_dynamic_particle_skip_priority: priority::PARTICLE_PRIORITY_LOWEST,
            particle_skipped: false,
        }
    }
}

/// `class ParticleSystem`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystem {
    /// `m_systemID`.
    pub id: ParticleSystemId,
    /// The template's name (`m_template`).
    pub template: Vec<u8>,
    /// The inherited `ParticleSystemInfo`.
    pub info: ParticleSystemInfo,
    /// Its particles, in its list's order (`m_systemParticlesHead` on).
    pub particles: Vec<Particle>,
    /// `m_attachedToDrawableID` (0: none; a drawable's id is its object's here).
    pub attached_to_drawable: ObjectId,
    /// `m_attachedToObjectID` (0: none).
    pub attached_to_object: ObjectId,
    pub local_transform: Matrix3D,
    pub transform: Matrix3D,
    pub burst_delay_left: u32,
    pub delay_left: u32,
    pub start_timestamp: u32,
    pub system_lifetime_left: u32,
    pub personality_store: u32,
    pub accumulated_size_bonus: f32,
    pub vel_coeff: Coord3D,
    pub count_coeff: f32,
    pub delay_coeff: f32,
    pub size_coeff: f32,
    pub pos: Coord3D,
    pub last_pos: Coord3D,
    /// `m_slaveSystem`, `m_masterSystem`.
    pub slave: Option<ParticleSystemId>,
    pub master: Option<ParticleSystemId>,
    /// `m_controlParticle`: the particle carrying this system (its system and personality).
    pub control_particle: Option<(ParticleSystemId, u32)>,
    pub is_local_identity: bool,
    pub is_identity: bool,
    pub is_forever: bool,
    pub is_stopped: bool,
    pub is_destroyed: bool,
    pub is_first_pos: bool,
    pub is_saveable: bool,
    pub skip_parent_xfrm: bool,
}

impl ParticleSystem {
    /// `getPosition()`: the local transform's translation.
    pub fn get_position(&self) -> Coord3D {
        let v = self.local_transform.get_translation();
        Coord3D { x: v.x, y: v.y, z: v.z }
    }
    /// `setPosition(pos)`.
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.local_transform.set_x_translation(pos.x);
        self.local_transform.set_y_translation(pos.y);
        self.local_transform.set_z_translation(pos.z);
        self.is_local_identity = false;
    }
    /// `setLocalTransform(matrix)`.
    pub fn set_local_transform(&mut self, m: &Matrix3D) {
        self.local_transform = *m;
        self.is_local_identity = false;
    }
    /// `rotateLocalTransformX(x)`.
    pub fn rotate_local_transform_x(&mut self, x: f32) {
        self.local_transform.rotate_x(x);
        self.is_local_identity = false;
    }
    /// `rotateLocalTransformY(y)`.
    pub fn rotate_local_transform_y(&mut self, y: f32) {
        self.local_transform.rotate_y(y);
        self.is_local_identity = false;
    }
    /// `rotateLocalTransformZ(z)`.
    pub fn rotate_local_transform_z(&mut self, z: f32) {
        self.local_transform.rotate_z(z);
        self.is_local_identity = false;
    }
    /// `attachToDrawable(draw)` (0: none).
    pub fn attach_to_drawable(&mut self, id: ObjectId) {
        self.attached_to_drawable = id;
    }
    /// `attachToObject(obj)` (0: none).
    pub fn attach_to_object(&mut self, id: ObjectId) {
        self.attached_to_object = id;
    }
    /// `start()`.
    pub fn start(&mut self) {
        self.is_stopped = false;
    }
    /// `stop()`.
    pub fn stop(&mut self) {
        self.is_stopped = true;
    }
    /// `setInitialDelay(delay)`.
    pub fn set_initial_delay(&mut self, delay: u32) {
        self.delay_left = delay;
    }
    /// `setEmissionVolumeSphereRadius(r)`.
    pub fn set_emission_volume_sphere_radius(&mut self, r: f32) {
        if self.info.emission_volume_type == volume_type::SPHERE {
            self.info.emission_volume[0] = r;
        }
    }
    /// `setEmissionVolumeCylinderRadius(r)`.
    pub fn set_emission_volume_cylinder_radius(&mut self, r: f32) {
        if self.info.emission_volume_type == volume_type::CYLINDER {
            self.info.emission_volume[0] = r;
        }
    }
    /// `getParticleCount()`.
    pub fn get_particle_count(&self) -> u32 {
        self.particles.len() as u32
    }
    /// `shouldBillboard()`.
    pub fn should_billboard(&self) -> bool {
        !self.info.is_ground_aligned
    }
    /// `isUsingStreak()`.
    pub fn is_using_streak(&self) -> bool {
        self.info.particle_type == particle_type::STREAK
    }

    /// `updateWindMotion()`.
    fn update_wind_motion(&mut self, random: &mut RandomValues) {
        let i = &mut self.info;
        match i.wind_motion {
            wind_motion::WIND_MOTION_PING_PONG => {
                let start_angle = i.wind_motion_start_angle;
                let end_angle = i.wind_motion_end_angle;
                let total_span = end_angle - start_angle;
                let half_span = total_span / 2.0;
                let diff_from_center = (half_span - i.wind_angle + start_angle).abs();
                let mut change = (1.0 - (diff_from_center / half_span)) * i.wind_angle_change;
                const MINIMUM_CHANGE: f32 = 0.005;
                if change < MINIMUM_CHANGE {
                    change = MINIMUM_CHANGE;
                }
                let turn = if i.wind_motion_moving_to_end_angle {
                    i.wind_angle += change;
                    i.wind_angle >= end_angle
                } else {
                    i.wind_angle -= change;
                    i.wind_angle <= start_angle
                };
                if turn {
                    i.wind_motion_moving_to_end_angle = !i.wind_motion_moving_to_end_angle;
                    i.wind_angle_change = random.game_client_random_value_real(i.wind_angle_change_min, i.wind_angle_change_max);
                    i.wind_motion_start_angle = random.game_client_random_value_real(i.wind_motion_start_angle_min, i.wind_motion_start_angle_max);
                    i.wind_motion_end_angle = random.game_client_random_value_real(i.wind_motion_end_angle_min, i.wind_motion_end_angle_max);
                }
            }
            wind_motion::WIND_MOTION_CIRCULAR => {
                if i.wind_angle_change == 0.0 {
                    i.wind_angle_change = random.game_client_random_value_real(i.wind_angle_change_min, i.wind_angle_change_max);
                }
                i.wind_angle += i.wind_angle_change;
                if i.wind_angle > TWO_PI {
                    i.wind_angle -= TWO_PI;
                } else if i.wind_angle < 0.0 {
                    i.wind_angle += TWO_PI;
                }
            }
            _ => {}
        }
    }

    /// `computePointOnUnitSphere()`.
    fn compute_point_on_unit_sphere(random: &mut RandomValues) -> Coord3D {
        let mut point;
        loop {
            point = Coord3D {
                x: random.game_client_random_value_real(-1.0, 1.0),
                y: random.game_client_random_value_real(-1.0, 1.0),
                z: random.game_client_random_value_real(-1.0, 1.0),
            };
            if !(point.x == 0.0 && point.y == 0.0 && point.z == 0.0) {
                break;
            }
        }
        point.normalize();
        point
    }

    /// `computeParticleVelocity(pos)`; `last_new_vel` is upstream's `static Coord3D newVel`.
    fn compute_particle_velocity(&self, pos: &Coord3D, random: &mut RandomValues, last_new_vel: &mut Coord3D, particle_scale: f32) -> Coord3D {
        let ev = &self.info.emission_velocity;
        let mut new_vel = *last_new_vel;
        match self.info.emission_velocity_type {
            velocity_type::ORTHO => {
                new_vel.x = ev[0].get_value(random);
                new_vel.y = ev[1].get_value(random);
                new_vel.z = ev[2].get_value(random);
            }
            velocity_type::CYLINDRICAL => {
                let radial_speed = ev[0].get_value(random);
                let angle = random.game_client_random_value_real(0.0, 2.0 * PI);
                new_vel.x = radial_speed * crate::ww::wwmath::cos(angle);
                new_vel.y = radial_speed * crate::ww::wwmath::sin(angle);
                new_vel.z = ev[1].get_value(random);
            }
            velocity_type::OUTWARD => {
                let speed = ev[0].get_value(random);
                let other_speed = ev[1].get_value(random);
                let sys_pos = Coord3D::default();
                match self.info.emission_volume_type {
                    volume_type::CYLINDER => {
                        let mut disk = Coord2D { x: pos.x - sys_pos.x, y: pos.y - sys_pos.y };
                        disk.normalize();
                        new_vel.x = speed * disk.x;
                        new_vel.y = speed * disk.y;
                        new_vel.z = other_speed;
                    }
                    volume_type::BOX | volume_type::SPHERE => {
                        new_vel = Coord3D { x: pos.x - sys_pos.x, y: pos.y - sys_pos.y, z: pos.z - sys_pos.z };
                        new_vel.normalize();
                        new_vel.x *= speed;
                        new_vel.y *= speed;
                        new_vel.z *= speed;
                    }
                    volume_type::LINE => {
                        let (s, e) = (self.info.line_start(), self.info.line_end());
                        let mut along = Coord3D { x: e.x - s.x, y: e.y - s.y, z: e.z - s.z };
                        along.normalize();
                        let up = Coord3D { x: 0.0, y: 0.0, z: 1.0 };
                        let perp = Coord3D::cross_product(&up, &along);
                        let up = Coord3D::cross_product(&along, &perp);
                        new_vel.x = speed * perp.x + other_speed * up.x;
                        new_vel.y = speed * perp.y + other_speed * up.y;
                        new_vel.z = speed * perp.z + other_speed * up.z;
                    }
                    volume_type::POINT => {
                        let vel = Self::compute_point_on_unit_sphere(random);
                        new_vel.x = speed * vel.x;
                        new_vel.y = speed * vel.y;
                        new_vel.z = speed * vel.z;
                    }
                    // (no case: the previous call's velocity)
                    _ => {}
                }
            }
            velocity_type::SPHERICAL => {
                let speed = ev[0].get_value(random);
                let vel = Self::compute_point_on_unit_sphere(random);
                new_vel.x = speed * vel.x;
                new_vel.y = speed * vel.y;
                new_vel.z = speed * vel.z;
            }
            velocity_type::HEMISPHERICAL => {
                let speed = ev[0].get_value(random);
                let mut vel;
                loop {
                    vel = Coord3D {
                        x: random.game_client_random_value_real(-1.0, 1.0),
                        y: random.game_client_random_value_real(-1.0, 1.0),
                        z: random.game_client_random_value_real(0.0, 1.0),
                    };
                    if !(vel.x == 0.0 && vel.y == 0.0 && vel.z == 0.0) {
                        break;
                    }
                }
                vel.normalize();
                new_vel.x = speed * vel.x;
                new_vel.y = speed * vel.y;
                new_vel.z = speed * vel.z;
            }
            _ => new_vel = Coord3D::default(),
        }
        new_vel.x *= self.vel_coeff.x * (0.5 + particle_scale / 2.0);
        new_vel.y *= self.vel_coeff.y * (0.5 + particle_scale / 2.0);
        new_vel.z *= self.vel_coeff.z * (0.5 + particle_scale / 2.0);
        *last_new_vel = new_vel;
        new_vel
    }

    /// `computeParticlePosition()`; `last_new_pos` is upstream's `static Coord3D newPos` (every case writes all
    /// three, so it only carries the hollow box's sides into the line it falls through to).
    fn compute_particle_position(&self, random: &mut RandomValues, last_new_pos: &mut Coord3D, particle_scale: f32) -> Coord3D {
        let v = &self.info.emission_volume;
        let hollow = self.info.is_emission_volume_hollow;
        let mut new_pos = *last_new_pos;
        let mut line = false;
        match self.info.emission_volume_type {
            volume_type::CYLINDER => {
                let angle = random.game_client_random_value_real(0.0, 2.0 * PI);
                let radius = if hollow { v[0] } else { random.game_client_random_value_real(0.0, v[0]) };
                new_pos.x = radius * crate::ww::wwmath::cos(angle);
                new_pos.y = radius * crate::ww::wwmath::sin(angle);
                let half_length = v[1] / 2.0;
                new_pos.z = random.game_client_random_value_real(-half_length, half_length);
            }
            volume_type::SPHERE => {
                let radius = if hollow { v[0] } else { random.game_client_random_value_real(0.0, v[0]) };
                new_pos = Self::compute_point_on_unit_sphere(random);
                new_pos.x *= radius;
                new_pos.y *= radius;
                new_pos.z *= radius;
            }
            volume_type::BOX => {
                let h = self.info.box_half_size();
                if hollow {
                    let side = random.game_client_random_value(0, 6);
                    if side % 3 == 0 {
                        new_pos.x = random.game_client_random_value_real(-h.x, h.x);
                        new_pos.y = random.game_client_random_value_real(-h.y, h.y);
                        new_pos.z = if side == 0 { -h.z } else { h.z };
                    } else if side % 3 == 1 {
                        new_pos.y = random.game_client_random_value_real(-h.y, h.y);
                        new_pos.z = random.game_client_random_value_real(-h.z, h.z);
                        // (upstream: halfSize.y for the far side)
                        new_pos.x = if side == 1 { -h.x } else { h.y };
                    } else {
                        new_pos.x = random.game_client_random_value_real(-h.x, h.x);
                        new_pos.z = random.game_client_random_value_real(-h.z, h.z);
                        new_pos.y = if side == 2 { -h.y } else { h.y };
                    }
                    // (no `break`: falls through into LINE)
                    line = true;
                } else {
                    new_pos.x = random.game_client_random_value_real(-h.x, h.x);
                    new_pos.y = random.game_client_random_value_real(-h.y, h.y);
                    new_pos.z = random.game_client_random_value_real(-h.z, h.z);
                }
            }
            volume_type::LINE => line = true,
            _ => new_pos = Coord3D::default(),
        }
        if line {
            let (start, end) = (self.info.line_start(), self.info.line_end());
            let delta = Coord3D { x: end.x - start.x, y: end.y - start.y, z: end.z - start.z };
            let t = random.game_client_random_value_real(0.0, 1.0);
            new_pos.x = start.x + t * delta.x;
            new_pos.y = start.y + t * delta.y;
            new_pos.z = start.z + t * delta.z;
        }
        new_pos.x *= 0.5 + particle_scale / 2.0;
        new_pos.y *= 0.5 + particle_scale / 2.0;
        new_pos.z *= 0.5 + particle_scale / 2.0;
        *last_new_pos = new_pos;
        new_pos
    }
}

/// `class ParticleSystemManager`: the templates, the systems and the particle limits.
#[derive(Clone, Debug)]
pub struct ParticleSystemManager {
    /// `m_templateMap` (by exact name, as `AsciiString`'s hash map).
    pub templates: BTreeMap<Vec<u8>, ParticleSystemTemplate>,
    /// `m_allParticleSystemList`, in its order.
    pub systems: Vec<ParticleSystem>,
    /// `m_uniqueSystemID`: the last id given.
    pub unique_system_id: ParticleSystemId,
    /// `m_particleCount`.
    pub particle_count: u32,
    /// `m_fieldParticleCount` (set by the renderer each frame).
    pub field_particle_count: u32,
    /// `m_lastLogicFrameUpdate` (`None`: -1, after `reset`).
    pub last_logic_frame_update: Option<u32>,
    /// `m_localPlayerIndex`.
    pub local_player_index: i32,
    /// `computeParticleVelocity`'s and `computeParticlePosition`'s statics.
    pub last_new_vel: Coord3D,
    pub last_new_pos: Coord3D,
    /// The next overall-list place.
    next_seq: u64,
}

impl Default for ParticleSystemManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticleSystemManager {
    /// `ParticleSystemManager()`.
    pub fn new() -> Self {
        ParticleSystemManager {
            templates: BTreeMap::new(),
            systems: Vec::new(),
            unique_system_id: INVALID_PARTICLE_SYSTEM_ID,
            particle_count: 0,
            field_particle_count: 0,
            last_logic_frame_update: Some(0),
            local_player_index: 0,
            last_new_vel: Coord3D::default(),
            last_new_pos: Coord3D::default(),
            next_seq: 0,
        }
    }

    /// `INI::load` of one particle system file (`init` loads `Data\INI\ParticleSystem.ini`).
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, PARTICLE_SYSTEM_BLOCKS, self)
    }

    /// `reset()`: every system gone, the counts and ids back to their start; the templates kept.
    pub fn reset(&mut self) {
        self.systems.clear();
        self.particle_count = 0;
        self.field_particle_count = 0;
        self.unique_system_id = INVALID_PARTICLE_SYSTEM_ID;
        self.last_logic_frame_update = None;
    }

    /// `findTemplate(name)`.
    pub fn find_template(&self, name: &[u8]) -> Option<&ParticleSystemTemplate> {
        self.templates.get(name)
    }

    /// `newTemplate(name)`: the one of that name, or a new one.
    pub fn new_template(&mut self, name: &[u8]) -> &mut ParticleSystemTemplate {
        self.templates.entry(name.to_vec()).or_insert_with(|| ParticleSystemTemplate { name: name.to_vec(), info: ParticleSystemInfo::default() })
    }

    /// `getParticleSystemCount()`.
    pub fn get_particle_system_count(&self) -> usize {
        self.systems.len()
    }

    /// The index of system `id` in the list.
    fn index_of(&self, id: ParticleSystemId) -> Option<usize> {
        if id == INVALID_PARTICLE_SYSTEM_ID {
            return None;
        }
        self.systems.iter().position(|s| s.id == id)
    }

    /// `findParticleSystem(id)`.
    pub fn find_particle_system(&self, id: ParticleSystemId) -> Option<&ParticleSystem> {
        self.index_of(id).map(|i| &self.systems[i])
    }

    /// `findParticleSystem(id)`, mutable.
    pub fn find_particle_system_mut(&mut self, id: ParticleSystemId) -> Option<&mut ParticleSystem> {
        self.index_of(id).map(|i| &mut self.systems[i])
    }

    /// `createParticleSystem(sysTemplate, createSlaves)` of the template named `name` at client frame `frame`: the
    /// new system's id (`None` without the template). `ParticleSystem::ParticleSystem` and, if asked, its slave
    /// (`createSlaveSystem`, which makes its own slaves).
    pub fn create_particle_system(&mut self, name: &[u8], create_slaves: bool, random: &mut RandomValues, frame: u32) -> Option<ParticleSystemId> {
        let t = self.templates.get(name)?.clone();
        self.unique_system_id = self.unique_system_id.wrapping_add(1);
        let id = self.unique_system_id;
        let s = &t.info;
        let mut info = s.clone();
        info.volume_particle_depth = DEFAULT_VOLUME_PARTICLE_DEPTH;
        let (low, hi) = (s.color_scale.get_minimum_value(), s.color_scale.get_maximum_value());
        info.color_scale.set_range(low / 255.0, hi / 255.0, DistributionType::Uniform);
        let delay_left = s.initial_delay.get_value(random) as u32;
        info.wind_motion_start_angle = random.game_client_random_value_real(info.wind_motion_start_angle_min, info.wind_motion_start_angle_max);
        info.wind_motion_end_angle = random.game_client_random_value_real(info.wind_motion_end_angle_min, info.wind_motion_end_angle_max);
        info.wind_angle = random.game_client_random_value_real(info.wind_motion_start_angle, info.wind_motion_end_angle);
        let sys = ParticleSystem {
            id,
            template: t.name.clone(),
            info,
            particles: Vec::new(),
            attached_to_drawable: 0,
            attached_to_object: 0,
            local_transform: Matrix3D::IDENTITY,
            transform: Matrix3D::IDENTITY,
            burst_delay_left: 0,
            delay_left,
            start_timestamp: frame,
            system_lifetime_left: s.system_lifetime,
            personality_store: 0,
            accumulated_size_bonus: 0.0,
            vel_coeff: Coord3D { x: 1.0, y: 1.0, z: 1.0 },
            count_coeff: 1.0,
            delay_coeff: 1.0,
            size_coeff: 1.0,
            pos: Coord3D::default(),
            last_pos: Coord3D::default(),
            slave: None,
            master: None,
            control_particle: None,
            is_local_identity: true,
            is_identity: true,
            is_forever: s.system_lifetime == 0,
            is_stopped: false,
            is_destroyed: false,
            is_first_pos: true,
            is_saveable: true,
            skip_parent_xfrm: false,
        };
        // the slave is made inside the constructor, before the system is listed
        let slave = if create_slaves && !s.slave_system_name.is_empty() { self.create_particle_system(&s.slave_system_name.clone(), true, random, frame) } else { None };
        let mut sys = sys;
        if let Some(sl) = slave {
            sys.slave = Some(sl);
            if let Some(i) = self.index_of(sl) {
                self.systems[i].master = Some(id);
            }
        }
        self.systems.push(sys);
        Some(id)
    }

    /// `createAttachedParticleSystemID(sysTemplate, attachTo, createSlaves)`.
    pub fn create_attached_particle_system_id(&mut self, name: &[u8], attach_to: ObjectId, create_slaves: bool, random: &mut RandomValues, frame: u32) -> ParticleSystemId {
        let Some(id) = self.create_particle_system(name, create_slaves, random, frame) else { return INVALID_PARTICLE_SYSTEM_ID };
        if attach_to != 0 {
            self.find_particle_system_mut(id).expect("made").attach_to_object(attach_to);
        }
        id
    }

    /// `ParticleSystem::destroy()` of system `id`: it stops emitting, and its slave is destroyed too.
    pub fn destroy(&mut self, id: ParticleSystemId) {
        let mut next = Some(id);
        while let Some(i) = next.and_then(|id| self.index_of(id)) {
            let s = &mut self.systems[i];
            s.is_destroyed = true;
            next = s.slave;
        }
    }

    /// `destroyParticleSystemByID(id)`.
    pub fn destroy_particle_system_by_id(&mut self, id: ParticleSystemId) {
        self.destroy(id);
    }

    /// `destroyAttachedSystems(obj)`.
    pub fn destroy_attached_systems(&mut self, obj: ObjectId) {
        if obj == 0 {
            return;
        }
        let ids: Vec<ParticleSystemId> = self.systems.iter().filter(|s| s.attached_to_object == obj).map(|s| s.id).collect();
        for id in ids {
            self.destroy(id);
        }
    }

    /// `~Particle()` of particle `k` of system index `si`: removed from both lists; the system it controlled detached
    /// and destroyed.
    fn delete_particle(&mut self, si: usize, k: usize) {
        let p = self.systems[si].particles.remove(k);
        self.particle_count -= 1;
        if let Some(c) = p.system_under_control {
            if let Some(ci) = self.index_of(c) {
                self.systems[ci].control_particle = None;
            }
            self.destroy(c);
        }
    }

    /// `~ParticleSystem()` of system index `si`: its slave and master unlinked, its particles deleted, its control
    /// particle detached, then unlisted (`friend_removeParticleSystem`).
    fn delete_system(&mut self, si: usize) {
        let id = self.systems[si].id;
        if let Some(sl) = self.systems[si].slave.take() {
            if let Some(i) = self.index_of(sl) {
                self.systems[i].master = None;
            }
        }
        if let Some(m) = self.systems[si].master.take() {
            if let Some(i) = self.index_of(m) {
                self.systems[i].slave = None;
            }
        }
        while !self.systems[si].particles.is_empty() {
            self.delete_particle(si, 0);
        }
        if let Some((cs, personality)) = self.systems[si].control_particle.take() {
            if let Some(i) = self.index_of(cs) {
                if let Some(p) = self.systems[i].particles.iter_mut().find(|p| p.personality == personality) {
                    p.system_under_control = None;
                }
            }
        }
        let si = self.index_of(id).expect("listed");
        self.systems.remove(si);
    }

    /// `removeOldestParticles(count, priorityCap)`: the oldest particle of the lowest priority below the cap, up to
    /// `count` times; upstream's answer, `countToRemove - count` with `count` an `UnsignedInt` that the loop's last
    /// `count--` test takes below zero when it runs out: `countToRemove + 1` then, whatever was removed, so
    /// `createParticle`'s comparison with the excess fails and the new particle is not made (kept).
    pub fn remove_oldest_particles(&mut self, count: u32, priority_cap: i32) -> i32 {
        let count_to_remove = count as i32;
        let mut count = count;
        loop {
            let c = count;
            count = count.wrapping_sub(1);
            if !(c != 0 && self.particle_count != 0) {
                break;
            }
            for pr in priority::PARTICLE_PRIORITY_LOWEST..priority_cap {
                let mut oldest: Option<(u64, usize, usize)> = None;
                for (si, s) in self.systems.iter().enumerate() {
                    for (k, p) in s.particles.iter().enumerate() {
                        if p.overall_priority == pr && oldest.is_none_or(|o| p.overall_seq < o.0) {
                            oldest = Some((p.overall_seq, si, k));
                        }
                    }
                }
                if let Some((_, si, k)) = oldest {
                    self.delete_particle(si, k);
                    break;
                }
            }
        }
        count_to_remove.wrapping_sub(count as i32)
    }

    /// `ParticleSystem::createParticle(info, priority, forceCreate)` in system index `si`: the new particle's place
    /// in the system's list (`None`: not made).
    fn create_particle(&mut self, si: usize, info: &ParticleInfo, pri: i32, force_create: bool, settings: &ParticleSettings, frame: u32) -> Option<usize> {
        if !force_create {
            if !settings.use_fx {
                return None;
            }
            if pri < settings.min_dynamic_particle_priority || (pri < settings.min_dynamic_particle_skip_priority && settings.particle_skipped) {
                return None;
            }
            let s = &self.systems[si];
            if s.get_particle_count() > 0 && pri == priority::AREA_EFFECT && s.info.is_ground_aligned && self.field_particle_count > settings.max_field_particle_count as u32 {
                return None;
            }
            if pri != priority::ALWAYS_RENDER {
                let num_in_excess = self.particle_count.wrapping_sub(settings.max_particle_count as u32) as i32;
                if num_in_excess > 0 && self.remove_oldest_particles(num_in_excess as u32, pri) != num_in_excess {
                    return None;
                }
                if settings.max_particle_count == 0 {
                    return None;
                }
            }
        }
        // (the system may have moved in the list if particles were removed: systems are not, only particles)
        let mut p = Particle::new(info, frame);
        p.overall_seq = self.next_seq;
        p.overall_priority = pri;
        self.next_seq += 1;
        self.particle_count += 1;
        let s = &mut self.systems[si];
        p.personality = s.personality_store;
        s.personality_store = s.personality_store.wrapping_add(1);
        s.particles.push(p);
        Some(s.particles.len() - 1)
    }

    /// `generateParticleInfo(particleNum, particleCount)` of system index `si`.
    fn generate_particle_info(&mut self, si: usize, particle_num: i32, particle_count: i32, random: &mut RandomValues, settings: &ParticleSettings) -> ParticleInfo {
        let mut info = ParticleInfo::default();
        if particle_count == 0 {
            return info;
        }
        let scale = settings.particle_scale;
        let (mut lnv, mut lnp) = (self.last_new_vel, self.last_new_pos);
        let s = &mut self.systems[si];
        info.pos = s.compute_particle_position(random, &mut lnp, scale);
        info.vel = s.compute_particle_velocity(&info.pos, random, &mut lnv, scale);
        if !s.is_identity {
            if s.is_first_pos {
                s.last_pos = s.pos;
                s.is_first_pos = false;
            }
            let f = 1.0 - (particle_num as f32 / particle_count as f32);
            let adj = Coord3D { x: f * (s.pos.x - s.last_pos.x), y: f * (s.pos.y - s.last_pos.y), z: f * (s.pos.z - s.last_pos.z) };
            let pr = s.transform.mul_vector3(&Vector3 { x: info.pos.x, y: info.pos.y, z: info.pos.z });
            info.pos = Coord3D { x: pr.x - adj.x, y: pr.y - adj.y, z: pr.z - adj.z };
            let vr = s.transform.rotate_vector(&Vector3 { x: info.vel.x, y: info.vel.y, z: info.vel.z });
            info.vel = Coord3D { x: vr.x, y: vr.y, z: vr.z };
        }
        let i = &s.info;
        info.vel_damping = i.vel_damping.get_value(random);
        info.angular_damping = i.angular_damping.get_value(random);
        info.angle_z = i.angle_z.get_value(random);
        info.angular_rate_z = i.angular_rate_z.get_value(random);
        info.lifetime = i.lifetime.get_value(random) as u32;
        info.size = i.start_size.get_value(random) * s.size_coeff * scale;
        info.size_rate = i.size_rate.get_value(random) * s.size_coeff * scale;
        info.size_rate_damping = i.size_rate_damping.get_value(random);
        info.size += s.accumulated_size_bonus;
        s.accumulated_size_bonus += i.start_size_rate.get_value(random);
        if s.accumulated_size_bonus != 0.0 {
            s.accumulated_size_bonus = s.accumulated_size_bonus.min(MAX_SIZE_BONUS);
        }
        for k in 0..MAX_KEYFRAMES {
            info.alpha_key[k].value = i.alpha_key[k].var.get_value(random);
            info.alpha_key[k].frame = i.alpha_key[k].frame;
            info.color_key[k] = i.color_key[k];
        }
        info.color_scale = i.color_scale.get_value(random);
        let pos = s.transform.mul_vector3(&Vector3 { x: 0.0, y: 0.0, z: 0.0 });
        info.emitter_pos = Coord3D { x: pos.x, y: pos.y, z: pos.z };
        info.particle_up_towards_emitter = i.is_particle_up_towards_emitter;
        info.wind_randomness = random.game_client_random_value_real(0.7, 1.3);
        self.last_new_vel = lnv;
        self.last_new_pos = lnp;
        info
    }

    /// `mergeRelatedParticleSystems(master, slave, false)`: a master's particle info with the slave's own size
    /// factors, angles, keys and colour scale, offset by the slave's offset.
    fn merge_related_particle_systems(&mut self, master: usize, slave: usize, random: &mut RandomValues, settings: &ParticleSettings) -> ParticleInfo {
        let mut merge = self.generate_particle_info(master, 1, 1, random, settings);
        let info = self.generate_particle_info(slave, 1, 1, random, settings);
        merge.lifetime = info.lifetime;
        merge.size *= info.size;
        merge.size_rate *= info.size_rate;
        merge.size_rate_damping *= info.size_rate_damping;
        merge.angle_z = info.angle_z;
        merge.angular_rate_z = info.angular_rate_z;
        merge.angular_damping = info.angular_damping;
        merge.alpha_key = info.alpha_key;
        merge.color_key = info.color_key;
        merge.color_scale = info.color_scale;
        let offset = self.systems[slave].info.slave_pos_offset;
        merge.pos.x += offset.x;
        merge.pos.y += offset.y;
        merge.pos.z += offset.z;
        merge
    }

    /// `ParticleSystemManager::update()` at logic frame `logic_frame` and client frame `client_frame`: once per logic
    /// frame, each system's update in list order (systems made during the pass are updated in it too); a system whose
    /// update is false is deleted.
    pub fn update<H: ParticleHost>(&mut self, host: &H, random: &mut RandomValues, settings: &ParticleSettings, logic_frame: u32, client_frame: u32) {
        if self.last_logic_frame_update == Some(logic_frame) {
            return;
        }
        self.last_logic_frame_update = Some(logic_frame);
        let mut i = 0;
        while i < self.systems.len() {
            let id = self.systems[i].id;
            let alive = self.update_system(i, host, random, settings, client_frame);
            let i_now = self.index_of(id).expect("a system is only removed here");
            if alive {
                i = i_now + 1;
            } else {
                self.delete_system(i_now);
                i = i_now;
            }
        }
    }

    /// `ParticleSystem::update(localPlayerIndex)` of system index `si`.
    fn update_system<H: ParticleHost>(&mut self, si: usize, host: &H, random: &mut RandomValues, settings: &ParticleSettings, frame: u32) -> bool {
        if !settings.use_fx {
            return false;
        }
        let id = self.systems[si].id;
        {
            let s = &mut self.systems[si];
            if s.delay_left != 0 {
                s.delay_left -= 1;
                if s.delay_left == 0 {
                    s.start_timestamp = frame;
                }
                return true;
            }
            if s.info.wind_motion != wind_motion::WIND_MOTION_NOT_USED {
                s.update_wind_motion(random);
            }
        }
        let mut parent_xfrm: Option<Matrix3D> = None;
        let mut is_shrouded = false;
        let mut attach_destroyed = false;
        {
            let s = &mut self.systems[si];
            if s.attached_to_drawable != 0 {
                match host.drawable(s.attached_to_drawable) {
                    Some(a) => {
                        if a.shrouded {
                            is_shrouded = true;
                        }
                        parent_xfrm = Some(a.transform);
                        s.last_pos = s.pos;
                        s.pos = a.position;
                    }
                    None => {
                        s.attached_to_drawable = 0;
                        attach_destroyed = true;
                    }
                }
            } else if s.attached_to_object != 0 {
                match host.object(s.attached_to_object, self.local_player_index) {
                    Some(a) => {
                        if !is_shrouded {
                            is_shrouded = a.shrouded;
                        }
                        parent_xfrm = Some(a.transform);
                        s.last_pos = s.pos;
                        s.pos = a.position;
                    }
                    None => {
                        s.attached_to_object = 0;
                        attach_destroyed = true;
                    }
                }
            }
        }
        if attach_destroyed {
            self.destroy(id);
        }
        // the control particle's position
        let control_pos = self.systems[si].control_particle.and_then(|(cs, personality)| {
            self.index_of(cs).and_then(|ci| self.systems[ci].particles.iter().find(|p| p.personality == personality)).map(|p| p.get_position())
        });
        {
            let s = &mut self.systems[si];
            let mut transform_set = false;
            if let Some(px) = parent_xfrm {
                if s.skip_parent_xfrm {
                    s.transform = s.local_transform;
                } else if !s.is_local_identity {
                    let mut m = Matrix3D::IDENTITY;
                    m.mul(&px, &s.local_transform);
                    s.transform = m;
                } else {
                    s.transform = px;
                }
                s.is_identity = false;
                transform_set = true;
            }
            if !transform_set {
                if !s.is_local_identity {
                    s.transform = s.local_transform;
                    s.is_identity = false;
                } else {
                    s.is_identity = true;
                }
            }
            if let Some(cp) = control_pos {
                s.transform.set_x_translation(cp.x);
                s.transform.set_y_translation(cp.y);
                s.transform.set_z_translation(cp.z);
                s.is_identity = false;
                s.last_pos = s.pos;
                s.pos = cp;
            }
        }

        // emission
        let s = &self.systems[si];
        let emitting = !s.is_destroyed && (s.is_forever || s.system_lifetime_left > 0) && !is_shrouded && !s.is_stopped && s.master.is_none();
        if emitting && s.burst_delay_left != 0 {
            self.systems[si].burst_delay_left -= 1;
        } else if emitting {
            let pri = s.info.priority;
            let count_coeff = s.count_coeff;
            let mut count = real_to_int(s.info.burst_count.get_value(random));
            count = (count as f32 * count_coeff) as i32;
            for i in 0..count {
                let si = self.index_of(id).expect("listed");
                let info = self.generate_particle_info(si, i, count, random, settings);
                if self.systems[si].info.is_emit_above_ground_only && info.pos.z < host.ground_height(info.pos.x, info.pos.y) {
                    continue;
                }
                let Some(k) = self.create_particle(si, &info, pri, false, settings, frame) else { continue };
                let personality = self.systems[si].particles[k].personality;
                let attached = self.systems[si].info.attached_system_name.clone();
                if !attached.is_empty() && self.templates.contains_key(&attached) {
                    let sys = self.create_particle_system(&attached, true, random, frame).expect("the template");
                    let ci = self.index_of(sys).expect("made");
                    self.systems[ci].control_particle = Some((id, personality));
                    let si = self.index_of(id).expect("listed");
                    if let Some(p) = self.systems[si].particles.iter_mut().find(|p| p.personality == personality) {
                        p.system_under_control = Some(sys);
                    }
                }
                let si = self.index_of(id).expect("listed");
                if let Some(slave) = self.systems[si].slave.and_then(|sl| self.index_of(sl)) {
                    let merge = self.merge_related_particle_systems(si, slave, random, settings);
                    self.create_particle(slave, &merge, pri, false, settings, frame);
                }
            }
            let si = self.index_of(id).expect("listed");
            let s = &mut self.systems[si];
            s.burst_delay_left = s.info.burst_delay.get_value(random) as u32;
            s.burst_delay_left = (s.burst_delay_left as f32 * s.delay_coeff) as u32;
        }

        // the particles
        let si = self.index_of(id).expect("listed");
        let mut system_pos = self.systems[si].get_position();
        if self.systems[si].info.wind_motion != wind_motion::WIND_MOTION_NOT_USED {
            let s = &self.systems[si];
            let at = if s.attached_to_object != 0 {
                host.object(s.attached_to_object, self.local_player_index)
            } else if s.attached_to_drawable != 0 {
                host.drawable(s.attached_to_drawable)
            } else {
                None
            };
            if let Some(a) = at {
                system_pos.x += a.position.x;
                system_pos.y += a.position.y;
                system_pos.z += a.position.z;
            }
        }
        let mut k = 0;
        while k < self.systems[si].particles.len() {
            let ParticleSystem { particles, info, .. } = &mut self.systems[si];
            let p = &mut particles[k];
            if info.gravity != 0.0 {
                p.apply_force(&Coord3D { x: 0.0, y: 0.0, z: info.gravity });
            }
            if p.update(info, system_pos, frame) {
                k += 1;
            } else {
                self.delete_particle(si, k);
            }
        }

        let s = &mut self.systems[si];
        if s.is_destroyed && s.particles.is_empty() {
            return false;
        }
        if !s.is_forever {
            if s.system_lifetime_left != 0 {
                s.system_lifetime_left -= 1;
            }
            if s.get_particle_count() != 0 {
                return true;
            }
            if s.system_lifetime_left == 0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INI: &[u8] = b"ParticleSystem Smoke\r
  Priority = WEAPON_TRAIL\r
  IsOneShot = No\r
  Shader = ALPHA\r
  Type = PARTICLE\r
  ParticleName = EXSmokNew1.tga\r
  AngleZ = 0.00 6.28\r
  AngularRateZ = 0.00 0.00\r
  AngularDamping = 1.00 1.00\r
  VelocityDamping = 0.95 0.95\r
  Gravity = -0.01\r
  SlaveSystem = \r
  SlavePosOffset = X:0.00 Y:0.00 Z:0.00\r
  PerParticleAttachedSystem = \r
  Lifetime = 30.00 30.00\r
  SystemLifetime = 5\r
  Size = 2.00 4.00\r
  StartSizeRate = 0.00 0.00\r
  SizeRate = 0.50 0.50\r
  SizeRateDamping = 1.00 1.00\r
  Alpha1 = 0.00 0.00 0\r
  Alpha2 = 1.00 1.00 10\r
  Alpha3 = 0.00 0.00 30\r
  Color1 = R:255 G:128 B:0 0\r
  Color2 = R:0 G:0 B:0 30\r
  ColorScale = 0.00 0.00\r
  BurstDelay = 2.00 2.00\r
  BurstCount = 3.00 3.00\r
  InitialDelay = 0.00 0.00\r
  DriftVelocity = X:0.00 Y:0.00 Z:0.50\r
  VelocityType = SPHERICAL\r
  VelOrthoX = 7.00 7.00\r
  VelOrthoY = 8.00 8.00\r
  VelOrthoZ = 9.00 9.00\r
  VelSpherical = 1.00 1.00\r
  VolumeType = BOX\r
  VolBoxHalfSize = X:1.00 Y:2.00 Z:3.00\r
  VolLineEnd = X:4.00 Y:5.00 Z:6.00\r
  VolSphereRadius = 10.00\r
  IsHollow = No\r
  IsGroundAligned = No\r
  IsEmitAboveGroundOnly = No\r
  IsParticleUpTowardsEmitter = No\r
  WindMotion = Unused\r
  WindAngleChangeMin = 0.15\r
  WindAngleChangeMax = 0.45\r
  WindPingPongStartAngleMin = 0.00\r
  WindPingPongStartAngleMax = 0.785\r
  WindPingPongEndAngleMin = 5.49\r
  WindPingPongEndAngleMax = 6.28\r
End\r
";

    fn loaded() -> ParticleSystemManager {
        let mut m = ParticleSystemManager::new();
        m.load_text(b"Data\\INI\\ParticleSystem.ini", INI).expect("parses");
        m
    }

    /// *Predictions, written before the first run:* one template `Smoke`: priority `WEAPON_TRAIL` (10), shader
    /// `ALPHA` (2), type `PARTICLE` (1), its texture; `SlaveSystem = ` empty (an empty string, not an error); the
    /// velocity union's first variable is `VelSpherical`'s 1..1 (read after `VelOrthoX`'s 7), the second `VelOrthoY`'s
    /// 8, the third 9; the volume union is `[10, 2, 3, 4, 5, 6]` (`VolSphereRadius` over the box's x); `Color1` red 1.0,
    /// green 128/255; `Alpha2` 1..1 at frame 10; keys 4..8 zero; `WindMotion = Unused` is 1 (`WIND_MOTION_NOT_USED`);
    /// the ping-pong end minimum 5.49.
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn a_particle_system_template_parses_with_its_unions() {
        let m = loaded();
        assert_eq!(m.templates.len(), 1);
        let t = m.find_template(b"Smoke").unwrap();
        let i = &t.info;
        assert_eq!((i.priority, i.shader_type, i.particle_type), (priority::WEAPON_TRAIL, shader_type::ALPHA, particle_type::PARTICLE));
        assert_eq!(i.particle_type_name, b"EXSmokNew1.tga");
        assert!(i.slave_system_name.is_empty() && i.attached_system_name.is_empty());
        assert_eq!((i.emission_velocity[0].low, i.emission_velocity[1].low, i.emission_velocity[2].low), (1.0, 8.0, 9.0));
        assert_eq!(i.emission_volume, [10.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(i.color_key[0].color, RGBColor { red: 1.0, green: 128.0 / 255.0, blue: 0.0 });
        assert_eq!((i.alpha_key[1].var.low, i.alpha_key[1].var.high, i.alpha_key[1].frame), (1.0, 1.0, 10));
        assert!(i.alpha_key[3..].iter().all(|k| *k == RandomKeyframe::default()));
        assert_eq!(i.wind_motion, wind_motion::WIND_MOTION_NOT_USED);
        assert_eq!(i.wind_motion_end_angle_min, 5.49);
        assert_eq!((i.system_lifetime, i.gravity, i.drift_velocity.z), (5, -0.01, 0.5));
        assert!(m.find_template(b"smoke").is_none(), "names are exact");
    }

    fn settings() -> ParticleSettings {
        ParticleSettings { max_particle_count: 100, ..ParticleSettings::default() }
    }

    /// *Predictions, written before the first run:* a `Smoke` system made at frame 0 draws three client randoms
    /// (wind start, end, angle; its initial delay is a constant 0, no draw). The first update emits a burst of 3
    /// (burst delay 2 then); updates 2 and 3 count the delay down (2, 1, 0... emitting again on the 4th: 6
    /// particles); the system lifetime is 5, so after the 5th update it emits no more and waits for its particles.
    /// Each particle starts at alpha 0 (key 1) and climbs 0.1 a frame toward key 2 (1.0 at frame 10); its size starts
    /// in 2..4 and grows 0.5 a frame; it dies when 30 frames have passed (lifetime 30) or its alpha drops under 0.02.
    /// With an ALPHA shader the first particle dies on its first update? No: its alpha is 0 + 0.1 = 0.1 after the
    /// first update. The particles rise by the drift (0.5) plus gravity's pull. With no host attachment the system is
    /// at the identity, so particles start in the box of half size (10, 2, 3) (the union: the radius is the box's x).
    /// The run is the same twice from the same seed. ~~The first burst dies at frame 31, the second at 34.~~
    ///
    /// *Results (2026-11-06):* the bursts and the system's end held; the deaths missed by one frame: a particle is
    /// updated in the update that makes it (emission comes before the particle loop), so its 30 frames run out at
    /// frame 30 and 33. Counts by frame: 3, 3, 3, 6 (to frame 29), 3 (30..32), 0 from 33.
    #[test]
    fn a_system_emits_bursts_and_its_particles_age() {
        let run = || {
            let mut m = loaded();
            let mut r = RandomValues::new(7);
            let id = m.create_particle_system(b"Smoke", true, &mut r, 0).unwrap();
            assert_eq!(id, 1);
            let mut counts = Vec::new();
            for f in 1..=40u32 {
                m.update(&NoParticleHost, &mut r, &settings(), f, f);
                counts.push(m.particle_count);
            }
            (m, counts)
        };
        let (m, counts) = run();
        let (_, again) = run();
        assert_eq!(counts, again, "deterministic");
        assert_eq!(&counts[..6], &[3, 3, 3, 6, 6, 6]);
        // lifetime 30, counted from the update that made them: the first burst (frame 1) dies at frame 30, the
        // second (frame 4) at 33
        assert_eq!(counts[28], 6);
        assert_eq!(counts[29], 3);
        assert_eq!(counts[32], 0);
        assert_eq!(m.get_particle_system_count(), 0, "the system goes when its lifetime and particles are done");
    }

    /// *Predictions, written before the first run:* after one update, each of the 3 particles has alpha 0.1 (0 + rate
    /// 1/10), size in [2.5, 4.5] (start + 0.5), sits within the box (|x| <= 10, |y| <= 2, |z| <= 3 before moving) plus
    /// one step of velocity (speed 1 spherical, damped 0.95) and the drift 0.5 + gravity -0.01 on z; red 1 - 1/30 and
    /// green 128/255 - (128/255)/30 (the colour moves to black over 30 frames).
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn a_particle_s_first_update_follows_its_keys() {
        let mut m = loaded();
        let mut r = RandomValues::new(3);
        m.create_particle_system(b"Smoke", true, &mut r, 0).unwrap();
        m.update(&NoParticleHost, &mut r, &settings(), 1, 1);
        let s = &m.systems[0];
        assert_eq!(s.particles.len(), 3);
        for p in &s.particles {
            assert!((p.alpha - 0.1).abs() < 1e-6, "{}", p.alpha);
            assert!(p.info.size >= 2.5 && p.info.size <= 4.5);
            let q = p.info.pos;
            assert!(q.x.abs() <= 10.0 + 1.0 && q.y.abs() <= 2.0 + 1.0 && q.z.abs() <= 3.0 + 1.0 + 0.5);
            assert!((p.color.red - (1.0 - 1.0 / 30.0)).abs() < 1e-6);
            let g = 128.0f32 / 255.0;
            assert!((p.color.green - (g - g / 30.0)).abs() < 1e-6);
        }
        let personalities: Vec<u32> = s.particles.iter().map(|p| p.personality).collect();
        assert_eq!(personalities, [0, 1, 2]);
    }

    /// *Predictions, written before the first run:* with `MaxParticleCount` 4, the first burst makes 3. In the
    /// second, the first particle sees an excess of -1 and is made (4); the second an excess of 0, made (5); the third
    /// an excess of 1: `removeOldestParticles(1, WEAPON_TRAIL)` finds nothing below its own priority and answers 2
    /// (see its docs), so it is not made. 5. (A draft of this prediction said 4; it was revised on reading
    /// `removeOldestParticles` again, before the run.) A second system of a lower priority (`Dust`, `DUST_TRAIL`)
    /// made earlier loses its oldest particle to an excess of 1, and the newcomer is still refused; ~~both of its
    /// particles go, and Smoke keeps 1~~.
    ///
    /// *Results (2026-11-06):* the 5 held. The two-system case missed: the removal brings the count back to the cap,
    /// so the third newcomer sees an excess of 0 and is made: Dust keeps 1, Smoke has 2, 3 in all.
    #[test]
    fn the_particle_cap_keeps_the_count() {
        let mut m = loaded();
        let mut r = RandomValues::new(3);
        m.create_particle_system(b"Smoke", true, &mut r, 0).unwrap();
        let s = ParticleSettings { max_particle_count: 4, ..ParticleSettings::default() };
        for f in 1..=4 {
            m.update(&NoParticleHost, &mut r, &s, f, f);
        }
        assert_eq!(m.particle_count, 5);

        let mut m = loaded();
        m.load_text(b"x.ini", b"ParticleSystem Dust\n  Priority = DUST_TRAIL\n  Shader = ALPHA_TEST\n  Lifetime = 100 100\n  BurstCount = 2 2\n  BurstDelay = 100 100\nEnd\n").unwrap();
        let mut r = RandomValues::new(3);
        let dust = m.create_particle_system(b"Dust", true, &mut r, 0).unwrap();
        let s2 = ParticleSettings { max_particle_count: 2, ..ParticleSettings::default() };
        m.update(&NoParticleHost, &mut r, &s2, 1, 1);
        assert_eq!(m.particle_count, 2);
        let smoke = m.create_particle_system(b"Smoke", true, &mut r, 1).unwrap();
        m.update(&NoParticleHost, &mut r, &s2, 2, 2);
        // Smoke's burst of 3: excess 0 (made, 3); excess 1: Dust's oldest removed (2), the newcomer refused; excess
        // 0 again: made (3)
        assert_eq!(m.find_particle_system(dust).unwrap().particles.len(), 1);
        assert_eq!(m.find_particle_system(smoke).unwrap().particles.len(), 2);
        assert_eq!(m.particle_count, 3);
    }
}
