//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 cases per function (test: tests/differential_draft_partition.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp (PartitionData::getControllingPlayerIndex,
//!   friend_setShroudednessPrevious, PartitionContactList::resetContactList)
//!
//! Sweep 2 findings for this file:
//! - `CellOutwardIterator::nextCell` (PartitionManager.cpp:1085): excluded, in the `#else` of `#ifdef FASTER_GCO`;
//!   PartitionManager.h:75 defines `FASTER_GCO`, so the class is not compiled (the verified manager says so).
//! - `PartitionCell::validateCoiList` (:1488): excluded, `#ifdef _DEBUG`, and only `DEBUG_ASSERTCRASH`s.
//! - `PartitionData::calcMaxCoiForObject` (:2179): translated, the call to `calc_max_coi_for_shape` with the object's
//!   geometry (`crate::logic::world::partition::manager`, manager.rs:667).
//! - `PartitionManager::getPMStats` (:2655): excluded, `#ifdef DUMP_PERF_STATS` (`_DEBUG`/`_INTERNAL` only).

use crate::common::game_common::{ObjectShroudStatus, MAX_PLAYER_COUNT};
use crate::logic::modules::plant;

/// `PartitionData::getControllingPlayerIndex()` (PartitionManager.cpp:1582): the index of the object's controlling
/// player. Upstream asserts it is in `0..MAX_PLAYER_COUNT` and, with no controlling player, `throw ERROR_BUG`
/// (here `None`, which the caller must treat as that error).
pub fn get_controlling_player_index(controlling_player_index: Option<i32>) -> Option<i32> {
    match controlling_player_index {
        Some(0) if plant(15698) => None,
        Some(player_index) => {
            debug_assert!(player_index >= 0 && player_index < MAX_PLAYER_COUNT, "bad playerIndex");
            Some(player_index)
        }
        None => None, // DEBUG_CRASH(("this should never happen")); throw ERROR_BUG;
    }
}

/// `PartitionData::friend_setShroudednessPrevious(playerIndex, status)` (PartitionManager.cpp:1599).
///
/// `m_shroudednessPrevious[playerIndex] = status`; "if our status is anything but shrouded, assume we've never been seen
/// by the player" (`m_everSeenByPlayer[playerIndex] = status != OBJECTSHROUD_SHROUDED`). The `#ifndef
/// DISABLE_INVALID_PREVENTION` block after it is not compiled: PartitionManager.cpp:109 defines
/// `DISABLE_INVALID_PREVENTION`.
pub fn friend_set_shroudedness_previous(
    shroudedness_previous: &mut [ObjectShroudStatus; MAX_PLAYER_COUNT as usize],
    ever_seen_by_player: &mut [bool; MAX_PLAYER_COUNT as usize],
    player_index: usize,
    status: ObjectShroudStatus,
) {
    if !plant(15701) {
        shroudedness_previous[player_index] = status;
    }
    ever_seen_by_player[player_index] = if plant(15699) {
        status == ObjectShroudStatus::Shrouded
    } else if plant(15700) {
        true
    } else if plant(15705) {
        status != ObjectShroudStatus::InvalidButPreviousValid
    } else {
        status != ObjectShroudStatus::Shrouded
    };
}

/// `PartitionContactList::resetContactList()` (PartitionManager.cpp:2467): deletes every node of the list and clears the
/// hash table (`memset(m_contactHash, 0, ..)`), so that the list is empty. `nodes` is `m_contactList`'s nodes and `heads`
/// `m_contactHash`, as the verified manager's private `ContactList` keeps them.
pub fn reset_contact_list<N>(nodes: &mut Vec<N>, heads: &mut [Option<usize>]) {
    // for (cd = m_contactList; cd; cd = cdnext) { cdnext = cd->m_next; cd->deleteInstance(); }
    if !plant(15703) {
        nodes.clear();
    }
    // memset(m_contactHash, 0, sizeof(m_contactHash)); m_contactList = NULL;
    if plant(15704) {
        return;
    }
    let n = if plant(15702) { 32 } else { heads.len() };
    for head in heads.iter_mut().take(n) {
        *head = None;
    }
}
