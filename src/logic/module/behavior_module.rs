// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BehaviorModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BehaviorModule.cpp
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

//! `BehaviorModule` and `BehaviorModuleData`: the base of every module an
//! `Object` holds in its behavior list (`m_behaviors`). The class itself adds
//! only the `get...()` interface accessors, which return `NULL` here and are
//! overridden by the interface classes; in the enum design they are the
//! `get_...` methods of [`Module`](crate::common::module::Module), which
//! answer per variant (see `common::module`).
//!
//! # Left out
//!
//! `BehaviorModuleInterface`'s twenty interface-acquisition virtuals
//! (`getParkingPlaceBehaviorInterface` ... `getCountermeasuresBehaviorInterface`),
//! `getStealth` and `getSpyVisionUpdate`: every one returns `NULL` in
//! `BehaviorModule`; they are added to `Module` with the first concrete module
//! (layer 9) that returns something else. The four `...Interface` classes
//! declared below `BehaviorModule` (`ParkingPlaceBehaviorInterface`,
//! `TransportPassengerInterface`, `CaveInterface`, `LandMineInterface`, and the
//! `RunwayReservationType` enum) are pure-virtual interfaces of concrete
//! modules and arrive with them.

use crate::common::ini::{IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleBase, ModuleData, ModuleDataKind, ModuleType, ObjectId, ObjectModule};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};

/// `class BehaviorModuleData : public ModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BehaviorModuleData {
    /// The `ModuleData` base.
    pub module_data: ModuleData,
}

impl BehaviorModuleData {
    /// `BehaviorModuleData()`.
    pub fn new() -> Self {
        BehaviorModuleData { module_data: ModuleData::new() }
    }

    /// `BehaviorModuleData::buildFieldParse(p)`: `ModuleData::buildFieldParse`,
    /// nothing.
    pub fn build_field_parse<T>(p: &mut MultiIniFieldParse<'_, T>) -> IniResult<()> {
        ModuleData::build_field_parse(p)
    }
}

/// `class BehaviorModule : public ObjectModule, public BehaviorModuleInterface`.
#[derive(Clone, Debug)]
pub struct BehaviorModule {
    /// The `ObjectModule` base.
    object_module: ObjectModule,
}

impl BehaviorModule {
    /// `BehaviorModule(thing, moduleData)`: `ObjectModule`'s checks
    /// (`INI_INVALID_DATA` for null data).
    pub fn new(object: ObjectId, module_data: Option<ModuleDataKind>) -> IniResult<Self> {
        Ok(BehaviorModule { object_module: ObjectModule::new(object, module_data)? })
    }

    /// `BehaviorModule::getInterfaceMask()`: 0.
    pub const fn get_interface_mask() -> i32 {
        0
    }

    /// `BehaviorModule::getModuleType()`: `MODULETYPE_BEHAVIOR`.
    pub const fn get_module_type() -> ModuleType {
        ModuleType::Behavior
    }

    /// The `ObjectModule` base.
    pub fn object_module(&self) -> &ObjectModule {
        &self.object_module
    }

    /// The `Module` base.
    pub fn module_base(&self) -> &ModuleBase {
        self.object_module.module()
    }

    /// `getObject()`, as an id.
    pub fn object(&self) -> ObjectId {
        self.object_module.object()
    }
}

impl Snapshot for BehaviorModule {
    /// `BehaviorModule::crc`: `ObjectModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.object_module.crc(xfer)
    }
    /// `BehaviorModule::xfer` (version 1): the version, then
    /// `ObjectModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.object_module.xfer(xfer)
    }
    /// `BehaviorModule::loadPostProcess`: `ObjectModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.object_module.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::module::test_support::{crc_of, load_bytes, save_bytes};
    use std::sync::Arc;

    #[test]
    fn a_behavior_module_is_the_base_of_the_behavior_type() {
        let data = ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::new()));
        let mut m = BehaviorModule::new(9, Some(data)).unwrap();
        assert_eq!(BehaviorModule::get_interface_mask(), 0);
        assert_eq!(BehaviorModule::get_module_type(), ModuleType::Behavior);
        assert_eq!(m.object(), 9);
        assert!(m.module_base().module_tag_name_key().is_err());
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1]);
        load_bytes(&mut m, vec![1, 1, 1]).unwrap();
        assert_eq!(crc_of(&mut m).unwrap(), 0);
        assert!(load_bytes(&mut m, vec![1, 2, 1]).is_err(), "ObjectModule's version 2 is above its current 1");
        assert!(BehaviorModule::new(9, None).is_err());
        let mut p: MultiIniFieldParse<'_, BehaviorModuleData> = MultiIniFieldParse::new();
        BehaviorModuleData::build_field_parse(&mut p).unwrap();
        assert_eq!(p.get_count(), 0);
    }
}
