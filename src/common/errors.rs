// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Errors.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Errors.h: Steven Johnson, August 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum ErrorCode`: the values upstream throws (rarely returns). The Rust
//! returns typed errors instead; these values are kept where an error has to
//! say which upstream code it stands for (e.g. `common::ini::Thrown`).
//!
//! They are `u32` here. `0xdead0001` does not fit `int`, so a conforming C++
//! compiler (g++, the differential reference) gives the enum an unsigned
//! underlying type, while MSVC, which the game was built with, keeps every
//! enum `int`, where the values are negative. Only the bits matter: they are
//! thrown and compared for equality, never ordered.

/// `ERROR_BASE`: "a nice, distinctive value".
pub const ERROR_BASE: u32 = 0xdead_0001;
/// `ERROR_BUG`: should not be possible under normal operation.
pub const ERROR_BUG: u32 = ERROR_BASE;
/// `ERROR_OUT_OF_MEMORY`.
pub const ERROR_OUT_OF_MEMORY: u32 = ERROR_BASE + 0x0001;
/// `ERROR_BAD_ARG`.
pub const ERROR_BAD_ARG: u32 = ERROR_BASE + 0x0002;
/// `ERROR_INVALID_FILE_VERSION`.
pub const ERROR_INVALID_FILE_VERSION: u32 = ERROR_BASE + 0x0003;
/// `ERROR_CORRUPT_FILE_FORMAT`.
pub const ERROR_CORRUPT_FILE_FORMAT: u32 = ERROR_BASE + 0x0004;
/// `ERROR_BAD_INI`.
pub const ERROR_BAD_INI: u32 = ERROR_BASE + 0x0005;
/// `ERROR_INVALID_D3D`.
pub const ERROR_INVALID_D3D: u32 = ERROR_BASE + 0x0006;
/// `ERROR_LAST`.
pub const ERROR_LAST: u32 = ERROR_BASE + 0x0007;
