// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Energy.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Westwood Studios Pacific, Steven Johnson, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Energy`: a player's power production and consumption, in kilowatts. A
//! player owns one; objects join and leave it as they are built, sold,
//! destroyed or sabotaged, and it says whether the player is in brown-out.
//!
//! # Cut points
//!
//! - **`TheGameLogic->getFrame()`** is a parameter (`frame`) of every method
//!   that reads the sabotage window.
//! - **`m_owner->onPowerBrownOutChange(!hasSufficientPower())`** (the owner's
//!   notification, sent by every method that changes a sum, when there is an
//!   owner): the methods return it as an [`OwnerNotice`] for the caller to
//!   deliver to the owning [`super::player`] (the player turns the disabled
//!   states and the UI over; that is layer 7's `Player`).
//! - **`Object`** arguments become the template's values, which the caller
//!   reads (`ThingTemplate::get_energy_production` / `get_energy_bonus`): the
//!   energy code needs no object, only the two numbers. Upstream's `obj == NULL`
//!   early return is the caller's `Option`.
//! - **The owner pointer** is the owning player's index (`Energy::init`'s
//!   `Player*`); the xfer saves the index and reads it back as the player
//!   that index names (`ThePlayerList->getNthPlayer`, `None` outside
//!   `0..MAX_PLAYER_COUNT`).
//!
//! # Integer semantics
//!
//! The sums are `Int`s. Upstream is compiled with signed overflow defined as
//! wrapping (`-fwrapv`, the MSVC x86 behaviour the game shipped with), so every
//! `+=` and every negation here wraps.
//!
//! # Deliberate differences
//!
//! - `xfer` on a save with no owner: upstream dereferences `NULL`
//!   (`m_owner->getPlayerIndex()`); here the index `-1` is written, which the
//!   load reads back as "no owner".
//! - `crc` is empty upstream and is not translated (nothing calls it).

use super::ids::PlayerIndex;
use crate::common::game_common::MAX_PLAYER_COUNT;
use crate::common::xfer::{Xfer, XferResult, XferVersion};

/// The notification upstream sends the owner: `onPowerBrownOutChange(brown_out)`
/// where `brown_out` is `!hasSufficientPower()` after the change. A repeated
/// signal is harmless upstream (the player re-checks), so one is returned for
/// every change that has an owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrownOutChange {
    /// The new value of `!hasSufficientPower()`.
    pub brown_out: bool,
}

/// What a method returns for its owner: `None` when `m_owner` is `NULL`.
pub type OwnerNotice = Option<BrownOutChange>;

/// `class Energy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Energy {
    /// `m_energyProduction`: level of energy production, in kw.
    energy_production: i32,
    /// `m_energyConsumption`: level of energy consumption, in kw.
    energy_consumption: i32,
    /// `m_powerSabotagedTillFrame`: while the frame is below this, no power.
    power_sabotaged_till_frame: u32,
    /// `m_owner`: the player this energy belongs to (`None` = `NULL`).
    owner: Option<PlayerIndex>,
}

impl Default for Energy {
    /// `Energy::Energy()`: no production, no consumption, no owner, no sabotage.
    fn default() -> Self {
        Energy::new()
    }
}

impl Energy {
    /// `Energy::Energy()`.
    pub fn new() -> Energy {
        Energy {
            energy_production: 0,
            energy_consumption: 0,
            power_sabotaged_till_frame: 0,
            owner: None,
        }
    }

    /// `init(owner)`: resets the sums and sets the owner (the sabotage window
    /// is kept, as upstream keeps it).
    pub fn init(&mut self, owner: Option<PlayerIndex>) {
        self.energy_production = 0;
        self.energy_consumption = 0;
        self.owner = owner;
    }

    /// The owning player's index, if any.
    pub fn owner(&self) -> Option<PlayerIndex> {
        self.owner
    }

    /// `getProduction()`: the production, or 0 while power is sabotaged.
    pub fn production(&self, frame: u32) -> i32 {
        if frame < self.power_sabotaged_till_frame {
            return 0;
        }
        self.energy_production
    }

    /// `getConsumption()`.
    pub fn consumption(&self) -> i32 {
        self.energy_consumption
    }

    /// `hasSufficientPower()`: production >= consumption, and not sabotaged.
    pub fn has_sufficient_power(&self, frame: u32) -> bool {
        if frame < self.power_sabotaged_till_frame {
            return false;
        }
        self.energy_production >= self.energy_consumption
    }

    /// `getEnergySupplyRatio()`: the fraction of the consumption that is
    /// produced, as `Real`. With no consumption it is the production itself
    /// (upstream's behaviour, kept). Zero while sabotaged.
    pub fn energy_supply_ratio(&self, frame: u32) -> f32 {
        if frame < self.power_sabotaged_till_frame {
            return 0.0;
        }
        if self.energy_consumption == 0 {
            return self.energy_production as f32;
        }
        self.energy_production as f32 / self.energy_consumption as f32
    }

    /// `setPowerSabotagedTillFrame(frame)`.
    pub fn set_power_sabotaged_till_frame(&mut self, frame: u32) {
        self.power_sabotaged_till_frame = frame;
    }

    /// `getPowerSabotagedTillFrame()`.
    pub fn power_sabotaged_till_frame(&self) -> u32 {
        self.power_sabotaged_till_frame
    }

    /// `adjustPower(powerDelta, adding)`. `adding == false` means the delta is
    /// being removed. A positive delta is production; a negative one is
    /// consumption, reversed (negative power is positive consumption).
    pub fn adjust_power(&mut self, frame: u32, power_delta: i32, adding: bool) -> OwnerNotice {
        if power_delta == 0 {
            return None;
        }
        if power_delta > 0 {
            if adding {
                self.add_production(frame, power_delta)
            } else {
                self.add_production(frame, power_delta.wrapping_neg())
            }
        } else if adding {
            self.add_consumption(frame, power_delta.wrapping_neg())
        } else {
            self.add_consumption(frame, power_delta)
        }
    }

    /// `objectEnteringInfluence(obj)`: an object of a template with this
    /// `EnergyProduction` (negative = consumes) joins the sums.
    pub fn object_entering_influence(&mut self, frame: u32, energy_production: i32) -> OwnerNotice {
        if energy_production < 0 {
            self.add_consumption(frame, energy_production.wrapping_neg())
        } else if energy_production > 0 {
            self.add_production(frame, energy_production)
        } else {
            None
        }
    }

    /// `objectLeavingInfluence(obj)`: the inverse of
    /// [`Energy::object_entering_influence`].
    pub fn object_leaving_influence(&mut self, frame: u32, energy_production: i32) -> OwnerNotice {
        if energy_production < 0 {
            self.add_consumption(frame, energy_production)
        } else if energy_production > 0 {
            self.add_production(frame, energy_production.wrapping_neg())
        } else {
            None
        }
    }

    /// `addPowerBonus(obj)`: the template's `EnergyBonus` joins the production
    /// (the "Control Rods" upgrade of the cold-fusion plant).
    pub fn add_power_bonus(&mut self, frame: u32, energy_bonus: i32) -> OwnerNotice {
        self.add_production(frame, energy_bonus)
    }

    /// `removePowerBonus(obj)`.
    pub fn remove_power_bonus(&mut self, frame: u32, energy_bonus: i32) -> OwnerNotice {
        self.add_production(frame, energy_bonus.wrapping_neg())
    }

    /// `addProduction(amt)`.
    fn add_production(&mut self, frame: u32, amt: i32) -> OwnerNotice {
        self.energy_production = self.energy_production.wrapping_add(amt);
        self.notify(frame)
    }

    /// `addConsumption(amt)`.
    fn add_consumption(&mut self, frame: u32, amt: i32) -> OwnerNotice {
        self.energy_consumption = self.energy_consumption.wrapping_add(amt);
        self.notify(frame)
    }

    /// The owner's notification after a change: none without an owner.
    fn notify(&self, frame: u32) -> OwnerNotice {
        self.owner.map(|_| BrownOutChange {
            brown_out: !self.has_sufficient_power(frame),
        })
    }

    /// `Energy::xfer`, version 3. Version 1 and 2 saved the two sums (not
    /// saved since version 2: they are rebuilt from the buildings). The owner
    /// is saved as its player index. Version 3 adds the sabotage frame.
    ///
    /// The owner's index is read back as the player it names; an index
    /// outside `0..MAX_PLAYER_COUNT` is no player (`None`), as
    /// `getNthPlayer` gives `NULL` for it.
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        let current: XferVersion = 3;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        if version < 2 {
            xfer.xfer_int(&mut self.energy_production)?;
            xfer.xfer_int(&mut self.energy_consumption)?;
        }
        let mut owning_player_index: i32 = self.owner.unwrap_or(-1);
        xfer.xfer_int(&mut owning_player_index)?;
        self.owner = if (0..MAX_PLAYER_COUNT).contains(&owning_player_index) {
            Some(owning_player_index)
        } else {
            None
        };
        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.power_sabotaged_till_frame)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_and_consumption_add_and_leave_symmetrically() {
        let mut e = Energy::new();
        assert!(
            e.object_entering_influence(0, 50).is_none(),
            "no owner, no notice"
        );
        e.object_entering_influence(0, -20);
        assert_eq!((e.production(0), e.consumption()), (50, 20));
        assert!(e.has_sufficient_power(0));
        e.object_leaving_influence(0, 50);
        e.object_leaving_influence(0, -20);
        assert_eq!((e.production(0), e.consumption()), (0, 0));
    }

    #[test]
    fn adjust_power_reverses_negative_deltas_as_upstream_does() {
        let mut e = Energy::new();
        e.adjust_power(0, 30, true);
        assert_eq!(e.production(0), 30);
        e.adjust_power(0, -10, true);
        assert_eq!(
            e.consumption(),
            10,
            "a negative delta being added is consumption"
        );
        e.adjust_power(0, -10, false);
        assert_eq!(
            e.consumption(),
            0,
            "a negative delta being removed takes it back"
        );
        e.adjust_power(0, 30, false);
        assert_eq!(e.production(0), 0);
        assert!(e.adjust_power(0, 0, true).is_none());
    }

    #[test]
    fn the_owner_hears_each_change_with_the_brown_out_state_after_it() {
        let mut e = Energy::new();
        e.init(Some(3));
        assert_eq!(
            e.adjust_power(0, -5, true),
            Some(BrownOutChange { brown_out: true })
        );
        assert_eq!(
            e.adjust_power(0, 5, true),
            Some(BrownOutChange { brown_out: false })
        );
        assert_eq!(
            e.add_power_bonus(0, 0),
            Some(BrownOutChange { brown_out: false }),
            "a bonus of 0 still notifies"
        );
    }

    #[test]
    fn sabotage_zeroes_the_production_and_the_ratio_until_its_frame() {
        let mut e = Energy::new();
        e.init(Some(0));
        e.add_power_bonus(0, 100);
        e.object_entering_influence(0, -50);
        e.set_power_sabotaged_till_frame(10);
        assert_eq!(e.production(9), 0);
        assert!(!e.has_sufficient_power(9));
        assert_eq!(e.energy_supply_ratio(9), 0.0);
        assert_eq!(e.production(10), 100);
        assert_eq!(e.energy_supply_ratio(10), 2.0);
    }

    #[test]
    fn supply_ratio_without_consumption_is_the_production() {
        let mut e = Energy::new();
        e.add_power_bonus(0, 7);
        assert_eq!(e.energy_supply_ratio(0), 7.0);
    }

    #[test]
    fn sums_wrap_as_upstream_does() {
        let mut e = Energy::new();
        e.add_power_bonus(0, i32::MAX);
        e.add_power_bonus(0, 1);
        assert_eq!(e.production(0), i32::MIN);
        e.adjust_power(0, i32::MIN, true);
        assert_eq!(
            e.consumption(),
            i32::MIN,
            "-i32::MIN wraps back to i32::MIN"
        );
    }

    #[test]
    fn the_owner_is_saved_by_index_and_read_back_as_that_player() {
        use crate::common::xfer_load::XferLoad;
        use crate::common::xfer_save::XferSave;
        let mut save = XferSave::new();
        save.open(b"energy").expect("open");
        let mut e = Energy::new();
        e.init(Some(2));
        e.add_power_bonus(0, 9);
        e.xfer(&mut save).expect("save");
        save.close().expect("close");
        let mut load = XferLoad::new(save.take_bytes());
        load.open(b"energy").expect("open");
        let mut back = Energy::new();
        back.xfer(&mut load).expect("load");
        assert_eq!(back.owner(), Some(2));
        assert_eq!(
            back.production(0),
            0,
            "the sums are not saved (version 2 on): they are rebuilt from the buildings"
        );
        assert_eq!(back.power_sabotaged_till_frame(), 0);
    }
}
