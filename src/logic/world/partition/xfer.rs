// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (`PartitionCell::crc/xfer`, `PartitionManager::crc/xfer/loadPostProcess`,
//   `SightingInfo::crc/xfer`)
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

//! The game CRC, save and load of the partition manager.
//!
//! The CRC covers each cell's per-player shroud counters and its coordinates:
//! the shroud is part of what lockstep peers compare, so a "shroud cheater"
//! desyncs. The save format is version 2: the cell size, the cell count, the
//! shroud of every cell (version 1 each), and the queue of pending unlooks.
//!
//! # Translation choices
//!
//! - `SC_INVALID_DATA` (upstream throws `SaveCode`, not `XferStatus`) is
//!   [`XferStatus::InvalidParameters`] here: a save made with a different cell
//!   size or cell count is refused.
//! - After a **load**, upstream sets `m_updatedSinceLastReset = FALSE` and calls
//!   `refreshShroudForLocalPlayer()`, which needs the local player. The
//!   `Snapshot` interface has no host, so the load sets the flag
//!   ([`PartitionManager::needs_shroud_refresh`]) and the caller runs
//!   [`PartitionManager::refresh_shroud_for_local_player`] when it finishes the
//!   load.

use super::cell::{PartitionCell, ShroudLevel, SightingInfo};
use super::manager::PartitionManager;
use crate::common::game_common::{PlayerMaskType, MAX_PLAYER_COUNT};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// The 64 bytes of `m_shroudLevel` as `xferUser` sees them.
fn shroud_bytes(levels: &[ShroudLevel; MAX_PLAYER_COUNT as usize]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 * MAX_PLAYER_COUNT as usize);
    for l in levels {
        v.extend_from_slice(&l.current_shroud.to_le_bytes());
        v.extend_from_slice(&l.active_shroud_level.to_le_bytes());
    }
    v
}

fn set_shroud_from_bytes(levels: &mut [ShroudLevel; MAX_PLAYER_COUNT as usize], b: &[u8]) {
    for (i, l) in levels.iter_mut().enumerate() {
        l.current_shroud = i16::from_le_bytes([b[4 * i], b[4 * i + 1]]);
        l.active_shroud_level = i16::from_le_bytes([b[4 * i + 2], b[4 * i + 3]]);
    }
}

impl Snapshot for PartitionCell {
    /// `PartitionCell::crc`: the shroud levels, then the coordinates.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut b = shroud_bytes(&self.shroud_level);
        xfer.xfer_user(&mut b)?;
        let mut x = self.cell_x.to_le_bytes();
        xfer.xfer_user(&mut x)?;
        let mut y = self.cell_y.to_le_bytes();
        xfer.xfer_user(&mut y)?;
        Ok(())
    }

    /// `PartitionCell::xfer`, version 1: the shroud levels.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        // xfer shroud data
        let mut b = shroud_bytes(&self.shroud_level);
        xfer.xfer_user(&mut b)?;
        set_shroud_from_bytes(&mut self.shroud_level, &b);
        Ok(())
    }

    /// `PartitionCell::loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}

impl Snapshot for SightingInfo {
    /// `SightingInfo::crc`: empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `SightingInfo::xfer`, version 1.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        // where
        xfer.xfer_coord3d(&mut self.where_)?;

        // how far
        xfer.xfer_real(&mut self.how_far)?;

        // for whom
        let mut m = self.for_whom.to_le_bytes();
        xfer.xfer_user(&mut m)?;
        self.for_whom = PlayerMaskType::from_le_bytes(m);

        // how much
        xfer.xfer_unsigned_int(&mut self.data)?;
        Ok(())
    }

    /// `SightingInfo::loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}

impl Snapshot for PartitionManager {
    /// `PartitionManager::crc`: every cell's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        for cell in &mut self.cells {
            cell.crc(xfer)?;
        }
        Ok(())
    }

    /// `PartitionManager::xfer`. Version 1: initial. 2: the pending unlooks.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        // cell size information
        let mut cell_size = self.cell_size;
        xfer.xfer_real(&mut cell_size)?;

        // cell size should match what we have allocated in the partition manager
        if cell_size != self.cell_size {
            return Err(XferStatus::InvalidParameters); // "Partition cell size has changed, this save game file is invalid"
        }

        // cell count
        let mut total_cell_count = self.total_cell_count;
        xfer.xfer_int(&mut total_cell_count)?;

        // total cell count better match what we have allocated for cells in the partition manager
        if total_cell_count != self.total_cell_count {
            return Err(XferStatus::InvalidParameters); // "Partition total cell count mismatch"
        }

        // xfer each cell information
        for i in 0..total_cell_count.max(0) as usize {
            xfer.xfer_snapshot(&mut self.cells[i])?;
        }

        // when loading tell the partition manager to rethink and refresh all shroud information
        if xfer.get_xfer_mode() == XferMode::Load {
            // tell partition manager to re-evaluate shroud things when next asked
            self.updated_since_last_reset = false;

            // refresh the shroud for the local player (done by the caller, see the module docs)
            self.needs_shroud_refresh = true;
        }

        if version >= 2 {
            let mut queue_size = self.pending_undo_shroud_reveals.len() as i32;
            xfer.xfer_int(&mut queue_size)?;
            // On save, I just wrote the number of elements. On load, the first line above was a decoy,
            // and the second found out the size.

            if xfer.get_xfer_mode() == XferMode::Load {
                // (there may already be entries: a setTeam call per sub-team during load queues unlooks)
                for _ in 0..queue_size.max(0) {
                    let mut new_info = SightingInfo::default();
                    xfer.xfer_snapshot(&mut new_info)?;
                    self.pending_undo_shroud_reveals.push_back(new_info);
                }
            } else {
                // On save, loop through the queue once without changing it.
                for i in 0..queue_size.max(0) as usize {
                    let mut save_info = self.pending_undo_shroud_reveals[i];
                    xfer.xfer_snapshot(&mut save_info)?;
                }
            }
        }
        // else: Version 1 save games will just not have any SightingInfos in the queue to be undone.
        Ok(())
    }

    /// `PartitionManager::loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}
