// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   (`CellAndObjectIntersection`, `PartitionData`'s members)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The records the manager links together: [`Coi`] (a
//! `CellAndObjectIntersection`, one object touching one cell) and
//! [`PartitionData`] (an object's presence in the grid).
//!
//! Upstream links them with raw pointers: a cell's list of COIs runs through
//! `m_prevCoi`/`m_nextCoi` of COIs that live inside arrays owned by modules,
//! the manager's module list and its dirty list run through the modules.
//! Here modules sit in a slab (`Vec<Option<PartitionData>>`) and every link
//! is an index: a module is a slab slot, a COI is a [`CoiRef`] (module slot,
//! position in that module's array). The lists are the same intrusive
//! lists, prepended and unlinked by the same steps, so their order (which the
//! iteration order of every query follows) is upstream's.

use crate::common::game_common::{ObjectShroudStatus, MAX_PLAYER_COUNT};
use crate::logic::world::ids::ObjectId;
use crate::logic::world::partition::host::GhostGeometry;

/// A COI: the module's slot and the position in its array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoiRef {
    /// The module's slot in the manager's slab.
    pub module: u32,
    /// The index in `m_coiArray`.
    pub index: u32,
}

/// `class CellAndObjectIntersection`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coi {
    /// `m_cell`: index of the cell being touched.
    pub(super) cell: Option<u32>,
    /// `m_module`: slot of the module touching (and so the object).
    pub(super) module: Option<u32>,
    /// `m_prevCoi` (in the cell's list).
    pub(super) prev: Option<CoiRef>,
    /// `m_nextCoi` (in the cell's list).
    pub(super) next: Option<CoiRef>,
}

/// `PartitionData::DirtyStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirtyStatus {
    /// `NOT_DIRTY`.
    NotDirty,
    /// `NEED_COLLISION_CHECK`.
    NeedCollisionCheck,
    /// `NEED_CELL_UPDATE_AND_COLLISION_CHECK`.
    NeedCellUpdateAndCollisionCheck,
}

/// `class PartitionData`: the part of an object that understands how to
/// maintain it in the space partitioning system.
#[derive(Clone, Debug)]
pub struct PartitionData {
    /// `m_object`: the owning object; `None` once detached, and for the
    /// module of a ghost object that has outlived its object.
    pub(super) object: Option<ObjectId>,
    /// `m_ghostObject`, as the data the manager reads from it.
    pub(super) ghost: Option<GhostGeometry>,
    /// `m_next` (the master module list).
    pub(super) next: Option<usize>,
    /// `m_prev`.
    pub(super) prev: Option<usize>,
    /// `m_nextDirty`.
    pub(super) next_dirty: Option<usize>,
    /// `m_prevDirty`.
    pub(super) prev_dirty: Option<usize>,
    /// `m_coiArray`; its length is `m_coiArrayCount`.
    pub(super) coi_array: Vec<Coi>,
    /// `m_coiInUseCount`.
    pub(super) coi_in_use_count: i32,
    /// `m_doneFlag`.
    pub(super) done_flag: i32,
    /// `m_dirtyStatus`.
    pub(super) dirty_status: DirtyStatus,
    /// `m_shroudedness`.
    pub(super) shroudedness: [ObjectShroudStatus; MAX_PLAYER_COUNT as usize],
    /// `m_shroudednessPrevious`.
    pub(super) shroudedness_previous: [ObjectShroudStatus; MAX_PLAYER_COUNT as usize],
    /// `m_everSeenByPlayer`.
    pub(super) ever_seen_by_player: [bool; MAX_PLAYER_COUNT as usize],
    /// `m_lastCell`: the cell index the object's centre was last in.
    pub(super) last_cell: Option<usize>,
}

impl PartitionData {
    /// `PartitionData::PartitionData()`.
    pub(super) fn new() -> PartitionData {
        PartitionData {
            object: None,
            ghost: None,
            next: None,
            prev: None,
            next_dirty: None,
            prev_dirty: None,
            coi_array: Vec::new(),
            coi_in_use_count: 0,
            done_flag: 0,
            dirty_status: DirtyStatus::NotDirty,
            shroudedness: [ObjectShroudStatus::Invalid; MAX_PLAYER_COUNT as usize],
            shroudedness_previous: [ObjectShroudStatus::Invalid; MAX_PLAYER_COUNT as usize],
            ever_seen_by_player: [false; MAX_PLAYER_COUNT as usize],
            last_cell: None,
        }
    }

    /// `isInNeedOfUpdatingCells()`.
    pub fn is_in_need_of_updating_cells(&self) -> bool {
        self.dirty_status == DirtyStatus::NeedCellUpdateAndCollisionCheck
    }

    /// `isInNeedOfCollisionCheck()`.
    pub fn is_in_need_of_collision_check(&self) -> bool {
        self.dirty_status != DirtyStatus::NotDirty
    }

    /// `wasSeenByAnyPlayers()`: the first player who has seen the object and
    /// now looks at a fogged version of it, else `MAX_PLAYER_COUNT` (upstream
    /// returns the `for` variable `i` after the loop, which its pre-standard
    /// compiler leaks; it is `MAX_PLAYER_COUNT` when the loop ran out).
    pub fn was_seen_by_any_players(&self) -> i32 {
        for i in 0..MAX_PLAYER_COUNT as usize {
            if self.ever_seen_by_player[i] && self.shroudedness[i] == ObjectShroudStatus::Fogged {
                return i as i32;
            }
        }
        MAX_PLAYER_COUNT
    }
}
