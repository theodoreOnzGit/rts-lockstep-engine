// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/StatusDamageHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StatusDamageHelper`: gives an object a timed status (`doStatusDamage`),
//! sets it, and clears it when its timer wakes the helper.

use super::object_helper::HelperEffect;
use crate::common::base_type::real_to_int_floor;
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_NONE};
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER};

/// `StatusDamageHelper`: the status being timed and the frame it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusDamageHelper {
    /// `m_statusToHeal` (`OBJECT_STATUS_NONE` when idle).
    pub status_to_heal: ObjectStatusTypes,
    /// `m_frameToHeal`.
    pub frame_to_heal: u32,
}

impl Default for StatusDamageHelper {
    /// The constructor: no status, frame 0, then `setWakeFrame(FOREVER)`
    /// (the caller applies [`StatusDamageHelper::CONSTRUCTOR_SLEEP`]).
    fn default() -> Self {
        StatusDamageHelper { status_to_heal: OBJECT_STATUS_NONE, frame_to_heal: 0 }
    }
}

impl StatusDamageHelper {
    /// The delay the constructor gives the module.
    pub const CONSTRUCTOR_SLEEP: UpdateSleepTime = UPDATE_SLEEP_FOREVER;

    /// `StatusDamageHelper::update()`: "we are sleep driven, so seeing an
    /// update means our timer is ready": clear the status, sleep forever.
    pub fn update(&mut self) -> (UpdateSleepTime, Vec<HelperEffect>) {
        let mut fx = Vec::new();
        self.clear_status_condition(&mut fx);
        (UPDATE_SLEEP_FOREVER, fx)
    }

    /// `clearStatusCondition()`: if a status is being timed, clear its bit
    /// and reset the timer.
    fn clear_status_condition(&mut self, fx: &mut Vec<HelperEffect>) {
        if self.status_to_heal != OBJECT_STATUS_NONE {
            fx.push(HelperEffect::ClearStatus(self.status_to_heal));
            self.status_to_heal = OBJECT_STATUS_NONE;
            self.frame_to_heal = 0;
        }
    }

    /// `doStatusDamage(status, duration)` at frame `now`. The duration is
    /// truncated to whole frames (`REAL_TO_INT_FLOOR`); the frame sum and the
    /// sleep wrap as the C++ unsigned arithmetic does.
    pub fn do_status_damage(&mut self, now: u32, status: ObjectStatusTypes, duration: f32) -> (UpdateSleepTime, Vec<HelperEffect>) {
        let duration_as_int = real_to_int_floor(duration);
        let mut fx = Vec::new();
        // Clear any different status we may have. Re-getting the same status just resets the timer.
        if self.status_to_heal != status {
            self.clear_status_condition(&mut fx);
        }
        fx.push(HelperEffect::SetStatus(status));
        self.status_to_heal = status;
        self.frame_to_heal = now.wrapping_add(duration_as_int as u32);
        (update_sleep(duration_as_int as u32), fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::object_status_types::{OBJECT_STATUS_AFLAME, OBJECT_STATUS_WET};

    #[test]
    fn constructor_is_idle_and_sleeps_forever() {
        assert_eq!(StatusDamageHelper::default(), StatusDamageHelper { status_to_heal: 0, frame_to_heal: 0 });
        assert_eq!(StatusDamageHelper::CONSTRUCTOR_SLEEP, UPDATE_SLEEP_FOREVER);
    }

    #[test]
    fn do_status_damage_sets_the_status_and_sleeps_the_whole_frames() {
        let mut h = StatusDamageHelper::default();
        let (sleep, fx) = h.do_status_damage(100, OBJECT_STATUS_AFLAME, 12.9);
        assert_eq!(fx, vec![HelperEffect::SetStatus(OBJECT_STATUS_AFLAME)]);
        assert_eq!(sleep, update_sleep(12));
        assert_eq!((h.status_to_heal, h.frame_to_heal), (OBJECT_STATUS_AFLAME, 112));
    }

    #[test]
    fn a_different_status_clears_the_old_one_first() {
        let mut h = StatusDamageHelper::default();
        h.do_status_damage(0, OBJECT_STATUS_AFLAME, 5.0);
        let (_, fx) = h.do_status_damage(2, OBJECT_STATUS_WET, 5.0);
        assert_eq!(fx, vec![HelperEffect::ClearStatus(OBJECT_STATUS_AFLAME), HelperEffect::SetStatus(OBJECT_STATUS_WET)]);
    }

    #[test]
    fn the_same_status_only_resets_the_timer() {
        let mut h = StatusDamageHelper::default();
        h.do_status_damage(0, OBJECT_STATUS_AFLAME, 5.0);
        let (sleep, fx) = h.do_status_damage(3, OBJECT_STATUS_AFLAME, 4.0);
        assert_eq!(fx, vec![HelperEffect::SetStatus(OBJECT_STATUS_AFLAME)]);
        assert_eq!((sleep, h.frame_to_heal), (update_sleep(4), 7));
    }

    #[test]
    fn update_clears_the_status_and_goes_back_to_sleep() {
        let mut h = StatusDamageHelper::default();
        h.do_status_damage(0, OBJECT_STATUS_AFLAME, 5.0);
        let (sleep, fx) = h.update();
        assert_eq!(sleep, UPDATE_SLEEP_FOREVER);
        assert_eq!(fx, vec![HelperEffect::ClearStatus(OBJECT_STATUS_AFLAME)]);
        assert_eq!(h, StatusDamageHelper::default());
        assert!(h.update().1.is_empty());
    }
}
