// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (the calls the helpers of `startNewGame` make:
//   `populateRandomSideAndColor`, `populateRandomStartPosition`, `placeObjectAtPosition`, `placeNetworkBuildingsForPlayer`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I7c, part 1: the world as `StartHost`, so the helpers `startNewGame` runs on the game's slots and
//! players (`game_logic_start.rs`) place the starting buildings and units in the world.
//!
//! - **The world:** the player templates (count, starting building and units, old faction), the logic's random values
//!   and seed, the thing factory and templates (placement view angle), the waypoints by name, the terrain's height,
//!   making an object on a player's default team, orientation, position, the create modules' `onBuildComplete`,
//!   activating a player's default team, the objects' AI and kinds, the players' `onStructureCreated`,
//!   `onStructureConstructionComplete` and `onUnitCreated`, the geometry, and the partition manager (`update`,
//!   `findPositionAround` with `live_partition`).
//! - **Cut points:** the challenge generals (`cuts.challenge_generals_locked`, by template name), the multiplayer
//!   settings' colour count (~~`cuts.new_game.colors`~~ the statics' settings since batch I56), the map cache (~~`cuts.map_cache`~~ the statics' `MapCache` since batch I56, by lower-cased name) and
//!   the pathfinder (`addObjectToPathfindMap`, `adjustDestination`, `updateGoal`: the contain cut points, batch I9).
//!
//! `p1w13-sonnet`, integration batch I7c, 2026-11-01.

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::Coord3D;
use crate::common::kind_of::KINDOF_IMMOBILE;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_start::{MapStartInfo, StartHost};
use crate::logic::world::ids::ObjectId;
use crate::logic::world::player_objects::player_on_structure_construction_complete;
use crate::logic::world::player_ops::player_on_unit_created;

impl StartHost for World {
    fn st_player_template_count(&self) -> i32 {
        self.statics.player_template_store.player_template_count()
    }
    fn st_player_template_exists(&self, i: i32) -> bool {
        self.statics.player_template_store.nth_player_template(i).is_some()
    }
    fn st_starting_building(&self, i: i32) -> Vec<u8> {
        self.statics.player_template_store.nth_player_template(i).map(|t| t.starting_building().to_vec()).unwrap_or_default()
    }
    fn st_starting_unit(&self, i: i32, n: i32) -> Vec<u8> {
        self.statics.player_template_store.nth_player_template(i).map(|t| t.starting_unit(n).to_vec()).unwrap_or_default()
    }
    fn st_is_old_faction(&self, i: i32) -> bool {
        self.statics.player_template_store.nth_player_template(i).is_some_and(|t| t.is_old_faction())
    }
    fn st_general_starts_locked(&self, i: i32) -> bool {
        let Some(t) = self.statics.player_template_store.nth_player_template(i) else { return false };
        let name = self.names.key_to_name(t.name_key());
        self.cuts.challenge_generals_locked.contains(name)
    }
    fn st_num_colors(&self) -> i32 {
        self.statics.multiplayer_settings.get_num_colors()
    }
    fn st_find_map(&self, lower_name: &[u8]) -> Option<MapStartInfo> {
        // (batch I56: the statics' `MapCache`; was `cuts.map_cache`, a table)
        self.statics.map_cache.maps.get(lower_name).map(|m| MapStartInfo {
            num_players: m.num_players,
            waypoints: m.waypoints.map.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        })
    }
    fn st_random(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn st_random_seed(&self) -> u32 {
        self.random.get_game_logic_random_seed()
    }
    fn st_find_template(&self, name: &[u8]) -> Option<u32> {
        World::thing_factory_find_template(self, name).map(u32::from)
    }
    fn st_waypoint(&self, name: &[u8]) -> Option<Coord3D> {
        let h = self.terrain.waypoint_by_name(&AsciiString::try_from_bytes(name).ok()?)?;
        self.terrain.waypoint(h).map(|w| *w.location())
    }
    fn st_ground_height(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn st_new_object(&mut self, template: u32, player: i32) -> Option<u32> {
        let team = self.players.get_nth(player).and_then(|p| p.default_team);
        let t = u16::try_from(template).ok()?;
        World::thing_factory_new_object(self, t, team, ObjectStatusMaskType::default()).ok().map(|o| o as u32)
    }
    fn st_placement_view_angle(&self, obj: u32) -> f32 {
        self.find_object(obj as ObjectId).map_or(0.0, |o| o.template().get_placement_view_angle())
    }
    fn st_set_orientation(&mut self, obj: u32, angle: f32) {
        self.set_orientation(obj as ObjectId, angle);
    }
    fn st_set_position(&mut self, obj: u32, pos: Coord3D) {
        self.set_position(obj as ObjectId, &pos);
    }
    fn st_build_complete(&mut self, obj: u32) {
        build_complete(self, obj as ObjectId);
    }
    fn st_activate_default_team(&mut self, player: i32) {
        if let Some(team) = self.players.get_nth(player).and_then(|p| p.default_team) {
            let _ = crate::logic::world::team_ops::team_set_active(self, team);
        }
    }
    fn st_add_to_pathfind_map(&mut self, obj: u32) {
        self.contain_pathfind_add_object(obj as ObjectId);
    }
    fn st_is_mobile_ai(&self, obj: u32) -> bool {
        self.find_object(obj as ObjectId).is_some_and(|o| o.ai.is_some() && !o.is_kind_of(KINDOF_IMMOBILE))
    }
    fn st_adjust_destination(&mut self, obj: u32, pos: Coord3D) -> Option<Coord3D> {
        let mut p = pos;
        self.contain_pathfind_adjust_destination(obj as ObjectId, &mut p).then_some(p)
    }
    fn st_update_goal(&mut self, obj: u32, pos: Coord3D) {
        self.contain_pathfind_update_goal(obj as ObjectId, &pos, crate::common::game_type::LAYER_GROUND);
    }
    fn st_structure_created(&mut self, player: i32, obj: u32) {
        // `onStructureCreated(NULL, obj)` is empty upstream
        let _ = player_on_structure_construction_complete(self, player, None, obj as ObjectId, false);
    }
    fn st_unit_created(&mut self, player: i32, obj: u32) {
        let _ = player_on_unit_created(self, player, None, Some(obj as ObjectId));
    }
    fn st_bounding_sphere_radius(&self, obj: u32) -> f32 {
        self.find_object(obj as ObjectId).map_or(0.0, |o| o.geometry_info.get_bounding_sphere_radius())
    }
    fn st_partition_update(&mut self) {
        if self.cuts.live_partition {
            self.partition_update();
        }
    }
    fn st_find_position_around(&mut self, pos: Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let mut p = pos;
        self.contain_partition_find_position_around(&mut p, min_radius, max_radius, 0).then_some(p)
    }
}

/// `startNewGame`'s loop over a map object's behaviour modules: each create module's `onBuildComplete()` (GameLogic.cpp:1902,
/// "This magically created thing needs to be considered as Built").
pub fn build_complete(w: &mut World, o: ObjectId) {
    for idx in w.modules_with(o, crate::common::module::MODULEINTERFACE_CREATE) {
        w.module_call(o, idx, crate::logic::world::module_call::ModuleCall::OnBuildComplete);
    }
}

#[cfg(test)]
mod tests {
    //! The helpers themselves are checked against the compiled GameLogic.cpp by `tests/differential_game_logic_start.rs`
    //! with a recording host; this checks a player's starting buildings placed in the world.
    use super::*;
    use crate::logic::world::game_logic_start::place_network_buildings_for_player;
    use crate::logic::world::player_host_world::tests::world;

    /// Player 1 (America: a barracks and a tank to start with) at start position 0: the barracks is made on its default
    /// team at `Player_1_Start`, built (its construction complete), and the tank is placed somewhere around it, also the
    /// player's. With `live_teams` and the partition manager live.
    #[test]
    fn a_players_starting_buildings_and_units_are_placed() {
        let (mut w, team) = world();
        w.cuts.live_teams = true;
        w.players.get_nth_mut(1).unwrap().default_team = Some(team);
        let start = Coord3D { x: 200.0, y: 200.0, z: 0.0 };
        w.terrain.friend_push_waypoint(1, b"Player_1_Start", start);
        place_network_buildings_for_player(0, 1, 0, &mut w).unwrap();
        let mine = w.player_iterate_objects(1);
        let names: Vec<Vec<u8>> = mine.iter().map(|o| w.find_object(*o).unwrap().template().get_name().to_vec()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        let barracks = mine[names.iter().position(|n| n == b"Barracks").expect("a barracks")];
        let tank = mine[names.iter().position(|n| n == b"Tank").expect("a tank")];
        let bp = w.find_object(barracks).unwrap().get_position();
        assert_eq!((bp.x, bp.y), (start.x, start.y));
        let tp = w.find_object(tank).unwrap().get_position();
        assert!((tp.x, tp.y) != (start.x, start.y));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
