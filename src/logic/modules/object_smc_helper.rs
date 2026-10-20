// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectSMCHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectSMCHelper`: "special model condition" helper. When it wakes (it is
//! only ever awakened forcibly), it clears the object's special model
//! conditions and sleeps forever.

use super::object_helper::HelperEffect;
use crate::logic::module::update_module::{UpdateSleepTime, UPDATE_SLEEP_FOREVER};

/// `ObjectSMCHelper`: no state of its own (version 1 xfers nothing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectSmcHelper;

impl ObjectSmcHelper {
    /// `ObjectSMCHelper::update()`: the effects, and the sleep (forever).
    pub fn update(&self) -> (UpdateSleepTime, Vec<HelperEffect>) {
        (UPDATE_SLEEP_FOREVER, vec![HelperEffect::ClearSpecialModelConditionStates])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_clears_special_model_conditions_then_sleeps_forever() {
        let (sleep, fx) = ObjectSmcHelper.update();
        assert_eq!(sleep, UPDATE_SLEEP_FOREVER);
        assert_eq!(fx, vec![HelperEffect::ClearSpecialModelConditionStates]);
    }
}
