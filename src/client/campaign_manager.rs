// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/CampaignManager.h (Mission, Campaign, CampaignManager)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/CampaignManager.cpp (the whole file)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheCampaignManager` (GitHub #61): the campaigns of `Data\INI\Campaign.ini` (`Campaign` blocks, each with its
//! `Mission` blocks), the current campaign and mission, the victory flag, the rank points and difficulty, and its save
//! (`xfer`, version 5, and `loadPostProcess`). The World holds it (`World::campaign`), as the logic reads it upstream
//! (`startNewGame`, the script engine, the victory actions, the save's `CHUNK_Campaign` and `CHUNK_GameLogic`).
//!
//! **Translated whole:** `INI::parseCampaignDefinition`, `m_campaignFieldParseTable`, `parseMissionPart`,
//! `Campaign::newMission`, `getMission`, `getNextMission`, `getFinalVictoryMovie`, `CampaignManager`'s constructor,
//! `getCurrentCampaign`, `getCurrentMission`, `gotoNextMission`, `setCampaignAndMission`, `setCampaign`, `getCurrentMap`,
//! `getCurrentMissionNumber`, `newCampaign`, `isVictorious`, `SetVictorious`, `setRankPoints`, `getRankPoints` (always 0,
//! upstream's comment), `getGameDifficulty`, `setGameDifficulty`, `xfer` and `loadPostProcess` (through
//! [`CampaignXferHost`], for `TheChallengeGameInfo` and `TheChallengeGenerals`).
//!
//! **Where this differs from upstream:** a mission's `BriefingVoice` keeps its event name (`AudioEventRTS`'s
//! `parseAudioEventRTS` token handling, `AudioEventName`); the pointers to a campaign and mission are indices into the
//! lists (upstream's `std::list` keeps them stable, and so does a `Vec` that is only replaced whole, by `newCampaign` and
//! `newMission`, which re-point nothing that is current: see [`CampaignManager::new_campaign`]).

use crate::common::audio_event_name::AudioEventName;
use crate::common::game_info::{GameInfo, GameInfoHost, SkirmishXfer};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};

/// `MAX_OBJECTIVE_LINES`.
pub const MAX_OBJECTIVE_LINES: usize = 5;
/// `MAX_DISPLAYED_UNITS`.
pub const MAX_DISPLAYED_UNITS: usize = 3;
/// `CampaignManager::INVALID_MISSION_NUMBER`.
pub const INVALID_MISSION_NUMBER: i32 = -1;
/// The file `CampaignManager::init` loads.
pub const CAMPAIGN_INI: &[u8] = b"Data\\INI\\Campaign.ini";
/// `DIFFICULTY_NORMAL` (a `GameDifficulty`, saved as its four bytes).
const DIFFICULTY_NORMAL: i32 = 1;

/// `class Mission`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mission {
    /// `m_name` (lower case: `Campaign::newMission`).
    pub name: Vec<u8>,
    pub map_name: Vec<u8>,
    pub next_mission: Vec<u8>,
    pub movie_label: Vec<u8>,
    pub mission_objectives_label: [Vec<u8>; MAX_OBJECTIVE_LINES],
    /// `m_briefingVoice` (its event name).
    pub briefing_voice: AudioEventName,
    pub location_name_label: Vec<u8>,
    pub unit_names: [Vec<u8>; MAX_DISPLAYED_UNITS],
    /// `m_voiceLength` (0: `Mission()`).
    pub voice_length: i32,
    pub general_name: Vec<u8>,
}

/// `class Campaign`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Campaign {
    /// `m_name` (lower case: `CampaignManager::newCampaign`).
    pub name: Vec<u8>,
    pub first_mission: Vec<u8>,
    pub campaign_name_label: Vec<u8>,
    /// `m_missions`, in the file's order.
    pub missions: Vec<Mission>,
    pub final_movie_name: Vec<u8>,
    /// `m_isChallengeCampaign` (FALSE: `Campaign()`).
    pub is_challenge_campaign: bool,
    pub player_faction_name: Vec<u8>,
}

impl Campaign {
    /// `isChallengeCampaign()`.
    pub fn is_challenge_campaign(&self) -> bool {
        self.is_challenge_campaign
    }

    /// `getFinalVictoryMovie()`.
    pub fn get_final_victory_movie(&self) -> &[u8] {
        &self.final_movie_name
    }

    /// `newMission(name)`: the name lower-cased; a mission of that name is deleted, and the new one is added last.
    pub fn new_mission(&mut self, name: &[u8]) -> &mut Mission {
        let name = name.to_ascii_lowercase();
        if let Some(at) = self.missions.iter().position(|m| m.name == name) {
            self.missions.remove(at);
        }
        self.missions.push(Mission { name, ..Default::default() });
        self.missions.last_mut().expect("just pushed")
    }

    /// `getMission(missionName)`: `NULL` for an empty name, else the mission of exactly that name (case counts: the
    /// name is not lower-cased here, upstream's), else `NULL` (a debug assertion upstream).
    pub fn get_mission(&self, mission_name: &[u8]) -> Option<usize> {
        if mission_name.is_empty() {
            return None;
        }
        self.missions.iter().position(|m| m.name == mission_name)
    }

    /// `getNextMission(current)`: the first mission's name for `None`, else the current one's next; lower-cased; an
    /// empty name (the campaign's end) or an unknown one is `NULL`.
    pub fn get_next_mission(&self, current: Option<usize>) -> Option<usize> {
        let name = match current {
            None => self.first_mission.clone(),
            Some(i) => self.missions.get(i).map(|m| m.next_mission.clone()).unwrap_or_default(),
        };
        let name = name.to_ascii_lowercase();
        if name.is_empty() {
            return None;
        }
        self.missions.iter().position(|m| m.name == name)
    }
}

/// `class CampaignManager`.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignManager {
    /// `m_campaignList`, in the files' order.
    pub campaign_list: Vec<Campaign>,
    /// `m_currentCampaign` (an index into the list).
    current_campaign: Option<usize>,
    /// `m_currentMission` (an index into the current campaign's missions).
    current_mission: Option<usize>,
    /// `m_victorious`.
    victorious: bool,
    /// `m_currentRankPoints`.
    current_rank_points: i32,
    /// `m_difficulty` (a `GameDifficulty`).
    difficulty: i32,
    /// `m_xferChallengeGeneralsPlayerTemplateNum`.
    xfer_challenge_generals_player_template_num: i32,
}

impl Default for CampaignManager {
    /// `CampaignManager()`.
    fn default() -> Self {
        CampaignManager {
            campaign_list: Vec::new(),
            current_campaign: None,
            current_mission: None,
            victorious: false,
            current_rank_points: 0,
            difficulty: DIFFICULTY_NORMAL,
            xfer_challenge_generals_player_template_num: 0,
        }
    }
}

/// What `CampaignManager::xfer` reaches outside the manager: `TheChallengeGameInfo` (a `SkirmishGameInfo`) and
/// `TheChallengeGenerals`.
pub trait CampaignXferHost: GameInfoHost {
    /// `TheChallengeGameInfo` taken out (`None` is `NULL`), for the xfer to work on.
    fn take_challenge_game_info(&mut self) -> Option<GameInfo>;
    /// `TheChallengeGameInfo` put back (`None`: `delete TheChallengeGameInfo; TheChallengeGameInfo = NULL`).
    fn set_challenge_game_info(&mut self, game: Option<GameInfo>);
    /// `TheChallengeGenerals->getCurrentPlayerTemplateNum()`.
    fn challenge_player_template_num(&self) -> i32;
    /// `TheChallengeGenerals->setCurrentPlayerTemplateNum(n)`.
    fn set_challenge_player_template_num(&mut self, n: i32);
}

impl CampaignManager {
    /// `CampaignManager()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: `Data\INI\Campaign.ini` loaded (`INI_LOAD_OVERWRITE`), its `Campaign` blocks.
    pub fn init(&mut self, data: &[u8]) -> IniResult<()> {
        Ini::load(CAMPAIGN_INI, data, IniLoadType::Overwrite, None, CAMPAIGN_BLOCKS, self)
    }

    /// `getCurrentCampaign()`.
    pub fn get_current_campaign(&self) -> Option<&Campaign> {
        self.current_campaign.and_then(|c| self.campaign_list.get(c))
    }

    /// `getCurrentMission()`.
    pub fn get_current_mission(&self) -> Option<&Mission> {
        let c = self.get_current_campaign()?;
        self.current_mission.and_then(|m| c.missions.get(m))
    }

    /// `gotoNextMission()`: `NULL` without a campaign or mission; else the current mission's next becomes current
    /// (`NULL` at the campaign's end).
    pub fn goto_next_mission(&mut self) -> Option<&Mission> {
        let c = self.current_campaign?;
        self.current_mission?;
        self.current_mission = self.campaign_list[c].get_next_mission(self.current_mission);
        self.get_current_mission()
    }

    /// `setCampaignAndMission(campaign, mission)`: an empty mission is `setCampaign(campaign)`; else the campaign of
    /// the lower-cased name and its mission of that name (`getMission`, `NULL` if unknown). An unknown campaign changes
    /// nothing.
    pub fn set_campaign_and_mission(&mut self, campaign: &[u8], mission: &[u8]) {
        if mission.is_empty() {
            self.set_campaign(campaign);
            return;
        }
        let campaign = campaign.to_ascii_lowercase();
        if let Some(c) = self.campaign_list.iter().position(|x| x.name == campaign) {
            self.current_campaign = Some(c);
            self.current_mission = self.campaign_list[c].get_mission(mission);
        }
    }

    /// `setCampaign(campaign)`: the campaign of the lower-cased name and its first mission; an unknown name (the empty
    /// one too) clears the campaign and mission and resets the rank points to 0 and the difficulty to normal.
    pub fn set_campaign(&mut self, campaign: &[u8]) {
        let campaign = campaign.to_ascii_lowercase();
        if let Some(c) = self.campaign_list.iter().position(|x| x.name == campaign) {
            self.current_campaign = Some(c);
            self.current_mission = self.campaign_list[c].get_next_mission(None);
            return;
        }
        self.current_campaign = None;
        self.current_mission = None;
        self.current_rank_points = 0;
        self.difficulty = DIFFICULTY_NORMAL;
    }

    /// `getCurrentMap()`: the current mission's map, else empty.
    pub fn get_current_map(&self) -> Vec<u8> {
        self.get_current_mission().map(|m| m.map_name.clone()).unwrap_or_default()
    }

    /// `getCurrentMissionNumber()`: the current mission's position in its campaign (0 based); `INVALID_MISSION_NUMBER`
    /// without a campaign; the campaign's mission count less one when the current mission is not in it (upstream counts
    /// every mission before giving up).
    pub fn get_current_mission_number(&self) -> i32 {
        let mut number = INVALID_MISSION_NUMBER;
        if let Some(c) = self.get_current_campaign() {
            for i in 0..c.missions.len() {
                number += 1;
                if Some(i) == self.current_mission {
                    return number;
                }
            }
        }
        number
    }

    /// `newCampaign(name)`: the name lower-cased; a campaign of that name is deleted, and the new one is added last.
    ///
    /// Upstream deletes the old campaign's object; a current campaign pointer to it would dangle (no load during a game
    /// does this). Here the current campaign and mission are cleared when the deleted one was current, and an index past
    /// it moves down one, so the same campaign stays current.
    pub fn new_campaign(&mut self, name: &[u8]) -> &mut Campaign {
        let name = name.to_ascii_lowercase();
        if let Some(at) = self.campaign_list.iter().position(|c| c.name == name) {
            self.campaign_list.remove(at);
            match self.current_campaign {
                Some(c) if c == at => {
                    self.current_campaign = None;
                    self.current_mission = None;
                }
                Some(c) if c > at => self.current_campaign = Some(c - 1),
                _ => {}
            }
        }
        self.campaign_list.push(Campaign { name, ..Default::default() });
        self.campaign_list.last_mut().expect("just pushed")
    }

    /// `isVictorious()`.
    pub fn is_victorious(&self) -> bool {
        self.victorious
    }
    /// `SetVictorious(victory)`.
    pub fn set_victorious(&mut self, victory: bool) {
        self.victorious = victory;
    }
    /// `setRankPoints(rankPoints)`.
    pub fn set_rank_points(&mut self, rank_points: i32) {
        self.current_rank_points = rank_points;
    }
    /// `getRankPoints()`: always 0 (upstream: every campaign mission starts at rank 0; `m_currentRankPoints` is kept
    /// and saved, never read).
    pub fn get_rank_points(&self) -> i32 {
        0
    }
    /// `m_currentRankPoints`, as saved.
    pub fn current_rank_points(&self) -> i32 {
        self.current_rank_points
    }
    /// `getGameDifficulty()`.
    pub fn get_game_difficulty(&self) -> i32 {
        self.difficulty
    }
    /// `setGameDifficulty(d)`.
    pub fn set_game_difficulty(&mut self, d: i32) {
        self.difficulty = d;
    }
    /// `m_xferChallengeGeneralsPlayerTemplateNum`.
    pub fn xfer_challenge_generals_player_template_num(&self) -> i32 {
        self.xfer_challenge_generals_player_template_num
    }

    /// `xfer(xfer)`, version 5: the current campaign's and mission's names, (2) the rank points, (3) the difficulty;
    /// loading, `setCampaignAndMission` of the names read; (4) whether the campaign is a challenge one, and then
    /// `TheChallengeGameInfo`'s `SkirmishGameInfo::xfer` (made when there is none: `init`, `clearSlotList`, `reset`),
    /// else `TheChallengeGameInfo` deleted; (5) `TheChallengeGenerals`' player template number.
    ///
    /// `XferLoad::xferSnapshot` would add `TheChallengeGameInfo` and its starting cash to the post-process list; their
    /// `loadPostProcess` is empty upstream, so nothing is added.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer, h: &mut impl CampaignXferHost) -> XferResult<()> {
        let current: XferVersion = 5;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut current_campaign = self.get_current_campaign().map(|c| c.name.clone()).unwrap_or_default();
        xfer.xfer_ascii_string(&mut current_campaign)?;
        let mut current_mission = self.get_current_mission().map(|m| m.name.clone()).unwrap_or_default();
        xfer.xfer_ascii_string(&mut current_mission)?;
        if version >= 2 {
            xfer.xfer_int(&mut self.current_rank_points)?;
        }
        if version >= 3 {
            let mut d = self.difficulty.to_le_bytes();
            xfer.xfer_user(&mut d)?;
            self.difficulty = i32::from_le_bytes(d);
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            self.set_campaign_and_mission(&current_campaign, &current_mission);
        }
        if version >= 4 {
            let mut is_challenge_campaign = self.get_current_campaign().is_some_and(|c| c.is_challenge_campaign);
            xfer.xfer_bool(&mut is_challenge_campaign)?;
            if is_challenge_campaign {
                let mut game = match h.take_challenge_game_info() {
                    Some(g) => g,
                    None => {
                        // NEW SkirmishGameInfo; init(); clearSlotList(); reset()
                        let mut g = GameInfo::new(h);
                        g.reset(h);
                        g.clear_slot_list(h);
                        g.reset(h);
                        g
                    }
                };
                let r = xfer.xfer_snapshot(&mut SkirmishXfer { game: &mut game, host: h });
                h.set_challenge_game_info(Some(game));
                r?;
            } else {
                h.set_challenge_game_info(None);
            }
        }
        if version >= 5 {
            let mut player_template_num = h.challenge_player_template_num();
            xfer.xfer_int(&mut player_template_num)?;
            self.xfer_challenge_generals_player_template_num = player_template_num;
        }
        Ok(())
    }

    /// `loadPostProcess()`: `TheChallengeGenerals` given the player template number the load read.
    pub fn load_post_process(&self, h: &mut impl CampaignXferHost) {
        h.set_challenge_player_template_num(self.xfer_challenge_generals_player_template_num);
    }
}

/// `parseMissionPart`'s `myFieldParse`.
pub static MISSION_FIELDS: [FieldParse<Mission>; 16] = [
    FieldParse::new("Map", |i, m, _| i.parse_ascii_string().map(|v| m.map_name = v), UserData::None),
    FieldParse::new("NextMission", |i, m, _| i.parse_ascii_string().map(|v| m.next_mission = v), UserData::None),
    FieldParse::new("IntroMovie", |i, m, _| i.parse_ascii_string().map(|v| m.movie_label = v), UserData::None),
    FieldParse::new("ObjectiveLine0", |i, m, _| i.parse_ascii_string().map(|v| m.mission_objectives_label[0] = v), UserData::None),
    FieldParse::new("ObjectiveLine1", |i, m, _| i.parse_ascii_string().map(|v| m.mission_objectives_label[1] = v), UserData::None),
    FieldParse::new("ObjectiveLine2", |i, m, _| i.parse_ascii_string().map(|v| m.mission_objectives_label[2] = v), UserData::None),
    FieldParse::new("ObjectiveLine3", |i, m, _| i.parse_ascii_string().map(|v| m.mission_objectives_label[3] = v), UserData::None),
    FieldParse::new("ObjectiveLine4", |i, m, _| i.parse_ascii_string().map(|v| m.mission_objectives_label[4] = v), UserData::None),
    FieldParse::new("BriefingVoice", |i, m, _| m.briefing_voice.parse(i), UserData::None),
    FieldParse::new("UnitNames0", |i, m, _| i.parse_ascii_string().map(|v| m.unit_names[0] = v), UserData::None),
    FieldParse::new("UnitNames1", |i, m, _| i.parse_ascii_string().map(|v| m.unit_names[1] = v), UserData::None),
    FieldParse::new("UnitNames2", |i, m, _| i.parse_ascii_string().map(|v| m.unit_names[2] = v), UserData::None),
    FieldParse::new("GeneralName", |i, m, _| i.parse_ascii_string().map(|v| m.general_name = v), UserData::None),
    FieldParse::new("LocationNameLabel", |i, m, _| i.parse_ascii_string().map(|v| m.location_name_label = v), UserData::None),
    FieldParse::new("VoiceLength", |i, m, _| i.parse_int().map(|v| m.voice_length = v), UserData::None),
    FieldParse::end(),
];

/// `parseMissionPart(ini, instance, ...)`: the next token names the mission (`newMission`), then its fields until `End`.
fn parse_mission_part(ini: &mut Ini<'_>, campaign: &mut Campaign, _: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let mission = campaign.new_mission(&name);
    ini.init_from_ini(mission, &MISSION_FIELDS)
}

/// `CampaignManager::m_campaignFieldParseTable`.
pub static CAMPAIGN_FIELDS: [FieldParse<Campaign>; 7] = [
    FieldParse::new("Mission", parse_mission_part, UserData::None),
    FieldParse::new("FirstMission", |i, c, _| i.parse_ascii_string().map(|v| c.first_mission = v), UserData::None),
    FieldParse::new("CampaignNameLabel", |i, c, _| i.parse_ascii_string().map(|v| c.campaign_name_label = v), UserData::None),
    FieldParse::new("FinalVictoryMovie", |i, c, _| i.parse_ascii_string().map(|v| c.final_movie_name = v), UserData::None),
    FieldParse::new("IsChallengeCampaign", |i, c, _| i.parse_bool().map(|v| c.is_challenge_campaign = v), UserData::None),
    FieldParse::new("PlayerFaction", |i, c, _| i.parse_ascii_string().map(|v| c.player_faction_name = v), UserData::None),
    FieldParse::end(),
];

/// `INI::parseCampaignDefinition`: the next token names the campaign (`newCampaign`), then its fields until `End`.
pub fn parse_campaign_definition(ini: &mut Ini<'_>, m: &mut CampaignManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let campaign = m.new_campaign(&name);
    ini.init_from_ini(campaign, &CAMPAIGN_FIELDS)
}

/// The `Campaign` row of `theTypeTable`.
pub static CAMPAIGN_BLOCKS: &[BlockParse<CampaignManager>] = &[BlockParse { token: "Campaign", parse: parse_campaign_definition }];

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic `Campaign.ini` (the shape of the game's: two campaigns, the second a challenge one; every mission
    /// field; a repeated mission and a repeated campaign, which replace the first).
    pub(crate) const INI: &[u8] = b"; synthetic
Campaign TRAINING
  CampaignNameLabel = CAMPAIGN:Training
  FirstMission = M0
  Mission M0
    Map = Maps\\Train\\Train.map
    NextMission = NONE
  End
End

Campaign USA
  CampaignNameLabel = CAMPAIGN:USA
  FirstMission = M01
  FinalVictoryMovie = USAEnd
  PlayerFaction = FactionAmerica
  Mission M01
    Map = Maps\\USA01\\USA01.map
    NextMission = M02
    IntroMovie = USA01Intro
    ObjectiveLine0 = OBJ:One
    ObjectiveLine4 = OBJ:Five
    BriefingVoice = USA01Voice
    UnitNames0 = UNIT:Ranger
    UnitNames2 = UNIT:Tank
    LocationNameLabel = LOC:Somewhere
    VoiceLength = 42
    GeneralName = GEN:Nobody
  End
  Mission M02
    Map = Maps\\Wrong\\Wrong.map
  End
  Mission m02
    Map = Maps\\USA02\\USA02.map
  End
End

Campaign Challenge_0
  IsChallengeCampaign = Yes
  FirstMission = Mission01
  Mission Mission01
    Map = Maps\\GC01\\GC01.map
    GeneralName = GUI:BioNameEntry_Pos0
  End
End

Campaign Training
  FirstMission = M9
  Mission M9
    Map = Maps\\Train2\\Train2.map
  End
End
";

    /// GitHub #61: a synthetic `Campaign.ini` parses. **Predicted before the run:** three campaigns, in the order
    /// `usa`, `challenge_0`, `training` (the second `Training` block deletes the first and is added last), names lower
    /// case; `usa`'s missions `m01`, `m02` (the second `M02`, written `m02`, replaces the first, keeping one, with the
    /// second's map), every field of `m01` as written, `NextMission`'s value as written (`M02`, lower-cased only when
    /// looked up); `challenge_0` a challenge campaign. **Result (2026-11-06):** as predicted, first run.
    #[test]
    fn a_synthetic_campaign_ini_parses() {
        let mut m = CampaignManager::new();
        m.init(INI).expect("the INI parses");
        let names: Vec<&[u8]> = m.campaign_list.iter().map(|c| c.name.as_slice()).collect();
        assert_eq!(names, [&b"usa"[..], b"challenge_0", b"training"]);
        let usa = &m.campaign_list[0];
        assert_eq!(usa.campaign_name_label, b"CAMPAIGN:USA");
        assert_eq!(usa.first_mission, b"M01");
        assert_eq!(usa.get_final_victory_movie(), b"USAEnd");
        assert_eq!(usa.player_faction_name, b"FactionAmerica");
        assert!(!usa.is_challenge_campaign());
        let ms: Vec<&[u8]> = usa.missions.iter().map(|x| x.name.as_slice()).collect();
        assert_eq!(ms, [&b"m01"[..], b"m02"]);
        let m01 = &usa.missions[0];
        assert_eq!(m01.map_name, b"Maps\\USA01\\USA01.map");
        assert_eq!(m01.next_mission, b"M02");
        assert_eq!(m01.movie_label, b"USA01Intro");
        assert_eq!(m01.mission_objectives_label[0], b"OBJ:One");
        assert!(m01.mission_objectives_label[1].is_empty());
        assert_eq!(m01.mission_objectives_label[4], b"OBJ:Five");
        assert_eq!(m01.briefing_voice.event_name, b"USA01Voice");
        assert_eq!(m01.unit_names[0], b"UNIT:Ranger");
        assert_eq!(m01.unit_names[2], b"UNIT:Tank");
        assert_eq!(m01.location_name_label, b"LOC:Somewhere");
        assert_eq!(m01.voice_length, 42);
        assert_eq!(m01.general_name, b"GEN:Nobody");
        assert_eq!(usa.missions[1].map_name, b"Maps\\USA02\\USA02.map");
        assert!(m.campaign_list[1].is_challenge_campaign());
        assert_eq!(m.campaign_list[2].first_mission, b"M9");
    }

    /// `setCampaign`, `gotoNextMission`, `getCurrentMissionNumber`, `setCampaignAndMission` and their edge cases, as
    /// CampaignManager.cpp reads.
    #[test]
    fn the_campaign_moves_through_its_missions() {
        let mut m = CampaignManager::new();
        m.init(INI).unwrap();
        assert_eq!(m.get_current_mission_number(), INVALID_MISSION_NUMBER);
        m.set_rank_points(9);
        m.set_game_difficulty(2);
        m.set_campaign(b"USA");
        assert_eq!(m.get_current_campaign().unwrap().name, b"usa");
        assert_eq!(m.get_current_map(), b"Maps\\USA01\\USA01.map");
        assert_eq!(m.get_current_mission_number(), 0);
        assert_eq!(m.goto_next_mission().map(|x| x.name.clone()), Some(b"m02".to_vec()));
        assert_eq!(m.get_current_mission_number(), 1);
        // the last mission: no next; the campaign stays, the mission is none, the map empty, the number the count less one
        assert!(m.goto_next_mission().is_none());
        assert!(m.get_current_campaign().is_some());
        assert!(m.get_current_map().is_empty());
        assert_eq!(m.get_current_mission_number(), 1);
        assert!(m.goto_next_mission().is_none());
        // a save's names: the mission's exact (lower-case) name; an unknown campaign changes nothing
        m.set_campaign_and_mission(b"usa", b"m02");
        assert_eq!(m.get_current_mission().unwrap().name, b"m02");
        m.set_campaign_and_mission(b"nowhere", b"m01");
        assert_eq!(m.get_current_mission().unwrap().name, b"m02");
        m.set_campaign_and_mission(b"usa", b"M01");
        assert!(m.get_current_mission().is_none(), "getMission compares the name as given");
        // TRAINING's NextMission is NONE, a mission that does not exist: the end
        m.set_campaign(b"training");
        assert_eq!(m.get_current_map(), b"Maps\\Train2\\Train2.map");
        assert_eq!((m.current_rank_points(), m.get_game_difficulty()), (9, 2));
        // the empty campaign: cleared, and the rank points and difficulty reset
        m.set_campaign(b"");
        assert!(m.get_current_campaign().is_none());
        assert_eq!((m.current_rank_points(), m.get_game_difficulty(), m.get_rank_points()), (0, DIFFICULTY_NORMAL, 0));
        // a campaign block read again keeps the current campaign the same one
        m.set_campaign(b"challenge_0");
        m.init(b"Campaign USA\n FirstMission = X\nEnd\n").unwrap();
        assert_eq!(m.get_current_campaign().unwrap().name, b"challenge_0");
        assert_eq!(m.campaign_list.last().unwrap().name, b"usa");
    }
}
