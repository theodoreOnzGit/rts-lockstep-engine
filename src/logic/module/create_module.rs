// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CreateModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/CreateModule.cpp
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

//! `CreateModule` and `CreateModuleData`: the base of the modules called when
//! an object is created (`onCreate`, "when you become a code Object", pure
//! virtual) and when it is finished building (`onBuildComplete`).
//!
//! `m_needToRunOnBuildComplete` ("prevent the multiple calling of
//! `onBuildComplete`") starts true, is cleared by the base `onBuildComplete`
//! (the concrete modules call it after their own work) and is saved. Unlike
//! its siblings, `CreateModule` does not use the standard data macro (the line
//! is commented out upstream), so `CreateModule` has no
//! `friend_newModuleData` of its own: [`CreateModuleData`] only has its
//! `buildFieldParse`.

use std::sync::Arc;

use crate::common::ini::{IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CREATE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `class CreateModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreateModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
}

impl CreateModuleData {
    /// `CreateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `CreateModuleData::buildFieldParse(p)`: the base's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CreateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)
    }

    // --- p1w10-sonnet ---
    /// `CreateModule::friend_newModuleData(ini)` (the data proc of the create modules that have no data class of
    /// their own): new data, read with `initFromINIMultiProc` if there is an `ini`.
    pub fn friend_new_module_data(
        ini: Option<&mut crate::common::ini::Ini<'_>>,
    ) -> IniResult<CreateModuleData> {
        let mut data = CreateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CreateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
    // --- end p1w10-sonnet ---
}

/// `class CreateModule : public BehaviorModule, public CreateModuleInterface`.
#[derive(Clone, Debug)]
pub struct CreateModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// `m_needToRunOnBuildComplete`.
    need_to_run_on_build_complete: bool,
}

impl CreateModule {
    /// `CreateModule(thing, moduleData)`: still to run `onBuildComplete`.
    /// `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<CreateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Create(data)))?;
        Ok(CreateModule {
            behavior,
            need_to_run_on_build_complete: true,
        })
    }

    /// `CreateModule::getInterfaceMask()`: `MODULEINTERFACE_CREATE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_CREATE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `onBuildComplete()` (the base's): no longer need to run it.
    pub fn on_build_complete(&mut self) {
        self.need_to_run_on_build_complete = false;
    }

    /// `shouldDoOnBuildComplete()`.
    pub fn should_do_on_build_complete(&self) -> bool {
        self.need_to_run_on_build_complete
    }
}

impl Snapshot for CreateModule {
    /// `CreateModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `CreateModule::xfer` (version 1): the version, `BehaviorModule::xfer`,
    /// `m_needToRunOnBuildComplete`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        xfer.xfer_bool(&mut self.need_to_run_on_build_complete)
    }
    /// `CreateModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    fn module() -> CreateModule {
        CreateModule::new(2, Some(Arc::new(CreateModuleData::new()))).unwrap()
    }

    #[test]
    fn on_build_complete_runs_once() {
        let mut m = module();
        assert!(m.should_do_on_build_complete());
        m.on_build_complete();
        assert!(!m.should_do_on_build_complete());
        m.on_build_complete();
        assert!(!m.should_do_on_build_complete());
        assert_eq!(CreateModule::get_interface_mask(), 8);
        assert!(CreateModule::new(2, None).is_err());
    }

    #[test]
    fn xfer_saves_the_flag() {
        let mut m = module();
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1, 1]);
        m.on_build_complete();
        let bytes = save_bytes(&mut m).unwrap();
        assert_eq!(bytes, [1, 1, 1, 1, 0]);
        let mut other = module();
        load_bytes(&mut other, bytes).unwrap();
        assert!(!other.should_do_on_build_complete());
        load_bytes(&mut other, vec![1, 1, 1, 1, 1]).unwrap();
        assert!(other.should_do_on_build_complete());
    }
}
