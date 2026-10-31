// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameInfo.h
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp (`GetBasePathFromPath` and the four map-derived paths
//     `setMap` probes)
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/ThreadUtils.cpp (`MultiByteToWideCharSingleLine`,
//     `WideCharStringToMultiByte`)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/strtok_r.cpp (Westwood's `strtok_r`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameSlot`, `GameInfo`, `SkirmishGameInfo`'s save block, and the game options string (`GameInfoToAsciiString`,
//! `ParseAsciiStringToGameInfo`) that a replay's header, a LAN game and a skirmish carry (layer 12 batch G1).
//!
//! # Shape
//!
//! Upstream's `GameInfo` holds eight `GameSlot*`; every class the game makes (`LANGameInfo`, the GameSpy staging room,
//! `SkirmishGameInfo`, the recorder's `ReplayGameInfo`) points them at its own array, so here a [`GameInfo`] owns its
//! eight slots and a `NULL` slot cannot occur. What the file reaches outside itself (the map cache, the file system, the
//! game text, `TheGameState`'s map paths, the multiplayer settings, the player template store, the global data, the
//! clock, the player list's academy statistics) is the [`GameInfoHost`] trait.
//!
//! `GameSlot`'s apparent template, colour and start position ask the static `isSlotLocalAlly(slot)`, which looks the slot
//! up in the global `TheGameInfo`, not in the game the slot belongs to; so does `GameInfo::isSkirmish` (the two-argument
//! `isSlotLocalAlly(game, slot)` is never called). Here the caller says where the slot is in `TheGameInfo`
//! ([`TheGameInfo`]).
//!
//! # Strings
//!
//! Upstream converts a slot's name with Windows' `WideCharToMultiByte(CP_UTF8)` and `MultiByteToWideChar(CP_UTF8)`
//! ([`wide_to_utf8`], [`utf8_to_wide_single_line`]): UTF-16 to UTF-8 and back, an ill-formed sequence becoming U+FFFD (the
//! converters' behaviour since Windows Vista; Windows 2000 and XP dropped such bytes instead). `MultiByteToWideCharSingleLine`
//! gives the converter a buffer of `strlen` units for `strlen + 1` with the terminator; the translation takes it that the
//! converter fills the buffer before it reports the shortfall (**not re-checked** on Windows; it matters only for a name
//! of single-byte characters, whose last unit is then the terminator's place, which the function overwrites with 0
//! anyway).
//!
//! # Numbers
//!
//! `atoi` and `sscanf("%x"/"%X"/"%d")` of a value their type cannot hold are undefined in C; `strtoul` of a magnitude
//! above `UnsignedInt` clamps on Windows (its `unsigned long` is 32 bits) but not on a 64-bit `long`. All three are
//! reported as [`Undefined::IntOutOfRange`] / [`Undefined::UnsignedOutOfRange`] rather than given one platform's answer.
//!
//! # Not translated
//!
//! `getApparentPlayerTemplateDisplayName` and the other display-only reads are translated; the debug logging is not.

use crate::common::ini::{IniError, Undefined};
use crate::common::money::Money;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::modules::plant;

type R<T> = Result<T, IniError>;

/// `MAX_SLOTS`.
pub const MAX_SLOTS: usize = 8;
/// `m_lanMaxOptionsLength` (LANAPI.h): `MAX_PACKET_SIZE - (8 + (16+1)*2 + 4 + (12+1)*2 + (1+1) + (1+1))`.
pub const LAN_MAX_OPTIONS_LENGTH: i32 = 476 - (8 + (16 + 1) * 2 + 4 + (12 + 1) * 2 + (1 + 1) + (1 + 1));

/// `enum SlotState` (an `Int` in a save: any value can be loaded).
pub mod slot_state {
    /// `SLOT_OPEN`.
    pub const OPEN: i32 = 0;
    /// `SLOT_CLOSED`.
    pub const CLOSED: i32 = 1;
    /// `SLOT_EASY_AI`.
    pub const EASY_AI: i32 = 2;
    /// `SLOT_MED_AI`.
    pub const MED_AI: i32 = 3;
    /// `SLOT_BRUTAL_AI`.
    pub const BRUTAL_AI: i32 = 4;
    /// `SLOT_PLAYER`.
    pub const PLAYER: i32 = 5;
}

/// `PLAYERTEMPLATE_RANDOM`.
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;
/// `PLAYERTEMPLATE_OBSERVER`.
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;
/// `PLAYERTEMPLATE_MIN`.
pub const PLAYERTEMPLATE_MIN: i32 = PLAYERTEMPLATE_OBSERVER;

/// `FirewallHelperClass::FIREWALL_MIN`.
pub const FIREWALL_MIN: i32 = 0;
/// `FirewallHelperClass::FIREWALL_TYPE_SIMPLE`.
pub const FIREWALL_TYPE_SIMPLE: i32 = 1;
/// `FirewallHelperClass::FIREWALL_MAX`.
pub const FIREWALL_MAX: i32 = 128;

/// What `MapCache` knows of a map (`MapMetaData`'s two fields this file reads).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapMeta {
    /// `m_numPlayers`.
    pub num_players: i32,
    /// `m_CRC`.
    pub crc: u32,
}

/// The world as `GameInfo.cpp` reaches it.
pub trait GameInfoHost {
    /// `TheGameText->fetch(label)`.
    fn game_text(&mut self, label: &str) -> Vec<u16>;
    /// `GetTickCount()` (the seed `reset` takes).
    fn tick_count(&mut self) -> u32;
    /// `TheGlobalData->m_defaultStartingCash`.
    fn default_starting_cash(&self) -> Money;
    /// `NET_CRC_INTERVAL` (a global of `Network.cpp`: 100 in a release build).
    fn net_crc_interval(&self) -> i32;
    /// `TheMapCache != NULL`.
    fn has_map_cache(&self) -> bool;
    /// `TheMapCache->find(name)`: the name exactly as given (the callers lowercase it first).
    fn map_cache_find(&self, lower_name: &[u8]) -> Option<MapMeta>;
    /// `TheFileSystem->openFile(path)` succeeded (the file is closed at once).
    fn file_exists(&mut self, path: &[u8]) -> bool;
    /// `TheGameState->realMapPathToPortableMapPath(path)`.
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8>;
    /// `TheGameState->portableMapPathToRealMapPath(path)`.
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8>;
    /// `TheMultiplayerSettings != NULL`.
    fn has_multiplayer_settings(&self) -> bool;
    /// `TheMultiplayerSettings->getNumColors()`.
    fn num_colors(&self) -> i32;
    /// `TheMultiplayerSettings->showRandomPlayerTemplate()`.
    fn show_random_player_template(&self) -> bool;
    /// `TheMultiplayerSettings->showRandomColor()`.
    fn show_random_color(&self) -> bool;
    /// `TheMultiplayerSettings->showRandomStartPos()`.
    fn show_random_start_pos(&self) -> bool;
    /// `TheMultiplayerSettings->getColor(PLAYERTEMPLATE_OBSERVER)->getColor()`.
    fn observer_color(&self) -> R<i32>;
    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    fn player_template_count(&self) -> i32;
    /// `ThePlayerTemplateStore->getNthPlayerTemplate(n)->getDisplayName()`.
    fn player_template_display_name(&self, n: i32) -> R<Vec<u16>>;
    /// `Money::deposit`'s `ThePlayerList->getNthPlayer(index)` and, when there is one, its academy's `recordIncome()`.
    fn record_income(&mut self, player_index: i32);
    /// `TheGameSpyGame && TheGameSpyGame->getConstSlot(0) == this slot` (only a `DEBUG_CRASH` follows: not observable).
    fn is_game_spy_host_slot(&self) -> bool {
        false
    }
}

/// `GameSlot`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSlot {
    pub state: i32,
    pub is_accepted: bool,
    pub has_map: bool,
    pub is_muted: bool,
    pub color: i32,
    pub start_pos: i32,
    pub player_template: i32,
    pub team_number: i32,
    pub orig_color: i32,
    pub orig_start_pos: i32,
    pub orig_player_template: i32,
    /// `m_name` (no terminating 0).
    pub name: Vec<u16>,
    /// `m_IP`: the constructor leaves it unset; the zero-filling allocator of every `GameInfo` the game makes zeroes it,
    /// and `setState` always sets it.
    pub ip: u32,
    pub port: u16,
    pub nat_behavior: i32,
    pub last_frame_in_game: u32,
    pub disconnected: bool,
}

impl Default for GameSlot {
    fn default() -> Self {
        GameSlot::new()
    }
}

/// Where a slot is in the global `TheGameInfo`, for the static `isSlotLocalAlly(slot)`.
#[derive(Clone, Copy, Debug)]
pub enum TheGameInfo<'a> {
    /// `TheGameInfo` is `NULL`: `getSlotIndex` dereferences it.
    Null,
    /// `TheGameInfo` and the slot's index in it (`None`: the slot is not one of its slots).
    Is(&'a GameInfo, Option<usize>),
}

impl GameSlot {
    /// `GameSlot()`: `reset()`, the name empty, the address 0.
    pub fn new() -> GameSlot {
        let mut s = GameSlot {
            state: 0,
            is_accepted: false,
            has_map: false,
            is_muted: false,
            color: 0,
            start_pos: 0,
            player_template: 0,
            team_number: 0,
            orig_color: 0,
            orig_start_pos: 0,
            orig_player_template: 0,
            name: Vec::new(),
            ip: 0,
            port: 0,
            nat_behavior: 0,
            last_frame_in_game: 0,
            disconnected: false,
        };
        s.reset();
        s
    }

    /// `reset()` (the name and the address stay).
    pub fn reset(&mut self) {
        self.state = slot_state::CLOSED;
        self.is_accepted = false;
        self.has_map = true;
        self.color = -1;
        self.start_pos = -1;
        self.player_template = -1;
        self.team_number = -1;
        self.nat_behavior = FIREWALL_TYPE_SIMPLE;
        self.last_frame_in_game = 0;
        self.disconnected = false;
        self.port = 0;
        self.is_muted = false;
        self.orig_player_template = -1;
        self.orig_start_pos = -1;
        self.orig_color = -1;
    }

    /// `saveOffOriginalInfo()`.
    pub fn save_off_original_info(&mut self) {
        self.orig_player_template = self.player_template;
        self.orig_start_pos = self.start_pos;
        self.orig_color = self.color;
    }

    /// `setAccept()`.
    pub fn set_accept(&mut self) {
        self.is_accepted = true;
    }

    /// `unAccept()`: a human slot only.
    pub fn un_accept(&mut self) {
        if self.is_human() {
            self.is_accepted = false;
        }
    }

    /// `setMapAvailability(hasMap)`: a human slot only.
    pub fn set_map_availability(&mut self, has_map: bool) {
        if self.is_human() {
            self.has_map = has_map;
        }
    }

    /// `setPlayerTemplate(t)`: an observer (or below) has no start position.
    pub fn set_player_template(&mut self, t: i32) {
        self.player_template = t;
        if t <= PLAYERTEMPLATE_MIN && !plant(3951) {
            self.start_pos = -1;
        }
    }

    /// `setState(state, name, IP)`.
    pub fn set_state(&mut self, state: i32, name: &[u16], ip: u32, host: &mut impl GameInfoHost) {
        let to_ai = state == slot_state::EASY_AI || state == slot_state::MED_AI || state == slot_state::BRUTAL_AI;
        if !(self.is_ai() && to_ai) {
            self.color = -1;
            self.start_pos = -1;
            self.player_template = -1;
            self.team_number = -1;
            let _ = state == slot_state::OPEN && host.is_game_spy_host_slot();
        }
        if state == slot_state::PLAYER {
            self.reset();
            self.state = state;
            self.name = name.to_vec();
        } else {
            self.state = state;
            self.is_accepted = true;
            self.has_map = true;
            let label = match state {
                slot_state::OPEN => "GUI:Open",
                slot_state::EASY_AI => "GUI:EasyAI",
                slot_state::MED_AI => "GUI:MediumAI",
                slot_state::BRUTAL_AI => "GUI:HardAI",
                _ => "GUI:Closed",
            };
            self.name = host.game_text(label);
        }
        self.ip = ip;
    }

    /// `isHuman()`.
    pub fn is_human(&self) -> bool {
        self.state == slot_state::PLAYER
    }
    /// `isOccupied()`.
    pub fn is_occupied(&self) -> bool {
        self.is_human() || self.is_ai()
    }
    /// `isAI()`.
    pub fn is_ai(&self) -> bool {
        self.state == slot_state::EASY_AI || self.state == slot_state::MED_AI || self.state == slot_state::BRUTAL_AI
    }
    /// `isOpen()`.
    pub fn is_open(&self) -> bool {
        self.state == slot_state::OPEN
    }
    /// `isPlayer(AsciiString)`: the name translated (`UnicodeString::translate`: each `char` sign-extended), compared
    /// without case.
    pub fn is_player_named_ascii(&self, user: &[u8]) -> bool {
        let u = translate(user);
        self.is_player_named(&u)
    }
    /// `isPlayer(UnicodeString)`.
    pub fn is_player_named(&self, user: &[u16]) -> bool {
        self.state == slot_state::PLAYER && compare_no_case_wide(&self.name, user)
    }
    /// `isPlayer(UnsignedInt ip)`.
    pub fn is_player_ip(&self, ip: u32) -> bool {
        self.state == slot_state::PLAYER && self.ip == ip
    }
    /// `disconnected()`.
    pub fn is_disconnected(&self) -> bool {
        self.is_human() && self.disconnected
    }

    /// `getApparentPlayerTemplateDisplayName()`.
    pub fn apparent_player_template_display_name(&self, the: TheGameInfo<'_>, host: &mut impl GameInfoHost) -> R<Vec<u16>> {
        if host.has_multiplayer_settings()
            && host.show_random_player_template()
            && self.orig_player_template == PLAYERTEMPLATE_RANDOM
            && !is_slot_local_ally(the, self)?
        {
            return Ok(host.game_text("GUI:Random"));
        } else if self.orig_player_template == PLAYERTEMPLATE_OBSERVER {
            return Ok(host.game_text("GUI:Observer"));
        }
        if self.player_template < 0 {
            return Ok(host.game_text("GUI:Random"));
        }
        host.player_template_display_name(self.player_template)
    }

    /// `getApparentPlayerTemplate()`.
    pub fn apparent_player_template(&self, the: TheGameInfo<'_>, host: &impl GameInfoHost) -> R<i32> {
        if host.has_multiplayer_settings() && host.show_random_player_template() && !is_slot_local_ally(the, self)? {
            return Ok(self.orig_player_template);
        }
        Ok(self.player_template)
    }

    /// `getApparentColor()`.
    pub fn apparent_color(&self, the: TheGameInfo<'_>, host: &impl GameInfoHost) -> R<i32> {
        if host.has_multiplayer_settings() && self.orig_player_template == PLAYERTEMPLATE_OBSERVER {
            return host.observer_color();
        }
        if host.has_multiplayer_settings() && host.show_random_color() && !is_slot_local_ally(the, self)? {
            return Ok(self.orig_color);
        }
        Ok(self.color)
    }

    /// `getApparentStartPos()`.
    pub fn apparent_start_pos(&self, the: TheGameInfo<'_>, host: &impl GameInfoHost) -> R<i32> {
        if host.has_multiplayer_settings() && host.show_random_start_pos() && !is_slot_local_ally(the, self)? {
            return Ok(self.orig_start_pos);
        }
        Ok(self.start_pos)
    }
}

/// `UnicodeString::translate(AsciiString)`: each `char` (signed) widened to a `WideChar`, up to a 0.
fn translate(s: &[u8]) -> Vec<u16> {
    s.iter().take_while(|&&b| b != 0).map(|&b| b as i8 as i16 as u16).collect()
}

/// `_wcsicmp(a, b) == 0` (ASCII letters folded, as `UnicodeString::compareNoCase`).
fn compare_no_case_wide(a: &[u16], b: &[u16]) -> bool {
    let low = |c: u16| if (u16::from(b'A')..=u16::from(b'Z')).contains(&c) { c + 32 } else { c };
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| low(x) == low(y))
}

/// `AsciiString::toLower()` of a map name: `strcpy` into a 2,048-byte buffer (a longer name overflows it), then
/// `tolower` of each `char` (undefined for a byte of 0x80 or more, a negative `char`).
fn to_lower(name: &[u8]) -> R<Vec<u8>> {
    if name.len() >= crate::common::ascii_string::MAX_FORMAT_BUF_LEN {
        return Err(IniError::undefined(Undefined::MessageOverflow));
    }
    if name.iter().any(|&b| b >= 0x80) {
        return Err(IniError::undefined(Undefined::TolowerNonAscii));
    }
    Ok(name.to_ascii_lowercase())
}

fn null_object() -> IniError {
    IniError::undefined(Undefined::NullObject)
}

/// The static `isSlotLocalAlly(slot)` (its slot index found in `TheGameInfo`).
fn is_slot_local_ally(the: TheGameInfo<'_>, slot: &GameSlot) -> R<bool> {
    let TheGameInfo::Is(game, slot_index) = the else {
        return Err(null_object());
    };
    let local_index = game.local_slot_num();
    // `localSlot` is fetched before the checks: NULL for a local index of -1, dereferenced only past them
    let slot_index = match slot_index {
        Some(i) if i < MAX_SLOTS => i as i32,
        _ => return Ok(false),
    };
    if local_index < 0 {
        return Ok(false);
    }
    if slot_index == local_index {
        return Ok(true);
    }
    let local = &game.slots[local_index as usize];
    if slot.team_number == local.team_number && slot.team_number >= 0 {
        return Ok(true);
    }
    Ok(if plant(3957) { local.player_template } else { local.orig_player_template } == PLAYERTEMPLATE_OBSERVER)
}

/// `GameInfo` (with its eight slots).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameInfo {
    pub preorder_mask: i32,
    pub crc_interval: i32,
    pub in_game: bool,
    pub in_progress: bool,
    pub surrendered: bool,
    pub game_id: i32,
    pub slots: [GameSlot; MAX_SLOTS],
    /// `m_localIP` (`reset` leaves it).
    pub local_ip: u32,
    pub map_name: Vec<u8>,
    pub map_crc: u32,
    pub map_size: u32,
    pub map_mask: i32,
    pub seed: i32,
    pub use_stats: i32,
    pub superweapon_restriction: u16,
    pub starting_cash: Money,
    pub old_factions_only: bool,
}

impl GameInfo {
    /// `GameInfo()` with its slots set (`setSlotPointer` for each of the eight), then `reset()`. `m_localIP` is the
    /// zero-filled allocator's 0.
    pub fn new(host: &mut impl GameInfoHost) -> GameInfo {
        let mut g = GameInfo {
            preorder_mask: 0,
            crc_interval: 0,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: Default::default(),
            local_ip: 0,
            map_name: Vec::new(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: 0,
            use_stats: 0,
            superweapon_restriction: 0,
            starting_cash: Money::new(),
            old_factions_only: false,
        };
        g.reset(host);
        g
    }

    /// `reset()`.
    pub fn reset(&mut self, host: &mut impl GameInfoHost) {
        self.crc_interval = host.net_crc_interval();
        self.in_game = false;
        self.in_progress = false;
        self.game_id = 0;
        self.map_name = b"NOMAP".to_vec();
        self.map_mask = 0;
        self.seed = host.tick_count() as i32;
        if plant(3958) {
            self.seed = 0;
        }
        self.use_stats = 1;
        self.surrendered = false;
        self.old_factions_only = false;
        self.map_crc = 0;
        self.map_size = 0;
        self.superweapon_restriction = 0;
        self.starting_cash = host.default_starting_cash();
        for s in self.slots.iter_mut() {
            s.reset();
        }
        self.preorder_mask = 0;
    }

    /// `isPlayerPreorder(index)`.
    pub fn is_player_preorder(&self, index: i32) -> bool {
        (0..MAX_SLOTS as i32).contains(&index) && self.preorder_mask & (1 << index) != 0
    }

    /// `markPlayerAsPreorder(index)`.
    pub fn mark_player_as_preorder(&mut self, index: i32) {
        if (0..MAX_SLOTS as i32).contains(&index) {
            self.preorder_mask |= 1 << index;
        }
    }

    /// `clearSlotList()`.
    pub fn clear_slot_list(&mut self, host: &mut impl GameInfoHost) {
        for s in self.slots.iter_mut() {
            s.set_state(slot_state::CLOSED, &[], 0, host);
        }
    }

    /// `getNumPlayers()`.
    pub fn num_players(&self) -> i32 {
        self.slots.iter().filter(|s| s.is_occupied()).count() as i32
    }

    /// `getNumNonObserverPlayers()`.
    pub fn num_non_observer_players(&self) -> i32 {
        self.slots.iter().filter(|s| s.is_occupied() && s.player_template != PLAYERTEMPLATE_OBSERVER).count() as i32
    }

    /// `getMaxPlayers()`.
    pub fn max_players(&self, host: &impl GameInfoHost) -> R<i32> {
        if !host.has_map_cache() {
            return Ok(-1);
        }
        Ok(host.map_cache_find(&to_lower(&self.map_name)?).map_or(-1, |m| m.num_players))
    }

    /// `enterGame()`.
    pub fn enter_game(&mut self, host: &mut impl GameInfoHost) {
        self.reset(host);
        self.in_game = true;
        self.in_progress = false;
    }

    /// `leaveGame()`.
    pub fn leave_game(&mut self, host: &mut impl GameInfoHost) {
        self.reset(host);
    }

    /// `startGame(gameID)`.
    pub fn start_game(&mut self, game_id: i32, host: &mut impl GameInfoHost) {
        self.game_id = game_id;
        self.close_open_slots(host);
        self.in_progress = true;
    }

    /// `endGame()`.
    pub fn end_game(&mut self) {
        self.in_game = false;
        self.in_progress = false;
    }

    /// `setSlot(slotNum, slotInfo)`: slot 0 is always accepted and has the map.
    pub fn set_slot(&mut self, slot_num: i32, mut slot: GameSlot) {
        if !(0..MAX_SLOTS as i32).contains(&slot_num) {
            return;
        }
        if slot_num == 0 {
            slot.set_accept();
            slot.set_map_availability(true);
        }
        self.slots[slot_num as usize] = slot;
    }

    /// `getSlot(slotNum)`.
    pub fn slot(&self, slot_num: i32) -> Option<&GameSlot> {
        usize::try_from(slot_num).ok().and_then(|i| self.slots.get(i))
    }

    /// `getSlot(slotNum)`, mutable.
    pub fn slot_mut(&mut self, slot_num: i32) -> Option<&mut GameSlot> {
        usize::try_from(slot_num).ok().and_then(|i| self.slots.get_mut(i))
    }

    /// `getLocalSlotNum()`: the first human slot at the local address, or -1 (and -1 when not in a game).
    pub fn local_slot_num(&self) -> i32 {
        if !self.in_game {
            return -1;
        }
        self.slots.iter().position(|s| s.is_player_ip(self.local_ip)).map_or(-1, |i| i as i32)
    }

    /// `getSlotNum(userName)`.
    pub fn slot_num(&self, user: &[u8]) -> i32 {
        if !self.in_game {
            return -1;
        }
        let u = translate(user);
        self.slots.iter().position(|s| s.is_player_named(&u)).map_or(-1, |i| i as i32)
    }

    /// `amIHost()`.
    pub fn am_i_host(&self) -> bool {
        self.in_game && self.slots[0].is_player_ip(self.local_ip)
    }

    /// `setMap(mapName)`: in a game this host leads, the contents mask from what the map cache and the file system have.
    pub fn set_map(&mut self, map_name: &[u8], host: &mut impl GameInfoHost) -> R<()> {
        self.map_name = map_name.to_vec();
        if !(self.in_game && self.am_i_host()) {
            return Ok(());
        }
        if !host.has_map_cache() {
            return Err(null_object());
        }
        if host.map_cache_find(&to_lower(map_name)?).is_none() {
            self.map_mask = 0;
            return Ok(());
        }
        self.map_mask = 1;
        let mut path = map_name.to_vec();
        for _ in 0..3 {
            path.pop();
        }
        path.extend_from_slice(b"tga");
        if host.file_exists(&path) {
            self.map_mask |= 2;
        }
        // the directories of the name, joined by '/', then "/map.ini" (`nextToken` on "\\/" while a '\\' is left)
        let mut new_map_name: Vec<u8> = Vec::new();
        if !map_name.is_empty() {
            let mut rest = map_name.to_vec();
            let mut token = next_token(&mut rest, b"\\/");
            while rest.contains(&b'\\') {
                if !new_map_name.is_empty() {
                    new_map_name.push(b'/');
                }
                new_map_name.extend_from_slice(&token);
                token = next_token(&mut rest, b"\\/");
            }
        }
        new_map_name.extend_from_slice(b"/map.ini");
        if host.file_exists(&new_map_name) {
            self.map_mask |= 4;
        }
        for (bit, file) in [(8, &b"map.str"[..]), (16, b"solo.ini"), (32, b"assetusage.txt"), (64, b"readme.txt")] {
            let mut p = base_path_from_path(&self.map_name);
            p.push(b'\\');
            p.extend_from_slice(file);
            if host.file_exists(&p) {
                self.map_mask |= bit;
            }
        }
        Ok(())
    }

    /// `setMapContentsMask(mask)`.
    pub fn set_map_contents_mask(&mut self, mask: i32) {
        self.map_mask = mask;
    }

    fn update_local_map_availability(&mut self, host: &impl GameInfoHost) -> R<()> {
        if !host.has_map_cache() {
            return Ok(());
        }
        if self.in_game && self.local_slot_num() >= 0 {
            let has = match host.map_cache_find(&to_lower(&self.map_name)?) {
                None => false,
                Some(m) => self.map_crc == m.crc,
            };
            let local = self.local_slot_num() as usize;
            self.slots[local].set_map_availability(has);
        }
        Ok(())
    }

    /// `setMapCRC(crc)`: in a game with a local slot, whether that slot has the map, from the map cache.
    pub fn set_map_crc(&mut self, crc: u32, host: &impl GameInfoHost) -> R<()> {
        self.map_crc = if plant(3953) { crc ^ 1 } else { crc };
        self.update_local_map_availability(host)
    }

    /// `setMapSize(size)`: the same test as `setMapCRC` (it compares the CRC, not the size).
    pub fn set_map_size(&mut self, size: u32, host: &impl GameInfoHost) -> R<()> {
        self.map_size = size;
        self.update_local_map_availability(host)
    }

    /// `setCRCInterval(val)`: at most 100.
    pub fn set_crc_interval(&mut self, val: i32) {
        self.crc_interval = if val < 100 { val } else { 100 };
    }

    /// `isColorTaken(colorIdx, slotToIgnore)`.
    pub fn is_color_taken(&self, color: i32, slot_to_ignore: i32) -> bool {
        self.slots.iter().enumerate().any(|(i, s)| s.color == color && i as i32 != slot_to_ignore)
    }

    /// `isStartPositionTaken(positionIdx, slotToIgnore)`.
    pub fn is_start_position_taken(&self, pos: i32, slot_to_ignore: i32) -> bool {
        self.slots.iter().enumerate().any(|(i, s)| s.start_pos == pos && i as i32 != slot_to_ignore)
    }

    /// `resetAccepted()`.
    pub fn reset_accepted(&mut self) {
        self.slots[0].set_accept();
        for s in self.slots.iter_mut().skip(1) {
            s.un_accept();
        }
    }

    /// `resetStartSpots()`.
    pub fn reset_start_spots(&mut self) {
        for s in self.slots.iter_mut() {
            s.start_pos = -1;
        }
    }

    /// `adjustSlotsForMap()`: unoccupied slots opened while the map has room, closed after.
    pub fn adjust_slots_for_map(&mut self, host: &mut impl GameInfoHost) -> R<()> {
        if !host.has_map_cache() {
            return Err(null_object());
        }
        let Some(md) = host.map_cache_find(&to_lower(&self.map_name)?) else {
            return Ok(());
        };
        let num_players = md.num_players;
        let mut num_player_slots = self.slots.iter().filter(|s| s.is_occupied()).count() as i32;
        for i in 0..MAX_SLOTS {
            if self.slots[i].is_occupied() {
                continue;
            }
            let mut new_slot = GameSlot::new();
            if num_players > num_player_slots {
                new_slot.set_state(slot_state::OPEN, &[], 0, host);
                self.set_slot(i as i32, new_slot);
                num_player_slots += 1;
            } else {
                new_slot.set_state(slot_state::CLOSED, &[], 0, host);
                self.set_slot(i as i32, new_slot);
            }
        }
        Ok(())
    }

    /// `closeOpenSlots()`.
    pub fn close_open_slots(&mut self, host: &mut impl GameInfoHost) {
        for i in 0..MAX_SLOTS {
            if !self.slots[i].is_occupied() && !(plant(3959) && self.slots[i].is_open()) {
                let mut new_slot = GameSlot::new();
                new_slot.set_state(slot_state::CLOSED, &[], 0, host);
                self.set_slot(i as i32, new_slot);
            }
        }
    }

    /// `isSkirmish()`: no other human, and an AI none of which is the local player's ally by the static
    /// `isSlotLocalAlly` (which looks in `TheGameInfo`; `the` is where this game's slot `i` is there, by index).
    pub fn is_skirmish<'a>(&self, the: impl Fn(usize) -> TheGameInfo<'a>) -> R<bool> {
        let mut saw_ai = false;
        for i in 0..MAX_SLOTS {
            if i as i32 == self.local_slot_num() {
                continue;
            }
            let s = &self.slots[i];
            if s.is_human() {
                return Ok(false);
            }
            if s.is_ai() {
                if is_slot_local_ally(the(i), s)? {
                    return Ok(false);
                }
                saw_ai = true;
            }
        }
        Ok(saw_ai)
    }

    /// `isMultiPlayer()`.
    pub fn is_multi_player(&self) -> bool {
        let local = self.local_slot_num();
        self.slots.iter().enumerate().any(|(i, s)| i as i32 != local && s.is_human())
    }

    /// `isSandbox()`: the local slot is dereferenced unchecked (a local slot of -1 is `NULL`).
    pub fn is_sandbox(&self) -> R<bool> {
        let local = self.local_slot_num();
        let Some(ls) = self.slot(local) else {
            return Err(null_object());
        };
        let local_team = ls.team_number;
        for (i, s) in self.slots.iter().enumerate() {
            if i as i32 == local {
                continue;
            }
            if s.is_occupied() && (s.team_number < 0 || s.team_number != local_team) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// `AsciiString::nextToken(&token, seps)` on a byte string: separators skipped, the token taken, the rest kept (both
/// cleared when only separators are left; nothing changes on an empty string).
fn next_token(s: &mut Vec<u8>, seps: &[u8]) -> Vec<u8> {
    if s.is_empty() {
        return Vec::new();
    }
    let start = s.iter().position(|b| !seps.contains(b)).unwrap_or(s.len());
    let end = start + s[start..].iter().position(|b| seps.contains(b)).unwrap_or(s.len() - start);
    if end > start {
        let token = s[start..end].to_vec();
        *s = s[end..].to_vec();
        token
    } else {
        s.clear();
        Vec::new()
    }
}

/// `GetBasePathFromPath(path)`: up to the last '\\', or empty.
pub fn base_path_from_path(path: &[u8]) -> Vec<u8> {
    match path.iter().rposition(|&b| b == b'\\') {
        Some(i) => path[..i].to_vec(),
        None => Vec::new(),
    }
}

/// `WideCharStringToMultiByte(s)`: UTF-16 to UTF-8 (`WideCharToMultiByte(CP_UTF8)`), a lone surrogate as U+FFFD.
pub fn wide_to_utf8(s: &[u16]) -> Vec<u8> {
    String::from_utf16_lossy(s).into_bytes()
}

/// `MultiByteToWideCharSingleLine(s)`: UTF-8 to UTF-16 (`MultiByteToWideChar(CP_UTF8)`, an ill-formed sequence as
/// U+FFFD), each '\n' and '\r' a space, cut at a 0.
pub fn utf8_to_wide_single_line(s: &[u8]) -> Vec<u16> {
    String::from_utf8_lossy(s)
        .encode_utf16()
        .take_while(|&c| c != 0)
        .map(|c| if c == u16::from(b'\n') || c == u16::from(b'\r') { u16::from(b' ') } else { c })
        .collect()
}

fn hex_upper(v: u32) -> String {
    format!("{v:X}")
}

/// `GameInfoToAsciiString(game)`. Undefined upstream: options before the slot list of 2,047 characters or more
/// (`AsciiString::format`'s `_vsnprintf` into its 2,048-byte buffer then throws `ERROR_OUT_OF_MEMORY`, or, at exactly
/// 2,047, leaves the string unterminated), and a human slot whose share of the remaining length is negative (the name
/// is cut a character at a time while longer than it: an empty name never is shorter, so the loop never ends).
pub fn game_info_to_ascii_string(game: Option<&GameInfo>, host: &impl GameInfoHost) -> R<Vec<u8>> {
    let Some(game) = game else {
        return Ok(Vec::new());
    };
    let mut map_name = host.real_map_path_to_portable(&game.map_name);
    let mut new_map_name: Vec<u8> = Vec::new();
    if !map_name.is_empty() {
        let mut token = next_token(&mut map_name, b"\\/");
        while map_name.contains(&b'\\') {
            if !new_map_name.is_empty() {
                new_map_name.push(b'/');
            }
            new_map_name.extend_from_slice(&token);
            token = next_token(&mut map_name, b"\\/");
        }
    }
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(
        format!(
            "US={};M={:02x}",
            game.use_stats,
            game.map_mask as u32
        )
        .as_bytes(),
    );
    out.extend_from_slice(&new_map_name);
    out.extend_from_slice(
        format!(
            ";MC={};MS={};SD={};C={};SR={};SC={};O={};",
            hex_upper(game.map_crc),
            game.map_size as i32,
            game.seed,
            game.crc_interval,
            game.superweapon_restriction,
            game.starting_cash.count_money(),
            if game.old_factions_only { 'Y' } else { 'N' }
        )
        .as_bytes(),
    );
    if out.len() >= crate::common::ascii_string::MAX_FORMAT_BUF_LEN - 1 {
        return Err(IniError::undefined(Undefined::MessageOverflow));
    }
    out.extend_from_slice(b"S=");
    for (i, slot) in game.slots.iter().enumerate() {
        let s: Vec<u8> = if slot.is_human() {
            let tmp = format!(
                ",{},{},{}{},{},{},{},{},{}:",
                hex_upper(slot.ip),
                slot.port,
                if slot.is_accepted { 'T' } else { 'F' },
                if slot.has_map { 'T' } else { 'F' },
                slot.color,
                slot.player_template,
                slot.start_pos,
                slot.team_number,
                slot.nat_behavior
            );
            let len_cur = tmp.len() as i32 + out.len() as i32 + 2;
            let len_rem = LAN_MAX_OPTIONS_LENGTH - len_cur;
            let len_max = len_rem / (MAX_SLOTS - i + usize::from(plant(3954))) as i32;
            if len_max < 0 {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            // `AsciiString(...c_str())` stops at a 0; the name is cut a byte at a time while longer than the share
            let mut name: Vec<u8> = wide_to_utf8(&slot.name).into_iter().take_while(|&b| b != 0).collect();
            while name.len() as i32 > len_max {
                name.pop();
            }
            let mut v = b"H".to_vec();
            v.extend_from_slice(&name);
            v.extend_from_slice(tmp.as_bytes());
            v
        } else if slot.is_ai() {
            let c = match slot.state {
                slot_state::EASY_AI => 'E',
                slot_state::MED_AI => 'M',
                _ => 'H',
            };
            format!("C{c},{},{},{},{}:", slot.color, slot.player_template, slot.start_pos, slot.team_number).into_bytes()
        } else if slot.state == slot_state::OPEN {
            b"O:".to_vec()
        } else {
            b"X:".to_vec()
        };
        out.extend_from_slice(&s);
    }
    out.push(b';');
    Ok(out)
}

/// Westwood's `strtok_r(strptr, delimiters, lasts)` on a buffer with 0 terminators: the token's start, or `None`;
/// `lasts` is an index into `buf`.
fn ww_strtok_r(buf: &mut [u8], strptr: Option<usize>, delims: &[u8], lasts: &mut usize) -> Option<usize> {
    if let Some(p) = strptr {
        *lasts = p;
    }
    let at = |i: usize| buf.get(i).copied().unwrap_or(0);
    if at(*lasts) == 0 {
        return None;
    }
    let cspn = |buf: &[u8], from: usize| (from..).take_while(|&i| buf.get(i).copied().unwrap_or(0) != 0 && !delims.contains(&buf[i])).count();
    let spn = |buf: &[u8], from: usize| (from..).take_while(|&i| buf.get(i).copied().unwrap_or(0) != 0 && delims.contains(&buf[i])).count();
    let mut dstart = cspn(buf, *lasts);
    if dstart == 0 {
        let dend = spn(buf, *lasts);
        *lasts += dend;
        if at(*lasts) == 0 {
            return None;
        }
        dstart = cspn(buf, *lasts);
    }
    let ret = *lasts;
    if at(*lasts + dstart) == 0 {
        *lasts += dstart;
    } else {
        buf[*lasts + dstart] = 0;
        *lasts += dstart + 1;
    }
    Some(ret)
}

/// The 0-terminated string at `at` (or empty for `None`: `AsciiString(NULL)`).
fn c_string(buf: &[u8], at: Option<usize>) -> Vec<u8> {
    match at {
        None => Vec::new(),
        Some(i) => buf[i..].iter().copied().take_while(|&b| b != 0).collect(),
    }
}

/// `atoi`: blanks, a sign, digits; a value outside `Int` is undefined.
fn atoi(s: &[u8]) -> R<i32> {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let neg = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v * 10 + i64::from(s[i] - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return Err(IniError::undefined(Undefined::IntOutOfRange));
        }
        i += 1;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).map_err(|_| IniError::undefined(Undefined::IntOutOfRange))
}

/// `strtol`/`strtoul`-style digits after blanks and a sign: the magnitude, the sign, and whether any digit was read.
fn scan_digits(s: &[u8], radix: u32, allow_0x: bool) -> (u128, bool, bool) {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let neg = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    if allow_0x && i + 1 < s.len() && s[i] == b'0' && (s[i + 1] == b'x' || s[i + 1] == b'X') && s.get(i + 2).is_some_and(|c| (*c as char).is_ascii_hexdigit()) {
        i += 2;
    }
    let mut v: u128 = 0;
    let mut any = false;
    while i < s.len() {
        let Some(d) = (s[i] as char).to_digit(radix) else { break };
        v = (v * u128::from(radix) + u128::from(d)).min(u128::from(u64::MAX) * 16);
        any = true;
        i += 1;
    }
    (v, neg, any)
}

/// `sscanf(s, "%x", &UnsignedInt)` (also `%X`): the variable keeps `init` when nothing matches; a magnitude above
/// `UnsignedInt` is undefined; a minus sign negates modulo 2^32.
fn scan_hex_u32(s: &[u8], init: u32) -> R<u32> {
    let (v, neg, any) = scan_digits(s, 16, true);
    if !any {
        return Ok(init);
    }
    if v > u128::from(u32::MAX) {
        return Err(IniError::undefined(Undefined::UnsignedOutOfRange));
    }
    let v = v as u32;
    Ok(if neg { v.wrapping_neg() } else { v })
}

/// `sscanf(s, "%d", &UnsignedInt)`: an `Int` stored into the unsigned variable (the variable keeps `init` when nothing
/// matches; a value outside `Int` is undefined).
fn scan_dec_into_u32(s: &[u8], init: u32) -> R<u32> {
    let (v, neg, any) = scan_digits(s, 10, false);
    if !any {
        return Ok(init);
    }
    let v = if neg { -(v as i128) } else { v as i128 };
    i32::try_from(v).map(|x| x as u32).map_err(|_| IniError::undefined(Undefined::IntOutOfRange))
}

/// `strtoul(s, NULL, 10)` into an `UnsignedInt`: a magnitude above `UnsignedInt` is platform dependent (clamped on
/// Windows), reported as undefined; a minus sign negates.
fn strtoul_u32(s: &[u8]) -> R<u32> {
    let (v, neg, _) = scan_digits(s, 10, false);
    if v > u128::from(u32::MAX) {
        return Err(IniError::undefined(Undefined::UnsignedOutOfRange));
    }
    let v = v as u32;
    Ok(if neg { v.wrapping_neg() } else { v })
}

/// `grabHexInt(s)`: `strtol("0x" s[0] s[1], NULL, 16)`.
fn grab_hex_int(s: &[u8]) -> i32 {
    let t = [b'0', b'x', s.first().copied().unwrap_or(0), if plant(3956) { 0 } else { s.get(1).copied().unwrap_or(0) }];
    let t: Vec<u8> = t.iter().copied().take_while(|&b| b != 0).collect();
    let (v, neg, any) = scan_digits(&t, 16, true);
    if !any {
        return 0;
    }
    let v = v as i64;
    (if neg { -v } else { v }) as i32
}

/// `ParseAsciiStringToGameInfo(game, options)`: true when the string is a complete, valid set of options; then, when
/// `game` is given, its slots and options are set from it.
pub fn parse_ascii_string_to_game_info(game: Option<&mut GameInfo>, options: &[u8], host: &mut impl GameInfoHost) -> R<bool> {
    let mut buf: Vec<u8> = options.iter().copied().take_while(|&b| b != 0).collect();
    buf.push(0);
    let mut new_slot: Vec<GameSlot> = (0..MAX_SLOTS).map(|_| GameSlot::new()).collect();
    let mut options_ok = true;
    let mut map_name: Vec<u8> = Vec::new();
    let mut map_contents_mask: Option<i32> = None;
    let (mut map_crc, mut map_size): (Option<u32>, Option<u32>) = (None, None);
    let mut seed = 0i32;
    let mut crc = 100i32;
    let mut old_factions_only = false;
    let mut use_stats = 1i32;
    let mut starting_cash = host.default_starting_cash();
    let mut restriction: u16 = 0;
    let (mut saw_map, mut saw_map_crc, mut saw_map_size, mut saw_seed, mut saw_slotlist) = (false, false, false, false, false);
    let (mut saw_use_stats, mut saw_sr, mut saw_sc, mut saw_old, mut saw_crc) = (false, false, false, false, false);

    let mut str_pos = 0usize;
    let mut buf_ptr = Some(0usize);
    while let Some(kv) = ww_strtok_r(&mut buf, buf_ptr, b";", &mut str_pos) {
        buf_ptr = None;
        let mut pos = 0usize;
        let key_ptr = ww_strtok_r(&mut buf, Some(kv), b"=", &mut pos);
        let val_ptr = ww_strtok_r(&mut buf, None, b"\n", &mut pos);
        let key = c_string(&buf, key_ptr);
        let val = c_string(&buf, val_ptr);
        if val.is_empty() {
            options_ok = false;
            break;
        }
        match key.as_slice() {
            b"US" => {
                use_stats = atoi(&val)?;
                saw_use_stats = true;
            }
            b"M" => {
                if val.len() < 3 {
                    options_ok = false;
                    break;
                }
                map_contents_mask = Some(grab_hex_int(&val));
                let mut tempstr = val[2..].to_vec();
                let mut token = next_token(&mut tempstr, b"\\/");
                while !tempstr.is_empty() {
                    map_name.extend_from_slice(&token);
                    map_name.push(b'\\');
                    token = next_token(&mut tempstr, b"\\/");
                }
                map_name.extend_from_slice(&token);
                map_name.push(b'\\');
                map_name.extend_from_slice(&token);
                map_name.push(b'.');
                if !host.has_map_cache() {
                    return Err(null_object());
                }
                map_name.extend_from_slice(b"map");
                map_name = host.portable_map_path_to_real(&map_name);
                saw_map = true;
            }
            b"MC" => {
                map_crc = Some(scan_hex_u32(&val, 0)?);
                saw_map_crc = true;
            }
            b"MS" => {
                map_size = Some(atoi(&val)? as u32);
                saw_map_size = true;
            }
            b"SD" => {
                seed = atoi(&val)?;
                saw_seed = true;
            }
            b"C" => {
                crc = atoi(&val)?;
                saw_crc = true;
            }
            b"SR" => {
                restriction = atoi(&val)? as u16;
                saw_sr = true;
            }
            b"SC" => {
                let amount = strtoul_u32(&val)?;
                starting_cash.init();
                let mut events = Vec::new();
                if starting_cash.deposit(amount, false, &mut events) {
                    host.record_income(starting_cash.player_index());
                }
                saw_sc = true;
            }
            b"O" => {
                old_factions_only = val.len() == 1 && val[0].eq_ignore_ascii_case(&b'y');
                saw_old = true;
            }
            _ if key.len() == 1 && key[0] == b'S' => {
                saw_slotlist = true;
                let mut raw_buf = val.clone();
                raw_buf.push(0);
                let mut first = Some(0usize);
                for (i, slot) in new_slot.iter_mut().enumerate() {
                    let raw_at = ww_strtok_r(&mut raw_buf, first, b":", &mut pos);
                    first = None;
                    let mut raw_slot = c_string(&raw_buf, raw_at);
                    raw_slot.push(0);
                    if !parse_slot(&mut raw_slot, i, slot, host)? {
                        options_ok = false;
                    }
                }
            }
            _ => {
                options_ok = false;
                break;
            }
        }
    }

    if options_ok && saw_map && saw_map_crc && saw_map_size && saw_seed && saw_slotlist && saw_crc && saw_use_stats && saw_sr && saw_sc && saw_old {
        let Some(game) = game else {
            return Ok(true);
        };
        for (i, s) in new_slot.into_iter().enumerate() {
            game.set_slot(i as i32, s);
        }
        game.set_map(&map_name, host)?;
        game.set_map_crc(map_crc.unwrap_or(0), host)?;
        game.set_map_size(map_size.unwrap_or(0), host)?;
        game.set_map_contents_mask(map_contents_mask.unwrap_or(0));
        game.seed = seed;
        game.set_crc_interval(crc);
        game.use_stats = use_stats;
        game.superweapon_restriction = restriction;
        game.starting_cash = starting_cash;
        game.old_factions_only = old_factions_only;
        return Ok(true);
    }
    Ok(false)
}

/// `TheMultiplayerSettings->getNumColors()` (dereferenced unchecked).
fn num_colors(host: &impl GameInfoHost) -> R<i32> {
    if !host.has_multiplayer_settings() {
        return Err(null_object());
    }
    Ok(host.num_colors())
}

/// `AsciiString(strtok_r(start, ",", &slotPos))`.
fn tok(raw: &mut [u8], start: Option<usize>, sp: &mut usize) -> Vec<u8> {
    let at = ww_strtok_r(raw, start, b",", sp);
    c_string(raw, at)
}

/// One slot of the `S=` list (`raw` 0-terminated): false where upstream sets `optionsOk = false` (each `break` leaves the
/// `switch`, so the next slot is read either way).
fn parse_slot(raw: &mut [u8], i: usize, slot: &mut GameSlot, host: &mut impl GameInfoHost) -> R<bool> {
    let mut sp = 0usize;
    let first = raw[0];
    match first {
        b'H' => {
            let v = tok(raw, Some(0), &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let name = utf8_to_wide_single_line(&v[1..]);
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let ip = scan_hex_u32(&v, 0)?;
            slot.set_state(slot_state::PLAYER, &name, ip, host);
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let port = scan_dec_into_u32(&v, 0)?;
            slot.port = port as u16;
            let v = tok(raw, None, &mut sp);
            if v.len() != 2 {
                return Ok(false);
            }
            if v[0] == b'T' {
                slot.set_accept();
            } else if v[0] == b'F' {
                slot.un_accept();
            }
            slot.set_map_availability(v[1] == b'T');
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let color = atoi(&v)?;
            if color < -1 || color >= num_colors(host)? {
                return Ok(false);
            }
            slot.color = color;
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let t = atoi(&v)?;
            if t < PLAYERTEMPLATE_MIN || t >= host.player_template_count() {
                return Ok(false);
            }
            slot.set_player_template(t);
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let start = atoi(&v)?;
            if start < -1 || start >= MAX_SLOTS as i32 {
                return Ok(false);
            }
            slot.start_pos = start;
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let team = atoi(&v)?;
            if team < -1 || team >= MAX_SLOTS as i32 / 2 {
                return Ok(false);
            }
            slot.team_number = team;
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let nat = atoi(&v)?;
            if !(FIREWALL_MIN..=FIREWALL_MAX).contains(&nat) {
                return Ok(false);
            }
            slot.nat_behavior = nat;
            Ok(true)
        }
        b'C' => {
            let v = tok(raw, Some(0), &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let mut ok = true;
            match v.get(1).copied().unwrap_or(0) {
                b'E' => slot.set_state(slot_state::EASY_AI, &[], 0, host),
                b'M' => slot.set_state(slot_state::MED_AI, &[], 0, host),
                b'H' => slot.set_state(slot_state::BRUTAL_AI, &[], 0, host),
                _ => ok = false,
            }
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let color = atoi(&v)?;
            if color < -1 || color >= num_colors(host)? {
                return Ok(false);
            }
            slot.color = color;
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let t = atoi(&v)?;
            if t < PLAYERTEMPLATE_MIN || t >= host.player_template_count() {
                return Ok(false);
            }
            slot.set_player_template(t);
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let start = atoi(&v)?;
            let mut bad = !(-1..MAX_SLOTS as i32).contains(&start);
            // upstream compares with the slot itself (`newSlot[i]`, not `newSlot[j]`), once for each earlier slot
            for _j in 0..i {
                if start >= 0 && start == slot.start_pos {
                    bad = true;
                }
            }
            if bad {
                return Ok(false);
            }
            slot.start_pos = start;
            let v = tok(raw, None, &mut sp);
            if v.is_empty() {
                return Ok(false);
            }
            let team = atoi(&v)?;
            if team < -1 || (team >= MAX_SLOTS as i32 / 2 && !(plant(3952) && team == MAX_SLOTS as i32 / 2)) {
                return Ok(false);
            }
            slot.team_number = team;
            Ok(ok)
        }
        b'O' => {
            slot.set_state(slot_state::OPEN, &[], 0, host);
            Ok(true)
        }
        b'X' => {
            slot.set_state(slot_state::CLOSED, &[], 0, host);
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// `SkirmishGameInfo::xfer` (version 4) of a game's options and slots; `map_name` goes through `Xfer::xferMapName`
/// (saved as the portable path, loaded back to the real one, and **not** in a CRC: `xferMapName` acts only on save and
/// load).
pub struct SkirmishXfer<'a, H: GameInfoHost> {
    pub game: &'a mut GameInfo,
    pub host: &'a mut H,
}

impl<H: GameInfoHost> Snapshot for SkirmishXfer<'_, H> {
    /// `crc`: empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let g = &mut *self.game;
        let current: XferVersion = 4;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_int(&mut g.preorder_mask)?;
        xfer.xfer_int(&mut g.crc_interval)?;
        xfer.xfer_bool(&mut g.in_game)?;
        xfer.xfer_bool(&mut g.in_progress)?;
        xfer.xfer_bool(&mut g.surrendered)?;
        xfer.xfer_int(&mut g.game_id)?;
        let mut count = MAX_SLOTS as i32;
        xfer.xfer_int(&mut count)?;
        let loading = xfer.get_xfer_mode() == XferMode::Load;
        for s in g.slots.iter_mut() {
            let mut state = s.state;
            xfer.xfer_int(&mut state)?;
            let mut name = s.name.clone();
            if version >= 2 {
                xfer.xfer_unicode_string(&mut name)?;
            }
            let mut accepted = s.is_accepted;
            xfer.xfer_bool(&mut accepted)?;
            let mut muted = s.is_muted;
            xfer.xfer_bool(&mut muted)?;
            s.is_muted = muted;
            let mut color = s.color;
            xfer.xfer_int(&mut color)?;
            let mut start_pos = s.start_pos;
            xfer.xfer_int(&mut start_pos)?;
            let mut template = s.player_template;
            xfer.xfer_int(&mut template)?;
            let mut team = s.team_number;
            xfer.xfer_int(&mut team)?;
            let mut orig_color = s.orig_color;
            xfer.xfer_int(&mut orig_color)?;
            let mut orig_start = s.orig_start_pos;
            xfer.xfer_int(&mut orig_start)?;
            let mut orig_template = s.orig_player_template;
            xfer.xfer_int(&mut orig_template)?;
            if loading {
                let name: Vec<u16> = name.iter().copied().take_while(|&c| c != 0).collect();
                s.set_state(state, &name, 0, self.host);
                if accepted && !plant(3955) {
                    s.set_accept();
                }
                s.set_player_template(orig_template);
                s.start_pos = orig_start;
                s.color = orig_color;
                s.save_off_original_info();
                s.team_number = team;
                s.color = color;
                s.start_pos = start_pos;
                s.set_player_template(template);
            }
        }
        xfer.xfer_unsigned_int(&mut g.local_ip)?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let mut tmp = self.host.real_map_path_to_portable(&g.map_name);
                xfer.xfer_ascii_string(&mut tmp)?;
            }
            XferMode::Load => {
                xfer.xfer_ascii_string(&mut g.map_name)?;
                g.map_name = self.host.portable_map_path_to_real(&g.map_name);
            }
            _ => {}
        }
        xfer.xfer_unsigned_int(&mut g.map_crc)?;
        xfer.xfer_unsigned_int(&mut g.map_size)?;
        xfer.xfer_int(&mut g.map_mask)?;
        xfer.xfer_int(&mut g.seed)?;
        if version >= 3 {
            xfer.xfer_unsigned_short(&mut g.superweapon_restriction)?;
            if version == 3 {
                // version 3's bool, gone since: read into a local and dropped
                let mut obsolete = false;
                xfer.xfer_bool(&mut obsolete)?;
            }
            xfer.xfer_snapshot(&mut g.starting_cash)?;
        } else if loading {
            g.superweapon_restriction = 0;
            g.starting_cash = self.host.default_starting_cash();
        }
        Ok(())
    }

    /// `loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}
