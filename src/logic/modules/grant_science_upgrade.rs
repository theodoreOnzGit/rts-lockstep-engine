// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GrantScienceUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/GrantScienceUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GrantScienceUpgrade`: grants a science to the object's controlling player. The
//! science is named by `GrantScience`, looked up on the first upgrade, and kept
//! in the module.
//!
//! Upstream: `m_scienceType` starts as `SCIENCE_INVALID`; while it is invalid,
//! `TheScienceStore->getScienceFromInternalName(name)` is read (so a failed lookup
//! is retried next time). Then `player->grantScience(m_scienceType)` if the object
//! has a controlling player. The grant is `CutCall::PlayerGrantScience`.
//!
//! Upstream behaviour kept as shipped: `grantScience` is called with whatever
//! `m_scienceType` is, including `SCIENCE_INVALID` after a failed lookup.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
/// The science `GrantScienceUpgrade::upgradeImplementation` grants: the cached
/// one if valid, otherwise `lookup(name)`, kept in `cached` (an invalid
/// result is looked up again next time).
pub fn science_to_grant(
    cached: &mut ScienceType,
    name: &[u8],
    lookup: impl FnOnce(&[u8]) -> ScienceType,
) -> ScienceType {
    if *cached == SCIENCE_INVALID || crate::logic::modules::plant(20) {
        *cached = lookup(name);
    }
    *cached
}
/// `class GrantScienceUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct GrantScienceUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `GrantScience`.
    pub grant_science_name: Vec<u8>,
}

impl Default for GrantScienceUpgradeModuleData {
    /// `GrantScienceUpgradeModuleData()`.
    fn default() -> Self {
        GrantScienceUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            grant_science_name: Vec::new(),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<GrantScienceUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(GrantScienceUpgradeModuleData, upgrade);

/// `GrantScienceUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<GrantScienceUpgradeModuleData>; 2] = [
    FieldParse::new(
        "GrantScience",
        |i, d, _| {
            d.grant_science_name = i.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl GrantScienceUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, GrantScienceUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<GrantScienceUpgradeModuleData> {
        let mut data = GrantScienceUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                GrantScienceUpgradeModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class GrantScienceUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct GrantScienceUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getGrantScienceUpgradeModuleData()`.
    data: Arc<GrantScienceUpgradeModuleData>,
    /// Module state.
    science: ScienceType,
}

impl PartialEq for GrantScienceUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl GrantScienceUpgrade {
    /// `GrantScienceUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<GrantScienceUpgradeModuleData>>,
    ) -> IniResult<GrantScienceUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(GrantScienceUpgrade {
            base,
            data,
            science: SCIENCE_INVALID,
        })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getGrantScienceUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<GrantScienceUpgradeModuleData> {
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

    /// `m_scienceType`: the science the first upgrade looked up (`SCIENCE_INVALID` before it).
    pub fn cached_science(&self) -> ScienceType {
        self.science
    }
}

impl UpgradeBehavior for GrantScienceUpgrade {
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
        let name = self.data.grant_science_name.clone();
        let statics = world.statics.clone();
        let mut lookup_error = None;
        let science = science_to_grant(&mut self.science, &name, |n| {
            match statics
                .science_store
                .get_science_from_internal_name(&mut world.names, n)
            {
                Ok(s) => s,
                Err(e) => {
                    lookup_error = Some(e);
                    SCIENCE_INVALID
                }
            }
        });
        if lookup_error.is_some() {
            return Err(IniError::undefined(
                crate::common::ini::Undefined::NameKeysExhausted,
            ));
        }
        if let Some(player) = world.get_controlling_player(me.object) {
            if !crate::logic::modules::plant(21) {
                world.cut(CutCall::PlayerGrantScience {
                    player,
                    science: science.0,
                });
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::GrantScienceUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::GrantScienceUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for GrantScienceUpgrade {
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

/// The factory's `NewModuleDataProc` for `GrantScienceUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::GrantScienceUpgrade(Arc::new(
        GrantScienceUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_lookup_runs_until_it_succeeds_then_is_kept() {
        let mut cached = SCIENCE_INVALID;
        let mut calls = 0;
        let first = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            SCIENCE_INVALID
        });
        assert_eq!(first, SCIENCE_INVALID);
        let second = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            ScienceType(7)
        });
        assert_eq!(second, ScienceType(7));
        let third = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            ScienceType(9)
        });
        assert_eq!((third, calls), (ScienceType(7), 2));
    }
}
