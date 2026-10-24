// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SupplyCenterDockUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyCenterDockUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SupplyCenterDockUpdate`: a [`DockUpdate`] that turns a supply truck's boxes into its owner's money. The action unloads every box
//! (`loseOneBox`, each worth `getSupplyBoxValue()`), adds the truck's upgraded boost, deposits the total and scores it as earned; a
//! stealthed centre with `GrantTemporaryStealth` grants the truck stealth for that long (if the truck's stealth is itself a grant, or the
//! truck cannot stealth on its own); the amount floats over the truck at the ground's height, unless the centre is stealthed, undetected
//! and someone else's. The action always ends the docking.
//!
//! The action is [`ModuleCall::DockAction`]; the rest is [`DockUpdate::dock_call`]. The money, the score and the floating text are
//! [`CutCall::PlayerMoneyDeposit`], [`CutCall::PlayerScoreAddMoneyEarned`] and [`CutCall::InGameUiAddFloatingText`].
//!
//! # Upstream facts kept
//!
//! - The total is an `UnsignedInt`; the boost (an `Int`) is added to it, wrapping.
//! - The floating text is at the truck's x and y and the terrain's ground height there, in the owner's colour with alpha 230.
//! - The xfer is version 1: the version and `DockUpdate`'s.
//!
//! # Where upstream is undefined
//!
//! Recorded in `world.cuts.module_errors`: a box, a deposit or a floating text for a centre with no controlling player
//! (`NullControllingPlayer`). A truck with no AI, or whose AI is not a supply truck's, ends the docking; a stealthed centre's grant to a
//! truck with no stealth module is a `DEBUG_CRASH`, nothing in a release build.
//!
//! `p1w13-sonnet`, batch R44, 2026-10-24.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateSleepTime};
use crate::logic::modules::dock_update::{new_dock_update, DockUpdate, DockUpdateModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class SupplyCenterDockUpdateModuleData : public DockUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SupplyCenterDockUpdateModuleData {
    /// The base.
    pub dock: DockUpdateModuleData,
    /// `m_grantTemporaryStealthFrames`.
    pub grant_temporary_stealth_frames: u32,
}

impl Default for SupplyCenterDockUpdateModuleData {
    /// `SupplyCenterDockUpdateModuleData()`.
    fn default() -> Self {
        SupplyCenterDockUpdateModuleData { dock: DockUpdateModuleData::default(), grant_temporary_stealth_frames: 0 }
    }
}

/// `DockUpdateModuleData::buildFieldParse`'s table.
static DOCK_FIELD_PARSE: [FieldParse<SupplyCenterDockUpdateModuleData>; 3] = crate::dock_update_field_parse!(SupplyCenterDockUpdateModuleData, dock);

/// `SupplyCenterDockUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<SupplyCenterDockUpdateModuleData>; 2] = [
    FieldParse::new(
        "GrantTemporaryStealth",
        |i, d, _| {
            d.grant_temporary_stealth_frames = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl SupplyCenterDockUpdateModuleData {
    /// `buildFieldParse(p)`: `DockUpdateModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SupplyCenterDockUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DOCK_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SupplyCenterDockUpdateModuleData> {
        let mut data = SupplyCenterDockUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SupplyCenterDockUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SupplyCenterDockUpdate(Arc::new(SupplyCenterDockUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SupplyCenterDockUpdate : public DockUpdate`.
#[derive(Clone, Debug)]
pub struct SupplyCenterDockUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<SupplyCenterDockUpdateModuleData>,
    /// The `DockUpdate` base's state.
    dock: DockUpdate,
}

impl PartialEq for SupplyCenterDockUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

/// `GameMakeColor(0, 0, 0, 230)`.
const FLOATING_ALPHA: i32 = (230u32 << 24) as i32;

impl SupplyCenterDockUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SupplyCenterDockUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.dock.update.clone())), SleepyUpdatePhase::Normal)?;
        let dock = new_dock_update(Arc::new(data.dock.clone()))?;
        Ok(SupplyCenterDockUpdate { update, data, dock })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares: the dock's.
    pub fn state_words(&self) -> Vec<u32> {
        self.dock.state_words()
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

    /// `xfer` with the scheduling half the slot holds: this class's version, then `DockUpdate::xfer`.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut dock_version: XferVersion = 1;
        xfer.xfer_version(&mut dock_version, 1)?;
        update.xfer(xfer)?;
        self.dock.xfer_fields(xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `action(docker, drone)`: "For me, this means taking boxes and turning them into money for my ownerplayer".
    fn action(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<bool, Undefined> {
        let us = me.object;
        let Some(ai) = world.objects.find(docker).ok_or(Undefined::NullObject)?.ai else { return Ok(false) };
        if !world.module_call(docker, ai, ModuleCall::HasSupplyTruckAiInterface).as_bool() {
            return Ok(false);
        }
        let mut value: u32 = 0;
        let owner = world.get_controlling_player(us);
        while world.module_call(docker, ai, ModuleCall::SupplyTruckLoseOneBox).as_bool() {
            owner.ok_or(Undefined::NullControllingPlayer)?;
            let per_box = if crate::logic::modules::plant(2145) { 1 } else { world.cuts.globals.base_value_per_supply_box };
            value = value.wrapping_add(per_box);
        }
        value = value.wrapping_add(world.module_call(docker, ai, ModuleCall::SupplyTruckGetUpgradedSupplyBoost).as_int(0) as u32);
        let stealthed = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED));
        if value > 0 {
            let player = owner.ok_or(Undefined::NullControllingPlayer)?;
            world.cut(CutCall::PlayerMoneyDeposit { player, amount: value });
            world.cut(CutCall::PlayerScoreAddMoneyEarned { player, amount: value as i32 });
            if self.data.grant_temporary_stealth_frames > 0 && stealthed {
                // "It shouldn't be possible for a unit to be OBJECT_STATUS_STEALTHED without a StealthUpdate module!" (a `DEBUG_CRASH`: nothing
                // in a release build)
                if let Some(stealth) = world.objects.find(docker).and_then(|o| o.stealth) {
                    let temporary = world.module_call(docker, stealth, ModuleCall::StealthIsTemporaryGrant).as_bool();
                    let can_stealth = world.objects.find(docker).is_some_and(|o| o.test_status(OBJECT_STATUS_CAN_STEALTH));
                    if temporary || (!can_stealth && !crate::logic::modules::plant(2146)) {
                        world.contain_stealth_receive_grant(docker, true, self.data.grant_temporary_stealth_frames);
                    }
                }
            }
        }
        let mut display_money = value > 0;
        if stealthed {
            let detected = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_DETECTED));
            if !world.is_locally_controlled(us) && !detected && !crate::logic::modules::plant(2147) {
                display_money = false;
            }
        }
        if display_money {
            let docker_pos = world.objects.find(docker).ok_or(Undefined::NullObject)?.get_position();
            let mut position = docker_pos;
            let ground = world.terrain_get_ground_height(position.x, position.y);
            if !crate::logic::modules::plant(2148) {
                position.z = ground;
            }
            let player = owner.ok_or(Undefined::NullControllingPlayer)?;
            let color = world.cuts.player_color(player) | FLOATING_ALPHA;
            world.cut(CutCall::InGameUiAddFloatingText { amount: value as i32, position, color });
        }
        Ok(false)
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }
}

impl UpdateBehavior for SupplyCenterDockUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// `update()`: `DockUpdate::update()` (the rest is `_DEBUG_ECONOMY` only).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.dock.update(world, me)
    }
    /// The calls of `DockUpdateInterface` and the action.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let ModuleCall::DockAction { docker, .. } = call {
            return match self.action(world, me, docker) {
                Ok(b) => ModuleReply::Bool(b),
                Err(u) => {
                    Self::undefined(world, me, u);
                    ModuleReply::Bool(false)
                }
            };
        }
        match self.dock.dock_call(world, me, &call, false) {
            Some(Ok(reply)) => reply,
            Some(Err(u)) => {
                Self::undefined(world, me, u);
                ModuleReply::Done
            }
            None => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SupplyCenterDockUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SupplyCenterDockUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SupplyCenterDockUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: see [`SupplyCenterDockUpdate::xfer_with`].
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
    use crate::common::game_type::INVALID_ID;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = SupplyCenterDockUpdateModuleData::default();
        assert_eq!(d.grant_temporary_stealth_frames, 0);
        let m = SupplyCenterDockUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[12], INVALID_ID as u32, "no active docker");
    }
}
