// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectScriptStatusBits.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum ObjectScriptStatusBit`: status bits set by map scripts. Upstream
//! notes that `Object` stores them in a `Byte`.

/// `OBJECT_STATUS_SCRIPT_DISABLED`.
pub const OBJECT_STATUS_SCRIPT_DISABLED: u8 = 0x01;
/// `OBJECT_STATUS_SCRIPT_UNPOWERED`.
pub const OBJECT_STATUS_SCRIPT_UNPOWERED: u8 = 0x02;
/// `OBJECT_STATUS_SCRIPT_UNSELLABLE`.
pub const OBJECT_STATUS_SCRIPT_UNSELLABLE: u8 = 0x04;
/// `OBJECT_STATUS_SCRIPT_UNSTEALTHED`: cannot stealth.
pub const OBJECT_STATUS_SCRIPT_UNSTEALTHED: u8 = 0x08;
/// `OBJECT_STATUS_SCRIPT_TARGETABLE`: targetable by the player, not
/// auto-acquired.
pub const OBJECT_STATUS_SCRIPT_TARGETABLE: u8 = 0x10;
