// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ExperienceScalarUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ExperienceScalarUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ExperienceScalarUpgrade`: adds `AddXPScalar` to the object's experience scalar
//! (the multiplier its tracker applies to experience gained).
//!
//! Upstream: `xpTracker->setExperienceScalar(xpTracker->getExperienceScalar() +
//! m_addXPScalar)`, on the object's tracker, which every object has (`Object`
//! makes it in its constructor).

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
/// `ExperienceScalarUpgrade::upgradeImplementation`'s scalar: `current + AddXPScalar`.
pub fn upgrade_implementation(current_scalar: f32, data: &ExperienceScalarUpgradeModuleData) -> f32 {
    if crate::logic::modules::plant(11) { current_scalar - data.add_xp_scalar } else { current_scalar + data.add_xp_scalar }
}
/// `class ExperienceScalarUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ExperienceScalarUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `AddXPScalar`.
    pub add_xp_scalar: f32,
}

impl Default for ExperienceScalarUpgradeModuleData {
    /// `ExperienceScalarUpgradeModuleData()`.
    fn default() -> Self {
        ExperienceScalarUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            add_xp_scalar: if crate::logic::modules::plant(23) { 1.0 } else { 0.0 },
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ExperienceScalarUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(ExperienceScalarUpgradeModuleData, upgrade);

/// `ExperienceScalarUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ExperienceScalarUpgradeModuleData>; 2] = [
    FieldParse::new("AddXPScalar", |i, d, _| { d.add_xp_scalar = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl ExperienceScalarUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ExperienceScalarUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ExperienceScalarUpgradeModuleData> {
        let mut data = ExperienceScalarUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ExperienceScalarUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ExperienceScalarUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ExperienceScalarUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getExperienceScalarUpgradeModuleData()`.
    data: Arc<ExperienceScalarUpgradeModuleData>,
}

impl PartialEq for ExperienceScalarUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ExperienceScalarUpgrade {
    /// `ExperienceScalarUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ExperienceScalarUpgradeModuleData>>) -> IniResult<ExperienceScalarUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ExperienceScalarUpgrade {
            base,
            data,
        })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getExperienceScalarUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ExperienceScalarUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for ExperienceScalarUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        if let Some(current) = world.objects.find(me.object).map(|o| o.experience.scalar) {
            let new = upgrade_implementation(current, &self.data);
            if let Some(o) = world.objects.find_mut(me.object) {
                if !crate::logic::modules::plant(12) {
                    o.experience.scalar = new;
                }
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ExperienceScalarUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ExperienceScalarUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ExperienceScalarUpgrade {
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

/// The factory's `NewModuleDataProc` for `ExperienceScalarUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ExperienceScalarUpgrade(Arc::new(ExperienceScalarUpgradeModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_scalar_is_added_to_the_current_one() {
        let d = ExperienceScalarUpgradeModuleData { add_xp_scalar: 0.25, ..Default::default() };
        assert_eq!(upgrade_implementation(1.0, &d), 1.25);
    }
}
