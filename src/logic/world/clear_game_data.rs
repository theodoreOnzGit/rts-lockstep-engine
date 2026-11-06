// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp (`GameLogic::clearGameData`)
//   GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp (`GameEngine::reset`)
//   GeneralsMD/Code/GameEngine/Source/Common/System/SubsystemInterface.cpp (`SubsystemInterfaceList::resetAll`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (`GameLogic::reset`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp (`AI::reset`)
//   and the `reset()` of each subsystem the World holds (named at each call below)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #64: `GameLogic::clearGameData` and `TheGameEngine->reset()`, so that a World can start its next game in place,
//! as upstream's one engine does.
//!
//! - **[`World::clear_game_data`]** is `clearGameData(showScoreScreen)`: refused outside a game (upstream's
//!   `DEBUG_CRASH`), the victory or defeat window closed, the score screen pushed (the client's: recorded in
//!   [`World::game_cleared`] for it), `TheGameEngine->reset()`, the game mode `GAME_NONE`.
//! - **[`World::engine_reset`]** is `GameEngine::reset`: `TheSubsystemList->resetAll()`, each subsystem's `reset()` in the
//!   reverse of `GameEngine::init`'s order. The World holds the logic's subsystems; the client's (`TheGameClient`,
//!   `TheRadar`, `TheParticleSystemManager`, `TheAudio`, `TheMetaMap`, ...) are the client's, which makes its own anew
//!   for a new game.
//! - **Who calls it:** the dispatcher's `MSG_CLEAR_GAME_DATA` (`DispatchHost::d_clear_game_data`), which the script
//!   engine's end-game timer appends (`ScriptEngineHost::se_append_clear_game_data`, both cuts until #64); the
//!   recorder's playback and the campaign starts (`RecorderHost::clear_game_data`); `GameState::loadGame`'s prologue
//!   ([`World::load_game`] when the World starts the save's own game, with `FALSE`; the generators then start afresh,
//!   a deviation documented there); the skirmish menu's `reallyDoStart` ([`World::start_skirmish_slots`], `FALSE`).
//!
//! **During a frame.** `MSG_CLEAR_GAME_DATA` is dispatched inside `GameLogic::update`, while the frame holds
//! `GameLogic`'s own members (`World::game_logic`): their part of `GameLogic::reset` is queued
//! (`GameLogicWrite::Reset`) and applied when the frame gives them back, after the frame's other writes. The rest of
//! the frame then runs on the empty World, as upstream's does (its frame counter, which `setDefaults` set to 0, then
//! advances to 1; `startNewGame` sets it to 0 again).
//!
//! **What the reset keeps, as upstream's does:** `TheCampaignManager` and `TheChallengeGenerals` (the client's
//! `GameClient::reset` leaves them), `TheChallengeGameInfo` (a global), `TheNameKeyGenerator` (not a listed subsystem),
//! the message stream's and command list's messages (`GameMessageList::reset` is empty, `TheCommandList` is not
//! listed), the random generators (`InitRandom` and `InitGameLogicRandom` reseed them at the next start), the file
//! system (the World's disk) and `TheMapCache`. Each of these is checked against a fresh World by the test below, which
//! also lists what upstream's own resets leave (`KEPT`: the boundaries `TerrainLogic::reset` keeps, the zone manager's
//! counters, the script engine's first-update flag and fades, the players' relationship maps until `initFromDict`); none
//! changes the next game's CRCs, measured.

use crate::common::recorder::{game_mode, RecorderHost};
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_update::{GameLogicState, MAX_SLOTS};

/// What [`World::clear_game_data`] did, for the client (the score screen is the client's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameCleared {
    /// The game mode of the game cleared.
    pub mode: i32,
    /// `clearGameData`'s `TheShell->push("Menus/ScoreScreen.wnd")`: the score screen opens.
    pub score_screen: bool,
}

/// `GameLogic::reset()`'s part on `GameLogic`'s own members (those `World::game_logic` holds).
pub(crate) fn reset_game_logic_state(st: &mut GameLogicState) {
    st.buildable_overrides.clear();
    st.control_bar_overrides.clear();
    st.game_paused = false;
    st.input_enabled_memory = true;
    st.mouse_visible_memory = true;
    st.crc = 0;
    st.progress_complete = [false; MAX_SLOTS];
    st.progress_complete_timeout = [0; MAX_SLOTS];
    st.force_game_start_by_timeout = false;
    st.object_toc.clear();
    st.scoring_enabled = true;
    st.show_behind_building_markers = true;
    st.draw_icon_ui = true;
    st.show_dynamic_lod = true;
    st.script_hulk_max_lifetime_override = -1;
    st.rank_points_to_add_at_game_start = 0;
}

impl World {
    /// `GameLogic::clearGameData(showScoreScreen)`. Whether there was a game to clear (`isInGame()`; upstream's
    /// `DEBUG_CRASH` and return otherwise).
    pub fn clear_game_data(&mut self, show_score_screen: bool) -> bool {
        if !RecorderHost::is_in_game(self) {
            self.cuts.team_live_errors.push("GameLogic::clearGameData: not in a game (DEBUG_CRASH upstream)".to_string());
            return false;
        }
        // setClearingGameData(TRUE)
        self.clearing_game_data = true;
        // TheStatsCollector->writeFileEnd() (no stats collector: `TheGlobalData->m_saveStats` is off in a release build)
        // TheScriptActions->closeWindows(FALSE): "Close victory or defeat windows."
        self.script_actions_close_windows(false);
        let mode = self.cuts.game_loop.game_mode;
        // `(!isInShellGame() || !isInGame()) && showScoreScreen`: the score screen (the client's shell; a fade, the
        // shell's push, `FixupScoreScreenMovieWindow`)
        let score_screen = mode != game_mode::SHELL && show_score_screen;
        // TheGameEngine->reset()
        self.engine_reset();
        // setGameMode(GAME_NONE)
        self.cuts.game_loop.game_mode = game_mode::NONE;
        // (`m_initialFile`'s quit, `HideControlBar`, `closeWindows`, the mouse, the background window: the client's)
        self.game_cleared = Some(GameCleared { mode, score_screen });
        // setClearingGameData(FALSE)
        self.clearing_game_data = false;
        true
    }

    /// `GameEngine::reset()`: `TheSubsystemList->resetAll()` (`TheNetwork` is not the logic's), each listed subsystem's
    /// `reset()` in the reverse of `GameEngine::init`'s order, for those the World holds. The comments name the client's
    /// in their place.
    pub fn engine_reset(&mut self) {
        // TheGameResultsQueue, TheGameState (made per save and load in the port: nothing held), TheGameStateMap (no
        // reset), TheActionManager (no state), TheMetaMap (the client's)
        // TheVictoryConditions->reset()
        self.victory.reset();
        // TheRadar->reset() (the client's)
        // TheRecorder->reset(): the file closed, its name cleared, `init()`
        self.with_recorder(|r, w| r.reset(w));
        // ThePlayerList->reset(): TheTeamFactory->clear(), then init()
        if let Err(e) = crate::logic::world::player_list::player_list_reset(self) {
            self.cuts.team_live_errors.push(format!("PlayerList::reset: {e:?}"));
        }
        // TheCrateSystem->reset(): the statics' overrides, below with the other stores
        // TheTeamFactory->reset()
        if let Err(e) = crate::logic::world::team_ops::factory_reset(self) {
            self.cuts.team_live_errors.push(format!("TeamFactory::reset: {e:?}"));
        }
        // TheGameLogic->reset()
        self.game_logic_reset();
        // TheAI->reset() (its data's overrides: below with the stores)
        self.ai_reset();
        // TheGameClient->reset() (the client's)
        // TheUpgradeCenter (button images only), TheThingFactory, TheBuildAssistant, TheArmorStore, TheDamageFXStore,
        // TheSpecialPowerStore, TheLocomotorStore, TheObjectCreationListStore, TheWeaponStore, TheFXListStore,
        // TheParticleSystemManager (the client's), ThePlayerTemplateStore (keeps its list), TheRankInfoStore, ...
        crate::logic::world::build_assistant::build_assistant_reset(&mut self.build_assistant);
        self.reset_stores();
        // TheCaveSystem->reset()
        self.cave_system.reset();
        // TheSidesList->reset()
        self.sides.reset();
        // TheMessageStream->reset(): `GameMessageList::reset()` is empty. TheModuleFactory, TheFunctionLexicon,
        // TheAudio, TheCDManager, TheGlobalLanguageData, TheTerrainRoads, TheTerrainTypes, TheMultiplayerSettings: no
        // state a game changes. TheScienceStore and TheGlobalData: the stores, above. TheGameText->reset(): the map's
        // strings (`initMapStringFile` is a cut).
    }

    /// `GameLogic::reset()`.
    fn game_logic_reset(&mut self) {
        // m_thingTemplateBuildableOverrides, m_controlBarOverrides, m_gamePaused, the memories, m_CRC, the progress,
        // m_objectTOC, the options, m_rankPointsToAddAtGameStart: `GameLogic`'s members, lent to the frame while it runs
        if self.in_logic_frame {
            self.cuts.pending_game_logic_writes.push(crate::logic::world::object_cuts::GameLogicWrite::Reset);
        } else {
            reset_game_logic_state(&mut self.game_logic);
        }
        // m_objVector cleared; setFPMode(); destroyAllObjectsImmediate(); m_nextObjID = 1
        self.reset();
        self.frame_objects_changed_trigger_areas = 0;
        // TheGhostObjectManager->reset() (the client's ghost objects)
        self.cut(crate::logic::world::object_cuts::CutCall::Host { call: "GhostObjectManager::reset", args: Vec::new(), text: Vec::new() });
        // ThePartitionManager->reset()
        self.partition.reset();
        // TheTerrainLogic->reset()
        self.terrain.reset();
        self.flush_terrain_events();
        // TheAI->reset(): GameEngine's own list resets it again after; its first call here is the one with objects gone
        self.ai_reset();
        // TheScriptEngine->reset()
        let mut se = std::mem::take(&mut self.script_engine);
        se.reset(self);
        self.script_engine = se;
        // (delete TheStatsCollector: none)
        // setDefaults(FALSE): m_frame = 0, the object list, m_nextObjID = 1
        self.set_defaults(false);
        // (the water transparency and weather overrides: the client's)
    }

    /// `AI::reset()`: the pathfinder reset, the AI data's overrides deleted (`m_aiData->m_next`), every group destroyed,
    /// the group and formation ids from their start.
    fn ai_reset(&mut self) {
        // m_pathfinder->reset() (which leaves `m_wallHeight`: `newMap` sets it)
        if let Some(pf) = self.pathfinder.as_mut() {
            let wall = pf.wall_height;
            pf.reset(Some(wall));
            self.pf_wall = pf.wall_view();
        }
        if !self.statics.ai_data_overrides.is_empty() {
            let st = std::sync::Arc::make_mut(&mut self.statics);
            let overrides = std::mem::take(&mut st.ai_data_overrides);
            if let Some(original) = overrides.into_iter().last() {
                st.ai_data = original;
            }
        }
        while let Some(id) = self.ai_groups.list.first().map(|g| g.get_id()) {
            self.ai_destroy_group_live(id);
        }
        // m_nextGroupID = 0; m_nextFormationID = NO_FORMATION_ID; getNextFormationID()
        let groups = crate::logic::world::ai_group_world::AiGroups::default();
        self.ai_groups.next_group_id = groups.next_group_id;
        self.ai_groups.next_formation_id = groups.next_formation_id;
    }

    /// The INI stores' `reset()`: each deletes the overrides a map's `map.ini` or `solo.ini` made
    /// (`INI_LOAD_CREATE_OVERRIDES`), and a template an override load made. In `resetAll`'s order: `TheCrateSystem`,
    /// `TheThingFactory`, `TheSpecialPowerStore`, `TheLocomotorStore`, `TheWeaponStore` (its delayed and historic
    /// damage too), `TheRankInfoStore`, `TheScienceStore`, `TheGlobalData`.
    fn reset_stores(&mut self) {
        let st = std::sync::Arc::make_mut(&mut self.statics);
        st.crate_system.reset();
        if let Err(e) = st.thing_factory.reset() {
            self.cuts.team_live_errors.push(format!("ThingFactory::reset: {e:?}"));
        }
        let st = std::sync::Arc::make_mut(&mut self.statics);
        st.special_power_store.reset();
        st.locomotor_store.reset();
        st.weapon_store.reset();
        st.rank_info_store.reset();
        st.science_store.reset();
        st.global_data.reset();
    }

    /// GitHub #64: the logic's part of `GameEngine::init` that a World made by hand or from an install had left out, so
    /// that it is as a World `engine_reset` leaves (upstream's engine after `init` and after `reset` agree there):
    /// `GameLogic::init`'s `TheScriptEngine->init()`, which ends in `reset()`, and `ThePlayerList->init()` (the neutral
    /// player alone, and local). The player list's error, if any.
    pub fn init_engine_subsystems(&mut self) -> Result<(), String> {
        let mut se = std::mem::take(&mut self.script_engine);
        se.reset(self);
        self.script_engine = se;
        crate::logic::world::player_list::player_list_init(self).map_err(|e| format!("PlayerList::init: {e:?}"))
    }

    /// GitHub #64: whether a game is under way: in a game other than the shell's, and `startNewGame` has run (the frame
    /// after `MSG_NEW_GAME` is dispatched). Before it, `ThePlayerList`'s local player is the neutral one (`init`'s), as in
    /// upstream's engine between games.
    pub fn game_started(&self) -> bool {
        let mode = self.cuts.game_loop.game_mode;
        mode != game_mode::NONE && mode != game_mode::SHELL && !self.start_new_game_pending()
    }

    /// GitHub #64 (a deviation, see [`World::load_game`]): the random generators as they are before any `InitRandom`,
    /// for a load into a World that has played (upstream's load seeds nothing, so its loaded game would draw from the
    /// generators' state after whatever ran before it).
    pub fn reset_random_for_load(&mut self) {
        self.random = crate::common::random_value::RandomValues::default();
    }

    /// GitHub #64: the last [`World::clear_game_data`]'s outcome, for the client's score screen.
    pub fn last_game_cleared(&self) -> Option<GameCleared> {
        self.game_cleared
    }
}

#[cfg(test)]
mod tests {
    //! GitHub #64: a World that has played a game, been cleared, and started a new one gives the CRCs of a fresh World
    //! starting that game; and what the clear leaves behind, field by field.
    use super::*;
    use crate::logic::world::campaign_world::tests::{base_of, campaign_archives, frames_to_game_over, start_frames, tanks, whole_fs, USA01, USA02};
    use crate::logic::world::game_logic_update::CRC_RECALC;

    /// Each of the World's members as its `Debug` text (the private ones by their accessors).
    fn fields(w: &World) -> Vec<(&'static str, String)> {
        vec![
            ("objects", format!("{:#?}", w.objects)),
            ("sched", format!("{:#?}", w.sched)),
            ("players", format!("{:#?}", w.players)),
            ("teams", format!("{:#?}", w.teams)),
            ("terrain", format!("{:#?}", w.terrain)),
            ("events", format!("{:#?}", w.events)),
            ("reads", format!("{:#?}", w.reads)),
            ("partition", format!("{:#?}", w.partition)),
            ("hook_log", format!("{:#?}", w.hook_log)),
            ("cuts", format!("{:#?}", w.cuts)),
            ("cave_system", format!("{:#?}", w.cave_system)),
            ("statics", format!("{:#?}", w.statics)),
            ("names", format!("{:#?}", w.names)),
            ("random", format!("{:#?}", w.random)),
            ("keys", format!("{:#?}", w.keys)),
            ("sides", format!("{:#?}", w.sides)),
            ("pending_script_lists", format!("{:#?}", w.pending_script_lists)),
            ("script_cur_id", format!("{:#?}", w.script_cur_id)),
            ("game_logic", format!("{:#?}", w.game_logic)),
            ("dispatch", format!("{:#?}", w.dispatch)),
            ("victory", format!("{:#?}", w.victory)),
            ("build_assistant", format!("{:#?}", w.build_assistant)),
            ("script_engine", format!("{:#?}", w.script_engine)),
            ("script_conditions", format!("{:#?}", w.script_conditions)),
            ("script_actions", format!("{:#?}", w.script_actions)),
            ("pathfinder", format!("{:#?}", w.pathfinder)),
            ("pf_wall", format!("{:#?}", w.pf_wall)),
            ("ai_self", format!("{:#?}", w.ai_self)),
            ("frame_objects_changed_trigger_areas", format!("{:#?}", w.frame_objects_changed_trigger_areas)),
            ("team_scripts", format!("{:#?}", w.team_scripts)),
            ("in_logic_frame", format!("{:#?}", w.in_logic_frame)),
            ("ai_stacks", format!("{:#?}", w.ai_stacks)),
            ("turret_ais", format!("{:#?}", w.turret_ais)),
            ("ai_prone_damage", format!("{:#?}", w.ai_prone_damage)),
            ("ai_groups", format!("{:#?}", w.ai_groups)),
            ("ai_players", format!("{:#?}", w.ai_players)),
            ("save_side", format!("{:#?}", w.save_side)),
            ("campaign", format!("{:#?}", w.campaign)),
            ("challenge_generals", format!("{:#?}", w.challenge_generals)),
            ("collision_other", format!("{:#?}", w.collision_other)),
            ("collision_self_commands", format!("{:#?}", w.collision_self_commands)),
            ("ai_own_calls", format!("{:#?}", w.ai_own_calls)),
            ("ai_update_base", format!("{:#?}", w.ai_update_base)),
            ("clearing_game_data", format!("{:#?}", w.clearing_game_data)),
            ("game_cleared", format!("{:#?}", w.game_cleared)),
            ("frame", format!("{:#?}", w.frame())),
            ("next_object_id", format!("{:#?}", w.next_object_id())),
            ("objects_to_destroy", format!("{:#?}", w.objects_to_destroy())),
            ("start_new_game", format!("{:#?}", w.start_new_game_pending())),
            ("in_update", format!("{:#?}", w.is_in_game_logic_update())),
            ("owner", format!("{:#?}", w.owner())),
        ]
    }

    /// The members of `a` that differ from `b`'s, each with its first differing lines (the nearest enclosing member's
    /// name, as `{:#?}` indents it, before each).
    fn differences(a: &World, b: &World) -> Vec<String> {
        let mut out = Vec::new();
        for ((name, x), (_, y)) in fields(a).into_iter().zip(fields(b)) {
            if x == y {
                continue;
            }
            let (xl, yl): (Vec<&str>, Vec<&str>) = (x.lines().collect(), y.lines().collect());
            let mut shown = Vec::new();
            let mut last_member = String::new();
            for i in 0..xl.len().max(yl.len()) {
                let (p, q) = (xl.get(i).copied().unwrap_or("<end>"), yl.get(i).copied().unwrap_or("<end>"));
                if p.starts_with("    ") && !p.starts_with("     ") && p.trim_end().ends_with(['{', '[', '(']) {
                    last_member = p.trim().to_string();
                }
                if p != q {
                    shown.push(format!("[{last_member}] {} | {}", p.trim(), q.trim()));
                    if shown.len() >= if std::env::var("RTS_DIFF_FIELD").is_ok_and(|f| f == name) { 60 } else { 3 } {
                        break;
                    }
                }
            }
            out.push(format!("{name} ({} vs {} lines): {}", xl.len(), yl.len(), shown.join(" ; ")));
        }
        out
    }

    /// The members a World cleared after a game still holds differently from the install's World (see the test).
    const KEPT: [&str; 13] =
        ["players", "terrain", "events", "partition", "cuts", "names", "keys", "script_engine", "pathfinder", "save_side", "campaign", "game_cleared", "frame"];

    /// GitHub #64: `clearGameData` lets a World start its next game in place.
    ///
    /// **Predicted before the run:** (1) `engine_reset` on the install's World, which has no game, changes no member;
    /// (2) `USA01` (won at once by its script) runs until the end-game timer's `MSG_CLEAR_GAME_DATA` reaches
    /// `clearGameData` on the frame after the timer runs out, 121 to 126 frames after its start frames: the World is then
    /// out of a game (`GAME_NONE`), `game_cleared` says single-player with the score screen, and it holds no object, no
    /// team, the neutral player alone, no sides, no waypoints or triggers, no pathfind map and no partition cells; the
    /// members that still differ from the install's World are only those upstream's reset keeps or that are not
    /// simulation state: the frame (1: `setDefaults` made it 0 and the frame then advanced), the disk (`USA01`'s folder),
    /// the name keys made during the game, the random generators, the campaign (victorious), the message arena, the
    /// recorder (made for the game), the client events not yet taken, the cut logs, `game_cleared`; (3) the score
    /// screen's `gotoNextMission` and Continue (`startNextCampaignGame`) then start `USA02` in that World, and a fresh copy
    /// of the install's World given the same campaign starts it too: the two give the same logic CRC on each of 300
    /// frames, the tank at x 180 in both; (4) cleared again by a `DEFEAT`, the same World starts the duel's skirmish
    /// against a medium computer player (seed 7) with the CRCs of a fresh World's for 300 frames; (5) a save of that
    /// skirmish loaded into the same World while it plays (`clearGameData(FALSE)`, then the engine's reset) and into a
    /// fresh copy of the install's World gives the same CRCs for 300 frames.
    #[test]
    fn a_cleared_world_starts_its_next_game_as_a_fresh_one() {
        let archives = campaign_archives(true);
        let base = base_of(&archives);
        let fs = whole_fs(&archives);
        // (1)
        let mut r = base.clone();
        r.engine_reset();
        let d = differences(&r, &base);
        eprintln!("engine_reset on the install's World: {d:#?}");
        assert!(d.is_empty());
        // (2)
        let mut a = base.clone();
        for m in [USA01, USA02, crate::logic::world::install_world::tests::MAP] {
            a.mount_map(&fs, m);
        }
        a.start_campaign(b"USA", 2).expect("USA01");
        start_frames(&mut a);
        let n = frames_to_game_over(&mut a, 200);
        eprintln!("cleared {n} frames after the start frames; game_cleared {:?}", a.game_cleared);
        assert_eq!(a.game_cleared, Some(GameCleared { mode: game_mode::SINGLE_PLAYER, score_screen: true }));
        assert_eq!(a.cuts.game_loop.game_mode, game_mode::NONE);
        assert!(a.objects.ids_in_order().is_empty());
        let d = differences(&a, &base);
        eprintln!("the cleared World against the install's: {d:#?}");
        let names: Vec<&str> = d.iter().map(|x| x.split(' ').next().unwrap_or("")).collect();
        assert_eq!(names, KEPT);
        assert!(a.cuts.team_live_errors.is_empty(), "{:?}", a.cuts.team_live_errors);
        // (3)
        assert_eq!(a.finish_single_player(), Some(USA02.to_vec()));
        let mut b = base.clone();
        b.mount_map(&fs, USA02);
        b.carry_campaign_from(&a);
        a.start_next_campaign_game().expect("USA02 in place");
        b.start_next_campaign_game().expect("USA02 fresh");
        let run = |a: &mut World, b: &mut World, what: &str| {
            for f in 0..300 {
                a.engine_update().expect("a frame");
                b.engine_update().expect("a frame");
                let (x, y) = (a.logic_crc(CRC_RECALC, None).expect("the CRC"), b.logic_crc(CRC_RECALC, None).expect("the CRC"));
                if x != y {
                    let (mut da, mut db) = (Vec::new(), Vec::new());
                    a.logic_crc(CRC_RECALC, Some(&mut da)).expect("the deep CRC");
                    b.logic_crc(CRC_RECALC, Some(&mut db)).expect("the deep CRC");
                    let at = da.iter().zip(&db).position(|(p, q)| p != q).unwrap_or(da.len().min(db.len()));
                    let show = |d: &[u8]| String::from_utf8_lossy(&d[at.saturating_sub(64)..(at + 64).min(d.len())]).into_owned();
                    panic!(
                        "{what}: in place {x:08x}, fresh {y:08x} at frame {f}; deep CRC streams {} and {} bytes, first difference at {at}: {:?} | {:?}; differences {:#?}",
                        da.len(),
                        db.len(),
                        show(&da),
                        show(&db),
                        differences(a, b)
                    );
                }
            }
            eprintln!("{what}: 300 frames, the CRCs equal, last {:08x}", a.logic_crc(CRC_RECALC, None).unwrap());
        };
        run(&mut a, &mut b, "USA02");
        assert_eq!((tanks(&a), tanks(&b)), (vec![180.0], vec![180.0]));
        // (4)
        let mut def = crate::logic::scripts::ScriptAction::new_for_read();
        def.action_type = crate::logic::scripts::action_type::DEFEAT;
        let mut engine = std::mem::take(&mut a.script_engine);
        a.execute_action_with(&mut engine, &def).expect("DEFEAT");
        a.script_engine = engine;
        frames_to_game_over(&mut a, 200);
        let mut c = base.clone();
        c.mount_map(&fs, crate::logic::world::install_world::tests::MAP);
        let medium = Some((crate::common::game_info::slot_state::MED_AI, 0));
        a.start_skirmish_against(crate::logic::world::install_world::tests::MAP, 0, 7, medium).expect("the skirmish in place");
        c.start_skirmish_against(crate::logic::world::install_world::tests::MAP, 0, 7, medium).expect("the skirmish fresh");
        run(&mut a, &mut c, "the skirmish");
        // (5)
        let bytes = a.save_game().expect("the skirmish's save");
        let mut e = base.clone();
        e.mount_map(&fs, crate::logic::world::install_world::tests::MAP);
        for w in [&mut a, &mut e] {
            w.save_side.new_game_on_load = true;
            w.load_game(bytes.clone()).unwrap_or_else(|x| panic!("the load: {x:?} {:?}", w.cuts.team_live_errors));
        }
        assert_eq!(a.game_cleared, Some(GameCleared { mode: game_mode::SKIRMISH, score_screen: false }));
        run(&mut a, &mut e, "the save loaded");
        for w in [&a, &b, &c, &e] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }
}
