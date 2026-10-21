// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/TempWeaponBonusHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TempWeaponBonusHelper`: holds one weapon bonus condition on an object for
//! a number of frames (`doTempWeaponBonus`), tinting its drawable, and clears
//! it when its sleep ends. Asleep until used. Runs while the object is
//! disabled (`getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`).

use std::sync::Arc;

use super::plant;
use super::object_helper::{HelperHost, ObjectHelper, TINT_STATUS_FRENZY};
use crate::common::disabled_types::{disabledmask_all, DisabledMaskType};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
// --- p1w10-sonnet ---
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
// --- end p1w10-sonnet ---
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
};
use crate::logic::weapon_bonus::condition::INVALID as WEAPONBONUSCONDITION_INVALID;

/// `class TempWeaponBonusHelper : public ObjectHelper`.
#[derive(Clone, Debug)]
pub struct TempWeaponBonusHelper {
    helper: ObjectHelper,
    /// `m_currentBonus` (`WEAPONBONUSCONDITION_INVALID` when idle).
    pub current_bonus: i32,
    /// `m_frameToRemove`.
    pub frame_to_remove: u32,
}

impl TempWeaponBonusHelper {
    /// `TempWeaponBonusHelper(thing, modData)`: the `ObjectHelper`, no bonus,
    /// frame 0, then a second `setWakeFrame(UPDATE_SLEEP_FOREVER)`.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, host: &mut impl HelperHost) -> IniResult<Self> {
        let mut me = TempWeaponBonusHelper::unconstructed(object, module_data)?;
        me.construct(host);
        Ok(me)
    }

    /// The first half of the constructor, which touches no world: no bonus, frame 0.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>) -> IniResult<Self> {
        let helper = ObjectHelper::unconstructed(object, module_data, SleepyUpdatePhase::Normal)?;
        Ok(TempWeaponBonusHelper { helper, current_bonus: WEAPONBONUSCONDITION_INVALID, frame_to_remove: 0 })
    }

    /// The second half: the base's wake, then a second `setWakeFrame(UPDATE_SLEEP_FOREVER)`. The world runs it once the module
    /// is part of its object.
    pub fn construct(&mut self, host: &mut impl HelperHost) {
        self.helper.construct(host);
        if !plant(55) {
            host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), UPDATE_SLEEP_FOREVER));
        }
    }

    /// `getDisabledTypesToProcess()`: `DISABLEDMASK_ALL`.
    pub fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        disabledmask_all()
    }

    /// `update()`: clear the bonus (and the tint, if there is a drawable),
    /// then sleep forever. (The "woke up too soon" assert is compiled out.)
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        self.clear_temp_weapon_bonus(host);
        UPDATE_SLEEP_FOREVER
    }

    /// `clearTempWeaponBonus()`.
    fn clear_temp_weapon_bonus(&mut self, host: &mut impl HelperHost) {
        if self.current_bonus != WEAPONBONUSCONDITION_INVALID {
            host.clear_weapon_bonus_condition(self.current_bonus);
            self.current_bonus = WEAPONBONUSCONDITION_INVALID;
            self.frame_to_remove = 0;
            if host.has_drawable() || plant(44) {
                host.clear_tint_status(TINT_STATUS_FRENZY);
            }
        }
    }

    /// `doTempWeaponBonus(status, duration)`: a different bonus clears the old
    /// one first; the same one resets the timer.
    pub fn do_temp_weapon_bonus(&mut self, host: &mut impl HelperHost, status: i32, duration: u32) {
        if self.current_bonus != status {
            self.clear_temp_weapon_bonus(host);
        }
        host.set_weapon_bonus_condition(status);
        self.current_bonus = status;
        self.frame_to_remove = if plant(45) { duration } else { host.frame().wrapping_add(duration) };
        if host.has_drawable() && !plant(43) {
            host.set_tint_status(TINT_STATUS_FRENZY);
        }
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), update_sleep(duration)));
    }

    /// The `ObjectHelper` base.
    pub fn helper(&self) -> &ObjectHelper {
        &self.helper
    }

    /// The `ObjectHelper` base, mutably.
    pub fn helper_mut(&mut self) -> &mut ObjectHelper {
        &mut self.helper
    }
}

impl Snapshot for TempWeaponBonusHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the base, `m_currentBonus` (`xferUser` of the enum, 4 bytes), `m_frameToRemove`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)?;
        let mut bytes = self.current_bonus.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.current_bonus = i32::from_le_bytes(bytes);
        if plant(46) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.frame_to_remove)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for TempWeaponBonusHelper {
    /// The same state: the same object and the same fields. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.helper.update_module().behavior().object() == other.helper.update_module().behavior().object() && self.current_bonus == other.current_bonus && self.frame_to_remove == other.frame_to_remove
    }
}

impl TempWeaponBonusHelper {
    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.update(world);
        };
        lend_clone(world, h, self, UpdateBody::TempWeaponBonusHelper, un, |w, m| w.in_update_module(me, |w| m.update(w)))
    }

    /// The world half of the constructor, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.construct(world);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::DoTempWeaponBonus { status, duration } => {
                lend_clone(world, me, self, UpdateBody::TempWeaponBonusHelper, un, |w, m| m.do_temp_weapon_bonus(w, status, duration));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// `xfer` with the scheduling half the slot holds (the live `UpdateModule`): the version, `ObjectHelper::xfer`, the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        ObjectHelper::xfer_with(update, xfer)?;
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

    /// The fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut bytes = self.current_bonus.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.current_bonus = i32::from_le_bytes(bytes);
        if plant(46) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.frame_to_remove)
    }
}

/// The variant's module, if `b` is one.
fn un(b: UpdateBody) -> Option<TempWeaponBonusHelper> {
    match b {
        UpdateBody::TempWeaponBonusHelper(m) => Some(m),
        _ => None,
    }
}
// --- end p1w10-sonnet ---
