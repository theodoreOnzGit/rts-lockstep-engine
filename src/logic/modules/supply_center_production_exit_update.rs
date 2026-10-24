// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SupplyCenterProductionExitUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SupplyCenterProductionExitUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SupplyCenterProductionExitUpdate`: the supply centre's exit, a [`DefaultProductionExitUpdate`](super::default_production_exit_update)
//! for supply trucks. A new truck stands at `UnitCreatePoint` (in the centre's frame) at the GROUND's height, faces the centre's way and
//! joins the pathfinder's map; its AI follows a path to the natural rally point (`NaturalRallyPoint`, NOT pushed out) and on to the player's
//! rally point if there is one (not adjusted, whatever the truck moves on), and its supply-truck AI is forced into wanting supplies. A
//! stealthed centre with `GrantTemporaryStealth` grants the truck stealth for that long (if its stealth is itself a grant, or it cannot
//! stealth on its own).
//!
//! The exit interface is the module calls of [`DefaultProductionExitUpdate`](super::default_production_exit_update) (`UseSpawnRallyPoint` is
//! the base's `FALSE`).
//!
//! # Upstream facts kept
//!
//! - The constructor leaves `m_rallyPoint` uninitialised (only `m_rallyPointExists` is set); the xfer saves it. The port starts it at zero
//!   (the C++ driver sets the member to it after construction, R23's precedent).
//! - `m_grantTemporaryStealthFrames` has no default (the data's constructor zeroes only the two points).
//!
//! # Where upstream is undefined
//!
//! Recorded in `world.cuts.module_errors`, the exit returns: the grant read when the data never set it (`UninitialisedModuleData`), and a
//! stealthed centre's grant to a truck with no stealth module (`stealth->` through `NULL`: `NullObject`).
//!
//! `p1w13-sonnet`, batch R45, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_1, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::production_exit::{exit_position, natural_rally_point, transform_point, RallyPoint};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class SupplyCenterProductionExitUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupplyCenterProductionExitUpdateModuleData {
    /// The base.
    pub update: UpdateModuleData,
    /// `m_unitCreatePoint`.
    pub unit_create_point: Coord3D,
    /// `m_naturalRallyPoint`.
    pub natural_rally_point: Coord3D,
    /// `m_grantTemporaryStealthFrames` (`None`: never set, uninitialised upstream).
    pub grant_temporary_stealth_frames: Option<u32>,
}

/// `SupplyCenterProductionExitUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<SupplyCenterProductionExitUpdateModuleData>; 4] = [
    FieldParse::new("UnitCreatePoint", |ini, d, _| { d.unit_create_point = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("NaturalRallyPoint", |ini, d, _| { d.natural_rally_point = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("GrantTemporaryStealth", |ini, d, _| { d.grant_temporary_stealth_frames = Some(ini.parse_duration_unsigned_int()?); Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SupplyCenterProductionExitUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SupplyCenterProductionExitUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SupplyCenterProductionExitUpdateModuleData> {
        let mut data = SupplyCenterProductionExitUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SupplyCenterProductionExitUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SupplyCenterProductionExitUpdate(Arc::new(SupplyCenterProductionExitUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SupplyCenterProductionExitUpdate : public UpdateModule, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct SupplyCenterProductionExitUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<SupplyCenterProductionExitUpdateModuleData>,
    /// `m_rallyPoint`, `m_rallyPointExists`.
    rally: RallyPoint,
}

impl PartialEq for SupplyCenterProductionExitUpdate {
    /// The same object, the same data object and the same rally point.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.rally == other.rally
    }
}

impl SupplyCenterProductionExitUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor's first half (`INI_INVALID_DATA` for null data); the wake is [`UpdateBehavior::construct`]'s.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SupplyCenterProductionExitUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(SupplyCenterProductionExitUpdate { update, data, rally: RallyPoint::default() })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The state the test compares: the rally point and whether it exists.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.rally.point.x.to_bits(), self.rally.point.y.to_bits(), self.rally.point.z.to_bits(), u32::from(self.rally.exists)]
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, creation: ObjectId, new_obj: ObjectId) -> Result<(), Undefined> {
        let Some(exit_angle) = world.objects.find(creation).map(|o| o.thing.orientation()) else { return Ok(()) };
        let d = self.data.clone();
        let mut create_point = transform_point(world, creation, &d.unit_create_point);
        create_point.z = world.terrain_get_ground_height(create_point.x, create_point.y);
        world.contain_set_position(new_obj, &create_point);
        world.contain_set_orientation(new_obj, exit_angle);
        // "@todo This really should be automatically wrapped up in an actication sequence for objects in general"
        world.contain_pathfind_add_object(new_obj);
        let mut exit_path = vec![natural_rally_point(world, creation, &d.natural_rally_point, crate::logic::modules::plant(2151))];
        if self.rally.exists {
            exit_path.push(self.rally.point);
        }
        if let Some(ai) = world.objects.find(new_obj).and_then(|o| o.ai) {
            world.module_call(new_obj, ai, ModuleCall::AiFollowExitProductionPath { path: exit_path, ignore_object: creation, command_source: CMD_FROM_AI });
            if world.module_call(new_obj, ai, ModuleCall::HasSupplyTruckAiInterface).as_bool() && !crate::logic::modules::plant(2152) {
                world.module_call(new_obj, ai, ModuleCall::SupplyTruckSetForceWantingState(true));
            }
        }
        let frames = d.grant_temporary_stealth_frames.ok_or(Undefined::UninitialisedModuleData)?;
        if frames != 0 && world.objects.find(creation).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED)) {
            let stealth = world.objects.find(new_obj).and_then(|o| o.stealth).ok_or(Undefined::NullObject)?;
            let temporary = world.module_call(new_obj, stealth, ModuleCall::StealthIsTemporaryGrant).as_bool();
            let can_stealth = world.objects.find(new_obj).is_some_and(|o| o.test_status(OBJECT_STATUS_CAN_STEALTH));
            if temporary || (!can_stealth && !crate::logic::modules::plant(2153)) {
                world.contain_stealth_receive_grant(new_obj, true, frames);
            }
        }
        Ok(())
    }
}

impl UpdateBehavior for SupplyCenterProductionExitUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, _world: &mut World, _me: ModuleHandle) -> UpdateSleepTime {
        UPDATE_SLEEP_FOREVER
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// `xfer`'s fields after the base: the rally point and whether it exists.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_coord3d(&mut self.rally.point)?;
        if crate::logic::modules::plant(2154) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.rally.exists)
    }
    /// The exit interface.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { .. } => ModuleReply::Int(DOOR_1),
            ModuleCall::ExitObjectViaDoor { obj, .. } => {
                if let Err(u) = self.exit_object_via_door(world, me.object, obj) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_) | ModuleCall::ExitObjectByBudding { .. } => ModuleReply::Done,
            ModuleCall::SetRallyPoint(p) => {
                self.rally.set(&p);
                ModuleReply::Done
            }
            ModuleCall::GetRallyPoint => self.rally.get().map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            ModuleCall::UseSpawnRallyPoint => ModuleReply::Bool(false),
            ModuleCall::GetNaturalRallyPoint => ModuleReply::Coord(natural_rally_point(world, me.object, &self.data.natural_rally_point, true)),
            ModuleCall::GetNaturalRallyPointWithOffset(offset) => ModuleReply::Coord(natural_rally_point(world, me.object, &self.data.natural_rally_point, offset)),
            ModuleCall::GetExitPosition => exit_position(world, me.object, &self.data.unit_create_point).map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SupplyCenterProductionExitUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SupplyCenterProductionExitUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SupplyCenterProductionExitUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
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
    fn the_grant_has_no_default() {
        let d = SupplyCenterProductionExitUpdateModuleData::default();
        assert_eq!(d.grant_temporary_stealth_frames, None);
        let m = SupplyCenterProductionExitUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0, 0, 0]);
    }
}
