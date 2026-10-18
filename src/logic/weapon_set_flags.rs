// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponSetFlags.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp (WeaponSetFlags::s_bitNameList)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   WeaponSet.cpp: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponSetFlags`, the [`BitFlags`] over [`super::weapon_types`]'s
//! `WeaponSetType`. The names are those of `WeaponSet.cpp`: note
//! `CRATEUPGRADE_ONE` (no `WEAPONSET_` prefix) and `WEAPON_RIDER1`..`8`.

use super::weapon_types::WEAPONSET_COUNT;
use crate::common::bit_flags::{BitFlagNames, BitFlags};

/// `WeaponSetFlags::s_bitNameList[]` (WeaponSet.cpp), without the `NULL`.
pub const WEAPON_SET_NAMES: [&str; 17] = [
    "VETERAN",
    "ELITE",
    "HERO",
    "PLAYER_UPGRADE",
    "CRATEUPGRADE_ONE",
    "CRATEUPGRADE_TWO",
    "VEHICLE_HIJACK",
    "CARBOMB",
    "MINE_CLEARING_DETAIL",
    "WEAPON_RIDER1",
    "WEAPON_RIDER2",
    "WEAPON_RIDER3",
    "WEAPON_RIDER4",
    "WEAPON_RIDER5",
    "WEAPON_RIDER6",
    "WEAPON_RIDER7",
    "WEAPON_RIDER8",
];

/// The name table of `WeaponSetFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WeaponSetNames;
impl BitFlagNames for WeaponSetNames {
    const NUM_BITS: usize = WEAPONSET_COUNT as usize;
    const NAMES: &'static [&'static str] = &WEAPON_SET_NAMES;
}

/// `typedef BitFlags<WEAPONSET_COUNT> WeaponSetFlags`.
pub type WeaponSetFlags = BitFlags<WeaponSetNames>;
