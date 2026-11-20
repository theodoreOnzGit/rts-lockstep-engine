//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANAPI.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPI.cpp
//!   (the incoming-message handlers, `LANAPIhandlers.cpp`, are in `lan_api_handlers`)
//!
//! LAN lobby and game-setup traffic: UDP broadcast of fixed-size [`LANMessage`]s on port 8086.
//!
//! # Shape
//!
//! * `LANMessage` is a packed 471-byte struct whose tail is a `union`; it is kept as its bytes with one typed
//!   accessor per union member, so it goes on the wire byte for byte as upstream's does.
//! * `m_games`/`m_lobbyPlayers` are `Vec`s kept in the order upstream's `addGame`/`addPlayer` produce (first
//!   by name, then, as upstream's loop does, "descending after the head"). `m_currentGame` is an index into
//!   `games` (kept right by `remove_game`).
//! * The `On*` callbacks (`LANAPICallbacks.cpp`), the registry, the preferences file, the map cache, the game
//!   text, `TheGlobalData` and `TheNetwork` are the [`LanApiHost`].
//! * `GetUserName`/`GetComputerName` are the host's [`LanApiHost::user_name`]/[`LanApiHost::computer_name`].
//!
//! # Not translated
//!
//! `LANGame` and `createSlotString`/`RequestSlotList` are inside comments upstream. `checkMOTD` reads the local
//! MOTD file (`m_useLocalMOTD`), which is [`LanApiHost::read_local_motd`].

use crate::common::ascii_string::AsciiString;
use crate::common::crc::Crc;
use crate::common::game_info::{game_info_to_ascii_string, slot_state, GameSlot};
use crate::common::unicode_string::UnicodeString;

use super::lan_game_info::{generate_game_options_string, LANGameInfo, LANPlayer, LANSlotExtra, LanGameInfoHost};
use super::net_command_msg::{MAX_MESSAGES, MAX_PACKET_SIZE, MAX_SLOTS, NETWORK_BASE_PORT_NUMBER};
use super::transport::Transport;
use crate::logic::modules::plant;

const SLOTS: usize = MAX_SLOTS as usize;

/// `lobbyPort`: the UDP port used by all LANAPI communication.
pub const LOBBY_PORT: u16 = 8086;
/// `g_lanPlayerNameLength`.
pub const G_LAN_PLAYER_NAME_LENGTH: usize = 12;
/// `g_lanLoginNameLength`.
pub const G_LAN_LOGIN_NAME_LENGTH: usize = 1;
/// `g_lanHostNameLength`.
pub const G_LAN_HOST_NAME_LENGTH: usize = 1;
/// `g_lanGameNameLength`.
pub const G_LAN_GAME_NAME_LENGTH: usize = 16;
/// `g_lanGameNameReservedLength`.
pub const G_LAN_GAME_NAME_RESERVED_LENGTH: usize = 16;
/// `g_lanMaxChatLength`.
pub const G_LAN_MAX_CHAT_LENGTH: usize = 100;
/// `m_lanMaxOptionsLength`: `MAX_PACKET_SIZE - (8 + (16+1)*2 + 4 + (12+1)*2 + (1+1) + (1+1))` = 400.
pub const M_LAN_MAX_OPTIONS_LENGTH: usize = (MAX_PACKET_SIZE as usize)
    - (8 + (G_LAN_GAME_NAME_LENGTH + 1) * 2 + 4 + (G_LAN_PLAYER_NAME_LENGTH + 1) * 2
        + (G_LAN_LOGIN_NAME_LENGTH + 1)
        + (G_LAN_HOST_NAME_LENGTH + 1));
/// `g_maxSerialLength`: including the trailing 0.
pub const G_MAX_SERIAL_LENGTH: usize = 23;
/// `LANAPI::s_resendDelta`: how often we announce ourselves, ms.
pub const S_RESEND_DELTA: u32 = 10 * 1000;

/// `LANAPIInterface::ChatType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatType {
    /// `LANCHAT_NORMAL`.
    Normal = 0,
    /// `LANCHAT_EMOTE`.
    Emote = 1,
    /// `LANCHAT_SYSTEM`.
    System = 2,
}

/// `LANAPIInterface::ReturnType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnType {
    /// `RET_OK`.
    Ok = 0,
    /// `RET_TIMEOUT`.
    Timeout,
    /// `RET_GAME_FULL`.
    GameFull,
    /// `RET_DUPLICATE_NAME`.
    DuplicateName,
    /// `RET_CRC_MISMATCH`.
    CrcMismatch,
    /// `RET_SERIAL_DUPE`.
    SerialDupe,
    /// `RET_GAME_STARTED`.
    GameStarted,
    /// `RET_GAME_EXISTS`.
    GameExists,
    /// `RET_GAME_GONE`.
    GameGone,
    /// `RET_BUSY`.
    Busy,
    /// `RET_UNKNOWN`.
    Unknown,
}

impl ReturnType {
    /// The enumerator for a wire `Int` (an unknown value is `RET_UNKNOWN`).
    pub fn from_i32(v: i32) -> ReturnType {
        match v {
            0 => ReturnType::Ok,
            1 => ReturnType::Timeout,
            2 => ReturnType::GameFull,
            3 => ReturnType::DuplicateName,
            4 => ReturnType::CrcMismatch,
            5 => ReturnType::SerialDupe,
            6 => ReturnType::GameStarted,
            7 => ReturnType::GameExists,
            8 => ReturnType::GameGone,
            9 => ReturnType::Busy,
            _ => ReturnType::Unknown,
        }
    }
}

impl ChatType {
    /// The enumerator for a wire `Int` (out of range: normal).
    pub fn from_i32(v: i32) -> ChatType {
        match v {
            1 => ChatType::Emote,
            2 => ChatType::System,
            _ => ChatType::Normal,
        }
    }
}

/// `LANAPI::PendingActionType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingActionType {
    /// `ACT_NONE`.
    None,
    /// `ACT_JOIN`.
    Join,
    /// `ACT_JOINDIRECTCONNECT`.
    JoinDirectConnect,
    /// `ACT_LEAVE`.
    Leave,
}

/// `LANMessage::LANMessageType`.
pub mod lan_message_type {
    /// `MSG_REQUEST_LOCATIONS`.
    pub const REQUEST_LOCATIONS: i32 = 0;
    /// `MSG_GAME_ANNOUNCE`.
    pub const GAME_ANNOUNCE: i32 = 1;
    /// `MSG_LOBBY_ANNOUNCE`.
    pub const LOBBY_ANNOUNCE: i32 = 2;
    /// `MSG_REQUEST_JOIN`.
    pub const REQUEST_JOIN: i32 = 3;
    /// `MSG_JOIN_ACCEPT`.
    pub const JOIN_ACCEPT: i32 = 4;
    /// `MSG_JOIN_DENY`.
    pub const JOIN_DENY: i32 = 5;
    /// `MSG_REQUEST_GAME_LEAVE`.
    pub const REQUEST_GAME_LEAVE: i32 = 6;
    /// `MSG_REQUEST_LOBBY_LEAVE`.
    pub const REQUEST_LOBBY_LEAVE: i32 = 7;
    /// `MSG_SET_ACCEPT`.
    pub const SET_ACCEPT: i32 = 8;
    /// `MSG_MAP_AVAILABILITY`.
    pub const MAP_AVAILABILITY: i32 = 9;
    /// `MSG_CHAT`.
    pub const CHAT: i32 = 10;
    /// `MSG_GAME_START`.
    pub const GAME_START: i32 = 11;
    /// `MSG_GAME_START_TIMER`.
    pub const GAME_START_TIMER: i32 = 12;
    /// `MSG_GAME_OPTIONS`.
    pub const GAME_OPTIONS: i32 = 13;
    /// `MSG_INACTIVE`.
    pub const INACTIVE: i32 = 14;
    /// `MSG_REQUEST_GAME_INFO`.
    pub const REQUEST_GAME_INFO: i32 = 15;
    /// `MSG_MAX`.
    pub const MAX: i32 = 16;
}

/// `sizeof(LANMessage)`: type (4) + name (13 wide = 26) + userName (2) + hostName (2) + the 437-byte union.
pub const LAN_MESSAGE_SIZE: usize = 4 + 26 + 2 + 2 + UNION_SIZE;
/// The union's size: `GameInfo` = gameName (34) + inProgress (1) + options (401) + isDirectConnect (1).
const UNION_SIZE: usize = 34 + 1 + (M_LAN_MAX_OPTIONS_LENGTH + 1) + 1;

const OFF_TYPE: usize = 0;
const OFF_NAME: usize = 4;
const OFF_USER: usize = OFF_NAME + 26;
const OFF_HOST: usize = OFF_USER + 2;
const OFF_UNION: usize = OFF_HOST + 2;

/// `struct LANMessage` (`#pragma pack(1)`) as its bytes; the union members are the typed accessors below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LANMessage {
    bytes: Vec<u8>,
}

impl Default for LANMessage {
    fn default() -> Self {
        LANMessage::new()
    }
}

impl LANMessage {
    /// A zeroed message.
    pub fn new() -> LANMessage {
        LANMessage { bytes: vec![0u8; LAN_MESSAGE_SIZE] }
    }

    /// A message from received bytes (a short packet is zero-filled, as the rest of upstream's `m_inBuffer` data
    /// would be stale; a long one is cut).
    pub fn from_bytes(data: &[u8]) -> LANMessage {
        let mut m = LANMessage::new();
        let n = data.len().min(LAN_MESSAGE_SIZE);
        m.bytes[..n].copy_from_slice(&data[..n]);
        m
    }

    /// The bytes sent (`sizeof(LANMessage)`).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn get_u32(&self, off: usize) -> u32 {
        u32::from_le_bytes([self.bytes[off], self.bytes[off + 1], self.bytes[off + 2], self.bytes[off + 3]])
    }
    fn set_u32(&mut self, off: usize, v: u32) {
        self.bytes[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    /// A `WideChar[n]` field read up to its first 0 (`UnicodeString(msg->field)`).
    fn get_wstr(&self, off: usize, n: usize) -> UnicodeString {
        let mut v: Vec<u16> = Vec::new();
        for i in 0..n {
            let c = u16::from_le_bytes([self.bytes[off + 2 * i], self.bytes[off + 2 * i + 1]]);
            if c == 0 {
                break;
            }
            v.push(c);
        }
        UnicodeString::try_from_chars(&v).unwrap_or_default()
    }
    /// `wcsncpy(field, s, max); field[max] = 0`: at most `max` characters, the rest of the `max + 1` zero.
    fn set_wstr(&mut self, off: usize, max: usize, s: &[u16]) {
        for i in 0..=max {
            let c = if i < max { s.get(i).copied().unwrap_or(0) } else { 0 };
            // wcsncpy stops copying at the source's terminator and zero-fills the rest
            let c = if s.iter().take(i).any(|&x| x == 0) { 0 } else { c };
            self.bytes[off + 2 * i..off + 2 * i + 2].copy_from_slice(&c.to_le_bytes());
        }
    }
    /// A `char[n]` field up to its first 0.
    fn get_cstr(&self, off: usize, n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for i in 0..n {
            let c = self.bytes[off + i];
            if c == 0 {
                break;
            }
            v.push(c);
        }
        v
    }
    /// `strncpy(field, s, max)` then 0-terminate at `field[max]`.
    fn set_cstr(&mut self, off: usize, max: usize, s: &[u8]) {
        for i in 0..=max {
            let c = if i < max { s.get(i).copied().unwrap_or(0) } else { 0 };
            let c = if s.iter().take(i).any(|&x| x == 0) { 0 } else { c };
            self.bytes[off + i] = c;
        }
    }

    /// `LANMessageType`.
    pub fn message_type(&self) -> i32 {
        self.get_u32(OFF_TYPE) as i32
    }
    /// `LANMessageType = t`.
    pub fn set_message_type(&mut self, t: i32) {
        self.set_u32(OFF_TYPE, t as u32);
    }
    /// `name`.
    pub fn name(&self) -> UnicodeString {
        self.get_wstr(OFF_NAME, G_LAN_PLAYER_NAME_LENGTH + 1)
    }
    /// `wcsncpy(name, s, g_lanPlayerNameLength); name[g_lanPlayerNameLength] = 0`.
    pub fn set_name(&mut self, s: &[u16]) {
        self.set_wstr(OFF_NAME, G_LAN_PLAYER_NAME_LENGTH, s);
    }
    /// `userName`.
    pub fn user_name(&self) -> Vec<u8> {
        self.get_cstr(OFF_USER, G_LAN_LOGIN_NAME_LENGTH + 1)
    }
    /// `strncpy(userName, s, g_lanLoginNameLength); userName[g_lanLoginNameLength] = 0`.
    pub fn set_user_name(&mut self, s: &[u8]) {
        self.set_cstr(OFF_USER, G_LAN_LOGIN_NAME_LENGTH, s);
    }
    /// `hostName`.
    pub fn host_name(&self) -> Vec<u8> {
        self.get_cstr(OFF_HOST, G_LAN_HOST_NAME_LENGTH + 1)
    }
    /// `strncpy(hostName, s, g_lanHostNameLength); hostName[g_lanHostNameLength] = 0`.
    pub fn set_host_name(&mut self, s: &[u8]) {
        self.set_cstr(OFF_HOST, G_LAN_HOST_NAME_LENGTH, s);
    }

    // ---- union members (offsets within the union) ----

    /// `StartTimer.seconds`.
    pub fn start_timer_seconds(&self) -> i32 {
        self.get_u32(OFF_UNION) as i32
    }
    /// `StartTimer.seconds = v`.
    pub fn set_start_timer_seconds(&mut self, v: i32) {
        self.set_u32(OFF_UNION, v as u32);
    }
    /// The `gameName` that every game-related member has at union offset 0.
    pub fn game_name(&self) -> UnicodeString {
        self.get_wstr(OFF_UNION, G_LAN_GAME_NAME_LENGTH + 1)
    }
    /// `wcsncpy(X.gameName, s, g_lanGameNameLength); X.gameName[g_lanGameNameLength] = 0`.
    pub fn set_game_name(&mut self, s: &[u16]) {
        self.set_wstr(OFF_UNION, G_LAN_GAME_NAME_LENGTH, s);
    }
    /// `GameInfo.inProgress`.
    pub fn game_info_in_progress(&self) -> bool {
        self.bytes[OFF_UNION + 34] != 0
    }
    /// `GameInfo.inProgress = v`.
    pub fn set_game_info_in_progress(&mut self, v: bool) {
        self.bytes[OFF_UNION + 34] = v as u8;
    }
    /// `GameInfo.options`.
    pub fn game_info_options(&self) -> Vec<u8> {
        self.get_cstr(OFF_UNION + 35, M_LAN_MAX_OPTIONS_LENGTH + 1)
    }
    /// `strncpy(GameInfo.options, s, m_lanMaxOptionsLength)` (the last byte stays 0 in a zeroed message).
    pub fn set_game_info_options(&mut self, s: &[u8]) {
        self.set_cstr(OFF_UNION + 35, M_LAN_MAX_OPTIONS_LENGTH, s);
    }
    /// `GameInfo.isDirectConnect`.
    pub fn game_info_is_direct_connect(&self) -> bool {
        self.bytes[OFF_UNION + 436] != 0
    }
    /// `GameInfo.isDirectConnect = v`.
    pub fn set_game_info_is_direct_connect(&mut self, v: bool) {
        self.bytes[OFF_UNION + 436] = v as u8;
    }
    /// `PlayerInfo.ip`.
    pub fn player_info_ip(&self) -> u32 {
        self.get_u32(OFF_UNION)
    }
    /// `PlayerInfo.ip = v`.
    pub fn set_player_info_ip(&mut self, v: u32) {
        self.set_u32(OFF_UNION, v);
    }
    /// `wcsncpy(PlayerInfo.playerName, s, n)` then 0-terminate after the `n` characters.
    pub fn set_player_info_name(&mut self, s: &[u16]) {
        let n = s.len().min(G_LAN_PLAYER_NAME_LENGTH);
        self.set_wstr(OFF_UNION + 4, n, &s[..n]);
    }
    /// `GameToJoin.gameIP`.
    pub fn game_to_join_game_ip(&self) -> u32 {
        self.get_u32(OFF_UNION)
    }
    /// `GameToJoin.gameIP = v`.
    pub fn set_game_to_join_game_ip(&mut self, v: u32) {
        self.set_u32(OFF_UNION, v);
    }
    /// `GameToJoin.exeCRC`.
    pub fn game_to_join_exe_crc(&self) -> u32 {
        self.get_u32(OFF_UNION + 4)
    }
    /// `GameToJoin.exeCRC = v`.
    pub fn set_game_to_join_exe_crc(&mut self, v: u32) {
        self.set_u32(OFF_UNION + 4, v);
    }
    /// `GameToJoin.iniCRC`.
    pub fn game_to_join_ini_crc(&self) -> u32 {
        self.get_u32(OFF_UNION + 8)
    }
    /// `GameToJoin.iniCRC = v`.
    pub fn set_game_to_join_ini_crc(&mut self, v: u32) {
        self.set_u32(OFF_UNION + 8, v);
    }
    /// `GameToJoin.serial` (`g_maxSerialLength` bytes).
    pub fn game_to_join_serial(&self) -> Vec<u8> {
        self.get_cstr(OFF_UNION + 12, G_MAX_SERIAL_LENGTH)
    }
    /// `strncpy(GameToJoin.serial, s, g_maxSerialLength); serial[g_maxSerialLength-1] = 0`.
    pub fn set_game_to_join_serial(&mut self, s: &[u8]) {
        self.set_cstr(OFF_UNION + 12, G_MAX_SERIAL_LENGTH - 1, s);
    }
    /// `GameJoined.gameIP` / `GameNotJoined.gameIP`.
    pub fn joined_game_ip(&self) -> u32 {
        self.get_u32(OFF_UNION + 34)
    }
    /// Sets `GameJoined.gameIP` / `GameNotJoined.gameIP`.
    pub fn set_joined_game_ip(&mut self, v: u32) {
        self.set_u32(OFF_UNION + 34, v);
    }
    /// `GameJoined.playerIP` / `GameNotJoined.playerIP`.
    pub fn joined_player_ip(&self) -> u32 {
        self.get_u32(OFF_UNION + 38)
    }
    /// Sets `GameJoined.playerIP` / `GameNotJoined.playerIP`.
    pub fn set_joined_player_ip(&mut self, v: u32) {
        self.set_u32(OFF_UNION + 38, v);
    }
    /// `GameJoined.slotPosition` / `GameNotJoined.reason`.
    pub fn joined_slot_or_reason(&self) -> i32 {
        self.get_u32(OFF_UNION + 42) as i32
    }
    /// Sets `GameJoined.slotPosition` / `GameNotJoined.reason`.
    pub fn set_joined_slot_or_reason(&mut self, v: i32) {
        self.set_u32(OFF_UNION + 42, v as u32);
    }
    /// `Accept.isAccepted`.
    pub fn accept_is_accepted(&self) -> bool {
        self.bytes[OFF_UNION + 34] != 0
    }
    /// `Accept.isAccepted = v`.
    pub fn set_accept_is_accepted(&mut self, v: bool) {
        self.bytes[OFF_UNION + 34] = v as u8;
    }
    /// `MapStatus.mapCRC`.
    pub fn map_status_map_crc(&self) -> u32 {
        self.get_u32(OFF_UNION + 34)
    }
    /// `MapStatus.mapCRC = v`.
    pub fn set_map_status_map_crc(&mut self, v: u32) {
        self.set_u32(OFF_UNION + 34, v);
    }
    /// `MapStatus.hasMap`.
    pub fn map_status_has_map(&self) -> bool {
        self.bytes[OFF_UNION + 38] != 0
    }
    /// `MapStatus.hasMap = v`.
    pub fn set_map_status_has_map(&mut self, v: bool) {
        self.bytes[OFF_UNION + 38] = v as u8;
    }
    /// `Chat.chatType`.
    pub fn chat_type(&self) -> i32 {
        self.get_u32(OFF_UNION + 34) as i32
    }
    /// `Chat.chatType = v`.
    pub fn set_chat_type(&mut self, v: i32) {
        self.set_u32(OFF_UNION + 34, v as u32);
    }
    /// `Chat.message`.
    pub fn chat_message(&self) -> UnicodeString {
        self.get_wstr(OFF_UNION + 38, G_LAN_MAX_CHAT_LENGTH + 1)
    }
    /// `wcsncpy(Chat.message, s, g_lanMaxChatLength); Chat.message[g_lanMaxChatLength] = 0`.
    pub fn set_chat_message(&mut self, s: &[u16]) {
        self.set_wstr(OFF_UNION + 38, G_LAN_MAX_CHAT_LENGTH, s);
    }
    /// `GameOptions.options`.
    pub fn game_options_options(&self) -> Vec<u8> {
        self.get_cstr(OFF_UNION, M_LAN_MAX_OPTIONS_LENGTH + 1)
    }
    /// `strncpy(GameOptions.options, s, m_lanMaxOptionsLength); options[m_lanMaxOptionsLength] = 0`.
    pub fn set_game_options_options(&mut self, s: &[u8]) {
        self.set_cstr(OFF_UNION, M_LAN_MAX_OPTIONS_LENGTH, s);
    }
}

/// (`%d` of an `UnsignedInt`: every number is printed as a signed `int`.)
/// `GetMessageTypeString` (LANAPI.cpp): a printable name for a message type (the "Map Availability" type has no
/// entry upstream and reads "Unknown Message").
pub fn get_message_type_string(ty: u32) -> String {
    use lan_message_type::*;
    let t = ty as i32;
    match t {
        REQUEST_LOCATIONS => format!("Request Locations ({})", ty as i32),
        GAME_ANNOUNCE => format!("Game Announce ({})", ty as i32),
        LOBBY_ANNOUNCE => format!("Lobby Announce ({})", ty as i32),
        REQUEST_JOIN => format!("Request Join ({})", ty as i32),
        JOIN_ACCEPT => format!("Join Accept ({})", ty as i32),
        JOIN_DENY => format!("Join Deny ({})", ty as i32),
        REQUEST_GAME_LEAVE => format!("Request Game Leave ({})", ty as i32),
        REQUEST_LOBBY_LEAVE => format!("Request Lobby Leave ({})", ty as i32),
        SET_ACCEPT => format!("Set Accept({})", ty as i32),
        CHAT => format!("Chat ({})", ty as i32),
        GAME_START => format!("Game Start ({})", ty as i32),
        GAME_START_TIMER => format!("Game Start Timer ({})", ty as i32),
        GAME_OPTIONS => format!("Game Options ({})", ty as i32),
        REQUEST_GAME_INFO => format!("Request GameInfo ({})", ty as i32),
        INACTIVE => format!("Inactive ({})", ty as i32),
        _ => format!("Unknown Message ({})", ty as i32),
    }
}

/// What the LANAPI reaches outside itself (see the module doc).
pub trait LanApiHost: LanGameInfoHost {
    /// `OnGameList(gameList)`.
    fn on_game_list(&mut self, games: &[LANGameInfo]);
    /// `OnPlayerList(playerList)`.
    fn on_player_list(&mut self, players: &[LANPlayer]);
    /// `OnGameJoin(ret, theGame)`.
    fn on_game_join(&mut self, ret: ReturnType, game: Option<&LANGameInfo>);
    /// `OnPlayerJoin(slot, playerName)`.
    fn on_player_join(&mut self, slot: i32, player_name: &UnicodeString);
    /// `OnHostLeave()`.
    fn on_host_leave(&mut self);
    /// `OnPlayerLeave(player)`.
    fn on_player_leave(&mut self, player: &UnicodeString);
    /// `OnAccept(playerIP, status)`.
    fn on_accept(&mut self, player_ip: u32, status: bool);
    /// `OnHasMap(playerIP, status)`.
    fn on_has_map(&mut self, player_ip: u32, status: bool);
    /// `OnChat(player, ip, message, format)`.
    fn on_chat(&mut self, player: &UnicodeString, ip: u32, message: &UnicodeString, format: ChatType);
    /// `OnGameStart()`.
    fn on_game_start(&mut self);
    /// `OnGameStartTimer(seconds)`.
    fn on_game_start_timer(&mut self, seconds: i32);
    /// `OnGameOptions(playerIP, playerSlot, options)`.
    fn on_game_options(&mut self, player_ip: u32, player_slot: i32, options: &[u8]);
    /// `OnGameCreate(ret)`.
    fn on_game_create(&mut self, ret: ReturnType);
    /// `OnNameChange(IP, newName)`.
    fn on_name_change(&mut self, ip: u32, new_name: &UnicodeString);
    /// `LANbuttonPushed`.
    fn lan_button_pushed(&self) -> bool;
    /// `LANSocketErrorDetected` (read).
    fn lan_socket_error_detected(&self) -> bool;
    /// `LANSocketErrorDetected = TRUE`.
    fn set_lan_socket_error_detected(&mut self);
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&mut self, label: &str) -> Vec<u16>;
    /// `GetStringFromRegistry("\\ergc", "", s)`: the CD serial.
    fn registry_serial(&mut self) -> Vec<u8>;
    /// `TheGlobalData->m_exeCRC`.
    fn exe_crc(&self) -> u32;
    /// `TheGlobalData->m_iniCRC`.
    fn ini_crc(&self) -> u32;
    /// `TheGlobalData->m_netMinPlayers`.
    fn net_min_players(&self) -> i32;
    /// `GetUserName`; None on failure.
    fn user_name(&mut self) -> Option<Vec<u8>>;
    /// `GetComputerName`; None on failure.
    fn computer_name(&mut self) -> Option<Vec<u8>>;
    /// `TheNetwork != NULL && TheNetwork->isPacketRouter()`.
    fn network_is_packet_router(&self) -> bool;
    /// `TheMapCache->findMap(map)->m_numPlayers`; None if not in the cache.
    fn map_num_players(&self, map: &[u8]) -> Option<i32>;
    /// `TheGameState->realMapPathToPortableMapPath`.
    fn map_to_portable(&self, map: &[u8]) -> Vec<u8>;
    /// The system chat line `RequestHasMap` shows when the local player lacks the map ("GUI:LocalPlayerNoMap" or
    /// "...WillTransfer", with the map's display name).
    fn no_map_message(&mut self, map: &[u8]) -> UnicodeString;
    /// `LANPreferences::getPreferredMap()`.
    fn preferred_map(&mut self) -> Vec<u8>;
    /// `prefs["RemoteIP0"] = entry; prefs.write()`.
    fn remember_remote_ip(&mut self, entry: &[u8]);
    /// `UnicodeStringToQuotedPrintable(name)`.
    fn quoted_printable(&self, name: &UnicodeString) -> Vec<u8>;
    /// The local MOTD file's text when `m_useLocalMOTD` (`m_playStats` raised to 30 if not positive); None when
    /// the option is off. An empty vector is a missing file.
    fn read_local_motd(&mut self) -> Option<Vec<u8>>;
    /// `GetTickCount()`.
    fn get_tick_count(&self) -> u32;
}

/// `class LANAPI`.
pub struct LANAPI<H: LanApiHost> {
    /// The services.
    pub host: H,
    /// `m_lobbyPlayers`.
    pub lobby_players: Vec<LANPlayer>,
    /// `m_games`.
    pub games: Vec<LANGameInfo>,
    /// `m_name`.
    pub name: UnicodeString,
    /// `m_userName`.
    pub user_name: AsciiString,
    /// `m_hostName`.
    pub host_name: AsciiString,
    /// `m_gameStartTime`.
    pub game_start_time: u32,
    /// `m_gameStartSeconds`.
    pub game_start_seconds: i32,
    /// `m_pendingAction`.
    pub pending_action: PendingActionType,
    /// `m_expiration`.
    pub expiration: u32,
    /// `m_actionTimeout`.
    pub action_timeout: u32,
    /// `m_directConnectRemoteIP`.
    pub direct_connect_remote_ip: u32,
    /// `m_lastResendTime`.
    pub last_resend_time: u32,
    /// `m_isInLANMenu`.
    pub is_in_lan_menu: bool,
    /// `m_inLobby`.
    pub in_lobby: bool,
    /// `m_currentGame` as an index into `games`.
    pub current_game: Option<usize>,
    /// `m_localIP`.
    pub local_ip: u32,
    /// `m_transport`.
    pub transport: Transport,
    /// `m_broadcastAddr` (`INADDR_BROADCAST`).
    pub broadcast_addr: u32,
    /// `m_lastUpdate`.
    pub last_update: u32,
    /// `m_lastGameopt`.
    pub last_gameopt: AsciiString,
    /// `m_isActive`.
    pub is_active: bool,
    /// `static UnsignedInt lastMOTDCheck` of `update`.
    last_motd_check: u32,
    /// `static UnsignedInt oldMOTDCRC` of `checkMOTD`.
    old_motd_crc: u32,
}

impl<H: LanApiHost> LANAPI<H> {
    /// `LANAPI::LANAPI()` (LANAPI.cpp:66).
    pub fn new(host: H) -> LANAPI<H> {
        LANAPI {
            host,
            lobby_players: Vec::new(),
            games: Vec::new(),
            name: UnicodeString::new(),
            user_name: AsciiString::default(),
            host_name: AsciiString::default(),
            game_start_time: 0,
            game_start_seconds: 0,
            pending_action: PendingActionType::None,
            expiration: 0,
            action_timeout: 5000, // ms
            direct_connect_remote_ip: 0,
            last_resend_time: 0,
            is_in_lan_menu: true,
            in_lobby: true,
            current_game: None,
            local_ip: 0,
            transport: Transport::new(),
            broadcast_addr: 0xFFFF_FFFF,
            last_update: 0,
            last_gameopt: AsciiString::default(),
            is_active: true,
            last_motd_check: 0,
            old_motd_crc: 0,
        }
    }

    fn now(&self) -> u32 {
        self.host.time_get_time()
    }

    /// `LANAPI::init` (LANAPI.cpp:98): (re)opens the transport on the lobby port with broadcasts allowed, and
    /// reads the login and machine names (`"unknown"` on failure).
    pub fn init(&mut self) {
        self.game_start_time = 0;
        self.game_start_seconds = 0;
        self.transport.reset();
        let host = &self.host;
        let mut tick = host.time_get_time();
        let _ = self.transport.init(self.local_ip, LOBBY_PORT, || {
            tick = tick.wrapping_add(1);
            tick
        });
        self.transport.allow_broadcasts(true);
        self.pending_action = PendingActionType::None;
        self.expiration = 0;
        self.in_lobby = true;
        self.is_in_lan_menu = true;
        self.current_game = None;
        self.direct_connect_remote_ip = 0;
        self.last_gameopt = AsciiString::default();
        let u = self.host.user_name().unwrap_or_else(|| b"unknown".to_vec());
        self.user_name = AsciiString::try_from_bytes(&u).unwrap_or_default();
        let c = self.host.computer_name().unwrap_or_else(|| b"unknown".to_vec());
        self.host_name = AsciiString::try_from_bytes(&c).unwrap_or_default();
    }

    /// `LANAPI::reset` (LANAPI.cpp:140): tells the lobby we are leaving, flushes, drops the lists.
    pub fn reset(&mut self) {
        if self.in_lobby {
            let mut msg = LANMessage::new();
            self.fill_in_lan_message(&mut msg);
            msg.set_message_type(lan_message_type::REQUEST_LOBBY_LEAVE);
            self.send_message(&msg, 0);
        }
        let now = self.now();
        self.transport.update(now);
        self.games.clear();
        self.lobby_players.clear();
        self.direct_connect_remote_ip = 0;
        self.pending_action = PendingActionType::None;
        self.expiration = 0;
        self.in_lobby = true;
        self.is_in_lan_menu = true;
        self.current_game = None;
    }

    /// `LANAPI::sendMessage` (LANAPI.cpp:185): to `ip` if given; else, in a direct-connect game, to each other
    /// human slot; else broadcast.
    pub fn send_message(&mut self, msg: &LANMessage, ip: u32) {
        if ip != 0 {
            self.transport.queue_send(ip, LOBBY_PORT, msg.as_bytes(), LAN_MESSAGE_SIZE as i32);
        } else if self.current_game.map_or(false, |g| self.games[g].get_is_direct_connect()) {
            let g = self.current_game.unwrap_or(0);
            let local_slot = self.games[g].get_local_slot_num(self.local_ip);
            for i in 0..SLOTS {
                if i as i32 != local_slot {
                    let (human, sip) = (self.games[g].info.slots[i].is_human(), self.games[g].info.slots[i].ip);
                    if human {
                        self.transport.queue_send(sip, LOBBY_PORT, msg.as_bytes(), LAN_MESSAGE_SIZE as i32);
                    }
                }
            }
        } else {
            self.transport.queue_send(self.broadcast_addr, LOBBY_PORT, msg.as_bytes(), LAN_MESSAGE_SIZE as i32);
        }
    }

    /// `LANAPI::checkMOTD` (LANAPI.cpp): every new MOTD file content is shown line by line as a system chat from
    /// "MOTD".
    pub fn check_motd(&mut self) {
        let motd = match self.host.read_local_motd() {
            Some(m) => m,
            None => return,
        };
        let mut new_motd_crc = 0u32;
        if !motd.is_empty() {
            let mut crc = Crc::new();
            crc.compute_crc(&motd);
            new_motd_crc = crc.get();
        }
        if self.old_motd_crc != new_motd_crc {
            self.old_motd_crc = new_motd_crc;
            for raw in motd.split(|&c| c == b'\n') {
                if raw.is_empty() && false {
                    continue;
                }
                let mut line: Vec<u8> = raw.to_vec();
                if line.last() == Some(&b'\r') {
                    line.pop(); // there is a trailing '\r'
                }
                if line.is_empty() {
                    line = b" ".to_vec();
                }
                let wide: Vec<u16> = line.iter().map(|&c| c as u16).collect();
                let ui = UnicodeString::try_from_chars(&wide).unwrap_or_default();
                let motd_name = UnicodeString::try_from_chars(&"MOTD".encode_utf16().collect::<Vec<u16>>()).unwrap_or_default();
                self.host.on_chat(&motd_name, 0, &ui, ChatType::System);
            }
        }
    }

    /// `LANAPI::update` (LANAPI.cpp:343): every 200 ms: services the transport, dispatches each received
    /// message (own broadcasts are ignored), re-announces every `s_resendDelta`, times out players, games and
    /// pending actions, runs the game start countdown and the MOTD check.
    pub fn update(&mut self) {
        if self.host.lan_button_pushed() {
            return;
        }
        const LAN_API_UPDATE_DELAY: u32 = 200;
        let now = self.now();
        if now > self.last_update.wrapping_add(LAN_API_UPDATE_DELAY) {
            self.last_update = now;
        } else {
            return;
        }

        if !self.transport.update(now) && !self.host.lan_socket_error_detected() {
            if self.is_in_lan_menu {
                self.host.set_lan_socket_error_detected();
            }
        }

        let mut i = 0;
        while i < MAX_MESSAGES as usize && !self.host.lan_button_pushed() {
            if self.transport.in_buffer[i].length > 0 {
                let sender_ip = self.transport.in_buffer[i].addr;
                if sender_ip == self.local_ip {
                    self.transport.in_buffer[i].length = 0;
                    i += 1;
                    continue;
                }
                let len = self.transport.in_buffer[i].length as usize;
                let data = self.transport.in_buffer[i].data[..len.min(super::net_command_msg::MAX_MESSAGE_LEN)].to_vec();
                let msg = LANMessage::from_bytes(&data);
                use lan_message_type::*;
                match msg.message_type() {
                    REQUEST_LOCATIONS => self.handle_request_locations(&msg, sender_ip),
                    GAME_ANNOUNCE => self.handle_game_announce(&msg, sender_ip),
                    LOBBY_ANNOUNCE => self.handle_lobby_announce(&msg, sender_ip),
                    REQUEST_GAME_INFO => self.handle_request_game_info(&msg, sender_ip),
                    REQUEST_JOIN => self.handle_request_join(&msg, sender_ip),
                    JOIN_ACCEPT => self.handle_join_accept(&msg, sender_ip),
                    JOIN_DENY => self.handle_join_deny(&msg, sender_ip),
                    REQUEST_GAME_LEAVE => self.handle_request_game_leave(&msg, sender_ip),
                    REQUEST_LOBBY_LEAVE => self.handle_request_lobby_leave(&msg, sender_ip),
                    SET_ACCEPT => self.handle_set_accept(&msg, sender_ip),
                    MAP_AVAILABILITY => self.handle_has_map(&msg, sender_ip),
                    CHAT => self.handle_chat(&msg, sender_ip),
                    GAME_START => self.handle_game_start(&msg, sender_ip),
                    GAME_START_TIMER => self.handle_game_start_timer(&msg, sender_ip),
                    GAME_OPTIONS => self.handle_game_options(&msg, sender_ip),
                    INACTIVE => self.handle_in_active(&msg, sender_ip),
                    _ => {}
                }
                self.transport.in_buffer[i].length = 0;
            }
            i += 1;
        }

        if self.host.lan_button_pushed() {
            return;
        }

        let now = self.now();
        if now > S_RESEND_DELTA.wrapping_add(self.last_resend_time) {
            self.last_resend_time = now;
            if self.in_lobby {
                let n = self.name.clone();
                self.request_set_name(n);
            } else if self.current_game.map_or(false, |g| !self.games[g].info.in_progress) {
                if self.am_i_host() {
                    let opts = self.generate_options();
                    self.request_game_options(&opts, true, 0);
                    self.request_game_announce();
                } else {
                    let mut text = b"User=".to_vec();
                    text.extend_from_slice(self.user_name.str());
                    self.request_game_options(&text, true, 0);
                    let mut text = b"Host=".to_vec();
                    text.extend_from_slice(self.host_name.str());
                    self.request_game_options(&text, true, 0);
                    self.request_game_options(b"HELLO", false, 0);
                }
            } else if self.current_game.is_some() {
                self.request_game_announce();
            }
        }

        let mut player_list_changed = false;
        let mut game_list_changed = false;

        // players we have not heard from in two announce periods are gone
        let mut p = 0;
        while p < self.lobby_players.len() {
            if self.lobby_players[p].get_last_heard().wrapping_add(S_RESEND_DELTA.wrapping_mul(2)) < now {
                self.lobby_players.remove(p);
                player_list_changed = true;
            } else {
                p += 1;
            }
        }
        let mut g = 0;
        while g < self.games.len() {
            if Some(g) != self.current_game
                && self.games[g].get_last_heard().wrapping_add(S_RESEND_DELTA.wrapping_mul(2)) < now
            {
                self.remove_game(g);
                game_list_changed = true;
            } else {
                g += 1;
            }
        }

        if let Some(cg) = self.current_game {
            if !self.games[cg].info.in_progress {
                if !self.am_i_host()
                    && self.games[cg].get_last_heard().wrapping_add(S_RESEND_DELTA.wrapping_mul(16)) < now
                {
                    // the host has stopped talking: leave as if it said so
                    let mut msg = LANMessage::new();
                    self.fill_in_lan_message(&mut msg);
                    msg.set_message_type(lan_message_type::REQUEST_GAME_LEAVE);
                    let host_name = self.games[cg].get_player_name(0);
                    msg.set_name(host_name.str());
                    let host_ip = self.games[cg].get_ip(0);
                    self.handle_request_game_leave(&msg, host_ip);
                    let text = UnicodeString::try_from_chars(&self.host.fetch_text("LAN:HostNotResponding")).unwrap_or_default();
                    let li = self.local_ip;
                    self.host.on_chat(&UnicodeString::new(), li, &text, ChatType::System);
                } else if self.am_i_host() {
                    for p in 1..SLOTS {
                        let (pip, heard) = match self.current_game {
                            Some(cg) => (self.games[cg].get_ip(p), self.games[cg].get_player_last_heard(p)),
                            None => break,
                        };
                        if pip != 0 && heard.wrapping_add(S_RESEND_DELTA.wrapping_mul(8)) < now {
                            let cg = self.current_game.unwrap_or(0);
                            let mut msg = LANMessage::new();
                            self.fill_in_lan_message(&mut msg);
                            let pname = self.games[cg].get_player_name(p);
                            let fmt = self.host.fetch_text("LAN:PlayerDropped");
                            let mut s: Vec<u16> = Vec::new();
                            // the string is a printf format with one %ls
                            let mut k = 0;
                            while k < fmt.len() {
                                if fmt[k] == '%' as u16 && k + 2 < fmt.len() + 1 && fmt.get(k + 1) == Some(&('l' as u16)) && fmt.get(k + 2) == Some(&('s' as u16)) {
                                    s.extend_from_slice(pname.str());
                                    k += 3;
                                } else {
                                    s.push(fmt[k]);
                                    k += 1;
                                }
                            }
                            let the_str = UnicodeString::try_from_chars(&s).unwrap_or_default();
                            msg.set_message_type(lan_message_type::REQUEST_GAME_LEAVE);
                            msg.set_name(pname.str());
                            self.handle_request_game_leave(&msg, pip);
                            let li = self.local_ip;
                            self.host.on_chat(&UnicodeString::new(), li, &the_str, ChatType::System);
                        }
                    }
                }
            }
        }

        if player_list_changed {
            let pl = self.lobby_players.clone();
            self.host.on_player_list(&pl);
        }
        if game_list_changed {
            let gl = self.games.clone();
            self.host.on_game_list(&gl);
        }

        if self.pending_action != PendingActionType::None && now > self.expiration {
            match self.pending_action {
                PendingActionType::Join | PendingActionType::JoinDirectConnect => {
                    self.host.on_game_join(ReturnType::Timeout, None);
                    self.pending_action = PendingActionType::None;
                    self.current_game = None;
                    self.in_lobby = true;
                }
                PendingActionType::Leave => {
                    let n = self.name.clone();
                    self.host.on_player_leave(&n);
                    self.pending_action = PendingActionType::None;
                    self.current_game = None;
                    self.in_lobby = true;
                }
                _ => {
                    self.pending_action = PendingActionType::None;
                }
            }
        }

        // game start countdown
        if self.game_start_time != 0 && self.game_start_seconds != 0 && self.game_start_time <= now {
            let s = self.game_start_seconds;
            self.request_game_start_timer(s);
        } else if self.game_start_time != 0 && self.game_start_time <= now {
            self.reset_game_start_timer();
            self.request_game_start();
        }

        const MOTD_INTERVAL: u32 = 30000;
        if now > self.last_motd_check.wrapping_add(MOTD_INTERVAL) {
            self.check_motd();
            self.last_motd_check = now;
        }
    }

    /// `GenerateGameOptionsString()` for my game.
    pub fn generate_options(&self) -> Vec<u8> {
        generate_game_options_string(self.current_game.map(|g| &self.games[g]), &self.host)
    }

    /// `setIsActive` (LANAPI.cpp): going inactive in a game setup tells the others.
    pub fn set_is_active(&mut self, is_active: bool) {
        if is_active != self.is_active && !is_active && !self.in_lobby && self.current_game.is_some() {
            let mut msg = LANMessage::new();
            self.fill_in_lan_message(&mut msg);
            msg.set_message_type(lan_message_type::INACTIVE);
            self.send_message(&msg, 0);
        }
        self.is_active = is_active;
    }

    /// `fillInLANMessage` (LANAPI.cpp:849): my name, login and machine in the message.
    pub fn fill_in_lan_message(&self, msg: &mut LANMessage) {
        msg.set_name(self.name.str());
        msg.set_user_name(self.user_name.str());
        msg.set_host_name(self.host_name.str());
    }

    /// `AmIHost`.
    pub fn am_i_host(&self) -> bool {
        self.current_game.map_or(false, |g| self.games[g].get_ip(0) == self.local_ip)
    }

    /// `GetMyName`.
    pub fn get_my_name(&self) -> UnicodeString {
        self.name.clone()
    }
    /// `GetLocalIP`.
    pub fn get_local_ip(&self) -> u32 {
        self.local_ip
    }

    /// `SetLocalIP(UnsignedInt)` (LANAPI.cpp): rebinds the transport to the new address.
    pub fn set_local_ip(&mut self, local_ip: u32) -> bool {
        self.local_ip = local_ip;
        self.transport.reset();
        let mut tick = self.host.time_get_time();
        let retval = self.transport.init(self.local_ip, LOBBY_PORT, || {
            tick = tick.wrapping_add(1);
            tick
        });
        self.transport.allow_broadcasts(true);
        retval
    }

    /// `SetLocalIP(AsciiString)`: resolves the name first.
    pub fn set_local_ip_str(&mut self, local_ip: &[u8]) {
        let resolved = super::network_util::resolve_ip(local_ip);
        self.set_local_ip(resolved);
    }

    // ---- lists -------------------------------------------------------------------------------------------------

    /// `LookupGame`: the first game with that name (case-sensitive, `UnicodeString::operator!=`).
    pub fn lookup_game(&self, game_name: &UnicodeString) -> Option<usize> {
        self.games.iter().position(|g| g.get_name() == *game_name)
    }
    /// `LookupGameByListOffset`.
    pub fn lookup_game_by_list_offset(&self, offset: i32) -> Option<usize> {
        if if plant(16406) { offset <= 0 } else { offset < 0 } || offset as usize >= self.games.len() {
            return None;
        }
        Some(offset as usize)
    }
    /// `LookupPlayer`: by IP.
    pub fn lookup_player(&self, player_ip: u32) -> Option<usize> {
        self.lobby_players.iter().position(|p| p.get_ip() == player_ip)
    }
    /// `removeGame`: out of the list (index; `m_currentGame` is kept pointing at the same game, or none).
    pub fn remove_game(&mut self, game: usize) {
        if game >= self.games.len() {
            return;
        }
        self.games.remove(game);
        self.current_game = match self.current_game {
            Some(c) if c == game => None,
            Some(c) if c > game => Some(c - 1),
            other => other,
        };
    }
    /// `removePlayer`.
    pub fn remove_player(&mut self, player: usize) {
        if player < self.lobby_players.len() {
            self.lobby_players.remove(player);
        }
    }
    /// `addGame` (LANAPI.cpp:960): into the list by name (before the head if it sorts earlier, else after the head
    /// and after every following game whose name sorts after it). Returns the index it landed at.
    pub fn add_game(&mut self, game: LANGameInfo) -> usize {
        if self.games.is_empty() {
            self.games.push(game);
            return 0;
        }
        let new_name = game.get_name();
        if new_name.compare_no_case_unicode(&self.games[0].get_name()) == std::cmp::Ordering::Less {
            self.games.insert(0, game);
            if let Some(c) = self.current_game.as_mut().filter(|_| !plant(16405)) {
                *c += 1;
            }
            return 0;
        }
        let mut g = 0usize;
        while g + 1 < self.games.len()
            && self.games[g + 1].get_name().compare_no_case_unicode(&new_name) == std::cmp::Ordering::Greater
        {
            g += 1;
        }
        self.games.insert(g + 1, game);
        if let Some(c) = self.current_game.as_mut() {
            if *c > g {
                *c += 1;
            }
        }
        g + 1
    }
    /// `addPlayer` (LANAPI.cpp:990): the same placement rule as `addGame`, by player name.
    pub fn add_player(&mut self, player: LANPlayer) {
        if self.lobby_players.is_empty() {
            self.lobby_players.push(player);
            return;
        }
        let new_name = player.get_name();
        if if plant(16404) { new_name.compare_no_case_unicode(&self.lobby_players[0].get_name()) != std::cmp::Ordering::Greater } else { new_name.compare_no_case_unicode(&self.lobby_players[0].get_name()) == std::cmp::Ordering::Less } {
            self.lobby_players.insert(0, player);
            return;
        }
        let mut p = 0usize;
        while p + 1 < self.lobby_players.len()
            && self.lobby_players[p + 1].get_name().compare_no_case_unicode(&new_name) == std::cmp::Ordering::Greater
        {
            p += 1;
        }
        self.lobby_players.insert(p + 1, player);
    }

    /// `LANAPI::getErrorStringFromReturnType` is defined in `LANAPICallbacks.cpp` (GUI text) and is the host's.
    pub fn reset_game_start_timer(&mut self) {
        self.game_start_time = 0;
        self.game_start_seconds = 0;
    }

    // ---- requests ------------------------------------------------------------------------------------------------

    /// `RequestLocations` (LANAPI.cpp:380).
    pub fn request_locations(&mut self) {
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::REQUEST_LOCATIONS);
        self.fill_in_lan_message(&mut msg);
        self.send_message(&msg, 0);
    }

    /// `RequestGameJoin` (LANAPI.cpp:390).
    pub fn request_game_join(&mut self, game: Option<usize>, ip: u32) {
        if self.pending_action != PendingActionType::None && (plant(16407) || self.pending_action != PendingActionType::JoinDirectConnect) {
            self.host.on_game_join(ReturnType::Busy, None);
            return;
        }
        let game = match game {
            Some(g) if g < self.games.len() => g,
            _ => {
                self.host.on_game_join(ReturnType::GameGone, None);
                return;
            }
        };
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::REQUEST_JOIN);
        self.fill_in_lan_message(&mut msg);
        msg.set_game_to_join_game_ip(self.games[game].info.slots[0].ip);
        msg.set_game_to_join_exe_crc(self.host.exe_crc());
        msg.set_game_to_join_ini_crc(self.host.ini_crc());
        let s = self.host.registry_serial();
        msg.set_game_to_join_serial(&s);
        self.send_message(&msg, ip);
        self.pending_action = PendingActionType::Join;
        self.expiration = if plant(16413) { self.now() } else { self.now().wrapping_add(self.action_timeout) };
    }

    /// `RequestGameJoinDirectConnect` (LANAPI.cpp:430).
    pub fn request_game_join_direct_connect(&mut self, ipaddress: u32) {
        if self.pending_action != PendingActionType::None {
            self.host.on_game_join(ReturnType::Busy, None);
            return;
        }
        if ipaddress == 0 {
            self.host.on_game_join(ReturnType::GameGone, None);
            return;
        }
        self.direct_connect_remote_ip = ipaddress;
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::REQUEST_GAME_INFO);
        self.fill_in_lan_message(&mut msg);
        msg.set_player_info_ip(if plant(16408) { ipaddress } else { self.local_ip });
        msg.set_player_info_name(self.name.str());
        self.send_message(&msg, ipaddress);
        self.pending_action = PendingActionType::JoinDirectConnect;
        self.expiration = self.now().wrapping_add(self.action_timeout);
    }

    /// `RequestGameLeave` (LANAPI.cpp:465): sent at once; the host leaves immediately, a joiner waits for the
    /// answer (or the timeout).
    pub fn request_game_leave(&mut self) {
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::REQUEST_GAME_LEAVE);
        self.fill_in_lan_message(&mut msg);
        let gname = self.current_game.map(|g| self.games[g].get_name()).unwrap_or_default();
        msg.set_game_name(gname.str());
        self.send_message(&msg, 0);
        let now = self.now();
        self.transport.update(now); // send immediately, before OnPlayerLeave resets everything
        if self.current_game.map_or(false, |g| self.games[g].get_ip(0) == self.local_ip) {
            let n = self.name.clone();
            self.host.on_player_leave(&n);
            if let Some(g) = self.current_game {
                self.remove_game(g);
            }
            self.current_game = None;
            self.in_lobby = true;
        } else {
            self.pending_action = PendingActionType::Leave;
            self.expiration = self.now().wrapping_add(self.action_timeout);
        }
    }

    /// `RequestGameAnnounce` (LANAPI.cpp:490): the host (or, in game, the packet router) announces the game.
    pub fn request_game_announce(&mut self) {
        let g = match self.current_game {
            Some(g) if !self.games[g].get_is_direct_connect() => g,
            _ => return,
        };
        if self.games[g].get_ip(0) == self.local_ip
            || (self.games[g].info.in_progress && self.host.network_is_packet_router())
        {
            let mut reply = LANMessage::new();
            self.fill_in_lan_message(&mut reply);
            reply.set_message_type(lan_message_type::GAME_ANNOUNCE);
            let game_opts = game_info_to_ascii_string(Some(&self.games[g].info), &self.host).unwrap_or_default();
            reply.set_game_info_options(&game_opts);
            let n = self.games[g].get_name();
            reply.set_game_name(n.str());
            reply.set_game_info_in_progress(self.games[g].info.in_progress);
            reply.set_game_info_is_direct_connect(self.games[g].get_is_direct_connect());
            self.send_message(&reply, 0);
        }
    }

    /// `RequestAccept` (LANAPI.cpp:515).
    pub fn request_accept(&mut self) {
        let g = match self.current_game {
            Some(g) if !self.in_lobby => g,
            _ => return,
        };
        let mut msg = LANMessage::new();
        self.fill_in_lan_message(&mut msg);
        msg.set_message_type(lan_message_type::SET_ACCEPT);
        msg.set_accept_is_accepted(!plant(16412));
        let n = self.games[g].get_name();
        msg.set_game_name(n.str());
        self.send_message(&msg, 0);
    }

    /// `RequestHasMap` (LANAPI.cpp:530): tells the others whether I have the map (by the CRC of its portable
    /// name); a missing map also posts a system chat line.
    pub fn request_has_map(&mut self) {
        let g = match self.current_game {
            Some(g) if !self.in_lobby => g,
            _ => return,
        };
        let mut msg = LANMessage::new();
        self.fill_in_lan_message(&mut msg);
        msg.set_message_type(lan_message_type::MAP_AVAILABILITY);
        let local_slot = self.games[g].get_local_slot_num(self.local_ip);
        let has_map = if plant(16411) { local_slot > 0 } else { local_slot >= 0 } && self.games[g].info.slots[local_slot as usize].has_map;
        msg.set_map_status_has_map(has_map);
        let n = self.games[g].get_name();
        msg.set_game_name(n.str());
        let mut crc = Crc::new();
        let portable = self.host.map_to_portable(&self.games[g].info.map_name);
        crc.compute_crc(&portable);
        msg.set_map_status_map_crc(crc.get());
        self.send_message(&msg, 0);
        if !has_map {
            let map = self.games[g].info.map_name.clone();
            let text = self.host.no_map_message(&map);
            let sys = UnicodeString::try_from_chars(&"SYSTEM".encode_utf16().collect::<Vec<u16>>()).unwrap_or_default();
            let li = self.local_ip;
            self.host.on_chat(&sys, li, &text, ChatType::System);
        }
    }

    /// `RequestChat` (LANAPI.cpp:575).
    pub fn request_chat(&mut self, message: UnicodeString, format: ChatType) {
        let mut msg = LANMessage::new();
        self.fill_in_lan_message(&mut msg);
        let gname = self.current_game.map(|g| self.games[g].get_name()).unwrap_or_default();
        msg.set_game_name(gname.str());
        msg.set_message_type(lan_message_type::CHAT);
        msg.set_chat_type(if plant(16410) { 0 } else { format as i32 });
        msg.set_chat_message(message.str());
        self.send_message(&msg, 0);
        let (n, li) = (self.name.clone(), self.local_ip);
        self.host.on_chat(&n, li, &message, format);
    }

    /// `RequestGameStart` (LANAPI.cpp:590): only the host, in a game.
    pub fn request_game_start(&mut self) {
        if self.in_lobby || self.current_game.map_or(true, |g| self.games[g].get_ip(0) != self.local_ip) {
            return;
        }
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::GAME_START);
        self.fill_in_lan_message(&mut msg);
        self.send_message(&msg, 0);
        let now = self.now();
        self.transport.update(now); // force a send
        self.host.on_game_start();
    }

    /// `RequestGameStartTimer` (LANAPI.cpp:610).
    pub fn request_game_start_timer(&mut self, seconds: i32) {
        if self.in_lobby || self.current_game.map_or(true, |g| self.games[g].get_ip(0) != self.local_ip) {
            return;
        }
        let now = self.now();
        self.game_start_time = now.wrapping_add(1000);
        self.game_start_seconds = if seconds != 0 { if plant(16409) { seconds } else { seconds - 1 } } else { 0 };
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::GAME_START_TIMER);
        msg.set_start_timer_seconds(seconds);
        self.fill_in_lan_message(&mut msg);
        self.send_message(&msg, 0);
        self.transport.update(now); // force a send
        self.host.on_game_start_timer(seconds);
    }

    /// `RequestGameOptions` (LANAPI.cpp:640): sends the options and applies them locally as if received.
    pub fn request_game_options(&mut self, game_options: &[u8], _is_public: bool, ip: u32) {
        if self.current_game.is_none() {
            return;
        }
        let mut msg = LANMessage::new();
        self.fill_in_lan_message(&mut msg);
        msg.set_message_type(lan_message_type::GAME_OPTIONS);
        msg.set_game_options_options(game_options);
        self.send_message(&msg, ip);
        self.last_gameopt = AsciiString::try_from_bytes(game_options).unwrap_or_default();
        let g = self.current_game.unwrap_or(0);
        for player in 0..SLOTS {
            if self.games[g].get_ip(player) == self.local_ip {
                let opts = msg.game_options_options();
                let li = self.local_ip;
                self.host.on_game_options(li, player as i32, &opts);
                break;
            }
        }
    }

    /// `RequestGameCreate` (LANAPI.cpp:670): makes the game with me as slot 0 (host), named
    /// "<localIP hex><seed hex><name>" cut to the name length, on my preferred map.
    pub fn request_game_create(&mut self, game_name: UnicodeString, is_direct_connect: bool) {
        if (!self.in_lobby || self.current_game.is_some()) && !is_direct_connect {
            self.host.on_game_create(ReturnType::Busy);
            return;
        }
        if self.pending_action != PendingActionType::None {
            self.host.on_game_create(ReturnType::Busy);
            return;
        }
        self.in_lobby = false;
        let mut my_game = LANGameInfo::new(&mut self.host);
        my_game.set_seed(self.host.get_tick_count() as i32);
        my_game.info.enter_game(&mut self.host);
        let mut s: Vec<u16> = format!("{:08X}{:08X}", self.local_ip, my_game.info.seed as u32).encode_utf16().collect();
        if game_name.is_empty() {
            s.extend_from_slice(self.name.str());
        } else {
            s.extend_from_slice(game_name.str());
        }
        while s.len() > G_LAN_GAME_NAME_LENGTH {
            s.pop();
        }
        my_game.set_name(UnicodeString::try_from_chars(&s).unwrap_or_default());

        let mut new_slot = GameSlot::new();
        new_slot.set_state(slot_state::PLAYER, self.name.str(), 0, &mut self.host);
        new_slot.ip = self.local_ip;
        new_slot.port = NETWORK_BASE_PORT_NUMBER as u16; // LAN game, everyone has a unique IP, so the same port is ok
        let mut extra = LANSlotExtra::new();
        extra.set_last_heard(0);
        extra.set_login_ascii(&self.user_name);
        extra.set_host_ascii(&self.host_name);
        my_game.set_slot(0, new_slot, extra);

        let map_name = self.host.preferred_map();
        my_game.set_map(&map_name, &mut self.host);
        my_game.set_is_direct_connect(is_direct_connect);
        my_game.set_last_heard(self.now());
        let idx = self.add_game(my_game);
        self.current_game = Some(idx);
        self.host.on_game_create(ReturnType::Ok);
    }

    /// `RequestSetName` (LANAPI.cpp:805): changes my lobby name and announces it.
    pub fn request_set_name(&mut self, mut new_name: UnicodeString) {
        let _ = new_name.trim();
        if self.pending_action != PendingActionType::None {
            // can't change name while a join/leave is pending: just report it
            let li = self.local_ip;
            self.host.on_name_change(li, &new_name);
            return;
        }
        self.last_resend_time = self.now();
        if self.in_lobby && self.pending_action == PendingActionType::None {
            self.name = new_name;
            let mut msg = LANMessage::new();
            self.fill_in_lan_message(&mut msg);
            msg.set_message_type(lan_message_type::LOBBY_ANNOUNCE);
            self.send_message(&msg, 0);

            let mut player = match self.lookup_player(self.local_ip) {
                Some(i) => self.lobby_players.remove(i),
                None => {
                    let mut p = LANPlayer::new();
                    p.set_ip(self.local_ip);
                    p
                }
            };
            player.set_name(self.name.clone());
            player.set_host_ascii(&self.host_name);
            player.set_login_ascii(&self.user_name);
            player.set_last_heard(self.now());
            let (pip, pname) = (player.get_ip(), player.get_name());
            self.add_player(player);
            self.host.on_name_change(pip, &pname);
        }
    }

    /// `RequestLobbyLeave` (LANAPI.cpp:870).
    pub fn request_lobby_leave(&mut self, forced: bool) {
        let mut msg = LANMessage::new();
        msg.set_message_type(lan_message_type::REQUEST_LOBBY_LEAVE);
        self.fill_in_lan_message(&mut msg);
        self.send_message(&msg, 0);
        if forced {
            let now = self.now();
            self.transport.update(now);
        }
    }
}
