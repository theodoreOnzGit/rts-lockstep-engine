// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ArmorUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ArmorUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ArmorUpgrade`: flags the object's body with the player-upgrade armor set, and
//! for the American chemical-suits upgrade puts the chemical-suit terrain decal
//! on the object's drawable.
//!
//! Upstream `upgradeImplementation`:
//!
//! 1. `if (!obj) return;`
//! 2. if the object has a body, `body->setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)`.
//! 3. if `isTriggeredBy("Upgrade_AmericaChemicalSuits")`,
//!    `obj->getDrawable()->setTerrainDecal(TERRAIN_DECAL_CHEMSUIT)`.
//!
//! The drawable is the client's, so the decal is a `ObjectClientEvent`. An object
//! with no drawable reaches a null dereference in step 3; that input is
//! `Undefined::NullDrawable` (an error, never sent to C++).

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
use crate::logic::armor_set::ARMORSET_PLAYER_UPGRADE;
use crate::common::ini::Undefined;
use crate::common::game_type::INVALID_DRAWABLE_ID;
use crate::logic::world::object_events::ObjectClientEvent;
/// `TERRAIN_DECAL_CHEMSUIT` (`GameClient/Drawable.h`, the enum `TerrainDecalType`):
/// the eighth value with `ALLOW_DEMORALIZE` undefined (as the shipped build).
pub const TERRAIN_DECAL_CHEMSUIT: i32 = 7;

/// The upgrade name that gets the chemical-suit decal.
pub const CHEMICAL_SUITS_UPGRADE: &[u8] = b"Upgrade_AmericaChemicalSuits";

/// What step 3 asks of the drawable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemSuitDecal {
    /// Not triggered by the chemical-suits upgrade: nothing.
    None,
    /// `setTerrainDecal(TERRAIN_DECAL_CHEMSUIT)` on the drawable.
    SetChemSuit,
    /// Triggered on an object with no drawable: upstream dereferences null.
    NullDrawable,
}

/// The effects of one `upgradeImplementation`, as a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmorUpgradePlan {
    /// Whether to call `setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)` on the body.
    pub set_armor_set_flag: bool,
    /// The decal request.
    pub chem_suit_decal: ChemSuitDecal,
}

/// `ArmorUpgrade::upgradeImplementation` as a plan. `has_body` and
/// `has_drawable` are the object's.
pub fn upgrade_implementation(data: &UpgradeModuleData, has_body: bool, has_drawable: bool) -> ArmorUpgradePlan {
    let triggered = data.mux_data().is_triggered_by(CHEMICAL_SUITS_UPGRADE) != crate::logic::modules::plant(6);
    let chem_suit_decal = match (triggered, has_drawable) {
        (false, _) => ChemSuitDecal::None,
        (true, true) => ChemSuitDecal::SetChemSuit,
        (true, false) => ChemSuitDecal::NullDrawable,
    };
    ArmorUpgradePlan { set_armor_set_flag: has_body && !crate::logic::modules::plant(5), chem_suit_decal }
}
/// `class ArmorUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ArmorUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
}

impl Default for ArmorUpgradeModuleData {
    /// `ArmorUpgradeModuleData()`.
    fn default() -> Self {
        ArmorUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ArmorUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(ArmorUpgradeModuleData, upgrade);

impl ArmorUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ArmorUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ArmorUpgradeModuleData> {
        let mut data = ArmorUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ArmorUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ArmorUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ArmorUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getArmorUpgradeModuleData()`.
    data: Arc<ArmorUpgradeModuleData>,
}

impl PartialEq for ArmorUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ArmorUpgrade {
    /// `ArmorUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ArmorUpgradeModuleData>>) -> IniResult<ArmorUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ArmorUpgrade {
            base,
            data,
        })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getArmorUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ArmorUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for ArmorUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let has_drawable = match world.objects.find(me.object) {
            Some(o) => o.drawable != INVALID_DRAWABLE_ID,
            None => return Ok(()), // "if( !obj ) return;"
        };
        let has_body = world.get_body_module(me.object).is_some();
        let plan = upgrade_implementation(&self.data.upgrade, has_body, has_drawable);
        if plan.set_armor_set_flag {
            if let Some(idx) = world.get_body_module(me.object) {
                world.module_call(me.object, idx, ModuleCall::SetArmorSetFlag(ARMORSET_PLAYER_UPGRADE));
            }
        }
        match plan.chem_suit_decal {
            ChemSuitDecal::None => {}
            ChemSuitDecal::SetChemSuit => {
                world.emit(ObjectClientEvent::DrawableSetTerrainDecal { object_id: me.object, decal: TERRAIN_DECAL_CHEMSUIT + crate::logic::modules::plant(7) as i32 });
            }
            ChemSuitDecal::NullDrawable => return Err(IniError::undefined(Undefined::NullDrawable)),
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ArmorUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ArmorUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ArmorUpgrade {
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

/// The factory's `NewModuleDataProc` for `ArmorUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ArmorUpgrade(Arc::new(ArmorUpgradeModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_armor_upgrade_sets_the_flag_only_with_a_body() {
        let d = UpgradeModuleData::new();
        assert_eq!(upgrade_implementation(&d, true, true), ArmorUpgradePlan { set_armor_set_flag: true, chem_suit_decal: ChemSuitDecal::None });
        assert!(!upgrade_implementation(&d, false, true).set_armor_set_flag);
        assert_eq!(ARMORSET_PLAYER_UPGRADE, 3);
    }

    #[test]
    fn chemical_suits_trigger_asks_for_the_decal_and_a_missing_drawable_is_flagged() {
        let d = UpgradeModuleData::new();
        d.mux_data_mut().trigger_upgrade_names = vec![CHEMICAL_SUITS_UPGRADE.to_vec()];
        assert_eq!(upgrade_implementation(&d, true, true).chem_suit_decal, ChemSuitDecal::SetChemSuit);
        assert_eq!(upgrade_implementation(&d, true, false).chem_suit_decal, ChemSuitDecal::NullDrawable);
        assert_eq!(upgrade_implementation(&UpgradeModuleData::new(), true, false).chem_suit_decal, ChemSuitDecal::None);
        assert_eq!(TERRAIN_DECAL_CHEMSUIT, 7);
    }
}
