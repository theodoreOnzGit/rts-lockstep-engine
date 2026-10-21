// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CostModifierUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CostModifierUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CostModifierUpgrade`: changes the cost of building a kind of object, for the player who owns the object, by a percentage.
//!
//! `EffectKindOf` is a kind-of bit list (`KindOfMaskType::parseFromINI`), `Percentage` `INI::parsePercentToReal`. Upstream
//! `upgradeImplementation`: `player->addKindOfProductionCostChange(kindOf, percentage)` on the controlling player, **dereferenced without a test**, so an
//! object with no controlling player is undefined upstream ([`Undefined::NullControllingPlayer`]). `onDelete` and `onCapture` undo and redo it
//! ([`CutCall::PlayerRemoveKindOfProductionCostChange`] and [`CutCall::PlayerAddKindOfProductionCostChange`]: the player's effect is `Player`'s,
//! layer 12's), tracking it with `setUpgradeExecuted`. `isSubObjectsUpgrade` is false.

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
use crate::common::kind_of::KindOfMaskType;
use crate::common::ini::Undefined;
use crate::logic::world::object_cuts::CutCall;

/// `class CostModifierUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct CostModifierUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_kindOf` (`KINDOFMASK_NONE`).
    pub kind_of: KindOfMaskType,
    /// `m_percentage`.
    pub percentage: f32,
}

impl Default for CostModifierUpgradeModuleData {
    /// `CostModifierUpgradeModuleData()`.
    fn default() -> Self {
        CostModifierUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            kind_of: KindOfMaskType::new(),
            percentage: 0.0,
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<CostModifierUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(CostModifierUpgradeModuleData, upgrade);

/// `CostModifierUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<CostModifierUpgradeModuleData>; 3] = [
    FieldParse::new("EffectKindOf", |ini, d, _| if crate::logic::modules::plant(646) { Ok(()) } else { d.kind_of.parse_from_ini(ini) }, UserData::None),
    FieldParse::new("Percentage", |ini, d, _| { d.percentage = if crate::logic::modules::plant(645) { ini.parse_real()? } else { ini.parse_percent_to_real()? }; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CostModifierUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CostModifierUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CostModifierUpgradeModuleData> {
        let mut data = CostModifierUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CostModifierUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class CostModifierUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct CostModifierUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getCostModifierUpgradeModuleData()`.
    data: Arc<CostModifierUpgradeModuleData>,
}

impl PartialEq for CostModifierUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl CostModifierUpgrade {
    /// `CostModifierUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CostModifierUpgradeModuleData>>) -> IniResult<CostModifierUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(CostModifierUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getCostModifierUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<CostModifierUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for CostModifierUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let player = world.get_controlling_player(me.object).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        // update the player with another TypeOfProductionCostChange
        if !crate::logic::modules::plant(647) {
            world.cut(CutCall::PlayerAddKindOfProductionCostChange { player, kind_of: self.data.kind_of.clone(), percent: self.data.percentage });
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        // if we haven't been upgraded there is nothing to clean up
        if !self.base.is_already_upgraded() && !crate::logic::modules::plant(640) {
            return;
        }
        // remove the cost change from the player
        if let Some(player) = world.get_controlling_player(me.object) {
            world.cut(CutCall::PlayerRemoveKindOfProductionCostChange { player, kind_of: self.data.kind_of.clone(), percent: self.data.percentage });
        }
        // this upgrade module is now "not upgraded"
        if !crate::logic::modules::plant(641) {
            self.base.mux_mut().set_upgrade_executed(false);
        }
    }

    fn on_capture(&mut self, world: &mut World, _me: ModuleHandle, old_owner: i32, new_owner: i32) {
        // do nothing if we haven't upgraded yet
        if !self.base.is_already_upgraded() {
            return;
        }
        // remove the cost change from the old player and add it to the new player
        if old_owner >= 0 {
            if !crate::logic::modules::plant(642) {
                world.cut(CutCall::PlayerRemoveKindOfProductionCostChange { player: old_owner, kind_of: self.data.kind_of.clone(), percent: self.data.percentage });
            }
            if !crate::logic::modules::plant(644) {
                self.base.mux_mut().set_upgrade_executed(false);
            }
        }
        if new_owner >= 0 {
            if !crate::logic::modules::plant(643) {
                world.cut(CutCall::PlayerAddKindOfProductionCostChange { player: new_owner, kind_of: self.data.kind_of.clone(), percent: self.data.percentage });
            }
            self.base.mux_mut().set_upgrade_executed(true);
        }
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::CostModifierUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CostModifierUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CostModifierUpgrade {
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

/// The factory's `NewModuleDataProc` for `CostModifierUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CostModifierUpgrade(Arc::new(CostModifierUpgradeModuleData::friend_new_module_data(ini)?)))
}
