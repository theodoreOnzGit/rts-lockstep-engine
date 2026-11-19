//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//! Checked for differential testing (#215) and left untested: upstream `GameSpyGameInfo.h` is `#error this file is obsolete`, so the C++ class
//! cannot be built as shipped, and its results packet is the older sibling of `GameSpyStagingRoom::generate*ResultsPacket`, which are tested
//! (`staging_room_game_info.rs`). The same `disconnected()` fix (`isHuman() && m_disconnected`, found there) was applied here by reading, not by a test.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpyGameInfo.h  (marked `#error this file is obsolete`
//!     upstream, but `GameSpyGameInfo.cpp` still compiles the class from the declarations it carries)
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpyGameInfo.cpp
//!
//! GameSpy staging-room game: a `GameInfo` whose slots also carry the GameSpy profile id, login name and locale,
//! and the end-of-game results packet reported to the (defunct) GameSpy stats service.
//!
//! # Not translated
//!
//! * `GetLocalChatConnectionAddress` (finds which local address talks to `peerchat.gamespy.com` by walking the
//!   Windows SNMP TCP table): the service is gone and the call is Windows-only. [`GameSpyGameInfo::new`] takes the
//!   local address as an argument (0 when unknown, which is what upstream sets when the lookup fails).
//! * `GameSpyStartGame`/`GameSpyLaunchGame`: shell and `TheGameLogic` start-up glue around `TheNAT`, the
//!   staging-room GUI and `TheGameSpyInfo`; `WOLDisplayGameOptions`/`WOLDisplaySlotList` are GUI drawing.
//! * `SBServer m_server` (the GameSpy server browser handle) and `Transport *m_transport`: opaque to this file.

use crate::common::ascii_string::AsciiString;
use crate::common::game_info::{GameInfo, GameInfoHost};

use super::net_command_msg::MAX_SLOTS;

const SLOTS: usize = MAX_SLOTS as usize;

/// What `GameSpyGameSlot` adds to a `GameSlot`.
#[derive(Clone, Debug, Default)]
pub struct GameSpyGameSlot {
    /// `m_profileID`.
    profile_id: i32,
    /// `m_gameSpyLogin`.
    login_name: AsciiString,
    /// `m_gameSpyLocale`.
    locale: AsciiString,
}

impl GameSpyGameSlot {
    /// `GameSpyGameSlot::GameSpyGameSlot()` (GameSpyGameInfo.cpp:53): empty login and locale, profile id 0.
    pub fn new() -> GameSpyGameSlot {
        GameSpyGameSlot::default()
    }
    /// `getProfileID`.
    pub fn get_profile_id(&self) -> i32 {
        self.profile_id
    }
    /// `setProfileID`.
    pub fn set_profile_id(&mut self, id: i32) {
        self.profile_id = id;
    }
    /// `getLoginName`.
    pub fn get_login_name(&self) -> AsciiString {
        self.login_name.clone()
    }
    /// `setLoginName`.
    pub fn set_login_name(&mut self, name: AsciiString) {
        self.login_name = name;
    }
    /// `getLocale`.
    pub fn get_locale(&self) -> AsciiString {
        self.locale.clone()
    }
    /// `setLocale`.
    pub fn set_locale(&mut self, name: AsciiString) {
        self.locale = name;
    }
}

/// A `PlayerInfo` of `TheGameSpyInfo->getPlayerInfoMap()`, as `startGame` reads it.
#[derive(Clone, Debug)]
pub struct GameSpyPlayerInfo {
    /// `m_profileID`.
    pub profile_id: i32,
    /// `m_locale`.
    pub locale: AsciiString,
}

/// What a player's results line needs from `ThePlayerList`/`ScoreKeeper`/`TheNetwork` (one per existing
/// `player<i>`, in slot order).
#[derive(Clone, Debug)]
pub struct PlayerResults {
    /// The slot index `i` of `player<i>`.
    pub slot: usize,
    /// `TheVictoryConditions->hasAchievedVictory(p)`.
    pub has_achieved_victory: bool,
    /// `keeper->getTotalUnitsDestroyed()`.
    pub units_killed: i32,
    /// `getTotalUnitsLost()`.
    pub units_lost: i32,
    /// `getTotalUnitsBuilt()`.
    pub units_built: i32,
    /// `getTotalBuildingsDestroyed()`.
    pub buildings_killed: i32,
    /// `getTotalBuildingsLost()`.
    pub buildings_lost: i32,
    /// `getTotalBuildingsBuilt()`.
    pub buildings_built: i32,
    /// `getTotalMoneyEarned()`.
    pub earnings: i32,
    /// `getTotalTechBuildingsCaptured()`.
    pub tech_captured: i32,
}

/// The game-wide inputs of the results packet.
#[derive(Clone, Debug)]
pub struct GameResultsInput {
    /// `TheVictoryConditions->getEndFrame()`.
    pub end_frame: i32,
    /// `TheNetwork->sawCRCMismatch()`.
    pub saw_crc_mismatch: bool,
    /// `TheNetwork->getAverageFPS()`.
    pub average_fps: i32,
    /// The existing players.
    pub players: Vec<PlayerResults>,
}

/// `class GameSpyGameInfo`.
#[derive(Clone, Debug)]
pub struct GameSpyGameInfo {
    /// The `GameInfo` base.
    pub info: GameInfo,
    /// `m_GameSpySlot`, one per slot.
    pub slots: [GameSpyGameSlot; SLOTS],
    /// `m_hasBeenQueried`.
    has_been_queried: bool,
    /// `m_isQM`: a quick match.
    is_qm: bool,
}

impl GameSpyGameInfo {
    /// `GameSpyGameInfo::GameSpyGameInfo()` (GameSpyGameInfo.cpp:435): not a quick match, not queried;
    /// `setLocalIP` of the chat connection's address (0 if none).
    pub fn new<H: GameInfoHost>(host: &mut H, local_ip: u32) -> GameSpyGameInfo {
        let mut info = GameInfo::new(host);
        info.local_ip = local_ip;
        GameSpyGameInfo {
            info,
            slots: Default::default(),
            has_been_queried: false,
            is_qm: false,
        }
    }

    /// `markGameAsQM`.
    pub fn mark_game_as_qm(&mut self) {
        self.is_qm = true;
    }

    /// `GameSpyGameInfo::init` (GameSpyGameInfo.cpp:579): `GameInfo::init()` (which is `reset()` here), then
    /// forgets any query.
    pub fn init<H: GameInfoHost>(&mut self, host: &mut H) {
        self.info.reset(host);
        self.has_been_queried = false;
    }

    /// `GameSpyGameInfo::resetAccepted` (GameSpyGameInfo.cpp:586): `GameInfo::resetAccepted()`; the host of a queried
    /// game becomes unqueried.
    pub fn reset_accepted(&mut self) {
        self.info.reset_accepted();
        if self.has_been_queried && self.info.am_i_host() {
            self.has_been_queried = false;
        }
    }

    /// `GameSpyGameInfo::getLocalSlotNum` (GameSpyGameInfo.cpp:598): the slot whose player is named
    /// `TheGameSpyInfo->getLocalName()`; -1 if none or not in a game.
    pub fn get_local_slot_num(&self, local_name: &[u8]) -> i32 {
        if !self.info.in_game {
            return -1;
        }
        let wide: Vec<u16> = local_name.iter().map(|&c| c as u16).collect();
        for i in 0..SLOTS {
            if self.info.slots[i].is_player_named(&wide) {
                return i as i32;
            }
        }
        -1
    }

    /// `GameSpyGameInfo::gotGOACall` (GameSpyGameInfo.cpp:618): the game was queried.
    pub fn got_goa_call(&mut self) {
        self.has_been_queried = true;
    }

    /// `GameSpyGameInfo::startGame` (GameSpyGameInfo.cpp:624), the part that is this class's: gives every human slot
    /// its login name (the ASCII of its name) and, from the player info map, its profile id and locale. (The rest
    /// creates `TheNAT` over the slot list and runs `establishConnectionPaths`.) A name with no entry in the map
    /// keeps its previous profile id and locale.
    pub fn start_game_fill_profiles(&mut self, lookup: impl Fn(&[u8]) -> Option<GameSpyPlayerInfo>) {
        for i in 0..SLOTS {
            if self.info.slots[i].is_human() {
                let gs_name: Vec<u8> = self.info.slots[i].name.iter().map(|&c| c as u8).collect();
                self.slots[i].set_login_name(AsciiString::try_from_bytes(&gs_name).unwrap_or_default());
                if let Some(pi) = lookup(&gs_name) {
                    self.slots[i].set_profile_id(pi.profile_id);
                    self.slots[i].set_locale(pi.locale);
                }
                // else DEBUG_CRASH("No player info for %s")
            }
        }
    }

    /// `GameSpyGameInfo::generateGameResultsPacket` (GameSpyGameInfo.cpp:662): the "key=value," line of the game's
    /// results — seed, my slot, desync, whether the game end was seen, the winning team, whether any team was left
    /// at the end, duration, player count, quick match — then each existing player's login, profile id, locale,
    /// scores, fps, cash, tech captured and disconnect flag, then the total length as ten zero-padded digits.
    pub fn generate_game_results_packet(&self, local_slot_num: i32, input: &GameResultsInput) -> AsciiString {
        let end_frame = input.end_frame;
        let saw_game_end = end_frame > 0;
        let mut winning_team: i32 = -1;
        let mut num_players = 0;
        let mut num_teams_at_game_end = 0;
        let mut last_team_at_game_end: i32 = -1;
        for p in &input.players {
            num_players += 1;
            let slot = &self.info.slots[p.slot];
            if p.has_achieved_victory {
                winning_team = slot.team_number;
            }
            if !slot.is_disconnected() {
                if slot.team_number != last_team_at_game_end || num_teams_at_game_end == 0 {
                    last_team_at_game_end = slot.team_number;
                    num_teams_at_game_end += 1;
                }
            }
        }
        let mut results = format!(
            "seed={},slotNum={},sawDesync={},sawGameEnd={},winningTeam={},disconEnd={},duration={},numPlayers={},isQM={}",
            self.info.seed,
            local_slot_num,
            input.saw_crc_mismatch as i32,
            saw_game_end as i32,
            winning_team,
            (num_teams_at_game_end != 0) as i32,
            end_frame,
            num_players,
            self.is_qm as i32
        );
        let mut player_id = 0;
        for p in &input.players {
            let slot = &self.slots[p.slot];
            let player_name = String::from_utf8_lossy(slot.get_login_name().str()).into_owned();
            let gs_player_id = slot.get_profile_id();
            let locale = String::from_utf8_lossy(slot.get_locale().str()).into_owned();
            let disconnected = self.info.slots[p.slot].is_disconnected();
            results.push_str(&format!(
                ",player{}={},playerID{}={},locale{}={}",
                player_id, player_name, player_id, gs_player_id, player_id, locale
            ));
            results.push_str(&format!(
                ",unitsKilled{}={},unitsLost{}={},unitsBuilt{}={}",
                player_id, p.units_killed, player_id, p.units_lost, player_id, p.units_built
            ));
            results.push_str(&format!(
                ",buildingsKilled{}={},buildingsLost{}={},buildingsBuilt{}={}",
                player_id, p.buildings_killed, player_id, p.buildings_lost, player_id, p.buildings_built
            ));
            results.push_str(&format!(
                ",fps{}={},cash{}={},capturedTech{}={},discon{}={}",
                player_id, input.average_fps, player_id, p.earnings, player_id, p.tech_captured, player_id,
                disconnected as i32
            ));
            player_id += 1;
        }
        let results_len = results.len() as i32 + 10;
        results.push_str(&format!("{:010}", results_len));
        AsciiString::try_from_bytes(results.as_bytes()).unwrap_or_default()
    }
}
