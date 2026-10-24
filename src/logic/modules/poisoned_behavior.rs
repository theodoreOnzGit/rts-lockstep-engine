// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PoisonedBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PoisonedBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PoisonedBehavior`: poison damage that keeps hurting. Poison damage (`onDamage` with `DAMAGE_POISON`) poisons the object for `PoisonDuration`
//! frames from now, taking the damage just dealt again every `PoisonDamageInterval` frames (the sooner of the pending damage frame and the next
//! interval), with the poison's death type, and tints the drawable; any healing (`onHealing`) cures it. The repeated damage is unresistable (so
//! that it does not poison again) with the poison's damage FX. The module sleeps between the frames and processes every disabled type.
//!
//! The damage module calls are [`ModuleCall::OnDamage`] and [`ModuleCall::OnHealing`].
//!
//! # Upstream facts kept
//!
//! - Healing cures even an object that is not poisoned (a tint clear, and the module asleep for ever).
//! - The cure at the end of the poison is not given to an object that is effectively dead.
//! - The sleep is to the sooner of the two frames, even a damage frame of 0 (`frameToSleepTime(0, ..)` is "now": no sleep).
//! - The death type is saved from version 2 as the raw bytes of the enum.

use std::sync::Arc;

use crate::common::disabled_types::disabledmask_all;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DeathType, DAMAGE_POISON, DAMAGE_UNRESISTABLE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, FOREVER,
    UPDATE_SLEEP_FOREVER,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `TINT_STATUS_POISONED` (`GameClient/Drawable.h` line 245, the enum `TintStatus`).
pub const TINT_STATUS_POISONED: i32 = 0x0000_0004;
/// `DEATH_POISONED`.
const DEATH_POISONED: DeathType = 5;

/// `class PoisonedBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoisonedBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_poisonDamageIntervalData`: "How often I retake poison damage dealt me".
    pub poison_damage_interval_data: u32,
    /// `m_poisonDurationData`: "And how long after the last poison dose I am poisoned".
    pub poison_duration_data: u32,
}

/// `PoisonedBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<PoisonedBehaviorModuleData>; 3] = [
    FieldParse::new(
        "PoisonDamageInterval",
        |ini, d, _| {
            d.poison_damage_interval_data = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "PoisonDuration",
        |ini, d, _| {
            d.poison_duration_data = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl PoisonedBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, PoisonedBehaviorModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<PoisonedBehaviorModuleData> {
        let mut data = PoisonedBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PoisonedBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class PoisonedBehavior : public UpdateModule, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct PoisonedBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getPoisonedBehaviorModuleData()`.
    data: Arc<PoisonedBehaviorModuleData>,
    /// `m_poisonDamageFrame`.
    poison_damage_frame: u32,
    /// `m_poisonOverallStopFrame`.
    poison_overall_stop_frame: u32,
    /// `m_poisonDamageAmount`.
    poison_damage_amount: f32,
    /// `m_deathType`.
    death_type: DeathType,
}

impl PartialEq for PoisonedBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state (the `Real` by its bits).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

impl PoisonedBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base (processing every disabled type), the data (`INI_INVALID_DATA` for
    /// null data), no poison, `DEATH_POISONED`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<PoisonedBehaviorModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let mut update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): DISABLEDMASK_ALL
        update.set_disabled_types_to_process(disabledmask_all());
        Ok(PoisonedBehavior {
            update,
            data,
            poison_damage_frame: 0,
            poison_overall_stop_frame: 0,
            poison_damage_amount: 0.0,
            death_type: DEATH_POISONED,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getPoisonedBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<PoisonedBehaviorModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the two frames, the amount, the death type.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.poison_damage_frame,
            self.poison_overall_stop_frame,
            self.poison_damage_amount.to_bits(),
            self.death_type as u32,
        ]
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

    /// `calcSleepTime()`.
    fn calc_sleep_time(&self, world: &World) -> UpdateSleepTime {
        let now = world.frame();
        if self.poison_overall_stop_frame == 0 || self.poison_overall_stop_frame == now {
            return UPDATE_SLEEP_FOREVER;
        }
        UpdateModule::frame_to_sleep_time(
            now,
            self.poison_damage_frame,
            self.poison_overall_stop_frame,
            FOREVER,
            FOREVER,
        )
    }

    /// `startPoisonedEffects(damageInfo)`.
    fn start_poisoned_effects(&mut self, world: &mut World, me: ObjectId, info: &DamageInfo) {
        let d = self.data.clone();
        let now = world.frame();
        // We are going to take the damage dealt by the original poisoner every so often for a while.
        self.poison_damage_amount = info.out.actual_damage_dealt;
        self.poison_overall_stop_frame = now.wrapping_add(d.poison_duration_data);
        // If we are getting re-poisoned, don't reset the damage counter if running, but do set it if unset
        self.poison_damage_frame =
            if self.poison_damage_frame != 0 && !crate::logic::modules::plant(1811) {
                self.poison_damage_frame
                    .min(now.wrapping_add(d.poison_damage_interval_data))
            } else {
                now.wrapping_add(d.poison_damage_interval_data)
            };
        self.death_type = info.input.death_type;
        if world.has_drawable(me) {
            world.emit(ObjectClientEvent::DrawableTint {
                object_id: me,
                status: TINT_STATUS_POISONED,
                on: true,
            });
        }
        let t = self.calc_sleep_time(world);
        update_behavior::set_wake_frame(world, t);
    }

    /// `stopPoisonedEffects()`.
    fn stop_poisoned_effects(&mut self, world: &mut World, me: ObjectId) {
        self.poison_damage_frame = 0;
        self.poison_overall_stop_frame = 0;
        self.poison_damage_amount = 0.0;
        if world.has_drawable(me) && !crate::logic::modules::plant(1812) {
            world.emit(ObjectClientEvent::DrawableTint {
                object_id: me,
                status: TINT_STATUS_POISONED,
                on: false,
            });
        }
    }
}

impl UpdateBehavior for PoisonedBehavior {
    /// "1: Initial version" (and 2, which added the death type).
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let now = world.frame();
        if self.poison_overall_stop_frame == 0 {
            // DEBUG_CRASH(("hmm, this should not happen"));
            return UPDATE_SLEEP_FOREVER;
        }
        if self.poison_damage_frame != 0 && now >= self.poison_damage_frame {
            let mut damage = DamageInfo::default();
            damage.input.amount = self.poison_damage_amount;
            damage.input.source_id = INVALID_ID;
            damage.input.damage_type = DAMAGE_UNRESISTABLE; // Not poison, as that will infect us again
            damage.input.damage_fx_override = if crate::logic::modules::plant(1813) {
                DAMAGE_UNRESISTABLE
            } else {
                DAMAGE_POISON
            }; // but this will ensure that the right effect is played
            damage.input.death_type = self.death_type;
            world.attempt_damage(me.object, &mut damage);
            self.poison_damage_frame = now.wrapping_add(self.data.poison_damage_interval_data);
        }
        if self.poison_overall_stop_frame != 0
            && now >= self.poison_overall_stop_frame
            && !world
                .objects
                .find(me.object)
                .is_some_and(|o| o.is_effectively_dead())
        {
            self.stop_poisoned_effects(world, me.object);
        }
        self.calc_sleep_time(world)
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The fields after the base: the two frames, the amount, the death type (version 2, its raw bytes).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.poison_damage_frame)?;
        xfer.xfer_unsigned_int(&mut self.poison_overall_stop_frame)?;
        xfer.xfer_real(&mut self.poison_damage_amount)?;
        if version >= 2 && !crate::logic::modules::plant(1814) {
            let mut bytes = self.death_type.to_le_bytes();
            xfer.xfer_user(&mut bytes)?;
            self.death_type = DeathType::from_le_bytes(bytes);
        }
        Ok(())
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            // "Damage has been dealt, this is an opportunity to react to that damage"
            ModuleCall::OnDamage(info) => {
                if info.input.damage_type == DAMAGE_POISON {
                    self.start_poisoned_effects(world, me.object, &info);
                }
                ModuleReply::Done
            }
            ModuleCall::OnHealing(_) => {
                if !crate::logic::modules::plant(1815) {
                    self.stop_poisoned_effects(world, me.object);
                    update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PoisonedBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PoisonedBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PoisonedBehavior {
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

/// The factory's `NewModuleDataProc` for `PoisonedBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PoisonedBehavior(Arc::new(
        PoisonedBehaviorModuleData::friend_new_module_data(ini)?,
    )))
}
