// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ContainModule.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ContainModuleInterface`'s data: the two enums and the one function with a
//! body.
//!
//! `ContainModule.h` declares **only an interface** (about sixty pure
//! virtuals: `isGarrisonable`, `onContaining`, `addToContain`,
//! `getContainMax`, ...) and `typedef ModuleData ContainModuleData`: there is
//! no `ContainModule` class (the base class is `OpenContain`, layer 9, which
//! also owns the contained-items list and every one of those functions), so
//! the enum `Module` has no contain variant and nothing here dispatches. What
//! this file keeps is what the header defines:
//!
//! - [`ObjectEnterExitType`] and [`EvacDisposition`];
//! - [`ContainModuleData`], an alias of [`ModuleData`](crate::common::module::ModuleData)
//!   (its INI is read by `Module::friend_newModuleData`: only `End`);
//! - `getContainerPipsToShow(numTotal, numFull)`, the one interface function
//!   with a body ([`get_container_pips_to_show`]).
//!
//! `ContainIterateFunc` (a C callback type) and `ContainedItemsList`
//! (`std::list<Object*>`) are layer 8's: objects.

pub use crate::common::module::ModuleData as ContainModuleData;

/// `enum ObjectEnterExitType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ObjectEnterExitType {
    /// `WANTS_TO_ENTER`.
    WantsToEnter = 0,
    /// `WANTS_TO_EXIT`.
    WantsToExit = 1,
    /// `WANTS_NEITHER`.
    WantsNeither = 2,
}

/// `enum EvacDisposition`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum EvacDisposition {
    /// `EVAC_INVALID`.
    Invalid = 0,
    /// `EVAC_TO_LEFT`.
    ToLeft = 1,
    /// `EVAC_TO_RIGHT`.
    ToRight = 2,
    /// `EVAC_BURST_FROM_CENTER`.
    BurstFromCenter = 3,
}

/// `ContainModuleInterface::getContainerPipsToShow(numTotal, numFull)` (the
/// default; "this exists really just so someone can override it to prevent
/// pip showings"): `(getContainMax(), getContainCount() + getExtraSlotsInUse())`,
/// and it returns true. `numFull` is an `Int`: the sum is made in
/// `UnsignedInt` (the count is unsigned) and converted back, so it wraps as
/// the shipped build's does. (The commented-out alternative in the source,
/// `getContainMax() - getExtraSlotsInUse()` and `getContainCount()`, is not
/// compiled.)
pub fn get_container_pips_to_show(contain_max: i32, contain_count: u32, extra_slots_in_use: i32) -> (i32, i32) {
    let num_total = contain_max;
    let num_full = contain_count.wrapping_add(extra_slots_in_use as u32) as i32;
    (num_total, num_full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pips_are_max_and_count_plus_extra() {
        assert_eq!(get_container_pips_to_show(10, 3, 2), (10, 5));
        assert_eq!(get_container_pips_to_show(-1, 0, 0), (-1, 0));
        // the sum is made unsigned and converted back
        assert_eq!(get_container_pips_to_show(4, 0, -1), (4, -1));
        assert_eq!(get_container_pips_to_show(4, 2, -3), (4, -1));
    }

    #[test]
    fn enums_have_upstream_values() {
        assert_eq!(ObjectEnterExitType::WantsNeither as i32, 2);
        assert_eq!(EvacDisposition::BurstFromCenter as i32, 3);
        assert_eq!(EvacDisposition::Invalid as i32, 0);
    }
}
