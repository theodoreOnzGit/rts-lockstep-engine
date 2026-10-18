// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DestroyModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Destroy/DestroyModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DestroyModule`: the base of the modules called when an object is
//! destroyed (`onDestroy`, pure virtual). No concrete module derives from it
//! in this release (the factory registers none: "destroy modules: (none)"),
//! and the class has no data class of its own, so the module data is any
//! [`ModuleDataKind`].

use crate::common::ini::IniResult;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DESTROY};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModule;

/// `class DestroyModule : public BehaviorModule, public DestroyModuleInterface`.
#[derive(Clone, Debug)]
pub struct DestroyModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
}

impl DestroyModule {
    /// `DestroyModule(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<ModuleDataKind>) -> IniResult<Self> {
        Ok(DestroyModule { behavior: BehaviorModule::new(object, module_data)? })
    }

    /// `DestroyModule::getInterfaceMask()`: `MODULEINTERFACE_DESTROY`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_DESTROY
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }
}

impl Snapshot for DestroyModule {
    /// `DestroyModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `DestroyModule::xfer` (version 1): the version, `BehaviorModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)
    }
    /// `DestroyModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::module::ModuleData;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    #[test]
    fn a_destroy_module_takes_any_data() {
        let data = || Some(ModuleDataKind::Module(std::sync::Arc::new(ModuleData::new())));
        let mut m = DestroyModule::new(2, data()).unwrap();
        assert_eq!(DestroyModule::get_interface_mask(), 0x200);
        assert_eq!(m.behavior().object(), 2);
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1]);
        load_bytes(&mut m, vec![1, 1, 1, 1]).unwrap();
        assert!(DestroyModule::new(2, None).is_err());
    }
}
