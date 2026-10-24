// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ActiveShroudUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ActiveShroudUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ActiveShroudUpgrade`: changes the object's shroud range (`NewShroudRange`), then
//! refreshes the partition cells so the object shrouds where it stands without
//! waiting.
//!
//! Upstream `upgradeImplementation` first tests `getActiveShroudUpgradeModuleData()`
//! for null; the module always has its data, so the test always passes.

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
/// `ActiveShroudUpgrade::upgradeImplementation`'s range: `NewShroudRange`.
pub fn upgrade_implementation(data: &ActiveShroudUpgradeModuleData) -> f32 {
    if crate::logic::modules::plant(15) {
        data.new_shroud_range * 2.0
    } else {
        data.new_shroud_range
    }
}
/// `class ActiveShroudUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ActiveShroudUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `NewShroudRange`.
    pub new_shroud_range: f32,
}

impl Default for ActiveShroudUpgradeModuleData {
    /// `ActiveShroudUpgradeModuleData()`.
    fn default() -> Self {
        ActiveShroudUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            new_shroud_range: if crate::logic::modules::plant(25) {
                1.0
            } else {
                0.0
            },
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ActiveShroudUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(ActiveShroudUpgradeModuleData, upgrade);

/// `ActiveShroudUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ActiveShroudUpgradeModuleData>; 2] = [
    FieldParse::new(
        "NewShroudRange",
        |i, d, _| {
            d.new_shroud_range = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ActiveShroudUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ActiveShroudUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<ActiveShroudUpgradeModuleData> {
        let mut data = ActiveShroudUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                ActiveShroudUpgradeModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class ActiveShroudUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ActiveShroudUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getActiveShroudUpgradeModuleData()`.
    data: Arc<ActiveShroudUpgradeModuleData>,
}

impl PartialEq for ActiveShroudUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ActiveShroudUpgrade {
    /// `ActiveShroudUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<ActiveShroudUpgradeModuleData>>,
    ) -> IniResult<ActiveShroudUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ActiveShroudUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getActiveShroudUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ActiveShroudUpgradeModuleData> {
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

impl UpgradeBehavior for ActiveShroudUpgrade {
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
        let range = upgrade_implementation(&self.data);
        if !crate::logic::modules::plant(14) {
            world.set_shroud_range(me.object, range);
        }
        if !crate::logic::modules::plant(13) {
            world.handle_partition_cell_maintenance(me.object); // "To shroud where I am without waiting."
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ActiveShroudUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ActiveShroudUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ActiveShroudUpgrade {
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

/// The factory's `NewModuleDataProc` for `ActiveShroudUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ActiveShroudUpgrade(Arc::new(
        ActiveShroudUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_upgrade_sets_the_configured_range() {
        let d = ActiveShroudUpgradeModuleData {
            new_shroud_range: 150.5,
            ..Default::default()
        };
        assert_eq!(upgrade_implementation(&d), 150.5);
        assert_eq!(
            upgrade_implementation(&ActiveShroudUpgradeModuleData::default()),
            0.0
        );
    }
}
