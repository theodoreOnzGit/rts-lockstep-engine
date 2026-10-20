// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WeaponBonusUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponBonusUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponBonusUpgrade`: the upgrade that sets the object's
//! `WEAPONBONUSCONDITION_PLAYER_UPGRADE` weapon bonus condition.
//!
//! No fields of its own (a plain [`UpgradeModuleData`]). Its effect is
//! [`upgrade_implementation`]; `isSubObjectsUpgrade` is false.

use crate::logic::weapon_bonus::condition::PLAYER_UPGRADE;

/// `WeaponBonusUpgrade::upgradeImplementation`: the bonus condition the object
/// takes (`setWeaponBonusCondition(WEAPONBONUSCONDITION_PLAYER_UPGRADE)`).
pub fn upgrade_implementation() -> i32 {
    PLAYER_UPGRADE
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upgrade_sets_the_player_upgrade_bonus_condition() {
        assert_eq!(upgrade_implementation(), 5);
        assert!(!is_sub_objects_upgrade());
    }
}
