// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RiderChangeContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/RiderChangeContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RiderChangeContain`: the GLA combat bike. A [`TransportContain`] whose look, weapon set, status, command set and locomotor depend on which of up
//! to eight kinds of rider is on it; a new rider throws the old one off, the bike takes the rider's veterancy (and gives it back), and a bike whose
//! rider leaves is scuttled: unselectable, toppled after `ScuttleDelay`.
//!
//! # Design
//!
//! As every class derived from `TransportContain` (see that module), with data and state of its own. The riders are matched with
//! [`World::template_is_equivalent_to_object`] (`ThingTemplate::isEquivalentTo` against the factory's template of that name). The selection messages
//! and the in-game UI calls are client events; the experience trackers, the second `kill` and the status changes are `contain_cuts` calls.
//!
//! # Upstream facts kept
//!
//! - **The class declares its own `m_extraSlotsInUse` and `m_frameExitNotBusy`**, private, which hide `TransportContain`'s protected ones. Its
//!   constructor zeroes its own; `TransportContain`'s code keeps using the base's. So `getExtraSlotsInUse` (overridden here) answers this class's copy,
//!   which nothing changes (always 0), while the slot count the base adds and subtracts lives in the base.
//! - **`xfer` writes `m_payloadCreated` twice**: `TransportContain::xfer` (which writes it and the base's two counters), then `m_payloadCreated` again and
//!   this class's two copies. `m_scuttledOnFrame` and `m_containing` are not saved.
//! - `isValidContainerFor` asks the base without the capacity check ("our rider will kick the other rider out"), refuses a scuttled bike, then looks
//!   the eight rider names up: a name with no template is `NULL`, whose `isEquivalentTo` answers `false` (the function checks `this`).
//! - `onContaining` with the payload created evacuates the current rider through `getAI()` unchecked (a bike with no AI is undefined:
//!   [`Undefined::NullAi`]); the rider's veterancy goes to the bike and the rider's experience to 0. `onRemoving` of a rider from a bike that is
//!   effectively dead destroys the rider; with the payload not yet created it skips the base's `onRemoving`.
//! - Scuttling happens only when both bike and rider have drawables (it sits inside that test), and reads `getAI()->isMoving()` unchecked.
//! - `isExitBusy` is `FALSE` (the base's is commented out); `getContainerPipsToShow` is the control bar's, not translated.
//!
//! `p1w13-sonnet`, batch R5, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_TOPPLED, MODEL_CONDITION_NAMES};
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_IMMOBILE, OBJECT_STATUS_NAMES, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNSELECTABLE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DAMAGE_UNRESISTABLE;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime};
use crate::logic::modules::open_contain::{self, out, OpenContainVirtuals};
use crate::logic::modules::transport_contain::{
    self, tc_is_valid_container_for, tc_on_containing, tc_on_removing, tc_update, transport_open_contain_virtuals, TransportContain,
    TransportContainModuleData, TransportContainVirtuals,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_set_flags::WEAPON_SET_NAMES;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_RIDERS`.
pub const MAX_RIDERS: usize = 8;
/// `DEATH_TOPPLED` (`DeathType`).
const DEATH_TOPPLED: i32 = 6;

/// `TheLocomotorSetNames[]` (AIUpdate.h, under `DEFINE_LOCOMOTORSET_NAMES`), without the `NULL`.
pub const LOCOMOTOR_SET_NAMES: [&str; 8] = crate::logic::ai::ai_update_types::LOCOMOTOR_SET_NAMES;

/// `struct RiderInfo`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiderInfo {
    /// `m_templateName`.
    pub template_name: Vec<u8>,
    /// `m_weaponSetFlag` (`WeaponSetType`).
    pub weapon_set_flag: i32,
    /// `m_modelConditionFlagType`.
    pub model_condition_flag_type: i32,
    /// `m_objectStatusType`.
    pub object_status_type: i32,
    /// `m_commandSet`.
    pub command_set: Vec<u8>,
    /// `m_locomotorSetType`.
    pub locomotor_set_type: i32,
}

/// `class RiderChangeContainModuleData : public TransportContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct RiderChangeContainModuleData {
    /// The `TransportContainModuleData` base.
    pub transport: TransportContainModuleData,
    /// `m_riders[MAX_RIDERS]` (the constructor leaves them; a rider no line names has no template name, and is never matched).
    pub riders: [RiderInfo; MAX_RIDERS],
    /// `m_scuttleFrames` (`ScuttleDelay`).
    pub scuttle_frames: u32,
    /// `m_scuttleState` (`ScuttleStatus`).
    pub scuttle_state: i32,
}

impl Default for RiderChangeContainModuleData {
    /// `RiderChangeContainModuleData()`: no scuttle delay, `MODELCONDITION_TOPPLED`.
    fn default() -> Self {
        RiderChangeContainModuleData {
            transport: TransportContainModuleData::default(),
            riders: Default::default(),
            scuttle_frames: 0,
            scuttle_state: MODELCONDITION_TOPPLED,
        }
    }
}

/// `parseRiderInfo`: the template name, the model condition, the weapon set, the object status (three `parseIndexList`s), the command set, the
/// locomotor set (`scanIndexList` of `TheLocomotorSetNames`).
fn parse_rider_info(ini: &mut Ini<'_>, rider: &mut RiderInfo) -> IniResult<()> {
    rider.template_name = ini.get_next_token(None)?;
    rider.model_condition_flag_type = ini.parse_index_list(&MODEL_CONDITION_NAMES)?;
    rider.weapon_set_flag = ini.parse_index_list(&WEAPON_SET_NAMES)?;
    rider.object_status_type = ini.parse_index_list(&OBJECT_STATUS_NAMES)?;
    rider.command_set = ini.get_next_token(None)?;
    rider.locomotor_set_type = Ini::scan_index_list(&ini.get_next_token(None)?, &LOCOMOTOR_SET_NAMES)?;
    Ok(())
}

fn lift_open(k: usize, i: &mut Ini<'_>, d: &mut RiderChangeContainModuleData, u: &UserData) -> IniResult<()> {
    match transport_contain::OPEN_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

fn lift_die(k: usize, i: &mut Ini<'_>, d: &mut RiderChangeContainModuleData, u: &UserData) -> IniResult<()> {
    match transport_contain::DIE_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

fn lift_transport(k: usize, i: &mut Ini<'_>, d: &mut RiderChangeContainModuleData, u: &UserData) -> IniResult<()> {
    match transport_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<RiderChangeContainModuleData>; 16] = [
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

static LIFTED_DIE: [FieldParse<RiderChangeContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die(3, i, d, u), UserData::None),
    FieldParse::end(),
];

static LIFTED_TRANSPORT: [FieldParse<RiderChangeContainModuleData>; 15] = [
    FieldParse::new("Slots", |i, d, u| lift_transport(0, i, d, u), UserData::None),
    FieldParse::new("ScatterNearbyOnExit", |i, d, u| lift_transport(1, i, d, u), UserData::None),
    FieldParse::new("OrientLikeContainerOnExit", |i, d, u| lift_transport(2, i, d, u), UserData::None),
    FieldParse::new("KeepContainerVelocityOnExit", |i, d, u| lift_transport(3, i, d, u), UserData::None),
    FieldParse::new("GoAggressiveOnExit", |i, d, u| lift_transport(4, i, d, u), UserData::None),
    FieldParse::new("ResetMoodCheckTimeOnExit", |i, d, u| lift_transport(5, i, d, u), UserData::None),
    FieldParse::new("DestroyRidersWhoAreNotFreeToExit", |i, d, u| lift_transport(6, i, d, u), UserData::None),
    FieldParse::new("ExitBone", |i, d, u| lift_transport(7, i, d, u), UserData::None),
    FieldParse::new("ExitPitchRate", |i, d, u| lift_transport(8, i, d, u), UserData::None),
    FieldParse::new("InitialPayload", |i, d, u| lift_transport(9, i, d, u), UserData::None),
    FieldParse::new("HealthRegen%PerSec", |i, d, u| lift_transport(10, i, d, u), UserData::None),
    FieldParse::new("ExitDelay", |i, d, u| lift_transport(11, i, d, u), UserData::None),
    FieldParse::new("ArmedRidersUpgradeMyWeaponSet", |i, d, u| lift_transport(12, i, d, u), UserData::None),
    FieldParse::new("DelayExitInAir", |i, d, u| lift_transport(13, i, d, u), UserData::None),
    FieldParse::end(),
];

/// `RiderChangeContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<RiderChangeContainModuleData>; 11] = [
    FieldParse::new("Rider1", |i, d, _| parse_rider_info(i, &mut d.riders[0]), UserData::None),
    FieldParse::new("Rider2", |i, d, _| parse_rider_info(i, &mut d.riders[1]), UserData::None),
    FieldParse::new("Rider3", |i, d, _| parse_rider_info(i, &mut d.riders[2]), UserData::None),
    FieldParse::new("Rider4", |i, d, _| parse_rider_info(i, &mut d.riders[3]), UserData::None),
    FieldParse::new("Rider5", |i, d, _| parse_rider_info(i, &mut d.riders[4]), UserData::None),
    FieldParse::new("Rider6", |i, d, _| parse_rider_info(i, &mut d.riders[5]), UserData::None),
    FieldParse::new("Rider7", |i, d, _| parse_rider_info(i, &mut d.riders[6]), UserData::None),
    FieldParse::new("Rider8", |i, d, _| parse_rider_info(i, &mut d.riders[7]), UserData::None),
    FieldParse::new("ScuttleDelay", |i, d, _| { d.scuttle_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScuttleStatus", |i, d, _| { d.scuttle_state = i.parse_index_list(&MODEL_CONDITION_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl RiderChangeContainModuleData {
    /// `RiderChangeContainModuleData::buildFieldParse(p)`: `TransportContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RiderChangeContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&LIFTED_TRANSPORT)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RiderChangeContainModuleData> {
        let mut data = RiderChangeContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RiderChangeContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `RiderChangeContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RiderChangeContain(Arc::new(RiderChangeContainModuleData::friend_new_module_data(ini)?)))
}

/// `class RiderChangeContain : public TransportContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct RiderChangeContain {
    tc: TransportContain,
    /// `getRiderChangeContainModuleData()`.
    data: Arc<RiderChangeContainModuleData>,
    /// This class's own `m_extraSlotsInUse` (it hides the base's; see the module docs).
    extra_slots_in_use: i32,
    /// This class's own `m_frameExitNotBusy` (it hides the base's).
    frame_exit_not_busy: u32,
    /// `m_scuttledOnFrame` (not saved).
    scuttled_on_frame: u32,
    /// `m_containing` ("doesn't require xfer").
    containing: bool,
}

/// `MAKE_OBJECT_STATUS_MASK(bit)`.
fn status_mask(bit: i32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::new();
    m.set(bit, 1);
    m
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl RiderChangeContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        TransportContain::get_interface_mask()
    }

    /// The constructor's first half: the base (on a copy of its part of the data), then this class's four fields.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RiderChangeContainModuleData>>) -> IniResult<RiderChangeContain> {
        let data = module_data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let tc = TransportContain::unconstructed(object, Some(Arc::new(data.transport.clone())))?;
        Ok(RiderChangeContain { tc, data, extra_slots_in_use: 0, frame_exit_not_busy: 0, scuttled_on_frame: 0, containing: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.tc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<RiderChangeContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's, then this class's two hidden copies and `m_scuttledOnFrame`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.tc.state_words();
        v.push(self.extra_slots_in_use as u32);
        v.push(self.frame_exit_not_busy);
        v.push(self.scuttled_on_frame);
        v
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.tc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.tc.xfer_with(update, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.tc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.tc.load_post_process_with(update);
    }

    /// What `xfer` writes after the base: `m_payloadCreated` (the base's, a second time), then this class's `m_extraSlotsInUse` and
    /// `m_frameExitNotBusy`.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if !crate::logic::modules::plant(1479) {
            xfer.xfer_bool(&mut self.tc.payload_created)?;
        }
        xfer.xfer_int(&mut self.extra_slots_in_use)?;
        xfer.xfer_unsigned_int(&mut self.frame_exit_not_busy)
    }

    /// The rider kind `rider` is: the first of the eight whose template (by name) is equivalent to the rider's.
    fn rider_index(&self, world: &World, rider: ObjectId) -> Option<usize> {
        (0..MAX_RIDERS).find(|&i| {
            let name = &self.data.riders[i].template_name;
            world.thing_factory_find_template(name).is_some_and(|t| world.template_is_equivalent_to_object(t, rider))
        })
    }
}

transport_open_contain_virtuals!(RiderChangeContain, RiderChangeContain);

impl TransportContainVirtuals for RiderChangeContain {
    fn tc(&self) -> &TransportContain {
        &self.tc
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        &mut self.tc
    }

    /// `getContainMax()`: the slot capacity.
    fn t_get_contain_max(&self, _world: &World) -> i32 {
        self.data.transport.slot_capacity
    }

    /// `getExtraSlotsInUse()`: this class's own copy (always 0).
    fn t_get_extra_slots_in_use(&mut self) -> i32 {
        self.extra_slots_in_use
    }

    /// `isExitBusy()`: `FALSE`.
    fn t_is_exit_busy(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        crate::logic::modules::plant(1471)
    }

    /// `isRiderChangeContain()`: `TRUE`.
    fn t_is_rider_change_contain(&self) -> bool {
        true
    }

    /// `friend_getRider()`: the first rider ("this does assume that infantry never ride double on the bike").
    fn t_friend_get_rider(&self, _world: &World) -> ObjectId {
        // `m_containListSize` and `m_containList.front()`, the module's own
        self.oc().contain_list().first().copied().unwrap_or(INVALID_ID)
    }

    /// `isValidContainerFor(rider, checkCapacity)`: the base's without the capacity, not scuttled, and one of the eight riders.
    fn t_is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, _check_capacity: bool) -> bool {
        // "Don't check capacity because our rider will kick the other rider out!"
        if tc_is_valid_container_for(self, world, me, obj, crate::logic::modules::plant(1472)) {
            if self.scuttled_on_frame != 0 {
                // "Scuttled... too late!"
                return false;
            }
            return self.rider_index(world, obj).is_some();
        }
        false
    }

    /// `onContaining(rider, wasSelected)`.
    fn t_on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        let us = me.object;
        self.containing = true;
        // "Remove our existing rider"
        if self.tc.payload_created {
            let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else {
                undefined(world, me, Undefined::NullAi);
                return;
            };
            out(self, world, me, |w| w.module_call(us, ai, ModuleCall::AiEvacuateInstantly { expose_stealth_units: true }));
        }

        // "If the rider is currently selected, transfer selection to the container"
        if world.has_drawable(us) && was_selected && !world.reads.local_selection.contains(&us) {
            world.emit(ObjectClientEvent::MessageCreateSelectedGroup { create_new: false, object_id: us });
            world.emit(ObjectClientEvent::UiSelectDrawable { object_id: us });
            world.emit(ObjectClientEvent::UiSetDisplayedMaxWarning { on: false });
        }

        // "Find the rider in the list and set the appropriate model condition"
        if let Some(i) = self.rider_index(world, rider) {
            let r = self.data.riders[i].clone();
            out(self, world, me, |w| {
                w.set_model_condition_state(us, r.model_condition_flag_type);
                w.set_weapon_set_flag(us, r.weapon_set_flag);
                let mut mask = ObjectStatusMaskType::new();
                mask.set(r.object_status_type, 1);
                if !crate::logic::modules::plant(1473) {
                    w.set_status(us, mask, true);
                }
                if let Some(o) = w.objects.find_mut(us) {
                    o.command_set_string_override = r.command_set.clone();
                }
                w.emit(ObjectClientEvent::ControlBarMarkUiDirty);
            });
            // "Change the locomotor."
            if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
                out(self, world, me, |w| w.module_call(us, ai, ModuleCall::AiChooseLocomotorSet(r.locomotor_set_type)));
            }
            if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED)) {
                if let Some(stealth) = world.objects.find(us).and_then(|o| o.stealth) {
                    out(self, world, me, |w| w.module_call(us, stealth, ModuleCall::StealthMarkAsDetected));
                }
            }
            // "Transfer experience from the rider to the bike."
            let level = world.objects.find(rider).map(|o| o.experience.level as i32).unwrap_or(0);
            if !crate::logic::modules::plant(1474) {
                out(self, world, me, |w| w.contain_experience_set_veterancy_level(us, level));
            }
            out(self, world, me, |w| w.contain_experience_set_experience_and_level(rider, 0));
        }

        // "Extend base class"
        tc_on_containing(self, world, me, rider, was_selected);
        self.containing = false;
    }

    /// `onRemoving(rider)`.
    fn t_on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        let bike = me.object;
        // "Note if the bike dies, the rider dies too."
        if world.objects.find(bike).is_some_and(|o| o.is_effectively_dead()) != crate::logic::modules::plant(1475) {
            out(self, world, me, |w| w.contain_destroy_object(rider));
            return;
        }
        if self.tc.payload_created {
            // "Extend base class"
            tc_on_removing(self, world, me, rider);
        }

        // "Find the rider in the list and clear various data."
        if let Some(i) = self.rider_index(world, rider) {
            let r = self.data.riders[i].clone();
            out(self, world, me, |w| {
                w.contain_clear_model_condition_flags(bike, &[r.model_condition_flag_type, MODELCONDITION_DOOR_1_CLOSING]);
                w.clear_weapon_set_flag(bike, r.weapon_set_flag);
                let mut mask = ObjectStatusMaskType::new();
                mask.set(r.object_status_type, 1);
                w.clear_status(bike, mask);
            });
            if world.get_controlling_player(rider).is_some() {
                // "Transfer experience from the bike to the rider."
                let level = world.objects.find(bike).map(|o| o.experience.level as i32).unwrap_or(0);
                out(self, world, me, |w| {
                    w.contain_experience_set_veterancy_level(rider, level);
                    w.contain_experience_set_experience_and_level(bike, 0);
                });
            }
        }

        // "If we're not replacing the rider, then if the cycle is selected, transfer selection to the rider getting off"
        if !self.containing && !crate::logic::modules::plant(1476) && world.has_drawable(bike) && world.has_drawable(rider) {
            let local = world.players.local_index();
            if world.get_controlling_player(bike).is_some() && world.get_controlling_player(bike) == local && world.reads.local_selection.contains(&bike) {
                world.emit(ObjectClientEvent::MessageCreateSelectedGroup { create_new: false, object_id: rider });
                world.emit(ObjectClientEvent::UiSelectDrawable { object_id: rider });
                world.emit(ObjectClientEvent::UiSetDisplayedMaxWarning { on: false });
                world.emit(ObjectClientEvent::MessageRemoveFromSelectedGroup { object_id: bike });
                world.emit(ObjectClientEvent::UiDeselectDrawable { object_id: bike });
            }
            // "Finally, scuttle the bike so nobody else can use it! <Design Spec>"
            self.scuttled_on_frame = world.frame();
            let scuttle_state = self.data.scuttle_state;
            out(self, world, me, |w| {
                w.set_status(bike, status_mask(OBJECT_STATUS_UNSELECTABLE), true);
                w.set_model_condition_state(bike, scuttle_state);
            });
            let Some(ai) = world.objects.find(bike).and_then(|o| o.ai) else {
                undefined(world, me, Undefined::NullAi);
                return;
            };
            let moving = out(self, world, me, |w| w.module_call(bike, ai, ModuleCall::AiIsMoving)).as_bool();
            if !moving != crate::logic::modules::plant(1477) {
                out(self, world, me, |w| w.set_status(bike, status_mask(OBJECT_STATUS_IMMOBILE), true));
            }
        }
    }

    /// `update()`: a scuttled bike topples once the delay is over, then the base's.
    fn t_update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.scuttled_on_frame != 0 {
            // "Bike in the process of getting scuttled."
            let now = world.frame();
            if self.scuttled_on_frame.wrapping_add(self.data.scuttle_frames) <= now || crate::logic::modules::plant(1478) {
                // "Sneaky, eh? Toppled heheh."
                let us = me.object;
                if !out(self, world, me, |w| w.contain_kill_with(us, DAMAGE_UNRESISTABLE, DEATH_TOPPLED)) {
                    undefined(world, me, Undefined::NullBodyModule);
                    return crate::logic::module::update_module::UPDATE_SLEEP_NONE;
                }
            }
        }
        // "extend base class"
        tc_update(self, world, me)
    }
}

impl UpdateBehavior for RiderChangeContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.tc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RiderChangeContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RiderChangeContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for RiderChangeContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.tc, xfer)
    }

    /// `xfer` (version 1): the version, `TransportContain::xfer`, then `m_payloadCreated` again and this class's two counters.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.tc, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.tc);
    }
}
