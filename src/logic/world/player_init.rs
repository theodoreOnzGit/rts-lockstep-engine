// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (`Player::initFromDict`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Slice 4 of `Player` (`p1w10-sonnet`): `Player::initFromDict`, the player a map's side dictionary describes, as a free
//! function over [`PlayerInitHost`]. Read `player_ops.rs` first.
//!
//! `initFromDict` reads the map's sides and skirmish sides (`TheSidesList`) and copies, qualifies and moves their script
//! lists, which are another unit (`logic::scripts`, verified by `differential_sides_scripts.rs`). The sides list and the
//! script lists are therefore the host's: [`PlayerInitHost`] names every question and every change `initFromDict` makes
//! (24 methods), and the layer-12 adapter answers them from the real [`SidesList`](crate::logic::sides_list::SidesList).
//!
//! # Upstream quirks kept
//!
//! - **`m_mpStartIndex` is read from the dictionary only after the human branch has used it** to qualify the Civilian
//!   skirmish side's name, so that branch uses the value left by the previous `initFromDict` (0 on a fresh player).
//! - **A human player takes the Civilian skirmish side's scripts** and the skirmish side loses them (`setScriptList(NULL)`).
//! - **`getBool(TheKey_playerIsSkirmish, &exists)` is tested by its value**, not by `exists`; a dictionary that holds the
//!   key as `false` is a non-skirmish player. A skirmish flag for which no skirmish side of the player's side exists makes
//!   the player human (`forceHuman`).
//! - **`m_playerNameKey` is the key of the dictionary's name, not of the skirmish-qualified name** the skirmish branch
//!   later gives `m_playerName`.
//! - **A player whose faction is not found is `init( NULL )`'d** (upstream asserts in debug builds and goes on).
//! - **`m_money.deposit( StartMoney )` plays the cash sound** (its `playSound` defaults to true) and records the income.
//!
//! # Where upstream is undefined
//!
//! A skirmish branch whose skirmish side was found but has no script list dereferences `NULL`
//! ([`PlayerError::Null`](super::player_ops::PlayerError::Null)).

use crate::common::dict::Dict;
use crate::common::game_common::{GameDifficulty, PlayerType};
use crate::common::name_key_generator::NameKeyType;
use crate::common::well_known_keys as wk;

use super::ids::PlayerIndex;
use super::player::*;
use super::player_objects::*;
use super::player_ops::*;
use super::player_xfer::PlayerXferHost;

/// A `ScriptList*` of the sides list.
pub type ScriptListRef = u32;

/// `MAX_GENERIC_SCRIPTS` (`Team.h`).
const MAX_GENERIC_SCRIPTS: usize = 16;

/// What `Player::initFromDict` (and `PlayerList::newGame`) ask of the sides list, the script lists and the other services.
pub trait PlayerInitHost: PlayerXferHost {
    /// `TheSidesList->getNumSides()`.
    fn sides_num_sides(&self) -> i32;
    /// `*TheSidesList->getSideInfo(i)->getDict()` (a copy).
    fn sides_side_dict(&self, i: i32) -> Dict;
    /// `TheSidesList->getSideInfo(i)->getScriptList()`.
    fn sides_side_scripts(&self, i: i32) -> Option<ScriptListRef>;
    /// `TheSidesList->getSideInfo(i)->setScriptList(l)`.
    fn sides_set_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>);
    /// `TheSidesList->getSideInfo(i)->getBuildList()`, taken and released (`getBuildList()` then `releaseBuildList()`).
    fn sides_take_side_build_list(&mut self, i: i32) -> Vec<crate::logic::sides_list::BuildListInfo>;
    /// `TheSidesList->getNumSkirmishSides()`.
    fn sides_num_skirmish_sides(&self) -> i32;
    /// `*TheSidesList->getSkirmishSideInfo(i)->getDict()` (a copy).
    fn sides_skirmish_side_dict(&self, i: i32) -> Dict;
    /// `TheSidesList->getSkirmishSideInfo(i)->getScriptList()`.
    fn sides_skirmish_side_scripts(&self, i: i32) -> Option<ScriptListRef>;
    /// `TheSidesList->getSkirmishSideInfo(i)->setScriptList(l)`.
    fn sides_set_skirmish_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>);
    /// `TheSidesList->getNumTeams()`.
    fn sides_num_teams(&self) -> i32;
    /// `*TheSidesList->getTeamInfo(i)->getDict()` (a copy).
    fn sides_team_dict(&self, i: i32) -> Dict;
    /// `TheSidesList->removeTeam(i)`.
    fn sides_remove_team(&mut self, i: i32);
    /// `TheSidesList->getNumSkirmishTeams()`.
    fn sides_num_skirmish_teams(&self) -> i32;
    /// `*TheSidesList->getSkirmishTeamInfo(i)->getDict()` (a copy).
    fn sides_skirmish_team_dict(&self, i: i32) -> Dict;
    /// `TheSidesList->findTeamInfo(name) != NULL`.
    fn sides_find_team_info(&mut self, name: &[u8]) -> bool;
    /// `TheSidesList->addTeam(&dict)`.
    fn sides_add_team(&mut self, d: &Dict);
    /// `list->duplicateAndQualify(qualifier, qualTemplatePlayerName, newPlayerName)`.
    fn script_list_duplicate_and_qualify(&mut self, list: ScriptListRef, qualifier: &[u8], qual_template_player_name: &[u8], new_player_name: &[u8]) -> ScriptListRef;
    /// `list->deleteInstance()`.
    fn script_list_delete(&mut self, list: ScriptListRef);
    /// `TheScriptEngine->getGlobalDifficulty()`.
    fn script_engine_global_difficulty(&self) -> GameDifficulty;
    /// `m_ai->setAIDifficulty(d)`.
    fn ai_set_difficulty(&mut self, p: PlayerIndex, d: GameDifficulty);
    /// `TheNetwork != NULL`.
    fn network_present(&self) -> bool;
    /// `TheTeamFactory->initFromSides(TheSidesList)`.
    fn team_factory_init_from_sides(&mut self) -> PResult<()>;
}

fn pl<H: PlayerInitHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

fn plm<H: PlayerInitHost>(h: &mut H, p: PlayerIndex) -> PResult<&mut Player> {
    h.players_mut().get_nth_mut(p).ok_or(PlayerError::NoPlayer)
}

/// `AsciiString::format("%s%d", s, n)`.
fn name_with_number(s: &[u8], n: i32) -> Vec<u8> {
    let mut v = s.to_vec();
    v.extend_from_slice(n.to_string().as_bytes());
    v
}

/// `NAMEKEY(name)`.
fn namekey<H: PlayerInitHost>(h: &mut H, name: &[u8]) -> NameKeyType {
    h.name_to_key(name)
}

/// A well-known key (`TheKey_x`): looked up the first time it is used.
pub(crate) fn wkey<H: PlayerInitHost>(h: &mut H, which: usize) -> NameKeyType {
    let (names, keys) = h.names();
    keys.key(which, names)
}

pub(crate) fn dict_ascii<H: PlayerInitHost>(h: &mut H, d: &Dict, which: usize) -> (Vec<u8>, bool) {
    let k = wkey(h, which);
    d.get_ascii_string(k)
}

/// The side a player template is for (`pt->getSide()`), if the dictionary's faction names a template.
fn faction_side<H: PlayerInitHost>(h: &mut H, d: &Dict) -> Option<Vec<u8>> {
    let (name, _) = dict_ascii(h, d, wk::PLAYER_FACTION);
    let key = namekey(h, &name);
    h.player_template_find(key).map(|t| h.player_template(t).side)
}

/// `Player::initFromDict(d)`.
pub fn player_init_from_dict<H: PlayerInitHost>(h: &mut H, p: PlayerIndex, d: &Dict) -> PResult<()> {
    let (tmplname, _) = dict_ascii(h, d, wk::PLAYER_FACTION);
    let key = namekey(h, &tmplname);
    let pt = if plant(501) { None } else { h.player_template_find(key) };
    player_init(h, p, pt)?;

    let k = wkey(h, wk::PLAYER_DISPLAY_NAME);
    let (display, _) = d.get_unicode_string(k);
    let mut display_name = crate::common::unicode_string::UnicodeString::default();
    display_name.set(&display).map_err(|_| PlayerError::Xfer(crate::common::xfer::XferStatus::InvalidParameters))?;
    if !plant(502) { plm(h, p)?.display_name = display_name; }
    let (pname, _) = dict_ascii(h, d, wk::PLAYER_NAME);
    if !plant(503) { plm(h, p)?.player_name = pname.clone(); }
    let name_key = namekey(h, &pname);
    plm(h, p)?.name_key = name_key.0 as i32 + plant(504) as i32;

    let mut skirmish = false;
    let mut force_human = false;
    let k = wkey(h, wk::PLAYER_IS_SKIRMISH);
    if d.get_bool(k).0 != plant(505) {
        // srj sez: check to ensure the map actually has a skirmish player...
        let side = pl(h, p)?.side.clone();
        for sp_idx in 0..h.sides_num_skirmish_sides() {
            let sd = h.sides_skirmish_side_dict(sp_idx);
            if faction_side(h, &sd).is_some_and(|s| (s == side) != plant(506)) {
                skirmish = true;
                break;
            }
        }
        if !skirmish && !plant(507) {
            force_human = true;
        }
    }

    let k = wkey(h, wk::PLAYER_IS_HUMAN);
    if d.get_bool(k).0 || (force_human && !plant(508)) {
        player_set_player_type(h, p, PlayerType::Human, false)?;
        let k = wkey(h, wk::PLAYER_IS_PREORDER);
        if d.get_bool(k).0 && !plant(509) {
            plm(h, p)?.is_preorder = true;
        }
        if h.sides_num_skirmish_sides() > if plant(510) { 1 } else { 0 } {
            // Human player gets scripts from CIVILIAN player.
            let my_side: &[u8] = if plant(511) { b"civilian" } else { b"Civilian" };
            let mut found: Option<(i32, Vec<u8>)> = None;
            for i in 0..h.sides_num_skirmish_sides() {
                let sd = h.sides_skirmish_side_dict(i);
                if faction_side(h, &sd).is_some_and(|s| s == my_side) {
                    let (n, _) = dict_ascii(h, &sd, wk::PLAYER_NAME);
                    let mp = if plant(512) { 0 } else { pl(h, p)?.mp_start_index };
                    found = Some((i, name_with_number(&n, mp)));
                    break;
                }
            }
            if let Some((i, qual_template_player_name)) = found {
                if let Some(list) = h.sides_skirmish_side_scripts(i) {
                    let qualifier = name_with_number(b"", pl(h, p)?.mp_start_index);
                    let scripts = h.script_list_duplicate_and_qualify(list, &qualifier, &qual_template_player_name, &pname);
                    if let Some(old) = h.sides_side_scripts(p) {
                        if !plant(513) {
                            h.script_list_delete(old);
                        }
                    }
                    h.sides_set_side_scripts(p, Some(scripts));
                    if !plant(514) { h.script_list_delete(list); }
                    if !plant(515) { h.sides_set_skirmish_side_scripts(i, None); }
                }
            }
        }
        skirmish = plant(516);
    } else {
        player_set_player_type(h, p, PlayerType::Computer, skirmish)?;
    }
    let k = wkey(h, wk::MULTIPLAYER_START_INDEX);
    plm(h, p)?.mp_start_index = d.get_int(k).0 + plant(517) as i32;
    if skirmish {
        // Copy and qualify scripts, and teams.
        let my_side = pl(h, p)?.side.clone();
        let mut found: Option<(i32, Vec<u8>)> = None;
        for skirmish_ndx in 0..h.sides_num_skirmish_sides() {
            let sd = h.sides_skirmish_side_dict(skirmish_ndx);
            if faction_side(h, &sd).is_some_and(|s| s == my_side) {
                let (n, _) = dict_ascii(h, &sd, wk::PLAYER_NAME);
                let mp = pl(h, p)?.mp_start_index;
                found = Some((skirmish_ndx, name_with_number(&n, mp)));
                break;
            }
        }
        let k = wkey(h, wk::SKIRMISH_DIFFICULTY);
        let (diff_int, exists) = d.get_int(k);
        let mut difficulty = h.script_engine_global_difficulty();
        if exists && !plant(518) {
            difficulty = difficulty_from_int(diff_int);
        }
        if pl(h, p)?.ai.is_some() != plant(519) {
            h.ai_set_difficulty(p, difficulty);
        }

        if let Some((skirmish_ndx, qual_template_player_name)) = found {
            if !plant(520) { plm(h, p)?.player_name = qual_template_player_name.clone(); }
            let qualifier = name_with_number(b"", pl(h, p)?.mp_start_index);
            let list = h.sides_skirmish_side_scripts(skirmish_ndx).ok_or(PlayerError::Null)?;
            let scripts = h.script_list_duplicate_and_qualify(list, &qualifier, &qual_template_player_name, &pname);
            if let Some(old) = h.sides_side_scripts(p) {
                h.script_list_delete(old);
            }
            h.sides_set_side_scripts(p, Some(scripts));
            let mut i = 0;
            while i < h.sides_num_teams() {
                let td = h.sides_team_dict(i);
                if (dict_ascii(h, &td, wk::TEAM_OWNER).0 == pname) != plant(522) {
                    // Remove any teams in this player.
                    h.sides_remove_team(i);
                    if !plant(521) {
                        i -= 1;
                    }
                }
                i += 1;
            }
            // Now add teams.
            let sd = h.sides_skirmish_side_dict(skirmish_ndx);
            let original_player_name = dict_ascii(h, &sd, wk::PLAYER_NAME).0;
            for i in 0..h.sides_num_skirmish_teams() {
                let team_dict = h.sides_skirmish_team_dict(i);
                if (dict_ascii(h, &team_dict, wk::TEAM_OWNER).0 != original_player_name) != plant(523) {
                    continue;
                }
                let mut team_dict = team_dict;
                let mp = pl(h, p)?.mp_start_index;
                let team_name = dict_ascii(h, &team_dict, wk::TEAM_NAME).0;
                let new_name = name_with_number(&team_name, mp);
                if h.sides_find_team_info(&new_name) && !plant(524) {
                    continue;
                }
                let k = wkey(h, wk::TEAM_OWNER);
                if !plant(525) { team_dict.set_ascii_string(k, &pname).map_err(|_| PlayerError::Null)?; }
                let k = wkey(h, wk::TEAM_NAME);
                if !plant(526) { team_dict.set_ascii_string(k, &new_name).map_err(|_| PlayerError::Null)?; }
                // qualify scripts.
                for which in [
                    wk::TEAM_ON_CREATE_SCRIPT,
                    wk::TEAM_ON_IDLE_SCRIPT,
                    wk::TEAM_ON_UNIT_DESTROYED_SCRIPT,
                    wk::TEAM_ON_DESTROYED_SCRIPT,
                    wk::TEAM_ENEMY_SIGHTED_SCRIPT,
                    wk::TEAM_ALL_CLEAR_SCRIPT,
                    wk::TEAM_PRODUCTION_CONDITION,
                ] {
                    let (tmp, exists) = dict_ascii(h, &team_dict, which);
                    if exists && (!tmp.is_empty() || plant(527)) {
                        let nn = name_with_number(&tmp, mp);
                        let k = wkey(h, which);
                        team_dict.set_ascii_string(k, &nn).map_err(|_| PlayerError::Null)?;
                    }
                }
                // Now do the TheKey_teamGenericScriptHookN (where N can be from 0 to 15.)
                for j in 0..MAX_GENERIC_SCRIPTS - plant(528) as usize {
                    let base = {
                        let k = wkey(h, wk::TEAM_GENERIC_SCRIPT_HOOK);
                        h.names().0.key_to_name(k).to_vec()
                    };
                    let key_name = name_with_number(&base, j as i32);
                    let k = namekey(h, &key_name);
                    let (tmp, exists) = team_dict.get_ascii_string(k);
                    if exists && !tmp.is_empty() {
                        let nn = name_with_number(&tmp, mp);
                        team_dict.set_ascii_string(k, &nn).map_err(|_| PlayerError::Null)?;
                    }
                }
                // Done. Now add the team.
                if !plant(529) { h.sides_add_team(&team_dict); }
            }
        }
    }
    plm(h, p)?.resource_gathering_manager = if plant(530) { None } else { Some(Default::default()) };
    plm(h, p)?.tunnel_system = if plant(531) { None } else { Some(Default::default()) };

    {
        let mut hc = pl_handicap(h, p)?;
        let (names, _) = h.names();
        if !plant(532) { hc.read_from_dict(names, d); }
        plm(h, p)?.handicap = hc;
    }

    // @todo Ack! the todo in PlayerList::reset() mentioning the need for a Player::reset() really needs to get done.
    {
        let me = plm(h, p)?;
        if !plant(533) { me.player_relations.map.clear(); }
        if !plant(534) { me.team_relations.map.clear(); }
        for b in me.attacked_by.iter_mut() {
            *b = plant(535);
        }
    }

    let k = wkey(h, wk::PLAYER_COLOR);
    let (c, exists) = d.get_int(k);
    if exists {
        let me = plm(h, p)?;
        me.color = (c as u32) | if plant(537) { 0 } else { 0xff00_0000 };
        me.night_color = if plant(536) { 0 } else { me.color };
    }

    let k = wkey(h, wk::PLAYER_NIGHT_COLOR);
    let (c, exists) = d.get_int(k);
    if exists {
        plm(h, p)?.night_color = (c as u32) | if plant(538) { 0 } else { 0xff00_0000 };
    }

    let k = wkey(h, wk::PLAYER_START_MONEY);
    let (m, exists) = d.get_int(k);
    if exists {
        player_money_deposit(h, p, m as u32, !plant(539))?;
    }

    {
        let me = plm(h, p)?;
        if !plant(540) { me.squads = vec![Squad::default(); NUM_HOTKEY_SQUADS]; }
        if !plant(541) { me.current_selection = Some(Squad::default()); }
    }
    Ok(())
}

fn pl_handicap<H: PlayerInitHost>(h: &H, p: PlayerIndex) -> PResult<crate::common::handicap::Handicap> {
    Ok(pl(h, p)?.handicap.clone())
}

/// `(GameDifficulty) diffInt`: the enum holds any int upstream; the three it names are what the Rust enum holds.
fn difficulty_from_int(v: i32) -> GameDifficulty {
    match v {
        0 => GameDifficulty::Easy,
        1 => GameDifficulty::Normal,
        _ => GameDifficulty::Hard,
    }
}
