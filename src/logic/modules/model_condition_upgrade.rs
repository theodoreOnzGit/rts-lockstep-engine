// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ModelConditionUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ModelConditionUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ModelConditionUpgrade`: sets one model condition flag (`ConditionFlag`)
//! on the object, unless the flag is `MODELCONDITION_INVALID`.
//!
//! `ConditionFlag` is read with `ModelConditionFlags::parseSingleBitFromINI`:
//! the index of the named bit (see `common::model_state`). The default is
//! `MODELCONDITION_INVALID`, so a block without the field does nothing.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::common::model_state::{ModelConditionFlagType, ModelConditionFlags, MODELCONDITION_INVALID};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// `struct ModelConditionUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ModelConditionUpgradeModuleData {
    /// The `UpgradeModuleData` base.
    pub base: Arc<UpgradeModuleData>,
    /// `m_conditionFlag`; `MODELCONDITION_INVALID` until read.
    pub condition_flag: ModelConditionFlagType,
}

impl Default for ModelConditionUpgradeModuleData {
    /// `ModelConditionUpgradeModuleData()`: `MODELCONDITION_INVALID`.
    fn default() -> Self {
        ModelConditionUpgradeModuleData { base: Arc::new(UpgradeModuleData::new()), condition_flag: MODELCONDITION_INVALID }
    }
}

impl ModelConditionUpgradeModuleData {
    /// `buildFieldParse(p)`: the base's, then `ConditionFlag`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ModelConditionUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&CONDITION_FLAG_FIELD_PARSE)?;
        p.add(&MUX_FIELD_PARSE)
    }

    /// The data read from `ini`, or the defaults with `None`.
    pub fn new_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModelConditionUpgradeModuleData> {
        let mut data = ModelConditionUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ModelConditionUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `{ "ConditionFlag", ModelConditionFlags::parseSingleBitFromINI, ... }`.
static CONDITION_FLAG_FIELD_PARSE: [FieldParse<ModelConditionUpgradeModuleData>; 2] = [
    FieldParse::new("ConditionFlag", |i, d, _| {
        d.condition_flag = ModelConditionFlags::parse_single_bit_from_ini(i)?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The five mux fields over this type.
static MUX_FIELD_PARSE: [FieldParse<ModelConditionUpgradeModuleData>; 6] = crate::upgrade_mux_field_table!(ModelConditionUpgradeModuleData, base);

/// `ModelConditionUpgrade::upgradeImplementation`: the flag the object takes
/// (`setModelConditionState`), or `None` when the flag is `MODELCONDITION_INVALID`.
pub fn upgrade_implementation(data: &ModelConditionUpgradeModuleData) -> Option<ModelConditionFlagType> {
    if data.condition_flag != MODELCONDITION_INVALID {
        Some(data.condition_flag)
    } else {
        None
    }
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_flag_does_nothing_and_a_valid_one_is_set() {
        let mut d = ModelConditionUpgradeModuleData::default();
        assert_eq!(upgrade_implementation(&d), None);
        d.condition_flag = 17;
        assert_eq!(upgrade_implementation(&d), Some(17));
    }
}
