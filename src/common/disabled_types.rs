// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/DisabledTypes.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/DisabledTypes.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   DisabledTypes.h: Kris Morness, September 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum DisabledType` and `DisabledMaskType`, its [`BitFlags`]. Note that
//! the name of bit 0 (`DISABLED_DEFAULT`) is `"DEFAULT"`, while the others
//! keep their `DISABLED_` prefix. `DISABLED_ANY` (65535) is "read-only", not
//! a bit.

use super::bit_flags::{BitFlagNames, BitFlags};

/// `enum DisabledType`.
pub type DisabledType = i32;

/// `DISABLED_DEFAULT`.
pub const DISABLED_DEFAULT: DisabledType = 0;
/// `DISABLED_HACKED`.
pub const DISABLED_HACKED: DisabledType = 1;
/// `DISABLED_EMP`.
pub const DISABLED_EMP: DisabledType = 2;
/// `DISABLED_HELD`.
pub const DISABLED_HELD: DisabledType = 3;
/// `DISABLED_PARALYZED`.
pub const DISABLED_PARALYZED: DisabledType = 4;
/// `DISABLED_UNMANNED`.
pub const DISABLED_UNMANNED: DisabledType = 5;
/// `DISABLED_UNDERPOWERED`.
pub const DISABLED_UNDERPOWERED: DisabledType = 6;
/// `DISABLED_FREEFALL`.
pub const DISABLED_FREEFALL: DisabledType = 7;
/// `DISABLED_AWESTRUCK`.
pub const DISABLED_AWESTRUCK: DisabledType = 8;
/// `DISABLED_BRAINWASHED`.
pub const DISABLED_BRAINWASHED: DisabledType = 9;
/// `DISABLED_SUBDUED`.
pub const DISABLED_SUBDUED: DisabledType = 10;
/// `DISABLED_SCRIPT_DISABLED`.
pub const DISABLED_SCRIPT_DISABLED: DisabledType = 11;
/// `DISABLED_SCRIPT_UNDERPOWERED`.
pub const DISABLED_SCRIPT_UNDERPOWERED: DisabledType = 12;
/// `DISABLED_COUNT`.
pub const DISABLED_COUNT: DisabledType = 13;
/// `DISABLED_ANY`.
pub const DISABLED_ANY: DisabledType = 65535;

/// Every enumerator in declaration order (name, value), for checks.
pub const DISABLED_ENUMERATORS: [(&str, DisabledType); 15] = [
    ("DISABLED_DEFAULT", DISABLED_DEFAULT),
    ("DISABLED_HACKED", DISABLED_HACKED),
    ("DISABLED_EMP", DISABLED_EMP),
    ("DISABLED_HELD", DISABLED_HELD),
    ("DISABLED_PARALYZED", DISABLED_PARALYZED),
    ("DISABLED_UNMANNED", DISABLED_UNMANNED),
    ("DISABLED_UNDERPOWERED", DISABLED_UNDERPOWERED),
    ("DISABLED_FREEFALL", DISABLED_FREEFALL),
    ("DISABLED_AWESTRUCK", DISABLED_AWESTRUCK),
    ("DISABLED_BRAINWASHED", DISABLED_BRAINWASHED),
    ("DISABLED_SUBDUED", DISABLED_SUBDUED),
    ("DISABLED_SCRIPT_DISABLED", DISABLED_SCRIPT_DISABLED),
    ("DISABLED_SCRIPT_UNDERPOWERED", DISABLED_SCRIPT_UNDERPOWERED),
    ("DISABLED_COUNT", DISABLED_COUNT),
    ("DISABLED_ANY", DISABLED_ANY),
];

/// `DisabledMaskType::s_bitNameList[]`, without the `NULL`.
pub const DISABLED_NAMES: [&str; 13] = [
    "DEFAULT",
    "DISABLED_HACKED",
    "DISABLED_EMP",
    "DISABLED_HELD",
    "DISABLED_PARALYZED",
    "DISABLED_UNMANNED",
    "DISABLED_UNDERPOWERED",
    "DISABLED_FREEFALL",
    "DISABLED_AWESTRUCK",
    "DISABLED_BRAINWASHED",
    "DISABLED_SUBDUED",
    "DISABLED_SCRIPT_DISABLED",
    "DISABLED_SCRIPT_UNDERPOWERED",
];

/// The name table of `DisabledMaskType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisabledNames;
impl BitFlagNames for DisabledNames {
    const NUM_BITS: usize = DISABLED_COUNT as usize;
    const NAMES: &'static [&'static str] = &DISABLED_NAMES;
}

/// `typedef BitFlags<DISABLED_COUNT> DisabledMaskType`.
pub type DisabledMaskType = BitFlags<DisabledNames>;

/// `DISABLEDMASK_NONE`: all clear.
pub const DISABLEDMASK_NONE: DisabledMaskType = DisabledMaskType::new();

/// `DISABLEDMASK_ALL` as `initDisabledMasks()` leaves it: every bit set.
pub fn disabledmask_all() -> DisabledMaskType {
    let mut m = DisabledMaskType::new();
    m.clear();
    m.flip();
    m
}
