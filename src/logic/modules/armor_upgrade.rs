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

//! `ArmorUpgrade`: flags the object's body with the player upgrade armor set,
//! and for the American chemical-suits upgrade, puts the chemical-suit terrain
//! decal on the object's drawable.
//!
//! Upstream (`ArmorUpgrade::upgradeImplementation`):
//!
//! 1. `if (!obj) return;`
//! 2. if the object has a body, `body->setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)`.
//! 3. if `isTriggeredBy("Upgrade_AmericaChemicalSuits")`,
//!    `obj->getDrawable()->setTerrainDecal(TERRAIN_DECAL_CHEMSUIT)`.
//!
//! The drawable is the client's: the decal call is a client request, so
//! [`upgrade_implementation`] returns it as a plan and the caller emits it.
//! An object with no drawable reaches a null dereference upstream (step 3);
//! that input is [`ChemSuitDecal::NullDrawable`], which the caller must not
//! send to C++ (it is counted, and Rust reports it).

use crate::logic::armor_set::ARMORSET_PLAYER_UPGRADE;
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// The upgrade name that gets the chemical-suit decal.
pub const CHEMICAL_SUITS_UPGRADE: &[u8] = b"Upgrade_AmericaChemicalSuits";

/// What step 3 asks of the drawable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemSuitDecal {
    /// Not triggered by the chemical-suits upgrade: nothing.
    None,
    /// `setTerrainDecal(TERRAIN_DECAL_CHEMSUIT)` on the drawable.
    SetChemSuit,
    /// Triggered, but the object has no drawable: upstream dereferences null.
    NullDrawable,
}

/// The effects of one `upgradeImplementation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmorUpgradePlan {
    /// Whether to call `setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)` on the body.
    pub set_armor_set_flag: bool,
    /// The decal request.
    pub chem_suit_decal: ChemSuitDecal,
}

/// `ArmorUpgrade::upgradeImplementation`, as a plan. `has_body` and
/// `has_drawable` are the object's; `data` is the module's mux data (the
/// trigger names are read through [`UpgradeModuleData::mux_data`]).
pub fn upgrade_implementation(data: &UpgradeModuleData, has_body: bool, has_drawable: bool) -> ArmorUpgradePlan {
    let triggered = data.mux_data().is_triggered_by(CHEMICAL_SUITS_UPGRADE);
    let chem_suit_decal = match (triggered, has_drawable) {
        (false, _) => ChemSuitDecal::None,
        (true, true) => ChemSuitDecal::SetChemSuit,
        (true, false) => ChemSuitDecal::NullDrawable,
    };
    ArmorUpgradePlan { set_armor_set_flag: has_body, chem_suit_decal }
}

/// The armor set the body takes.
pub const ARMOR_SET_TAKEN: i32 = ARMORSET_PLAYER_UPGRADE;

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_armor_upgrade_sets_the_flag_only_with_a_body() {
        let d = UpgradeModuleData::new();
        assert_eq!(
            upgrade_implementation(&d, true, true),
            ArmorUpgradePlan { set_armor_set_flag: true, chem_suit_decal: ChemSuitDecal::None }
        );
        assert!(!upgrade_implementation(&d, false, true).set_armor_set_flag);
        assert_eq!(ARMOR_SET_TAKEN, 3);
    }

    #[test]
    fn chemical_suits_trigger_asks_for_the_decal_and_a_missing_drawable_is_flagged() {
        let d = UpgradeModuleData::new();
        d.mux_data_mut().trigger_upgrade_names = vec![CHEMICAL_SUITS_UPGRADE.to_vec()];
        assert_eq!(upgrade_implementation(&d, true, true).chem_suit_decal, ChemSuitDecal::SetChemSuit);
        assert_eq!(upgrade_implementation(&d, true, false).chem_suit_decal, ChemSuitDecal::NullDrawable);
        let other = UpgradeModuleData::new();
        assert_eq!(upgrade_implementation(&other, true, false).chem_suit_decal, ChemSuitDecal::None);
    }
}
