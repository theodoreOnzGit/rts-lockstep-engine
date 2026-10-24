// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/OverlordContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OverlordContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `OverlordContain`: China's Overlord tank. Empty, it is a [`TransportContain`]; once it holds a portable structure (a gattling cannon, a bunker,
//! a propaganda tower) it "redirects": the contain calls go to that rider's own contain module, so the Overlord carries what the bunker carries.
//!
//! # Design
//!
//! As every class derived from `TransportContain` (see that module), with data and state of its own. `getRedirectedContain()` is the first rider's
//! contain while redirection is on ([`OverlordContain::redirected`]): the queries read it in its slot (`open_contain::view_*`), the calls are that
//! module's [`ModuleCall`]s. Because upstream's base code asks `getContainCount()`, `getContainedItemsList()` and `getContainMax()` through the
//! virtual (so that `TransportContain::update` heals the bunker's passengers, for example), those queries take the world on every contain class.
//!
//! # Upstream facts kept
//!
//! - Redirection starts when a portable structure is contained (not while redirected) and ends only on death and deletion: `onDie` and `onDelete`
//!   switch it off themselves so that the rider is killed or emptied through the Overlord's own list.
//! - `onCapture` gives the first rider the new owner's default team and does NOT extend the base (no one is ordered out); with a rider and no new owner
//!   it is undefined ([`Undefined::NullControllingPlayer`]).
//! - `killAllContained` with no redirection kills NO ONE ("we don't want it to kill our turret"). `isEnclosingContainerFor` is `FALSE` only for the
//!   first rider. `isDisplayedOnControlBar` and `isKickOutOnCapture` are `FALSE` unless redirected.
//! - `isPassengerAllowedToFire(id)` refuses a known passenger that is neither infantry nor a portable structure, and anyone while the Overlord is
//!   itself contained, then asks `TransportContain::isPassengerAllowedToFire()` with the default argument (`INVALID_ID`, not `id`).
//! - `createPayload` (from `onObjectCreated`) is the Helix's: every `PayloadTemplateName`, with the Overlord's own team.
//! - `clientVisibleContainedFlashAsSelected` loops `while( *it )` over the list (reads `end()` past the last rider:
//!   [`Undefined::EndIteratorDereferenced`]; the C++ reference stops there, edited by `build.rs`).
//! - `xfer` is version 1: the version, the base's, then `m_redirectionActivated`.
//!
//! `p1w13-sonnet`, batch R6, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_PORTABLE_STRUCTURE};
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::BODY_RUBBLE;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime};
use crate::logic::modules::open_contain::{self, out, OpenContainVirtuals};
use crate::logic::modules::transport_contain::{
    self, tc_get_contain_max, tc_is_passenger_allowed_to_fire, tc_is_valid_container_for,
    tc_on_containing, tc_on_removing, transport_open_contain_virtuals, TransportContain,
    TransportContainModuleData, TransportContainVirtuals,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class OverlordContainModuleData : public TransportContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlordContainModuleData {
    /// The `TransportContainModuleData` base.
    pub transport: TransportContainModuleData,
    /// `m_payloadTemplateNameData` (`PayloadTemplateName`, appended).
    pub payload_template_name_data: Vec<Vec<u8>>,
    /// `m_experienceSinkForRider` (`ExperienceSinkForRider`).
    pub experience_sink_for_rider: bool,
}

impl Default for OverlordContainModuleData {
    /// `OverlordContainModuleData()`: the base's defaults, `m_experienceSinkForRider = TRUE`.
    fn default() -> Self {
        OverlordContainModuleData {
            transport: TransportContainModuleData::default(),
            payload_template_name_data: Vec::new(),
            experience_sink_for_rider: true,
        }
    }
}

fn lift_open(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut OverlordContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::OPEN_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

fn lift_die(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut OverlordContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::DIE_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

fn lift_transport(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut OverlordContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<OverlordContainModuleData>; 16] = [
    FieldParse::new(
        "ContainMax",
        |i, d, u| lift_open(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "EnterSound",
        |i, d, u| lift_open(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new(
        "DamagePercentToUnits",
        |i, d, u| lift_open(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "BurnedDeathToUnits",
        |i, d, u| lift_open(4, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowInsideKindOf",
        |i, d, u| lift_open(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ForbidInsideKindOf",
        |i, d, u| lift_open(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersAllowedToFire",
        |i, d, u| lift_open(7, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersInTurret",
        |i, d, u| lift_open(8, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "NumberOfExitPaths",
        |i, d, u| lift_open(9, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DoorOpenTime",
        |i, d, u| lift_open(10, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "WeaponBonusPassedToPassengers",
        |i, d, u| lift_open(11, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowAlliesInside",
        |i, d, u| lift_open(12, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowEnemiesInside",
        |i, d, u| lift_open(13, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowNeutralInside",
        |i, d, u| lift_open(14, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<OverlordContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die(3, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

static LIFTED_TRANSPORT: [FieldParse<OverlordContainModuleData>; 15] = [
    FieldParse::new(
        "Slots",
        |i, d, u| lift_transport(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ScatterNearbyOnExit",
        |i, d, u| lift_transport(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "OrientLikeContainerOnExit",
        |i, d, u| lift_transport(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "KeepContainerVelocityOnExit",
        |i, d, u| lift_transport(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "GoAggressiveOnExit",
        |i, d, u| lift_transport(4, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ResetMoodCheckTimeOnExit",
        |i, d, u| lift_transport(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DestroyRidersWhoAreNotFreeToExit",
        |i, d, u| lift_transport(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExitBone",
        |i, d, u| lift_transport(7, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExitPitchRate",
        |i, d, u| lift_transport(8, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "InitialPayload",
        |i, d, u| lift_transport(9, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "HealthRegen%PerSec",
        |i, d, u| lift_transport(10, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExitDelay",
        |i, d, u| lift_transport(11, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ArmedRidersUpgradeMyWeaponSet",
        |i, d, u| lift_transport(12, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DelayExitInAir",
        |i, d, u| lift_transport(13, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `OverlordContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<OverlordContainModuleData>; 3] = [
    FieldParse::new(
        "PayloadTemplateName",
        |i, d, _| i.parse_ascii_string_vector_append(&mut d.payload_template_name_data),
        UserData::None,
    ),
    FieldParse::new(
        "ExperienceSinkForRider",
        |i, d, _| {
            d.experience_sink_for_rider = i.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl OverlordContainModuleData {
    /// `OverlordContainModuleData::buildFieldParse(p)`: `TransportContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, OverlordContainModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&LIFTED_TRANSPORT)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<OverlordContainModuleData> {
        let mut data = OverlordContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, OverlordContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `OverlordContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::OverlordContain(Arc::new(
        OverlordContainModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class OverlordContain : public TransportContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlordContain {
    tc: TransportContain,
    /// `getOverlordContainModuleData()`.
    data: Arc<OverlordContainModuleData>,
    /// `m_redirectionActivated`.
    redirection_activated: bool,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl OverlordContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        TransportContain::get_interface_mask()
    }

    /// The constructor's first half: the base (on a copy of its part of the data), `m_redirectionActivated = FALSE`, `m_payloadCreated = FALSE`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<OverlordContainModuleData>>,
    ) -> IniResult<OverlordContain> {
        let data = module_data
            .ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let mut tc =
            TransportContain::unconstructed(object, Some(Arc::new(data.transport.clone())))?;
        tc.payload_created = false;
        Ok(OverlordContain {
            tc,
            data,
            redirection_activated: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.tc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<OverlordContainModuleData> {
        &self.data
    }

    /// The state `xfer` writes, as words: the base's, then `m_redirectionActivated`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.tc.state_words();
        v.push(u32::from(self.redirection_activated));
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

    /// The module's answer to one of `Object`'s calls (a ring of redirections found on the way is recorded: see [`Self::check_ring`]).
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        let r = open_contain::contain_module_call(self, world, me, call);
        Self::check_ring(world, me);
        r
    }

    /// A read through a ring of redirecting containers went too deep (`open_contain::view_*`): upstream recurses until the stack is gone.
    fn check_ring(world: &mut World, me: ModuleHandle) {
        if world.cuts.contain.view_overflow.replace(false) {
            undefined(world, me, Undefined::EndlessLoop);
        }
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.tc.xfer_with(update, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// What `xfer` writes after the base: `m_redirectionActivated`.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if crate::logic::modules::plant(1499) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.redirection_activated)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.tc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.tc.load_post_process_with(update);
    }

    /// `getRedirectedContain()`: the first rider, if it has a contain module and redirection is on ("if I am empty, say no").
    fn redirected(&self, world: &World) -> Option<ObjectId> {
        if self.oc().contain_list_size() < 1 {
            return None;
        }
        if !self.redirection_activated {
            return None;
        }
        let my_guy = *self.oc().contain_list().first()?;
        world.get_contain(my_guy).map(|_| my_guy)
    }

    /// A call on the redirected contain (`getRedirectedContain()->...`); a ring of redirections (an Overlord that carries, through others, itself)
    /// recurses for ever upstream: past `MAX_RECURSION` levels it is recorded as [`Undefined::EndlessLoop`].
    fn redirect_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        my_guy: ObjectId,
        call: ModuleCall,
    ) -> ModuleReply {
        let Some(c) = world.get_contain(my_guy) else {
            return ModuleReply::Unhandled;
        };
        if world.cuts.contain.recursion >= open_contain::MAX_RECURSION {
            undefined(world, me, Undefined::EndlessLoop);
            return ModuleReply::Unhandled;
        }
        world.cuts.contain.recursion += 1;
        let r = out(self, world, me, |w| w.module_call(my_guy, c, call));
        world.cuts.contain.recursion -= 1;
        r
    }

    /// `OverlordContain::createPayload()` (the Helix's).
    fn overlord_create_payload(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        open_contain::enable_load_sounds(self, false);
        let list = self.data.payload_template_name_data.clone();
        for name in list {
            if let Some(template) = world.thing_factory_find_template(&name) {
                let team = world.get_team(us).unwrap_or(0);
                let payload = out(self, world, me, |w| {
                    w.contain_new_payload_object(template, team, ObjectStatusMaskType::new())
                });
                if self.is_valid_container_for(world, me, payload, true)
                    && !crate::logic::modules::plant(1491)
                {
                    self.add_to_contain(world, me, payload);
                }
            }
        }
        open_contain::enable_load_sounds(self, true);
        self.tc.payload_created = true;
    }
}

transport_open_contain_virtuals!(OverlordContain, OverlordContain);

impl TransportContainVirtuals for OverlordContain {
    fn tc(&self) -> &TransportContain {
        &self.tc
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        &mut self.tc
    }

    /// `createPayload()`.
    fn create_payload(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        self.overlord_create_payload(world, me);
        Ok(())
    }

    /// `onObjectCreated()`: `OverlordContain::createPayload()`.
    fn t_on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        self.overlord_create_payload(world, me);
    }

    /// `onBodyDamageStateChange(info, old, new)`: the one rider takes the state ("I want just me").
    fn t_on_body_damage_state_change(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        _info: &DamageInfo,
        _old_state: i32,
        new_state: i32,
    ) {
        if new_state != BODY_RUBBLE
            && self.oc().contain_list_size() == 1
            && !crate::logic::modules::plant(1492)
        {
            // `m_containList.front()`: undefined when the size disagrees with an empty list
            let Some(&my_guy) = self.oc().contain_list().first() else {
                undefined(world, me, Undefined::NullObject);
                return;
            };
            if !out(self, world, me, |w| {
                w.contain_set_damage_state(my_guy, new_state)
            }) {
                undefined(world, me, Undefined::NullBodyModule);
            }
        }
    }

    /// `onDie(info)`: with a redirected contain, redirection off and the rider killed first.
    fn t_on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if self.redirected(world).is_none() {
            open_contain::base_on_die(self, world, me, info);
            return;
        }
        self.redirection_activated = false;
        let Some(&my_guy) = self.oc().contain_list().first() else {
            undefined(world, me, Undefined::NullObject);
            return;
        };
        if !out(self, world, me, |w| w.contain_kill(my_guy)) {
            undefined(world, me, Undefined::NullBodyModule);
            return;
        }
        open_contain::base_on_die(self, world, me, info);
    }

    /// `onDelete()`: with a redirected contain, the rider emptied, redirection off, then the Overlord emptied.
    fn t_on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        let Some(my_guy) = self.redirected(world) else {
            open_contain::base_on_delete(self, world, me);
            return;
        };
        // "Without my throwing the redirect switch, teardown deletion will get confused"
        self.redirect_call(
            world,
            me,
            my_guy,
            ModuleCall::RemoveAllContained {
                expose_stealth_units: false,
            },
        );
        self.redirection_activated = false;
        self.remove_all_contained(world, me, false);
        open_contain::base_on_delete(self, world, me);
    }

    /// `onCapture(old, new)`: "need to capture our specific rider"; the base is not extended.
    fn t_on_capture(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        _old_owner: i32,
        new_owner: i32,
    ) {
        if self.oc().contain_list_size() < 1 {
            return;
        }
        // `m_containList.front()`: a size that disagrees with an empty list reads `front()` of nothing (batch R9: found by the mob scene)
        let Some(&my_guy) = self.oc().contain_list().first() else {
            undefined(world, me, Undefined::NullObject);
            return;
        };
        if new_owner < 0 {
            undefined(world, me, Undefined::NullControllingPlayer);
            return;
        }
        let team = world.cuts.player_default_team(new_owner);
        if !crate::logic::modules::plant(1493) {
            out(self, world, me, |w| w.contain_set_team(my_guy, team));
        }
    }

    /// `isGarrisonable()`: the redirected contain's, else `FALSE`.
    fn t_is_garrisonable(&mut self, world: &mut World, me: ModuleHandle) -> bool {
        match self.redirected(world) {
            None => false,
            Some(g) => self
                .redirect_call(world, me, g, ModuleCall::IsGarrisonable)
                .as_bool(),
        }
    }

    /// `isKickOutOnCapture()`: the redirected contain's, else `FALSE` ("me the Overlord doesn't want to").
    fn t_is_kick_out_on_capture(&self, world: &World) -> bool {
        match self.redirected(world) {
            None => false,
            Some(g) => open_contain::view_is_kick_out_on_capture(world, g).unwrap_or(false),
        }
    }

    /// `addToContainList(obj)`.
    fn t_add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        match self.redirected(world) {
            None => open_contain::base_add_to_contain_list(self, world, me, rider),
            Some(g) => {
                self.redirect_call(world, me, g, ModuleCall::AddToContainList(rider));
            }
        }
    }

    /// `addToContain(obj)`.
    fn t_add_to_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        match self.redirected(world) {
            None => open_contain::base_add_to_contain(self, world, me, rider),
            Some(g) if !crate::logic::modules::plant(1494) => {
                self.redirect_call(world, me, g, ModuleCall::AddToContain(rider));
            }
            Some(_) => open_contain::base_add_to_contain(self, world, me, rider),
        }
    }

    /// `removeFromContain(obj, exposeStealthUnits)`.
    fn t_remove_from_contain(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        expose_stealth_units: bool,
    ) {
        match self.redirected(world) {
            None => {
                open_contain::base_remove_from_contain(self, world, me, rider, expose_stealth_units)
            }
            Some(g) => {
                self.redirect_call(
                    world,
                    me,
                    g,
                    ModuleCall::RemoveFromContainExposing {
                        obj: rider,
                        expose_stealth_units,
                    },
                );
            }
        }
    }

    /// `removeAllContained(exposeStealthUnits)`: redirected, each of the rider's riders through this module's `removeFromContain`.
    fn t_remove_all_contained(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        expose_stealth_units: bool,
    ) {
        let Some(g) = self.redirected(world) else {
            open_contain::base_remove_all_contained(self, world, me, expose_stealth_units);
            return;
        };
        let full_list = open_contain::view_contained_items_list(world, g).unwrap_or_default();
        for obj in full_list {
            self.remove_from_contain(world, me, obj, expose_stealth_units);
        }
    }

    /// The list `iterateContained` walks: the redirected contain's.
    fn t_get_contained_items_list_for_iteration(&self, world: &World) -> Vec<ObjectId> {
        match self.redirected(world) {
            None => self.oc().contain_list().to_vec(),
            Some(g) => {
                open_contain::view_contained_items_list_for_iteration(world, g).unwrap_or_default()
            }
        }
    }

    /// `onContaining(obj, wasSelected)`.
    fn t_on_containing(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        was_selected: bool,
    ) {
        let us = me.object;
        let Some(g) = self.redirected(world) else {
            tc_on_containing(self, world, me, rider, was_selected);
            if world
                .objects
                .find(rider)
                .is_some_and(|o| o.is_kind_of(KINDOF_PORTABLE_STRUCTURE))
            {
                // "Am now carrying something"
                if !crate::logic::modules::plant(1495) {
                    self.redirection_activated = true;
                }
                // "And this contain style explicitly sucks XP from our little friend."
                if self.data.experience_sink_for_rider {
                    out(self, world, me, |w| {
                        w.contain_experience_set_sink(rider, us)
                    });
                }
                if world
                    .objects
                    .find(us)
                    .is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED))
                {
                    out(self, world, me, |w| {
                        w.contain_stealth_receive_grant(rider, true, 0)
                    });
                }
            }
            return;
        };
        open_contain::base_on_containing(self, world, me, rider, was_selected);
        self.redirect_call(
            world,
            me,
            g,
            ModuleCall::OnContaining {
                obj: rider,
                was_selected,
            },
        );
    }

    /// `killAllContained()`: only the redirected contain's ("we don't want it to kill our turret").
    fn t_kill_all_contained(&mut self, world: &mut World, me: ModuleHandle) {
        if let Some(g) = self.redirected(world) {
            self.redirect_call(world, me, g, ModuleCall::KillAllContained);
        } else if crate::logic::modules::plant(1496) {
            open_contain::base_kill_all_contained(self, world, me);
        }
    }

    /// `onRemoving(obj)`.
    fn t_on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        let Some(g) = self.redirected(world) else {
            tc_on_removing(self, world, me, rider);
            return;
        };
        open_contain::base_on_removing(self, world, me, rider);
        self.redirect_call(world, me, g, ModuleCall::OnRemoving(rider));
    }

    /// `isValidContainerFor(obj, checkCapacity)`.
    fn t_is_valid_container_for(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
        check_capacity: bool,
    ) -> bool {
        match self.redirected(world) {
            None => tc_is_valid_container_for(self, world, me, obj, check_capacity),
            Some(g) => self
                .redirect_call(
                    world,
                    me,
                    g,
                    ModuleCall::IsValidContainerFor {
                        obj,
                        check_capacity,
                    },
                )
                .as_bool(),
        }
    }

    /// `getContainCount()`.
    fn t_get_contain_count(&self, world: &World) -> u32 {
        match self.redirected(world) {
            None => self.oc().contain_list_size(),
            Some(g) => open_contain::view_contain_count(world, g).unwrap_or(0),
        }
    }

    /// `getContainMax()`.
    fn t_get_contain_max(&self, world: &World) -> i32 {
        match self.redirected(world) {
            None => tc_get_contain_max(self),
            Some(g) => open_contain::view_contain_max(world, g).unwrap_or(0),
        }
    }

    /// `getContainedItemsList()`.
    fn t_get_contained_items_list(&self, world: &World) -> Vec<ObjectId> {
        match self.redirected(world) {
            None => self.oc().contain_list().to_vec(),
            Some(g) if !crate::logic::modules::plant(1497) => {
                open_contain::view_contained_items_list(world, g).unwrap_or_default()
            }
            Some(_) => self.oc().contain_list().to_vec(),
        }
    }

    /// `isEnclosingContainerFor(obj)`: not for the first rider (the Overlord's own passenger), yes for everyone else.
    fn t_is_enclosing_container_for(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        obj: ObjectId,
    ) -> bool {
        !(self.oc().contain_list_size() > 0 && self.oc().contain_list().first() == Some(&obj))
    }

    /// `isDisplayedOnControlBar()`: the redirected contain's, else `FALSE`.
    fn t_is_displayed_on_control_bar(&self, world: &World) -> bool {
        match self.redirected(world) {
            None => false,
            Some(g) => open_contain::view_is_displayed_on_control_bar(world, g).unwrap_or(false),
        }
    }

    /// `friend_getRider()`: the first rider.
    fn t_friend_get_rider(&self, _world: &World) -> ObjectId {
        self.oc()
            .contain_list()
            .first()
            .copied()
            .unwrap_or(INVALID_ID)
    }

    /// `isPassengerAllowedToFire(id)`.
    fn t_is_passenger_allowed_to_fire(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        id: ObjectId,
    ) -> bool {
        if let Some(p) = world.objects.find(id) {
            // "only allow infantry, and turrets and such. no vehicles."
            if !p.is_kind_of(KINDOF_INFANTRY) && !p.is_kind_of(KINDOF_PORTABLE_STRUCTURE) {
                return false;
            }
        }
        // "nested containment voids firing, always"
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.contained_by != INVALID_ID)
            != crate::logic::modules::plant(1498)
        {
            return false;
        }
        tc_is_passenger_allowed_to_fire(self, world, me, INVALID_ID)
    }

    /// `isSpecialOverlordStyleContainer()`: `TRUE`.
    fn t_is_special_overlord_style_container(&self) -> bool {
        true
    }

    /// `clientVisibleContainedFlashAsSelected()`: the portable structures of the Overlord's own list flash.
    fn t_other_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::ClientVisibleContainedFlashAsSelected => {
                for object in self.oc().contain_list().to_vec() {
                    if world
                        .objects
                        .find(object)
                        .is_some_and(|o| o.is_kind_of(KINDOF_PORTABLE_STRUCTURE))
                        && world.has_drawable(object)
                    {
                        world
                            .emit(ObjectClientEvent::DrawableFlashAsSelected { object_id: object });
                    }
                }
                // upstream: `while( *it )` reads `end()` after the last one
                undefined(world, me, Undefined::EndIteratorDereferenced);
                ModuleReply::Done
            }
            ModuleCall::IsSpecificRiderFreeToExit { obj } => {
                ModuleReply::Bool(self.is_specific_rider_free_to_exit(world, me, obj))
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl UpdateBehavior for OverlordContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.tc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = OpenContainVirtuals::update(self, world, me);
        Self::check_ring(world, me);
        r
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::OverlordContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::OverlordContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for OverlordContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.tc, xfer)
    }

    /// `xfer` (version 1): the version, `TransportContain::xfer`, then `m_redirectionActivated`.
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
