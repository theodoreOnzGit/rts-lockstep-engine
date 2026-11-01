// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/{AssaultTransport,DeliverPayload,DeployStyle,HackInternet,SupplyTruck,
//     Worker}AIUpdate.h (their module data, constructors and field tables inline)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/{Chinook,Dozer,Jet,Missile,RailedTransport}AIUpdate.h (their module data)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/{Chinook,Dozer,Jet,Missile,RailedTransport}AIUpdate.cpp
//     (their module data's constructors and buildFieldParse)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI subclasses' module data (`JetAIUpdateModuleData` and the rest): their INI fields after `AIUpdateModuleData`'s, as
//! [`AiUpdateModuleData::subclass`]. `p1w13-sonnet`, integration batch I9b-5.
//!
//! Each subclass's data is its upstream class's members as INI leaves them (names as strings, the FX lists by name), and its
//! module factory data procedure (`new_jet_module_data` and the rest) makes the defaults and reads `AIUpdateModuleData`'s
//! fields then its own, as its `buildFieldParse` adds them. `WanderAIUpdate` and `TransportAIUpdate` have `AIUpdateModuleData`
//! itself (`MAKE_STANDARD_MODULE_MACRO`). What an AI keeps of its data when it is made (a template found or not, an FX list's
//! handle) is the factory's: [`AiSubclassModuleData`] is the INI's view.
//!
//! # Upstream facts kept
//!
//! - **`LockonBlinky` is a `Real` read by `parseBool`.** `JetAIUpdateModuleData::m_lockonBlinky` is declared `Real`, the
//!   constructor never sets it, and `INI::parseBool` writes one `Bool` byte into its first byte. A `Real` whose lowest byte is
//!   1 is a non-zero denormal, so a `LockonBlinky = Yes` reads as true; `No` writes 0 into that byte only. The data here keeps
//!   the `Bool` the INI wrote (`false` before any), the value the member's readers see where the rest of the `Real` is zero.
//! - `ChinookAIUpdateModuleData` is a `SupplyTruckAIUpdateModuleData`: its fields come after the supply truck's.
//! - `ChinookAIUpdateModuleData`'s `m_rappelSpeed` is `|TheGlobalData->m_gravity| * LOGICFRAMES_PER_SECOND / 2` when the data is
//!   made. This port takes `GlobalData`'s constructor gravity (-1), as `GenerateMinefieldBehavior`'s data does: a map's
//!   `GameData.ini` gravity is not seen (Open, until the data constructor can read the store).
//! - `MissileAIUpdateModuleData`'s `GarrisonHitKillCount` is read by `parseUnsignedInt` and `KillSelfDelay` by
//!   `parseDurationUnsignedInt` into `Int`s: the bits are kept.

use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::{Coord3D, RGBColor};
use crate::common::damage_fx::FxListName;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::logic::ai::ai_update_data::AiUpdateModuleData;
use crate::logic::ai::ai_update_types::{LocomotorSetType, LOCOMOTORSET_NORMAL, LOCOMOTOR_SET_NAMES};
use crate::logic::modules::plant;
use crate::logic::radius_decal::RadiusDecalTemplate;

/// `JetAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, PartialEq)]
pub struct JetAiModuleData {
    pub out_of_ammo_damage_per_second: f32,
    pub takeoff_dist_for_max_lift: f32,
    pub min_height: f32,
    pub parking_offset: f32,
    pub sneaky_offset_when_attacking: f32,
    pub keeps_parking_space_when_airborne: bool,
    pub needs_runway: bool,
    pub takeoff_pause: u32,
    pub attacking_loco: LocomotorSetType,
    pub returning_loco: LocomotorSetType,
    pub attack_loco_persist_time: u32,
    pub attackers_miss_persist_time: u32,
    pub lockon_time: u32,
    pub lockon_cursor: Vec<u8>,
    pub lockon_initial_dist: f32,
    pub lockon_freq: f32,
    pub lockon_angle_spin: f32,
    /// The `Bool` byte `parseBool` writes into the `Real` `m_lockonBlinky` (see the module's notes).
    pub lockon_blinky: bool,
    pub return_to_base_idle_time: u32,
}

impl Default for JetAiModuleData {
    /// `JetAIUpdateModuleData()`.
    fn default() -> Self {
        JetAiModuleData {
            out_of_ammo_damage_per_second: 0.0,
            takeoff_dist_for_max_lift: 0.0,
            min_height: 0.0,
            parking_offset: 0.0,
            sneaky_offset_when_attacking: 0.0,
            keeps_parking_space_when_airborne: true,
            needs_runway: true,
            takeoff_pause: 0,
            attacking_loco: LOCOMOTORSET_NORMAL,
            returning_loco: LOCOMOTORSET_NORMAL,
            attack_loco_persist_time: 0,
            attackers_miss_persist_time: 0,
            lockon_time: 0,
            lockon_cursor: Vec::new(),
            lockon_initial_dist: 100.0,
            lockon_freq: 0.5,
            lockon_angle_spin: 720.0,
            lockon_blinky: false,
            return_to_base_idle_time: 0,
        }
    }
}

/// `MissileAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, PartialEq)]
pub struct MissileAiModuleData {
    pub try_to_follow_target: bool,
    pub fuel_lifetime: u32,
    pub ignition_delay: u32,
    pub initial_vel: f32,
    pub initial_dist: f32,
    pub dive_distance: f32,
    pub ignition_fx: FxListName,
    pub use_weapon_speed: bool,
    pub detonate_on_no_fuel: bool,
    pub garrison_hit_kill_count: i32,
    pub garrison_hit_kill_kindof: KindOfMaskType,
    pub garrison_hit_kill_kindof_not: KindOfMaskType,
    pub garrison_hit_kill_fx: FxListName,
    pub distance_scatter_when_jammed: f32,
    pub lock_distance: f32,
    pub detonate_calls_kill: bool,
    pub kill_self_delay: i32,
}

impl Default for MissileAiModuleData {
    /// `MissileAIUpdateModuleData()`.
    fn default() -> Self {
        MissileAiModuleData {
            try_to_follow_target: true,
            fuel_lifetime: 0,
            ignition_delay: 0,
            initial_vel: 0.0,
            initial_dist: 0.0,
            dive_distance: 0.0,
            ignition_fx: None,
            use_weapon_speed: false,
            detonate_on_no_fuel: false,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_kindof: KindOfMaskType::default(),
            garrison_hit_kill_kindof_not: KindOfMaskType::default(),
            garrison_hit_kill_fx: None,
            distance_scatter_when_jammed: 75.0,
            lock_distance: 75.0,
            detonate_calls_kill: false,
            kill_self_delay: if plant(4047) { 0 } else { 3 },
        }
    }
}

/// `SupplyTruckAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, PartialEq)]
pub struct SupplyTruckAiModuleData {
    pub max_boxes_data: i32,
    pub center_delay: u32,
    pub warehouse_delay: u32,
    pub warehouse_scan_distance: f32,
    pub supplies_depleted_voice: AudioEventName,
}

impl Default for SupplyTruckAiModuleData {
    /// `SupplyTruckAIUpdateModuleData()`.
    fn default() -> Self {
        SupplyTruckAiModuleData {
            max_boxes_data: 0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AudioEventName::default(),
        }
    }
}

/// `ChinookAIUpdateModuleData`: a supply truck's data and its own members.
#[derive(Clone, Debug, PartialEq)]
pub struct ChinookAiModuleData {
    pub supply: SupplyTruckAiModuleData,
    pub rope_name: Vec<u8>,
    pub rotor_wash_particle_system: Vec<u8>,
    pub rappel_speed: f32,
    pub rope_drop_speed: f32,
    pub rope_width: f32,
    pub rope_final_height: f32,
    pub rope_wobble_len: f32,
    pub rope_wobble_amp: f32,
    pub rope_wobble_rate: f32,
    pub rope_color: RGBColor,
    pub num_ropes: u32,
    pub per_rope_delay_min: u32,
    pub per_rope_delay_max: u32,
    pub min_drop_height: f32,
    pub wait_for_ropes_to_drop: bool,
    pub upgraded_supply_boost: i32,
}

/// `GlobalData()`'s gravity, which the chinook's rappel speed is made from (see the module's notes).
const CONSTRUCTOR_GRAVITY: f32 = -1.0;

impl Default for ChinookAiModuleData {
    /// `ChinookAIUpdateModuleData()`.
    fn default() -> Self {
        ChinookAiModuleData {
            supply: SupplyTruckAiModuleData::default(),
            rope_name: b"GenericRope".to_vec(),
            rotor_wash_particle_system: Vec::new(),
            rappel_speed: CONSTRUCTOR_GRAVITY.abs() * LOGICFRAMES_PER_SECOND as f32 * 0.5,
            rope_drop_speed: 1e10,
            rope_width: 0.5,
            rope_final_height: 0.0,
            rope_wobble_len: 10.0,
            rope_wobble_amp: 1.0,
            rope_wobble_rate: 0.1,
            rope_color: RGBColor { red: 0.9, green: 0.8, blue: 0.7 },
            num_ropes: 4,
            per_rope_delay_min: 0x7fff_ffff,
            per_rope_delay_max: 0x7fff_ffff,
            min_drop_height: 30.0,
            wait_for_ropes_to_drop: true,
            upgraded_supply_boost: 0,
        }
    }
}

/// `DozerAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DozerAiModuleData {
    pub repair_health_percent_per_second: f32,
    pub bored_time: f32,
    pub bored_range: f32,
}

/// `WorkerAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerAiModuleData {
    pub max_boxes_data: i32,
    pub repair_health_percent_per_second: f32,
    pub bored_time: f32,
    pub bored_range: f32,
    pub center_delay: u32,
    pub warehouse_delay: u32,
    pub warehouse_scan_distance: f32,
    pub supplies_depleted_voice: AudioEventName,
    pub upgraded_supply_boost: i32,
}

impl Default for WorkerAiModuleData {
    /// `WorkerAIUpdateModuleData()`.
    fn default() -> Self {
        WorkerAiModuleData {
            max_boxes_data: 0,
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AudioEventName::default(),
            upgraded_supply_boost: 0,
        }
    }
}

/// `DeliverPayloadAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeliverPayloadAiModuleData {
    pub door_delay: u32,
    pub max_distance_to_target: f32,
    pub max_number_attempts: i32,
    pub drop_delay: u32,
    pub drop_offset: Coord3D,
    pub drop_variance: Coord3D,
    pub put_in_container_name: Vec<u8>,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: f32,
}

/// `DeployStyleAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeployStyleAiModuleData {
    pub unpack_time: u32,
    pub pack_time: u32,
    pub reset_turret_before_packing: bool,
    pub turrets_function_only_when_deployed: bool,
    pub turrets_must_center_before_packing: bool,
    pub manual_deploy_animations: bool,
}

/// `HackInternetAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HackInternetAiModuleData {
    pub unpack_time: u32,
    pub pack_time: u32,
    pub cash_update_delay: u32,
    pub cash_update_delay_fast: u32,
    pub regular_cash_amount: u32,
    pub veteran_cash_amount: u32,
    pub elite_cash_amount: u32,
    pub heroic_cash_amount: u32,
    pub xp_per_cash_update: u32,
    pub pack_unpack_variation_factor: f32,
}

/// `AssaultTransportAIUpdateModuleData`'s own members.
#[derive(Clone, Debug, PartialEq)]
pub struct AssaultTransportAiModuleData {
    pub members_get_healed_at_life_ratio: f32,
    pub clear_range_required_to_continue_attack_move: f32,
}

impl Default for AssaultTransportAiModuleData {
    /// `AssaultTransportAIUpdateModuleData()`.
    fn default() -> Self {
        AssaultTransportAiModuleData { members_get_healed_at_life_ratio: 0.0, clear_range_required_to_continue_attack_move: 50.0 }
    }
}

/// The subclass part of an AI's module data: which class's data it is, with that class's own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AiSubclassModuleData {
    /// `AIUpdateModuleData` itself (`AIUpdateInterface`, `WanderAIUpdate`, `TransportAIUpdate`).
    #[default]
    Plain,
    Jet(JetAiModuleData),
    Missile(MissileAiModuleData),
    SupplyTruck(SupplyTruckAiModuleData),
    Chinook(ChinookAiModuleData),
    Dozer(DozerAiModuleData),
    Worker(WorkerAiModuleData),
    DeliverPayload(DeliverPayloadAiModuleData),
    DeployStyle(DeployStyleAiModuleData),
    HackInternet(HackInternetAiModuleData),
    AssaultTransport(AssaultTransportAiModuleData),
    /// `RailedTransportAIUpdateModuleData`: `m_pathPrefixName`.
    RailedTransport { path_prefix_name: Vec<u8> },
}

/// A field of one subclass's table: the parse runs on that subclass's members; on any other data it is the table of the wrong
/// class (a defect of this port, `INI_BUG`).
macro_rules! sub_field {
    ($name:literal, $variant:ident, |$ini:ident, $s:ident| $body:expr) => {
        FieldParse::new(
            $name,
            |$ini, d, _| match &mut d.subclass {
                AiSubclassModuleData::$variant($s) => {
                    $body;
                    Ok(())
                }
                _ => Err(IniError::new(IniErrorKind::Bug)),
            },
            UserData::None,
        )
    };
}

/// The supply truck's members of a supply truck's or a chinook's data.
fn supply(d: &mut AiUpdateModuleData) -> IniResult<&mut SupplyTruckAiModuleData> {
    match &mut d.subclass {
        AiSubclassModuleData::SupplyTruck(s) => Ok(s),
        AiSubclassModuleData::Chinook(c) => Ok(&mut c.supply),
        _ => Err(IniError::new(IniErrorKind::Bug)),
    }
}

/// `INI::parseFXList`, as far as the token goes: the next token names the list, `None` is `NULL` (as
/// `CrateCollideModuleData::parse_fx_list`).
fn parse_fx_list(ini: &mut Ini<'_>) -> IniResult<FxListName> {
    let token = ini.get_next_token(None)?;
    Ok(if token.eq_ignore_ascii_case(b"None") { None } else { Some(token) })
}

/// `JetAIUpdateModuleData::buildFieldParse`'s own table.
static JET_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 20] = [
    sub_field!("OutOfAmmoDamagePerSecond", Jet, |i, j| j.out_of_ammo_damage_per_second = i.parse_percent_to_real()?),
    sub_field!("NeedsRunway", Jet, |i, j| j.needs_runway = i.parse_bool()?),
    sub_field!("KeepsParkingSpaceWhenAirborne", Jet, |i, j| j.keeps_parking_space_when_airborne = i.parse_bool()?),
    sub_field!("TakeoffDistForMaxLift", Jet, |i, j| j.takeoff_dist_for_max_lift = i.parse_percent_to_real()?),
    sub_field!("TakeoffPause", Jet, |i, j| j.takeoff_pause = i.parse_duration_unsigned_int()?),
    sub_field!("MinHeight", Jet, |i, j| j.min_height = i.parse_real()?),
    sub_field!("ParkingOffset", Jet, |i, j| j.parking_offset = i.parse_real()?),
    sub_field!("SneakyOffsetWhenAttacking", Jet, |i, j| j.sneaky_offset_when_attacking = i.parse_real()?),
    sub_field!("AttackLocomotorType", Jet, |i, j| j.attacking_loco = i.parse_index_list(&LOCOMOTOR_SET_NAMES)?),
    sub_field!("AttackLocomotorPersistTime", Jet, |i, j| j.attack_loco_persist_time = i.parse_duration_unsigned_int()?),
    sub_field!("AttackersMissPersistTime", Jet, |i, j| j.attackers_miss_persist_time = i.parse_duration_unsigned_int()?),
    sub_field!("ReturnForAmmoLocomotorType", Jet, |i, j| {
        let v = i.parse_index_list(&LOCOMOTOR_SET_NAMES)?;
        if plant(4048) { j.attacking_loco = v } else { j.returning_loco = v }
    }),
    sub_field!("LockonTime", Jet, |i, j| j.lockon_time = i.parse_duration_unsigned_int()?),
    sub_field!("LockonCursor", Jet, |i, j| j.lockon_cursor = i.parse_ascii_string()?),
    sub_field!("LockonInitialDist", Jet, |i, j| j.lockon_initial_dist = i.parse_real()?),
    sub_field!("LockonFreq", Jet, |i, j| j.lockon_freq = i.parse_real()?),
    sub_field!("LockonAngleSpin", Jet, |i, j| j.lockon_angle_spin = i.parse_angle_real()?),
    sub_field!("LockonBlinky", Jet, |i, j| j.lockon_blinky = i.parse_bool()?),
    sub_field!("ReturnToBaseIdleTime", Jet, |i, j| j.return_to_base_idle_time = i.parse_duration_unsigned_int()?),
    FieldParse::end(),
];

/// `MissileAIUpdateModuleData::buildFieldParse`'s own table.
static MISSILE_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 18] = [
    sub_field!("TryToFollowTarget", Missile, |i, m| m.try_to_follow_target = i.parse_bool()?),
    sub_field!("FuelLifetime", Missile, |i, m| m.fuel_lifetime = i.parse_duration_unsigned_int()?),
    sub_field!("IgnitionDelay", Missile, |i, m| m.ignition_delay = i.parse_duration_unsigned_int()?),
    sub_field!("InitialVelocity", Missile, |i, m| m.initial_vel = i.parse_velocity_real()?),
    sub_field!("DistanceToTravelBeforeTurning", Missile, |i, m| m.initial_dist = i.parse_real()?),
    sub_field!("DistanceToTargetBeforeDiving", Missile, |i, m| m.dive_distance = i.parse_real()?),
    sub_field!("DistanceToTargetForLock", Missile, |i, m| m.lock_distance = i.parse_real()?),
    sub_field!("IgnitionFX", Missile, |i, m| m.ignition_fx = parse_fx_list(i)?),
    sub_field!("UseWeaponSpeed", Missile, |i, m| m.use_weapon_speed = i.parse_bool()?),
    sub_field!("DetonateOnNoFuel", Missile, |i, m| m.detonate_on_no_fuel = i.parse_bool()?),
    sub_field!("DistanceScatterWhenJammed", Missile, |i, m| m.distance_scatter_when_jammed = i.parse_real()?),
    sub_field!("GarrisonHitKillRequiredKindOf", Missile, |i, m| m.garrison_hit_kill_kindof.parse_from_ini(i)?),
    sub_field!("GarrisonHitKillForbiddenKindOf", Missile, |i, m| m.garrison_hit_kill_kindof_not.parse_from_ini(i)?),
    sub_field!("GarrisonHitKillCount", Missile, |i, m| m.garrison_hit_kill_count = i.parse_unsigned_int()? as i32),
    sub_field!("GarrisonHitKillFX", Missile, |i, m| m.garrison_hit_kill_fx = parse_fx_list(i)?),
    sub_field!("DetonateCallsKill", Missile, |i, m| m.detonate_calls_kill = i.parse_bool()?),
    sub_field!("KillSelfDelay", Missile, |i, m| m.kill_self_delay = i.parse_duration_unsigned_int()? as i32),
    FieldParse::end(),
];

/// `SupplyTruckAIUpdateModuleData::buildFieldParse`'s own table (also a chinook's, before its own).
static SUPPLY_TRUCK_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 6] = [
    FieldParse::new("MaxBoxes", |i, d, _| { supply(d)?.max_boxes_data = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SupplyCenterActionDelay", |i, d, _| { supply(d)?.center_delay = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SupplyWarehouseActionDelay", |i, d, _| { supply(d)?.warehouse_delay = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SupplyWarehouseScanDistance", |i, d, _| { supply(d)?.warehouse_scan_distance = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SuppliesDepletedVoice", |i, d, _| supply(d)?.supplies_depleted_voice.parse(i), UserData::None),
    FieldParse::end(),
];

/// `ChinookAIUpdateModuleData::buildFieldParse`'s own table.
static CHINOOK_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 17] = [
    sub_field!("RappelSpeed", Chinook, |i, c| c.rappel_speed = i.parse_velocity_real()?),
    sub_field!("RopeDropSpeed", Chinook, |i, c| c.rope_drop_speed = i.parse_velocity_real()?),
    sub_field!("RopeName", Chinook, |i, c| c.rope_name = i.parse_ascii_string()?),
    sub_field!("RopeFinalHeight", Chinook, |i, c| c.rope_final_height = i.parse_real()?),
    sub_field!("RopeWidth", Chinook, |i, c| c.rope_width = i.parse_real()?),
    sub_field!("RopeWobbleLen", Chinook, |i, c| c.rope_wobble_len = i.parse_real()?),
    sub_field!("RopeWobbleAmplitude", Chinook, |i, c| c.rope_wobble_amp = i.parse_real()?),
    sub_field!("RopeWobbleRate", Chinook, |i, c| c.rope_wobble_rate = i.parse_angular_velocity_real()?),
    sub_field!("RopeColor", Chinook, |i, c| c.rope_color = i.parse_rgb_color()?),
    sub_field!("NumRopes", Chinook, |i, c| c.num_ropes = i.parse_unsigned_int()?),
    sub_field!("PerRopeDelayMin", Chinook, |i, c| c.per_rope_delay_min = i.parse_duration_unsigned_int()?),
    sub_field!("PerRopeDelayMax", Chinook, |i, c| c.per_rope_delay_max = i.parse_duration_unsigned_int()?),
    sub_field!("MinDropHeight", Chinook, |i, c| c.min_drop_height = i.parse_real()?),
    sub_field!("WaitForRopesToDrop", Chinook, |i, c| c.wait_for_ropes_to_drop = i.parse_bool()?),
    sub_field!("RotorWashParticleSystem", Chinook, |i, c| c.rotor_wash_particle_system = i.parse_ascii_string()?),
    sub_field!("UpgradedSupplyBoost", Chinook, |i, c| c.upgraded_supply_boost = i.parse_int()?),
    FieldParse::end(),
];

/// `DozerAIUpdateModuleData::buildFieldParse`'s own table.
static DOZER_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 4] = [
    sub_field!("RepairHealthPercentPerSecond", Dozer, |i, z| z.repair_health_percent_per_second = i.parse_percent_to_real()?),
    sub_field!("BoredTime", Dozer, |i, z| z.bored_time = i.parse_duration_real()?),
    sub_field!("BoredRange", Dozer, |i, z| z.bored_range = i.parse_real()?),
    FieldParse::end(),
];

/// `WorkerAIUpdateModuleData::buildFieldParse`'s own table.
static WORKER_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 10] = [
    sub_field!("MaxBoxes", Worker, |i, w| w.max_boxes_data = i.parse_int()?),
    sub_field!("RepairHealthPercentPerSecond", Worker, |i, w| w.repair_health_percent_per_second = i.parse_percent_to_real()?),
    sub_field!("BoredTime", Worker, |i, w| w.bored_time = i.parse_duration_real()?),
    sub_field!("BoredRange", Worker, |i, w| w.bored_range = i.parse_real()?),
    sub_field!("SupplyCenterActionDelay", Worker, |i, w| w.center_delay = i.parse_duration_unsigned_int()?),
    sub_field!("SupplyWarehouseActionDelay", Worker, |i, w| w.warehouse_delay = i.parse_duration_unsigned_int()?),
    sub_field!("SupplyWarehouseScanDistance", Worker, |i, w| w.warehouse_scan_distance = i.parse_real()?),
    sub_field!("SuppliesDepletedVoice", Worker, |i, w| w.supplies_depleted_voice.parse(i)?),
    sub_field!("UpgradedSupplyBoost", Worker, |i, w| w.upgraded_supply_boost = i.parse_int()?),
    FieldParse::end(),
];

/// `DeliverPayloadAIUpdateModuleData::buildFieldParse`'s own table.
static DELIVER_PAYLOAD_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 10] = [
    sub_field!("DoorDelay", DeliverPayload, |i, p| p.door_delay = i.parse_duration_unsigned_int()?),
    sub_field!("PutInContainer", DeliverPayload, |i, p| p.put_in_container_name = i.parse_ascii_string()?),
    sub_field!("DeliveryDistance", DeliverPayload, |i, p| p.max_distance_to_target = i.parse_real()?),
    sub_field!("MaxAttempts", DeliverPayload, |i, p| p.max_number_attempts = i.parse_int()?),
    sub_field!("DropDelay", DeliverPayload, |i, p| p.drop_delay = i.parse_duration_unsigned_int()?),
    sub_field!("DropOffset", DeliverPayload, |i, p| p.drop_offset = i.parse_coord3d()?),
    sub_field!("DropVariance", DeliverPayload, |i, p| p.drop_variance = i.parse_coord3d()?),
    sub_field!("DeliveryDecal", DeliverPayload, |i, p| RadiusDecalTemplate::parse_radius_decal_template(i, &mut p.delivery_decal_template)?),
    sub_field!("DeliveryDecalRadius", DeliverPayload, |i, p| p.delivery_decal_radius = i.parse_real()?),
    FieldParse::end(),
];

/// `DeployStyleAIUpdateModuleData::buildFieldParse`'s own table.
static DEPLOY_STYLE_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 7] = [
    sub_field!("UnpackTime", DeployStyle, |i, s| s.unpack_time = i.parse_duration_unsigned_int()?),
    sub_field!("PackTime", DeployStyle, |i, s| s.pack_time = i.parse_duration_unsigned_int()?),
    sub_field!("ResetTurretBeforePacking", DeployStyle, |i, s| s.reset_turret_before_packing = i.parse_bool()?),
    sub_field!("TurretsFunctionOnlyWhenDeployed", DeployStyle, |i, s| s.turrets_function_only_when_deployed = i.parse_bool()?),
    sub_field!("TurretsMustCenterBeforePacking", DeployStyle, |i, s| s.turrets_must_center_before_packing = i.parse_bool()?),
    sub_field!("ManualDeployAnimations", DeployStyle, |i, s| s.manual_deploy_animations = i.parse_bool()?),
    FieldParse::end(),
];

/// `HackInternetAIUpdateModuleData::buildFieldParse`'s own table.
static HACK_INTERNET_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 11] = [
    sub_field!("UnpackTime", HackInternet, |i, h| h.unpack_time = i.parse_duration_unsigned_int()?),
    sub_field!("PackTime", HackInternet, |i, h| h.pack_time = i.parse_duration_unsigned_int()?),
    sub_field!("PackUnpackVariationFactor", HackInternet, |i, h| h.pack_unpack_variation_factor = i.parse_real()?),
    sub_field!("CashUpdateDelay", HackInternet, |i, h| h.cash_update_delay = i.parse_duration_unsigned_int()?),
    sub_field!("CashUpdateDelayFast", HackInternet, |i, h| h.cash_update_delay_fast = i.parse_duration_unsigned_int()?),
    sub_field!("RegularCashAmount", HackInternet, |i, h| h.regular_cash_amount = i.parse_unsigned_int()?),
    sub_field!("VeteranCashAmount", HackInternet, |i, h| h.veteran_cash_amount = i.parse_unsigned_int()?),
    sub_field!("EliteCashAmount", HackInternet, |i, h| h.elite_cash_amount = i.parse_unsigned_int()?),
    sub_field!("HeroicCashAmount", HackInternet, |i, h| h.heroic_cash_amount = i.parse_unsigned_int()?),
    sub_field!("XpPerCashUpdate", HackInternet, |i, h| h.xp_per_cash_update = i.parse_unsigned_int()?),
    FieldParse::end(),
];

/// `AssaultTransportAIUpdateModuleData::buildFieldParse`'s own table.
static ASSAULT_TRANSPORT_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 3] = [
    sub_field!("MembersGetHealedAtLifeRatio", AssaultTransport, |i, a| a.members_get_healed_at_life_ratio = i.parse_real()?),
    sub_field!("ClearRangeRequiredToContinueAttackMove", AssaultTransport, |i, a| a.clear_range_required_to_continue_attack_move = i.parse_real()?),
    FieldParse::end(),
];

/// `RailedTransportAIUpdateModuleData::buildFieldParse`'s own table.
static RAILED_TRANSPORT_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 2] = [
    FieldParse::new(
        "PathPrefixName",
        |i, d, _| match &mut d.subclass {
            AiSubclassModuleData::RailedTransport { path_prefix_name } => {
                *path_prefix_name = i.parse_ascii_string()?;
                Ok(())
            }
            _ => Err(IniError::new(IniErrorKind::Bug)),
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl AiSubclassModuleData {
    /// The AI module class whose data this is, by its module name: its data's defaults, or `None` for a name that is no AI's.
    pub fn for_module(name: &[u8]) -> Option<AiSubclassModuleData> {
        Some(match name {
            b"AIUpdateInterface" | b"WanderAIUpdate" | b"TransportAIUpdate" => AiSubclassModuleData::Plain,
            b"JetAIUpdate" => AiSubclassModuleData::Jet(JetAiModuleData::default()),
            b"MissileAIUpdate" => AiSubclassModuleData::Missile(MissileAiModuleData::default()),
            b"SupplyTruckAIUpdate" => AiSubclassModuleData::SupplyTruck(SupplyTruckAiModuleData::default()),
            b"ChinookAIUpdate" => AiSubclassModuleData::Chinook(ChinookAiModuleData::default()),
            b"DozerAIUpdate" => AiSubclassModuleData::Dozer(DozerAiModuleData::default()),
            b"WorkerAIUpdate" => AiSubclassModuleData::Worker(WorkerAiModuleData::default()),
            b"DeliverPayloadAIUpdate" => AiSubclassModuleData::DeliverPayload(DeliverPayloadAiModuleData::default()),
            b"DeployStyleAIUpdate" => AiSubclassModuleData::DeployStyle(DeployStyleAiModuleData::default()),
            b"HackInternetAIUpdate" => AiSubclassModuleData::HackInternet(HackInternetAiModuleData::default()),
            b"AssaultTransportAIUpdate" => AiSubclassModuleData::AssaultTransport(AssaultTransportAiModuleData::default()),
            b"RailedTransportAIUpdate" => AiSubclassModuleData::RailedTransport { path_prefix_name: Vec::new() },
            _ => return None,
        })
    }

    /// The subclass's `buildFieldParse` after `AIUpdateModuleData::buildFieldParse(p)`: its own tables (a chinook's, the supply
    /// truck's then its own).
    pub fn add_field_parse(&self, p: &mut MultiIniFieldParse<'_, AiUpdateModuleData>) -> IniResult<()> {
        match self {
            AiSubclassModuleData::Plain => Ok(()),
            AiSubclassModuleData::Jet(_) => p.add(&JET_FIELD_PARSE),
            AiSubclassModuleData::Missile(_) => p.add(&MISSILE_FIELD_PARSE),
            AiSubclassModuleData::SupplyTruck(_) => p.add(&SUPPLY_TRUCK_FIELD_PARSE),
            AiSubclassModuleData::Chinook(_) => {
                p.add(&SUPPLY_TRUCK_FIELD_PARSE)?;
                p.add(&CHINOOK_FIELD_PARSE)
            }
            AiSubclassModuleData::Dozer(_) => p.add(&DOZER_FIELD_PARSE),
            AiSubclassModuleData::Worker(_) => p.add(&WORKER_FIELD_PARSE),
            AiSubclassModuleData::DeliverPayload(_) => p.add(&DELIVER_PAYLOAD_FIELD_PARSE),
            AiSubclassModuleData::DeployStyle(_) => p.add(&DEPLOY_STYLE_FIELD_PARSE),
            AiSubclassModuleData::HackInternet(_) => p.add(&HACK_INTERNET_FIELD_PARSE),
            AiSubclassModuleData::AssaultTransport(_) => p.add(&ASSAULT_TRANSPORT_FIELD_PARSE),
            AiSubclassModuleData::RailedTransport { .. } => p.add(&RAILED_TRANSPORT_FIELD_PARSE),
        }
    }
}

/// `friend_newModuleData(ini)` of the AI module class `name`: `AIUpdateModuleData`'s defaults with the subclass's, filled from
/// `ini` by the base's table then the subclass's. `None` for a name that is no AI's.
pub fn new_subclass_module_data(name: &[u8], ini: Option<&mut Ini<'_>>) -> Option<IniResult<AiUpdateModuleData>> {
    let subclass = AiSubclassModuleData::for_module(name)?;
    let tables = subclass.clone();
    let mut data = AiUpdateModuleData { subclass, ..AiUpdateModuleData::default() };
    Some(match ini {
        Some(ini) => ini.init_from_ini_multi_proc(&mut data, |p| AiUpdateModuleData::build_field_parse_for(&tables, p)).map(|_| data),
        None => Ok(data),
    })
}

/// The module factory's data procedure of an AI module class (its `friend_newModuleData`), by the class's name.
macro_rules! subclass_data_proc {
    ($f:ident, $name:literal) => {
        #[doc = concat!("`", $name, "::friend_newModuleData` as the module factory's data procedure.")]
        pub fn $f(ini: Option<&mut Ini<'_>>) -> IniResult<crate::common::module::ModuleDataKind> {
            let data = new_subclass_module_data($name.as_bytes(), ini).expect("an AI module class")?;
            Ok(crate::common::module::ModuleDataKind::AiUpdate(std::sync::Arc::new(data)))
        }
    };
}
subclass_data_proc!(new_jet_module_data, "JetAIUpdate");
subclass_data_proc!(new_missile_module_data, "MissileAIUpdate");
subclass_data_proc!(new_supply_truck_module_data, "SupplyTruckAIUpdate");
subclass_data_proc!(new_chinook_module_data, "ChinookAIUpdate");
subclass_data_proc!(new_dozer_module_data, "DozerAIUpdate");
subclass_data_proc!(new_worker_module_data, "WorkerAIUpdate");
subclass_data_proc!(new_deliver_payload_module_data, "DeliverPayloadAIUpdate");
subclass_data_proc!(new_deploy_style_module_data, "DeployStyleAIUpdate");
subclass_data_proc!(new_hack_internet_module_data, "HackInternetAIUpdate");
subclass_data_proc!(new_assault_transport_module_data, "AssaultTransportAIUpdate");
subclass_data_proc!(new_railed_transport_module_data, "RailedTransportAIUpdate");

#[cfg(test)]
mod tests {
    use super::*;

    /// Every AI module class has its data's defaults (the constructors' values), and no other name is an AI's.
    #[test]
    fn each_class_has_its_defaults() {
        let jet = AiSubclassModuleData::for_module(b"JetAIUpdate").unwrap();
        match jet {
            AiSubclassModuleData::Jet(j) => {
                assert!(j.needs_runway && j.keeps_parking_space_when_airborne && !j.lockon_blinky);
                assert_eq!((j.lockon_initial_dist, j.lockon_freq, j.lockon_angle_spin), (100.0, 0.5, 720.0));
            }
            d => panic!("{d:?}"),
        }
        match AiSubclassModuleData::for_module(b"ChinookAIUpdate").unwrap() {
            AiSubclassModuleData::Chinook(c) => {
                assert_eq!(c.rappel_speed, 15.0);
                assert_eq!(c.supply.warehouse_scan_distance, 100.0);
                assert_eq!(c.rope_name, b"GenericRope");
            }
            d => panic!("{d:?}"),
        }
        assert_eq!(AiSubclassModuleData::for_module(b"WanderAIUpdate"), Some(AiSubclassModuleData::Plain));
        assert_eq!(AiSubclassModuleData::for_module(b"PhysicsBehavior"), None);
    }
}
