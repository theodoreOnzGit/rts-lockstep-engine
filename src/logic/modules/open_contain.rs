// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/OpenContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `OpenContain`: "an open container can actually contain other objects". The base of every container of the game (transports, garrisons, tunnels, caves, the
//! heal contain, the parachute, ...). It is an update module and the contain, collide, die and damage interfaces of its object, and the exit interface that
//! lets contained objects walk out of the door.
//!
//! # Design
//!
//! The derived classes override a good part of the base's virtuals, and the base calls them (`addToContain` asks `isEnclosingContainerFor` and `redeployOccupants`
//! and `onContaining`; `exitObjectViaDoor` calls `removeFromContain`; ...). As for `ActiveBody` that is a trait: [`OpenContainVirtuals`] has one method per
//! virtual, each defaulting to the base class's function (a free function of this module, generic over the derived class), and the derived classes hold the base by
//! value and override what upstream overrides. [`contain_module_call`] answers the [`ModuleCall`]s of `Object` and of the other modules from the trait, so the call
//! reaches the override as the vtable would send it.
//!
//! Every call out to the world goes through [`out`], which lends the module's current state to its slot (the module is checked out while it runs and `Object`'s
//! calls can reach it again: `onContainedBy` asks the container `isEnclosingContainerFor`). `f` must not use the module.
//!
//! The contain list is a `Vec<ObjectId>` in list order (`push_back` is the end). Loops that upstream writes with the iterator advanced before the call (so that
//! the callee may remove the item) walk a snapshot of the list.
//!
//! # Cut points
//!
//! The partition manager, the pathfinder, the terrain layers, the drawable and the art's bones are cut points of the world (`world::contain_cuts`); the
//! Object-level calls run for real unless a test stubs them (`ContainCutTables::stub_object_calls`). `Drawable::getModelConditionFlags()` and
//! `Drawable::isSelected()` are reads back from the client: state of the world (`ClientReads`).
//!
//! # Upstream facts kept
//!
//! - `onDelete` destroys every contained object with `destroyObject`, one by one, from the module's own list; `onDie` runs the damage to the contained, kills
//!   the riders who are not free to exit, and removes everybody.
//! - `killAllContained` empties the list one by one with `onRemoving` and `onRemovedFrom` before each `kill()`, and does not touch the stealth count.
//! - `harmAndForceExitAllContained` restarts from the head of the list after every rider ("Patch 1.03"): a rider that does not come out of the list (a
//!   `removeFromContain` that refuses) would make it loop for ever; the translation stops after as many rounds as the list had members at the start plus one
//!   and records it as upstream-undefined.
//! - `exitObjectViaDoor` and `exitObjectInAHurry` set the door countdown from the data and do not check that the exit object is contained.
//! - `processDamageToContained` loops `while( *it )` on a `std::list` iterator: past the last element it reads the list's end node (undefined); the translation
//!   stops at the end of the list.
//! - `getNaturalRallyPoint` ignores its `offset` argument.
//! - `loadPostProcess` finds the objects of the saved contain list by id; one that is missing is `SC_INVALID_DATA`.
//!
//! `p1w13-sonnet`, 2026-10-22.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::{Coord3D, PI};
use crate::common::game_common::{PlayerMaskType, Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KindOfMaskType, KINDOF_MONEY_HACKER, KINDOF_STEALTH_GARRISON, KINDOF_VEHICLE};
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_COUNT, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING};
use crate::common::module::{ModuleDataKind, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_IS_ATTACKING;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::common::trig::{cos, sin};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_BURNED, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::contain_module::ObjectEnterExitType;
use crate::logic::module::die_module::{DieMuxData, DIE_MUX_DATA_FIELD_PARSE};
use crate::logic::module::update_module::{
    ExitDoorType, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::lend_clone;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `CONTAIN_MAX_UNKNOWN`: "means we don't care, infinite, unassigned, whatever".
pub const CONTAIN_MAX_UNKNOWN: i32 = -1;

/// `MAX_FIRE_POINTS`.
pub const MAX_FIRE_POINTS: usize = 32;

/// `CMD_FROM_AI` (`CommandSourceType`).
pub const CMD_FROM_AI: i32 = 2;

/// `class OpenContainModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenContainModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_dieMuxData`.
    pub die_mux_data: DieMuxData,
    /// `m_containMax`: "how many things we can have inside (-1 = "I don't care")".
    pub contain_max: i32,
    /// `m_enterSound`: "sound to play on entering".
    pub enter_sound: AudioEventName,
    /// `m_exitSound`: "sound to play on exiting".
    pub exit_sound: AudioEventName,
    /// `m_passengersAllowedToFire`: "can the passengers shoot out of us?".
    pub passengers_allowed_to_fire: bool,
    /// `m_passengersInTurret`: "the Firepoint bones are in our turret, not our chassis".
    pub passengers_in_turret: bool,
    /// `m_numberOfExitPaths`: "will alternate through ExitStart/End paths as we exit people".
    pub number_of_exit_paths: i32,
    /// `m_damagePercentageToUnits`.
    pub damage_percentage_to_units: f32,
    /// `m_isBurnedDeathToUnits`: "turn off the hardcoded burn death when killing guys in transport".
    pub is_burned_death_to_units: bool,
    /// `m_doorOpenTime`.
    pub door_open_time: u32,
    /// `m_allowInsideKindOf`: "objects must have at least one of these kind of bits set to be contained by us".
    pub allow_inside_kind_of: KindOfMaskType,
    /// `m_forbidInsideKindOf`: "objects must have NONE of these kind of bits set to be contained by us".
    pub forbid_inside_kind_of: KindOfMaskType,
    /// `m_weaponBonusPassedToPassengers`: "do our passengers get to use our weapon bonuses?".
    pub weapon_bonus_passed_to_passengers: bool,
    /// `m_allowAlliesInside`.
    pub allow_allies_inside: bool,
    /// `m_allowEnemiesInside`.
    pub allow_enemies_inside: bool,
    /// `m_allowNeutralInside`.
    pub allow_neutral_inside: bool,
}

impl Default for OpenContainModuleData {
    /// `OpenContainModuleData()`: "everything is allowed", "nothing is forbidden".
    fn default() -> Self {
        let mut allow = KindOfMaskType::new();
        allow.flip();
        OpenContainModuleData {
            update: UpdateModuleData::new(),
            die_mux_data: DieMuxData::new(),
            contain_max: CONTAIN_MAX_UNKNOWN,
            enter_sound: AudioEventName::default(),
            exit_sound: AudioEventName::default(),
            passengers_allowed_to_fire: false,
            passengers_in_turret: false,
            number_of_exit_paths: 1,
            damage_percentage_to_units: 0.0,
            is_burned_death_to_units: true,
            door_open_time: 1,
            allow_inside_kind_of: allow,
            forbid_inside_kind_of: KindOfMaskType::new(),
            weapon_bonus_passed_to_passengers: false,
            allow_allies_inside: true,
            allow_enemies_inside: true,
            allow_neutral_inside: true,
        }
    }
}

/// `OpenContainModuleData::buildFieldParse`'s own table (`dataFieldParse`).
pub(crate) static FIELD_PARSE: [FieldParse<OpenContainModuleData>; 16] = [
    FieldParse::new("ContainMax", |i, d, _| { d.contain_max = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("EnterSound", |i, d, _| d.enter_sound.parse(i), UserData::None),
    FieldParse::new("ExitSound", |i, d, _| d.exit_sound.parse(i), UserData::None),
    FieldParse::new("DamagePercentToUnits", |i, d, _| { d.damage_percentage_to_units = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("BurnedDeathToUnits", |i, d, _| { d.is_burned_death_to_units = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowInsideKindOf", |i, d, _| d.allow_inside_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("ForbidInsideKindOf", |i, d, _| d.forbid_inside_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("PassengersAllowedToFire", |i, d, _| { d.passengers_allowed_to_fire = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("PassengersInTurret", |i, d, _| { d.passengers_in_turret = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("NumberOfExitPaths", |i, d, _| { d.number_of_exit_paths = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorOpenTime", |i, d, _| { d.door_open_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("WeaponBonusPassedToPassengers", |i, d, _| { d.weapon_bonus_passed_to_passengers = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowAlliesInside", |i, d, _| { d.allow_allies_inside = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowEnemiesInside", |i, d, _| { d.allow_enemies_inside = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AllowNeutralInside", |i, d, _| { d.allow_neutral_inside = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `DieMuxData::getFieldParse()` at `offsetof(OpenContainModuleData, m_dieMuxData)`.
pub(crate) static DIE_MUX_FIELD_PARSE: [FieldParse<OpenContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_mux(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_mux(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_mux(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_mux(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// Lifts entry `k` of the die mux table onto `OpenContainModuleData::die_mux_data`.
fn lift_die_mux(k: usize, i: &mut Ini<'_>, d: &mut OpenContainModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MUX_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die_mux_data, u),
        None => Ok(()),
    }
}

impl OpenContainModuleData {
    /// `OpenContainModuleData::buildFieldParse(p)`: the update base, then this class's fields and the die mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, OpenContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        p.add(&DIE_MUX_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<OpenContainModuleData> {
        let mut data = OpenContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, OpenContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `OpenContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::OpenContain(Arc::new(OpenContainModuleData::friend_new_module_data(ini)?)))
}

/// `class OpenContain : public UpdateModule, public ContainModuleInterface, public CollideModuleInterface, public DieModuleInterface, public DamageModuleInterface,
/// public ExitInterface`.
#[derive(Clone, Debug)]
pub struct OpenContain {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getOpenContainModuleData()`.
    data: Arc<OpenContainModuleData>,
    /// `m_containList`: "the list of contained objects".
    contain_list: Vec<ObjectId>,
    /// `m_containListSize`: "size of contained list".
    contain_list_size: u32,
    /// `m_objectEnterExitInfo`.
    object_enter_exit_info: BTreeMap<ObjectId, ObjectEnterExitType>,
    /// `m_stealthUnitsContained`: "number of stealth units that can't be seen by enemy players".
    stealth_units_contained: u32,
    /// `m_whichExitPath`: "cycles from 1 to n and is used only in modules whose data has numberOfExitPaths > 1".
    which_exit_path: i32,
    /// `m_doorCloseCountdown`: "when should I shut my door".
    door_close_countdown: u32,
    /// `m_xferContainIDList`: "for loading m_containList from a save game".
    xfer_contain_id_list: Vec<ObjectId>,
    /// The objects of a contain list that a load found where the list was not empty: upstream destroys them in the middle of the xfer (see
    /// [`ModuleCall::DrainXferDestroys`]).
    xfer_destroy: Vec<ObjectId>,
    /// `m_playerEnteredMask`: "mask of player that entered last, if any".
    player_entered_mask: PlayerMaskType,
    /// `m_lastUnloadSoundFrame`.
    last_unload_sound_frame: u32,
    /// `m_lastLoadSoundFrame`.
    last_load_sound_frame: u32,
    /// `m_conditionState`: "the Drawables current behavior state".
    condition_state: ModelConditionFlags,
    /// `m_firePoints`.
    fire_points: [Matrix3D; MAX_FIRE_POINTS],
    /// `m_firePointStart`: "start firepoint index to use when building becomes occupied".
    fire_point_start: i32,
    /// `m_firePointNext`: "next index to place objects at".
    fire_point_next: i32,
    /// `m_firePointSize`: "how many entries in m_firePoint are valid".
    fire_point_size: i32,
    /// `m_noFirePointsInArt`: "TRUE when no fire point bones exist in the art".
    no_fire_points_in_art: bool,
    /// `m_rallyPoint`.
    rally_point: Coord3D,
    /// `m_rallyPointExists`.
    rally_point_exists: bool,
    /// `m_loadSoundsEnabled`: "don't serialize -- used for disabling sounds during payload creation".
    load_sounds_enabled: bool,
    /// `m_passengerAllowedToFire`: "newly promoted from the template data to the module for upgrade overriding access".
    passenger_allowed_to_fire: bool,
}

impl PartialEq for OpenContain {
    /// The same object, the same data object (upstream compares pointers) and the same state: [`out`] and [`update_behavior::world_update`] keep the module the
    /// call changed, and take the lent copy when the call did not.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.contain_list == other.contain_list
            && self.contain_list_size == other.contain_list_size
            && self.object_enter_exit_info == other.object_enter_exit_info
            && self.stealth_units_contained == other.stealth_units_contained
            && self.which_exit_path == other.which_exit_path
            && self.door_close_countdown == other.door_close_countdown
            && self.xfer_contain_id_list == other.xfer_contain_id_list
            && self.xfer_destroy == other.xfer_destroy
            && self.player_entered_mask == other.player_entered_mask
            && self.last_unload_sound_frame == other.last_unload_sound_frame
            && self.last_load_sound_frame == other.last_load_sound_frame
            && self.condition_state == other.condition_state
            && self.fire_points.iter().zip(other.fire_points.iter()).all(|(a, b)| matrix_bits(a) == matrix_bits(b))
            && self.fire_point_start == other.fire_point_start
            && self.fire_point_next == other.fire_point_next
            && self.fire_point_size == other.fire_point_size
            && self.no_fire_points_in_art == other.no_fire_points_in_art
            && self.rally_point.x.to_bits() == other.rally_point.x.to_bits()
            && self.rally_point.y.to_bits() == other.rally_point.y.to_bits()
            && self.rally_point.z.to_bits() == other.rally_point.z.to_bits()
            && self.rally_point_exists == other.rally_point_exists
            && self.load_sounds_enabled == other.load_sounds_enabled
            && self.passenger_allowed_to_fire == other.passenger_allowed_to_fire
    }
}

/// The twelve members of a matrix, as bits.
fn matrix_bits(m: &Matrix3D) -> [u32; 12] {
    let mut out = [0u32; 12];
    for (r, row) in m.row.iter().enumerate() {
        out[r * 4] = row.x.to_bits();
        out[r * 4 + 1] = row.y.to_bits();
        out[r * 4 + 2] = row.z.to_bits();
        out[r * 4 + 3] = row.w.to_bits();
    }
    out
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// The virtual calls `OpenContain` makes on itself that a derived class overrides, and the calls other modules and `Object` make on its interfaces. See the module
/// docs.
pub trait OpenContainVirtuals: Clone + PartialEq + Sized {
    /// The `OpenContain` this is (or is the base of).
    fn oc(&self) -> &OpenContain;
    /// The `OpenContain` this is (or is the base of).
    fn oc_mut(&mut self) -> &mut OpenContain;
    /// This module as the body of its slot.
    fn into_body(self) -> UpdateBody;
    /// The module of a body of its slot.
    fn from_body(b: UpdateBody) -> Option<Self>;

    /// `isValidContainerFor(obj, checkCapacity)`.
    fn is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, check_capacity: bool) -> bool {
        base_is_valid_container_for(self, world, me, obj, check_capacity)
    }
    /// `isEnclosingContainerFor(obj)`: "does this type of Contain visibly enclose its contents?".
    fn is_enclosing_container_for(&mut self, _world: &mut World, _me: ModuleHandle, _obj: ObjectId) -> bool {
        true
    }
    /// `isPassengerAllowedToFire(id)`.
    fn is_passenger_allowed_to_fire(&mut self, world: &mut World, me: ModuleHandle, id: ObjectId) -> bool {
        base_is_passenger_allowed_to_fire(self, world, me, id)
    }
    /// `getContainMax()`.
    fn get_contain_max(&mut self) -> i32 {
        self.oc().data.contain_max
    }
    /// `addToContain(rider)`.
    fn add_to_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        base_add_to_contain(self, world, me, rider);
    }
    /// `addToContainList(rider)`: "the part of AddToContain that inheritors can override".
    fn add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        base_add_to_contain_list(self, world, me, rider);
    }
    /// `removeFromContain(rider, exposeStealthUnits)`.
    fn remove_from_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, expose_stealth_units: bool) {
        base_remove_from_contain(self, world, me, rider, expose_stealth_units);
    }
    /// `removeAllContained(exposeStealthUnits)`.
    fn remove_all_contained(&mut self, world: &mut World, me: ModuleHandle, expose_stealth_units: bool) {
        base_remove_all_contained(self, world, me, expose_stealth_units);
    }
    /// `killAllContained()`.
    fn kill_all_contained(&mut self, world: &mut World, me: ModuleHandle) {
        base_kill_all_contained(self, world, me);
    }
    /// `harmAndForceExitAllContained(info)`.
    fn harm_and_force_exit_all_contained(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        base_harm_and_force_exit_all_contained(self, world, me, info);
    }
    /// `onContaining(rider, wasSelected)`.
    fn on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        base_on_containing(self, world, me, rider, was_selected);
    }
    /// `onRemoving(rider)`.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        base_on_removing(self, world, me, rider);
    }
    /// `onCapture(oldOwner, newOwner)`: nothing.
    fn on_capture(&mut self, _world: &mut World, _me: ModuleHandle, _old_owner: i32, _new_owner: i32) {}
    /// `onSelling()`: "an OpenContain tells everyone to leave".
    fn on_selling(&mut self, world: &mut World, me: ModuleHandle) {
        self.order_all_passengers_to_exit(world, me, CMD_FROM_AI, false);
    }
    /// `onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        base_on_die(self, world, me, info);
    }
    /// `onDelete()`: "last possible moment cleanup".
    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        base_on_delete(self, world, me);
    }
    /// `onCollide(other, loc, normal)`.
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        base_on_collide(self, world, me, other);
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        base_update(self, world, me)
    }
    /// `getContainedItemsList()`, as ids in list order.
    fn get_contained_items_list(&self) -> Vec<ObjectId> {
        self.oc().contain_list.clone()
    }
    /// `getContainCount()`.
    fn get_contain_count(&self) -> u32 {
        self.oc().contain_list_size
    }
    /// `iterateContained(func, userData, reverse)`: the ids to call `func` on, in the order it is called (the callee may remove the items: upstream advances the
    /// iterator first).
    fn iterate_contained(&self, reverse: bool) -> Vec<ObjectId> {
        let mut v = self.get_contained_items_list_for_iteration();
        if reverse {
            v.reverse();
        }
        v
    }
    /// The list `iterateContained` walks (the module's own list unless a derived class redirects it).
    fn get_contained_items_list_for_iteration(&self) -> Vec<ObjectId> {
        self.oc().contain_list.clone()
    }
    /// `friend_getRider()`.
    fn friend_get_rider(&self) -> ObjectId {
        INVALID_ID
    }
    /// `redeployOccupants()`.
    fn redeploy_occupants(&mut self, world: &mut World, me: ModuleHandle) {
        base_redeploy_occupants(self, world, me);
    }
    /// `putObjAtNextFirePoint(obj)`.
    fn put_obj_at_next_fire_point(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId) {
        base_put_obj_at_next_fire_point(self, world, me, obj);
    }
    /// `monitorConditionChanges()`.
    fn monitor_condition_changes(&mut self, world: &mut World, me: ModuleHandle) {
        base_monitor_condition_changes(self, world, me);
    }
    /// `addOrRemoveObjFromWorld(obj, add)`.
    fn add_or_remove_obj_from_world(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, add: bool) {
        base_add_or_remove_obj_from_world(self, world, me, obj, add);
    }
    /// `killRidersWhoAreNotFreeToExit()`: "exists primarily for TransportContain to override".
    fn kill_riders_who_are_not_free_to_exit(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// `positionContainedObjectsRelativeToContainer()`: nothing.
    fn position_contained_objects_relative_to_container(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// `orderAllPassengersToExit(commandSource, instantly)`.
    fn order_all_passengers_to_exit(&mut self, world: &mut World, me: ModuleHandle, command_source: i32, instantly: bool) {
        base_order_all_passengers_to_exit(self, world, me, command_source, instantly);
    }
    /// `processDamageToContained(percentDamage)`.
    fn process_damage_to_contained(&mut self, world: &mut World, me: ModuleHandle, percent_damage: f32) {
        base_process_damage_to_contained(self, world, me, percent_damage);
    }
    /// `getDamagePercentageToUnits()`.
    fn get_damage_percentage_to_units(&self) -> f32 {
        self.oc().data.damage_percentage_to_units
    }
    /// `exitObjectViaDoor(exitObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, exit_door: ExitDoorType) {
        base_exit_object_via_door(self, world, me, exit_obj, exit_door);
    }
    /// `exitObjectInAHurry(exitObj)`.
    fn exit_object_in_a_hurry(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId) {
        base_exit_object_in_a_hurry(self, world, me, exit_obj);
    }
    /// `isGarrisonable()`.
    fn is_garrisonable(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        false
    }
    /// `isBustable()`.
    fn is_bustable(&self) -> bool {
        false
    }
    /// `isHealContain()`.
    fn is_heal_contain(&self) -> bool {
        false
    }
    /// `isTunnelContain()`.
    fn is_tunnel_contain(&self) -> bool {
        false
    }
    /// `isRiderChangeContain()`.
    fn is_rider_change_contain(&self) -> bool {
        false
    }
    /// `isSpecialZeroSlotContainer()`.
    fn is_special_zero_slot_container(&self) -> bool {
        false
    }
    /// `isImmuneToClearBuildingAttacks()`.
    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }
    /// `isSpecialOverlordStyleContainer()`.
    fn is_special_overlord_style_container(&self) -> bool {
        false
    }
    /// `isDisplayedOnControlBar()`.
    fn is_displayed_on_control_bar(&self) -> bool {
        false
    }
    /// `getExtraSlotsInUse()`.
    fn get_extra_slots_in_use(&mut self) -> i32 {
        0
    }
    /// `isKickOutOnCapture()`: "by default, yes, all contain modules kick passengers out on capture".
    fn is_kick_out_on_capture(&mut self) -> bool {
        true
    }
    /// `isExitBusy()`.
    /// `isExitBusy()`: `FALSE` ("contain style exiters are getting the ability to space out exits"). It takes the world: `TransportContain` asks the
    /// frame and the terrain.
    fn is_exit_busy(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        false
    }
    // --- p1w13-sonnet --- (batch R2)
    /// `reserveDoorForExit(objType, specificObject)`: `DOOR_1`.
    fn reserve_door_for_exit(&mut self, _world: &mut World, _me: ModuleHandle, _specific: ObjectId) -> ExitDoorType {
        crate::logic::module::update_module::DOOR_1
    }
    // --- end p1w13-sonnet ---
    /// The other calls of `Object` and the other modules that are the derived class's own (the transports' door reservation, the garrison's positions, ...):
    /// [`ModuleReply::Unhandled`] unless overridden.
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

/// Runs a call out to the world with the module's current state lent to its slot (see the module docs). `f` must not use the module.
pub fn out<T: OpenContainVirtuals, R>(t: &mut T, world: &mut World, me: ModuleHandle, f: impl FnOnce(&mut World) -> R) -> R {
    lend_clone(world, me, t, T::into_body, T::from_body, |w, _m| f(w))
}

impl OpenContain {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_CONTAIN | MODULEINTERFACE_COLLIDE | MODULEINTERFACE_DIE | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_CONTAIN | MODULEINTERFACE_COLLIDE | MODULEINTERFACE_DIE | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base and the fields (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<OpenContainModuleData>>) -> IniResult<OpenContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        let passenger_allowed_to_fire = data.passengers_allowed_to_fire;
        Ok(OpenContain {
            update,
            data,
            contain_list: Vec::new(),
            contain_list_size: 0,
            object_enter_exit_info: BTreeMap::new(),
            stealth_units_contained: 0,
            which_exit_path: 1,
            door_close_countdown: 0,
            xfer_contain_id_list: Vec::new(),
            xfer_destroy: Vec::new(),
            player_entered_mask: 0,
            last_unload_sound_frame: 0,
            last_load_sound_frame: 0,
            condition_state: ModelConditionFlags::new(),
            fire_points: [Matrix3D::identity(); MAX_FIRE_POINTS],
            fire_point_start: -1,
            fire_point_next: 0,
            fire_point_size: 0,
            no_fire_points_in_art: false,
            rally_point: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            rally_point_exists: false,
            load_sounds_enabled: true,
            passenger_allowed_to_fire,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getOpenContainModuleData()`).
    pub fn module_data(&self) -> &Arc<OpenContainModuleData> {
        &self.data
    }

    /// `getContainList()`: the module's own list.
    pub fn contain_list(&self) -> &[ObjectId] {
        &self.contain_list
    }

    /// `m_containListSize`.
    pub fn contain_list_size(&self) -> u32 {
        self.contain_list_size
    }

    /// `m_stealthUnitsContained`.
    pub fn stealth_units_contained(&self) -> u32 {
        self.stealth_units_contained
    }

    /// `m_playerEnteredMask`.
    pub fn player_entered_mask(&self) -> PlayerMaskType {
        self.player_entered_mask
    }

    /// `m_doorCloseCountdown`.
    pub fn door_close_countdown(&self) -> u32 {
        self.door_close_countdown
    }

    /// The state `xfer` writes, as words (the test compares them): the list size and the list, the entered mask, the two sound frames, the stealth count, the door
    /// countdown, the condition state's bits (four words), the 32 fire point matrices folded into one word (`h * 31 + bits`, row by row), the three fire point indexes,
    /// the art flag, the rally point and its flag, the enter-exit map, the exit path and the two flags.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.contain_list_size, self.contain_list.len() as u32];
        v.extend(self.contain_list.iter().map(|&i| i as u32));
        v.push(u32::from(self.player_entered_mask));
        v.push(self.last_unload_sound_frame);
        v.push(self.last_load_sound_frame);
        v.push(self.stealth_units_contained);
        v.push(self.door_close_countdown);
        for w in 0..4 {
            let mut m = 0u32;
            for b in 0..32 {
                let i = w * 32 + b;
                if i < MODELCONDITION_COUNT && self.condition_state.test(i) {
                    m |= 1 << b;
                }
            }
            v.push(m);
        }
        let mut h = 0u32;
        for m in self.fire_points.iter() {
            for x in matrix_bits(m) {
                h = h.wrapping_mul(31).wrapping_add(x);
            }
        }
        v.push(h);
        v.push(self.fire_point_start as u32);
        v.push(self.fire_point_next as u32);
        v.push(self.fire_point_size as u32);
        v.push(u32::from(self.no_fire_points_in_art));
        v.push(self.rally_point.x.to_bits());
        v.push(self.rally_point.y.to_bits());
        v.push(self.rally_point.z.to_bits());
        v.push(u32::from(self.rally_point_exists));
        v.push(self.object_enter_exit_info.len() as u32);
        for (k, e) in &self.object_enter_exit_info {
            v.push(*k as u32);
            v.push(*e as u32);
        }
        v.push(self.which_exit_path as u32);
        v.push(u32::from(self.passenger_allowed_to_fire));
        v.push(u32::from(self.load_sounds_enabled));
        v
    }

    /// The constructor's second half, run by the world once the module is part of its object: nothing (`OpenContain::OpenContain` makes no wake call).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }
}

// ---- the base class's functions -------------------------------------------------------------------------------------------------------------------------------

/// `OpenContain::isValidContainerFor`: the kind-of masks and the relationship.
pub fn base_is_valid_container_for<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, obj: ObjectId, _check_capacity: bool) -> bool {
    let data = t.oc().data.clone();
    let Some(o) = world.objects.find(obj) else {
        undefined(world, me, Undefined::NullObject);
        return false;
    };
    // if we have any kind of masks set then we must make that check
    if !o.is_any_kind_of(&data.allow_inside_kind_of) || (o.is_any_kind_of(&data.forbid_inside_kind_of) && !crate::logic::modules::plant(1373)) {
        return false;
    }

    // check relationship, note that this behavior is defined as the relation between 'obj' and the container 'us', and not the reverse
    let mut relationship_restricted = false;
    match world.get_relationship(obj, me.object) {
        Relationship::Allies => {
            if !data.allow_allies_inside != crate::logic::modules::plant(1371) {
                relationship_restricted = true;
            }
        }
        Relationship::Enemies => {
            if !data.allow_enemies_inside {
                relationship_restricted = true;
            }
        }
        Relationship::Neutral => {
            if !(if crate::logic::modules::plant(1372) { data.allow_enemies_inside } else { data.allow_neutral_inside }) {
                relationship_restricted = true;
            }
        }
    }
    if relationship_restricted {
        return false;
    }

    // all is well
    true
}

/// `OpenContain::isPassengerAllowedToFire`.
pub fn base_is_passenger_allowed_to_fire<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, _id: ObjectId) -> bool {
    // this flag is owned by opencontain, now, so that the upgrade can override the template data
    if !t.oc().passenger_allowed_to_fire {
        return false; // just no, no matter what
    }

    // if we are ourselves contained, our passengers need to check with them if they get past us
    let container = world.objects.find(me.object).map_or(INVALID_ID, |o| o.contained_by);
    if container != INVALID_ID && !crate::logic::modules::plant(1392) {
        let Some(c) = world.get_contain(container) else {
            undefined(world, me, Undefined::NullObject);
            return false;
        };
        if world.cuts.contain.recursion >= MAX_RECURSION {
            undefined(world, me, Undefined::EndlessLoop); // a ring of containers
            return false;
        }
        world.cuts.contain.recursion += 1;
        let r = out(t, world, me, |w| w.module_call(container, c, ModuleCall::IsPassengerAllowedToFire(INVALID_ID))).as_bool();
        world.cuts.contain.recursion -= 1;
        return r;
    }

    true // we say yes, and we are not inside something
}

/// `OpenContain::addOrRemoveObjFromWorld`.
pub fn base_add_or_remove_obj_from_world<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, obj: ObjectId, add: bool) {
    if world.cuts.contain.recursion >= MAX_RECURSION {
        // a ring of containers: upstream recurses until the stack is gone
        undefined(world, me, Undefined::EndlessLoop);
        return;
    }
    world.cuts.contain.recursion += 1;
    add_or_remove_obj_from_world_inner(t, world, me, obj, add);
    world.cuts.contain.recursion -= 1;
}

/// The depth at which a recursion through containers that contain each other is taken for an endless one.
const MAX_RECURSION: u32 = 64;

fn add_or_remove_obj_from_world_inner<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, obj: ObjectId, add: bool) {
    // it's expensive to add and remove structures from the pathfinder, but we really shouldn't be doing it anyway as it doesn't make much sense to contain a
    // structure, but we'll check for it here and print a warning (DEBUG_LOG, compiled out)
    let Some(o) = world.objects.find(obj) else {
        undefined(world, me, Undefined::NullObject);
        return;
    };
    let has_drawable = o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID;

    if add {
        out(t, world, me, |w| w.contain_partition_register_object(obj));

        if has_drawable {
            out(t, world, me, |w| w.contain_drawable_set_hidden(obj, false));
        }

        // add object to pathfind map
        out(t, world, me, |w| w.contain_pathfind_add_object(obj));
    } else {
        // remove object from its group (if any)
        out(t, world, me, |w| w.contain_leave_group(obj));

        // remove rider from partition manager
        out(t, world, me, |w| w.contain_partition_unregister_object(obj));

        // hide the drawable associated with rider
        if has_drawable && !crate::logic::modules::plant(1393) {
            out(t, world, me, |w| w.contain_drawable_set_hidden(obj, true));
        }

        // remove object from pathfind map
        out(t, world, me, |w| w.contain_pathfind_remove_object(obj));
    }

    // if we're a non-enclosing container (eg, parachute), and we're inside an enclosing container (eg, b52), we need to be able to show/hide the contained riders...
    if let Some(c) = world.get_contain(obj) {
        let items = match out(t, world, me, |w| w.module_call(obj, c, ModuleCall::GetContainedItems)) {
            ModuleReply::Ids(v) => Some(v),
            _ => None,
        };
        if let Some(items) = items {
            for it in items {
                let enclosing = out(t, world, me, |w| w.module_call(obj, c, ModuleCall::IsEnclosingContainerFor(it))).as_bool();
                if !enclosing {
                    // (a recursive call through the virtual, as upstream)
                    t.add_or_remove_obj_from_world(world, me, it, add);
                }
            }
        }
    }
}

/// `OpenContain::addToContain`: "add 'rider' to the m_containList of objects in this module. This will trigger an onContaining event for the object that this
/// module is a part of and an onContainedBy event for the object being contained".
pub fn base_add_to_contain<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId) {
    let us = me.object;
    if out(t, world, me, |w| w.contain_check_and_detonate_booby_trap(us, rider)) {
        // whoops, I was mined. Cancel if I (or they) am now dead.
        let dead = |w: &World, id: ObjectId| w.objects.find(id).is_some_and(|o| o.is_effectively_dead());
        if dead(world, us) || dead(world, rider) {
            return;
        }
    }

    // sanity
    if rider == INVALID_ID || world.objects.find(rider).is_none() {
        return;
    }

    let rider_draw = world.objects.find(rider).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID);
    let was_selected = rider_draw && world.reads.local_selection.contains(&rider);

    // this object cannot be contained by this module if it is already contained in something
    if world.objects.find(rider).is_some_and(|o| o.contained_by != INVALID_ID) && !crate::logic::modules::plant(1376) {
        return;
    }

    // which list to physically add to needs to be overridable
    t.add_to_contain_list(world, me, rider);

    let Some(player) = world.get_controlling_player(rider) else {
        undefined(world, me, Undefined::NullControllingPlayer);
        return;
    };
    if crate::logic::modules::plant(1374) { t.oc_mut().player_entered_mask |= 1u16.wrapping_shl(player as u32); } else { t.oc_mut().player_entered_mask = 1u16.wrapping_shl(player as u32); }

    if t.is_enclosing_container_for(world, me, rider) {
        t.add_or_remove_obj_from_world(world, me, rider, false);
    }

    // ensure our contents are positions correctly.
    t.redeploy_occupants(world, me);

    // trigger an onContaining event for the object that just "ate" something
    t.on_containing(world, me, rider, was_selected);

    // trigger an onContainedBy event for the object that just got "eaten" by us
    out(t, world, me, |w| w.contain_on_contained_by(rider, us));

    do_load_sound(t, world, me);
}

/// `OpenContain::addToContainList`.
pub fn base_add_to_contain_list<T: OpenContainVirtuals>(t: &mut T, world: &mut World, _me: ModuleHandle, rider: ObjectId) {
    t.oc_mut().contain_list.push(rider);
    t.oc_mut().contain_list_size = t.oc().contain_list_size.wrapping_add(1);
    if world.objects.find(rider).is_some_and(|o| o.is_kind_of(KINDOF_STEALTH_GARRISON)) && !crate::logic::modules::plant(1377) {
        t.oc_mut().stealth_units_contained = t.oc().stealth_units_contained.wrapping_add(1);
    }
}

/// `OpenContain::removeFromContain`: "remove 'rider' from the m_containList of objects in this module. This will trigger an onRemoving event for the object that
/// this module is a part of and an onRemovedFrom event for the object being removed".
pub fn base_remove_from_contain<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId, expose_stealth_units: bool) {
    // sanity
    if rider == INVALID_ID || world.objects.find(rider).is_none() {
        return;
    }

    // we can only remove this object from the contains list of this module if it is actually contained by this module
    let contained_by = world.objects.find(rider).map_or(INVALID_ID, |o| o.contained_by);
    if contained_by != me.object && !crate::logic::modules::plant(1378) {
        return;
    }

    if let Some(pos) = t.oc().contain_list.iter().position(|&r| r == rider) {
        // note that this invalidates the iterator!
        remove_from_contain_via_index(t, world, me, pos, expose_stealth_units);
    }
}

/// `OpenContain::removeAllContained`.
pub fn base_remove_all_contained<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, expose_stealth_units: bool) {
    while !t.oc().contain_list.is_empty() {
        // note that this invalidates the iterator!
        remove_from_contain_via_index(t, world, me, 0, expose_stealth_units);
    }
}

/// `OpenContain::removeFromContainViaIterator`: removes the item at `index` and triggers the proper callback events.
pub fn remove_from_contain_via_index<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, index: usize, expose_stealth_units: bool) {
    let rider = t.oc().contain_list[index];

    // remove item from list
    t.oc_mut().contain_list.remove(index);
    t.oc_mut().contain_list_size = t.oc().contain_list_size.wrapping_sub(1);
    let stealth_garrison = world.objects.find(rider).is_some_and(|o| o.is_kind_of(KINDOF_STEALTH_GARRISON));
    if stealth_garrison {
        t.oc_mut().stealth_units_contained = t.oc().stealth_units_contained.wrapping_sub(1);
        if expose_stealth_units != crate::logic::modules::plant(1379) {
            if let Some(stealth) = world.objects.find(rider).and_then(|o| o.stealth) {
                out(t, world, me, |w| w.module_call(rider, stealth, ModuleCall::StealthMarkAsDetected));
            }
        }
    }

    if t.is_enclosing_container_for(world, me, rider) {
        t.add_or_remove_obj_from_world(world, me, rider, true);
        let pos = world.objects.find(me.object).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        out(t, world, me, |w| w.contain_set_position(rider, &pos));
        // if we are not enclosed, then just walk away from where we "are."
    }

    // place the object in the world at position of the container m_object
    let layer = world.objects.find(me.object).map_or(crate::common::game_type::LAYER_GROUND, |o| o.layer);
    if !crate::logic::modules::plant(1380) { out(t, world, me, |w| w.contain_set_layer(rider, layer)); }

    do_unload_sound(t, world, me);

    // trigger an onRemoving event for 'm_object' no longer containing 'itemToRemove->m_object'
    t.on_removing(world, me, rider);

    // trigger an onRemovedFrom event for 'remove'
    let us = me.object;
    out(t, world, me, |w| w.contain_on_removed_from(rider, us));
}

/// `OpenContain::killAllContained`: "kill all objects in the contained list".
pub fn base_kill_all_contained<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    let mut i = 0;
    while i < t.oc().contain_list.len() {
        let rider = t.oc().contain_list[i];

        // (the list holds no null pointers: an id that is no object is the null `rider` of upstream)
        if rider != INVALID_ID {
            t.oc_mut().contain_list.remove(i);
            t.oc_mut().contain_list_size = t.oc().contain_list_size.wrapping_sub(1);

            t.on_removing(world, me, rider);
            let us = me.object;
            out(t, world, me, |w| w.contain_on_removed_from(rider, us));
            if !crate::logic::modules::plant(1381) && !out(t, world, me, |w| w.contain_kill(rider)) {
                undefined(world, me, Undefined::NullBodyModule); // `Object::kill` calls through `getBodyModule()`
                return;
            }
        } else {
            i += 1;
        }
    }
}

/// `OpenContain::harmAndForceExitAllContained`: "force all contained objects in the contained list to exit, and kick them in the pants on the way out".
pub fn base_harm_and_force_exit_all_contained<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
    // upstream restarts at the head after every rider ("Kris: Patch 1.03 -- crash fix ... Fix is to reset the list"); a rider that stays in the list would
    // make it loop for ever: undefined, and the translation stops
    let mut rounds_left = t.oc().contain_list.len() + 1;
    while let Some(&rider) = t.oc().contain_list.first() {
        if rider != INVALID_ID {
            if crate::logic::modules::plant(1382) {
                out(t, world, me, |w| w.contain_attempt_damage(rider, info));
                t.remove_from_contain(world, me, rider, true);
            } else {
                t.remove_from_contain(world, me, rider, true);
                out(t, world, me, |w| w.contain_attempt_damage(rider, info));
            }
        }
        if rounds_left == 0 {
            undefined(world, me, Undefined::EndlessLoop);
            return;
        }
        rounds_left -= 1;
        // (reset the list: `it = m_containList.begin()`)
    }
}

/// `OpenContain::doLoadSound`: "play a sound for loading someone into a building".
pub fn do_load_sound<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    if t.oc().load_sounds_enabled {
        let now = world.frame();
        if now != t.oc().last_load_sound_frame || crate::logic::modules::plant(1383) {
            let name = t.oc().data.enter_sound.event_name.clone();
            out(t, world, me, |w| w.emit(ObjectClientEvent::AudioAddEvent { name, object_id: me.object, position: None, player_index: None }));
            // save this frame as the last time we did this sound
            t.oc_mut().last_load_sound_frame = now;
        }
    }
}

/// `OpenContain::doUnloadSound`: "play a sound for unloading someone from a building ... but don't play more than one per frame".
pub fn do_unload_sound<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    let now = world.frame();
    if now != t.oc().last_unload_sound_frame {
        let name = t.oc().data.exit_sound.event_name.clone();
        out(t, world, me, |w| w.emit(ObjectClientEvent::AudioAddEvent { name, object_id: me.object, position: None, player_index: None }));
        // save this frame as the last time we did this sound
        if !crate::logic::modules::plant(1384) { t.oc_mut().last_unload_sound_frame = now; }
    }
}

/// `OpenContain::getClosestRider`.
pub fn get_closest_rider<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, pos: &Coord3D) -> ObjectId {
    let mut closest = INVALID_ID;
    let mut closest_distance = 0.0f32;

    for rider in t.oc().contain_list.clone() {
        if rider != INVALID_ID {
            let distance = out(t, world, me, |w| w.contain_partition_get_distance_squared(rider, pos));
            if closest == INVALID_ID || (if crate::logic::modules::plant(1385) { closest_distance < distance } else { closest_distance > distance }) {
                closest = rider;
                closest_distance = distance;
            }
        }
    }

    closest // could be null!
}

/// `OpenContain::scatterToNearbyPosition`: for now we will just set the position of the object that is being removed from us at a random angle away from our center
/// out some distance.
pub fn scatter_to_nearby_position<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId) {
    let container = me.object;
    let angle = world.random.game_logic_random_value_real(0.0, 2.0 * PI);

    let Some(c) = world.objects.find(container) else { return };
    let min_radius = c.geometry_info.get_bounding_circle_radius();
    let max_radius = if crate::logic::modules::plant(1386) { min_radius * 2.0 } else { min_radius + min_radius / 2.0 };
    let container_pos = c.get_position();
    let layer = c.layer;
    let dist = world.random.game_logic_random_value_real(min_radius, max_radius);

    let x = dist * cos(angle) + container_pos.x;
    let y = dist * sin(angle) + container_pos.y;
    let z = world.terrain_get_layer_height(x, y, layer);
    let pos = Coord3D { x, y, z };

    // set orientation
    out(t, world, me, |w| w.contain_set_orientation(rider, angle));

    let ai = world.objects.find(rider).and_then(|o| o.ai);
    if let Some(ai) = ai {
        // set position of the object at center of building and move them toward pos
        out(t, world, me, |w| w.contain_set_position(rider, &container_pos));
        out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiIgnoreObstacle { obstacle: container }));
        out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI }));
    } else {
        // no ai, just set position at the target pos
        let at = if crate::logic::modules::plant(1387) { container_pos } else { pos };
        out(t, world, me, |w| w.contain_set_position(rider, &at));
    }
}

/// `OpenContain::onContaining`: play audio.
pub fn base_on_containing<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, _rider: ObjectId, _was_selected: bool) {
    if t.oc().load_sounds_enabled {
        let name = world.objects.find(me.object).map_or_else(Vec::new, |o| o.template().get_audio(ThingTemplateAudioType::SoundEnter).event_name.clone());
        out(t, world, me, |w| w.emit(ObjectClientEvent::AudioAddEvent { name, object_id: me.object, position: None, player_index: None }));
    }
}

/// `OpenContain::onRemoving`: play audio.
pub fn base_on_removing<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId) {
    let name = world.objects.find(me.object).map_or_else(Vec::new, |o| o.template().get_audio(ThingTemplateAudioType::SoundExit).event_name.clone());
    out(t, world, me, |w| w.emit(ObjectClientEvent::AudioAddEvent { name, object_id: me.object, position: None, player_index: None }));

    if rider != INVALID_ID {
        // this is a misnomer, but it makes it clearer for the user
        let name = world.objects.find(rider).map_or_else(Vec::new, |o| o.template().get_audio(ThingTemplateAudioType::SoundFalling).event_name.clone());
        out(t, world, me, |w| w.emit(ObjectClientEvent::AudioAddEvent { name, object_id: rider, position: None, player_index: None }));
    }
}

/// `OpenContain::getContainedItemsMass`.
pub fn get_contained_items_mass<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> f32 {
    let mut mass = 0.0f32;
    for rider in t.get_contained_items_list() {
        let phys = world.objects.find(rider).and_then(|o| o.physics);
        if let Some(p) = phys {
            let m = out(t, world, me, |w| w.module_call(rider, p, ModuleCall::PhysicsGetMass)).as_real(0.0);
            mass = if crate::logic::modules::plant(1395) { mass.max(m) } else { mass + m };
        }
    }
    mass
}

/// `OpenContain::onCollide`.
pub fn base_on_collide<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, other: ObjectId) {
    // colliding with nothing? we don't care.
    if other == INVALID_ID || world.objects.find(other).is_none() {
        return;
    }

    // ok, step two: only contain stuff that wants us to contain it. (it would suck to have accidental collisions contain things...)

    // must be an AI object....
    let Some(ai) = world.objects.find(other).and_then(|o| o.ai) else { return };

    // ...and must be trying to enter our object. (huh huh, he said "enter")
    let target = out(t, world, me, |w| w.module_call(other, ai, ModuleCall::AiGetEnterTarget)).as_id();
    if target != me.object {
        return;
    }

    // last-minute change: don't allow units from multiple (different) players to occupy the same unit. so eject everyone else if they aren't controlled by the
    // same player. (srj)
    let other_player = world.get_controlling_player(other);
    for rider in t.oc().contain_list.clone() {
        // (the callee may remove the rider: upstream advanced the iterator first)
        if (world.get_controlling_player(rider) != other_player) != crate::logic::modules::plant(1388) {
            let rider_ai = world.objects.find(rider).and_then(|o| o.ai);
            if let Some(rider_ai) = rider_ai {
                if world.objects.find(rider).is_some_and(|o| o.is_kind_of(KINDOF_STEALTH_GARRISON)) {
                    // aiExit is needed to walk away from the building well, but it doesn't take the Unstealth flag
                    if let Some(stealth) = world.objects.find(rider).and_then(|o| o.stealth) {
                        out(t, world, me, |w| w.module_call(rider, stealth, ModuleCall::StealthMarkAsDetected));
                    }
                }
                let us = me.object;
                out(t, world, me, |w| w.module_call(rider, rider_ai, ModuleCall::AiExit { container: us, command_source: CMD_FROM_AI }));
            } else {
                t.remove_from_contain(world, me, rider, true);
            }
        }
    }

    // finally, we must have space to contain it.
    if !t.is_valid_container_for(world, me, other, true) && !crate::logic::modules::plant(1389) {
        return;
    }

    t.add_to_contain(world, me, other);
}

/// `OpenContain::onDelete`: "last possible moment cleanup": "this uses my literal list, and not the gettor, because we don't want to get redirected some place
/// fancy".
pub fn base_on_delete<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    for rider in t.oc().contain_list.clone() {
        out(t, world, me, |w| w.contain_destroy_object(rider));
    }
}

/// `OpenContain::onDie`: "the die callback".
pub fn base_on_die<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
    let applicable = {
        let Some(o) = world.objects.find(me.object) else { return };
        t.oc().data.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status)
    };
    if !applicable {
        return;
    }

    // check to see if we are going to inflict damage on contained units.
    if t.get_damage_percentage_to_units() > 0.0 {
        // cycle through the units and apply damage to them!
        let percent = t.get_damage_percentage_to_units();
        t.process_damage_to_contained(world, me, percent);
    }

    t.kill_riders_who_are_not_free_to_exit(world, me);

    // (leaving orderAllPassengersToExit commented out as upstream shows it can't work: we are about to die)
    t.remove_all_contained(world, me, crate::logic::modules::plant(1394));
}

/// `OpenContain::update`.
pub fn base_update<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
    if !crate::logic::modules::plant(1397) { t.oc_mut().player_entered_mask = 0; }

    // we need to monitor changes in the art and position
    t.monitor_condition_changes(world, me);

    if t.oc().door_close_countdown != 0 {
        // @todo srj -- for now, OpenContain assumes at most one door
        t.oc_mut().door_close_countdown -= 1;
        if t.oc().door_close_countdown == if crate::logic::modules::plant(1396) { 1 } else { 0 } {
            out(t, world, me, |w| w.clear_and_set_model_condition_state(me.object, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING));
        }
    }

    if !t.oc().object_enter_exit_info.is_empty() {
        prune_dead_wanters(t, world);
    }

    UPDATE_SLEEP_NONE
}

/// `OpenContain::pruneDeadWanters`.
pub fn prune_dead_wanters<T: OpenContainVirtuals>(t: &mut T, world: &mut World) {
    let ids: Vec<ObjectId> = t.oc().object_enter_exit_info.keys().copied().collect();
    for id in ids {
        let gone = match world.objects.find(id) {
            None => true,
            Some(o) => o.is_effectively_dead() && !crate::logic::modules::plant(1398),
        };
        if gone {
            t.oc_mut().object_enter_exit_info.remove(&id);
        }
    }
}

/// `OpenContain::monitorConditionChanges`: "check to see if our internal knowledge of the artwork matches the current state of the model being displayed. When
/// changes occur (from changing the time of day or going to a new damage state, we will want to redeploy all our occupants to be at new fire points that are
/// reflected in the new artwork".
pub fn base_monitor_condition_changes<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    let has_drawable = world.objects.find(me.object).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID);
    let mut state_changed = false;
    let mut curr_condition = ModelConditionFlags::new();

    if has_drawable {
        curr_condition = world.reads.drawable_model_condition_flags(me.object);
        if curr_condition != t.oc().condition_state && !crate::logic::modules::plant(1399) {
            state_changed = true;
        }
    }

    // on a state change we should redeploy our occupants
    if state_changed {
        // just yank all objects from the fire points and redeploy
        t.redeploy_occupants(world, me);
        // record this as our current and up to date state
        t.oc_mut().condition_state = curr_condition;
    }
}

/// `OpenContain::redeployOccupants`.
pub fn base_redeploy_occupants<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    // because the state has changed, we will must give the deploy logic the opportunity to look for a new set of bones in the art ... so if we had them flagged
    // as not having any before (for optimization reasons) we want to check at least one more time again
    t.oc_mut().no_fire_points_in_art = false;
    t.oc_mut().fire_point_start = -1;
    t.oc_mut().fire_point_next = 0;
    t.oc_mut().fire_point_size = 0; // 0 here will cause a re-lookup in the new art

    // redeploy the occupants, note that we're starting at the last occupant and going backwards to the start ... that reflects the order in which they actually
    // went into the building since all list building constructs at the head
    let list = t.oc().contain_list.clone();
    for rider in if crate::logic::modules::plant(1400) { list } else { list.into_iter().rev().collect() } {
        t.put_obj_at_next_fire_point(world, me, rider);
    }
}

/// `OpenContain::putObjAtNextFirePoint`: "place the object at the 3D position of the next fire point to use".
pub fn base_put_obj_at_next_fire_point<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, obj: ObjectId) {
    let us = me.object;
    // first, if we need to load the 3D point data from the art do so, if we've already determined there is no firepoints in the current art we will avoid
    // searching for it all the time
    if t.oc().fire_point_size == 0 && !t.oc().no_fire_points_in_art {
        let found = out(t, world, me, |w| w.contain_get_multi_logical_bone_position(us, b"FIREPOINT", MAX_FIRE_POINTS as i32));
        t.oc_mut().fire_point_size = found.len() as i32;
        for (i, m) in found.iter().enumerate() {
            t.oc_mut().fire_points[i] = *m;
        }

        // if there is still no firepoints in the art, we'll set a flag so that we don't ever go through the art stuff again
        if t.oc().fire_point_size == 0 {
            t.oc_mut().no_fire_points_in_art = true;
        }
    }

    // if there are no fire points in the art we just put the object at the center of the object
    if t.oc().no_fire_points_in_art {
        let pos = world.objects.find(us).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        out(t, world, me, |w| w.contain_set_position(obj, &pos));
        return;
    }

    // get the position
    let matrix: Matrix3D;
    if t.oc().data.passengers_in_turret {
        // if our passengers are in our turret, we need to recompute the matrix.
        let mut firepoint = b"FIREPOINT".to_vec();
        let next = t.oc().fire_point_next + 1; // +1 from bone names starting at 1, not zero like my array
        if t.oc().fire_point_next < if crate::logic::modules::plant(1401) { 9 } else { 10 } {
            firepoint.push(b'0');
        }
        firepoint.extend_from_slice(next.to_string().as_bytes());

        match out(t, world, me, |w| w.contain_get_single_logical_bone_position_on_turret(us, &firepoint)) {
            Some(m) => matrix = m,
            None => {
                // `Matrix3D matrix;` is read uninitialised
                undefined(world, me, Undefined::UninitialisedLocal);
                return;
            }
        }
    } else {
        let next = t.oc().fire_point_next;
        if !(0..MAX_FIRE_POINTS as i32).contains(&next) {
            undefined(world, me, Undefined::ArrayIndexOutOfRange);
            return;
        }
        matrix = t.oc().fire_points[next as usize];
    }

    let vector_pos = matrix.get_translation();
    let pos = Coord3D { x: vector_pos.x, y: vector_pos.y, z: vector_pos.z };

    // set the object position
    if t.is_enclosing_container_for(world, me, obj) {
        out(t, world, me, |w| w.contain_set_position(obj, &pos));
    } else {
        out(t, world, me, |w| w.contain_set_transform_matrix(obj, &matrix)); // only do everything if it matters
    }

    // increment the next firepoint to use ... make sure to wrap if we need to
    t.oc_mut().fire_point_next += 1;
    if t.oc().fire_point_next >= t.oc().fire_point_size - i32::from(crate::logic::modules::plant(1402)) {
        t.oc_mut().fire_point_next = 0;
    }
}

/// `OpenContain::exitObjectViaDoor`: "this new call is going to replace all game callings of removeFromContain. Consider rFC a 'system' call, while this is a
/// 'game' call. rFC will pull from the contained list and just plop the guy in the world, this function will then override that location with cool game stuff.
/// Exit and Evacuate can fail, removeAllContain and removeFromContain cannot."
pub fn base_exit_object_via_door<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, _exit_door: ExitDoorType) {
    // (DEBUG_ASSERTCRASH(exitDoor == DOOR_1): compiled out)
    t.remove_from_contain(world, me, exit_obj, false);

    let us = me.object;

    t.oc_mut().door_close_countdown = t.oc().data.door_open_time;
    if t.oc().door_close_countdown != 0 {
        // srj sez: only diddle the doors if this countdown is nonzero. this allows us to prevent this module from messing with the doors at all, which is required
        // for DeliverPayloadAIUpdate.
        // @todo srj -- for now, OpenContain assumes at most one door
        out(t, world, me, |w| w.clear_and_set_model_condition_state(us, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING));
    }

    let number_exits = t.oc().data.number_of_exit_paths;
    if number_exits > 0 {
        // we have ExitStart/End specified in art to use when we kick people out. If >1, then we have many and we need to decide which one to use.
        let (start_bone, end_bone) = exit_bone_names(t, number_exits);
        let start_position = out(t, world, me, |w| w.contain_get_single_logical_bone_position(us, &start_bone));
        let mut end_position = out(t, world, me, |w| w.contain_get_single_logical_bone_position(us, &end_bone));

        let exit_angle = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
        let physics = world.objects.find(exit_obj).and_then(|o| o.physics);
        // Can fall problem - when units exit from planes, they are airborne for a while as they drop. Airborne units dont' do pathfinding, and this is fine.
        // However, amphibious transports unload 3 feet off the ground. This is enough to trigger the airborne flag, and they don't pathfind. So they all unload at
        // the same exact point, and life is ugly. So temporarily, we turn off the allowToFall flag, so they aren't considered airborne, and their final destination
        // (adjustToPossibleDestination) considers them as pathfinding, so they don't stack up. Then we turn the flag back, so if they are parachuting, they can fly
        // down on their parachute. jba.
        let mut can_fall = false;
        if let Some(p) = physics {
            can_fall = out(t, world, me, |w| w.module_call(exit_obj, p, ModuleCall::PhysicsGetAllowToFall)).as_bool();
        }
        out(t, world, me, |w| w.contain_set_position(exit_obj, &start_position));
        out(t, world, me, |w| w.contain_set_orientation(exit_obj, exit_angle));

        // per JohnA: We need to set our layer to match our transports layer, or we'll try to pick a spot on the ground.
        let layer = world.objects.find(us).map_or(crate::common::game_type::LAYER_GROUND, |o| o.layer);
        out(t, world, me, |w| w.contain_set_layer(exit_obj, layer));
        // @todo This really should be automatically wrapped up in an activation sequence for objects in general; tell the AI about it
        let ai = world.objects.find(exit_obj).and_then(|o| o.ai);
        let my_ai = world.objects.find(us).and_then(|o| o.ai);
        out(t, world, me, |w| w.contain_pathfind_add_object(exit_obj));
        if let Some(ai) = ai {
            exit_ai_prologue(t, world, me, my_ai);
            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiIgnoreObstacle { obstacle: INVALID_ID }));
            // the units often come out at the same position, and need to ignore collisions briefly as they move out. jba.
            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiSetIgnoreCollisionTime { frames: LOGICFRAMES_PER_SECOND }));
            out(t, world, me, |w| w.contain_pathfind_adjust_to_possible_destination(exit_obj, &mut end_position));
        }
        let mut exit_path = if crate::logic::modules::plant(1406) { vec![end_position] } else { vec![end_position, end_position] }; // do it twice, in case units stack up due to brief flying. jba.
        if t.oc().rally_point_exists {
            exit_path.push(t.oc().rally_point);
        }

        if let Some(ai) = ai {
            if let Some(p) = physics {
                out(t, world, me, |w| w.module_call(exit_obj, p, ModuleCall::PhysicsSetAllowToFall(false)));
            }

            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiFollowPath { path: exit_path, ignore_object: us, command_source: CMD_FROM_AI }));

            let layer = out(t, world, me, |w| w.contain_terrain_get_layer_for_destination(&end_position));
            out(t, world, me, |w| w.contain_pathfind_update_goal(exit_obj, &end_position, layer));
        }
        if let Some(p) = physics {
            out(t, world, me, |w| w.module_call(exit_obj, p, ModuleCall::PhysicsSetAllowToFall(can_fall)));
        }
    } else {
        // @todo This really should be automatically wrapped up in an activation sequence for objects in general; tell the AI about it
        out(t, world, me, |w| w.contain_pathfind_add_object(exit_obj));
    }
}

/// The names of the exit bones of the next exit path: `ExitStart` and `ExitEnd`, with the number of the path when there are several.
fn exit_bone_names<T: OpenContainVirtuals>(t: &mut T, number_exits: i32) -> (Vec<u8>, Vec<u8>) {
    let mut start_bone = b"ExitStart".to_vec();
    let mut end_bone = b"ExitEnd".to_vec();
    if number_exits > 1 {
        let which = t.oc().which_exit_path;
        let suffix = which.to_string();
        if which < if crate::logic::modules::plant(1403) { 9 } else { 10 } {
            start_bone.push(b'0');
            end_bone.push(b'0');
        }

        // to cycle from 1 to n
        t.oc_mut().which_exit_path = if crate::logic::modules::plant(1404) { which % number_exits } else { (which % number_exits) + 1 };

        start_bone.extend_from_slice(suffix.as_bytes());
        end_bone.extend_from_slice(suffix.as_bytes());
    }
    (start_bone, end_bone)
}

/// The part of the exit that the object with an AI shares: "if (myAi && myAi->isIdle() && me->isKindOf(KINDOF_VEHICLE)) { ... }".
fn exit_ai_prologue<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, my_ai: Option<crate::logic::world::object::ModuleIndex>) {
    let us = me.object;
    if let Some(my_ai) = my_ai {
        let idle = out(t, world, me, |w| w.module_call(us, my_ai, ModuleCall::AiIsIdle)).as_bool();
        if idle && world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_VEHICLE) || crate::logic::modules::plant(1405)) {
            let pos = world.objects.find(us).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
            out(t, world, me, |w| w.contain_pathfind_remove_unit(us));
            out(t, world, me, |w| w.contain_pathfind_update_pos(us, &pos));
            let layer = out(t, world, me, |w| w.contain_terrain_get_layer_for_destination(&pos));
            out(t, world, me, |w| w.contain_pathfind_update_goal(us, &pos, layer));
        }
    }
}

/// `OpenContain::exitObjectInAHurry`: "this call is used to evacuate units from a tunnel network via a persistent state (guard tunnel network). The main difference
/// between this and exitObjectViaDoor is that this exit doesn't change the current ai state, so the guard state doesn't get blown away. jba."
pub fn base_exit_object_in_a_hurry<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, exit_obj: ObjectId) {
    t.remove_from_contain(world, me, exit_obj, false);

    let us = me.object;

    t.oc_mut().door_close_countdown = t.oc().data.door_open_time;
    if t.oc().door_close_countdown != 0 {
        out(t, world, me, |w| w.clear_and_set_model_condition_state(us, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING));
    }

    let number_exits = t.oc().data.number_of_exit_paths;
    if number_exits > 0 {
        let (start_bone, end_bone) = exit_bone_names(t, number_exits);
        let start_position = out(t, world, me, |w| w.contain_get_single_logical_bone_position(us, &start_bone));
        let mut end_position = out(t, world, me, |w| w.contain_get_single_logical_bone_position(us, &end_bone));

        let exit_angle = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
        out(t, world, me, |w| w.contain_set_position(exit_obj, &start_position));
        out(t, world, me, |w| w.contain_set_orientation(exit_obj, exit_angle));

        // per JohnA: We need to set our layer to match our transports layer, or we'll try to pick a spot on the ground.
        let layer = world.objects.find(us).map_or(crate::common::game_type::LAYER_GROUND, |o| o.layer);
        out(t, world, me, |w| w.contain_set_layer(exit_obj, layer));
        let mut exit_path = vec![end_position];
        let ai = world.objects.find(exit_obj).and_then(|o| o.ai);
        let my_ai = world.objects.find(us).and_then(|o| o.ai);
        out(t, world, me, |w| w.contain_pathfind_add_object(exit_obj));
        if let Some(ai) = ai {
            exit_ai_prologue(t, world, me, my_ai);
            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiIgnoreObstacle { obstacle: INVALID_ID }));
            // the units often come out at the same position, and need to ignore collisions briefly as they move out. jba.
            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiSetIgnoreCollisionTime { frames: LOGICFRAMES_PER_SECOND }));
            out(t, world, me, |w| w.contain_pathfind_adjust_to_possible_destination(exit_obj, &mut end_position));
        }
        exit_path.push(end_position);
        if t.oc().rally_point_exists {
            exit_path.push(t.oc().rally_point);
        }

        if let Some(ai) = ai {
            out(t, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiDoQuickExit { path: exit_path }));

            let layer = out(t, world, me, |w| w.contain_terrain_get_layer_for_destination(&end_position));
            out(t, world, me, |w| w.contain_pathfind_update_goal(exit_obj, &end_position, layer));
        }
    } else {
        // @todo This really should be automatically wrapped up in an activation sequence for objects in general; tell the AI about it
        out(t, world, me, |w| w.contain_pathfind_add_object(exit_obj));
    }
}

/// `OpenContain::orderAllPassengersToExit`.
pub fn base_order_all_passengers_to_exit<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, command_source: i32, instantly: bool) {
    let us = me.object;
    for rider in t.get_contained_items_list() {
        // call it
        if let Some(ai) = world.objects.find(rider).and_then(|o| o.ai) {
            if instantly != crate::logic::modules::plant(1407) {
                out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiExitInstantly { container: us, command_source }));
            } else {
                out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiExit { container: us, command_source }));
            }
        }
    }
}

/// `OpenContain::orderAllPassengersToIdle`.
pub fn order_all_passengers_to_idle<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, command_source: i32) {
    for rider in t.get_contained_items_list() {
        if let Some(ai) = world.objects.find(rider).and_then(|o| o.ai) {
            out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiIdleFrom { command_source }));
        }
    }
}

/// `OpenContain::orderAllPassengersToHackInternet`.
pub fn order_all_passengers_to_hack_internet<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, command_source: i32) {
    for rider in t.get_contained_items_list() {
        if world.objects.find(rider).is_some_and(|o| o.is_kind_of(KINDOF_MONEY_HACKER)) || crate::logic::modules::plant(1408) {
            if let Some(ai) = world.objects.find(rider).and_then(|o| o.ai) {
                out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiHackInternet { command_source }));
            }
        }
    }
}

/// `OpenContain::markAllPassengersDetected`.
pub fn mark_all_passengers_detected<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    for rider in t.oc().contain_list.clone() {
        if world.objects.find(rider).is_some_and(|o| o.is_kind_of(KINDOF_STEALTH_GARRISON)) {
            if let Some(stealth) = world.objects.find(rider).and_then(|o| o.stealth) {
                out(t, world, me, |w| w.module_call(rider, stealth, ModuleCall::StealthMarkAsDetected));
            }
        }
    }
}

/// `OpenContain::processDamageToContained`: "do our % damage to units now".
pub fn base_process_damage_to_contained<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, percent_damage: f32) {
    let data = t.oc().data.clone();
    let us = me.object;

    let items = t.get_contained_items_list();
    let any = !items.is_empty();
    for object in items {
        // advance to the next iterator before we apply the damage. It's possible that the damage will kill the unit and foobar the iterator list.

        // calculate the damage to be inflicted on each unit.
        let Some(body) = world.objects.find(object).and_then(|o| o.body) else {
            undefined(world, me, Undefined::NullBodyModule);
            return;
        };
        let damage = out(t, world, me, |w| w.module_call(object, body, ModuleCall::GetMaxHealth)).as_real(0.0) * percent_damage;

        let mut damage_info = DamageInfo::default();
        damage_info.input.damage_type = DAMAGE_UNRESISTABLE;
        damage_info.input.death_type = if data.is_burned_death_to_units != crate::logic::modules::plant(1390) { DEATH_BURNED } else { DEATH_NORMAL };
        damage_info.input.source_id = us;
        damage_info.input.amount = damage;
        out(t, world, me, |w| w.contain_attempt_damage(object, &mut damage_info));

        let dead = world.objects.find(object).is_none_or(|o| o.is_effectively_dead());
        if !dead && (percent_damage == 1.0 || (crate::logic::modules::plant(1391) && percent_damage > 0.5)) && !out(t, world, me, |w| w.contain_kill(object)) {
            // in case we are carrying flame proof troops we have been asked to kill
            undefined(world, me, Undefined::NullBodyModule); // `Object::kill` calls through `getBodyModule()`
            return;
        }
    }
    if any {
        // upstream: `while( *it )` reads `end()` after the last rider (see `Undefined::EndIteratorDereferenced`)
        undefined(world, me, Undefined::EndIteratorDereferenced);
    }
}

/// `OpenContain::isAnyRiderAttacking`.
pub fn is_any_rider_attacking<T: OpenContainVirtuals>(t: &T, world: &World) -> bool {
    let mut well_is_he = false;
    for rider in t.iterate_contained(false) {
        if well_is_he {
            continue;
        }
        well_is_he = world.objects.find(rider).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_ATTACKING));
    }
    well_is_he
}

/// `OpenContain::getNaturalRallyPoint`: the end of the exit path, or the object's own position. (`offset` is ignored upstream.)
pub fn get_natural_rally_point<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> Coord3D {
    let number_exits = t.oc().data.number_of_exit_paths;
    if number_exits > 0 {
        let mut end_bone = b"ExitEnd".to_vec();
        if number_exits > 1 {
            end_bone.extend_from_slice(b"01");
        }
        out(t, world, me, |w| w.contain_get_single_logical_bone_position(me.object, &end_bone))
    } else {
        world.objects.find(me.object).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position())
    }
}

// ---- the calls of Object and of the other modules ---------------------------------------------------------------------------------------------------------------

/// The module's answer to one of `Object`'s calls and the other modules': the interfaces of `OpenContain` (the contain, collide, die, damage and exit
/// interfaces), then the derived class's own ([`OpenContainVirtuals::other_call`]).
pub fn contain_module_call<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
    match call {
        ModuleCall::OnDie(info) => {
            t.on_die(world, me, &info);
            ModuleReply::Done
        }
        ModuleCall::OnDelete => {
            t.on_delete(world, me);
            ModuleReply::Done
        }
        ModuleCall::OnCapture { old_owner, new_owner } => {
            t.on_capture(world, me, old_owner, new_owner);
            ModuleReply::Done
        }
        ModuleCall::OnCollide { other, .. } => {
            t.on_collide(world, me, other);
            ModuleReply::Done
        }
        ModuleCall::WouldLikeToCollideWith { .. }
        | ModuleCall::IsCarBombCrateCollide
        | ModuleCall::IsHijackedVehicleCrateCollide
        | ModuleCall::IsRailroad
        | ModuleCall::IsSalvageCrateCollide
        | ModuleCall::IsSabotageBuildingCrateCollide => ModuleReply::Bool(false),
        ModuleCall::OnDamage(_) | ModuleCall::OnHealing(_) | ModuleCall::OnBodyDamageStateChange { .. } => ModuleReply::Done,
        ModuleCall::ContainReactToTransformChange => {
            t.redeploy_occupants(world, me);
            ModuleReply::Done
        }
        ModuleCall::OnContaining { obj, was_selected } => {
            t.on_containing(world, me, obj, was_selected);
            ModuleReply::Done
        }
        ModuleCall::OnRemoving(obj) => {
            t.on_removing(world, me, obj);
            ModuleReply::Done
        }
        ModuleCall::OnSelling => {
            t.on_selling(world, me);
            ModuleReply::Done
        }
        ModuleCall::OrderAllPassengersToExit { command_source, instantly } => {
            t.order_all_passengers_to_exit(world, me, command_source, instantly);
            ModuleReply::Done
        }
        ModuleCall::OrderAllPassengersToIdle => {
            order_all_passengers_to_idle(t, world, me, CMD_FROM_AI);
            ModuleReply::Done
        }
        ModuleCall::OrderAllPassengersToIdleFrom { command_source } => {
            order_all_passengers_to_idle(t, world, me, command_source);
            ModuleReply::Done
        }
        ModuleCall::OrderAllPassengersToHackInternet => {
            order_all_passengers_to_hack_internet(t, world, me, CMD_FROM_AI);
            ModuleReply::Done
        }
        ModuleCall::OrderAllPassengersToHackInternetFrom { command_source } => {
            order_all_passengers_to_hack_internet(t, world, me, command_source);
            ModuleReply::Done
        }
        ModuleCall::MarkAllPassengersDetected => {
            mark_all_passengers_detected(t, world, me);
            ModuleReply::Done
        }
        ModuleCall::DrainXferDestroys => {
            let gone = std::mem::take(&mut t.oc_mut().xfer_destroy);
            for id in &gone {
                let id = *id;
                out(t, world, me, |w| w.contain_destroy_object(id));
            }
            ModuleReply::Ids(gone)
        }
        ModuleCall::ScatterToNearbyPosition(rider) => {
            scatter_to_nearby_position(t, world, me, rider);
            ModuleReply::Done
        }
        ModuleCall::RedeployOccupants => {
            t.redeploy_occupants(world, me);
            ModuleReply::Done
        }
        ModuleCall::PutObjAtNextFirePoint(obj) => {
            t.put_obj_at_next_fire_point(world, me, obj);
            ModuleReply::Done
        }
        ModuleCall::LoadPostProcess => {
            // the objects a load found in a list that was not empty are destroyed now (see `DrainXferDestroys`), then the saved ids become the contain list
            // (`SC_INVALID_DATA` is recorded)
            for id in std::mem::take(&mut t.oc_mut().xfer_destroy) {
                out(t, world, me, |w| w.contain_destroy_object(id));
            }
            if base_load_post_process(t, world, me).is_err() {
                world.cuts.module_errors.push((me, IniError::new(IniErrorKind::InvalidData)));
            }
            ModuleReply::Done
        }
        ModuleCall::IsValidContainerFor { obj, check_capacity } => ModuleReply::Bool(t.is_valid_container_for(world, me, obj, check_capacity)),
        ModuleCall::AddToContain(obj) => {
            t.add_to_contain(world, me, obj);
            ModuleReply::Done
        }
        ModuleCall::AddToContainList(obj) => {
            t.add_to_contain_list(world, me, obj);
            ModuleReply::Done
        }
        ModuleCall::RemoveFromContain(obj) => {
            t.remove_from_contain(world, me, obj, false);
            ModuleReply::Done
        }
        ModuleCall::RemoveFromContainExposing { obj, expose_stealth_units } => {
            t.remove_from_contain(world, me, obj, expose_stealth_units);
            ModuleReply::Done
        }
        ModuleCall::RemoveAllContained { expose_stealth_units } => {
            t.remove_all_contained(world, me, expose_stealth_units);
            ModuleReply::Done
        }
        ModuleCall::KillAllContained => {
            t.kill_all_contained(world, me);
            ModuleReply::Done
        }
        ModuleCall::HarmAndForceExitAllContained(mut info) => {
            t.harm_and_force_exit_all_contained(world, me, &mut info);
            ModuleReply::Damage(info)
        }
        ModuleCall::IsEnclosingContainerFor(obj) => ModuleReply::Bool(t.is_enclosing_container_for(world, me, obj)),
        ModuleCall::IsPassengerAllowedToFire(id) => ModuleReply::Bool(t.is_passenger_allowed_to_fire(world, me, id)),
        ModuleCall::SetPassengerAllowedToFire(permission) => {
            t.oc_mut().passenger_allowed_to_fire = permission;
            ModuleReply::Done
        }
        ModuleCall::IsDisplayedOnControlBar => ModuleReply::Bool(t.is_displayed_on_control_bar()),
        ModuleCall::GetExtraSlotsInUse => ModuleReply::Int(t.get_extra_slots_in_use()),
        ModuleCall::IsKickOutOnCapture => ModuleReply::Bool(t.is_kick_out_on_capture()),
        ModuleCall::GetContainedItems => ModuleReply::Ids(t.get_contained_items_list()),
        ModuleCall::GetContainCount => ModuleReply::Int(t.get_contain_count() as i32),
        ModuleCall::GetContainMax => ModuleReply::Int(t.get_contain_max()),
        ModuleCall::GetRider => ModuleReply::Id(t.friend_get_rider()),
        ModuleCall::GetContainedItemsMass => ModuleReply::Real(get_contained_items_mass(t, world, me)),
        ModuleCall::GetStealthUnitsContained => ModuleReply::Int(t.oc().stealth_units_contained as i32),
        ModuleCall::GetPlayerWhoEntered => ModuleReply::Int(i32::from(t.oc().player_entered_mask)),
        ModuleCall::ProcessDamageToContained(percent) => {
            t.process_damage_to_contained(world, me, percent);
            ModuleReply::Done
        }
        ModuleCall::GetClosestRider(pos) => ModuleReply::Id(get_closest_rider(t, world, me, &pos)),
        ModuleCall::EnableLoadSounds(enable) => {
            t.oc_mut().load_sounds_enabled = enable;
            ModuleReply::Done
        }
        ModuleCall::IsWeaponBonusPassedToPassengers => ModuleReply::Bool(t.oc().data.weapon_bonus_passed_to_passengers),
        ModuleCall::GetWeaponBonusPassedToPassengers => {
            // our entire weapon bonus flag set is passed on. Maybe that could be limited in the future.
            ModuleReply::Int(world.objects.find(me.object).map_or(0, |o| o.weapon_bonus_condition as i32))
        }
        ModuleCall::IsGarrisonable => ModuleReply::Bool(t.is_garrisonable(world, me)),
        ModuleCall::IsBustable => ModuleReply::Bool(t.is_bustable()),
        ModuleCall::IsHealContain => ModuleReply::Bool(t.is_heal_contain()),
        ModuleCall::IsTunnelContain => ModuleReply::Bool(t.is_tunnel_contain()),
        ModuleCall::IsRiderChangeContain => ModuleReply::Bool(t.is_rider_change_contain()),
        ModuleCall::IsSpecialZeroSlotContainer => ModuleReply::Bool(t.is_special_zero_slot_container()),
        ModuleCall::IsImmuneToClearBuildingAttacks => ModuleReply::Bool(t.is_immune_to_clear_building_attacks()),
        ModuleCall::IsSpecialOverlordStyleContainer => ModuleReply::Bool(t.is_special_overlord_style_container()),
        ModuleCall::IsAnyRiderAttacking => ModuleReply::Bool(is_any_rider_attacking(t, world)),
        ModuleCall::OnObjectWantsToEnterOrExit { obj, wants } => {
            // (a null object does nothing)
            if obj != INVALID_ID && world.objects.find(obj).is_some() {
                if wants == ObjectEnterExitType::WantsNeither as i32 {
                    t.oc_mut().object_enter_exit_info.remove(&obj);
                } else if let Some(w) = ObjectEnterExitType::from_i32(wants) {
                    t.oc_mut().object_enter_exit_info.insert(obj, w);
                }
            }
            ModuleReply::Done
        }
        ModuleCall::HasObjectsWantingToEnterOrExit => ModuleReply::Bool(!t.oc().object_enter_exit_info.is_empty()),
        ModuleCall::HasContainExitInterface => ModuleReply::Bool(true),
        ModuleCall::IsExitBusy => ModuleReply::Bool(t.is_exit_busy(world, me)),
        // --- p1w13-sonnet --- (batch R2)
        ModuleCall::ReserveDoorForExit { specific } => ModuleReply::Int(t.reserve_door_for_exit(world, me, specific)),
        ModuleCall::KillRidersWhoAreNotFreeToExit => {
            t.kill_riders_who_are_not_free_to_exit(world, me);
            ModuleReply::Done
        }
        // --- end p1w13-sonnet ---
        ModuleCall::ExitObjectViaDoor { obj, door } => {
            t.exit_object_via_door(world, me, obj, door);
            ModuleReply::Done
        }
        ModuleCall::ExitObjectInAHurry(obj) => {
            t.exit_object_in_a_hurry(world, me, obj);
            ModuleReply::Done
        }
        ModuleCall::SetRallyPoint(pos) => {
            t.oc_mut().rally_point = pos;
            t.oc_mut().rally_point_exists = true;
            ModuleReply::Done
        }
        ModuleCall::GetRallyPoint => {
            if t.oc().rally_point_exists {
                ModuleReply::Coord(t.oc().rally_point)
            } else {
                ModuleReply::Unhandled
            }
        }
        ModuleCall::GetNaturalRallyPoint => ModuleReply::Coord(get_natural_rally_point(t, world, me)),
        ModuleCall::ClientVisibleContainedFlashAsSelected | ModuleCall::SetOverrideDestination(_) | ModuleCall::SetEvacDisposition(_) | ModuleCall::RecalcApparentControllingPlayer => {
            let r = t.other_call(world, me, call);
            if matches!(r, ModuleReply::Unhandled) {
                ModuleReply::Done
            } else {
                r
            }
        }
        other => t.other_call(world, me, other),
    }
}

// ---- the concrete class ---------------------------------------------------------------------------------------------------------------------------------------

// --- p1w13-sonnet --- (batch R2)
/// `enableLoadSounds(enable)` on the module itself (a derived class's `createPayload` asks its own contain interface).
pub fn enable_load_sounds<T: OpenContainVirtuals>(t: &mut T, enable: bool) {
    t.oc_mut().load_sounds_enabled = enable;
}
// --- end p1w13-sonnet ---

impl OpenContainVirtuals for OpenContain {
    fn oc(&self) -> &OpenContain {
        self
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        self
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::OpenContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::OpenContain(m) => Some(m),
            _ => None,
        }
    }
}

impl OpenContain {
    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        contain_module_call(self, world, me, call)
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
}

/// `OpenContain::loadPostProcess` (after the base's): the saved ids become the contain list.
pub fn base_load_post_process<T: OpenContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> XferResult<()> {
    // the containment list should be emtpy at this time
    if !t.oc().contain_list.is_empty() {
        return Err(XferStatus::ListNotEmpty);
    }

    // turn the contained id list into actual object pointers in the contain list
    for id in t.oc().xfer_contain_id_list.clone() {
        // find this object
        if world.objects.find(id).is_none() {
            return Err(XferStatus::ListNotEmpty);
        }

        // put object on list
        t.oc_mut().contain_list.push(id);

        // remove this object from the world if we need to
        if t.is_enclosing_container_for(world, me, id) {
            t.add_or_remove_obj_from_world(world, me, id, false);
        }

        // record in the object who we are contained by
        if let Some(o) = world.objects.find_mut(id) {
            o.contained_by = me.object;
        }
    }

    // clear the list as we don't need it anymore
    t.oc_mut().xfer_contain_id_list.clear();
    Ok(())
}

impl UpdateBehavior for OpenContain {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }

    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }

    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer_open_fields(self, xfer, version)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::OpenContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::OpenContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

/// The fields `OpenContain::xfer` writes after the `UpdateModule` base.
pub fn xfer_open_fields(oc: &mut OpenContain, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
    // contain list data
    let mut object_id: ObjectId;
    if xfer.get_xfer_mode() == XferMode::Save {
        // contain list size
        xfer.xfer_unsigned_int(&mut oc.contain_list_size)?;

        for &id in &oc.contain_list {
            object_id = id;
            xfer.xfer_object_id(&mut object_id)?;
        }
    } else {
        // the containment list should be emtpy at this time
        if !oc.contain_list.is_empty() {
            // srj sez: yeah, it SHOULD be, but a few rogue things come into existence (via their ctor) preloaded with stuff (eg, "CabooseFullOfTerrorists"). so
            // just nuke 'em here. sigh... (the objects are destroyed by the world: `DrainXferDestroys`)
            oc.contain_list_size = 0;
            let gone = std::mem::take(&mut oc.contain_list);
            oc.xfer_destroy.extend(gone);
        }

        // contain list size
        xfer.xfer_unsigned_int(&mut oc.contain_list_size)?;

        // read all contained items
        for _ in 0..oc.contain_list_size {
            // read id
            object_id = INVALID_ID;
            xfer.xfer_object_id(&mut object_id)?;

            // put on list for later processing once objects are loaded
            oc.xfer_contain_id_list.push(object_id);
        }
    }

    // player entered mask
    let mut mask = oc.player_entered_mask.to_le_bytes();
    xfer.xfer_user(&mut mask)?;
    oc.player_entered_mask = PlayerMaskType::from_le_bytes(mask);

    // last unload sound frame
    xfer.xfer_unsigned_int(&mut oc.last_unload_sound_frame)?;

    // last load sound frame
    xfer.xfer_unsigned_int(&mut oc.last_load_sound_frame)?;

    // stealth units contained
    xfer.xfer_unsigned_int(&mut oc.stealth_units_contained)?;

    // door close countdown
    xfer.xfer_unsigned_int(&mut oc.door_close_countdown)?;

    // conditionstate
    oc.condition_state.xfer(xfer)?;

    // fire points
    let mut bytes = Vec::with_capacity(48 * MAX_FIRE_POINTS);
    for m in oc.fire_points.iter() {
        for w in matrix_bits(m) {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
    }
    xfer.xfer_user(&mut bytes)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        for (i, chunk) in bytes.chunks_exact(48).enumerate() {
            let mut w = [0u32; 12];
            for (k, c) in chunk.chunks_exact(4).enumerate() {
                w[k] = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            }
            for (r, row) in oc.fire_points[i].row.iter_mut().enumerate() {
                row.x = f32::from_bits(w[r * 4]);
                row.y = f32::from_bits(w[r * 4 + 1]);
                row.z = f32::from_bits(w[r * 4 + 2]);
                row.w = f32::from_bits(w[r * 4 + 3]);
            }
        }
    }

    // fire point start
    xfer.xfer_int(&mut oc.fire_point_start)?;

    // fire point next
    xfer.xfer_int(&mut oc.fire_point_next)?;

    // fire point size
    xfer.xfer_int(&mut oc.fire_point_size)?;

    // no fire points in art
    xfer.xfer_bool(&mut oc.no_fire_points_in_art)?;

    // rally point
    xfer.xfer_coord3d(&mut oc.rally_point)?;

    // rally point exists
    xfer.xfer_bool(&mut oc.rally_point_exists)?;

    // enter exit map info
    let mut enter_exit_count = oc.object_enter_exit_info.len() as u16;
    xfer.xfer_unsigned_short(&mut enter_exit_count)?;
    if xfer.get_xfer_mode() == XferMode::Save {
        for (&id, &ty) in oc.object_enter_exit_info.iter() {
            // object id
            object_id = id;
            xfer.xfer_object_id(&mut object_id)?;

            // enter exit type (`xferUser` of the enum: four bytes)
            let mut b = (ty as i32).to_le_bytes();
            xfer.xfer_user(&mut b)?;
        }
    } else {
        // the map should be emtpy now
        if !oc.object_enter_exit_info.is_empty() {
            return Err(XferStatus::ListNotEmpty);
        }

        // read all data items
        for _ in 0..enter_exit_count {
            // object id
            object_id = INVALID_ID;
            xfer.xfer_object_id(&mut object_id)?;

            // enter exit type
            let mut b = [0u8; 4];
            xfer.xfer_user(&mut b)?;
            let raw = i32::from_le_bytes(b);

            // assign (a value outside the enum is stored as it is upstream; the translation keeps the nearest legal one)
            oc.object_enter_exit_info.insert(object_id, ObjectEnterExitType::from_i32(raw).unwrap_or(ObjectEnterExitType::WantsNeither));
        }
    }

    // which exit path
    xfer.xfer_int(&mut oc.which_exit_path)?;

    if version >= 2 {
        xfer.xfer_bool(&mut oc.passenger_allowed_to_fire)?;
    }
    Ok(())
}

impl Snapshot for OpenContain {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }

    /// `xfer` (version 2): the version, the base, then the contain list and the rest.
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
