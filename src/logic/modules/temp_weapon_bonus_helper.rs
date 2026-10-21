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
        let helper = ObjectHelper::new(object, module_data, SleepyUpdatePhase::Normal, host)?;
        if !plant(55) {
            host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), UPDATE_SLEEP_FOREVER));
        }
        Ok(TempWeaponBonusHelper { helper, current_bonus: WEAPONBONUSCONDITION_INVALID, frame_to_remove: 0 })
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
