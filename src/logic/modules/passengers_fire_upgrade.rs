// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PassengersFireUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/PassengersFireUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PassengersFireUpgrade`: lets the passengers of its object fire from inside.
//!
//! No fields of its own. Upstream `upgradeImplementation`: `if (contain) contain->setPassengerAllowedToFire(TRUE)`; the contain modules are not
//! translated, so the effect is the cut point [`CutCall::ContainSetPassengerAllowedToFire`]. `isSubObjectsUpgrade` is false.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::logic::world::object_cuts::CutCall;

/// `class PassengersFireUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct PassengersFireUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
}

impl Default for PassengersFireUpgradeModuleData {
    /// `PassengersFireUpgradeModuleData()`.
    fn default() -> Self {
        PassengersFireUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<PassengersFireUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(PassengersFireUpgradeModuleData, upgrade);

impl PassengersFireUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PassengersFireUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<PassengersFireUpgradeModuleData> {
        let mut data = PassengersFireUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PassengersFireUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class PassengersFireUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct PassengersFireUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getPassengersFireUpgradeModuleData()`.
    data: Arc<PassengersFireUpgradeModuleData>,
}

impl PartialEq for PassengersFireUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl PassengersFireUpgrade {
    /// `PassengersFireUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<PassengersFireUpgradeModuleData>>) -> IniResult<PassengersFireUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(PassengersFireUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getPassengersFireUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<PassengersFireUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for PassengersFireUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        if (world.get_contain(me.object).is_some() || crate::logic::modules::plant(609)) && !crate::logic::modules::plant(610) {
            world.cut(CutCall::ContainSetPassengerAllowedToFire { object: me.object });
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::PassengersFireUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PassengersFireUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PassengersFireUpgrade {
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

/// The factory's `NewModuleDataProc` for `PassengersFireUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PassengersFireUpgrade(Arc::new(PassengersFireUpgradeModuleData::friend_new_module_data(ini)?)))
}
