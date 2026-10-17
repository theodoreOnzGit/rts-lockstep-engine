// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponSetType.h
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponStatus.h
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponBonusConditionFlags.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The weapon-set indices, the weapon status and the bonus-condition flag
//! word. The weapon-set *names* (`TheWeaponSetNames`) and the flag type
//! `WeaponSetFlags` come with `WeaponSetFlags.h`.

/// `enum WeaponSetType`: an index ("the bit shifting is built in, so this
/// is a 0,1,2,3,4,5 enum").
pub type WeaponSetType = i32;
/// `WEAPONSET_VETERAN`.
pub const WEAPONSET_VETERAN: WeaponSetType = 0;
/// `WEAPONSET_ELITE`.
pub const WEAPONSET_ELITE: WeaponSetType = 1;
/// `WEAPONSET_HERO`.
pub const WEAPONSET_HERO: WeaponSetType = 2;
/// `WEAPONSET_PLAYER_UPGRADE`: from an upgrade the player bought.
pub const WEAPONSET_PLAYER_UPGRADE: WeaponSetType = 3;
/// `WEAPONSET_CRATEUPGRADE_ONE`.
pub const WEAPONSET_CRATEUPGRADE_ONE: WeaponSetType = 4;
/// `WEAPONSET_CRATEUPGRADE_TWO`.
pub const WEAPONSET_CRATEUPGRADE_TWO: WeaponSetType = 5;
/// `WEAPONSET_VEHICLE_HIJACK`.
pub const WEAPONSET_VEHICLE_HIJACK: WeaponSetType = 6;
/// `WEAPONSET_CARBOMB`.
pub const WEAPONSET_CARBOMB: WeaponSetType = 7;
/// `WEAPONSET_MINE_CLEARING_DETAIL`.
pub const WEAPONSET_MINE_CLEARING_DETAIL: WeaponSetType = 8;
/// `WEAPONSET_RIDER1` (combat-bike riders, 1 to 8).
pub const WEAPONSET_RIDER1: WeaponSetType = 9;
/// `WEAPONSET_RIDER2`.
pub const WEAPONSET_RIDER2: WeaponSetType = 10;
/// `WEAPONSET_RIDER3`.
pub const WEAPONSET_RIDER3: WeaponSetType = 11;
/// `WEAPONSET_RIDER4`.
pub const WEAPONSET_RIDER4: WeaponSetType = 12;
/// `WEAPONSET_RIDER5`.
pub const WEAPONSET_RIDER5: WeaponSetType = 13;
/// `WEAPONSET_RIDER6`.
pub const WEAPONSET_RIDER6: WeaponSetType = 14;
/// `WEAPONSET_RIDER7`.
pub const WEAPONSET_RIDER7: WeaponSetType = 15;
/// `WEAPONSET_RIDER8`.
pub const WEAPONSET_RIDER8: WeaponSetType = 16;
/// `WEAPONSET_COUNT`.
pub const WEAPONSET_COUNT: WeaponSetType = 17;

/// `enum WeaponStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum WeaponStatus {
    ReadyToFire = 0,
    OutOfAmmo = 1,
    BetweenFiringShots = 2,
    ReloadingClip = 3,
    PreAttack = 4,
}
/// `WEAPON_STATUS_COUNT`.
pub const WEAPON_STATUS_COUNT: i32 = 5;

/// `typedef UnsignedInt WeaponBonusConditionFlags`.
pub type WeaponBonusConditionFlags = u32;
