// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SubdualDamageHelper.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/SubdualDamageHelper.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SubdualDamageHelper`: heals an object's subdual damage a step at a time. Asleep until `notifySubdualDamage(amount)` with a positive amount
//! wakes it (`ModuleCall::NotifySubdualDamage`, from `Object::attemptDamage`'s subdual path); then every frame it counts down
//! `getSubdualDamageHealRate()` and, at zero, applies `-getSubdualDamageHealAmount()` of `DAMAGE_SUBDUAL_UNRESISTABLE` to the body, going back to
//! sleep once the body has no subdual damage left. Like the other helpers it runs while the object is disabled (`DISABLEDMASK_ALL`).
//!
//! `m_healingStepCountdown` is an `UnsignedInt`: the `--` of a zero wraps to `0xffffffff` (the module then waits 4 billion frames, which is what
//! upstream does after an `update()` that finds the countdown already at zero).
//!
//! The body half of the object is `ModuleCall`s (`GetSubdualDamageHealRate`, `GetSubdualDamageHealAmount`, `HasAnySubdualDamage`,
//! `AttemptDamage`); a missing body is `getObject()->getBodyModule()` returning `NULL` and the call through it a `NULL` dereference
//! ([`Undefined::NullBodyModule`]).
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::disabled_types::disabledmask_all;
use crate::common::ini::{IniError, IniResult, Undefined};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_SUBDUAL_UNRESISTABLE};
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::object_helper::ObjectHelper;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use std::sync::Arc;

/// `class SubdualDamageHelper : public ObjectHelper`.
#[derive(Clone, Debug)]
pub struct SubdualDamageHelper {
    helper: ObjectHelper,
    /// `m_healingStepCountdown`.
    healing_step_countdown: u32,
}

impl PartialEq for SubdualDamageHelper {
    /// The same object and the same countdown.
    fn eq(&self, other: &Self) -> bool {
        self.helper.update_module().behavior().object()
            == other.helper.update_module().behavior().object()
            && self.healing_step_countdown == other.healing_step_countdown
    }
}

impl SubdualDamageHelper {
    /// The first half of the constructor, which touches no world: the `ObjectHelper`, a countdown of zero.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
    ) -> IniResult<Self> {
        let mut helper =
            ObjectHelper::unconstructed(object, module_data, SleepyUpdatePhase::Normal)?;
        // `getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`; the scheduler reads the arena's base
        helper
            .update_module_mut()
            .set_disabled_types_to_process(disabledmask_all());
        Ok(SubdualDamageHelper {
            helper,
            healing_step_countdown: 0,
        })
    }

    /// The `ObjectHelper` base.
    pub fn helper(&self) -> &ObjectHelper {
        &self.helper
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.healing_step_countdown]
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
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, `ObjectHelper::xfer`, the countdown.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        ObjectHelper::xfer_with(update, xfer)?;
        self.xfer_fields(xfer, version)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `getObject()->getBodyModule()`, or the error of dereferencing `NULL`.
    fn body(world: &World, me: ModuleHandle) -> Result<u16, IniError> {
        world
            .get_body_module(me.object)
            .ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))
    }

    /// `body->getSubdualDamageHealRate()`.
    fn heal_rate(world: &mut World, me: ModuleHandle, body: u16) -> u32 {
        match world.module_call(me.object, body, ModuleCall::GetSubdualDamageHealRate) {
            ModuleReply::Int(r) => r as u32,
            _ => 0,
        }
    }

    /// `notifySubdualDamage(amount)`: "if there is damage to heal, start counting down".
    pub fn notify_subdual_damage(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        amount: f32,
    ) -> Result<(), IniError> {
        if amount > 0.0 {
            let body = Self::body(world, me)?;
            self.healing_step_countdown = Self::heal_rate(world, me, body);
            if !crate::logic::modules::plant(783) {
                update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
            }
        }
        Ok(())
    }

    /// `update()` after the host call plumbing: an error is the `NULL` dereference of a missing body.
    fn update_checked(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) -> Result<UpdateSleepTime, IniError> {
        let body = Self::body(world, me)?;
        self.healing_step_countdown = self.healing_step_countdown.wrapping_sub(1);
        if self.healing_step_countdown > 0 && !crate::logic::modules::plant(781) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        self.healing_step_countdown = Self::heal_rate(world, me, body);
        let mut remove_subdue_damage = DamageInfo::default();
        remove_subdue_damage.input.damage_type = DAMAGE_SUBDUAL_UNRESISTABLE;
        let amount =
            match world.module_call(me.object, body, ModuleCall::GetSubdualDamageHealAmount) {
                ModuleReply::Real(a) => a,
                _ => 0.0,
            };
        remove_subdue_damage.input.amount = if crate::logic::modules::plant(782) {
            amount
        } else {
            -amount
        };
        world.module_call(
            me.object,
            body,
            ModuleCall::AttemptDamage(remove_subdue_damage),
        );
        let any = world
            .module_call(me.object, body, ModuleCall::HasAnySubdualDamage)
            .as_bool();
        Ok(if any || crate::logic::modules::plant(784) {
            UPDATE_SLEEP_NONE
        } else {
            UPDATE_SLEEP_FOREVER
        })
    }

    /// The fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.healing_step_countdown)
    }
}

impl UpdateBehavior for SubdualDamageHelper {
    fn update_base(&self) -> &UpdateModule {
        self.helper.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_checked(world, me) {
            Ok(s) => s,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_FOREVER
            }
        }
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        // the `ObjectHelper`'s wake, then the constructor's own `setWakeFrame(UPDATE_SLEEP_FOREVER)`
        self.helper.construct(world);
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SubdualDamageHelper(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SubdualDamageHelper(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::NotifySubdualDamage { amount } => {
                if let Err(e) = self.notify_subdual_damage(world, me, amount) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SubdualDamageHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the base, `m_healingStepCountdown`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)?;
        self.xfer_fields(xfer, version)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}
