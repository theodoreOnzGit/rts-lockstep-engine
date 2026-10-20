// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectWeaponStatusHelper.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectWeaponStatusHelper`: runs every frame, in the final update phase,
//! so the model condition follows the weapon status (`adjustModelConditionForWeaponStatus`).
//! Unlike the other helpers it starts awake (`UPDATE_SLEEP_NONE`).

use super::object_helper::HelperEffect;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateSleepTime, UPDATE_SLEEP_NONE};

/// The update phase of this helper (`getUpdatePhase()`): `PHASE_FINAL`, so it
/// runs after every normal update module.
pub const UPDATE_PHASE: SleepyUpdatePhase = SleepyUpdatePhase::Final;

/// The delay its constructor gives the module (`setWakeFrame(UPDATE_SLEEP_NONE)`).
pub const CONSTRUCTOR_SLEEP: UpdateSleepTime = UPDATE_SLEEP_NONE;

/// `ObjectWeaponStatusHelper`: no state of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectWeaponStatusHelper;

impl ObjectWeaponStatusHelper {
    /// `ObjectWeaponStatusHelper::update()`: adjusts the model condition; runs every frame.
    pub fn update(&self) -> (UpdateSleepTime, Vec<HelperEffect>) {
        (UPDATE_SLEEP_NONE, vec![HelperEffect::AdjustModelConditionForWeaponStatus])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_runs_every_frame_in_the_final_phase() {
        let (sleep, fx) = ObjectWeaponStatusHelper.update();
        assert_eq!(sleep, UPDATE_SLEEP_NONE);
        assert_eq!(fx, vec![HelperEffect::AdjustModelConditionForWeaponStatus]);
        assert_eq!(UPDATE_PHASE, SleepyUpdatePhase::Final);
        assert_eq!(CONSTRUCTOR_SLEEP, UPDATE_SLEEP_NONE);
    }
}
