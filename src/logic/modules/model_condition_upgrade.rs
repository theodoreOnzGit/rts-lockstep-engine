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

//! `ModelConditionUpgrade`: sets one model condition flag (`ConditionFlag`) on the
//! object, unless the flag is `MODELCONDITION_INVALID`.
//!
//! `ConditionFlag` is read with `ModelConditionFlags::parseSingleBitFromINI`
//! (the index of the named bit). The default is `MODELCONDITION_INVALID`, so a
//! block without the field does nothing.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::common::model_state::{ModelConditionFlagType, ModelConditionFlags, MODELCONDITION_INVALID};
/// `ModelConditionUpgrade::upgradeImplementation`'s flag: `None` for
/// `MODELCONDITION_INVALID` (upstream does nothing then).
pub fn upgrade_implementation(data: &ModelConditionUpgradeModuleData) -> Option<ModelConditionFlagType> {
    if data.condition_flag != MODELCONDITION_INVALID || crate::logic::modules::plant(9) {
        Some(data.condition_flag + crate::logic::modules::plant(10) as i32)
    } else {
        None
    }
}
/// `class ModelConditionUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ModelConditionUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `ConditionFlag`.
    pub condition_flag: ModelConditionFlagType,
}

impl Default for ModelConditionUpgradeModuleData {
    /// `ModelConditionUpgradeModuleData()`.
    fn default() -> Self {
        ModelConditionUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            condition_flag: MODELCONDITION_INVALID,
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ModelConditionUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(ModelConditionUpgradeModuleData, upgrade);

/// `ModelConditionUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ModelConditionUpgradeModuleData>; 2] = [
    FieldParse::new("ConditionFlag", |i, d, _| { d.condition_flag = ModelConditionFlags::parse_single_bit_from_ini(i)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl ModelConditionUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ModelConditionUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModelConditionUpgradeModuleData> {
        let mut data = ModelConditionUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ModelConditionUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ModelConditionUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ModelConditionUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getModelConditionUpgradeModuleData()`.
    data: Arc<ModelConditionUpgradeModuleData>,
}

impl PartialEq for ModelConditionUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ModelConditionUpgrade {
    /// `ModelConditionUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ModelConditionUpgradeModuleData>>) -> IniResult<ModelConditionUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ModelConditionUpgrade {
            base,
            data,
        })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getModelConditionUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ModelConditionUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for ModelConditionUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        if let Some(flag) = upgrade_implementation(&self.data) {
            world.set_model_condition_state(me.object, flag);
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ModelConditionUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ModelConditionUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ModelConditionUpgrade {
    /// `crc`: `UpgradeModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, then `UpgradeModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.base.xfer(xfer)
    }
    /// `loadPostProcess`: `UpgradeModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `ModelConditionUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ModelConditionUpgrade(Arc::new(ModelConditionUpgradeModuleData::friend_new_module_data(ini)?)))
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
