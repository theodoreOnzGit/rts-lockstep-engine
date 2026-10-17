// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Powers.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Powers.h: Colin Day, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The anonymous `POWER_*` enum and `PowerNames[]` (defined under
//! `DEFINE_POWER_NAMES`, which `ThingTemplate.cpp` uses). Upstream's comments
//! give bit values (0x1, 0x2, 0x4), but the enumerators are 0..3.

/// `POWER_NONE`.
pub const POWER_NONE: i32 = 0;
/// `POWER_FASTER`.
pub const POWER_FASTER: i32 = 1;
/// `POWER_DOUBLE_SHOT`.
pub const POWER_DOUBLE_SHOT: i32 = 2;
/// `POWER_SELF_HEALING`.
pub const POWER_SELF_HEALING: i32 = 3;
/// `POWERS_NUM_POWERS`.
pub const POWERS_NUM_POWERS: i32 = 4;

/// `PowerNames[]`, without the `NULL` terminator.
pub const POWER_NAMES: [&str; 4] = ["NONE", "FASTER", "DOUBLE_SHOT", "SELF_HEALING"];
