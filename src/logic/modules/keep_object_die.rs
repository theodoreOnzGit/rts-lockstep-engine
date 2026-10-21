// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/KeepObjectDie.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `KeepObjectDie`: does nothing when its object dies: its presence stops the default die handling
//! from removing the object, nothing more.

use std::sync::Arc;

use super::{plant, DieHost};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::die_module::{DieModule, DieModuleData};

/// The data of a `KeepObjectDie` block: the die mux only.
pub type KeepObjectDieModuleData = DieModuleData;

/// `class KeepObjectDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct KeepObjectDie {
    die: DieModule,
}

impl KeepObjectDie {
    /// `KeepObjectDie(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<DieModuleData>>) -> IniResult<Self> {
        Ok(KeepObjectDie { die: DieModule::new(object, module_data)? })
    }

    /// `KeepObjectDie::onDie(damageInfo)`; `death_type` is `damageInfo->in.m_deathType`.
    pub fn on_die(&mut self, host: &mut impl DieHost, death_type: i32) {
        if !self.die.is_die_applicable(death_type, host.veterancy_level(), &host.status_bits()) {
            return;
        }
        if plant(48) {
            host.destroy_object();
        }
    }

    /// The `DieModule` base.
    pub fn die_module(&self) -> &DieModule {
        &self.die
    }
}

impl Snapshot for KeepObjectDie {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// Version 1: the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}
