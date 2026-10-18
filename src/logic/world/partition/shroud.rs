// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (the shroud and fog of war, the unlook queue, the threat and value maps,
//   `storeFoggedCells`/`restoreFoggedCells`, `getShrouedStatus`)
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

//! Shroud, fog and the value maps (the logic side of visibility).
//!
//! Every player has its own shroud counter per cell, so the shroud state is
//! part of the simulation (and of the CRC) even for players nobody is watching.
//! A cell's status for a player is **shrouded** (never seen, `current == 1`),
//! **fogged** (seen, nobody looking now, `current == 0`) or **clear**
//! (`current < 0`: the count of lookers). When a cell's status changes, every
//! object touching it is told to recompute its own visibility lazily
//! (`invalidateShroudedStatusForAllCois`), and, for the local player only, the
//! client is told ([`PartitionClientEvent::SetShroudLevel`]).
//!
//! # Translation choices
//!
//! - The six `hLine*` callbacks of upstream (passed to `DiscreteCircle::drawCircle`
//!   with a `void *` payload) are one function over an operation enum; they
//!   clamp the row to the grid and skip cells outside it, as upstream does.
//! - The threat and value maps weigh a cell by its distance from the circle's
//!   centre: `sqrt(pow(..) + pow(..))` is evaluated in **`double`** (the C
//!   library's, here petir's `f64` functions) and narrowed to the `Real`
//!   `distance`; the weight `1 - distance / radius` is then `float` arithmetic.
//! - Player-indexed arrays are indexed unchecked upstream; an index outside
//!   `0..MAX_PLAYER_COUNT` does nothing here.

use super::cell::{PartitionCell, SightingInfo};
use super::host::PartitionHost;
use super::manager::{PartitionClientEvent, PartitionManager};
use crate::common::base_type::{real_to_unsigned_int, Coord3D};
use crate::common::discrete_circle::{DiscreteCircle, DiscreteCircleError};
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, PlayerMaskType, MAX_PLAYER_COUNT};
use crate::common::kind_of::{KINDOF_IMMOBILE, KINDOF_MINE};
use crate::common::game_common::Relationship;
use crate::logic::world::ids::ObjectId;
use petir::Real;

/// `STORE_DONTTOUCH`.
pub const STORE_DONTTOUCH: u8 = 0;
/// `STORE_FOG`.
pub const STORE_FOG: u8 = 1;
/// `STORE_PERMANENTLY_REVEALED`.
pub const STORE_PERMANENTLY_REVEALED: u8 = 2;

/// `struct ShroudStatusStoreRestore`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShroudStatusStoreRestore {
    /// `m_foggedOrRevealed[MAX_PLAYER_COUNT]`.
    pub fogged_or_revealed: [Vec<u8>; MAX_PLAYER_COUNT as usize],
    /// `m_cellsWide`.
    pub cells_wide: i32,
}

/// `enum ValueOrThreat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueOrThreat {
    /// `VOT_CashValue`.
    CashValue = 1,
    /// `VOT_ThreatValue`.
    ThreatValue = 2,
}

/// Which of the six `hLine*` callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineOp {
    AddLooker,
    RemoveLooker,
    AddShrouder,
    RemoveShrouder,
}

/// Which of the four `hLine*Threat/Value` callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueOp {
    AddThreat,
    RemoveThreat,
    AddValue,
    RemoveValue,
}

/// `struct ThreatValueParms`.
#[derive(Clone, Copy, Debug)]
struct ThreatValueParms {
    player_index: i32,
    x_center: f32,
    y_center: f32,
    radius: f32,
    threat_or_value: u32,
}

impl PartitionManager {
    // ---- per-object shroudedness ---------------------------------------------

    /// `PartitionData::invalidateShroudedStatusForPlayer(playerIndex)`.
    /// (`DISABLE_INVALID_PREVENTION` is defined upstream: unconditional.)
    pub(super) fn invalidate_shrouded_status_for_player(&mut self, slot: usize, player: usize) {
        self.md_mut(slot).shroudedness[player] = ObjectShroudStatus::Invalid;
    }

    /// `PartitionData::invalidateShroudedStatusForAllPlayers()`.
    pub(super) fn invalidate_shrouded_status_for_all_players(&mut self, slot: usize) {
        for i in 0..MAX_PLAYER_COUNT as usize {
            self.invalidate_shrouded_status_for_player(slot, i);
        }
    }

    /// `PartitionCell::invalidateShroudedStatusForAllCois(playerIndex)`.
    fn invalidate_shrouded_status_for_all_cois(&mut self, cell: usize, player: usize) {
        let mut coi = self.cells[cell].first_coi;
        while let Some(r) = coi {
            if let Some(m) = self.coi(r).module {
                self.invalidate_shrouded_status_for_player(m as usize, player);
            }
            coi = self.coi(r).next;
        }
    }

    /// The edge trigger shared by `addLooker`, `removeLooker`, `addShrouder`:
    /// on a change of status, tell the objects to think about their
    /// shroudedness, and update the client if this is the local player.
    fn cell_shroud_edge<H: PartitionHost>(&mut self, host: &H, cell: usize, player: usize, old: CellShroudStatus, new: CellShroudStatus) {
        if old != new {
            self.invalidate_shrouded_status_for_all_cois(cell, player);
            if host.local_player_index() == Some(player as i32) {
                // and if this is the local player, do the Client update.
                let c = &self.cells[cell];
                let (x, y) = (c.get_cell_x(), c.get_cell_y());
                self.events.push(PartitionClientEvent::SetShroudLevel { x, y, status: new });
            }
        }
    }

    /// `PartitionCell::addLooker(playerIndex)`.
    pub(super) fn cell_add_looker<H: PartitionHost>(&mut self, host: &H, cell: usize, player: i32) {
        if !(0..MAX_PLAYER_COUNT).contains(&player) {
            return;
        }
        let p = player as usize;
        let old = self.cells[cell].get_shroud_status_for_player(p);
        self.cells[cell].step_add_looker(p);
        let new = self.cells[cell].get_shroud_status_for_player(p);
        self.cell_shroud_edge(host, cell, p, old, new);
    }

    /// `PartitionCell::removeLooker(playerIndex)`.
    pub(super) fn cell_remove_looker<H: PartitionHost>(&mut self, host: &H, cell: usize, player: i32) {
        if !(0..MAX_PLAYER_COUNT).contains(&player) {
            return;
        }
        let p = player as usize;
        let old = self.cells[cell].get_shroud_status_for_player(p);
        self.cells[cell].step_remove_looker(p);
        let new = self.cells[cell].get_shroud_status_for_player(p);
        self.cell_shroud_edge(host, cell, p, old, new);
    }

    /// `PartitionCell::addShrouder(playerIndex)`.
    pub(super) fn cell_add_shrouder<H: PartitionHost>(&mut self, host: &H, cell: usize, player: i32) {
        if !(0..MAX_PLAYER_COUNT).contains(&player) {
            return;
        }
        let p = player as usize;
        let old = self.cells[cell].get_shroud_status_for_player(p);
        self.cells[cell].step_add_shrouder(p);
        let new = self.cells[cell].get_shroud_status_for_player(p);
        self.cell_shroud_edge(host, cell, p, old, new);
    }

    /// `PartitionCell::removeShrouder(playerIndex)`: never a change to the client.
    pub(super) fn cell_remove_shrouder(&mut self, cell: usize, player: i32) {
        if !(0..MAX_PLAYER_COUNT).contains(&player) {
            return;
        }
        self.cells[cell].step_remove_shrouder(player as usize);
    }

    /// `PartitionData::getShroudedStatus(playerIndex)`: how much of the object
    /// the player can see, recomputed lazily from the cells the object touches.
    /// `INVALID` until the first `update()` after a reset.
    pub(super) fn module_shrouded_status<H: PartitionHost>(&mut self, host: &H, slot: usize, player: usize) -> ObjectShroudStatus {
        if !self.updated_since_last_reset {
            // The shroud should be invalid until update has been called. Set myself as invalid so
            // the next time you ask I'll recompute.
            self.invalidate_shrouded_status_for_player(slot, player);
            return self.md(slot).shroudedness[player];
        }

        if self.md(slot).shroudedness[player] == ObjectShroudStatus::Invalid {
            let mut shrouded_cells = 0;
            let mut fogged_cells = 0;
            let in_use = self.md(slot).coi_in_use_count;
            for i in 0..in_use.max(0) as usize {
                let Some(cell) = self.md(slot).coi_array.get(i).and_then(|c| c.cell) else {
                    continue;
                };
                match self.cells[cell as usize].get_shroud_status_for_player(player) {
                    CellShroudStatus::Shrouded => shrouded_cells += 1,
                    CellShroudStatus::Fogged => fogged_cells += 1,
                    CellShroudStatus::Clear => {}
                }
            }

            let object = self.md(slot).object;
            let ghost = self.md(slot).ghost.as_ref().map(|g| g.handle);
            let previous = self.md(slot).shroudedness_previous[player];

            if in_use == 0 {
                // Stuff off the map have no COIs since they aren't in the PartitionCell range
                let m = self.md_mut(slot);
                m.shroudedness[player] = ObjectShroudStatus::Shrouded;
                m.ever_seen_by_player[player] = false; // force object as never seen by the player
            } else if shrouded_cells == in_use {
                // every cell I use is shrouded
                let m = self.md_mut(slot);
                m.shroudedness[player] = ObjectShroudStatus::Shrouded;
                m.ever_seen_by_player[player] = false; // force object as never seen by the player
                if let Some(g) = ghost {
                    if previous == ObjectShroudStatus::Fogged {
                        // we are shrouding an area that used to be fogged so release our memory of what was there.
                        self.events.push(PartitionClientEvent::GhostFreeSnapShot { ghost: g, player: player as i32 });
                    }
                }
            } else if shrouded_cells + fogged_cells == in_use {
                self.md_mut(slot).shroudedness[player] = ObjectShroudStatus::Fogged; // object is visible but fogged.
                if let (Some(obj), Some(g)) = (object, ghost) {
                    // object does not exist for modules holding only GhostObjects
                    // fogged but may not be visible if faction unit or faction building that has not been seen before
                    let ever_seen = self.md(slot).ever_seen_by_player[player];
                    if host.player_relationship_to_object_team(player as i32, obj) == Relationship::Neutral {
                        // anything neutral that moves around will not be rendered inside fog.
                        if !host.is_kind_of(obj, KINDOF_IMMOBILE) {
                            self.md_mut(slot).shroudedness[player] = ObjectShroudStatus::Shrouded;
                        }
                    } else {
                        // Not neutral: enemy unit will always be shrouded unless it's a building that's already
                        // been seen by the player. Fogged Mines are also always shrouded no matter what.
                        if !(host.is_kind_of(obj, KINDOF_IMMOBILE) && ever_seen) || host.is_kind_of(obj, KINDOF_MINE) {
                            self.md_mut(slot).shroudedness[player] = ObjectShroudStatus::Shrouded;
                        }
                    }
                    if self.md(slot).shroudedness[player] == ObjectShroudStatus::Fogged {
                        // successfully applied fog to object so check if we need to freeze its state
                        if previous < ObjectShroudStatus::Fogged {
                            // object was not previously fogged but now is: record its current state
                            self.events.push(PartitionClientEvent::GhostSnapShot { ghost: g, player: player as i32 });
                        }
                    }
                }
            } else if shrouded_cells == 0 && fogged_cells == 0 {
                // Record that this object was seen by the player (used to show fogged enemy faction buildings).
                let m = self.md_mut(slot);
                m.ever_seen_by_player[player] = true;
                m.shroudedness[player] = ObjectShroudStatus::Clear;
                if let Some(g) = ghost {
                    if previous == ObjectShroudStatus::Fogged {
                        // previously fogged but now visible, so we no longer need a ghost snapshot.
                        self.events.push(PartitionClientEvent::GhostFreeSnapShot { ghost: g, player: player as i32 });
                    }
                }
            } else {
                // I am at least partially clear otherwise
                let m = self.md_mut(slot);
                m.ever_seen_by_player[player] = true;
                m.shroudedness[player] = ObjectShroudStatus::PartialClear;
                if let Some(g) = ghost {
                    if previous == ObjectShroudStatus::Fogged {
                        self.events.push(PartitionClientEvent::GhostFreeSnapShot { ghost: g, player: player as i32 });
                    }
                }
            }

            if in_use != 0 {
                // only remember the previous state of objects actually on the map.
                let m = self.md_mut(slot);
                m.shroudedness_previous[player] = m.shroudedness[player];
            }
        }

        self.md(slot).shroudedness[player]
    }

    /// `Object::getShroudedStatus(playerIndex)` (`Object.cpp`), which the
    /// `PartitionFilterFreeOfFog` and the AI call: `KINDOF_ALWAYS_VISIBLE`
    /// objects are always clear, an object outside the partition system
    /// (garrisoned, say) is clear, otherwise the module's answer.
    pub fn get_object_shrouded_status<H: PartitionHost>(&mut self, host: &H, object: ObjectId, player: i32) -> ObjectShroudStatus {
        if host.is_kind_of(object, crate::common::kind_of::KINDOF_ALWAYS_VISIBLE) {
            return ObjectShroudStatus::Clear;
        }
        match self.by_object.get(&object) {
            Some(&slot) if (0..MAX_PLAYER_COUNT).contains(&player) => self.module_shrouded_status(host, slot, player as usize),
            Some(_) => ObjectShroudStatus::Invalid,
            None => ObjectShroudStatus::Clear,
        }
    }

    // ---- the cells' status ----------------------------------------------------

    /// `getShroudStatusForPlayer(playerIndex, x, y)`: shrouded for a bad
    /// player or a cell off the grid.
    pub fn get_shroud_status_for_player_at_cell(&self, player_index: i32, x: i32, y: i32) -> CellShroudStatus {
        if player_index < 0 {
            return CellShroudStatus::Shrouded; // Safety.
        }
        match self.cell_index(x, y) {
            Some(c) if player_index < MAX_PLAYER_COUNT => self.cells[c].get_shroud_status_for_player(player_index as usize),
            _ => CellShroudStatus::Shrouded,
        }
    }

    /// `getShroudStatusForPlayer(playerIndex, loc)`.
    pub fn get_shroud_status_for_player(&self, player_index: i32, loc: &Coord3D) -> CellShroudStatus {
        let (x, y) = self.world_to_cell(loc.x, loc.y);
        self.get_shroud_status_for_player_at_cell(player_index, x, y)
    }

    /// `getPropShroudStatusForPlayer(playerIndex, loc)`: a prop is a point on a
    /// cell corner, so the four cells around it are compared.
    pub fn get_prop_shroud_status_for_player(&self, player_index: i32, loc: &Coord3D) -> ObjectShroudStatus {
        let (x, y) = self.world_to_cell(loc.x - self.cell_size * 0.5, loc.y - self.cell_size * 0.5);

        let cell_stat = self.get_shroud_status_for_player_at_cell(player_index, x, y);
        if cell_stat != self.get_shroud_status_for_player_at_cell(player_index, x + 1, y) {
            return ObjectShroudStatus::PartialClear;
        }
        if cell_stat != self.get_shroud_status_for_player_at_cell(player_index, x + 1, y + 1) {
            return ObjectShroudStatus::PartialClear;
        }
        if cell_stat != self.get_shroud_status_for_player_at_cell(player_index, x, y + 1) {
            return ObjectShroudStatus::PartialClear;
        }
        if cell_stat == CellShroudStatus::Shrouded {
            return ObjectShroudStatus::Shrouded;
        }
        if cell_stat == CellShroudStatus::Clear {
            return ObjectShroudStatus::Clear;
        }
        ObjectShroudStatus::Fogged
    }

    // ---- looking and shrouding in circles -------------------------------------

    /// One row of a circle: `hLineAddLooker` and its siblings.
    fn h_line_look<H: PartitionHost>(&mut self, host: &H, x1: i32, x2: i32, y: i32, player: i32, op: LineOp) {
        if y < 0 || y >= self.cell_count_y || x1 >= self.cell_count_x || x2 < 0 {
            return;
        }
        let lo = x1.max(0);
        let hi = x2.min(self.cell_count_x - 1);
        let mut x = lo;
        while x <= hi {
            if let Some(cell) = self.cell_index(x, y) {
                match op {
                    LineOp::AddLooker => self.cell_add_looker(host, cell, player),
                    LineOp::RemoveLooker => self.cell_remove_looker(host, cell, player),
                    LineOp::AddShrouder => self.cell_add_shrouder(host, cell, player),
                    LineOp::RemoveShrouder => self.cell_remove_shrouder(cell, player),
                }
            }
            x += 1;
        }
    }

    /// The common shape of `doShroudReveal`, `undoShroudReveal`, `doShroudCover`
    /// and `undoShroudCover`: a circle of cells, once for each player in the
    /// mask, from the last player down to the first.
    fn shroud_circle<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType, op: LineOp) -> Result<(), DiscreteCircleError> {
        let (cell_center_x, cell_center_y) = self.world_to_cell(center_x, center_y);

        let mut cell_radius = self.world_to_cell_dist(radius);
        if cell_radius < 1 {
            cell_radius = 1;
        }

        let circle = DiscreteCircle::new(cell_center_x, cell_center_y, cell_radius)?;

        let mut current_index = host.player_count() - 1;
        while current_index >= 0 {
            // Object's Look is the one who knows about allies. Anyone can pass a player mask to me
            // and all of those players will have an active looker applied to a bunch of cells.
            if let Some(mask) = host.player_mask(current_index) {
                if player_mask & mask != 0 {
                    circle.draw_circle(|x1, x2, y| self.h_line_look(host, x1, x2, y, current_index, op))?;
                }
            }
            current_index -= 1;
        }
        Ok(())
    }

    /// `doShroudReveal(centerX, centerY, radius, playerMask)`: the main accessor
    /// of the shroud system. Allies are the caller's business (`Object::look`).
    pub fn do_shroud_reveal<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.shroud_circle(host, center_x, center_y, radius, player_mask, LineOp::AddLooker)
    }

    /// `undoShroudReveal(...)`.
    pub fn undo_shroud_reveal<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.shroud_circle(host, center_x, center_y, radius, player_mask, LineOp::RemoveLooker)
    }

    /// `doShroudCover(...)`.
    pub fn do_shroud_cover<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.shroud_circle(host, center_x, center_y, radius, player_mask, LineOp::AddShrouder)
    }

    /// `undoShroudCover(...)`.
    pub fn undo_shroud_cover<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.shroud_circle(host, center_x, center_y, radius, player_mask, LineOp::RemoveShrouder)
    }

    // ---- the unlook queue ------------------------------------------------------

    /// `queueUndoShroudReveal(...)`: the unlook happens `m_unlookPersistDuration`
    /// frames from now. The queue is in order because the delay is a constant.
    pub fn queue_undo_shroud_reveal<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, player_mask: PlayerMaskType) {
        let now = host.frame();
        let new_info = SightingInfo {
            where_: Coord3D { x: center_x, y: center_y, z: 0.0 },
            how_far: radius,
            for_whom: player_mask,
            data: now.wrapping_add(host.unlook_persist_duration()),
        };
        self.pending_undo_shroud_reveals.push_back(new_info);
    }

    /// `processPendingUndoShroudRevealQueue(considerTimestamp)`: keep popping
    /// and processing until the front one is in the future.
    pub(super) fn process_pending_undo_shroud_reveal_queue<H: PartitionHost>(&mut self, host: &H, consider_timestamp: bool) {
        let compare_time = if consider_timestamp { host.frame() } else { u32::MAX };

        while self.pending_undo_shroud_reveals.front().is_some_and(|f| f.data < compare_time) {
            let Some(this_info) = self.pending_undo_shroud_reveals.pop_front() else {
                break;
            };
            // (a circle too large to draw is skipped; upstream's is undefined there)
            let _ = self.undo_shroud_reveal(host, this_info.where_.x, this_info.where_.y, this_info.how_far, this_info.for_whom);
        }
    }

    /// `processEntirePendingUndoShroudRevealQueue()`: rush through the whole
    /// queue regardless of timestamp (a map resize, a full-map undo).
    pub fn process_entire_pending_undo_shroud_reveal_queue<H: PartitionHost>(&mut self, host: &H) {
        self.process_pending_undo_shroud_reveal_queue(host, false);
    }

    /// `resetPendingUndoShroudRevealQueue()`: delete everything without doing anything with it.
    pub(super) fn reset_pending_undo_shroud_reveal_queue(&mut self) {
        self.pending_undo_shroud_reveals.clear();
    }

    /// The queue, oldest first (for the CRC/save and tests).
    pub fn pending_undo_shroud_reveals(&self) -> impl Iterator<Item = &SightingInfo> {
        self.pending_undo_shroud_reveals.iter()
    }

    // ---- whole-map operations ---------------------------------------------------

    /// `revealMapForPlayer(playerIndex)`: look and stop on every cell, which clears
    /// all passive shroud without the ally logic of the normal look.
    pub fn reveal_map_for_player<H: PartitionHost>(&mut self, host: &H, player_index: i32) {
        for i in 0..self.cells.len() {
            self.cell_add_looker(host, i, player_index);
            self.cell_remove_looker(host, i, player_index);
        }
    }

    /// `revealMapForPlayerPermanently(playerIndex)`: skip the remove, so the
    /// player counts as actively looking at everything (Observer mode).
    pub fn reveal_map_for_player_permanently<H: PartitionHost>(&mut self, host: &H, player_index: i32) {
        for i in 0..self.cells.len() {
            self.cell_add_looker(host, i, player_index);
        }
    }

    /// `undoRevealMapForPlayerPermanently(playerIndex)`: adds a layer of
    /// permanent blindness (debugging only).
    pub fn undo_reveal_map_for_player_permanently<H: PartitionHost>(&mut self, host: &H, player_index: i32) {
        // First make sure no lingering looks will leave holes when they aren't wanted.
        self.process_entire_pending_undo_shroud_reveal_queue(host);

        for i in 0..self.cells.len() {
            self.cell_remove_looker(host, i, player_index);
        }
    }

    /// `shroudMapForPlayer(playerIndex)`: reset the shroud to passive shroud
    /// (can be explored again).
    pub fn shroud_map_for_player<H: PartitionHost>(&mut self, host: &H, player_index: i32) {
        // First make sure no lingering looks will leave holes when they aren't wanted.
        self.process_entire_pending_undo_shroud_reveal_queue(host);

        // By pulsing a blast of shroud like this, everything not actively looked at becomes passive shroud
        for i in 0..self.cells.len() {
            self.cell_add_shrouder(host, i, player_index);
            self.cell_remove_shrouder(i, player_index);
        }
    }

    /// `refreshShroudForLocalPlayer()`: push the local player's shroud back out
    /// to the display (a drawing refresh), and have its objects recompute.
    pub fn refresh_shroud_for_local_player<H: PartitionHost>(&mut self, host: &H) {
        self.needs_shroud_refresh = false;
        self.events.push(PartitionClientEvent::ClearShroud);

        let Some(player_index) = host.local_player_index() else {
            return;
        };
        for i in 0..self.cells.len() {
            let (x, y) = (self.cells[i].get_cell_x(), self.cells[i].get_cell_y());
            if (0..MAX_PLAYER_COUNT).contains(&player_index) {
                let status = self.cells[i].get_shroud_status_for_player(player_index as usize);
                self.events.push(PartitionClientEvent::SetShroudLevel { x, y, status });
                self.invalidate_shrouded_status_for_all_cois(i, player_index as usize);
            }
        }
    }

    /// Whether an `xfer` load asked for `refresh_shroud_for_local_player`.
    pub fn needs_shroud_refresh(&self) -> bool {
        self.needs_shroud_refresh
    }

    // ---- threat and value maps ---------------------------------------------------

    /// One row of the threat/value circle: `hLineAddThreat` and its siblings.
    fn h_line_value(&mut self, x1: i32, x2: i32, y: i32, parms: &ThreatValueParms, op: ValueOp) {
        if y < 0 || y >= self.cell_count_y || x1 >= self.cell_count_x || x2 < 0 {
            return;
        }
        let lo = x1.max(0);
        let hi = x2.min(self.cell_count_x - 1);
        let mut x = lo;
        while x <= hi {
            if let Some(cell) = self.cell_index(x, y) {
                // `Real distance = sqrt(pow(x - xCenter, 2) + pow(y - yCenter, 2))`: the squares and the
                // root are `double` (`x - xCenter` is a float, widened), the result is narrowed to the
                // `Real` it is stored in; `mulVal = 1 - distance / radius` is then float arithmetic.
                let dx = f64::from(x as f32 - parms.x_center);
                let dy = f64::from(y as f32 - parms.y_center);
                let distance = Real::sqrt(Real::powf(dx, 2.0) + Real::powf(dy, 2.0)) as f32;
                // (a NaN passes through `clamp`, as it does through upstream's if/else-if)
                let mul_val = (1.0f32 - distance / parms.radius).clamp(0.0, 1.0);

                let amount = real_to_unsigned_int(parms.threat_or_value as f32 * mul_val);
                let c: &mut PartitionCell = &mut self.cells[cell];
                match op {
                    ValueOp::AddThreat => c.add_threat_value(parms.player_index, amount),
                    ValueOp::RemoveThreat => c.remove_threat_value(parms.player_index, amount),
                    ValueOp::AddValue => c.add_cash_value(parms.player_index, amount),
                    ValueOp::RemoveValue => c.remove_cash_value(parms.player_index, amount),
                }
            }
            x += 1;
        }
    }

    /// The common shape of `doThreatAffect`, `undoThreatAffect`, `doValueAffect`
    /// and `undoValueAffect`.
    #[allow(clippy::too_many_arguments)] // one function behind upstream's four `*ThreatAffect`/`*ValueAffect`
    fn value_circle<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, value: u32, player_mask: PlayerMaskType, op: ValueOp) -> Result<(), DiscreteCircleError> {
        let (cell_center_x, cell_center_y) = self.world_to_cell(center_x, center_y);
        let f_cell_center_x = cell_center_x as f32;
        let f_cell_center_y = cell_center_y as f32;

        let mut cell_radius = self.world_to_cell_dist(radius);
        if cell_radius < 1 {
            cell_radius = 1;
        }

        let f_cell_radius = cell_radius.wrapping_add(1) as f32;

        let circle = DiscreteCircle::new(cell_center_x, cell_center_y, cell_radius)?;

        let mut parms = ThreatValueParms { player_index: 0, x_center: f_cell_center_x, y_center: f_cell_center_y, radius: f_cell_radius, threat_or_value: value };

        let mut current_index = host.player_count() - 1;
        while current_index >= 0 {
            if let Some(mask) = host.player_mask(current_index) {
                if player_mask & mask != 0 {
                    parms.player_index = current_index;
                    circle.draw_circle(|x1, x2, y| self.h_line_value(x1, x2, y, &parms, op))?;
                }
            }
            current_index -= 1;
        }
        Ok(())
    }

    /// `doThreatAffect(...)`.
    pub fn do_threat_affect<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, threat_val: u32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.value_circle(host, center_x, center_y, radius, threat_val, player_mask, ValueOp::AddThreat)
    }

    /// `undoThreatAffect(...)`.
    pub fn undo_threat_affect<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, threat_val: u32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.value_circle(host, center_x, center_y, radius, threat_val, player_mask, ValueOp::RemoveThreat)
    }

    /// `doValueAffect(...)`.
    pub fn do_value_affect<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, value_val: u32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.value_circle(host, center_x, center_y, radius, value_val, player_mask, ValueOp::AddValue)
    }

    /// `undoValueAffect(...)`.
    pub fn undo_value_affect<H: PartitionHost>(&mut self, host: &H, center_x: f32, center_y: f32, radius: f32, value_val: u32, player_mask: PlayerMaskType) -> Result<(), DiscreteCircleError> {
        self.value_circle(host, center_x, center_y, radius, value_val, player_mask, ValueOp::RemoveValue)
    }

    /// The masks of every player slot (`allPlayerMasks[i] = getNthPlayer(i)->getPlayerMask()`).
    fn all_player_masks<H: PartitionHost>(host: &H) -> [PlayerMaskType; MAX_PLAYER_COUNT as usize] {
        let mut masks = [0 as PlayerMaskType; MAX_PLAYER_COUNT as usize];
        let total = host.player_count();
        for i in 0..total.clamp(0, MAX_PLAYER_COUNT) {
            if let Some(m) = host.player_mask(i) {
                masks[i as usize] = m;
            }
        }
        masks
    }

    /// `getMostValuableLocation(playerIndex, whichPlayerTypes, valType)`: the
    /// (cell-corner) location of the cell with the largest value, or `None`
    /// where upstream leaves `outLocation` untouched (no players of those
    /// types, or no cells).
    pub fn get_most_valuable_location<H: PartitionHost>(&self, host: &H, player_index: i32, which_player_types: u32, val_type: ValueOrThreat) -> Option<Coord3D> {
        let player_mask = host.players_with_relationship(player_index, which_player_types);
        if player_mask == 0 {
            return None;
        }

        let all_player_masks = Self::all_player_masks(host);

        let mut greatest_value_cell: i32 = -1;
        let mut max_cell_value: i32 = -1;
        for i in 0..self.cells.len() {
            let mut cell_value: i32 = 0;

            for player in 0..MAX_PLAYER_COUNT {
                // (upstream's BitTest argument order is (allPlayerMasks[player], playerMask); symmetric)
                if all_player_masks[player as usize] & player_mask != 0 {
                    let v = if val_type == ValueOrThreat::CashValue { self.cells[i].get_cash_value(player) } else { self.cells[i].get_threat_value(player) };
                    cell_value = cell_value.wrapping_add(v as i32);
                }
            }

            if cell_value > max_cell_value {
                max_cell_value = cell_value;
                greatest_value_cell = i as i32;
            }
        }

        if greatest_value_cell == -1 || max_cell_value == -1 {
            return None;
        }

        let c = &self.cells[greatest_value_cell as usize];
        let size = host.partition_cell_size();
        Some(Coord3D { x: c.get_cell_x() as f32 * size, y: c.get_cell_y() as f32 * size, z: 0.0 })
    }

    /// `getNearestGroupWithValue(...)`: the (cell-corner) location of the first
    /// cell, in the breadth-first walk from `source_location`, whose value passes
    /// the test; `None` where upstream leaves `outLocation` untouched. Also
    /// returns whether the walk reached the part of upstream's walk that reads out of
    /// bounds (see [`PartitionManager::iterate_cells_breadth_first`]).
    #[allow(clippy::too_many_arguments)] // the signature of upstream's `getNearestGroupWithValue`
    pub fn get_nearest_group_with_value<H: PartitionHost>(
        &self,
        host: &H,
        player_index: i32,
        which_player_types: u32,
        val_type: ValueOrThreat,
        source_location: &Coord3D,
        value_required: i32,
        _greater_than: bool,
    ) -> (Option<Coord3D>, bool) {
        let allowed_players_masks = host.players_with_relationship(player_index, which_player_types);
        if allowed_players_masks == 0 {
            return (None, false);
        }

        let all_players_mask = Self::all_player_masks(host);

        // upstream: `parms.greaterThan = valueRequired;` (an Int stored in a Bool)
        let greater_than = value_required != 0;

        let (nearest_great, undefined_reached) = self.iterate_cells_breadth_first(source_location, |cell| {
            let mut val: u32 = 0;
            for i in 0..MAX_PLAYER_COUNT {
                if allowed_players_masks & all_players_mask[i as usize] != 0 {
                    if val_type == ValueOrThreat::CashValue {
                        val = val.wrapping_add(cell.get_cash_value(i));
                    } else {
                        val = val.wrapping_add(cell.get_threat_value(i));
                    }
                }
            }
            // `val > valueRequired` compares unsigned with the Int converted to unsigned
            let required = value_required as u32;
            (val > required && greater_than) || (val < required && !greater_than)
        });
        if nearest_great != -1 {
            let c = &self.cells[nearest_great as usize];
            let size = host.partition_cell_size();
            return (Some(Coord3D { x: c.get_cell_x() as f32 * size, y: c.get_cell_y() as f32 * size, z: 0.0 }), undefined_reached);
        }
        (None, undefined_reached)
    }

    // ---- storing and restoring fog (map border resizing) -------------------------

    /// `storeFoggedCells(outPartitionStore, storeToFog)`: the first pass
    /// (`store_to_fog`) records fogged cells, the second records permanently
    /// revealed ones.
    pub fn store_fogged_cells(&self, out: &mut ShroudStatusStoreRestore, store_to_fog: bool) {
        if store_to_fog {
            // This is the first pass
            out.cells_wide = self.cell_count_x;

            // Resize all of our arrays.
            for p in 0..MAX_PLAYER_COUNT as usize {
                out.fogged_or_revealed[p] = vec![STORE_DONTTOUCH; self.total_cell_count.max(0) as usize];
            }
        }

        for p in 0..MAX_PLAYER_COUNT as usize {
            if out.fogged_or_revealed[p].len() != self.total_cell_count.max(0) as usize {
                continue;
            }

            for j in 0..self.cell_count_y {
                for i in 0..self.cell_count_x {
                    let idx = (j * self.cell_count_x + i) as usize;
                    let status = self.cells[idx].get_shroud_status_for_player(p);
                    if store_to_fog && status == CellShroudStatus::Fogged {
                        out.fogged_or_revealed[p][idx] = STORE_FOG;
                    }
                    if !store_to_fog && status == CellShroudStatus::Clear {
                        out.fogged_or_revealed[p][idx] = STORE_PERMANENTLY_REVEALED;
                    }
                }
            }
        }
    }

    /// `restoreFoggedCells(inPartitionStore, restoreToFog)`.
    pub fn restore_fogged_cells<H: PartitionHost>(&mut self, host: &H, input: &ShroudStatusStoreRestore, restore_to_fog: bool) {
        let store_width = input.cells_wide;
        if store_width <= 0 {
            return; // upstream divides by it
        }
        let store_height = input.fogged_or_revealed[0].len() as i32 / store_width;

        for p in 0..MAX_PLAYER_COUNT as usize {
            for j in 0..store_height {
                if j >= self.cell_count_y {
                    // All done.
                    return;
                }

                for i in 0..store_width {
                    if i >= self.cell_count_x {
                        // Skip to the next line. This info will be thrown away.
                        break;
                    }

                    let Some(&byte_to_restore) = input.fogged_or_revealed[p].get((j * store_width + i) as usize) else {
                        continue;
                    };
                    if byte_to_restore == STORE_DONTTOUCH {
                        continue;
                    }

                    let idx = (j * self.cell_count_x + i) as usize;
                    if byte_to_restore == STORE_FOG && restore_to_fog {
                        // restore the fog status of this cell
                        self.cell_add_looker(host, idx, p as i32);
                        self.cell_remove_looker(host, idx, p as i32);
                    }

                    if byte_to_restore == STORE_PERMANENTLY_REVEALED && !restore_to_fog {
                        // Add an extra looker.
                        self.cell_add_looker(host, idx, p as i32);
                    }
                }
            }
        }
    }
}
