// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SupplyWarehouseDockUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyWarehouseDockUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SupplyWarehouseDockUpdate`: a [`DockUpdate`] that holds supply boxes and hands one to each supply truck's action: a truck farther than
//! twice its bounding radius is nudged by a random step (up to 4 in x and y, `0.4 * PATHFIND_CELL_SIZE_F` truncated to `Int`) and turned
//! away; a closer one takes a box if its supply-truck AI will take it (`gainOneBox`), else the box is put back. An empty warehouse that
//! deletes itself is destroyed. Crippling it kills the truck inside (unless it flies) or sends the waiting one back to wanting supplies.
//! The drawable shows the boxes (`updateDrawableSupplyStatus`).
//!
//! The calls: [`ModuleCall::DockAction`], [`ModuleCall::DockSetDockCrippled`] (this class's), [`ModuleCall::SupplyWarehouseSetCashValue`],
//! [`ModuleCall::SupplyWarehouseGetBoxesStored`], [`ModuleCall::OnObjectCreated`] and [`ModuleCall::LoadPostProcess`] (the drawable's
//! status after a load); the rest is [`DockUpdate::dock_call`].
//!
//! # Upstream facts kept
//!
//! - The box is taken before the truck is asked ("so the docker sees that I am shy by one box (or empty) from within his gainOneBox()").
//! - `setCashValue` is `ceil(cash / (float)m_baseValuePerSupplyBox)`.
//! - The xfer is version 1: the version, `DockUpdate`'s, then `m_boxesStored`.
//!
//! # Where upstream is undefined
//!
//! Recorded in `world.cuts.module_errors`: a docker with no AI (`getAIUpdateInterface()->` through `NULL`: `NullAi`), a crippled dock's
//! waiting victim with no AI (`NullAi`), a victim inside with no body (`Object::kill`: `NullBodyModule`), and a cash value whose box count
//! does not fit an `Int` (`FloatToIntOutOfRange`).
//!
//! `p1w13-sonnet`, batch R44, 2026-10-24.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateSleepTime};
use crate::logic::modules::dock_update::{new_dock_update, DockUpdate, DockUpdateModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `PATHFIND_CELL_SIZE_F`.
const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// `class SupplyWarehouseDockUpdateModuleData : public DockUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SupplyWarehouseDockUpdateModuleData {
    /// The base.
    pub dock: DockUpdateModuleData,
    /// `m_startingBoxesData`.
    pub starting_boxes_data: i32,
    /// `m_deleteWhenEmpty`.
    pub delete_when_empty: bool,
}

impl Default for SupplyWarehouseDockUpdateModuleData {
    /// `SupplyWarehouseDockUpdateModuleData()`.
    fn default() -> Self {
        SupplyWarehouseDockUpdateModuleData { dock: DockUpdateModuleData::default(), starting_boxes_data: 1, delete_when_empty: false }
    }
}

/// `DockUpdateModuleData::buildFieldParse`'s table.
static DOCK_FIELD_PARSE: [FieldParse<SupplyWarehouseDockUpdateModuleData>; 3] = crate::dock_update_field_parse!(SupplyWarehouseDockUpdateModuleData, dock);

/// `SupplyWarehouseDockUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<SupplyWarehouseDockUpdateModuleData>; 3] = [
    FieldParse::new(
        "StartingBoxes",
        |i, d, _| {
            d.starting_boxes_data = i.parse_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DeleteWhenEmpty",
        |i, d, _| {
            d.delete_when_empty = i.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl SupplyWarehouseDockUpdateModuleData {
    /// `buildFieldParse(p)`: `DockUpdateModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SupplyWarehouseDockUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DOCK_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SupplyWarehouseDockUpdateModuleData> {
        let mut data = SupplyWarehouseDockUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SupplyWarehouseDockUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SupplyWarehouseDockUpdate(Arc::new(SupplyWarehouseDockUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SupplyWarehouseDockUpdate : public DockUpdate`.
#[derive(Clone, Debug)]
pub struct SupplyWarehouseDockUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<SupplyWarehouseDockUpdateModuleData>,
    /// The `DockUpdate` base's state.
    dock: DockUpdate,
    /// `m_boxesStored`.
    boxes_stored: i32,
}

impl PartialEq for SupplyWarehouseDockUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl SupplyWarehouseDockUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SupplyWarehouseDockUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.dock.update.clone())), SleepyUpdatePhase::Normal)?;
        let dock = new_dock_update(Arc::new(data.dock.clone()))?;
        let boxes_stored = data.starting_boxes_data;
        Ok(SupplyWarehouseDockUpdate { update, data, dock, boxes_stored })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares: the dock's, then `m_boxesStored`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.dock.state_words();
        w.push(self.boxes_stored as u32);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half (nothing).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: this class's version, `DockUpdate::xfer`, then `m_boxesStored`.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut dock_version: XferVersion = 1;
        xfer.xfer_version(&mut dock_version, 1)?;
        update.xfer(xfer)?;
        self.dock.xfer_fields(xfer)?;
        if !crate::logic::modules::plant(2141) {
            xfer.xfer_int(&mut self.boxes_stored)?;
        }
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's (the drawable's part is [`ModuleCall::LoadPostProcess`]).
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `draw->updateDrawableSupplyStatus(m_startingBoxesData, m_boxesStored)`, if there is a drawable.
    fn update_drawable_supply_status(&self, world: &mut World, me: ObjectId) {
        if world.has_drawable(me) {
            world.emit(ObjectClientEvent::DrawableUpdateSupplyStatus { object_id: me, max_boxes: self.data.starting_boxes_data, current_boxes: self.boxes_stored });
        }
    }

    /// `action(docker, drone)`.
    fn action(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<bool, Undefined> {
        if self.boxes_stored == 0 {
            return Ok(false);
        }
        let us = me.object;
        let d = world.objects.find(docker).ok_or(Undefined::NullObject)?;
        let close_enough = d.geometry_info.get_bounding_circle_radius() * 2.0;
        let close_enough_sqr = close_enough * close_enough;
        let cur_dist_sqr = world.body_get_distance_squared(docker, us);
        if cur_dist_sqr > close_enough_sqr && !crate::logic::modules::plant(2142) {
            let mut new_pos = world.objects.find(docker).ok_or(Undefined::NullObject)?.get_position();
            // `Real range = 0.4*PATHFIND_CELL_SIZE_F;`, then `GameLogicRandomValue(-range, range)` truncates it to `Int`
            let range = (0.4f64 * f64::from(PATHFIND_CELL_SIZE_F)) as f32;
            let (lo, hi) = if crate::logic::modules::plant(2149) { (-3, 3) } else { (-range as i32, range as i32) };
            new_pos.x += world.random.game_logic_random_value(lo, hi) as f32;
            new_pos.y += world.random.game_logic_random_value(lo, hi) as f32;
            world.set_position(docker, &new_pos);
            return Ok(false); // "not close enough."
        }
        // "so the docker sees that I am shy by one box (or empty) from within his gainOneBox()"
        self.boxes_stored -= 1;
        let ai = world.objects.find(docker).and_then(|o| o.ai).ok_or(Undefined::NullAi)?;
        let truck = world.module_call(docker, ai, ModuleCall::HasSupplyTruckAiInterface).as_bool();
        if truck && world.module_call(docker, ai, ModuleCall::SupplyTruckGainOneBox { remaining_stock: self.boxes_stored }).as_bool() {
            if self.boxes_stored == 0 && self.data.delete_when_empty {
                world.body_destroy_object(us);
                return Ok(false); // "Yer done.  And so am I."
            }
            self.update_drawable_supply_status(world, us);
            return Ok(true);
        }
        if !crate::logic::modules::plant(2143) {
            self.boxes_stored += 1; // "take it back, since there was noone to gain the box"
        }
        Ok(false)
    }

    /// `setDockCrippled(setting)`.
    fn set_dock_crippled(&mut self, world: &mut World, me: ModuleHandle, setting: bool) -> Result<(), Undefined> {
        let active = self.dock.active_docker();
        if setting && active != crate::common::game_type::INVALID_ID && world.objects.find(active).is_some() {
            if self.dock.docker_inside() {
                if !world.is_using_airborne_locomotor(active) {
                    world.get_body_module(active).ok_or(Undefined::NullBodyModule)?;
                    if !crate::logic::modules::plant(2144) {
                        world.kill(active, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                    }
                }
            } else {
                let ai = world.objects.find(active).and_then(|o| o.ai).ok_or(Undefined::NullAi)?;
                if world.module_call(active, ai, ModuleCall::HasSupplyTruckAiInterface).as_bool() {
                    world.cut(crate::logic::world::object_cuts::CutCall::AiIdle { object: active }); // `aiIdle(CMD_FROM_AI)`
                    world.module_call(active, ai, ModuleCall::SupplyTruckSetForceWantingState(true));
                }
            }
        }
        if let Some(Err(u)) = self.dock.dock_call(world, me, &ModuleCall::DockSetDockCrippled(setting), false) {
            return Err(u);
        }
        Ok(())
    }

    /// `setCashValue(cashValue)`.
    fn set_cash_value(&mut self, world: &mut World, me: ObjectId, cash_value: i32) -> Result<(), Undefined> {
        let per_box = world.cuts.globals.base_value_per_supply_box as i32;
        let boxes = (cash_value as f32 / per_box as f32).ceil();
        if !(boxes > i32::MIN as f32 - 1.0 && boxes < i32::MAX as f32) {
            return Err(Undefined::FloatToIntOutOfRange);
        }
        self.boxes_stored = boxes as i32;
        self.update_drawable_supply_status(world, me);
        Ok(())
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }
}

impl UpdateBehavior for SupplyWarehouseDockUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.dock.update(world, me)
    }
    /// The calls of `DockUpdateInterface`, the action, this class's own and the drawable's status.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let r = match call {
            ModuleCall::DockAction { docker, .. } => self.action(world, me, docker).map(ModuleReply::Bool),
            ModuleCall::DockSetDockCrippled(setting) => self.set_dock_crippled(world, me, setting).map(|()| ModuleReply::Done),
            ModuleCall::SupplyWarehouseSetCashValue(cash) => self.set_cash_value(world, me.object, cash).map(|()| ModuleReply::Done),
            ModuleCall::SupplyWarehouseGetBoxesStored => Ok(ModuleReply::Int(self.boxes_stored)),
            ModuleCall::OnObjectCreated | ModuleCall::LoadPostProcess => {
                self.update_drawable_supply_status(world, me.object);
                Ok(ModuleReply::Done)
            }
            _ => match self.dock.dock_call(world, me, &call, false) {
                Some(r) => r,
                None => Ok(ModuleReply::Unhandled),
            },
        };
        match r {
            Ok(reply) => reply,
            Err(u) => {
                Self::undefined(world, me, u);
                if matches!(call, ModuleCall::DockAction { .. }) {
                    ModuleReply::Bool(false)
                } else {
                    ModuleReply::Done
                }
            }
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SupplyWarehouseDockUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SupplyWarehouseDockUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SupplyWarehouseDockUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: see [`SupplyWarehouseDockUpdate::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = self.xfer_with(&mut base, xfer);
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
    fn a_warehouse_starts_with_its_boxes() {
        let d = SupplyWarehouseDockUpdateModuleData { starting_boxes_data: 7, ..Default::default() };
        let m = SupplyWarehouseDockUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words().last(), Some(&7));
    }
}
