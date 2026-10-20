// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/KeepObjectDie.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `KeepObjectDie`: on an applicable death it does nothing at all; the object
//! is kept (its other death effects, such as the Object's own, still run).

use crate::logic::module::die_module::DieModuleData;

/// `KeepObjectDie::onDie(damageInfo)`: nothing to do. `applicable` is
/// `isDieApplicable(damageInfo)`; the call is a no-op either way.
pub fn on_die(_applicable: bool) -> Vec<()> {
    Vec::new()
}

/// The data of a `KeepObjectDie` block: the die mux only.
pub type KeepObjectDieModuleData = DieModuleData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_die_has_no_effect_either_way() {
        assert!(on_die(true).is_empty());
        assert!(on_die(false).is_empty());
    }
}
