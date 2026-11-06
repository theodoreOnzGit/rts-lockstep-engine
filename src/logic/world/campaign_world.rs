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
//! **Where this differs from upstream:** `GameLogic::clearGameData` is a cut in the port (`RecorderHost::clear_game_data`),
//! so the next mission is started in a World without a game (the install's, as a save's load is: `import::world_for_save`)
//! given the campaign's state ([`World::carry_campaign_from`]). `Campaign.ini` and `ChallengeMode.ini` are read with the
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

    /// The campaign's state carried into `self`, a World without a game (see the module docs: the port has no
    /// `clearGameData`): `TheCampaignManager` and `TheChallengeGenerals`, which upstream's engine keeps across games.
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

#[cfg(test)]
mod tests {
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

    const USA01: &[u8] = b"Maps\\USA01\\USA01.map";
    const USA02: &[u8] = b"Maps\\USA02\\USA02.map";
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
            sc.sides.get_side_info_mut(i).expect("the side").scripts = Some(crate::logic::scripts::ScriptList { groups: Vec::new(), scripts: Vec::new() });
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
    fn campaign_install() -> (World, FileSystem<MemoryDevice>) {
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
    fn start_frames(w: &mut World) {
        for _ in 0..30 {
            w.engine_update().expect("a frame");
            if w.players.local_index().is_some() {
                return;
            }
        }
        panic!("no local player in 30 frames: {:?}", w.cuts.team_live_errors);
    }

    /// The x of each Tank, in the object list's order.
    fn tanks(w: &World) -> Vec<f32> {
        w.objects.ids_in_order().into_iter().filter_map(|o| w.find_object(o)).filter(|o| o.template().get_name() == b"Tank").map(|o| o.get_position().x).collect()
    }

    /// A script's `VICTORY`, executed as the script engine executes an action.
    fn victory(w: &mut World) {
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
}
