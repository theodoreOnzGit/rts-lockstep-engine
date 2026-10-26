// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/NeutronMissileUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `NeutronMissileUpdate`: the flight of the superweapon's missile (the nuke, the SCUD). Launched by a weapon
//! (`projectileLaunchAtObjectOrPosition`), it waits for its next update, appears at its launcher's launch bone (turned on end), and from
//! then on accelerates along its nose, turning towards its target at no more than `MaxTurnRate` once it has flown
//! `DistanceToTravelBeforeTurning` (aiming first at a point `TargetFromDirectlyAbove` over the target, then straight down at half speed).
//! For `SpecialSpeedTime` frames after the launch its height follows a curve of its own and its drawable jitters. It detonates (is killed)
//! when it collides with anything but its launcher once armed, or when it is no longer above the terrain. A delivery decal marks the target.
//! It is an update module, a die module and the object's `ProjectileUpdateInterface`.
//!
//! # Upstream facts kept
//!
//! - A launch whose launcher is gone by the next update destroys the missile (its state stays `LAUNCH`).
//! - The no-turn distance is spent only by updates that started in `ATTACK`.
//! - `angleCoeff` ("the more we have to turn, the slower we go") is computed and never used.
//! - The missile's drawable is read unchecked by the launch and by the special-speed climb ([`Undefined::NullDrawable`]).
//! - `xfer` saves the exhaust template by name; the decal is not saved (`xferRadiusDecal`: a load clears it).
//!
//! `p1w13-sonnet`, batch R67, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::radius_decal::{RadiusDecal, RadiusDecalTemplate};
use crate::logic::weapon_host::WeaponHost;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `STRAIGHT_DOWN_SLOW_FACTOR`.
const STRAIGHT_DOWN_SLOW_FACTOR: f32 = 0.5;
/// `PI` (`Real`).
const PI: f32 = std::f32::consts::PI;
/// `TURRET_INVALID`.
const TURRET_INVALID: i32 = -1;

/// `enum MissileStateType`.
pub const PRELAUNCH: i32 = 0;
pub const LAUNCH: i32 = 1;
pub const ATTACK: i32 = 2;
pub const DEAD: i32 = 3;

/// An FX list by name (`INI::parseFXList`: `None` is `NULL`).
pub type Named = Option<Vec<u8>>;

/// `class NeutronMissileUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct NeutronMissileUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_initialDist`.
    pub initial_dist: f32,
    /// `m_maxTurnRate` (radians per frame once read; the default 999 is raw).
    pub max_turn_rate: f32,
    /// `m_forwardDamping`.
    pub forward_damping: f32,
    /// `m_relativeSpeed`.
    pub relative_speed: f32,
    /// `m_targetFromDirectlyAbove`: "aim first for dest+offset, then dest".
    pub target_from_directly_above: f32,
    /// `m_specialAccelFactor`, `m_specialSpeedTime`, `m_specialSpeedHeight`, `m_specialJitterDistance`.
    pub special_accel_factor: f32,
    pub special_speed_time: u32,
    pub special_speed_height: f32,
    pub special_jitter_distance: f32,
    /// `m_launchFX`, `m_ignitionFX`.
    pub launch_fx: Named,
    pub ignition_fx: Named,
    /// `m_deliveryDecalTemplate`, `m_deliveryDecalRadius`.
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: f32,
}

impl Default for NeutronMissileUpdateModuleData {
    /// `NeutronMissileUpdateModuleData()`.
    fn default() -> Self {
        NeutronMissileUpdateModuleData {
            update: UpdateModuleData::new(),
            initial_dist: 0.0,
            max_turn_rate: 999.0,
            forward_damping: 0.0,
            relative_speed: 1.0,
            target_from_directly_above: 0.0,
            special_accel_factor: 1.0,
            special_speed_time: 0,
            special_speed_height: 0.0,
            special_jitter_distance: 0.0,
            launch_fx: None,
            ignition_fx: None,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `NULL`.
fn parse_named(ini: &mut Ini<'_>) -> IniResult<Named> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

static FIELD_PARSE: [FieldParse<NeutronMissileUpdateModuleData>; 14] = [
    FieldParse::new("DistanceToTravelBeforeTurning", |i, d, _| { d.initial_dist = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxTurnRate", |i, d, _| { d.max_turn_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ForwardDamping", |i, d, _| { d.forward_damping = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RelativeSpeed", |i, d, _| { d.relative_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TargetFromDirectlyAbove", |i, d, _| { d.target_from_directly_above = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LaunchFX", |i, d, _| { d.launch_fx = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialSpeedTime", |i, d, _| { d.special_speed_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialSpeedHeight", |i, d, _| { d.special_speed_height = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialAccelFactor", |i, d, _| { d.special_accel_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialJitterDistance", |i, d, _| { d.special_jitter_distance = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("IgnitionFX", |i, d, _| { d.ignition_fx = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DeliveryDecal", |i, d, _| RadiusDecalTemplate::parse_radius_decal_template(i, &mut d.delivery_decal_template), UserData::None),
    FieldParse::new("DeliveryDecalRadius", |i, d, _| { d.delivery_decal_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl NeutronMissileUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, NeutronMissileUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<NeutronMissileUpdateModuleData> {
        let mut data = NeutronMissileUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, NeutronMissileUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `NeutronMissileUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::NeutronMissileUpdate(Arc::new(NeutronMissileUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class NeutronMissileUpdate : public UpdateModule, public DieModuleInterface, public ProjectileUpdateInterface`.
#[derive(Clone, Debug)]
pub struct NeutronMissileUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<NeutronMissileUpdateModuleData>,
    /// `m_state`.
    state: i32,
    /// `m_targetPos`, `m_intermedPos`.
    target_pos: Coord3D,
    intermed_pos: Coord3D,
    /// `m_launcherID`.
    launcher_id: ObjectId,
    /// `m_attach_wslot`, `m_attach_specificBarrelToUse`.
    attach_wslot: i32,
    attach_specific_barrel_to_use: i32,
    /// `m_accel`, `m_vel`.
    accel: Coord3D,
    vel: Coord3D,
    /// `m_stateTimestamp`.
    state_timestamp: u32,
    /// `m_isLaunched`, `m_isArmed`.
    is_launched: bool,
    is_armed: bool,
    /// `m_noTurnDistLeft`.
    no_turn_dist_left: f32,
    /// `m_reachedIntermediatePos`.
    reached_intermediate_pos: bool,
    /// `m_frameAtLaunch`, `m_heightAtLaunch`.
    frame_at_launch: u32,
    height_at_launch: f32,
    /// `m_deliveryDecal` (the client event's index is 0).
    delivery_decal: RadiusDecal,
    /// `m_exhaustSysTmpl`, by name (`None` for `NULL`).
    exhaust_sys_tmpl: Option<Vec<u8>>,
}

impl PartialEq for NeutronMissileUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
            && self.delivery_decal == other.delivery_decal
            && self.exhaust_sys_tmpl == other.exhaust_sys_tmpl
    }
}

fn c3(v: &Vector3) -> Coord3D {
    Coord3D { x: v.x, y: v.y, z: v.z }
}

/// `calcTransform(obj, pos, maxTurnRate, newTransform)`: the transform turned towards `pos` by at most `max_turn_rate`, and the angle turned.
fn calc_transform(transform: &Matrix3D, obj_pos: &Coord3D, pos: &Coord3D, max_turn_rate: f32) -> (f32, Matrix3D) {
    // convert to Vector3, to use all its handy stuff
    let obj_pos = Vector3::new(obj_pos.x, obj_pos.y, obj_pos.z);
    let other_pos = Vector3::new(pos.x, pos.y, pos.z);
    let obj_dir = transform.rotate_vector(&Vector3::new(1.0, 0.0, 0.0));
    let mut other_dir = other_pos - obj_pos;
    other_dir.normalize();
    // dot of two unit vectors is cos of angle between them
    // bound it in case of numerical error
    let c = Vector3::dot_product(&obj_dir, &other_dir).clamp(-1.0, 1.0);
    let mut angle = crate::common::trig::acos(c);
    let new_dir = if angle.abs() < max_turn_rate {
        // close enough -- point exactly in the right dir
        other_dir
    } else {
        // nah, turn as much as we can: around the axis perpendicular to these two vecs
        angle = max_turn_rate;
        let obj_cross_other = Vector3::normalized_cross_product(&obj_dir, &other_dir);
        let rot_mtx = Matrix3D::from_axis_angle(&obj_cross_other, angle);
        rot_mtx.rotate_vector(&obj_dir)
    };
    let mut new_transform = Matrix3D::IDENTITY;
    new_transform.build_transform_matrix(&obj_pos, &new_dir);
    (angle, new_transform)
}

impl NeutronMissileUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half (it touches no world): the base and the state (the timestamp is the world half's).
    pub fn unconstructed(object: ObjectId, data: Option<Arc<NeutronMissileUpdateModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        let no_turn_dist_left = data.initial_dist;
        Ok(NeutronMissileUpdate {
            update,
            data,
            state: PRELAUNCH,
            target_pos: Coord3D::default(),
            intermed_pos: Coord3D::default(),
            launcher_id: INVALID_ID,
            attach_wslot: 0,
            attach_specific_barrel_to_use: 0,
            accel: Coord3D::default(),
            vel: Coord3D::default(),
            state_timestamp: 0,
            is_launched: false,
            is_armed: false,
            no_turn_dist_left,
            reached_intermediate_pos: true,
            frame_at_launch: 0,
            height_at_launch: 0.0,
            delivery_decal: RadiusDecal::default(),
            exhaust_sys_tmpl: None,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<NeutronMissileUpdateModuleData> {
        &self.data
    }

    /// The state as words: the state, the target and intermediate positions, the launcher, the slot and barrel, the acceleration and the
    /// velocity, the timestamp, launched, armed, the no-turn distance, reached, the launch frame and height, and whether the decal is empty.
    pub fn state_words(&self) -> Vec<u32> {
        let b = |c: &Coord3D| [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()];
        let mut v = vec![self.state as u32];
        v.extend(b(&self.target_pos));
        v.extend(b(&self.intermed_pos));
        v.extend([self.launcher_id as u32, self.attach_wslot as u32, self.attach_specific_barrel_to_use as u32]);
        v.extend(b(&self.accel));
        v.extend(b(&self.vel));
        v.extend([
            self.state_timestamp,
            u32::from(self.is_launched),
            u32::from(self.is_armed),
            self.no_turn_dist_left.to_bits(),
            u32::from(self.reached_intermediate_pos),
            self.frame_at_launch,
            self.height_at_launch.to_bits(),
            u32::from(self.delivery_decal.is_empty()),
        ]);
        v
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
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

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    fn clear_decal(&mut self, world: &mut World, me: ObjectId) {
        self.delivery_decal.clear(world, me, 0);
    }

    /// `projectileLaunchAtObjectOrPosition(victim, victimPos, launcher, wslot, specificBarrelToUse, detWeap, exhaustSysOverride)`.
    #[allow(clippy::too_many_arguments)] // (upstream's seven parameters, with the world and the object)
    fn projectile_launch(&mut self, world: &mut World, me: ObjectId, victim: ObjectId, victim_pos: Coord3D, launcher: ObjectId, wslot: i32, barrel: i32, exhaust: Option<Vec<u8>>) {
        let launcher = world.objects.find(launcher).map(|_| launcher);
        self.launcher_id = launcher.unwrap_or(INVALID_ID);
        self.attach_wslot = wslot;
        self.attach_specific_barrel_to_use = barrel;
        self.vel = Coord3D::default();
        if let Some(l) = launcher {
            if let Some(phys) = world.objects.find(l).and_then(|o| o.physics) {
                if !crate::logic::modules::plant(2371) {
                    self.vel = world.module_call(l, phys, ModuleCall::PhysicsGetVelocity).as_coord();
                }
            }
        }
        self.projectile_fire(world, me, victim, victim_pos, exhaust);
    }

    /// `projectileFireAtObjectOrPosition(victim, victimPos, detWeap, exhaustSysOverride)`.
    fn projectile_fire(&mut self, world: &mut World, me: ObjectId, victim: ObjectId, victim_pos: Coord3D, exhaust: Option<Vec<u8>>) {
        self.exhaust_sys_tmpl = exhaust;
        self.state = LAUNCH;
        self.state_timestamp = world.frame();
        // "CalcTarget will add half the target's height. But in this case, we are aiming at the ground and need to stay aiming at the ground."
        self.target_pos = if world.objects.find(victim).is_some() { Self::position(world, victim) } else { victim_pos };
        self.intermed_pos = self.target_pos;
        self.intermed_pos.z += self.data.target_from_directly_above;
        self.clear_decal(world, me);
        let owner = world.get_controlling_player(me);
        let (t, radius, pos) = (self.data.delivery_decal_template.clone(), self.data.delivery_decal_radius, self.target_pos);
        t.create_radius_decal(world, (me, 0), pos, radius, owner, &mut self.delivery_decal);
    }

    /// `doLaunch()`: "Implement LAUNCH state".
    fn do_launch(&mut self, world: &mut World, me: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        if !self.is_launched {
            // "if our launch vehicle is gone, destroy ourselves"
            let Some(launcher) = world.objects.find(self.launcher_id).map(|_| self.launcher_id) else {
                self.launcher_id = INVALID_ID;
                world.body_destroy_object(me);
                return Ok(());
            };
            let attach = match WeaponHost::projectile_launch_offset(world, launcher, self.attach_wslot, self.attach_specific_barrel_to_use, TURRET_INVALID) {
                Some(o) => o.attach_transform,
                None => Matrix3D::IDENTITY, // ("ProjectileLaunchPos not found!")
            };
            let mut world_transform = WeaponHost::convert_bone_transform_to_world(world, launcher, &attach);
            let tmp = world_transform.get_translation();
            let world_pos = c3(&tmp);
            // "the missile on the raising up launch platform is actually 45 degrees from the missile that is flying around the world ... we
            // want to rotate it 'on end and in place'"
            world_transform.rotate_x(PI / 2.0);
            if !world.has_drawable(me) {
                return Err(Undefined::NullDrawable);
            }
            world.emit(ObjectClientEvent::DrawableSetHidden { object_id: me, hidden: false });
            world.set_transform_matrix(me, &world_transform);
            world.set_position(me, &world_pos);
            world.contain_experience_set_sink(me, self.launcher_id);
            self.is_launched = true;
            if d.target_from_directly_above != 0.0 && !crate::logic::modules::plant(2372) {
                self.reached_intermediate_pos = false;
            }
            if let Some(fx) = d.launch_fx.clone() {
                world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: me });
            }
            self.height_at_launch = Self::position(world, me).z;
            self.frame_at_launch = world.frame();
        }
        // fall
        let mut pos = Self::position(world, me);
        pos.x += self.vel.x;
        pos.y += self.vel.y;
        pos.z += self.vel.z;
        world.set_position(me, &pos);
        if let Some(fx) = d.ignition_fx.clone() {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: me });
        }
        if let Some(t) = self.exhaust_sys_tmpl.clone() {
            world.emit(ObjectClientEvent::ParticleSystemCreateAttached { template: t, object_id: me });
        }
        self.state = ATTACK;
        self.state_timestamp = world.frame();
        // arm the missile's "warhead"
        self.is_armed = true;
        Ok(())
    }

    /// `doAttack()`: "Implement ATTACK state".
    fn do_attack(&mut self, world: &mut World, me: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let mut speed = d.relative_speed;
        if d.target_from_directly_above != 0.0 && self.reached_intermediate_pos {
            speed *= STRAIGHT_DOWN_SLOW_FACTOR;
        }
        let transform = world.objects.find(me).map(|o| *o.thing.transform_matrix()).unwrap_or(Matrix3D::IDENTITY);
        // "if we're still in the no-turning-time, OR if we're out of fuel"
        let mx = if self.no_turn_dist_left > 0.0 {
            transform
        } else {
            // "Orient toward destination" (the angle's speed coefficient is computed and never used)
            let goal = if self.reached_intermediate_pos { self.target_pos } else { self.intermed_pos };
            let rate = if crate::logic::modules::plant(2373) { 999.0 } else { d.max_turn_rate };
            calc_transform(&transform, &Self::position(world, me), &goal, rate).1
        };
        // "get true forward direction of missile"
        let mut true_dir = mx.get_x_vector();
        true_dir.normalize();
        // "Move forward along forward direction"
        let damping = d.forward_damping;
        self.accel.x = speed * true_dir.x - damping * self.vel.x;
        self.accel.y = speed * true_dir.y - damping * self.vel.y;
        self.accel.z = speed * true_dir.z - damping * self.vel.z;
        self.vel.x += self.accel.x;
        self.vel.y += self.accel.y;
        self.vel.z += self.accel.z;
        let mut pos = Self::position(world, me);
        let now = world.frame();
        if d.special_speed_time > 0 && now <= self.frame_at_launch.wrapping_add(d.special_speed_time) {
            if !world.has_drawable(me) {
                return Err(Undefined::NullDrawable);
            }
            world.emit(ObjectClientEvent::DrawableSetInstanceMatrix { object_id: me, matrix: None });
            let elapsed = now.wrapping_sub(self.frame_at_launch);
            if elapsed < d.special_speed_time {
                let time_frac = elapsed as f32 / d.special_speed_time as f32;
                let mut accel_factor = d.special_accel_factor;
                if accel_factor < 0.01 {
                    accel_factor = 0.01;
                }
                let mut new_pos = pos;
                let s = accel_factor * time_frac;
                new_pos.z = self.height_at_launch + (s * s / accel_factor) * d.special_speed_height;
                self.vel.x = new_pos.x - pos.x;
                self.vel.y = new_pos.y - pos.y;
                self.vel.z = new_pos.z - pos.z;
                if d.special_jitter_distance > 0.0 && !crate::logic::modules::plant(2374) {
                    let amplitude = (1.0 - time_frac) * d.special_jitter_distance;
                    let y = world.random.game_logic_random_value_real(-1.0, 1.0) * amplitude;
                    let z = world.random.game_logic_random_value_real(-1.0, 1.0) * amplitude;
                    let vectmp = mx.rotate_vector(&Vector3::new(0.0, y, z));
                    let mut mtxtmp = Matrix3D::IDENTITY;
                    mtxtmp.translate_v(&vectmp);
                    world.emit(ObjectClientEvent::DrawableSetInstanceMatrix { object_id: me, matrix: Some(mtxtmp) });
                }
            }
        }
        pos.x += self.vel.x;
        pos.y += self.vel.y;
        pos.z += self.vel.z;
        world.set_transform_matrix(me, &mx);
        world.set_position(me, &pos);
        Ok(())
    }

    /// `detonate()`: killed (not destroyed) "so that its Die modules are called".
    fn detonate(&mut self, world: &mut World, me: ObjectId) {
        self.clear_decal(world, me);
        world.kill(me, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        self.state = DEAD;
        if world.has_drawable(me) {
            world.emit(ObjectClientEvent::DrawableSetHidden { object_id: me, hidden: true });
        }
    }

    /// `projectileHandleCollision(other)`.
    fn handle_collision(&mut self, world: &mut World, me: ObjectId, other: ObjectId) -> bool {
        // "check if our warhead is 'armed' - if not, we are inert"
        if !self.is_armed {
            return true;
        }
        // "Don't hit your own launcher, ever."
        if other != INVALID_ID && self.launcher_id == other && !crate::logic::modules::plant(2375) {
            return true;
        }
        // "collided with something... blow'd up!"
        self.detonate(world, me);
        // "mark ourself as 'no collisions' (since we might still exist in slow death mode)"
        world.set_status(me, mask_of(OBJECT_STATUS_NO_COLLISIONS), true);
        true
    }

    /// The body of `update()`.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<(), Undefined> {
        self.delivery_decal.update(world, me, 0);
        if !self.reached_intermediate_pos {
            let dist_sqr = world.partition_distance_squared_to_pos(me, &self.intermed_pos, DistanceCalculationType::FromCenter3D);
            let r = world.objects.find(me).map_or(0.0, |o| o.geometry_info.get_bounding_sphere_radius());
            if dist_sqr <= r * r {
                self.reached_intermediate_pos = true;
                world.set_position(me, &self.intermed_pos);
                let v = self.vel;
                let vel = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                self.vel = Coord3D { x: 0.0, y: 0.0, z: -vel * STRAIGHT_DOWN_SLOW_FACTOR };
            }
        }
        let old_pos = Self::position(world, me);
        let old_pos_valid = self.state == ATTACK; // "not valid till *after* we've launched"
        match self.state {
            LAUNCH => self.do_launch(world, me)?,
            ATTACK => self.do_attack(world, me)?,
            _ => {} // PRELAUNCH, DEAD: nothing
        }
        if self.no_turn_dist_left > 0.0 && old_pos_valid && !crate::logic::modules::plant(2376) {
            let new_pos = Self::position(world, me);
            let (dx, dy, dz) = (new_pos.x - old_pos.x, new_pos.y - old_pos.y, new_pos.z - old_pos.z);
            let dist_this_turn = (dx * dx + dy * dy + dz * dz).sqrt();
            self.no_turn_dist_left -= dist_this_turn;
        }
        if self.state != PRELAUNCH && self.state != DEAD && !world.is_above_terrain(me) {
            // "the normal always points straight down"
            let normal = Coord3D { x: 0.0, y: 0.0, z: -1.0 };
            let pos = Self::position(world, me);
            world.on_collide(me, INVALID_ID, Some(pos), Some(normal));
        }
        Ok(())
    }
}

impl UpdateBehavior for NeutronMissileUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// The constructor's world half: `m_stateTimestamp = TheGameLogic->getFrame()`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.state_timestamp = world.frame();
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut b = self.state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.state = i32::from_le_bytes(b);
        xfer.xfer_coord3d(&mut self.target_pos)?;
        xfer.xfer_coord3d(&mut self.intermed_pos)?;
        xfer.xfer_object_id(&mut self.launcher_id)?;
        let mut b = self.attach_wslot.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.attach_wslot = i32::from_le_bytes(b);
        xfer.xfer_int(&mut self.attach_specific_barrel_to_use)?;
        xfer.xfer_coord3d(&mut self.accel)?;
        xfer.xfer_coord3d(&mut self.vel)?;
        xfer.xfer_unsigned_int(&mut self.state_timestamp)?;
        xfer.xfer_bool(&mut self.is_launched)?;
        xfer.xfer_bool(&mut self.is_armed)?;
        if !crate::logic::modules::plant(2377) {
            xfer.xfer_real(&mut self.no_turn_dist_left)?;
        }
        xfer.xfer_bool(&mut self.reached_intermediate_pos)?;
        xfer.xfer_unsigned_int(&mut self.frame_at_launch)?;
        xfer.xfer_real(&mut self.height_at_launch)?;
        // "decal, if any" (nothing is saved; a load clears it)
        let loading = xfer.get_xfer_mode() == crate::common::xfer::XferMode::Load;
        self.delivery_decal.xfer_radius_decal(loading);
        // "particle system template", by name
        let mut name = self.exhaust_sys_tmpl.clone().unwrap_or_default();
        xfer.xfer_ascii_string(&mut name)?;
        if loading {
            // (the manager answers a template for any name: `findTemplate` never fails here)
            self.exhaust_sys_tmpl = (!name.is_empty()).then_some(name);
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::NeutronMissileUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::NeutronMissileUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasProjectileUpdateInterface => ModuleReply::Bool(true),
            ModuleCall::ProjectileGetLauncherId => ModuleReply::Id(self.launcher_id),
            ModuleCall::ProjectileLaunchAtObjectOrPosition { victim, destination, source, wslot, barrel, exhaust, .. } => {
                self.projectile_launch(world, me.object, victim, destination, source, wslot, barrel, exhaust);
                ModuleReply::Done
            }
            ModuleCall::ProjectileHandleCollision { other } => ModuleReply::Bool(self.handle_collision(world, me.object, other)),
            // `setFramesTillCountermeasureDiversionOccurs` and `projectileNowJammed` are empty
            ModuleCall::ProjectileSetFramesTillCountermeasureDiversion(_) | ModuleCall::ProjectileNowJammed => ModuleReply::Done,
            ModuleCall::OnDie(_) => {
                self.clear_decal(world, me.object);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for NeutronMissileUpdate {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = NeutronMissileUpdateModuleData::default();
        assert_eq!((d.max_turn_rate, d.relative_speed, d.special_accel_factor), (999.0, 1.0, 1.0));
        let m = NeutronMissileUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[0], PRELAUNCH as u32);
        let (angle, _) = calc_transform(&Matrix3D::IDENTITY, &Coord3D::default(), &Coord3D { x: 10.0, y: 0.0, z: 0.0 }, 0.1);
        assert_eq!(angle, 0.0);
    }
}
