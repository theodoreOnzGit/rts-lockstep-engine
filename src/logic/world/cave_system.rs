// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/CaveSystem.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/CaveSystem.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   CaveSystem.cpp: Graham Smallwood, July 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CaveSystem` (`TheCaveSystem`): "system responsible for keeping track of
//! all cave systems on the map". Each cave index owns one
//! [`TunnelTracker`]; the player's own tunnel network is a tracker too, held
//! by the `Player`.
//!
//! # Translation choices
//!
//! - `std::vector<TunnelTracker*>` is `Vec<Option<TunnelTracker>>`: "a vector
//!   of pointers where the indexes are known by others, so it can have NULLs
//!   in it to keep position".
//! - **Negative indices.** Upstream compares `Int` with `size_t`, so a
//!   negative index is a huge unsigned number: [`CaveSystem::can_switch_index_to_index`]
//!   and [`CaveSystem::get_tunnel_tracker_for_cave_index`] treat it as off the
//!   end (no tracker), exactly as the comparison does; `registerNewCave` with
//!   one would push `NULL` until memory runs out, which is undefined
//!   behaviour and is [`CaveError::NegativeIndex`] here. (An index of many
//!   millions would be accepted and allocate that many slots, as upstream.)
//! - **A tracker is checked out to be used with the world**:
//!   [`World::with_cave_tracker`] takes it from its slot for the call.
//! - `xfer` on a save with a `NULL` slot throws `XFER_INVALID_PARAMETERS`
//!   (`XferSave::xferSnapshot(NULL)`) after writing the count and the
//!   trackers before it; here it returns that error at the same point. Any
//!   mode other than `XFER_SAVE` takes the load branch, as upstream's `else`.
//! - There is no `loadPostProcess` upstream (`{ }`): the trackers are
//!   post-processed one by one ([`CaveSystem::load_post_process_world`]).
//!
//! # Verified
//!
//! Against upstream's `CaveSystem.cpp`, compiled unchanged, by
//! `tests/differential_tunnel_tracker.rs` (results in `tunnel_tracker.rs`).

use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::world::game_logic::World;
use crate::logic::world::tunnel_tracker::{TunnelError, TunnelTracker};

/// An input for which upstream is undefined behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaveError {
    /// `registerNewCave(theIndex)` with `theIndex < 0`: the `while` loop would
    /// push `NULL` forever.
    NegativeIndex(i32),
}

/// `class CaveSystem`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaveSystem {
    /// `m_tunnelTrackerVector`.
    tunnel_trackers: Vec<Option<TunnelTracker>>,
}

impl CaveSystem {
    /// `CaveSystem::CaveSystem()`.
    pub fn new() -> CaveSystem {
        CaveSystem { tunnel_trackers: Vec::new() }
    }

    /// `init()`: does nothing.
    pub fn init(&mut self) {}

    /// `reset()`: every tracker is deleted and the vector emptied.
    pub fn reset(&mut self) {
        self.tunnel_trackers.clear();
    }

    /// `update()`: does nothing.
    pub fn update(&mut self) {}

    /// The number of slots (`m_tunnelTrackerVector.size()`).
    pub fn len(&self) -> usize {
        self.tunnel_trackers.len()
    }

    /// Whether there are no slots.
    pub fn is_empty(&self) -> bool {
        self.tunnel_trackers.is_empty()
    }

    /// The slot at `index`, as `size() > index` sees it (a negative index is
    /// past the end).
    fn slot(&self, index: i32) -> Option<&Option<TunnelTracker>> {
        if index < 0 {
            return None;
        }
        self.tunnel_trackers.get(index as usize)
    }

    /// `canSwitchIndexToIndex(oldIndex, newIndex)`: "if either Index has guys
    /// in it, no, you can't". When permission is granted, the caller must
    /// unregister and re-register with the new number.
    pub fn can_switch_index_to_index(&self, old_index: i32, new_index: i32) -> bool {
        if let Some(Some(old)) = self.slot(old_index) {
            if old.get_contain_count() > 0 {
                return false;
            }
        }
        if let Some(Some(new)) = self.slot(new_index) {
            if new.get_contain_count() > 0 {
                return false;
            }
        }
        // both are either empty or non-existent, so go ahead
        true
    }

    /// `registerNewCave(theIndex)`: "all caves are born with a default index,
    /// which could be new". Fills `NULL` slots up to the index and makes a
    /// tracker at it, or re-creates a slot that was `NULL`.
    pub fn register_new_cave(&mut self, index: i32) -> Result<(), CaveError> {
        if index < 0 {
            return Err(CaveError::NegativeIndex(index));
        }
        let index = index as usize;
        let mut need_to_create = false;
        if index >= self.tunnel_trackers.len() {
            // "You are new and off the edge, so I will fill NULLs up to you and then make a newTracker at that spot"
            while index >= self.tunnel_trackers.len() {
                self.tunnel_trackers.push(None);
            }
            need_to_create = true;
        } else if self.tunnel_trackers[index].is_none() {
            // "else you either exist or have existed, so I will either let things be or re-create that slot"
            need_to_create = true;
        }
        if need_to_create {
            self.tunnel_trackers[index] = Some(TunnelTracker::new());
        }
        Ok(())
    }

    /// `unregisterCave(theIndex)`: "doesn't need to do a thing. ContainModule
    /// logic knows how to say goodbye, and a TunnelTracker knows how to exist
    /// while having no entry points."
    pub fn unregister_cave(&mut self, _index: i32) {}

    /// `getTunnelTrackerForCaveIndex(theIndex)`; `None` is upstream's `NULL`
    /// ("No one should be interested in a sub-cave that doesn't exist").
    pub fn get_tunnel_tracker_for_cave_index(&self, index: i32) -> Option<&TunnelTracker> {
        self.slot(index).and_then(|s| s.as_ref())
    }

    /// [`CaveSystem::get_tunnel_tracker_for_cave_index`] for a caller that changes it.
    pub fn get_tunnel_tracker_for_cave_index_mut(&mut self, index: i32) -> Option<&mut TunnelTracker> {
        if index < 0 {
            return None;
        }
        self.tunnel_trackers.get_mut(index as usize).and_then(|s| s.as_mut())
    }

    /// The post-process phase after a load: each tracker's, in index order.
    pub fn load_post_process_world(&mut self, world: &mut World) -> Result<(), TunnelError> {
        for i in 0..self.tunnel_trackers.len() {
            if let Some(mut t) = self.tunnel_trackers[i].take() {
                let r = t.load_post_process_world(world);
                self.tunnel_trackers[i] = Some(t);
                r?;
            }
        }
        Ok(())
    }
}

impl World {
    /// Runs `f` on the tracker of cave `index`, checked out of its slot so
    /// that `f` has the world (see the module docs). `None` if there is no
    /// tracker there.
    pub fn with_cave_tracker<R>(&mut self, index: i32, f: impl FnOnce(&mut TunnelTracker, &mut World) -> R) -> Option<R> {
        if index < 0 {
            return None;
        }
        let mut tracker = self.cave_system.tunnel_trackers.get_mut(index as usize)?.take()?;
        let r = f(&mut tracker, self);
        if let Some(slot) = self.cave_system.tunnel_trackers.get_mut(index as usize) {
            *slot = Some(tracker);
        }
        Some(r)
    }
}

impl Snapshot for CaveSystem {
    /// `crc`: `{ }`.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Version 1: the number of slots as an `UnsignedShort`, then each
    /// tracker.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // tunnel tracker size and data
        let mut count = self.tunnel_trackers.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for slot in self.tunnel_trackers.iter_mut() {
                match slot {
                    Some(t) => xfer.xfer_snapshot(t)?,
                    // `XferSave::xferSnapshot(NULL)`
                    None => return Err(XferStatus::InvalidParameters),
                }
            }
        } else {
            // the list must be empty now
            if !self.tunnel_trackers.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            // read each item
            for _ in 0..count {
                let mut tracker = TunnelTracker::new();
                xfer.xfer_snapshot(&mut tracker)?;
                self.tunnel_trackers.push(Some(tracker));
            }
        }
        Ok(())
    }

    /// `loadPostProcess() { }`: the trackers need the world, see
    /// [`CaveSystem::load_post_process_world`].
    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    #[test]
    fn caves_register_in_place_and_off_the_end_with_nulls_between() {
        let mut c = CaveSystem::new();
        c.init();
        c.update();
        assert!(c.is_empty());
        c.register_new_cave(2).unwrap();
        assert_eq!(c.len(), 3);
        assert!(c.get_tunnel_tracker_for_cave_index(0).is_none() && c.get_tunnel_tracker_for_cave_index(2).is_some());
        c.register_new_cave(0).unwrap();
        c.get_tunnel_tracker_for_cave_index_mut(2).unwrap().add_to_contain_list(5);
        c.register_new_cave(2).unwrap();
        assert_eq!(c.get_tunnel_tracker_for_cave_index(2).unwrap().get_contain_count(), 1, "an existing slot is left alone");
        assert!(c.get_tunnel_tracker_for_cave_index(-1).is_none() && c.get_tunnel_tracker_for_cave_index(9).is_none());
        assert!(c.get_tunnel_tracker_for_cave_index_mut(-1).is_none());
        assert_eq!(c.register_new_cave(-3), Err(CaveError::NegativeIndex(-3)));
        c.unregister_cave(0);
        assert!(c.get_tunnel_tracker_for_cave_index(0).is_some());
    }

    #[test]
    fn a_switch_needs_both_caves_empty_or_missing() {
        let mut c = CaveSystem::new();
        c.register_new_cave(1).unwrap();
        assert!(c.can_switch_index_to_index(-1, 7));
        assert!(c.can_switch_index_to_index(1, 3));
        c.get_tunnel_tracker_for_cave_index_mut(1).unwrap().add_to_contain_list(5);
        assert!(!c.can_switch_index_to_index(1, 3));
        assert!(!c.can_switch_index_to_index(3, 1));
        c.reset();
        assert!(c.is_empty());
    }

    #[test]
    fn xfer_round_trips_and_a_null_slot_cannot_be_saved() {
        let mut c = CaveSystem::new();
        c.register_new_cave(1).unwrap();
        c.get_tunnel_tracker_for_cave_index_mut(1).unwrap().add_to_contain_list(4);
        let mut save = XferSave::new();
        save.open(b"c").unwrap();
        assert_eq!(c.xfer(&mut save), Err(XferStatus::InvalidParameters), "slot 0 is NULL");
        c.register_new_cave(0).unwrap();
        let mut save = XferSave::new();
        save.open(b"c").unwrap();
        c.xfer(&mut save).unwrap();
        save.close().unwrap();
        let bytes = save.take_bytes();
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"c").unwrap();
        let mut back = CaveSystem::new();
        back.xfer(&mut load).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.get_tunnel_tracker_for_cave_index(1).unwrap().get_contain_list_size(), 1);
        // loading into a non-empty system is refused
        let mut load = XferLoad::new(bytes);
        load.open(b"c").unwrap();
        assert_eq!(back.xfer(&mut load), Err(XferStatus::ListNotEmpty));
        let mut crc = crate::common::xfer_crc::XferCrc::new();
        back.crc(&mut crc).unwrap();
        back.load_post_process();
    }

    #[test]
    fn a_tracker_is_checked_out_with_the_world() {
        let mut w = World::new();
        w.cave_system.register_new_cave(0).unwrap();
        assert_eq!(w.with_cave_tracker(0, |t, _| t.on_tunnel_created(3)), Some(()));
        assert_eq!(w.cave_system.get_tunnel_tracker_for_cave_index(0).unwrap().friend_get_tunnel_count(), 1, "put back afterwards");
        assert!(w.with_cave_tracker(1, |_, _| ()).is_none() && w.with_cave_tracker(-1, |_, _| ()).is_none());
        let mut cave = std::mem::take(&mut w.cave_system);
        assert!(cave.load_post_process_world(&mut w).is_ok());
    }
}
