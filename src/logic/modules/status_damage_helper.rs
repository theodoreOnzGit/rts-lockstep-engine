// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/StatusDamageHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StatusDamageHelper`: sets an object status for a number of frames
//! (`doStatusDamage`) and clears it when its sleep ends. Asleep until used.
//! Unlike most update modules it runs while the object is disabled
//! (`getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`).

use std::sync::Arc;

use super::plant;
use super::object_helper::{HelperHost, ObjectHelper};
use crate::common::base_type::real_to_int_floor;
use crate::common::disabled_types::{disabledmask_all, DisabledMaskType};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_NONE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
};

/// `class StatusDamageHelper : public ObjectHelper`.
#[derive(Clone, Debug)]
pub struct StatusDamageHelper {
    helper: ObjectHelper,
    /// `m_statusToHeal` (`OBJECT_STATUS_NONE` when idle).
    pub status_to_heal: ObjectStatusTypes,
    /// `m_frameToHeal`.
    pub frame_to_heal: u32,
}

impl StatusDamageHelper {
    /// `StatusDamageHelper(thing, modData)`: the `ObjectHelper`, no status,
    /// frame 0, then `setWakeFrame(UPDATE_SLEEP_FOREVER)` (a second wake call
    /// after the base's).
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, host: &mut impl HelperHost) -> IniResult<Self> {
        let helper = ObjectHelper::new(object, module_data, SleepyUpdatePhase::Normal, host)?;
        if !plant(56) {
            host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), UPDATE_SLEEP_FOREVER));
        }
        Ok(StatusDamageHelper { helper, status_to_heal: OBJECT_STATUS_NONE, frame_to_heal: 0 })
    }

    /// `getDisabledTypesToProcess()`: `DISABLEDMASK_ALL`.
    pub fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        if plant(40) { DisabledMaskType::new() } else { disabledmask_all() }
    }

    /// `update()`: "we are sleep driven, so seeing an update means our timer
    /// is ready implicitly": clear the status, sleep forever. (Upstream's
    /// `DEBUG_ASSERTCRASH` that it did not wake too soon is compiled out.)
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        self.clear_status_condition(host);
        UPDATE_SLEEP_FOREVER
    }

    /// `clearStatusCondition()`: if a status is being timed, clear its bit
    /// and reset the timer.
    fn clear_status_condition(&mut self, host: &mut impl HelperHost) {
        if self.status_to_heal != OBJECT_STATUS_NONE {
            host.clear_status(self.status_to_heal);
            self.status_to_heal = OBJECT_STATUS_NONE;
            if !plant(42) {
                self.frame_to_heal = 0;
            }
        }
    }

    /// `doStatusDamage(status, duration)`. The duration is truncated to whole
    /// frames (`REAL_TO_INT_FLOOR`); the frame sum and the sleep wrap as the
    /// C++ unsigned arithmetic does.
    pub fn do_status_damage(&mut self, host: &mut impl HelperHost, status: ObjectStatusTypes, duration: f32) {
        let duration_as_int = if plant(41) { duration as i32 } else { real_to_int_floor(duration) };
        // Clear any different status we may have. Re-getting the same status just resets the timer.
        if self.status_to_heal != status && !plant(38) {
            self.clear_status_condition(host);
        }
        host.set_status(status);
        self.status_to_heal = status;
        self.frame_to_heal = host.frame().wrapping_add(duration_as_int as u32).wrapping_add(u32::from(plant(39)));
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), update_sleep(duration_as_int as u32)));
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

impl Snapshot for StatusDamageHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the base, `m_statusToHeal` (`xferUser` of the enum, 4 bytes), `m_frameToHeal`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)?;
        if !plant(53) {
            let mut bytes = self.status_to_heal.to_le_bytes();
            xfer.xfer_user(&mut bytes)?;
            self.status_to_heal = i32::from_le_bytes(bytes);
        }
        xfer.xfer_unsigned_int(&mut self.frame_to_heal)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}
