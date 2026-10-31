// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`placeObjectAtPosition`, `placeNetworkBuildingsForPlayer`, `getLoadScreen`, `handleNameChange`,
//   `checkForDuplicateColors`, `populateRandomSideAndColor`, `populateRandomStartPosition`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The helpers `GameLogic::startNewGame` runs on the game's slots and players: duplicate colours cleared, random
//! factions, colours and start positions chosen, a renamed map object fixed, each network player's starting
//! buildings and units placed, and the load screen chosen (layer 12 batch L3a).
//!
//! # Shape
//!
//! The functions act on the game's [`GameInfo`] (its slots) and reach the rest through [`StartHost`]: the player
//! templates, the challenge generals, the multiplayer settings' colours, the map cache, the logic's random numbers,
//! waypoints, the terrain's height, the thing factory, objects, players, the pathfinder and the partition manager.
//!
//! # Upstream facts kept
//!
//! - `populateRandomSideAndColor` (`MORE_RANDOM`): a random faction is drawn from the templates with a starting
//!   building (and, in an old-factions-only game, an old faction; a general that starts locked is left out), after
//!   `GetGameLogicRandomSeed() % 7` throw-away draws; an empty list divides by zero. A random colour is drawn until
//!   one not taken comes up (an endless loop when every colour is taken).
//! - `populateRandomStartPosition`: the map is found case-blind in the map cache, and **dereferenced even when it is
//!   not there** (the distances read its waypoints); a start position already set marks itself taken with `posIdx >= 0
//!   || posIdx >= numPlayers` (a position of 8 or more is past the array). The first random player draws a free
//!   position; later ones without a placed team take the free position farthest (the sum of distances) from those
//!   taken, and teammates the free position nearest their team's first; observers then draw a position someone has
//!   taken (with no players at all, position 0; with players but none taken, for ever).
//! - `placeObjectAtPosition`: an unknown template makes `newObject` throw (nothing catches it).
//! - `placeNetworkBuildingsForPlayer`: no start waypoint or no starting building places nothing; the units go around
//!   the rally point (or just south of the building), each at a position the partition manager finds after updating.

use crate::common::base_type::Coord3D;
use crate::common::game_info::{GameInfo, MAX_SLOTS, PLAYERTEMPLATE_OBSERVER};
use crate::common::ini::{IniError, Undefined};
use crate::common::recorder::game_mode;
use crate::logic::modules::plant;

type R<T> = Result<T, IniError>;

/// `MAX_MP_STARTING_UNITS`.
pub const MAX_MP_STARTING_UNITS: i32 = 10;
/// How many draws an endless loop is allowed before it is reported (each draw advances the logic's seed, so the
/// loop cannot end once every value it accepts is gone; the bound only stops the translation).
const ENDLESS: u32 = 100_000;

fn undefined(u: Undefined) -> IniError {
    IniError::undefined(u)
}

/// A map in the map cache, as `populateRandomStartPosition` reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapStartInfo {
    /// `m_numPlayers`.
    pub num_players: i32,
    /// `m_waypoints` (by name).
    pub waypoints: Vec<(Vec<u8>, Coord3D)>,
}

/// Which load screen `getLoadScreen` makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadScreenKind {
    Shell,
    SinglePlayer,
    Challenge,
    MultiPlayer,
    GameSpy,
}

/// The world as the helpers reach it. Templates, objects and players are named by the world's handles.
pub trait StartHost {
    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    fn st_player_template_count(&self) -> i32;
    /// `getNthPlayerTemplate(i) != NULL`.
    fn st_player_template_exists(&self, i: i32) -> bool;
    /// The template's `getStartingBuilding()`.
    fn st_starting_building(&self, i: i32) -> Vec<u8>;
    /// The template's `getStartingUnit(n)`.
    fn st_starting_unit(&self, i: i32, n: i32) -> Vec<u8>;
    /// The template's `isOldFaction()`.
    fn st_is_old_faction(&self, i: i32) -> bool;
    /// `TheChallengeGenerals->getGeneralByTemplateName(template's name)`, and that general `!isStartingEnabled()`.
    fn st_general_starts_locked(&self, i: i32) -> bool;
    /// `TheMultiplayerSettings->getNumColors()`.
    fn st_num_colors(&self) -> i32;
    /// `TheMapCache->findMap(name)` of a name already lower-cased.
    fn st_find_map(&self, lower_name: &[u8]) -> Option<MapStartInfo>;
    /// `GameLogicRandomValue(lo, hi)`.
    fn st_random(&mut self, lo: i32, hi: i32) -> i32;
    /// `GetGameLogicRandomSeed()`.
    fn st_random_seed(&self) -> u32;
    /// `TheThingFactory->findTemplate(name)`.
    fn st_find_template(&self, name: &[u8]) -> Option<u32>;
    /// `findNamedWaypoint(name)`'s location.
    fn st_waypoint(&self, name: &[u8]) -> Option<Coord3D>;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn st_ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheThingFactory->newObject(template, player's default team)`: `None` when it makes none.
    fn st_new_object(&mut self, template: u32, player: i32) -> Option<u32>;
    /// The object's template's `getPlacementViewAngle()`.
    fn st_placement_view_angle(&self, obj: u32) -> f32;
    /// `obj->setOrientation(angle)`.
    fn st_set_orientation(&mut self, obj: u32, angle: f32);
    /// `obj->setPosition(pos)`.
    fn st_set_position(&mut self, obj: u32, pos: Coord3D);
    /// Each create module's `onBuildComplete()`.
    fn st_build_complete(&mut self, obj: u32);
    /// `player->getDefaultTeam() != NULL`, then its `setActive()`.
    fn st_activate_default_team(&mut self, player: i32);
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    fn st_add_to_pathfind_map(&mut self, obj: u32);
    /// `obj->getAIUpdateInterface() && !obj->isKindOf(KINDOF_IMMOBILE)`.
    fn st_is_mobile_ai(&self, obj: u32) -> bool;
    /// `TheAI->pathfinder()->adjustDestination(obj, its locomotor set, &pos)`: the adjusted position, or `None`.
    fn st_adjust_destination(&mut self, obj: u32, pos: Coord3D) -> Option<Coord3D>;
    /// `TheAI->pathfinder()->updateGoal(obj, &pos, LAYER_GROUND)`.
    fn st_update_goal(&mut self, obj: u32, pos: Coord3D);
    /// `player->onStructureCreated(NULL, obj)` then `onStructureConstructionComplete(NULL, obj, FALSE)`.
    fn st_structure_created(&mut self, player: i32, obj: u32);
    /// `player->onUnitCreated(NULL, obj)`.
    fn st_unit_created(&mut self, player: i32, obj: u32);
    /// The object's `getGeometryInfo().getBoundingSphereRadius()`.
    fn st_bounding_sphere_radius(&self, obj: u32) -> f32;
    /// `ThePartitionManager->update()`.
    fn st_partition_update(&mut self);
    /// `ThePartitionManager->findPositionAround(&pos, options with minRadius and maxRadius, &result)`.
    fn st_find_position_around(&mut self, pos: Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D>;
}

/// `AsciiString::toLower` (a byte of 0x80 or more is undefined, as `GameState`'s).
fn to_lower(s: &[u8]) -> R<Vec<u8>> {
    if s.iter().any(|&b| b >= 0x80) {
        return Err(undefined(Undefined::TolowerNonAscii));
    }
    Ok(s.to_ascii_lowercase())
}

/// `checkForDuplicateColors(game)`: from the last slot down, an occupied slot's chosen colour is kept only if no
/// other slot has it (random colours are left alone).
pub fn check_for_duplicate_colors(game: Option<&mut GameInfo>, h: &impl StartHost) {
    let Some(game) = game else {
        return;
    };
    for i in (0..MAX_SLOTS).rev() {
        if !game.slots[i].is_occupied() {
            continue;
        }
        let color = game.slots[i].color;
        if color < 0 || color >= h.st_num_colors() {
            continue;
        }
        game.slots[i].color = -1;
        if !game.is_color_taken(color, -1) && !plant(4021) {
            game.slots[i].color = color;
        }
    }
}

/// `populateRandomSideAndColor(game)`.
pub fn populate_random_side_and_color(game: Option<&mut GameInfo>, h: &mut impl StartHost) -> R<()> {
    let Some(game) = game else {
        return Ok(());
    };
    let count = h.st_player_template_count();
    let mut start_slots: Vec<i32> = Vec::new();
    for i in 0..count {
        if !h.st_player_template_exists(i) || h.st_starting_building(i).is_empty() {
            continue;
        }
        if game.old_factions_only && !h.st_is_old_faction(i) {
            continue;
        }
        if h.st_general_starts_locked(i) && !plant(4022) {
            continue;
        }
        start_slots.push(i);
    }
    for i in 0..MAX_SLOTS {
        if !game.slots[i].is_occupied() {
            continue;
        }
        let mut idx = game.slots[i].player_template;
        while idx != PLAYERTEMPLATE_OBSERVER && (idx < 0 || idx >= h.st_player_template_count()) {
            let silly = h.st_random_seed() % 7;
            for _ in 0..silly {
                h.st_random(0, 1);
            }
            let r = h.st_random(0, 1000);
            if start_slots.is_empty() {
                return Err(undefined(Undefined::ModuloByZero));
            }
            // an Int modulo a size_t: the draw is converted to unsigned
            idx = start_slots[(r as u32 as usize) % start_slots.len()];
            if !h.st_player_template_exists(idx) || h.st_starting_building(idx).is_empty() {
                idx = -1;
            } else {
                game.slots[i].set_player_template(idx);
            }
        }
        let mut color = game.slots[i].color;
        if color < 0 || color >= h.st_num_colors() {
            let mut draws = 0u32;
            while color == -1 {
                color = h.st_random(0, h.st_num_colors() - 1);
                if game.is_color_taken(color, -1) {
                    color = -1;
                }
                draws += 1;
                if draws > ENDLESS {
                    return Err(undefined(Undefined::EndlessLoop));
                }
            }
            game.slots[i].color = color;
        }
    }
    Ok(())
}

/// `sqrt(sqr(dx) + sqr(dy))` in `Real`s.
fn distance(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// `populateRandomStartPosition(game)`.
pub fn populate_random_start_position(game: Option<&mut GameInfo>, h: &mut impl StartHost) -> R<()> {
    let Some(game) = game else {
        return Ok(());
    };
    let md = h.st_find_map(&to_lower(&game.map_name)?);
    let num_players = md.as_ref().map_or(MAX_SLOTS as i32, |m| m.num_players);
    let n = num_players.clamp(0, MAX_SLOTS as i32) as usize;
    let mut dist = [[0f32; MAX_SLOTS]; MAX_SLOTS];
    for (i, row) in dist.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            if i != j && i < n && j < n {
                // md->m_waypoints, dereferenced even when the map is not in the cache
                let Some(m) = &md else {
                    return Err(undefined(Undefined::NullObject));
                };
                let find = |k: usize| {
                    let name = format!("Player_{}_Start", k + 1).into_bytes();
                    m.waypoints.iter().find(|w| w.0 == name).map(|w| w.1)
                };
                *cell = match (find(i), find(j)) {
                    (Some(p1), Some(p2)) => distance(&p1, &p2),
                    _ => 1_000_000.0,
                };
            }
        }
    }
    let mut picked = false;
    let mut taken = [false; MAX_SLOTS];
    for (i, t) in taken.iter_mut().enumerate() {
        *t = i as i32 >= num_players;
    }
    for s in game.slots.iter() {
        if !s.is_occupied() || s.player_template == PLAYERTEMPLATE_OBSERVER {
            continue;
        }
        let pos = s.start_pos;
        if pos >= 0 || pos >= num_players {
            picked = true;
            *taken.get_mut(pos as usize).ok_or_else(|| undefined(Undefined::ArrayIndexOutOfRange))? = true;
        }
    }
    let mut team_pos = [-1i32; MAX_SLOTS];
    for i in 0..MAX_SLOTS {
        let s = &game.slots[i];
        if !s.is_occupied() || s.player_template == PLAYERTEMPLATE_OBSERVER {
            continue;
        }
        let mut pos = s.start_pos;
        if pos >= 0 && pos < num_players {
            continue;
        }
        let team = s.team_number;
        let team_at = |t: i32| usize::try_from(t).ok().filter(|&t| t < MAX_SLOTS).ok_or_else(|| undefined(Undefined::ArrayIndexOutOfRange));
        if !picked {
            let mut draws = 0u32;
            while pos == -1 {
                pos = h.st_random(0, num_players - 1);
                if game.is_start_position_taken(pos, -1) {
                    pos = -1;
                }
                draws += 1;
                if draws > ENDLESS {
                    return Err(undefined(Undefined::EndlessLoop));
                }
            }
            picked = true;
            game.slots[i].start_pos = pos;
            *taken.get_mut(pos as usize).ok_or_else(|| undefined(Undefined::ArrayIndexOutOfRange))? = true;
            if team > -1 {
                team_pos[team_at(team)?] = pos;
            }
        } else if num_players > MAX_SLOTS as i32 {
            // both searches below index taken[] up to numPlayers
            return Err(undefined(Undefined::ArrayIndexOutOfRange));
        } else if team < 0 || team_pos[team_at(team)?] == -1 {
            let mut farthest_distance = 0f32;
            let mut farthest = -1i32;
            for p in 0..n {
                if taken[p] {
                    continue;
                }
                let mut d = 0f32;
                for k in 0..n {
                    if taken[k] && k != p {
                        d += dist[p][k];
                    }
                }
                if farthest < 0 {
                    farthest = p as i32;
                    farthest_distance = d;
                } else if d > farthest_distance && !plant(4024) {
                    farthest_distance = d;
                    farthest = p as i32;
                }
            }
            game.slots[i].start_pos = farthest;
            // taken[farthestIndex] with no free position is taken[-1]
            *taken.get_mut(usize::try_from(farthest).map_err(|_| undefined(Undefined::ArrayIndexOutOfRange))?).expect("in range") = true;
            if team > -1 {
                team_pos[team_at(team)?] = farthest;
            }
        } else {
            let base = team_pos[team_at(team)?] as usize;
            let mut closest_dist = f32::MAX;
            let mut closest = 0usize;
            for k in 0..n {
                let d = dist[base][k];
                if (!taken[k] || plant(4023)) && d < closest_dist {
                    closest_dist = d;
                    closest = k;
                }
            }
            game.slots[i].start_pos = closest as i32;
            taken[closest] = true;
        }
    }
    let in_game = game.slots.iter().filter(|s| s.is_occupied() && s.player_template != PLAYERTEMPLATE_OBSERVER).count();
    for i in 0..MAX_SLOTS {
        if !game.slots[i].is_occupied() || game.slots[i].player_template != PLAYERTEMPLATE_OBSERVER {
            continue;
        }
        let mut pos = -1;
        if in_game == 0 {
            pos = 0;
        }
        let mut draws = 0u32;
        while pos == -1 {
            pos = h.st_random(0, num_players - 1);
            if !game.is_start_position_taken(pos, -1) {
                pos = -1;
            }
            draws += 1;
            if draws > ENDLESS {
                return Err(undefined(Undefined::EndlessLoop));
            }
        }
        game.slots[i].start_pos = pos;
    }
    Ok(())
}

/// `handleNameChange(mapObj)`: two renamed templates fixed (`AmericaTankLeopard` and `AmericaVehicleHumVee`), each
/// map object's template found again by its new name.
pub fn handle_name_change(name: &mut Vec<u8>, template: &mut Option<u32>, h: &impl StartHost) {
    if name.as_slice() == b"AmericaTankLeopard" {
        *name = b"AmericaTankCrusader".to_vec();
        *template = h.st_find_template(name);
    }
    if name.as_slice() == b"AmericaVehicleHumVee" && !plant(4025) {
        *name = b"AmericaVehicleHumvee".to_vec();
        *template = h.st_find_template(name);
    }
}

/// `placeObjectAtPosition(slotNum, name, pos, player, template)`: the object made for the player's default team,
/// turned to its template's placement angle, placed, built, its team activated, added to the pathfinder, and a
/// mobile one moved to the destination the pathfinder adjusts (`pos` is updated then).
pub fn place_object_at_position(name: &[u8], pos: &mut Coord3D, player: i32, h: &mut impl StartHost) -> R<Option<u32>> {
    // newObject(NULL, ...) throws ERROR_BAD_ARG, which nothing catches
    let template = h.st_find_template(name).ok_or_else(|| undefined(Undefined::NewObjectFailed))?;
    let Some(obj) = h.st_new_object(template, player) else {
        return Ok(None);
    };
    let angle = h.st_placement_view_angle(obj);
    h.st_set_orientation(obj, angle);
    h.st_set_position(obj, *pos);
    h.st_build_complete(obj);
    h.st_activate_default_team(player);
    h.st_add_to_pathfind_map(obj);
    if h.st_is_mobile_ai(obj) {
        if let Some(adjusted) = h.st_adjust_destination(obj, *pos) {
            *pos = adjusted;
            h.st_update_goal(obj, *pos);
            h.st_set_position(obj, *pos);
        }
    }
    Ok(Some(obj))
}

/// `placeNetworkBuildingsForPlayer(slotNum, slot, player, template)`.
pub fn place_network_buildings_for_player(start_pos: i32, player: i32, template: i32, h: &mut impl StartHost) -> R<()> {
    let start = format!("Player_{}_Start", start_pos.wrapping_add(1)).into_bytes();
    let rally = format!("Player_{}_Rally", start_pos.wrapping_add(1)).into_bytes();
    let waypoint = h.st_waypoint(&start);
    let rally_waypoint = h.st_waypoint(&rally);
    let Some(mut pos) = waypoint else {
        return Ok(());
    };
    pos.z = h.st_ground_height(pos.x, pos.y);
    let building = h.st_starting_building(template);
    if building.is_empty() {
        return Ok(());
    }
    let Some(con_yard) = place_object_at_position(&building, &mut pos, player, h)? else {
        return Ok(());
    };
    h.st_structure_created(player, con_yard);
    let radius = h.st_bounding_sphere_radius(con_yard);
    pos.y -= radius / 2.0;
    pos.z = h.st_ground_height(pos.x, pos.y);
    if let Some(r) = rally_waypoint {
        pos = r;
        pos.z = h.st_ground_height(pos.x, pos.y);
    }
    for i in 0..MAX_MP_STARTING_UNITS {
        let unit = h.st_starting_unit(template, i);
        if unit.is_empty() {
            continue;
        }
        let radius = h.st_bounding_sphere_radius(con_yard);
        let (min_radius, max_radius) = (radius * 0.7, radius * 1.3);
        h.st_partition_update();
        if let Some(mut at) = h.st_find_position_around(pos, min_radius, max_radius) {
            if let Some(u) = place_object_at_position(&unit, &mut at, player, h)? {
                if !plant(4026) {
                    h.st_unit_created(player, u);
                }
            }
        }
    }
    Ok(())
}

/// `getLoadScreen(loadingSaveGame)`: by the game mode; a single-player campaign (not from a save) has its own, a
/// challenge campaign the challenge one; `GAME_NONE` (and any other mode) none.
pub fn load_screen_kind(mode: i32, campaign_is_challenge: Option<bool>, loading_save_game: bool) -> Option<LoadScreenKind> {
    match mode {
        game_mode::SHELL | game_mode::REPLAY => Some(LoadScreenKind::Shell),
        game_mode::SINGLE_PLAYER => Some(match campaign_is_challenge {
            Some(challenge) if !loading_save_game => {
                if challenge {
                    LoadScreenKind::Challenge
                } else {
                    LoadScreenKind::SinglePlayer
                }
            }
            _ => LoadScreenKind::Shell,
        }),
        game_mode::SKIRMISH | game_mode::LAN => Some(LoadScreenKind::MultiPlayer),
        game_mode::INTERNET => Some(LoadScreenKind::GameSpy),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_screens() {
        assert_eq!(load_screen_kind(game_mode::SINGLE_PLAYER, Some(true), false), Some(LoadScreenKind::Challenge));
        assert_eq!(load_screen_kind(game_mode::SINGLE_PLAYER, Some(false), true), Some(LoadScreenKind::Shell));
        assert_eq!(load_screen_kind(game_mode::NONE, None, false), None);
    }
}
