//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANPlayer.h
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp
//!
//! # Shape
//!
//! Upstream's `LANGameInfo` derives `GameInfo` and points the base's slot pointers at its own `LANGameSlot`s
//! (`GameSlot` plus a `LANPlayer`, the serial and the time last heard). The port's `GameInfo`
//! (`crate::common::game_info`) holds its eight `GameSlot`s by value, so [`LANGameInfo`] holds a `GameInfo` and
//! the per-slot LAN extras ([`LANSlotExtra`]) side by side, index for index. The base-class functions are
//! `self.info`'s.
//!
//! # Not translated
//!
//! `LANDisplayGameList`, `LANEnableStartButton`, `LANDisplaySlotList` and `LANDisplayGameOptions` draw into
//! GameWindow list boxes (GUI drawing code: `GadgetListBox*`, `HideGameInfoWindow`); the shell GUI is the client's.
//! `LANEnableStartButton` is a host call here ([`LanGameInfoHost::lan_enable_start_button`]).

use std::collections::BTreeMap;

use crate::common::game_info::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, GameInfo, GameInfoHost, GameSlot, slot_state,
};
use crate::common::unicode_string::UnicodeString;
use crate::common::ascii_string::AsciiString;

use super::net_command_msg::MAX_SLOTS;

const SLOTS: usize = MAX_SLOTS as usize;

/// `class LANPlayer`: a player in the lobby, identified by IP address.
#[derive(Clone, Debug, Default)]
pub struct LANPlayer {
    /// `m_name`.
    name: UnicodeString,
    /// `m_login`.
    login: UnicodeString,
    /// `m_host`: the machine name.
    host: UnicodeString,
    /// `m_lastHeard`.
    last_heard: u32,
    /// `m_IP`.
    ip: u32,
}

impl LANPlayer {
    /// `LANPlayer::LANPlayer()`.
    pub fn new() -> LANPlayer {
        LANPlayer::default()
    }
    /// `getName`.
    pub fn get_name(&self) -> UnicodeString {
        self.name.clone()
    }
    /// `setName`.
    pub fn set_name(&mut self, name: UnicodeString) {
        self.name = name;
    }
    /// `getLogin`.
    pub fn get_login(&self) -> UnicodeString {
        self.login.clone()
    }
    /// `setLogin(UnicodeString)`.
    pub fn set_login(&mut self, name: UnicodeString) {
        self.login = name;
    }
    /// `setLogin(AsciiString)`: `m_login.translate(name)`.
    pub fn set_login_ascii(&mut self, name: &AsciiString) {
        let _ = self.login.translate(name);
    }
    /// `getHost`.
    pub fn get_host(&self) -> UnicodeString {
        self.host.clone()
    }
    /// `setHost(UnicodeString)`.
    pub fn set_host(&mut self, name: UnicodeString) {
        self.host = name;
    }
    /// `setHost(AsciiString)`.
    pub fn set_host_ascii(&mut self, name: &AsciiString) {
        let _ = self.host.translate(name);
    }
    /// `getLastHeard`.
    pub fn get_last_heard(&self) -> u32 {
        self.last_heard
    }
    /// `setLastHeard`.
    pub fn set_last_heard(&mut self, last_heard: u32) {
        self.last_heard = last_heard;
    }
    /// `getIP`.
    pub fn get_ip(&self) -> u32 {
        self.ip
    }
    /// `setIP`.
    pub fn set_ip(&mut self, ip: u32) {
        self.ip = ip;
    }
}

/// What a `LANGameSlot` adds to a `GameSlot`: the `LANPlayer` (`m_user`, filled in by `getUser`), the serial
/// and when we last heard from the slot.
#[derive(Clone, Debug, Default)]
pub struct LANSlotExtra {
    /// `m_user`.
    user: LANPlayer,
    /// `m_serial`.
    serial: AsciiString,
    /// `m_lastHeard`.
    last_heard: u32,
}

impl LANSlotExtra {
    /// `LANGameSlot::LANGameSlot()`: `m_lastHeard = 0`.
    pub fn new() -> LANSlotExtra {
        LANSlotExtra::default()
    }
    /// `LANGameSlot::getUser` (LANGameInfo.cpp:55): the slot's user for a human slot (its IP, last heard and name
    /// copied in), else None.
    pub fn get_user(&mut self, slot: &GameSlot) -> Option<&LANPlayer> {
        if slot.is_human() {
            self.user.set_ip(slot.ip);
            self.user.set_last_heard(self.last_heard);
            let name = UnicodeString::try_from_chars(&slot.name).unwrap_or_default();
            self.user.set_name(name);
            return Some(&self.user);
        }
        None
    }
    /// `LANGameSlot::isUser(LANPlayer *user)` (LANGameInfo.cpp:69): a player slot with the user's IP.
    pub fn is_user_player(&self, slot: &GameSlot, user: Option<&LANPlayer>) -> bool {
        user.map_or(false, |u| slot.state == slot_state::PLAYER && u.get_ip() == slot.ip)
    }
    /// `LANGameSlot::isUser(UnicodeString userName)` (LANGameInfo.cpp:75): a player slot with that name, ignoring
    /// case.
    pub fn is_user_name(&self, slot: &GameSlot, user_name: &UnicodeString) -> bool {
        let name = UnicodeString::try_from_chars(&slot.name).unwrap_or_default();
        slot.state == slot_state::PLAYER && user_name.compare_no_case_unicode(&name) == std::cmp::Ordering::Equal
    }
    /// `isLocalPlayer` (LANGameInfo.cpp:81): a human slot whose IP is `TheLAN->GetLocalIP()`.
    pub fn is_local_player(slot: &GameSlot, local_ip: u32) -> bool {
        slot.is_human() && local_ip == slot.ip
    }
    /// `setLogin(UnicodeString)`.
    pub fn set_login(&mut self, name: UnicodeString) {
        self.user.set_login(name);
    }
    /// `setLogin(AsciiString)`.
    pub fn set_login_ascii(&mut self, name: &AsciiString) {
        self.user.set_login_ascii(name);
    }
    /// `setHost(UnicodeString)`.
    pub fn set_host(&mut self, name: UnicodeString) {
        self.user.set_host(name);
    }
    /// `setHost(AsciiString)`.
    pub fn set_host_ascii(&mut self, name: &AsciiString) {
        self.user.set_host_ascii(name);
    }
    /// `setSerial`.
    pub fn set_serial(&mut self, serial: AsciiString) {
        self.serial = serial;
    }
    /// `getSerial`.
    pub fn get_serial(&self) -> AsciiString {
        self.serial.clone()
    }
    /// `setLastHeard`.
    pub fn set_last_heard(&mut self, t: u32) {
        self.last_heard = t;
    }
    /// `getLastHeard`.
    pub fn get_last_heard(&self) -> u32 {
        self.last_heard
    }
    /// The login the slot's user holds (`slot->getUser()->getLogin()`).
    pub fn user_login(&self) -> UnicodeString {
        self.user.get_login()
    }
    /// The machine name the slot's user holds.
    pub fn user_host(&self) -> UnicodeString {
        self.user.get_host()
    }
}

/// What `resetAccepted` and `ParseGameOptionsString` ask of `TheLAN` and the lobby GUI.
pub trait LanGameInfoHost: GameInfoHost {
    /// `TheLAN != NULL`.
    fn has_lan(&self) -> bool;
    /// `TheLAN->GetLocalIP()`.
    fn lan_local_ip(&self) -> u32;
    /// `TheLAN->ResetGameStartTimer()`.
    fn lan_reset_game_start_timer(&mut self);
    /// `TheLAN->GetMyGame() == this`, given the game being reset.
    fn lan_my_game_is_this(&self) -> bool;
    /// `TheLAN->AmIHost()`.
    fn lan_am_i_host(&self) -> bool;
    /// `LANEnableStartButton(enabled)`.
    fn lan_enable_start_button(&mut self, enabled: bool);
    /// `TheLAN->RequestHasMap()`.
    fn lan_request_has_map(&mut self);
    /// `lanUpdateSlotList()`.
    fn lan_update_slot_list(&mut self);
    /// `updateGameOptions()`.
    fn lan_update_game_options(&mut self);
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
}

/// `class LANGameInfo`.
#[derive(Clone, Debug)]
pub struct LANGameInfo {
    /// The `GameInfo` base.
    pub info: GameInfo,
    /// `m_LANSlot`'s extras, one per slot.
    pub extras: [LANSlotExtra; SLOTS],
    /// `m_lastHeard`.
    last_heard: u32,
    /// `m_gameName`.
    game_name: UnicodeString,
    /// `m_isDirectConnect` (uninitialised upstream; false).
    is_direct_connect: bool,
}

impl LANGameInfo {
    /// `LANGameInfo::LANGameInfo()` (LANGameInfo.cpp:95): `setLocalIP(TheLAN->GetLocalIP())`.
    pub fn new<H: LanGameInfoHost>(host: &mut H) -> LANGameInfo {
        let mut info = GameInfo::new(host);
        info.local_ip = host.lan_local_ip();
        LANGameInfo {
            info,
            extras: Default::default(),
            last_heard: 0,
            game_name: UnicodeString::new(),
            is_direct_connect: false,
        }
    }

    /// `LANGameInfo::setSlot` (LANGameInfo.cpp:106): the host's slot (0) is always accepted and has the map.
    pub fn set_slot(&mut self, slot_num: i32, slot_info: GameSlot, extra: LANSlotExtra) {
        if slot_num < 0 || slot_num >= MAX_SLOTS {
            return;
        }
        self.info.slots[slot_num as usize] = slot_info;
        self.extras[slot_num as usize] = extra;
        if slot_num == 0 {
            self.info.slots[0].set_accept();
            self.info.slots[0].set_map_availability(true);
        }
    }

    /// `getLANSlot`: the slot and its extras.
    pub fn get_lan_slot(&mut self, slot_num: i32) -> Option<(&mut GameSlot, &mut LANSlotExtra)> {
        if slot_num < 0 || slot_num >= MAX_SLOTS {
            return None;
        }
        let i = slot_num as usize;
        Some((&mut self.info.slots[i], &mut self.extras[i]))
    }

    /// `LANGameInfo::getLocalSlotNum` (LANGameInfo.cpp:134): the first local-player slot, -1 when not in a game.
    pub fn get_local_slot_num(&self, local_ip: u32) -> i32 {
        if !self.info.in_game {
            return -1;
        }
        for i in 0..SLOTS {
            if LANSlotExtra::is_local_player(&self.info.slots[i], local_ip) {
                return i as i32;
            }
        }
        -1
    }

    /// `LANGameInfo::getSlotNum` (LANGameInfo.cpp:150): the slot of a user by name, -1 if absent.
    pub fn get_slot_num(&self, user_name: &UnicodeString) -> i32 {
        if !self.info.in_game {
            return -1;
        }
        for i in 0..SLOTS {
            if self.extras[i].is_user_name(&self.info.slots[i], user_name) {
                return i as i32;
            }
        }
        -1
    }

    /// `LANGameInfo::amIHost` (LANGameInfo.cpp:165).
    pub fn am_i_host(&self, local_ip: u32) -> bool {
        if !self.info.in_game {
            return false;
        }
        LANSlotExtra::is_local_player(&self.info.slots[0], local_ip)
    }

    /// `LANGameInfo::setMap` (LANGameInfo.cpp:175): `GameInfo::setMap`.
    pub fn set_map<H: GameInfoHost>(&mut self, map_name: &[u8], host: &mut H) {
        let _ = self.info.set_map(map_name, host);
    }

    /// `LANGameInfo::setSeed` (LANGameInfo.cpp:180): `GameInfo::setSeed`.
    pub fn set_seed(&mut self, seed: i32) {
        self.info.seed = seed;
    }

    /// `LANGameInfo::resetAccepted` (LANGameInfo.cpp:185): restarts the game start timer, re-enables the start
    /// button on the host, and un-accepts every slot.
    pub fn reset_accepted<H: LanGameInfoHost>(&mut self, host: &mut H) {
        if host.has_lan() {
            host.lan_reset_game_start_timer();
            if host.lan_my_game_is_this() && host.lan_am_i_host() {
                host.lan_enable_start_button(true);
            }
        }
        for i in 0..SLOTS {
            self.info.slots[i].un_accept();
        }
    }

    /// `getLastHeard`.
    pub fn get_last_heard(&self) -> u32 {
        self.last_heard
    }
    /// `setLastHeard`.
    pub fn set_last_heard(&mut self, last_heard: u32) {
        self.last_heard = last_heard;
    }
    /// `setName`.
    pub fn set_name(&mut self, name: UnicodeString) {
        self.game_name = name;
    }
    /// `getName`.
    pub fn get_name(&self) -> UnicodeString {
        self.game_name.clone()
    }
    /// `getIP(who)`.
    pub fn get_ip(&self, who: usize) -> u32 {
        self.info.slots[who].ip
    }
    /// `setIP(who, IP)`.
    pub fn set_ip(&mut self, who: usize, ip: u32) {
        self.info.slots[who].ip = ip;
    }
    /// `setIsDirectConnect`.
    pub fn set_is_direct_connect(&mut self, v: bool) {
        self.is_direct_connect = v;
    }
    /// `getIsDirectConnect`.
    pub fn get_is_direct_connect(&self) -> bool {
        self.is_direct_connect
    }
    /// `setPlayerName(who, name)`.
    pub fn set_player_name(&mut self, who: usize, name: &UnicodeString) {
        self.info.slots[who].name = name.str().to_vec();
    }
    /// `getPlayerName(who)`.
    pub fn get_player_name(&self, who: usize) -> UnicodeString {
        UnicodeString::try_from_chars(&self.info.slots[who].name).unwrap_or_default()
    }
    /// `getPlayerLastHeard(who)`: 0 for a non-human slot.
    pub fn get_player_last_heard(&self, who: usize) -> u32 {
        if self.info.slots[who].is_human() {
            return self.extras[who].get_last_heard();
        }
        0
    }
    /// `setPlayerLastHeard(who, lastHeard)`: ignored for a non-human slot.
    pub fn set_player_last_heard(&mut self, who: usize, last_heard: u32) {
        if self.info.slots[who].is_human() {
            self.extras[who].set_last_heard(last_heard);
        }
    }
    /// `getHostIP`: the host slot's IP, 0 if it is not human.
    pub fn get_host_ip(&self) -> u32 {
        if self.info.slots[0].is_human() {
            return self.info.slots[0].ip;
        }
        0
    }
}

/// `GenerateGameOptionsString` (LANGameInfo.cpp): the options string of my game, only if I am its host.
pub fn generate_game_options_string<H: LanGameInfoHost>(my_game: Option<&LANGameInfo>, host: &H) -> Vec<u8> {
    match my_game {
        Some(g) if g.am_i_host(host.lan_local_ip()) => game_info_to_ascii_string(Some(&g.info), host).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `ParseGameOptionsString` (LANGameInfo.cpp): applies a game-options string to a game, remembering each human
/// slot's login and machine across the parse; a joiner whose map availability changed asks for the map and
/// refreshes the lobby; every human slot is stamped `now` as last heard.
pub fn parse_game_options_string<H: LanGameInfoHost>(
    game: Option<&mut LANGameInfo>,
    options: &[u8],
    host: &mut H,
) -> bool {
    let game = match game {
        Some(g) if host.has_lan() => g,
        _ => return false,
    };
    let local_ip = host.lan_local_ip();
    let old_local_slot_num = if game.info.in_game { game.get_local_slot_num(local_ip) } else { -1 };
    let was_in_game = old_local_slot_num >= 0;
    let old_map_crc = game.info.map_crc;
    let mut old_logins: BTreeMap<Vec<u16>, UnicodeString> = BTreeMap::new();
    let mut old_machines: BTreeMap<Vec<u16>, UnicodeString> = BTreeMap::new();
    for i in 0..SLOTS {
        if game.info.slots[i].is_human() {
            let key = game.info.slots[i].name.clone();
            old_logins.insert(key.clone(), game.extras[i].user_login());
            old_machines.insert(key, game.extras[i].user_host());
        }
    }
    if parse_ascii_string_to_game_info(Some(&mut game.info), options, host).unwrap_or(false) {
        let new_local_slot_num = if game.info.in_game { game.get_local_slot_num(local_ip) } else { -1 };
        let is_in_game = new_local_slot_num >= 0;
        if !host.lan_am_i_host() && is_in_game {
            let new_map_crc = game.info.map_crc;
            if (old_map_crc ^ new_map_crc) != 0 || (!was_in_game && is_in_game) {
                host.lan_request_has_map();
                host.lan_update_slot_list();
                host.lan_update_game_options();
            }
        }
        let now = host.time_get_time();
        for i in 0..SLOTS {
            if game.info.slots[i].is_human() {
                game.extras[i].set_last_heard(now);
                let key = game.info.slots[i].name.clone();
                if let Some(l) = old_logins.get(&key) {
                    game.extras[i].set_login(l.clone());
                }
                if let Some(m) = old_machines.get(&key) {
                    game.extras[i].set_host(m.clone());
                }
            }
        }
        return true;
    }
    false
}
