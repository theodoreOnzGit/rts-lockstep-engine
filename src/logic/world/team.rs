// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Team.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp
//   (`TeamFactory`: the constructor, `reset`, the id counters, `findTeamByID`,
//   `loadPostProcess`'s counter recovery)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The team arena: `TeamFactory`'s two id counters and its two ordered maps.
//! **Teams are not translated yet**: [`Team`] and [`TeamPrototype`] are
//! shells that layer 7 fills in place.
//!
//! # What is kept from upstream
//!
//! - **Ids are pre-incremented counters that start at 0**, so the first team
//!   is 1 and the first prototype is 1 (0 is the invalid id):
//!   [`TeamArena::allocate_team_id`] is `++m_uniqueTeamID`.
//! - **`reset` zeroes both counters** and clears the maps.
//! - **`loadPostProcess` recovers each counter as "highest id in use, plus
//!   one"** ([`TeamArena::recover_counters`]), after which the next
//!   allocation is `highest + 2`: the counter is incremented before use, so
//!   one id is skipped after every load. Kept as shipped; a replay that
//!   crosses a save depends on it.
//! - **Ordered maps.** Upstream's `TeamPrototypeMap` is a `std::map` keyed
//!   by prototype id: ordered, so `BTreeMap`. The teams of a prototype
//!   (`DLINK` list, newest first) are a layer-7 detail of [`TeamPrototype`].
//! - **The relation maps** (`TeamRelationMap`, `PlayerRelationMap`) are
//!   `hash_map`s that only `xfer` iterates; see the determinism section of
//!   `docs/phase1-porting-graph.md`. Layer 7 uses ordered maps.

use std::collections::BTreeMap;

use super::ids::{TeamId, TeamPrototypeId, TEAM_ID_INVALID, TEAM_PROTOTYPE_ID_INVALID};

/// The shell of `Team`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    /// `m_id`.
    pub id: TeamId,
    /// `m_proto`, by id.
    pub prototype: TeamPrototypeId,
}

/// The shell of `TeamPrototype`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPrototype {
    /// `m_id`.
    pub id: TeamPrototypeId,
}

/// `TeamFactory`. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct TeamArena {
    /// `m_uniqueTeamID`.
    unique_team_id: TeamId,
    /// `m_uniqueTeamPrototypeID`.
    unique_prototype_id: TeamPrototypeId,
    /// `m_prototypes` (ordered by id).
    prototypes: BTreeMap<TeamPrototypeId, TeamPrototype>,
    /// Every `Team` instance, by id (upstream reaches them through their
    /// prototypes' instance lists and `findTeamByID`).
    teams: BTreeMap<TeamId, Team>,
}

impl TeamArena {
    /// `TeamFactory::TeamFactory()`: both counters at the invalid id.
    pub fn new() -> TeamArena {
        TeamArena { unique_team_id: TEAM_ID_INVALID, unique_prototype_id: TEAM_PROTOTYPE_ID_INVALID, ..TeamArena::default() }
    }

    /// `++m_uniqueTeamID`.
    pub fn allocate_team_id(&mut self) -> TeamId {
        self.unique_team_id = self.unique_team_id.wrapping_add(1);
        self.unique_team_id
    }

    /// `++m_uniqueTeamPrototypeID`.
    pub fn allocate_prototype_id(&mut self) -> TeamPrototypeId {
        self.unique_prototype_id = self.unique_prototype_id.wrapping_add(1);
        self.unique_prototype_id
    }

    /// `m_uniqueTeamID` (the last id handed out).
    pub fn last_team_id(&self) -> TeamId {
        self.unique_team_id
    }

    /// `m_uniqueTeamPrototypeID`.
    pub fn last_prototype_id(&self) -> TeamPrototypeId {
        self.unique_prototype_id
    }

    /// `TeamFactory::reset()`: counters to the invalid id, maps cleared.
    pub fn reset(&mut self) {
        *self = TeamArena::new();
    }

    /// Adds a prototype under its id (the map `m_prototypes[id] = proto`).
    pub fn insert_prototype(&mut self, p: TeamPrototype) {
        self.prototypes.insert(p.id, p);
    }

    /// Adds a team instance under its id.
    pub fn insert_team(&mut self, t: Team) {
        self.teams.insert(t.id, t);
    }

    /// `findTeamByID(id)`.
    pub fn find_team_by_id(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    /// `findTeamPrototypeByID(id)`.
    pub fn find_prototype_by_id(&self, id: TeamPrototypeId) -> Option<&TeamPrototype> {
        self.prototypes.get(&id)
    }

    /// The prototypes, in id order (`std::map` order).
    pub fn prototypes(&self) -> impl Iterator<Item = &TeamPrototype> {
        self.prototypes.values()
    }

    /// The teams, in id order.
    pub fn teams(&self) -> impl Iterator<Item = &Team> {
        self.teams.values()
    }

    /// `TeamFactory::loadPostProcess`'s first part: both counters become the
    /// highest id in use plus one (0 when none). See the module docs for the
    /// skipped id that follows.
    pub fn recover_counters(&mut self) {
        self.unique_team_id = 0;
        self.unique_prototype_id = 0;
        for p in self.prototypes.values() {
            if p.id >= self.unique_prototype_id {
                self.unique_prototype_id = p.id.wrapping_add(1);
            }
        }
        for t in self.teams.values() {
            if t.id >= self.unique_team_id {
                self.unique_team_id = t.id.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_pre_incremented_from_zero() {
        let mut t = TeamArena::new();
        assert_eq!(t.allocate_team_id(), 1);
        assert_eq!(t.allocate_team_id(), 2);
        assert_eq!(t.allocate_prototype_id(), 1);
        assert_eq!((t.last_team_id(), t.last_prototype_id()), (2, 1));
        t.reset();
        assert_eq!(t.allocate_team_id(), 1);
    }

    #[test]
    fn load_recovers_highest_plus_one_so_the_next_id_skips_one() {
        let mut t = TeamArena::new();
        t.insert_prototype(TeamPrototype { id: 4 });
        t.insert_prototype(TeamPrototype { id: 2 });
        t.insert_team(Team { id: 9, prototype: 4 });
        t.insert_team(Team { id: 3, prototype: 2 });
        t.recover_counters();
        assert_eq!((t.last_team_id(), t.last_prototype_id()), (10, 5));
        assert_eq!(t.allocate_team_id(), 11, "9 was the highest; 10 is skipped");
        assert_eq!(t.find_team_by_id(3).map(|x| x.prototype), Some(2));
        assert_eq!(t.prototypes().map(|p| p.id).collect::<Vec<_>>(), [2, 4]);
        let mut empty = TeamArena::new();
        empty.allocate_team_id();
        empty.recover_counters();
        assert_eq!(empty.last_team_id(), 0);
        assert_eq!(empty.teams().count(), 0);
    }
}
