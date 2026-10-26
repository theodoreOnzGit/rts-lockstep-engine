// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SlavedUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SlavedUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SlavedUpdate`: a slave of a spawn behaviour (scout and battle drones, the stinger site's soldiers). Every quarter second it follows its
//! master: a master gone, dead or unmanned frees it (unmanned, idle); a master on another side takes it along (`defect`). A wounded master
//! (or one below `RepairWhenBelowHealth%`) is repaired: the drone flies to it, unpacks, welds (sparks, a sound, the healing) and retracts, at
//! random spots `RepairRange` from it. Otherwise it goes to the master's victim (`AttackRange`, granting the drone-spotting bonus within
//! `DistToTargetToGrantRangeBonus`), scouts ahead of a moving master (`ScoutRange`), or guards a point `GuardMaxRange` from it.
//!
//! # Upstream facts kept
//!
//! - The random directions are `GameLogicRandomValue(0, 2*PI)`: the integer draw, 0 to 6; the repair altitudes the real draw.
//! - The guard wander uses `GuardMaxRange` for the radius, not `GuardWanderRange` (which only enables it).
//! - `sqr` of an `Int` range is `Int` arithmetic (`Undefined::SignedIntOverflow` past `Int`); the health percentage is a `Real` truncated to
//!   `Int` (`Undefined::FloatToIntOutOfRange` outside it).
//! - The scout test reads the master's AI unchecked (`Undefined::NullAi`), and the side test the master's team (`Undefined::NullTeam`); the
//!   welding sparks read the drawable unchecked (`Undefined::NullDrawable`).
//! - `xfer`: the version, the update base, the slaver, the guard offset, the countdown, the repair state (its raw word) and repairing.
//!
//! `p1w13-sonnet`, batch R57, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND};
use crate::logic::world::object_state::mask_of;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{
    ModelConditionFlagType, MODELCONDITION_BETWEEN_FIRING_SHOTS_B, MODELCONDITION_BETWEEN_FIRING_SHOTS_C, MODELCONDITION_FIRING_B, MODELCONDITION_FIRING_C,
    MODELCONDITION_PACKING, MODELCONDITION_RELOADING_B, MODELCONDITION_RELOADING_C, MODELCONDITION_UNPACKING,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNSELECTABLE};
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::condition::DRONE_SPOTTING as WEAPONBONUSCONDITION_DRONE_SPOTTING;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `SLAVED_UPDATE_RATE`: `LOGICFRAMES_PER_SECOND/4` ("a low priority module that only needs to be called every this many frames").
pub const SLAVED_UPDATE_RATE: i32 = LOGICFRAMES_PER_SECOND / 4;
/// `CLOSE_ENOUGH_SQR`.
const CLOSE_ENOUGH_SQR: f32 = 15.0 * 15.0;
/// `STRAY_MULTIPLIER`.
const STRAY_MULTIPLIER: f32 = 2.0;
/// `LOCOMOTORSET_NORMAL`, `LOCOMOTORSET_PANIC`.
const LOCOMOTORSET_NORMAL: i32 = 0;
const LOCOMOTORSET_PANIC: i32 = 4;
/// `PI` (BaseType.h).
const PI: f32 = std::f32::consts::PI;

/// `RepairStates`.
pub const REPAIRSTATE_NONE: i32 = 0;
pub const REPAIRSTATE_UNPACKING: i32 = 1;
pub const REPAIRSTATE_PACKING: i32 = 2;
pub const REPAIRSTATE_READY: i32 = 3;
pub const REPAIRSTATE_EXTENDING: i32 = 4;
pub const REPAIRSTATE_RETRACTING: i32 = 5;
pub const REPAIRSTATE_WELDING: i32 = 6;

/// `class SlavedUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SlavedUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_guardMaxRange`, `m_guardWanderRange`, `m_attackRange`, `m_attackWanderRange`, `m_scoutRange`, `m_scoutWanderRange`,
    /// `m_distToTargetToGrantRangeBonus`, `m_repairRange`.
    pub guard_max_range: i32,
    pub guard_wander_range: i32,
    pub attack_range: i32,
    pub attack_wander_range: i32,
    pub scout_range: i32,
    pub scout_wander_range: i32,
    pub dist_to_target_to_grant_range_bonus: i32,
    pub repair_range: i32,
    /// `m_repairMinAltitude`, `m_repairMaxAltitude`, `m_repairRatePerSecond`.
    pub repair_min_altitude: f32,
    pub repair_max_altitude: f32,
    pub repair_rate_per_second: f32,
    /// `m_repairWhenHealthBelowPercentage`.
    pub repair_when_health_below_percentage: i32,
    /// `m_minReadyFrames`, `m_maxReadyFrames`, `m_minWeldFrames`, `m_maxWeldFrames` (durations, kept in `Int`s).
    pub min_ready_frames: i32,
    pub max_ready_frames: i32,
    pub min_weld_frames: i32,
    pub max_weld_frames: i32,
    /// `m_weldingSysName`, `m_weldingFXBone`.
    pub welding_sys_name: Vec<u8>,
    pub welding_fx_bone: Vec<u8>,
    /// `m_stayOnSameLayerAsMaster`.
    pub stay_on_same_layer_as_master: bool,
}

impl Default for SlavedUpdateModuleData {
    /// `SlavedUpdateModuleData()` (the repair range is not set: the pool's zero).
    fn default() -> Self {
        SlavedUpdateModuleData {
            update: UpdateModuleData::new(),
            guard_max_range: 0,
            guard_wander_range: 0,
            attack_range: 0,
            attack_wander_range: 0,
            scout_range: 0,
            scout_wander_range: 0,
            dist_to_target_to_grant_range_bonus: 0,
            repair_range: 0,
            repair_min_altitude: 0.0,
            repair_max_altitude: 0.0,
            repair_rate_per_second: 0.0,
            repair_when_health_below_percentage: 0,
            min_ready_frames: 0,
            max_ready_frames: 0,
            min_weld_frames: 0,
            max_weld_frames: 0,
            welding_sys_name: Vec::new(),
            welding_fx_bone: Vec::new(),
            stay_on_same_layer_as_master: false,
        }
    }
}

/// `INI::parseDurationUnsignedInt` into an `Int` field (the bits kept).
fn duration(ini: &mut Ini<'_>) -> IniResult<i32> {
    Ok(ini.parse_duration_unsigned_int()? as i32)
}

static FIELD_PARSE: [FieldParse<SlavedUpdateModuleData>; 20] = [
    FieldParse::new("GuardMaxRange", |i, d, _| { d.guard_max_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GuardWanderRange", |i, d, _| { d.guard_wander_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AttackRange", |i, d, _| { d.attack_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AttackWanderRange", |i, d, _| { d.attack_wander_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScoutRange", |i, d, _| { d.scout_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScoutWanderRange", |i, d, _| { d.scout_wander_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairRange", |i, d, _| { d.repair_range = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMinAltitude", |i, d, _| { d.repair_min_altitude = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMaxAltitude", |i, d, _| { d.repair_max_altitude = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DistToTargetToGrantRangeBonus", |i, d, _| { d.dist_to_target_to_grant_range_bonus = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairRatePerSecond", |i, d, _| { d.repair_rate_per_second = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairWhenBelowHealth%", |i, d, _| { d.repair_when_health_below_percentage = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMinReadyTime", |i, d, _| { d.min_ready_frames = duration(i)?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMaxReadyTime", |i, d, _| { d.max_ready_frames = duration(i)?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMinWeldTime", |i, d, _| { d.min_weld_frames = duration(i)?; Ok(()) }, UserData::None),
    FieldParse::new("RepairMaxWeldTime", |i, d, _| { d.max_weld_frames = duration(i)?; Ok(()) }, UserData::None),
    FieldParse::new("RepairWeldingSys", |i, d, _| { d.welding_sys_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("RepairWeldingFXBone", |i, d, _| { d.welding_fx_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("StayOnSameLayerAsMaster", |i, d, _| { d.stay_on_same_layer_as_master = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SlavedUpdateModuleData {
    /// `buildFieldParse(p)`: the update base's (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SlavedUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SlavedUpdateModuleData> {
        let mut data = SlavedUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SlavedUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `SlavedUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SlavedUpdate(Arc::new(SlavedUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `sqr` of an `Int`: `Int` arithmetic.
fn sqr_int(x: i32) -> Result<f32, Undefined> {
    x.checked_mul(x).map(|v| v as f32).ok_or(Undefined::SignedIntOverflow)
}

/// `(Int)r`.
fn real_to_int(r: f32) -> Result<i32, Undefined> {
    if r > -2_147_483_649.0 && r < 2_147_483_648.0 {
        Ok(r as i32)
    } else {
        Err(Undefined::FloatToIntOutOfRange)
    }
}

/// `class SlavedUpdate : public UpdateModule, public SlavedUpdateInterface`.
#[derive(Clone, Debug)]
pub struct SlavedUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getSlavedUpdateModuleData()`.
    data: Arc<SlavedUpdateModuleData>,
    /// `m_slaver`.
    slaver: ObjectId,
    /// `m_guardPointOffset`.
    guard_point_offset: Coord3D,
    /// `m_framesToWait`.
    frames_to_wait: i32,
    /// `m_repairState`.
    repair_state: i32,
    /// `m_repairing`.
    repairing: bool,
}

impl PartialEq for SlavedUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl SlavedUpdate {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SlavedUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(SlavedUpdate { update, data, slaver: INVALID_ID, guard_point_offset: Coord3D::default(), frames_to_wait: 0, repair_state: REPAIRSTATE_NONE, repairing: false })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<SlavedUpdateModuleData> {
        &self.data
    }

    /// The slaver, the guard offset (bits), the countdown, the repair state, repairing.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.slaver as u32,
            self.guard_point_offset.x.to_bits(),
            self.guard_point_offset.y.to_bits(),
            self.guard_point_offset.z.to_bits(),
            self.frames_to_wait as u32,
            self.repair_state as u32,
            u32::from(self.repairing),
        ]
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

    fn ai(world: &World, id: ObjectId) -> Option<crate::logic::world::object::ModuleIndex> {
        world.objects.find(id).and_then(|o| o.ai)
    }

    fn ai_call(world: &mut World, id: ObjectId, call: ModuleCall) -> Option<ModuleReply> {
        let ai = Self::ai(world, id)?;
        Some(world.module_call(id, ai, call))
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// A random direction: `GameLogicRandomValue( 0, 2*PI )` (the `Int` draw).
    fn random_direction(world: &mut World) -> f32 {
        world.random.game_logic_random_value(0, (2.0 * PI) as i32) as f32
    }

    /// `getCurLocomotor()->...` when the AI has a locomotor.
    fn has_locomotor(world: &mut World, id: ObjectId) -> bool {
        Self::ai_call(world, id, ModuleCall::AiHasCurLocomotor).is_some_and(|r| r.as_bool())
    }

    fn set_use_precise_z_pos(world: &mut World, id: ObjectId, on: bool) {
        if Self::has_locomotor(world, id) {
            Self::ai_call(world, id, ModuleCall::AiLocomotorSetUsePreciseZPos(on));
        }
    }

    /// `onObjectCreated()`.
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        if self.data.repair_rate_per_second > 0.0 {
            world.set_model_condition_state(us, MODELCONDITION_PACKING);
        }
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        if self.frames_to_wait > 0 {
            self.frames_to_wait -= 1;
        }
        if self.repair_state == REPAIRSTATE_NONE {
            if self.frames_to_wait > 0 {
                return Ok(());
            }
            self.frames_to_wait = if crate::logic::modules::plant(2271) { 1 } else { SLAVED_UPDATE_RATE };
        }
        if self.slaver == INVALID_ID {
            return Ok(());
        }
        if world.objects.find(us).is_none() || Self::ai(world, us).is_none() || !Self::has_locomotor(world, us) {
            return Ok(());
        }
        let master = self.slaver;
        let gone = match world.objects.find(master) {
            None => true,
            Some(m) => m.is_effectively_dead() || m.is_disabled_by_type(DISABLED_UNMANNED),
        };
        if gone {
            self.stop_slaved_effects(world, us);
            world.body_set_disabled(us, DISABLED_UNMANNED);
            Self::ai_call(world, us, ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI });
            return Ok(());
        }
        // "slaver must have been hijacked or something.. we will join his team"
        let master_team = world.get_team(master).ok_or(Undefined::NullTeam)?;
        let my_team = world.get_team(us);
        if world.cuts.team_relationship(master_team, my_team) != Relationship::Allies {
            world.defect(us, master_team, 0);
        }
        if d.stay_on_same_layer_as_master {
            let layer = world.objects.find(master).map(|o| o.layer).unwrap_or_default();
            world.contain_set_layer(us, layer);
        }
        world.clear_weapon_bonus_condition(master, WEAPONBONUSCONDITION_DRONE_SPOTTING);
        let master_ai = Self::ai(world, master);
        let target = match master_ai {
            Some(ai) => Some(world.module_call(master, ai, ModuleCall::AiGetCurrentVictim).as_id()).filter(|&v| world.objects.find(v).is_some()),
            None => None,
        };
        let mut health_percentage = 100;
        if d.repair_rate_per_second > 0.0 {
            if let Some((health, max_health)) = world.body_health(master) {
                health_percentage = real_to_int(health / max_health * 100.0)?;
            }
        }
        if health_percentage <= d.repair_when_health_below_percentage {
            // "Prioritize repairing"
            return self.do_repair_logic(world, us);
        }
        if d.attack_range != 0 {
            if let Some(target) = target {
                self.end_repair(world, us);
                return self.do_attack_logic(world, us, target);
            }
        }
        if d.scout_range != 0 {
            let ai = master_ai.ok_or(Undefined::NullAi)?;
            if let ModuleReply::Coord(master_dest) = world.module_call(master, ai, ModuleCall::AiGetPathLastNodePosition) {
                let dist_sqr = world.partition_distance_squared_to_pos(master, &master_dest, DistanceCalculationType::FromBoundingSphere2D);
                let half = d.guard_max_range as f32 * 0.5;
                if dist_sqr > half * half {
                    self.end_repair(world, us);
                    return self.do_scout_logic(world, us, &master_dest);
                }
            }
        }
        if health_percentage < 100 && !crate::logic::modules::plant(2272) {
            return self.do_repair_logic(world, us);
        }
        let mut pinned = Self::position(world, master);
        pinned.x += self.guard_point_offset.x;
        pinned.y += self.guard_point_offset.y;
        self.guard_point_offset.z = world.terrain_get_ground_height(pinned.x, pinned.y);
        if d.guard_max_range != 0 {
            let idle = Self::ai_call(world, us, ModuleCall::AiIsIdle).is_some_and(|r| r.as_bool());
            if idle && world.partition_distance_squared_to_pos(us, &pinned, DistanceCalculationType::FromCenter3D) > CLOSE_ENOUGH_SQR {
                self.end_repair(world, us);
                self.do_guard_logic(world, us, &mut pinned);
            } else {
                let stray = STRAY_MULTIPLIER * d.guard_max_range as f32;
                if world.partition_distance_squared(us, master, DistanceCalculationType::FromCenter3D) > stray * stray {
                    self.end_repair(world, us);
                    self.do_guard_logic(world, us, &mut pinned);
                }
            }
        }
        Ok(())
    }

    /// `doAttackLogic(target)`.
    fn do_attack_logic(&mut self, world: &mut World, us: ObjectId, target: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let master = self.slaver;
        let target_pos = Self::position(world, target);
        let dist = world.partition_distance_squared_to_pos(us, &target_pos, DistanceCalculationType::FromBoundingSphere2D);
        let mut attack_position = if dist > sqr_int(d.attack_range)? {
            // "Outside of range, so pick a point near the master in the direction of the target."
            let master_pos = Self::position(world, master);
            let mut v = target_pos;
            v.sub(&master_pos);
            v.normalize();
            v.scale(d.attack_range as f32);
            let mut p = master_pos;
            p.add(&v);
            p
        } else {
            target_pos
        };
        if d.attack_wander_range != 0 {
            let dir = Self::random_direction(world);
            self.guard_point_offset = Coord3D::default();
            self.guard_point_offset.x += d.attack_wander_range as f32 * trig::cos(dir);
            self.guard_point_offset.y += d.attack_wander_range as f32 * trig::sin(dir);
            attack_position.x += self.guard_point_offset.x;
            attack_position.y += self.guard_point_offset.y;
            self.guard_point_offset.z = world.terrain_get_ground_height(attack_position.x, attack_position.y);
        }
        Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos: attack_position, command_source: CMD_FROM_AI });
        if dist < sqr_int(d.dist_to_target_to_grant_range_bonus)? && !crate::logic::modules::plant(2273) {
            world.set_weapon_bonus_condition(master, WEAPONBONUSCONDITION_DRONE_SPOTTING);
        }
        Ok(())
    }

    /// `doScoutLogic(mastersDestination)`.
    fn do_scout_logic(&mut self, world: &mut World, us: ObjectId, masters_destination: &Coord3D) -> Result<(), Undefined> {
        let d = self.data.clone();
        let master = self.slaver;
        let dist = world.partition_distance_squared_to_pos(us, masters_destination, DistanceCalculationType::FromBoundingSphere2D);
        let mut scout_position = if dist > sqr_int(d.scout_range)? {
            let master_pos = Self::position(world, master);
            let mut v = *masters_destination;
            v.sub(&master_pos);
            v.normalize();
            v.scale(d.scout_range as f32);
            let mut p = master_pos;
            p.add(&v);
            p
        } else {
            *masters_destination
        };
        if d.scout_wander_range != 0 {
            let dir = Self::random_direction(world);
            self.guard_point_offset = Coord3D::default();
            self.guard_point_offset.x += d.scout_wander_range as f32 * trig::cos(dir);
            self.guard_point_offset.y += d.scout_wander_range as f32 * trig::sin(dir);
            scout_position.x += self.guard_point_offset.x;
            scout_position.y += self.guard_point_offset.y;
            self.guard_point_offset.z = world.terrain_get_ground_height(scout_position.x, scout_position.y);
        }
        Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos: scout_position, command_source: CMD_FROM_AI });
        Ok(())
    }

    /// `doGuardLogic(pinnedPosition)`.
    fn do_guard_logic(&mut self, world: &mut World, us: ObjectId, pinned: &mut Coord3D) {
        let d = self.data.clone();
        if d.guard_wander_range != 0 {
            let dir = Self::random_direction(world);
            // (the wander radius is `m_guardMaxRange`, upstream's)
            let radius = if crate::logic::modules::plant(2274) { d.guard_wander_range } else { d.guard_max_range } as f32;
            self.guard_point_offset = Coord3D::default();
            self.guard_point_offset.x += radius * trig::cos(dir);
            self.guard_point_offset.y += radius * trig::sin(dir);
            pinned.x += self.guard_point_offset.x;
            pinned.y += self.guard_point_offset.y;
            self.guard_point_offset.z = world.terrain_get_ground_height(pinned.x, pinned.y);
        }
        Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos: *pinned, command_source: CMD_FROM_AI });
    }

    /// `doRepairLogic()`.
    fn do_repair_logic(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let master = self.slaver;
        if Self::ai(world, us).is_none() {
            return Ok(());
        }
        let dist_sqr = world.partition_distance_squared(us, master, DistanceCalculationType::FromBoundingSphere2D);
        let close_enough = dist_sqr < 12.0 * 12.0;
        if close_enough {
            match self.repair_state {
                REPAIRSTATE_NONE => self.set_repair_state(world, us, REPAIRSTATE_READY)?,
                REPAIRSTATE_READY | REPAIRSTATE_EXTENDING if self.frames_to_wait == 0 => self.set_repair_state(world, us, REPAIRSTATE_WELDING)?,
                REPAIRSTATE_UNPACKING | REPAIRSTATE_WELDING | REPAIRSTATE_RETRACTING if self.frames_to_wait == 0 => {
                    self.set_repair_state(world, us, REPAIRSTATE_READY)?
                }
                _ => {}
            }
        } else {
            self.repairing = false;
            let radius = world.objects.find(master).map_or(0.0, |o| o.geometry_info.get_bounding_sphere_radius());
            let close_enough_for_z_precision = dist_sqr < (radius * 2.0) * (radius * 2.0);
            Self::set_use_precise_z_pos(world, us, close_enough_for_z_precision);
            let mut pos = Self::position(world, master);
            let altitude = world.random.game_logic_random_value_real(d.repair_min_altitude, d.repair_max_altitude);
            pos.z += altitude;
            Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI });
            if self.frames_to_wait == 0 {
                self.set_repair_state(world, us, REPAIRSTATE_READY)?;
            }
        }
        if close_enough && self.repairing {
            if let Some(body) = world.get_body_module(master) {
                let mut healing = DamageInfo::default();
                healing.input.amount = d.repair_rate_per_second / if crate::logic::modules::plant(2275) { 15.0 } else { LOGICFRAMES_PER_SECOND as f32 };
                healing.input.damage_type = DAMAGE_HEALING;
                healing.input.death_type = DEATH_NONE;
                world.module_call(master, body, ModuleCall::AttemptHealing(healing));
            }
        }
        Ok(())
    }

    /// `endRepair()`.
    fn end_repair(&mut self, world: &mut World, us: ObjectId) {
        if self.repair_state != REPAIRSTATE_NONE {
            self.repair_state = REPAIRSTATE_NONE;
            self.frames_to_wait = SLAVED_UPDATE_RATE;
            self.repairing = false;
            Self::set_repair_model_condition_states(world, us, MODELCONDITION_PACKING);
        }
        if Self::ai(world, us).is_some() {
            Self::ai_call(world, us, ModuleCall::AiChooseLocomotorSet(LOCOMOTORSET_NORMAL));
            if Self::has_locomotor(world, us) {
                Self::ai_call(world, us, ModuleCall::AiLocomotorSetUltraAccurate(false));
                Self::ai_call(world, us, ModuleCall::AiLocomotorSetUsePreciseZPos(false));
            }
        }
    }

    /// `setRepairModelConditionStates(flag)`.
    fn set_repair_model_condition_states(world: &mut World, us: ObjectId, flag: ModelConditionFlagType) {
        for f in [
            MODELCONDITION_PACKING,
            MODELCONDITION_UNPACKING,
            MODELCONDITION_FIRING_B,
            MODELCONDITION_FIRING_C,
            MODELCONDITION_BETWEEN_FIRING_SHOTS_B,
            MODELCONDITION_BETWEEN_FIRING_SHOTS_C,
            MODELCONDITION_RELOADING_B,
            MODELCONDITION_RELOADING_C,
        ] {
            world.clear_model_condition_state(us, f);
        }
        world.set_model_condition_state(us, flag);
    }

    /// `setRepairState(repairState)`.
    fn set_repair_state(&mut self, world: &mut World, us: ObjectId, repair_state: i32) -> Result<(), Undefined> {
        let d = self.data.clone();
        if repair_state == self.repair_state {
            return Ok(());
        }
        match repair_state {
            REPAIRSTATE_UNPACKING => {
                Self::set_repair_model_condition_states(world, us, MODELCONDITION_UNPACKING);
                self.frames_to_wait = 15;
            }
            REPAIRSTATE_PACKING => {
                Self::set_repair_model_condition_states(world, us, MODELCONDITION_PACKING);
                self.frames_to_wait = 15;
            }
            REPAIRSTATE_READY => match self.repair_state {
                REPAIRSTATE_NONE => {
                    Self::set_repair_model_condition_states(world, us, MODELCONDITION_UNPACKING);
                    self.repair_state = REPAIRSTATE_UNPACKING;
                    self.frames_to_wait = 15;
                }
                REPAIRSTATE_WELDING => {
                    self.repair_state = REPAIRSTATE_RETRACTING;
                    self.frames_to_wait = 5;
                    Self::set_repair_model_condition_states(world, us, MODELCONDITION_FIRING_C);
                    if !crate::logic::modules::plant(2276) {
                        self.move_to_new_repair_spot(world, us);
                    }
                }
                _ => {
                    self.repair_state = REPAIRSTATE_READY;
                    self.frames_to_wait = world.random.game_logic_random_value(d.min_ready_frames, d.max_ready_frames);
                }
            },
            REPAIRSTATE_WELDING => {
                if self.repair_state == REPAIRSTATE_READY {
                    self.repair_state = REPAIRSTATE_EXTENDING;
                    self.frames_to_wait = 5;
                    Self::set_repair_model_condition_states(world, us, MODELCONDITION_FIRING_B);
                } else {
                    self.repair_state = REPAIRSTATE_WELDING;
                    self.frames_to_wait = world.random.game_logic_random_value(d.min_weld_frames, d.max_weld_frames);
                    if !d.welding_sys_name.is_empty() && world.body_particle_find_template(&d.welding_sys_name) {
                        let id = world.create_particle_system_unplaced(d.welding_sys_name.clone());
                        if !world.has_drawable(us) {
                            return Err(Undefined::NullDrawable);
                        }
                        let me_pos = Self::position(world, us);
                        let pos = match world.drawable_pristine_bones(us, &d.welding_fx_bone, 0, 1).first() {
                            Some(b) => Coord3D { x: b.x + me_pos.x, y: b.y + me_pos.y, z: b.z + me_pos.z },
                            None => me_pos,
                        };
                        world.particle_system_set_position(id, pos);
                        let time = self.frames_to_wait.checked_mul(LOGICFRAMES_PER_SECOND).ok_or(Undefined::SignedIntOverflow)? as f32;
                        world.particle_system_set_lifetime_range(id, time, time);
                        world.emit(ObjectClientEvent::Sound { sound: ObjectSound::RepairSparks, object_id: INVALID_ID, position: Some(pos) });
                    }
                    if !self.repairing {
                        self.repairing = !crate::logic::modules::plant(2277);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `moveToNewRepairSpot()`.
    fn move_to_new_repair_spot(&mut self, world: &mut World, us: ObjectId) {
        let d = self.data.clone();
        let master = self.slaver;
        if d.repair_range == 0 {
            return;
        }
        let dir = Self::random_direction(world);
        self.guard_point_offset = Self::position(world, master);
        self.guard_point_offset.x += d.repair_range as f32 * trig::cos(dir);
        self.guard_point_offset.y += d.repair_range as f32 * trig::sin(dir);
        self.guard_point_offset.z = world.terrain_get_ground_height(self.guard_point_offset.x, self.guard_point_offset.y);
        let altitude = world.random.game_logic_random_value_real(d.repair_min_altitude, d.repair_max_altitude);
        self.guard_point_offset.z += altitude;
        if Self::ai(world, us).is_some() {
            Self::ai_call(world, us, ModuleCall::AiChooseLocomotorSet(LOCOMOTORSET_PANIC));
            // `ai->getCurLocomotor()->setUltraAccurate( true )`, unchecked
            Self::ai_call(world, us, ModuleCall::AiLocomotorSetUltraAccurate(true));
            Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos: self.guard_point_offset, command_source: CMD_FROM_AI });
            Self::set_use_precise_z_pos(world, us, true);
        }
    }

    /// `startSlavedEffects(slaver)`: "we have been marked as Slaved".
    fn start_slaved_effects(&mut self, world: &mut World, us: ObjectId, slaver: ObjectId) {
        if world.objects.find(slaver).is_none() {
            return;
        }
        self.slaver = slaver;
        let d = self.data.clone();
        let dir = Self::random_direction(world);
        self.guard_point_offset = Coord3D::default();
        self.guard_point_offset.x += d.guard_max_range as f32 * trig::cos(dir);
        self.guard_point_offset.y += d.guard_max_range as f32 * trig::sin(dir);
        world.body_set_status(us, mask_of(OBJECT_STATUS_UNSELECTABLE), true);
        if world.objects.find(slaver).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED)) {
            world.contain_stealth_receive_grant(us, true, 0);
        }
    }

    /// `stopSlavedEffects()`: "we are no longer slaved".
    fn stop_slaved_effects(&mut self, world: &mut World, us: ObjectId) {
        self.slaver = INVALID_ID;
        self.guard_point_offset = Coord3D::default();
        if !crate::logic::modules::plant(2278) {
            world.body_set_status(us, mask_of(OBJECT_STATUS_UNSELECTABLE), false);
        }
        world.body_clear_disabled(us, DISABLED_HELD);
    }
}

impl UpdateBehavior for SlavedUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run_update(world, me.object) {
            world.cuts.module_errors.push((me, IniError::undefined(e)));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.slaver)?;
        xfer.xfer_coord3d(&mut self.guard_point_offset)?;
        xfer.xfer_int(&mut self.frames_to_wait)?;
        // `xferUser(&m_repairState, sizeof(RepairStates))`
        let mut b = self.repair_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.repair_state = i32::from_le_bytes(b);
        if crate::logic::modules::plant(2279) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.repairing)
    }
    /// The slaved interface (`onEnslave`, `onSlaverDie`, `onSlaverDamage`, `getSlaverID`) and `onObjectCreated`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::SlavedOnEnslave { slaver } => {
                self.start_slaved_effects(world, me.object, slaver);
                ModuleReply::Done
            }
            ModuleCall::SlavedOnSlaverDie => {
                self.stop_slaved_effects(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::SlavedOnSlaverDamage(info) => {
                Self::ai_call(world, me.object, ModuleCall::AiGoProne { info, command_source: CMD_FROM_AI });
                ModuleReply::Done
            }
            ModuleCall::GetSlaverId => ModuleReply::Id(self.slaver),
            // batch R73 (SpawnBehavior): the slaved update interface; `isSelfTasking()` is the base's FALSE
            ModuleCall::HasSlavedUpdateInterface => ModuleReply::Bool(true),
            ModuleCall::SlavedIsSelfTasking => ModuleReply::Bool(false),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SlavedUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SlavedUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SlavedUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base and the fields.
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
        assert_eq!(SLAVED_UPDATE_RATE, 7);
        let m = SlavedUpdate::unconstructed(1, Some(Arc::new(SlavedUpdateModuleData::default()))).unwrap();
        assert_eq!(m.state_words(), vec![INVALID_ID as u32, 0, 0, 0, 0, 0, 0]);
    }
}
