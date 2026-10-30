// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PhysicsUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PhysicsBehavior`: the simple rigid body physics of an object (an update module in the physics phase, and a collide module). Each frame the
//! forces applied since the last (gravity, friction, the modules' forces) are integrated into the velocity and the velocity into the
//! transform; pitch, roll and yaw rates turn it; the ground stops it (a bounce where allowed); a landing collides with the ground, deals
//! falling damage from a steep enough fall and may kill an object resting on the ground. A collision with another object pushes the two
//! apart (or crushes the other, or crashes a vehicle into a structure).
//!
//! # Upstream facts kept
//!
//! - `m_turning` is not set by the constructor (`resetDynamicPhysics` and `setTurning` set it), and `m_originalAllowBounce` is never set at
//!   all: both are the zero the memory pool fills a new module with (`MemoryPool::allocateBlockImplementation` memsets the block), so
//!   `TURN_NONE` and `false`; a bounce that ends (`handleBounce` with no upward force) turns bouncing off. ~~Reading either was
//!   `Undefined::UninitialisedMember`~~ **CORRECTED 2026-10-25** (batch R50 as first committed excluded those cases; the pool's zero-fill
//!   is what the shipped game reads).
//! - `m_lastCollidee` is not saved; `m_velMag` is (a stale `-1` after a load means "recompute").
//! - The minimum fall speed for damage is `sqrt(|2 * gravity * height|)` of the height the INI gives (40 by default), with the gravity of
//!   `TheGlobalData` when the data was made: here, `cuts.globals.gravity` when it is used (the same while the global data does not change).
//! - The two crash weapons are found in the weapon store by name when used (upstream finds them when the data is made or read; a name the
//!   store does not have is `NULL`, and no weapon is fired).
//! - `applyRandomRotation`'s "random modifier" is `GameLogicRandomValue(-1.0f, 1.0f)`, the integer draw: -1, 0 or 1.
//! - A force with a NaN component is ignored; a NaN position after the integration destroys the object (and the update goes on).
//! - Collision forces apply to this object only; the other object's physics applies its own when it collides.
//! - A force from another module wakes the module (`SLEEPY_PHYSICS`), one from its own update does not.
//!
//! # Where upstream is undefined
//!
//! The two uninitialised members above; a crush test on an object with no body module (`getBodyModule()->getFrontCrushed()` unchecked:
//! `NullBodyModule`); a parachuting object bouncing off a structure with no physics in its container chain (`NullPhysics`); recorded in
//! `world.cuts.module_errors`.
//!
//! `p1w13-sonnet`, batch R50, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::{sqr, Coord3D};
use crate::common::disabled_types::{DISABLED_FREEFALL, DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_common::{LOGICFRAMES_PER_SECOND, SECONDS_PER_LOGICFRAME_REAL};
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_DRONE, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use crate::common::model_state::{MODELCONDITION_FREEFALL, MODELCONDITION_SPLATTED, MODELCONDITION_STUNNED, MODELCONDITION_STUNNED_FLAILING};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE};
use crate::common::object_status_types::{OBJECT_STATUS_BRAKING, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_PARACHUTING};
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_CRUSH, DAMAGE_FALLING, DAMAGE_UNRESISTABLE, DEATH_CRUSHED, DEATH_NORMAL, HUGE_DAMAGE_AMOUNT};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_damage::CrushSquishTestType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

const DEFAULT_MASS: f32 = 1.0;
const DEFAULT_SHOCK_YAW: f32 = 0.05;
const DEFAULT_SHOCK_PITCH: f32 = 0.025;
const DEFAULT_SHOCK_ROLL: f32 = 0.025;
const DEFAULT_FORWARD_FRICTION: f32 = 0.15;
const DEFAULT_LATERAL_FRICTION: f32 = 0.15;
const DEFAULT_Z_FRICTION: f32 = 0.8;
const DEFAULT_AERO_FRICTION: f32 = 0.0;
/// "Air friction used to be completely separate and unclamped, so this constant must be split."
const MIN_AERO_FRICTION: f32 = 0.00;
const MIN_NON_AERO_FRICTION: f32 = 0.01;
const MAX_FRICTION: f32 = 0.99;
const STUN_RELIEF_EPSILON: f32 = 0.5;
const MOTIVE_FRAMES: u32 = (LOGICFRAMES_PER_SECOND / 3) as u32;
const INVALID_VEL_MAG: f32 = -1.0;
/// `LOCOMOTORSET_TAXIING`.
const LOCOMOTORSET_TAXIING: i32 = 5;
/// `LOCOMOTORSURFACE_WATER`, `LOCOMOTORSURFACE_CLIFF`.
const LOCOMOTORSURFACE_WATER: u32 = 1 << 1;
const LOCOMOTORSURFACE_CLIFF: u32 = 1 << 2;
/// `TURN_NONE`.
const TURN_NONE: i32 = 0;

// `PhysicsFlagsType`
const STICK_TO_GROUND: i32 = 0x0001;
const ALLOW_BOUNCE: i32 = 0x0002;
const APPLY_FRICTION2D_WHEN_AIRBORNE: i32 = 0x0004;
const UPDATE_EVER_RUN: i32 = 0x0008;
const WAS_AIRBORNE_LAST_FRAME: i32 = 0x0010;
const ALLOW_COLLIDE_FORCE: i32 = 0x0020;
const ALLOW_TO_FALL: i32 = 0x0040;
const HAS_PITCHROLLYAW: i32 = 0x0080;
const IMMUNE_TO_FALLING_DAMAGE: i32 = 0x0100;
const IS_IN_FREEFALL: i32 = 0x0200;
const IS_IN_UPDATE: i32 = 0x0400;
const IS_STUNNED: i32 = 0x0800;

/// `heightToSpeed(height)`: "back-calc it from our speed & gravity... v = sqrt(2*g*h)".
fn height_to_speed(gravity: f32, height: f32) -> f32 {
    (2.0 * gravity * height).abs().sqrt()
}

/// `class PhysicsBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct PhysicsBehaviorModuleData {
    /// The base.
    pub update: UpdateModuleData,
    /// `m_mass`, `m_shockResistance`, `m_shockMaxYaw`, `m_shockMaxPitch`, `m_shockMaxRoll`.
    pub mass: f32,
    pub shock_resistance: f32,
    pub shock_max_yaw: f32,
    pub shock_max_pitch: f32,
    pub shock_max_roll: f32,
    /// `m_forwardFriction`, `m_lateralFriction`, `m_ZFriction`, `m_aerodynamicFriction` (per frame).
    pub forward_friction: f32,
    pub lateral_friction: f32,
    pub z_friction: f32,
    pub aerodynamic_friction: f32,
    /// `m_centerOfMassOffset`.
    pub center_of_mass_offset: f32,
    /// `m_killWhenRestingOnGround`, `m_allowBouncing`, `m_allowCollideForce`.
    pub kill_when_resting_on_ground: bool,
    pub allow_bouncing: bool,
    pub allow_collide_force: bool,
    /// The height `m_minFallSpeedForDamage` is made from (see the module docs).
    pub min_fall_height_for_damage: f32,
    /// `m_fallHeightDamageFactor`, `m_pitchRollYawFactor`.
    pub fall_height_damage_factor: f32,
    pub pitch_roll_yaw_factor: f32,
    /// `m_vehicleCrashesIntoBuildingWeaponTemplate`, `m_vehicleCrashesIntoNonBuildingWeaponTemplate`, by name.
    pub vehicle_crashes_into_building_weapon: Option<Vec<u8>>,
    pub vehicle_crashes_into_non_building_weapon: Option<Vec<u8>>,
}

impl Default for PhysicsBehaviorModuleData {
    /// `PhysicsBehaviorModuleData()`.
    fn default() -> Self {
        PhysicsBehaviorModuleData {
            update: UpdateModuleData::default(),
            mass: DEFAULT_MASS,
            shock_resistance: 0.0,
            shock_max_yaw: DEFAULT_SHOCK_YAW,
            shock_max_pitch: DEFAULT_SHOCK_PITCH,
            shock_max_roll: DEFAULT_SHOCK_ROLL,
            forward_friction: DEFAULT_FORWARD_FRICTION,
            lateral_friction: DEFAULT_LATERAL_FRICTION,
            z_friction: DEFAULT_Z_FRICTION,
            aerodynamic_friction: DEFAULT_AERO_FRICTION,
            center_of_mass_offset: 0.0,
            kill_when_resting_on_ground: false,
            allow_bouncing: false,
            allow_collide_force: true,
            min_fall_height_for_damage: 40.0,
            // "was 10. now is 1."
            fall_height_damage_factor: 1.0,
            // "defaulting it to 2 is, in fact, the right thing for now... (srj)"
            pitch_roll_yaw_factor: 2.0,
            vehicle_crashes_into_building_weapon: Some(b"VehicleCrashesIntoBuildingWeapon".to_vec()),
            vehicle_crashes_into_non_building_weapon: Some(b"VehicleCrashesIntoNonBuildingWeapon".to_vec()),
        }
    }
}

/// `parseFrictionPerSec`: the per-second friction as a per-frame one.
fn parse_friction_per_sec(ini: &mut Ini<'_>) -> IniResult<f32> {
    Ok(ini.parse_real()? * SECONDS_PER_LOGICFRAME_REAL)
}

/// `INI::parseWeaponTemplate`: the next token (found in the store when used; `None` names no template).
fn parse_weapon(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `PhysicsBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<PhysicsBehaviorModuleData>; 19] = [
    FieldParse::new("Mass", |i, d, _| { d.mass = i.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShockResistance", |i, d, _| { d.shock_resistance = i.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShockMaxYaw", |i, d, _| { d.shock_max_yaw = i.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShockMaxPitch", |i, d, _| { d.shock_max_pitch = i.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShockMaxRoll", |i, d, _| { d.shock_max_roll = i.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ForwardFriction", |i, d, _| { d.forward_friction = parse_friction_per_sec(i)?; Ok(()) }, UserData::None),
    FieldParse::new("LateralFriction", |i, d, _| { d.lateral_friction = parse_friction_per_sec(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ZFriction", |i, d, _| { d.z_friction = parse_friction_per_sec(i)?; Ok(()) }, UserData::None),
    FieldParse::new("AerodynamicFriction", |i, d, _| { d.aerodynamic_friction = parse_friction_per_sec(i)?; Ok(()) }, UserData::None),
    FieldParse::new("CenterOfMassOffset", |i, d, _| { d.center_of_mass_offset = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowBouncing", |i, d, _| { d.allow_bouncing = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowCollideForce", |i, d, _| { d.allow_collide_force = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("KillWhenRestingOnGround", |i, d, _| { d.kill_when_resting_on_ground = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("MinFallHeightForDamage", |i, d, _| { d.min_fall_height_for_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FallHeightDamageFactor", |i, d, _| { d.fall_height_damage_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PitchRollYawFactor", |i, d, _| { d.pitch_roll_yaw_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("VehicleCrashesIntoBuildingWeaponTemplate", |i, d, _| { d.vehicle_crashes_into_building_weapon = parse_weapon(i)?; Ok(()) }, UserData::None),
    FieldParse::new(
        "VehicleCrashesIntoNonBuildingWeaponTemplate",
        |i, d, _| {
            d.vehicle_crashes_into_non_building_weapon = parse_weapon(i)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// batch R79: the table's tokens, for a derived class's table (`RailroadBehaviorModuleData`).
pub const PHYSICS_FIELD_TOKENS: [&str; 18] = [
    "Mass",
    "ShockResistance",
    "ShockMaxYaw",
    "ShockMaxPitch",
    "ShockMaxRoll",
    "ForwardFriction",
    "LateralFriction",
    "ZFriction",
    "AerodynamicFriction",
    "CenterOfMassOffset",
    "AllowBouncing",
    "AllowCollideForce",
    "KillWhenRestingOnGround",
    "MinFallHeightForDamage",
    "FallHeightDamageFactor",
    "PitchRollYawFactor",
    "VehicleCrashesIntoBuildingWeaponTemplate",
    "VehicleCrashesIntoNonBuildingWeaponTemplate",
];

/// batch R79: the `k`th entry of the table on the data (a derived class's table calls it on its base).
pub fn physics_field(k: usize, i: &mut Ini<'_>, d: &mut PhysicsBehaviorModuleData, u: &UserData) -> IniResult<()> {
    debug_assert_eq!(FIELD_PARSE[k].token, Some(PHYSICS_FIELD_TOKENS[k]));
    (FIELD_PARSE[k].parse.expect("an entry"))(i, d, u)
}

impl PhysicsBehaviorModuleData {
    /// `buildFieldParse(p)`: `UpdateModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PhysicsBehaviorModuleData>) -> IniResult<()> {
        crate::logic::module::behavior_module::BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<PhysicsBehaviorModuleData> {
        let mut data = PhysicsBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PhysicsBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class PhysicsBehavior : public UpdateModule, public CollideModuleInterface`.
#[derive(Clone, Debug)]
pub struct PhysicsBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getPhysicsBehaviorModuleData()`.
    data: Arc<PhysicsBehaviorModuleData>,
    /// `m_yawRate`, `m_rollRate`, `m_pitchRate`.
    yaw_rate: f32,
    roll_rate: f32,
    pitch_rate: f32,
    /// `m_bounceSound`'s event name (`None`: `NULL`).
    bounce_sound: Option<Vec<u8>>,
    /// `m_accel`, `m_prevAccel`, `m_vel`.
    accel: Coord3D,
    prev_accel: Coord3D,
    vel: Coord3D,
    /// `m_turning` (the pool's zero until set; see the module docs).
    turning: i32,
    /// `m_ignoreCollisionsWith`.
    ignore_collisions_with: ObjectId,
    /// `m_flags`.
    flags: i32,
    /// `m_mass`.
    mass: f32,
    /// `m_currentOverlap`, `m_previousOverlap`, `m_lastCollidee`.
    current_overlap: ObjectId,
    previous_overlap: ObjectId,
    last_collidee: ObjectId,
    /// `m_motiveForceExpires`.
    motive_force_expires: u32,
    /// `m_extraBounciness`, `m_extraFriction`.
    extra_bounciness: f32,
    extra_friction: f32,
    /// `m_pui != NULL`: the object is a projectile with a projectile update module (the index of its first).
    pui: Option<u16>,
    /// `m_velMag` ("recalced when m_vel changes").
    vel_mag: f32,
}

impl PartialEq for PhysicsBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words() && self.bounce_sound == other.bounce_sound
    }
}

impl PhysicsBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s and `MODULEINTERFACE_COLLIDE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_COLLIDE
    }

    /// The constructor's first half (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<PhysicsBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Physics)?;
        let mut m = PhysicsBehavior {
            update,
            yaw_rate: 0.0,
            roll_rate: 0.0,
            pitch_rate: 0.0,
            bounce_sound: None,
            accel: Coord3D::default(),
            prev_accel: Coord3D::default(),
            vel: Coord3D::default(),
            turning: TURN_NONE,
            ignore_collisions_with: INVALID_ID,
            flags: 0,
            mass: data.mass,
            current_overlap: INVALID_ID,
            previous_overlap: INVALID_ID,
            last_collidee: INVALID_ID,
            motive_force_expires: 0,
            extra_bounciness: 0.0,
            extra_friction: 0.0,
            pui: None,
            vel_mag: 0.0,
            data: data.clone(),
        };
        m.set_flag(ALLOW_BOUNCE, data.allow_bouncing);
        m.set_flag(ALLOW_COLLIDE_FORCE, data.allow_collide_force);
        Ok(m)
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The `UpdateModule` base, to change (batch R79: a derived class's `xfer`).
    pub fn update_module_mut(&mut self) -> &mut UpdateModule {
        &mut self.update
    }

    /// The state the test compares: the rates, the three vectors, the turning, the ids, the flags, the mass, the frames and the
    /// modifiers, as words.
    pub fn state_words(&self) -> Vec<u32> {
        let b = |x: f32| x.to_bits();
        vec![
            b(self.yaw_rate),
            b(self.roll_rate),
            b(self.pitch_rate),
            b(self.accel.x),
            b(self.accel.y),
            b(self.accel.z),
            b(self.prev_accel.x),
            b(self.prev_accel.y),
            b(self.prev_accel.z),
            b(self.vel.x),
            b(self.vel.y),
            b(self.vel.z),
            self.turning as u32,
            self.ignore_collisions_with as u32,
            self.flags as u32,
            b(self.mass),
            self.current_overlap as u32,
            self.previous_overlap as u32,
            self.last_collidee as u32,
            self.motive_force_expires,
            b(self.extra_bounciness),
            b(self.extra_friction),
            b(self.vel_mag),
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half (`SLEEPY_PHYSICS`: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`).
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

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }


    fn set_flag(&mut self, f: i32, set: bool) {
        if set {
            self.flags |= f;
        } else {
            self.flags &= !f;
        }
    }

    fn get_flag(&self, f: i32) -> bool {
        self.flags & f != 0
    }

    /// `getMass()`: the module's mass and the contained items'.
    fn get_mass(&self, world: &mut World, us: ObjectId) -> f32 {
        let mut mass = self.mass;
        if let Some(contain) = world.get_contain(us) {
            mass += world.module_call(us, contain, ModuleCall::GetContainedItemsMass).as_real(0.0);
        }
        mass
    }

    /// The friction, with the extra and clamped: `getAerodynamicFriction` and the three others.
    fn clamped(f: f32, min: f32) -> f32 {
        let mut f = f;
        if f < min {
            f = min;
        }
        if f > MAX_FRICTION {
            f = MAX_FRICTION;
        }
        f
    }

    fn get_aerodynamic_friction(&self) -> f32 {
        Self::clamped(self.data.aerodynamic_friction + self.extra_friction, MIN_AERO_FRICTION)
    }

    fn get_forward_friction(&self) -> f32 {
        let forward = if crate::logic::modules::plant(2202) { self.data.lateral_friction } else { self.data.forward_friction };
        Self::clamped(forward + self.extra_friction, MIN_NON_AERO_FRICTION)
    }

    fn get_lateral_friction(&self) -> f32 {
        Self::clamped(self.data.lateral_friction + self.extra_friction, MIN_NON_AERO_FRICTION)
    }

    /// `getObject()->getUnitDirectionVector2D()`.
    fn unit_dir_2d(world: &mut World, us: ObjectId) -> Coord3D {
        world.objects.find_mut(us).map_or_else(Coord3D::default, |o| o.thing.unit_direction_vector_2d())
    }

    /// `isMotive()`.
    fn is_motive(&self, world: &World) -> bool {
        self.motive_force_expires > world.frame()
    }

    /// `applyForce(force)`: "apply a force at the object's CG".
    fn apply_force(&mut self, world: &mut World, us: ObjectId, force: &Coord3D) {
        if force.x.is_nan() || force.y.is_nan() || force.z.is_nan() {
            return;
        }
        // F = ma  -->  a = F/m  (divide force by mass)
        let mass = self.get_mass(world, us);
        let mut mod_force = *force;
        if self.is_motive(world) && !crate::logic::modules::plant(2207) {
            let dir = Self::unit_dir_2d(world, us);
            // "Only accept the lateral acceleration."
            let lateral_dot = force.x * (-dir.y) + force.y * dir.x;
            mod_force.x = lateral_dot * -dir.y;
            mod_force.y = lateral_dot * dir.x;
        }
        let mass_inv = 1.0 / mass;
        self.accel.x += mod_force.x * mass_inv;
        self.accel.y += mod_force.y * mass_inv;
        self.accel.z += mod_force.z * mass_inv;
        self.wake_unless_in_update(world);
    }

    /// `SLEEPY_PHYSICS`: "when a force is applied by an external module, we must wake up, even if the force is zero."
    fn wake_unless_in_update(&self, world: &mut World) {
        if !self.get_flag(IS_IN_UPDATE) {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        }
    }

    /// `applyShock(force)`.
    fn apply_shock(&mut self, world: &mut World, us: ObjectId, force: &Coord3D) {
        let mut resisted = *force;
        let r = self.data.shock_resistance.clamp_upstream();
        resisted.x *= 1.0 - r;
        resisted.y *= 1.0 - r;
        resisted.z *= 1.0 - r;
        self.apply_force(world, us, &resisted);
    }

    /// `applyRandomRotation()`.
    fn apply_random_rotation(&mut self, world: &mut World) {
        // "Ignore any pitch, roll & yaw rotation if behavior is stick to ground"
        if self.get_flag(STICK_TO_GROUND) {
            return;
        }
        // "Set bounce to true for a while until the unit is complete bouncing"
        self.set_flag(ALLOW_BOUNCE, true);
        // `GameLogicRandomValue(-1.0f, 1.0f)` is the integer one: -1, 0 or 1
        let r = world.random.game_logic_random_value(-1, 1) as f32;
        self.yaw_rate += self.data.shock_max_yaw * r;
        let r = world.random.game_logic_random_value(-1, 1) as f32;
        self.pitch_rate += self.data.shock_max_pitch * r;
        let r = world.random.game_logic_random_value(-1, 1) as f32;
        self.roll_rate += self.data.shock_max_roll * r;
        self.wake_unless_in_update(world);
    }

    /// `applyMotiveForce(force)`.
    fn apply_motive_force(&mut self, world: &mut World, us: ObjectId, force: &Coord3D) {
        self.motive_force_expires = 0; // "make it accept this force unquestioningly :)"
        self.apply_force(world, us, force);
        self.motive_force_expires = world.frame().wrapping_add(MOTIVE_FRAMES);
    }

    /// `resetDynamicPhysics()`.
    fn reset_dynamic_physics(&mut self, world: &mut World, us: ObjectId) {
        self.accel = Coord3D::default();
        self.prev_accel = Coord3D::default();
        self.vel = Coord3D::default();
        self.vel_mag = 0.0;
        self.turning = TURN_NONE;
        self.yaw_rate = 0.0;
        self.roll_rate = 0.0;
        self.pitch_rate = 0.0;
        self.set_flag(HAS_PITCHROLLYAW, false);
        let sleep = self.calc_sleep_time(world, us);
        update_behavior::set_wake_frame(world, sleep);
    }

    fn update_pry_flag(&mut self) {
        let any = self.pitch_rate != 0.0 || self.roll_rate != 0.0 || self.yaw_rate != 0.0;
        self.set_flag(HAS_PITCHROLLYAW, any);
    }

    /// `applyYPRDamping(factor)`.
    fn apply_ypr_damping(&mut self, factor: f32) {
        self.pitch_rate *= factor;
        self.roll_rate *= factor;
        self.yaw_rate *= factor;
        self.update_pry_flag();
    }

    /// `applyFrictionalForces()`.
    fn apply_frictional_forces(&mut self, world: &mut World, us: ObjectId) {
        // "Are we a plane that is taxiing on a deck with a height offset?"
        let on_deck = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_DECK_HEIGHT_OFFSET));
        let deck_taxiing = on_deck
            && match world.objects.find(us).and_then(|o| o.ai) {
                Some(ai) => matches!(world.module_call(us, ai, ModuleCall::AiGetCurLocomotorSetType), ModuleReply::Int(LOCOMOTORSET_TAXIING)),
                None => false,
            };
        if self.get_flag(APPLY_FRICTION2D_WHEN_AIRBORNE) || !world.is_significantly_above_terrain(us) || deck_taxiing {
            self.apply_ypr_damping(1.0 - DEFAULT_LATERAL_FRICTION);
            if self.vel.x != 0.0 || self.vel.y != 0.0 {
                let dir = Self::unit_dir_2d(world, us);
                let mass = self.get_mass(world, us);
                let lateral_dot = self.vel.x * (-dir.y) + self.vel.y * dir.x;
                let lateral_vel_x = lateral_dot * -dir.y;
                let lateral_vel_y = lateral_dot * dir.x;
                let lf = mass * self.get_lateral_friction();
                let mut accel = Coord3D { x: -(lf * lateral_vel_x), y: -(lf * lateral_vel_y), z: 0.0 };
                if !self.is_motive(world) {
                    let forward_dot = self.vel.x * dir.x + self.vel.y * dir.y;
                    let forward_vel_x = forward_dot * dir.x;
                    let forward_vel_y = forward_dot * dir.y;
                    let ff = mass * self.get_forward_friction();
                    accel.x += -(ff * forward_vel_x);
                    accel.y += -(ff * forward_vel_y);
                }
                self.apply_force(world, us, &accel);
            }
        } else {
            let aerodynamics = -self.get_aerodynamic_friction(); // "negated!"
            // "Air resistance is proportional to velocity in the opposite direction"
            self.accel.x += self.vel.x * aerodynamics;
            self.accel.y += self.vel.y * aerodynamics;
            self.accel.z += self.vel.z * aerodynamics;
            self.apply_ypr_damping(1.0 + aerodynamics);
        }
    }

    /// `setAngles(yaw, pitch, roll)`.
    fn set_angles(world: &mut World, us: ObjectId, yaw: f32, pitch: f32, roll: f32) {
        let pos = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
        let mut xfrm = Matrix3D::default();
        xfrm.make_identity();
        xfrm.translate(pos.x, pos.y, pos.z);
        // "here we DO want to use in-place-etc, cuz we're not adding to any existing rot/etc"
        xfrm.in_place_pre_rotate_x(-roll);
        xfrm.in_place_pre_rotate_y(pitch);
        xfrm.in_place_pre_rotate_z(yaw);
        world.set_transform_matrix(us, &xfrm);
    }

    /// `handleBounce(oldZ, newZ, groundZ, &bounceForce)`.
    fn handle_bounce(&mut self, world: &mut World, me: ModuleHandle, old_z: f32, new_z: f32, ground_z: f32) -> Result<(bool, Coord3D), Undefined> {
        let us = me.object;
        if !(self.get_flag(ALLOW_BOUNCE) && new_z <= ground_z) {
            return Ok((false, Coord3D::default()));
        }
        const MIN_STIFF: f32 = 0.01;
        const MAX_STIFF: f32 = 0.99;
        // (`clamp` answers a NaN as the two tests do)
        let stiffness = world.cuts.globals.ground_stiffness.clamp(MIN_STIFF, MAX_STIFF);
        let mut desired_accel_z = 0.0;
        let vz = self.vel.z;
        if old_z > ground_z && vz < 0.0 {
            desired_accel_z = vz.abs() * stiffness;
        }
        let bounce_force = Coord3D { x: 0.0, y: 0.0, z: self.get_mass(world, us) * desired_accel_z };
        const DAMPING: f32 = 0.7;
        self.apply_ypr_damping(DAMPING);
        if vz < 0.0 {
            let mx = world.objects.find(us).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
            let zvec = mx.get_z_vector();
            let roll_angle = if zvec.z > 0.0 { 0.0 } else { std::f32::consts::PI };
            // "don't flip both pitch and roll... we'll flip twice."
            let pitch_angle = 0.0;
            let yaw_angle = mx.get_z_rotation();
            Self::set_angles(world, us, yaw_angle, pitch_angle, roll_angle);
        }
        if bounce_force.z > 0.0 {
            self.test_stunned_unit_for_destruction(world, me);
            Ok((true, bounce_force))
        } else {
            // `setAllowBouncing(m_originalAllowBounce)`: a member no code sets, so the pool's zero (see the module docs)
            self.set_flag(ALLOW_BOUNCE, false);
            Ok((false, bounce_force))
        }
    }

    /// `calcSleepTime()`.
    fn calc_sleep_time(&self, world: &mut World, us: ObjectId) -> UpdateSleepTime {
        let zero = |v: &Coord3D| v.x == 0.0 && v.y == 0.0 && v.z == 0.0;
        let on_ground_layer = world.objects.find(us).is_some_and(|o| o.layer == LAYER_GROUND);
        if zero(&self.vel)
            && zero(&self.accel)
            && !self.get_flag(HAS_PITCHROLLYAW)
            && !self.is_motive(world)
            && on_ground_layer
            && !world.is_above_terrain(us)
            && self.current_overlap == INVALID_ID
            && self.previous_overlap == INVALID_ID
            && self.get_flag(UPDATE_EVER_RUN)
        {
            UPDATE_SLEEP_FOREVER
        } else {
            UPDATE_SLEEP_NONE
        }
    }

    /// `getVelocityMagnitude()`.
    fn get_velocity_magnitude(&mut self) -> f32 {
        if self.vel_mag == INVALID_VEL_MAG {
            self.vel_mag = (sqr(self.vel.x) + sqr(self.vel.y) + sqr(self.vel.z)).sqrt();
        }
        self.vel_mag
    }

    /// `scrubVelocityZ(desiredVelocity)`.
    fn scrub_velocity_z(&mut self, desired: f32) {
        if desired.abs() < 0.001 {
            self.vel.z = 0.0;
        } else if (desired < 0.0 && self.vel.z < desired) || (desired > 0.0 && self.vel.z > desired) {
            self.vel.z = desired;
        }
        self.vel_mag = INVALID_VEL_MAG;
    }

    /// `scrubVelocity2D(desiredVelocity)`.
    fn scrub_velocity_2d(&mut self, desired: f32) {
        if desired < 0.001 {
            self.vel.x = 0.0;
            self.vel.y = 0.0;
        } else {
            let cur = (self.vel.x * self.vel.x + self.vel.y * self.vel.y).sqrt();
            if desired > cur || crate::logic::modules::plant(2208) {
                return;
            }
            let desired = desired / cur;
            self.vel.x *= desired;
            self.vel.y *= desired;
        }
        self.vel_mag = INVALID_VEL_MAG;
    }

    /// `doBounceSound(prevPos)` (the volume is `FIX_AUDIO`'s, not built: the event as given).
    fn do_bounce_sound(&self, world: &mut World, us: ObjectId) {
        if let Some(name) = self.bounce_sound.clone() {
            world.audio_add_event(name, us);
        }
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, IniError> {
        let us = me.object;
        let d = self.data.clone();
        let airborne_at_start = world.is_above_terrain(us);
        let mut active_vel_z = 0.0;
        let mut bounce_force = Coord3D::default();
        let mut got_bounce_force = false;
        self.set_flag(IS_IN_UPDATE, true);
        if !self.get_flag(UPDATE_EVER_RUN) {
            // "set the flag so that we don't get bogus collisions on the first frame."
            self.set_flag(WAS_AIRBORNE_LAST_FRAME, airborne_at_start);
        }
        self.prev_accel = self.accel;
        let held = world.objects.find(us).is_some_and(|o| o.is_disabled_by_type(DISABLED_HELD));
        if !held {
            let mut mtx = world.objects.find(us).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
            // `applyGravitationalForces()`
            if !crate::logic::modules::plant(2201) {
                self.accel.z += world.cuts.globals.gravity;
            }
            self.apply_frictional_forces(world, us);
            // "integrate acceleration into velocity"
            self.vel.x += self.accel.x;
            self.vel.y += self.accel.y;
            self.vel.z += self.accel.z;
            // "when vel gets tiny, just clamp to zero"
            const THRESH: f32 = 0.001;
            if self.vel.x.abs() < THRESH {
                self.vel.x = 0.0;
            }
            if self.vel.y.abs() < THRESH {
                self.vel.y = 0.0;
            }
            if self.vel.z.abs() < THRESH {
                self.vel.z = 0.0;
            }
            self.vel_mag = INVALID_VEL_MAG;
            let old_pos_z = mtx.get_z_translation();
            // "integrate velocity into position"
            let braking = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_BRAKING));
            if braking {
                // "Don't update position if the locomotor is braking." "Things other than projectiles don't cheat in z."
                if !world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_PROJECTILE)) {
                    mtx.adjust_z_translation(self.vel.z);
                }
            } else {
                mtx.adjust_x_translation(self.vel.x);
                mtx.adjust_y_translation(self.vel.y);
                mtx.adjust_z_translation(self.vel.z);
            }
            if mtx.get_x_translation().is_nan() || mtx.get_y_translation().is_nan() || mtx.get_z_translation().is_nan() {
                // DEBUG_CRASH(("Object position is NAN, deleting."));
                world.body_destroy_object(us);
            }
            // "Check when to clear the stunned status"
            if self.get_flag(IS_STUNNED)
                && ((self.vel.x.abs() < STUN_RELIEF_EPSILON && self.vel.y.abs() < STUN_RELIEF_EPSILON && self.vel.z.abs() < STUN_RELIEF_EPSILON)
                    || !world.is_significantly_above_terrain(us))
            {
                self.set_flag(IS_STUNNED, false);
                world.clear_model_condition_state(us, MODELCONDITION_STUNNED);
            }
            if self.get_flag(HAS_PITCHROLLYAW) {
                let yaw_rate_to_use = self.yaw_rate * d.pitch_roll_yaw_factor;
                let mut pitch_rate_to_use = self.pitch_rate * d.pitch_roll_yaw_factor;
                let roll_rate_to_use = self.roll_rate * d.pitch_roll_yaw_factor;
                // "With a center of mass listing, pitchRate needs to dampen towards straight down/straight up"
                let offset = d.center_of_mass_offset;
                if offset != 0.0 {
                    let xvec = mtx.get_x_vector();
                    let xy = (sqr(xvec.x) + sqr(xvec.y)).sqrt();
                    let pitch_angle = f64::from(xvec.z).atan2(f64::from(xy)) as f32;
                    let half_pi = std::f32::consts::PI / 2.0;
                    let remaining_angle = if offset > 0.0 { half_pi - pitch_angle } else { -half_pi + pitch_angle };
                    let s = trig::sin(remaining_angle);
                    pitch_rate_to_use *= s;
                }
                // "note, we DON'T want to Pre-rotate (either inplace or not), since we want to add our mods to the existing matrix."
                mtx.rotate_x(roll_rate_to_use);
                mtx.rotate_y(pitch_rate_to_use);
                mtx.rotate_z(yaw_rate_to_use);
            }
            // "do not allow object to pass through the ground"
            let layer = world.objects.find(us).map_or(LAYER_GROUND, |o| o.layer);
            let mut ground_z = world.terrain_get_layer_height(mtx.get_x_translation(), mtx.get_y_translation(), layer);
            if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_DECK_HEIGHT_OFFSET)) {
                ground_z += world.get_carrier_deck_height(us);
            }
            let (got, force) = self.handle_bounce(world, me, old_pos_z, mtx.get_z_translation(), ground_z).map_err(IniError::undefined)?;
            got_bounce_force = got;
            bounce_force = force;
            // "remember our z-vel prior to doing ground-slam adjustment"
            active_vel_z = self.vel.z;
            if mtx.get_z_translation() <= ground_z {
                // "Remove the excess z velocity."
                let dz = ground_z - mtx.get_z_translation();
                self.vel.z += dz;
                if self.vel.z > 0.0 {
                    self.vel.z = 0.0;
                }
                self.vel_mag = INVALID_VEL_MAG;
                mtx.set_z_translation(ground_z);
                // "this flag is ALWAYS cleared once we hit the ground."
                self.set_flag(ALLOW_TO_FALL, false);
                // "When a stunned object hits the ground the first time, change its model state from stunned flailing to just stunned."
                if self.get_flag(IS_STUNNED) {
                    world.clear_model_condition_state(us, MODELCONDITION_STUNNED_FLAILING);
                    world.set_model_condition_state(us, MODELCONDITION_STUNNED);
                }
            } else if mtx.get_z_translation() > ground_z {
                if self.get_flag(IS_IN_FREEFALL) {
                    world.body_set_disabled(us, DISABLED_FREEFALL);
                    world.set_model_condition_state(us, MODELCONDITION_FREEFALL);
                } else if self.get_flag(STICK_TO_GROUND) && !self.get_flag(ALLOW_TO_FALL) {
                    mtx.set_z_translation(ground_z);
                }
            }
            if got_bounce_force {
                // "Right the object after the bounce since the pitch and roll may have been affected"
                let yaw_angle = world.objects.find(us).map_or(0.0, |o| o.thing.transform_matrix().get_z_rotation());
                Self::set_angles(world, us, yaw_angle, 0.0, 0.0);
                let mut after_bounce = world.objects.find(us).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
                after_bounce.set_translation(&mtx.get_translation());
                world.set_transform_matrix(us, &after_bounce);
            } else {
                world.set_transform_matrix(us, &mtx);
            }
        }
        // "reset the acceleration for accumulation next frame"
        self.accel = Coord3D::default();
        // "clear overlap object, which will be set by PhysicsCollide later"
        self.previous_overlap = self.current_overlap;
        self.current_overlap = INVALID_ID;
        if got_bounce_force && self.get_flag(ALLOW_BOUNCE) {
            self.apply_force(world, us, &bounce_force);
        }
        let airborne_at_end = world.is_above_terrain(us);
        if self.get_flag(WAS_AIRBORNE_LAST_FRAME) && !airborne_at_end && !self.get_flag(IMMUNE_TO_FALLING_DAMAGE) {
            self.do_bounce_sound(world, us);
            // "the normal always points straight down"
            let normal = Coord3D { x: 0.0, y: 0.0, z: -1.0 };
            let pos = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
            crate::logic::modules::lend_clone(world, me, self, UpdateBody::PhysicsBehavior, Self::from_body, |w, _| w.on_collide(us, INVALID_ID, Some(pos), Some(normal)));
            // "(note that m_minFallSpeedForDamage is always POSITIVE.)"
            let min_fall_speed = height_to_speed(world.cuts.globals.gravity, d.min_fall_height_for_damage);
            let net_speed = -active_vel_z - min_fall_speed;
            if net_speed > 0.0 && self.pui.is_none() {
                // "only apply force if it's a pretty steep fall"
                const MIN_ANGLE_TAN: f32 = 3.0;
                const TINY_DELTA: f32 = 0.01;
                if (self.vel.x.abs() <= TINY_DELTA || (active_vel_z / self.vel.x).abs() >= MIN_ANGLE_TAN)
                    && (self.vel.y.abs() <= TINY_DELTA || (active_vel_z / self.vel.y).abs() >= MIN_ANGLE_TAN)
                {
                    let factor = if crate::logic::modules::plant(2203) { 1.0 } else { d.fall_height_damage_factor };
                    let damage_amt = net_speed * self.get_mass(world, us) * factor;
                    let mut info = DamageInfo::default();
                    info.input.damage_type = DAMAGE_FALLING;
                    info.input.death_type = crate::logic::terrain_logic::DEATH_SPLATTED;
                    info.input.source_id = us;
                    info.input.amount = damage_amt;
                    info.input.shock_wave_amount = 0.0;
                    crate::logic::modules::lend_clone(world, me, self, UpdateBody::PhysicsBehavior, Self::from_body, |w, _| w.attempt_damage(us, &mut info));
                    // "if this killed us, add SPLATTED to get a cool death."
                    if world.objects.find(us).is_some_and(|o| o.is_effectively_dead()) {
                        world.set_model_condition_state(us, MODELCONDITION_SPLATTED);
                    }
                }
            }
        }
        if !airborne_at_end {
            // "just in case."
            self.set_flag(IS_IN_FREEFALL, false);
            if world.objects.find(us).is_some_and(|o| o.is_disabled_by_type(DISABLED_FREEFALL)) {
                world.body_clear_disabled(us, DISABLED_FREEFALL);
            }
            world.clear_model_condition_state(us, MODELCONDITION_FREEFALL);
        }
        // "If we are effectively dead, we shouldn't recall kill."
        let very_small = self.vel.x.abs() < 0.01 && self.vel.y.abs() < 0.01 && self.vel.z.abs() < 0.01;
        if d.kill_when_resting_on_ground && !airborne_at_end && very_small && !crate::logic::modules::plant(2204) {
            let (drone, dead, unmanned) = world.objects.find(us).map_or((false, false, false), |o| {
                (o.is_kind_of(KINDOF_DRONE), o.is_effectively_dead(), o.is_disabled_by_type(DISABLED_UNMANNED))
            });
            if !drone || dead || unmanned {
                self.kill(world, me)?;
            }
        }
        self.set_flag(UPDATE_EVER_RUN, true);
        self.set_flag(WAS_AIRBORNE_LAST_FRAME, airborne_at_end);
        self.set_flag(IS_IN_UPDATE, false);
        Ok(self.calc_sleep_time(world, us))
    }

    /// `obj->kill()` (`DAMAGE_UNRESISTABLE`, `DEATH_NORMAL`), the module lent to its slot.
    fn kill(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let us = me.object;
        if world.get_body_module(us).is_none() {
            // `kill` reads `getBodyModule()->getMaxHealth()` unchecked
            return Err(IniError::undefined(Undefined::NullBodyModule));
        }
        crate::logic::modules::lend_clone(world, me, self, UpdateBody::PhysicsBehavior, Self::from_body, |w, _| w.kill(us, DAMAGE_UNRESISTABLE, DEATH_NORMAL));
        Ok(())
    }

    /// `testStunnedUnitForDestruction()`: "whether unit needs to die because of being on illegal cell, upside down, outside legal bounds".
    fn test_stunned_unit_for_destruction(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.get_flag(IS_STUNNED) {
            return;
        }
        let us = me.object;
        let pos = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
        let upside_down = world.objects.find(us).is_some_and(|o| o.thing.transform_matrix().get_z_vector().z < 0.0);
        let off_map = world.objects.find(us).is_some_and(|o| o.is_off_map());
        let result = if upside_down || off_map {
            self.kill(world, me)
        } else {
            let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return };
            let surfaces = |w: &mut World, s: u32| w.module_call(us, ai, ModuleCall::AiHasLocomotorForSurface(s)).as_bool();
            if (world.terrain_is_cliff_cell(pos.x, pos.y) && !surfaces(world, LOCOMOTORSURFACE_CLIFF))
                || (world.terrain_is_underwater_water_z(pos.x, pos.y).is_some() && !surfaces(world, LOCOMOTORSURFACE_WATER))
            {
                self.kill(world, me)
            } else {
                Ok(())
            }
        };
        if let Err(e) = result {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// `onCollide(other, loc, normal)` (`other` is `INVALID_ID` for `NULL`).
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId, loc: Option<Coord3D>, normal: Option<Coord3D>) -> Result<(), IniError> {
        let us = me.object;
        if let Some(pui) = self.pui {
            if world.module_call(us, pui, ModuleCall::ProjectileHandleCollision { other }).as_bool() {
                return Ok(());
            }
        }
        let obj_contained_by = world.objects.find(us).map_or(INVALID_ID, |o| o.contained_by);
        if world.objects.find(other).is_none() {
            // "if other is null, we are colliding with the ground"
            if world.objects.find(obj_contained_by).is_some() {
                world.on_collide(obj_contained_by, INVALID_ID, loc, normal);
            }
            return Ok(());
        }
        let other_contained_by = world.objects.find(other).map_or(INVALID_ID, |o| o.contained_by);
        if other_contained_by == us || obj_contained_by == other {
            return Ok(());
        }
        let parachuting = |w: &World, id: ObjectId| w.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_PARACHUTING));
        if parachuting(world, us) && parachuting(world, other) {
            return Ok(());
        }
        let ai = world.objects.find(us).and_then(|o| o.ai);
        if let Some(ai) = ai {
            if world.module_call(us, ai, ModuleCall::AiGetIgnoredObstacleId).as_id() == other {
                let infantry = world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_INFANTRY));
                if infantry && world.objects.find(other).is_some_and(|o| o.is_disabled_by_type(DISABLED_UNMANNED)) {
                    world.body_clear_disabled(other, DISABLED_UNMANNED);
                    world.set_captured(other, true);
                    let team = world.objects.find(us).map(|o| o.team).filter(|t| *t != 0);
                    world.crate_defect(other, team, 0);
                    let name = world.objects.find(us).map(|o| o.name.clone()).unwrap_or_default();
                    world.cut(CutCall::ScriptEngineTransferObjectName { name, object: other });
                    world.body_destroy_object(us);
                }
                return Ok(());
            }
        }
        if let Some(ai_other) = world.objects.find(other).and_then(|o| o.ai) {
            if world.module_call(other, ai_other, ModuleCall::AiGetIgnoredObstacleId).as_id() == us {
                return Ok(());
            }
        }
        if self.ignore_collisions_with != INVALID_ID && self.ignore_collisions_with == other {
            return Ok(());
        }
        let immobile = world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_IMMOBILE));
        let other_immobile = world.objects.find(other).is_some_and(|o| o.is_kind_of(KINDOF_IMMOBILE));
        match world.objects.find(other).and_then(|o| o.physics) {
            Some(op) => {
                if world.module_call(other, op, ModuleCall::PhysicsIsIgnoringCollisionsWith(us)).as_bool() {
                    return Ok(());
                }
            }
            None => {
                // "if the other object has no physics, and is mobile, don't collide with it"
                if !other_immobile {
                    return Ok(());
                }
            }
        }
        if self.check_for_overlap_collision(world, me, other)? {
            return Ok(());
        }
        let _mass = if immobile { 999999.0 } else { self.get_mass(world, us) };
        if let Some(ai) = ai {
            let dead = world.objects.find(us).is_some_and(|o| o.is_effectively_dead());
            if !((dead || parachuting(world, us)) && other_immobile) && !world.module_call(us, ai, ModuleCall::AiProcessCollision { other }).as_bool() {
                return Ok(());
            }
        }
        let (us_pos, us_geom, us_above, us_destroyed) = {
            let above = world.is_above_terrain(us);
            let o = world.objects.find(us).expect("the module's object");
            (o.get_position(), o.geometry_info.clone(), above, o.is_destroyed())
        };
        let (them_pos, them_geom) = {
            let o = world.objects.find(other).expect("checked above");
            (o.get_position(), o.geometry_info.clone())
        };
        let us_center = us_geom.get_center_position(&us_pos);
        let them_center = them_geom.get_center_position(&them_pos);
        let mut delta = Coord3D { x: them_center.x - us_center.x, y: them_center.y - us_center.y, z: them_center.z - us_center.z };
        let (dist_sqr, us_radius, them_radius) = if us_above {
            (sqr(delta.x) + sqr(delta.y) + sqr(delta.z), us_geom.get_bounding_sphere_radius(), them_geom.get_bounding_sphere_radius())
        } else {
            delta.z = 0.0;
            (sqr(delta.x) + sqr(delta.y), us_geom.get_bounding_circle_radius(), them_geom.get_bounding_circle_radius())
        };
        if dist_sqr > sqr(us_radius + them_radius) {
            return Ok(());
        }
        self.last_collidee = other;
        let mut dist = dist_sqr.sqrt();
        let mut overlap = us_radius + them_radius - dist;
        if dist < 1.0 {
            dist = 1.0;
        }
        if !self.get_flag(ALLOW_COLLIDE_FORCE) {
            return Ok(());
        }
        let factor;
        if other_immobile && !us_destroyed {
            if parachuting(world, us) {
                // "a parachuting object bounces off the structure, as does its container"
                let mut obj_to_bounce = us;
                while let Some(c) = world.objects.find(obj_to_bounce).map(|o| o.contained_by).filter(|c| world.objects.find(*c).is_some()) {
                    obj_to_bounce = c;
                }
                let bounce_out_dist = us_radius * 0.1;
                let mut tmp = world.objects.find(obj_to_bounce).map_or_else(Coord3D::default, |o| o.get_position());
                tmp.x -= bounce_out_dist * delta.x / dist;
                tmp.y -= bounce_out_dist * delta.y / dist;
                world.set_position(obj_to_bounce, &tmp);
                if obj_to_bounce == us {
                    self.scrub_velocity_2d(0.0);
                } else {
                    let Some(p) = world.objects.find(obj_to_bounce).and_then(|o| o.physics) else {
                        return Err(IniError::undefined(Undefined::NullPhysics));
                    };
                    world.module_call(obj_to_bounce, p, ModuleCall::PhysicsScrubVelocity2D(0.0));
                }
                return Ok(());
            }
            const MIN_STIFF: f32 = 0.01;
            const MAX_STIFF: f32 = 0.99;
            // (`clamp` answers a NaN as the two tests do)
            let stiffness = world.cuts.globals.structure_stiffness.clamp(MIN_STIFF, MAX_STIFF);
            let mut mag = self.get_velocity_magnitude();
            let min_bounce_speed = 1.0 / (LOGICFRAMES_PER_SECOND as f32 * 5.0);
            if mag < min_bounce_speed {
                mag = min_bounce_speed;
            }
            factor = -mag * self.get_mass(world, us) * stiffness;
            if delta.z < 0.0 && us_pos.z >= world.cuts.body.globals.default_structure_rubble_height {
                let vehicle = world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_VEHICLE));
                let structure = world.objects.find(other).is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE));
                let weapon = if structure { &self.data.vehicle_crashes_into_building_weapon } else { &self.data.vehicle_crashes_into_non_building_weapon };
                if vehicle {
                    Self::fire_temp_weapon(world, us, weapon);
                }
                if structure {
                    world.body_destroy_object(us);
                    return Ok(());
                }
            }
            self.vel = Coord3D::default();
            self.vel_mag = INVALID_VEL_MAG;
        } else {
            if overlap > 5.0 && !crate::logic::modules::plant(2205) {
                overlap = 5.0;
            }
            factor = -overlap;
        }
        let force = Coord3D { x: factor * delta.x / dist, y: factor * delta.y / dist, z: factor * delta.z / dist };
        self.apply_force(world, us, &force);
        Ok(())
    }

    /// `TheWeaponStore->createAndFireTempWeapon(template, obj, obj->getPosition())` (a name the store does not have is `NULL`: nothing).
    fn fire_temp_weapon(world: &mut World, us: ObjectId, weapon: &Option<Vec<u8>>) {
        let Some(name) = weapon else { return };
        let found = world.statics.clone().weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(name)).map(|t| t.name.clone());
        if let Some(weapon) = found {
            let position = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
            world.cut(CutCall::WeaponCreateAndFireTemp { weapon, source: us, position });
        }
    }

    /// `checkForOverlapCollision(other)`: "return true if we want to skip having physics push us apart".
    fn check_for_overlap_collision(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let v = self.vel;
        if v.x.abs() < 0.01 && v.y.abs() < 0.01 && v.z.abs() < 0.01 {
            return Ok(false);
        }
        let crusher_me = me.object;
        let crushee_other = other;
        let self_crushing_other = world.can_crush_or_squish(crusher_me, Some(crushee_other), CrushSquishTestType::CrushOnly);
        let self_being_crushed = world.can_crush_or_squish(crushee_other, Some(crusher_me), CrushSquishTestType::CrushOnly);
        if self_crushing_other && self_being_crushed {
            // DEBUG_CRASH(("... is attempting to crush ... but it is reciprocating -- shouldn't be possible!"));
            return Ok(false);
        }
        if self_being_crushed {
            // "Let the other guy do the crushing"
            return Ok(true);
        }
        if !self_crushing_other {
            return Ok(false);
        }
        // `addOverlap(crusheeOther)`
        if self.current_overlap != crushee_other {
            self.current_overlap = crushee_other;
        }
        if self.previous_overlap != crushee_other {
            // "we don't want to do damage, just to trigger the minor DamageFX, if any"
            let mut info = DamageInfo::default();
            info.input.damage_type = DAMAGE_CRUSH;
            info.input.death_type = DEATH_CRUSHED;
            info.input.source_id = crusher_me;
            info.input.amount = 0.0;
            crate::logic::modules::lend_clone(world, me, self, UpdateBody::PhysicsBehavior, Self::from_body, |w, _| w.attempt_damage(crushee_other, &mut info));
        }
        let crushee_pos = world.objects.find(crushee_other).map_or_else(Coord3D::default, |o| o.get_position());
        let crusher_pos = world.objects.find(crusher_me).map_or_else(Coord3D::default, |o| o.get_position());
        let Some(body) = world.get_body_module(crushee_other) else {
            return Err(IniError::undefined(Undefined::NullBodyModule));
        };
        let front_crushed = world.module_call(crushee_other, body, ModuleCall::GetFrontCrushed).as_bool();
        let back_crushed = world.module_call(crushee_other, body, ModuleCall::GetBackCrushed).as_bool();
        if front_crushed && back_crushed {
            return Ok(true);
        }
        let dir = Self::unit_dir_2d(world, crusher_me);
        let crushee_dir = Self::unit_dir_2d(world, crushee_other);
        let crush_point_offset_distance = world.objects.find(crushee_other).map_or(0.0, |o| o.geometry_info.get_major_radius()) / 2.0;
        let crush_point_offset = Coord3D { x: crushee_dir.x * crush_point_offset_distance, y: crushee_dir.y * crush_point_offset_distance, z: 0.0 };
        #[derive(Clone, Copy, PartialEq)]
        enum Crush {
            Total,
            Front,
            Back,
        }
        let crush_target = if front_crushed || back_crushed {
            if front_crushed {
                Crush::Back
            } else {
                Crush::Front
            }
        } else {
            // the vector from me to a crush point, and its perpendicular distance from my direction
            let perp = |point: Coord3D| -> (f32, Coord3D) {
                let to = Coord3D { x: point.x - crusher_pos.x, y: point.y - crusher_pos.y, z: 0.0 };
                let ray_length = to.x * dir.x + to.y * dir.y;
                let dir_vector = Coord3D { x: ray_length * dir.x, y: ray_length * dir.y, z: 0.0 };
                let perp_vector = Coord3D { x: dir_vector.x - to.x, y: dir_vector.y - to.y, z: 0.0 };
                (perp_vector.length(), to)
            };
            let front_point = Coord3D { x: crushee_pos.x + crush_point_offset.x, y: crushee_pos.y + crush_point_offset.y, z: crushee_pos.z };
            let back_point = Coord3D { x: crushee_pos.x - crush_point_offset.x, y: crushee_pos.y - crush_point_offset.y, z: crushee_pos.z };
            let (front_perp, front_vector) = perp(front_point);
            let (back_perp, back_vector) = perp(back_point);
            let (center_perp, center_vector) = perp(crushee_pos);
            let eq = |a: f32, b: f32| (a - b).abs() <= 0.15;
            if front_perp <= center_perp && front_perp <= back_perp {
                if eq(front_perp, center_perp) || eq(front_perp, back_perp) {
                    let front_len = front_vector.length();
                    if eq(front_perp, center_perp) {
                        if front_len < center_vector.length() {
                            Crush::Front
                        } else {
                            Crush::Total
                        }
                    } else if front_len < back_vector.length() {
                        Crush::Front
                    } else {
                        Crush::Back
                    }
                } else {
                    Crush::Front
                }
            } else if back_perp <= center_perp && back_perp <= front_perp {
                if eq(back_perp, center_perp) || eq(back_perp, front_perp) {
                    let back_len = back_vector.length();
                    if eq(back_perp, center_perp) {
                        if back_len < center_vector.length() {
                            Crush::Back
                        } else {
                            Crush::Total
                        }
                    } else if back_len < front_vector.length() {
                        Crush::Back
                    } else {
                        Crush::Front
                    }
                } else {
                    Crush::Back
                }
            } else if eq(center_perp, back_perp) || eq(center_perp, front_perp) {
                let center_len = center_vector.length();
                if eq(center_perp, front_perp) {
                    if center_len < front_vector.length() {
                        Crush::Total
                    } else {
                        Crush::Front
                    }
                } else if center_len < back_vector.length() {
                    Crush::Total
                } else {
                    Crush::Back
                }
            } else {
                Crush::Total
            }
        };
        // `2.25 * crushPointOffsetDistance * crushPointOffsetDistance` (a double, stored in a Real)
        let distance_too_far_squared = (2.25 * f64::from(crush_point_offset_distance) * f64::from(crush_point_offset_distance)) as f32;
        // (upstream's `NO_CRUSH` start is always replaced by one of the three)
        let c = match crush_target {
            Crush::Total => crushee_pos,
            Crush::Front => Coord3D { x: crushee_pos.x + crush_point_offset.x, y: crushee_pos.y + crush_point_offset.y, z: crushee_pos.z },
            Crush::Back => Coord3D { x: crushee_pos.x - crush_point_offset.x, y: crushee_pos.y - crush_point_offset.y, z: crushee_pos.z },
        };
        let dx = c.x - crusher_pos.x;
        let dy = c.y - crusher_pos.y;
        let dot = dir.x * dx + dir.y * dy;
        let distance_squared = dx * dx + dy * dy;
        let crush_it = dot < 0.0 && distance_squared < distance_too_far_squared;
        if crush_it {
            let mut info = DamageInfo::default();
            info.input.damage_type = DAMAGE_CRUSH;
            info.input.death_type = DEATH_CRUSHED;
            info.input.source_id = crusher_me;
            info.input.amount = if crate::logic::modules::plant(2206) { 0.0 } else { HUGE_DAMAGE_AMOUNT }; // "make sure they die"
            crate::logic::modules::lend_clone(world, me, self, UpdateBody::PhysicsBehavior, Self::from_body, |w, _| w.attempt_damage(crushee_other, &mut info));
        }
        Ok(true)
    }

    /// `getForwardSpeed2D()`.
    fn get_forward_speed_2d(&self, world: &mut World, us: ObjectId) -> f32 {
        let dir = Self::unit_dir_2d(world, us);
        let vx = self.vel.x * dir.x;
        let vy = self.vel.y * dir.y;
        let dot = vx + vy;
        let speed = (vx * vx + vy * vy).sqrt();
        if dot >= 0.0 {
            speed
        } else {
            -speed
        }
    }

    /// `getForwardSpeed3D()`.
    fn get_forward_speed_3d(&self, world: &World, us: ObjectId) -> f32 {
        let dir = world.objects.find(us).map_or_else(|| Vector3::new(0.0, 0.0, 0.0), |o| o.thing.transform_matrix().get_x_vector());
        let vx = self.vel.x * dir.x;
        let vy = self.vel.y * dir.y;
        let vz = self.vel.z * dir.z;
        let dot = vx + vy + vz;
        let speed = (vx * vx + vy * vy + vz * vz).sqrt();
        if dot >= 0.0 {
            speed
        } else {
            -speed
        }
    }
}

/// `min(1.0f, max(0.0f, x))`.
trait ClampUpstream {
    fn clamp_upstream(self) -> f32;
}

impl ClampUpstream for f32 {
    fn clamp_upstream(self) -> f32 {
        let m = if 0.0f32 > self { 0.0 } else { self };
        if 1.0f32 < m {
            1.0
        } else {
            m
        }
    }
}

impl UpdateBehavior for PhysicsBehavior {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run_update(world, me) {
            Ok(sleep) => sleep,
            Err(e) => {
                self.set_flag(IS_IN_UPDATE, false);
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// `SLEEPY_PHYSICS`: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer.xfer_real(&mut self.yaw_rate)?;
        xfer.xfer_real(&mut self.roll_rate)?;
        xfer.xfer_real(&mut self.pitch_rate)?;
        xfer.xfer_coord3d(&mut self.accel)?;
        xfer.xfer_coord3d(&mut self.prev_accel)?;
        xfer.xfer_coord3d(&mut self.vel)?;
        if version < 2 {
            let mut tmp = Coord3D::default();
            xfer.xfer_coord3d(&mut tmp)?;
        }
        // `xferUser(&m_turning, sizeof(PhysicsTurningType))`
        let mut b = self.turning.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.turning = i32::from_le_bytes(b);
        }
        xfer.xfer_object_id(&mut self.ignore_collisions_with)?;
        xfer.xfer_int(&mut self.flags)?;
        xfer.xfer_real(&mut self.mass)?;
        xfer.xfer_object_id(&mut self.current_overlap)?;
        xfer.xfer_object_id(&mut self.previous_overlap)?;
        xfer.xfer_unsigned_int(&mut self.motive_force_expires)?;
        xfer.xfer_real(&mut self.extra_bounciness)?;
        xfer.xfer_real(&mut self.extra_friction)?;
        if !crate::logic::modules::plant(2209) {
            xfer.xfer_real(&mut self.vel_mag)?;
        }
        Ok(())
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::OnObjectCreated => {
                // `m_pui = getPui(getObject())`: a projectile's first module with a projectile update interface
                self.pui = None;
                if world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_PROJECTILE)) {
                    for idx in world.all_modules(us) {
                        if idx != me.index && world.module_call(us, idx, ModuleCall::HasProjectileUpdateInterface).as_bool() {
                            self.pui = Some(idx);
                            break;
                        }
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::OnCollide { other, loc, normal } => {
                if let Err(e) = self.on_collide(world, me, other, loc, normal) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::WouldLikeToCollideWith { .. } => ModuleReply::Bool(false),
            ModuleCall::PhysicsApplyForce(f) => {
                self.apply_force(world, us, &f);
                ModuleReply::Done
            }
            ModuleCall::PhysicsApplyShock(f) => {
                self.apply_shock(world, us, &f);
                ModuleReply::Done
            }
            ModuleCall::PhysicsApplyRandomRotation => {
                self.apply_random_rotation(world);
                ModuleReply::Done
            }
            ModuleCall::PhysicsApplyMotiveForce(f) => {
                self.apply_motive_force(world, us, &f);
                ModuleReply::Done
            }
            ModuleCall::PhysicsClearAcceleration => {
                self.accel = Coord3D::default();
                ModuleReply::Done
            }
            ModuleCall::PhysicsResetDynamicPhysics => {
                self.reset_dynamic_physics(world, us);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAngles { yaw, pitch, roll } => {
                Self::set_angles(world, us, yaw, pitch, roll);
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetMass => ModuleReply::Real(self.get_mass(world, us)),
            ModuleCall::PhysicsSetMass(m) => {
                self.mass = m;
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetCenterOfMassOffset => ModuleReply::Real(self.data.center_of_mass_offset),
            ModuleCall::PhysicsGetAcceleration => ModuleReply::Coord(self.prev_accel),
            ModuleCall::PhysicsGetVelocity => ModuleReply::Coord(self.vel),
            ModuleCall::PhysicsGetVelocityMagnitude => ModuleReply::Real(self.get_velocity_magnitude()),
            ModuleCall::PhysicsGetForwardSpeed2D => ModuleReply::Real(self.get_forward_speed_2d(world, us)),
            ModuleCall::PhysicsGetForwardSpeed3D => ModuleReply::Real(self.get_forward_speed_3d(world, us)),
            ModuleCall::PhysicsGetCurrentOverlap => ModuleReply::Id(self.current_overlap),
            ModuleCall::PhysicsGetPreviousOverlap => ModuleReply::Id(self.previous_overlap),
            ModuleCall::PhysicsGetLastCollidee => ModuleReply::Id(self.last_collidee),
            ModuleCall::PhysicsAddOverlap(o) => {
                if world.objects.find(o).is_some() && self.current_overlap != o {
                    self.current_overlap = o;
                }
                ModuleReply::Done
            }
            ModuleCall::PhysicsIsMotive => ModuleReply::Bool(self.is_motive(world)),
            ModuleCall::PhysicsGetTurning => ModuleReply::Int(self.turning),
            ModuleCall::PhysicsSetTurning(t) => {
                self.turning = t;
                ModuleReply::Done
            }
            ModuleCall::PhysicsScrubVelocity2D(v) => {
                self.scrub_velocity_2d(v);
                ModuleReply::Done
            }
            ModuleCall::PhysicsScrubVelocityZ(v) => {
                self.scrub_velocity_z(v);
                ModuleReply::Done
            }
            ModuleCall::PhysicsTransferVelocityFrom(v) => {
                // `transferVelocityTo(that)`, `that`'s half: `that->m_vel.add(&m_vel); that->m_velMag = INVALID_VEL_MAG;`
                self.vel.x += v.x;
                self.vel.y += v.y;
                self.vel.z += v.z;
                self.vel_mag = INVALID_VEL_MAG;
                ModuleReply::Done
            }
            ModuleCall::PhysicsAddVelocityTo(v) => {
                self.vel.x += v.x;
                self.vel.y += v.y;
                self.vel.z += v.z;
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetPitchRate(p) => {
                self.pitch_rate = p;
                self.update_pry_flag();
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetRollRate(r) => {
                self.roll_rate = r;
                self.update_pry_flag();
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetYawRate(y) => {
                self.yaw_rate = y;
                self.update_pry_flag();
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAllowToFall(b) => {
                self.set_flag(ALLOW_TO_FALL, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetAllowToFall => ModuleReply::Bool(self.get_flag(ALLOW_TO_FALL)),
            ModuleCall::PhysicsSetStickToGround(b) => {
                self.set_flag(STICK_TO_GROUND, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAllowBouncing(b) => {
                self.set_flag(ALLOW_BOUNCE, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAllowCollideForce(b) => {
                self.set_flag(ALLOW_COLLIDE_FORCE, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAllowAirborneFriction(b) => {
                self.set_flag(APPLY_FRICTION2D_WHEN_AIRBORNE, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetImmuneToFallingDamage(b) => {
                self.set_flag(IMMUNE_TO_FALLING_DAMAGE, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetStunned(b) => {
                self.set_flag(IS_STUNNED, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetIsStunned => ModuleReply::Bool(self.get_flag(IS_STUNNED)),
            ModuleCall::PhysicsGetIsInFreeFall => ModuleReply::Bool(self.get_flag(IS_IN_FREEFALL)),
            ModuleCall::PhysicsSetIsInFreeFall(b) => {
                self.set_flag(IS_IN_FREEFALL, b);
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetExtraBounciness(b) => {
                self.extra_bounciness = b;
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetExtraFriction(f) => {
                self.extra_friction = f;
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetBounceSound => ModuleReply::Sound(self.bounce_sound.clone()),
            ModuleCall::PhysicsSetBounceSound(s) => {
                self.bounce_sound = s;
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetIgnoreCollisionsWith(o) => {
                self.ignore_collisions_with = if world.objects.find(o).is_some() { o } else { INVALID_ID };
                ModuleReply::Done
            }
            ModuleCall::PhysicsIsIgnoringCollisionsWith(id) => ModuleReply::Bool(id != INVALID_ID && id == self.ignore_collisions_with),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PhysicsBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PhysicsBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PhysicsBehavior {
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
        let d = PhysicsBehaviorModuleData::default();
        assert_eq!((d.mass, d.z_friction, d.pitch_roll_yaw_factor, d.allow_collide_force), (1.0, 0.8, 2.0, true));
        let m = PhysicsBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        // m_turning: the memory pool's zero, TURN_NONE
        assert_eq!(m.state_words()[12], TURN_NONE as u32);
        assert_eq!(m.state_words()[14] as i32, ALLOW_COLLIDE_FORCE);
        assert_eq!(height_to_speed(-1.0, 40.0), 80.0f32.sqrt());
    }
}
