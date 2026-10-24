// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DefaultProductionExitUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DefaultProductionExitUpdate`: how a factory puts out what it made. A new object stands at `UnitCreatePoint` (in the factory's frame, on the
//! factory's layer at that layer's height), faces the factory's way, joins the pathfinder's map, and its AI follows a path to the natural rally
//! point (`NaturalRallyPoint` pushed out two cells) and, if the player set a rally point and the unit moves on the ground, on to the rally point
//! where the pathfinder adjusts it to. The door is always free; budding does nothing; `UseSpawnRallyPoint` lets spawned units use the rally
//! point.
//!
//! The exit interface is the module calls `ExitObjectViaDoor`, `ReserveDoorForExit`, `UnreserveDoorForExit`, `ExitObjectByBudding`,
//! `SetRallyPoint`, `GetRallyPoint`, `UseSpawnRallyPoint`, `GetNaturalRallyPoint`, `GetNaturalRallyPointWithOffset`, `GetExitPosition` and
//! `IsExitBusy`; the object's moves are the stub-aware `World::contain_*` calls the contain modules use.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
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

/// `class DefaultProductionExitUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefaultProductionExitUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_unitCreatePoint`.
    pub unit_create_point: Coord3D,
    /// `m_naturalRallyPoint`.
    pub natural_rally_point: Coord3D,
    /// `m_useSpawnRallyPoint`.
    pub use_spawn_rally_point: bool,
}

/// `DefaultProductionExitUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<DefaultProductionExitUpdateModuleData>; 4] = [
    FieldParse::new("UnitCreatePoint", |ini, d, _| { d.unit_create_point = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("NaturalRallyPoint", |ini, d, _| { d.natural_rally_point = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("UseSpawnRallyPoint", |ini, d, _| { d.use_spawn_rally_point = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl DefaultProductionExitUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DefaultProductionExitUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DefaultProductionExitUpdateModuleData> {
        let mut data = DefaultProductionExitUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DefaultProductionExitUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class DefaultProductionExitUpdate : public UpdateModule, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct DefaultProductionExitUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getDefaultProductionExitUpdateModuleData()`.
    data: Arc<DefaultProductionExitUpdateModuleData>,
    /// `m_rallyPoint`, `m_rallyPointExists`.
    rally: RallyPoint,
}

impl PartialEq for DefaultProductionExitUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same rally point.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.rally == other.rally
    }
}

impl DefaultProductionExitUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data), no rally point.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<DefaultProductionExitUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(DefaultProductionExitUpdate { update, data, rally: RallyPoint::default() })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<DefaultProductionExitUpdateModuleData> {
        &self.data
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
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

    /// The module's state as words, for the tests that compare it with upstream's: the rally point and whether it exists.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.rally.point.x.to_bits(), self.rally.point.y.to_bits(), self.rally.point.z.to_bits(), u32::from(self.rally.exists)]
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, creation: ObjectId, new_obj: ObjectId) {
        let Some((exit_angle, layer)) = world.objects.find(creation).map(|o| (o.thing.orientation(), o.layer)) else { return };
        let d = self.data.clone();
        let mut create_point = transform_point(world, creation, &d.unit_create_point);
        create_point.z = world.terrain_get_layer_height(create_point.x, create_point.y, layer);
        world.contain_set_position(new_obj, &create_point);
        world.contain_set_orientation(new_obj, exit_angle);
        world.contain_set_layer(new_obj, layer);
        // "@todo This really should be automatically wrapped up in an actication sequence for objects in general"
        world.contain_pathfind_add_object(new_obj);
        let mut tmp = natural_rally_point(world, creation, &d.natural_rally_point, true);
        let mut exit_path = vec![tmp];
        let ai = world.objects.find(new_obj).and_then(|o| o.ai);
        if self.rally.exists && !crate::logic::modules::plant(1841) {
            tmp = self.rally.point;
            if let Some(ai) = ai {
                if world.module_call(new_obj, ai, ModuleCall::AiIsDoingGroundMovement).as_bool() && world.contain_pathfind_adjust_destination(new_obj, &mut tmp) {
                    exit_path.push(tmp);
                }
            }
        }
        if let Some(ai) = ai {
            world.module_call(new_obj, ai, ModuleCall::AiFollowExitProductionPath { path: exit_path, ignore_object: creation, command_source: CMD_FROM_AI });
        }
    }
}

impl UpdateBehavior for DefaultProductionExitUpdate {
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
    /// The fields after the base: the rally point and whether it exists.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_coord3d(&mut self.rally.point)?;
        if crate::logic::modules::plant(1842) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.rally.exists)
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { .. } => ModuleReply::Int(DOOR_1),
            ModuleCall::ExitObjectViaDoor { obj, .. } => {
                self.exit_object_via_door(world, me.object, obj);
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_) | ModuleCall::ExitObjectByBudding { .. } => ModuleReply::Done,
            ModuleCall::SetRallyPoint(p) => {
                self.rally.set(&p);
                ModuleReply::Done
            }
            ModuleCall::GetRallyPoint => self.rally.get().map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            // "Check if the building has requested spawn units (like those that are airdropped) to use the rally points of the building."
            ModuleCall::UseSpawnRallyPoint => ModuleReply::Bool(self.data.use_spawn_rally_point),
            ModuleCall::GetNaturalRallyPoint => ModuleReply::Coord(natural_rally_point(world, me.object, &self.data.natural_rally_point, true)),
            ModuleCall::GetNaturalRallyPointWithOffset(offset) => ModuleReply::Coord(natural_rally_point(world, me.object, &self.data.natural_rally_point, offset)),
            ModuleCall::GetExitPosition => exit_position(world, me.object, &self.data.unit_create_point).map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DefaultProductionExitUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DefaultProductionExitUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for DefaultProductionExitUpdate {
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

/// The factory's `NewModuleDataProc` for `DefaultProductionExitUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DefaultProductionExitUpdate(Arc::new(DefaultProductionExitUpdateModuleData::friend_new_module_data(ini)?)))
}
