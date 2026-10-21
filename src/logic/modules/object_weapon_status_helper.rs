// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectWeaponStatusHelper.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectWeaponStatusHelper`: keeps the object's model condition in step with its weapon status; it starts active and runs in the final update phase, after every normal update module.

use std::sync::Arc;

use super::plant;
use super::object_helper::{HelperHost, ObjectHelper};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};

/// `class ObjectWeaponStatusHelper : public ObjectHelper`; it has no state of its own.
#[derive(Clone, Debug)]
pub struct ObjectWeaponStatusHelper {
    helper: ObjectHelper,
}

impl ObjectWeaponStatusHelper {
    /// `ObjectWeaponStatusHelper(thing, modData)`: the `ObjectHelper` (asleep), then `setWakeFrame(UPDATE_SLEEP_NONE)` ("unlike other helpers, this one needs to start active").
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, host: &mut impl HelperHost) -> IniResult<Self> {
        let helper = ObjectHelper::new(object, module_data, if plant(35) { SleepyUpdatePhase::Normal } else { SleepyUpdatePhase::Final }, host)?;
        if !plant(36) {
            host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), UPDATE_SLEEP_NONE));
        }
        Ok(ObjectWeaponStatusHelper { helper })
    }

    /// `ObjectWeaponStatusHelper::update()`.
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        host.adjust_model_condition_for_weapon_status();
        if plant(37) { crate::logic::module::update_module::UPDATE_SLEEP_FOREVER } else { UPDATE_SLEEP_NONE }
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

impl Snapshot for ObjectWeaponStatusHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the version, `ObjectHelper::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}
