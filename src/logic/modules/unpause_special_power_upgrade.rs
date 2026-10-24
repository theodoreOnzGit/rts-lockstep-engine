// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UnpauseSpecialPowerUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UnpauseSpecialPowerUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `UnpauseSpecialPowerUpgrade`: unpauses the countdown of one special power of its object.
//!
//! `SpecialPowerTemplate` is `INI::parseSpecialPowerTemplate`: the token names a template of `TheSpecialPowerStore` (`None`, and a name the
//! store does not know, store a `NULL` pointer). **Here the name is kept and looked up in `world.statics.special_power_store` when the
//! upgrade runs**, because a module's data is made by a `NewModuleDataProc` that has no store to look in (the same choice
//! `ObjectCreationUpgrade` made for its list); the answer is the same while the store does not change between the INI load and the game,
//! which it does not. (~~upstream throws `ERROR_BUG` while `TheSpecialPowerStore` is null~~ the Rust read has no store to be null.)
//! Upstream `upgradeImplementation`: for every behaviour module with a `SpecialPowerModuleInterface`, in module order, `if
//! (sp->getSpecialPowerTemplate() == m_specialPower) sp->pauseCountdown(FALSE)`: the comparison is of pointers, so a module whose own template
//! is `NULL` matches a `NULL` here ([`ModuleCall::GetSpecialPowerTemplateId`], `PauseCountdown`). `isSubObjectsUpgrade` is false.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::MODULEINTERFACE_SPECIAL_POWER;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class UnpauseSpecialPowerUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct UnpauseSpecialPowerUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_specialPower`, as the token that names it (see the module docs); `None` for a pointer that is `NULL` before any lookup.
    pub special_power: Option<Vec<u8>>,
}

impl Default for UnpauseSpecialPowerUpgradeModuleData {
    /// `UnpauseSpecialPowerUpgradeModuleData()`.
    fn default() -> Self {
        UnpauseSpecialPowerUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            special_power: None,
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<UnpauseSpecialPowerUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(UnpauseSpecialPowerUpgradeModuleData, upgrade);

/// `UnpauseSpecialPowerUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<UnpauseSpecialPowerUpgradeModuleData>; 2] = [
    FieldParse::new(
        "SpecialPowerTemplate",
        |ini, d, _| {
            d.special_power = Some(ini.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl UnpauseSpecialPowerUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, UnpauseSpecialPowerUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<UnpauseSpecialPowerUpgradeModuleData> {
        let mut data = UnpauseSpecialPowerUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                UnpauseSpecialPowerUpgradeModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class UnpauseSpecialPowerUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct UnpauseSpecialPowerUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getUnpauseSpecialPowerUpgradeModuleData()`.
    data: Arc<UnpauseSpecialPowerUpgradeModuleData>,
}

impl PartialEq for UnpauseSpecialPowerUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl UnpauseSpecialPowerUpgrade {
    /// `UnpauseSpecialPowerUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<UnpauseSpecialPowerUpgradeModuleData>>,
    ) -> IniResult<UnpauseSpecialPowerUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(UnpauseSpecialPowerUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getUnpauseSpecialPowerUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<UnpauseSpecialPowerUpgradeModuleData> {
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

impl UpgradeBehavior for UnpauseSpecialPowerUpgrade {
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
        // `m_specialPower`: the store's template of that name, `NULL` if it has none
        let statics = world.statics.clone();
        let wanted: Option<u32> = self
            .data
            .special_power
            .as_deref()
            .and_then(|n| statics.special_power_store.find_special_power_template(n))
            .map(|t| t.id());
        for idx in world.modules_with(me.object, MODULEINTERFACE_SPECIAL_POWER) {
            let theirs =
                match world.module_call(me.object, idx, ModuleCall::GetSpecialPowerTemplateId) {
                    ModuleReply::Int(id) => Some(id as u32),
                    _ => None,
                };
            let matches = if crate::logic::modules::plant(670) {
                true
            } else if crate::logic::modules::plant(671) {
                theirs.is_some() && theirs == wanted
            } else {
                theirs == wanted
            };
            if matches {
                world.module_call(
                    me.object,
                    idx,
                    ModuleCall::PauseCountdown {
                        disabling: crate::logic::modules::plant(672),
                    },
                );
                if crate::logic::modules::plant(673) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::UnpauseSpecialPowerUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::UnpauseSpecialPowerUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for UnpauseSpecialPowerUpgrade {
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

/// The factory's `NewModuleDataProc` for `UnpauseSpecialPowerUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::UnpauseSpecialPowerUpgrade(Arc::new(
        UnpauseSpecialPowerUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}
