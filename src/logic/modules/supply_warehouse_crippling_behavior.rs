// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SupplyWarehouseCripplingBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SupplyWarehouseCripplingBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SupplyWarehouseCripplingBehavior`: a supply warehouse that stops working when it is badly hurt and heals itself. Entering the really damaged
//! state cripples the object's dock, leaving it uncripples it. Any damage holds off the self-healing for `SelfHealSupression` frames and wakes the
//! module then; awake, it heals `SelfHealAmount` every `SelfHealDelay` frames until the body is at full health.
//!
//! The damage module calls are [`ModuleCall::OnDamage`] and [`ModuleCall::OnBodyDamageStateChange`]; the dock's is
//! [`ModuleCall::DockSetDockCrippled`].
//!
//! # Upstream facts kept
//!
//! - The dock and the body are read unchecked ([`Undefined::NullObject`], [`Undefined::NullBodyModule`]).
//! - The sleeps are `UnsignedInt` differences (wrapping) and the full-health test is an exact `Real` comparison.
//! - Healing is not `onHealing` (which a damage module also gets): "this can't be in onHealing, as the healing comes from here".

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::global_data::BODY_REALLYDAMAGED;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class SupplyWarehouseCripplingBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupplyWarehouseCripplingBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_selfHealSupression`: "Time since last damage until I can start to heal".
    pub self_heal_supression: u32,
    /// `m_selfHealDelay`: "Once I am okay to heal, how often to do so".
    pub self_heal_delay: u32,
    /// `m_selfHealAmount`: "And how much".
    pub self_heal_amount: f32,
}

/// `SupplyWarehouseCripplingBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<SupplyWarehouseCripplingBehaviorModuleData>; 4] = [
    FieldParse::new(
        "SelfHealSupression",
        |ini, d, _| {
            d.self_heal_supression = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SelfHealDelay",
        |ini, d, _| {
            d.self_heal_delay = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SelfHealAmount",
        |ini, d, _| {
            d.self_heal_amount = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl SupplyWarehouseCripplingBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, SupplyWarehouseCripplingBehaviorModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<SupplyWarehouseCripplingBehaviorModuleData> {
        let mut data = SupplyWarehouseCripplingBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                SupplyWarehouseCripplingBehaviorModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class SupplyWarehouseCripplingBehavior : public UpdateModule, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct SupplyWarehouseCripplingBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getSupplyWarehouseCripplingBehaviorModuleData()`.
    data: Arc<SupplyWarehouseCripplingBehaviorModuleData>,
    /// `m_healingSupressedUntilFrame`.
    healing_supressed_until_frame: u32,
    /// `m_nextHealingFrame`.
    next_healing_frame: u32,
}

impl PartialEq for SupplyWarehouseCripplingBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

impl SupplyWarehouseCripplingBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data), the frames 0.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<SupplyWarehouseCripplingBehaviorModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(SupplyWarehouseCripplingBehavior {
            update,
            data,
            healing_supressed_until_frame: 0,
            next_healing_frame: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getSupplyWarehouseCripplingBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<SupplyWarehouseCripplingBehaviorModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the two frames.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.healing_supressed_until_frame, self.next_healing_frame]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: asleep for ever.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
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

    /// `resetSelfHealSupression()`.
    fn reset_self_heal_supression(&mut self, world: &World) {
        let now = world.frame();
        self.healing_supressed_until_frame = now.wrapping_add(self.data.self_heal_supression);
        self.next_healing_frame = if crate::logic::modules::plant(1821) {
            now
        } else {
            self.healing_supressed_until_frame
        };
    }

    /// `myDock->setDockCrippled(setting)` (`startCrippledEffects`, `stopCrippledEffects`); the dock is read unchecked.
    fn set_dock_crippled(world: &mut World, me: ObjectId, setting: bool) -> Result<(), IniError> {
        let dock = world
            .get_dock_update_interface(me)
            .ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        world.module_call(me, dock, ModuleCall::DockSetDockCrippled(setting));
        Ok(())
    }

    /// The body of `update()`. An error is upstream's undefined behaviour.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<UpdateSleepTime, IniError> {
        let now = world.frame();
        self.next_healing_frame = now.wrapping_add(self.data.self_heal_delay);
        world.attempt_healing(me, self.data.self_heal_amount, INVALID_ID);
        let body = world
            .get_body_module(me)
            .ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        let health = world
            .module_call(me, body, ModuleCall::GetHealth)
            .as_real(0.0);
        let max_health = world
            .module_call(me, body, ModuleCall::GetMaxHealth)
            .as_real(0.0);
        if health == max_health && !crate::logic::modules::plant(1822) {
            return Ok(UPDATE_SLEEP_FOREVER); // this can't be in onHealing, as the healing comes from here
        }
        Ok(update_sleep(self.next_healing_frame.wrapping_sub(now)))
    }
}

impl UpdateBehavior for SupplyWarehouseCripplingBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run(world, me.object) {
            Ok(t) => t,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_FOREVER
            }
        }
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The fields after the base: the two frames.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.healing_supressed_until_frame)?;
        if crate::logic::modules::plant(1823) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.next_healing_frame)
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            // "Damage has been dealt, this is an opportunity to react to that damage"
            ModuleCall::OnDamage(_) => {
                let now = world.frame();
                self.reset_self_heal_supression(world);
                // "we got hit, time to get up for work after a quick snooze"
                update_behavior::set_wake_frame(
                    world,
                    update_sleep(self.healing_supressed_until_frame.wrapping_sub(now)),
                );
                ModuleReply::Done
            }
            ModuleCall::OnBodyDamageStateChange {
                old_state,
                new_state,
                ..
            } => {
                let r = if new_state == BODY_REALLYDAMAGED {
                    Self::set_dock_crippled(world, me.object, true)
                } else if old_state == BODY_REALLYDAMAGED && !crate::logic::modules::plant(1824) {
                    Self::set_dock_crippled(world, me.object, false)
                } else {
                    Ok(())
                };
                if let Err(e) = r {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SupplyWarehouseCripplingBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SupplyWarehouseCripplingBehavior {
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

/// The factory's `NewModuleDataProc` for `SupplyWarehouseCripplingBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SupplyWarehouseCripplingBehavior(Arc::new(
        SupplyWarehouseCripplingBehaviorModuleData::friend_new_module_data(ini)?,
    )))
}
