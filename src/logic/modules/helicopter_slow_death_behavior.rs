// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HelicopterSlowDeathUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HelicopterSlowDeathUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HelicopterSlowDeathBehavior`: a [`SlowDeathBehavior`] for helicopters, which spiral down. Its `beginSlowDeath` is the base's, then: the
//! drawable's ambient sound stopped, the death loop sound started, a frame picked for the blade to fly off, the spiral begun at the object's
//! facing (`SpiralOrbitForwardSpeed`, self spin from `MinSelfSpin`), the locomotor's lift cut (by `FallHowFast`) and its braking capped
//! (`MaxBraking`), and a particle system attached (at a bone of the art, or at `AttachParticleLoc`). Each frame of the slow death, until it is
//! on the ground: the object spins about its centre (its transform turned by the self spin, which ping-pongs between the minimum and maximum
//! every `SelfSpinUpdateDelay`), is pushed along the spiral (a motive force, the spiral's angle and damped speed), and at the blade frame
//! throws its blade (FX and OCL at the blade's bone, and the pilot ejected from a veteran or better). It lands at its layer's height (plus
//! one) or on a tree: hit-ground FX and OCL, held, `MODELCONDITION_SPECIAL_DAMAGED`, the sound stopped. `DelayFromGroundToFinalDeath` later:
//! the final FX and OCL, the rubble object made with its transform, and the helicopter destroyed.
//!
//! The module is a [`SlowDeathBehavior`] (the base's state and methods, [`slow_death_field_parse!`](crate::slow_death_field_parse) for its data)
//! plus this class's: `BeginSlowDeath` is this class's, the other calls the base's.
//!
//! # Upstream facts kept
//!
//! - The extra `beginSlowDeath` runs even when the base's returns early (a death already begun): the sound, the blade frame and the spiral are
//!   set again.
//! - A helicopter with no AI stops after the self spin is set: no lift, no braking, no particle system.
//! - The blade frame is `GameLogicRandomValueReal(min, max)` converted to an unsigned frame count, counted down by the updates; the blade's
//!   position is the object's, or the bone's, pushed through the object's transform whenever there is a drawable (the bone or not).
//! - The self spin's delay is compared as `now - lastUpdate > delay` (an unsigned difference against a real), and only when the delay is not
//!   zero; the step is `SelfSpinUpdateAmount / LOGICFRAMES_PER_SECOND`.
//! - The landing's layer is the highest layer under the object (the position's `z` at 99999), set on the object every frame in the air.
//! - The death sound is not saved: after a load, a landing removes the handle the module last had.
//! - A `FinalRubbleObject` that is no template makes `newObject(NULL)` throw `ERROR_BAD_ARG` (recorded as an `IniErrorKind::BadArg`
//!   module error) after the final FX and OCL, before the destruction.
//! - The xfer is version 1: the version, `SlowDeathBehavior`'s, then the orbit direction, the forward angle and speed, the self spin and its
//!   direction, and the three frames.
//!
//! # Where upstream is undefined
//!
//! A frame of the fall with no physics (`getPhysics()->applyMotiveForce` through `NULL`: `NullPhysics`), a fall begun with an AI but no current
//! locomotor (`NullObject`), a blade frame out of the unsigned range (`FloatToUnsignedOutOfRange`), and a pilot ejected from an object with no
//! controlling player (`EjectPilotDie`'s), recorded in `world.cuts.module_errors`; and the base's.
//!
//! `p1w13-sonnet`, batch R48, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::{VeterancyLevel, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{INVALID_ANGLE, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_SHRUBBERY;
use crate::common::model_state::MODELCONDITION_SPECIAL_DAMAGED;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::eject_pilot_die::EjectPilotDie;
use crate::logic::modules::slow_death_behavior::{SlowDeathBehavior, SlowDeathBehaviorModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `ORBIT_DIRECTION_LEFT` (`1.0f`, kept in an `Int`).
const ORBIT_DIRECTION_LEFT: i32 = 1;

/// A list by name (`None` for `NULL`).
type Named = Option<Vec<u8>>;

/// `class HelicopterSlowDeathBehaviorModuleData : public SlowDeathBehaviorModuleData`.
#[derive(Clone, Debug)]
pub struct HelicopterSlowDeathBehaviorModuleData {
    /// The base.
    pub slow: SlowDeathBehaviorModuleData,
    /// `m_spiralOrbitTurnRate` (rad per frame), `m_spiralOrbitForwardSpeed` (per frame), `m_spiralOrbitForwardSpeedDamping`.
    pub spiral_orbit_turn_rate: f32,
    pub spiral_orbit_forward_speed: f32,
    pub spiral_orbit_forward_speed_damping: f32,
    /// `m_minSelfSpin`, `m_maxSelfSpin` (rad per frame), `m_selfSpinUpdateDelay` (frames), `m_selfSpinUpdateAmount` (rad).
    pub min_self_spin: f32,
    pub max_self_spin: f32,
    pub self_spin_update_delay: f32,
    pub self_spin_update_amount: f32,
    /// `m_fallHowFast`: "a fraction of gravity we use to modify the helicopter locomotor lift".
    pub fall_how_fast: f32,
    /// `m_minBladeFlyOffDelay`, `m_maxBladeFlyOffDelay` (frames).
    pub min_blade_fly_off_delay: f32,
    pub max_blade_fly_off_delay: f32,
    /// `m_attachParticleSystem` (by name), `m_attachParticleBone`, `m_attachParticleLoc`.
    pub attach_particle_system: Named,
    pub attach_particle_bone: Vec<u8>,
    pub attach_particle_loc: Coord3D,
    /// `m_bladeObjectName` (read, never used), `m_bladeBone`.
    pub blade_object_name: Vec<u8>,
    pub blade_bone: Vec<u8>,
    /// `m_oclEjectPilot`, `m_fxBlade`, `m_oclBlade`, `m_fxHitGround`, `m_oclHitGround`, `m_fxFinalBlowUp`, `m_oclFinalBlowUp`.
    pub ocl_eject_pilot: Named,
    pub fx_blade: Named,
    pub ocl_blade: Named,
    pub fx_hit_ground: Named,
    pub ocl_hit_ground: Named,
    pub fx_final_blow_up: Named,
    pub ocl_final_blow_up: Named,
    /// `m_delayFromGroundToFinalDeath` (frames).
    pub delay_from_ground_to_final_death: f32,
    /// `m_finalRubbleObject`.
    pub final_rubble_object: Vec<u8>,
    /// `m_maxBraking`.
    pub max_braking: f32,
    /// `m_deathSound`'s event name (empty: none).
    pub death_sound: Vec<u8>,
}

impl Default for HelicopterSlowDeathBehaviorModuleData {
    /// `HelicopterSlowDeathBehaviorModuleData()`.
    fn default() -> Self {
        HelicopterSlowDeathBehaviorModuleData {
            slow: SlowDeathBehaviorModuleData::default(),
            spiral_orbit_turn_rate: 0.0,
            spiral_orbit_forward_speed: 0.0,
            spiral_orbit_forward_speed_damping: 1.0, // no damping
            min_self_spin: 0.0,
            max_self_spin: 0.0,
            self_spin_update_delay: 0.0,
            self_spin_update_amount: 0.0,
            fall_how_fast: 0.0,
            min_blade_fly_off_delay: 0.0,
            max_blade_fly_off_delay: 0.0,
            attach_particle_system: None,
            attach_particle_bone: Vec::new(),
            attach_particle_loc: Coord3D::default(),
            blade_object_name: Vec::new(),
            blade_bone: Vec::new(),
            ocl_eject_pilot: None,
            fx_blade: None,
            ocl_blade: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            fx_final_blow_up: None,
            ocl_final_blow_up: None,
            delay_from_ground_to_final_death: 0.0,
            final_rubble_object: Vec::new(),
            max_braking: 99999.0,
            death_sound: Vec::new(),
        }
    }
}

/// `INI::parseFXList`, `INI::parseObjectCreationList` and `INI::parseParticleSystemTemplate`: the next token, `None` for `NULL` (an OCL is
/// found in the store when it is used).
fn parse_named(ini: &mut Ini<'_>) -> IniResult<Named> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

fn slow_of(d: &mut HelicopterSlowDeathBehaviorModuleData) -> &mut SlowDeathBehaviorModuleData {
    &mut d.slow
}

/// `SlowDeathBehaviorModuleData::buildFieldParse`'s table.
static SLOW_FIELD_PARSE: [FieldParse<HelicopterSlowDeathBehaviorModuleData>; 20] = crate::slow_death_field_parse!(HelicopterSlowDeathBehaviorModuleData, slow_of);

/// `HelicopterSlowDeathBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<HelicopterSlowDeathBehaviorModuleData>; 27] = [
    FieldParse::new("SpiralOrbitTurnRate", |i, d, _| { d.spiral_orbit_turn_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpiralOrbitForwardSpeed", |i, d, _| { d.spiral_orbit_forward_speed = i.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpiralOrbitForwardSpeedDamping", |i, d, _| { d.spiral_orbit_forward_speed_damping = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinSelfSpin", |i, d, _| { d.min_self_spin = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxSelfSpin", |i, d, _| { d.max_self_spin = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SelfSpinUpdateDelay", |i, d, _| { d.self_spin_update_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SelfSpinUpdateAmount", |i, d, _| { d.self_spin_update_amount = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FallHowFast", |i, d, _| { d.fall_how_fast = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinBladeFlyOffDelay", |i, d, _| { d.min_blade_fly_off_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxBladeFlyOffDelay", |i, d, _| { d.max_blade_fly_off_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AttachParticle", |i, d, _| { d.attach_particle_system = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("AttachParticleBone", |i, d, _| { d.attach_particle_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("AttachParticleLoc", |i, d, _| { d.attach_particle_loc = i.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("BladeObjectName", |i, d, _| { d.blade_object_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("BladeBoneName", |i, d, _| { d.blade_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OCLEjectPilot", |i, d, _| { d.ocl_eject_pilot = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXBlade", |i, d, _| { d.fx_blade = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLBlade", |i, d, _| { d.ocl_blade = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXHitGround", |i, d, _| { d.fx_hit_ground = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLHitGround", |i, d, _| { d.ocl_hit_ground = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXFinalBlowUp", |i, d, _| { d.fx_final_blow_up = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLFinalBlowUp", |i, d, _| { d.ocl_final_blow_up = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DelayFromGroundToFinalDeath", |i, d, _| { d.delay_from_ground_to_final_death = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FinalRubbleObject", |i, d, _| { d.final_rubble_object = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "SoundDeathLoop",
        |i, d, _| {
            // `INI::parseAudioEventRTS`: the event's name unless it is `NoSound`
            let token = i.get_next_token(None)?;
            if !token.eq_ignore_ascii_case(b"NoSound") {
                d.death_sound = token;
            }
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("MaxBraking", |i, d, _| { d.max_braking = i.parse_acceleration_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl HelicopterSlowDeathBehaviorModuleData {
    /// `buildFieldParse(p)`: `SlowDeathBehaviorModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, HelicopterSlowDeathBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&SLOW_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HelicopterSlowDeathBehaviorModuleData> {
        let mut data = HelicopterSlowDeathBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HelicopterSlowDeathBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HelicopterSlowDeathBehavior(Arc::new(HelicopterSlowDeathBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `(UnsignedInt)x`: undefined outside (-1, 2^32).
fn real_to_unsigned(x: f32) -> Option<u32> {
    (x > -1.0 && x < 4_294_967_296.0).then_some(x as u32)
}

/// `class HelicopterSlowDeathBehavior : public SlowDeathBehavior`.
#[derive(Clone, Debug)]
pub struct HelicopterSlowDeathBehavior {
    /// The base.
    base: SlowDeathBehavior,
    /// The data (this class's part; the base holds its own).
    data: Arc<HelicopterSlowDeathBehaviorModuleData>,
    /// `m_orbitDirection`: "-1 or +1".
    orbit_direction: i32,
    /// `m_forwardAngle`: "angle of the direction forward we want to move in our downspin".
    forward_angle: f32,
    /// `m_forwardSpeed`: "the speed we're travelling forward in our spiral orbit".
    forward_speed: f32,
    /// `m_selfSpin`: "rads per frame that we change our facing direction".
    self_spin: f32,
    /// `m_selfSpinTowardsMax`.
    self_spin_towards_max: bool,
    /// `m_lastSelfSpinUpdateFrame`, `m_bladeFlyOffFrame`, `m_hitGroundFrame`.
    last_self_spin_update_frame: u32,
    blade_fly_off_frame: u32,
    hit_ground_frame: u32,
    /// `m_deathSound`'s playing handle (not saved).
    death_sound_handle: u32,
}

impl PartialEq for HelicopterSlowDeathBehavior {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words() && self.death_sound_handle == other.death_sound_handle
    }
}

impl HelicopterSlowDeathBehavior {
    /// `getInterfaceMask()`: `SlowDeathBehavior`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half: the base's (`INI_INVALID_DATA` for null data or a probability modifier under 1), then this class's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<HelicopterSlowDeathBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SlowDeathBehavior::unconstructed(object, Some(Arc::new(data.slow.clone())))?;
        Ok(HelicopterSlowDeathBehavior {
            base,
            data,
            orbit_direction: ORBIT_DIRECTION_LEFT,
            forward_angle: 0.0,
            forward_speed: 0.0,
            self_spin: 0.0,
            self_spin_towards_max: false,
            last_self_spin_update_frame: 0,
            blade_fly_off_frame: 0,
            hit_ground_frame: 0,
            death_sound_handle: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.base.update_module()
    }

    /// The state the test compares: the base's, then this class's eight fields.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.base.state_words();
        w.extend([
            self.orbit_direction as u32,
            self.forward_angle.to_bits(),
            self.forward_speed.to_bits(),
            self.self_spin.to_bits(),
            u32::from(self.self_spin_towards_max),
            self.last_self_spin_update_frame,
            self.blade_fly_off_frame,
            self.hit_ground_frame,
        ]);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the base's lists, then `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.find_effects(world);
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: this class's version, `SlowDeathBehavior::xfer`, then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        update_behavior::xfer_with(&mut self.base, update, xfer)?;
        xfer.xfer_int(&mut self.orbit_direction)?;
        xfer.xfer_real(&mut self.forward_angle)?;
        xfer.xfer_real(&mut self.forward_speed)?;
        if !crate::logic::modules::plant(2181) {
            xfer.xfer_real(&mut self.self_spin)?;
        }
        xfer.xfer_bool(&mut self.self_spin_towards_max)?;
        xfer.xfer_unsigned_int(&mut self.last_self_spin_update_frame)?;
        xfer.xfer_unsigned_int(&mut self.blade_fly_off_frame)?;
        xfer.xfer_unsigned_int(&mut self.hit_ground_frame)?;
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The OCL by name if the store has it (`INI::parseObjectCreationList` stored `NULL` for a name that is no list).
    fn known_ocl(world: &mut World, ocl: &Named) -> Option<Vec<u8>> {
        let statics = world.statics.clone();
        ocl.clone().filter(|n| statics.object_creation_list_store.find_object_creation_list(&mut world.names, n).is_some())
    }

    /// `FXList::doFXObj(fx, copter)` and `ObjectCreationList::create(ocl, copter, NULL)`.
    fn fx_and_ocl(world: &mut World, us: ObjectId, fx: &Named, ocl: &Named) {
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: us });
        }
        if let Some(ocl) = Self::known_ocl(world, ocl) {
            world.cut(CutCall::OclCreate { object: us, ocl });
        }
    }

    /// `beginSlowDeath(damageInfo)`: the base's, then the spiral's start.
    fn begin_slow_death(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        // "extending functionality"
        self.base.begin_slow_death(world, me);
        let us = me.object;
        let d = self.data.clone();
        // "stop the current movement sound"
        if world.has_drawable(us) && !crate::logic::modules::plant(2182) {
            world.emit(ObjectClientEvent::DrawableStopAmbientSound { object_id: us });
        }
        if !d.death_sound.is_empty() {
            self.death_sound_handle = world.audio_add_event(d.death_sound.clone(), us);
        }
        // "pick a frame we will fly the blade off at"
        let r = world.random.game_logic_random_value_real(d.min_blade_fly_off_delay, d.max_blade_fly_off_delay);
        self.blade_fly_off_frame = real_to_unsigned(r).ok_or(Undefined::FloatToUnsignedOutOfRange)?;
        // "for now, make it always fall to the left"
        self.orbit_direction = ORBIT_DIRECTION_LEFT;
        // "record the current forward angle of the object to be the start angle of our downward spin circle angle"
        self.forward_angle = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
        self.forward_speed = d.spiral_orbit_forward_speed;
        // "start our self spinning at the min self spin rate", "we will start off changing the self spin towards the MaxSelfSpin"
        self.self_spin = d.min_self_spin;
        self.self_spin_towards_max = true;
        // "the fall of the helicopter"
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return Ok(()) };
        if !world.module_call(us, ai, ModuleCall::AiHasCurLocomotor).as_bool() {
            return Err(Undefined::NullObject);
        }
        let fall = if crate::logic::modules::plant(2183) { d.fall_how_fast } else { 1.0 - d.fall_how_fast };
        world.module_call(us, ai, ModuleCall::AiLocomotorSetMaxLift(-world.cuts.globals.gravity * fall));
        world.module_call(us, ai, ModuleCall::AiLocomotorSetMaxBraking(d.max_braking));
        // "attach particle system to bone if present"
        if let Some(template) = d.attach_particle_system.clone() {
            let id = world.create_particle_system_unplaced(template);
            if !d.attach_particle_bone.is_empty() {
                if let Some(pos) = world.contain_drawable_pristine_bone(us, &d.attach_particle_bone) {
                    world.particle_system_set_position(id, pos);
                }
            } else {
                // "use location coord specified ... it will be zero if not given which is center of obj anyway"
                world.particle_system_set_position(id, d.attach_particle_loc);
            }
            world.body_particle_attach_to_object(id, us);
        }
        Ok(())
    }

    /// The part of `update()` after the base's.
    fn update_spiral(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        // "get out of here if we're not activated yet"
        if !self.base.is_slow_death_activated() {
            return Ok(());
        }
        let d = self.data.clone();
        let copter = me.object;
        let now = world.frame();
        // "update self spin and orbit if we haven't hit the ground yet"
        if self.hit_ground_frame == 0 {
            let mut xfrm = world.objects.find(copter).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
            xfrm.in_place_pre_rotate_z(self.self_spin * self.orbit_direction as f32);
            world.set_transform_matrix(copter, &xfrm);
            // "we will ping pong back and forth between the MinSelfSpin and MaxSelfSpin defined in INI"
            if d.self_spin_update_delay != 0.0 && now.wrapping_sub(self.last_self_spin_update_frame) as f32 > d.self_spin_update_delay {
                let step = d.self_spin_update_amount / LOGICFRAMES_PER_SECOND as f32;
                if self.self_spin_towards_max {
                    self.self_spin += step;
                    if self.self_spin > d.max_self_spin {
                        self.self_spin = d.max_self_spin; // "cap at max"
                        self.self_spin_towards_max = false;
                    }
                } else {
                    self.self_spin -= if crate::logic::modules::plant(2184) { -step } else { step };
                    if self.self_spin < d.min_self_spin {
                        self.self_spin = d.min_self_spin; // "cap at min"
                        self.self_spin_towards_max = true;
                    }
                }
                self.last_self_spin_update_frame = now;
            }
            let physics = world.objects.find(copter).and_then(|o| o.physics).ok_or_else(|| IniError::undefined(Undefined::NullPhysics))?;
            // "apply a force to the helicopter pushing it in a forward motion according to the forward angle & speed"
            let force = Coord3D { x: trig::cos(self.forward_angle) * self.forward_speed, y: trig::sin(self.forward_angle) * self.forward_speed, z: 0.0 };
            world.module_call(copter, physics, ModuleCall::PhysicsApplyMotiveForce(force));
            self.forward_angle += d.spiral_orbit_turn_rate * self.orbit_direction as f32;
            self.forward_speed *= if crate::logic::modules::plant(2185) { 1.0 } else { d.spiral_orbit_forward_speed_damping };
            // "is it time to have the blade fly off"
            if self.blade_fly_off_frame > 0 {
                self.blade_fly_off_frame -= 1;
                if self.blade_fly_off_frame == 0 {
                    self.fly_off_blade(world, copter)?;
                }
            }
        }
        // "Here we want to make sure we crash if we collide with a tree on the way down"
        let mut hit_a_tree = false;
        if self.hit_ground_frame == 0 {
            if let Some(phys) = world.objects.find(copter).and_then(|o| o.physics) {
                let tree = world.module_call(copter, phys, ModuleCall::PhysicsGetLastCollidee).as_id();
                if tree != INVALID_ID && world.objects.find(tree).is_some_and(|t| t.is_kind_of(KINDOF_SHRUBBERY)) {
                    hit_a_tree = true;
                }
            }
        }
        // "when we hit the ground"
        let pos = world.objects.find(copter).map(|o| o.get_position()).unwrap_or_default();
        if self.hit_ground_frame == 0 {
            // "if we haven't yet hit the ground, adjust our layer properly so we crash on bridges"
            let tmp = Coord3D { z: 99999.0, ..pos };
            let new_layer = world.terrain_get_highest_layer_for_destination(&tmp);
            world.contain_set_layer(copter, new_layer);
            let ground = world.terrain_get_layer_height(tmp.x, tmp.y, new_layer);
            let reach = if crate::logic::modules::plant(2186) { 0.0 } else { 1.0 };
            if pos.z <= ground + reach || hit_a_tree {
                // "mark the frame we hit the ground on"
                self.hit_ground_frame = now;
                Self::fx_and_ocl(world, copter, &d.fx_hit_ground, &d.ocl_hit_ground);
                // "hold the copter in place now"
                world.body_set_disabled(copter, DISABLED_HELD);
                // "special damage state ... has the blades no on anymore"
                world.set_model_condition_state(copter, MODELCONDITION_SPECIAL_DAMAGED);
                // "Stop the sound from playing."
                if !crate::logic::modules::plant(2187) {
                    world.audio_remove_event(self.death_sound_handle);
                }
            }
        }
        // "if we're on the ground, see if it's time for our final boom"
        if self.hit_ground_frame != 0 && now.wrapping_sub(self.hit_ground_frame) as f32 > d.delay_from_ground_to_final_death {
            Self::fx_and_ocl(world, copter, &d.fx_final_blow_up, &d.ocl_final_blow_up);
            // "we are now all done, destroy us and make a rubble shell copter"
            let Some(template) = world.thing_factory_find_template(&d.final_rubble_object) else {
                // `newObject(NULL, ...)`: "throw ERROR_BAD_ARG"
                return Err(IniError::new(IniErrorKind::BadArg));
            };
            let team = world.get_team(copter);
            if let Ok(rubble) = world.thing_factory_new_object(template, team, ObjectStatusMaskType::default()) {
                if !crate::logic::modules::plant(2188) {
                    let mx = world.objects.find(copter).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
                    world.contain_set_transform_matrix(rubble, &mx);
                }
            }
            // "destroy the copter finally"
            world.body_destroy_object(copter);
        }
        Ok(())
    }

    /// The blade's flight: its FX and OCL at the blade's position, and the pilot ejected from a veteran or better.
    fn fly_off_blade(&mut self, world: &mut World, copter: ObjectId) -> Result<(), IniError> {
        let d = self.data.clone();
        let mut blade_pos = world.objects.find(copter).map(|o| o.get_position()).unwrap_or_default();
        // "get the blade position from the bone in the model"
        if world.has_drawable(copter) {
            if let Some(b) = world.contain_drawable_pristine_bone(copter, &d.blade_bone) {
                blade_pos = b;
            }
            blade_pos = world.contain_convert_bone_pos_to_world_pos(copter, &blade_pos);
        }
        // "run the fx at the blade position"
        if let Some(fx) = d.fx_blade.clone() {
            world.emit(ObjectClientEvent::FxListDoFxPos { fx, position: blade_pos });
        }
        if let Some(ocl) = Self::known_ocl(world, &d.ocl_blade) {
            world.cut(CutCall::OclCreateAtPrimary { primary: copter, ocl, primary_pos: blade_pos, angle: INVALID_ANGLE });
        }
        // "if we have (potentially) a pilot ejection, do it here."
        let veteran = world.objects.find(copter).is_some_and(|o| o.experience.level as i32 > VeterancyLevel::Regular as i32);
        if let Some(ocl) = Self::known_ocl(world, &d.ocl_eject_pilot).filter(|_| veteran && !crate::logic::modules::plant(2189)) {
            EjectPilotDie::eject_pilot(world, Some(&ocl), copter, INVALID_ID)?;
        }
        Ok(())
    }
}

impl UpdateBehavior for HelicopterSlowDeathBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update_base()
    }
    /// `update()`: the base's, then the spiral (never sleeps).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        UpdateBehavior::update(&mut self.base, world, me);
        if let Err(e) = self.update_spiral(world, me) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.construct(world, me);
    }
    /// `BeginSlowDeath` is this class's; `OnDie` is the base's, run with this class lent to the slot (the base's `onDie` may choose this
    /// module, whose `beginSlowDeath` is this class's); the slow death interface's other calls are the base's.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::HelicopterSlowDeathBehavior, Self::from_body, |w, m| m.base.on_die(w, me, &info));
                ModuleReply::Done
            }
            ModuleCall::BeginSlowDeath(_) => {
                let r = crate::logic::modules::lend_clone(world, me, self, UpdateBody::HelicopterSlowDeathBehavior, Self::from_body, |w, m| m.begin_slow_death(w, me));
                if let Err(u) = r {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => self.base.other_call(world, me, call),
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HelicopterSlowDeathBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HelicopterSlowDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for HelicopterSlowDeathBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer`: see [`HelicopterSlowDeathBehavior::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.base.update_module().clone();
        let r = self.xfer_with(&mut update, xfer);
        *self.base.update_module_mut() = update;
        r
    }
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = HelicopterSlowDeathBehaviorModuleData::default();
        assert_eq!((d.spiral_orbit_forward_speed_damping, d.max_braking, d.slow.probability_modifier), (1.0, 99999.0, 10));
        let m = HelicopterSlowDeathBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[5..], [1, 0, 0, 0, 0, 0, 0, 0]);
    }
}
