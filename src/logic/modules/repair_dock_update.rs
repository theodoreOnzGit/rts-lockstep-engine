// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RepairDockUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RepairDockUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RepairDockUpdate`: a [`DockUpdate`] whose `action()` repairs the docker: on the first action of a repair it works out the health to
//! add each frame (the missing health over `TimeForFullHeal`), then heals the docker's body by that much each action (and its drone, if it
//! has one, fully) until the docker is at its maximum. Docks of this kind give their dockers a final command to their rally point.
//!
//! The action is [`ModuleCall::DockAction`]; the rest is [`DockUpdate::dock_call`].
//!
//! # Upstream facts kept
//!
//! - The repair starts when `m_lastRepair == 0`, which is `INVALID_ID`: a second docker starting while the first is unfinished does not
//!   restart the rate, and `m_lastRepair` is cleared only when a docker is full.
//! - The healing goes straight to the body (`body->attemptHealing`, not `Object::attemptHealing`), from the dock, as `DAMAGE_HEALING`,
//!   `DEATH_NONE`.
//! - The xfer is version 1: the version, `DockUpdate`'s (its own version 1, the `UpdateModule` base, its fields), then `m_lastRepair` and
//!   `m_healthToAddPerFrame`.
//!
//! # Where upstream is undefined
//!
//! A docker (or drone) with no body is read through `NULL` (`NullBodyModule`); recorded in `world.cuts.module_errors`, the action
//! returns `FALSE`.
//!
//! `p1w13-sonnet`, batch R43, 2026-10-24.

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateSleepTime};
use crate::logic::modules::dock_update::{new_dock_update, DockUpdate, DockUpdateModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class RepairDockUpdateModuleData : public DockUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct RepairDockUpdateModuleData {
    /// The base.
    pub dock: DockUpdateModuleData,
    /// `m_framesForFullHeal`: "time (in frames) something becomes fully repaired".
    pub frames_for_full_heal: f32,
}

impl Default for RepairDockUpdateModuleData {
    /// `RepairDockUpdateModuleData()`: "1 frame, instant heal by default (keeps away from divide by 0's)".
    fn default() -> Self {
        RepairDockUpdateModuleData { dock: DockUpdateModuleData::default(), frames_for_full_heal: 1.0 }
    }
}

/// `DockUpdateModuleData::buildFieldParse`'s table.
static DOCK_FIELD_PARSE: [FieldParse<RepairDockUpdateModuleData>; 3] = crate::dock_update_field_parse!(RepairDockUpdateModuleData, dock);

/// `RepairDockUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<RepairDockUpdateModuleData>; 2] = [
    FieldParse::new(
        "TimeForFullHeal",
        |i, d, _| {
            d.frames_for_full_heal = i.parse_duration_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl RepairDockUpdateModuleData {
    /// `buildFieldParse(p)`: `DockUpdateModuleData`'s (the update base, then its fields), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RepairDockUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DOCK_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RepairDockUpdateModuleData> {
        let mut data = RepairDockUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RepairDockUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RepairDockUpdate(Arc::new(RepairDockUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class RepairDockUpdate : public DockUpdate`.
#[derive(Clone, Debug)]
pub struct RepairDockUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<RepairDockUpdateModuleData>,
    /// The `DockUpdate` base's state.
    dock: DockUpdate,
    /// `m_lastRepair`: "object we were repairing last".
    last_repair: ObjectId,
    /// `m_healthToAddPerFrame`: "health to add per frame to current docked object".
    health_to_add_per_frame: f32,
}

impl PartialEq for RepairDockUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl RepairDockUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RepairDockUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.dock.update.clone())), SleepyUpdatePhase::Normal)?;
        let dock = new_dock_update(Arc::new(data.dock.clone()))?;
        Ok(RepairDockUpdate { update, data, dock, last_repair: INVALID_ID, health_to_add_per_frame: 0.0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares: the dock's, then `m_lastRepair` and `m_healthToAddPerFrame`.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.dock.state_words();
        w.push(self.last_repair as u32);
        w.push(self.health_to_add_per_frame.to_bits());
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

    /// `xfer` with the scheduling half the slot holds: this class's version, then `DockUpdate::xfer` (its version, the base, its fields),
    /// then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut dock_version: XferVersion = 1;
        xfer.xfer_version(&mut dock_version, 1)?;
        update.xfer(xfer)?;
        self.dock.xfer_fields(xfer)?;
        xfer.xfer_object_id(&mut self.last_repair)?;
        if !crate::logic::modules::plant(2138) {
            xfer.xfer_real(&mut self.health_to_add_per_frame)?;
        }
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `action(docker, drone)`: "Return TRUE to continue the docking process, Return FALSE to complete the dockin process".
    fn action(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId, drone: ObjectId) -> Result<bool, Undefined> {
        if world.objects.find(docker).is_none() {
            return Ok(false);
        }
        let body = world.get_body_module(docker);
        let health_of = |world: &mut World, b| {
            let health = world.module_call(docker, b, ModuleCall::GetHealth).as_real(0.0);
            let max_health = world.module_call(docker, b, ModuleCall::GetMaxHealth).as_real(0.0);
            (health, max_health)
        };
        if self.last_repair == 0 {
            let b = body.ok_or(Undefined::NullBodyModule)?;
            let (health, max_health) = health_of(world, b);
            self.last_repair = docker;
            self.health_to_add_per_frame = (max_health - health) / self.data.frames_for_full_heal;
        }
        let b = body.ok_or(Undefined::NullBodyModule)?;
        let (health, max_health) = health_of(world, b);
        if health >= max_health && !crate::logic::modules::plant(2139) {
            self.last_repair = INVALID_ID;
            return Ok(false);
        }
        let mut healing = DamageInfo::default();
        healing.input.amount = self.health_to_add_per_frame;
        healing.input.source_id = me.object;
        healing.input.damage_type = DAMAGE_HEALING;
        healing.input.death_type = DEATH_NONE;
        world.module_call(docker, b, ModuleCall::AttemptHealing(healing.clone()));
        if world.objects.find(drone).is_some() {
            let db = world.get_body_module(drone).ok_or(Undefined::NullBodyModule)?;
            healing.input.amount = world.module_call(drone, db, ModuleCall::GetMaxHealth).as_real(0.0);
            world.module_call(drone, db, ModuleCall::AttemptHealing(healing));
        }
        Ok(true)
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }
}

impl UpdateBehavior for RepairDockUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.dock.update(world, me)
    }
    /// The calls of `DockUpdateInterface` and the action.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let ModuleCall::DockAction { docker, drone } = call {
            return match self.action(world, me, docker, drone) {
                Ok(b) => ModuleReply::Bool(b),
                Err(u) => {
                    Self::undefined(world, me, u);
                    ModuleReply::Bool(false)
                }
            };
        }
        // "A minority of docks want to give you a final command to their rally point"
        match self.dock.dock_call(world, me, &call, true) {
            Some(Ok(reply)) => reply,
            Some(Err(u)) => {
                Self::undefined(world, me, u);
                ModuleReply::Done
            }
            None => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RepairDockUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RepairDockUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RepairDockUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: see [`RepairDockUpdate::xfer_with`].
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
    fn the_defaults_are_the_constructors() {
        let d = RepairDockUpdateModuleData::default();
        assert_eq!((d.frames_for_full_heal, d.dock.number_approach_positions_data, d.dock.is_allow_passthrough), (1.0, 0, true));
        let m = RepairDockUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[9..17], [0, u32::MAX, 0, 0, 0, 0, 1, 0], "no approach positions, the bones unread, the dock open");
    }
}
