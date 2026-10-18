// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/PlayerList.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp
//   (the constructor, getNthPlayer, findPlayerWithNameKey, init, setLocalPlayer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The player arena: `PlayerList`'s array of `MAX_PLAYER_COUNT` players, its
//! count, and the local player. **Players are not translated yet**: [`Player`]
//! is an empty shell that layer 7 fills in place.
//!
//! # What is kept from upstream
//!
//! - **Players are an array, indexed by player index.** `PlayerList` makes all
//!   `MAX_PLAYER_COUNT` (16) players in its constructor, whether or not the
//!   game uses them, and `getNthPlayer(i)` is `NULL` outside `0..16`
//!   ([`PlayerArena::get_nth`] returns `None`). Index 0 is the neutral player.
//! - **`m_playerCount` bounds name lookups, not indexing**:
//!   `findPlayerWithNameKey` walks `0..m_playerCount`; `update`, `newMap`,
//!   `updateTeamStates` walk all 16 ([`PlayerArena::iter`]).
//! - **The local player is a pointer that may be unset only before `init`**;
//!   `setLocalPlayer(NULL)` selects the neutral player.
//!   `becomingLocalPlayer(true/false)` (a notification to the UI) is the
//!   [`LocalPlayerChange`] that [`PlayerArena::set_local_player`] returns for
//!   the caller to send; the arena itself sends nothing.
//! - Everything else `Player` and `PlayerList` do (relationships, energy,
//!   money, the sides list, `xfer`) is layer 7.

use super::energy::Energy;
use super::mission_stats::MissionStats;
use super::score_keeper::ScoreKeeper;
use super::ids::PlayerIndex;

/// `MAX_PLAYER_COUNT` (`GameCommon.h`).
pub use crate::common::game_common::MAX_PLAYER_COUNT;

/// The shell of `Player` (layer 7 fills it in).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    /// `m_playerIndex`.
    pub player_index: PlayerIndex,
    /// `m_playerNameKey` (`NameKeyType`); 0 = `NAMEKEY_INVALID` until named.
    pub name_key: i32,
    // --- p1w6-energy ---
    /// `m_energy` (`Energy`): the power bookkeeping. `init(owner)` is `Player::init`'s.
    pub energy: Energy,
    /// `m_scoreKeeper` (`ScoreKeeper`): `Player::init` calls `reset(m_playerIndex)`.
    pub score_keeper: ScoreKeeper,
    /// `m_missionStats` (`MissionStats`).
    pub mission_stats: MissionStats,
    // --- end p1w6-energy ---
}

impl Player {
    /// `Player::Player(playerIndex)`: only the index is set here.
    pub fn new(player_index: PlayerIndex) -> Player {
        Player {
            player_index,
            name_key: 0,
            // --- p1w6-energy ---
            energy: Energy::new(),
            score_keeper: ScoreKeeper::new(),
            mission_stats: MissionStats::new(),
            // --- end p1w6-energy ---
        }
    }
}

/// What `setLocalPlayer` tells the caller to notify (`becomingLocalPlayer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalPlayerChange {
    /// `m_local->becomingLocalPlayer(false)`: the old local player, if any.
    pub lost: Option<PlayerIndex>,
    /// `player->becomingLocalPlayer(true)`.
    pub gained: PlayerIndex,
}

/// `PlayerList`.
#[derive(Clone, Debug)]
pub struct PlayerArena {
    /// `m_players[MAX_PLAYER_COUNT]`.
    players: Vec<Player>,
    /// `m_playerCount`.
    player_count: i32,
    /// `m_local`, by index.
    local: Option<PlayerIndex>,
}

impl Default for PlayerArena {
    fn default() -> Self {
        PlayerArena::new()
    }
}

impl PlayerArena {
    /// `PlayerList::PlayerList()`: 16 players, count 0, no local player.
    pub fn new() -> PlayerArena {
        PlayerArena { players: (0..MAX_PLAYER_COUNT).map(Player::new).collect(), player_count: 0, local: None }
    }

    /// `getNthPlayer(i)`: `None` outside `0..MAX_PLAYER_COUNT`.
    pub fn get_nth(&self, i: PlayerIndex) -> Option<&Player> {
        if !(0..MAX_PLAYER_COUNT).contains(&i) {
            return None;
        }
        self.players.get(i as usize)
    }

    /// `getNthPlayer(i)` for a caller that changes the player.
    pub fn get_nth_mut(&mut self, i: PlayerIndex) -> Option<&mut Player> {
        if !(0..MAX_PLAYER_COUNT).contains(&i) {
            return None;
        }
        self.players.get_mut(i as usize)
    }

    /// `getNeutralPlayer()`: player 0.
    pub fn neutral(&self) -> &Player {
        &self.players[0]
    }

    /// `getPlayerCount()`.
    pub fn player_count(&self) -> i32 {
        self.player_count
    }

    /// `m_playerCount = n` (`newGame` sets it as it adds the sides' players).
    pub fn set_player_count(&mut self, n: i32) {
        self.player_count = n.clamp(0, MAX_PLAYER_COUNT);
    }

    /// All `MAX_PLAYER_COUNT` players, in index order.
    pub fn iter(&self) -> impl Iterator<Item = &Player> {
        self.players.iter()
    }

    /// `findPlayerWithNameKey(key)`: the first of `0..m_playerCount` with the key.
    pub fn find_with_name_key(&self, key: i32) -> Option<&Player> {
        self.players.iter().take(self.player_count as usize).find(|p| p.name_key == key)
    }

    /// `getLocalPlayer()`: `None` before `init`.
    pub fn local(&self) -> Option<&Player> {
        self.local.and_then(|i| self.get_nth(i))
    }

    /// The local player's index.
    pub fn local_index(&self) -> Option<PlayerIndex> {
        self.local
    }

    /// `PlayerList::init()`: count 1, and the neutral player becomes local.
    /// Returns the notification `setLocalPlayer(m_players[0])` implies.
    pub fn init(&mut self) -> Option<LocalPlayerChange> {
        self.player_count = 1;
        self.set_local_player(Some(0))
    }

    /// `setLocalPlayer(player)`: `None` (upstream's `NULL`) selects neutral. A
    /// change returns what to notify; setting the current local player again
    /// returns `None`. An index outside the array also selects neutral (a
    /// pointer outside `m_players` is undefined upstream).
    pub fn set_local_player(&mut self, player: Option<PlayerIndex>) -> Option<LocalPlayerChange> {
        let idx = match player {
            Some(i) if (0..MAX_PLAYER_COUNT).contains(&i) => i,
            _ => 0,
        };
        if self.local == Some(idx) {
            return None;
        }
        let lost = self.local;
        self.local = Some(idx);
        Some(LocalPlayerChange { lost, gained: idx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_players_exist_from_the_start_and_indexing_is_bounded() {
        let a = PlayerArena::new();
        assert_eq!(a.iter().count(), 16);
        assert_eq!(a.get_nth(15).map(|p| p.player_index), Some(15));
        assert!(a.get_nth(16).is_none() && a.get_nth(-1).is_none());
        assert_eq!(a.player_count(), 0);
        assert!(a.local().is_none());
        assert_eq!(a.neutral().player_index, 0);
    }

    #[test]
    fn init_makes_neutral_local_and_null_selects_neutral() {
        let mut a = PlayerArena::new();
        assert_eq!(a.init(), Some(LocalPlayerChange { lost: None, gained: 0 }));
        assert_eq!(a.player_count(), 1);
        assert_eq!(a.set_local_player(Some(0)), None, "no change, no notification");
        assert_eq!(a.set_local_player(Some(3)), Some(LocalPlayerChange { lost: Some(0), gained: 3 }));
        assert_eq!(a.local().map(|p| p.player_index), Some(3));
        assert_eq!(a.set_local_player(None), Some(LocalPlayerChange { lost: Some(3), gained: 0 }));
        assert_eq!(a.set_local_player(Some(99)), None, "outside the array: neutral, already local");
    }

    #[test]
    fn name_key_lookup_stops_at_the_player_count() {
        let mut a = PlayerArena::new();
        a.get_nth_mut(2).unwrap().name_key = 77;
        a.set_player_count(2);
        assert!(a.find_with_name_key(77).is_none(), "player 2 is beyond m_playerCount");
        a.set_player_count(3);
        assert_eq!(a.find_with_name_key(77).map(|p| p.player_index), Some(2));
        a.set_player_count(99);
        assert_eq!(a.player_count(), 16);
    }
}
