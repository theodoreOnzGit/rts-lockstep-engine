// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StealthUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StealthUpgrade`: gives its object the ability to stealth, and its spawn's slaves too.
//!
//! No fields of its own. Upstream `upgradeImplementation`: `setStatus(OBJECT_STATUS_CAN_STEALTH)`, then, for an object that is
//! `KINDOF_SPAWNS_ARE_THE_WEAPONS` and has a spawn behaviour, `giveSlavesStealthUpgrade(TRUE)` (the cut point [`CutCall::SpawnGiveSlavesStealthUpgrade`]).
//! `isSubObjectsUpgrade` is false.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse};
use crate::common::kind_of::KINDOF_SPAWNS_ARE_THE_WEAPONS;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_CAN_STEALTH;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class StealthUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct StealthUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
}

impl Default for StealthUpgradeModuleData {
    /// `StealthUpgradeModuleData()`.
    fn default() -> Self {
        StealthUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<StealthUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(StealthUpgradeModuleData, upgrade);

impl StealthUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, StealthUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<StealthUpgradeModuleData> {
        let mut data = StealthUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StealthUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StealthUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct StealthUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getStealthUpgradeModuleData()`.
    data: Arc<StealthUpgradeModuleData>,
}

impl PartialEq for StealthUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl StealthUpgrade {
    /// `StealthUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<StealthUpgradeModuleData>>,
    ) -> IniResult<StealthUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(StealthUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getStealthUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<StealthUpgradeModuleData> {
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

impl UpgradeBehavior for StealthUpgrade {
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
        let object = me.object;
        if !crate::logic::modules::plant(611) {
            world.set_status(
                object,
                mask_of(if crate::logic::modules::plant(612) {
                    crate::common::object_status_types::OBJECT_STATUS_STEALTHED
                } else {
                    OBJECT_STATUS_CAN_STEALTH
                }),
                true,
            );
        }
        let kind = world
            .objects
            .find(object)
            .is_some_and(|o| o.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS))
            || crate::logic::modules::plant(613);
        let spawn = world.get_spawn_behavior_interface(object).is_some()
            || crate::logic::modules::plant(614);
        if (if crate::logic::modules::plant(616) {
            kind || spawn
        } else {
            kind && spawn
        }) && !crate::logic::modules::plant(615)
        {
            world.cut(CutCall::SpawnGiveSlavesStealthUpgrade { object });
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::StealthUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StealthUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StealthUpgrade {
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

/// The factory's `NewModuleDataProc` for `StealthUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StealthUpgrade(Arc::new(
        StealthUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}
