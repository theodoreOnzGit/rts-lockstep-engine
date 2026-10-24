// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SubObjectsUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/SubObjectsUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SubObjectsUpgrade`: shows and hides named sub-objects of its object's drawable.
//!
//! `ShowSubObjects` and `HideSubObjects` are `parseAsciiStringVectorAppend` lists. Upstream `upgradeImplementation` does nothing if the object or its
//! player already has a conflicting upgrade (it is also run again by `forceRefreshSubObjectUpgradeStatus`, which is why `isSubObjectsUpgrade` is
//! **true**), else, on an object with a drawable, shows and hides the names (`Drawable::showSubObject`) and has the drawable update its sub-objects.
//! The client calls are [`ObjectClientEvent::DrawableShowSubObject`] and [`ObjectClientEvent::DrawableUpdateSubObjects`]. The player of the object
//! is dereferenced without a test, so an object with no controlling player is undefined upstream ([`Undefined::NullControllingPlayer`]).

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::common::game_type::INVALID_DRAWABLE_ID;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::world::object_events::ObjectClientEvent;

/// `class SubObjectsUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct SubObjectsUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_showSubObjectNames`.
    pub show_sub_object_names: Vec<Vec<u8>>,
    /// `m_hideSubObjectNames`.
    pub hide_sub_object_names: Vec<Vec<u8>>,
}

impl Default for SubObjectsUpgradeModuleData {
    /// `SubObjectsUpgradeModuleData()`.
    fn default() -> Self {
        SubObjectsUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            show_sub_object_names: Vec::new(),
            hide_sub_object_names: Vec::new(),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<SubObjectsUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(SubObjectsUpgradeModuleData, upgrade);

/// `SubObjectsUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<SubObjectsUpgradeModuleData>; 3] = [
    FieldParse::new(
        "ShowSubObjects",
        |ini, d, _| {
            if crate::logic::modules::plant(626) {
                d.show_sub_object_names.clear();
            }
            ini.parse_ascii_string_vector_append(&mut d.show_sub_object_names)
        },
        UserData::None,
    ),
    FieldParse::new(
        "HideSubObjects",
        |ini, d, _| if crate::logic::modules::plant(627) { ini.parse_ascii_string_vector_append(&mut d.show_sub_object_names) } else { ini.parse_ascii_string_vector_append(&mut d.hide_sub_object_names) },
        UserData::None,
    ),
    FieldParse::end(),
];

impl SubObjectsUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SubObjectsUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SubObjectsUpgradeModuleData> {
        let mut data = SubObjectsUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SubObjectsUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class SubObjectsUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct SubObjectsUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getSubObjectsUpgradeModuleData()`.
    data: Arc<SubObjectsUpgradeModuleData>,
}

impl PartialEq for SubObjectsUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl SubObjectsUpgrade {
    /// `SubObjectsUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SubObjectsUpgradeModuleData>>) -> IniResult<SubObjectsUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(SubObjectsUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getSubObjectsUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<SubObjectsUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for SubObjectsUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let object = me.object;
        let statics = world.statics.clone();
        let (activation, conflicting) = self.data.upgrade.mux_data_mut().get_upgrade_activation_masks(&statics.upgrade_center, &mut world.names)?;
        let conflicting = if crate::logic::modules::plant(619) { activation } else { conflicting };

        if !crate::logic::modules::plant(617) && world.objects.find(object).map_or(UpgradeMaskType::new(), |o| o.object_upgrades_completed).test_for_any(&conflicting) {
            return Ok(());
        }
        let player = world.get_controlling_player(object).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        if !crate::logic::modules::plant(618) && world.cuts.player_completed_upgrade_mask(player).test_for_any(&conflicting) {
            return Ok(());
        }

        if world.objects.find(object).is_some_and(|o| o.drawable != INVALID_DRAWABLE_ID) || crate::logic::modules::plant(620) {
            let mut update_sub_objects = false;
            let (first, second) = if crate::logic::modules::plant(622) {
                (&self.data.hide_sub_object_names, &self.data.show_sub_object_names)
            } else {
                (&self.data.show_sub_object_names, &self.data.hide_sub_object_names)
            };
            let (first_shows, second_shows) = if crate::logic::modules::plant(621) { (false, true) } else { (!crate::logic::modules::plant(622), crate::logic::modules::plant(622)) };
            for name in first {
                world.emit(ObjectClientEvent::DrawableShowSubObject { object_id: object, name: name.clone(), show: first_shows });
                update_sub_objects = true;
            }
            for name in second {
                world.emit(ObjectClientEvent::DrawableShowSubObject { object_id: object, name: name.clone(), show: second_shows });
                update_sub_objects = true;
            }
            if (update_sub_objects || crate::logic::modules::plant(623)) && !crate::logic::modules::plant(624) {
                world.emit(ObjectClientEvent::DrawableUpdateSubObjects { object_id: object });
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        !crate::logic::modules::plant(625)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::SubObjectsUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SubObjectsUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SubObjectsUpgrade {
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

/// The factory's `NewModuleDataProc` for `SubObjectsUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SubObjectsUpgrade(Arc::new(SubObjectsUpgradeModuleData::friend_new_module_data(ini)?)))
}
