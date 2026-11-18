//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/StagingRoomGameInfo.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/StagingRoomGameInfo.cpp
//!
//! The live GameSpy staging room: a `GameInfo` whose eight slots are `GameSpyGameSlot`s (login, profile id, locale,
//! ping, wins/losses, rank points, favourite side). `game_spy_game_info.rs` drafted the obsolete class of
//! `GameSpyGameInfo.h`; this is the one the game links (`TheGameSpyGame`).
//!
//! # Not translated
//!
//! * `GetLocalChatConnectionAddress` (StagingRoomGameInfo.cpp, Windows SNMP walk of the TCP table to find the local
//!   address that talks to the chat server): Windows-only and the service is gone. Also skipped in
//!   `game_spy_game_info.rs`.
//! * `launchGame`'s tail is a sequence of calls into `TheNetwork`, `TheGameLogic`, `TheMapCache`, the buddy queue
//!   and the message box: they are the [`StagingRoomHost`]; the order and the early bail-out are kept.

use crate::common::ascii_string::AsciiString;
use crate::common::game_info::{GameInfo, GameInfoHost};
use crate::common::unicode_string::UnicodeString;

use super::net_command_msg::MAX_SLOTS;

const SLOTS: usize = MAX_SLOTS as usize;

/// `class GameSpyGameSlot` (StagingRoomGameInfo.h): the GameSpy extras of one slot.
#[derive(Clone, Debug, Default)]
pub struct GameSpyStagingSlot {
    /// `m_profileID`.
    pub profile_id: i32,
    /// `m_gameSpyLogin`.
    pub login_name: AsciiString,
    /// `m_gameSpyLocale`.
    pub locale: AsciiString,
    /// `m_pingStr`.
    pub ping_str: AsciiString,
    /// `m_pingInt`.
    pub ping_int: i32,
    /// `m_wins`.
    pub wins: i32,
    /// `m_losses`.
    pub losses: i32,
    /// `m_rankPoints`.
    pub rank_points: i32,
    /// `m_favoriteSide`.
    pub favorite_side: i32,
}

impl GameSpyStagingSlot {
    /// `GameSpyGameSlot::GameSpyGameSlot()` (StagingRoomGameInfo.cpp:53): everything cleared or 0.
    pub fn new() -> GameSpyStagingSlot {
        GameSpyStagingSlot::default()
    }
    /// `GameSpyGameSlot::setPingString` (StagingRoomGameInfo.cpp): remembers the string and its value from
    /// `TheGameSpyInfo->getPingValue(pingStr)`.
    pub fn set_ping_string(&mut self, ping_str: AsciiString, ping_value: i32) {
        self.ping_str = ping_str;
        self.ping_int = ping_value;
    }
}

/// A `PlayerInfo` of `TheGameSpyInfo->getPlayerInfoMap()`, as `startGame` reads it.
#[derive(Clone, Debug, Default)]
pub struct StagingPlayerInfo {
    /// `m_profileID`.
    pub profile_id: i32,
    /// `m_locale`.
    pub locale: AsciiString,
    /// `m_rankPoints`.
    pub rank_points: i32,
    /// `m_side`.
    pub side: i32,
}

/// What `generateGameSpyGameResultsPacket` / `generateLadderGameResultsPacket` read per slot that has a
/// `player<i>` in `ThePlayerList` (`findPlayerWithNameKey(NAMEKEY("player%d"))`). A slot with no player is
/// represented by `None` in the arrays below.
#[derive(Clone, Debug, Default)]
pub struct StagingPlayerResults {
    /// `TheVictoryConditions->hasAchievedVictory(p)`.
    pub has_achieved_victory: bool,
    /// `p->getPlayerTemplate()->getSide()`.
    pub side: AsciiString,
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
    /// `TheGameSpyPSMessageQueue->findPlayerStatsByID(profileID).locale` (ladder packet only).
    pub stats_locale: i32,
}

/// The game-wide inputs of the results packets.
#[derive(Clone, Debug, Default)]
pub struct StagingResultsInput {
    /// `TheVictoryConditions->getEndFrame()`.
    pub end_frame: i32,
    /// `TheNetwork->sawCRCMismatch()`.
    pub saw_crc_mismatch: bool,
    /// `TheNetwork->getAverageFPS()`.
    pub average_fps: i32,
    /// `TheGameState->realMapPathToPortableMapPath(getMap())` (ladder packet only).
    pub portable_map_path: AsciiString,
    /// One per slot.
    pub players: [Option<StagingPlayerResults>; SLOTS],
}

/// What `startGame` / `launchGame` reach outside the room.
pub trait StagingRoomHost {
    /// `TheGameSpyInfo->getLocalName()`.
    fn local_name(&self) -> AsciiString;
    /// `TheGameSpyInfo->getInternalIP()`.
    fn internal_ip(&self) -> u32;
    /// `TheGameSpyInfo->getLocalProfileID()`.
    fn local_profile_id(&self) -> i32;
    /// `TheGameSpyInfo->getPlayerInfoMap()->find(name)`.
    fn find_player_info(&self, name: &AsciiString) -> Option<StagingPlayerInfo>;
    /// `TheGameSpyInfo->getPingValue(pingStr)`.
    fn ping_value(&self, ping_str: &AsciiString) -> i32;
    /// `TheGameSpyInfo->didPlayerPreorder(profileID)`.
    fn did_player_preorder(&self, profile_id: i32) -> bool;
    /// `TheGameSpyInfo->leaveStagingRoom()`; false if `TheGameSpyInfo` is NULL.
    fn leave_staging_room(&mut self) -> bool;
    /// `TheNAT != NULL`.
    fn nat_exists(&self) -> bool;
    /// `delete TheNAT; TheNAT = NULL`.
    fn nat_delete(&mut self);
    /// `TheNAT = NEW NAT(); attachSlotList(m_slot, localSlot, localIP); establishConnectionPaths()`.
    fn nat_create_and_establish(&mut self, local_slot: i32, local_ip: u32);
    /// `TheNAT->getSlotPort(slot)`, or None if `TheNAT` is NULL.
    fn nat_slot_port(&self, slot: i32) -> Option<u16>;
    /// `TheNetwork != NULL`.
    fn network_exists(&self) -> bool;
    /// `delete TheNetwork; TheNetwork = NULL`.
    fn network_delete(&mut self);
    /// `TheNetwork = NetworkInterface::createNetwork(); TheNetwork->init()`.
    fn network_create(&mut self);
    /// `TheNetwork->setLocalAddress(ip, port)`.
    fn network_set_local_address(&mut self, ip: u32, port: u16);
    /// `TheNetwork->attachTransport(TheNAT->getTransport())` if `TheNAT`, else `TheNetwork->initTransport()`.
    fn network_attach_transport(&mut self, use_nat_transport: bool);
    /// `TheNetwork->parseUserList(this)`.
    fn network_parse_user_list(&mut self);
    /// `TheGameLogic->isInGame()`.
    fn game_logic_is_in_game(&self) -> bool;
    /// `TheGameLogic->clearGameData()`.
    fn game_logic_clear_game_data(&mut self);
    /// `DoAnyMapTransfers(this)`.
    fn do_any_map_transfers(&mut self) -> bool;
    /// `TheMapCache->updateCache(); TheMapCache->findMap(map) == NULL`.
    fn map_cache_lacks_map(&mut self, map: &[u8]) -> bool;
    /// `GSMessageBoxOk(fetch("GUI:Error"), fetch("GUI:CouldNotTransferMap")); PopBackToLobby()`.
    fn message_box_and_pop_back_to_lobby(&mut self);
    /// `TheWritableGlobalData->m_pendingFile = map`.
    fn set_pending_file(&mut self, map: &[u8]);
    /// `TheMessageStream->appendMessage(MSG_NEW_GAME)->appendIntegerArgument(GAME_INTERNET)`.
    fn append_new_game_message_internet(&mut self);
    /// `TheWritableGlobalData->m_useFpsLimit = false`.
    fn set_use_fps_limit_false(&mut self);
    /// `InitGameLogicRandom(seed)`.
    fn init_game_logic_random(&mut self, seed: i32);
    /// The buddy `BUDDYREQUEST_SETSTATUS` with `GP_PLAYING`, status string "Loading" and the game name as the
    /// location string (`WideCharStringToMultiByte(getGameName())`), added to `TheGameSpyBuddyMessageQueue`.
    fn buddy_set_status_loading(&mut self, location: &[u8]);
}

/// `class GameSpyStagingRoom`.
#[derive(Clone, Debug)]
pub struct GameSpyStagingRoom {
    /// The `GameInfo` base.
    pub info: GameInfo,
    /// `m_GameSpySlot`.
    pub gs_slots: [GameSpyStagingSlot; SLOTS],
    /// `m_gameName`.
    game_name: UnicodeString,
    /// `m_id`.
    id: i32,
    /// `m_localName`.
    local_name: AsciiString,
    /// `m_requiresPassword`.
    requires_password: bool,
    /// `m_allowObservers`.
    allow_observers: bool,
    /// `m_version`.
    version: u32,
    /// `m_exeCRC`.
    exe_crc: u32,
    /// `m_iniCRC`.
    ini_crc: u32,
    /// `m_isQM`.
    is_qm: bool,
    /// `m_ladderIP`.
    ladder_ip: AsciiString,
    /// `m_pingStr`.
    ping_str: AsciiString,
    /// `m_pingInt`.
    ping_int: i32,
    /// `m_ladderPort`.
    ladder_port: u16,
    /// `m_reportedNumPlayers`.
    reported_num_players: i32,
    /// `m_reportedMaxPlayers`.
    reported_max_players: i32,
    /// `m_reportedNumObservers`.
    reported_num_observers: i32,
}

impl GameSpyStagingRoom {
    /// `GameSpyStagingRoom::GameSpyStagingRoom()` (StagingRoomGameInfo.cpp): slot pointers set, local IP 0, no
    /// transport, local name "localhost", empty ladder IP, ladder port 0. The members the constructor does not
    /// initialise (id, flags, CRCs, reported counts) are 0 here.
    pub fn new<H: GameInfoHost>(host: &mut H) -> GameSpyStagingRoom {
        let mut info = GameInfo::new(host);
        info.local_ip = 0;
        GameSpyStagingRoom {
            info,
            gs_slots: Default::default(),
            game_name: UnicodeString::new(),
            id: 0,
            local_name: AsciiString::try_from_bytes(b"localhost").unwrap_or_default(),
            requires_password: false,
            allow_observers: false,
            version: 0,
            exe_crc: 0,
            ini_crc: 0,
            is_qm: false,
            ladder_ip: AsciiString::default(),
            ping_str: AsciiString::default(),
            ping_int: 0,
            ladder_port: 0,
            reported_num_players: 0,
            reported_max_players: 0,
            reported_num_observers: 0,
        }
    }

    /// `cleanUpSlotPointers` (StagingRoomGameInfo.cpp): points each `GameInfo` slot at `m_GameSpySlot[i]`. The slots
    /// are parallel arrays here, so there is no pointer to fix.
    pub fn clean_up_slot_pointers(&mut self) {}

    /// `setID`.
    pub fn set_id(&mut self, id: i32) {
        self.id = id;
    }
    /// `getID`.
    pub fn get_id(&self) -> i32 {
        self.id
    }
    /// `setHasPassword`.
    pub fn set_has_password(&mut self, v: bool) {
        self.requires_password = v;
    }
    /// `getHasPassword`.
    pub fn get_has_password(&self) -> bool {
        self.requires_password
    }
    /// `setAllowObservers`.
    pub fn set_allow_observers(&mut self, v: bool) {
        self.allow_observers = v;
    }
    /// `getAllowObservers`.
    pub fn get_allow_observers(&self) -> bool {
        self.allow_observers
    }
    /// `setVersion`.
    pub fn set_version(&mut self, v: u32) {
        self.version = v;
    }
    /// `getVersion`.
    pub fn get_version(&self) -> u32 {
        self.version
    }
    /// `setExeCRC`.
    pub fn set_exe_crc(&mut self, v: u32) {
        self.exe_crc = v;
    }
    /// `getExeCRC`.
    pub fn get_exe_crc(&self) -> u32 {
        self.exe_crc
    }
    /// `setIniCRC`.
    pub fn set_ini_crc(&mut self, v: u32) {
        self.ini_crc = v;
    }
    /// `getIniCRC`.
    pub fn get_ini_crc(&self) -> u32 {
        self.ini_crc
    }
    /// `setReportedNumPlayers`.
    pub fn set_reported_num_players(&mut self, v: i32) {
        self.reported_num_players = v;
    }
    /// `getReportedNumPlayers`.
    pub fn get_reported_num_players(&self) -> i32 {
        self.reported_num_players
    }
    /// `setReportedMaxPlayers`.
    pub fn set_reported_max_players(&mut self, v: i32) {
        self.reported_max_players = v;
    }
    /// `getReportedMaxPlayers`.
    pub fn get_reported_max_players(&self) -> i32 {
        self.reported_max_players
    }
    /// `setReportedNumObservers`.
    pub fn set_reported_num_observers(&mut self, v: i32) {
        self.reported_num_observers = v;
    }
    /// `getReportedNumObservers`.
    pub fn get_reported_num_observers(&self) -> i32 {
        self.reported_num_observers
    }
    /// `setLadderIP`.
    pub fn set_ladder_ip(&mut self, v: AsciiString) {
        self.ladder_ip = v;
    }
    /// `getLadderIP`.
    pub fn get_ladder_ip(&self) -> AsciiString {
        self.ladder_ip.clone()
    }
    /// `setLadderPort`.
    pub fn set_ladder_port(&mut self, v: u16) {
        self.ladder_port = v;
    }
    /// `getLadderPort`.
    pub fn get_ladder_port(&self) -> u16 {
        self.ladder_port
    }
    /// `GameSpyStagingRoom::setPingString` (StagingRoomGameInfo.cpp).
    pub fn set_ping_string(&mut self, ping_str: AsciiString, ping_value: i32) {
        self.ping_str = ping_str;
        self.ping_int = ping_value;
    }
    /// `getPingString`.
    pub fn get_ping_string(&self) -> AsciiString {
        self.ping_str.clone()
    }
    /// `getPingAsInt`.
    pub fn get_ping_as_int(&self) -> i32 {
        self.ping_int
    }
    /// `markGameAsQM`.
    pub fn mark_game_as_qm(&mut self) {
        self.is_qm = true;
    }
    /// `isQMGame`.
    pub fn is_qm_game(&self) -> bool {
        self.is_qm
    }
    /// `setGameName`.
    pub fn set_game_name(&mut self, name: UnicodeString) {
        self.game_name = name;
    }
    /// `getGameName`.
    pub fn get_game_name(&self) -> UnicodeString {
        self.game_name.clone()
    }
    /// `setLocalName`.
    pub fn set_local_name(&mut self, name: AsciiString) {
        self.local_name = name;
    }

    /// `getGameSpySlot` (StagingRoomGameInfo.cpp): the GameSpy extras of slot `index`; None if out of range
    /// (upstream's `getSlot` returns NULL there and the cast then yields NULL).
    pub fn get_game_spy_slot(&mut self, index: i32) -> Option<&mut GameSpyStagingSlot> {
        if index < 0 || index >= MAX_SLOTS {
            return None;
        }
        Some(&mut self.gs_slots[index as usize])
    }

    /// `GameSpyStagingRoom::init` (StagingRoomGameInfo.cpp): `GameInfo::init()` (`reset()` here).
    pub fn init<H: GameInfoHost>(&mut self, host: &mut H) {
        self.info.reset(host);
    }

    /// `GameSpyStagingRoom::amIHost` (StagingRoomGameInfo.cpp): false when not in a game, else whether slot 0's
    /// player is named `m_localName`.
    pub fn am_i_host(&self) -> bool {
        if !self.info.in_game {
            return false;
        }
        self.info.slots[0].is_player_named_ascii(self.local_name.str())
    }

    /// `GameSpyStagingRoom::resetAccepted` (StagingRoomGameInfo.cpp): `GameInfo::resetAccepted()`; the body that
    /// follows `if (amIHost())` is commented out upstream.
    pub fn reset_accepted(&mut self) {
        self.info.reset_accepted();
        if self.am_i_host() {
            // upstream: peerStateChanged(...) / m_hasBeenQueried = false — commented out
        }
    }

    /// `GameSpyStagingRoom::getLocalSlotNum` (StagingRoomGameInfo.cpp): -1 when not in a game; else the first slot
    /// whose player is named `TheGameSpyInfo->getLocalName()`; -1 if none.
    pub fn get_local_slot_num<H: StagingRoomHost>(&self, host: &H) -> i32 {
        if !self.info.in_game {
            return -1;
        }
        let local_name = host.local_name();
        for i in 0..SLOTS {
            if self.info.slots[i].is_player_named_ascii(local_name.str()) {
                return i as i32;
            }
        }
        -1
    }

    /// `GameSpyStagingRoom::startGame` (StagingRoomGameInfo.cpp): sets the local IP from the internal IP, drops any
    /// `TheNAT`, fills each human slot's login (and, from the player info map, profile id, locale, rank points and
    /// favourite side; for a quick match only the local slot's profile id), then with fewer than two humans
    /// launches at once and leaves the staging room, else creates the NAT over the slot list and starts the
    /// connection rounds. `DEBUG_ASSERTCRASH`es are dropped.
    pub fn start_game<H: StagingRoomHost>(&mut self, host: &mut H, _game_id: i32) {
        let local_ip = host.internal_ip();
        self.info.local_ip = local_ip;

        if host.nat_exists() {
            host.nat_delete();
        }

        let mut num_humans = 0;
        for i in 0..SLOTS {
            if self.info.slots[i].is_human() {
                num_humans += 1;
                let mut gs_name = AsciiString::default();
                let _ = gs_name.translate(&self.info.slots[i].name);
                self.gs_slots[i].login_name = gs_name.clone();

                if self.is_qm {
                    if self.get_local_slot_num(host) == i as i32 {
                        self.gs_slots[i].profile_id = host.local_profile_id();
                    }
                } else if let Some(pi) = host.find_player_info(&gs_name) {
                    self.gs_slots[i].profile_id = pi.profile_id;
                    self.gs_slots[i].locale = pi.locale;
                    self.gs_slots[i].rank_points = pi.rank_points;
                    self.gs_slots[i].favorite_side = pi.side;
                }
                // else DEBUG_CRASH("No player info for %s")
            }
        }

        if num_humans < 2 {
            self.launch_game(host);
            host.leave_staging_room();
        } else {
            let local_slot = self.get_local_slot_num(host);
            host.nat_create_and_establish(local_slot, self.info.local_ip);
        }
    }

    /// `GameSpyStagingRoom::generateGameSpyGameResultsPacket` (StagingRoomGameInfo.cpp): the backslash-delimited
    /// "\seed\..\hostname\..." line for the GameSpy stats server. Counts a slot with a `player<i>` as human, an
    /// AI slot without one as AI; `winningTeam` and the `numTeamsAtGameEnd` scan are computed (and unused in the
    /// text) as upstream does.
    pub fn generate_game_spy_game_results_packet<H: StagingRoomHost>(
        &self,
        host: &H,
        input: &StagingResultsInput,
    ) -> AsciiString {
        let end_frame = input.end_frame;
        let local_slot_num = self.get_local_slot_num(host);
        let mut _winning_team: i32 = -1;
        let mut num_humans = 0;
        let mut num_ais = 0;
        let mut num_teams_at_game_end = 0;
        let mut last_team_at_game_end: i32 = -1;
        for i in 0..SLOTS {
            if let Some(p) = &input.players[i] {
                num_humans += 1;
                if p.has_achieved_victory {
                    _winning_team = self.info.slots[i].team_number;
                }
                let slot = &self.info.slots[i];
                if !slot.disconnected {
                    if slot.team_number != last_team_at_game_end || num_teams_at_game_end == 0 {
                        last_team_at_game_end = slot.team_number;
                        num_teams_at_game_end += 1;
                    }
                }
            } else if self.info.slots[i].is_ai() {
                num_ais += 1;
            }
        }
        let num_players = num_humans + num_ais;

        let mut map_name: Vec<u8> = Vec::new();
        for &c in self.info.map_name.iter() {
            map_name.push(if c == b'\\' { b'/' } else { c });
        }
        let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();

        let mut results = format!(
            "\\seed\\{}\\hostname\\{}\\mapname\\{}\\numplayers\\{}\\duration\\{}\\gamemode\\exiting\\localplayer\\{}",
            self.info.seed,
            lossy(self.gs_slots[0].login_name.str()),
            lossy(&map_name),
            num_players,
            end_frame,
            local_slot_num
        );

        let mut player_id = 0;
        for i in 0..SLOTS {
            if let Some(p) = &input.players[i] {
                let slot = &self.gs_slots[i];
                let player_name = if self.info.slots[i].is_human() {
                    lossy(slot.login_name.str())
                } else {
                    "AIPlayer".to_string()
                };
                let gs_player_id = slot.profile_id;
                let disconnected = self.info.slots[i].disconnected;
                let mut result = "loss";
                if disconnected {
                    result = "discon";
                } else if input.saw_crc_mismatch {
                    result = "desync";
                } else if p.has_achieved_victory {
                    result = "win";
                }
                let mut side = lossy(p.side.str());
                if side == "America" {
                    side = "USA".to_string(); // conform to GameSpy
                }
                results.push_str(&format!(
                    "\\player_{}\\{}\\pid_{}\\{}\\team_{}\\{}\\result_{}\\{}\\side_{}\\{}",
                    player_id,
                    player_name,
                    player_id,
                    gs_player_id,
                    player_id,
                    self.info.slots[i].team_number,
                    player_id,
                    result,
                    player_id,
                    side
                ));
                player_id += 1;
            }
        }
        AsciiString::try_from_bytes(results.as_bytes()).unwrap_or_default()
    }

    /// `GameSpyStagingRoom::generateLadderGameResultsPacket` (StagingRoomGameInfo.cpp): the "key=value," line sent
    /// to the ladder server. As upstream, `ladderIP=` follows `map=...` with no comma between them, and the packet
    /// ends in `,size=` and ten zero-padded digits holding `getLength()+10`, taken after `,size=` was appended.
    pub fn generate_ladder_game_results_packet<H: StagingRoomHost>(
        &self,
        host: &H,
        input: &StagingResultsInput,
    ) -> AsciiString {
        let end_frame = input.end_frame;
        let local_slot_num = self.get_local_slot_num(host);
        let saw_game_end = end_frame > 0;
        let mut winning_team: i32 = -1;
        let mut num_players = 0;
        let mut num_teams_at_game_end = 0;
        let mut last_team_at_game_end: i32 = -1;
        for i in 0..SLOTS {
            if let Some(p) = &input.players[i] {
                num_players += 1;
                if p.has_achieved_victory {
                    winning_team = self.info.slots[i].team_number;
                }
                let slot = &self.info.slots[i];
                if !slot.disconnected {
                    if slot.team_number != last_team_at_game_end || num_teams_at_game_end == 0 {
                        last_team_at_game_end = slot.team_number;
                        num_teams_at_game_end += 1;
                    }
                }
            }
        }
        let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        let mut results = format!(
            "seed={},slotNum={},sawDesync={},sawGameEnd={},winningTeam={},disconEnd={},duration={},numPlayers={},isQM={},map={}",
            self.info.seed,
            local_slot_num,
            input.saw_crc_mismatch as i32,
            saw_game_end as i32,
            winning_team,
            (num_teams_at_game_end < 2) as i32,
            end_frame,
            num_players,
            self.is_qm as i32,
            lossy(input.portable_map_path.str())
        );
        results.push_str(&format!(
            "ladderIP={},ladderPort={}",
            lossy(self.ladder_ip.str()),
            self.ladder_port
        ));

        let mut player_id = 0;
        for i in 0..SLOTS {
            if let Some(p) = &input.players[i] {
                let slot = &self.gs_slots[i];
                let player_name = lossy(slot.login_name.str());
                let gs_player_id = slot.profile_id;
                let disconnected = self.info.slots[i].disconnected;
                results.push_str(&format!(
                    ",player{}={},playerID{}={},locale{}={}",
                    player_id, player_name, player_id, gs_player_id, player_id, p.stats_locale
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
                    ",fps{}={},cash{}={},capturedTech{}={},discon{}={},side{}={}",
                    player_id,
                    input.average_fps,
                    player_id,
                    p.earnings,
                    player_id,
                    p.tech_captured,
                    player_id,
                    disconnected as i32,
                    player_id,
                    lossy(p.side.str())
                ));
                player_id += 1;
            }
        }

        // Add a trailing size value (so the server can ensure it got the entire packet)
        results.push_str(",size=");
        let results_len = results.len() as i32 + 10;
        results.push_str(&format!("{:010}", results_len));
        AsciiString::try_from_bytes(results.as_bytes()).unwrap_or_default()
    }

    /// `GameSpyStagingRoom::launchGame` (StagingRoomGameInfo.cpp): marks the game in progress, marks preordering
    /// human slots, replaces `TheNetwork` with a fresh one bound to the local address (the NAT's slot port, or
    /// 8888 without a NAT) and the NAT's transport or a new one, parses the user list, clears any running game,
    /// does the map transfers and bails back to the lobby if the map is still missing; otherwise queues
    /// `MSG_NEW_GAME`, seeds the logic random, sets the buddy status to "Loading" and drops `TheNAT`.
    pub fn launch_game<H: StagingRoomHost>(&mut self, host: &mut H) {
        self.info.in_progress = true;

        for i in 0..SLOTS {
            if self.info.slots[i].is_human() {
                if host.did_player_preorder(self.gs_slots[i].profile_id) {
                    self.info.mark_player_as_preorder(i as i32);
                }
            }
        }

        if host.network_exists() {
            host.network_delete();
        }
        host.network_create();

        let local_slot = self.get_local_slot_num(host);
        let port = host.nat_slot_port(local_slot).unwrap_or(8888);
        host.network_set_local_address(self.info.local_ip, port);
        let use_nat = host.nat_exists();
        host.network_attach_transport(use_nat);
        host.network_parse_user_list();

        if host.game_logic_is_in_game() {
            host.game_logic_clear_game_data();
        }

        let files_ok = host.do_any_map_transfers();

        if !files_ok || host.map_cache_lacks_map(&self.info.map_name) {
            if host.network_exists() {
                host.network_delete();
            }
            host.message_box_and_pop_back_to_lobby();
            return;
        }

        host.set_pending_file(&self.info.map_name);
        host.append_new_game_message_internet();
        host.set_use_fps_limit_false();
        host.init_game_logic_random(self.info.seed);

        let mut location: Vec<u8> = Vec::new();
        for &c in self.game_name.str().iter() {
            location.push(c as u8);
        }
        host.buddy_set_status_loading(&location);

        if host.nat_exists() {
            host.nat_delete();
        }
    }

    /// `GameSpyStagingRoom::reset` (StagingRoomGameInfo.cpp): `GameInfo::reset()` (the rest is debug logging).
    pub fn reset<H: GameInfoHost>(&mut self, host: &mut H) {
        self.info.reset(host);
    }
}
