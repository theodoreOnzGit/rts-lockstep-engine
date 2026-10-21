// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DestroyDie.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DestroyDie`: destroys its object when it dies (if the die mux applies).
//! The applicability check is the layer-5 [`DieModuleData::is_die_applicable`],
//! which the caller evaluates with the object's veterancy and status bits.

use crate::logic::module::die_module::DieModuleData;

/// What a [`DestroyDie`] asks of its object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestroyDieEffect {
    /// `TheGameLogic->destroyObject(obj)`.
    DestroyObject,
}

/// `DestroyDie::onDie(damageInfo)`. `applicable` is
/// `isDieApplicable(damageInfo)` (the caller's evaluation of the mux).
pub fn on_die(applicable: bool) -> Option<DestroyDieEffect> {
    if applicable { Some(DestroyDieEffect::DestroyObject) } else { None }
}

/// The data of a `DestroyDie` block: the die mux only.
pub type DestroyDieModuleData = DieModuleData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_applicable_death_destroys_the_object_and_others_do_not() {
        assert_eq!(on_die(true), Some(DestroyDieEffect::DestroyObject));
        assert_eq!(on_die(false), None);
    }
}
