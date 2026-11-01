// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (the calls `startNewGame` makes out of
//   `GameLogic`, and the side and team dictionaries it fills)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I7c, part 2: the world as `NewGameHost`, so `GameLogic::startNewGame` ([`start_new_game`]) sets a
//! game up in the world. [`World::start_new_game`] is the entry point; `GameLogic::update`'s `startNewGame` (batch I4's
//! cut point) is now it, on the frame's own `GameLogic` members (`GameLogicHost::u_start_new_game` takes them).
//!
//! The dispatcher's `MSG_NEW_GAME` (`prepareNewGame`, then `startNewGame(FALSE)`) runs inside the frame, which holds
//! `GameLogic`'s members. `prepareNewGame` writes one of them, the rank points: the world keeps them in
//! `cuts.pending_rank_points` and `World::logic_update` writes them when the frame ends (only the next frame's
//! `startNewGame` reads them). `startNewGame(FALSE)` with no game pending (`prepareNewGame` has just cleared it) only
//! marks the game pending, touching none of the members.
//!
//! - **The world:** `setDefaults`, the frame, the rank limit (`cuts.player_host.rank_level_limit`), the sides list (each
//!   slot's side as `TheSidesList->addSide` gets it, with `TheKey_*` in upstream's order; the singleton teams;
//!   `validateSides`), `TheTeamFactory->reset()`, `ThePlayerList->newGame()`, `newMap()`, `validateTeam`, the players'
//!   relationships and skill points, `TheVictoryConditions` (batch I5), the terrain (`newMap`, the extent, the layers),
//!   the partition manager (`init`, the reveals, with `live_partition`), the map objects ([`NewGameCuts::map_objects`],
//!   each with its properties), the thing templates, the objects made from them, the landmark bridges (batch I2), and
//!   the map properties (batch O1).
//! - **Cut points** ([`CutCall::Host`], answers in [`NewGameCuts`]): the map and its INI and string files, the save
//!   directory, the campaign manager, the game's options by source, the multiplayer settings (colours, shroud), the
//!   map's multiplayer scripts, the script engine, the radar, the ghost objects, the pathfinder, the network, the game
//!   engine, the client (load screen, background, shell, control bar, view, drawables, props, trees, transitions,
//!   window manager, display) and the stats collector.
//!
//! `p1w13-sonnet`, integration batch I7c, 2026-11-01.

use crate::common::base_type::Coord3D;
use crate::common::dict::Dict;
use crate::common::game_common::Relationship;
use crate::common::game_info::GameInfo;
use crate::common::ini::IniError;
use crate::common::kind_of::{
    KINDOF_CAN_CAST_REFLECTIONS, KINDOF_CLEARED_BY_BUILD, KINDOF_COMMANDCENTER, KINDOF_OPTIMIZED_TREE, KINDOF_PROP, KINDOF_SHRUBBERY,
    KINDOF_WALK_ON_TOP_OF_WALL,
};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::well_known_keys as wk;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_new_game::{start_new_game, GameSource, MapObj, NewGameHost, SideDict, StaticLod, TemplateKind};
use crate::logic::world::game_logic_start::LoadScreenKind;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_list::{
    player_list_get_players_with_relationship, player_list_new_game, player_list_new_map, player_list_validate_team,
};
use crate::logic::world::player_ops::{player_add_skill_points, player_is_local_player, player_is_player_active, player_set_player_relationship};
use crate::logic::world::team_ops::{factory_reset, team_set_active};

type R<T> = Result<T, IniError>;

/// The answers of the units `startNewGame` reaches that the world does not hold yet. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct NewGameCuts {
    /// `TheGlobalData->m_mapName`.
    pub map_name: Vec<u8>,
    /// `TheGlobalData->m_pendingFile` (`prepareNewGame` makes it the map name).
    pub pending_file: Vec<u8>,
    /// `TheGameState`'s pristine map name.
    pub pristine_map_name: Vec<u8>,
    /// The paths `TheGameState->isInSaveDirectory` says are in the save directory.
    pub save_directory: Vec<Vec<u8>>,
    /// `TheCampaignManager->getCurrentCampaign()`: `Some(isChallengeCampaign)` when there is one.
    pub campaign: Option<bool>,
    /// `TheLAN != NULL`.
    pub has_lan: bool,
    /// The game options of each source (`TheLAN->GetMyGame()`, `TheSkirmishGameInfo`, ...).
    pub games: Vec<(GameSource, GameInfo)>,
    /// The source `TheGameInfo` names, and the game taken out of it while `startNewGame` works on it.
    pub selected: Option<GameSource>,
    /// See `selected`.
    pub taken: Option<GameInfo>,
    /// `TheMapCache != NULL`.
    pub has_map_cache: bool,
    /// The files `TheFileSystem->doesFileExist` finds.
    pub files: Vec<Vec<u8>>,
    /// `TheGameEngine->isMultiplayerSession()`.
    pub multiplayer_session: bool,
    /// `TheMultiplayerSettings`' colours: each colour and night colour.
    pub colors: Vec<(i32, i32)>,
    /// `TheMultiplayerSettings->isShroudInMultiplayer()` (TRUE, its constructor's).
    pub shroud_in_multiplayer: bool,
    /// The answer of the multiplayer scripts file (`None`: it does not open; `Some(false)`: it does not parse).
    pub multiplayer_scripts: Option<bool>,
    /// The map objects (`MapObject::getFirstMapObject` and on), each with its properties.
    pub map_objects: Vec<(MapObj, Dict)>,
    /// `TheRecorder->getGameMode()`.
    pub recorder_game_mode: i32,
    /// `TheGameLODManager->getStaticLODLevel()`.
    pub static_lod: Option<StaticLod>,
    /// `isInInternetGame()`.
    pub internet_game: bool,
    /// `GameLogic`'s `m_width`, `m_height` (`setWidth`, `setHeight`).
    pub world_size: (f32, f32),
}

impl World {
    /// `GameLogic::startNewGame(loadingSaveGame)` (see [`start_new_game`]).
    pub fn start_new_game(&mut self, loading_save_game: bool) -> R<()> {
        let mut state = std::mem::take(&mut self.game_logic);
        let r = start_new_game(&mut state, loading_save_game, self);
        self.game_logic = state;
        r
    }

    /// Logs a call of `startNewGame` into a unit the world does not hold.
    fn ng_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }

    /// `TheKey_<which>`.
    fn ng_key(&mut self, which: usize) -> crate::common::name_key_generator::NameKeyType {
        self.keys.key(which, &mut self.names)
    }
}

impl NewGameHost for World {
    fn sn_set_loading_map(&mut self, on: bool) {
        self.ng_cut("GameLogic::setLoadingMap", &[i64::from(on)]);
    }
    fn sn_map_name(&self) -> Vec<u8> {
        self.cuts.new_game.map_name.clone()
    }
    fn sn_set_pristine_map_name(&mut self, name: &[u8]) {
        self.cuts.new_game.pristine_map_name = name.to_vec();
    }
    fn sn_pristine_map_name(&self) -> Vec<u8> {
        self.cuts.new_game.pristine_map_name.clone()
    }
    fn sn_is_in_save_directory(&self, path: &[u8]) -> bool {
        self.cuts.new_game.save_directory.iter().any(|p| p == path)
    }
    fn sn_destroy_background(&mut self) -> bool {
        false
    }
    fn sn_has_load_screen(&self) -> bool {
        self.cuts.game_loop.has_load_screen
    }
    fn sn_set_load_screen(&mut self, kind: Option<LoadScreenKind>) {
        self.cut(CutCall::Host { call: "GameLogic::getLoadScreen", args: Vec::new(), text: format!("{kind:?}").into_bytes() });
        self.cuts.game_loop.has_load_screen = kind.is_some();
    }
    fn sn_load_screen_init(&mut self, with_game: bool) {
        self.ng_cut("LoadScreen::init", &[i64::from(with_game)]);
    }
    fn sn_load_screen_update(&mut self, progress: i32) {
        self.ng_cut("LoadScreen::update", &[i64::from(progress)]);
    }
    fn sn_delete_load_screen(&mut self) {
        self.cuts.game_loop.has_load_screen = false;
    }
    fn sn_set_load_screen_render(&mut self, on: bool) {
        self.ng_cut("GlobalData::m_loadScreenRender", &[i64::from(on)]);
    }
    fn sn_clear_tivo_fast_mode(&mut self) {
        self.ng_cut("GlobalData::m_TiVOFastMode", &[0]);
    }
    fn sn_set_defaults(&mut self, loading_save_game: bool) {
        self.set_defaults(loading_save_game);
    }
    fn sn_set_rank_level_limit(&mut self, limit: i32) {
        self.cuts.player_host.rank_level_limit = limit;
    }
    fn sn_set_frame(&mut self, frame: u32) {
        self.set_frame(frame);
    }
    fn sn_current_campaign(&self) -> Option<bool> {
        self.cuts.new_game.campaign
    }
    fn sn_campaign_not_victorious(&mut self) -> bool {
        self.ng_cut("CampaignManager::SetVictorious", &[0]);
        true
    }
    fn sn_has_lan(&self) -> bool {
        self.cuts.new_game.has_lan
    }
    fn sn_select_game(&mut self, source: GameSource) -> bool {
        let ng = &mut self.cuts.new_game;
        ng.selected = (source != GameSource::None).then_some(source);
        ng.taken = ng.selected.and_then(|s| ng.games.iter().find(|(src, _)| *src == s).map(|(_, g)| g.clone()));
        ng.taken.is_some()
    }
    fn sn_take_game(&mut self) -> GameInfo {
        self.cuts.new_game.taken.take().expect("startNewGame takes the game's options only after selecting a game")
    }
    fn sn_put_game(&mut self, game: GameInfo) {
        let ng = &mut self.cuts.new_game;
        if let Some(s) = ng.selected {
            if let Some(entry) = ng.games.iter_mut().find(|(src, _)| *src == s) {
                entry.1 = game.clone();
            }
        }
        ng.taken = Some(game);
    }
    fn sn_delete_skirmish_game_info(&mut self) {
        self.cuts.new_game.games.retain(|(s, _)| *s != GameSource::Skirmish);
    }
    fn sn_has_map_cache(&self) -> bool {
        self.cuts.new_game.has_map_cache
    }
    fn sn_file_exists(&mut self, path: &[u8]) -> bool {
        self.cuts.new_game.files.iter().any(|f| f == path)
    }
    fn sn_load_ini(&mut self, path: &[u8]) -> R<()> {
        self.cut(CutCall::Host { call: "INI::load", args: Vec::new(), text: path.to_vec() });
        Ok(())
    }
    fn sn_init_map_string_file(&mut self, path: &[u8]) {
        self.cut(CutCall::Host { call: "GameText::initMapStringFile", args: Vec::new(), text: path.to_vec() });
    }
    fn sn_asset_purge_and_preload(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn sn_terrain_load_map(&mut self, name: &[u8]) -> R<()> {
        // the map's height map, waypoints, triggers and objects are put in the world by whoever loads it
        // (`World::terrain`, `cuts.new_game.map_objects`)
        self.cut(CutCall::Host { call: "TerrainLogic::loadMap", args: Vec::new(), text: name.to_vec() });
        Ok(())
    }
    fn sn_is_multiplayer_session(&self) -> bool {
        self.cuts.new_game.multiplayer_session
    }
    fn sn_sides_prepare_for_mp(&mut self) {
        self.ng_cut("SidesList::prepareForMP_or_Skirmish", &[]);
    }
    fn sn_observer_template(&self) -> Option<i32> {
        let store = &self.statics.player_template_store;
        store.templates().iter().position(|t| self.names.key_to_name(t.name_key()) == b"FactionObserver").map(|i| i as i32)
    }
    fn sn_template_name(&self, i: i32) -> Vec<u8> {
        self.statics.player_template_store.nth_player_template(i).map(|t| self.names.key_to_name(t.name_key()).to_vec()).unwrap_or_default()
    }
    fn sn_color(&self, index: i32) -> Option<(i32, i32)> {
        usize::try_from(index).ok().and_then(|i| self.cuts.new_game.colors.get(i)).copied()
    }
    fn sn_add_side(&mut self, side: &SideDict) {
        // the keys in the order `startNewGame` sets them
        let mut d = Dict::new();
        let k = self.ng_key(wk::PLAYER_NAME);
        let _ = d.set_ascii_string(k, &side.player_name);
        let k = self.ng_key(wk::PLAYER_IS_HUMAN);
        let _ = d.set_bool(k, side.is_human);
        let k = self.ng_key(wk::PLAYER_DISPLAY_NAME);
        let _ = d.set_unicode_string(k, &side.display_name);
        if let Some(f) = &side.faction {
            let k = self.ng_key(wk::PLAYER_FACTION);
            let _ = d.set_ascii_string(k, f);
        }
        if side.is_preorder {
            let k = self.ng_key(wk::PLAYER_IS_PREORDER);
            let _ = d.set_bool(k, true);
        }
        let k = self.ng_key(wk::PLAYER_ALLIES);
        let _ = d.set_ascii_string(k, &side.allies);
        let k = self.ng_key(wk::PLAYER_ENEMIES);
        let _ = d.set_ascii_string(k, &side.enemies);
        let k = self.ng_key(wk::PLAYER_COLOR);
        let _ = d.set_int(k, side.color);
        let k = self.ng_key(wk::PLAYER_NIGHT_COLOR);
        let _ = d.set_int(k, side.night_color);
        let k = self.ng_key(wk::MULTIPLAYER_START_INDEX);
        let _ = d.set_int(k, side.start_index);
        let k = self.ng_key(wk::MULTIPLAYER_IS_LOCAL);
        let _ = d.set_bool(k, side.is_local);
        if let Some(difficulty) = side.skirmish {
            let k = self.ng_key(wk::PLAYER_IS_SKIRMISH);
            let _ = d.set_bool(k, true);
            if let Some(diff) = difficulty {
                let k = self.ng_key(wk::SKIRMISH_DIFFICULTY);
                let _ = d.set_int(k, diff);
            }
        }
        self.sides.add_side(&d);
    }
    fn sn_add_team(&mut self, team_name: &[u8], owner: &[u8]) {
        let mut d = Dict::new();
        let k = self.ng_key(wk::TEAM_NAME);
        let _ = d.set_ascii_string(k, team_name);
        let k = self.ng_key(wk::TEAM_OWNER);
        let _ = d.set_ascii_string(k, owner);
        let k = self.ng_key(wk::TEAM_IS_SINGLETON);
        let _ = d.set_bool(k, true);
        self.sides.add_team(&d);
    }
    fn sn_validate_sides(&mut self) {
        let mut names = crate::logic::sides_list::MapNames::new(std::mem::take(&mut self.names));
        let r = self.sides.validate_sides(&mut names);
        self.names = names.generator;
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("SidesList::validateSides: {e:?}"));
        }
    }
    fn sn_team_factory_reset(&mut self) {
        if let Err(e) = factory_reset(self) {
            self.cuts.team_live_errors.push(format!("TeamFactory::reset: {e:?}"));
        }
    }
    fn sn_player_list_new_game(&mut self) {
        if let Err(e) = player_list_new_game(self) {
            self.cuts.team_live_errors.push(format!("PlayerList::newGame: {e:?}"));
        }
    }
    fn sn_script_engine_new_map(&mut self) {
        self.ng_cut("ScriptEngine::newMap", &[]);
    }
    fn sn_add_multiplayer_scripts(&mut self) -> Option<bool> {
        self.ng_cut("MultiplayerScripts.scb", &[]);
        self.cuts.new_game.multiplayer_scripts
    }
    fn sn_radar_new_map(&mut self) {
        self.ng_cut("Radar::newMap", &[]);
    }
    fn sn_ui_client_not_quiet(&mut self) {
        self.cuts.player_host.client_quiet = false;
    }
    fn sn_victory_setup(&mut self) {
        self.victory_cache_player_ptrs();
        self.victory.set_victory_conditions(crate::logic::world::victory_conditions::VICTORY_NOBUILDINGS);
    }
    fn sn_terrain_extent(&self) -> (Coord3D, Coord3D) {
        self.terrain.extent().map(|r| (r.lo, r.hi)).unwrap_or_default()
    }
    fn sn_set_world_size(&mut self, width: f32, height: f32) {
        self.cuts.new_game.world_size = (width, height);
    }
    fn sn_partition_init(&mut self) {
        if !self.cuts.live_partition {
            self.ng_cut("PartitionManager::init", &[]);
            return;
        }
        self.partition_init();
        let mut pm = std::mem::take(&mut self.partition);
        pm.refresh_shroud_for_local_player(self);
        self.partition = pm;
    }
    fn sn_local_player(&self) -> Option<i32> {
        (0..16).find(|&i| player_is_local_player(self, i))
    }
    fn sn_ghost_reset(&mut self, local_player: i32) {
        self.ng_cut("GhostObjectManager::reset", &[i64::from(local_player)]);
    }
    fn sn_terrain_new_map(&mut self, loading_save_game: bool) {
        let statics = self.statics.clone();
        let Some(g) = statics.global_data.the_global_data() else { return };
        if let Err(e) = self.terrain.new_map(g) {
            self.cuts.team_live_errors.push(format!("TerrainLogic::newMap({loading_save_game}): {e:?}"));
        }
    }
    fn sn_map_objects(&self) -> Vec<MapObj> {
        self.cuts.new_game.map_objects.iter().map(|(m, _)| m.clone()).collect()
    }
    fn sn_set_map_object(&mut self, index: usize, name: &[u8], template: Option<u32>) {
        if let Some((m, _)) = self.cuts.new_game.map_objects.get_mut(index) {
            m.name = name.to_vec();
            m.template = template;
        }
    }
    fn sn_template_is_bridge(&self, t: u32) -> bool {
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).is_some_and(|t| t.get_final_override().data.is_bridge())
    }
    fn sn_template_is(&self, t: u32, kind: TemplateKind) -> bool {
        let k = match kind {
            TemplateKind::WalkOnTopOfWall => KINDOF_WALK_ON_TOP_OF_WALL,
            TemplateKind::Shrubbery => KINDOF_SHRUBBERY,
            TemplateKind::OptimizedTree => KINDOF_OPTIMIZED_TREE,
            TemplateKind::Prop => KINDOF_PROP,
            TemplateKind::ClearedByBuild => KINDOF_CLEARED_BY_BUILD,
        };
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).is_some_and(|t| t.get_final_override().data.is_kind_of(k))
    }
    fn sn_template_fence_width(&self, t: u32) -> f32 {
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).map_or(0.0, |t| t.get_final_override().data.get_fence_width())
    }
    fn sn_has_neutral_player(&self) -> bool {
        self.players.get_nth(0).is_some()
    }
    fn sn_new_object_neutral(&mut self, t: u32) -> R<Option<u32>> {
        let team = self.players.get_nth(0).and_then(|p| p.default_team);
        self.sn_new_object_for_team(t, team)
    }
    fn sn_new_object_for_team(&mut self, t: u32, team: Option<u32>) -> R<Option<u32>> {
        let Ok(t) = u16::try_from(t) else { return Ok(None) };
        Ok(World::thing_factory_new_object(self, t, team, ObjectStatusMaskType::default()).ok().map(|o| o as u32))
    }
    fn sn_recorder_game_mode(&self) -> i32 {
        self.cuts.new_game.recorder_game_mode
    }
    fn sn_add_landmark_bridge(&mut self, obj: u32) {
        self.terrain_add_landmark_bridge_to_logic(obj as ObjectId);
    }
    fn sn_add_wall_piece(&mut self, obj: u32) {
        self.ng_cut("Pathfinder::addWallPiece", &[i64::from(obj)]);
    }
    fn sn_update_obj_from_map(&mut self, obj: u32, map_object: usize) {
        let Some(props) = self.cuts.new_game.map_objects.get(map_object).map(|(_, d)| d.clone()) else { return };
        if let Err(e) = self.update_obj_values_from_map_properties(obj as ObjectId, &props) {
            self.cuts.team_live_errors.push(format!("Object::updateObjValuesFromMapProperties: {e:?}"));
        }
    }
    fn sn_radar_refresh_and_pathfinder_new_map(&mut self) {
        self.ng_cut("Radar::refreshTerrain, Pathfinder::newMap", &[]);
    }
    fn sn_find_player(&self, name: &[u8]) -> Option<i32> {
        // `findPlayerWithNameKey(NAMEKEY(name))`: a player whose name key is that name's (names and keys are one to one)
        let players = &self.players;
        (0..players.player_count()).find(|&i| {
            players.get_nth(i).is_some_and(|p| self.names.key_to_name(crate::common::name_key_generator::NameKeyType(p.name_key as u32)) == name)
        })
    }
    fn sn_reveal_permanently(&mut self, player: i32) {
        if self.cuts.live_partition {
            let mut pm = std::mem::take(&mut self.partition);
            pm.reveal_map_for_player_permanently(self, player);
            self.partition = pm;
        } else {
            self.ng_cut("PartitionManager::revealMapForPlayerPermanently", &[i64::from(player)]);
        }
    }
    fn sn_reveal(&mut self, player: i32) {
        if self.cuts.live_partition {
            let mut pm = std::mem::take(&mut self.partition);
            pm.reveal_map_for_player(self, player);
            self.partition = pm;
        } else {
            self.ng_cut("PartitionManager::revealMapForPlayer", &[i64::from(player)]);
        }
    }
    fn sn_shroud_in_multiplayer(&self) -> bool {
        self.cuts.new_game.shroud_in_multiplayer
    }
    fn sn_use_trees(&self) -> bool {
        self.statics.global_data.the_global_data().is_some_and(|g| g.use_trees)
    }
    fn sn_static_lod(&self) -> StaticLod {
        self.cuts.new_game.static_lod.unwrap_or(StaticLod::High)
    }
    fn sn_use_shadow_volumes(&self) -> bool {
        self.statics.global_data.the_global_data().is_some_and(|g| g.use_shadow_volumes)
    }
    fn sn_tree_drawable(&mut self, t: u32, angle: f32, pos: Coord3D) {
        self.ng_cut("tree drawable", &[i64::from(t), i64::from(angle.to_bits()), i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits())]);
    }
    fn sn_add_prop(&mut self, t: u32, pos: Coord3D, angle: f32) {
        self.ng_cut("TerrainVisual::addProp", &[i64::from(t), i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits()), i64::from(angle.to_bits())]);
    }
    fn sn_validate_team(&mut self, owner: &[u8]) -> R<Option<u32>> {
        player_list_validate_team(self, owner).map_err(|_| IniError::undefined(crate::common::ini::Undefined::NullObject))
    }
    fn sn_player_exists(&self, i: i32) -> bool {
        self.players.get_nth(i).is_some()
    }
    fn sn_object_casts_reflections(&self, obj: u32) -> bool {
        self.find_object(obj as ObjectId).is_some_and(|o| o.is_kind_of(KINDOF_CAN_CAST_REFLECTIONS))
    }
    fn sn_draws_in_mirror(&mut self, obj: u32) {
        self.ng_cut("Drawable::setDrawableStatus(DRAWS_IN_MIRROR)", &[i64::from(obj)]);
    }
    fn sn_set_layer_for(&mut self, obj: u32, pos: Coord3D) {
        let layer = self.with_terrain(|t, w| t.layer_for_destination(w, &pos)).unwrap_or(crate::common::game_type::LAYER_GROUND);
        self.set_layer(obj as ObjectId, layer);
    }
    fn sn_team_set_active(&mut self, team: u32) {
        let _ = team_set_active(self, team);
    }
    fn sn_set_slot_player_template(&mut self, slot: usize, t: i32) {
        let ng = &mut self.cuts.new_game;
        let Some(s) = ng.selected else { return };
        if let Some((_, g)) = ng.games.iter_mut().find(|(src, _)| *src == s) {
            if let Some(sl) = g.slots.get_mut(slot) {
                sl.set_player_template(t);
            }
        }
        if let Some(sl) = ng.taken.as_mut().and_then(|g| g.slots.get_mut(slot)) {
            sl.set_player_template(t);
        }
    }
    fn sn_is_internet_game(&self) -> bool {
        self.cuts.new_game.internet_game
    }
    fn sn_preload_assets(&mut self) {
        self.ng_cut("preload assets", &[]);
    }
    fn sn_view_defaults(&mut self) {
        self.ng_cut("TacticalView defaults", &[]);
    }
    fn sn_recorder_init_controls(&mut self) {
        self.ng_cut("Recorder::initControls", &[]);
    }
    fn sn_initial_camera_name(&self) -> Vec<u8> {
        b"InitialCameraPosition".to_vec()
    }
    fn sn_view_look_at(&mut self, pos: Coord3D) {
        self.ng_cut("TacticalView::lookAt", &[i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits())]);
    }
    fn sn_view_init_height(&mut self) {
        self.ng_cut("TacticalView::initHeightForMap", &[]);
    }
    fn sn_player_list_new_map(&mut self) {
        if let Err(e) = player_list_new_map(self) {
            self.cuts.team_live_errors.push(format!("PlayerList::newMap: {e:?}"));
        }
    }
    fn sn_players_with_relationship(&self, player: i32, allowed: u32) -> u16 {
        player_list_get_players_with_relationship(self, player, allowed).unwrap_or(0)
    }
    fn sn_set_relationship(&mut self, a: i32, b: i32, rel: i32) {
        let r = match rel {
            0 => Relationship::Enemies,
            2 => Relationship::Allies,
            _ => Relationship::Neutral,
        };
        let _ = player_set_player_relationship(self, a, Some(b), r);
    }
    fn sn_player_count(&self) -> i32 {
        self.players.player_count()
    }
    fn sn_neutral_player(&self) -> Option<i32> {
        self.players.get_nth(0).map(|p| p.player_index)
    }
    fn sn_player_is_human(&self, i: i32) -> bool {
        self.players.get_nth(i).is_some_and(|p| p.player_type == crate::common::game_common::PlayerType::Human)
    }
    fn sn_add_skill_points(&mut self, player: i32, points: i32) {
        let _ = player_add_skill_points(self, player, points);
    }
    fn sn_network_load_progress_complete(&mut self) {
        self.ng_cut("Network::loadProgressComplete", &[]);
    }
    fn sn_sleep(&mut self, _ms: u32) {}
    fn sn_transition_fade(&mut self) {
        self.ng_cut("TransitionHandler::setGroup(FadeWholeScreen)", &[]);
    }
    fn sn_transition_finished(&mut self) -> bool {
        true
    }
    fn sn_window_manager_update(&mut self) {}
    fn sn_display_draw(&mut self) {}
    fn sn_shell_screen(&mut self) {
        self.ng_cut("Shell main menu", &[]);
    }
    fn sn_stats_collector_reset(&mut self) {
        self.ng_cut("StatsCollector::reset", &[]);
    }
    fn sn_replay_local_player(&mut self, observer: Option<i32>) {
        self.ng_cut("replay local player", &[i64::from(observer.unwrap_or(-1))]);
    }
    fn sn_control_bar_scheme(&mut self) {
        self.ng_cut("ControlBar::setControlBarSchemeByPlayer", &[]);
    }
    fn sn_view_ok_to_adjust_height(&mut self) {
        self.ng_cut("TacticalView::setOkToAdjustHeight", &[]);
    }
    fn sn_player_active(&self, i: i32) -> bool {
        self.players.get_nth(i).is_some() && player_is_player_active(self, i).unwrap_or(false)
    }
    fn sn_player_objects(&self, i: i32) -> Vec<u32> {
        self.player_iterate_objects(i).into_iter().map(|o| o as u32).collect()
    }
    fn sn_is_command_center(&self, obj: u32) -> bool {
        self.find_object(obj as ObjectId).is_some_and(|o| o.is_kind_of(KINDOF_COMMANDCENTER))
    }
    fn sn_controlling_player_mask(&self, obj: u32) -> Option<u16> {
        self.get_controlling_player(obj as ObjectId).map(|p| 1u16.checked_shl(p as u32).unwrap_or(0))
    }
    fn sn_locally_controlled(&self, obj: u32) -> bool {
        self.is_locally_controlled(obj as ObjectId)
    }
    fn sn_control_bar_finish(&mut self, shell: bool) {
        self.ng_cut("ControlBar::initSpecialPowershortcutBar", &[i64::from(shell)]);
    }
    fn sn_gamespy_playing(&mut self) {}
    fn sn_drawables_on_level_start(&mut self) {
        self.ng_cut("Drawable::onLevelStart", &[]);
    }
    fn sn_gamespy_disconnections(&mut self) {}
    fn sn_replay_message(&mut self) {}
}

#[cfg(test)]
mod tests {
    //! `startNewGame` itself is checked against the compiled GameLogic.cpp by `tests/differential_game_logic_new_game.rs`
    //! with a recording host; this checks a game set up in the world from a map's sides and objects.
    use super::*;
    use crate::common::recorder::game_mode;
    use crate::logic::world::player_host_world::tests::world;

    /// A single-player map without game options: the map's sides (the neutral one, and `Player1`, human, America) and a
    /// tank owned by `teamPlayer1`. `startNewGame(FALSE)` marks the game pending; the next frame starts it: the replay
    /// observer's side is added, the player list is made from the sides (the neutral player, `Player1`, the observer),
    /// and the tank is made on `Player1`'s team.
    #[test]
    fn a_frame_starts_a_pending_game_from_the_map() {
        let (mut w, _) = world();
        w.cuts.live_teams = true;
        w.cuts.game_loop.game_mode = game_mode::SINGLE_PLAYER;
        w.cuts.new_game.colors = vec![(0x00ff_ffff, 0x0080_8080)];
        w.cuts.new_game.map_name = b"Maps\\Test\\Test.map".to_vec();
        w.sides = crate::logic::sides_list::SidesList::new();
        for (name, human, faction) in [(&b""[..], false, &b""[..]), (b"Player1", true, b"FactionAmerica")] {
            let mut d = Dict::new();
            let k = w.ng_key(wk::PLAYER_NAME);
            d.set_ascii_string(k, name).unwrap();
            let k = w.ng_key(wk::PLAYER_IS_HUMAN);
            d.set_bool(k, human).unwrap();
            let k = w.ng_key(wk::PLAYER_FACTION);
            d.set_ascii_string(k, faction).unwrap();
            w.sides.add_side(&d);
        }
        let tank = World::thing_factory_find_template(&w, b"Tank").map(u32::from);
        let mut props = Dict::new();
        let k = w.names.name_to_key(b"originalOwner");
        props.set_ascii_string(k, b"teamPlayer1").unwrap();
        let obj = MapObj {
            name: b"Tank".to_vec(),
            template: tank,
            bridge_or_road: false,
            draws_in_mirror: false,
            location: Coord3D { x: 120.0, y: 80.0, z: 0.0 },
            angle: 0.0,
            original_owner: b"teamPlayer1".to_vec(),
        };
        w.cuts.new_game.map_objects = vec![(obj, props)];

        w.start_new_game(false).unwrap();
        assert!(w.start_new_game_pending());
        w.logic_update().unwrap();
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        let names: Vec<Vec<u8>> = (0..w.players.player_count())
            .map(|i| w.names.key_to_name(crate::common::name_key_generator::NameKeyType(w.players.get_nth(i).unwrap().name_key as u32)).to_vec())
            .collect();
        assert_eq!(names, [b"".to_vec(), b"Player1".to_vec(), b"ReplayObserver".to_vec()]);
        let tanks: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|o| w.find_object(*o).unwrap().template().get_name() == b"Tank").collect();
        assert_eq!(tanks.len(), 1);
        assert_eq!(w.get_controlling_player(tanks[0]), Some(1));
        assert_eq!(w.find_object(tanks[0]).unwrap().get_position().x, 120.0);
    }
}

#[cfg(test)]
mod dispatched_tests {
    use crate::common::message_types::MSG_NEW_GAME;
    use crate::common::recorder::game_mode;
    use crate::logic::world::player_host_world::tests::world;

    /// `MSG_NEW_GAME` (single player, hard, 7 rank points) dispatched in a frame: the game mode and global difficulty are
    /// set, the game is pending, and the rank points are `GameLogic`'s once the frame ends.
    #[test]
    fn a_dispatched_new_game_is_pending_with_its_rank_points() {
        let (mut w, _) = world();
        w.cuts.game_loop.game_mode = game_mode::SHELL;
        w.push_command(MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SINGLE_PLAYER);
            m.append_integer_argument(2);
            m.append_integer_argument(7);
        });
        w.logic_update().unwrap();
        assert_eq!(w.cuts.game_loop.game_mode, game_mode::SINGLE_PLAYER);
        assert_eq!(w.cuts.player_host.global_difficulty, crate::common::game_common::GameDifficulty::Hard);
        assert!(w.start_new_game_pending());
        assert_eq!(w.game_logic.rank_points_to_add_at_game_start, 7);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
