// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Handicap.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Handicap.cpp
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

//! `Handicap`: the per-player multipliers that balance the game (build cost
//! and build time, each for generic things and for buildings). Each value
//! starts at 1.0 and is overwritten by the `HANDICAP_*` keys of a dictionary
//! (the player's or the side's properties).
//!
//! # Cut point
//!
//! Upstream picks the row by `tmpl->isKindOf(KINDOF_STRUCTURE)`, which needs
//! the thing template (layer 5). [`Handicap::get_handicap`] takes that one
//! fact as `is_structure`, and the caller evaluates it.

use super::dict::Dict;
use super::name_key_generator::NameKeyGenerator;

/// `Handicap::HANDICAP_TYPE_COUNT`.
pub const HANDICAP_TYPE_COUNT: usize = 2;
/// `Handicap::THING_TYPE_COUNT`.
pub const THING_TYPE_COUNT: usize = 2;

/// `enum Handicap::HandicapType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum HandicapType {
    BuildCost = 0,
    BuildTime = 1,
}

/// `enum Handicap::ThingType` (private upstream).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
enum ThingType {
    /// A thing that is nothing else.
    Generic = 0,
    Buildings = 1,
}

/// The names of `readFromDict`, in `HandicapType` and `ThingType` order.
const HT_NAMES: [&str; HANDICAP_TYPE_COUNT] = ["BUILDCOST", "BUILDTIME"];
const TT_NAMES: [&str; THING_TYPE_COUNT] = ["GENERIC", "BUILDINGS"];

/// `class Handicap`.
#[derive(Clone, Debug, PartialEq)]
pub struct Handicap {
    /// `m_handicaps[HANDICAP_TYPE_COUNT][THING_TYPE_COUNT]`.
    handicaps: [[f32; THING_TYPE_COUNT]; HANDICAP_TYPE_COUNT],
}

impl Default for Handicap {
    fn default() -> Self {
        Handicap::new()
    }
}

impl Handicap {
    /// `Handicap()`: every multiplier at 1.0.
    pub fn new() -> Self {
        Handicap {
            handicaps: [[1.0f32; THING_TYPE_COUNT]; HANDICAP_TYPE_COUNT],
        }
    }

    /// `init()`: every multiplier back to 1.0.
    pub fn init(&mut self) {
        self.handicaps = [[1.0f32; THING_TYPE_COUNT]; HANDICAP_TYPE_COUNT];
    }

    /// `readFromDict(d)`: sets each multiplier whose `HANDICAP_<TYPE>_<KIND>`
    /// key the dictionary holds. Does not reset the others (upstream says
    /// so: call [`Handicap::init`] first when that matters).
    ///
    /// The keys are interned in upstream's order (type outer, kind inner),
    /// which fixes the key values the name table assigns.
    pub fn read_from_dict(&mut self, keys: &mut NameKeyGenerator, d: &Dict) {
        for (i, ht) in HT_NAMES.iter().enumerate() {
            for (j, tt) in TT_NAMES.iter().enumerate() {
                let name = format!("HANDICAP_{ht}_{tt}");
                let key = keys.name_to_key(name.as_bytes());
                let (r, exists) = d.get_real(key);
                if exists {
                    self.handicaps[i][j] = r;
                }
            }
        }
    }

    /// `getHandicap(t, tmpl)`: the multiplier for `t` on a thing that is a
    /// structure (`is_structure`) or not.
    pub fn get_handicap(&self, ht: HandicapType, is_structure: bool) -> f32 {
        let tt = if is_structure {
            ThingType::Buildings
        } else {
            ThingType::Generic
        };
        self.handicaps[ht as usize][tt as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_multiplier_starts_at_one_and_init_restores_it() {
        let mut h = Handicap::default();
        for t in [HandicapType::BuildCost, HandicapType::BuildTime] {
            assert_eq!(h.get_handicap(t, false), 1.0);
            assert_eq!(h.get_handicap(t, true), 1.0);
        }
        h.handicaps[0][1] = 0.25;
        assert_eq!(h.get_handicap(HandicapType::BuildCost, true), 0.25);
        h.init();
        assert_eq!(h.get_handicap(HandicapType::BuildCost, true), 1.0);
    }

    #[test]
    fn read_from_dict_keeps_the_entries_it_does_not_find() {
        let mut keys = NameKeyGenerator::new();
        let mut dict = Dict::new();
        dict.set_real(keys.name_to_key(b"HANDICAP_BUILDTIME_GENERIC"), 2.0)
            .expect("set");
        let mut h = Handicap::new();
        h.read_from_dict(&mut keys, &dict);
        assert_eq!(h.get_handicap(HandicapType::BuildTime, false), 2.0);
        assert_eq!(h.get_handicap(HandicapType::BuildTime, true), 1.0);
        assert_eq!(h.get_handicap(HandicapType::BuildCost, false), 1.0);
    }
}
