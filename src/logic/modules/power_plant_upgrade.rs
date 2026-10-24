// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PowerPlantUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/PowerPlantUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PowerPlantUpgrade`: adds the power bonus of its object (the "overcharged" rods of a power plant) to the player who owns it, and
//! tells the object's `PowerPlantUpdate` to extend its rods.
//!
//! No fields of its own. Upstream `upgradeImplementation`: `if (player) player->addPowerBonus(obj)` ([`CutCall::PlayerAddPowerBonus`]: the
//! energy is `Player`'s, layer 12's), then every behaviour module of the object that has a `PowerPlantUpdateInterface` gets `extendRods(TRUE)`
//! ([`ModuleCall::ExtendRods`], sent to every module but this one, which has none). `onDelete` and `onCapture` move the bonus (a disabled
//! object's capture does nothing). `loadPostProcess` re-adds the bonus "if we are turned on": "Most upgrade modules have state change effects
//! that are themselves saved. This one is a fire and forget" -- that half needs the world, so it is
//! [`UpgradeBehavior::load_post_process_world`] ([`ModuleCall::LoadPostProcess`]). `isSubObjectsUpgrade` is false.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class PowerPlantUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct PowerPlantUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
}

impl Default for PowerPlantUpgradeModuleData {
    /// `PowerPlantUpgradeModuleData()`.
    fn default() -> Self {
        PowerPlantUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<PowerPlantUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(PowerPlantUpgradeModuleData, upgrade);

impl PowerPlantUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, PowerPlantUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<PowerPlantUpgradeModuleData> {
        let mut data = PowerPlantUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                PowerPlantUpgradeModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class PowerPlantUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct PowerPlantUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getPowerPlantUpgradeModuleData()`.
    data: Arc<PowerPlantUpgradeModuleData>,
}

impl PartialEq for PowerPlantUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl PowerPlantUpgrade {
    /// `PowerPlantUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<PowerPlantUpgradeModuleData>>,
    ) -> IniResult<PowerPlantUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(PowerPlantUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getPowerPlantUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<PowerPlantUpgradeModuleData> {
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

impl UpgradeBehavior for PowerPlantUpgrade {
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
        // add the new power production to the object
        if let Some(player) = world.get_controlling_player(me.object) {
            if !crate::logic::modules::plant(653) {
                world.cut(CutCall::PlayerAddPowerBonus {
                    player,
                    object: me.object,
                });
            }
        }
        // for each behaviour module with a PowerPlantUpdateInterface: ppui->extendRods(TRUE)
        for idx in world.all_modules(me.object) {
            if idx == me.index || (crate::logic::modules::plant(655) && idx == me.index + 1) {
                continue;
            }
            world.module_call(
                me.object,
                idx,
                ModuleCall::ExtendRods {
                    extend: !crate::logic::modules::plant(654),
                },
            );
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        // if we haven't been upgraded there is nothing to clean up
        if !self.base.is_already_upgraded() && !crate::logic::modules::plant(650) {
            return;
        }
        // remove the power bonus from the player
        if let Some(player) = world.get_controlling_player(me.object) {
            world.cut(CutCall::PlayerRemovePowerBonus {
                player,
                object: me.object,
            });
        }
        // this upgrade module is now "not upgraded"
        if !crate::logic::modules::plant(658) {
            self.base.mux_mut().set_upgrade_executed(false);
        }
    }

    fn on_capture(&mut self, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
        // do nothing if we haven't upgraded yet
        if !self.base.is_already_upgraded() {
            return;
        }
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
            && !crate::logic::modules::plant(651)
        {
            return;
        }
        let (old_owner, new_owner) = if crate::logic::modules::plant(652) {
            (new_owner, old_owner)
        } else {
            (old_owner, new_owner)
        };
        // remove power bonus from old owner
        if old_owner >= 0 {
            world.cut(CutCall::PlayerRemovePowerBonus {
                player: old_owner,
                object: me.object,
            });
            self.base.mux_mut().set_upgrade_executed(false);
        }
        // add power bonus to the new owner
        if new_owner >= 0 {
            world.cut(CutCall::PlayerAddPowerBonus {
                player: new_owner,
                object: me.object,
            });
            self.base.mux_mut().set_upgrade_executed(true);
        }
    }

    fn load_post_process_world(&mut self, world: &mut World, me: ModuleHandle) {
        // Most upgrade modules have state change effects that are themselves saved. This one is a fire and forget.
        // So we need to re-fire on load if we are turned on.
        if (self.base.is_already_upgraded() || crate::logic::modules::plant(657))
            && !crate::logic::modules::plant(656)
        {
            // add the new power production to the object
            if let Some(player) = world.get_controlling_player(me.object) {
                world.cut(CutCall::PlayerAddPowerBonus {
                    player,
                    object: me.object,
                });
            }
        }
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::PowerPlantUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PowerPlantUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PowerPlantUpgrade {
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
    /// `loadPostProcess`: `UpgradeModule::loadPostProcess` (its world half is `load_post_process_world`).
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `PowerPlantUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PowerPlantUpgrade(Arc::new(
        PowerPlantUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}
