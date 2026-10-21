// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectRepulsorHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectRepulsorHelper`: clears `OBJECT_STATUS_REPULSOR` when woken, then sleeps until forcibly awakened.

use std::sync::Arc;

use super::plant;
use super::object_helper::{HelperHost, ObjectHelper};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::common::object_status_types::OBJECT_STATUS_REPULSOR;

/// `class ObjectRepulsorHelper : public ObjectHelper`; it has no state of its own.
#[derive(Clone, Debug)]
pub struct ObjectRepulsorHelper {
    helper: ObjectHelper,
}

impl ObjectRepulsorHelper {
    /// `ObjectRepulsorHelper(thing, modData)`: the `ObjectHelper` (asleep), then nothing.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, host: &mut impl HelperHost) -> IniResult<Self> {
        let helper = ObjectHelper::new(object, module_data, SleepyUpdatePhase::Normal, host)?;
        Ok(ObjectRepulsorHelper { helper })
    }

    /// `ObjectRepulsorHelper::update()`.
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        host.clear_status(OBJECT_STATUS_REPULSOR + i32::from(plant(34)));
        UPDATE_SLEEP_FOREVER
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

impl Snapshot for ObjectRepulsorHelper {
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
