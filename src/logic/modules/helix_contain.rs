// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HelixContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/HelixContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HelixContain`: China's Helix helicopter. A [`TransportContain`] that can also carry one portable structure (a gattling cannon, a bunker, a
//! propaganda tower, ...) on its back: that object is not in the contain list but remembered by id (`m_portableStructureID`), moved with the Helix
//! on every update, given the Helix's damage state, killed and destroyed with it and handed to the new owner on capture.
//!
//! # Design
//!
//! As every class derived from `TransportContain` (see that module), with data and state of its own, so the construction, the slot entry points and
//! `xfer` are written here rather than by `derived_transport_contain_glue!`. The data holds `TransportContainModuleData` by value
//! ([`HelixContainModuleData::transport`]) and lifts its three field tables onto it.
//!
//! # Upstream facts kept
//!
//! - `createPayload` loads every `PayloadTemplateName` (any number, each once), with `newObject(template, getTeam())` (the Helix's own team, not the
//!   player's default team as `TransportContain`'s), and ignores `InitialPayload`; a name that is no template is skipped. `onObjectCreated` calls it, so
//!   the first `update` finds the payload created.
//! - `isValidContainerFor`, `addToContain`, `addToContainList`, `removeFromContain` and `isEnclosingContainerFor` dereference the object unchecked: a
//!   NULL object (a payload the factory did not make) is undefined ([`Undefined::NullObject`]).
//! - `addToContain(List)` take the FIRST portable structure only while none is held; the destroy of a previous one it guards with is dead code (it looks
//!   the previous one up by `m_portableStructureID`, which is `INVALID_ID` on that path). A second portable structure goes into the contain list.
//! - `removeFromContain` of the portable structure forgets it only while it still exists, and never removes it from anything (the kill is commented
//!   out upstream).
//! - `onCapture` gives the portable structure the new owner's default team and does NOT extend the base: the riders are not ordered out. With a portable
//!   structure and no new owner it is undefined ([`Undefined::NullControllingPlayer`]).
//! - `onDie` and `onDelete` kill and destroy the portable structure first; `Object::kill` of one with no body is undefined
//!   ([`Undefined::NullBodyModule`]), and so is `onBodyDamageStateChange` (anything but `BODY_RUBBLE`) for one with no body.
//! - `isPassengerAllowedToFire(id)`: never while the Helix is itself contained; always for the portable structure; otherwise only for an existing
//!   infantry rider, by `TransportContain`'s rule.
//! - `xfer` is version 2: the version, then (from 2) `m_portableStructureID`, then the base's. `getContainerPipsToShow` (`ShouldDrawPips`) is the
//!   control bar's, not translated.
//!
//! `p1w13-sonnet`, batch R4, 2026-10-22.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_HELD;
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
    self, tc_is_passenger_allowed_to_fire, tc_is_valid_container_for, tc_on_containing,
    tc_on_removing, tc_update, transport_open_contain_virtuals, TransportContain,
    TransportContainModuleData, TransportContainVirtuals,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::condition::GARRISONED;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class HelixContainModuleData : public TransportContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct HelixContainModuleData {
    /// The `TransportContainModuleData` base.
    pub transport: TransportContainModuleData,
    /// `m_payloadTemplateNameData` (`PayloadTemplateName`, appended).
    pub payload_template_name_data: Vec<Vec<u8>>,
    /// `m_drawPips` (`ShouldDrawPips`; the control bar's).
    pub draw_pips: bool,
}

impl Default for HelixContainModuleData {
    /// `HelixContainModuleData()`: the base's defaults, `m_drawPips = TRUE`.
    fn default() -> Self {
        HelixContainModuleData {
            transport: TransportContainModuleData::default(),
            payload_template_name_data: Vec::new(),
            draw_pips: true,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table (as `TransportContain` lifts it) onto the `transport` base.
fn lift_open(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut HelixContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::OPEN_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `transport` base.
fn lift_die(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut HelixContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::DIE_BASE_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of `TransportContainModuleData`'s own table onto the `transport` base.
fn lift_transport(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut HelixContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match transport_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.transport, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<HelixContainModuleData>; 16] = [
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

static LIFTED_DIE: [FieldParse<HelixContainModuleData>; 5] = [
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

static LIFTED_TRANSPORT: [FieldParse<HelixContainModuleData>; 15] = [
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

/// `HelixContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<HelixContainModuleData>; 3] = [
    FieldParse::new(
        "PayloadTemplateName",
        |i, d, _| i.parse_ascii_string_vector_append(&mut d.payload_template_name_data),
        UserData::None,
    ),
    FieldParse::new(
        "ShouldDrawPips",
        |i, d, _| {
            d.draw_pips = i.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl HelixContainModuleData {
    /// `HelixContainModuleData::buildFieldParse(p)`: `TransportContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, HelixContainModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&LIFTED_TRANSPORT)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HelixContainModuleData> {
        let mut data = HelixContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HelixContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `HelixContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HelixContain(Arc::new(
        HelixContainModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class HelixContain : public TransportContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct HelixContain {
    tc: TransportContain,
    /// `getHelixContainModuleData()`.
    data: Arc<HelixContainModuleData>,
    /// `m_portableStructureID`.
    portable_structure_id: ObjectId,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl HelixContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        TransportContain::get_interface_mask()
    }

    /// The constructor's first half: the base (on a copy of its part of the data), `m_payloadCreated = FALSE`, `m_portableStructureID = INVALID_ID`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<HelixContainModuleData>>,
    ) -> IniResult<HelixContain> {
        let data = module_data
            .ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let mut tc =
            TransportContain::unconstructed(object, Some(Arc::new(data.transport.clone())))?;
        tc.payload_created = false;
        Ok(HelixContain {
            tc,
            data,
            portable_structure_id: INVALID_ID,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.tc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<HelixContainModuleData> {
        &self.data
    }

    /// The state `xfer` writes, as words: the base's, then `m_portableStructureID`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.tc.state_words();
        v.push(self.portable_structure_id as u32);
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
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds (see `xfer`).
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.xfer_own_fields(xfer)?;
        self.tc.xfer_with(update, xfer)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.tc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.tc.load_post_process_with(update);
    }

    /// The version (2), then (from version 2) `m_portableStructureID`: what `xfer` writes before the base.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = if crate::logic::modules::plant(1459) {
            1
        } else {
            2
        };
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        if version >= 2 {
            xfer.xfer_object_id(&mut self.portable_structure_id)?;
        }
        Ok(())
    }

    /// `getPortableStructure()`: `findObjectByID(m_portableStructureID)`; `INVALID_ID` for none.
    fn get_portable_structure(&self, world: &World) -> ObjectId {
        if self.portable_structure_id != INVALID_ID
            && world.objects.find(self.portable_structure_id).is_some()
        {
            self.portable_structure_id
        } else {
            INVALID_ID
        }
    }

    /// `obj->isKindOf(KINDOF_PORTABLE_STRUCTURE)`, or `None` for a NULL object (undefined: the callers dereference it).
    fn is_portable(world: &mut World, me: ModuleHandle, obj: ObjectId) -> Option<bool> {
        match world.objects.find(obj) {
            Some(o) => Some(o.is_kind_of(KINDOF_PORTABLE_STRUCTURE)),
            None => {
                undefined(world, me, Undefined::NullObject);
                None
            }
        }
    }

    /// The shared head of `addToContain` and `addToContainList`: `true` when `obj` became the portable structure.
    fn take_portable(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
    ) -> Option<bool> {
        let portable = Self::is_portable(world, me, obj)?;
        if portable
            && (self.portable_structure_id == INVALID_ID) != crate::logic::modules::plant(1451)
        {
            let old = self.get_portable_structure(world);
            if old != INVALID_ID {
                out(self, world, me, |w| w.contain_destroy_object(old));
            }
            self.portable_structure_id = obj;
            // "fool portable into thinking my object is his container"
            if let Some(o) = world.objects.find_mut(obj) {
                o.contained_by = me.object;
            }
            return Some(true);
        }
        Some(false)
    }

    /// `HelixContain::createPayload()` (non-virtual from `onObjectCreated`, which names it qualified; the virtual reaches the same).
    fn helix_create_payload(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        // `object->getContain()` is this module
        open_contain::enable_load_sounds(self, false);
        let list = self.data.payload_template_name_data.clone();
        for name in list {
            if let Some(template) = world.thing_factory_find_template(&name) {
                let team = world.get_team(us).unwrap_or(0);
                let payload = out(self, world, me, |w| {
                    w.contain_new_payload_object(template, team, ObjectStatusMaskType::new())
                });
                if self.is_valid_container_for(world, me, payload, true)
                    && !crate::logic::modules::plant(1452)
                {
                    self.add_to_contain(world, me, payload);
                }
                // else DEBUG_CRASH (nothing in a release build)
            }
        }
        open_contain::enable_load_sounds(self, true);
        self.tc.payload_created = true;
    }
}

transport_open_contain_virtuals!(HelixContain, HelixContain);

impl TransportContainVirtuals for HelixContain {
    fn tc(&self) -> &TransportContain {
        &self.tc
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        &mut self.tc
    }

    /// `createPayload()`.
    fn create_payload(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        self.helix_create_payload(world, me);
        Ok(())
    }

    /// `onObjectCreated()`: `HelixContain::createPayload()`.
    fn t_on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        if !crate::logic::modules::plant(1453) {
            self.helix_create_payload(world, me);
        }
    }

    /// `update()`: the portable structure follows the Helix, then the base's.
    fn t_update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let portable = self.get_portable_structure(world);
        if portable != INVALID_ID && !crate::logic::modules::plant(1454) {
            let (pos, angle) = world
                .objects
                .find(me.object)
                .map(|o| (o.position, o.thing.orientation()))
                .unwrap_or_default();
            out(self, world, me, |w| {
                w.contain_set_position(portable, &pos);
                w.contain_set_orientation(portable, angle);
            });
        }
        tc_update(self, world, me)
    }

    /// `redeployOccupants()`: every rider to 8 above the Helix.
    fn t_redeploy_occupants(&mut self, world: &mut World, me: ModuleHandle) {
        let mut fire_pos = world
            .objects
            .find(me.object)
            .map(|o| o.position)
            .unwrap_or_default();
        fire_pos.z += if crate::logic::modules::plant(1455) {
            0.0
        } else {
            8.0
        };
        // `m_containList`, the module's own
        for rider in self.oc().contain_list().to_vec() {
            if rider != INVALID_ID {
                out(self, world, me, |w| {
                    w.contain_set_position(rider, &fire_pos)
                });
            }
        }
    }

    /// `onBodyDamageStateChange(info, old, new)`: the portable structure takes the new state (not rubble).
    fn t_on_body_damage_state_change(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        _info: &DamageInfo,
        _old_state: i32,
        new_state: i32,
    ) {
        let portable = self.get_portable_structure(world);
        if (new_state != BODY_RUBBLE) != crate::logic::modules::plant(1456)
            && portable != INVALID_ID
            && !out(self, world, me, |w| {
                w.contain_set_damage_state(portable, new_state)
            })
        {
            undefined(world, me, Undefined::NullBodyModule);
        }
    }

    /// `onDie(info)`: the portable structure is killed, then the base's.
    fn t_on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        let portable = self.get_portable_structure(world);
        if portable != INVALID_ID && !out(self, world, me, |w| w.contain_kill(portable)) {
            undefined(world, me, Undefined::NullBodyModule);
            return;
        }
        open_contain::base_on_die(self, world, me, info);
    }

    /// `onDelete()`: the portable structure is destroyed, then the base's.
    fn t_on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        let portable = self.get_portable_structure(world);
        if portable != INVALID_ID {
            out(self, world, me, |w| w.contain_destroy_object(portable));
        }
        open_contain::base_on_delete(self, world, me);
    }

    /// `onCapture(old, new)`: "need to setteam() the portable structure, that's all".
    fn t_on_capture(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        _old_owner: i32,
        new_owner: i32,
    ) {
        let portable = self.get_portable_structure(world);
        if portable == INVALID_ID {
            return;
        }
        if new_owner < 0 {
            undefined(world, me, Undefined::NullControllingPlayer);
            return;
        }
        let team = world.cuts.player_default_team(new_owner);
        if !crate::logic::modules::plant(1457) {
            out(self, world, me, |w| w.contain_set_team(portable, team));
        }
    }

    /// `addToContainList(obj)`.
    fn t_add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        if self.take_portable(world, me, rider) == Some(false) {
            open_contain::base_add_to_contain_list(self, world, me, rider);
        }
    }

    /// `addToContain(obj)`.
    fn t_add_to_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        if self.take_portable(world, me, rider) == Some(false) {
            open_contain::base_add_to_contain(self, world, me, rider);
        }
    }

    /// `removeFromContain(obj, exposeStealthUnits)`: the portable structure is only forgotten (while it exists).
    fn t_remove_from_contain(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        expose_stealth_units: bool,
    ) {
        let Some(portable) = Self::is_portable(world, me, rider) else {
            return;
        };
        if portable && rider == self.portable_structure_id {
            if self.get_portable_structure(world) != INVALID_ID
                && !crate::logic::modules::plant(1458)
            {
                self.portable_structure_id = INVALID_ID;
            }
        } else {
            open_contain::base_remove_from_contain(self, world, me, rider, expose_stealth_units);
        }
    }

    /// `isValidContainerFor(obj, checkCapacity)`: a portable structure while none is held, else the base's.
    fn t_is_valid_container_for(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
        check_capacity: bool,
    ) -> bool {
        let Some(portable) = Self::is_portable(world, me, obj) else {
            return false;
        };
        if portable && self.portable_structure_id == INVALID_ID {
            return true;
        }
        tc_is_valid_container_for(self, world, me, obj, check_capacity)
    }

    /// `friend_getRider()`: the portable structure, if it exists.
    fn t_friend_get_rider(&self, world: &World) -> ObjectId {
        self.get_portable_structure(world)
    }

    /// `isEnclosingContainerFor(obj)`: not for the portable structure.
    fn t_is_enclosing_container_for(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
    ) -> bool {
        if world.objects.find(obj).is_none() {
            undefined(world, me, Undefined::NullObject);
            return true;
        }
        if self.portable_structure_id == obj && self.get_portable_structure(world) == obj {
            return false;
        }
        true
    }

    /// `isPassengerAllowedToFire(id)`.
    fn t_is_passenger_allowed_to_fire(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        id: ObjectId,
    ) -> bool {
        // "nested containment voids firing, always"
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.contained_by != INVALID_ID)
        {
            return false;
        }
        if self.portable_structure_id != INVALID_ID && self.portable_structure_id == id {
            return true;
        }
        if world
            .objects
            .find(id)
            .is_some_and(|o| o.is_kind_of(KINDOF_INFANTRY))
        {
            return tc_is_passenger_allowed_to_fire(self, world, me, id);
        }
        false
    }

    /// `onContaining(obj, wasSelected)`: the base's, then the garrisoned weapon bonus, `DISABLED_HELD`, and stealth for a portable structure.
    fn t_on_containing(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        was_selected: bool,
    ) {
        tc_on_containing(self, world, me, rider, was_selected);
        // "give the object a garrisoned version of its weapon"
        if !crate::logic::modules::plant(1460) {
            out(self, world, me, |w| {
                w.set_weapon_bonus_condition(rider, GARRISONED)
            });
        }
        out(self, world, me, |w| {
            w.contain_set_disabled(rider, DISABLED_HELD)
        });
        let portable = world
            .objects
            .find(rider)
            .is_some_and(|o| o.is_kind_of(KINDOF_PORTABLE_STRUCTURE));
        let stealthed = world
            .objects
            .find(me.object)
            .is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED));
        if portable && stealthed != crate::logic::modules::plant(1461) {
            out(self, world, me, |w| {
                w.contain_stealth_receive_grant(rider, true, 0)
            });
        }
    }

    /// `onRemoving(obj)`: the base's, then the weapon bonus and `DISABLED_HELD` cleared.
    fn t_on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        tc_on_removing(self, world, me, rider);
        // "give the object back a regular weapon"
        if !crate::logic::modules::plant(1462) {
            out(self, world, me, |w| {
                w.clear_weapon_bonus_condition(rider, GARRISONED)
            });
        }
        out(self, world, me, |w| {
            w.contain_clear_disabled(rider, DISABLED_HELD)
        });
    }

    /// `isSpecialOverlordStyleContainer()`: `TRUE`.
    fn t_is_special_overlord_style_container(&self) -> bool {
        true
    }

    /// `clientVisibleContainedFlashAsSelected()`: the portable structure's drawable flashes; the rest as the base.
    fn t_other_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::ClientVisibleContainedFlashAsSelected => {
                if self.portable_structure_id != INVALID_ID {
                    let portable = self.get_portable_structure(world);
                    if portable != INVALID_ID
                        && world
                            .objects
                            .find(portable)
                            .is_some_and(|o| o.is_kind_of(KINDOF_PORTABLE_STRUCTURE))
                        && world.has_drawable(portable)
                    {
                        world.emit(ObjectClientEvent::DrawableFlashAsSelected {
                            object_id: portable,
                        });
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::IsSpecificRiderFreeToExit { obj } => {
                ModuleReply::Bool(self.is_specific_rider_free_to_exit(world, me, obj))
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl UpdateBehavior for HelixContain {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        self.tc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HelixContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HelixContain(m) => Some(m),
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

impl Snapshot for HelixContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.tc, xfer)
    }

    /// `xfer` (version 2): the version, `m_portableStructureID`, then `TransportContain::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.xfer_own_fields(xfer)?;
        Snapshot::xfer(&mut self.tc, xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.tc);
    }
}
