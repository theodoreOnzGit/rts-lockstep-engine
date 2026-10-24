// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ArmorSet.h (ArmorSetType, ArmorSetFlags)
//   GeneralsMD/Code/GameEngine/Source/Common/BitFlags.cpp (ArmorSetFlags::s_bitNameList)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum ArmorSetType` and `ArmorSetFlags`, its [`BitFlags`]. The rest of
//! `ArmorSet.h` (`ArmorTemplateSet`, its `SparseMatchFinder`) comes with
//! the armor templates (layer 4).

use crate::common::bit_flags::{BitFlagNames, BitFlags};

/// `enum ArmorSetType`.
pub type ArmorSetType = i32;

/// `ARMORSET_VETERAN`.
pub const ARMORSET_VETERAN: ArmorSetType = 0;
/// `ARMORSET_ELITE`.
pub const ARMORSET_ELITE: ArmorSetType = 1;
/// `ARMORSET_HERO`.
pub const ARMORSET_HERO: ArmorSetType = 2;
/// `ARMORSET_PLAYER_UPGRADE`.
pub const ARMORSET_PLAYER_UPGRADE: ArmorSetType = 3;
/// `ARMORSET_WEAK_VERSUS_BASEDEFENSES`.
pub const ARMORSET_WEAK_VERSUS_BASEDEFENSES: ArmorSetType = 4;
/// `ARMORSET_SECOND_LIFE`.
pub const ARMORSET_SECOND_LIFE: ArmorSetType = 5;
/// `ARMORSET_CRATE_UPGRADE_ONE`.
pub const ARMORSET_CRATE_UPGRADE_ONE: ArmorSetType = 6;
/// `ARMORSET_CRATE_UPGRADE_TWO`.
pub const ARMORSET_CRATE_UPGRADE_TWO: ArmorSetType = 7;
/// `ARMORSET_COUNT`.
pub const ARMORSET_COUNT: ArmorSetType = 8;

/// Every enumerator in declaration order (name, value), for checks.
pub const ARMOR_SET_ENUMERATORS: [(&str, ArmorSetType); 9] = [
    ("ARMORSET_VETERAN", ARMORSET_VETERAN),
    ("ARMORSET_ELITE", ARMORSET_ELITE),
    ("ARMORSET_HERO", ARMORSET_HERO),
    ("ARMORSET_PLAYER_UPGRADE", ARMORSET_PLAYER_UPGRADE),
    (
        "ARMORSET_WEAK_VERSUS_BASEDEFENSES",
        ARMORSET_WEAK_VERSUS_BASEDEFENSES,
    ),
    ("ARMORSET_SECOND_LIFE", ARMORSET_SECOND_LIFE),
    ("ARMORSET_CRATE_UPGRADE_ONE", ARMORSET_CRATE_UPGRADE_ONE),
    ("ARMORSET_CRATE_UPGRADE_TWO", ARMORSET_CRATE_UPGRADE_TWO),
    ("ARMORSET_COUNT", ARMORSET_COUNT),
];

/// `ArmorSetFlags::s_bitNameList[]` (BitFlags.cpp), without the `NULL`.
pub const ARMOR_SET_NAMES: [&str; 8] = [
    "VETERAN",
    "ELITE",
    "HERO",
    "PLAYER_UPGRADE",
    "WEAK_VERSUS_BASEDEFENSES",
    "SECOND_LIFE",
    "CRATE_UPGRADE_ONE",
    "CRATE_UPGRADE_TWO",
];

/// The name table of `ArmorSetFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArmorSetNames;
impl BitFlagNames for ArmorSetNames {
    const NUM_BITS: usize = ARMORSET_COUNT as usize;
    const NAMES: &'static [&'static str] = &ARMOR_SET_NAMES;
}

/// `typedef BitFlags<ARMORSET_COUNT> ArmorSetFlags`.
pub type ArmorSetFlags = BitFlags<ArmorSetNames>;
