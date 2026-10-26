// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WaveGuideUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WaveGuideUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WaveGuideUpdate`: the dam-burst wave. The object disables itself on its first update ("water waves objects are disabled by default
//! when they are created", which a constructor cannot do) and waits while disabled. Once enabled, after `WaveDelay` frames it starts:
//! its looping sound, a turn and a move to the first point of the waypoint path named `WaveGuide1` (whose last point is the wave's final
//! destination) and an order to its AI to follow the path; the wave front is a row of sample points across the object (`YSize` wide,
//! one every `LinearWaveSpacing`, bent back by `WaveBendMagnitude`), each with two spray systems (a third every fifth point). A path that
//! is no path (a node with more than one link, or no second node) destroys the wave.
//!
//! Every frame after that: a random splash sound twice a second (when a roll of 1-100 beats `RandomSplashSoundFrequency`), the sample
//! points carried to the world (on the ground); the end of the wave within `PATH_EXTRA_DISTANCE` of the destination (a splash, the wave
//! destroyed, the radar's terrain redrawn); else the sprays brought down to the ground where it is below `PreferredHeight`, the water
//! pushed at every point, the shore splashes every other frame (where the line `ShorelineEffectDistance` behind the front crosses the
//! shore) and the damage: every object within `DamageRadius` of a point, in front of it, not yet wet, not a wave guide or a bridge tower
//! and not above the water (a bridge may be), gets a splash, goes wet, topples, takes `DamageAmount` of water damage (`DEATH_FLOODED`),
//! shows flooded, and a bridge is replaced by a `WaterWaveBridge` along it, with the `BridgeParticle` across it.
//!
//! The client's parts are records: the particle systems ([`World::create_particle_system_unplaced`] and the position the cut points keep,
//! [`World::particle_system_find_position`]), the water ([`ObjectClientEvent::TerrainVisualAddWaterVelocity`]), the radar, the sounds and
//! the drawable. The bridges are `cuts.bridge_at` ([`World::terrain_find_bridge_at`]); the ground the terrain cut points'.
//!
//! # Upstream facts kept
//!
//! - The "angle" that decides that an object is in front of a point is the dot product of the direction to the object with the point's
//!   world position, not with the wave's heading.
//! - `deleteBridge` is called with the bridge found, `NULL` when there was none.
//! - The front's points are spaced in `Int`s: `halfY` and each step (`y += m_linearWaveSpacing`) are conversions to `Int`, undefined out of
//!   range ([`Undefined::FloatToIntOutOfRange`]); a spacing below 1 never moves `y`, and the row stops at `MAX_WAVEGUIDE_SHAPE_POINTS`.
//! - A `WaterWaveBridge` that is no template is `newObject(NULL)`'s `throw ERROR_BAD_ARG`.
//! - A waypoint path that loops never ends `startMoving`'s walk to its last node (the test makes none).
//!
//! [`ObjectClientEvent::TerrainVisualAddWaterVelocity`]: crate::logic::world::object_events::ObjectClientEvent::TerrainVisualAddWaterVelocity
//! [`Undefined::FloatToIntOutOfRange`]: crate::common::ini::Undefined::FloatToIntOutOfRange
//!
//! `p1w13-sonnet`, batch R70, 2026-10-26.

use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::disabled_types::DISABLED_DEFAULT;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::kind_of::{KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_WAVEGUIDE};
use crate::common::model_state::MODELCONDITION_FLOODED;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_WET;
use crate::logic::world::object_state::mask_of;
use crate::common::trig::{cos, sin};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_WATER};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::auto_deposit_update::real_to_int;
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::production_exit::transform_point;
use crate::logic::modules::topple_update::{TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::object_creation_list::DEATH_FLOODED;
use crate::logic::weapon::PATHFIND_CELL_SIZE_F;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `#define PATH_EXTRA_DISTANCE (10 * PATHFIND_CELL_SIZE_F)`.
pub const PATH_EXTRA_DISTANCE: f32 = 10.0 * PATHFIND_CELL_SIZE_F;
/// `MAX_WAVEGUIDE_SHAPE_POINTS`.
pub const MAX_WAVEGUIDE_SHAPE_POINTS: usize = 64;
/// `MAX_SHAPE_EFFECTS`.
pub const MAX_SHAPE_EFFECTS: usize = 3;
/// `INVALID_PARTICLE_SYSTEM_ID`.
const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;

/// `class WaveGuideUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveGuideUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_waveDelay`: "delay in frames to start the wave from when we become enabled".
    pub wave_delay: f32,
    /// `m_ySize`.
    pub y_size: f32,
    /// `m_linearWaveSpacing`.
    pub linear_wave_spacing: f32,
    /// `m_waveBendMagnitude`: "for curved waveshape, larger # = more straight".
    pub wave_bend_magnitude: f32,
    /// `m_waterVelocity` (distance per frame).
    pub water_velocity: f32,
    /// `m_preferredHeight`: "our preferred water height after the wave passes".
    pub preferred_height: f32,
    /// `m_shorelineEffectDistance`.
    pub shoreline_effect_distance: f32,
    /// `m_damageRadius`.
    pub damage_radius: f32,
    /// `m_damageAmount`.
    pub damage_amount: f32,
    /// `m_toppleForce`.
    pub topple_force: f32,
    /// `m_randomSplashSound`.
    pub random_splash_sound: AudioEventName,
    /// `m_randomSplashSoundFrequency`.
    pub random_splash_sound_frequency: i32,
    /// `m_bridgeParticle`: the template's name, `None` for `NULL`.
    pub bridge_particle: Option<Vec<u8>>,
    /// `m_bridgeParticleAngleFudge`.
    pub bridge_particle_angle_fudge: f32,
    /// `m_loopingSound`.
    pub looping_sound: AudioEventName,
}

/// `INI::parseParticleSystemTemplate`: `None` is `NULL`; any other name is looked up when it is used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `WaveGuideUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<WaveGuideUpdateModuleData>; 16] = [
    FieldParse::new("WaveDelay", |ini, d, _| { d.wave_delay = ini.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("YSize", |ini, d, _| { d.y_size = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LinearWaveSpacing", |ini, d, _| { d.linear_wave_spacing = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("WaveBendMagnitude", |ini, d, _| { d.wave_bend_magnitude = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("WaterVelocity", |ini, d, _| { d.water_velocity = ini.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PreferredHeight", |ini, d, _| { d.preferred_height = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShorelineEffectDistance", |ini, d, _| { d.shoreline_effect_distance = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DamageRadius", |ini, d, _| { d.damage_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DamageAmount", |ini, d, _| { d.damage_amount = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ToppleForce", |ini, d, _| { d.topple_force = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RandomSplashSound", |ini, d, _| d.random_splash_sound.parse(ini), UserData::None),
    FieldParse::new("RandomSplashSoundFrequency", |ini, d, _| { d.random_splash_sound_frequency = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeParticle", |ini, d, _| { d.bridge_particle = parse_particle_system(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeParticleAngleFudge", |ini, d, _| { d.bridge_particle_angle_fudge = ini.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LoopingSound", |ini, d, _| d.looping_sound.parse(ini), UserData::None),
    FieldParse::end(),
];

impl WaveGuideUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, WaveGuideUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults (all zero, no sounds, no bridge particle), filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<WaveGuideUpdateModuleData> {
        let mut data = WaveGuideUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, WaveGuideUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class WaveGuideUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct WaveGuideUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getWaveGuideUpdateModuleData()`.
    data: Arc<WaveGuideUpdateModuleData>,
    /// `m_activeFrame`: "frame we became active on".
    active_frame: u32,
    /// `m_needDisable`.
    need_disable: bool,
    /// `m_initialized`: "set to TRUE after we're enabled and in motion".
    initialized: bool,
    /// `m_shapePoints`: the front's sample points, in the object's frame.
    shape_points: [Coord3D; MAX_WAVEGUIDE_SHAPE_POINTS],
    /// `m_transformedShapePoints`: the same in the world, on the ground.
    transformed_shape_points: [Coord3D; MAX_WAVEGUIDE_SHAPE_POINTS],
    /// `m_shapeEffects`: the particle systems on the front.
    shape_effects: [[u32; MAX_SHAPE_EFFECTS]; MAX_WAVEGUIDE_SHAPE_POINTS],
    /// `m_shapePointCount`.
    shape_point_count: i32,
    /// `m_splashSoundFrame`: "frame we last tried to play a splash sound on".
    splash_sound_frame: u32,
    /// `m_finalDestination`.
    final_destination: Coord3D,
}

impl PartialEq for WaveGuideUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

/// The bits of a point, for the state the tests compare and for `xferUser`'s raw bytes.
fn point_words(p: &Coord3D) -> [u32; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

impl WaveGuideUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data), to be disabled, nothing begun.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<WaveGuideUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(WaveGuideUpdate {
            update,
            data,
            active_frame: 0,
            need_disable: true,
            initialized: false,
            shape_points: [Coord3D::default(); MAX_WAVEGUIDE_SHAPE_POINTS],
            transformed_shape_points: [Coord3D::default(); MAX_WAVEGUIDE_SHAPE_POINTS],
            shape_effects: [[INVALID_PARTICLE_SYSTEM_ID; MAX_SHAPE_EFFECTS]; MAX_WAVEGUIDE_SHAPE_POINTS],
            shape_point_count: 0,
            splash_sound_frame: 0,
            final_destination: Coord3D::default(),
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getWaveGuideUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<WaveGuideUpdateModuleData> {
        &self.data
    }

    /// The state as words, for the tests that compare it with upstream's: the active frame, the two flags, the point count, the splash
    /// frame, the destination, then every point, transformed point and effect id.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.active_frame, u32::from(self.need_disable), u32::from(self.initialized), self.shape_point_count as u32, self.splash_sound_frame];
        v.extend(point_words(&self.final_destination));
        for p in self.shape_points.iter().chain(self.transformed_shape_points.iter()) {
            v.extend(point_words(p));
        }
        v.extend(self.shape_effects.iter().flatten().copied());
        v
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: nothing.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `TheParticleSystemManager->createParticleSystem(findTemplate(name))`: the system's id, `None` for no template.
    fn create_system(world: &mut World, template: Option<&[u8]>) -> Option<u32> {
        template.map(|t| world.create_particle_system_unplaced(t.to_vec()))
    }

    /// `startMoving()`: "start the waveguide moving along its waypoint path, bringing water destruction and havok upon the land"; false
    /// for a path the wave cannot follow.
    fn start_moving(&mut self, world: &mut World, me: ObjectId) -> bool {
        let d = self.data.clone();
        world.audio_add_event(d.looping_sound.event_name.clone(), me);
        let name = crate::common::ascii_string::AsciiString::try_from_bytes(b"WaveGuide1").unwrap_or_default();
        let Some(waypoint) = world.terrain.waypoint_by_name(&name) else { return true };
        let node = |world: &World, h| world.terrain.waypoint(h).cloned().expect("a handle the terrain gave");
        // "verify that the waypoint path does not have any branches, also record the last waypoint as our final destination"
        let mut verify = Some(waypoint);
        while let Some(h) = verify {
            let w = node(world, h);
            if w.num_links() > 1 && !crate::logic::modules::plant(2401) {
                // ("The waypoint path cannot have multiple link choices at any node")
                return false;
            }
            self.final_destination = *w.location();
            verify = w.link(0).unwrap_or(None);
        }
        let first = node(world, waypoint);
        let Some(next) = first.link(0).unwrap_or(None) else {
            // ("There must be a linked waypoint path to follow")
            return false;
        };
        let next = node(world, next);
        // "turn the guide to the first waypoint direction"
        let v = Coord2D { x: next.location().x - first.location().x, y: next.location().y - first.location().y };
        let angle = v.to_angle();
        world.set_orientation(me, angle);
        if let Some(ai) = world.objects.find(me).and_then(|o| o.ai) {
            // "set the wave position to the first waypoint location", on the ground
            let (x, y) = (first.location().x, first.location().y);
            let pos = Coord3D { x, y, z: world.terrain_get_ground_height(x, y) };
            world.set_position(me, &pos);
            world.module_call(me, ai, ModuleCall::AiFollowWaypointPath { waypoint: first.id(), command_source: CMD_FROM_AI });
            world.module_call(me, ai, ModuleCall::AiSetPathExtraDistance(PATH_EXTRA_DISTANCE));
        }
        true
    }

    /// `initWaveGuide()`: "the wave guide has started moving ... this is called once".
    fn init_wave_guide(&mut self, world: &mut World, me: ObjectId) -> Result<bool, IniError> {
        if !self.start_moving(world, me) {
            return Ok(false);
        }
        self.compute_wave_shape_points()?;
        for i in 0..self.shape_point_count as usize {
            for (j, name) in [b"WaveSpray01", b"WaveSpray02", b"WaveSpray03"].iter().enumerate() {
                if j == 2 && (i % 5 != 0 || crate::logic::modules::plant(2402)) {
                    continue;
                }
                if let Some(id) = Self::create_system(world, Some(&name[..])) {
                    world.particle_system_set_position(id, self.shape_points[i]);
                    world.body_particle_attach_to_object(id, me);
                    self.shape_effects[i][j] = id;
                }
            }
        }
        Ok(true)
    }

    /// `computeWaveShapePoints()`.
    fn compute_wave_shape_points(&mut self) -> Result<(), IniError> {
        let d = self.data.clone();
        let half_y = real_to_int(d.y_size / 2.0)?;
        let mut y = half_y.wrapping_neg();
        while y < half_y {
            if self.shape_point_count as usize >= MAX_WAVEGUIDE_SHAPE_POINTS {
                break;
            }
            let p = &mut self.shape_points[self.shape_point_count as usize];
            // `if( modData->m_waveBendMagnitude )`: any value but zero (a NaN too)
            p.x = if d.wave_bend_magnitude != 0.0 && !crate::logic::modules::plant(2403) { y.wrapping_mul(y).wrapping_neg() as f32 / d.wave_bend_magnitude } else { 0.0 };
            p.y = y as f32;
            p.z = 0.0;
            self.shape_point_count += 1;
            y = real_to_int(y as f32 + d.linear_wave_spacing)?;
        }
        Ok(())
    }

    /// `transformWaveShape()`: the sample points in the world, on the ground.
    fn transform_wave_shape(&mut self, world: &mut World, me: ObjectId) {
        for i in 0..self.shape_point_count as usize {
            let mut t = transform_point(world, me, &self.shape_points[i]);
            t.z = world.terrain_get_ground_height(t.x, t.y);
            self.transformed_shape_points[i] = t;
        }
    }

    /// `doShapeEffects()`: the front's sprays brought down where the ground is below the water.
    fn do_shape_effects(&self, world: &mut World) {
        let d = &self.data;
        for i in 0..self.shape_point_count as usize {
            for j in 0..MAX_SHAPE_EFFECTS {
                let id = self.shape_effects[i][j];
                if let Some(mut pos) = world.particle_system_find_position(id) {
                    if self.transformed_shape_points[i].z < d.preferred_height {
                        pos.z = self.transformed_shape_points[i].z;
                        world.particle_system_set_position(id, pos);
                    }
                }
            }
        }
    }

    /// `doWaterMotion()`: "given all our sample points, make the wave go".
    fn do_water_motion(&self, world: &mut World) {
        let d = &self.data;
        for p in &self.transformed_shape_points[..self.shape_point_count as usize] {
            world.emit(ObjectClientEvent::TerrainVisualAddWaterVelocity { x: p.x, y: p.y, velocity: d.water_velocity, preferred_height: d.preferred_height });
        }
    }

    /// `doShoreEffects()`: the splashes where the line behind the front crosses the shore, every other frame.
    fn do_shore_effects(&self, world: &mut World, me: ObjectId) {
        if world.frame() & 0x1 != 0 && !crate::logic::modules::plant(2404) {
            return;
        }
        let d = self.data.clone();
        let n = self.shape_point_count as usize;
        let mut effect_points = [Coord3D::default(); MAX_WAVEGUIDE_SHAPE_POINTS];
        for (e, s) in effect_points.iter_mut().zip(&self.shape_points).take(n) {
            let p = Coord3D { x: s.x - d.shoreline_effect_distance, y: s.y, z: s.z };
            *e = transform_point(world, me, &p);
        }
        let mut under_water = true;
        for (i, &point) in effect_points.iter().enumerate().take(n) {
            let terrain_z = world.terrain_get_ground_height(point.x, point.y);
            if terrain_z > d.preferred_height {
                if under_water && i != 0 {
                    // "the prev point is actuall on the water so we'll use it"
                    if let Some(id) = Self::create_system(world, Some(b"WaveSplashRight01")) {
                        world.particle_system_set_position(id, effect_points[i - 1]);
                    }
                }
                under_water = false;
            } else {
                if !under_water && i != 0 {
                    if let Some(id) = Self::create_system(world, Some(b"WaveSplashLeft01")) {
                        world.particle_system_set_position(id, point);
                    }
                }
                under_water = true;
            }
        }
    }

    /// `doDamage()`: "do damage to things that have fallen victim in the path of this enourmous wave".
    fn do_damage(&self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        let d = self.data.clone();
        for i in 0..self.shape_point_count as usize {
            let point = self.transformed_shape_points[i];
            for obj in world.partition_iterate_objects_in_range_at(point, d.damage_radius, 0) {
                let Some(o) = world.objects.find(obj) else { continue };
                if o.is_kind_of(KINDOF_WAVEGUIDE) || o.is_kind_of(KINDOF_BRIDGE_TOWER) {
                    continue;
                }
                let is_bridge = o.is_kind_of(KINDOF_BRIDGE);
                let obj_pos = o.get_position();
                let wet = o.test_status(OBJECT_STATUS_WET);
                if obj_pos.z > d.preferred_height && !is_bridge {
                    continue;
                }
                // "forget Z, this is really a top down 2D calculation"
                let mut v = Coord3D { x: obj_pos.x - point.x, y: obj_pos.y - point.y, z: 0.0 };
                v.normalize();
                let angle = v.x * point.x + v.y * point.y + v.z * point.z;
                // `if( angle < 0 )` (a NaN is not in front)
                let in_front = angle < 0.0;
                if !in_front && !crate::logic::modules::plant(2405) {
                    continue;
                }
                if wet && !crate::logic::modules::plant(2406) {
                    continue;
                }
                if let Some(id) = Self::create_system(world, Some(b"WaveHit01")) {
                    let pos = Coord3D { x: self.shape_points[i].x, y: self.shape_points[i].y, z: obj_pos.z };
                    world.particle_system_set_position(id, pos);
                    world.body_particle_attach_to_object(id, me);
                }
                world.set_status(obj, mask_of(OBJECT_STATUS_WET), true);
                let topple = Coord3D { x: obj_pos.x - point.x, y: obj_pos.y - point.y, z: 0.0 };
                world.topple(obj, Some(topple), d.topple_force, TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX);
                let mut info = DamageInfo::default();
                info.input.damage_type = DAMAGE_WATER;
                info.input.death_type = DEATH_FLOODED;
                info.input.source_id = me;
                info.input.amount = if crate::logic::modules::plant(2407) { 0.0 } else { d.damage_amount };
                world.attempt_damage(obj, &mut info);
                if world.has_drawable(obj) {
                    world.set_model_condition_state(obj, MODELCONDITION_FLOODED);
                    world.emit(ObjectClientEvent::DrawableSetShadowsEnabled { object_id: obj, enabled: false });
                }
                if is_bridge {
                    self.replace_bridge(world, obj)?;
                }
            }
        }
        Ok(())
    }

    /// A bridge the wave hit: a `WaterWaveBridge` in its place, along it (the old bridge's direction), the bridge particle across it, and the
    /// old bridge deleted.
    fn replace_bridge(&self, world: &mut World, obj: ObjectId) -> Result<(), IniError> {
        let d = &self.data;
        let template = world.thing_factory_find_template(b"WaterWaveBridge").ok_or_else(|| IniError::new(IniErrorKind::BadArg))?;
        // (`newObject` throws on failure; the world's errors are not upstream's)
        let new_bridge = world.thing_factory_new_object(template, None, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
        let obj_pos = world.objects.find(obj).map(|o| o.get_position()).unwrap_or_default();
        let mut angle = 0.0;
        let old_bridge = world.terrain_find_bridge_at(obj_pos);
        if let Some((from, to)) = old_bridge {
            angle = Coord2D { x: to.x - from.x, y: to.y - from.y }.to_angle();
        }
        world.set_position(new_bridge, &obj_pos);
        world.set_orientation(new_bridge, if crate::logic::modules::plant(2408) { 0.0 } else { angle });
        if let Some(id) = Self::create_system(world, d.bridge_particle.as_deref()) {
            let z = Coord3D { x: 0.0, y: 0.0, z: 1.0 };
            let u = Coord3D { x: cos(angle + d.bridge_particle_angle_fudge), y: sin(angle + d.bridge_particle_angle_fudge), z: 0.0 };
            let y = Coord3D::cross_product(&z, &u);
            let x = Coord3D::cross_product(&y, &z);
            let transform = Matrix3D::new(x.x, y.x, z.x, obj_pos.x, x.y, y.y, z.y, obj_pos.y, x.z, y.z, z.z, obj_pos.z);
            world.particle_system_set_local_transform(id, &transform);
        }
        world.terrain_delete_bridge(old_bridge);
        Ok(())
    }

    /// The body of `update()`.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        if self.need_disable {
            self.need_disable = false;
            world.body_set_disabled(me, DISABLED_DEFAULT);
            return Ok(());
        }
        // "do nothing if we're disabled"
        if world.objects.find(me).is_some_and(|o| o.is_disabled()) && !crate::logic::modules::plant(2409) {
            return Ok(());
        }
        let d = self.data.clone();
        let frame = world.frame();
        // "mark the active frame if we haven't done so yet"
        if self.active_frame == 0 {
            self.active_frame = frame;
        }
        if (frame.wrapping_sub(self.active_frame) as f32) < d.wave_delay {
            return Ok(());
        }
        if !self.initialized {
            if !self.init_wave_guide(world, me)? {
                world.body_destroy_object(me);
                return Ok(());
            }
            self.initialized = true;
        }
        // "play random sounds every once in a while"
        if frame.wrapping_sub(self.splash_sound_frame) as f32 > LOGICFRAMES_PER_SECOND as f32 / 2.0 {
            self.splash_sound_frame = frame;
            if world.random.game_logic_random_value(1, 100) > d.random_splash_sound_frequency {
                world.audio_add_event(d.random_splash_sound.event_name.clone(), me);
            }
        }
        self.transform_wave_shape(world, me);
        // "check to see if we're at our final destination"
        let current = world.objects.find(me).map(|o| o.get_position()).unwrap_or_default();
        let dist_squared = PATH_EXTRA_DISTANCE * PATH_EXTRA_DISTANCE;
        let v = Coord2D { x: self.final_destination.x - current.x, y: self.final_destination.y - current.y };
        if v.x * v.x + v.y * v.y <= dist_squared && !crate::logic::modules::plant(2400) {
            if let Some(id) = Self::create_system(world, Some(b"WaveSplash01")) {
                let m = world.objects.find(me).map(|o| *o.thing.transform_matrix()).unwrap_or(Matrix3D::IDENTITY);
                world.particle_system_set_local_transform(id, &m);
            }
            world.body_destroy_object(me);
            world.emit(ObjectClientEvent::RadarRefreshTerrain);
            return Ok(());
        }
        self.do_shape_effects(world);
        self.do_water_motion(world);
        self.do_shore_effects(world, me);
        self.do_damage(world, me)
    }
}

impl UpdateBehavior for WaveGuideUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    /// The fields after the base: the active frame, the two flags, the points and the effects as raw bytes (`xferUser`), the count, the
    /// splash frame and the destination.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.active_frame)?;
        xfer.xfer_bool(&mut self.need_disable)?;
        xfer.xfer_bool(&mut self.initialized)?;
        for points in [&mut self.shape_points, &mut self.transformed_shape_points] {
            let mut bytes: Vec<u8> = points.iter().flat_map(point_words).flat_map(u32::to_le_bytes).collect();
            xfer.xfer_user(&mut bytes)?;
            for (p, c) in points.iter_mut().zip(bytes.chunks_exact(12)) {
                let w = |k: usize| f32::from_bits(u32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]]));
                *p = Coord3D { x: w(0), y: w(1), z: w(2) };
            }
        }
        let mut bytes: Vec<u8> = self.shape_effects.iter().flatten().flat_map(|id| id.to_le_bytes()).collect();
        xfer.xfer_user(&mut bytes)?;
        for (id, c) in self.shape_effects.iter_mut().flatten().zip(bytes.chunks_exact(4)) {
            *id = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        }
        if crate::logic::modules::plant(2410) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.shape_point_count)?;
        xfer.xfer_unsigned_int(&mut self.splash_sound_frame)?;
        xfer.xfer_coord3d(&mut self.final_destination)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::WaveGuideUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::WaveGuideUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for WaveGuideUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `WaveGuideUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::WaveGuideUpdate(Arc::new(WaveGuideUpdateModuleData::friend_new_module_data(ini)?)))
}
