// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp (the file's statics and
//     globals, startNextCampaignGame, ScoreScreenEnableControls, ScoreScreenInit, FixupScoreScreenMovieWindow,
//     ScoreScreenShutdown, ScoreScreenUpdate, ScoreScreenInput, ScoreScreenSystem, initSkirmish, PlayMovieAndBlock,
//     initSinglePlayer, displayChallengeWinLoss, finishSinglePlayerInit, initReplaySinglePlayer, initLANMultiPlayer,
//     initInternetMultiPlayer, initReplayMultiPlayer, populatePlayerInfo (up to its internet branch),
//     grabMultiPlayerInfo, grabSinglePlayerInfo, hideWindows, setObserverWindows, populateSideInfo)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp (clearGameData's score screen half:
//     the fade, the push, showShell(FALSE), FixupScoreScreenMovieWindow)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp
//     (CheckForCDAtGameStart, with startNextCampaignGame as its callback)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   ScoreScreen.cpp: Chris Huybregts, June 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The score screen (GitHub #89 part 2): `Menus/ScoreScreen.wnd`, which `GameLogic::clearGameData` pushes when a game
//! ends, with the per-player score grid read from each player's `ScoreKeeper`.
//!
//! # As upstream reads it
//!
//! **The push** (`clearGameData`, before `TheGameEngine->reset()`): `FadeWholeScreen` set, `Menus/ScoreScreen.wnd`
//! pushed (its init runs at once: the shell's stack top is hidden in a game), `showShell(FALSE)`, the fade reversed,
//! `FixupScoreScreenMovieWindow` (the blank layout, if any, shown and brought forward). The init therefore reads the
//! players before the engine resets them.
//!
//! **Init** (`ScoreScreenInit`): `g_playMusic` and `DontShowMainMenu` set; the window keys made and the windows found
//! under `ScoreScreen.wnd:ParentScoreScreen`; the replay's file name kept; *game saved* hidden; the replay popup
//! (`getPopupReplayLayout`) made and hidden; *Save Replay* enabled only while recording. Then by the game: a replay
//! (multiplayer or single player), an internet game, a LAN game, a skirmish, else single player
//! (`initSinglePlayer`: the campaign's rank points and difficulty from the game, the single player grid, the blank
//! layout made; `finishSinglePlayerInit` waits for the first update). A challenge campaign hides *Save Replay*. The
//! layout shown, the focus on the parent, `ReplayWasPressed` cleared.
//!
//! **The grid.** Multiplayer and skirmish (`grabMultiPlayerInfo`): the players named `player0` .. `player7`, by score
//! (`calculateScore`) highest first (a score already taken is moved up by 1, 2, ...), each row
//! `populatePlayerInfo` (or `setObserverWindows` for an observer), the unused rows hidden (`hideWindows`). Single
//! player (`grabSinglePlayerInfo`): the local player's row (an observer's: the first human player's), then for USA,
//! China and GLA the sum of the allies and then of the enemies of that base side (in a single-player game only those
//! listed on the score screen) as a row of their own (`populateSideInfo`, `GUI:<side>Allies` / `Enemies`). A row is
//! the name (or `GUI:Player`), units built / lost / destroyed, buildings built / lost / destroyed, the money earned,
//! each in the player's colour, and the side's icon. The local player's row fills the academy's advice in a skirmish
//! (or internet) game. A skirmish records the local player's result in the battle honours.
//!
//! **finishSinglePlayerInit** (the first update): after a victory, a challenge's win display; `gotoNextMission`; at the
//! campaign's end *Continue* reads `GUI:EndCampaign` and the campaign's completion goes to the battle honours, the
//! buttons hidden, the final movie played; otherwise *Continue* reads `GUI:SaveAndContinue` and the mission saved
//! (`missionSave`), *game saved* shown. After a defeat a challenge's loss display and *Continue* reads `GUI:Retry`. The
//! blank layout destroyed, *OK* and *Continue* shown, the chat hidden; `ScoreScreenShow` set for a campaign that is
//! not a challenge.
//!
//! **System** (`GBM_SELECTED`): `ScoreScreenShow` removed, `ReplayWasPressed` cleared; *OK* pops the screen and
//! clears the campaign; *Continue* (single player) starts the next mission (`CheckForCDAtGameStart(
//! startNextCampaignGame)`: the screen popped at once, the shell hidden, `MSG_NEW_GAME`), or pops at the campaign's
//! end; *Save Replay* opens the replay popup; *Emote* and the chat entry go to `TheLAN`. **Input**: Escape's release
//! presses *OK*. **Update**: the replay popup's update while shown, `finishSinglePlayerInit` once, the score screen's
//! music once (`TheGameInfo`'s local template's, else `FactionObserver`'s). **Shutdown**: `DontShowMainMenu` cleared,
//! the layout hidden, the shell told.
//!
//! # Upstream quirks that are kept
//!
//! - `GBM_SELECTED` falls through into `GEM_EDIT_DONE` (no `break`): the pressed button is then compared with the chat
//!   entry. Every message, known or not, returns `MSG_HANDLED`.
//! - Two players with the same score: the second is moved up by the running `adder` (1, then 2, ...), and a moved
//!   score that lands on a third player's replaces it in the map: that player has no row.
//! - `populatePlayerInfo` sets `StaticTextBuildingsDestroyed` twice; `hideWindows` and `setObserverWindows` hide it twice.
//! - `grabSinglePlayerInfo` compares with the local player it was given (an observer's first human), but reads the side
//!   image of `getLocalPlayer()` again; an observer with no human player returns before `hideWindows`.
//! - `populateSideInfo` leaves the winner marker as it was when the side has no image.
//! - Upstream checks `pos > MAX_SLOTS` (not `>=`) in `populatePlayerInfo` and `populateSideInfo`: row 8 is looked up,
//!   and a file without it is undefined.
//! - The windows the file uses without a check (`staticTextGameSaved`, `buttonSaveReplay` in a single-player game,
//!   `parent`, the grid's, the challenge's, `s_blankLayout`) are recorded as undefined when missing
//!   ([`GameWindowManager::set_undefined`]); `DEBUG_ASSERTCRASH` is compiled out of the release.
//! - `getNthPlayer(k)` for `k` up to `MAX_PLAYER_COUNT`: the unused players exist (blank, human by default): an
//!   observer's search for the first human can find one of them.
//! - `s_blankLayout` is replaced by a later `initSinglePlayer` without being destroyed.
//!
//! # Where this differs from upstream
//!
//! - **The game's data** the file reads (`ThePlayerList` and each `ScoreKeeper`, `TheGameLogic`'s mode and frame,
//!   `TheRecorder`, `TheGameInfo`, `TheVictoryConditions`, `TheScriptEngine`'s difficulty, `TheCampaignManager`,
//!   `TheChallengeGenerals`, the player templates) is a copy taken where upstream's init reads it, in
//!   `clearGameData` before the engine's reset ([`ScoreScreenGame::from_world`], kept by the World as
//!   `World::score_screen_game`), given to the screen by the client ([`ScoreScreen::game`]). The client never
//!   changes the simulation: `calculateScore`, the campaign's `setRankPoints` / `setGameDifficulty` /
//!   `gotoNextMission` / `setCampaign` and the academy's advice (on a copy of the client random stream) change the
//!   copy; *Continue* hands the copy's campaign to the client ([`ShellRequest::StartNextCampaignGame`]), which starts
//!   the next mission in the World that played, as upstream's one engine does.
//! - **The mission save** (`TheGameState->missionSave()`) is a [`ShellRequest::MissionSave`] with the screen's
//!   campaign; the client writes it to the save folder.
//! - ~~**The battle honours** (`SkirmishBattleHonors`, gap #92): the calls are recorded (`HonorsCall`), the stats file is
//!   not read or written. The campaign's completion (`finishSinglePlayerInit`) is translated as the calls it makes; a
//!   skirmish's update (`populatePlayerInfo`'s tail with `updateSkirmishBattleHonors`, `updateChallengeMedals`) is
//!   recorded as one `HonorsCall::SkirmishResult` once its early return is passed.~~ **Corrected 2026-11-12, #157:**
//!   the battle honours are translated: `SkirmishStats.ini` is read and written through the host's user data folder
//!   ([`crate::client::battle_honors::SkirmishBattleHonors`]); the campaign's completion and the skirmish's tail
//!   ([`skirmish_stats_skipped`], [`skirmish_stats_update`]: `isSlotLocalAlly`, `updateSkirmishBattleHonors`,
//!   `updateChallengeMedals`) are translated. What `updateSkirmishBattleHonors` reads of the game (the superweapons and
//!   units the local player built, the frame, the side) is copied with the game ([`SkirmishHonorsInputs`]).
//!   `updateChallengeMedals`' return for an allied computer cannot change its result: `isSkirmish()` is already false
//!   with one (found by #157's planted bug 9526).
//! - **Multiplayer** (gaps, never faked): `TheLAN` and `TheGameSpyInfo` do not exist in the client (`NULL`): the chat
//!   and emote send nothing, the buddy request of `ButtonAdd<i>` is recorded as undefined (`TheGameSpyInfo` read on
//!   `NULL`), `GameSpyToggleOverlay` is recorded, the internet branch of `populatePlayerInfo` (the stats report,
//!   `updateMPBattleHonors`) and `initInternetMultiPlayer`'s buddy status request are not translated (recorded).
//! - `CopyProtect::validate()` (the launcher's shared memory) is the host's ([`ScoreScreenHost::copy_protect_validate`],
//!   true: as the launcher set it).
//! - **The final movie** (`PlayMovieAndBlock`): Bink was never released; `TheVideoPlayer->open` gives no stream, and the
//!   function returns as upstream's does then. ~~`TheGameLODManager` is not translated~~ **Corrected 2026-11-12 (#160):**
//!   `TheGameLODManager` is translated (`crate::client::game_lod`) but not read here yet (a gap of #160): the movie is
//!   asked for whatever the LOD.
//! - The music (`TheAudio->addAudioEvent`) is the host's audio list; `removeAudioEvent(AHSV_StopTheMusicFade)` and
//!   `TheAudio->update()` are the host's audio (not recorded).
//! - ~~`PopupReplay.wnd`'s callbacks (`PopupReplayUpdate`, its init) are not translated: the lexicon records them.~~
//!   **Corrected 2026-11-10 (#98):** they are translated (`crate::client::popup_replay`): *Save Replay* copies the
//!   last replay in the replay folder; `PopupReplayUpdate` is called through the host's lexicon
//!   ([`crate::client::shell::ShellHost::layout_callback`]).
//! - `TheInGameUI->freeMessageResources()` is the host's ([`ScoreScreenHost::free_message_resources`]).
//!
//! # Verification
//!
//! The grid (`grabMultiPlayerInfo`, `grabSinglePlayerInfo`, `hideWindows`, `setObserverWindows`, `populateSideInfo`,
//! `populatePlayerInfo` to its skirmish tail) is compared with ScoreScreen.cpp compiled (tests/differential_score_grid.rs,
//! 2026-11-10): 20,000 cases, 79,843 named rows, 0 mismatches; planted bugs 8921 .. 8924 each caught. The callbacks are
//! tested from a synthetic `ScoreScreen.wnd`.
//!
//! # Representation
//!
//! [`ScoreScreen`]: the file's statics (`parent`, the buttons, `screenType`, ...) and globals (`DontShowMainMenu`,
//! `ReplayWasPressed`, `g_playMusic`, `LastReplayFileName`: [`ScoreScreenGlobals`]), with the copy of the game it
//! scores. The window ids are the host's name keys ([`crate::client::shell::ShellHost::name_to_key`]).

use std::collections::BTreeMap;

use crate::client::campaign_manager::CampaignManager;
use crate::client::challenge_generals::ChallengeGenerals;
use crate::client::control_bar_popup::{format_wide, FormatArg};
use crate::client::gadget;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::main_menu::MenuHost;
use crate::client::map_util::MapUtilHost;
use crate::client::shell::{self, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::client::window_transitions as wt;
use crate::common::game_common::{PlayerType, Relationship};
use crate::common::game_info::GameInfo;
use crate::common::recorder::{game_mode, RecorderMode};
use crate::logic::modules::plant;
use crate::logic::world::score_keeper::ScoreKeeper;

/// `SCORESCREEN_SINGLEPLAYER` .. `SCORESCREEN_REPLAY`.
pub const SCORESCREEN_SINGLEPLAYER: i32 = 0;
pub const SCORESCREEN_SKIRMISH: i32 = 1;
pub const SCORESCREEN_LAN: i32 = 2;
pub const SCORESCREEN_INTERNET: i32 = 3;
pub const SCORESCREEN_REPLAY: i32 = 4;

/// `MAX_SLOTS`.
pub const MAX_SLOTS: i32 = 8;
/// `MAX_PLAYER_COUNT`.
pub const MAX_PLAYER_COUNT: i32 = 16;
/// `MAX_GLOBAL_GENERAL_TYPES`.
pub const MAX_GLOBAL_GENERAL_TYPES: i32 = 9;

/// BattleHonors.h's bits the file sets (#157: `crate::client::battle_honors`'s).
pub use crate::client::battle_honors::{
    BATTLE_HONOR_AIR_WING, BATTLE_HONOR_APOCALYPSE, BATTLE_HONOR_BATTLE_TANK, BATTLE_HONOR_BLITZ10, BATTLE_HONOR_BLITZ5, BATTLE_HONOR_CAMPAIGN_CHINA, BATTLE_HONOR_CAMPAIGN_GLA, BATTLE_HONOR_CAMPAIGN_USA,
    BATTLE_HONOR_CHALLENGE_MODE, BATTLE_HONOR_LOYALTY_CHINA, BATTLE_HONOR_LOYALTY_GLA, BATTLE_HONOR_LOYALTY_USA, BATTLE_HONOR_STREAK, BH_CHALLENGE_MASK,
};
use crate::client::battle_honors::SkirmishBattleHonors;
use crate::client::user_preferences::{report_undefined, HostUserData, UserDataFiles};
use crate::common::game_info::slot_state;

/// `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// The grid's per-row windows, in the order `populatePlayerInfo` sets them (`StaticTextBuildingsDestroyed` twice, as
/// upstream).
const ROW_STATS: [&str; 7] = ["UnitsBuilt", "UnitsLost", "UnitsDestroyed", "BuildingsBuilt", "BuildingsLost", "BuildingsDestroyed", "BuildingsDestroyed"];

/// A player template's fields the file reads (`getSide`, `getSideIconImage`'s name, `getScoreScreen`,
/// `getScoreScreenMusic`), with its name (for `findPlayerTemplate("FactionObserver")`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScoreTemplate {
    pub name: Vec<u8>,
    pub side: Vec<u8>,
    pub side_icon_image: Vec<u8>,
    pub score_screen_image: Vec<u8>,
    pub score_screen_music: Vec<u8>,
}

/// A player as the file reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScorePlayer {
    /// `m_playerName` (`findPlayerWithNameKey(NAMEKEY("player%d"))` compares its key).
    pub name: Vec<u8>,
    /// `getPlayerDisplayName()`.
    pub display_name: Vec<u16>,
    /// `getPlayerColor()`.
    pub color: i32,
    /// `getScoreKeeper()` (a copy: `calculateScore` changes it).
    pub score_keeper: ScoreKeeper,
    /// `isPlayerObserver()`.
    pub observer: bool,
    /// `isPlayerDead()` (`isPlayerActive()` is `!observer && !dead`).
    pub dead: bool,
    /// `getPlayerType() == PLAYER_HUMAN`.
    pub human: bool,
    /// `getBaseSide()`.
    pub base_side: Vec<u8>,
    /// `getPlayerTemplate()`: an index into [`ScoreScreenGame::templates`].
    pub template: Option<usize>,
    /// `getListInScoreScreen()`.
    pub list_in_score_screen: bool,
    /// `getSkillPoints()`.
    pub skill_points: i32,
}

/// What the score screen reads of the game that ended, copied where upstream's init reads it (in `clearGameData`,
/// before the engine's reset).
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreScreenGame {
    /// `TheGameLogic`'s mode (`isInReplayGame`, `isInInternetGame`, ...).
    pub game_mode: i32,
    /// `TheRecorder->getMode()` (`None` without a recorder: `RECORDERMODETYPE_NONE`'s answers).
    pub recorder_mode: Option<RecorderMode>,
    /// `TheRecorder->isMultiplayer()`.
    pub recorder_multiplayer: bool,
    /// `TheRecorder->getLastReplayFileName()`.
    pub last_replay_file_name: Vec<u8>,
    /// `getNthPlayer(0 .. MAX_PLAYER_COUNT)`.
    pub players: Vec<ScorePlayer>,
    /// `getPlayerCount()` (`findPlayerWithNameKey` searches those).
    pub player_count: i32,
    /// `getLocalPlayer()`.
    pub local: Option<usize>,
    /// `relations[p][q]`: `getNthPlayer(p)->getRelationship(getNthPlayer(q)->getDefaultTeam())`.
    pub relations: Vec<Vec<Relationship>>,
    /// `ThePlayerTemplateStore`, in its order.
    pub templates: Vec<ScoreTemplate>,
    /// `TheScriptEngine->getGlobalDifficulty()`.
    pub global_difficulty: i32,
    /// `TheGameInfo` (`None`: `NULL`).
    pub game_info: Option<GameInfo>,
    /// `TheVictoryConditions->isLocalAlliedVictory()`, `isLocalAlliedDefeat()`.
    pub local_allied_victory: bool,
    pub local_allied_defeat: bool,
    /// `TheCampaignManager` (the screen's copy).
    pub campaign: CampaignManager,
    /// `TheChallengeGenerals`.
    pub challenge_generals: ChallengeGenerals,
    /// The local player's `calculateAcademyAdvice(&info)`: the tips' labels (`None`: `FALSE`, no tip).
    pub local_academy_advice: Option<Vec<Vec<u8>>>,
    /// (#157) What `updateSkirmishBattleHonors` reads of the local player and the logic.
    pub honors_inputs: SkirmishHonorsInputs,
}

/// (#157) The superweapons `updateSkirmishBattleHonors` counts (`TheThingFactory->findTemplate(name)`), in its order:
/// the SCUD storms, the particle cannons, the nuclear missiles.
pub const APOCALYPSE_TEMPLATES: [&str; 15] = [
    "GLAScudStorm", "Boss_GLAScudStorm", "Chem_GLAScudStorm", "Slth_GLAScudStorm", "Demo_GLAScudStorm",
    "AmericaParticleCannonUplink", "AirF_AmericaParticleCannonUplink", "Lazr_AmericaParticleCannonUplink", "SupW_AmericaParticleCannonUplink", "Boss_ParticleCannonUplink",
    "ChinaNuclearMissileLauncher", "Boss_NuclearMissileLauncher", "Infa_ChinaNuclearMissileLauncher", "Nuke_ChinaNuclearMissileLauncher", "Tank_ChinaNuclearMissileLauncher",
];

/// (#157) What `updateSkirmishBattleHonors` reads besides the stats and `TheGameInfo`, copied with the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkirmishHonorsInputs {
    /// The local player's `ScoreKeeper::getTotalObjectsBuilt(findTemplate(name))` for each of
    /// [`APOCALYPSE_TEMPLATES`] (`None`: no such template, upstream's `isEquivalentTo(NULL)`).
    pub objects_built: Vec<Option<i32>>,
    /// `getTotalUnitsBuilt(VEHICLE, AIRCRAFT)` and `getTotalUnitsBuilt(AIRCRAFT, none)`.
    pub vehicles_built: i32,
    pub aircraft_built: i32,
    /// `TheGameLogic->getFrame()`.
    pub frame: u32,
    /// `ThePlayerList->getLocalPlayer()->getSide()`.
    pub local_side: Vec<u8>,
}

impl Default for SkirmishHonorsInputs {
    /// Nothing built (every template found), frame 0, no side.
    fn default() -> Self {
        SkirmishHonorsInputs { objects_built: vec![Some(0); APOCALYPSE_TEMPLATES.len()], vehicles_built: 0, aircraft_built: 0, frame: 0, local_side: Vec::new() }
    }
}

impl SkirmishHonorsInputs {
    /// The copy from `w` (read only).
    pub fn from_world(w: &crate::logic::world::game_logic::World) -> SkirmishHonorsInputs {
        use crate::common::kind_of::{KindOfMaskType, KINDOF_AIRCRAFT, KINDOF_VEHICLE};
        let Some(lp) = w.players.local_index().and_then(|l| w.players.get_nth(l)) else { return Self::default() };
        let tf = &w.statics.thing_factory;
        let s = &lp.score_keeper;
        let objects_built = APOCALYPSE_TEMPLATES
            .iter()
            .map(|n| {
                // (no template: `isEquivalentTo(NULL)` is only reached when something was built)
                let Some(target) = tf.find_template(n.as_bytes()) else {
                    return (s.get_total_objects_built(|_| true) == 0).then_some(0);
                };
                Some(s.get_total_objects_built(|name| tf.find_template(name).is_some_and(|t| crate::common::thing_template::is_equivalent_to(t, target))))
            })
            .collect();
        let kind_of = |name: &[u8]| tf.find_template(name).map(|t| *t.get_final_override().data.kindof());
        let (mut valid, mut invalid) = (KindOfMaskType::default(), KindOfMaskType::default());
        valid.set(KINDOF_VEHICLE, 1);
        invalid.set(KINDOF_AIRCRAFT, 1);
        let vehicles_built = s.get_total_units_built(&valid, &invalid, kind_of);
        let (mut valid, invalid) = (KindOfMaskType::default(), KindOfMaskType::default());
        valid.set(KINDOF_AIRCRAFT, 1);
        let aircraft_built = s.get_total_units_built(&valid, &invalid, kind_of);
        SkirmishHonorsInputs { objects_built, vehicles_built, aircraft_built, frame: w.frame(), local_side: lp.side.clone() }
    }
}

impl ScoreScreenGame {
    /// The copy of what the score screen reads, taken from `w` as `clearGameData` leaves it before `TheGameEngine->reset()`.
    /// The academy's advice is computed on copies (of the stats and the client random stream): the World is not
    /// changed.
    pub fn from_world(w: &crate::logic::world::game_logic::World) -> ScoreScreenGame {
        let store = &w.statics.player_template_store;
        let templates: Vec<ScoreTemplate> = (0..store.player_template_count())
            .map(|i| {
                let t = store.nth_player_template(i).expect("a template of the store's count");
                ScoreTemplate {
                    name: t.name(&w.names).to_vec(),
                    side: t.side().to_vec(),
                    side_icon_image: t.side_icon_image_name().to_vec(),
                    score_screen_image: t.score_screen().to_vec(),
                    score_screen_music: t.score_screen_music().to_vec(),
                }
            })
            .collect();
        let players: Vec<ScorePlayer> = w
            .players
            .iter()
            .map(|p| ScorePlayer {
                name: p.player_name.clone(),
                display_name: p.display_name.str().to_vec(),
                color: p.color as i32,
                score_keeper: p.score_keeper.clone(),
                observer: p.observer,
                dead: p.is_player_dead,
                human: p.player_type == PlayerType::Human,
                base_side: p.base_side.clone(),
                template: p.template.map(|t| t as usize).filter(|&t| t < templates.len()),
                list_in_score_screen: p.list_in_score_screen,
                skill_points: p.skill_points,
            })
            .collect();
        let n = players.len();
        let tq = w.tq();
        let relations = (0..n).map(|p| (0..n).map(|q| tq.player_relationship_to_player_default_team(p as i32, q as i32)).collect()).collect();
        let local = w.players.local_index().map(|i| i as usize);
        let mode = w.cuts.game_loop.game_mode;
        let challenge = w.campaign.get_current_campaign().is_some_and(|c| c.is_challenge_campaign());
        let source = if mode == game_mode::SKIRMISH {
            Some(crate::logic::world::game_logic_new_game::GameSource::Skirmish)
        } else if mode == game_mode::SINGLE_PLAYER && challenge {
            Some(crate::logic::world::game_logic_new_game::GameSource::Challenge)
        } else {
            None
        };
        let game_info = source.and_then(|s| w.cuts.new_game.games.iter().find(|(g, _)| *g == s).map(|(_, g)| g.clone()));
        let local_academy_advice = local.and_then(|l| {
            let lp = w.players.get_nth(l as i32)?;
            let mut stats = lp.academy_stats.clone();
            let mut rng = w.random.clone();
            let inputs = crate::logic::world::academy_stats::AdviceInputs {
                local_mines: lp.academy_stats.mines(),
                neutral_vehicles_sniped: w.players.neutral().academy_stats.vehicles_sniped(),
                use_alternate_mouse: false,
            };
            let advice = stats.calculate_academy_advice(w.frame(), &inputs, &mut rng);
            (advice.num_tips() != 0).then(|| advice.tips.iter().map(|t| t.as_bytes().to_vec()).collect())
        });
        ScoreScreenGame {
            game_mode: mode,
            recorder_mode: w.cuts.game_loop.recorder.as_ref().map(|r| r.mode),
            recorder_multiplayer: w.recorder_is_multiplayer().unwrap_or(false),
            last_replay_file_name: crate::common::recorder::LAST_REPLAY_FILE_NAME.to_vec(),
            players,
            player_count: w.players.player_count(),
            local,
            relations,
            templates,
            global_difficulty: w.script_engine.game_difficulty,
            game_info,
            local_allied_victory: w.victory.is_local_allied_victory(w).unwrap_or(false),
            local_allied_defeat: w.victory.is_local_allied_defeat(w).unwrap_or(false),
            campaign: w.campaign.clone(),
            challenge_generals: w.challenge_generals.clone(),
            local_academy_advice,
            honors_inputs: SkirmishHonorsInputs::from_world(w),
        }
    }
}

/// ~~A `SkirmishBattleHonors` call the file makes (gap #92: recorded, the stats file is neither read nor written).~~
/// **Corrected 2026-11-12, #157:** `HonorsCall` is removed: the calls are made on the translated
/// [`SkirmishBattleHonors`], read from and written to the host's user data folder ([`GridUserData`]).
///
/// `isSlotLocalAlly(game, slot)` (ScoreScreen.cpp's static): no local slot, the local slot itself, or a slot on the
/// local slot's team (a slot without a team is not an ally).
pub fn is_slot_local_ally(game: &GameInfo, slot: usize) -> bool {
    let local = game.local_slot_num();
    let Some(local_slot) = game.slot(local) else { return true };
    if slot as i32 == local {
        return true;
    }
    let s = &game.slots[slot];
    // (plant 9525, differential builds only: a slot without a team counted as an ally)
    if s.team_number < 0 {
        return plant(9525);
    }
    s.team_number == local_slot.team_number
}

/// `updateChallengeMedals(medals)`: in a skirmish (`is_skirmish`: `TheGameInfo->isSkirmish()`) with no computer ally,
/// the medal of the number of hard computer opponents (1 to 7).
pub fn update_challenge_medals(medals: &mut i32, game: &GameInfo, is_skirmish: bool) {
    if !is_skirmish {
        return;
    }
    let (mut num_ais, mut num_brutals) = (0, 0);
    for i in 0..MAX_SLOTS as usize {
        let slot = &game.slots[i];
        if slot.is_ai() && !is_slot_local_ally(game, i) {
            num_ais += 1;
            if slot.state == slot_state::BRUTAL_AI {
                num_brutals += 1;
            }
        } else if slot.is_ai() {
            // "can't get challenge medals with AI helpers"
            return;
        }
    }
    if num_ais != 0 && (1..=7).contains(&num_brutals) {
        // (plant 9526, differential builds only: the medal of one brutal opponent more)
        let k = if plant(9526) { (num_brutals as usize).min(6) } else { num_brutals as usize - 1 };
        *medals |= BH_CHALLENGE_MASK[k];
    }
}

/// `updateSkirmishBattleHonors(stats)`: the honours a won skirmish earns (see the module docs). `Err` where upstream
/// is undefined (a superweapon template the data lacks, reached with objects built).
pub fn update_skirmish_battle_honors(stats: &mut SkirmishBattleHonors, inp: &SkirmishHonorsInputs, game: &GameInfo) -> Result<(), &'static str> {
    // (plant 9537, differential builds only: the streak honour from 4 wins)
    if stats.get_win_streak() >= if plant(9537) { 4 } else { 5 } {
        stats.set_honors(BATTLE_HONOR_STREAK);
    }
    // "For Apocalypse Honor, see if the player has used each category of super weapon."
    for (i, _) in APOCALYPSE_TEMPLATES.iter().enumerate() {
        let n = inp.objects_built.get(i).copied().flatten().ok_or("updateSkirmishBattleHonors: findTemplate gave NULL (isEquivalentTo(NULL))")?;
        if n > 0 {
            match i / 5 {
                0 => stats.set_built_scud(),
                1 => stats.set_built_particle_cannon(),
                _ => stats.set_built_nuke(),
            }
        }
    }
    if stats.built_nuke() && stats.built_particle_cannon() && stats.built_scud() {
        stats.set_honors(BATTLE_HONOR_APOCALYPSE);
    }
    if inp.vehicles_built >= 50 {
        stats.set_honors(BATTLE_HONOR_BATTLE_TANK);
    }
    if inp.aircraft_built >= 20 {
        stats.set_honors(BATTLE_HONOR_AIR_WING);
    }
    // `TheGameLogic->getFrame() / LOGICFRAMES_PER_SECOND / 60` (unsigned)
    let minutes = inp.frame / LOGICFRAMES_PER_SECOND / 60;
    // (plant 9523, differential builds only: `<=` for `<`)
    if if plant(9523) { minutes <= 5 } else { minutes < 5 } {
        stats.set_honors(BATTLE_HONOR_BLITZ5);
    }
    if minutes < 10 {
        stats.set_honors(BATTLE_HONOR_BLITZ10);
    }
    for (side, bit) in [(&b"America"[..], BATTLE_HONOR_LOYALTY_USA), (b"China", BATTLE_HONOR_LOYALTY_CHINA), (b"GLA", BATTLE_HONOR_LOYALTY_GLA)] {
        if stats.get_num_games_loyal() >= 20 && inp.local_side == side {
            stats.set_honors(bit);
        }
    }
    // the endurance medals
    let (mut num_easy, mut num_medium, mut num_brutal) = (0, 0, 0);
    for i in 0..MAX_SLOTS as usize {
        let slot = &game.slots[i];
        if slot.is_ai() && !is_slot_local_ally(game, i) {
            match slot.state {
                slot_state::EASY_AI => num_easy += 1,
                slot_state::MED_AI => num_medium += 1,
                slot_state::BRUTAL_AI => num_brutal += 1,
                _ => {}
            }
        }
        // (else an allied computer: `anyAlliedAI`, not read)
    }
    if num_easy != 0 || num_medium != 0 || num_brutal != 0 {
        let map = game.map_name.clone();
        let old_easy = stats.get_endurance_medal(&map, slot_state::EASY_AI);
        let old_medium = stats.get_endurance_medal(&map, slot_state::MED_AI);
        let old_brutal = stats.get_endurance_medal(&map, slot_state::BRUTAL_AI);
        if num_easy != 0 {
            // (plant 9524, differential builds only: the hard opponents left out of the easy medal)
            let n = if plant(9524) { num_easy + num_medium } else { num_easy + num_medium + num_brutal };
            stats.set_endurance_medal(&map, slot_state::EASY_AI, old_easy.max(n));
        }
        if num_medium != 0 {
            stats.set_endurance_medal(&map, slot_state::MED_AI, old_medium.max(num_medium + num_brutal));
        }
        if num_brutal != 0 {
            stats.set_endurance_medal(&map, slot_state::BRUTAL_AI, old_brutal.max(num_brutal));
        }
    }
    Ok(())
}

/// `populatePlayerInfo`'s skirmish tail's early return: a sandbox game, or one neither won nor lost by the local
/// allies, with the player still active (`isPlayerActive`) records nothing ("Skipping skirmish stats update").
pub fn skirmish_stats_skipped(sandbox: bool, victory: bool, defeat: bool, active: bool) -> bool {
    (sandbox || !(defeat || victory)) && active
}

/// `populatePlayerInfo`'s skirmish tail past its early return: the stats (`SkirmishBattleHonors stats;`, from `fs`)
/// counted, `updateSkirmishBattleHonors` and `updateChallengeMedals` after a win, the last general and the loyalty,
/// then `write()`. `side` is `player->getPlayerTemplate()->getSide()`. `Err`: an undefined step (nothing written).
pub fn skirmish_stats_update<F: UserDataFiles + ?Sized>(fs: &mut F, victory: bool, side: &[u8], inp: &SkirmishHonorsInputs, game: &GameInfo, is_skirmish: bool) -> Result<SkirmishBattleHonors, &'static str> {
    let mut stats = SkirmishBattleHonors::new(fs);
    if victory {
        let w = stats.get_wins();
        stats.set_wins(w.wrapping_add(1));
        // (plant 9528, differential builds only: the best streak taken before the streak is counted)
        let best_first = plant(9528).then(|| stats.get_best_win_streak().max(stats.get_win_streak()));
        let st = stats.get_win_streak();
        stats.set_win_streak(st.wrapping_add(1));
        let best = best_first.unwrap_or_else(|| stats.get_best_win_streak().max(stats.get_win_streak()));
        stats.set_best_win_streak(best);
        update_skirmish_battle_honors(&mut stats, inp, game)?;
        let mut challenge_medals = stats.get_challenge_medals();
        update_challenge_medals(&mut challenge_medals, game, is_skirmish);
        stats.set_challenge_medals(challenge_medals);
    } else {
        let l = stats.get_losses();
        stats.set_losses(l.wrapping_add(1));
        if !plant(9538) {
            // (plant 9538, differential builds only: a loss keeps the streak)
            stats.set_win_streak(0);
        }
    }
    let last_general = stats.get_last_general();
    stats.set_last_general(side);
    if last_general != stats.get_last_general() && !plant(9527) {
        // (plant 9527, differential builds only: a change of general keeps the count)
        stats.set_num_games_loyal(0);
    } else {
        let n = stats.get_num_games_loyal();
        stats.set_num_games_loyal(n.wrapping_add(1));
    }
    stats.write(fs);
    Ok(stats)
}

/// The score grid's host as the user data folder ([`ScoreGridHost::grid_user_data_read`] and kin).
pub struct GridUserData<'a, H: ?Sized>(pub &'a mut H);

impl<H: ScoreGridHost + ?Sized> UserDataFiles for GridUserData<'_, H> {
    fn path_user_data(&self) -> Vec<u8> {
        self.0.grid_path_user_data()
    }
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.0.grid_user_data_read(path)
    }
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        self.0.grid_user_data_write(path, bytes)
    }
}

/// ScoreScreen.cpp's globals that other files read: `DontShowMainMenu` (MainMenu.cpp, PopupSaveLoad.cpp),
/// `ReplayWasPressed` (PopupSaveLoad.cpp), `g_playMusic`, `LastReplayFileName` (PopupReplay.cpp).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScoreScreenGlobals {
    pub dont_show_main_menu: bool,
    pub replay_was_pressed: bool,
    pub play_music: bool,
    pub last_replay_file_name: Vec<u8>,
}

/// ScoreScreen.cpp's statics, its globals and the copy of the game it scores.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScoreScreen {
    pub parent_id: i32,
    pub button_ok_id: i32,
    pub text_entry_chat_id: i32,
    pub button_emote_id: i32,
    pub chat_box_border_id: i32,
    pub button_continue_id: i32,
    pub button_buddies_id: i32,
    pub button_save_replay_id: i32,
    pub backdrop_id: i32,
    pub listbox_chat_id: i32,
    pub listbox_academy_id: i32,
    pub static_text_academy_title_id: i32,
    pub parent: Option<WinId>,
    pub button_ok: Option<WinId>,
    pub button_continue: Option<WinId>,
    pub text_entry_chat: Option<WinId>,
    pub button_emote: Option<WinId>,
    pub chat_box_border: Option<WinId>,
    pub button_buddies: Option<WinId>,
    pub static_text_game_saved: Option<WinId>,
    pub backdrop: Option<WinId>,
    pub challenge_portrait: Option<WinId>,
    pub challenge_remarks: Option<WinId>,
    pub challenge_win_loss_text: Option<WinId>,
    pub gadget_parent: Option<WinId>,
    pub listbox_chat: Option<WinId>,
    pub listbox_academy: Option<WinId>,
    pub static_text_academy_title: Option<WinId>,
    pub override_player_display_name: bool,
    pub can_save_replay: bool,
    pub need_to_finish_single_player_init: bool,
    pub button_is_finish_campaign: bool,
    /// `s_blankLayout`.
    pub blank_layout: Option<usize>,
    /// `screenType`.
    pub screen_type: i32,
    pub globals: ScoreScreenGlobals,
    /// The game the screen scores (set by the owner before the push; `None`: no game).
    pub game: Option<ScoreScreenGame>,
}

/// What the score grid needs (`populatePlayerInfo` and the functions it is called with): the statics, the name keys,
/// the game's text and the mapped images. A host of its own for the differential test (tests/differential_score_grid.rs).
pub trait ScoreGridHost: GuiHost {
    /// ScoreScreen.cpp's statics.
    fn score_screen(&mut self) -> &mut ScoreScreen;
    /// `TheNameKeyGenerator->nameToKey(name)`.
    fn grid_name_to_key(&mut self, name: &[u8]) -> u32;
    /// `TheGameText->fetch(label)`.
    fn grid_fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheMappedImageCollection->findImageByName(name)` (`None`: `NULL`).
    fn grid_find_image(&self, name: &[u8]) -> Option<Vec<u8>>;
    /// A call of the file that is not translated (multiplayer, the movie), recorded.
    fn score_untranslated(&mut self, what: &[u8]);
    /// (#157) The user data folder `SkirmishBattleHonors` reads and writes (`getPath_UserData()`, `fopen`): none by
    /// default.
    fn grid_path_user_data(&self) -> Vec<u8> {
        Vec::new()
    }
    fn grid_user_data_read(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn grid_user_data_write(&mut self, _path: &[u8], _bytes: &[u8]) -> bool {
        false
    }
}

/// What the score screen needs besides the main menu's host.
pub trait ScoreScreenHost: MenuHost + MapUtilHost + ScoreGridHost {
    /// `CopyProtect::validate()` (the launcher's shared memory): true, as the launcher sets it.
    fn copy_protect_validate(&self) -> bool {
        true
    }
    /// `TheInGameUI->freeMessageResources()`.
    fn free_message_resources(&mut self) {}
}

/// UTF-16 of ASCII text.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `UnicodeString::trim()`: leading and trailing white space removed (`iswspace`).
fn trim(t: &[u16]) -> Vec<u16> {
    let sp = |c: &u16| matches!(*c, 0x20 | 0x09..=0x0D);
    let a = t.iter().position(|c| !sp(c)).unwrap_or(t.len());
    let b = t.iter().rposition(|c| !sp(c)).map_or(a, |i| i + 1);
    t[a..b.max(a)].to_vec()
}

/// The window `ScoreScreen.wnd:<name>` under `parent`.
fn find<H: ScoreGridHost>(wm: &GameWindowManager, h: &mut H, parent: Option<WinId>, name: &str) -> Option<WinId> {
    let id = h.grid_name_to_key(format!("ScoreScreen.wnd:{name}").as_bytes()) as i32;
    wm.win_get_window_from_id(parent, id)
}

/// A window upstream uses without a check: `None` records the `NULL` call as undefined.
fn must(wm: &mut GameWindowManager, w: Option<WinId>, what: &'static str) -> Option<WinId> {
    if w.is_none() {
        wm.set_undefined(what);
    }
    w
}

/// `if (w) w->winHide(hide)`.
fn hide_if(wm: &mut GameWindowManager, w: Option<WinId>, hide: bool) {
    if let Some(w) = w {
        wm.win_hide(w, hide);
    }
}

/// `w->winHide(hide)` on a window used without a check.
fn hide_must(wm: &mut GameWindowManager, w: Option<WinId>, hide: bool, what: &'static str) {
    if let Some(w) = must(wm, w, what) {
        wm.win_hide(w, hide);
    }
}

/// `TheMappedImageCollection->findImageByName(name)` (`None`: `NULL`).
fn image<H: ScoreGridHost>(h: &H, name: &[u8]) -> Option<Vec<u8>> {
    h.grid_find_image(name)
}

/// `win->winSetEnabledImage(0, image)`.
fn set_enabled_image(wm: &mut GameWindowManager, w: WinId, image: Option<Vec<u8>>) {
    wm.tree.windows[w].inst.enabled_draw_data[0].image = image;
}

/// `win->winSetEnabledTextColors(color, win->winGetEnabledTextBorderColor())`.
fn set_text_color(wm: &mut GameWindowManager, w: WinId, color: i32) {
    let border = wm.tree.windows[w].inst.enabled_text.border_color;
    wm.win_set_text_colors(w, 0, color, border);
}

/// `parent->winSetEnabledImage(0, image)` and the image status set, when the image exists.
fn set_parent_image<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, name: &[u8]) {
    if let Some(img) = image(h, name) {
        let parent = h.score_screen().parent;
        let Some(p) = must(wm, parent, "ScoreScreen: no ParentScoreScreen (parent->winSetEnabledImage)") else { return };
        set_enabled_image(wm, p, Some(img));
        wm.win_set_status(p, win_status::IMAGE);
    }
}

/// `startNextCampaignGame()`: the screen popped at once, the shell hidden; the rest (the pending file, a challenge's
/// `TheChallengeGameInfo`, `MSG_NEW_GAME`, `InitRandom(0)`) is the World's `start_next_campaign_game`, which the client
/// runs with the screen's campaign ([`ShellRequest::StartNextCampaignGame`]).
pub fn start_next_campaign_game<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    shell::pop_immediate(wm, h);
    shell::hide_shell(wm, h);
    let campaign = h.score_screen().game.as_ref().map(|g| g.campaign.clone());
    let Some(campaign) = campaign else {
        wm.set_undefined("startNextCampaignGame: no game (TheCampaignManager)");
        return;
    };
    h.request(ShellRequest::StartNextCampaignGame { campaign: Box::new(campaign) });
}

/// `CheckForCDAtGameStart(startNextCampaignGame)`: the start at once with the disc; else ~~the prompt
/// (`ExMessageBoxOkCancel`, not translated: recorded)~~ **corrected 2026-11-12, #157:** the insert-disc prompt
/// (SkirmishGameOptionsMenu.cpp's, `crate::client::skirmish_game_options::check_for_cd_at_game_start_with`).
fn check_for_cd_at_game_start<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    crate::client::skirmish_game_options::check_for_cd_at_game_start_with(wm, h, "startNextCampaignGame", start_next_campaign_game);
}

/// `ScoreScreenEnableControls(enable)`.
pub fn score_screen_enable_controls<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, enable: bool) {
    let s = h.score_screen();
    let (ok, cont, buddies, parent, replay_id, can_save) = (s.button_ok, s.button_continue, s.button_buddies, s.parent, s.button_save_replay_id, s.can_save_replay);
    for b in [ok, cont, buddies].into_iter().flatten() {
        if !wm.is_hidden(Some(b)) {
            wm.win_enable(b, enable);
        }
    }
    if let Some(b) = wm.win_get_window_from_id(parent, replay_id) {
        if !wm.is_hidden(Some(b)) {
            wm.win_enable(b, enable && can_save);
        }
    }
}

/// `GameLogic::clearGameData`'s score screen (`showScoreScreen` and not the shell's game): `FadeWholeScreen` set,
/// `Menus/ScoreScreen.wnd` pushed, `showShell(FALSE)`, the fade reversed, `FixupScoreScreenMovieWindow()`. The game the
/// screen reads must be in [`ScoreScreen::game`] first.
pub fn clear_game_data_score_screen<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    wt::set_group(wm, h, b"FadeWholeScreen", false);
    shell::push(wm, h, b"Menus/ScoreScreen.wnd", false);
    shell::show_shell(wm, h, false);
    wt::reverse(wm, h, b"FadeWholeScreen");
    fixup_score_screen_movie_window(wm, h);
}

/// `ScoreScreenInit(layout, userData)`.
pub fn score_screen_init<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let s = h.score_screen();
    s.globals.play_music = true;
    s.globals.dont_show_main_menu = true;
    s.button_is_finish_campaign = false;
    // the keys
    let mut k = |n: &str| h.name_to_key(format!("ScoreScreen.wnd:{n}").as_bytes()) as i32;
    let keys = [
        k("ParentScoreScreen"),
        k("ButtonOk"),
        k("TextEntryChat"),
        k("ButtonEmote"),
        k("ListboxChatWindowScoreScreen"),
        k("ListboxWarschoolAdvice"),
        k("StaticTextWarSchool"),
        k("ChatBoxBorder"),
        k("ButtonBuddy"),
        k("ButtonContinue"),
        k("ButtonSaveReplay"),
        k("MainBackdrop"),
    ];
    let s = h.score_screen();
    [
        s.parent_id,
        s.button_ok_id,
        s.text_entry_chat_id,
        s.button_emote_id,
        s.listbox_chat_id,
        s.listbox_academy_id,
        s.static_text_academy_title_id,
        s.chat_box_border_id,
        s.button_buddies_id,
        s.button_continue_id,
        s.button_save_replay_id,
        s.backdrop_id,
    ] = keys;
    let parent = wm.win_get_window_from_id(None, keys[0]);
    let w = |id: i32| wm.win_get_window_from_id(parent, id);
    let (ok, chat, emote, lchat, lacademy, academy_title, border, buddies, cont, backdrop) = (w(keys[1]), w(keys[2]), w(keys[3]), w(keys[4]), w(keys[5]), w(keys[6]), w(keys[7]), w(keys[8]), w(keys[9]), w(keys[11]));
    let saved = find(wm, h, parent, "StaticTextGameSaveComplete");
    let portrait = find(wm, h, parent, "BigPortrait");
    let win_loss = find(wm, h, parent, "ChallengeWinLossText");
    let remarks = find(wm, h, parent, "GeneralRemarks");
    let gadget_parent = find(wm, h, parent, "GadgetParent");
    let last_replay = h.score_screen().game.as_ref().map(|g| g.last_replay_file_name.clone()).unwrap_or_default();
    let s = h.score_screen();
    s.parent = parent;
    s.button_ok = ok;
    s.text_entry_chat = chat;
    s.button_emote = emote;
    s.listbox_chat = lchat;
    s.listbox_academy = lacademy;
    s.static_text_academy_title = academy_title;
    s.chat_box_border = border;
    s.button_continue = cont;
    s.button_buddies = buddies;
    s.static_text_game_saved = saved;
    s.backdrop = backdrop;
    s.challenge_portrait = portrait;
    s.challenge_win_loss_text = win_loss;
    s.challenge_remarks = remarks;
    s.gadget_parent = gadget_parent;
    // `LastReplayFileName = TheRecorder->getLastReplayFileName().str()`
    s.globals.last_replay_file_name = last_replay;
    hide_must(wm, saved, true, "ScoreScreenInit: no StaticTextGameSaveComplete (staticTextGameSaved->winHide)");
    if wm.undefined.is_some() {
        return;
    }
    h.score_screen().override_player_display_name = false;
    if let Some(r) = shell::get_popup_replay_layout(wm, h) {
        wm.layout_hide(r, true);
    }
    let Some(game) = h.score_screen().game.as_ref().map(|g| (g.recorder_mode, g.game_mode, g.recorder_multiplayer)) else {
        wm.set_undefined("ScoreScreenInit: no game to score (ThePlayerList, TheRecorder)");
        return;
    };
    let (recorder_mode, mode, recorder_multiplayer) = game;
    // (`getMode()` with no recorder: none)
    let recorder_mode = recorder_mode.unwrap_or(RecorderMode::None);
    h.score_screen().can_save_replay = recorder_mode == RecorderMode::Record;
    let replay_id = h.score_screen().button_save_replay_id;
    let save_replay = wm.win_get_window_from_id(parent, replay_id);
    if recorder_mode == RecorderMode::None {
        if let Some(b) = save_replay {
            wm.win_enable(b, false);
        }
    }
    h.score_screen().need_to_finish_single_player_init = false;
    if mode == game_mode::REPLAY {
        hide_if(wm, save_replay, true);
        if recorder_multiplayer {
            init_replay_multi_player(wm, h);
        } else {
            h.score_screen().override_player_display_name = true;
            init_replay_single_player(wm, h);
        }
        wt::set_group(wm, h, b"ScoreScreenShow", false);
    } else if mode == game_mode::INTERNET {
        init_internet_multi_player(wm, h);
        wt::set_group(wm, h, b"ScoreScreenShow", false);
    } else if mode == game_mode::LAN {
        init_lan_multi_player(wm, h);
        wt::set_group(wm, h, b"ScoreScreenShow", false);
    } else if mode == game_mode::SKIRMISH {
        init_skirmish(wm, h);
        wt::set_group(wm, h, b"ScoreScreenShow", false);
    } else {
        h.score_screen().override_player_display_name = true;
        init_single_player(wm, h);
        hide_must(wm, save_replay, true, "ScoreScreenInit: no ButtonSaveReplay (buttonSaveReplay->winHide)");
    }
    if wm.undefined.is_some() {
        return;
    }
    // challenge maps have no replay
    let challenge = h.score_screen().game.as_ref().is_some_and(|g| g.campaign.get_current_campaign().is_some_and(|c| c.is_challenge_campaign()));
    if challenge {
        let Some(b) = must(wm, save_replay, "ScoreScreenInit: no ButtonSaveReplay in a challenge (buttonSaveReplay->winEnable)") else { return };
        wm.win_enable(b, false);
        wm.win_hide(b, true);
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
    h.score_screen().globals.replay_was_pressed = false;
    fixup_score_screen_movie_window(wm, h);
}

/// `FixupScoreScreenMovieWindow()`.
pub fn fixup_score_screen_movie_window<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(b) = h.score_screen().blank_layout {
        wm.layout_hide(b, false);
        wm.layout_bring_forward(b);
    }
}

/// `ScoreScreenShutdown(layout, userData)`.
pub fn score_screen_shutdown<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.score_screen().globals.dont_show_main_menu = false;
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
    // (`TheAudio->removeAudioEvent(AHSV_StopTheMusicFade)`: the host's audio)
}

/// `ScoreScreenUpdate(layout, userData)`.
pub fn score_screen_update<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    if let Some(r) = shell::get_popup_replay_layout(wm, h) {
        if !wm.layouts[r].hidden {
            // `PopupReplayUpdate(popupReplayLayout, NULL)` (~~PopupReplay.cpp, not translated~~ **Corrected 2026-11-10
            // (#98):** translated, `crate::client::popup_replay`; called through the host's lexicon)
            h.layout_callback(wm, b"PopupReplayUpdate", r, None);
        }
    }
    if h.score_screen().need_to_finish_single_player_init {
        finish_single_player_init(wm, h);
        h.score_screen().need_to_finish_single_player_init = false;
    }
    let s = h.score_screen();
    let has_game_info = s.game.as_ref().is_some_and(|g| g.game_info.is_some());
    if has_game_info && s.globals.play_music {
        s.globals.play_music = false;
        let g = s.game.as_ref().expect("checked");
        let gi = g.game_info.as_ref().expect("checked");
        let local = gi.local_slot_num();
        let slot_template = (0..MAX_SLOTS).contains(&local).then(|| gi.slots[local as usize].player_template);
        let pt = match slot_template {
            Some(t) if t >= 0 => g.templates.get(t as usize),
            _ => g.templates.iter().find(|t| t.name == b"FactionObserver"),
        };
        let Some(pt) = pt else {
            wm.set_undefined("ScoreScreenUpdate: no player template for the music (pt->getScoreScreenMusic on NULL)");
            return;
        };
        let music = pt.score_screen_music.clone();
        if !music.is_empty() {
            // (`removeAudioEvent(AHSV_StopTheMusicFade)`; the event `setShouldFade(TRUE)`; `TheAudio->update()`)
            h.add_audio_event(&music);
        }
    }
}

/// `ScoreScreenInput(window, msg, mData1, mData2)`.
pub fn score_screen_input<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let s = h.score_screen();
        let (ok, id) = (s.button_ok, s.button_ok_id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(ok), id as u32);
    }
    MsgHandled::Handled
}

/// `ScoreScreenSystem(window, msg, mData1, mData2)`: every message is `MSG_HANDLED`.
pub fn score_screen_system<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            score_screen_selected(wm, h, d1);
            if wm.undefined.is_none() {
                // (no `break`: on into `GEM_EDIT_DONE`)
                score_screen_edit_done(wm, h, d1);
            }
        }
        te::gem::EDIT_DONE => score_screen_edit_done(wm, h, d1),
        _ => {}
    }
    MsgHandled::Handled
}

/// The chat entry's text read, cleared and trimmed (`txtInput`), as *Emote* and `GEM_EDIT_DONE` read it.
fn take_chat_text<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) -> Vec<u16> {
    let chat = h.score_screen().text_entry_chat;
    let t = te::get_text(wm, h, chat);
    te::set_text(wm, h, chat, &[]);
    trim(&t)
}

/// `ScoreScreenSystem`'s `GBM_SELECTED`.
fn score_screen_selected<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, d1: WindowMsgData) {
    wt::remove(wm, h, b"ScoreScreenShow", true);
    h.score_screen().globals.replay_was_pressed = false;
    let Some(control) = data_win(d1) else {
        wm.set_undefined("ScoreScreenSystem: GBM_SELECTED without a control (control->winGetWindowId on NULL)");
        return;
    };
    let id = wm.tree.windows[control].id;
    let s = h.score_screen().clone();
    if id == s.button_ok_id {
        shell::pop(wm, h);
        if let Some(g) = h.score_screen().game.as_mut() {
            g.campaign.set_campaign(b"");
        }
    } else if id == s.button_continue_id {
        if !s.button_is_finish_campaign {
            h.score_screen().globals.replay_was_pressed = true;
        }
        if s.screen_type == SCORESCREEN_SINGLEPLAYER {
            let map = s.game.as_ref().map(|g| g.campaign.get_current_map()).unwrap_or_default();
            if map.is_empty() {
                h.score_screen().globals.replay_was_pressed = false;
                shell::pop(wm, h);
            } else {
                check_for_cd_at_game_start(wm, h);
            }
        }
    } else if id == s.button_buddies_id {
        h.score_untranslated(b"GameSpyToggleOverlay(GSOVERLAY_BUDDY)");
    } else if id == s.button_save_replay_id {
        score_screen_enable_controls(wm, h, false);
        let Some(l) = shell::get_popup_replay_layout(wm, h) else {
            wm.set_undefined("ScoreScreenSystem: no Menus/PopupReplay.wnd (saveReplayLayout->runInit on NULL)");
            return;
        };
        shell::run_layout(wm, h, l, 0, None);
        wm.layout_hide(l, false);
        wm.layout_bring_forward(l);
    } else if id == s.button_emote_id {
        let t = take_chat_text(wm, h);
        // `if (TheLAN) TheLAN->RequestChat(txtInput, LANCHAT_EMOTE)`: no `TheLAN` in the client
        let _ = t;
    }
    for i in 0..MAX_SLOTS {
        let key = h.name_to_key(format!("ScoreScreen.wnd:ButtonAdd{i}").as_bytes()) as i32;
        if id == key {
            // `TheGameSpyInfo->getBuddyMap()`: no `TheGameSpyInfo` in the client
            wm.set_undefined("ScoreScreenSystem: ButtonAdd pressed (TheGameSpyInfo->getBuddyMap on NULL)");
            return;
        }
    }
}

/// `ScoreScreenSystem`'s `GEM_EDIT_DONE`.
fn score_screen_edit_done<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, d1: WindowMsgData) {
    let Some(control) = data_win(d1) else {
        wm.set_undefined("ScoreScreenSystem: GEM_EDIT_DONE without a control (control->winGetWindowId on NULL)");
        return;
    };
    if wm.tree.windows[control].id == h.score_screen().text_entry_chat_id {
        let t = take_chat_text(wm, h);
        // `if (TheLAN) TheLAN->RequestChat(txtInput, LANCHAT_NORMAL)`: no `TheLAN` in the client
        let _ = t;
    }
}

/// `initSkirmish()`.
fn init_skirmish<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.score_screen().screen_type = SCORESCREEN_SKIRMISH;
    grab_multi_player_info(wm, h);
    let s = h.score_screen();
    for w in [s.text_entry_chat, s.button_emote, s.chat_box_border, s.button_buddies, s.button_continue, s.listbox_chat, s.static_text_game_saved] {
        hide_if(wm, w, true);
    }
}

/// `PlayMovieAndBlock(movieTitle)`: `TheVideoPlayer->open(movieTitle)` gives no stream (Bink was never released), so it
/// returns.
fn play_movie_and_block<H: ScoreScreenHost>(h: &mut H, movie: &[u8]) {
    let mut what = b"PlayMovieAndBlock: no video stream for ".to_vec();
    what.extend_from_slice(movie);
    h.score_untranslated(&what);
}

/// `initSinglePlayer()`.
fn init_single_player<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    let s = h.score_screen();
    s.screen_type = SCORESCREEN_SINGLEPLAYER;
    let Some(g) = s.game.as_mut() else {
        wm.set_undefined("initSinglePlayer: no game (ThePlayerList)");
        return;
    };
    // `TheCampaignManager->setRankPoints(ThePlayerList->getLocalPlayer()->getSkillPoints())`
    let Some(skill) = g.local.and_then(|l| g.players.get(l)).map(|p| p.skill_points) else {
        wm.set_undefined("initSinglePlayer: no local player (getLocalPlayer()->getSkillPoints on NULL)");
        return;
    };
    g.campaign.set_rank_points(skill);
    // `TheCampaignManager->setGameDifficulty(TheScriptEngine->getGlobalDifficulty())`
    let d = g.global_difficulty;
    g.campaign.set_game_difficulty(d);
    grab_single_player_info(wm, h);
    if wm.undefined.is_some() {
        return;
    }
    h.score_screen().need_to_finish_single_player_init = true;
    let blank = h.create_layout(wm, b"Menus/BlankWindow.wnd");
    h.score_screen().blank_layout = blank;
    let Some(b) = must_layout(wm, blank, "initSinglePlayer: Menus/BlankWindow.wnd did not load (s_blankLayout->hide on NULL)") else { return };
    wm.layout_hide(b, false);
    wm.layout_bring_forward(b);
    let Some(&first) = wm.layouts[b].windows.first() else {
        wm.set_undefined("initSinglePlayer: the blank layout has no window (getFirstWindow()->winClearStatus on NULL)");
        return;
    };
    wm.win_clear_status(first, win_status::IMAGE);
}

/// A layout upstream uses without a check.
fn must_layout(wm: &mut GameWindowManager, l: Option<usize>, what: &'static str) -> Option<usize> {
    if l.is_none() {
        wm.set_undefined(what);
    }
    l
}

/// `displayChallengeWinLoss(imageGeneral, strHeader, strRemarks)`.
fn display_challenge_win_loss<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, general_image: Option<Vec<u8>>, header: &[u16], remarks: &[u16]) {
    let s = h.score_screen().clone();
    hide_must(wm, s.backdrop, true, "displayChallengeWinLoss: no MainBackdrop");
    hide_must(wm, s.gadget_parent, true, "displayChallengeWinLoss: no GadgetParent");
    hide_must(wm, s.challenge_win_loss_text, false, "displayChallengeWinLoss: no ChallengeWinLossText");
    hide_must(wm, s.challenge_remarks, false, "displayChallengeWinLoss: no GeneralRemarks");
    hide_must(wm, s.challenge_portrait, false, "displayChallengeWinLoss: no BigPortrait");
    let Some(p) = must(wm, s.parent, "displayChallengeWinLoss: no ParentScoreScreen") else { return };
    if wm.undefined.is_some() {
        return;
    }
    let img = image(h, b"GeneralsChallengeWinLoss");
    set_enabled_image(wm, p, img);
    set_enabled_image(wm, s.challenge_portrait.expect("checked"), general_image);
    gadget::static_text_set_text(wm, h, s.challenge_win_loss_text, header);
    gadget::static_text_set_text(wm, h, s.challenge_remarks, remarks);
}

/// The challenge's win or loss display of `finishSinglePlayerInit` (`win`: the defeated general's image, string and win
/// sound, `GUI:ChallengeWinText`; else the victorious ones and `GUI:ChallengeLossText`).
fn challenge_display<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, win: bool) {
    let g = h.score_screen().game.as_ref().expect("a game");
    let Some(name) = g.campaign.get_current_mission().map(|m| m.general_name.clone()) else {
        wm.set_undefined("finishSinglePlayerInit: no current mission (getCurrentMission()->m_generalName on NULL)");
        return;
    };
    let Some(general) = g.challenge_generals.get_general_by_general_name(&name).cloned() else {
        wm.set_undefined("finishSinglePlayerInit: no general of the mission's name (general->getImage... on NULL)");
        return;
    };
    let (img, text, header, sound) = if win {
        (&general.image_defeated, &general.defeated, &b"GUI:ChallengeWinText"[..], &general.win_sound)
    } else {
        (&general.image_victorious, &general.victorious, &b"GUI:ChallengeLossText"[..], &general.loss_sound)
    };
    let img = image(h, img);
    let remarks = h.fetch_text(text);
    let fmt = h.fetch_text(header);
    let named = h.fetch_text(&name);
    let header = format_wide(&fmt, &[FormatArg::Str(named)]);
    display_challenge_win_loss(wm, h, img, &header, &remarks);
    // `AudioEventRTS event(general->getWinSound())` (or the loss sound), `addAudioEvent`, `TheAudio->update()`
    h.add_audio_event(sound);
}

/// `finishSinglePlayerInit()`.
fn finish_single_player_init<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    let copy_protect_ok = h.copy_protect_validate();
    let Some(g) = h.score_screen().game.as_ref() else {
        wm.set_undefined("finishSinglePlayerInit: no game (TheCampaignManager)");
        return;
    };
    let victorious = g.campaign.is_victorious();
    let challenge = g.campaign.get_current_campaign().is_some_and(|c| c.is_challenge_campaign());
    let cont = h.score_screen().button_continue;
    if copy_protect_ok && victorious {
        if challenge {
            challenge_display(wm, h, true);
            if wm.undefined.is_some() {
                return;
            }
        }
        let g = h.score_screen().game.as_mut().expect("checked");
        g.campaign.goto_next_mission();
        if g.campaign.get_current_map().is_empty() {
            let t = h.fetch_text(b"GUI:EndCampaign");
            gadget::button_set_text(wm, h, cont, &t);
            h.score_screen().button_is_finish_campaign = true;
            let g = h.score_screen().game.as_ref().expect("checked");
            if let Some(campaign) = g.campaign.get_current_campaign().cloned() {
                let difficulty = g.campaign.get_game_difficulty();
                // (#157) `SkirmishBattleHonors stats;`: the stats file, completed and written
                let mut stats = SkirmishBattleHonors::new(&mut HostUserData(h));
                let is = |n: &[u8]| campaign.name.eq_ignore_ascii_case(n);
                if is(b"USA") {
                    stats.set_usa_campaign_complete(difficulty);
                    stats.set_honors(BATTLE_HONOR_CAMPAIGN_USA);
                }
                if is(b"China") {
                    stats.set_china_campaign_complete(difficulty);
                    stats.set_honors(BATTLE_HONOR_CAMPAIGN_CHINA);
                }
                // (twice, as upstream)
                for _ in 0..2 {
                    if is(b"GLA") {
                        stats.set_gla_campaign_complete(difficulty);
                        stats.set_honors(BATTLE_HONOR_CAMPAIGN_GLA);
                    }
                }
                for i in 0..MAX_GLOBAL_GENERAL_TYPES {
                    if is(format!("CHALLENGE_{i}").as_bytes()) {
                        stats.set_challenge_campaign_complete(i, difficulty);
                        stats.set_honors(BATTLE_HONOR_CHALLENGE_MODE);
                    }
                }
                stats.write(&mut HostUserData(h));
                report_undefined(wm, &mut stats.prefs);
                let s = h.score_screen().clone();
                for w in [s.button_ok, s.button_continue, s.text_entry_chat, s.button_emote, s.listbox_chat, s.listbox_academy, s.static_text_academy_title, s.chat_box_border, s.button_buddies] {
                    hide_if(wm, w, true);
                }
                if !campaign.get_final_victory_movie().is_empty() {
                    // (`TheGameLODManager`'s low-resolution test: ~~not translated~~ **Corrected 2026-11-12 (#160):** translated
                    // in `client::game_lod`, not read here yet; the movie is asked for)
                    play_movie_and_block(h, campaign.get_final_victory_movie());
                }
            }
        } else {
            let t = h.fetch_text(b"GUI:SaveAndContinue");
            gadget::button_set_text(wm, h, cont, &t);
            // `TheGameState->missionSave()`
            let campaign = h.score_screen().game.as_ref().expect("checked").campaign.clone();
            h.request(ShellRequest::MissionSave { campaign: Box::new(campaign) });
            let saved = h.score_screen().static_text_game_saved;
            hide_if(wm, saved, false);
        }
    } else {
        if challenge {
            challenge_display(wm, h, false);
            if wm.undefined.is_some() {
                return;
            }
        }
        let t = h.fetch_text(b"GUI:Retry");
        gadget::button_set_text(wm, h, cont, &t);
    }
    h.free_message_resources();
    let blank = h.score_screen().blank_layout.take();
    let Some(b) = must_layout(wm, blank, "finishSinglePlayerInit: no blank layout (s_blankLayout->destroyWindows on NULL)") else { return };
    shell::destroy_layout(wm, b);
    let s = h.score_screen().clone();
    wm.win_set_focus(h, s.parent);
    hide_if(wm, s.button_ok, false);
    hide_if(wm, s.button_continue, false);
    for w in [s.text_entry_chat, s.button_emote, s.listbox_chat, s.listbox_academy, s.static_text_academy_title, s.chat_box_border, s.button_buddies] {
        hide_if(wm, w, true);
    }
    let not_challenge = s.game.as_ref().and_then(|g| g.campaign.get_current_campaign()).is_some_and(|c| !c.is_challenge_campaign());
    if not_challenge {
        wt::set_group(wm, h, b"ScoreScreenShow", false);
    }
}

/// `initReplaySinglePlayer()`.
fn init_replay_single_player<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.score_screen().screen_type = SCORESCREEN_REPLAY;
    grab_single_player_info(wm, h);
    let s = h.score_screen();
    for w in [s.static_text_game_saved, s.text_entry_chat, s.button_emote, s.chat_box_border, s.button_continue, s.button_buddies, s.listbox_chat, s.listbox_academy, s.static_text_academy_title] {
        hide_if(wm, w, true);
    }
}

/// `initLANMultiPlayer()`.
fn init_lan_multi_player<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.score_screen().screen_type = SCORESCREEN_LAN;
    grab_multi_player_info(wm, h);
    let chat = h.score_screen().text_entry_chat;
    te::set_text(wm, h, chat, &[]);
    wm.win_set_focus(h, chat);
    let s = h.score_screen().clone();
    hide_if(wm, s.static_text_game_saved, true);
    hide_if(wm, s.text_entry_chat, false);
    hide_if(wm, s.button_emote, false);
    hide_if(wm, s.button_continue, true);
    hide_if(wm, s.listbox_chat, false);
    hide_if(wm, s.listbox_academy, true);
    hide_if(wm, s.static_text_academy_title, true);
    hide_if(wm, s.chat_box_border, false);
    hide_if(wm, s.button_buddies, true);
}

/// `initInternetMultiPlayer()`: with no `TheGameSpyInfo` (the client has no GameSpy) the buddy button is hidden, and
/// there is no `TheGameSpyBuddyMessageQueue`: it returns before the status request.
fn init_internet_multi_player<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.score_screen().screen_type = SCORESCREEN_INTERNET;
    grab_multi_player_info(wm, h);
    let chat = h.score_screen().text_entry_chat;
    te::set_text(wm, h, chat, &[]);
    wm.win_set_focus(h, chat);
    let s = h.score_screen().clone();
    hide_if(wm, s.static_text_game_saved, true);
    hide_if(wm, s.button_continue, true);
    hide_if(wm, s.text_entry_chat, true);
    hide_if(wm, s.button_emote, true);
    hide_if(wm, s.listbox_chat, false);
    hide_if(wm, s.listbox_academy, false);
    hide_if(wm, s.static_text_academy_title, false);
    hide_if(wm, s.chat_box_border, false);
    // `if(TheGameSpyInfo && getLocalProfileID() == 0) hide; else show`: `TheGameSpyInfo` is `NULL` here, so shown
    hide_must(wm, s.button_buddies, false, "initInternetMultiPlayer: no ButtonBuddy (buttonBuddies->winHide)");
    // `if (!TheGameSpyBuddyMessageQueue) return;`
}

/// `initReplayMultiPlayer()`.
fn init_replay_multi_player<H: ScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.score_screen().screen_type = SCORESCREEN_REPLAY;
    grab_multi_player_info(wm, h);
    let s = h.score_screen();
    for w in [s.static_text_game_saved, s.text_entry_chat, s.button_emote, s.listbox_chat, s.listbox_academy, s.static_text_academy_title, s.chat_box_border, s.button_continue, s.button_buddies] {
        hide_if(wm, w, true);
    }
}

/// A grid window `ScoreScreen.wnd:<base><pos>` under the parent; `None` is recorded as undefined (upstream's
/// `DEBUG_ASSERTCRASH` is compiled out, then the window is used).
fn grid_win<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, base: &str, pos: i32) -> Option<WinId> {
    let parent = h.score_screen().parent;
    let w = find(wm, h, parent, &format!("{base}{pos}"));
    must(wm, w, "ScoreScreen: a grid window is missing (win->... on NULL)")
}

/// A number into a grid window: its text, the colour, shown.
fn grid_number<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, base: &str, pos: i32, value: i32, color: i32) -> bool {
    let Some(w) = grid_win(wm, h, &format!("StaticText{base}"), pos) else { return false };
    gadget::static_text_set_text(wm, h, Some(w), &wide(&format!("{value}")));
    set_text_color(wm, w, color);
    wm.win_hide(w, false);
    true
}

/// The seven numbers of a row (units built, lost, destroyed; buildings built, lost, destroyed twice; the money earned).
fn grid_numbers<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, pos: i32, values: [i32; 7], money: i32, color: i32) -> bool {
    for (base, v) in ROW_STATS.iter().zip(values) {
        if !grid_number(wm, h, base, pos, v, color) {
            return false;
        }
    }
    grid_number(wm, h, "Resources", pos, money, color)
}

/// `populatePlayerInfo(player, pos)` (the player by its index in the game's list), up to its internet branch (#89: the
/// stats report is a gap).
pub fn populate_player_info<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, player: usize, pos: i32) {
    if pos > MAX_SLOTS {
        return;
    }
    let s = h.score_screen();
    let override_name = s.override_player_display_name;
    let screen_type = s.screen_type;
    let Some(g) = s.game.as_ref() else { return };
    let Some(p) = g.players.get(player).cloned() else { return };
    let local = g.local;
    let mode = g.game_mode;
    let advice = g.local_academy_advice.clone();
    let side_icon = p.template.and_then(|t| g.templates.get(t)).map(|t| t.side_icon_image.clone());
    let color = p.color;
    let sk = &p.score_keeper;
    // the player's name
    let Some(w) = grid_win(wm, h, "StaticTextPlayer", pos) else { return };
    let name = if override_name { h.grid_fetch_text(b"GUI:Player") } else { p.display_name.clone() };
    gadget::static_text_set_text(wm, h, Some(w), &name);
    wm.win_hide(w, false);
    set_text_color(wm, w, color);
    let Some(w) = grid_win(wm, h, "StaticTextObserver", pos) else { return };
    wm.win_hide(w, true);
    let values = [sk.total_units_built(), sk.total_units_lost(), sk.total_units_destroyed(), sk.total_buildings_built(), sk.total_buildings_lost(), sk.total_buildings_destroyed(), sk.total_buildings_destroyed()];
    if !grid_numbers(wm, h, pos, values, sk.total_money_earned(), color) {
        return;
    }
    if Some(player) == local {
        let s = h.score_screen();
        let (academy, title) = (s.listbox_academy, s.static_text_academy_title);
        if let Some(l) = academy {
            wm.win_hide(l, false);
            hide_if(wm, title, false);
            if mode == game_mode::SKIRMISH || mode == game_mode::INTERNET {
                for tip in advice.unwrap_or_default() {
                    let mut t = wide("\n\n");
                    t.extend(h.grid_fetch_text(&tip));
                    lb::add_entry_text(wm, h, l, &t, wt::game_make_color(255, 255, 255, 255), -1, -1, true);
                }
            }
        }
    }
    // the winner marker
    let Some(w) = grid_win(wm, h, "GameWindowWinner", pos) else { return };
    wm.win_hide(w, false);
    if let Some(icon) = side_icon {
        let img = image(h, &icon);
        set_enabled_image(wm, w, img);
    }
    if screen_type == SCORESCREEN_SKIRMISH && Some(player) == local {
        let g = h.score_screen().game.as_ref().expect("checked");
        let sandbox = match g.game_info.as_ref() {
            Some(gi) => gi.is_sandbox(),
            None => {
                wm.set_undefined("populatePlayerInfo: a skirmish without TheGameInfo (TheGameInfo->isSandbox on NULL)");
                return;
            }
        };
        let Ok(sandbox) = sandbox else {
            wm.set_undefined("populatePlayerInfo: TheGameInfo has no local slot (isSandbox reads NULL)");
            return;
        };
        let (victory, defeat) = (g.local_allied_victory, g.local_allied_defeat);
        if skirmish_stats_skipped(sandbox, victory, defeat, !p.observer && !p.dead) {
            return;
        }
        // (#157: translated; the template is read only after the stats file, as upstream)
        let side = p.template.and_then(|t| g.templates.get(t)).map(|t| t.side.clone());
        let inp = g.honors_inputs.clone();
        let gi = g.game_info.clone().expect("checked");
        let is_skirmish = gi.is_skirmish(|i| crate::common::game_info::TheGameInfo::Is(&gi, Some(i)));
        let Some(side) = side else {
            wm.set_undefined("populatePlayerInfo: the local player has no template (getPlayerTemplate()->getSide on NULL)");
            return;
        };
        let Ok(is_skirmish) = is_skirmish else {
            wm.set_undefined("populatePlayerInfo: isSkirmish read a NULL slot");
            return;
        };
        let r = skirmish_stats_update(&mut GridUserData(h), victory, &side, &inp, &gi, is_skirmish);
        match r {
            Ok(mut stats) => report_undefined(wm, &mut stats.prefs),
            Err(e) => {
                wm.set_undefined(e);
                return;
            }
        }
    }
    if screen_type == SCORESCREEN_INTERNET {
        h.score_untranslated(b"populatePlayerInfo: SCORESCREEN_INTERNET stats (GameSpy)");
    }
}

/// `grabMultiPlayerInfo()`: the players `player0` .. `player7` by score, highest first.
pub fn grab_multi_player_info<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H) {
    let has_local = h.score_screen().game.as_ref().is_some_and(|g| g.local.is_some());
    if has_local {
        set_parent_image(wm, h, b"MutiPlayer_ScoreScreen");
    }
    let mut scores: BTreeMap<i32, usize> = BTreeMap::new();
    let mut adder = 1i32;
    let mut player_count = 0;
    for i in 0..MAX_SLOTS {
        let name = format!("player{i}").into_bytes();
        let g = h.score_screen().game.as_mut().expect("a game");
        let found = g.players.iter().take(g.player_count.max(0) as usize).position(|p| p.name == name);
        if let Some(p) = found {
            let mut score = g.players[p].score_keeper.calculate_score();
            if scores.contains_key(&score) {
                score = score.wrapping_add(adder);
                // (plant 8921, differential builds only: the adder not counted up)
                if !plant(8921) {
                    adder = adder.wrapping_add(1);
                }
            }
            scores.insert(score, p);
            player_count += 1;
        }
    }
    hide_windows(wm, h, player_count);
    let order: Vec<usize> = if plant(8922) {
        // (plant 8922, differential builds only: lowest score first)
        scores.values().copied().collect()
    } else {
        scores.values().rev().copied().collect()
    };
    for (count, p) in order.into_iter().enumerate() {
        if wm.undefined.is_some() {
            return;
        }
        let observer = h.score_screen().game.as_ref().expect("a game").players[p].observer;
        if observer {
            set_observer_windows(wm, h, p, count as i32);
        } else {
            populate_player_info(wm, h, p, count as i32);
        }
    }
}

/// `ScoreGather`: a side's sums.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScoreGather {
    pub total_money_earned: i32,
    pub total_money_spent: i32,
    pub total_units_destroyed: i32,
    pub total_units_built: i32,
    pub total_units_lost: i32,
    pub total_buildings_destroyed: i32,
    pub total_buildings_built: i32,
    pub total_buildings_lost: i32,
    /// `m_sideImage` (the image's name; `None`: `NULL`).
    pub side_image: Option<Vec<u8>>,
}

/// `grabSinglePlayerInfo()`.
pub fn grab_single_player_info<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut player_count = 0;
    let Some(g) = h.score_screen().game.as_ref() else { return };
    let real_local = g.local;
    let mut local = real_local;
    if let Some(l) = real_local {
        let observer = g.players[l].observer;
        if !observer {
            populate_player_info(wm, h, l, player_count);
            player_count += 1;
        } else {
            local = None;
            for k in 0..MAX_PLAYER_COUNT as usize {
                let g = h.score_screen().game.as_ref().expect("a game");
                if g.players.get(k).is_some_and(|p| p.human) {
                    local = Some(k);
                    populate_player_info(wm, h, k, player_count);
                    player_count += 1;
                    break;
                }
            }
        }
        if wm.undefined.is_some() {
            return;
        }
        let g = h.score_screen().game.as_ref().expect("a game");
        let screen = g.players[l].template.and_then(|t| g.templates.get(t)).map(|t| t.score_screen_image.clone());
        if let Some(screen) = screen {
            set_parent_image(wm, h, &screen);
        }
    }
    let Some(local) = local else { return };
    // USA_FRIEND, CHINA_FRIEND, GLA_FRIEND, USA_ENEMY, CHINA_ENEMY, GLA_ENEMY
    for j in 0..6 {
        let is_friend = j < 3;
        let side: &[u8] = [&b"USA"[..], b"China", b"GLA"][j % 3];
        let mut sg = ScoreGather::default();
        let mut populate = false;
        let mut color = 0;
        let mut icon: Option<Vec<u8>> = None;
        let g = h.score_screen().game.as_ref().expect("a game");
        for (i, player) in g.players.iter().enumerate().take(MAX_PLAYER_COUNT as usize) {
            if i == local || side != &player.base_side[..] {
                continue;
            }
            if g.game_mode != game_mode::SINGLE_PLAYER || player.list_in_score_screen {
                let rel = g.relations[local][i];
                // (plant 8923, differential builds only: allies and enemies swapped)
                let (want_friend, want_enemy) = if plant(8923) { (Relationship::Enemies, Relationship::Allies) } else { (Relationship::Allies, Relationship::Enemies) };
                if (is_friend && rel == want_friend) || (!is_friend && rel == want_enemy) {
                    let sk = &player.score_keeper;
                    sg.total_buildings_built = sg.total_buildings_built.wrapping_add(sk.total_buildings_built());
                    sg.total_buildings_destroyed = sg.total_buildings_destroyed.wrapping_add(sk.total_buildings_destroyed());
                    sg.total_buildings_lost = sg.total_buildings_lost.wrapping_add(sk.total_buildings_lost());
                    sg.total_money_earned = sg.total_money_earned.wrapping_add(sk.total_money_earned());
                    sg.total_money_spent = sg.total_money_spent.wrapping_add(sk.total_money_spent());
                    sg.total_units_built = sg.total_units_built.wrapping_add(sk.total_units_built());
                    sg.total_units_destroyed = sg.total_units_destroyed.wrapping_add(sk.total_units_destroyed());
                    sg.total_units_lost = sg.total_units_lost.wrapping_add(sk.total_units_lost());
                    let Some(t) = player.template.and_then(|t| g.templates.get(t)) else {
                        wm.set_undefined("grabSinglePlayerInfo: a player without a template (getPlayerTemplate()->getSideIconImage on NULL)");
                        return;
                    };
                    // (`sg.m_sideImage = ...->getSideIconImage()`: the image found below, the last one's)
                    icon = Some(t.side_icon_image.clone());
                    color = player.color;
                    populate = true;
                }
            }
        }
        sg.side_image = icon.and_then(|i| h.grid_find_image(&i));
        if populate {
            let mut label = b"GUI:".to_vec();
            label.extend_from_slice(side);
            label.extend_from_slice(if is_friend { b"Allies" } else { b"Enemies" });
            let text = h.grid_fetch_text(&label);
            populate_side_info(wm, h, &text, &sg, player_count, color);
            if wm.undefined.is_some() {
                return;
            }
            player_count += 1;
        }
    }
    hide_windows(wm, h, player_count);
}

/// `hideWindows(pos)`: the rows from `pos` hidden.
pub fn hide_windows<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, pos: i32) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }
    for i in pos..MAX_SLOTS {
        let names = ["StaticTextPlayer", "StaticTextObserver", "StaticTextUnitsBuilt", "StaticTextUnitsLost", "StaticTextUnitsDestroyed", "StaticTextBuildingsBuilt", "StaticTextBuildingsLost", "StaticTextBuildingsDestroyed", "StaticTextBuildingsDestroyed", "StaticTextResources", "GameWindowWinner"];
        for n in names {
            let Some(w) = grid_win(wm, h, n, i) else { return };
            wm.win_hide(w, true);
        }
    }
}

/// `setObserverWindows(player, i)`: an observer's row (the name in white, the observer marker, the side's icon).
pub fn set_observer_windows<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, player: usize, i: i32) {
    if !(0..MAX_SLOTS).contains(&i) {
        return;
    }
    let color = -1i32; // 0xffffffff
    let g = h.score_screen().game.as_ref().expect("a game");
    let p = g.players[player].clone();
    let icon = p.template.and_then(|t| g.templates.get(t)).map(|t| t.side_icon_image.clone());
    let Some(w) = grid_win(wm, h, "StaticTextPlayer", i) else { return };
    gadget::static_text_set_text(wm, h, Some(w), &p.display_name);
    wm.win_hide(w, false);
    set_text_color(wm, w, color);
    let Some(w) = grid_win(wm, h, "StaticTextObserver", i) else { return };
    wm.win_hide(w, false);
    for n in ["StaticTextUnitsBuilt", "StaticTextUnitsLost", "StaticTextUnitsDestroyed", "StaticTextBuildingsBuilt", "StaticTextBuildingsLost", "StaticTextBuildingsDestroyed", "StaticTextBuildingsDestroyed", "StaticTextResources"] {
        let Some(w) = grid_win(wm, h, n, i) else { return };
        wm.win_hide(w, true);
    }
    let Some(w) = grid_win(wm, h, "GameWindowWinner", i) else { return };
    wm.win_hide(w, false);
    if let Some(icon) = icon {
        let img = image(h, &icon);
        set_enabled_image(wm, w, img);
    }
}

/// `populateSideInfo(side, sg, pos, color)`.
pub fn populate_side_info<H: ScoreGridHost>(wm: &mut GameWindowManager, h: &mut H, side: &[u16], sg: &ScoreGather, pos: i32, color: i32) {
    if !(0..=MAX_SLOTS).contains(&pos) {
        return;
    }
    let Some(w) = grid_win(wm, h, "StaticTextPlayer", pos) else { return };
    gadget::static_text_set_text(wm, h, Some(w), side);
    wm.win_hide(w, false);
    set_text_color(wm, w, color);
    let Some(w) = grid_win(wm, h, "StaticTextObserver", pos) else { return };
    wm.win_hide(w, true);
    // (populateSideInfo sets BuildingsDestroyed once)
    let values = [sg.total_units_built, sg.total_units_lost, sg.total_units_destroyed, sg.total_buildings_built, sg.total_buildings_lost, sg.total_buildings_destroyed];
    for (base, v) in ROW_STATS.iter().take(6).zip(values) {
        if !grid_number(wm, h, base, pos, v, color) {
            return;
        }
    }
    if !grid_number(wm, h, "Resources", pos, sg.total_money_earned, color) {
        return;
    }
    let Some(w) = grid_win(wm, h, "GameWindowWinner", pos) else { return };
    // (plant 8924, differential builds only: the marker shown even without an image)
    if sg.side_image.is_some() || plant(8924) {
        wm.win_hide(w, false);
        set_enabled_image(wm, w, sg.side_image.clone());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, hidden, settle, tick, H};
    use crate::common::game_info::slot_state;

    const F: &str = "ScoreScreen.wnd";
    const CAMPAIGN: &[u8] = b"Campaign USA\n FirstMission = M01\n FinalVictoryMovie = FinalUSA\n Mission M01\n  Map = Maps\\USA01\\USA01.map\n  NextMission = M02\n End\n Mission M02\n  Map = Maps\\USA02\\USA02.map\n End\nEnd\n\
        Campaign Challenge_0\n IsChallengeCampaign = Yes\n FirstMission = M01\n Mission M01\n  Map = Maps\\GC01\\GC01.map\n  GeneralName = GUI:GenA\n End\nEnd\n";
    const CHALLENGE: &[u8] = b"ChallengeGenerals\n GeneralPersona0\n  BioNameString = GUI:GenA\n  Campaign = Challenge_0\n  DefeatedImage = imgDefeated\n  VictoriousImage = imgVictorious\n  DefeatedString = GUI:Def\n  VictoriousString = GUI:Vic\n  WinSound = WinS\n  LossSound = LossS\n End\nEnd\n";

    fn tpl(name: &str, side: &str, icon: &str, screen: &str, music: &str) -> ScoreTemplate {
        ScoreTemplate { name: name.into(), side: side.into(), side_icon_image: icon.into(), score_screen_image: screen.into(), score_screen_music: music.into() }
    }

    /// A score keeper with these totals (player `me`; units destroyed of player 0 counted as another's).
    pub(crate) fn keeper(me: i32, units_built: i32, units_lost: i32, units_destroyed: i32, buildings_built: i32, buildings_lost: i32, buildings_destroyed: i32, money: i32) -> ScoreKeeper {
        let mut k = ScoreKeeper::new();
        let mut ud = [0; 16];
        ud[0] = units_destroyed;
        let mut bd = [0; 16];
        bd[0] = buildings_destroyed;
        k.friend_set_totals(me, money, 7, units_built, units_lost, buildings_built, buildings_lost, ud, bd);
        k
    }

    pub(crate) fn player(name: &str, display: &str, color: i32, k: ScoreKeeper, human: bool, side: &str, template: Option<usize>, listed: bool) -> ScorePlayer {
        ScorePlayer {
            name: name.into(),
            display_name: wide(display),
            color,
            score_keeper: k,
            observer: false,
            dead: false,
            human,
            base_side: side.into(),
            template,
            list_in_score_screen: listed,
            skill_points: 777,
        }
    }

    /// A game of `mode` with these players (the neutral player 0 first, not human), the local player `local`, every
    /// relation neutral, recording.
    pub(crate) fn game(mode: i32, mut players: Vec<ScorePlayer>, local: usize) -> ScoreScreenGame {
        let mut neutral = player("", "Neutral", 0, ScoreKeeper::new(), false, "", None, false);
        neutral.human = false;
        players.insert(0, neutral);
        let count = players.len() as i32;
        while players.len() < MAX_PLAYER_COUNT as usize {
            players.push(player("", "", 0, ScoreKeeper::new(), true, "", None, false));
        }
        let n = players.len();
        let mut campaign = CampaignManager::new();
        campaign.init(CAMPAIGN).expect("Campaign.ini");
        let mut challenge_generals = ChallengeGenerals::new();
        challenge_generals.init(CHALLENGE).expect("ChallengeMode.ini");
        ScoreScreenGame {
            game_mode: mode,
            recorder_mode: Some(RecorderMode::Record),
            recorder_multiplayer: false,
            last_replay_file_name: b"00000000".to_vec(),
            players,
            player_count: count,
            local: Some(local),
            relations: vec![vec![Relationship::Neutral; n]; n],
            templates: vec![tpl("FactionAmerica", "America", "imgUSA", "imgScoreUSA", "ScoreUSA"), tpl("FactionChina", "China", "imgChina", "imgScoreChina", "ScoreChina"), tpl("FactionObserver", "Observer", "", "", "ObsMusic")],
            global_difficulty: 2,
            game_info: None,
            local_allied_victory: true,
            local_allied_defeat: false,
            campaign,
            challenge_generals,
            local_academy_advice: Some(vec![b"ACADEMY:A".to_vec(), b"ACADEMY:B".to_vec()]),
            honors_inputs: SkirmishHonorsInputs::default(),
        }
    }

    /// `TheSkirmishGameInfo` for two slots: the local human (template 0) and an easy AI, no teams.
    pub(crate) fn skirmish_info() -> GameInfo {
        let mut g = GameInfo::new(&mut crate::client::gui_util::SkirmishData::default());
        g.in_game = true;
        g.local_ip = 1;
        g.slots[0].state = slot_state::PLAYER;
        g.slots[0].ip = 1;
        g.slots[0].player_template = 0;
        g.slots[0].team_number = -1;
        g.slots[1].state = slot_state::EASY_AI;
        g.slots[1].team_number = -1;
        g
    }

    /// (#157) `SkirmishStats.ini` as the screen wrote it to the host's user data folder.
    fn stats_file(h: &H) -> String {
        String::from_utf8_lossy(h.user_data.files.get(&b"SkirmishStats.ini"[..]).map_or(&[][..], |v| v.as_slice())).into_owned()
    }

    /// The shell in a game (hidden), then `clearGameData`'s score screen with `g`.
    pub(crate) fn scored(g: ScoreScreenGame) -> (GameWindowManager, H) {
        let (mut wm, mut h) = booted();
        settle(&mut wm, &mut h);
        shell::hide_shell(&mut wm, &mut h);
        h.requests.clear();
        h.score_screen.game = Some(g);
        clear_game_data_score_screen(&mut wm, &mut h);
        (wm, h)
    }

    fn w(wm: &GameWindowManager, name: &str) -> WinId {
        // (the last of the name: a screen made again makes new windows)
        let full = format!("{F}:{name}");
        let i = wm.tree.windows.iter().rposition(|x| x.name() == full.as_bytes()).unwrap_or_else(|| panic!("{name}"));
        i
    }

    /// A grid row: the name, then the seven numbers' texts, each `None` when hidden, and the row's colour.
    fn row(wm: &GameWindowManager, i: i32) -> (Option<String>, Vec<Option<String>>, i32) {
        let t = |n: &str| {
            let x = w(wm, &format!("{n}{i}"));
            (!hidden(wm, x)).then(|| String::from_utf16_lossy(&gadget::static_text_get_text(wm, x)))
        };
        let nums = ["StaticTextUnitsBuilt", "StaticTextUnitsLost", "StaticTextUnitsDestroyed", "StaticTextBuildingsBuilt", "StaticTextBuildingsLost", "StaticTextBuildingsDestroyed", "StaticTextResources"].iter().map(|n| t(n)).collect();
        (t("StaticTextPlayer"), nums, wm.tree.windows[w(wm, &format!("StaticTextPlayer{i}"))].inst.enabled_text.color)
    }

    fn nums(v: [i32; 7]) -> Vec<Option<String>> {
        v.iter().map(|n| Some(n.to_string())).collect()
    }

    fn winner(wm: &GameWindowManager, i: i32) -> (bool, Option<String>) {
        let x = w(wm, &format!("GameWindowWinner{i}"));
        (!hidden(wm, x), wm.tree.windows[x].inst.enabled_draw_data[0].image.as_ref().map(|i| String::from_utf8_lossy(i).into_owned()))
    }

    /// #89 part 2: a skirmish's score screen, predicted from ScoreScreen.cpp before the run. The players: `player0`
    /// (the local human, America, 5 units built, 1 lost, 3 destroyed, 2 buildings built, 0 lost, 1 destroyed, 1000
    /// earned: score 500 + 1000 + 200 + 300 + 100 = 2100), `player1` (China AI, the same totals: 2100, moved to 2101),
    /// `player2` (China AI, 1 unit built: 100).
    ///
    /// - the grid by score: `player1`'s row first (2101), then `player0`'s, then `player2`'s; the display names (a
    ///   skirmish does not override them); each number in the player's colour; rows 3 .. 7 hidden; the winner markers
    ///   with the side icons (`ImgUSA`, `ImgChina`);
    /// - the academy's two tips added for the local player (`\n\nT:ACADEMY:A`, ...), the list shown;
    /// - the chat, emote, border, buddies, continue, chat list and *game saved* hidden; *OK* shown; *Save Replay*
    ///   enabled (recording);
    /// - the battle honours: the skirmish's result recorded (victory, side `America`), nothing written;
    /// - `DontShowMainMenu` set; the first update plays `ScoreUSA` (the local slot's template);
    /// - Escape's release presses *OK*: the screen popped, the main menu's init again, `DontShowMainMenu` cleared, the
    ///   campaign cleared; no undefined behaviour.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn a_skirmish_scores_by_score_highest_first() {
        let k = |u| keeper(1, u, 1, 3, 2, 0, 1, 1000);
        let mut g = game(
            game_mode::SKIRMISH,
            vec![
                player("player0", "Me", 11, k(5), true, "USA", Some(0), true),
                player("player1", "Red", 22, keeper(2, 5, 1, 3, 2, 0, 1, 1000), false, "China", Some(1), true),
                player("player2", "Blue", 33, keeper(3, 1, 0, 0, 0, 0, 0, 0), false, "China", Some(1), true),
            ],
            1,
        );
        g.game_info = Some(skirmish_info());
        let (mut wm, mut h) = scored(g);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(h.score_screen.screen_type, SCORESCREEN_SKIRMISH);
        assert!(h.score_screen.globals.dont_show_main_menu);
        assert_eq!(row(&wm, 0), (Some("Red".into()), nums([5, 1, 3, 2, 0, 1, 1000]), 22));
        assert_eq!(row(&wm, 1), (Some("Me".into()), nums([5, 1, 3, 2, 0, 1, 1000]), 11));
        assert_eq!(row(&wm, 2), (Some("Blue".into()), nums([1, 0, 0, 0, 0, 0, 0]), 33));
        for i in 3..8 {
            assert_eq!(row(&wm, i).0, None, "row {i}");
            assert!(!winner(&wm, i).0);
        }
        assert_eq!(winner(&wm, 0), (true, Some("ImgChina".into())));
        assert_eq!(winner(&wm, 1), (true, Some("ImgUSA".into())));
        let l = w(&wm, "ListboxWarschoolAdvice");
        assert_eq!(lb::get_num_entries(&wm, l), 2);
        assert_eq!(String::from_utf16_lossy(&lb::get_text(&mut wm, &mut h, l, 0, 0)), "\n\nT:ACADEMY:A");
        assert!(!hidden(&wm, l));
        for n in ["TextEntryChat", "ButtonEmote", "ChatBoxBorder", "ButtonBuddy", "ButtonContinue", "ListboxChatWindowScoreScreen", "StaticTextGameSaveComplete"] {
            assert!(hidden(&wm, w(&wm, n)), "{n}");
        }
        assert!(!hidden(&wm, w(&wm, "ButtonOk")));
        assert!(wm.is_enabled(Some(w(&wm, "ButtonSaveReplay"))));
        // (#157) the win counted in SkirmishStats.ini: a win, a streak of 1, the blitz honours (frame 0), the
        // endurance medal of the easy opponent on the map, the first game with the general
        assert_eq!(stats_file(&h), "BestWinStreak = 1\r\nChallenge = 0\r\nHonors = 49152\r\nLastHouse = America\r\nLoyalGames = 0\r\nNOMAP_2 = 1\r\nWinStreak = 1\r\nWins = 1\r\n");
        h.audio.clear();
        tick(&mut wm, &mut h);
        assert!(h.audio.iter().any(|a| a == b"ScoreUSA"), "{:?}", h.audio);
        assert!(!h.score_screen.globals.play_music);
        // Escape
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        settle(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        assert!(!h.score_screen.globals.dont_show_main_menu);
        assert!(h.score_screen.game.as_ref().expect("game").campaign.get_current_campaign().is_none());
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// The single player game of the campaign tests: the local player (USA, skill 777), a listed Chinese enemy, an
    /// unlisted Chinese enemy, a listed GLA ally and a listed USA ally; the campaign `usa` at its first mission.
    fn campaign_game(victorious: bool, mission: &[u8]) -> ScoreScreenGame {
        let mut g = game(
            game_mode::SINGLE_PLAYER,
            vec![
                player("ThePlayer", "Me", 11, keeper(1, 4, 1, 2, 3, 1, 1, 500), true, "USA", Some(0), true),
                player("China1", "C1", 22, keeper(2, 10, 2, 0, 1, 0, 0, 100), false, "China", Some(1), true),
                player("China2", "C2", 23, keeper(3, 50, 0, 0, 0, 0, 0, 0), false, "China", Some(1), false),
                player("GLA1", "G1", 33, keeper(4, 1, 0, 0, 0, 0, 0, 5), false, "GLA", Some(1), true),
                player("USA2", "U2", 44, keeper(5, 2, 0, 1, 0, 0, 0, 10), false, "USA", Some(0), true),
            ],
            1,
        );
        for (q, r) in [(2, Relationship::Enemies), (3, Relationship::Enemies), (4, Relationship::Allies), (5, Relationship::Allies)] {
            g.relations[1][q] = r;
        }
        g.campaign.set_campaign_and_mission(b"usa", mission);
        g.campaign.set_victorious(victorious);
        g.local_academy_advice = None;
        g
    }

    /// #89 part 2: a campaign mission won with one to follow, predicted from ScoreScreen.cpp before the run:
    ///
    /// - init: the rows `T:GUI:Player` (the local player, the name overridden), `T:GUI:USAAllies` (USA2: 2, 0, 1, 0, 0,
    ///   0, 10), `T:GUI:GLAAllies` (GLA1), `T:GUI:ChinaEnemies` (China1 only: China2 is not listed on the score screen);
    ///   rows 4 .. 7 hidden; the parent's image `ImgScoreUSA`; *Save Replay* hidden; the blank layout made and shown;
    ///   the campaign's rank points 777 and difficulty 2;
    /// - the first update (`finishSinglePlayerInit`): the next mission (M02); *Continue* reads `T:GUI:SaveAndContinue`;
    ///   a `MissionSave` request with the campaign at M02; *game saved* shown; the blank layout destroyed; *OK* and
    ///   *Continue* shown; no music (no `TheGameInfo` in a campaign);
    /// - *Continue*: `ReplayWasPressed`; the screen popped at once, the shell hidden, a `StartNextCampaignGame` request
    ///   with the campaign at M02 and the rank points 777.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn a_campaign_victory_saves_and_continues() {
        let (mut wm, mut h) = scored(campaign_game(true, b"m01"));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(row(&wm, 0), (Some("T:GUI:Player".into()), nums([4, 1, 2, 3, 1, 1, 500]), 11));
        assert_eq!(row(&wm, 1), (Some("T:GUI:USAAllies".into()), nums([2, 0, 1, 0, 0, 0, 10]), 44));
        assert_eq!(row(&wm, 2).0.as_deref(), Some("T:GUI:GLAAllies"));
        assert_eq!(row(&wm, 3), (Some("T:GUI:ChinaEnemies".into()), nums([10, 2, 0, 1, 0, 0, 100]), 22));
        assert_eq!(row(&wm, 4).0, None);
        let p = w(&wm, "ParentScoreScreen");
        assert_eq!(wm.tree.windows[p].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgScoreUSA"[..]));
        assert!(hidden(&wm, w(&wm, "ButtonSaveReplay")));
        let blank = h.score_screen.blank_layout.expect("the blank layout");
        assert!(!wm.layouts[blank].hidden);
        let g = h.score_screen.game.as_ref().expect("game");
        assert_eq!((g.campaign.current_rank_points(), g.campaign.get_game_difficulty()), (777, 2));
        tick(&mut wm, &mut h);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let cont = w(&wm, "ButtonContinue");
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(cont)), "T:GUI:SaveAndContinue");
        let [ShellRequest::MissionSave { campaign }] = h.requests.as_slice() else { panic!("{:?}", h.requests) };
        assert_eq!(campaign.get_current_map(), b"Maps\\USA02\\USA02.map");
        assert!(!hidden(&wm, w(&wm, "StaticTextGameSaveComplete")));
        assert!(wm.layouts[blank].deleted && h.score_screen.blank_layout.is_none());
        assert!(!hidden(&wm, w(&wm, "ButtonOk")) && !hidden(&wm, cont));
        assert!(h.audio.is_empty() || !h.audio.iter().any(|a| a.starts_with(b"Score")));
        h.requests.clear();
        settle(&mut wm, &mut h);
        click(&mut wm, &mut h, cont);
        assert!(h.score_screen.globals.replay_was_pressed);
        assert!(!h.shell.is_shell_active());
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        let [ShellRequest::StartNextCampaignGame { campaign }] = h.requests.as_slice() else { panic!("{:?}", h.requests) };
        assert_eq!((campaign.get_current_map(), campaign.current_rank_points()), (b"Maps\\USA02\\USA02.map".to_vec(), 777));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #89 part 2: a defeat, and the campaign's end, predicted before the run:
    ///
    /// - a defeat on M01: *Continue* reads `T:GUI:Retry`, no mission save; *Continue* starts M01 again;
    /// - a victory on M02 (the last): *Continue* reads `T:GUI:EndCampaign`; the honours `setUSACampaignComplete(2)`,
    ///   `setHonors(BATTLE_HONOR_CAMPAIGN_USA)`, `write()`; the final movie asked for (`FinalUSA`: no stream, recorded);
    ///   *OK* and *Continue* hidden and then shown again by the end of `finishSinglePlayerInit` (upstream's order);
    ///   *Continue* then pops the screen (no map), `ReplayWasPressed` false.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn a_defeat_retries_and_the_last_victory_ends_the_campaign() {
        let (mut wm, mut h) = scored(campaign_game(false, b"m01"));
        tick(&mut wm, &mut h);
        let cont = w(&wm, "ButtonContinue");
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(cont)), "T:GUI:Retry");
        assert!(h.requests.is_empty(), "{:?}", h.requests);
        settle(&mut wm, &mut h);
        click(&mut wm, &mut h, cont);
        let [ShellRequest::StartNextCampaignGame { campaign }] = h.requests.as_slice() else { panic!("{:?}", h.requests) };
        assert_eq!(campaign.get_current_map(), b"Maps\\USA01\\USA01.map");

        let (mut wm, mut h) = scored(campaign_game(true, b"m02"));
        tick(&mut wm, &mut h);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let cont = w(&wm, "ButtonContinue");
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(cont)), "T:GUI:EndCampaign");
        assert!(h.score_screen.button_is_finish_campaign);
        assert_eq!(stats_file(&h), format!("Honors = {BATTLE_HONOR_CAMPAIGN_USA}\r\nUSACampaign_2 = 1\r\n"));
        assert!(h.untranslated.iter().any(|u| u == b"PlayMovieAndBlock: no video stream for FinalUSA"));
        assert!(!hidden(&wm, cont) && !hidden(&wm, w(&wm, "ButtonOk")));
        h.requests.clear();
        settle(&mut wm, &mut h);
        click(&mut wm, &mut h, cont);
        settle(&mut wm, &mut h);
        assert!(h.requests.is_empty(), "{:?}", h.requests);
        assert!(!h.score_screen.globals.replay_was_pressed);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
    }

    /// #89 part 2: a challenge won, predicted before the run: the challenge's display (the backdrop and the grid's
    /// parent hidden, the header `T:GUI:ChallengeWinText` (no `%s` in the test's text), the remarks `T:GUI:Def`, the
    /// portrait `ImgDefeated`, the parent's image `GeneralsChallengeWinLoss` not found: `NULL`), the win sound `WinS`;
    /// *Save Replay* disabled and hidden; the challenge's end in the honours (`setChallengeCampaignComplete(0, 2)`,
    /// `setHonors(BATTLE_HONOR_CHALLENGE_MODE)`, `write()`), no `ScoreScreenShow`.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn a_challenge_victory_shows_the_defeated_general() {
        let mut g = campaign_game(true, b"");
        g.campaign.set_campaign(b"challenge_0");
        g.campaign.set_victorious(true);
        let (mut wm, mut h) = scored(g);
        let sr = w(&wm, "ButtonSaveReplay");
        assert!(hidden(&wm, sr) && !wm.is_enabled(Some(sr)));
        h.audio.clear();
        tick(&mut wm, &mut h);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(hidden(&wm, w(&wm, "MainBackdrop")) && hidden(&wm, w(&wm, "GadgetParent")));
        assert_eq!(String::from_utf16_lossy(&gadget::static_text_get_text(&wm, w(&wm, "ChallengeWinLossText"))), "T:GUI:ChallengeWinText");
        assert_eq!(String::from_utf16_lossy(&gadget::static_text_get_text(&wm, w(&wm, "GeneralRemarks"))), "T:GUI:Def");
        let portrait = w(&wm, "BigPortrait");
        assert!(!hidden(&wm, portrait));
        assert_eq!(wm.tree.windows[portrait].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgDefeated"[..]));
        assert_eq!(wm.tree.windows[w(&wm, "ParentScoreScreen")].inst.enabled_draw_data[0].image, None);
        assert!(h.audio.iter().any(|a| a == b"WinS"));
        assert_eq!(stats_file(&h), format!("ChallengeCampaign0_2 = 1\r\nHonors = {BATTLE_HONOR_CHALLENGE_MODE}\r\n"));
    }

    /// #89 part 2: the other screens and the buttons, predicted before the run:
    ///
    /// - LAN, with an observer (`player1`): the observer's row has its name in white (`0xffffffff`), the observer
    ///   marker shown, its numbers hidden, the winner marker shown; the chat entry, emote, chat list and border shown,
    ///   the focus on the chat entry; *Emote* empties the chat entry (no `TheLAN` to send to); `GEM_EDIT_DONE` the same;
    ///   *Buddies* is recorded (`GameSpyToggleOverlay`); *Save Replay* disables the buttons and runs the replay popup's
    ///   init (~~not translated: recorded~~ **Corrected 2026-11-10 (#98):** translated: its `parent` found) and shows it;
    ///   ~~the next update records `PopupReplayUpdate`~~ **Corrected 2026-11-10 (#98):** the next update runs the
    ///   translated `PopupReplayUpdate` (nothing recorded);
    /// - internet: the academy's list and title shown, the buddy button shown (no `TheGameSpyInfo`), the stats report
    ///   recorded for each row;
    /// - a multiplayer replay: the replay type, *Save Replay* hidden; a single player replay with an observer as the
    ///   local player: the first human player's row (the name overridden);
    /// - `ButtonAdd0` pressed: undefined (`TheGameSpyInfo` is `NULL`).
    ///
    /// **Result (2026-11-10):** one miss, in the prediction: the focus is the parent's, not the chat entry's.
    /// `initLANMultiPlayer` gives it to the chat entry, and `ScoreScreenInit` gives it to the parent after (upstream's
    /// order, kept). The rest as predicted.
    #[test]
    fn lan_internet_replays_and_the_buttons() {
        let players = || {
            let mut obs = player("player1", "Watcher", 22, ScoreKeeper::new(), true, "Observer", Some(2), true);
            obs.observer = true;
            vec![player("player0", "Me", 11, keeper(1, 5, 1, 3, 2, 0, 1, 1000), true, "USA", Some(0), true), obs]
        };
        let (mut wm, mut h) = scored(game(game_mode::LAN, players(), 1));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(h.score_screen.screen_type, SCORESCREEN_LAN);
        assert_eq!(row(&wm, 1).0.as_deref(), Some("Watcher"));
        assert_eq!(row(&wm, 1).2, -1);
        assert!(row(&wm, 1).1.iter().all(|n| n.is_none()));
        assert!(!hidden(&wm, w(&wm, "StaticTextObserver1")) && winner(&wm, 1).0);
        for n in ["TextEntryChat", "ButtonEmote", "ListboxChatWindowScoreScreen", "ChatBoxBorder"] {
            assert!(!hidden(&wm, w(&wm, n)), "{n}");
        }
        let chat = w(&wm, "TextEntryChat");
        // (prediction missed: `initLANMultiPlayer` gives the chat entry the focus, then `ScoreScreenInit`'s own
        // `winSetFocus(parent)` after it takes it back: the parent has it)
        assert_eq!(wm.keyboard_focus, Some(w(&wm, "ParentScoreScreen")));
        te::set_text(&mut wm, &mut h, Some(chat), &wide("  hello "));
        let emote = w(&wm, "ButtonEmote");
        click(&mut wm, &mut h, emote);
        assert!(te::get_text(&mut wm, &mut h, Some(chat)).is_empty());
        te::set_text(&mut wm, &mut h, Some(chat), &wide("x"));
        let parent = w(&wm, "ParentScoreScreen");
        wm.win_send_system_msg(&mut h, Some(parent), te::gem::EDIT_DONE, win_data(Some(chat)), 0);
        assert!(te::get_text(&mut wm, &mut h, Some(chat)).is_empty());
        h.untranslated.clear();
        // (the buddy button is hidden in a LAN game: its press sent as a click would send it)
        let buddy = w(&wm, "ButtonBuddy");
        wm.win_send_system_msg(&mut h, Some(parent), ggm::GBM_SELECTED, win_data(Some(buddy)), 0);
        assert!(h.untranslated.iter().any(|u| u == b"GameSpyToggleOverlay(GSOVERLAY_BUDDY)"));
        let sr = w(&wm, "ButtonSaveReplay");
        click(&mut wm, &mut h, sr);
        assert!(!wm.is_enabled(Some(w(&wm, "ButtonOk"))));
        assert!(h.popup_replay.parent.is_some() && !h.untranslated.iter().any(|u| u.starts_with(b"PopupReplay")), "{:?}", h.untranslated);
        let popup = h.shell.popup_replay_layout.expect("the popup");
        assert!(!wm.layouts[popup].hidden);
        tick(&mut wm, &mut h);
        assert!(!h.untranslated.iter().any(|u| u == b"PopupReplayUpdate"));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let add = w(&wm, "ButtonAdd0");
        wm.win_send_system_msg(&mut h, Some(parent), ggm::GBM_SELECTED, win_data(Some(add)), 0);
        assert!(wm.undefined.is_some_and(|u| u.contains("TheGameSpyInfo")));

        let (wm, h) = scored(game(game_mode::INTERNET, players(), 1));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(!hidden(&wm, w(&wm, "ListboxWarschoolAdvice")) && !hidden(&wm, w(&wm, "ButtonBuddy")));
        assert!(h.untranslated.iter().any(|u| u == b"populatePlayerInfo: SCORESCREEN_INTERNET stats (GameSpy)"));

        let mut g = game(game_mode::REPLAY, players(), 1);
        g.recorder_multiplayer = true;
        let (wm, h) = scored(g);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(h.score_screen.screen_type, SCORESCREEN_REPLAY);
        assert!(hidden(&wm, w(&wm, "ButtonSaveReplay")));
        assert_eq!(row(&wm, 0).0.as_deref(), Some("Me"));

        let mut g = game(game_mode::REPLAY, players(), 2);
        g.players[2].observer = true;
        let (wm, h) = scored(g);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(h.score_screen.override_player_display_name);
        assert_eq!(row(&wm, 0).0.as_deref(), Some("T:GUI:Player"));
        // (the first human of the list: player 1, `player0`)
        assert_eq!(row(&wm, 0).1[0].as_deref(), Some("5"));
    }

    /// #89 part 2: a file without `StaticTextGameSaveComplete` is upstream's crash (`staticTextGameSaved->winHide` on
    /// `NULL`): recorded as undefined, the init stops there. Predicted, and **result (2026-11-10):** as predicted.
    #[test]
    fn a_missing_window_is_undefined() {
        let (mut wm, mut h) = booted();
        settle(&mut wm, &mut h);
        h.score_screen.game = Some(game(game_mode::SKIRMISH, vec![], 0));
        // the screen's file loaded, then its *game saved* window renamed away before the init looks
        shell::hide_shell(&mut wm, &mut h);
        let l = crate::client::shell::ShellHost::create_layout(&mut h, &mut wm, b"Menus/ScoreScreen.wnd").expect("the file");
        let saved = w(&wm, "StaticTextGameSaveComplete");
        wm.tree.windows[saved].id = -5;
        score_screen_init(&mut wm, &mut h, l);
        assert_eq!(wm.undefined, Some("ScoreScreenInit: no StaticTextGameSaveComplete (staticTextGameSaved->winHide)"));
    }

    /// `trim` as `UnicodeString::trim`.
    #[test]
    fn trim_as_upstream() {
        assert_eq!(trim(&wide(" \t a b \n")), wide("a b"));
        assert_eq!(trim(&wide("   ")), Vec::<u16>::new());
    }
}
