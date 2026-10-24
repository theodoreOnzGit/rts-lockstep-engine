// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ScoreKeeper.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ScoreKeeper.cpp
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

//! `ScoreKeeper`: one player's tally of the match, the totals the score is
//! computed from and the counts per kind of thing that the battle honours and
//! the end-of-game screens read.
//!
//! # Keys
//!
//! Upstream keys its per-kind maps by the `ThingTemplate` pointer
//! (`std::map<const ThingTemplate*, Int>`), so the iteration order of a map is
//! the allocation order of the templates. That order is not a property of the
//! game (it differs between runs), and nothing in the game reads it: the
//! queries sum or count over a map, and the save writes entries in that order.
//! Here a map is a [`BTreeMap`] keyed by the template's **name**, which is
//! unique (the template store is keyed by name, case-insensitively) and is
//! what the save already writes. Save bytes are therefore in name order; a
//! reader of the file does not depend on the order.
//!
//! # Cut points
//!
//! - **`TheGameLogic->isScoringEnabled()`** is the `scoring` argument of each
//!   `add_*` and `remove_*`: upstream returns before touching anything when it
//!   is off.
//! - **`Object`** arguments are a [`ScoredObject`]: the template's name and
//!   kind-of mask, the controlling player's index and whether the object is
//!   under construction. Nothing else of the object is read.
//! - **`ThingTemplate` queries** (`isKindOfMulti` in the unit totals,
//!   `isEquivalentTo` in the object count) take a closure from the template
//!   store: the name to the template's kind-of mask, or to "equivalent".
//!   `ThingFactory::findTemplate`, asked at load, is the closure `known` of
//!   [`ScoreKeeper::xfer`].
//!
//! # Integer semantics
//!
//! Every counter is an `Int`, and upstream is compiled with signed overflow
//! defined as wrapping (`-fwrapv`), so every `+`, `*` and `++` here wraps.
//!
//! # Deliberate differences
//!
//! - A destroyed-object event whose controller is outside `0..MAX_PLAYER_COUNT`
//!   records nothing. Upstream indexes `m_totalUnitsDestroyed[playerIdx]` out of
//!   bounds (undefined behaviour); no controller in the game has such an index.
//! - `crc` is empty upstream and is not translated.

use std::collections::BTreeMap;

use super::ids::PlayerIndex;
use crate::common::game_common::MAX_PLAYER_COUNT;
use crate::common::kind_of::{
    KindOfMaskType, KINDOF_INFANTRY, KINDOF_SCORE, KINDOF_SCORE_CREATE, KINDOF_SCORE_DESTROY,
    KINDOF_STRUCTURE, KINDOF_VEHICLE, KINDOFMASK_NONE,
};
use crate::common::xfer::{Xfer, XferResult, XferStatus, XferVersion};

/// `ObjectCountMap`: how many of each template (by name).
pub type ObjectCountMap = BTreeMap<Vec<u8>, i32>;

/// The part of an object the score keeper reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoredObject {
    /// `getTemplate()->getName()`.
    pub template_name: Vec<u8>,
    /// The template's kind-of mask (`isKindOf`, `isKindOfMulti`).
    pub kind_of: KindOfMaskType,
    /// `getControllingPlayer()->getPlayerIndex()`.
    pub controller: PlayerIndex,
    /// `testStatus(OBJECT_STATUS_UNDER_CONSTRUCTION)`.
    pub under_construction: bool,
}

/// `scoringBuildingMask`: a structure that scores (`KINDOF_STRUCTURE`, `KINDOF_SCORE`).
fn scoring_building_mask() -> KindOfMaskType {
    KindOfMaskType::from_indices(&[KINDOF_STRUCTURE, KINDOF_SCORE])
}

/// `scoringBuildingCreateMask`: a structure that scores when it is created.
fn scoring_building_create_mask() -> KindOfMaskType {
    KindOfMaskType::from_indices(&[KINDOF_STRUCTURE, KINDOF_SCORE_CREATE])
}

/// `scoringBuildingDestroyMask`: a structure that scores when it is destroyed.
fn scoring_building_destroy_mask() -> KindOfMaskType {
    KindOfMaskType::from_indices(&[KINDOF_STRUCTURE, KINDOF_SCORE_DESTROY])
}

/// The index of a per-player array for a controller, if it is one.
fn player_slot(controller: PlayerIndex) -> Option<usize> {
    if (0..MAX_PLAYER_COUNT).contains(&controller) {
        Some(controller as usize)
    } else {
        None
    }
}

/// `count + 1` on a map entry, creating it at 0 (`m_objectsX[t] = existing + 1`).
fn bump(map: &mut ObjectCountMap, name: &[u8], delta: i32) {
    let entry = map.entry(name.to_vec()).or_insert(0);
    *entry = entry.wrapping_add(delta);
}

/// `class ScoreKeeper`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreKeeper {
    /// `m_totalMoneyEarned`: harvested, refined, received in crates.
    total_money_earned: i32,
    /// `m_totalMoneySpent`: spent on units, buildings and repairs.
    total_money_spent: i32,
    /// `m_totalUnitsDestroyed[MAX_PLAYER_COUNT]`: enemy units killed, by owner.
    total_units_destroyed: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_totalUnitsBuilt`: units built from a building.
    total_units_built: i32,
    /// `m_totalUnitsLost`: our units lost.
    total_units_lost: i32,
    /// `m_totalBuildingsDestroyed[MAX_PLAYER_COUNT]`, by owner.
    total_buildings_destroyed: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_totalBuildingsBuilt`.
    total_buildings_built: i32,
    /// `m_totalBuildingsLost`.
    total_buildings_lost: i32,
    /// `m_totalTechBuildingsCaptured`.
    total_tech_buildings_captured: i32,
    /// `m_totalFactionBuildingsCaptured`.
    total_faction_buildings_captured: i32,
    /// `m_currentScore`: the last [`ScoreKeeper::calculate_score`].
    current_score: i32,
    /// `m_myPlayerIdx`: our own kills do not score.
    my_player_index: i32,
    /// `m_objectsBuilt`.
    objects_built: ObjectCountMap,
    /// `m_objectsDestroyed[MAX_PLAYER_COUNT]`, by owner.
    objects_destroyed: [ObjectCountMap; MAX_PLAYER_COUNT as usize],
    /// `m_objectsLost`.
    objects_lost: ObjectCountMap,
    /// `m_objectsCaptured`.
    objects_captured: ObjectCountMap,
}

impl Default for ScoreKeeper {
    /// `ScoreKeeper()`: `reset(0)`.
    fn default() -> Self {
        ScoreKeeper::new()
    }
}

impl ScoreKeeper {
    /// Every total zero, every map empty, player 0 (`reset` before its body).
    fn blank() -> ScoreKeeper {
        ScoreKeeper {
            total_money_earned: 0,
            total_money_spent: 0,
            total_units_destroyed: [0; MAX_PLAYER_COUNT as usize],
            total_units_built: 0,
            total_units_lost: 0,
            total_buildings_destroyed: [0; MAX_PLAYER_COUNT as usize],
            total_buildings_built: 0,
            total_buildings_lost: 0,
            total_tech_buildings_captured: 0,
            total_faction_buildings_captured: 0,
            current_score: 0,
            my_player_index: 0,
            objects_built: ObjectCountMap::new(),
            objects_destroyed: std::array::from_fn(|_| ObjectCountMap::new()),
            objects_lost: ObjectCountMap::new(),
            objects_captured: ObjectCountMap::new(),
        }
    }

    /// `ScoreKeeper::ScoreKeeper()`: reset for player 0.
    pub fn new() -> ScoreKeeper {
        ScoreKeeper::blank()
    }

    /// `reset(playerIdx)`: zero every total and clear every map.
    pub fn reset(&mut self, player_idx: i32) {
        *self = ScoreKeeper::blank();
        self.my_player_index = player_idx;
    }

    /// `addMoneySpent(money)`.
    pub fn add_money_spent(&mut self, money: i32) {
        self.total_money_spent = self.total_money_spent.wrapping_add(money);
    }

    /// `addMoneyEarned(money)`.
    pub fn add_money_earned(&mut self, money: i32) {
        self.total_money_earned = self.total_money_earned.wrapping_add(money);
    }

    /// `getTotalMoneyEarned()`.
    pub fn total_money_earned(&self) -> i32 {
        self.total_money_earned
    }

    /// `getTotalMoneySpent()`.
    pub fn total_money_spent(&self) -> i32 {
        self.total_money_spent
    }

    /// `getTotalUnitsDestroyed()`: over every owner, ours included.
    pub fn total_units_destroyed(&self) -> i32 {
        self.total_units_destroyed.iter().fold(0i32, |c, &n| c.wrapping_add(n))
    }

    /// `getTotalUnitsBuilt()`.
    pub fn total_units_built(&self) -> i32 {
        self.total_units_built
    }

    /// `getTotalUnitsLost()`.
    pub fn total_units_lost(&self) -> i32 {
        self.total_units_lost
    }

    /// `getTotalBuildingsDestroyed()`: over every owner, ours included.
    pub fn total_buildings_destroyed(&self) -> i32 {
        self.total_buildings_destroyed.iter().fold(0i32, |c, &n| c.wrapping_add(n))
    }

    /// `getTotalBuildingsBuilt()`.
    pub fn total_buildings_built(&self) -> i32 {
        self.total_buildings_built
    }

    /// `getTotalBuildingsLost()`.
    pub fn total_buildings_lost(&self) -> i32 {
        self.total_buildings_lost
    }

    /// `getTotalTechBuildingsCaptured()`.
    pub fn total_tech_buildings_captured(&self) -> i32 {
        self.total_tech_buildings_captured
    }

    /// `getTotalFactionBuildingsCaptured()`.
    pub fn total_faction_buildings_captured(&self) -> i32 {
        self.total_faction_buildings_captured
    }

    /// `addObjectBuilt(o)`: a scoring structure, or a scoring infantry or
    /// vehicle, is counted and its template's count goes up.
    pub fn add_object_built(&mut self, o: &ScoredObject, scoring: bool) {
        if !scoring {
            return;
        }
        let mut add_to_count = false;
        let k = &o.kind_of;
        // a scoring structure, or a scoring structure created (the two masks act alike)
        if k.test_set_and_clear(&scoring_building_mask(), &KINDOFMASK_NONE)
            || k.test_set_and_clear(&scoring_building_create_mask(), &KINDOFMASK_NONE)
        {
            self.total_buildings_built = self.total_buildings_built.wrapping_add(1);
            add_to_count = true;
        } else if (k.test(KINDOF_INFANTRY) || k.test(KINDOF_VEHICLE)) && (k.test(KINDOF_SCORE) || k.test(KINDOF_SCORE_CREATE)) {
            self.total_units_built = self.total_units_built.wrapping_add(1);
            add_to_count = true;
        }
        if add_to_count {
            bump(&mut self.objects_built, &o.template_name, 1);
        }
    }

    /// `removeObjectBuilt(o)`: the inverse of [`ScoreKeeper::add_object_built`].
    /// Entries stay in the map even at zero or below, as upstream keeps them.
    pub fn remove_object_built(&mut self, o: &ScoredObject, scoring: bool) {
        if !scoring {
            return;
        }
        let mut remove_from_count = false;
        let k = &o.kind_of;
        if k.test_set_and_clear(&scoring_building_mask(), &KINDOFMASK_NONE)
            || k.test_set_and_clear(&scoring_building_create_mask(), &KINDOFMASK_NONE)
        {
            self.total_buildings_built = self.total_buildings_built.wrapping_sub(1);
            remove_from_count = true;
        } else if (k.test(KINDOF_INFANTRY) || k.test(KINDOF_VEHICLE)) && (k.test(KINDOF_SCORE) || k.test(KINDOF_SCORE_CREATE)) {
            self.total_units_built = self.total_units_built.wrapping_sub(1);
            remove_from_count = true;
        }
        if remove_from_count {
            bump(&mut self.objects_built, &o.template_name, -1);
        }
    }

    /// `getTotalUnitsBuilt(validMask, invalidMask)`: the sum of the counts of
    /// the templates whose kind-of mask passes `validMask`/`invalidMask`
    /// (`kind_of_of` gives a template's mask by name; `None` is no template).
    pub fn get_total_units_built(
        &self,
        valid: &KindOfMaskType,
        invalid: &KindOfMaskType,
        mut kind_of_of: impl FnMut(&[u8]) -> Option<KindOfMaskType>,
    ) -> i32 {
        let mut count = 0i32;
        for (name, &num_built) in &self.objects_built {
            if let Some(k) = kind_of_of(name) {
                if k.test_set_and_clear(valid, invalid) {
                    count = count.wrapping_add(num_built);
                }
            }
        }
        count
    }

    /// `getTotalObjectsBuilt(pTemplate)`: how many **entries** of the built map
    /// are equivalent to the template (`is_equivalent(name)` answers
    /// `ThingTemplate::isEquivalentTo`). Upstream counts entries, not the
    /// counts they hold; so does this.
    pub fn get_total_objects_built(&self, mut is_equivalent: impl FnMut(&[u8]) -> bool) -> i32 {
        let mut count = 0i32;
        for name in self.objects_built.keys() {
            if is_equivalent(name) {
                count = count.wrapping_add(1);
            }
        }
        count
    }

    /// `addObjectCaptured(o)`: a structure captured. A scoring structure is a
    /// faction building, any other a tech building.
    pub fn add_object_captured(&mut self, o: &ScoredObject, scoring: bool) {
        if !scoring {
            return;
        }
        let mut add_to_count = false;
        if o.kind_of.test(KINDOF_STRUCTURE) {
            if o.kind_of.test(KINDOF_SCORE) {
                self.total_faction_buildings_captured = self.total_faction_buildings_captured.wrapping_add(1);
            } else {
                self.total_tech_buildings_captured = self.total_tech_buildings_captured.wrapping_add(1);
            }
            add_to_count = true;
        }
        if add_to_count {
            bump(&mut self.objects_captured, &o.template_name, 1);
        }
    }

    /// `addObjectDestroyed(o)`: a finished scoring structure, or a finished
    /// scoring infantry or vehicle, killed by its controller. The controller's
    /// slot is recorded (see the module's deliberate differences).
    pub fn add_object_destroyed(&mut self, o: &ScoredObject, scoring: bool) {
        if !scoring {
            return;
        }
        let Some(slot) = player_slot(o.controller) else {
            return;
        };
        let mut add_to_count = false;
        let k = &o.kind_of;
        if k.test_set_and_clear(&scoring_building_mask(), &KINDOFMASK_NONE)
            || k.test_set_and_clear(&scoring_building_destroy_mask(), &KINDOFMASK_NONE)
        {
            if !o.under_construction {
                self.total_buildings_destroyed[slot] = self.total_buildings_destroyed[slot].wrapping_add(1);
                add_to_count = true;
            }
        } else if (k.test(KINDOF_INFANTRY) || k.test(KINDOF_VEHICLE))
            && (k.test(KINDOF_SCORE) || k.test(KINDOF_SCORE_DESTROY))
            && !o.under_construction
        {
            self.total_units_destroyed[slot] = self.total_units_destroyed[slot].wrapping_add(1);
            add_to_count = true;
        }
        if add_to_count {
            bump(&mut self.objects_destroyed[slot], &o.template_name, 1);
        }
    }

    /// `addObjectLost(o)`: one of ours was destroyed. Same kinds as
    /// [`ScoreKeeper::add_object_destroyed`], without the owner's slot.
    pub fn add_object_lost(&mut self, o: &ScoredObject, scoring: bool) {
        if !scoring {
            return;
        }
        let mut add_to_count = false;
        let k = &o.kind_of;
        if k.test_set_and_clear(&scoring_building_mask(), &KINDOFMASK_NONE)
            || k.test_set_and_clear(&scoring_building_destroy_mask(), &KINDOFMASK_NONE)
        {
            if !o.under_construction {
                self.total_buildings_lost = self.total_buildings_lost.wrapping_add(1);
                add_to_count = true;
            }
        } else if (k.test(KINDOF_INFANTRY) || k.test(KINDOF_VEHICLE))
            && (k.test(KINDOF_SCORE) || k.test(KINDOF_SCORE_DESTROY))
            && !o.under_construction
        {
            self.total_units_lost = self.total_units_lost.wrapping_add(1);
            add_to_count = true;
        }
        if add_to_count {
            bump(&mut self.objects_lost, &o.template_name, 1);
        }
    }

    /// `calculateScore()`: 100 per unit built, 100 per building built, the
    /// money earned, and 100 per unit and building destroyed by anyone but us.
    /// Stored as `m_currentScore` and returned.
    pub fn calculate_score(&mut self) -> i32 {
        let mut score = 0i32;
        score = score.wrapping_add(self.total_units_built.wrapping_mul(100));
        score = score.wrapping_add(self.total_money_earned);
        score = score.wrapping_add(self.total_buildings_built.wrapping_mul(100));
        for i in 0..MAX_PLAYER_COUNT as usize {
            if i as i32 == self.my_player_index {
                continue;
            }
            score = score.wrapping_add(self.total_units_destroyed[i].wrapping_mul(100));
            score = score.wrapping_add(self.total_buildings_destroyed[i].wrapping_mul(100));
        }
        self.current_score = score;
        self.current_score
    }

    /// `m_currentScore`: the last computed score.
    pub fn current_score(&self) -> i32 {
        self.current_score
    }

    /// `xferObjectCountMap(xfer, map)`, version 1: the entry count (a
    /// `UnsignedShort`), then per entry the template name and its count. On
    /// load an entry is set (`(*map)[t] = count`, as upstream), and a name
    /// `known` does not name is `SC_INVALID_DATA`.
    fn xfer_object_count_map<X: Xfer + ?Sized>(
        xfer: &mut X,
        map: &mut ObjectCountMap,
        known: &mut impl FnMut(&[u8]) -> bool,
    ) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut map_size = map.len() as u16;
        xfer.xfer_unsigned_short(&mut map_size)?;
        if xfer.get_xfer_mode() == crate::common::xfer::XferMode::Save {
            for (name, count) in map.iter() {
                let mut name = name.clone();
                xfer.xfer_ascii_string(&mut name)?;
                let mut count = *count;
                xfer.xfer_int(&mut count)?;
            }
        } else {
            for _ in 0..map_size {
                let mut name = Vec::new();
                xfer.xfer_ascii_string(&mut name)?;
                if !known(&name) {
                    return Err(XferStatus::ErrorUnknown);
                }
                let mut count = 0i32;
                xfer.xfer_int(&mut count)?;
                map.insert(name, count);
            }
        }
        Ok(())
    }

    /// `ScoreKeeper::xfer`, version 1. `known(name)` is `TheThingFactory->
    /// findTemplate(name) != NULL`, asked for every name read on load.
    ///
    /// The per-owner arrays go out as 16 native-endian `Int`s (`xferUser`),
    /// and the per-owner maps as a count of 16 followed by 16 maps.
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X, known: &mut impl FnMut(&[u8]) -> bool) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_int(&mut self.total_money_earned)?;
        xfer.xfer_int(&mut self.total_money_spent)?;
        xfer_int_array(xfer, &mut self.total_units_destroyed)?;
        xfer.xfer_int(&mut self.total_units_built)?;
        xfer.xfer_int(&mut self.total_units_lost)?;
        xfer_int_array(xfer, &mut self.total_buildings_destroyed)?;
        xfer.xfer_int(&mut self.total_buildings_built)?;
        xfer.xfer_int(&mut self.total_buildings_lost)?;
        xfer.xfer_int(&mut self.total_tech_buildings_captured)?;
        xfer.xfer_int(&mut self.total_faction_buildings_captured)?;
        xfer.xfer_int(&mut self.current_score)?;
        xfer.xfer_int(&mut self.my_player_index)?;
        Self::xfer_object_count_map(xfer, &mut self.objects_built, known)?;
        let mut destroyed_array_size = MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut destroyed_array_size)?;
        if destroyed_array_size != MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::ErrorUnknown);
        }
        for map in self.objects_destroyed.iter_mut() {
            Self::xfer_object_count_map(xfer, map, known)?;
        }
        Self::xfer_object_count_map(xfer, &mut self.objects_lost, known)?;
        Self::xfer_object_count_map(xfer, &mut self.objects_captured, known)
    }
}

/// `xferUser(array, sizeof(Int) * MAX_PLAYER_COUNT)`: the raw bytes of the
/// array, native (little-endian) `Int`s.
fn xfer_int_array<X: Xfer + ?Sized>(xfer: &mut X, values: &mut [i32; MAX_PLAYER_COUNT as usize]) -> XferResult<()> {
    let mut bytes = [0u8; 4 * MAX_PLAYER_COUNT as usize];
    for (i, v) in values.iter().enumerate() {
        bytes[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    xfer.xfer_user(&mut bytes)?;
    for (i, v) in values.iter_mut().enumerate() {
        *v = i32::from_le_bytes([bytes[4 * i], bytes[4 * i + 1], bytes[4 * i + 2], bytes[4 * i + 3]]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(name: &str, kinds: &[i32], controller: PlayerIndex, uc: bool) -> ScoredObject {
        ScoredObject {
            template_name: name.as_bytes().to_vec(),
            kind_of: KindOfMaskType::from_indices(kinds),
            controller,
            under_construction: uc,
        }
    }

    #[test]
    fn a_built_unit_scores_and_counts_under_its_name() {
        let mut s = ScoreKeeper::new();
        let tank = object("Tank", &[KINDOF_VEHICLE, KINDOF_SCORE], 1, false);
        s.add_object_built(&tank, true);
        s.add_object_built(&tank, true);
        assert_eq!(s.total_units_built(), 2);
        assert_eq!(s.calculate_score(), 200);
        assert_eq!(s.get_total_objects_built(|n| n == b"Tank"), 1, "entries, not counts");
        assert_eq!(s.get_total_units_built(&KindOfMaskType::from_indices(&[KINDOF_VEHICLE]), &KINDOFMASK_NONE, |_| Some(tank.kind_of)), 2);
        s.remove_object_built(&tank, true);
        assert_eq!(s.total_units_built(), 1);
    }

    #[test]
    fn our_own_kills_do_not_score_but_are_totalled() {
        let mut s = ScoreKeeper::new();
        s.reset(2);
        let enemy = object("Ranger", &[KINDOF_INFANTRY, KINDOF_SCORE], 5, false);
        let own = object("Ranger", &[KINDOF_INFANTRY, KINDOF_SCORE], 2, false);
        s.add_object_destroyed(&enemy, true);
        s.add_object_destroyed(&own, true);
        assert_eq!(s.total_units_destroyed(), 2);
        assert_eq!(s.calculate_score(), 100);
    }

    #[test]
    fn scoring_off_changes_nothing() {
        let mut s = ScoreKeeper::new();
        s.add_object_built(&object("X", &[KINDOF_STRUCTURE, KINDOF_SCORE], 0, false), false);
        s.add_money_earned(5);
        assert_eq!(s.total_buildings_built(), 0);
        assert_eq!(s.total_money_earned(), 5, "the money totals are not gated");
    }

    #[test]
    fn a_capture_is_faction_or_tech_by_the_score_kind() {
        let mut s = ScoreKeeper::new();
        s.add_object_captured(&object("Barracks", &[KINDOF_STRUCTURE, KINDOF_SCORE], 0, false), true);
        s.add_object_captured(&object("Radar", &[KINDOF_STRUCTURE], 0, false), true);
        assert_eq!((s.total_faction_buildings_captured(), s.total_tech_buildings_captured()), (1, 1));
    }

    #[test]
    fn a_scoring_building_under_construction_is_not_destroyed_or_lost() {
        let mut s = ScoreKeeper::new();
        let site = object("Dozer", &[KINDOF_STRUCTURE, KINDOF_SCORE_DESTROY], 1, true);
        s.add_object_destroyed(&site, true);
        s.add_object_lost(&site, true);
        assert_eq!((s.total_buildings_destroyed(), s.total_buildings_lost()), (0, 0));
        let built = object("Dozer", &[KINDOF_STRUCTURE, KINDOF_SCORE_DESTROY], 1, false);
        s.add_object_destroyed(&built, true);
        assert_eq!(s.total_buildings_destroyed(), 1);
    }

    #[test]
    fn a_map_entry_stays_when_its_count_returns_to_zero() {
        let mut s = ScoreKeeper::new();
        let tank = object("Tank", &[KINDOF_VEHICLE, KINDOF_SCORE], 0, false);
        s.add_object_built(&tank, true);
        s.remove_object_built(&tank, true);
        assert_eq!(s.get_total_objects_built(|_| true), 1);
    }
}
