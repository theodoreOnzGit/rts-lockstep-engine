// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StatusBitsUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StatusBitsUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StatusBitsUpgrade`: sets and clears object status bits when the upgrade fires.
//!
//! `StatusToSet` and `StatusToClear` are `ObjectStatusMaskType::parseFromINI` (bit lists). Upstream `upgradeImplementation`: `obj->setStatus(toSet); obj->clearStatus(toClear)`, in that
//! order, with `Object::setStatus`'s own reactions (`World::set_status`). `isSubObjectsUpgrade` is false.

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
use crate::common::object_status_types::ObjectStatusMaskType;

/// `class StatusBitsUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct StatusBitsUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_statusToSet`.
    pub status_to_set: ObjectStatusMaskType,
    /// `m_statusToClear`.
    pub status_to_clear: ObjectStatusMaskType,
}

impl Default for StatusBitsUpgradeModuleData {
    /// `StatusBitsUpgradeModuleData()`.
    fn default() -> Self {
        StatusBitsUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            status_to_set: if crate::logic::modules::plant(604) { crate::logic::world::object_state::mask_of(crate::common::object_status_types::OBJECT_STATUS_UNSELECTABLE) } else { ObjectStatusMaskType::new() },
            status_to_clear: ObjectStatusMaskType::new(),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<StatusBitsUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(StatusBitsUpgradeModuleData, upgrade);

/// `StatusBitsUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<StatusBitsUpgradeModuleData>; 3] = [
    FieldParse::new("StatusToSet", |ini, d, _| d.status_to_set.parse_from_ini(ini), UserData::None),
    FieldParse::new("StatusToClear", |ini, d, _| if crate::logic::modules::plant(606) { d.status_to_set.parse_from_ini(ini) } else { d.status_to_clear.parse_from_ini(ini) }, UserData::None),
    FieldParse::end(),
];

impl StatusBitsUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, StatusBitsUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<StatusBitsUpgradeModuleData> {
        let mut data = StatusBitsUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StatusBitsUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StatusBitsUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct StatusBitsUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getStatusBitsUpgradeModuleData()`.
    data: Arc<StatusBitsUpgradeModuleData>,
}

impl PartialEq for StatusBitsUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl StatusBitsUpgrade {
    /// `StatusBitsUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<StatusBitsUpgradeModuleData>>) -> IniResult<StatusBitsUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(StatusBitsUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getStatusBitsUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<StatusBitsUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for StatusBitsUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let (to_set, to_clear) = (self.data.status_to_set, self.data.status_to_clear);
        if crate::logic::modules::plant(601) {
            world.clear_status(me.object, to_clear);
            world.set_status(me.object, to_set, true);
            return Ok(());
        }
        if !crate::logic::modules::plant(603) {
            world.set_status(me.object, to_set, !crate::logic::modules::plant(605));
        }
        if !crate::logic::modules::plant(602) {
            world.clear_status(me.object, to_clear);
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::StatusBitsUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StatusBitsUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StatusBitsUpgrade {
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

/// The factory's `NewModuleDataProc` for `StatusBitsUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StatusBitsUpgrade(Arc::new(StatusBitsUpgradeModuleData::friend_new_module_data(ini)?)))
}
