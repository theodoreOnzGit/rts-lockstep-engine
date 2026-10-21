// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectHelper.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectHelper.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectHelper`: the base of the sleepy helper modules, and the object-side
//! effects the helpers cause.
//!
//! The helpers are update modules that sleep until forcibly awakened. Their
//! `update()` and their `do*` calls change the object (status bits, model
//! condition, weapon bonus, drawable tint). Those changes are cut points into
//! `Object`, so each helper returns them as a [`HelperEffect`] list, in the
//! order upstream makes them; the caller applies the list to the object.
//!
//! # Cut points
//!
//! - `TheGameLogic->getFrame()` is passed in as `now`.
//! - `getObject()->status / weapon bonus / model condition / drawable` are
//!   the [`HelperEffect`]s; `getDrawable()` is passed in as `has_drawable`.
//! - `setWakeFrame(getObject(), delay)` is the returned [`UpdateSleepTime`]:
//!   the caller applies it to the module's own wake frame, as upstream does.

use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::game_common::{FOREVER, NEVER};
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER};

/// `TINT_STATUS_FRENZY` (`Drawable.h`).
pub const TINT_STATUS_FRENZY: i32 = 0x0000_0010;

/// One change a helper makes to its object, in upstream's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperEffect {
    /// `getObject()->setStatus(MAKE_OBJECT_STATUS_MASK(status))`.
    SetStatus(ObjectStatusTypes),
    /// `getObject()->clearStatus(MAKE_OBJECT_STATUS_MASK(status))`.
    ClearStatus(ObjectStatusTypes),
    /// `getObject()->clearSpecialModelConditionStates()`.
    ClearSpecialModelConditionStates,
    /// `getObject()->adjustModelConditionForWeaponStatus()`.
    AdjustModelConditionForWeaponStatus,
    /// `getObject()->setWeaponBonusCondition(bonus)`.
    SetWeaponBonusCondition(i32),
    /// `getObject()->clearWeaponBonusCondition(bonus)`.
    ClearWeaponBonusCondition(i32),
    /// `getDrawable()->setTintStatus(tint)` (`on`) or `clearTintStatus(tint)`
    /// (`!on`); only made when the object has a drawable.
    DrawableTint { status: i32, on: bool },
}

/// `ObjectHelper::sleepUntil(when)`: the delay to give the module's
/// `setWakeFrame`, or `None` for a destroyed object (nothing is done).
///
/// `NEVER` and `FOREVER` sleep forever (`setWakeFrame(NEVER)` would otherwise
/// awaken at once, `NEVER` being 0).
pub fn sleep_until(destroyed: bool, now: u32, when: u32) -> Option<UpdateSleepTime> {
    if destroyed {
        return None;
    }
    if when == NEVER || when == FOREVER {
        Some(UPDATE_SLEEP_FOREVER)
    } else {
        Some(update_sleep(when.wrapping_sub(now)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_until_never_and_forever_sleep_forever() {
        assert_eq!(sleep_until(false, 100, NEVER), Some(UPDATE_SLEEP_FOREVER));
        assert_eq!(sleep_until(false, 100, FOREVER), Some(UPDATE_SLEEP_FOREVER));
    }

    #[test]
    fn sleep_until_is_a_delay_from_now_and_nothing_for_destroyed() {
        assert_eq!(sleep_until(false, 100, 130), Some(update_sleep(30)));
        assert_eq!(sleep_until(false, 100, 100), Some(update_sleep(0)));
        assert_eq!(sleep_until(true, 100, 130), None);
    }
}
