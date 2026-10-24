// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MaxHealthUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/MaxHealthUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MaxHealthUpgrade`: adds `AddMaxHealth` to the body's maximum health, with the
//! `ChangeType` that says what happens to current health.
//!
//! Upstream: `body->setMaxHealth(body->getMaxHealth() + m_addMaxHealth,
//! m_maxHealthChangeType)`, if the object has a body module. The body is another
//! module, reached by `World::module_call`, so the change type goes through the
//! new `ModuleCall::SetMaxHealthWithChange`.
//!
//! `ChangeType` is read with `INI::parseIndexList(TheMaxHealthChangeTypeNames)`.
//! Upstream's list has no `NULL` terminator: an unknown token walks past its end
//! (undefined), so an unknown token is an INI error here and is never sent to
//! C++ by the differential test.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{MAX_HEALTH_CHANGE_TYPE_NAMES, SAME_CURRENTHEALTH};
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
/// `MaxHealthUpgrade::upgradeImplementation`'s arguments of the body's
/// `setMaxHealth`: `(current max + AddMaxHealth, ChangeType)`.
pub fn upgrade_implementation(
    current_max_health: f32,
    data: &MaxHealthUpgradeModuleData,
) -> (f32, i32) {
    (
        current_max_health + data.add_max_health,
        if crate::logic::modules::plant(16) {
            SAME_CURRENTHEALTH
        } else {
            data.max_health_change_type
        },
    )
}
/// `class MaxHealthUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct MaxHealthUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `AddMaxHealth`.
    pub add_max_health: f32,
    /// `ChangeType`.
    pub max_health_change_type: i32,
}

impl Default for MaxHealthUpgradeModuleData {
    /// `MaxHealthUpgradeModuleData()`.
    fn default() -> Self {
        MaxHealthUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            add_max_health: 0.0,
            max_health_change_type: if crate::logic::modules::plant(24) {
                1
            } else {
                SAME_CURRENTHEALTH
            },
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<MaxHealthUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(MaxHealthUpgradeModuleData, upgrade);

/// `MaxHealthUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<MaxHealthUpgradeModuleData>; 3] = [
    FieldParse::new(
        "AddMaxHealth",
        |i, d, _| {
            d.add_max_health = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ChangeType",
        |i, d, _| {
            d.max_health_change_type = i.parse_index_list(&MAX_HEALTH_CHANGE_TYPE_NAMES)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl MaxHealthUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, MaxHealthUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<MaxHealthUpgradeModuleData> {
        let mut data = MaxHealthUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MaxHealthUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class MaxHealthUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct MaxHealthUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getMaxHealthUpgradeModuleData()`.
    data: Arc<MaxHealthUpgradeModuleData>,
}

impl PartialEq for MaxHealthUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl MaxHealthUpgrade {
    /// `MaxHealthUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<MaxHealthUpgradeModuleData>>,
    ) -> IniResult<MaxHealthUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(MaxHealthUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getMaxHealthUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<MaxHealthUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for MaxHealthUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) -> Result<(), IniError> {
        let Some(idx) = world.get_body_module(me.object) else {
            return Ok(());
        };
        // body->getMaxHealth()
        let current = match world.module_call(me.object, idx, ModuleCall::GetMaxHealth) {
            ModuleReply::Real(r) => r,
            _ => 0.0,
        };
        let (mut max_health, change_type) = upgrade_implementation(current, &self.data);
        if crate::logic::modules::plant(17) {
            max_health = current - self.data.add_max_health;
        }
        if let Some(trace) = &mut world.cuts.trace {
            trace.push(format!(
                "setMaxHealth {} {}",
                max_health.to_bits(),
                change_type
            ));
        }
        if !crate::logic::modules::plant(18) {
            world.module_call(
                me.object,
                idx,
                ModuleCall::SetMaxHealthWithChange {
                    max_health,
                    change_type,
                },
            );
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::MaxHealthUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MaxHealthUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for MaxHealthUpgrade {
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

/// The factory's `NewModuleDataProc` for `MaxHealthUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MaxHealthUpgrade(Arc::new(
        MaxHealthUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_max_health_is_raised_and_the_change_type_passed_on() {
        let d = MaxHealthUpgradeModuleData {
            add_max_health: 50.0,
            max_health_change_type: 1,
            ..Default::default()
        };
        assert_eq!(upgrade_implementation(200.0, &d), (250.0, 1));
        assert_eq!(
            upgrade_implementation(100.0, &MaxHealthUpgradeModuleData::default()),
            (100.0, SAME_CURRENTHEALTH)
        );
    }
}
