// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RadarUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/RadarUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RadarUpgrade`: gives the player who owns the object a radar, and tells the object's `RadarUpdate` to extend the radar.
//!
//! `DisableProof` (`INI::parseBool`) says the radar survives being disabled. Upstream `upgradeImplementation`: `player->addRadar(disableProof)`
//! on the controlling player, **dereferenced without a test** (an object with no controlling player is undefined upstream,
//! [`Undefined::NullControllingPlayer`]; [`CutCall::PlayerAddRadar`]: the radar count is `Player`'s, layer 12's), then
//! `NAMEKEY("RadarUpdate")` (which assigns a key if the name is new) and `findUpdateModule`, and `extendRadar()` on what it finds
//! ([`ModuleCall::ExtendRadar`] to the object's module of that class name). `onDelete` and `onCapture` move the radar, and do nothing for a disabled
//! object ("because we've already done it"). `isSubObjectsUpgrade` is false.
//!
//! `findModule` compares the class name here and the key of the class name upstream; upstream's `getModuleNameKey()` assigns the key of a
//! class the first time a module of it is asked, here nothing is assigned, so a later `nameToKey` of a brand new name can get a different key.

use std::sync::Arc;

use crate::common::ini::Undefined;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
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

/// `class RadarUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct RadarUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_isDisableProof`.
    pub is_disable_proof: bool,
}

impl Default for RadarUpgradeModuleData {
    /// `RadarUpgradeModuleData()`.
    fn default() -> Self {
        RadarUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            is_disable_proof: false,
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<RadarUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(RadarUpgradeModuleData, upgrade);

/// `RadarUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<RadarUpgradeModuleData>; 2] = [
    FieldParse::new(
        "DisableProof",
        |ini, d, _| {
            d.is_disable_proof = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl RadarUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, RadarUpgradeModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RadarUpgradeModuleData> {
        let mut data = RadarUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RadarUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class RadarUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct RadarUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getRadarUpgradeModuleData()`.
    data: Arc<RadarUpgradeModuleData>,
}

impl PartialEq for RadarUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl RadarUpgrade {
    /// `RadarUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<RadarUpgradeModuleData>>,
    ) -> IniResult<RadarUpgrade> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(RadarUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getRadarUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<RadarUpgradeModuleData> {
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

impl UpgradeBehavior for RadarUpgrade {
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
        let player = world
            .get_controlling_player(me.object)
            .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        // update the player with another radar facility
        world.cut(CutCall::PlayerAddRadar {
            player,
            disable_proof: self.data.is_disable_proof != crate::logic::modules::plant(662),
        });
        // find the radar update module of this object
        if !crate::logic::modules::plant(664) {
            world
                .names
                .try_name_to_key(b"RadarUpdate")
                .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        }
        if let Some(idx) = world.find_module(me.object, b"RadarUpdate") {
            if !crate::logic::modules::plant(663) {
                world.module_call(me.object, idx, ModuleCall::ExtendRadar);
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        // if we haven't been upgraded there is nothing to clean up
        if !self.base.is_already_upgraded() {
            return;
        }
        // If we're currently disabled, we shouldn't do anything, because we've already done it.
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
            && !crate::logic::modules::plant(660)
        {
            return;
        }
        // remove the radar from the player
        if let Some(player) = world.get_controlling_player(me.object) {
            world.cut(CutCall::PlayerRemoveRadar {
                player,
                disable_proof: self.data.is_disable_proof,
            });
        }
        // this upgrade module is now "not upgraded"
        if !crate::logic::modules::plant(665) {
            self.base.mux_mut().set_upgrade_executed(false);
        }
    }

    fn on_capture(&mut self, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
        // do nothing if we haven't upgraded yet
        if !self.base.is_already_upgraded() {
            return;
        }
        // If we're currently disabled, we shouldn't do anything, because we've already done it.
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
            && !crate::logic::modules::plant(661)
        {
            return;
        }
        let (old_owner, new_owner) = if crate::logic::modules::plant(666) {
            (new_owner, old_owner)
        } else {
            (old_owner, new_owner)
        };
        // remove radar from old player and add to new player
        if old_owner >= 0 {
            world.cut(CutCall::PlayerRemoveRadar {
                player: old_owner,
                disable_proof: self.data.is_disable_proof,
            });
            self.base.mux_mut().set_upgrade_executed(false);
        }
        if new_owner >= 0 {
            world.cut(CutCall::PlayerAddRadar {
                player: new_owner,
                disable_proof: self.data.is_disable_proof,
            });
            self.base.mux_mut().set_upgrade_executed(true);
        }
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::RadarUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RadarUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RadarUpgrade {
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

/// The factory's `NewModuleDataProc` for `RadarUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RadarUpgrade(Arc::new(
        RadarUpgradeModuleData::friend_new_module_data(ini)?,
    )))
}
