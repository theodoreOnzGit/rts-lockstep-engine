// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`startNewGame`, `loadMapINI`, `updateLoadProgress`, `deleteLoadScreen`, `findAndSelectCommandCenter`, and the
//   `LOAD_PROGRESS_*` enum)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameLogic::startNewGame`: a map loaded and a game set up, from the game's options to the first frame (layer 12
//! batch L3b).
//!
//! # Shape
//!
//! [`start_new_game`] follows upstream's order call for call; every subsystem is a call on [`NewGameHost`] (over
//! [`StartHost`], batch L3a's, and [`GameLogicHost`], batch L1's), and the decisions are here: which game options to
//! use, the sides and teams the slots make (each side's dictionary is a [`SideDict`]), whether the multiplayer
//! scripts are added, which map objects become bridges, props, trees and objects, the network players' starting
//! buildings, the camera, the challenge campaign's relationships, the skill points, the wait for the other players,
//! the screen's fade, the shell or the control bar, and the command centers selected in a multiplayer game. The
//! game's options are taken from the host and put back around each part that reads them (`TheGameInfo` names them
//! throughout upstream).
//!
//! # Release build
//!
//! `DUMP_PERF_STATS`, `_PROFILE`, `DO_UNIT_TIMINGS` and the debug logs are not compiled.
//!
//! # Upstream facts kept
//!
//! - A first call (not loading a save, no new game pending) only sets the pristine map name, makes a single-player
//!   game's load screen, and marks a new game pending: the game starts on the next logic frame.
//! - A single-player game without game options deletes `TheSkirmishGameInfo`.
//! - `loadMapINI` cuts the map's path four characters from the end back to the last separator (a name without one
//!   loads `\map.ini`), and uses the pristine map's directory for a map in the save directory.
//! - Each occupied slot's side is local when the slot is human and its name equals the local slot's (the local slot
//!   dereferenced unchecked); the replay observer's side and team are always added.
//! - A multiplayer scripts file that does not parse ends `startNewGame` there (the map is half set up).
//! - `getColor` of a colour out of range is NULL, dereferenced; so are the local player, the replay observer, the
//!   neutral player (for bridges and for `validateTeam`), a map object's NULL team, and `TheRecorder` at the end.
//! - The wait for the other players polls `isProgressComplete` and `testTimeOut`, sleeping 100 ms each time.
//! - In a multiplayer game (by the recorder) each active player's first command center is selected.

use crate::common::base_type::Coord3D;
use crate::common::game_common::normalize_angle;
use crate::common::game_info::{slot_state, GameInfo, GameSlot, MAX_SLOTS, PLAYERTEMPLATE_OBSERVER};
use crate::common::ini::{IniError, Undefined};
use crate::common::recorder::game_mode;
use crate::logic::modules::plant;
use crate::logic::world::game_logic_start::{
    check_for_duplicate_colors, handle_name_change, load_screen_kind, place_network_buildings_for_player, populate_random_side_and_color,
    populate_random_start_position, LoadScreenKind, StartHost,
};
use crate::logic::world::game_logic_update::{is_progress_complete, last_heard_from, select_object, test_time_out, GameLogicHost, GameLogicState};

type R<T> = Result<T, IniError>;

fn undefined(u: Undefined) -> IniError {
    IniError::undefined(u)
}

/// The progress points of the load screen (`LOAD_PROGRESS_*`).
pub mod load_progress {
    use crate::common::game_info::MAX_SLOTS;
    pub const START: i32 = 0;
    pub const POST_PARTICLE_INI_LOAD: i32 = START + 1;
    pub const POST_LOAD_MAP: i32 = POST_PARTICLE_INI_LOAD + 1;
    pub const SIDE_POPULATION: i32 = POST_LOAD_MAP + 1;
    pub const POST_SIDE_LIST_INIT: i32 = SIDE_POPULATION + 1 + MAX_SLOTS as i32;
    pub const POST_PLAYER_LIST_RESET: i32 = POST_SIDE_LIST_INIT + 1;
    pub const POST_SCRIPT_ENGINE_NEW_MAP: i32 = POST_PLAYER_LIST_RESET + 1;
    pub const POST_VICTORY_CONDITION_SETUP: i32 = POST_SCRIPT_ENGINE_NEW_MAP + 2;
    pub const POST_VICTORY_CONDITION_SET_VICTORY_CONDITION: i32 = POST_VICTORY_CONDITION_SETUP + 1;
    pub const POST_GHOST_OBJECT_MANAGER_RESET: i32 = POST_VICTORY_CONDITION_SET_VICTORY_CONDITION + 1;
    pub const POST_TERRAIN_LOGIC_NEW_MAP: i32 = POST_GHOST_OBJECT_MANAGER_RESET + 1;
    pub const POST_BRIDGE_LOAD: i32 = POST_TERRAIN_LOGIC_NEW_MAP + 1;
    pub const POST_PATHFINDER_NEW_MAP: i32 = POST_BRIDGE_LOAD + 1;
    pub const LOOP_ALL_THE_FREAKN_OBJECTS: i32 = POST_BRIDGE_LOAD + 1;
    pub const MAX_ALL_THE_FREAKN_OBJECTS: i32 = 80;
    pub const LOOP_INITIAL_NETWORK_BUILDINGS: i32 = MAX_ALL_THE_FREAKN_OBJECTS + 1;
    pub const POST_INITIAL_NETWORK_BUILDINGS: i32 = LOOP_INITIAL_NETWORK_BUILDINGS + MAX_SLOTS as i32 + 1;
    pub const POST_PRELOAD_ASSETS: i32 = POST_INITIAL_NETWORK_BUILDINGS + 1;
    pub const POST_STARTING_CAMERA: i32 = POST_PRELOAD_ASSETS + 1;
    pub const POST_STARTING_CAMERA_2: i32 = POST_STARTING_CAMERA + 1;
    pub const END: i32 = 100;
}

/// `DIFFICULTY_EASY`, `DIFFICULTY_NORMAL`, `DIFFICULTY_HARD`.
pub const DIFFICULTY_EASY: i32 = 0;
pub const DIFFICULTY_NORMAL: i32 = 1;
pub const DIFFICULTY_HARD: i32 = 2;
/// `Relationship`: `ENEMIES`, `NEUTRAL`, `ALLIES`.
pub const ENEMIES: i32 = 0;
pub const NEUTRAL: i32 = 1;
pub const ALLIES: i32 = 2;
/// `ALLOW_ENEMIES`.
pub const ALLOW_ENEMIES: u32 = 0x04;

/// Where the game's options come from (`TheGameInfo` is set to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSource {
    /// No game options (a campaign, the shell).
    None,
    /// `TheLAN->GetMyGame()`.
    Lan,
    /// `TheGameSpyGame`.
    GameSpy,
    /// `TheRecorder->getGameInfo()` (a replay).
    Recorder,
    /// `TheSkirmishGameInfo`.
    Skirmish,
    /// `TheChallengeGameInfo`.
    Challenge,
}

/// A side's dictionary as `startNewGame` fills it for a slot (`TheSidesList->addSide(&d)`).
#[derive(Clone, Debug, PartialEq)]
pub struct SideDict {
    pub player_name: Vec<u8>,
    pub is_human: bool,
    pub display_name: Vec<u16>,
    /// `playerFaction`, when a template was found.
    pub faction: Option<Vec<u8>>,
    /// `playerIsPreorder` (set only when true).
    pub is_preorder: bool,
    pub allies: Vec<u8>,
    pub enemies: Vec<u8>,
    pub color: i32,
    pub night_color: i32,
    pub start_index: i32,
    pub is_local: bool,
    /// `playerIsSkirmish` and `skirmishDifficulty` (a skirmish game, or its replay): the difficulty when the slot is
    /// an AI.
    pub skirmish: Option<Option<i32>>,
}

/// A map object as `startNewGame` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapObj {
    pub name: Vec<u8>,
    pub template: Option<u32>,
    /// `getFlag(FLAG_BRIDGE_FLAGS) || getFlag(FLAG_ROAD_FLAGS)`.
    pub bridge_or_road: bool,
    /// `getFlag(FLAG_DRAWS_IN_MIRROR)`.
    pub draws_in_mirror: bool,
    pub location: Coord3D,
    pub angle: f32,
    /// The properties' `originalOwner` (empty when there is none).
    pub original_owner: Vec<u8>,
}

/// A template's kinds `startNewGame` asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    WalkOnTopOfWall,
    Shrubbery,
    OptimizedTree,
    Prop,
    ClearedByBuild,
}

/// `TheGameLODManager->getStaticLODLevel()`, as far as it matters here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticLod {
    High,
    Custom,
    Other,
}

/// The world as `startNewGame` reaches it, beyond [`StartHost`] and [`GameLogicHost`].
pub trait NewGameHost: StartHost + GameLogicHost {
    /// `setLoadingMap(on)`.
    fn sn_set_loading_map(&mut self, on: bool);
    /// `TheGlobalData->m_mapName`.
    fn sn_map_name(&self) -> Vec<u8>;
    /// `TheGameState->setPristineMapName(name)`.
    fn sn_set_pristine_map_name(&mut self, name: &[u8]);
    /// `TheGameState->getSaveGameInfo()->pristineMapName`.
    fn sn_pristine_map_name(&self) -> Vec<u8>;
    /// `TheGameState->isInSaveDirectory(path)`.
    fn sn_is_in_save_directory(&self, path: &[u8]) -> bool;
    /// `m_background != NULL`, then its windows destroyed and it deleted.
    fn sn_destroy_background(&mut self) -> bool;
    /// `m_loadScreen != NULL`.
    fn sn_has_load_screen(&self) -> bool;
    /// `m_loadScreen = getLoadScreen(...)` (`None` is `NULL`).
    fn sn_set_load_screen(&mut self, kind: Option<LoadScreenKind>);
    /// `m_loadScreen->init(game)` (`with_game`: the game's options, else `NULL`).
    fn sn_load_screen_init(&mut self, with_game: bool);
    /// `m_loadScreen->update(progress)`.
    fn sn_load_screen_update(&mut self, progress: i32);
    /// `delete m_loadScreen; m_loadScreen = NULL`.
    fn sn_delete_load_screen(&mut self);
    /// `TheWritableGlobalData->m_loadScreenRender = on`.
    fn sn_set_load_screen_render(&mut self, on: bool);
    /// `TheWritableGlobalData->m_TiVOFastMode = FALSE`.
    fn sn_clear_tivo_fast_mode(&mut self);
    /// `setDefaults(loadingSaveGame)`.
    fn sn_set_defaults(&mut self, loading_save_game: bool);
    /// `m_rankLevelLimit = limit`.
    fn sn_set_rank_level_limit(&mut self, limit: i32);
    /// `m_frame = frame`.
    fn sn_set_frame(&mut self, frame: u32);
    /// `TheCampaignManager->getCurrentCampaign()`: `Some(isChallengeCampaign)` when there is one.
    fn sn_current_campaign(&self) -> Option<bool>;
    /// `TheCampaignManager != NULL`, then its `SetVictorious(FALSE)`.
    fn sn_campaign_not_victorious(&mut self) -> bool;
    /// `TheLAN != NULL`.
    fn sn_has_lan(&self) -> bool;
    /// `TheGameInfo = game = ...` from that source; whether there is a game.
    fn sn_select_game(&mut self, source: GameSource) -> bool;
    /// The game's options, taken out (the host keeps `TheGameInfo` naming them) and put back by [`Self::sn_put_game`].
    fn sn_take_game(&mut self) -> GameInfo;
    /// The game's options, put back.
    fn sn_put_game(&mut self, game: GameInfo);
    /// `delete TheSkirmishGameInfo; TheSkirmishGameInfo = NULL`.
    fn sn_delete_skirmish_game_info(&mut self);
    /// `TheMapCache != NULL`.
    fn sn_has_map_cache(&self) -> bool;
    /// `TheFileSystem->doesFileExist(path)`.
    fn sn_file_exists(&mut self, path: &[u8]) -> bool;
    /// `INI ini; ini.load(path, INI_LOAD_CREATE_OVERRIDES, NULL)`.
    fn sn_load_ini(&mut self, path: &[u8]) -> R<()>;
    /// `TheGameText->initMapStringFile(path)`.
    fn sn_init_map_string_file(&mut self, path: &[u8]);
    /// `TheDisplay != NULL`, then its `doSmartAssetPurgeAndPreload(path)`.
    fn sn_asset_purge_and_preload(&mut self, path: &[u8]) -> bool;
    /// `TheTerrainLogic->loadMap(name, false)`.
    fn sn_terrain_load_map(&mut self, name: &[u8]) -> R<()>;
    /// `TheGameEngine->isMultiplayerSession()`.
    fn sn_is_multiplayer_session(&self) -> bool;
    /// `TheSidesList->prepareForMP_or_Skirmish()`.
    fn sn_sides_prepare_for_mp(&mut self);
    /// `ThePlayerTemplateStore->findPlayerTemplate(NAMEKEY("FactionObserver"))`'s index in the store.
    fn sn_observer_template(&self) -> Option<i32>;
    /// `KEYNAME(template->getNameKey())` of a template that exists.
    fn sn_template_name(&self, i: i32) -> Vec<u8>;
    /// `TheMultiplayerSettings->getColor(index)`: its colour and night colour (`None` when out of range: NULL).
    fn sn_color(&self, index: i32) -> Option<(i32, i32)>;
    /// `TheSidesList->addSide(&d)`.
    fn sn_add_side(&mut self, side: &SideDict);
    /// `TheSidesList->addTeam(&d)` of a singleton team.
    fn sn_add_team(&mut self, team_name: &[u8], owner: &[u8]);
    /// `TheSidesList->validateSides()`.
    fn sn_validate_sides(&mut self);
    /// `TheTeamFactory->reset()`.
    fn sn_team_factory_reset(&mut self);
    /// `ThePlayerList->newGame()`.
    fn sn_player_list_new_game(&mut self);
    /// `TheScriptEngine->newMap()`.
    fn sn_script_engine_new_map(&mut self);
    /// `Data\Scripts\MultiplayerScripts.scb` opened, parsed, and the first player's scripts added to side 0 (each at
    /// index 0): `None` when it does not open, `Some(false)` when the parse fails.
    fn sn_add_multiplayer_scripts(&mut self) -> Option<bool>;
    /// `TheRadar->newMap(TheTerrainLogic)`.
    fn sn_radar_new_map(&mut self);
    /// `TheInGameUI->setClientQuiet(FALSE)`.
    fn sn_ui_client_not_quiet(&mut self);
    /// `TheVictoryConditions->cachePlayerPtrs()` and `setVictoryConditions(VICTORY_NOBUILDINGS)`.
    fn sn_victory_setup(&mut self);
    /// `TheTerrainLogic->getExtent(&extent)`: its low and high corners.
    fn sn_terrain_extent(&self) -> (Coord3D, Coord3D);
    /// `setWidth(w)` and `setHeight(h)`.
    fn sn_set_world_size(&mut self, width: f32, height: f32);
    /// `ThePartitionManager->init()` and `refreshShroudForLocalPlayer()`.
    fn sn_partition_init(&mut self);
    /// `ThePlayerList->getLocalPlayer()`'s index (`None`: NULL).
    fn sn_local_player(&self) -> Option<i32>;
    /// `TheGhostObjectManager->setLocalPlayerIndex(index)` and `reset()`.
    fn sn_ghost_reset(&mut self, local_player: i32);
    /// `TheTerrainLogic->newMap(loadingSaveGame)`.
    fn sn_terrain_new_map(&mut self, loading_save_game: bool);
    /// The map objects (`MapObject::getFirstMapObject` and on).
    fn sn_map_objects(&self) -> Vec<MapObj>;
    /// A map object's name and template set (`handleNameChange`).
    fn sn_set_map_object(&mut self, index: usize, name: &[u8], template: Option<u32>);
    /// The template's `isBridge()`.
    fn sn_template_is_bridge(&self, t: u32) -> bool;
    /// The template's `isKindOf(kind)`.
    fn sn_template_is(&self, t: u32, kind: TemplateKind) -> bool;
    /// The template's `getFenceWidth()`.
    fn sn_template_fence_width(&self, t: u32) -> f32;
    /// `ThePlayerList->getNeutralPlayer() != NULL`.
    fn sn_has_neutral_player(&self) -> bool;
    /// `TheThingFactory->newObject(template, the neutral player's default team)`.
    fn sn_new_object_neutral(&mut self, t: u32) -> R<Option<u32>>;
    /// `TheThingFactory->newObject(template, team)` (`None`: a NULL team).
    fn sn_new_object_for_team(&mut self, t: u32, team: Option<u32>) -> R<Option<u32>>;
    /// `TheRecorder->getGameMode()` (the recorded game's mode).
    fn sn_recorder_game_mode(&self) -> i32;
    /// `TheTerrainLogic->addLandmarkBridgeToLogic(obj)`.
    fn sn_add_landmark_bridge(&mut self, obj: u32);
    /// `TheAI->pathfinder()->addWallPiece(obj)`.
    fn sn_add_wall_piece(&mut self, obj: u32);
    /// `obj->updateObjValuesFromMapProperties(map object's properties)`.
    fn sn_update_obj_from_map(&mut self, obj: u32, map_object: usize);
    /// `TheRadar->refreshTerrain(TheTerrainLogic)` and `TheAI->pathfinder()->newMap()`.
    fn sn_radar_refresh_and_pathfinder_new_map(&mut self);
    /// `ThePlayerList->findPlayerWithNameKey(NAMEKEY(name))`'s index.
    fn sn_find_player(&self, name: &[u8]) -> Option<i32>;
    /// `ThePartitionManager->revealMapForPlayerPermanently(index)`.
    fn sn_reveal_permanently(&mut self, player: i32);
    /// `ThePartitionManager->revealMapForPlayer(index)`.
    fn sn_reveal(&mut self, player: i32);
    /// `TheMultiplayerSettings->isShroudInMultiplayer()`.
    fn sn_shroud_in_multiplayer(&self) -> bool;
    /// `TheGlobalData->m_useTrees`.
    fn sn_use_trees(&self) -> bool;
    /// `TheGameLODManager->getStaticLODLevel()`.
    fn sn_static_lod(&self) -> StaticLod;
    /// `TheGlobalData->m_useShadowVolumes`.
    fn sn_use_shadow_volumes(&self) -> bool;
    /// An optimized tree's drawable made, turned, placed and destroyed (the client's).
    fn sn_tree_drawable(&mut self, t: u32, angle: f32, pos: Coord3D);
    /// `TheTerrainVisual->addProp(template, &pos, angle)`.
    fn sn_add_prop(&mut self, t: u32, pos: Coord3D, angle: f32);
    /// `ThePlayerList->validateTeam(owner)`: the team named so, else the neutral player's default team (`None`: NULL);
    /// an error when there is no neutral player to ask.
    fn sn_validate_team(&mut self, owner: &[u8]) -> R<Option<u32>>;
    /// `ThePlayerList->getNthPlayer(i) != NULL`.
    fn sn_player_exists(&self, i: i32) -> bool;
    /// The object's `isKindOf(KINDOF_CAN_CAST_REFLECTIONS)`.
    fn sn_object_casts_reflections(&self, obj: u32) -> bool;
    /// The object's drawable (if any) set to `DRAWABLE_STATUS_DRAWS_IN_MIRROR`.
    fn sn_draws_in_mirror(&mut self, obj: u32);
    /// `obj->setLayer(TheTerrainLogic->getLayerForDestination(&pos))`.
    fn sn_set_layer_for(&mut self, obj: u32, pos: Coord3D);
    /// `team->setActive()`.
    fn sn_team_set_active(&mut self, team: u32);
    /// `slot->setPlayerTemplate(t)` on the game's slot.
    fn sn_set_slot_player_template(&mut self, slot: usize, t: i32);
    /// `isInInternetGame()`.
    fn sn_is_internet_game(&self) -> bool;
    /// The assets preloaded (`TheGlobalData->m_preloadAssets`, all times of day or the current one) and the control
    /// bar's communicator shown.
    fn sn_preload_assets(&mut self);
    /// `TheTacticalView->setAngleAndPitchToDefault()` and `setZoomToDefault()`.
    fn sn_view_defaults(&mut self);
    /// `TheRecorder->initControls()`.
    fn sn_recorder_init_controls(&mut self);
    /// `TheNameKeyGenerator->keyToName(TheKey_InitialCameraPosition)`.
    fn sn_initial_camera_name(&self) -> Vec<u8>;
    /// `TheTacticalView->lookAt(&pos)`.
    fn sn_view_look_at(&mut self, pos: Coord3D);
    /// `TheTacticalView->initHeightForMap()`.
    fn sn_view_init_height(&mut self);
    /// `ThePlayerList->newMap()`.
    fn sn_player_list_new_map(&mut self);
    /// `ThePlayerList->getPlayersWithRelationship(index, allowed)`.
    fn sn_players_with_relationship(&self, player: i32, allowed: u32) -> u16;
    /// `a->setPlayerRelationship(b, rel)`.
    fn sn_set_relationship(&mut self, a: i32, b: i32, rel: i32);
    /// `ThePlayerList->getPlayerCount()`.
    fn sn_player_count(&self) -> i32;
    /// `ThePlayerList->getNeutralPlayer()`'s index.
    fn sn_neutral_player(&self) -> Option<i32>;
    /// `ThePlayerList->getNthPlayer(i)` exists and `getPlayerType() == PLAYER_HUMAN`.
    fn sn_player_is_human(&self, i: i32) -> bool;
    /// `player->addSkillPoints(points)`.
    fn sn_add_skill_points(&mut self, player: i32, points: i32);
    /// `TheNetwork->loadProgressComplete()` and `liteupdate()`.
    fn sn_network_load_progress_complete(&mut self);
    /// `Sleep(ms)`.
    fn sn_sleep(&mut self, ms: u32);
    /// `TheTransitionHandler->setGroup("FadeWholeScreen")`.
    fn sn_transition_fade(&mut self);
    /// `TheTransitionHandler->isFinished()`.
    fn sn_transition_finished(&mut self) -> bool;
    /// `TheWindowManager->update()`.
    fn sn_window_manager_update(&mut self);
    /// `TheDisplay->draw()`.
    fn sn_display_draw(&mut self);
    /// The shell's main menu pushed (or its top screen shown), and the control bar hidden.
    fn sn_shell_screen(&mut self);
    /// `TheStatsCollector` reset, or made when `TheGlobalData->m_playStats > 0`.
    fn sn_stats_collector_reset(&mut self);
    /// A replay: the local player set to the replay observer (perhaps NULL), the radar forced on, the shroud
    /// refreshed.
    fn sn_replay_local_player(&mut self, observer: Option<i32>);
    /// `TheControlBar->setControlBarSchemeByPlayer(ThePlayerList->getLocalPlayer())`.
    fn sn_control_bar_scheme(&mut self);
    /// `TheTacticalView->setOkToAdjustHeight(TRUE)`.
    fn sn_view_ok_to_adjust_height(&mut self);
    /// `ThePlayerList->getNthPlayer(i)` exists and `isPlayerActive()`.
    fn sn_player_active(&self, i: i32) -> bool;
    /// The player's objects, in `iterateObjects`' order.
    fn sn_player_objects(&self, i: i32) -> Vec<u32>;
    /// `obj->isKindOf(KINDOF_COMMANDCENTER)`.
    fn sn_is_command_center(&self, obj: u32) -> bool;
    /// `obj->getControllingPlayer()->getPlayerMask()` (`None`: no controlling player).
    fn sn_controlling_player_mask(&self, obj: u32) -> Option<u16>;
    /// `obj->isLocallyControlled()`.
    fn sn_locally_controlled(&self, obj: u32) -> bool;
    /// `TheControlBar->initSpecialPowershortcutBar(local player)` and the control bar hidden (shell) or shown.
    fn sn_control_bar_finish(&mut self, shell: bool);
    /// The GameSpy buddy status set to playing (an internet game with the buddy queue and `TheGameSpyGame`).
    fn sn_gamespy_playing(&mut self);
    /// Each drawable's `onLevelStart()`.
    fn sn_drawables_on_level_start(&mut self);
    /// `TheGameSpyInfo->updateAdditionalGameSpyDisconnections(1)`, when there is one.
    fn sn_gamespy_disconnections(&mut self);
    /// A replay with the UI and the game text: the fast-forward instructions shown.
    fn sn_replay_message(&mut self);
}

/// `updateLoadProgress(progress)`.
pub fn update_load_progress(progress: i32, h: &mut impl NewGameHost) {
    if h.sn_has_load_screen() {
        h.sn_load_screen_update(progress);
    }
}

/// `loadMapINI(mapName)`: the map's directory (or the pristine map's, for a map in the save directory) and its
/// `map.ini`, `solo.ini` (as overrides), `map.str` and asset usage list.
pub fn load_map_ini(map_name: &[u8], h: &mut impl NewGameHost) -> R<()> {
    if !h.sn_has_map_cache() {
        return Ok(());
    }
    // strcpy into a _MAX_PATH buffer
    if map_name.len() >= 260 {
        return Err(undefined(Undefined::MessageOverflow));
    }
    let mut filename = map_name.to_vec();
    if h.sn_is_in_save_directory(&filename) {
        filename = h.sn_pristine_map_name();
        if filename.len() >= 260 {
            return Err(undefined(Undefined::MessageOverflow));
        }
    }
    if filename.len() < 4 {
        return Ok(());
    }
    // back from four before the end to the last separator (or the start), cut there
    let mut e = filename.len() - 4;
    while e > 0 && filename[e] != b'\\' && filename[e] != b'/' {
        e -= 1;
    }
    filename.truncate(e);
    let path = |leaf: &[u8]| -> R<Vec<u8>> {
        let mut p = filename.clone();
        p.push(b'\\');
        p.extend_from_slice(leaf);
        // sprintf into a _MAX_PATH buffer
        if p.len() >= 260 {
            return Err(undefined(Undefined::MessageOverflow));
        }
        Ok(p)
    };
    let p = path(b"map.ini")?;
    if h.sn_file_exists(&p) {
        h.sn_load_ini(&p)?;
    }
    let p = path(b"solo.ini")?;
    if h.sn_file_exists(&p) {
        h.sn_load_ini(&p)?;
    }
    let p = path(b"map.str")?;
    if h.sn_file_exists(&p) {
        h.sn_init_map_string_file(&p);
    }
    let p = path(b"AssetUsage.txt")?;
    h.sn_asset_purge_and_preload(&p);
    Ok(())
}

/// `startNewGame(loadingSaveGame)`.
pub fn start_new_game(st: &mut GameLogicState, loading_save_game: bool, h: &mut impl NewGameHost) -> R<()> {
    use load_progress as lp;
    h.sn_set_loading_map(true);
    if !loading_save_game {
        let m = h.sn_map_name();
        h.sn_set_pristine_map_name(&m);
        if !h.u_start_new_game_pending() {
            if h.u_game_mode() == game_mode::SINGLE_PLAYER {
                h.sn_destroy_background();
                let kind = load_screen_kind(h.u_game_mode(), h.sn_current_campaign(), loading_save_game);
                h.sn_set_load_screen(kind);
                if h.sn_has_load_screen() {
                    h.sn_set_load_screen_render(true);
                    h.sn_load_screen_init(false);
                }
            }
            h.u_set_start_new_game_pending(true);
            return Ok(());
        }
    }
    h.sn_set_rank_level_limit(1000);
    h.sn_set_defaults(loading_save_game);
    h.sn_set_load_screen_render(true);
    h.sn_clear_tivo_fast_mode();
    st.show_behind_building_markers = true;
    st.draw_icon_ui = true;
    st.show_dynamic_lod = true;
    st.script_hulk_max_lifetime_override = -1;
    let mode = h.u_game_mode();
    let is_challenge = mode == game_mode::SINGLE_PLAYER && h.sn_current_campaign() == Some(true);
    let source = if h.u_has_network() {
        if h.sn_has_lan() {
            GameSource::Lan
        } else {
            GameSource::GameSpy
        }
    } else if h.u_has_recorder() && h.u_recorder_playback() {
        GameSource::Recorder
    } else if mode == game_mode::SKIRMISH {
        GameSource::Skirmish
    } else if is_challenge {
        GameSource::Challenge
    } else {
        GameSource::None
    };
    let has_game = h.sn_select_game(source);
    let mut local_slot = 0usize;
    let mut skirmish_or_replay = false;
    if has_game {
        let mut game = h.sn_take_game();
        if !loading_save_game {
            st.superweapon_restriction = game.superweapon_restriction;
        }
        check_for_duplicate_colors(Some(&mut game), h);
        for s in game.slots.iter_mut() {
            if !loading_save_game {
                s.save_off_original_info();
            }
            if s.is_ai() {
                skirmish_or_replay = true;
            }
        }
        let r = populate_random_side_and_color(Some(&mut game), h).and_then(|_| populate_random_start_position(Some(&mut game), h));
        h.sn_put_game(game);
        r?;
    } else {
        if !loading_save_game {
            st.superweapon_restriction = 0;
        }
        if mode == game_mode::SINGLE_PLAYER {
            h.sn_delete_skirmish_game_info();
        }
    }
    if !h.sn_has_load_screen() {
        let kind = load_screen_kind(mode, h.sn_current_campaign(), loading_save_game);
        h.sn_set_load_screen(kind);
        if h.sn_has_load_screen() {
            h.u_mouse_set_visible(false);
            h.sn_load_screen_init(has_game);
            update_load_progress(lp::START, h);
        }
    }
    h.sn_destroy_background();
    h.u_set_fp_mode();
    h.sn_campaign_not_victorious();
    h.u_set_start_new_game_pending(false);
    if h.sn_has_load_screen() {
        update_load_progress(lp::POST_PARTICLE_INI_LOAD, h);
    }
    h.sn_set_frame(0);
    let map = h.sn_map_name();
    load_map_ini(&map, h)?;
    h.sn_terrain_load_map(&map)?;
    update_load_progress(lp::POST_LOAD_MAP, h);

    let progress_count = lp::SIDE_POPULATION;
    if has_game {
        if h.sn_is_multiplayer_session() || skirmish_or_replay {
            h.sn_sides_prepare_for_mp();
        }
        let game = h.sn_take_game();
        let r = add_slot_sides(st, &game, progress_count, skirmish_or_replay, &mut local_slot, h);
        h.sn_put_game(game);
        r?;
    }
    // the replay observer's side and team, always
    let faction = h.sn_observer_template().map(|t| h.sn_template_name(t));
    let (color, night_color) = h.sn_color(0).ok_or_else(|| undefined(Undefined::NullObject))?;
    let observer = SideDict {
        player_name: b"ReplayObserver".to_vec(),
        is_human: true,
        display_name: "Observer".encode_utf16().collect(),
        faction,
        is_preorder: false,
        allies: Vec::new(),
        enemies: Vec::new(),
        color,
        night_color,
        start_index: 0,
        is_local: false,
        skirmish: None,
    };
    h.sn_add_side(&observer);
    h.sn_add_team(b"teamReplayObserver", b"ReplayObserver");
    h.sn_validate_sides();
    update_load_progress(lp::POST_SIDE_LIST_INIT, h);
    h.sn_team_factory_reset();
    h.sn_player_list_new_game();
    update_load_progress(lp::POST_PLAYER_LIST_RESET, h);
    h.sn_script_engine_new_map();
    update_load_progress(lp::POST_SCRIPT_ENGINE_NEW_MAP, h);
    if h.sn_is_multiplayer_session() || skirmish_or_replay {
        let mut num_teams = 0;
        let mut last_team = -1;
        if has_game {
            let game = h.sn_take_game();
            for s in game.slots.iter() {
                if s.is_occupied() && s.player_template != PLAYERTEMPLATE_OBSERVER && (s.team_number == -1 || s.team_number != last_team) {
                    num_teams += 1;
                    last_team = s.team_number;
                }
            }
            h.sn_put_game(game);
        }
        if num_teams > 1 && h.sn_add_multiplayer_scripts() == Some(false) && !plant(4033) {
            // the parse failed: startNewGame returns here
            return Ok(());
        }
    }
    update_load_progress(lp::POST_VICTORY_CONDITION_SETUP, h);
    h.sn_radar_new_map();
    h.sn_ui_client_not_quiet();
    h.sn_victory_setup();
    update_load_progress(lp::POST_VICTORY_CONDITION_SET_VICTORY_CONDITION, h);
    let (lo, hi) = h.sn_terrain_extent();
    h.sn_set_world_size(hi.x - lo.x, hi.y - lo.y);
    h.sn_partition_init();
    let local = h.sn_local_player().ok_or_else(|| undefined(Undefined::NullLocalPlayer))?;
    h.sn_ghost_reset(local);
    update_load_progress(lp::POST_GHOST_OBJECT_MANAGER_RESET, h);
    h.sn_terrain_new_map(loading_save_game);
    update_load_progress(lp::POST_TERRAIN_LOGIC_NEW_MAP, h);

    // bridges and wall walkers first
    let objects = h.sn_map_objects();
    for (index, m) in objects.iter().enumerate() {
        if m.bridge_or_road {
            continue;
        }
        let Some(t) = m.template else {
            continue;
        };
        let is_bridge = h.sn_template_is_bridge(t);
        let walks = h.sn_template_is(t, TemplateKind::WalkOnTopOfWall);
        if !is_bridge && !walks {
            continue;
        }
        if !h.sn_has_neutral_player() {
            return Err(undefined(Undefined::NullObject));
        }
        if let Some(obj) = h.sn_new_object_neutral(t)? {
            let mut pos = m.location;
            pos.z += h.st_ground_height(pos.x, pos.y);
            let angle = normalize_angle(m.angle);
            h.st_set_orientation(obj, angle);
            h.st_set_position(obj, pos);
            if is_bridge {
                h.sn_add_landmark_bridge(obj);
            }
            if walks {
                h.sn_add_wall_piece(obj);
            }
            h.sn_update_obj_from_map(obj, index);
        }
    }
    update_load_progress(lp::POST_BRIDGE_LOAD, h);
    h.sn_radar_refresh_and_pathfinder_new_map();
    update_load_progress(lp::POST_PATHFINDER_NEW_MAP, h);
    let observer_player = h.sn_find_player(b"ReplayObserver").ok_or_else(|| undefined(Undefined::NullObject))?;
    h.sn_reveal_permanently(observer_player);
    if has_game {
        let game = h.sn_take_game();
        let slots: Vec<GameSlot> = game.slots.to_vec();
        h.sn_put_game(game);
        for (i, s) in slots.iter().enumerate() {
            if !s.is_occupied() {
                continue;
            }
            // player->getPlayerIndex(), the player unchecked where it is read
            let player = h.sn_find_player(format!("player{i}").as_bytes());
            if s.player_template == PLAYERTEMPLATE_OBSERVER {
                h.sn_reveal_permanently(player.ok_or_else(|| undefined(Undefined::NullObject))?);
            } else if !h.sn_shroud_in_multiplayer() {
                h.sn_reveal(player.ok_or_else(|| undefined(Undefined::NullObject))?);
            }
        }
    }
    let mut use_trees = h.sn_use_trees();
    let lod = h.sn_static_lod();
    let mut force_fluff_to_prop = lod != StaticLod::High;
    if lod == StaticLod::Custom && h.sn_use_shadow_volumes() {
        force_fluff_to_prop = false;
    }
    if h.u_has_recorder() && h.u_recorder_is_multiplayer() {
        use_trees = true;
        force_fluff_to_prop = true;
    }
    let mut progress_count = lp::LOOP_ALL_THE_FREAKN_OBJECTS;
    let mut timer = h.u_time_get_time();
    if loading_save_game {
        for m in objects.iter() {
            let Some(t) = m.template else {
                continue;
            };
            if h.sn_template_is(t, TemplateKind::Shrubbery) && !use_trees {
                continue;
            }
            let mut pos = m.location;
            pos.z += h.st_ground_height(pos.x, pos.y);
            let angle = normalize_angle(m.angle);
            if h.sn_template_is(t, TemplateKind::OptimizedTree) {
                h.sn_tree_drawable(t, angle, pos);
            }
        }
    } else {
        for (index, m0) in objects.iter().enumerate() {
            if m0.bridge_or_road {
                continue;
            }
            let mut m = m0.clone();
            handle_name_change(&mut m.name, &mut m.template, h);
            if m.name != m0.name {
                h.sn_set_map_object(index, &m.name, m.template);
            }
            let Some(t) = m.template else {
                continue;
            };
            if h.sn_template_is_bridge(t) || h.sn_template_is(t, TemplateKind::WalkOnTopOfWall) {
                continue;
            }
            if h.sn_template_is(t, TemplateKind::Shrubbery) && !use_trees {
                continue;
            }
            let mut pos = m.location;
            pos.z += h.st_ground_height(pos.x, pos.y);
            let angle = normalize_angle(m.angle);
            if h.sn_template_is(t, TemplateKind::OptimizedTree) {
                h.sn_tree_drawable(t, angle, pos);
                continue;
            }
            let is_prop = h.sn_template_is(t, TemplateKind::Prop);
            let is_fluff = h.sn_template_is(t, TemplateKind::ClearedByBuild) && h.sn_template_fence_width(t) == 0.0;
            if is_prop || (is_fluff && force_fluff_to_prop && !plant(4031)) {
                h.sn_add_prop(t, pos, angle);
                continue;
            }
            let team = h.sn_validate_team(&m.original_owner)?;
            if let Some(obj) = h.sn_new_object_for_team(t, team)? {
                if m.draws_in_mirror || h.sn_object_casts_reflections(obj) {
                    h.sn_draws_in_mirror(obj);
                }
                h.st_set_orientation(obj, angle);
                h.st_set_position(obj, pos);
                h.sn_update_obj_from_map(obj, index);
                h.sn_set_layer_for(obj, pos);
                h.st_build_complete(obj);
                // team->setActive(), the team unchecked
                let team = team.ok_or_else(|| undefined(Undefined::NullTeam))?;
                h.sn_team_set_active(team);
                h.st_add_to_pathfind_map(obj);
            }
            // timeGetTime() > timer + 500: an UnsignedInt against an Int sum, compared unsigned
            if h.u_time_get_time() as u32 > timer.wrapping_add(500) as u32 {
                if progress_count < lp::MAX_ALL_THE_FREAKN_OBJECTS {
                    progress_count += 1;
                }
                update_load_progress(progress_count, h);
                timer = h.u_time_get_time();
            }
        }
    }

    let mut progress_count = lp::LOOP_INITIAL_NETWORK_BUILDINGS;
    if has_game && !loading_save_game {
        let game = h.sn_take_game();
        let slots: Vec<GameSlot> = game.slots.to_vec();
        let internet_old_only = game.old_factions_only;
        h.sn_put_game(game);
        for (i, s) in slots.iter().enumerate() {
            if !s.is_occupied() {
                continue;
            }
            let player = h.sn_find_player(format!("player{i}").as_bytes());
            if s.player_template == PLAYERTEMPLATE_OBSERVER {
                h.sn_set_slot_player_template(i, 0);
                if let Some(obs) = h.sn_observer_template() {
                    if (0..h.st_player_template_count()).any(|j| j == obs) {
                        h.sn_set_slot_player_template(i, obs);
                    }
                }
            } else {
                let t = s.player_template;
                // pt->getName(), the template unchecked
                if !h.st_player_template_exists(t) {
                    return Err(undefined(Undefined::NullObject));
                }
                if h.st_general_starts_locked(t) {
                    continue;
                }
                if h.sn_is_internet_game() && internet_old_only && !h.st_is_old_faction(t) {
                    continue;
                }
                // the player is dereferenced once a building is placed; the translation asks for it at once
                let player = player.ok_or_else(|| undefined(Undefined::NullObject))?;
                place_network_buildings_for_player(s.start_pos, player, t, h)?;
            }
            update_load_progress(progress_count, h);
            progress_count += 1;
        }
    }
    update_load_progress(lp::POST_INITIAL_NETWORK_BUILDINGS, h);
    h.sn_preload_assets();
    update_load_progress(lp::POST_PRELOAD_ASSETS, h);
    h.sn_view_defaults();
    if h.u_has_recorder() {
        h.sn_recorder_init_controls();
    }
    let mut camera = h.sn_initial_camera_name();
    if has_game {
        let game = h.sn_take_game();
        let slot = game.slots[local_slot].clone();
        h.sn_put_game(game);
        if slot.is_human() {
            camera = format!("Player_{}_Start", slot.start_pos.wrapping_add(1)).into_bytes();
        }
    }
    update_load_progress(lp::POST_STARTING_CAMERA, h);
    let pos = h.st_waypoint(&camera).unwrap_or(Coord3D { x: 50.0, y: 50.0, z: 0.0 });
    h.sn_view_look_at(pos);
    h.sn_view_init_height();
    h.sn_view_defaults();
    update_load_progress(lp::POST_STARTING_CAMERA_2, h);
    h.st_partition_update();
    if !loading_save_game {
        h.sn_player_list_new_map();
    }
    if is_challenge {
        challenge_relationships(h)?;
    }
    // isInSinglePlayerGame(): a single-player game, or the replay of one
    let single = mode == game_mode::SINGLE_PLAYER || (h.u_has_recorder() && h.u_recorder_playback() && h.sn_recorder_game_mode() == game_mode::SINGLE_PLAYER);
    if !loading_save_game && single {
        for i in 0..16 {
            if h.sn_player_is_human(i) && !plant(4035) {
                h.sn_add_skill_points(i, st.rank_points_to_add_at_game_start);
            }
        }
    }
    update_load_progress(lp::END, h);
    let multiplayer = mode == game_mode::LAN || mode == game_mode::INTERNET;
    if multiplayer && h.u_has_network() {
        h.sn_network_load_progress_complete();
    }
    while !is_progress_complete(st, h) {
        update_load_progress(101, h);
        test_time_out(st, h);
        h.sn_sleep(100);
    }
    if !loading_save_game {
        h.sn_transition_fade();
        while !h.sn_transition_finished() {
            h.sn_window_manager_update();
            if !h.sn_transition_finished() {
                h.sn_display_draw();
                h.u_set_fp_mode();
                h.sn_sleep(33);
            }
        }
    }
    if h.sn_has_load_screen() {
        h.u_mouse_set_visible(true);
        h.sn_delete_load_screen();
    }
    if mode == game_mode::SHELL {
        h.sn_shell_screen();
    } else {
        h.sn_stats_collector_reset();
        if mode == game_mode::REPLAY {
            let o = h.sn_find_player(b"ReplayObserver");
            h.sn_replay_local_player(o);
        }
        h.sn_control_bar_scheme();
    }
    h.sn_view_ok_to_adjust_height();
    // TheRecorder->isMultiplayer(), unchecked
    if !h.u_has_recorder() {
        return Err(undefined(Undefined::NullObject));
    }
    if h.u_recorder_is_multiplayer() {
        for i in 0..16 {
            if !h.sn_player_active(i) {
                continue;
            }
            for obj in h.sn_player_objects(i) {
                if h.sn_is_command_center(obj) {
                    let mask = h.sn_controlling_player_mask(obj).ok_or_else(|| undefined(Undefined::NullControllingPlayer))?;
                    let local = h.sn_locally_controlled(obj);
                    select_object(Some(obj), true, mask, local, h);
                    if !plant(4036) {
                        break;
                    }
                }
            }
        }
    }
    h.sn_control_bar_finish(mode == game_mode::SHELL);
    h.sn_set_load_screen_render(false);
    if h.sn_is_internet_game() {
        h.sn_gamespy_playing();
    }
    if !loading_save_game {
        h.sn_drawables_on_level_start();
    }
    h.sn_set_loading_map(false);
    h.sn_gamespy_disconnections();
    if mode == game_mode::REPLAY {
        h.sn_replay_message();
    }
    Ok(())
}

/// The sides and teams of the game's occupied slots, and which slot is local.
fn add_slot_sides(st: &mut GameLogicState, game: &GameInfo, progress_count: i32, skirmish: bool, local_slot: &mut usize, h: &mut impl NewGameHost) -> R<()> {
    let local_num = game.local_slot_num();
    for i in 0..MAX_SLOTS {
        let slot = &game.slots[i];
        if !slot.is_human() {
            st.progress_complete[i] = true;
            last_heard_from(st, i as i32, h);
        }
        if !slot.is_occupied() {
            continue;
        }
        let player_name = format!("player{i}").into_bytes();
        let template = if slot.player_template >= 0 {
            h.st_player_template_exists(slot.player_template).then_some(slot.player_template)
        } else {
            h.sn_observer_template()
        };
        let faction = template.map(|t| h.sn_template_name(t));
        let team = slot.team_number;
        let (mut allies, mut enemies) = (Vec::new(), Vec::new());
        for j in 0..MAX_SLOTS {
            let other = &game.slots[j];
            if i == j || !other.is_occupied() {
                continue;
            }
            let name = format!("player{j}").into_bytes();
            let list = if (team == -1 && !plant(4032)) || other.team_number != team { &mut enemies } else { &mut allies };
            if !list.is_empty() {
                list.push(b' ');
            }
            list.extend_from_slice(&name);
        }
        let (color, night_color) = h.sn_color(slot.color).ok_or_else(|| undefined(Undefined::NullObject))?;
        let is_local = slot.is_human() && {
            // game->getSlot(game->getLocalSlotNum())->getName(): the local slot unchecked
            let local = usize::try_from(local_num).ok().and_then(|n| game.slots.get(n)).ok_or_else(|| undefined(Undefined::NullObject))?;
            slot.name == local.name
        };
        let skirmish_entry = skirmish.then_some(match slot.state {
            slot_state::EASY_AI => Some(DIFFICULTY_EASY),
            slot_state::MED_AI => Some(DIFFICULTY_NORMAL),
            slot_state::BRUTAL_AI => Some(DIFFICULTY_HARD),
            _ => None,
        });
        // slotNameAscii.translate(slot->getName()): each character cut to a byte
        let ascii: Vec<u8> = slot.name.iter().map(|&c| c as u8).collect();
        if slot.is_human() && game.slot_num(&ascii) == local_num {
            *local_slot = i;
        }
        let side = SideDict {
            player_name: player_name.clone(),
            is_human: slot.is_human(),
            display_name: slot.name.clone(),
            faction,
            is_preorder: game.is_player_preorder(i as i32),
            allies,
            enemies,
            color,
            night_color,
            start_index: slot.start_pos,
            is_local,
            skirmish: skirmish_entry,
        };
        h.sn_add_side(&side);
        let mut team_name = b"team".to_vec();
        team_name.extend_from_slice(&player_name);
        h.sn_add_team(&team_name, &player_name);
        update_load_progress(progress_count + i as i32, h);
    }
    Ok(())
}

/// A challenge campaign: the local player made the enemy of "ThePlayer"'s enemies (or, without "ThePlayer", of
/// everyone but the neutral player and "PlyrCivilian", and its own ally).
fn challenge_relationships(h: &mut impl NewGameHost) -> R<()> {
    let local = h.sn_local_player().ok_or_else(|| undefined(Undefined::NullLocalPlayer))?;
    match h.sn_find_player(b"ThePlayer") {
        Some(the_player) => {
            let mut mask = h.sn_players_with_relationship(the_player, ALLOW_ENEMIES);
            while mask != 0 {
                let enemy = h.u_each_player_from_mask(&mut mask).ok_or_else(|| undefined(Undefined::NullObject))?;
                h.sn_set_relationship(enemy, local, ENEMIES);
                h.sn_set_relationship(local, enemy, ENEMIES);
            }
        }
        None => {
            let neutral = h.sn_neutral_player();
            let civilian = h.sn_find_player(b"PlyrCivilian");
            for i in 0..h.sn_player_count() {
                // thatPlayer->setPlayerRelationship, the player unchecked
                if !h.sn_player_exists(i) {
                    return Err(undefined(Undefined::NullObject));
                }
                let rel = if i == local && !plant(4034) {
                    ALLIES
                } else if Some(i) != neutral && Some(i) != civilian {
                    ENEMIES
                } else {
                    NEUTRAL
                };
                h.sn_set_relationship(i, local, rel);
                h.sn_set_relationship(local, i, rel);
            }
        }
    }
    Ok(())
}
