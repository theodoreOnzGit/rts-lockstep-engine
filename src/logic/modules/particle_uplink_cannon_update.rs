// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ParticleUplinkCannonUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ParticleUplinkCannonUpdate`: the particle cannon superweapon. As its special power's ready frame nears, the building charges, raises
//! its antenna and readies (`BeginChargeTime`, `RaiseAntennaTime`, `ReadyDelayTime` before the ready frame), and its client effects grow:
//! flares at the outer nodes (the bones `OuterEffectBoneName01`...), lasers from them to the connector, flares at the connector and at the
//! laser's base. Fired (`initiateIntentToDoSpecialPower`), it sends a beam up from the fire bone and, `BeamTravelTime` later, one down
//! onto the target, which it moves: a human player drives it (`setSpecialPowerOverridableDestination`, faster on a double click), a script
//! along a waypoint path, a computer player in an S-curve across the target (`SwathOfDeathDistance`, `SwathOfDeathAmplitude`). The beam
//! leaves `TotalScorchMarks` scorches and `TotalDamagePulses` pulses of `DamagePerSecond` over `TotalFiringTime` to what is alive within
//! its radius, each pulse maybe leaving a remnant object; it then decays (`WidthGrowTime`) and the building packs up. A building that is
//! sold, or underpowered, EMP'd, subdued or hacked while it fires, stops.
//!
//! # Upstream facts kept
//!
//! - The data class derives from `ModuleData`, not `UpdateModuleData`; `DoubleClickToFastDriveDelay`'s default, 500, is frames (only a
//!   parsed value is a duration).
//! - `update()` is always awake (`UPDATE_SLEEP_NONE`), even with invalid settings ("can't return UPDATE_SLEEP_FOREVER unless we are
//!   sleepy"); it runs while the building is subdued, underpowered, EMP'd or hacked (`getDisabledTypesToProcess`).
//! - The frame thresholds are `UnsignedInt` differences: before the frames they subtract, they wrap.
//! - `TotalFiringTime` is divided by `LOGICFRAMES_PER_SECOND` as integers before it becomes seconds.
//! - `createGroundToOrbitLaser` clears the orbit-to-target beam's id, not its own, when it destroys its old beam; `calculateUpBonePositions`
//!   tests the connector's bone name for the fire bone too; the effects are torn down and rebuilt on every status change.
//! - The beam's radius is the client's (`LaserUpdate::getCurrentLaserRadius`, a cut point): the damage and scorch radii follow it.
//! - The sounds and the client's drawable ids are not saved but for the beams' ids, which are; `m_frames` is saved and never used.
//! - The destructor's `killEverything` is not run here (the world has no hook for a module's destruction).
//! - Unchecked upstream, so undefined here: the special power module never found (`Undefined::NullSpecialPowerModule`), a `NULL` target
//!   position for a human's or a computer's shot (`Undefined::NullLocation`), a building with no controlling player when it scorches
//!   (`Undefined::NullControllingPlayer`), a waypoint with no next link (`Undefined::NullObject`), more than `MAX_OUTER_NODES` outer
//!   bones (`Undefined::ArrayIndexOutOfRange`), and a next scorch or pulse frame that is no `UnsignedInt` (`Undefined::FloatToUnsignedOutOfRange`).
//!
//! `p1w13-sonnet`, batch R75, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DisabledMaskType, DISABLED_EMP, DISABLED_HACKED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED};
use crate::common::game_common::{ObjectShroudStatus, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{DrawableId, SCORCH_1, SCORCH_4};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_DEPLOYED, MODELCONDITION_PACKING, MODELCONDITION_UNPACKING};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageType, DamageTypeFlags, DeathType, DAMAGE_LASER, DEATH_NAMES};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::spectre_gunship_deployment_update::COMMAND_FIRED_BY_SCRIPT;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `MAX_OUTER_NODES`.
pub const MAX_OUTER_NODES: usize = 16;
/// `DEATH_LASERED`.
const DEATH_LASERED: DeathType = 9;
/// `PI` and `TWO_PI` (BaseType.h: `3.14159265359f` and `6.28318530718f`, the same floats as these).
const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = std::f32::consts::TAU;
/// `FROM_CENTER_2D` (`DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;

/// `PUCStatus`.
pub const STATUS_IDLE: i32 = 0;
pub const STATUS_CHARGING: i32 = 1;
pub const STATUS_PREPARING: i32 = 2;
pub const STATUS_ALMOST_READY: i32 = 3;
pub const STATUS_READY_TO_FIRE: i32 = 4;
pub const STATUS_PREFIRE: i32 = 5;
pub const STATUS_FIRING: i32 = 6;
pub const STATUS_POSTFIRE: i32 = 7;
pub const STATUS_PACKING: i32 = 8;

/// `LaserStatus`.
pub const LASERSTATUS_NONE: i32 = 0;
pub const LASERSTATUS_BORN: i32 = 1;
pub const LASERSTATUS_DECAYING: i32 = 2;
pub const LASERSTATUS_DEAD: i32 = 3;

/// `IntensityTypes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intensity {
    Light,
    Medium,
    Intense,
}

/// `class ParticleUplinkCannonUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleUplinkCannonUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_beginChargeFrames`, `m_raiseAntennaFrames`, `m_readyDelayFrames`, `m_widthGrowFrames`, `m_beamTravelFrames`, `m_totalFiringFrames`,
    /// `m_framesBetweenLaunchFXRefresh`.
    pub begin_charge_frames: u32,
    pub raise_antenna_frames: u32,
    pub ready_delay_frames: u32,
    pub width_grow_frames: u32,
    pub beam_travel_frames: u32,
    pub total_firing_frames: u32,
    pub frames_between_launch_fx_refresh: u32,
    /// `m_outerEffectBaseBoneName`, `m_outerEffectNumBones`.
    pub outer_effect_base_bone_name: Vec<u8>,
    pub outer_effect_num_bones: u32,
    /// The outer nodes' flares: light, medium, intense.
    pub outer_nodes_flares: [Vec<u8>; 3],
    /// `m_connectorBoneName`, the connector's lasers (medium, intense) and flares (medium, intense).
    pub connector_bone_name: Vec<u8>,
    pub connector_lasers: [Vec<u8>; 2],
    pub connector_flares: [Vec<u8>; 2],
    /// The laser base's flares: light, medium, intense.
    pub laser_base_flares: [Vec<u8>; 3],
    /// `m_fireBoneName`, `m_particleBeamLaserName`.
    pub fire_bone_name: Vec<u8>,
    pub particle_beam_laser_name: Vec<u8>,
    /// `m_groundHitFX`, `m_beamLaunchFX` (by name; `None` is `NULL`).
    pub ground_hit_fx: Option<Vec<u8>>,
    pub beam_launch_fx: Option<Vec<u8>>,
    /// `m_swathOfDeathDistance`, `m_swathOfDeathAmplitude`, `m_totalScorchMarks`, `m_scorchMarkScalar`.
    pub swath_of_death_distance: f32,
    pub swath_of_death_amplitude: f32,
    pub total_scorch_marks: u32,
    pub scorch_mark_scalar: f32,
    /// `m_totalDamagePulses`, `m_damagePerSecond`, `m_damageType`, `m_deathType`, `m_damageRadiusScalar`, `m_revealRange`.
    pub total_damage_pulses: u32,
    pub damage_per_second: f32,
    pub damage_type: DamageType,
    pub death_type: DeathType,
    pub damage_radius_scalar: f32,
    pub reveal_range: f32,
    /// `m_specialPowerTemplate`, by name (found in the store when used; an unknown name, and `None`, is `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    /// The sounds: powering up, unpacking to ready, firing to idle, annihilation.
    pub powerup_sound_name: Vec<u8>,
    pub unpack_to_ready_sound_name: Vec<u8>,
    pub firing_to_idle_sound_name: Vec<u8>,
    pub annihilation_sound_name: Vec<u8>,
    /// `m_damagePulseRemnantObjectName`.
    pub damage_pulse_remnant_object_name: Vec<u8>,
    /// `m_manualDrivingSpeed`, `m_manualFastDrivingSpeed`, `m_doubleClickToFastDriveDelay`.
    pub manual_driving_speed: f32,
    pub manual_fast_driving_speed: f32,
    pub double_click_to_fast_drive_delay: u32,
}

impl Default for ParticleUplinkCannonUpdateModuleData {
    /// `ParticleUplinkCannonUpdateModuleData()`.
    fn default() -> Self {
        ParticleUplinkCannonUpdateModuleData {
            update: UpdateModuleData::new(),
            begin_charge_frames: 0,
            raise_antenna_frames: 0,
            ready_delay_frames: 0,
            width_grow_frames: 0,
            beam_travel_frames: 0,
            total_firing_frames: 0,
            frames_between_launch_fx_refresh: 30,
            outer_effect_base_bone_name: Vec::new(),
            outer_effect_num_bones: 0,
            outer_nodes_flares: Default::default(),
            connector_bone_name: Vec::new(),
            connector_lasers: Default::default(),
            connector_flares: Default::default(),
            laser_base_flares: Default::default(),
            fire_bone_name: Vec::new(),
            particle_beam_laser_name: Vec::new(),
            ground_hit_fx: None,
            beam_launch_fx: None,
            swath_of_death_distance: 0.0,
            swath_of_death_amplitude: 0.0,
            total_scorch_marks: 0,
            scorch_mark_scalar: 1.0,
            total_damage_pulses: 0,
            damage_per_second: 0.0,
            damage_type: DAMAGE_LASER,
            death_type: DEATH_LASERED,
            damage_radius_scalar: 1.0,
            reveal_range: 0.0,
            special_power_template: None,
            powerup_sound_name: Vec::new(),
            unpack_to_ready_sound_name: Vec::new(),
            firing_to_idle_sound_name: Vec::new(),
            annihilation_sound_name: Vec::new(),
            damage_pulse_remnant_object_name: Vec::new(),
            manual_driving_speed: 0.0,
            manual_fast_driving_speed: 0.0,
            double_click_to_fast_drive_delay: 500,
        }
    }
}

/// `INI::parseFXList`: `None` is no list; any other name is looked up when used.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

type D = ParticleUplinkCannonUpdateModuleData;

static FIELD_PARSE: [FieldParse<D>; 42] = [
    FieldParse::new("SpecialPowerTemplate", |i, d: &mut D, _| { d.special_power_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("BeginChargeTime", |i, d: &mut D, _| { d.begin_charge_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RaiseAntennaTime", |i, d: &mut D, _| { d.raise_antenna_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ReadyDelayTime", |i, d: &mut D, _| { d.ready_delay_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("WidthGrowTime", |i, d: &mut D, _| { d.width_grow_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BeamTravelTime", |i, d: &mut D, _| { d.beam_travel_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("TotalFiringTime", |i, d: &mut D, _| { d.total_firing_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RevealRange", |i, d: &mut D, _| { d.reveal_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("OuterEffectBoneName", |i, d: &mut D, _| { d.outer_effect_base_bone_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OuterEffectNumBones", |i, d: &mut D, _| { d.outer_effect_num_bones = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OuterNodesLightFlareParticleSystem", |i, d: &mut D, _| { d.outer_nodes_flares[0] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OuterNodesMediumFlareParticleSystem", |i, d: &mut D, _| { d.outer_nodes_flares[1] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OuterNodesIntenseFlareParticleSystem", |i, d: &mut D, _| { d.outer_nodes_flares[2] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ConnectorBoneName", |i, d: &mut D, _| { d.connector_bone_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ConnectorMediumLaserName", |i, d: &mut D, _| { d.connector_lasers[0] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ConnectorIntenseLaserName", |i, d: &mut D, _| { d.connector_lasers[1] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ConnectorMediumFlare", |i, d: &mut D, _| { d.connector_flares[0] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ConnectorIntenseFlare", |i, d: &mut D, _| { d.connector_flares[1] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("FireBoneName", |i, d: &mut D, _| { d.fire_bone_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("LaserBaseLightFlareParticleSystemName", |i, d: &mut D, _| { d.laser_base_flares[0] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("LaserBaseMediumFlareParticleSystemName", |i, d: &mut D, _| { d.laser_base_flares[1] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("LaserBaseIntenseFlareParticleSystemName", |i, d: &mut D, _| { d.laser_base_flares[2] = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ParticleBeamLaserName", |i, d: &mut D, _| { d.particle_beam_laser_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("SwathOfDeathDistance", |i, d: &mut D, _| { d.swath_of_death_distance = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SwathOfDeathAmplitude", |i, d: &mut D, _| { d.swath_of_death_amplitude = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TotalScorchMarks", |i, d: &mut D, _| { d.total_scorch_marks = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScorchMarkScalar", |i, d: &mut D, _| { d.scorch_mark_scalar = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("BeamLaunchFX", |i, d: &mut D, _| { d.beam_launch_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DelayBetweenLaunchFX", |i, d: &mut D, _| { d.frames_between_launch_fx_refresh = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GroundHitFX", |i, d: &mut D, _| { d.ground_hit_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DamagePerSecond", |i, d: &mut D, _| { d.damage_per_second = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TotalDamagePulses", |i, d: &mut D, _| { d.total_damage_pulses = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DamageType", |i, d: &mut D, _| { d.damage_type = DamageTypeFlags::parse_single_bit_from_ini(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DeathType", |i, d: &mut D, _| { d.death_type = i.parse_index_list(&DEATH_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::new("DamageRadiusScalar", |i, d: &mut D, _| { d.damage_radius_scalar = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PoweringUpSoundLoop", |i, d: &mut D, _| { d.powerup_sound_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("UnpackToIdleSoundLoop", |i, d: &mut D, _| { d.unpack_to_ready_sound_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("FiringToPackSoundLoop", |i, d: &mut D, _| { d.firing_to_idle_sound_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("GroundAnnihilationSoundLoop", |i, d: &mut D, _| { d.annihilation_sound_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("DamagePulseRemnantObjectName", |i, d: &mut D, _| { d.damage_pulse_remnant_object_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ManualDrivingSpeed", |i, d: &mut D, _| { d.manual_driving_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// The two fields after `ManualDrivingSpeed` (a second table: the first is at its size).
static FIELD_PARSE_2: [FieldParse<D>; 3] = [
    FieldParse::new("ManualFastDrivingSpeed", |i, d: &mut D, _| { d.manual_fast_driving_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "DoubleClickToFastDriveDelay",
        |i, d: &mut D, _| { d.double_click_to_fast_drive_delay = i.parse_duration_unsigned_int()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ParticleUplinkCannonUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, D>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        p.add(&FIELD_PARSE_2)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<D> {
        let mut data = D::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, D::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `ParticleUplinkCannonUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ParticleUplinkCannonUpdate(Arc::new(D::friend_new_module_data(ini)?)))
}

/// An `AudioEventRTS` of the module's: its name (set by `onObjectCreated`) and its playing handle.
#[derive(Clone, Debug, Default, PartialEq)]
struct Sound {
    name: Vec<u8>,
    handle: u32,
}

/// `class ParticleUplinkCannonUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct ParticleUplinkCannonUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<D>,
    /// `m_specialPowerModule` (`NULL` until `onObjectCreated` finds it).
    special_power_module: Option<ModuleIndex>,
    /// `m_powerupSound`, `m_unpackToReadySound`, `m_firingToIdleSound`, `m_annihilationSound`.
    powerup_sound: Sound,
    unpack_to_ready_sound: Sound,
    firing_to_idle_sound: Sound,
    annihilation_sound: Sound,
    /// `m_outerNodeOrientations`, `m_outerNodePositions`.
    outer_node_orientations: [Matrix3D; MAX_OUTER_NODES],
    outer_node_positions: [Coord3D; MAX_OUTER_NODES],
    /// `m_connectorNodePosition`, `m_laserOriginPosition`, `m_initialTargetPosition`, `m_currentTargetPosition`,
    /// `m_overrideTargetDestination`.
    connector_node_position: Coord3D,
    laser_origin_position: Coord3D,
    initial_target_position: Coord3D,
    current_target_position: Coord3D,
    override_target_destination: Coord3D,
    /// `m_status` (`PUCStatus`), `m_laserStatus` (`LaserStatus`), `m_frames`.
    status: i32,
    laser_status: i32,
    frames: u32,
    /// `m_outerSystemIDs`, `m_laserBeamIDs`, `m_groundToOrbitBeamID`, `m_orbitToTargetBeamID`, `m_connectorSystemID`, `m_laserBaseSystemID`.
    outer_system_ids: [u32; MAX_OUTER_NODES],
    laser_beam_ids: [DrawableId; MAX_OUTER_NODES],
    ground_to_orbit_beam_id: DrawableId,
    orbit_to_target_beam_id: DrawableId,
    connector_system_id: u32,
    laser_base_system_id: u32,
    /// `m_scorchMarksMade`, `m_nextScorchMarkFrame`, `m_nextLaunchFXFrame`, `m_damagePulsesMade`, `m_nextDamagePulseFrame`,
    /// `m_startAttackFrame`, `m_startDecayFrame`, `m_lastDrivingClickFrame`, `m_2ndLastDrivingClickFrame`, `m_nextDestWaypointID`.
    scorch_marks_made: u32,
    next_scorch_mark_frame: u32,
    next_launch_fx_frame: u32,
    damage_pulses_made: u32,
    next_damage_pulse_frame: u32,
    start_attack_frame: u32,
    start_decay_frame: u32,
    last_driving_click_frame: u32,
    second_last_driving_click_frame: u32,
    next_dest_waypoint_id: u32,
    /// `m_upBonesCached`, `m_defaultInfoCached`, `m_invalidSettings`, `m_manualTargetMode`, `m_scriptedWaypointMode`,
    /// `m_clientShroudedLastFrame`.
    up_bones_cached: bool,
    default_info_cached: bool,
    invalid_settings: bool,
    manual_target_mode: bool,
    scripted_waypoint_mode: bool,
    client_shrouded_last_frame: bool,
}

impl PartialEq for ParticleUplinkCannonUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.special_power_module == other.special_power_module
            && self.state_words() == other.state_words()
    }
}

fn bits(p: &Coord3D) -> [u32; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

impl ParticleUplinkCannonUpdate {
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    /// The constructor (it touches no world): idle, no laser, no ids, the positions zero and the orientations the identity
    /// (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<D>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let mut base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK4( DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_HACKED )
        let mut mask = DisabledMaskType::default();
        for t in [DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_HACKED] {
            mask.set(t, 1);
        }
        base.update_mut().set_disabled_types_to_process(mask);
        Ok(ParticleUplinkCannonUpdate {
            base,
            data,
            special_power_module: None,
            powerup_sound: Sound::default(),
            unpack_to_ready_sound: Sound::default(),
            firing_to_idle_sound: Sound::default(),
            annihilation_sound: Sound::default(),
            outer_node_orientations: [Matrix3D::IDENTITY; MAX_OUTER_NODES],
            outer_node_positions: [Coord3D::default(); MAX_OUTER_NODES],
            connector_node_position: Coord3D::default(),
            laser_origin_position: Coord3D::default(),
            initial_target_position: Coord3D::default(),
            current_target_position: Coord3D::default(),
            override_target_destination: Coord3D::default(),
            status: STATUS_IDLE,
            laser_status: LASERSTATUS_NONE,
            frames: 0,
            outer_system_ids: [0; MAX_OUTER_NODES],
            laser_beam_ids: [0; MAX_OUTER_NODES],
            ground_to_orbit_beam_id: 0,
            orbit_to_target_beam_id: 0,
            connector_system_id: 0,
            laser_base_system_id: 0,
            scorch_marks_made: 0,
            next_scorch_mark_frame: 0,
            next_launch_fx_frame: 0,
            damage_pulses_made: 0,
            next_damage_pulse_frame: 0,
            start_attack_frame: 0,
            start_decay_frame: 0,
            last_driving_click_frame: 0,
            second_last_driving_click_frame: 0,
            next_dest_waypoint_id: 0,
            up_bones_cached: false,
            default_info_cached: false,
            invalid_settings: false,
            manual_target_mode: false,
            scripted_waypoint_mode: false,
            client_shrouded_last_frame: false,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    pub fn module_data(&self) -> &Arc<D> {
        &self.data
    }

    /// The statuses, the frames, the ids, the positions and orientations (bits), the counters and frames, the flags and the four sounds'
    /// playing handles.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.status as u32, self.laser_status as u32, self.frames];
        v.extend(self.outer_system_ids);
        v.extend(self.laser_beam_ids.map(|d| d as u32));
        v.extend([self.ground_to_orbit_beam_id as u32, self.orbit_to_target_beam_id as u32, self.connector_system_id, self.laser_base_system_id]);
        for p in &self.outer_node_positions {
            v.extend(bits(p));
        }
        for m in &self.outer_node_orientations {
            v.extend(m.to_array().map(f32::to_bits));
        }
        for p in [self.connector_node_position, self.laser_origin_position, self.initial_target_position, self.current_target_position, self.override_target_destination] {
            v.extend(bits(&p));
        }
        v.extend([
            self.scorch_marks_made,
            self.next_scorch_mark_frame,
            self.next_launch_fx_frame,
            self.damage_pulses_made,
            self.next_damage_pulse_frame,
            self.start_attack_frame,
            self.start_decay_frame,
            self.last_driving_click_frame,
            self.second_last_driving_click_frame,
            self.next_dest_waypoint_id,
        ]);
        v.extend(
            [self.up_bones_cached, self.default_info_cached, self.invalid_settings, self.manual_target_mode, self.scripted_waypoint_mode, self.client_shrouded_last_frame]
                .map(u32::from),
        );
        v.extend([self.powerup_sound.handle, self.unpack_to_ready_sound.handle, self.firing_to_idle_sound.handle, self.annihilation_sound.handle]);
        v
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The special power template's id in the store (`None`: `NULL`).
    fn template_id(world: &World, name: &Option<Vec<u8>>) -> Option<u32> {
        name.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id())
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    fn special_power(&self) -> Result<ModuleIndex, IniError> {
        self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))
    }

    /// The outer effect's bone count, an index into the arrays (more than `MAX_OUTER_NODES` overruns them).
    fn num_bones(&self) -> Result<usize, IniError> {
        let n = self.data.outer_effect_num_bones as usize;
        if n > MAX_OUTER_NODES {
            return Err(IniError::undefined(Undefined::ArrayIndexOutOfRange));
        }
        Ok(n)
    }

    /// `TheAudio->removeAudioEvent(sound.getPlayingHandle())`.
    fn remove_sound(world: &mut World, s: &Sound) {
        world.audio_remove_event(s.handle);
    }

    /// `killEverything()`: the effects, the orbit-to-target beam and the four sounds.
    fn kill_everything(&mut self, world: &mut World) -> Result<(), IniError> {
        self.remove_all_effects(world)?;
        // "This laser is independent from the other effects and needs to be specially handled."
        if self.orbit_to_target_beam_id != 0 {
            if world.contain_effect_exists(self.orbit_to_target_beam_id) && !crate::logic::modules::plant(2460) {
                world.client_destroy_drawable(self.orbit_to_target_beam_id);
            }
            self.orbit_to_target_beam_id = 0;
        }
        for s in [&self.powerup_sound, &self.unpack_to_ready_sound, &self.firing_to_idle_sound, &self.annihilation_sound] {
            Self::remove_sound(world, s);
        }
        Ok(())
    }

    /// `onObjectCreated()`: the special power module, the connector and laser origin at the building, and the sounds' names (no special
    /// power named: invalid settings).
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        let d = self.data.clone();
        if d.special_power_template.is_none() {
            self.invalid_settings = true;
            return;
        }
        let template = Self::template_id(world, &d.special_power_template);
        self.special_power_module = template.and_then(|t| world.get_special_power_module(us, Some(t)));
        let pos = Self::position(world, us);
        self.connector_node_position = pos;
        self.laser_origin_position = pos;
        self.powerup_sound.name = d.powerup_sound_name.clone();
        self.unpack_to_ready_sound.name = d.unpack_to_ready_sound_name.clone();
        self.firing_to_idle_sound.name = d.firing_to_idle_sound_name.clone();
        self.annihilation_sound.name = d.annihilation_sound_name.clone();
    }

    /// `initiateIntentToDoSpecialPower(specialPowerTemplate, targetObj, targetPos, way, commandOptions)`.
    #[allow(clippy::too_many_arguments)]
    fn initiate_intent(&mut self, world: &mut World, us: ObjectId, template: u32, target_object: ObjectId, target_pos: Option<Coord3D>, way: u32, command_options: u32) -> Result<bool, IniError> {
        let d = self.data.clone();
        let sp = self.special_power()?;
        let ours = match world.module_call(us, sp, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => id as u32,
            _ => 0,
        };
        if ours != template {
            // "Check to make sure our modules are connected."
            return Ok(false);
        }
        let waypoint = world.terrain.waypoint_by_id(way).filter(|_| way != 0);
        if command_options & COMMAND_FIRED_BY_SCRIPT == 0 {
            // "All human players have manual control and must "drive" the beam around!"
            let target = target_pos.ok_or_else(|| IniError::undefined(Undefined::NullLocation))?;
            self.start_attack_frame = world.frame();
            self.laser_status = LASERSTATUS_NONE;
            self.manual_target_mode = true;
            self.initial_target_position = target;
            self.override_target_destination = target;
            self.current_target_position = target;
        } else if way != 0 {
            // "Script fired with a specific waypoint path, so set things up to follow the waypoint!"
            let now = world.frame();
            let node = waypoint.and_then(|h| world.terrain.waypoint(h).cloned()).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
            let pos = *node.location();
            self.start_attack_frame = now.max(1);
            self.scripted_waypoint_mode = true;
            self.laser_status = LASERSTATUS_NONE;
            self.set_logical_status(world, us, STATUS_READY_TO_FIRE)?;
            world.module_call(us, sp, ModuleCall::SetReadyFrame { frame: now });
            self.initial_target_position = pos;
            self.current_target_position = pos;
            self.next_dest_waypoint_id = node.id() as u32;
            let link_count = node.num_links();
            let which = world.random.game_logic_random_value(0, link_count - 1);
            let h = waypoint.expect("found above");
            let next = world.terrain.waypoint_link(h, which).map_err(|_| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
            if let Some(next) = next.and_then(|n| world.terrain.waypoint(n).cloned()) {
                self.next_dest_waypoint_id = next.id() as u32;
                self.override_target_destination = *next.location();
            }
        } else {
            // "All computer controlled players have automatic control -- the "S" curve."
            let now = world.frame();
            let pos = match target_pos {
                Some(p) => p,
                None => match world.objects.find(target_object) {
                    Some(o) => o.get_position(),
                    // (an uninitialised `Coord3D`)
                    None => return Err(IniError::undefined(Undefined::NullLocation)),
                },
            };
            self.initial_target_position = pos;
            self.start_attack_frame = now.max(1);
            self.laser_status = LASERSTATUS_NONE;
            self.set_logical_status(world, us, STATUS_READY_TO_FIRE)?;
            world.module_call(us, sp, ModuleCall::SetReadyFrame { frame: now });
        }
        self.start_decay_frame = self.start_attack_frame.wrapping_add(d.total_firing_frames);
        if let Some(own) = world.get_special_power_module(us, Some(template)) {
            world.module_call(us, own, ModuleCall::MarkSpecialPowerTriggered { location: Some(self.initial_target_position) });
        }
        Ok(true)
    }

    /// `isPowerCurrentlyInUse(command)`.
    fn is_power_currently_in_use(&self, world: &World) -> bool {
        self.start_attack_frame != 0 && self.start_attack_frame <= world.frame()
    }

    /// `setSpecialPowerOverridableDestination(loc)`: unless the building is disabled, the beam is driven there, and the clicks remembered.
    fn set_overridable_destination(&mut self, world: &World, us: ObjectId, loc: Coord3D) {
        if world.objects.find(us).is_some_and(|o| o.is_disabled()) {
            return;
        }
        self.override_target_destination = loc;
        self.manual_target_mode = true;
        if !crate::logic::modules::plant(2461) {
            self.second_last_driving_click_frame = self.last_driving_click_frame;
        }
        self.last_driving_click_frame = world.frame();
    }

    /// A frame of the beam's timeline: `birth + factor * (death - birth)`, as the `UnsignedInt` the float converts to.
    fn timeline_frame(birth: u32, death: u32, factor: f32) -> Result<u32, IniError> {
        let f = birth as f32 + factor * death.wrapping_sub(birth) as f32;
        if !(f > -1.0 && f < 4_294_967_296.0) {
            return Err(IniError::undefined(Undefined::FloatToUnsignedOutOfRange));
        }
        Ok(f as u32)
    }

    /// `update()`'s body; an error is undefined upstream.
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        if self.invalid_settings {
            // "can't return UPDATE_SLEEP_FOREVER unless we are sleepy..."
            return Ok(UPDATE_SLEEP_NONE);
        }
        let d = self.data.clone();
        let Some(o) = world.objects.find(us) else { return Ok(UPDATE_SLEEP_NONE) };
        // "must test SOLD *before* the others."
        if o.test_status(OBJECT_STATUS_SOLD) {
            if self.status != STATUS_IDLE {
                self.set_logical_status(world, us, STATUS_IDLE)?;
                self.kill_everything(world)?;
            }
            return Ok(UPDATE_SLEEP_NONE);
        }
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.is_effectively_dead() {
            return Ok(UPDATE_SLEEP_NONE);
        }
        let now = world.frame();
        let sp = self.special_power()?;
        let ready_to_fire_frame =
            if world.module_call(us, sp, ModuleCall::IsReady).as_bool() { now } else { world.module_call(us, sp, ModuleCall::GetReadyFrame).as_int(0) as u32 };
        let almost_ready_frame = ready_to_fire_frame.wrapping_sub(d.ready_delay_frames);
        let raise_antenna_frame = almost_ready_frame.wrapping_sub(d.raise_antenna_frames);
        let begin_charge_frame = raise_antenna_frame.wrapping_sub(d.begin_charge_frames);

        if self.start_attack_frame != 0 && self.start_attack_frame <= now {
            if self.start_decay_frame > now {
                let o = world.objects.find(us).expect("checked");
                if [DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_HACKED].iter().any(|&t| o.is_disabled_by_type(t)) && !crate::logic::modules::plant(2462)
                {
                    // "We must end the special power early! ABORT! ABORT!"
                    self.start_decay_frame = now;
                }
            }
            let end_decay_frame = self.start_decay_frame.wrapping_add(d.width_grow_frames);
            let orbital_birth_frame = self.start_attack_frame.wrapping_add(d.beam_travel_frames);
            let orbital_decay_start = self.start_decay_frame.wrapping_add(d.beam_travel_frames);
            let orbital_death_frame = orbital_decay_start.wrapping_add(d.width_grow_frames);
            match self.laser_status {
                LASERSTATUS_NONE => {
                    // "Means we are eligible to fire!"
                    if orbital_birth_frame <= now {
                        self.create_orbit_to_target_laser(world, d.width_grow_frames);
                        self.laser_status = LASERSTATUS_BORN;
                        self.scorch_marks_made = 0;
                        self.next_scorch_mark_frame = now;
                        self.damage_pulses_made = 0;
                        self.next_damage_pulse_frame = now;
                    }
                }
                LASERSTATUS_BORN => {
                    let beam = self.orbit_to_target_beam_id;
                    if world.contain_effect_exists(beam) && orbital_decay_start <= now {
                        if world.client_drawable_has_laser(beam) {
                            world.client_laser_set_decay_frames(beam, d.width_grow_frames);
                        }
                        self.laser_status = LASERSTATUS_DECAYING;
                    }
                }
                LASERSTATUS_DECAYING => {
                    let beam = self.orbit_to_target_beam_id;
                    if world.contain_effect_exists(beam) {
                        Self::remove_sound(world, &self.annihilation_sound);
                        if orbital_death_frame <= now {
                            world.client_destroy_drawable(beam);
                            self.orbit_to_target_beam_id = 0;
                            self.laser_status = LASERSTATUS_DEAD;
                            self.start_attack_frame = 0;
                            self.set_logical_status(world, us, STATUS_IDLE)?;
                        }
                    }
                }
                // "Nothing... this is just a wait state."
                _ => {}
            }

            let beam = self.orbit_to_target_beam_id;
            if world.contain_effect_exists(beam) && orbital_birth_frame <= now && now <= orbital_death_frame {
                self.move_beam(world, us, now, orbital_birth_frame, orbital_death_frame)?;
                // "Regardless of which method we used to set the target position, make sure the z position is at the terrain."
                self.current_target_position.z = world.terrain_get_ground_height(self.current_target_position.x, self.current_target_position.y);
                let mut orbit_position = self.current_target_position;
                orbit_position.z += 500.0;
                let (mut scorch_radius, mut damage_radius) = (0.0f32, 0.0f32);
                // "Reset the laser position"
                if world.client_drawable_has_laser(beam) {
                    world.client_laser_init(beam, orbit_position, self.current_target_position, 0);
                    let radius = world.client_laser_current_radius(beam);
                    scorch_radius = radius * d.scorch_mark_scalar;
                    damage_radius = radius * if crate::logic::modules::plant(2463) { d.scorch_mark_scalar } else { d.damage_radius_scalar };
                }
                // "Create scorch marks periodically"
                if self.next_scorch_mark_frame <= now {
                    self.scorch_marks_made = self.scorch_marks_made.wrapping_add(1);
                    // "Yes, this is just client fluff!"
                    let scorch_type = world.random.game_client_random_value(SCORCH_1, SCORCH_4);
                    world.emit(ObjectClientEvent::GameClientAddScorch { position: self.current_target_position, radius: scorch_radius, scorch_type });
                    let next_factor = self.scorch_marks_made as f32 / d.total_scorch_marks as f32;
                    self.next_scorch_mark_frame = Self::timeline_frame(orbital_birth_frame, orbital_death_frame, next_factor)?;
                    if let Some(fx) = &d.ground_hit_fx {
                        world.emit(ObjectClientEvent::FxListDoFxPos { fx: fx.clone(), position: self.current_target_position });
                    }
                    // "Also reveal vision because the owning player has full rights to watch the carnage he created!"
                    let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                    let mask = world.cuts.player_mask(player);
                    let (x, y) = (self.current_target_position.x, self.current_target_position.y);
                    world.partition_do_shroud_reveal(x, y, d.reveal_range, mask);
                    world.partition_undo_shroud_reveal(x, y, d.reveal_range, mask);
                }
                // "Handle damage pulses"
                if self.next_damage_pulse_frame <= now {
                    self.damage_pulses_made = self.damage_pulses_made.wrapping_add(1);
                    self.damage_pulse(world, us, damage_radius)?;
                    let next_factor = self.damage_pulses_made as f32 / d.total_damage_pulses as f32;
                    self.next_damage_pulse_frame = Self::timeline_frame(orbital_birth_frame, orbital_death_frame, next_factor)?;
                }
            }

            if end_decay_frame <= now {
                self.set_logical_status(world, us, STATUS_PACKING)?;
            } else if self.start_decay_frame <= now {
                self.set_logical_status(world, us, STATUS_POSTFIRE)?;
            } else {
                self.set_logical_status(world, us, STATUS_FIRING)?;
            }
        } else if ready_to_fire_frame <= now {
            self.set_logical_status(world, us, STATUS_READY_TO_FIRE)?;
        } else if almost_ready_frame <= now {
            self.set_logical_status(world, us, STATUS_ALMOST_READY)?;
        } else if raise_antenna_frame <= now {
            self.set_logical_status(world, us, STATUS_PREPARING)?;
        } else if begin_charge_frame <= now {
            self.set_logical_status(world, us, STATUS_CHARGING)?;
        } else if self.status == STATUS_ALMOST_READY {
        } else if self.status == STATUS_READY_TO_FIRE && !crate::logic::modules::plant(2464) {
            // "The particle cannon timer has been reset (sabotaged?) If so, when ready, pack it up again."
            self.set_logical_status(world, us, STATUS_PACKING)?;
        }

        if self.status == STATUS_FIRING && self.next_launch_fx_frame <= now {
            // "Generate iteration of fxlist for beam launching"
            if let Some(fx) = &d.beam_launch_fx {
                world.emit(ObjectClientEvent::FxListDoFxPos { fx: fx.clone(), position: self.laser_origin_position });
            }
            self.next_launch_fx_frame = now.wrapping_add(d.frames_between_launch_fx_refresh);
        }

        // "Handle shrouded status changes for the client player."
        if let Some(local) = world.players.local_index() {
            let shrouded = world.get_shrouded_status(us, local) != ObjectShroudStatus::Clear;
            if shrouded {
                // "We can't see it so any client effects that have been added logically needs to be removed!"
                self.remove_all_effects(world)?;
            } else {
                // "We can see it -- we only want to do anything if we just started seeing it"
                let reveal_this_frame = self.client_shrouded_last_frame != shrouded;
                if reveal_this_frame {
                    self.set_client_status(world, us, self.status, reveal_this_frame)?;
                }
            }
            self.client_shrouded_last_frame = shrouded;
        }
        Ok(UPDATE_SLEEP_NONE)
    }

    /// The beam's new target: the S-curve of a computer's shot, else driven towards the override destination (a script's along its path).
    fn move_beam(&mut self, world: &mut World, us: ObjectId, now: u32, birth: u32, death: u32) -> Result<(), IniError> {
        let d = self.data.clone();
        let me_pos = Self::position(world, us);
        if !self.manual_target_mode && !self.scripted_waypoint_mode {
            // "First determine the factor of time completed (ranges between 0.0 and 1.0)"
            let factor = now.wrapping_sub(birth) as f32 / death.wrapping_sub(birth) as f32;
            // "We're generating a swath that travels the points between sin( -1PI ) and sin( 1PI )"
            let radians = (factor * TWO_PI) - PI;
            let cx_distance = (factor * d.swath_of_death_distance) - (d.swath_of_death_distance * 0.5);
            // "Now calculate the amplitude value."
            let height = radians.sin();
            let cx_height = height * d.swath_of_death_amplitude;
            let mut to_initial = self.initial_target_position;
            to_initial.sub(&me_pos);
            let target_distance = to_initial.length();
            // "Calculate the point position assuming the target position is on the x axis relative to the building."
            self.current_target_position.x = cx_distance + target_distance;
            self.current_target_position.y = cx_height;
            self.current_target_position.z = 0.0;
            let target_distance = self.current_target_position.length();
            // "rotate that offset so it's aligned to along the building -> target vector."
            let mut building_to_target = Vector2::new(self.initial_target_position.x - me_pos.x, self.initial_target_position.y - me_pos.y);
            building_to_target.normalize();
            let mut cartesian = Vector2::new(self.current_target_position.x, self.current_target_position.y);
            cartesian.normalize();
            let dot = Vector2::dot_product(&building_to_target, &cartesian);
            // "Account for numerical errors.  Also, acos(-1.00000) is coming out QNAN"
            let inner = if -0.99999f32 > dot { -0.99999f32 } else { dot };
            let dot = if 0.99999f32 < inner { 0.99999f32 } else { inner };
            let mut angle = crate::common::trig::acos(dot);
            if building_to_target.y >= 0.0 && !crate::logic::modules::plant(2465) {
                angle = -angle;
            }
            let mut mtx = Matrix3D::from_translation(&Vector3::new(1.0, 0.0, 0.0));
            mtx.scale(target_distance);
            mtx.rotate_z(-angle);
            let v = mtx.get_x_vector();
            self.current_target_position.x = me_pos.x + v.x;
            self.current_target_position.y = me_pos.y + v.y;
        } else {
            let mut speed = d.manual_driving_speed;
            if self.scripted_waypoint_mode || self.last_driving_click_frame.wrapping_sub(self.second_last_driving_click_frame) < d.double_click_to_fast_drive_delay {
                // "Because we double clicked, use the faster driving speed."
                speed = d.manual_fast_driving_speed;
            }
            // "Convert speed to speed per frame."
            speed /= LOGICFRAMES_PER_SECOND as f32;
            let mut vector = self.override_target_destination;
            vector.sub(&self.current_target_position);
            let distance = vector.length();
            if distance < speed {
                // "Don't allow the speed to overshoot the target point if close."
                speed = distance;
                // "If we're in scripted waypoint mode, go to the next waypoint!"
                if self.scripted_waypoint_mode {
                    if let Some(h) = world.terrain.waypoint_by_id(self.next_dest_waypoint_id) {
                        let link_count = world.terrain.waypoint(h).map_or(0, |w| w.num_links());
                        let which = world.random.game_logic_random_value(0, link_count - 1);
                        let next = world.terrain.waypoint_link(h, which).map_err(|_| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
                        let node = next.and_then(|n| world.terrain.waypoint(n).cloned()).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
                        self.next_dest_waypoint_id = node.id() as u32;
                        self.override_target_destination = *node.location();
                    }
                }
            }
            // "Unitize the vector then apply the distance we will move."
            vector.normalize();
            vector.scale(speed);
            self.current_target_position.x += vector.x;
            self.current_target_position.y += vector.y;
        }
        Ok(())
    }

    /// A damage pulse: a share of the firing's damage to everything alive within the radius, and the remnant object.
    fn damage_pulse(&mut self, world: &mut World, us: ObjectId, damage_radius: f32) -> Result<(), IniError> {
        let d = self.data.clone();
        let total_firing_seconds = (d.total_firing_frames / LOGICFRAMES_PER_SECOND as u32) as f32;
        let damage_per_pulse = (total_firing_seconds * d.damage_per_second) / d.total_damage_pulses as f32;
        let mut info = DamageInfo::default();
        info.input.amount = damage_per_pulse;
        info.input.source_id = us;
        info.input.damage_type = d.damage_type;
        info.input.death_type = if crate::logic::modules::plant(2466) { DEATH_LASERED } else { d.death_type };
        let found = world.partition_iterate_objects_in_range_at(self.current_target_position, damage_radius, FROM_CENTER_2D);
        for obj in found {
            if world.objects.find(obj).is_none_or(|o| o.is_effectively_dead()) {
                continue;
            }
            if let Some(body) = world.get_body_module(obj) {
                world.module_call(obj, body, ModuleCall::AttemptDamage(info.clone()));
            }
        }
        if !d.damage_pulse_remnant_object_name.is_empty() {
            // "Create a remnant damaging object that will fade over time to represent the burning trail."
            let thing = world.statics.thing_factory.find_template(&d.damage_pulse_remnant_object_name).map(|t| t.data.get_template_id());
            if let Some(thing) = thing {
                let team = world.get_team(us);
                // (`newObject` throws on failure; the world's errors are not upstream's)
                let remnant = world.thing_factory_new_object(thing, team, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
                world.set_position(remnant, &self.current_target_position);
            }
        }
        Ok(())
    }

    /// The particle system template `name` names (`TheParticleSystemManager->findTemplate`): `None` for an empty name or one it has not.
    fn find_system(world: &mut World, name: &[u8]) -> Option<Vec<u8>> {
        (!name.is_empty() && world.body_particle_find_template(name)).then(|| name.to_vec())
    }

    /// `createOuterNodeParticleSystems(intensity)`.
    fn create_outer_node_particle_systems(&mut self, world: &mut World, intensity: Intensity) -> Result<(), IniError> {
        let d = self.data.clone();
        let name = match intensity {
            Intensity::Light => &d.outer_nodes_flares[0],
            Intensity::Medium => &d.outer_nodes_flares[1],
            Intensity::Intense => &d.outer_nodes_flares[2],
        };
        if let Some(t) = Self::find_system(world, name) {
            let n = self.num_bones()?;
            for i in 0..n {
                let sys = world.create_particle_system_unplaced(t.clone());
                self.outer_system_ids[i] = sys;
                world.particle_system_set_position(sys, self.outer_node_positions[i]);
                world.particle_system_set_local_transform(sys, &self.outer_node_orientations[i]);
            }
        }
        Ok(())
    }

    /// `createConnectorLasers(intensity)`: the bones first (once).
    fn create_connector_lasers(&mut self, world: &mut World, us: ObjectId, intensity: Intensity) -> Result<(), IniError> {
        // "Cache bone positions for the laser when it is ready to fire"
        if !self.up_bones_cached {
            self.calculate_up_bone_positions(world, us);
            self.up_bones_cached = true;
        }
        let d = self.data.clone();
        let name = match intensity {
            Intensity::Light => return Ok(()),
            Intensity::Medium => &d.connector_lasers[0],
            Intensity::Intense => &d.connector_lasers[1],
        };
        if name.is_empty() {
            return Ok(());
        }
        let Some(template) = world.statics.thing_factory.find_template(name).map(|t| t.data.get_template_id()) else { return Ok(()) };
        let n = self.num_bones()?;
        for i in 0..n {
            let beam = world.client_new_drawable(template);
            self.laser_beam_ids[i] = beam;
            if world.client_drawable_has_laser(beam) {
                world.client_laser_init(beam, self.outer_node_positions[i], self.connector_node_position, 0);
            }
        }
        Ok(())
    }

    /// `createConnectorFlare(intensity)`.
    fn create_connector_flare(&mut self, world: &mut World, intensity: Intensity) {
        let d = self.data.clone();
        let name = match intensity {
            Intensity::Light => return,
            Intensity::Medium => &d.connector_flares[0],
            Intensity::Intense => &d.connector_flares[1],
        };
        if let Some(t) = Self::find_system(world, name) {
            let sys = world.create_particle_system_unplaced(t);
            self.connector_system_id = sys;
            world.particle_system_set_position(sys, self.connector_node_position);
        }
    }

    /// `createLaserBaseFlare(intensity)`.
    fn create_laser_base_flare(&mut self, world: &mut World, intensity: Intensity) {
        let d = self.data.clone();
        let name = match intensity {
            Intensity::Light => &d.laser_base_flares[0],
            Intensity::Medium => &d.laser_base_flares[1],
            Intensity::Intense => &d.laser_base_flares[2],
        };
        if let Some(t) = Self::find_system(world, name) {
            let sys = world.create_particle_system_unplaced(t);
            self.laser_base_system_id = sys;
            world.particle_system_set_position(sys, self.laser_origin_position);
        }
    }

    /// `createGroundToOrbitLaser(growthFrames)`.
    fn create_ground_to_orbit_laser(&mut self, world: &mut World, growth_frames: u32) {
        let d = self.data.clone();
        if world.contain_effect_exists(self.ground_to_orbit_beam_id) {
            world.client_destroy_drawable(self.ground_to_orbit_beam_id);
            // (upstream clears the other beam's id)
            if crate::logic::modules::plant(2467) {
                self.ground_to_orbit_beam_id = 0;
            } else {
                self.orbit_to_target_beam_id = 0;
            }
        }
        if d.particle_beam_laser_name.is_empty() {
            return;
        }
        let Some(template) = world.statics.thing_factory.find_template(&d.particle_beam_laser_name).map(|t| t.data.get_template_id()) else { return };
        let beam = world.client_new_drawable(template);
        self.ground_to_orbit_beam_id = beam;
        if world.client_drawable_has_laser(beam) {
            let mut orbit_position = self.laser_origin_position;
            orbit_position.z += 500.0;
            world.client_laser_init(beam, self.laser_origin_position, orbit_position, growth_frames as i32);
        }
    }

    /// `createOrbitToTargetLaser(growthFrames)`: the beam onto the target, and the annihilation sound at it.
    fn create_orbit_to_target_laser(&mut self, world: &mut World, growth_frames: u32) {
        let d = self.data.clone();
        if world.contain_effect_exists(self.orbit_to_target_beam_id) {
            Self::remove_sound(world, &self.annihilation_sound);
            world.client_destroy_drawable(self.orbit_to_target_beam_id);
            self.orbit_to_target_beam_id = 0;
        }
        if d.particle_beam_laser_name.is_empty() {
            return;
        }
        if let Some(template) = world.statics.thing_factory.find_template(&d.particle_beam_laser_name).map(|t| t.data.get_template_id()) {
            let beam = world.client_new_drawable(template);
            self.orbit_to_target_beam_id = beam;
            if world.client_drawable_has_laser(beam) {
                let mut orbit_position = self.initial_target_position;
                orbit_position.z += 500.0;
                world.client_laser_init(beam, orbit_position, self.initial_target_position, growth_frames as i32);
            }
        }
        if !self.annihilation_sound.name.is_empty() {
            self.annihilation_sound.handle = world.audio_add_event_at_drawable(self.annihilation_sound.name.clone(), self.orbit_to_target_beam_id);
        }
    }

    /// `removeAllEffects()`: the outer systems and lasers, the connector's and base's systems, and the ground-to-orbit beam.
    fn remove_all_effects(&mut self, world: &mut World) -> Result<(), IniError> {
        let n = self.num_bones()?;
        for i in 0..n {
            if self.outer_system_ids[i] != 0 {
                world.destroy_particle_system_by_id(self.outer_system_ids[i]);
                self.outer_system_ids[i] = 0;
            }
            if world.contain_effect_exists(self.laser_beam_ids[i]) {
                world.client_destroy_drawable(self.laser_beam_ids[i]);
                self.laser_beam_ids[i] = 0;
            }
        }
        if self.connector_system_id != 0 {
            world.destroy_particle_system_by_id(self.connector_system_id);
            self.connector_system_id = 0;
        }
        if self.laser_base_system_id != 0 && !crate::logic::modules::plant(2468) {
            world.destroy_particle_system_by_id(self.laser_base_system_id);
            self.laser_base_system_id = 0;
        }
        if world.contain_effect_exists(self.ground_to_orbit_beam_id) {
            world.client_destroy_drawable(self.ground_to_orbit_beam_id);
            self.ground_to_orbit_beam_id = 0;
        }
        Ok(())
    }

    /// `calculateDefaultInformation()`: the outer nodes' world positions and orientations, from the bones; false (invalid settings) where
    /// the art has not exactly `OuterEffectNumBones` of them.
    fn calculate_default_information(&mut self, world: &mut World, us: ObjectId) -> Result<bool, IniError> {
        let d = self.data.clone();
        let bones = world.object_get_multi_logical_bone_transforms(us, &d.outer_effect_base_bone_name, d.outer_effect_num_bones as i32);
        if bones.len() as u32 != d.outer_effect_num_bones {
            self.invalid_settings = true;
            return Ok(false);
        }
        let n = self.num_bones()?;
        for (i, (p, m)) in bones.iter().enumerate().take(n) {
            self.laser_beam_ids[i] = 0;
            self.outer_system_ids[i] = 0;
            // "Convert the local bone position into world space."
            let (wp, wm) = world.object_convert_bone_pos_to_world_pos(us, p, m);
            self.outer_node_positions[i] = wp;
            self.outer_node_orientations[i] = wm;
        }
        Ok(true)
    }

    /// `calculateUpBonePositions()`: the connector and the laser's origin from the drawable's current bones (the connector's name is tested
    /// for both).
    fn calculate_up_bone_positions(&mut self, world: &mut World, us: ObjectId) {
        let d = self.data.clone();
        if !world.has_drawable(us) {
            return;
        }
        if !d.connector_bone_name.is_empty() {
            if let Some((p, m)) = world.drawable_get_current_client_bone(us, &d.connector_bone_name) {
                self.connector_node_position = world.object_convert_bone_pos_to_world_pos(us, &p, &m).0;
            }
        }
        let fire_tested = if crate::logic::modules::plant(2469) { &d.fire_bone_name } else { &d.connector_bone_name };
        if !fire_tested.is_empty() {
            if let Some((p, m)) = world.drawable_get_current_client_bone(us, &d.fire_bone_name) {
                self.laser_origin_position = world.object_convert_bone_pos_to_world_pos(us, &p, &m).0;
            }
        }
    }

    /// `setLogicalStatus(newStatus)`: the model condition and the sounds of the new status, then the client's effects.
    fn set_logical_status(&mut self, world: &mut World, us: ObjectId, new_status: i32) -> Result<(), IniError> {
        if self.status == new_status {
            return Ok(());
        }
        let deploy_clear = [MODELCONDITION_PACKING, MODELCONDITION_UNPACKING];
        match new_status {
            STATUS_IDLE => {
                // "Set unpacked animation"
                world.contain_clear_model_condition_flags(us, &[MODELCONDITION_PACKING, MODELCONDITION_UNPACKING, MODELCONDITION_DEPLOYED]);
                for s in [&self.powerup_sound, &self.unpack_to_ready_sound, &self.firing_to_idle_sound, &self.annihilation_sound] {
                    Self::remove_sound(world, s);
                }
            }
            STATUS_CHARGING => {
                self.laser_status = LASERSTATUS_NONE;
                self.powerup_sound.handle = world.audio_add_event(self.powerup_sound.name.clone(), us);
                for s in [&self.unpack_to_ready_sound, &self.firing_to_idle_sound, &self.annihilation_sound] {
                    Self::remove_sound(world, s);
                }
            }
            STATUS_PREPARING => {
                // "Set unpacking animation"
                world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_PACKING, MODELCONDITION_DEPLOYED], &[MODELCONDITION_UNPACKING]);
                if !self.unpack_to_ready_sound.name.is_empty() {
                    self.unpack_to_ready_sound.handle = world.audio_add_event(self.unpack_to_ready_sound.name.clone(), us);
                }
                for s in [&self.firing_to_idle_sound, &self.annihilation_sound] {
                    Self::remove_sound(world, s);
                }
                self.laser_status = LASERSTATUS_NONE;
            }
            STATUS_ALMOST_READY => {
                // "Set deployed animation"
                world.clear_and_set_model_condition_flags(us, &deploy_clear, &[MODELCONDITION_DEPLOYED]);
                self.laser_status = LASERSTATUS_NONE;
            }
            STATUS_READY_TO_FIRE => {
                world.clear_and_set_model_condition_flags(us, &deploy_clear, &[MODELCONDITION_DEPLOYED]);
                for s in [&self.powerup_sound, &self.firing_to_idle_sound, &self.annihilation_sound] {
                    Self::remove_sound(world, s);
                }
                self.laser_status = LASERSTATUS_NONE;
            }
            STATUS_FIRING => {
                world.clear_and_set_model_condition_flags(us, &deploy_clear, &[MODELCONDITION_DEPLOYED]);
                if !self.firing_to_idle_sound.name.is_empty() {
                    self.firing_to_idle_sound.handle = world.audio_add_event(self.firing_to_idle_sound.name.clone(), us);
                }
                for s in [&self.powerup_sound, &self.unpack_to_ready_sound, &self.annihilation_sound] {
                    Self::remove_sound(world, s);
                }
                self.next_launch_fx_frame = 0;
            }
            STATUS_PACKING => {
                // "Set packing animation"
                world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_UNPACKING, MODELCONDITION_DEPLOYED], &[MODELCONDITION_PACKING]);
            }
            // STATUS_PREFIRE, STATUS_POSTFIRE
            _ => {}
        }
        self.status = new_status;
        self.set_client_status(world, us, self.status, false)
    }

    /// `setClientStatus(newStatus, revealThisFrame)`: the outer nodes cached (once), every effect removed, and the status's effects made.
    fn set_client_status(&mut self, world: &mut World, us: ObjectId, new_status: i32, reveal_this_frame: bool) -> Result<(), IniError> {
        let d = self.data.clone();
        // "Caches bone positions for positioning the client beams."
        if !self.default_info_cached {
            if !self.calculate_default_information(world, us)? {
                self.invalid_settings = true;
                return Ok(());
            }
            self.default_info_cached = true;
        }
        // "the new philosophy is to delete EVERYTHING between status changes and create the ones we want!"
        self.remove_all_effects(world)?;
        match new_status {
            STATUS_CHARGING => self.create_outer_node_particle_systems(world, Intensity::Light)?,
            STATUS_PREPARING => self.create_outer_node_particle_systems(world, Intensity::Medium)?,
            STATUS_ALMOST_READY => {
                self.create_outer_node_particle_systems(world, Intensity::Medium)?;
                self.create_connector_lasers(world, us, Intensity::Medium)?;
                self.create_connector_flare(world, Intensity::Medium);
            }
            STATUS_READY_TO_FIRE => {
                self.create_outer_node_particle_systems(world, Intensity::Medium)?;
                self.create_connector_lasers(world, us, Intensity::Medium)?;
                self.create_connector_flare(world, Intensity::Medium);
                self.create_laser_base_flare(world, Intensity::Light);
            }
            STATUS_FIRING => {
                self.create_ground_to_orbit_laser(world, if reveal_this_frame { 0 } else { d.width_grow_frames });
                self.create_outer_node_particle_systems(world, Intensity::Intense)?;
                self.create_connector_lasers(world, us, Intensity::Intense)?;
                self.create_connector_flare(world, Intensity::Intense);
                self.create_laser_base_flare(world, Intensity::Intense);
            }
            STATUS_POSTFIRE => {
                self.create_outer_node_particle_systems(world, Intensity::Medium)?;
                self.create_connector_lasers(world, us, Intensity::Medium)?;
                self.create_connector_flare(world, Intensity::Medium);
                self.create_laser_base_flare(world, Intensity::Medium);
                self.create_ground_to_orbit_laser(world, 0);
                let beam = self.ground_to_orbit_beam_id;
                if world.contain_effect_exists(beam) && world.client_drawable_has_laser(beam) {
                    world.client_laser_set_decay_frames(beam, if reveal_this_frame { 0 } else { d.width_grow_frames });
                }
            }
            // STATUS_IDLE, STATUS_PREFIRE, STATUS_PACKING
            _ => {}
        }
        Ok(())
    }

    /// `doesSpecialPowerHaveOverridableDestinationActive()`.
    fn overridable_destination_active(&self) -> bool {
        self.status == STATUS_PREFIRE || self.status == STATUS_FIRING || self.status == STATUS_POSTFIRE
    }

    fn record<T>(world: &mut World, me: ModuleHandle, r: Result<T, IniError>) -> Option<T> {
        match r {
            Ok(t) => Some(t),
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                None
            }
        }
    }
}

impl UpdateBehavior for ParticleUplinkCannonUpdate {
    const XFER_VERSION: XferVersion = 3;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.do_update(world, me.object);
        Self::record(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// The statuses, the frames, the ids, the outer nodes, the positions, the flags, the counters and frames; from version 2 the decay frame
    /// (before it, the start of the attack and the firing time); from version 3 the modes and the next waypoint.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let user_i32 = |xfer: &mut dyn Xfer, v: &mut i32| -> XferResult<()> {
            let mut b = v.to_le_bytes();
            xfer.xfer_user(&mut b)?;
            *v = i32::from_le_bytes(b);
            Ok(())
        };
        user_i32(xfer, &mut self.status)?;
        user_i32(xfer, &mut self.laser_status)?;
        xfer.xfer_unsigned_int(&mut self.frames)?;
        // "we do not need to tie up the special power module pointer, it is done on object creation"
        let words = |xfer: &mut dyn Xfer, a: &mut [u32; MAX_OUTER_NODES]| -> XferResult<()> {
            let mut b = [0u8; 4 * MAX_OUTER_NODES];
            for (i, w) in a.iter().enumerate() {
                b[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
            }
            xfer.xfer_user(&mut b)?;
            for (i, w) in a.iter_mut().enumerate() {
                *w = u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().expect("four bytes"));
            }
            Ok(())
        };
        words(xfer, &mut self.outer_system_ids)?;
        let mut ids = self.laser_beam_ids.map(|d| d as u32);
        words(xfer, &mut ids)?;
        self.laser_beam_ids = ids.map(|d| d as DrawableId);
        xfer.xfer_drawable_id(&mut self.ground_to_orbit_beam_id)?;
        xfer.xfer_drawable_id(&mut self.orbit_to_target_beam_id)?;
        let user_u32 = |xfer: &mut dyn Xfer, v: &mut u32| -> XferResult<()> {
            let mut b = v.to_le_bytes();
            xfer.xfer_user(&mut b)?;
            *v = u32::from_le_bytes(b);
            Ok(())
        };
        user_u32(xfer, &mut self.connector_system_id)?;
        user_u32(xfer, &mut self.laser_base_system_id)?;
        // the outer node positions (twelve bytes each) and orientations (forty-eight)
        let mut b = [0u8; 12 * MAX_OUTER_NODES];
        for (i, p) in self.outer_node_positions.iter().enumerate() {
            for (k, f) in [p.x, p.y, p.z].iter().enumerate() {
                b[12 * i + 4 * k..12 * i + 4 * k + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        xfer.xfer_user(&mut b)?;
        for (i, p) in self.outer_node_positions.iter_mut().enumerate() {
            let f = |k: usize| f32::from_le_bytes(b[12 * i + 4 * k..12 * i + 4 * k + 4].try_into().expect("four bytes"));
            *p = Coord3D { x: f(0), y: f(1), z: f(2) };
        }
        let mut b = [0u8; 48 * MAX_OUTER_NODES];
        for (i, m) in self.outer_node_orientations.iter().enumerate() {
            for (k, f) in m.to_array().iter().enumerate() {
                b[48 * i + 4 * k..48 * i + 4 * k + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        xfer.xfer_user(&mut b)?;
        for (i, m) in self.outer_node_orientations.iter_mut().enumerate() {
            let mut a = [0f32; 12];
            for (k, f) in a.iter_mut().enumerate() {
                *f = f32::from_le_bytes(b[48 * i + 4 * k..48 * i + 4 * k + 4].try_into().expect("four bytes"));
            }
            *m = Matrix3D::from_array(a);
        }
        xfer.xfer_coord3d(&mut self.connector_node_position)?;
        xfer.xfer_coord3d(&mut self.laser_origin_position)?;
        xfer.xfer_coord3d(&mut self.override_target_destination)?;
        xfer.xfer_bool(&mut self.up_bones_cached)?;
        xfer.xfer_bool(&mut self.default_info_cached)?;
        xfer.xfer_bool(&mut self.invalid_settings)?;
        xfer.xfer_coord3d(&mut self.initial_target_position)?;
        xfer.xfer_coord3d(&mut self.current_target_position)?;
        xfer.xfer_unsigned_int(&mut self.scorch_marks_made)?;
        xfer.xfer_unsigned_int(&mut self.next_scorch_mark_frame)?;
        xfer.xfer_unsigned_int(&mut self.next_launch_fx_frame)?;
        xfer.xfer_unsigned_int(&mut self.damage_pulses_made)?;
        xfer.xfer_unsigned_int(&mut self.next_damage_pulse_frame)?;
        xfer.xfer_unsigned_int(&mut self.start_attack_frame)?;
        if xfer.get_xfer_mode() == crate::common::xfer::XferMode::Save || version >= 2 {
            xfer.xfer_unsigned_int(&mut self.start_decay_frame)?;
        } else {
            self.start_decay_frame = self.start_attack_frame.wrapping_add(self.data.total_firing_frames);
        }
        xfer.xfer_unsigned_int(&mut self.last_driving_click_frame)?;
        xfer.xfer_unsigned_int(&mut self.second_last_driving_click_frame)?;
        if version >= 3 && !crate::logic::modules::plant(2470) {
            xfer.xfer_bool(&mut self.manual_target_mode)?;
            xfer.xfer_bool(&mut self.scripted_waypoint_mode)?;
            xfer.xfer_unsigned_int(&mut self.next_dest_waypoint_id)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ParticleUplinkCannonUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ParticleUplinkCannonUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// `onObjectCreated`, `initiateIntentToDoSpecialPower`, the overridable destination and the special power update's queries.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::InitiateIntentToDoSpecialPower { template, target_object, target_position, waypoint, command_options } => {
                let r = self.initiate_intent(world, me.object, template, target_object, target_position, waypoint, command_options);
                ModuleReply::Bool(Self::record(world, me, r).unwrap_or(false))
            }
            ModuleCall::SetSpecialPowerOverridableDestination(loc) => {
                self.set_overridable_destination(world, me.object, loc);
                ModuleReply::Done
            }
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive => ModuleReply::Bool(self.overridable_destination_active()),
            ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(true),
            ModuleCall::SpecialPowerUpdateIsActive => ModuleReply::Bool(self.status != STATUS_IDLE),
            ModuleCall::SpecialPowerUpdateGetCommandOption => ModuleReply::Int(0),
            ModuleCall::IsPowerCurrentlyInUse => ModuleReply::Bool(self.is_power_currently_in_use(world)),
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => ModuleReply::Bool(true),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for ParticleUplinkCannonUpdate {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.update_mut().crc(xfer)
    }
    /// `xfer` (version 3): the version, `UpdateModule::xfer`, the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.base.update().clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        *self.base.update_mut() = base;
        r
    }
    fn load_post_process(&mut self) {
        self.base.update_mut().load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = D::default();
        assert_eq!((d.frames_between_launch_fx_refresh, d.double_click_to_fast_drive_delay), (30, 500));
        assert_eq!((d.scorch_mark_scalar, d.damage_radius_scalar, d.damage_type, d.death_type), (1.0, 1.0, DAMAGE_LASER, DEATH_LASERED));
        let m = ParticleUplinkCannonUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!((m.status, m.laser_status), (STATUS_IDLE, LASERSTATUS_NONE));
        assert!(m.outer_node_orientations.iter().all(|o| *o == Matrix3D::IDENTITY));
        assert!(m.update_module().get_disabled_types_to_process().test(DISABLED_HACKED));
    }
}
