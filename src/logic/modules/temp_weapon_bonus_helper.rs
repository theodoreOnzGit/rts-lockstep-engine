// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/TempWeaponBonusHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TempWeaponBonusHelper`: gives an object a timed weapon bonus condition
//! (`doTempWeaponBonus`) and tints its drawable while it lasts; the helper
//! removes both when its timer wakes it.

use super::object_helper::{HelperEffect, TINT_STATUS_FRENZY};
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::logic::weapon_bonus::condition::INVALID as WEAPONBONUSCONDITION_INVALID;

/// `TempWeaponBonusHelper`: the bonus being timed and the frame it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempWeaponBonusHelper {
    /// `m_currentBonus` (`WEAPONBONUSCONDITION_INVALID` when idle).
    pub current_bonus: i32,
    /// `m_frameToRemove`.
    pub frame_to_remove: u32,
}

impl Default for TempWeaponBonusHelper {
    /// The constructor: no bonus, frame 0, then `setWakeFrame(FOREVER)`.
    fn default() -> Self {
        TempWeaponBonusHelper { current_bonus: WEAPONBONUSCONDITION_INVALID, frame_to_remove: 0 }
    }
}

impl TempWeaponBonusHelper {
    /// The delay the constructor gives the module.
    pub const CONSTRUCTOR_SLEEP: UpdateSleepTime = UPDATE_SLEEP_FOREVER;

    /// `TempWeaponBonusHelper::update()`: clear the bonus (and the tint, if
    /// there is a drawable), then sleep forever.
    pub fn update(&mut self, has_drawable: bool) -> (UpdateSleepTime, Vec<HelperEffect>) {
        let mut fx = Vec::new();
        self.clear_temp_weapon_bonus(has_drawable, &mut fx);
        (UPDATE_SLEEP_FOREVER, fx)
    }

    /// `clearTempWeaponBonus()`.
    fn clear_temp_weapon_bonus(&mut self, has_drawable: bool, fx: &mut Vec<HelperEffect>) {
        if self.current_bonus != WEAPONBONUSCONDITION_INVALID {
            fx.push(HelperEffect::ClearWeaponBonusCondition(self.current_bonus));
            self.current_bonus = WEAPONBONUSCONDITION_INVALID;
            self.frame_to_remove = 0;
            if has_drawable {
                fx.push(HelperEffect::DrawableTint { status: TINT_STATUS_FRENZY, on: false });
            }
        }
    }

    /// `doTempWeaponBonus(status, duration)` at frame `now`: a different bonus
    /// clears the old one first; the same one resets the timer.
    pub fn do_temp_weapon_bonus(&mut self, now: u32, has_drawable: bool, status: i32, duration: u32) -> (UpdateSleepTime, Vec<HelperEffect>) {
        let mut fx = Vec::new();
        if self.current_bonus != status {
            self.clear_temp_weapon_bonus(has_drawable, &mut fx);
        }
        fx.push(HelperEffect::SetWeaponBonusCondition(status));
        self.current_bonus = status;
        self.frame_to_remove = now.wrapping_add(duration);
        if has_drawable {
            fx.push(HelperEffect::DrawableTint { status: TINT_STATUS_FRENZY, on: true });
        }
        (update_sleep(duration), fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_is_idle_and_sleeps_forever() {
        assert_eq!(TempWeaponBonusHelper::default(), TempWeaponBonusHelper { current_bonus: -1, frame_to_remove: 0 });
        assert_eq!(TempWeaponBonusHelper::CONSTRUCTOR_SLEEP, UPDATE_SLEEP_FOREVER);
    }

    #[test]
    fn do_temp_weapon_bonus_sets_the_bonus_and_tints_when_there_is_a_drawable() {
        let mut h = TempWeaponBonusHelper::default();
        let (sleep, fx) = h.do_temp_weapon_bonus(50, true, 1, 30);
        assert_eq!(fx, vec![HelperEffect::SetWeaponBonusCondition(1), HelperEffect::DrawableTint { status: TINT_STATUS_FRENZY, on: true }]);
        assert_eq!((sleep, h.current_bonus, h.frame_to_remove), (update_sleep(30), 1, 80));
        let (_, fx) = h.do_temp_weapon_bonus(51, false, 1, 30);
        assert_eq!(fx, vec![HelperEffect::SetWeaponBonusCondition(1)]);
    }

    #[test]
    fn a_different_bonus_clears_the_old_one_and_its_tint_first() {
        let mut h = TempWeaponBonusHelper::default();
        h.do_temp_weapon_bonus(0, true, 1, 30);
        let (_, fx) = h.do_temp_weapon_bonus(1, true, 2, 10);
        assert_eq!(
            fx,
            vec![
                HelperEffect::ClearWeaponBonusCondition(1),
                HelperEffect::DrawableTint { status: TINT_STATUS_FRENZY, on: false },
                HelperEffect::SetWeaponBonusCondition(2),
                HelperEffect::DrawableTint { status: TINT_STATUS_FRENZY, on: true },
            ]
        );
    }

    #[test]
    fn update_clears_the_bonus_and_goes_back_to_sleep() {
        let mut h = TempWeaponBonusHelper::default();
        h.do_temp_weapon_bonus(0, false, 3, 30);
        let (sleep, fx) = h.update(false);
        assert_eq!((sleep, fx), (UPDATE_SLEEP_FOREVER, vec![HelperEffect::ClearWeaponBonusCondition(3)]));
        assert_eq!(h, TempWeaponBonusHelper::default());
        assert!(h.update(true).1.is_empty());
    }
}
