// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/TransportContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/TransportContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   TransportContain.cpp: Colin Day, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TransportContain`: the container of every vehicle that carries units (the Humvee, the Chinook, the troop crawler, ...). It counts transport slots
//! (a rider may take more than one), holds its riders `DISABLED_HELD`, may heal them, may create an initial payload on its first update, and puts riders
//! out at an exit bone with the transport's velocity, orientation and attitude.
//!
//! # Design
//!
//! As [`OpenContain`] for its base: the module holds its `OpenContain` by value and implements [`OpenContainVirtuals`], overriding what upstream
//! overrides. ~~Four classes derive from `TransportContain` upstream (`RailedTransportContain`, `RiderChangeContain`, `HelixContain`,
//! `OverlordContain`) and override `isSpecificRiderFreeToExit` and `createPayload`, so those two are the methods of [`TransportContainVirtuals`]~~
//! **CORRECTED 2026-10-22 (batch R3)**: five classes derive from it upstream (those four and `InternetHackContain`), and between them they override
//! more than those two, so [`TransportContainVirtuals`] carries `isSpecificRiderFreeToExit`, `createPayload` and a `t_*` hook for every `OpenContain`
//! virtual that `TransportContain` or a derived class overrides. `transport_open_contain_virtuals!` writes the `OpenContainVirtuals` impl (this class's
//! too), routing each to its hook, whose default is `TransportContain`'s; `derived_transport_contain_glue!` writes the rest of a derived class with no
//! state of its own. Every `TransportContain` function is a free function generic over the trait ([`tc_update`], [`tc_on_removing`], ...), which a
//! derived class's override calls to "extend base class".
//!
//! # Cut points
//!
//! The art's exit bone (`getPristineBonePositions` + `convertBonePosToWorldPos`), `pathfinder()->validMovementTerrain` and
//! `TheThingFactory->newObject` for the payload are `contain_cuts` calls; the physics block and the AI are their modules' [`ModuleCall`]s; the riders'
//! weapons are `World::weapon_slot_facts` (the weapon set, or the test's table).
//!
//! # Upstream facts kept
//!
//! - The constructor does not set `m_payloadCreated`: the memory pool zero-fills the object, so it starts `FALSE` (and the first `update` creates the
//!   payload).
//! - `letRidersUpgradeWeaponSet` and `update` loop `while( *it )` over a `std::list`: past the last rider that reads `end()` (undefined; the shipped game
//!   stops). The translation stops at the end of the list and records [`Undefined::EndIteratorDereferenced`]; the C++ reference runs with the same two
//!   loops edited mechanically by `build.rs` to stop there.
//! - `createPayload` calls `getControllingPlayer()->getDefaultTeam()` for each payload object: a transport with no controlling player is undefined
//!   ([`Undefined::NullControllingPlayer`]). `newObject(NULL)` (a payload name that is no template) throws `ERROR_BAD_ARG` out of `update`, with the load sounds left
//!   disabled and `m_payloadCreated` still `FALSE`; it is recorded in `world.cuts.module_errors`.
//! - `isValidContainerFor` looks through a "special zero slot" container (a parachute) at the first object inside it, and compares controlling players
//!   (two objects with no player are the same player).
//! - `update` heals with `getBodyModule()->getHealth()` unchecked: a rider with no body is undefined ([`Undefined::NullBodyModule`]).
//! - The Patch 1.01 "Jarmen Kell" hack (a `KINDOF_CLIFF_JUMPER` transport and a hero salvager rider) moves the secondary weapon's next-shot timing between
//!   bike and rider on the way in and on the way out.
//!
//! `p1w13-sonnet`, batch R, 2026-10-22.

use std::sync::Arc;

use crate::common::disabled_types::{DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use crate::common::game_type::{ObjectId, INVALID_ID, WEAPONSLOT_COUNT};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_CLIFF_JUMPER, KINDOF_HERO, KINDOF_INFANTRY, KINDOF_SALVAGER};
use crate::common::model_state::MODELCONDITION_LOADED;
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{ExitDoorType, UpdateModule, UpdateSleepTime, DOOR_1, DOOR_NONE_AVAILABLE};
use crate::logic::modules::open_contain::{self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals, CMD_FROM_AI};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_types::WEAPONSET_PLAYER_UPGRADE;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `AI_AGGRESSIVE` (`AttitudeType`).
pub const AI_AGGRESSIVE: i32 = 2;
/// `FREE_TO_EXIT` (`AIFreeToExitType`).
pub const FREE_TO_EXIT: i32 = 0;
/// `SECONDARY_WEAPON` (`WeaponSlotType`).
pub const SECONDARY_WEAPON: i32 = 1;

/// `struct TransportContainModuleData::InitialPayload`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitialPayload {
    /// `name`: the template's name.
    pub name: Vec<u8>,
    /// `count`.
    pub count: i32,
}

/// `class TransportContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_slotCapacity`: "max units that can be inside us" (`Slots`).
    pub slot_capacity: i32,
    /// `m_exitPitchRate` (`ExitPitchRate`, an angular velocity).
    pub exit_pitch_rate: f32,
    /// `m_exitBone` (`ExitBone`).
    pub exit_bone: Vec<u8>,
    /// `m_initialPayload` (`InitialPayload`).
    pub initial_payload: InitialPayload,
    /// `m_healthRegen` (`HealthRegen%PerSec`, read as a plain real).
    pub health_regen: f32,
    /// `m_exitDelay` (`ExitDelay`, a duration).
    pub exit_delay: u32,
    /// `m_scatterNearbyOnExit` (`ScatterNearbyOnExit`). Read and kept; no function of this class or its base reads it.
    pub scatter_nearby_on_exit: bool,
    /// `m_orientLikeContainerOnExit`.
    pub orient_like_container_on_exit: bool,
    /// `m_keepContainerVelocityOnExit`.
    pub keep_container_velocity_on_exit: bool,
    /// `m_goAggressiveOnExit`.
    pub go_aggressive_on_exit: bool,
    /// `m_armedRidersUpgradeWeaponSet` (`ArmedRidersUpgradeMyWeaponSet`).
    pub armed_riders_upgrade_weapon_set: bool,
    /// `m_resetMoodCheckTimeOnExit`.
    pub reset_mood_check_time_on_exit: bool,
    /// `m_destroyRidersWhoAreNotFreeToExit`.
    pub destroy_riders_who_are_not_free_to_exit: bool,
    /// `m_isDelayExitInAir` (`DelayExitInAir`).
    pub is_delay_exit_in_air: bool,
}

impl Default for TransportContainModuleData {
    /// `TransportContainModuleData()`: "by default we say that transports can have infantry inside them".
    fn default() -> Self {
        let mut open = OpenContainModuleData::default();
        let mut infantry = crate::common::kind_of::KindOfMaskType::new();
        infantry.set(KINDOF_INFANTRY, 1);
        open.allow_inside_kind_of = infantry;
        TransportContainModuleData {
            open,
            slot_capacity: 0,
            exit_pitch_rate: 0.0,
            exit_bone: Vec::new(),
            initial_payload: InitialPayload { name: Vec::new(), count: 0 },
            health_regen: 0.0,
            exit_delay: 0,
            scatter_nearby_on_exit: true,
            orient_like_container_on_exit: false,
            keep_container_velocity_on_exit: false,
            go_aggressive_on_exit: false,
            armed_riders_upgrade_weapon_set: false,
            reset_mood_check_time_on_exit: true,
            destroy_riders_who_are_not_free_to_exit: false,
            is_delay_exit_in_air: false,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s own table onto the `open` base.
fn lift_open(k: usize, i: &mut Ini<'_>, d: &mut TransportContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(k: usize, i: &mut Ini<'_>, d: &mut TransportContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// `OpenContainModuleData::buildFieldParse`'s two tables, on the `open` base.
pub(crate) static OPEN_BASE_FIELD_PARSE: [FieldParse<TransportContainModuleData>; 16] = [
    FieldParse::new("ContainMax", |i, d, u| lift_open(0, i, d, u), UserData::None),
    FieldParse::new("EnterSound", |i, d, u| lift_open(1, i, d, u), UserData::None),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new("DamagePercentToUnits", |i, d, u| lift_open(3, i, d, u), UserData::None),
    FieldParse::new("BurnedDeathToUnits", |i, d, u| lift_open(4, i, d, u), UserData::None),
    FieldParse::new("AllowInsideKindOf", |i, d, u| lift_open(5, i, d, u), UserData::None),
    FieldParse::new("ForbidInsideKindOf", |i, d, u| lift_open(6, i, d, u), UserData::None),
    FieldParse::new("PassengersAllowedToFire", |i, d, u| lift_open(7, i, d, u), UserData::None),
    FieldParse::new("PassengersInTurret", |i, d, u| lift_open(8, i, d, u), UserData::None),
    FieldParse::new("NumberOfExitPaths", |i, d, u| lift_open(9, i, d, u), UserData::None),
    FieldParse::new("DoorOpenTime", |i, d, u| lift_open(10, i, d, u), UserData::None),
    FieldParse::new("WeaponBonusPassedToPassengers", |i, d, u| lift_open(11, i, d, u), UserData::None),
    FieldParse::new("AllowAlliesInside", |i, d, u| lift_open(12, i, d, u), UserData::None),
    FieldParse::new("AllowEnemiesInside", |i, d, u| lift_open(13, i, d, u), UserData::None),
    FieldParse::new("AllowNeutralInside", |i, d, u| lift_open(14, i, d, u), UserData::None),
    FieldParse::end(),
];

/// The die mux table, on the `open` base.
pub(crate) static DIE_BASE_FIELD_PARSE: [FieldParse<TransportContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// `TransportContainModuleData::parseInitialPayload`: a name, then an optional count (`INI::scanInt`; 1 when there is none).
fn parse_initial_payload(ini: &mut Ini<'_>, d: &mut TransportContainModuleData, _u: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let count = match ini.get_next_token_or_null(None) {
        Some(c) => Ini::scan_int(&c)?,
        None => 1,
    };
    d.initial_payload.name = name;
    d.initial_payload.count = count;
    Ok(())
}

/// `TransportContainModuleData::buildFieldParse`'s own table (`dataFieldParse`).
pub(crate) static FIELD_PARSE: [FieldParse<TransportContainModuleData>; 15] = [
    FieldParse::new("Slots", |i, d, _| { d.slot_capacity = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScatterNearbyOnExit", |i, d, _| { d.scatter_nearby_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("OrientLikeContainerOnExit", |i, d, _| { d.orient_like_container_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("KeepContainerVelocityOnExit", |i, d, _| { d.keep_container_velocity_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("GoAggressiveOnExit", |i, d, _| { d.go_aggressive_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ResetMoodCheckTimeOnExit", |i, d, _| { d.reset_mood_check_time_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DestroyRidersWhoAreNotFreeToExit", |i, d, _| { d.destroy_riders_who_are_not_free_to_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitBone", |i, d, _| { d.exit_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitPitchRate", |i, d, _| { d.exit_pitch_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialPayload", parse_initial_payload, UserData::None),
    FieldParse::new("HealthRegen%PerSec", |i, d, _| { d.health_regen = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitDelay", |i, d, _| { d.exit_delay = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ArmedRidersUpgradeMyWeaponSet", |i, d, _| { d.armed_riders_upgrade_weapon_set = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DelayExitInAir", |i, d, _| { d.is_delay_exit_in_air = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl TransportContainModuleData {
    /// `TransportContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, TransportContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OPEN_BASE_FIELD_PARSE)?;
        p.add(&DIE_BASE_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<TransportContainModuleData> {
        let mut data = TransportContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, TransportContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `TransportContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::TransportContain(Arc::new(TransportContainModuleData::friend_new_module_data(ini)?)))
}

/// `class TransportContain : public OpenContain`.
#[derive(Clone, Debug)]
pub struct TransportContain {
    oc: OpenContain,
    /// `getTransportContainModuleData()`.
    data: Arc<TransportContainModuleData>,
    /// `m_payloadCreated` (protected; never set by the constructor, see the module docs).
    pub(crate) payload_created: bool,
    /// `m_extraSlotsInUse`: "transports have the ability to carry guys who take up more than one spot".
    pub(crate) extra_slots_in_use: i32,
    /// `m_frameExitNotBusy`.
    pub(crate) frame_exit_not_busy: u32,
}

impl PartialEq for TransportContain {
    /// The base's identity and state (see [`OpenContain`]'s `PartialEq`), and this class's three fields.
    fn eq(&self, other: &Self) -> bool {
        self.oc == other.oc
            && self.payload_created == other.payload_created
            && self.extra_slots_in_use == other.extra_slots_in_use
            && self.frame_exit_not_busy == other.frame_exit_not_busy
    }
}

impl TransportContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask()
    }

    /// The constructor's first half: the base (on a copy of its part of the data), `m_extraSlotsInUse = 0`, `m_frameExitNotBusy = 0`;
    /// `m_payloadCreated` is the pool's zero.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<TransportContainModuleData>>) -> IniResult<TransportContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(TransportContain { oc, data, payload_created: false, extra_slots_in_use: 0, frame_exit_not_busy: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<TransportContainModuleData> {
        &self.data
    }

    /// The base.
    pub fn open_contain(&self) -> &OpenContain {
        &self.oc
    }

    /// The base, to change.
    pub fn open_contain_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }

    /// The state `xfer` writes, as words: the base's, then `m_payloadCreated`, `m_extraSlotsInUse`, `m_frameExitNotBusy`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        v.push(u32::from(self.payload_created));
        v.push(self.extra_slots_in_use as u32);
        v.push(self.frame_exit_not_busy);
        v
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds (see [`TransportContain`]'s `Snapshot`).
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        // extend base class
        self.oc.xfer_with(update, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `crc` with the scheduling half the slot holds: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// The three fields `TransportContain::xfer` writes after the base: payload created, extra slots in use, frame exit not busy.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_bool(&mut self.payload_created)?;
        xfer.xfer_int(&mut self.extra_slots_in_use)?;
        xfer.xfer_unsigned_int(&mut self.frame_exit_not_busy)
    }
}

/// The virtuals `TransportContain` adds, which the classes derived from it override, and (`t_*`, batch R3) a hook for every `OpenContain` virtual that
/// `TransportContain` overrides or a derived class overrides: [`transport_open_contain_virtuals!`] routes the `OpenContainVirtuals` method to the hook, whose
/// default is `TransportContain`'s own, so a derived class overrides only what upstream's does. See the module docs.
pub trait TransportContainVirtuals: OpenContainVirtuals {
    /// The `TransportContain` this is (or is the base of).
    fn tc(&self) -> &TransportContain;
    /// The `TransportContain`, to change.
    fn tc_mut(&mut self) -> &mut TransportContain;
    /// `isSpecificRiderFreeToExit(obj)`.
    fn is_specific_rider_free_to_exit(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId) -> bool {
        tc_is_specific_rider_free_to_exit(self, world, me, obj)
    }
    /// `createPayload()`; `Err` for the throw of `newObject(NULL)`.
    fn create_payload(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        tc_create_payload(self, world, me)
    }
    // --- p1w13-sonnet --- (batch R3: the hooks of the derived transports)
    /// `isValidContainerFor(obj, checkCapacity)`.
    fn t_is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, check_capacity: bool) -> bool {
        tc_is_valid_container_for(self, world, me, obj, check_capacity)
    }
    /// `onCapture(oldOwner, newOwner)`.
    fn t_on_capture(&mut self, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
        tc_on_capture(self, world, me, old_owner, new_owner)
    }
    /// `onContaining(rider, wasSelected)`.
    fn t_on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        tc_on_containing(self, world, me, rider, was_selected)
    }
    /// `onRemoving(rider)`.
    fn t_on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        tc_on_removing(self, world, me, rider)
    }
    /// `update()`.
    fn t_update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        tc_update(self, world, me)
    }
    /// `isRiderChangeContain()`.
    fn t_is_rider_change_contain(&self) -> bool {
        false
    }
    /// `isSpecialOverlordStyleContainer()`.
    fn t_is_special_overlord_style_container(&self) -> bool {
        false
    }
    /// `getContainMax()`.
    fn t_get_contain_max(&mut self) -> i32 {
        tc_get_contain_max(self)
    }
    /// `getExtraSlotsInUse()`.
    fn t_get_extra_slots_in_use(&mut self) -> i32 {
        self.tc().extra_slots_in_use
    }
    /// `isExitBusy()`.
    fn t_is_exit_busy(&mut self, world: &mut World, me: ModuleHandle) -> bool {
        tc_is_exit_busy(self, world, me)
    }
    /// `reserveDoorForExit(objType, specificObject)`.
    fn t_reserve_door_for_exit(&mut self, world: &mut World, me: ModuleHandle, specific: ObjectId) -> ExitDoorType {
        tc_reserve_door_for_exit(self, world, me, specific)
    }
    /// `isDisplayedOnControlBar()`.
    fn t_is_displayed_on_control_bar(&self) -> bool {
        true
    }
    /// `killRidersWhoAreNotFreeToExit()`.
    fn t_kill_riders_who_are_not_free_to_exit(&mut self, world: &mut World, me: ModuleHandle) {
        tc_kill_riders_who_are_not_free_to_exit(self, world, me)
    }
    /// `isPassengerAllowedToFire(id)`.
    fn t_is_passenger_allowed_to_fire(&mut self, world: &mut World, me: ModuleHandle, id: ObjectId) -> bool {
        tc_is_passenger_allowed_to_fire(self, world, me, id)
    }
    /// `exitObjectViaDoor(exitObj, exitDoor)`: `OpenContain`'s.
    fn t_exit_object_via_door(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, exit_door: ExitDoorType) {
        open_contain::base_exit_object_via_door(self, world, me, exit_obj, exit_door);
    }
    // batch R4 (HelixContain): the `OpenContain` virtuals the overlord-style containers override, each defaulting to `OpenContain`'s
    /// `addToContain(rider)`.
    fn t_add_to_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_add_to_contain(self, world, me, rider);
    }
    /// `addToContainList(rider)`.
    fn t_add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_add_to_contain_list(self, world, me, rider);
    }
    /// `removeFromContain(rider, exposeStealthUnits)`.
    fn t_remove_from_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, expose_stealth_units: bool) {
        open_contain::base_remove_from_contain(self, world, me, rider, expose_stealth_units);
    }
    /// `isEnclosingContainerFor(obj)`: `TRUE`.
    fn t_is_enclosing_container_for(&mut self, _world: &mut World, _me: ModuleHandle, _obj: ObjectId) -> bool {
        true
    }
    /// `friend_getRider()`: `NULL`.
    fn t_friend_get_rider(&self, _world: &World) -> ObjectId {
        INVALID_ID
    }
    /// `redeployOccupants()`.
    fn t_redeploy_occupants(&mut self, world: &mut World, me: ModuleHandle) {
        open_contain::base_redeploy_occupants(self, world, me);
    }
    /// `onDie(damageInfo)`.
    fn t_on_die(&mut self, world: &mut World, me: ModuleHandle, info: &crate::logic::damage::DamageInfo) {
        open_contain::base_on_die(self, world, me, info);
    }
    /// `onDelete()`.
    fn t_on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        open_contain::base_on_delete(self, world, me);
    }
    /// A call no `OpenContain` virtual answers: `isSpecificRiderFreeToExit`, else unhandled.
    fn t_other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::IsSpecificRiderFreeToExit { obj } => ModuleReply::Bool(self.is_specific_rider_free_to_exit(world, me, obj)),
            _ => ModuleReply::Unhandled,
        }
    }
    /// `onObjectCreated()`: nothing.
    fn t_on_object_created(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// `onBodyDamageStateChange(damageInfo, oldState, newState)`: nothing.
    fn t_on_body_damage_state_change(&mut self, _world: &mut World, _me: ModuleHandle, _info: &crate::logic::damage::DamageInfo, _old_state: i32, _new_state: i32) {}
    // --- end p1w13-sonnet ---
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// `getContainMax()`: the slot capacity.
pub fn tc_get_contain_max<T: TransportContainVirtuals>(t: &T) -> i32 {
    t.tc().data.slot_capacity
}

/// `TransportContain::isValidContainerFor(rider, checkCapacity)`.
pub fn tc_is_valid_container_for<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId, check_capacity: bool) -> bool {
    // sanity
    if rider == INVALID_ID || world.objects.find(rider).is_none() {
        return false;
    }
    let mut rider = rider;

    // "when we pass this object to contain in a transport plane, we want to check the infantry, not the parachute"
    if let Some(contain) = world.get_contain(rider) {
        if out(t, world, me, |w| w.module_call(rider, contain, ModuleCall::IsSpecialZeroSlotContainer)).as_bool() {
            if let ModuleReply::Ids(items) = out(t, world, me, |w| w.module_call(rider, contain, ModuleCall::GetContainedItems)) {
                // replace the object we are checking with the *first* object contained within it
                if let Some(&front) = items.first() {
                    if front != INVALID_ID && !crate::logic::modules::plant(1411) {
                        rider = front;
                    }
                }
            }
        }
    }

    // extend functionality
    if !open_contain::base_is_valid_container_for(t, world, me, rider, check_capacity) {
        return false;
    }

    // "no... actually, only OUR OWN units can be transported"
    if (world.get_controlling_player(rider) != world.get_controlling_player(me.object)) != crate::logic::modules::plant(1412) {
        return false;
    }

    let transport_slot_count = out(t, world, me, |w| w.get_transport_slot_count(rider));
    // if 0, this object isn't transportable
    if transport_slot_count == 0 {
        return false;
    }

    if check_capacity {
        let contain_max = t.get_contain_max();
        let contain_count = t.get_contain_count() as i32;
        if crate::logic::modules::plant(1413) {
            return contain_count + transport_slot_count <= contain_max;
        }
        t.tc().extra_slots_in_use + contain_count + transport_slot_count <= contain_max
    } else {
        true
    }
}

/// `TransportContain::letRidersUpgradeWeaponSet`: the transport takes `WEAPONSET_PLAYER_UPGRADE` while any infantry rider has a weapon that is not a contact
/// weapon and does damage.
pub fn tc_let_riders_upgrade_weapon_set<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    if !t.tc().data.armed_riders_upgrade_weapon_set {
        return;
    }
    let us = me.object;
    let mut any_rider_has_viable_weapon = false;
    let riders = t.get_contained_items_list();
    for rider in riders {
        if world.objects.find(rider).is_none_or(|o| !o.template().is_kind_of(KINDOF_INFANTRY)) {
            continue;
        }
        for w in 0..WEAPONSLOT_COUNT {
            if let Some(f) = world.weapon_slot_facts(rider, w) {
                // "THIS MAY NEED TO CHECK MORE WEAPON ATTRIBUTES TO WORK BEST"
                if !f.is_contact_weapon && (f.is_damage_weapon != crate::logic::modules::plant(1414)) {
                    any_rider_has_viable_weapon = true;
                    break;
                }
            }
        }
    }
    // upstream: `while( *it )` reads `end()` after the last rider
    undefined(world, me, Undefined::EndIteratorDereferenced);

    if any_rider_has_viable_weapon {
        out(t, world, me, |w| w.set_weapon_set_flag(us, WEAPONSET_PLAYER_UPGRADE));
    } else if !crate::logic::modules::plant(1415) {
        out(t, world, me, |w| w.clear_weapon_set_flag(us, WEAPONSET_PLAYER_UPGRADE));
    }
}

/// Whether the Patch 1.01 Jarmen Kell hack applies to this transport and rider.
fn is_jarmen_kell(world: &World, transport: ObjectId, rider: ObjectId) -> bool {
    let (Some(t), Some(r)) = (world.objects.find(transport), world.objects.find(rider)) else { return false };
    t.template().is_kind_of(KINDOF_CLIFF_JUMPER) && r.template().is_kind_of(KINDOF_HERO) && r.template().is_kind_of(KINDOF_SALVAGER)
}

/// `TransportContain::onContaining(rider, wasSelected)`.
pub fn tc_on_containing<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
    open_contain::base_on_containing(t, world, me, rider, was_selected);

    // objects inside a transport are held
    if !crate::logic::modules::plant(1416) {
        out(t, world, me, |w| w.contain_set_disabled(rider, DISABLED_HELD));
    }

    let transport_slot_count = out(t, world, me, |w| w.get_transport_slot_count(rider));
    t.tc_mut().extra_slots_in_use += transport_slot_count - 1;

    // when we go from holding nothing to holding something we have a model condition to visually show the change
    if t.get_contain_count() == 1 {
        out(t, world, me, |w| w.contain_drawable_model_condition(me.object, MODELCONDITION_LOADED, true));
    }

    if t.tc().data.armed_riders_upgrade_weapon_set {
        tc_let_riders_upgrade_weapon_set(t, world, me);
    }

    // Patch 1.01: "Force Jarmen Kell to transfer weapon timer for snipe to and from the combat bike." Rider to bike.
    if is_jarmen_kell(world, me.object, rider) && !crate::logic::modules::plant(1417) {
        out(t, world, me, |w| w.weapon_transfer_next_shot_stats(me.object, SECONDARY_WEAPON, rider, SECONDARY_WEAPON));
    }
}

/// `TransportContain::onRemoving(rider)`.
pub fn tc_on_removing<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, rider: ObjectId) {
    open_contain::base_on_removing(t, world, me, rider);

    // object is no longer held inside a transport
    out(t, world, me, |w| w.contain_clear_disabled(rider, DISABLED_HELD));

    let d = t.tc().data.clone();
    let us = me.object;
    if !d.exit_bone.is_empty() {
        if let Some(world_pos) = out(t, world, me, |w| w.contain_exit_bone_world_position(us, &d.exit_bone)) {
            out(t, world, me, |w| w.contain_set_position(rider, &world_pos));
        }
    }

    if d.orient_like_container_on_exit != crate::logic::modules::plant(1418) {
        let angle = world.objects.find(us).map(|o| o.thing.orientation()).unwrap_or(0.0);
        out(t, world, me, |w| w.contain_set_orientation(rider, angle));
    }

    if d.keep_container_velocity_on_exit {
        let parent = world.objects.find(us).and_then(|o| o.physics);
        let child = world.objects.find(rider).and_then(|o| o.physics);
        if let (Some(parent), Some(child)) = (parent, child) {
            let mut starting_force = out(t, world, me, |w| w.module_call(us, parent, ModuleCall::PhysicsGetVelocity)).as_coord();
            let mass = out(t, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsGetMass)).as_real(0.0);
            starting_force.x *= mass;
            starting_force.y *= mass;
            if !crate::logic::modules::plant(1419) {
                starting_force.z *= mass;
            }
            out(t, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsApplyMotiveForce(starting_force)));

            let com = out(t, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsGetCenterOfMassOffset)).as_real(0.0);
            let pitch_rate = com * d.exit_pitch_rate;
            out(t, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsSetPitchRate(pitch_rate)));
        }
    }

    let transport_slot_count = out(t, world, me, |w| w.get_transport_slot_count(rider));
    t.tc_mut().extra_slots_in_use -= transport_slot_count - 1;

    // when we are empty again, clear the model condition for loaded
    if t.get_contain_count() == 0 {
        out(t, world, me, |w| w.contain_drawable_model_condition(us, MODELCONDITION_LOADED, false));
    }

    if out(t, world, me, |w| w.is_above_terrain(us)) != crate::logic::modules::plant(1420) {
        // "temporarily mark the guy as being allowed to fall (overriding his locomotor's stick-to-ground attribute)"
        if let Some(physics) = world.objects.find(rider).and_then(|o| o.physics) {
            out(t, world, me, |w| w.module_call(rider, physics, ModuleCall::PhysicsSetAllowToFall(true)));
        }
    }

    // "AI might need help using this transport in a good way. Make the passengers aggressive."
    let rider_ai = world.objects.find(rider).and_then(|o| o.ai);
    if d.go_aggressive_on_exit {
        if let Some(ai) = rider_ai {
            out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiSetAttitude(AI_AGGRESSIVE)));
        }
    }

    if world.objects.find(us).is_some_and(|o| o.is_effectively_dead()) != crate::logic::modules::plant(1421) {
        open_contain::scatter_to_nearby_position(t, world, me, rider);
    }

    if d.reset_mood_check_time_on_exit {
        if let Some(ai) = world.objects.find(rider).and_then(|o| o.ai) {
            out(t, world, me, |w| w.module_call(rider, ai, ModuleCall::AiWakeUpAndAttemptToTarget));
        }
    }

    t.tc_mut().frame_exit_not_busy = world.frame().wrapping_add(if crate::logic::modules::plant(1422) { 0 } else { d.exit_delay });

    if d.armed_riders_upgrade_weapon_set {
        tc_let_riders_upgrade_weapon_set(t, world, me);
    }

    // Patch 1.01, the way out: bike to rider
    if is_jarmen_kell(world, us, rider) {
        if crate::logic::modules::plant(1423) {
            out(t, world, me, |w| w.weapon_transfer_next_shot_stats(us, SECONDARY_WEAPON, rider, SECONDARY_WEAPON));
        } else {
            out(t, world, me, |w| w.weapon_transfer_next_shot_stats(rider, SECONDARY_WEAPON, us, SECONDARY_WEAPON));
        }
    }
}

/// `TransportContain::createPayload()`.
pub fn tc_create_payload<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
    let d = t.tc().data.clone();
    let count = d.initial_payload.count;
    let payload_template = world.thing_factory_find_template(&d.initial_payload.name);
    let us = me.object;

    // `object->getContain()` is this module (a transport's contain); the calls go through the interface, so they reach any override
    open_contain::enable_load_sounds(t, false);
    for _ in 0..count {
        // "We are creating a transport that comes with a initial payload, so add it now!"
        let Some(player) = world.get_controlling_player(us) else {
            undefined(world, me, Undefined::NullControllingPlayer);
            return Ok(());
        };
        let team = world.cuts.player_default_team(player);
        let Some(template) = payload_template else {
            // `newObject(NULL)` throws ERROR_BAD_ARG out of update: the load sounds stay off, the payload is not marked created
            return Err(IniError::new(IniErrorKind::BadArg));
        };
        let payload = out(t, world, me, |w| w.contain_new_payload_object(template, team, ObjectStatusMaskType::new()));
        if t.is_valid_container_for(world, me, payload, true) && !crate::logic::modules::plant(1424) {
            t.add_to_contain(world, me, payload);
        }
        // else DEBUG_CRASH: "PutInContainer is full, or not valid for the payload" (nothing in a release build)
    }
    open_contain::enable_load_sounds(t, true);
    t.tc_mut().payload_created = true;
    Ok(())
}

/// `TransportContain::update()`: the payload on the first call, the riders' healing, then the base's.
pub fn tc_update<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
    let d = t.tc().data.clone();
    if !t.tc().payload_created {
        if let Err(e) = t.create_payload(world, me) {
            world.cuts.module_errors.push((me, e));
            return crate::logic::module::update_module::UPDATE_SLEEP_NONE;
        }
    }

    if d.health_regen != 0.0 && !crate::logic::modules::plant(1425) {
        // "This transport has a health regeneration value, so go through and heal all inside."
        let us = me.object;
        let items = t.get_contained_items_list();
        for object in items {
            // "Determine if we need healing or not."
            let Some((health, max)) = out(t, world, me, |w| w.body_health(object)) else {
                undefined(world, me, Undefined::NullBodyModule);
                return open_contain::base_update(t, world, me);
            };
            if health < max || crate::logic::modules::plant(1426) {
                // "Calculate the health to be regenerated on each unit."
                let regen = max * d.health_regen / 100.0 * SECONDS_PER_LOGICFRAME_REAL;
                out(t, world, me, |w| w.attempt_healing(object, regen, us));
            }
        }
        // upstream: `while( *it )` reads `end()` after the last rider
        undefined(world, me, Undefined::EndIteratorDereferenced);
    }

    // extend
    open_contain::base_update(t, world, me)
}

/// `TransportContain::killRidersWhoAreNotFreeToExit`: destroyed or killed, by the data's choice, before `removeAllContained` forces them out.
pub fn tc_kill_riders_who_are_not_free_to_exit<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) {
    let d = t.tc().data.clone();
    // "increment the iterator, since death will pull this guy from the list somewhere else": a snapshot of the module's own list
    let list: Vec<ObjectId> = t.oc().contain_list().to_vec();
    for obj in list {
        if !t.is_specific_rider_free_to_exit(world, me, obj) {
            if d.destroy_riders_who_are_not_free_to_exit != crate::logic::modules::plant(1427) {
                out(t, world, me, |w| w.contain_destroy_object(obj));
            } else if !out(t, world, me, |w| w.contain_kill(obj)) {
                undefined(world, me, Undefined::NullBodyModule); // `Object::kill` calls through `getBodyModule()`
            }
        }
    }
}

/// `TransportContain::isSpecificRiderFreeToExit(specificObject)`: "an override, not an extend".
pub fn tc_is_specific_rider_free_to_exit<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, specific: ObjectId) -> bool {
    if specific == INVALID_ID {
        return true; // "I can, in general, exit people."
    }
    let us = me.object;

    // "present solely for some transports to override, so that they can land before allowing people to exit"
    if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
        let free = out(t, world, me, |w| w.module_call(us, ai, ModuleCall::AiGetFreeToExit { exiter: specific })).as_int(FREE_TO_EXIT);
        if (free != FREE_TO_EXIT) != crate::logic::modules::plant(1428) {
            return false;
        }
    }

    // "I can always kick people out if I am in the air, I know what I'm doing"
    if world.is_using_airborne_locomotor(us) {
        return true;
    }

    let Some(his_ai) = world.objects.find(specific).and_then(|o| o.ai) else {
        return false;
    };
    if !out(t, world, me, |w| w.module_call(specific, his_ai, ModuleCall::AiHasCurLocomotor)).as_bool() {
        return false;
    }

    // "He can't get to this spot naturally, so I can't force him there. (amphib transport)"
    let my_position = world.objects.find(us).map(|o| o.position).unwrap_or_default();
    if !out(t, world, me, |w| w.contain_valid_movement_terrain(us, specific, &my_position)) && !crate::logic::modules::plant(1429) {
        return false;
    }
    true
}

/// `TransportContain::isPassengerAllowedToFire(id)`: infantry only, and the rule of an overlord-style container that holds this transport.
pub fn tc_is_passenger_allowed_to_fire<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, id: ObjectId) -> bool {
    if let Some(passenger) = world.objects.find(id) {
        // "only allow infantry, and turrets and such. no vehicles."
        if !passenger.template().is_kind_of(KINDOF_INFANTRY) != crate::logic::modules::plant(1430) {
            return false;
        }
    }

    // "but wait! I may be riding on an Overlord"
    let he_who_contains_me = world.objects.find(me.object).map_or(INVALID_ID, |o| o.contained_by);
    if he_who_contains_me != INVALID_ID {
        if let Some(his_contain) = world.get_contain(he_who_contains_me) {
            if out(t, world, me, |w| w.module_call(he_who_contains_me, his_contain, ModuleCall::IsSpecialOverlordStyleContainer)).as_bool() {
                return out(t, world, me, |w| w.module_call(he_who_contains_me, his_contain, ModuleCall::IsPassengerAllowedToFire(id))).as_bool();
            }
        }
    }

    open_contain::base_is_passenger_allowed_to_fire(t, world, me, INVALID_ID)
}

/// `TransportContain::reserveDoorForExit(objType, specificObject)`.
pub fn tc_reserve_door_for_exit<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, specific: ObjectId) -> ExitDoorType {
    if t.is_specific_rider_free_to_exit(world, me, specific) {
        DOOR_1
    } else {
        DOOR_NONE_AVAILABLE
    }
}

/// `TransportContain::isExitBusy()`.
pub fn tc_is_exit_busy<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle) -> bool {
    if t.tc().data.is_delay_exit_in_air && out(t, world, me, |w| w.is_above_terrain(me.object)) {
        return true;
    }
    if crate::logic::modules::plant(1431) {
        return world.frame() <= t.tc().frame_exit_not_busy;
    }
    world.frame() < t.tc().frame_exit_not_busy
}

/// `TransportContain::onCapture(oldOwner, newOwner)`: everybody out.
pub fn tc_on_capture<T: TransportContainVirtuals>(t: &mut T, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
    if old_owner != new_owner {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled_by_type(DISABLED_UNMANNED)) != crate::logic::modules::plant(1432) {
            // "If this vehicle was sniped, then instantly eject everyone (otherwise, the next guy to eject will recapture it)."
            t.remove_all_contained(world, me, false);
        } else {
            t.order_all_passengers_to_exit(world, me, CMD_FROM_AI, false);
        }
    }
}

// --- p1w13-sonnet --- (batch R3)
/// `impl OpenContainVirtuals` for `TransportContain` and the classes derived from it: the base and the slot variant, and every virtual that
/// `TransportContain` (or a derived class) overrides routed to its [`TransportContainVirtuals`] hook.
macro_rules! transport_open_contain_virtuals {
    ($ty:ty, $variant:ident) => {
        impl $crate::logic::modules::open_contain::OpenContainVirtuals for $ty {
            fn oc(&self) -> &$crate::logic::modules::open_contain::OpenContain {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::tc(self).open_contain()
            }
            fn oc_mut(&mut self) -> &mut $crate::logic::modules::open_contain::OpenContain {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::tc_mut(self).open_contain_mut()
            }
            fn into_body(self) -> $crate::logic::world::update_body::UpdateBody {
                $crate::logic::world::update_body::UpdateBody::$variant(self)
            }
            fn from_body(b: $crate::logic::world::update_body::UpdateBody) -> Option<Self> {
                match b {
                    $crate::logic::world::update_body::UpdateBody::$variant(m) => Some(m),
                    _ => None,
                }
            }
            fn is_valid_container_for(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, obj: $crate::common::game_type::ObjectId, check_capacity: bool) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_valid_container_for(self, world, me, obj, check_capacity)
            }
            fn on_capture(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, old_owner: i32, new_owner: i32) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_capture(self, world, me, old_owner, new_owner)
            }
            fn on_containing(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, rider: $crate::common::game_type::ObjectId, was_selected: bool) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_containing(self, world, me, rider, was_selected)
            }
            fn on_removing(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, rider: $crate::common::game_type::ObjectId) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_removing(self, world, me, rider)
            }
            fn update(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) -> $crate::logic::module::update_module::UpdateSleepTime {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_update(self, world, me)
            }
            fn is_rider_change_contain(&self) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_rider_change_contain(self)
            }
            fn is_special_overlord_style_container(&self) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_special_overlord_style_container(self)
            }
            fn get_contain_max(&mut self) -> i32 {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_get_contain_max(self)
            }
            fn get_extra_slots_in_use(&mut self) -> i32 {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_get_extra_slots_in_use(self)
            }
            fn is_exit_busy(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_exit_busy(self, world, me)
            }
            fn reserve_door_for_exit(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, specific: $crate::common::game_type::ObjectId) -> $crate::logic::module::update_module::ExitDoorType {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_reserve_door_for_exit(self, world, me, specific)
            }
            fn is_displayed_on_control_bar(&self) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_displayed_on_control_bar(self)
            }
            fn kill_riders_who_are_not_free_to_exit(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_kill_riders_who_are_not_free_to_exit(self, world, me)
            }
            fn is_passenger_allowed_to_fire(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, id: $crate::common::game_type::ObjectId) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_passenger_allowed_to_fire(self, world, me, id)
            }
            fn add_to_contain(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, rider: $crate::common::game_type::ObjectId) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_add_to_contain(self, world, me, rider)
            }
            fn add_to_contain_list(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, rider: $crate::common::game_type::ObjectId) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_add_to_contain_list(self, world, me, rider)
            }
            fn remove_from_contain(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, rider: $crate::common::game_type::ObjectId, expose_stealth_units: bool) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_remove_from_contain(self, world, me, rider, expose_stealth_units)
            }
            fn is_enclosing_container_for(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, obj: $crate::common::game_type::ObjectId) -> bool {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_is_enclosing_container_for(self, world, me, obj)
            }
            fn friend_get_rider(&self, world: &$crate::logic::world::game_logic::World) -> $crate::common::game_type::ObjectId {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_friend_get_rider(self, world)
            }
            fn redeploy_occupants(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_redeploy_occupants(self, world, me)
            }
            fn on_die(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, info: &$crate::logic::damage::DamageInfo) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_die(self, world, me, info)
            }
            fn on_delete(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_delete(self, world, me)
            }
            fn on_object_created(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_object_created(self, world, me)
            }
            fn on_body_damage_state_change(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, info: &$crate::logic::damage::DamageInfo, old_state: i32, new_state: i32) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_on_body_damage_state_change(self, world, me, info, old_state, new_state)
            }
            fn exit_object_via_door(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, exit_obj: $crate::common::game_type::ObjectId, exit_door: $crate::logic::module::update_module::ExitDoorType) {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_exit_object_via_door(self, world, me, exit_obj, exit_door)
            }
            fn other_call(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, call: $crate::logic::world::module_call::ModuleCall) -> $crate::logic::world::module_call::ModuleReply {
                $crate::logic::modules::transport_contain::TransportContainVirtuals::t_other_call(self, world, me, call)
            }
        }
    };
}
pub(crate) use transport_open_contain_virtuals;

/// The rest of a class derived from `TransportContain` with no fields of its own (it holds the base as `tc`): construction, the slot's entry points,
/// `UpdateBehavior`, and `Snapshot` (version 1, then "extend base class", as every derived transport's `xfer` is).
macro_rules! derived_transport_contain_glue {
    ($ty:ident, $variant:ident) => {
        impl $ty {
            /// `getInterfaceMask()`: the base's.
            pub const fn get_interface_mask() -> i32 {
                $crate::logic::modules::transport_contain::TransportContain::get_interface_mask()
            }
            /// The constructor's first half: the base's.
            pub fn unconstructed(
                object: $crate::common::game_type::ObjectId,
                module_data: Option<std::sync::Arc<$crate::logic::modules::transport_contain::TransportContainModuleData>>,
            ) -> $crate::common::ini::IniResult<$ty> {
                Ok($ty { tc: $crate::logic::modules::transport_contain::TransportContain::unconstructed(object, module_data)? })
            }
            /// The `UpdateModule` base, for the scheduler's bookkeeping.
            pub fn update_module(&self) -> &$crate::logic::module::update_module::UpdateModule {
                self.tc.update_module()
            }
            /// The module data.
            pub fn module_data(&self) -> &std::sync::Arc<$crate::logic::modules::transport_contain::TransportContainModuleData> {
                self.tc.module_data()
            }
            /// The `TransportContain` base.
            pub fn transport_contain(&self) -> &$crate::logic::modules::transport_contain::TransportContain {
                &self.tc
            }
            /// The state `xfer` writes, as words: the base's (no fields of its own).
            pub fn state_words(&self) -> Vec<u32> {
                self.tc.state_words()
            }
            /// The constructor's second half: the base's.
            pub fn world_construct(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) {
                self.tc.world_construct(world, me);
            }
            /// `update()` as the scheduler runs it.
            pub fn world_update(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::sleepy::UpdateHandle) -> $crate::logic::module::update_module::UpdateSleepTime {
                $crate::logic::modules::update_behavior::world_update(self, world, me)
            }
            /// The module's answer to one of `Object`'s calls.
            pub fn module_call(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle, call: $crate::logic::world::module_call::ModuleCall) -> $crate::logic::world::module_call::ModuleReply {
                $crate::logic::modules::open_contain::contain_module_call(self, world, me, call)
            }
            /// `xfer` with the scheduling half the slot holds: the version, then the base's.
            pub fn xfer_with(
                &mut self,
                update: &mut $crate::logic::module::update_module::UpdateModule,
                xfer: &mut dyn $crate::common::xfer::Xfer,
            ) -> $crate::common::xfer::XferResult<()> {
                let current: $crate::common::xfer::XferVersion = 1;
                let mut version = current;
                xfer.xfer_version(&mut version, current)?;
                self.tc.xfer_with(update, xfer)
            }
            /// `crc` with the scheduling half: "extend base class".
            pub fn crc_with(
                &mut self,
                update: &mut $crate::logic::module::update_module::UpdateModule,
                xfer: &mut dyn $crate::common::xfer::Xfer,
            ) -> $crate::common::xfer::XferResult<()> {
                self.tc.crc_with(update, xfer)
            }
            /// `loadPostProcess` with the scheduling half: "extend base class".
            pub fn load_post_process_with(&mut self, update: &mut $crate::logic::module::update_module::UpdateModule) {
                self.tc.load_post_process_with(update);
            }
        }

        impl $crate::logic::modules::update_behavior::UpdateBehavior for $ty {
            const XFER_VERSION: $crate::common::xfer::XferVersion = 1;
            fn update_base(&self) -> &$crate::logic::module::update_module::UpdateModule {
                self.tc.update_module()
            }
            fn update(&mut self, world: &mut $crate::logic::world::game_logic::World, me: $crate::logic::world::module_call::ModuleHandle) -> $crate::logic::module::update_module::UpdateSleepTime {
                $crate::logic::modules::open_contain::OpenContainVirtuals::update(self, world, me)
            }
            fn into_body(self) -> $crate::logic::world::update_body::UpdateBody {
                $crate::logic::world::update_body::UpdateBody::$variant(self)
            }
            fn from_body(body: $crate::logic::world::update_body::UpdateBody) -> Option<Self> {
                match body {
                    $crate::logic::world::update_body::UpdateBody::$variant(m) => Some(m),
                    _ => None,
                }
            }
            fn other_call(&mut self, _world: &mut $crate::logic::world::game_logic::World, _me: $crate::logic::world::module_call::ModuleHandle, _call: $crate::logic::world::module_call::ModuleCall) -> $crate::logic::world::module_call::ModuleReply {
                $crate::logic::world::module_call::ModuleReply::Unhandled
            }
        }

        impl $crate::common::xfer::Snapshot for $ty {
            /// `crc`: "extend base class".
            fn crc(&mut self, xfer: &mut dyn $crate::common::xfer::Xfer) -> $crate::common::xfer::XferResult<()> {
                $crate::common::xfer::Snapshot::crc(&mut self.tc, xfer)
            }
            /// `xfer` (version 1): the version, then `TransportContain::xfer`.
            fn xfer(&mut self, xfer: &mut dyn $crate::common::xfer::Xfer) -> $crate::common::xfer::XferResult<()> {
                let current: $crate::common::xfer::XferVersion = 1;
                let mut version = current;
                xfer.xfer_version(&mut version, current)?;
                $crate::common::xfer::Snapshot::xfer(&mut self.tc, xfer)
            }
            /// `loadPostProcess`: "extend base class".
            fn load_post_process(&mut self) {
                $crate::common::xfer::Snapshot::load_post_process(&mut self.tc);
            }
        }
    };
}
pub(crate) use derived_transport_contain_glue;

transport_open_contain_virtuals!(TransportContain, TransportContain);
// --- end p1w13-sonnet ---

impl TransportContainVirtuals for TransportContain {
    fn tc(&self) -> &TransportContain {
        self
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        self
    }
}

impl UpdateBehavior for TransportContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::TransportContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::TransportContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for TransportContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc(xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then the three fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.oc.xfer(xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        self.oc.load_post_process();
    }
}
