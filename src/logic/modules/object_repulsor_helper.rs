// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectRepulsorHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectRepulsorHelper`: ends the temporary `OBJECT_STATUS_REPULSOR` status
//! when it wakes (`Object::setStatus` arms it with `sleepUntil`).

use super::object_helper::HelperEffect;
use crate::common::object_status_types::OBJECT_STATUS_REPULSOR;
use crate::logic::module::update_module::{UpdateSleepTime, UPDATE_SLEEP_FOREVER};

/// `ObjectRepulsorHelper`: no state of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectRepulsorHelper;

impl ObjectRepulsorHelper {
    /// `ObjectRepulsorHelper::update()`: clears `OBJECT_STATUS_REPULSOR`, sleeps forever.
    pub fn update(&self) -> (UpdateSleepTime, Vec<HelperEffect>) {
        (UPDATE_SLEEP_FOREVER, vec![HelperEffect::ClearStatus(OBJECT_STATUS_REPULSOR)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_clears_the_repulsor_status_then_sleeps_forever() {
        let (sleep, fx) = ObjectRepulsorHelper.update();
        assert_eq!(sleep, UPDATE_SLEEP_FOREVER);
        assert_eq!(fx, vec![HelperEffect::ClearStatus(9)]);
    }
}
