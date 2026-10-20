// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WeaponSetUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponSetUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponSetUpgrade`: the upgrade that flags its object as having the player
//! upgrade, so that the weapon set chooser picks the upgraded weapon set from
//! INI.
//!
//! It has no fields of its own: its data is a plain [`UpgradeModuleData`].
//! Its one effect is [`WeaponSetUpgrade::upgrade_implementation`]: the object
//! takes `WEAPONSET_PLAYER_UPGRADE`. `isSubObjectsUpgrade` is false.
//!
//! Upstream's `crc`, `xfer` (version 1) and `loadPostProcess` only extend the
//! base class; they are the layer-5 [`UpgradeModule`] ones.
//!
//! # Verification
//!
//! Unit tests below; the differential test (`tests/differential_upgrade_modules.rs`)
//! is written against the C++ of this file once the wiring lands.

use crate::logic::weapon_types::{WeaponSetType, WEAPONSET_PLAYER_UPGRADE};

/// `WeaponSetUpgrade::upgradeImplementation`: the weapon set flag the object
/// takes (`setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE)`).
pub fn upgrade_implementation() -> WeaponSetType {
    WEAPONSET_PLAYER_UPGRADE
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upgrade_sets_the_player_upgrade_weapon_set_flag() {
        assert_eq!(upgrade_implementation(), WEAPONSET_PLAYER_UPGRADE);
        assert_eq!(WEAPONSET_PLAYER_UPGRADE, 3);
    }

    #[test]
    fn it_is_not_a_sub_objects_upgrade() {
        assert!(!is_sub_objects_upgrade());
    }
}
