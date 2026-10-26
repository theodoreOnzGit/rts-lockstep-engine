// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpawnBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SpawnBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpawnBehavior`: an object that keeps a number of spawn (a stinger site's soldiers, a mob's members, a spectre's... anything
//! `SpawnTemplateName` lists, in turn). Every half second it makes the spawn whose replacement time has passed, through its exit interface
//! (by its door, by budding onto the spawn nearest it, or, for its first burst, by the door of the building that produced it), tells each
//! spawn's slaved update who its master is, and keeps their ids. A spawn's death (`onSpawnDeath`) schedules its replacement
//! `SpawnReplaceDelay` later; a one-shot spawner stops after its batch. Its master's damage and death are passed to the spawn, which die
//! with it if `SpawnedRequireSpawner`. With `AggregateHealth` the spawner is the mob: its health is its spawn's, its veterancy theirs,
//! selecting one selects all, and it dies with the last of them. The spawner answers for its spawn (`SpawnBehaviorInterface`).
//!
//! # Upstream facts kept
//!
//! - `update()`'s first run fills the replacement times: with an `InitialBurst`, every entry is `runtimeProduced` (0 or 1), not the
//!   birth frame it computes; without, the entry's index. The frame is compared unsigned (`UnsignedInt > Int`), so a negative time
//!   never comes due.
//! - `reclaimOrphanSpawn` skips every template name that differs from the previous one, which starts empty: it reclaims nothing unless a
//!   name is empty.
//! - The xfer is the version (2), **`BehaviorModule::xfer`** (not `UpdateModule`'s: the scheduling is not saved), the burst flag, the
//!   spawn template's name, the counters, the replacement times and the spawn ids, the flags and the counts; the template name iterator is
//!   not saved and survives a load.
//! - `m_spawnCount` starts at `NONE_SPAWNED_YET` (`0xffffffff` in an `Int`: -1).
//! - `computeAggregateStates` averages the spawn's positions over their count with no check (a `0` count divides by zero); the weapon
//!   bonus it reads is never used.
//! - Undefined upstream, and so here: an empty `SpawnTemplateName` list (`*begin()`, `Undefined::EndIteratorDereferenced`); a new spawn
//!   with no controlling player (`onUnitCreated`, `Undefined::NullControllingPlayer`); an aggregate spawner or spawn with no body or no
//!   drawable (`Undefined::NullBodyModule`, `Undefined::NullDrawable`); a spawn with no body killed with its spawner (`Object::kill`,
//!   `Undefined::NullBodyModule`); an aggregate health out of an `Int`'s range
//!   (`Undefined::FloatToIntOutOfRange`). An unknown spawn template is `ThingFactory::newObject(NULL)`'s `throw ERROR_BAD_ARG`.
//!
//! The player's `onUnitCreated` (the player's object operations are not on `World` yet) and the spawn's `getAbleToAttackSpecificObject`
//! and `getAbleToUseWeaponAgainstTarget` (the weapon layer's) are cut points.
//!
//! `p1w13-sonnet`, batch R73, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_MASKED, OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageTypeFlags, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieMuxData, DIE_MUX_DATA_FIELD_PARSE};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_NONE_AVAILABLE, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_host::CanAttackResult;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `SPAWN_UPDATE_RATE`: `LOGICFRAMES_PER_SECOND/2` ("a low priority module that only needs to be called every this many frames").
pub const SPAWN_UPDATE_RATE: i32 = crate::common::game_common::LOGICFRAMES_PER_SECOND / 2;
/// `NONE_SPAWNED_YET` (`0xffffffff` in an `Int`).
pub const NONE_SPAWNED_YET: i32 = -1;
/// `SPAWN_DELAY_MIN_FRAMES` ("about as rapidly as you'd expect people to successively exit through the same door").
const SPAWN_DELAY_MIN_FRAMES: u32 = 16;
/// `CMD_FROM_AI`.
const CMD_FROM_AI: i32 = 2;

/// `class SpawnBehaviorModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnBehaviorModuleData {
    pub behavior: BehaviorModuleData,
    /// `m_spawnNumberData`, `m_spawnStartNumberData` (never parsed), `m_spawnReplaceDelayData`, `m_initialBurst`.
    pub spawn_number: i32,
    pub spawn_start_number: i32,
    pub spawn_replace_delay: i32,
    pub initial_burst: i32,
    /// `m_isOneShotData`, `m_canReclaimOrphans`, `m_aggregateHealth`, `m_exitByBudding`, `m_spawnedRequireSpawner`, `m_slavesHaveFreeWill`.
    pub is_one_shot: bool,
    pub can_reclaim_orphans: bool,
    pub aggregate_health: bool,
    pub exit_by_budding: bool,
    pub spawned_require_spawner: bool,
    pub slaves_have_free_will: bool,
    /// `m_damageTypesToPropagateToSlaves` (parsed here; `HiveStructureBody` reads its own).
    pub damage_types_to_propagate_to_slaves: DamageTypeFlags,
    /// `m_spawnTemplateNameData`.
    pub spawn_template_names: Vec<Vec<u8>>,
    /// `m_dieMuxData`.
    pub die_mux_data: DieMuxData,
}

impl Default for SpawnBehaviorModuleData {
    /// `SpawnBehaviorModuleData()`.
    fn default() -> Self {
        SpawnBehaviorModuleData {
            behavior: BehaviorModuleData::new(),
            spawn_number: 0,
            spawn_start_number: 0,
            spawn_replace_delay: 0,
            initial_burst: 0,
            is_one_shot: false,
            can_reclaim_orphans: false,
            aggregate_health: false,
            exit_by_budding: false,
            spawned_require_spawner: false,
            slaves_have_free_will: false,
            damage_types_to_propagate_to_slaves: DamageTypeFlags::default(),
            spawn_template_names: Vec::new(),
            die_mux_data: DieMuxData::new(),
        }
    }
}

static FIELD_PARSE: [FieldParse<SpawnBehaviorModuleData>; 12] = [
    FieldParse::new("SpawnNumber", |i, d, _| { d.spawn_number = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SpawnReplaceDelay", |i, d, _| { d.spawn_replace_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("OneShot", |i, d, _| { d.is_one_shot = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("CanReclaimOrphans", |i, d, _| { d.can_reclaim_orphans = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AggregateHealth", |i, d, _| { d.aggregate_health = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitByBudding", |i, d, _| { d.exit_by_budding = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SpawnTemplateName", |i, d, _| i.parse_ascii_string_vector_append(&mut d.spawn_template_names), UserData::None),
    FieldParse::new("SpawnedRequireSpawner", |i, d, _| { d.spawned_require_spawner = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "PropagateDamageTypesToSlavesWhenExisting",
        |i, d, _| { d.damage_types_to_propagate_to_slaves = i.parse_damage_type_flags()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::new("InitialBurst", |i, d, _| { d.initial_burst = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SlavesHaveFreeWill", |i, d, _| { d.slaves_have_free_will = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `DieMuxData::getFieldParse()` at `offsetof(SpawnBehaviorModuleData, m_dieMuxData)`.
static DIE_MUX_FIELD_PARSE: [FieldParse<SpawnBehaviorModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_mux(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_mux(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_mux(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_mux(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// Lifts entry `k` of the die mux table onto `SpawnBehaviorModuleData::die_mux_data`.
fn lift_die_mux(k: usize, i: &mut Ini<'_>, d: &mut SpawnBehaviorModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MUX_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die_mux_data, u),
        None => Ok(()),
    }
}

impl SpawnBehaviorModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, this class's, then the die mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpawnBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        p.add(&DIE_MUX_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpawnBehaviorModuleData> {
        let mut data = SpawnBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpawnBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `SpawnBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpawnBehavior(Arc::new(SpawnBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class SpawnBehavior : public UpdateModule, public SpawnBehaviorInterface, public DieModuleInterface, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct SpawnBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<SpawnBehaviorModuleData>,
    /// `m_spawnTemplate`, by name (`None`: `NULL`); found in the factory when the world first holds the module.
    spawn_template: Option<Vec<u8>>,
    /// `m_oneShotCountdown`, `m_framesToWait`, `m_firstBatchCount`.
    one_shot_countdown: i32,
    frames_to_wait: i32,
    first_batch_count: i32,
    /// `m_replacementTimes`, `m_spawnIDs`.
    replacement_times: Vec<i32>,
    spawn_ids: Vec<ObjectId>,
    /// `m_active`, `m_aggregateHealth`, `m_initialBurstTimesInited`.
    active: bool,
    aggregate_health: bool,
    initial_burst_times_inited: bool,
    /// `m_spawnCount`, `m_selfTaskingSpawnCount`, `m_initialBurstCountdown`.
    spawn_count: i32,
    self_tasking_spawn_count: u32,
    initial_burst_countdown: u32,
    /// `m_templateNameIterator`, as an index into the data's names.
    template_name_iterator: usize,
}

impl PartialEq for SpawnBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.spawn_template == other.spawn_template
            && self.state_words() == other.state_words()
    }
}

/// The best of the slaves' answers, `getCanAnySlaves...`'s reckoning: the first possible one (the rest not asked); else an invalid shot if
/// any; else not possible.
fn reckon(results: impl IntoIterator<Item = i32>) -> i32 {
    let mut invalid_shot = false;
    for result in results {
        match result {
            r if r == CanAttackResult::Possible as i32 || r == CanAttackResult::PossibleAfterMoving as i32 => return r,
            r if r == CanAttackResult::InvalidShot as i32 => invalid_shot = true,
            // ATTACKRESULT_NOT_POSSIBLE, and the DEBUG_CRASH of an unhandled result ("Treating as not possible...")
            _ => {}
        }
    }
    // "Prioritize the reasonings!"
    if invalid_shot {
        CanAttackResult::InvalidShot as i32
    } else {
        CanAttackResult::NotPossible as i32
    }
}

impl SpawnBehavior {
    /// `getInterfaceMask()`: update, die and damage.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world (`INI_INVALID_DATA` for null data). The spawn template is found when the
    /// world first holds the module ([`SpawnBehavior::world_construct`]).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SpawnBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(UpdateModuleData { behavior: data.behavior.clone() });
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(SpawnBehavior {
            update,
            spawn_template: None,
            one_shot_countdown: if data.is_one_shot { data.spawn_number } else { -1 },
            frames_to_wait: 0,
            first_batch_count: 0,
            replacement_times: Vec::new(),
            spawn_ids: Vec::new(),
            active: true,
            aggregate_health: data.aggregate_health,
            initial_burst_times_inited: false,
            spawn_count: NONE_SPAWNED_YET,
            self_tasking_spawn_count: 0,
            initial_burst_countdown: data.initial_burst as u32,
            template_name_iterator: 0,
            data,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<SpawnBehaviorModuleData> {
        &self.data
    }

    /// The spawn template's name (empty: `NULL`), then the counters, the flags, the counts, the iterator, the replacement times and the ids.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![
            self.one_shot_countdown as u32,
            self.frames_to_wait as u32,
            self.first_batch_count as u32,
            u32::from(self.active),
            u32::from(self.aggregate_health),
            u32::from(self.initial_burst_times_inited),
            self.spawn_count as u32,
            self.self_tasking_spawn_count,
            self.initial_burst_countdown,
            self.template_name_iterator as u32,
            self.replacement_times.len() as u32,
        ];
        v.extend(self.replacement_times.iter().map(|&t| t as u32));
        v.push(self.spawn_ids.len() as u32);
        v.extend(self.spawn_ids.iter().map(|&id| id as u32));
        v
    }

    /// `m_spawnTemplate`'s name (`None`: `NULL`).
    pub fn spawn_template(&self) -> Option<&[u8]> {
        self.spawn_template.as_deref()
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: `m_spawnTemplate = TheThingFactory->findTemplate(*m_templateNameIterator)` (an empty list:
    /// undefined, recorded).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        match self.data.spawn_template_names.first() {
            Some(name) => self.spawn_template = Self::find_template(world, name),
            None => world.cuts.module_errors.push((me, IniError::undefined(Undefined::EndIteratorDereferenced))),
        }
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the scheduling is not saved (`BehaviorModule::xfer`).
    pub fn xfer_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.xfer(xfer)
    }

    /// `crc` with the scheduling half the slot holds: `BehaviorModule::crc`.
    pub fn crc_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, _update: &mut UpdateModule) {
        self.load_post_process();
    }

    /// `TheThingFactory->findTemplate(name)`'s name (`None`: `NULL`).
    fn find_template(world: &World, name: &[u8]) -> Option<Vec<u8>> {
        world.statics.thing_factory.find_template(name).map(|_| name.to_vec())
    }

    /// The live spawn, in list order.
    fn live_spawn(&self, world: &World) -> Vec<ObjectId> {
        self.spawn_ids.iter().copied().filter(|&id| world.objects.find(id).is_some()).collect()
    }

    /// `(*update)->getSlavedUpdateInterface()` of the first of the object's modules that has one.
    fn slaved_interface(world: &mut World, id: ObjectId) -> Option<ModuleIndex> {
        world.all_modules(id).into_iter().find(|&idx| world.module_call(id, idx, ModuleCall::HasSlavedUpdateInterface).as_bool())
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// `onDelete()`: the spawn still alive destroyed, if they require their spawner.
    fn on_delete(&mut self, world: &mut World) {
        if !self.data.spawned_require_spawner {
            return;
        }
        for id in self.spawn_ids.clone() {
            if world.objects.find(id).is_some_and(|o| !o.is_effectively_dead()) && !crate::logic::modules::plant(2440) {
                world.body_destroy_object(id);
            }
        }
    }

    /// `onDie(damageInfo)`: if the die mux applies, each spawn's slaved update hears of it and forgets its producer; the spawn are killed
    /// if they require their spawner.
    fn on_die(&mut self, world: &mut World, us: ObjectId, info: &DamageInfo) -> Result<(), IniError> {
        let applicable = {
            let Some(o) = world.objects.find(us) else { return Ok(()) };
            self.data.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status)
        };
        if !applicable {
            return Ok(());
        }
        for id in self.live_spawn(world) {
            // "Go through all my spawns and see if they have a SlavedUpdate I can tell I was killed to"
            if let Some(sdu) = Self::slaved_interface(world, id) {
                world.module_call(id, sdu, ModuleCall::SlavedOnSlaverDie);
            }
            // "our spawner has died, we must invalidate the ID now in the spawned object"
            if !crate::logic::modules::plant(2441) {
                world.set_producer(id, INVALID_ID);
            }
        }
        // "kill anything that we have spawned if our module data directs us to do so"
        if self.data.spawned_require_spawner {
            for id in self.spawn_ids.clone() {
                if world.objects.find(id).is_some_and(|o| !o.is_effectively_dead()) {
                    // (`Object::kill` reads the body unchecked)
                    if world.get_body_module(id).is_none() {
                        return Err(IniError::undefined(Undefined::NullBodyModule));
                    }
                    world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                }
            }
        }
        Ok(())
    }

    /// `onDamage(info)`: each spawn's slaved update hears of it.
    fn on_damage(&mut self, world: &mut World, info: &DamageInfo) {
        for id in self.live_spawn(world) {
            if let Some(sdu) = Self::slaved_interface(world, id) {
                world.module_call(id, sdu, ModuleCall::SlavedOnSlaverDamage(info.clone()));
            }
        }
    }

    /// `shouldTryToSpawn()`.
    fn should_try_to_spawn(&mut self, world: &World, us: ObjectId) -> bool {
        // "Not if we are turned off"
        if !self.active {
            return false;
        }
        let Some(o) = world.objects.find(us) else { return false };
        if o.status.test(OBJECT_STATUS_RECONSTRUCTING) && self.data.is_one_shot {
            // "If we are a Hole rebuild, not only should we not, but we should never ask again."
            self.active = false;
            return false;
        }
        // "Not if we are under construction or being sold"
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.test_status(OBJECT_STATUS_SOLD) {
            return false;
        }
        // "Not if we are civilian controlled"
        !world.is_neutral_controlled(us)
    }

    /// `reclaimOrphanSpawn()`: the orphan nearest the spawner among its player's objects of a spawn template. ("the list may have
    /// redundancy, this will skip some of it": every name that differs from the previous one, which starts empty, is skipped.)
    fn reclaim_orphan_spawn(&self, world: &mut World, us: ObjectId) -> Result<Option<ObjectId>, IniError> {
        let mut closest: Option<ObjectId> = None;
        let mut prev_name: Vec<u8> = Vec::new();
        for name in self.data.spawn_template_names.clone() {
            if prev_name != name && !crate::logic::modules::plant(2442) {
                continue;
            }
            let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            let template = world.statics.thing_factory.find_template(&name).map(|t| t.data.get_template_id());
            // (each name starts the search afresh: m_closest NULL, m_closestDistSq BIG_DISTANCE)
            closest = None;
            let mut closest_dist_sq = 99999999.9f32;
            for obj in world.player_iterate_objects(player) {
                let Some(o) = world.objects.find(obj) else { continue };
                // "if template doesn't match do nothing" (isEquivalentTo: the same template; NULL matches nothing)
                if template != Some(o.template().get_template_id()) {
                    continue;
                }
                // "this object must be orphaned"
                if o.producer_id != INVALID_ID {
                    continue;
                }
                let dist_sq = world.partition_get_distance_squared_center_2d(us, obj);
                if dist_sq < closest_dist_sq {
                    closest = Some(obj);
                    closest_dist_sq = dist_sq;
                }
            }
            prev_name = name;
        }
        Ok(closest)
    }

    /// `createSpawn()`: whether a spawn was made (or reclaimed).
    fn create_spawn(&mut self, world: &mut World, us: ObjectId) -> Result<bool, IniError> {
        let d = self.data.clone();
        let Some(exit) = world.get_object_exit_interface(us) else {
            // "Something cannot have SpawnBehavior without an exit interface"
            return Ok(false);
        };
        let exit_door = match world.module_call(us, exit, ModuleCall::ReserveDoorForExit { specific: INVALID_ID }) {
            ModuleReply::Int(door) => door,
            _ => DOOR_NONE_AVAILABLE,
        };
        if exit_door == DOOR_NONE_AVAILABLE {
            return Ok(false);
        }
        // "try to reclaim orphaned objects if possible"
        let mut new_spawn = None;
        let mut reclaimed_orphan = false;
        if d.can_reclaim_orphans && !d.is_one_shot {
            new_spawn = self.reclaim_orphan_spawn(world, us)?;
            reclaimed_orphan = new_spawn.is_some();
        }
        let new_spawn = match new_spawn {
            Some(s) => s,
            None => {
                let name = d.spawn_template_names.get(self.template_name_iterator).ok_or_else(|| IniError::undefined(Undefined::EndIteratorDereferenced))?;
                self.spawn_template = Self::find_template(world, name);
                let template = world.statics.thing_factory.find_template(name).map(|t| t.data.get_template_id());
                let template = template.ok_or_else(|| IniError::new(IniErrorKind::BadArg))?;
                let team = world.get_team(us);
                // (`newObject` throws on failure; the world's errors are not upstream's)
                let s = world.thing_factory_new_object(template, team, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
                // "Count this unit towards our score."
                let player = world.get_controlling_player(s).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                world.player_on_unit_created(player, us, s);
                // "Each time m_spawn template is used, it will increment m_templateNameIterator, ... looping back to the beginning"
                self.template_name_iterator += 1;
                if self.template_name_iterator == d.spawn_template_names.len() || crate::logic::modules::plant(2443) {
                    self.template_name_iterator = 0;
                }
                s
            }
        };
        world.set_producer(new_spawn, us);
        // "If they have a SlavedUpdate, then I have to tell them who their daddy is from now on."
        if let Some(sdu) = Self::slaved_interface(world, new_spawn) {
            world.module_call(new_spawn, sdu, ModuleCall::SlavedOnEnslave { slaver: us });
        }
        self.spawn_ids.push(new_spawn);
        if !reclaimed_orphan {
            if d.exit_by_budding {
                let mut barracks_exit_success = false;
                if self.initial_burst_countdown > 0 {
                    let producer = world.objects.find(us).map_or(INVALID_ID, |o| o.producer_id);
                    let barracks = world.objects.find(producer).filter(|o| o.is_kind_of(KINDOF_STRUCTURE)).map(|o| o.id);
                    if let Some(barracks) = barracks {
                        if let Some(barracks_exit) = world.get_object_exit_interface(barracks) {
                            let door = match world.module_call(barracks, barracks_exit, ModuleCall::ReserveDoorForExit { specific: INVALID_ID }) {
                                ModuleReply::Int(door) => door,
                                _ => DOOR_NONE_AVAILABLE,
                            };
                            world.module_call(barracks, barracks_exit, ModuleCall::ExitObjectViaDoor { obj: new_spawn, door });
                            // "let parents producer exit him, but he thinks it was me"
                            world.set_producer(new_spawn, us);
                            self.initial_burst_countdown -= 1;
                            barracks_exit_success = true;
                        }
                    }
                }
                if !barracks_exit_success {
                    // "find the closest spawn to the nexus..."
                    let mut bud_host = INVALID_ID;
                    let mut closest = 999999.9f32;
                    for cur in self.live_spawn(world) {
                        if cur == new_spawn {
                            continue;
                        }
                        let tape_measure = world.partition_get_distance_squared_center_2d(cur, us);
                        if tape_measure < closest {
                            closest = tape_measure;
                            bud_host = cur;
                        }
                    }
                    // "also handles the NULL pointer okay"
                    world.module_call(us, exit, ModuleCall::ExitObjectByBudding { obj: new_spawn, bud_host });
                }
            } else {
                world.module_call(us, exit, ModuleCall::ExitObjectViaDoor { obj: new_spawn, door: exit_door });
            }
        } else {
            world.module_call(us, exit, ModuleCall::UnreserveDoorForExit(exit_door));
        }
        if d.is_one_shot {
            self.one_shot_countdown = self.one_shot_countdown.wrapping_sub(1);
        }
        if self.spawn_count == NONE_SPAWNED_YET && !crate::logic::modules::plant(2444) {
            self.spawn_count = 1;
        } else {
            self.spawn_count = self.spawn_count.wrapping_add(1);
        }
        Ok(true)
    }

    /// `onSpawnDeath(deadSpawn, damageInfo)`: its replacement scheduled; the last spawn of an aggregate takes the spawner with it.
    fn on_spawn_death(&mut self, world: &mut World, us: ObjectId, dead: ObjectId, info: &DamageInfo) {
        // "If the iterator is at the end, we didn't find deadSpawn, so bail out."
        let Some(i) = self.spawn_ids.iter().position(|&id| id == dead) else { return };
        // "When one dies, you push (now + delay) as the time a new one should be made"
        let delay = if crate::logic::modules::plant(2445) { 0 } else { self.data.spawn_replace_delay };
        let replacement_time = (delay as u32).wrapping_add(world.frame()) as i32;
        self.replacement_times.push(replacement_time);
        self.spawn_ids.remove(i);
        self.spawn_count = self.spawn_count.wrapping_sub(1);
        // "I'm dead without my spawn"
        if self.spawn_count == 0 && self.aggregate_health {
            if world.objects.find(info.input.source_id).is_some() {
                world.body_score_the_kill(info.input.source_id, us);
            }
            world.body_destroy_object(us);
        }
    }

    /// `maySpawnSelfTaskAI(maxSelfTaskersRatio)`.
    fn may_spawn_self_task_ai(&self, world: &mut World, us: ObjectId, max_self_taskers_ratio: f32) -> bool {
        if self.spawn_count == 0 {
            return false;
        }
        if max_self_taskers_ratio == 0.0 {
            return false;
        }
        // "if my last attack command was from player or script, I need to forbid my spawn from disobeying that command"
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return false };
        let ModuleReply::Int(last) = world.module_call(us, ai, ModuleCall::AiGetLastCommandSource) else { return false };
        if last != CMD_FROM_AI {
            return false;
        }
        let cur_self_taskers_ratio = self.self_tasking_spawn_count as f32 / self.spawn_count as f32;
        cur_self_taskers_ratio < max_self_taskers_ratio
    }

    /// `getClosestSlave(pos)`: the spawn nearest `pos` in 2D (the first of equals), `INVALID_ID` for none.
    fn get_closest_slave(&self, world: &mut World, pos: Coord3D) -> ObjectId {
        let mut closest = INVALID_ID;
        let mut closest_distance = 0.0f32;
        for id in self.live_spawn(world) {
            let p = Self::position(world, id);
            let (dx, dy) = (pos.x - p.x, pos.y - p.y);
            let distance = dx * dx + dy * dy;
            if closest == INVALID_ID || closest_distance > distance {
                closest = id;
                closest_distance = distance;
            }
        }
        closest
    }

    /// Each live spawn's AI told `call` (`orderSlavesTo...`).
    fn order_ais(&self, world: &mut World, call: ModuleCall) {
        for id in self.live_spawn(world) {
            if let Some(ai) = world.objects.find(id).and_then(|o| o.ai) {
                world.module_call(id, ai, call.clone());
            }
        }
    }

    /// `computeAggregateStates()`: the mob's veterancy, selection, health box, health and mask.
    fn compute_aggregate_states(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        // "sanity"
        if !self.aggregate_health {
            return Ok(());
        }
        let spawn_count_max = self.data.spawn_number;
        let mut spawn_count: i32 = 0;
        let mut avg_spawn_pos = Coord3D::default();
        let mut acr_health = 0.0f32;
        let mut avg_health_max = 0.0f32;
        let vet_level = world.objects.find(us).map_or(crate::common::game_common::VeterancyLevel::Regular, |o| o.get_veterancy_level());
        let mut somebody_is_selected = false;
        let mut somebody_is_not_selected = false;
        self.self_tasking_spawn_count = 0;
        for id in self.live_spawn(world) {
            if let Some(sdu) = Self::slaved_interface(world, id) {
                self.self_tasking_spawn_count += u32::from(world.module_call(id, sdu, ModuleCall::SlavedIsSelfTasking).as_bool());
            }
            // VETERANCY LEVEL: "whoever has more, set the other guy"
            let spawn_vet_level = world.objects.find(id).map_or(crate::common::game_common::VeterancyLevel::Regular, |o| o.get_veterancy_level());
            if spawn_vet_level as i32 > vet_level as i32 {
                world.body_experience_set_veterancy_level(us, spawn_vet_level);
            } else if (spawn_vet_level as i32) < vet_level as i32 && !crate::logic::modules::plant(2446) {
                world.body_experience_set_veterancy_level(id, vet_level);
            }
            avg_spawn_pos.add(&Self::position(world, id));
            let body = world.get_body_module(id).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
            acr_health += world.module_call(id, body, ModuleCall::GetHealth).as_real(0.0);
            avg_health_max += world.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0);
            if !world.has_drawable(id) {
                return Err(IniError::undefined(Undefined::NullDrawable));
            }
            if world.drawable_is_selected(id) {
                somebody_is_selected = true;
            } else {
                somebody_is_not_selected = true;
            }
            spawn_count += 1;
        }
        // SELECTION STATE: "IF ANY ONE OF MY SPAWN ARE SELECTED RIGHT NOW, I JUST SET THEM ALL SELECTED"
        if somebody_is_selected {
            if !world.has_drawable(us) {
                return Err(IniError::undefined(Undefined::NullDrawable));
            }
            let me_selected = world.drawable_is_selected(us);
            if !me_selected || somebody_is_not_selected {
                // "not creating new team so pass false"
                world.emit(ObjectClientEvent::MessageAppendCreateSelectedGroup);
                world.emit(ObjectClientEvent::MessageAppendBooleanArgument(false));
                if somebody_is_not_selected {
                    for id in self.live_spawn(world) {
                        if !world.drawable_is_selected(id) {
                            world.ui_select_drawable(id);
                            world.emit(ObjectClientEvent::UiSetDisplayedMaxWarning { on: false });
                            world.emit(ObjectClientEvent::MessageAppendBooleanArgument(false));
                            world.emit(ObjectClientEvent::MessageAppendObjectIdArgument(id));
                        }
                    }
                }
                // "if somebody is selected then I sure need to be!"
                if !world.drawable_is_selected(us) && !crate::logic::modules::plant(2447) {
                    world.ui_select_drawable(us);
                    world.emit(ObjectClientEvent::UiSetDisplayedMaxWarning { on: false });
                    world.emit(ObjectClientEvent::MessageAppendBooleanArgument(false));
                    world.emit(ObjectClientEvent::MessageAppendObjectIdArgument(us));
                }
            }
        }
        // HEALTH BOX POSITION: "pick a centered, average spot to draw the health box"
        avg_spawn_pos.scale(1.0 / spawn_count as f32);
        avg_spawn_pos.sub(&Self::position(world, us));
        if let Some(o) = world.objects.find_mut(us) {
            o.health_box_offset = avg_spawn_pos;
        }
        // HEALTH STATE: "make my health an aggregate of all my spawns' healths"
        let body = world.get_body_module(us).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        let percent = if spawn_count != 0 {
            avg_health_max /= spawn_count as f32;
            let perfect_total_health = avg_health_max * spawn_count_max as f32;
            let actual_health = acr_health / perfect_total_health;
            let p = 100.0f32 * actual_health;
            // (`setInitialHealth(Int)`: the `Real` truncated)
            if !(p > -2147483904.0 && p < 2147483648.0) {
                return Err(IniError::undefined(Undefined::FloatToIntOutOfRange));
            }
            p as i32
        } else {
            // "I been sick <"
            0
        };
        world.module_call(us, body, ModuleCall::SetInitialHealth(percent));
        // HOUSEKEEPING: "make sure no enemies are shooting at the nexus, since it doesn't 'exist'"
        let mut mask = ObjectStatusMaskType::default();
        mask.set(OBJECT_STATUS_MASKED, 1);
        world.body_set_status(us, mask, true);
        Ok(())
    }

    /// `update()`'s body; an error is undefined upstream (or a throw).
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        // EVERY FRAME
        if self.aggregate_health {
            self.compute_aggregate_states(world, us)?;
        }
        let d = self.data.clone();
        if !self.initial_burst_times_inited {
            self.initial_burst_times_inited = true;
            // "this was produced by a production module, rather than a script or worldbuilder"
            let runtime_produced = world.objects.find(us).is_some_and(|o| o.producer_id != INVALID_ID);
            let now = world.frame();
            let mut burst_init_count = self.initial_burst_countdown as i32;
            for list_index in 0..d.spawn_number {
                if d.initial_burst > 0 {
                    let mut _birth_frame = now;
                    if runtime_produced && burst_init_count > 0 {
                        burst_init_count -= 1;
                        _birth_frame = _birth_frame.wrapping_add((list_index as u32).wrapping_mul(SPAWN_DELAY_MIN_FRAMES));
                    }
                    // "Set all spawns to be created in rapid succession" (the flag, not the birth frame)
                    self.replacement_times.push(i32::from(runtime_produced));
                } else {
                    // "Set all spawns to be created as soon as possible"
                    self.replacement_times.push(list_index);
                }
            }
        }
        // SPARSELY SOLVED
        self.frames_to_wait = self.frames_to_wait.wrapping_sub(1);
        if self.frames_to_wait > 0 {
            return Ok(UPDATE_SLEEP_NONE);
        }
        self.frames_to_wait = SPAWN_UPDATE_RATE;
        // "Go through the list and make a spawn for each number that is less than now's frame"
        if self.should_try_to_spawn(world, us) {
            let mut i = 0;
            while i < self.replacement_times.len() {
                let replacement_time = self.replacement_times[i];
                let current_time = world.frame();
                // (`UnsignedInt > Int`: compared unsigned)
                if current_time > replacement_time as u32 {
                    // "If you create one, you pop the number off the list"
                    if self.create_spawn(world, us)? {
                        self.replacement_times.remove(i);
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            if d.is_one_shot && self.one_shot_countdown <= 0 {
                // "I only trigger one batch.  ie on Creation."
                self.active = false;
            }
        }
        Ok(UPDATE_SLEEP_NONE)
    }

    /// `r`'s value, or its error recorded as the module's (undefined upstream, or a throw).
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

impl UpdateBehavior for SpawnBehavior {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.do_update(world, me.object);
        Self::record(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// (Not used: the xfer is `BehaviorModule`'s, [`Snapshot::xfer`].)
    fn xfer_fields(&mut self, _xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpawnBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpawnBehavior(m) => Some(m),
            _ => None,
        }
    }
    /// The die and damage interfaces, `onDelete` and the `SpawnBehaviorInterface`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::OnDelete => self.on_delete(world),
            ModuleCall::OnDie(info) => {
                let r = self.on_die(world, us, &info);
                Self::record(world, me, r);
            }
            ModuleCall::OnDamage(info) => self.on_damage(world, &info),
            ModuleCall::OnHealing(_) | ModuleCall::OnBodyDamageStateChange { .. } => {}
            ModuleCall::HasSpawnBehaviorInterface => return ModuleReply::Bool(true),
            ModuleCall::SpawnMaySpawnSelfTaskAi(ratio) => return ModuleReply::Bool(self.may_spawn_self_task_ai(world, us, ratio)),
            ModuleCall::OnSpawnDeath { dead, info } => self.on_spawn_death(world, us, dead, &info),
            ModuleCall::GetClosestSlave(pos) => return ModuleReply::Id(self.get_closest_slave(world, pos)),
            ModuleCall::SpawnOrderSlavesToAttackTarget { target, max_shots, command_source } => {
                self.order_ais(world, ModuleCall::AiForceAttackObject { victim: target, max_shots, command_source })
            }
            ModuleCall::SpawnOrderSlavesToAttackPosition { pos, max_shots, command_source } => {
                self.order_ais(world, ModuleCall::AiAttackPosition { pos: Some(pos), max_shots, command_source })
            }
            ModuleCall::SpawnOrderSlavesToGoIdle { command_source } => self.order_ais(world, ModuleCall::AiIdleFrom { command_source }),
            ModuleCall::OrderSlavesDisabledUntil { disabled_type, frame } => {
                for id in self.live_spawn(world) {
                    if let Some(ai) = world.objects.find(id).and_then(|o| o.ai) {
                        world.module_call(id, ai, ModuleCall::AiIdle);
                    }
                    world.body_set_disabled_until(id, disabled_type, frame);
                }
            }
            ModuleCall::OrderSlavesToClearDisabled { disabled_type } => {
                for id in self.live_spawn(world) {
                    world.body_clear_disabled(id, disabled_type);
                }
            }
            ModuleCall::SpawnGetCanAnySlavesAttackSpecificTarget { attack_type, target, command_source } => {
                let spawn = self.live_spawn(world);
                let results = spawn.into_iter().map(|id| world.object_get_able_to_attack_specific_object(id, attack_type, target, command_source));
                return ModuleReply::Int(reckon(results));
            }
            ModuleCall::SpawnCanAnySlavesUseWeaponAgainstTarget { attack_type, victim, pos, command_source } => {
                let spawn = self.live_spawn(world);
                let results = spawn.into_iter().map(|id| world.object_get_able_to_use_weapon_against_target(id, attack_type, victim, pos, command_source));
                return ModuleReply::Int(reckon(results));
            }
            ModuleCall::SpawnGiveSlavesStealthUpgrade(grant) => {
                let mut mask = ObjectStatusMaskType::default();
                mask.set(OBJECT_STATUS_CAN_STEALTH, 1);
                for id in self.live_spawn(world) {
                    world.body_set_status(id, mask, grant);
                }
            }
            ModuleCall::CanAnySlavesAttack => {
                let any = self.live_spawn(world).into_iter().any(|id| world.is_able_to_attack(id));
                return ModuleReply::Bool(any);
            }
            ModuleCall::SpawnAreAllSlavesStealthed => {
                // "if you have NO spawns, then they are considered stealthed ;)"
                for id in self.live_spawn(world) {
                    let allowed = match world.objects.find(id).and_then(|o| o.stealth) {
                        Some(s) => world.module_call(id, s, ModuleCall::StealthAllowedToStealth { owner: id }).as_bool(),
                        None => false,
                    };
                    if !allowed {
                        return ModuleReply::Bool(false);
                    }
                }
                return ModuleReply::Bool(true);
            }
            ModuleCall::SpawnRevealSlaves => {
                for id in self.live_spawn(world) {
                    if let Some(s) = world.objects.find(id).and_then(|o| o.stealth) {
                        world.module_call(id, s, ModuleCall::StealthMarkAsDetected);
                    }
                }
            }
            ModuleCall::SpawnDoSlavesHaveFreedom => return ModuleReply::Bool(self.data.slaves_have_free_will),
            ModuleCall::SpawnStopSpawning => self.active = false,
            ModuleCall::SpawnStartSpawning => self.active = true,
            _ => return ModuleReply::Unhandled,
        }
        ModuleReply::Done
    }
}

impl Snapshot for SpawnBehavior {
    /// `crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.behavior().clone().crc(xfer)
    }
    /// `xfer` (version 2): the version, `BehaviorModule::xfer`, the burst flag (from version 2), the spawn template's name, the counters,
    /// the replacement times, the ids, the flags and the counts.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 2;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.behavior().clone().xfer(xfer)?;
        if version >= 2 && !crate::logic::modules::plant(2448) {
            xfer.xfer_bool(&mut self.initial_burst_times_inited)?;
        }
        let mut name = self.spawn_template.clone().unwrap_or_default();
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            // (the factory is the world's, which the load does not see: the name is kept. A saved name is one the factory held, so the
            // `SC_INVALID_DATA` throw of an unknown one is not reached by a save of this module.)
            self.spawn_template = (!name.is_empty()).then_some(name);
        }
        xfer.xfer_int(&mut self.one_shot_countdown)?;
        xfer.xfer_int(&mut self.frames_to_wait)?;
        xfer.xfer_int(&mut self.first_batch_count)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.replacement_times.clear();
        }
        xfer.xfer_stl_int_list(&mut self.replacement_times)?;
        xfer.xfer_stl_object_id_list(&mut self.spawn_ids)?;
        xfer.xfer_bool(&mut self.active)?;
        xfer.xfer_bool(&mut self.aggregate_health)?;
        xfer.xfer_int(&mut self.spawn_count)?;
        xfer.xfer_unsigned_int(&mut self.self_tasking_spawn_count)
    }
    /// `loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.update.behavior().clone().load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = SpawnBehaviorModuleData { is_one_shot: true, spawn_number: 3, initial_burst: 2, ..Default::default() };
        let m = SpawnBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!((m.one_shot_countdown, m.spawn_count, m.initial_burst_countdown), (3, NONE_SPAWNED_YET, 2));
        assert!(m.active && !m.initial_burst_times_inited);
        assert_eq!(reckon([0, 1, 0]), CanAttackResult::InvalidShot as i32);
        assert_eq!(reckon([1, 2, 3]), CanAttackResult::PossibleAfterMoving as i32);
        assert_eq!(reckon([]), CanAttackResult::NotPossible as i32);
    }
}
