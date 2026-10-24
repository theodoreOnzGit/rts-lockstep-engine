// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WeaponSetUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponSetUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponSetUpgrade`: flags its object with the player upgrade, so the weapon
//! set chooser takes the upgraded weapon set from INI.
//!
//! No fields of its own (a plain `UpgradeModuleData`). Upstream
//! `upgradeImplementation`: `obj->setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE)`.
//! `isSubObjectsUpgrade` is false. The effect is `World::set_weapon_set_flag`.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::weapon_types::WEAPONSET_PLAYER_UPGRADE;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
/// `class WeaponSetUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct WeaponSetUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
}

impl Default for WeaponSetUpgradeModuleData {
    /// `WeaponSetUpgradeModuleData()`.
    fn default() -> Self {
        WeaponSetUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<WeaponSetUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(WeaponSetUpgradeModuleData, upgrade);

impl WeaponSetUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, WeaponSetUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<WeaponSetUpgradeModuleData> {
        let mut data = WeaponSetUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, WeaponSetUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class WeaponSetUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct WeaponSetUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getWeaponSetUpgradeModuleData()`.
    data: Arc<WeaponSetUpgradeModuleData>,
}

impl PartialEq for WeaponSetUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl WeaponSetUpgrade {
    /// `WeaponSetUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<WeaponSetUpgradeModuleData>>,
    ) -> IniResult<WeaponSetUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(WeaponSetUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getWeaponSetUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<WeaponSetUpgradeModuleData> {
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

impl UpgradeBehavior for WeaponSetUpgrade {
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
        if !crate::logic::modules::plant(2) {
            world.set_weapon_set_flag(
                me.object,
                if crate::logic::modules::plant(1) {
                    0
                } else {
                    WEAPONSET_PLAYER_UPGRADE
                },
            );
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::WeaponSetUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::WeaponSetUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for WeaponSetUpgrade {
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

/// The factory's `NewModuleDataProc` for `WeaponSetUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::WeaponSetUpgrade(Arc::new(
        WeaponSetUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_upgrade_sets_the_player_upgrade_weapon_set_flag() {
        assert_eq!(WEAPONSET_PLAYER_UPGRADE, 3);
    }
}
