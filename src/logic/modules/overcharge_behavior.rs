// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/OverchargeBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `OverchargeBehavior`: a power plant's overcharge. Turned on (`enable`, `toggle`), it extends the rods of every power plant update of the object,
//! adds the object's power bonus to its player and wakes every frame to drain `HealthPercentToDrainPerSecond` of the maximum health per second
//! (penalty damage from the object itself); below `NotAllowedWhenHealthBelowPercent` of the maximum it turns itself off, telling the local
//! player ("GUI:OverchargeExhausted" and a radar event). Turned off, it retracts the rods, removes the bonus and sleeps. Deleted or captured
//! while on, it moves the bonus; after a load it adds it again.
//!
//! The power bonus is [`CutCall::PlayerAddPowerBonus`] / [`CutCall::PlayerRemovePowerBonus`] (as `PowerPlantUpgrade`'s), the rods
//! [`ModuleCall::ExtendRods`] to every module of the object, the drain [`World::attempt_damage`]. The body is read unchecked upstream: none is
//! [`Undefined::NullBodyModule`]. `onDamage` does nothing.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_PENALTY, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `RADAR_EVENT_INFORMATION` (`Radar.h`).
const RADAR_EVENT_INFORMATION: i32 = 4;

/// `class OverchargeBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverchargeBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_healthPercentToDrainPerSecond`.
    pub health_percent_to_drain_per_second: f32,
    /// `m_notAllowedWhenHealthBelowPercent`.
    pub not_allowed_when_health_below_percent: f32,
}

/// `OverchargeBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<OverchargeBehaviorModuleData>; 3] = [
    FieldParse::new("HealthPercentToDrainPerSecond", |ini, d, _| { d.health_percent_to_drain_per_second = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("NotAllowedWhenHealthBelowPercent", |ini, d, _| { d.not_allowed_when_health_below_percent = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl OverchargeBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, OverchargeBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults (both 0), filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<OverchargeBehaviorModuleData> {
        let mut data = OverchargeBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, OverchargeBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class OverchargeBehavior : public UpdateModule, public DamageModuleInterface, public OverchargeBehaviorInterface`.
#[derive(Clone, Debug)]
pub struct OverchargeBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getOverchargeBehaviorModuleData()`.
    data: Arc<OverchargeBehaviorModuleData>,
    /// `m_overchargeActive`.
    overcharge_active: bool,
}

impl PartialEq for OverchargeBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.overcharge_active == other.overcharge_active
    }
}

impl OverchargeBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and
    /// `m_overchargeActive` false.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<OverchargeBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(OverchargeBehavior { update, data, overcharge_active: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getOverchargeBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<OverchargeBehaviorModuleData> {
        &self.data
    }

    /// The state the test compares: `m_overchargeActive`.
    pub fn state_words(&self) -> [u32; 1] {
        [u32::from(self.overcharge_active)]
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
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, the field.
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

    /// `enable(enable)`: the rods, the player's power bonus, the flag and the wake frame, on a change only.
    fn enable(&mut self, world: &mut World, me: ModuleHandle, enable: bool) {
        if enable == self.overcharge_active && !crate::logic::modules::plant(1941) {
            return;
        }
        // for each behaviour module with a PowerPlantUpdateInterface: ppui->extendRods(enable)
        for idx in world.all_modules(me.object) {
            if idx != me.index {
                world.module_call(me.object, idx, ModuleCall::ExtendRods { extend: enable });
            }
        }
        if let Some(player) = world.get_controlling_player(me.object) {
            world.cut(if enable { CutCall::PlayerAddPowerBonus { player, object: me.object } } else { CutCall::PlayerRemovePowerBonus { player, object: me.object } });
        }
        self.overcharge_active = enable;
        update_behavior::set_wake_frame(world, if enable { UPDATE_SLEEP_NONE } else { UPDATE_SLEEP_FOREVER });
    }

    /// `onDelete()`: the bonus off the player, if on.
    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.overcharge_active {
            return;
        }
        if let Some(player) = world.get_controlling_player(me.object) {
            world.cut(CutCall::PlayerRemovePowerBonus { player, object: me.object });
        }
        self.overcharge_active = false;
    }

    /// `onCapture(oldOwner, newOwner)`: the bonus moved, if on and not disabled (`-1`: `NULL`).
    fn on_capture(&mut self, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
        if !self.overcharge_active {
            return;
        }
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) && !crate::logic::modules::plant(1942) {
            return;
        }
        if old_owner >= 0 {
            world.cut(CutCall::PlayerRemovePowerBonus { player: old_owner, object: me.object });
        }
        if new_owner >= 0 {
            world.cut(CutCall::PlayerAddPowerBonus { player: new_owner, object: me.object });
        }
    }
}

impl UpdateBehavior for OverchargeBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if !self.overcharge_active {
            return UPDATE_SLEEP_NONE;
        }
        let d = self.data.clone();
        let Some((_, max_health)) = world.body_health(me.object) else {
            world.cuts.module_errors.push((me, IniError::undefined(Undefined::NullBodyModule)));
            return UPDATE_SLEEP_NONE;
        };
        let mut info = DamageInfo::default();
        info.input.amount = (max_health * d.health_percent_to_drain_per_second) / LOGICFRAMES_PER_SECOND as f32;
        info.input.source_id = me.object;
        info.input.damage_type = DAMAGE_PENALTY;
        info.input.death_type = DEATH_NORMAL;
        if !crate::logic::modules::plant(1943) {
            world.attempt_damage(me.object, &mut info);
        }
        let (health, max_health) = world.body_health(me.object).unwrap_or((0.0, 0.0));
        if health < max_health * d.not_allowed_when_health_below_percent {
            self.enable(world, me, crate::logic::modules::plant(1944));
            // (pointers compared: no local player and no controlling player are equal too)
            if world.players.local_index() == world.get_controlling_player(me.object) && !crate::logic::modules::plant(1945) {
                world.emit(ObjectClientEvent::UiMessage { label: b"GUI:OverchargeExhausted".to_vec() });
                let position = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
                world.emit(ObjectClientEvent::RadarCreateEvent { position, event_type: RADAR_EVENT_INFORMATION });
            }
            return UPDATE_SLEEP_NONE;
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The field after the base: `m_overchargeActive`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_bool(&mut self.overcharge_active)
    }
    /// `toggle`, `enable`, `isOverchargeActive`, `onDamage` (nothing), `onDelete`, `onCapture` and `loadPostProcess`'s world part.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OverchargeToggle => {
                let on = !self.overcharge_active;
                self.enable(world, me, on);
                ModuleReply::Done
            }
            ModuleCall::OverchargeEnable(on) => {
                self.enable(world, me, on);
                ModuleReply::Done
            }
            ModuleCall::IsOverchargeActive => ModuleReply::Bool(self.overcharge_active),
            ModuleCall::OnDamage(_) => ModuleReply::Done,
            ModuleCall::OnDelete => {
                self.on_delete(world, me);
                ModuleReply::Done
            }
            ModuleCall::OnCapture { old_owner, new_owner } => {
                self.on_capture(world, me, old_owner, new_owner);
                ModuleReply::Done
            }
            ModuleCall::LoadPostProcess => {
                if self.overcharge_active && !crate::logic::modules::plant(1946) {
                    if let Some(player) = world.get_controlling_player(me.object) {
                        world.cut(CutCall::PlayerAddPowerBonus { player, object: me.object });
                    }
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::OverchargeBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::OverchargeBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for OverchargeBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
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

/// The factory's `NewModuleDataProc` for `OverchargeBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::OverchargeBehavior(Arc::new(OverchargeBehaviorModuleData::friend_new_module_data(ini)?)))
}
