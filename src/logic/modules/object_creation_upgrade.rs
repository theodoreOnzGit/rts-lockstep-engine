// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectCreationUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ObjectCreationUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectCreationUpgrade`: makes the objects of an `ObjectCreationList` at its object when the upgrade fires.
//!
//! `UpgradeObject` names the list (`INI::parseObjectCreationList`: the store is looked in when the INI is read, `None` and an unknown name give no list). Here
//! the name is kept and the list is found when it is used ([`CutCall::OclCreate`], answered by `OclHost` of the world), as `ScienceRequired` is. Upstream
//! `upgradeImplementation`: `if (m_ocl) ObjectCreationList::create(m_ocl, getObject(), NULL)`. `onDelete` is empty. `isSubObjectsUpgrade` is false.

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
use crate::logic::world::object_cuts::CutCall;

/// `class ObjectCreationUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ObjectCreationUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_ocl`, by name (`None` for `NULL`).
    pub ocl: Option<Vec<u8>>,
}

impl Default for ObjectCreationUpgradeModuleData {
    /// `ObjectCreationUpgradeModuleData()`.
    fn default() -> Self {
        ObjectCreationUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            ocl: None,
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ObjectCreationUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(ObjectCreationUpgradeModuleData, upgrade);

/// `ObjectCreationUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ObjectCreationUpgradeModuleData>; 2] = [
    FieldParse::new(
        "UpgradeObject",
        |ini, d, _| {
            // INI::parseObjectCreationList: `None` is no list; any other name is looked up (here, when it is used)
            let token = ini.get_next_token(None)?;
            d.ocl = (!token.eq_ignore_ascii_case(b"None")).then(|| if crate::logic::modules::plant(631) { token.to_ascii_uppercase() } else { token });
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ObjectCreationUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ObjectCreationUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ObjectCreationUpgradeModuleData> {
        let mut data = ObjectCreationUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ObjectCreationUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ObjectCreationUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ObjectCreationUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getObjectCreationUpgradeModuleData()`.
    data: Arc<ObjectCreationUpgradeModuleData>,
}

impl PartialEq for ObjectCreationUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ObjectCreationUpgrade {
    /// `ObjectCreationUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ObjectCreationUpgradeModuleData>>) -> IniResult<ObjectCreationUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ObjectCreationUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getObjectCreationUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ObjectCreationUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for ObjectCreationUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        if crate::logic::modules::plant(630) {
            return Ok(());
        }
        if let Some(ocl) = self.data.ocl.clone().or_else(|| crate::logic::modules::plant(629).then(Vec::new)) {
            world.cut(CutCall::OclCreate { object: me.object, ocl });
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ObjectCreationUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ObjectCreationUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ObjectCreationUpgrade {
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

/// The factory's `NewModuleDataProc` for `ObjectCreationUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ObjectCreationUpgrade(Arc::new(ObjectCreationUpgradeModuleData::friend_new_module_data(ini)?)))
}
