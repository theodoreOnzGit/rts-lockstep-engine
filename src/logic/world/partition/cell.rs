// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (`ShroudLevel`, `PartitionCell`, `SightingInfo`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! One cell of the partition grid: the per-player shroud counters, the
//! per-player threat and cash value maps, the terrain height range and the head
//! of the list of object intersections. The shroud *transitions* that tell
//! objects to re-think their visibility live in the manager (they need the
//! object table); the arithmetic on the counters is here.

use super::data::CoiRef;
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CellShroudStatus, PlayerMaskType, MAX_PLAYER_COUNT};

/// `HUGE_DIST` as the cell constructor uses it.
const HUGE_DIST: f32 = super::collide::HUGE_DIST;

/// `struct ShroudLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShroudLevel {
    /// `m_currentShroud`: 1 means shrouded, 0 is not, negative is the count
    /// of people looking.
    pub current_shroud: i16,
    /// `m_activeShroudLevel`: 0 means passive shroud, positive is the count
    /// of people shrouding.
    pub active_shroud_level: i16,
}

/// `class PartitionCell`.
#[derive(Clone, Debug)]
pub struct PartitionCell {
    /// `m_firstCoiInCell`.
    pub(super) first_coi: Option<CoiRef>,
    /// `m_shroudLevel`.
    pub(super) shroud_level: [ShroudLevel; MAX_PLAYER_COUNT as usize],
    /// `m_loTerrainZ`.
    pub(super) lo_terrain_z: f32,
    /// `m_hiTerrainZ`.
    pub(super) hi_terrain_z: f32,
    /// `m_threatValue`.
    pub(super) threat_value: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_cashValue`.
    pub(super) cash_value: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_coiCount`.
    pub(super) coi_count: i16,
    /// `m_cellX`.
    pub(super) cell_x: i16,
    /// `m_cellY`.
    pub(super) cell_y: i16,
}

impl Default for PartitionCell {
    /// `PartitionCell::PartitionCell()`: passive shroud (1, 0) for every
    /// player, zero values, no intersections, an empty terrain range.
    fn default() -> Self {
        PartitionCell {
            first_coi: None,
            shroud_level: [ShroudLevel { current_shroud: 1, active_shroud_level: 0 }; MAX_PLAYER_COUNT as usize],
            lo_terrain_z: HUGE_DIST,
            hi_terrain_z: -HUGE_DIST,
            threat_value: [0; MAX_PLAYER_COUNT as usize],
            cash_value: [0; MAX_PLAYER_COUNT as usize],
            coi_count: 0,
            cell_x: 0,
            cell_y: 0,
        }
    }
}

impl PartitionCell {
    /// `init(x, y, loZ, hiZ)`.
    pub fn init(&mut self, x: i32, y: i32, lo_z: f32, hi_z: f32) {
        self.cell_x = x as i16;
        self.cell_y = y as i16;
        self.lo_terrain_z = lo_z;
        self.hi_terrain_z = hi_z;
    }

    /// `getCoiCount()`.
    pub fn get_coi_count(&self) -> i32 {
        i32::from(self.coi_count)
    }
    /// `getCellX()`.
    pub fn get_cell_x(&self) -> i32 {
        i32::from(self.cell_x)
    }
    /// `getCellY()`.
    pub fn get_cell_y(&self) -> i32 {
        i32::from(self.cell_y)
    }
    /// `getLoTerrain()`.
    pub fn get_lo_terrain(&self) -> f32 {
        self.lo_terrain_z
    }
    /// `getHiTerrain()`.
    pub fn get_hi_terrain(&self) -> f32 {
        self.hi_terrain_z
    }
    /// `m_shroudLevel[player]` (a copy), for tests and the CRC.
    pub fn shroud_level(&self, player: usize) -> ShroudLevel {
        self.shroud_level[player]
    }

    /// `getShroudStatusForPlayer(playerIndex)`: there are three answers, but
    /// the question still requires "to whom". `playerIndex` must be in
    /// `0..MAX_PLAYER_COUNT` (upstream indexes the array unchecked).
    pub fn get_shroud_status_for_player(&self, player_index: usize) -> CellShroudStatus {
        let cur = self.shroud_level[player_index].current_shroud;
        if cur == 1 {
            CellShroudStatus::Shrouded
        } else if cur == 0 {
            CellShroudStatus::Fogged // ie nobody actively looking
        } else {
            CellShroudStatus::Clear
        }
    }

    /// The counter step of `addLooker`: "a 1 will go straight to -1, otherwise
    /// it just gets decremented" (`min(current - 1, -1)` in `int`, stored in a `short`).
    pub(super) fn step_add_looker(&mut self, p: usize) {
        let v = i32::from(self.shroud_level[p].current_shroud) - 1;
        self.shroud_level[p].current_shroud = (if v < -1 { v } else { -1 }) as i16;
    }

    /// The counter step of `removeLooker`: a -1 goes up to `min(active, 1)`,
    /// otherwise it just gets incremented (`short` wrap).
    pub(super) fn step_remove_looker(&mut self, p: usize) {
        if self.shroud_level[p].current_shroud == -1 {
            let a = self.shroud_level[p].active_shroud_level;
            self.shroud_level[p].current_shroud = if a < 1 { a } else { 1 };
        } else {
            self.shroud_level[p].current_shroud = self.shroud_level[p].current_shroud.wrapping_add(1);
        }
    }

    /// The counter step of `addShrouder`: the active level goes up and a 0
    /// current level becomes 1.
    pub(super) fn step_add_shrouder(&mut self, p: usize) {
        self.shroud_level[p].active_shroud_level = self.shroud_level[p].active_shroud_level.wrapping_add(1);
        if self.shroud_level[p].current_shroud == 0 {
            self.shroud_level[p].current_shroud = 1;
        }
    }

    /// `removeShrouder`: just decrement the active level (never a client change).
    pub(super) fn step_remove_shrouder(&mut self, p: usize) {
        self.shroud_level[p].active_shroud_level = self.shroud_level[p].active_shroud_level.wrapping_sub(1);
    }

    /// `getThreatValue(playerIndex)`: 0 for a player index out of range.
    pub fn get_threat_value(&self, player_index: i32) -> u32 {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            self.threat_value[player_index as usize] as u32
        } else {
            0
        }
    }

    /// `addThreatValue`: unsigned arithmetic stored in an `Int` (wraps).
    pub fn add_threat_value(&mut self, player_index: i32, threat_value: u32) {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            let v = &mut self.threat_value[player_index as usize];
            *v = (*v as u32).wrapping_add(threat_value) as i32;
        }
    }

    /// `removeThreatValue`.
    pub fn remove_threat_value(&mut self, player_index: i32, threat_value: u32) {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            let v = &mut self.threat_value[player_index as usize];
            *v = (*v as u32).wrapping_sub(threat_value) as i32;
        }
    }

    /// `getCashValue(playerIndex)`.
    pub fn get_cash_value(&self, player_index: i32) -> u32 {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            self.cash_value[player_index as usize] as u32
        } else {
            0
        }
    }

    /// `addCashValue`.
    pub fn add_cash_value(&mut self, player_index: i32, cash_value: u32) {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            let v = &mut self.cash_value[player_index as usize];
            *v = (*v as u32).wrapping_add(cash_value) as i32;
        }
    }

    /// `removeCashValue`.
    pub fn remove_cash_value(&mut self, player_index: i32, cash_value: u32) {
        if (0..MAX_PLAYER_COUNT).contains(&player_index) {
            let v = &mut self.cash_value[player_index as usize];
            *v = (*v as u32).wrapping_sub(cash_value) as i32;
        }
    }
}

/// `class SightingInfo`: one pending "undo a reveal" (position, radius, who,
/// and the frame it is due).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightingInfo {
    /// `m_where`.
    pub where_: Coord3D,
    /// `m_howFar`.
    pub how_far: f32,
    /// `m_forWhom`.
    pub for_whom: PlayerMaskType,
    /// `m_data`: for a pending unlook, the frame it is due.
    pub data: u32,
}

impl Default for SightingInfo {
    /// `SightingInfo::SightingInfo()` calls `reset()`.
    fn default() -> Self {
        SightingInfo { where_: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, how_far: 0.0, for_whom: 0, data: 0 }
    }
}

impl SightingInfo {
    /// `reset()`.
    pub fn reset(&mut self) {
        *self = SightingInfo::default();
    }

    /// `isInvalid()`: `m_howFar == 0.0f`.
    pub fn is_invalid(&self) -> bool {
        self.how_far == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_cell_is_passively_shrouded_for_everyone() {
        let c = PartitionCell::default();
        for p in 0..16 {
            assert_eq!(c.get_shroud_status_for_player(p), CellShroudStatus::Shrouded);
        }
        assert_eq!(c.get_lo_terrain(), 1_000_000.0);
        assert_eq!(c.get_hi_terrain(), -1_000_000.0);
    }

    #[test]
    fn looker_counters_follow_the_decrease_and_increase_algorithms() {
        let mut c = PartitionCell::default();
        c.step_add_looker(2); // 1 goes straight to -1
        assert_eq!(c.shroud_level(2).current_shroud, -1);
        c.step_add_looker(2);
        assert_eq!(c.shroud_level(2).current_shroud, -2);
        c.step_remove_looker(2);
        assert_eq!(c.shroud_level(2).current_shroud, -1);
        c.step_remove_looker(2); // -1 goes up to min(active, 1) = 0: fogged
        assert_eq!(c.shroud_level(2).current_shroud, 0);
        assert_eq!(c.get_shroud_status_for_player(2), CellShroudStatus::Fogged);
        c.step_add_shrouder(2); // active 1, a 0 current becomes 1
        assert_eq!(c.shroud_level(2), ShroudLevel { current_shroud: 1, active_shroud_level: 1 });
        c.step_add_looker(2);
        c.step_remove_looker(2); // back to min(active=1, 1) = 1
        assert_eq!(c.shroud_level(2).current_shroud, 1);
    }

    #[test]
    fn value_maps_ignore_bad_players_and_wrap_like_unsigned_arithmetic() {
        let mut c = PartitionCell::default();
        c.add_threat_value(3, 10);
        c.add_threat_value(16, 10);
        c.add_threat_value(-1, 10);
        assert_eq!(c.get_threat_value(3), 10);
        assert_eq!(c.get_threat_value(16), 0);
        c.remove_threat_value(3, 11);
        assert_eq!(c.get_threat_value(3), u32::MAX);
        c.add_cash_value(0, 7);
        c.remove_cash_value(0, 2);
        assert_eq!(c.get_cash_value(0), 5);
    }

    #[test]
    fn sighting_info_reset_and_invalid() {
        let mut s = SightingInfo { where_: Coord3D { x: 1.0, y: 2.0, z: 3.0 }, how_far: 5.0, for_whom: 3, data: 9 };
        assert!(!s.is_invalid());
        s.reset();
        assert!(s.is_invalid());
        assert_eq!(s, SightingInfo::default());
    }
}
