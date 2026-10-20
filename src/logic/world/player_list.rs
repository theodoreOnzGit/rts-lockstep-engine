// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/PlayerList.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PlayerList` (`p1w10-sonnet`): the functions of the list of sixteen players, as free functions over [`PlayerInitHost`].
//! The list itself is [`PlayerArena`](super::player::PlayerArena) (data and indexing); this file is what `PlayerList.cpp`
//! does with it: `init`, `reset`, `newGame`, `update`, `newMap`, `updateTeamStates`, `teamAboutToBeDeleted`,
//! `setLocalPlayer`, `validateTeam`, `getPlayerFromMask`, `getEachPlayerFromMask`, `getPlayersWithRelationship`, `crc`
//! and `xfer`.
//!
//! # Upstream quirks kept
//!
//! - **`newGame` leaves a side whose player name is empty to the neutral player**, and gives every other side the next
//!   slot (`m_players[m_playerCount++]`); a map with more than 16 sides indexes past the table
//!   ([`PlayerError::NoPlayer`](super::player_ops::PlayerError::NoPlayer)).
//! - **`multiplayerIsLocal` is tested by its value**, as `playerIsSkirmish` is.
//! - **A side whose name finds no player in the second loop** (the neutral side: its name is empty) dereferences `NULL`
//!   at `p->setPlayerRelationship`; kept as [`PlayerError::Null`](super::player_ops::PlayerError::Null) -- upstream's maps
//!   always put the neutral side first and its name key is the empty name's, which the neutral player has.
//! - **`getPlayersWithRelationship` skips the source player in the loop** (it was added by the `ALLOW_SAME_PLAYER` test) and
//!   walks `getPlayerCount()` players, not 16; a player's relationship is the one it has to the other's *default team*.
//! - **`xfer` refuses a file whose player count differs** (`SC_INVALID_DATA`) and `crc` writes the count and the players'
//!   `crc`.
//! - **`validateTeam` falls back to the neutral player's default team** for a name that is neither a team nor a player.

use crate::common::dict::Dict;
use crate::common::game_common::{PlayerMaskType, PlayerType, Relationship};
use crate::common::well_known_keys as wk;
use crate::common::xfer::{Xfer, XferMode};

use super::ids::{PlayerIndex, TeamId};
use super::player::*;
use super::player_init::*;
use super::player_objects::*;
use super::player_ops::*;
use super::player_xfer::*;
use super::team_ops::*;

/// `AllowPlayerRelationship` (`PlayerList.h`).
pub const ALLOW_SAME_PLAYER: u32 = 0x01;
/// `ALLOW_ALLIES`.
pub const ALLOW_ALLIES: u32 = 0x02;
/// `ALLOW_ENEMIES`.
pub const ALLOW_ENEMIES: u32 = 0x04;
/// `ALLOW_NEUTRAL`.
pub const ALLOW_NEUTRAL: u32 = 0x08;

fn pl<H: PlayerInitHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

/// `PlayerList::setLocalPlayer(player)`: a null player selects the neutral player; a change tells the old local player it
/// is no longer local, then the new one that it is.
pub fn player_list_set_local_player<H: PlayerInitHost>(h: &mut H, player: Option<PlayerIndex>) -> PResult<()> {
    if let Some(change) = h.players_mut().set_local_player(player) {
        if let Some(lost) = change.lost {
            player_becoming_local_player(h, lost, false)?;
        }
        player_becoming_local_player(h, change.gained, true)?;
    }
    Ok(())
}

/// `PlayerList::init()`: one player, all sixteen `init( NULL )`, the neutral player local.
pub fn player_list_init<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    h.players_mut().set_player_count(1);
    for i in 0..MAX_PLAYER_COUNT {
        player_init(h, i, None)?;
    }
    // call setLocalPlayer so that becomingLocalPlayer() gets called appropriately
    player_list_set_local_player(h, Some(0))
}

/// `PlayerList::reset()`.
pub fn player_list_reset<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    factory_clear(h)?; // cleans up energy, among other things
    player_list_init(h)
}

/// `PlayerList::update()`.
pub fn player_list_update<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    for i in 0..MAX_PLAYER_COUNT {
        player_update(h, i)?;
    }
    Ok(())
}

/// `PlayerList::newMap()`.
pub fn player_list_new_map<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    for i in 0..MAX_PLAYER_COUNT {
        player_new_map(h, i)?;
    }
    Ok(())
}

/// `PlayerList::teamAboutToBeDeleted(team)`.
pub fn player_list_team_about_to_be_deleted<H: PlayerInitHost>(h: &mut H, team: TeamId) -> PResult<()> {
    for i in 0..MAX_PLAYER_COUNT {
        player_remove_team_relationship(h, i, Some(team))?;
    }
    Ok(())
}

/// `PlayerList::updateTeamStates()`.
pub fn player_list_update_team_states<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    for i in 0..MAX_PLAYER_COUNT {
        player_update_team_states(h, i)?;
    }
    Ok(())
}

/// `PlayerList::validateTeam(owner)`.
pub fn player_list_validate_team<H: PlayerInitHost>(h: &mut H, owner: &[u8]) -> PResult<Option<TeamId>> {
    // owner could be a player or team. first, check team names.
    if let Some(t) = factory_find_team(h, owner)? {
        return Ok(Some(t));
    }
    // no team or player named that could be found
    Ok(pl(h, 0)?.default_team)
}

/// `PlayerList::getPlayerFromMask(mask)`.
pub fn player_list_get_player_from_mask<H: PlayerInitHost>(h: &H, mask: PlayerMaskType) -> Option<PlayerIndex> {
    (0..MAX_PLAYER_COUNT).find(|i| h.players().get_nth(*i).is_some_and(|p| (1u16 << p.player_index) == mask))
}

/// `PlayerList::getEachPlayerFromMask(maskToAdjust)`: the next player (in index order) whose bit is in the mask; its bit is
/// taken out of the mask; none empties the mask.
pub fn player_list_get_each_player_from_mask<H: PlayerInitHost>(h: &H, mask_to_adjust: &mut PlayerMaskType) -> Option<PlayerIndex> {
    for i in 0..MAX_PLAYER_COUNT {
        if let Some(p) = h.players().get_nth(i) {
            let m = 1u16 << p.player_index;
            if m & *mask_to_adjust != 0 {
                *mask_to_adjust &= !m;
                return Some(i);
            }
        }
    }
    *mask_to_adjust = 0;
    None
}

/// `PlayerList::getPlayersWithRelationship(srcPlayerIndex, allowedRelationships)`.
pub fn player_list_get_players_with_relationship<H: PlayerInitHost>(h: &H, src_player_index: PlayerIndex, allowed: u32) -> PResult<PlayerMaskType> {
    let mut ret_val: PlayerMaskType = 0;
    if allowed == 0 {
        return Ok(ret_val);
    }
    let Some(src) = h.players().get_nth(src_player_index) else { return Ok(ret_val) };
    let src_mask = 1u16 << src.player_index;
    if allowed & ALLOW_SAME_PLAYER != 0 {
        ret_val |= src_mask;
    }
    for i in 0..h.players().player_count() {
        let Some(player) = h.players().get_nth(i) else { continue };
        if player.player_index == src_player_index {
            continue;
        }
        let m = 1u16 << player.player_index;
        match player_get_relationship(h, src_player_index, player.default_team)? {
            Relationship::Enemies => {
                if allowed & ALLOW_ENEMIES != 0 {
                    ret_val |= m;
                }
            }
            Relationship::Allies => {
                if allowed & ALLOW_ALLIES != 0 {
                    ret_val |= m;
                }
            }
            Relationship::Neutral => {
                if allowed & ALLOW_NEUTRAL != 0 {
                    ret_val |= m;
                }
            }
        }
    }
    Ok(ret_val)
}

/// `PlayerList::newGame()`: called during `GameLogic::startNewGame()`.
pub fn player_list_new_game<H: PlayerInitHost>(h: &mut H) -> PResult<()> {
    factory_clear(h)?; // cleans up energy, among other things

    // first, re-init ourselves.
    player_list_init(h)?;

    // ok, now create the rest of players we need.
    let mut set_local = false;
    for i in 0..h.sides_num_sides() {
        let d: Dict = h.sides_side_dict(i);
        let (pname, _) = dict_ascii(h, &d, wk::PLAYER_NAME);
        if pname.is_empty() {
            continue; // it's neutral, which we've already done, so skip it.
        }

        let count = h.players().player_count();
        let p = count;
        h.players_mut().set_player_count(count + 1);
        h.players().get_nth(p).ok_or(PlayerError::NoPlayer)?;
        player_init_from_dict(h, p, &d)?;

        // Multiplayer override
        let k = wkey(h, wk::MULTIPLAYER_IS_LOCAL);
        if d.get_bool(k).0 {
            player_list_set_local_player(h, Some(p))?;
            set_local = true;
        }

        let k = wkey(h, wk::PLAYER_IS_HUMAN);
        if !set_local && !h.network_present() && d.get_bool(k).0 {
            player_list_set_local_player(h, Some(p))?;
            set_local = true;
        }

        // Set the build list. Build list is attached to player now, so release it from the side info.
        let list = h.sides_take_side_build_list(i);
        player_set_build_list(h, p, list)?;
    }

    if !set_local {
        for i in 0..h.sides_num_sides() {
            if h.players().get_nth(i).is_some() && i != 0 {
                player_set_player_type(h, i, PlayerType::Human, false)?;
                player_list_set_local_player(h, Some(i))?;
                set_local = true;
                break;
            }
            if h.players().get_nth(i).is_none() {
                return Err(PlayerError::Null);
            }
        }
    }
    let _ = set_local;

    // must reset teams *after* creating players.
    h.team_factory_init_from_sides()?;

    for i in 0..h.sides_num_sides() {
        let d: Dict = h.sides_side_dict(i);
        let (pname, _) = dict_ascii(h, &d, wk::PLAYER_NAME);
        let key = h.name_to_key(&pname);
        let p = find_player_with_name_key(h, key).ok_or(PlayerError::Null)?;

        for (which, rel) in [(wk::PLAYER_ENEMIES, Relationship::Enemies), (wk::PLAYER_ALLIES, Relationship::Allies)] {
            let (list, _) = dict_ascii(h, &d, which);
            let mut rest = crate::common::ascii_string::AsciiString::try_from_bytes(&list).map_err(|_| PlayerError::Null)?;
            let mut tok = crate::common::ascii_string::AsciiString::default();
            while rest.next_token(&mut tok, None).unwrap_or(false) {
                let key = h.name_to_key(tok.str());
                if let Some(p2) = find_player_with_name_key(h, key) {
                    player_set_player_relationship(h, p, Some(p2), rel)?;
                }
            }
        }

        // finally, make sure self & neutral are correct.
        player_set_player_relationship(h, p, Some(p), Relationship::Allies)?;
        if p != 0 {
            player_set_player_relationship(h, p, Some(0), Relationship::Neutral)?;
        }

        player_set_default_team(h, p)?;
    }
    Ok(())
}

/// `PlayerList::findPlayerWithNameKey(key)`: the first of the `getPlayerCount()` players with that name key.
pub fn find_player_with_name_key<H: PlayerInitHost>(h: &H, key: crate::common::name_key_generator::NameKeyType) -> Option<PlayerIndex> {
    let a = h.players();
    (0..a.player_count()).find(|i| a.get_nth(*i).is_some_and(|p| p.name_key == key.0 as i32))
}

/// `PlayerList::crc(xfer)`: the player count, then each player's `crc`.
pub fn player_list_crc<H: PlayerInitHost>(h: &mut H, xfer: &mut dyn Xfer) -> PResult<()> {
    let mut count = h.players().player_count();
    xfer.xfer_int(&mut count)?;
    for i in 0..h.players().player_count() {
        crc_nested(h, xfer, |h, x| player_crc(h, x, i))?;
    }
    Ok(())
}

/// `PlayerList::xfer(xfer)`, version 1.
pub fn player_list_xfer<H: PlayerInitHost>(h: &mut H, xfer: &mut dyn Xfer) -> PResult<()> {
    let current_version = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // xfer the player count
    let mut player_count = h.players().player_count();
    xfer.xfer_int(&mut player_count)?;

    // sanity, the player count read from the file should match our player count that was setup from the bare bones map
    // load since that data can't change during run time
    if player_count != h.players().player_count() {
        return Err(PlayerError::InvalidData);
    }

    // xfer each of the player data
    for i in 0..player_count {
        xfer_nested(h, xfer, |h, x| player_xfer(h, x, i))?;
    }
    let _ = XferMode::Save;
    Ok(())
}

/// `PlayerList::loadPostProcess()`: nothing.
pub fn player_list_load_post_process<H: PlayerInitHost>(_h: &mut H) {}
