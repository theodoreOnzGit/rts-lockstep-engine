// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DamageModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/DamageModule.cpp
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

//! `DamageModule` and `DamageModuleData`: the base of the modules called when
//! an object is damaged or healed or its body damage state changes
//! (`onDamage`, `onHealing`, `onBodyDamageStateChange`: pure virtual, the
//! concrete modules' own, taking a `DamageInfo` and a `BodyDamageType`).
//!
//! The data class has a field table with **no entries** (its one
//! `DamageTypes` line is commented out upstream), which is still added to the
//! table list: [`DamageModuleData::build_field_parse`] adds an empty table.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `class DamageModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DamageModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
}

/// `DamageModuleData`'s one table: the terminator only.
pub static DAMAGE_MODULE_DATA_FIELD_PARSE: [FieldParse<DamageModuleData>; 1] = [FieldParse::end()];

impl DamageModuleData {
    /// `DamageModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `DamageModuleData::buildFieldParse(p)`: the base's, then an empty table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DamageModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DAMAGE_MODULE_DATA_FIELD_PARSE)
    }

    /// `DamageModule::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DamageModuleData> {
        let mut data = DamageModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DamageModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class DamageModule : public BehaviorModule, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct DamageModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getDamageModuleData()`).
    data: Arc<DamageModuleData>,
}

impl DamageModule {
    /// `DamageModule(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<DamageModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Damage(data.clone())))?;
        Ok(DamageModule { behavior, data })
    }

    /// `DamageModule::getInterfaceMask()`: `MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_DAMAGE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `getDamageModuleData()`.
    pub fn damage_module_data(&self) -> &Arc<DamageModuleData> {
        &self.data
    }
}

impl Snapshot for DamageModule {
    /// `DamageModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `DamageModule::xfer` (version 1): the version, `BehaviorModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)
    }
    /// `DamageModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    #[test]
    fn the_data_has_an_empty_table_and_reads_only_end() {
        let mut p = MultiIniFieldParse::new();
        DamageModuleData::build_field_parse(&mut p).unwrap();
        assert_eq!(p.get_count(), 1, "the one table, with only its terminator");
        fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<DamageModuleData>>) -> IniResult<()> {
            out.push(DamageModuleData::friend_new_module_data(Some(ini)));
            Ok(())
        }
        static BLOCKS: [BlockParse<Vec<IniResult<DamageModuleData>>>; 1] = [BlockParse {
            token: "D",
            parse: block,
        }];
        let mut out = Vec::new();
        let _ = Ini::load(
            b"d.ini",
            b"D\nEnd\nD\n DamageTypes = ALL\nEnd\n",
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        assert!(out[0].is_ok());
        assert_eq!(
            out[1].clone().unwrap_err().kind,
            IniErrorKind::UnknownToken,
            "the DamageTypes line is commented out upstream"
        );
    }

    #[test]
    fn the_module_holds_its_data_and_xfers_versions() {
        let mut m = DamageModule::new(2, Some(Arc::new(DamageModuleData::new()))).unwrap();
        assert_eq!(DamageModule::get_interface_mask(), 4);
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1]);
        load_bytes(&mut m, vec![1, 1, 1, 1]).unwrap();
        assert!(
            load_bytes(&mut m, vec![1, 1, 2, 1]).is_err(),
            "BehaviorModule's version 2 is above its current 1"
        );
        assert!(DamageModule::new(2, None).is_err());
        assert!(m
            .damage_module_data()
            .behavior
            .module_data
            .module_tag_name_key()
            .is_err());
    }
}
