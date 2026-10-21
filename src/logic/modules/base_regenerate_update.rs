// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BaseRegenerateUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BaseRenerateUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, July 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BaseRegenerateUpdate`: the automatic healing of base structures. Damage
//! (other than healing) starts a delay, and after it the object heals a
//! share of its maximum health every three frames, until it is at full
//! health, sold, or the global rate is zero.
//!
//! The module is a damage module as well (`onDamage`). The world calls
//! [`BaseRegenerateUpdate::on_damage`] from its damage dispatch (layer 9's
//! wiring); the `onHealing` and `onBodyDamageStateChange` hooks of upstream
//! are empty and are not translated.
//!
//! # Differences from upstream
//!
//! - `TheGlobalData` and the object's body and status bits go through
//!   [`BaseRegenHost`]; the healing goes through `attempt_healing`, with the
//!   object as its source, as upstream's `attemptHealing(amount, me)`.
//! - The healing amount is computed in `f32` in the upstream order:
//!   `HEAL_RATE * (maxHealth * percent) / LOGICFRAMES_PER_SECOND`.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageType, DAMAGE_HEALING};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
// --- p1w10-sonnet ---
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
// --- end p1w10-sonnet ---
use crate::logic::modules::UpdateHost;

/// The calls `BaseRegenerateUpdate` makes on the global data, its object's
/// status bits and its body, besides the [`UpdateHost`] ones.
pub trait BaseRegenHost: UpdateHost {
    /// `TheGlobalData->m_baseRegenHealthPercentPerSecond`.
    fn base_regen_health_percent_per_second(&self) -> f32;
    /// `TheGlobalData->m_baseRegenDelay` (frames).
    fn base_regen_delay(&self) -> u32;
    /// `getObject()->testStatus(OBJECT_STATUS_UNDER_CONSTRUCTION)`.
    fn is_under_construction(&self) -> bool;
    /// `getObject()->testStatus(OBJECT_STATUS_SOLD)`.
    fn is_sold(&self) -> bool;
    /// `getBodyModule()->getMaxHealth()`.
    fn max_health(&mut self) -> f32;
    /// `getBodyModule()->getHealth()`.
    fn health(&mut self) -> f32;
    /// `getObject()->attemptHealing(amount, getObject())`.
    fn attempt_healing(&mut self, amount: f32);
}

/// `class BaseRegenerateUpdateModuleData : public UpdateModuleData`: no fields of its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BaseRegenerateUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
}

impl BaseRegenerateUpdateModuleData {
    /// `BaseRegenerateUpdateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `BaseRegenerateUpdateModuleData::buildFieldParse(p)`: the base's, then
    /// an empty table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, BaseRegenerateUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_REGENERATE_UPDATE_MODULE_DATA_FIELD_PARSE)
    }

    /// `BaseRegenerateUpdate::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<BaseRegenerateUpdateModuleData> {
        let mut data = BaseRegenerateUpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, BaseRegenerateUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The empty `dataFieldParse` table of the class.
pub static BASE_REGENERATE_UPDATE_MODULE_DATA_FIELD_PARSE: [FieldParse<BaseRegenerateUpdateModuleData>; 1] = [FieldParse::end()];

/// `UpdateSleepTime` of `HEAL_RATE` frames between heals.
const HEAL_RATE: i32 = 3;

/// `class BaseRegenerateUpdate : public UpdateModule, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct BaseRegenerateUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getBaseRegenerateUpdateModuleData()`.
    data: Arc<BaseRegenerateUpdateModuleData>,
}

impl BaseRegenerateUpdate {
    /// `BaseRegenerateUpdate(thing, moduleData)`: asleep for ever when the
    /// global rate is zero, else due next frame. `INI_INVALID_DATA` for null
    /// data.
    pub fn new(object: ObjectId, module_data: Option<Arc<BaseRegenerateUpdateModuleData>>, host: &mut impl BaseRegenHost) -> IniResult<Self> {
        let mut me = BaseRegenerateUpdate::unconstructed(object, module_data)?;
        me.construct(host);
        Ok(me)
    }

    // --- p1w10-sonnet ---
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<BaseRegenerateUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let mut update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        // p1w11-sonnet: `getDisabledTypesToProcess()` is `MAKE_DISABLED_MASK(DISABLED_UNDERPOWERED)`; the scheduler reads the arena's base
        let mut mask = crate::common::disabled_types::DisabledMaskType::new();
        mask.set(crate::common::disabled_types::DISABLED_UNDERPOWERED, 1);
        update.set_disabled_types_to_process(mask);
        Ok(BaseRegenerateUpdate { update, data })
    }

    /// The second half of the constructor: `setWakeFrame` (forever when the global rate is zero, else every frame). The world
    /// runs it once the module is part of its object.
    pub fn construct(&mut self, host: &mut impl BaseRegenHost) {
        let wake = if host.base_regen_health_percent_per_second() == 0.0 { UPDATE_SLEEP_FOREVER } else { UPDATE_SLEEP_NONE };
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), wake));
    }
    // --- end p1w10-sonnet ---

    /// `onDamage(damageInfo)`: damage other than healing, with a rate, starts
    /// the delay before healing; anything else sleeps for ever. `damage_type`
    /// is `damageInfo->in.m_damageType`.
    pub fn on_damage(&mut self, host: &mut impl BaseRegenHost, damage_type: DamageType) {
        let wake = if host.base_regen_health_percent_per_second() > 0.0 && damage_type != DAMAGE_HEALING {
            update_sleep(host.base_regen_delay())
        } else {
            UPDATE_SLEEP_FOREVER
        };
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), wake));
    }

    /// `update()`: heal a little every `HEAL_RATE` frames, or sleep (until
    /// damaged again, or for ever when sold).
    pub fn update(&mut self, host: &mut impl BaseRegenHost) -> UpdateSleepTime {
        if host.is_under_construction() {
            return UPDATE_SLEEP_NONE;
        }
        if host.is_sold() {
            return UPDATE_SLEEP_FOREVER;
        }
        let max = host.max_health();
        if max == host.health() {
            return UPDATE_SLEEP_FOREVER;
        }
        let amount = (HEAL_RATE as f32 * (max * host.base_regen_health_percent_per_second())) / LOGICFRAMES_PER_SECOND as f32;
        host.attempt_healing(amount);
        update_sleep(HEAL_RATE as u32)
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getBaseRegenerateUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<BaseRegenerateUpdateModuleData> {
        &self.data
    }
}

impl Snapshot for BaseRegenerateUpdate {
    /// `BaseRegenerateUpdate::crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// Version 1: the base.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for BaseRegenerateUpdate {
    /// The same state: the same data object (upstream compares pointers). (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

/// The factory's `NewModuleDataProc` for `BaseRegenerateUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<crate::common::module::ModuleDataKind> {
    Ok(crate::common::module::ModuleDataKind::BaseRegenerateUpdate(Arc::new(BaseRegenerateUpdateModuleData::friend_new_module_data(ini)?)))
}

impl BaseRegenerateUpdate {
    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot for the
    /// duration, so that a call that reaches it from inside (`destroyObject` runs `onDelete` on every module of the object, this
    /// one included) is delivered at once on its live state, as upstream's plain recursion delivers it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.update(world);
        };
        let lent = crate::logic::world::update_body::UpdateBody::BaseRegenerateUpdate(self.clone());
        let (r, back) = world.lend_body(h, lent, |w| w.in_update_module(me, |w| self.update(w)));
        if let Some(crate::logic::world::update_body::UpdateBody::BaseRegenerateUpdate(b)) = back {
            *self = b;
        }
        r
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.construct(world);
    }

    /// The module's answer to one of `Object`'s calls: `onDamage` of the damage interface.
    pub fn module_call(&mut self, world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDamage(info) => {
                self.on_damage(world, info.input.damage_type);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// The fields after the base: none.
    fn xfer_fields(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer` with the scheduling half the slot holds (the live `UpdateModule`): the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        update.xfer(xfer)?;
        self.xfer_fields(xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update.crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update.load_post_process();
    }
}
// --- end p1w10-sonnet ---
