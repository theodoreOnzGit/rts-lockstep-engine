// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (`GameClient::init`: `TheChallengeGenerals->init()`,
//   `TheCampaignManager->init()`)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/MainMenu.cpp (`prepareCampaignGame`,
//   `setupGameStart`, `doGameStart`: the logic's part)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ChallengeMenu.cpp (`setGeneralCampaign`, the Play
//   button: the logic's part)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp (`finishSinglePlayerInit`'s
//   `gotoNextMission`, `startNextCampaignGame`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #61: campaign missions and the Generals' Challenge in the World. The menus that start them are the client's
//! (C5, not translated); what they hand the logic is translated here, in the order they do it:
//!
//! - **A campaign** ([`World::start_campaign`]): the main menu's campaign button (`setCampaign`), the difficulty menu's
//!   `prepareCampaignGame` (`setGlobalDifficulty`), `setupGameStart` (the campaign's difficulty, the pending file the
//!   first mission's map) and `doGameStart` (`MSG_NEW_GAME` with `GAME_SINGLE_PLAYER`, the difficulty and the rank
//!   points, `InitRandom(0)`). The game starts on a later frame, as any: `prepareNewGame` makes the pending file the map,
//!   and `startNewGame` reads the map's sides (the human side is the local player, the others the map's computer players)
//!   and its single-player scripts. No game options are selected (`TheGameInfo` is `NULL` in a campaign).
//! - **A challenge** ([`World::start_challenge`]): `ChallengeMenu.cpp`'s `setGeneralCampaign` (the general's campaign, its
//!   player template, `TheChallengeGameInfo`'s slot 0 the local player with it) and the Play button (the pending file and
//!   `TheChallengeGameInfo`'s map, the challenge's difficulty, `MSG_NEW_GAME`). `startNewGame` reads
//!   `TheChallengeGameInfo` (`GameSource::Challenge`).
//! - **Victory moves to the next mission**: a script's `VICTORY` sets `TheCampaignManager`'s victory flag; the score
//!   screen's `finishSinglePlayerInit` then calls `gotoNextMission` ([`World::finish_single_player`]), and its Continue
//!   button `startNextCampaignGame` ([`World::start_next_campaign_game`]).
//!
//! ~~**Where this differs from upstream:** `GameLogic::clearGameData` is a cut in the port (`RecorderHost::clear_game_data`),
//! so the next mission is started in a World without a game (the install's, as a save's load is: `import::world_for_save`)
//! given the campaign's state ([`World::carry_campaign_from`]).~~ **Corrected 2026-11-06 (#64):** `clearGameData` is
//! translated ([`World::clear_game_data`]): the end-game timer's `MSG_CLEAR_GAME_DATA` clears the World, and the score
//! screen's Continue starts the next mission in it ([`PendingMission::next`]). `Campaign.ini` and `ChallengeMode.ini` are read with the
//! logic's INI by [`World::from_install`] (upstream's `GameClient::init` reads them); an install without them loads
//! with no campaigns (upstream's `INI::load` throws).

use crate::client::campaign_manager::CAMPAIGN_INI;
use crate::client::challenge_generals::{CHALLENGE_MODE_INI, NUM_GENERALS};
use crate::common::file_system::{FileSystem, LocalDevice};
use crate::common::game_info::{slot_state, GameInfo, GameSlot};
use crate::common::ini::IniError;
use crate::common::message_types::MSG_NEW_GAME;
use crate::common::recorder::{game_mode, RecorderHost};
use crate::logic::world::game_logic::World;

/// A mission of [`World::campaign_missions`]: its campaign, its name, its map and whether it is on the World's disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionEntry {
    pub campaign: Vec<u8>,
    pub mission: Vec<u8>,
    pub map: Vec<u8>,
    pub challenge: bool,
}

/// Why a campaign or challenge did not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignStartError {
    /// `setCampaign` found no such campaign, or it has no first mission (its map is empty).
    NoMission(Vec<u8>),
    /// The challenge's general index is out of range, or its player template is not in the store.
    NoGeneral(i32),
}

impl World {
    /// `GameClient::init`'s `TheChallengeGenerals->init()` and `TheCampaignManager->init()`, from the install's files:
    /// each file loaded if present. Which files were read.
    pub fn load_campaigns<D: LocalDevice>(&mut self, fs: &FileSystem<D>) -> Result<(bool, bool), IniError> {
        let challenge = match fs.open_file(CHALLENGE_MODE_INI) {
            Some(bytes) => {
                self.challenge_generals.init(&bytes)?;
                true
            }
            None => false,
        };
        let campaign = match fs.open_file(CAMPAIGN_INI) {
            Some(bytes) => {
                self.campaign.init(&bytes)?;
                true
            }
            None => false,
        };
        Ok((challenge, campaign))
    }

    /// Every campaign's missions, in the files' order (the campaigns' then their missions'), for a list to start from.
    pub fn campaign_missions(&self) -> Vec<MissionEntry> {
        let mut out = Vec::new();
        for c in &self.campaign.campaign_list {
            for m in &c.missions {
                out.push(MissionEntry { campaign: c.name.clone(), mission: m.name.clone(), map: m.map_name.clone(), challenge: c.is_challenge_campaign });
            }
        }
        out
    }

    /// The main menu's campaign start: `TheCampaignManager->setCampaign(campaign)`, then `prepareCampaignGame(diff)`
    /// (`TheScriptEngine->setGlobalDifficulty`), `setupGameStart(getCurrentMap(), diff)` and `doGameStart()`. The game
    /// starts on a later frame. The mission's map must be on the World's disk ([`World::mount_map`]). The map started.
    pub fn start_campaign(&mut self, campaign: &[u8], difficulty: i32) -> Result<Vec<u8>, CampaignStartError> {
        self.campaign.set_campaign(campaign);
        if self.campaign.get_current_map().is_empty() {
            return Err(CampaignStartError::NoMission(campaign.to_vec()));
        }
        self.set_global_difficulty(difficulty);
        self.setup_game_start(difficulty);
        self.do_game_start(true);
        Ok(self.cuts.new_game.pending_file.clone())
    }

    /// [`World::start_campaign`] at a mission other than the first: `setCampaignAndMission` (as a save's load sets it),
    /// then the same start. Upstream's menus start only a campaign's first mission (the others by Continue, or a
    /// cheat's `MissionSelect` menu, `MapSelectMenu.cpp`); this is for a list that offers every mission.
    pub fn start_campaign_mission(&mut self, campaign: &[u8], mission: &[u8], difficulty: i32) -> Result<Vec<u8>, CampaignStartError> {
        self.campaign.set_campaign_and_mission(campaign, &mission.to_ascii_lowercase());
        let current = self.campaign.get_current_campaign().map(|c| c.name.clone());
        if current.as_deref() != Some(&campaign.to_ascii_lowercase()[..]) || self.campaign.get_current_map().is_empty() {
            return Err(CampaignStartError::NoMission(campaign.to_vec()));
        }
        self.set_global_difficulty(difficulty);
        self.setup_game_start(difficulty);
        self.do_game_start(true);
        Ok(self.cuts.new_game.pending_file.clone())
    }

    /// `setupGameStart(mapName, diff)` without the challenge menu: the campaign's difficulty, and the pending file the
    /// current mission's map.
    fn setup_game_start(&mut self, difficulty: i32) {
        self.campaign.set_game_difficulty(difficulty);
        self.cuts.new_game.pending_file = self.campaign.get_current_map();
    }

    /// `doGameStart()` (and the end of `startNextCampaignGame`, which clears no game: `clear`): the game cleared if one is
    /// on, `MSG_NEW_GAME`
    /// (`GAME_SINGLE_PLAYER`, the campaign's difficulty and rank points), `InitRandom(0)`. `TheRecorder` (an engine
    /// subsystem upstream) is made when the World has none, as [`World::start_skirmish_slots`] does.
    fn do_game_start(&mut self, clear: bool) {
        if clear && RecorderHost::is_in_game(self) {
            RecorderHost::clear_game_data(self);
        }
        if self.cuts.game_loop.recorder.is_none() {
            self.recorder_init();
        }
        let (difficulty, rank_points) = (self.campaign.get_game_difficulty(), self.campaign.get_rank_points());
        self.push_command(MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SINGLE_PLAYER);
            m.append_integer_argument(difficulty);
            m.append_integer_argument(rank_points);
        });
        self.random.init_random(0);
    }

    /// `TheChallengeGameInfo` set up as `ChallengeMenu.cpp`'s init (`init`, `clearSlotList`, `reset`, `enterGame`; made
    /// when there is none), then slot 0 the local player with player template `template_num` (its display name).
    fn challenge_game_with_player(&mut self, template_num: i32, map: &[u8]) -> Result<(), CampaignStartError> {
        use crate::client::campaign_manager::CampaignXferHost;
        let display = self
            .statics
            .player_template_store
            .nth_player_template(template_num)
            .map(|t| t.display_name().str().to_vec())
            .ok_or(CampaignStartError::NoGeneral(template_num))?;
        let mut g = self.take_challenge_game_info().unwrap_or_else(|| GameInfo::new(self));
        g.reset(self);
        g.clear_slot_list(self);
        g.reset(self);
        g.enter_game(self);
        let _ = g.set_map(map, self);
        let mut slot = GameSlot::new();
        slot.set_state(slot_state::PLAYER, &display, 0, self);
        slot.set_player_template(template_num);
        g.set_slot(0, slot);
        self.set_challenge_game_info(Some(g));
        Ok(())
    }

    /// The challenge menu's Play button for general `index` (`setGeneralCampaign(index)`: `setCampaign` of the general's
    /// campaign, `TheChallengeGenerals`' player template number, `TheChallengeGameInfo`'s slot 0; then the pending file
    /// and `TheChallengeGameInfo`'s map the campaign's first map, the challenge's difficulty for the campaign and the
    /// script engine, `MSG_NEW_GAME`, `InitRandom(0)`). `difficulty` is what `setupGameStart` gave `TheChallengeGenerals`
    /// before the menu opened. The map started.
    pub fn start_challenge(&mut self, index: i32, difficulty: i32) -> Result<Vec<u8>, CampaignStartError> {
        if index < 0 || index as usize >= NUM_GENERALS {
            return Err(CampaignStartError::NoGeneral(index));
        }
        self.challenge_generals.set_current_difficulty(difficulty);
        let general = self.challenge_generals.get_challenge_generals()[index as usize].clone();
        self.campaign.set_campaign(general.get_campaign());
        let template_num = self.statics.player_template_store.template_num_by_name(&self.names, general.get_player_template_name());
        self.challenge_generals.set_current_player_template_num(template_num);
        let map = self.campaign.get_current_map();
        if map.is_empty() {
            return Err(CampaignStartError::NoMission(general.get_campaign().to_vec()));
        }
        self.challenge_game_with_player(template_num, &map)?;
        self.cuts.new_game.pending_file = map.clone();
        let d = self.challenge_generals.get_current_difficulty();
        self.campaign.set_game_difficulty(d);
        self.set_global_difficulty(d);
        // (the Play button clears a game in progress before its message, as `doGameStart` does)
        self.do_game_start(true);
        Ok(map)
    }

    /// The score screen's `finishSinglePlayerInit`, the logic's part: when `TheCampaignManager->isVictorious()`,
    /// `gotoNextMission()`. The next mission's map (`getCurrentMap()`; empty at the campaign's end, or after a defeat
    /// when no mission was current), or `None` when the game was not won (the Continue button is then Retry).
    pub fn finish_single_player(&mut self) -> Option<Vec<u8>> {
        if !self.campaign.is_victorious() {
            return None;
        }
        self.campaign.goto_next_mission();
        Some(self.campaign.get_current_map())
    }

    /// The campaign's state carried into `self`, a World without a game ~~(see the module docs: the port has no
    /// `clearGameData`)~~: `TheCampaignManager` and `TheChallengeGenerals`, which upstream's engine keeps across games.
    /// **Corrected 2026-11-06 (#64):** the next mission starts in the World that played (`clearGameData`,
    /// [`World::clear_game_data`]); this makes the fresh World the tests compare it with.
    pub fn carry_campaign_from(&mut self, played: &World) {
        self.campaign = played.campaign.clone();
        self.challenge_generals = played.challenge_generals.clone();
    }

    /// The Continue button's `startNextCampaignGame()`: the pending file the current mission's map; in a challenge
    /// campaign `TheChallengeGameInfo` set up again with the challenge's player template (slot 0) and that map; then
    /// `MSG_NEW_GAME` as `doGameStart`. The map started, or `NoMission` at the campaign's end (the score screen pops the
    /// shell then instead).
    pub fn start_next_campaign_game(&mut self) -> Result<Vec<u8>, CampaignStartError> {
        let map = self.campaign.get_current_map();
        if map.is_empty() {
            let name = self.campaign.get_current_campaign().map(|c| c.name.clone()).unwrap_or_default();
            return Err(CampaignStartError::NoMission(name));
        }
        self.cuts.new_game.pending_file = map.clone();
        if self.campaign.get_current_campaign().is_some_and(|c| c.is_challenge_campaign()) {
            let t = self.challenge_generals.get_current_player_template_num();
            self.challenge_game_with_player(t, &map)?;
            if RecorderHost::is_in_game(self) {
                RecorderHost::clear_game_data(self);
            }
        }
        self.do_game_start(false);
        Ok(map)
    }
}

/// GitHub #62: what a mission list's choice starts (the client's list; the game's own menus are C5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MissionChoice {
    /// A campaign's mission: its first ([`World::start_campaign`], the main menu's path) or another
    /// ([`World::start_campaign_mission`]).
    Campaign { campaign: Vec<u8>, mission: Vec<u8> },
    /// The Generals' Challenge's general of this index ([`World::start_challenge`]).
    Challenge(i32),
    /// The score screen's Continue (or Retry after a defeat): the campaign's current mission
    /// ([`World::start_next_campaign_game`]), with the campaign carried from the game played.
    Continue,
    /// A mission save loaded ([`PendingMission::from_mission_save`]): its load has already made the start's message.
    MissionSave,
}

/// GitHub #62 (new: upstream's file system holds every map): a mission chosen and waiting for its map's folder to be on
/// the World's disk. The World is a copy of the install's World without a game (the base); its map is read from the
/// file system given to [`PendingMission::mount`] (the client's art device: in the browser the player's archives, read
/// by range, as the art is), once a frame until every byte of the folder has arrived; [`PendingMission::start`] then
/// makes the start's message. Nothing of the folder is mounted before all of it is held, so every machine starts the
/// mission from the same bytes.
#[derive(Clone, Debug)]
pub struct PendingMission {
    /// The World the mission starts in.
    pub world: World,
    pub choice: MissionChoice,
    pub difficulty: i32,
    /// The mission's map (`getCurrentMap()` once the choice is made).
    pub map: Vec<u8>,
    /// The files of the map's folder mounted, once they have all arrived.
    pub mounted: Option<usize>,
}

impl PendingMission {
    /// A choice from the list, started in a copy of `base` at `difficulty`: the map it would start, found as the start
    /// will find it (`setCampaign`, `setCampaignAndMission` or the general's campaign, on a copy of the campaign manager).
    /// `NoMission` or `NoGeneral` as the start would fail.
    pub fn new(base: &World, choice: MissionChoice, difficulty: i32) -> Result<PendingMission, CampaignStartError> {
        let mut campaign = base.campaign.clone();
        match &choice {
            MissionChoice::Campaign { campaign: c, mission } => campaign.set_campaign_and_mission(c, &mission.to_ascii_lowercase()),
            MissionChoice::Challenge(i) => {
                let general = base.challenge_generals.get_challenge_generals().get(*i as usize).filter(|_| *i >= 0).ok_or(CampaignStartError::NoGeneral(*i))?;
                campaign.set_campaign(general.get_campaign());
            }
            MissionChoice::Continue | MissionChoice::MissionSave => {}
        }
        let map = campaign.get_current_map();
        if map.is_empty() {
            let name = match &choice {
                MissionChoice::Campaign { campaign: c, .. } => c.clone(),
                _ => campaign.get_current_campaign().map(|c| c.name.clone()).unwrap_or_default(),
            };
            return Err(CampaignStartError::NoMission(name));
        }
        Ok(PendingMission { world: base.clone(), choice, difficulty, map, mounted: None })
    }

    /// The score screen's Continue after `played`'s game ended: ~~`played`'s campaign (after `finishSinglePlayerInit`,
    /// [`World::finish_single_player`]) carried into a copy of `base`~~ **Corrected 2026-11-06 (#64):** the World that
    /// played it, cleared by its `clearGameData` and given `finishSinglePlayerInit` ([`World::finish_single_player`]), in
    /// which the next game starts, as upstream's one engine starts it; and its current map: the next mission after a
    /// victory, the same one after a defeat (Retry). `NoMission` at the campaign's end (the score screen pops the shell).
    pub fn next(played: World) -> Result<PendingMission, CampaignStartError> {
        let world = played;
        let map = world.campaign.get_current_map();
        if map.is_empty() {
            let name = world.campaign.get_current_campaign().map(|c| c.name.clone()).unwrap_or_default();
            return Err(CampaignStartError::NoMission(name));
        }
        let difficulty = world.campaign.get_game_difficulty();
        Ok(PendingMission { world, choice: MissionChoice::Continue, difficulty, map, mounted: None })
    }

    /// A mission save (the score screen's Save and Continue, `GameState::missionSave`) loaded into a copy of `base`
    /// ([`World::load_game`]: its campaign read, and the start's message made, as upstream's `loadGame`): the mission it
    /// starts, waiting for its map. An error when the load fails or the file is not a mission save.
    pub fn from_mission_save(base: &World, bytes: Vec<u8>) -> Result<PendingMission, String> {
        let summary = crate::client::save_game::read_save_summary(bytes.clone()).map_err(|e| format!("the save: {e:?}"))?;
        if summary.info.save_file_type != crate::common::game_state::SAVE_FILE_TYPE_MISSION {
            return Err("not a mission save".to_string());
        }
        let mut world = base.clone();
        world.load_game(bytes).map_err(|e| format!("the mission save's load: {e:?} {:?}", world.cuts.team_live_errors))?;
        let map = summary.info.mission_map_name;
        let difficulty = world.campaign.get_game_difficulty();
        Ok(PendingMission { world, choice: MissionChoice::MissionSave, difficulty, map, mounted: None })
    }

    /// The map's folder put on the World's disk from `fs` ([`World::mount_map_when_ready`]): whether it is mounted. Call
    /// once a frame (the bytes asked for arrive between frames) until it is.
    pub fn mount<D: LocalDevice>(&mut self, fs: &FileSystem<D>) -> bool {
        if self.mounted.is_none() {
            self.mounted = self.world.mount_map_when_ready(fs, &self.map);
        }
        self.mounted.is_some()
    }

    /// The start: the menu's or the score screen's calls for the choice. The game starts on the World's next frames, as
    /// any. Returns the World and the map started.
    pub fn start(self) -> Result<(World, Vec<u8>), CampaignStartError> {
        let mut w = self.world;
        let first = |w: &World, c: &[u8]| {
            let mut m = w.campaign.clone();
            m.set_campaign(c);
            m.get_current_mission().map(|x| x.name.clone())
        };
        let map = match &self.choice {
            MissionChoice::Campaign { campaign, mission } if first(&w, campaign).as_deref() == Some(&mission.to_ascii_lowercase()[..]) => w.start_campaign(campaign, self.difficulty)?,
            MissionChoice::Campaign { campaign, mission } => w.start_campaign_mission(campaign, mission, self.difficulty)?,
            MissionChoice::Challenge(i) => w.start_challenge(*i, self.difficulty)?,
            MissionChoice::Continue => w.start_next_campaign_game()?,
            MissionChoice::MissionSave => w.cuts.new_game.pending_file.clone(),
        };
        Ok((w, map))
    }
}

impl World {
    /// GitHub #62: `GameState::loadGame`'s end for a mission save: `InitRandom(0)`, the pending file the save's mission
    /// map, `MSG_NEW_GAME` (`GAME_SINGLE_PLAYER`, the loaded campaign's difficulty and rank points). (Upstream's
    /// `clearGameData` before the load is the caller's: a World without a game, as for any load.)
    pub(crate) fn start_loaded_mission(&mut self, map: &[u8]) {
        self.cuts.new_game.pending_file = map.to_vec();
        self.do_game_start(false);
    }

    /// GitHub #62: `GameState::missionSave()`'s description: `GUI:MissionSave` formatted with the current campaign's
    /// name (`m_campaignNameLabel`'s text) and its mission number plus one. `None` outside a campaign (upstream
    /// dereferences the campaign unchecked).
    pub fn mission_save_description(&self) -> Option<Vec<u16>> {
        use crate::client::control_bar_popup::{format_wide, FormatArg};
        use crate::common::science::GameTextFetch;
        let c = self.campaign.get_current_campaign()?;
        let fmt = self.statics.game_text.fetch(b"GUI:MissionSave");
        let name = self.statics.game_text.fetch(&c.campaign_name_label);
        let n = self.campaign.get_current_mission_number() + 1;
        Some(format_wide(fmt.str(), &[FormatArg::Str(name.str().to_vec()), FormatArg::Int(n as i64)]))
    }

    /// GitHub #62: whether a single-player game has ended as upstream ends it: the script engine's end-game timer (started
    /// by `VICTORY`, `DEFEAT` and their kin) has run out, where upstream's `ScriptEngine::update` appends
    /// `MSG_CLEAR_GAME_DATA`, ~~(a cut in the port: `se_append_clear_game_data`)~~ whose `clearGameData` shows the score
    /// screen. The client's score screen opens then. **Corrected 2026-11-06 (#64):** the message is appended and its
    /// `clearGameData` runs on the next frame ([`World::clear_game_data`]); the game is over once a single-player game has
    /// been cleared with the score screen, and no game has started since.
    pub fn single_player_game_over(&self) -> bool {
        !RecorderHost::is_in_game(self) && self.game_cleared.is_some_and(|c| c.score_screen && c.mode == game_mode::SINGLE_PLAYER)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! GitHub #61: a campaign mission started from an install, victory moving to the next mission, a challenge, and a
    //! campaign save loading and running on.
    use super::*;
    use crate::common::dict::Dict;
    use crate::common::file_system::MemoryDevice;
    use crate::formats::big::build_big;
    use crate::formats::data_chunk_output::DataChunkOutput;
    use crate::logic::scripts::{action_type, ScriptAction};
    use crate::logic::world::game_logic_new_game::GameSource;
    use crate::logic::world::game_logic_update::CRC_RECALC;
    use crate::logic::world::install_world::tests::{install_archives_for, player_sides};
    use crate::logic::world::install_world::DEFAULT_LANGUAGE;

    pub(crate) const USA01: &[u8] = b"Maps\\USA01\\USA01.map";
    pub(crate) const USA02: &[u8] = b"Maps\\USA02\\USA02.map";
    const GC01: &[u8] = b"Maps\\GC01\\GC01.map";

    /// `Campaign.ini`: `USA`, two missions; `Challenge_0`, a challenge of one mission.
    const CAMPAIGN: &[u8] = b"Campaign USA\n FirstMission = M01\n Mission M01\n  Map = Maps\\USA01\\USA01.map\n  NextMission = M02\n End\n\
        Mission M02\n  Map = Maps\\USA02\\USA02.map\n End\nEnd\n\
        Campaign Challenge_0\n IsChallengeCampaign = Yes\n FirstMission = Mission01\n Mission Mission01\n  Map = Maps\\GC01\\GC01.map\n End\nEnd\n";
    /// `ChallengeMode.ini`: general 0 plays America in `Challenge_0`.
    const CHALLENGE: &[u8] = b"ChallengeGenerals\n GeneralPersona0\n  StartsEnabled = Yes\n  Campaign = Challenge_0\n  PlayerTemplate = FactionAmerica\n End\nEnd\n";

    /// A single-player mission's map: a 30 by 30 flat height map, the sides (the neutral one; `ThePlayer`, human, America;
    /// and, unless `challenge`, `PlyrEnemy`, a computer America with its team), each with an empty script list, and a Tank of
    /// `teamThePlayer` at `x`, 100 (in a challenge map, the player's side is the challenge slot's, so the tank is the
    /// enemy's).
    fn mission_map(w: &mut World, x: f32, challenge: bool) -> Vec<u8> {
        mission_map_with(w, x, challenge, false)
    }

    /// GitHub #62: [`mission_map`], and with `victory` a script of `ThePlayer`'s (or, in a challenge map, `PlyrEnemy`'s) that
    /// wins at once (`CONDITION_TRUE`, `VICTORY`, one shot, every difficulty), so the game ends 120 frames later
    /// (`FRAMES_TO_SHOW_WIN_LOSE_MESSAGE`).
    pub(crate) fn mission_map_with(w: &mut World, x: f32, challenge: bool, victory: bool) -> Vec<u8> {
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        for v in [30, 30, 0, 1, 30, 30, 900] {
            out.write_int(v);
        }
        out.write_array_of_bytes(&[4u8; 900]);
        out.close_data_chunk();
        let templates = crate::logic::script_templates::script_templates(&mut w.names);
        let mut sc = crate::logic::sides_list::SidesCtx::new(w.names.clone(), templates);
        let mut sides: Vec<(&[u8], bool, &[u8])> = vec![(b"", false, b"")];
        if !challenge {
            sides.push((b"ThePlayer", true, b"FactionAmerica"));
        }
        sides.push((b"PlyrEnemy", false, b"FactionAmerica"));
        for (name, human, faction) in sides {
            let mut d = Dict::new();
            d.set_ascii_string(sc.names.generator.name_to_key(b"playerName"), name).unwrap();
            d.set_bool(sc.names.generator.name_to_key(b"playerIsHuman"), human).unwrap();
            d.set_ascii_string(sc.names.generator.name_to_key(b"playerFaction"), faction).unwrap();
            sc.sides.add_side(&d);
        }
        for i in 0..sc.sides.get_num_sides() {
            let mut scripts = Vec::new();
            if victory && i == 1 {
                use crate::logic::scripts::{condition_type, Condition, OrCondition, Script};
                let mut s = Script::new();
                s.script_name = b"WinAtOnce".to_vec();
                (s.is_active, s.is_one_shot, s.easy, s.normal, s.hard) = (true, true, true, true, true);
                s.conditions = vec![OrCondition { conditions: vec![Condition::new(condition_type::CONDITION_TRUE, &sc.templates)] }];
                s.actions = vec![ScriptAction::new(action_type::VICTORY, &sc.templates)];
                scripts.push(s);
            }
            sc.sides.get_side_info_mut(i).expect("the side").scripts = Some(crate::logic::scripts::ScriptList { groups: Vec::new(), scripts });
        }
        crate::logic::sides_list::SidesList::write_sides_data_chunk(&mut out, &mut sc).expect("the sides written");
        out.open_data_chunk(b"ObjectsList", crate::logic::map_object::K_OBJECTS_VERSION_3);
        out.open_data_chunk(b"Object", crate::logic::map_object::K_OBJECTS_VERSION_3);
        for r in [x, 100.0, 0.0, 0.0] {
            out.write_real(r);
        }
        out.write_int(0);
        out.write_ascii_string(b"Tank");
        let mut props = Dict::new();
        let owner: &[u8] = if challenge { b"teamPlyrEnemy" } else { b"teamThePlayer" };
        props.set_ascii_string(w.names.name_to_key(b"originalOwner"), owner).unwrap();
        out.write_dict(&props, &w.names);
        out.close_data_chunk();
        // (a challenge's map: the start waypoints its slot is placed at, as `placeNetworkBuildingsForPlayer` finds them)
        if challenge {
            for (i, (name, wx, wy)) in [(&b"Player_1_Start"[..], 60.0f32, 60.0f32), (b"Player_2_Start", 240.0, 240.0)].into_iter().enumerate() {
                out.open_data_chunk(b"Object", crate::logic::map_object::K_OBJECTS_VERSION_3);
                for r in [wx, wy, 0.0, 0.0] {
                    out.write_real(r);
                }
                out.write_int(0);
                out.write_ascii_string(b"*Waypoints/Waypoint");
                let mut props = Dict::new();
                props.set_int(w.names.name_to_key(b"waypointID"), i as i32 + 1).unwrap();
                props.set_ascii_string(w.names.name_to_key(b"waypointName"), name).unwrap();
                out.write_dict(&props, &w.names);
                out.close_data_chunk();
            }
        }
        out.close_data_chunk();
        out.finish()
    }

    /// The tests' install (`install_world`'s two-player one) with `CampaignZH.big`: `Campaign.ini`, `ChallengeMode.ini` and
    /// the three missions' maps (the first two differ in the tank's x: 120 and 180).
    pub(crate) fn campaign_install() -> (World, FileSystem<MemoryDevice>) {
        let mut scratch = World::new();
        let duel = crate::logic::world::install_world::tests::duel_map(&mut scratch);
        let m1 = mission_map(&mut scratch, 120.0, false);
        let m2 = mission_map(&mut scratch, 180.0, false);
        let gc = mission_map(&mut scratch, 150.0, true);
        let mut d = MemoryDevice::new();
        let mut archives = install_archives_for(&duel, false);
        archives.push((
            b"CampaignZH.big".to_vec(),
            build_big(&[(b"Data\\INI\\Campaign.ini", CAMPAIGN), (b"Data\\INI\\ChallengeMode.ini", CHALLENGE), (USA01, &m1), (USA02, &m2), (GC01, &gc)]),
        ));
        for (name, bytes) in archives {
            d.insert(&[&b"ZH\\"[..], &name].concat(), bytes);
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        let (mut base, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!((report.campaigns, report.challenge_mode), (2, true));
        for m in [USA01, USA02, GC01] {
            assert_eq!(base.mount_map(&fs, m), 1);
        }
        // (the shipped map cache lists every map, the campaigns' too; the tests' lists only the duel: the challenge's map is
        // given the duel's entry, two starts, which `populateRandomStartPosition` dereferences unchecked)
        let duel = base.statics.map_cache.find_map(crate::logic::world::install_world::tests::MAP).expect("the duel").clone();
        std::sync::Arc::make_mut(&mut base.statics).map_cache.maps.insert(GC01.to_ascii_lowercase(), duel);
        (base, fs)
    }

    /// Frames until the game has a local player (at most 30).
    pub(crate) fn start_frames(w: &mut World) {
        for _ in 0..30 {
            w.engine_update().expect("a frame");
            if w.game_started() && w.players.local_index().is_some() {
                return;
            }
        }
        panic!("no local player in 30 frames: {:?}", w.cuts.team_live_errors);
    }

    /// The x of each Tank, in the object list's order.
    pub(crate) fn tanks(w: &World) -> Vec<f32> {
        w.objects.ids_in_order().into_iter().filter_map(|o| w.find_object(o)).filter(|o| o.template().get_name() == b"Tank").map(|o| o.get_position().x).collect()
    }

    /// A script's `VICTORY`, executed as the script engine executes an action.
    pub(crate) fn victory(w: &mut World) {
        let mut a = ScriptAction::new_for_read();
        a.action_type = action_type::VICTORY;
        let mut engine = std::mem::take(&mut w.script_engine);
        w.execute_action_with(&mut engine, &a).expect("VICTORY");
        w.script_engine = engine;
    }

    /// GitHub #61: a campaign's first mission starts from an install, and victory moves to the next.
    ///
    /// **Predicted before the run:** (1) the install reads two campaigns and the challenge personas; (2)
    /// `start_campaign("USA", hard)` gives the first mission's map, and within 30 frames the game is single-player on it:
    /// the players are the map's (the neutral one, `ThePlayer`, `PlyrEnemy`) and the replay observer, the local player
    /// `ThePlayer` (index 1, the map's human side), the script engine's difficulty hard, no game options (`TheGameInfo`
    /// `NULL`), the one tank at x 120 on `ThePlayer`; the save summary's campaign `usa` mission 0; 100 frames with no live
    /// error; (3) a `VICTORY` sets the campaign victorious, and `finish_single_player` moves to `m02`, giving its map; (4)
    /// in a fresh copy of the install's World carrying the campaign, `start_next_campaign_game` starts `m02` (tank at x
    /// 180, mission number 1, not victorious: `startNewGame` clears it); (5) its victory ends the campaign:
    /// `finish_single_player` gives an empty map and `start_next_campaign_game` is `NoMission`; (6) a defeat (no victory)
    /// gives `None`. **Result (2026-11-06):** as predicted, first run.
    #[test]
    fn a_campaign_mission_starts_and_victory_moves_to_the_next() {
        let (base, _fs) = campaign_install();
        // (2)
        let mut a = base.clone();
        assert_eq!(a.start_campaign(b"USA", 2), Ok(USA01.to_vec()));
        start_frames(&mut a);
        eprintln!("players {:?}, local {:?}", player_sides(&a), a.players.local_index());
        assert_eq!(a.cuts.game_loop.game_mode, game_mode::SINGLE_PLAYER);
        let names: Vec<String> = player_sides(&a).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["", "ThePlayer", "PlyrEnemy", "ReplayObserver"]);
        assert_eq!(a.players.local_index(), Some(1));
        assert_eq!(a.script_engine.game_difficulty, crate::logic::world::script_engine::difficulty::HARD);
        assert!(a.cuts.new_game.games.is_empty(), "no game options in a campaign");
        assert_eq!(tanks(&a), [120.0]);
        let tank = a.objects.ids_in_order().into_iter().find(|&o| a.find_object(o).is_some_and(|x| x.template().get_name() == b"Tank")).unwrap();
        assert_eq!(a.get_controlling_player(tank), Some(1));
        for _ in 0..100 {
            a.engine_update().expect("a frame");
        }
        assert!(a.cuts.team_live_errors.is_empty(), "{:?}", a.cuts.team_live_errors);
        let s = crate::client::save_game::read_save_summary(a.save_game().expect("the save")).expect("the summary");
        assert_eq!((s.info.campaign_side.as_slice(), s.info.mission_number), (&b"usa"[..], 0));
        // (6)
        assert!(!a.campaign.is_victorious());
        assert_eq!(a.clone().finish_single_player(), None);
        // (3)
        victory(&mut a);
        assert!(a.campaign.is_victorious());
        assert_eq!(a.finish_single_player(), Some(USA02.to_vec()));
        // (4)
        let mut b = base.clone();
        b.carry_campaign_from(&a);
        assert_eq!(b.start_next_campaign_game(), Ok(USA02.to_vec()));
        start_frames(&mut b);
        assert_eq!(tanks(&b), [180.0]);
        assert_eq!(b.campaign.get_current_mission_number(), 1);
        assert!(!b.campaign.is_victorious());
        assert_eq!(b.players.local_index(), Some(1));
        // (5)
        victory(&mut b);
        assert_eq!(b.finish_single_player(), Some(Vec::new()));
        let mut c = base.clone();
        c.carry_campaign_from(&b);
        assert_eq!(c.start_next_campaign_game(), Err(CampaignStartError::NoMission(b"usa".to_vec())));
        for w in [&a, &b] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// GitHub #61: a campaign mission's save loads and runs on, as a skirmish's does (#58).
    ///
    /// **Predicted before the run:** the `USA` campaign's first mission, run 60 frames and saved by the World, loads into
    /// two copies of the install's World with no game (`new_game_on_load`): each is single-player in `usa`/`m01` with
    /// difficulty hard, the same players and local player as the saved game, the tank where it was; the two loads give the
    /// same logic CRC for 150 frames; and the save's victory then still moves to `m02`. **Result (2026-11-06):** as
    /// predicted, first run.
    #[test]
    fn a_campaign_save_loads_and_runs_on() {
        let (base, _fs) = campaign_install();
        let mut a = base.clone();
        a.start_campaign(b"USA", 2).expect("the campaign");
        start_frames(&mut a);
        for _ in 0..60 {
            a.engine_update().expect("a frame");
        }
        let bytes = a.save_game().unwrap_or_else(|e| panic!("the save: {e:?} {:?}", a.cuts.team_live_errors));
        let load = |bytes: Vec<u8>| {
            let mut w = base.clone();
            w.save_side.new_game_on_load = true;
            w.load_game(bytes).unwrap_or_else(|e| panic!("the load: {e:?} {:?}", w.cuts.team_live_errors));
            w
        };
        let (mut b, mut c) = (load(bytes.clone()), load(bytes));
        for w in [&b, &c] {
            assert_eq!(w.cuts.game_loop.game_mode, game_mode::SINGLE_PLAYER);
            assert_eq!(w.campaign.get_current_mission().map(|m| m.name.clone()), Some(b"m01".to_vec()));
            assert_eq!(w.campaign.get_game_difficulty(), 2);
            assert_eq!(player_sides(w), player_sides(&a));
            assert_eq!(w.players.local_index(), a.players.local_index());
            assert_eq!(tanks(w), tanks(&a));
        }
        let crc = |w: &mut World| {
            w.engine_update().expect("a frame");
            w.logic_crc(CRC_RECALC, None).expect("the CRC")
        };
        for f in 0..150 {
            assert_eq!(crc(&mut b), crc(&mut c), "two loads of one campaign save part at frame {f} after the load");
        }
        victory(&mut b);
        assert_eq!(b.finish_single_player(), Some(USA02.to_vec()));
        for w in [&a, &b, &c] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// GitHub #61: a Generals' Challenge starts. **Predicted before the run:** `start_challenge(0, normal)` sets campaign
    /// `challenge_0` and `TheChallengeGenerals`' template to `FactionAmerica`'s index, makes `TheChallengeGameInfo` with
    /// slot 0 a human of that template, and gives `GC01`'s map; within 30 frames the game is single-player from
    /// `TheChallengeGameInfo` (`GameSource::Challenge`): the slot's player is local, the map's `PlyrEnemy` is a player, the
    /// enemy's tank is on the map; and a save of it carries `TheChallengeGameInfo`, which a load reads back with the same
    /// slots and starts the same players. **Result (2026-11-06):** the first run failed in the fixture (`startNewGame`:
    /// `Undefined(NullObject)`, the challenge's map missing from the tests' map cache, which `populateRandomStartPosition`
    /// dereferences unchecked; fixed in the fixture). **Miss:** two tanks, not one: the challenge slot's own starting unit.
    /// The rest as predicted; the local player is `player0`, America.
    #[test]
    fn a_challenge_starts_and_its_save_loads() {
        let (base, _fs) = campaign_install();
        let mut a = base.clone();
        let america = a.statics.player_template_store.template_num_by_name(&a.names, b"FactionAmerica");
        assert_eq!(a.start_challenge(0, 1), Ok(GC01.to_vec()));
        assert_eq!(a.challenge_generals.get_current_player_template_num(), america);
        let slot0 = |w: &World| {
            let (_, g) = w.cuts.new_game.games.iter().find(|(s, _)| *s == GameSource::Challenge).expect("TheChallengeGameInfo");
            (g.slots[0].state, g.slots[0].player_template)
        };
        assert_eq!(slot0(&a), (crate::common::game_info::slot_state::PLAYER, america));
        start_frames(&mut a);
        eprintln!("challenge players {:?}, local {:?}", player_sides(&a), a.players.local_index());
        assert_eq!(a.cuts.game_loop.game_mode, game_mode::SINGLE_PLAYER);
        assert!(player_sides(&a).iter().any(|(n, _)| n == "PlyrEnemy"));
        let local = a.players.local_index().and_then(|p| player_sides(&a).get(p as usize).cloned());
        assert_eq!(local.map(|(n, s)| (n, s)), Some(("player0".to_string(), "America".to_string())));
        // (and the slot's own starting Tank, `placeNetworkBuildingsForPlayer`'s: missed in the prediction)
        let t = tanks(&a);
        assert!(t.len() == 2 && t.contains(&150.0), "{t:?}");
        for _ in 0..30 {
            a.engine_update().expect("a frame");
        }
        let bytes = a.save_game().unwrap_or_else(|e| panic!("the save: {e:?} {:?}", a.cuts.team_live_errors));
        let mut b = base.clone();
        b.save_side.new_game_on_load = true;
        b.load_game(bytes).unwrap_or_else(|e| panic!("the load: {e:?} {:?}", b.cuts.team_live_errors));
        assert_eq!(slot0(&b), slot0(&a));
        assert_eq!(player_sides(&b), player_sides(&a));
        assert_eq!(b.players.local_index(), a.players.local_index());
        assert_eq!(b.challenge_generals.get_current_player_template_num(), america);
        for w in [&a, &b] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    // ------------------------------------------------------------------------------------------------------------------
    // GitHub #62: a mission's map read by range, as the browser's client reads it.
    // ------------------------------------------------------------------------------------------------------------------

    /// `USA01`'s folder: the map, its `map.ini` and its `map.str` (the last a cut: `GameText::initMapStringFile`).
    const USA01_INI: &[u8] = b"Maps\\USA01\\map.ini";
    const USA01_STR: &[u8] = b"Maps\\USA01\\map.str";

    /// GitHub #62: the campaign install as a player's folder holds it: `install_world`'s archives with `MapsZH.big` holding
    /// a 300 KiB member first (so the maps lie past the 64 KiB the browser fetches of an archive's start), the map cache
    /// (the duel's entry, and the challenge map's, not multiplayer, as the shipped cache lists every map), the duel, and the
    /// three missions' folders; `CampaignZH.big` holds `Campaign.ini` and `ChallengeMode.ini`; `ScriptsZH.big` the skirmish
    /// scripts. With `victory`, `USA01` wins at once ([`mission_map_with`]).
    pub(crate) fn campaign_archives(victory: bool) -> Vec<(Vec<u8>, Vec<u8>)> {
        use crate::formats::big::{ArchiveFile, FilenameList};
        let mut scratch = World::new();
        let duel = crate::logic::world::install_world::tests::duel_map(&mut scratch);
        let scb = crate::logic::world::install_world::tests::skirmish_scripts(&mut scratch.names);
        let m1 = mission_map_with(&mut scratch, 120.0, false, victory);
        let m2 = mission_map(&mut scratch, 180.0, false);
        let gc = mission_map(&mut scratch, 150.0, true);
        let mut archives = install_archives_for(&duel, false);
        let i = archives.iter().position(|(n, _)| n == b"MapsZH.big").expect("MapsZH.big");
        let old = ArchiveFile::open_archive_file(b"MapsZH.big", archives[i].1.clone()).expect("an archive");
        let mut list = FilenameList::new();
        old.get_file_list_in_directory(b"", b"", b"*", &mut list, true);
        let filler = vec![0x3cu8; 300 * 1024];
        let mut members: Vec<(Vec<u8>, Vec<u8>)> = vec![(b"Maps\\Filler\\filler.bin".to_vec(), filler)];
        for name in list.iter() {
            let mut bytes = old.open_file(name).expect("a member").to_vec();
            if name.eq_ignore_ascii_case(b"maps\\mapcache.ini") {
                bytes.extend_from_slice(
                    b"MapCache maps_5Cgc01_5Cgc01_2Emap\n fileSize = 1\n fileCRC = 7\n isOfficial = yes\n isMultiplayer = no\n\
                    numPlayers = 2\n extentMin = X:0.00 Y:0.00 Z:0.00\n extentMax = X:300.00 Y:300.00 Z:0.00\n nameLookupTag = MAP_3ADuel\n\
                    Player_1_Start = X:60.00 Y:60.00 Z:0.00\n Player_2_Start = X:240.00 Y:240.00 Z:0.00\nEND\n",
                );
            }
            members.push((name.to_vec(), bytes));
        }
        members.push((USA01.to_vec(), m1));
        members.push((USA01_INI.to_vec(), b"; USA01's map.ini\n".to_vec()));
        members.push((USA01_STR.to_vec(), b"MAP:USA01\n\"Mission one\"\nEND\n".to_vec()));
        members.push((USA02.to_vec(), m2));
        members.push((GC01.to_vec(), gc));
        let refs: Vec<(&[u8], &[u8])> = members.iter().map(|(p, b)| (p.as_slice(), b.as_slice())).collect();
        archives[i].1 = build_big(&refs);
        archives.push((b"CampaignZH.big".to_vec(), build_big(&[(b"Data\\INI\\Campaign.ini", CAMPAIGN), (b"Data\\INI\\ChallengeMode.ini", CHALLENGE)])));
        archives.push((b"ScriptsZH.big".to_vec(), build_big(&[(b"Data\\Scripts\\SkirmishScripts.scb", &scb)])));
        archives
    }

    /// The archives mounted whole (the logic's load, as the client's `import::Starting` mounts what the page gives whole).
    pub(crate) fn whole_fs(archives: &[(Vec<u8>, Vec<u8>)]) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        for (name, bytes) in archives {
            d.insert(&[&b"ZH\\"[..], name].concat(), bytes.clone());
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        fs
    }

    /// The archives as the browser's client holds them after the start (`import::Starting`'s art device): each by its
    /// length and its first 64 KiB (the page's first fetch, `web/index.html`'s `prefixOf`), keyed by its index; every
    /// other byte is read by range.
    fn ranged_fs(archives: &[(Vec<u8>, Vec<u8>)]) -> FileSystem<crate::common::file_system::RangedDevice> {
        let mut d = crate::common::file_system::RangedDevice::new();
        for (key, (name, bytes)) in archives.iter().enumerate() {
            let keep = crate::formats::big::ArchiveFile::directory_prefix_needed(bytes.len(), bytes).max(bytes.len().min(64 * 1024));
            d.insert_ranged(&[&b"ZH\\"[..], name].concat(), bytes.len(), key as u32, bytes[..keep].to_vec());
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        fs
    }

    /// The page's part: every range the device wants read from the archives and handed back (in reverse order when
    /// `reverse`). The ranges served.
    fn serve(fs: &mut FileSystem<crate::common::file_system::RangedDevice>, archives: &[(Vec<u8>, Vec<u8>)], reverse: bool) -> usize {
        let mut wanted = fs.local.device.take_wanted();
        if reverse {
            wanted.reverse();
        }
        for &(key, o, l) in &wanted {
            let b = &archives[key as usize].1;
            fs.local.device.provide(key, o, b[o..o + l].to_vec());
        }
        wanted.len()
    }

    /// A pending mission mounted from the ranged device: the rounds it waited (each round the page serves what was asked).
    fn mount_by_range(p: &mut PendingMission, fs: &mut FileSystem<crate::common::file_system::RangedDevice>, archives: &[(Vec<u8>, Vec<u8>)], reverse: bool) -> usize {
        let mut rounds = 0;
        while !p.mount(fs) {
            rounds += 1;
            assert!(rounds < 5, "the map's folder never arrived");
            serve(fs, archives, reverse);
        }
        rounds
    }

    /// The install's World (no game, no mission map mounted) from the archives given whole.
    pub(crate) fn base_of(archives: &[(Vec<u8>, Vec<u8>)]) -> World {
        let (base, report) = World::from_install(&whole_fs(archives), DEFAULT_LANGUAGE).expect("the install");
        assert_eq!((report.campaigns, report.challenge_mode), (2, true));
        base
    }

    /// Frames until [`World::single_player_game_over`] (at most `limit`): how many.
    pub(crate) fn frames_to_game_over(w: &mut World, limit: usize) -> usize {
        for f in 1..=limit {
            w.engine_update().expect("a frame");
            if w.single_player_game_over() {
                return f;
            }
        }
        panic!("the game did not end in {limit} frames");
    }

    /// GitHub #62: a mission chosen from the list starts from a map read by range, and its victory moves to the next
    /// mission's map, read by range too.
    ///
    /// **Predicted before the run:** (1) the install's World has none of the missions' files on its disk; (2) the choice
    /// `USA`/`M01` (the campaign's first mission) resolves to `USA01`'s map; its first mount waits (the members lie past
    /// the 64 KiB held) and asks for exactly three ranges, the folder's three files; after one round it mounts 3 files, whose
    /// bytes are the archive's members; the 300 KiB filler is never read, so fewer than 64 KiB arrive; (3) started, the game
    /// is single-player on `USA01` within 30 frames, the local player `ThePlayer` (1), the tank at x 120, the map's
    /// `map.ini` read (an `INI::load` cut naming it); (4) its script wins on the first frames, so the game is over 120 frames
    /// after the `VICTORY` (between 120 and 125 frames after the start frames), and not one frame before; (5) the score
    /// screen's `finish_single_player` gives `USA02`; `PendingMission::next` resolves to it, waits one round (one range: its
    /// folder is the map alone), mounts 1 file and starts it: the tank at x 180, mission number 1, not victorious; (6) a
    /// defeat there (the `DEFEAT` action) ends the game too, `finish_single_player` is `None` and Continue (Retry) resolves
    /// to `USA02` again; (7) a choice of `M02` from the list starts `USA02` directly, mission number 1; (8) a choice of a
    /// campaign that is not there is `NoMission`, and a general out of range `NoGeneral`.
    #[test]
    fn a_mission_read_by_range_starts_and_victory_moves_to_the_next() {
        let archives = campaign_archives(true);
        let base = base_of(&archives);
        let mut fs = ranged_fs(&archives);
        // (1)
        for m in [USA01, USA02, GC01, USA01_INI] {
            assert!(!base.save_side.disk.contains_key(&m.to_ascii_lowercase()), "{:?} on the base's disk", String::from_utf8_lossy(m));
        }
        // (2)
        let mut p = PendingMission::new(&base, MissionChoice::Campaign { campaign: b"USA".to_vec(), mission: b"M01".to_vec() }, 2).expect("the choice");
        assert_eq!(p.map, USA01.to_vec());
        assert!(!p.mount(&fs), "the first mount waits");
        let asked = serve(&mut fs, &archives, false);
        assert_eq!(asked, 3);
        assert_eq!(mount_by_range(&mut p, &mut fs, &archives, false), 0);
        assert_eq!(p.mounted, Some(3));
        let maps = crate::formats::big::ArchiveFile::open_archive_file(b"MapsZH.big", archives.iter().find(|(n, _)| n == b"MapsZH.big").unwrap().1.clone()).unwrap();
        for m in [USA01, USA01_INI, USA01_STR] {
            assert_eq!(p.world.save_side.disk.get(&m.to_ascii_lowercase()).map(Vec::as_slice), maps.open_file(m), "{:?}", String::from_utf8_lossy(m));
        }
        let arrived = fs.local.device.stats().bytes_arrived;
        assert!(arrived < 64 * 1024, "{arrived} bytes arrived");
        // (3)
        let (mut a, map) = p.start().expect("the start");
        assert_eq!(map, USA01.to_vec());
        a.cuts.trace = Some(Vec::new());
        start_frames(&mut a);
        let trace = a.cuts.trace.take().unwrap_or_default();
        let started = a.frame();
        assert_eq!(a.cuts.game_loop.game_mode, game_mode::SINGLE_PLAYER);
        assert_eq!(a.players.local_index(), Some(1));
        assert_eq!(tanks(&a), [120.0]);
        let ini = trace.iter().any(|t| t.contains("\"INI::load\"") && t.contains(&format!("text: {:?}", USA01_INI.to_vec())));
        assert!(ini, "the mission's map.ini read");
        // (4)
        let n = frames_to_game_over(&mut a, 200);
        eprintln!("game over {n} frames after the start frames (frame {started} to {})", a.frame());
        assert!((120..=125).contains(&n), "{n}");
        assert!(a.campaign.is_victorious());
        // (5)
        assert_eq!(a.finish_single_player(), Some(USA02.to_vec()));
        let mut p = PendingMission::next(a.clone()).expect("the next mission");
        assert_eq!(p.map, USA02.to_vec());
        assert_eq!(mount_by_range(&mut p, &mut fs, &archives, false), 1);
        assert_eq!(p.mounted, Some(1));
        let (mut b, map) = p.start().expect("the next start");
        assert_eq!(map, USA02.to_vec());
        start_frames(&mut b);
        assert_eq!(tanks(&b), [180.0]);
        assert_eq!(b.campaign.get_current_mission_number(), 1);
        assert!(!b.campaign.is_victorious());
        // (6)
        let mut d = ScriptAction::new_for_read();
        d.action_type = action_type::DEFEAT;
        let mut engine = std::mem::take(&mut b.script_engine);
        b.execute_action_with(&mut engine, &d).expect("DEFEAT");
        b.script_engine = engine;
        frames_to_game_over(&mut b, 200);
        assert_eq!(b.finish_single_player(), None);
        let retry = PendingMission::next(b.clone()).expect("Retry");
        assert_eq!(retry.map, USA02.to_vec());
        // (7)
        let mut p = PendingMission::new(&base, MissionChoice::Campaign { campaign: b"usa".to_vec(), mission: b"M02".to_vec() }, 1).expect("M02");
        assert_eq!(mount_by_range(&mut p, &mut fs, &archives, false), 0, "its range arrived for the Continue");
        let (mut c, _) = p.start().expect("M02's start");
        start_frames(&mut c);
        assert_eq!((tanks(&c), c.campaign.get_current_mission_number()), (vec![180.0], 1));
        // (8)
        assert_eq!(PendingMission::new(&base, MissionChoice::Campaign { campaign: b"Nowhere".to_vec(), mission: Vec::new() }, 1).err(), Some(CampaignStartError::NoMission(b"Nowhere".to_vec())));
        assert_eq!(PendingMission::new(&base, MissionChoice::Challenge(12), 1).err(), Some(CampaignStartError::NoGeneral(12)));
        for w in [&a, &b, &c] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// GitHub #62: the logic's CRCs do not depend on how the map arrived.
    ///
    /// **Predicted before the run:** the `USA` campaign's first mission started three ways, from the map read by range
    /// with the ranges served in order, from it read by range with them served in reverse order, and from the archives
    /// held whole (`World::mount_map`, as a native install mounts it), gives the same logic CRC on each of 300 frames
    /// (through the victory at frame ~2 and the game's end); and the challenge (general 0) started from a map read by range
    /// twice gives the same CRCs for 200 frames.
    #[test]
    fn a_mission_read_by_range_runs_as_one_read_whole() {
        let archives = campaign_archives(true);
        let base = base_of(&archives);
        let run = |choice: MissionChoice, how: u8, frames: usize| -> Vec<u32> {
            let mut p = PendingMission::new(&base, choice, 2).expect("the choice");
            if how == 2 {
                p.mounted = Some(p.world.mount_map(&whole_fs(&archives), &p.map.clone()));
            } else {
                let mut fs = ranged_fs(&archives);
                mount_by_range(&mut p, &mut fs, &archives, how == 1);
            }
            let (mut w, _) = p.start().expect("the start");
            (0..frames)
                .map(|_| {
                    w.engine_update().expect("a frame");
                    w.logic_crc(CRC_RECALC, None).expect("the CRC")
                })
                .collect()
        };
        let usa = || MissionChoice::Campaign { campaign: b"USA".to_vec(), mission: b"M01".to_vec() };
        let (x, y, z) = (run(usa(), 0, 300), run(usa(), 1, 300), run(usa(), 2, 300));
        assert_eq!(x, y, "served in order and in reverse");
        assert_eq!(x, z, "by range and whole");
        eprintln!("USA01 CRC at 300: {:08x}", x[299]);
        let (g1, g2) = (run(MissionChoice::Challenge(0), 0, 200), run(MissionChoice::Challenge(0), 1, 200));
        assert_eq!(g1, g2);
    }

    /// GitHub #62, part 2: the score screen's Save and Continue (`GameState::missionSave`) and its load.
    ///
    /// **Predicted before the run:** after `USA01`'s victory and the score screen's `gotoNextMission` (~~on a copy of the
    /// install's World carrying the played campaign, as the client does it~~ **corrected 2026-11-06 (#64):** on the World
    /// that played, cleared by `clearGameData`, as the client now does it), the mission save holds two blocks,
    /// `CHUNK_GameState` and `CHUNK_Campaign`, its file type `SAVE_FILE_TYPE_MISSION` (1) and its mission map `USA02`'s; its
    /// description is `GUI:MissionSave` formatted, which the tests' text lacks, so upstream's `MISSING: 'GUI:MissionSave'`;
    /// loaded into a copy of the install's World (`PendingMission::from_mission_save`), its campaign is `usa`, mission
    /// `m02`, difficulty hard (2), and its map waits for one round (one range) and mounts; the game starts on `USA02`
    /// (the tank at x 180, local player 1, mission number 1); two loads give the same logic CRCs for 150 frames; a normal
    /// save is refused as not a mission save.
    #[test]
    fn a_mission_save_loads_the_next_mission() {
        let archives = campaign_archives(true);
        let base = base_of(&archives);
        let mut fs = ranged_fs(&archives);
        let mut p = PendingMission::new(&base, MissionChoice::Campaign { campaign: b"USA".to_vec(), mission: b"M01".to_vec() }, 2).expect("the choice");
        mount_by_range(&mut p, &mut fs, &archives, false);
        let (mut a, _) = p.start().expect("the start");
        start_frames(&mut a);
        frames_to_game_over(&mut a, 200);
        // (#64: the World that played, cleared by its `clearGameData`, as upstream's score screen saves after the clear)
        let mut played = a.clone();
        assert_eq!(played.finish_single_player(), Some(USA02.to_vec()));
        let desc = played.mission_save_description().expect("a campaign");
        assert_eq!(String::from_utf16_lossy(&desc), "MISSING: 'GUI:MissionSave'");
        let bytes = played.mission_save(&desc).expect("the mission save");
        let s = crate::client::save_game::read_save_summary(bytes.clone()).expect("the summary");
        let blocks: Vec<String> = s.blocks.iter().map(|b| String::from_utf8_lossy(&b.name).into_owned()).collect();
        eprintln!("mission save: {} bytes, blocks {blocks:?}", bytes.len());
        assert_eq!(blocks, ["CHUNK_GameState", "CHUNK_Campaign"]);
        assert_eq!((s.info.save_file_type, s.info.mission_map_name.as_slice()), (crate::common::game_state::SAVE_FILE_TYPE_MISSION, USA02));
        let load = |fs: &mut FileSystem<crate::common::file_system::RangedDevice>| {
            let mut p = PendingMission::from_mission_save(&base, bytes.clone()).expect("the load");
            assert_eq!(p.world.campaign.get_current_mission().map(|m| m.name.clone()), Some(b"m02".to_vec()));
            assert_eq!(p.world.campaign.get_game_difficulty(), 2);
            let rounds = mount_by_range(&mut p, fs, &archives, false);
            let (mut w, map) = p.start().expect("the start");
            assert_eq!(map, USA02.to_vec());
            start_frames(&mut w);
            (w, rounds)
        };
        let (mut b, rounds) = load(&mut ranged_fs(&archives));
        assert_eq!(rounds, 1);
        let (mut c, _) = load(&mut fs);
        for w in [&b, &c] {
            assert_eq!((tanks(w), w.players.local_index(), w.campaign.get_current_mission_number()), (vec![180.0], Some(1), 1));
        }
        for f in 0..150 {
            b.engine_update().expect("a frame");
            c.engine_update().expect("a frame");
            assert_eq!(b.logic_crc(CRC_RECALC, None).unwrap(), c.logic_crc(CRC_RECALC, None).unwrap(), "frame {f}");
        }
        let normal = a.save_game().expect("a normal save");
        assert_eq!(PendingMission::from_mission_save(&base, normal).err(), Some("not a mission save".to_string()));
    }

    /// Whether `trace` (`cuts.trace`) holds an `INI::load` of `path` (in any case).
    fn ini_loaded(trace: &[String], path: &[u8]) -> bool {
        let forms = [path.to_vec(), path.to_ascii_lowercase()];
        trace.iter().any(|t| t.contains("\"INI::load\"") && forms.iter().any(|f| t.contains(&format!("text: {f:?}"))))
    }

    /// GitHub #64, part 2: a normal save of a mission started from the list reads its map's folder on load.
    ///
    /// **Predicted before the run:** `USA01` started from the list (its folder read by range) runs 30 frames and is saved
    /// (`World::save_game`: a normal save, its pristine map `USA01`'s); (1) into a copy of the install's World,
    /// `mount_save_map_when_ready` on a fresh ranged device waits once (`None`, three ranges asked for), and after the page
    /// serves them mounts 3 files (the map, `map.ini`, `map.str`); (2) the load (`new_game_on_load`) then reads `USA01`'s
    /// `map.ini` (an `INI::load` cut naming it), the tank at x 120, local player 1; (3) the control, the defect: a load into
    /// a copy of the install's World without the mount reads no `map.ini`; (4) the load whose folder came by range and one
    /// whose folder was mounted whole from the archives give the same logic CRCs for 150 frames; (5) bytes that are not a
    /// save are an error.
    #[test]
    fn a_normal_save_of_a_listed_mission_reads_its_folder_on_load() {
        let archives = campaign_archives(true);
        let base = base_of(&archives);
        let mut fs = ranged_fs(&archives);
        let mut p = PendingMission::new(&base, MissionChoice::Campaign { campaign: b"USA".to_vec(), mission: b"M01".to_vec() }, 2).expect("the choice");
        mount_by_range(&mut p, &mut fs, &archives, false);
        let (mut a, _) = p.start().expect("the start");
        start_frames(&mut a);
        for _ in 0..30 {
            a.engine_update().expect("a frame");
        }
        let bytes = a.save_game().expect("the save");
        let s = crate::client::save_game::read_save_summary(bytes.clone()).expect("the summary");
        assert_eq!(s.info.save_file_type, crate::common::game_state::SAVE_FILE_TYPE_NORMAL);
        eprintln!("the save's pristine map {:?}", String::from_utf8_lossy(&s.pristine_map));
        let load = |w: &mut World| -> Vec<String> {
            w.save_side.new_game_on_load = true;
            w.cuts.trace = Some(Vec::new());
            w.load_game(bytes.clone()).unwrap_or_else(|e| panic!("the load: {e:?} {:?}", w.cuts.team_live_errors));
            w.cuts.trace.take().unwrap_or_default()
        };
        // (1)
        let mut fresh = ranged_fs(&archives);
        let mut b = base.clone();
        assert_eq!(b.mount_save_map_when_ready(&fresh, &bytes), Ok(None), "the first mount waits");
        assert_eq!(serve(&mut fresh, &archives, false), 3);
        assert_eq!(b.mount_save_map_when_ready(&fresh, &bytes), Ok(Some(3)));
        // (2)
        let trace = load(&mut b);
        assert!(ini_loaded(&trace, USA01_INI), "the folder's map.ini read on load");
        assert_eq!((tanks(&b), b.players.local_index()), (vec![120.0], Some(1)));
        // (3)
        let mut c = base.clone();
        let trace = load(&mut c);
        assert!(!ini_loaded(&trace, USA01_INI), "without the mount, no map.ini (the defect)");
        // (4)
        let mut d = base.clone();
        assert_eq!(d.mount_map(&whole_fs(&archives), USA01), 3);
        load(&mut d);
        for f in 0..150 {
            b.engine_update().expect("a frame");
            d.engine_update().expect("a frame");
            assert_eq!(b.logic_crc(CRC_RECALC, None).unwrap(), d.logic_crc(CRC_RECALC, None).unwrap(), "frame {f}");
        }
        // (5)
        assert!(base.clone().mount_save_map_when_ready(&fresh, b"not a save").is_err());
        for w in [&a, &b, &c, &d] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// GitHub #62, not a check: with `RTS_WRITE_CAMPAIGN_INSTALL=<dir>` set, writes [`campaign_archives`] (with `USA01`'s
    /// victory script) into `<dir>`, for the browser's mission list to be tried without the game's files. The repository's
    /// own test data, nothing of EA's.
    #[test]
    fn write_the_campaign_install_when_asked() {
        let Ok(dir) = std::env::var("RTS_WRITE_CAMPAIGN_INSTALL") else { return };
        std::fs::create_dir_all(&dir).expect("the folder");
        for (name, bytes) in campaign_archives(true) {
            std::fs::write(std::path::Path::new(&dir).join(String::from_utf8_lossy(&name).as_ref()), bytes).expect("written");
        }
    }
}
