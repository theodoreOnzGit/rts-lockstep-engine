// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp (the file's
//     statics and globals, getNextSelectablePlayer, SkirmishPreferences (getSlotList, setSlotList, getUserName,
//     getPreferredColor, getPreferredFaction, usesSystemMapDir, getPreferredMap, getSuperweaponRestricted,
//     setSuperweaponRestricted, getStartingCash, setStartingCash, write), setFPSTextBox, reallyDoStart,
//     cancelStartBecauseOfNoCD, IsFirstCDPresent, checkCDCallback, CheckForCDAtGameStart, startPressed,
//     MapSelectorTooltip, positionStartSpotControls, positionAdditionalImages, positionStartSpots (both),
//     updateMapStartSpots, handlePlayerSelection, handleColorSelection, handlePlayerTemplateSelection,
//     handleStartPositionSelection, handleTeamSelection, handleStartingCashSelection, handleLimitSuperweaponsClick,
//     InitSkirmishGameGadgets, skirmishUpdateSlotList, skirmishPositionStartSpots, initSkirmishGameOptions,
//     updateSkirmishGameOptions, SkirmishGameOptionsMenuInit, shutdownComplete, SkirmishGameOptionsMenuShutdown,
//     SkirmishGameOptionsMenuUpdate, SkirmishGameOptionsMenuInput, SkirmishGameOptionsMenuSystem)
//   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (UserPreferences::getInt, getAsciiString, setInt,
//     setAsciiString, intAsStr)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanLobbyMenu.cpp
//     (LANPreferences::usesSystemMapDir)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The skirmish game-options screen (GitHub #89): `Menus/SkirmishGameOptionsMenu.wnd`'s callbacks, with the start-spot
//! and map-preview placement the map picker shares (`positionStartSpots`) and `TheSkirmishGameInfo`.
//!
//! # As upstream reads it
//!
//! **Init** makes `TheSkirmishGameInfo` (or ends the game it holds), enters it, finds the screen's gadgets and fills
//! their lists (`InitSkirmishGameGadgets`: the seven computer slots' Open / Closed / Easy / Medium / Hard, the colours,
//! factions and teams through GUIUtil.cpp, the starting money), then sets slot 0 to the player (`Skirmish.ini`'s name,
//! colour and faction), slot 1 to a computer player whose skill follows the skirmish wins (easy, medium after 5 wins,
//! hard after 10), the slot list `Skirmish.ini` keeps, the seed (`GetTickCount`), the starting money, the superweapon
//! limit and the map (`Skirmish.ini`'s, else the cache's first multiplayer map); places the start spots on the map
//! (`skirmishPositionStartSpots`), the game-speed slider (`FPS`, 15 to 61) and shows the screen. **Update** sets the
//! focus two frames in, then starts the `SkirmishGameOptionsMenuFade` transition, and completes a shutdown once the
//! animations and transitions are done. **System**: a combo box's selection writes the slot's player state, colour (if
//! no other slot holds it), faction or team, or the starting money, into the game, then refreshes the slot list
//! (`skirmishUpdateSlotList` through GUIUtil's `UpdateSlotList`); a start-spot button gives that spot to the next of the
//! player's slots without one (a right click takes it back); *Select Map* opens `Menus/SkirmishMapSelectMenu.wnd` over
//! the screen (`crate::client::skirmish_map_select`); *Start* writes the preferences and starts the game if the map is
//! known and big enough (`startPressed`: `reallyDoStart`'s `MSG_NEW_GAME` with `GAME_SKIRMISH`, or
//! `GAME_SINGLE_PLAYER` on a solo map, and the speed); *Back* writes them and pops the screen, deleting the game; the
//! player name's entry renames slot 0. **Input**: Escape's release presses *Back*.
//!
//! # Upstream quirks that are kept
//!
//! - **Update** calls `TheShell->shutdownComplete(layout)` every frame the animations and transitions are done, not
//!   only after a shutdown (with nothing pending it only resets the animate manager).
//! - `GSM_SLIDER_TRACK` has no `break`: it falls into `GBM_SELECTED` with the slider as the control, where nothing
//!   matches (but a pushed button stops it there).
//! - Setting a combo box's selection from code (init, `UpdateSlotList`) sends `GCM_SELECTED` to this screen, whose
//!   handlers write the selection back into the slot: init's `GadgetComboBoxSetSelectedPos(comboBoxPlayer[i], 0)` sets
//!   each computer slot `Open` before slot 1 and the kept slot list are set.
//! - `handleColorSelection` sets a colour outside `-1 .. getNumColors()` without the free-colour check.
//! - `positionStartSpotControls` checks a new spot against the other spots' **screen** positions with its own position
//!   relative to the map window (the two agree only for a map window at the screen's corner), and nudges it right or
//!   down by the other's size.
//! - `positionAdditionalImages`' static `prevMMD` is never set: its "already done" check never returns. The supply and
//!   tech positions are pushed to the front of their lists (the lists are in reverse order).
//! - `startPressed` with a map the cache lacks shows *Can't find map* and then reads the cache's end iterator:
//!   recorded as undefined. `reallyDoStart` clamps the speed to 15 .. 60 (61 and above: 1000, no limit).
//! - *Back* deletes `TheSkirmishGameInfo` after `TheShell->pop()`; a callback that reads it afterwards (while the
//!   screen shuts down) finds `NULL` (the handlers check; `skirmishUpdateSlotList` does not: recorded as undefined).
//! - `SkirmishPreferences::getPreferredMap` and `isValidMap` / `getDefaultMap` call `TheMapCache->updateCache()`.
//!
//! # Where this differs from upstream
//!
//! - **The start** is a [`ShellRequest::StartSkirmish`] to the host: the game (after `startGame(0)`), its mode
//!   (`GAME_SKIRMISH`, or `GAME_SINGLE_PLAYER` on a solo map) and the speed, which the client turns into the World's
//!   skirmish start (`World::start_skirmish_game`: `m_mapName`, `InitGameLogicRandom(seed)`, `MSG_NEW_GAME`). A game in
//!   progress is cleared first ([`ShellRequest::ClearGameData`]), as the main menu's start does.
//! - **The preferences** (`Skirmish.ini`, `Network.ini`'s `UseSystemMapDir`) are the host's maps
//!   ([`SkirmishHost::prefs_load`], [`SkirmishHost::prefs_write`]): `UserPreferences::load` and `write`'s file reading
//!   and writing are not translated (the client keeps them in memory, GitHub issue draft `issue89_preferences`).
//! - **Battle honours** (`SkirmishBattleHonors`, `SkirmishStats.ini`): the wins are the host's
//!   ([`SkirmishHost::skirmish_wins`]); `populateSkirmishBattleHonors` (the honours list box, BattleHonors.cpp's
//!   `InsertBattleHonor`) is **not translated**: recorded through [`SkirmishHost::untranslated`]. *Reset* clears the
//!   host's stats ([`SkirmishHost::clear_battle_honors`]).
//! - `IsFirstCDPresent` is the host's ([`MenuHost::first_cd_present`]); the insert-disc prompt
//!   (`ExMessageBoxOkCancel`, ExtendedMessageBox.cpp) is not translated: recorded, and the start does not happen (its
//!   cancel callback, which clears `buttonPushed`, is translated: [`cancel_start_because_of_no_cd`]).
//! - `TheMouse->setCursor(Mouse::ARROW)` and `setMouseText` (the shutdown's) are not modelled; the tooltip callbacks
//!   of LobbyUtils.cpp (`playerTemplateComboBoxTooltip`, `playerTemplateListBoxTooltip`) and BattleHonors.cpp are set
//!   by name and recorded as untranslated when they run.
//! - `GetUnsignedIntFromRegistry("", "Preorder")` is the host's ([`SkirmishHost::is_preorder`]);
//!   `IPEnumeration::getMachineName` too ([`SkirmishHost::machine_name`]).
//! - `QuotedPrintableToUnicodeString` of an odd byte count: upstream's static buffer leftovers are not modelled
//!   ([`crate::common::map_cache::quoted_printable_to_unicode_string`]).
//! - `UnicodeString::trim` of a name whose end unit is not ASCII (`iswspace` there is the locale's): the name is kept
//!   untrimmed (**not re-checked**).
//! - A window the file lacks: upstream dereferences most of them unchecked; such a step is recorded as undefined
//!   ([`GameWindowManager::set_undefined`]).
//!
//! # Representation
//!
//! [`SkirmishGameOptions`]: the file's statics and globals (`TheSkirmishGameInfo`, `skirmishMapSelectLayout`,
//! `TheSupplyAndTechImageLocations`, `doUpdateSlotList`, `sandboxOk`) and GUIUtil's `winInitialized`; the host gives
//! it with the World's data ([`SkirmishHost::skirmish_split`]), which is `TheSkirmishGameInfo`'s `GameInfoHost`.

use std::collections::BTreeMap;

use crate::client::control_bar_popup::{format_wide, FormatArg};
use crate::client::gadget::{button_set_text, check_box_is_checked, check_box_set_checked, static_text_set_text};
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_slider::{slider_get_position, slider_set_position};
use crate::client::gadget_text_entry as te;
use crate::client::gui_util::{self as gu, GuiUtilHost, SkirmishData};
use crate::client::main_menu::DIFFICULTY_NORMAL;
use crate::client::map_util::{self, find_draw_positions, get_map_preview_image, SUPPLY_TECH_SIZE};
use crate::client::message_box::message_box_ok;
use crate::client::shell::{self, ShellRequest};
use crate::client::skirmish_map_select::SkirmishMapSelect;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, UserData, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::client::window_transitions as wt;
use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::game_info::{game_info_to_ascii_string, parse_ascii_string_to_game_info, slot_state, GameInfo, GameSlot, TheGameInfo, MAX_SLOTS, PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_RANDOM};
use crate::common::ini::game_make_color;
use crate::common::map_cache::{quoted_printable_to_ascii_string, quoted_printable_to_unicode_string, unicode_string_to_quoted_printable, MapMetaData};
use crate::common::money::Money;
use crate::common::recorder::game_mode;
use crate::common::unicode_string::UnicodeString;

/// `GREATER_NO_FPS_LIMIT`.
pub const GREATER_NO_FPS_LIMIT: i32 = 60;

/// The layout's name, and `layoutFilename` / `parentName` / `gadgetsToHide` / `perPlayerGadgetsToHide`.
const LAYOUT_FILENAME: &str = "SkirmishGameOptionsMenu.wnd";
const PARENT_NAME: &str = "SkirmishGameOptionsMenuParent";
const GADGETS_TO_HIDE: [&str; 5] = ["TextEntryPlayerName", "StaticTextPlayers", "StaticTextColor", "StaticTextTeam", "StaticTextFaction"];
const PER_PLAYER_GADGETS_TO_HIDE: [&str; 4] = ["ComboBoxPlayer", "ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"];

/// What the skirmish screens need besides GUIUtil's host.
pub trait SkirmishHost: GuiUtilHost {
    /// SkirmishGameOptionsMenu.cpp's statics.
    fn skirmish(&mut self) -> &mut SkirmishGameOptions;
    /// The statics and the World's data (`TheSkirmishGameInfo`'s `GameInfoHost`) together.
    fn skirmish_split(&mut self) -> (&mut SkirmishGameOptions, &mut SkirmishData);
    /// SkirmishMapSelectMenu.cpp's statics.
    fn map_select(&mut self) -> &mut SkirmishMapSelect;
    /// `UserPreferences::load(leaf)`: the file's keys and values (empty without the file).
    fn prefs_load(&mut self, leaf: &[u8]) -> BTreeMap<Vec<u8>, Vec<u8>>;
    /// `UserPreferences::write()`: the keys and values written to the file; whether it opened.
    fn prefs_write(&mut self, leaf: &[u8], prefs: &BTreeMap<Vec<u8>, Vec<u8>>) -> bool;
    /// `SkirmishBattleHonors::getWins()`.
    fn skirmish_wins(&self) -> i32 {
        0
    }
    /// `SkirmishBattleHonors::clear()` then `write()`.
    fn clear_battle_honors(&mut self) {}
    /// `IPEnumeration::getMachineName()`.
    fn machine_name(&self) -> Vec<u8> {
        Vec::new()
    }
    /// `GetUnsignedIntFromRegistry("", "Preorder", isPreorder)` gave a non-zero value.
    fn is_preorder(&self) -> bool {
        false
    }
    /// `TheGameEngine->getQuitting()`.
    fn quitting(&self) -> bool {
        false
    }
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32 {
        0
    }
    /// `TheGameLogic->isLoadingMap()`.
    fn is_loading_map(&self) -> bool {
        false
    }
    /// A function upstream has here that this translation does not (recorded for the owner).
    fn untranslated(&mut self, what: &[u8]);
}

/// SkirmishGameOptionsMenu.cpp's statics and globals (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkirmishGameOptions {
    /// `TheSkirmishGameInfo`.
    pub game: Option<GameInfo>,
    pub parent_skirmish_game_options_id: i32,
    pub text_entry_player_name_id: i32,
    pub combo_box_player_id: [i32; MAX_SLOTS],
    pub combo_box_color_id: [i32; MAX_SLOTS],
    pub combo_box_player_template_id: [i32; MAX_SLOTS],
    pub combo_box_team_id: [i32; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    pub text_entry_map_display_id: i32,
    pub button_exit_id: i32,
    pub button_start_id: i32,
    pub button_select_map_id: i32,
    pub button_reset_id: i32,
    pub window_map_id: i32,
    pub slider_game_speed_id: i32,
    pub static_text_game_speed_id: i32,
    pub check_box_limit_superweapons_id: i32,
    pub combo_box_starting_cash_id: i32,
    pub static_text_game_speed: Option<WinId>,
    pub parent_skirmish_game_options: Option<WinId>,
    pub button_exit: Option<WinId>,
    pub button_start: Option<WinId>,
    pub button_select_map: Option<WinId>,
    pub text_entry_map_display: Option<WinId>,
    pub button_reset: Option<WinId>,
    pub window_map: Option<WinId>,
    pub text_entry_player_name: Option<WinId>,
    pub check_box_limit_superweapons: Option<WinId>,
    pub combo_box_starting_cash: Option<WinId>,
    pub combo_box_player: [Option<WinId>; MAX_SLOTS],
    pub combo_box_color: [Option<WinId>; MAX_SLOTS],
    pub combo_box_player_template: [Option<WinId>; MAX_SLOTS],
    pub combo_box_team: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    /// `skirmishMapSelectLayout`.
    pub skirmish_map_select_layout: Option<usize>,
    pub initial_gadget_delay: i32,
    pub just_entered: bool,
    pub button_pushed: bool,
    pub still_needs_to_set_options: bool,
    /// `doUpdateSlotList`.
    pub do_update_slot_list: bool,
    /// `sandboxOk`.
    pub sandbox_ok: bool,
    /// `TheSupplyAndTechImageLocations.m_supplyPosList`.
    pub supply_pos_list: Vec<ICoord2D>,
    /// `TheSupplyAndTechImageLocations.m_techPosList`.
    pub tech_pos_list: Vec<ICoord2D>,
    /// GUIUtil.cpp's `winInitialized`.
    pub win_initialized: bool,
}

impl Default for SkirmishGameOptions {
    /// The statics' initial values.
    fn default() -> Self {
        SkirmishGameOptions {
            game: None,
            parent_skirmish_game_options_id: 0,
            text_entry_player_name_id: 0,
            combo_box_player_id: [0; MAX_SLOTS],
            combo_box_color_id: [0; MAX_SLOTS],
            combo_box_player_template_id: [0; MAX_SLOTS],
            combo_box_team_id: [0; MAX_SLOTS],
            button_map_start_position_id: [0; MAX_SLOTS],
            text_entry_map_display_id: 0,
            button_exit_id: 0,
            button_start_id: 0,
            button_select_map_id: 0,
            button_reset_id: 0,
            window_map_id: 0,
            slider_game_speed_id: 0,
            static_text_game_speed_id: 0,
            check_box_limit_superweapons_id: 0,
            combo_box_starting_cash_id: 0,
            static_text_game_speed: None,
            parent_skirmish_game_options: None,
            button_exit: None,
            button_start: None,
            button_select_map: None,
            text_entry_map_display: None,
            button_reset: None,
            window_map: None,
            text_entry_player_name: None,
            check_box_limit_superweapons: None,
            combo_box_starting_cash: None,
            combo_box_player: [None; MAX_SLOTS],
            combo_box_color: [None; MAX_SLOTS],
            combo_box_player_template: [None; MAX_SLOTS],
            combo_box_team: [None; MAX_SLOTS],
            button_map_start_position: [None; MAX_SLOTS],
            skirmish_map_select_layout: None,
            initial_gadget_delay: 2,
            just_entered: false,
            button_pushed: false,
            still_needs_to_set_options: false,
            do_update_slot_list: true,
            sandbox_ok: false,
            supply_pos_list: Vec::new(),
            tech_pos_list: Vec::new(),
            win_initialized: false,
        }
    }
}

fn key_of<H: SkirmishHost>(h: &mut H, name: &str) -> i32 {
    h.name_to_key(format!("{LAYOUT_FILENAME}:{name}").as_bytes()) as i32
}

/// `atoi(s)`: leading blanks, a sign, digits (a value out of `Int`'s range is undefined in C: clamped here).
fn atoi(s: &[u8]) -> i32 {
    let s: Vec<u8> = s.iter().copied().skip_while(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)).collect();
    let (neg, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, &s[..]),
    };
    let mut v: i64 = 0;
    for &c in digits.iter().take_while(|c| c.is_ascii_digit()) {
        v = (v * 10 + i64::from(c - b'0')).min(i64::from(u32::MAX) + 1);
    }
    let v = if neg { -v } else { v };
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `strtoul(s, NULL, 10)` as Windows' 32-bit `unsigned long` reads it (a magnitude past it is `ULONG_MAX`; a minus
/// sign negates).
fn strtoul(s: &[u8]) -> u32 {
    let s: Vec<u8> = s.iter().copied().skip_while(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)).collect();
    let (neg, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, &s[..]),
    };
    let mut v: u64 = 0;
    for &c in digits.iter().take_while(|c| c.is_ascii_digit()) {
        v = v * 10 + u64::from(c - b'0');
        if v > u64::from(u32::MAX) {
            return u32::MAX;
        }
    }
    if neg {
        (v as u32).wrapping_neg()
    } else {
        v as u32
    }
}

/// `UserPreferences` (a `std::map<AsciiString, AsciiString>`) and its subclasses' readers used here: `Skirmish.ini`
/// (`SkirmishPreferences`) and `Network.ini` (`LANPreferences`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserPreferences {
    /// The leaf name `load` was given.
    pub leaf: Vec<u8>,
    pub map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl UserPreferences {
    /// The constructor's `load(leaf)` (the host's file).
    pub fn load<H: SkirmishHost>(h: &mut H, leaf: &[u8]) -> UserPreferences {
        UserPreferences { leaf: leaf.to_vec(), map: h.prefs_load(leaf) }
    }

    /// `getAsciiString(key, defaultValue)`.
    pub fn get_ascii_string(&self, key: &[u8], default: &[u8]) -> Vec<u8> {
        self.map.get(key).cloned().unwrap_or_else(|| default.to_vec())
    }

    /// `getInt(key, defaultValue)`: `atoi` of a non-empty value.
    pub fn get_int(&self, key: &[u8], default: i32) -> i32 {
        let v = self.get_ascii_string(key, b"");
        if v.is_empty() {
            default
        } else {
            atoi(&v)
        }
    }

    /// `setAsciiString(key, val)`.
    pub fn set_ascii_string(&mut self, key: &[u8], val: &[u8]) {
        self.map.insert(key.to_vec(), val.to_vec());
    }

    /// `setInt(key, val)` (`intAsStr`: `%d`).
    pub fn set_int(&mut self, key: &[u8], val: i32) {
        self.set_ascii_string(key, format!("{val}").as_bytes());
    }

    /// `UserPreferences::write()`.
    pub fn write_file<H: SkirmishHost>(&self, h: &mut H) -> bool {
        h.prefs_write(&self.leaf, &self.map)
    }

    /// `LANPreferences::usesSystemMapDir()` (and `SkirmishPreferences::usesSystemMapDir()`, the same): no entry, or
    /// `yes` without regard to case.
    pub fn uses_system_map_dir(&self) -> bool {
        match self.map.get(&b"UseSystemMapDir"[..]) {
            None => true,
            Some(v) => v.eq_ignore_ascii_case(b"yes"),
        }
    }

    /// `SkirmishPreferences::getSlotList()`.
    pub fn get_slot_list(&self) -> Vec<u8> {
        self.get_ascii_string(b"SlotList", b"")
    }

    /// `SkirmishPreferences::setSlotList()`: `GameInfoToAsciiString(TheSkirmishGameInfo)`.
    pub fn set_slot_list<H: SkirmishHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        let (o, d) = h.skirmish_split();
        match game_info_to_ascii_string(o.game.as_ref(), d) {
            Ok(s) => self.set_ascii_string(b"SlotList", &s),
            Err(_) => wm.set_undefined("SkirmishPreferences::setSlotList: GameInfoToAsciiString"),
        }
    }

    /// `SkirmishPreferences::getUserName()`: the quoted-printable `UserName`, trimmed, else the machine's name.
    pub fn get_user_name<H: SkirmishHost>(&self, h: &H) -> Vec<u16> {
        let machine = || h.machine_name().iter().map(|&b| b as i8 as i16 as u16).take_while(|&c| c != 0).collect::<Vec<u16>>();
        let Some(v) = self.map.get(&b"UserName"[..]) else { return machine() };
        let raw = quoted_printable_to_unicode_string(v);
        let mut ret = UnicodeString::try_from_chars(&raw).unwrap_or_default();
        let ret = match ret.trim() {
            Ok(()) => ret.str().to_vec(),
            // (`iswspace` of a unit above 0x7F: the locale's; kept untrimmed, see the module docs)
            Err(_) => raw,
        };
        if ret.is_empty() {
            return machine();
        }
        ret
    }

    /// `SkirmishPreferences::getPreferredColor()`: `Color`, -1 without it or out of `-1 .. getNumColors()`.
    pub fn get_preferred_color<H: SkirmishHost>(&self, h: &H) -> i32 {
        let Some(v) = self.map.get(&b"Color"[..]) else { return -1 };
        let ret = atoi(v);
        if ret < -1 || ret >= h.util_data().settings.get_num_colors() {
            -1
        } else {
            ret
        }
    }

    /// `SkirmishPreferences::getPreferredFaction()`: `PlayerTemplate`, random without it, out of range, or for a
    /// template without a starting building.
    pub fn get_preferred_faction<H: SkirmishHost>(&self, h: &H) -> i32 {
        let Some(v) = self.map.get(&b"PlayerTemplate"[..]) else { return PLAYERTEMPLATE_RANDOM };
        let d = h.util_data();
        let mut ret = atoi(v);
        if ret < PLAYERTEMPLATE_MIN || ret >= d.template_count() {
            ret = PLAYERTEMPLATE_RANDOM;
        }
        if ret >= 0 {
            match d.template(ret) {
                None => ret = PLAYERTEMPLATE_RANDOM,
                Some(t) if t.starting_building.is_empty() => ret = PLAYERTEMPLATE_RANDOM,
                Some(_) => {}
            }
        }
        ret
    }

    /// `SkirmishPreferences::getPreferredMap()`: the quoted-printable `Map`, trimmed, if it is a valid multiplayer
    /// map; else `getDefaultMap(TRUE)`.
    pub fn get_preferred_map<H: SkirmishHost>(&self, h: &mut H) -> Vec<u8> {
        let Some(v) = self.map.get(&b"Map"[..]) else { return map_util::get_default_map(h, true) };
        let ret = trim_ascii(&quoted_printable_to_ascii_string(v));
        if ret.is_empty() || !map_util::is_valid_map(h, &ret, true) {
            return map_util::get_default_map(h, true);
        }
        ret
    }

    /// `SkirmishPreferences::getSuperweaponRestricted()`: `SuperweaponRestrict` is `yes` without regard to case.
    pub fn get_superweapon_restricted(&self) -> bool {
        self.map.get(&b"SuperweaponRestrict"[..]).is_some_and(|v| v.eq_ignore_ascii_case(b"yes"))
    }

    /// `SkirmishPreferences::setSuperweaponRestricted(b)`: `Yes` or `No`.
    pub fn set_superweapon_restricted(&mut self, restricted: bool) {
        self.set_ascii_string(b"SuperweaponRestrict", if restricted { b"Yes" } else { b"No" });
    }

    /// `SkirmishPreferences::getStartingCash()`: `strtoul` of `StartingCash`, else the settings' default.
    pub fn get_starting_cash<H: SkirmishHost>(&self, h: &H) -> Money {
        let Some(v) = self.map.get(&b"StartingCash"[..]) else { return *h.util_data().settings.get_default_starting_money() };
        let mut money = Money::new();
        money.deposit(strtoul(v), false, &mut Vec::new());
        money
    }

    /// `SkirmishPreferences::setStartingCash(cash)`: `%d` of the amount (an amount above `Int`'s range is negative).
    pub fn set_starting_cash(&mut self, cash: &Money) {
        self.set_ascii_string(b"StartingCash", format!("{}", cash.count_money() as i32).as_bytes());
    }

    /// `SkirmishPreferences::write()`: the game's slot 0 colour, faction and name, its map, starting money, superweapon
    /// limit and slot list, the speed slider's position, written (false without a game).
    pub fn write<H: SkirmishHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) -> bool {
        let Some(g) = h.skirmish().game.clone() else { return false };
        self.set_ascii_string(b"Color", format!("{}", g.slots[0].color).as_bytes());
        self.set_ascii_string(b"PlayerTemplate", format!("{}", g.slots[0].player_template).as_bytes());
        self.set_ascii_string(b"Map", &g.map_name);
        self.set_ascii_string(b"UserName", &unicode_string_to_quoted_printable(&g.slots[0].name));
        self.set_starting_cash(&g.starting_cash);
        self.set_superweapon_restricted(g.superweapon_restriction != 0);
        self.set_slot_list(wm, h);
        let (p, id) = (h.skirmish().parent_skirmish_game_options, h.skirmish().slider_game_speed_id);
        let Some(slider) = wm.win_get_window_from_id(p, id) else {
            wm.set_undefined("SkirmishPreferences::write: no SliderGameSpeed (GadgetSliderGetPosition of NULL)");
            return false;
        };
        let max_fps = slider_get_position(wm, slider);
        self.set_int(b"FPS", max_fps);
        self.write_file(h)
    }
}

/// `AsciiString::trim()`: leading and trailing blanks (`isspace`).
fn trim_ascii(s: &[u8]) -> Vec<u8> {
    let sp = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let start = s.iter().position(|c| !sp(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !sp(c)).map_or(start, |e| e + 1);
    s[start..end.max(start)].to_vec()
}

/// `SkirmishPreferences prefs` (`load("Skirmish.ini")`).
pub fn skirmish_preferences<H: SkirmishHost>(h: &mut H) -> UserPreferences {
    UserPreferences::load(h, b"Skirmish.ini")
}

/// `getNextSelectablePlayer(start)`: the first slot from `start` without a start spot that is the player's or (for the
/// host) a computer's; -1.
pub fn get_next_selectable_player(g: &GameInfo, start: i32) -> i32 {
    if !g.am_i_host() {
        return -1;
    }
    for j in start.max(0)..MAX_SLOTS as i32 {
        let s = &g.slots[j as usize];
        if s.start_pos == -1 && (j == g.local_slot_num() || s.is_ai()) {
            return j;
        }
    }
    -1
}

/// `setFPSTextBox(sliderPos)`: `--` above 60, else the speed (`%2d`), the text disabled at the global data's limit.
pub fn set_fps_text_box<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, slider_pos: i32) {
    let Some(t) = h.skirmish().static_text_game_speed else { return };
    wm.win_enable(t, true);
    if slider_pos > GREATER_NO_FPS_LIMIT {
        static_text_set_text(wm, h, Some(t), &"--".encode_utf16().collect::<Vec<u16>>());
        return;
    } else if slider_pos == h.util_data().frames_per_second_limit {
        wm.win_enable(t, false);
    }
    let text: Vec<u16> = format!("{slider_pos:2}").encode_utf16().collect();
    static_text_set_text(wm, h, Some(t), &text);
}

/// `reallyDoStart()`: the speed read and clamped, `startGame(0)`, and the start asked of the host (see the module
/// docs).
pub fn really_do_start<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.env().in_game {
        h.request(ShellRequest::ClearGameData);
    }
    let (p, id) = (h.skirmish().parent_skirmish_game_options, h.skirmish().slider_game_speed_id);
    let Some(slider) = wm.win_get_window_from_id(p, id) else {
        wm.set_undefined("reallyDoStart: no SliderGameSpeed (GadgetSliderGetPosition of NULL)");
        return;
    };
    let mut max_fps = slider_get_position(wm, slider);
    if max_fps > GREATER_NO_FPS_LIMIT {
        max_fps = 1000;
    }
    if max_fps < 15 {
        max_fps = 15;
    }
    let (o, d) = h.skirmish_split();
    let Some(g) = o.game.as_mut() else {
        wm.set_undefined("reallyDoStart: TheSkirmishGameInfo is NULL");
        return;
    };
    // (`TheWritableGlobalData->m_mapName = map`: the World's, in the start)
    g.start_game(0, d);
    // (`InitGameLogicRandom(seed)`: the World's, in the start)
    let is_skirmish = d.map_cache.find_map(&g.map_name).is_none_or(|m| m.is_multiplayer);
    let mode = if is_skirmish { game_mode::SKIRMISH } else { game_mode::SINGLE_PLAYER };
    let game = Box::new(g.clone());
    h.request(ShellRequest::StartSkirmish { game, mode, difficulty: DIFFICULTY_NORMAL, max_fps });
}

/// `cancelStartBecauseOfNoCD(userData)`: the insert-disc prompt's cancel.
pub fn cancel_start_because_of_no_cd<H: SkirmishHost>(h: &mut H) {
    h.skirmish().button_pushed = false;
}

/// `CheckForCDAtGameStart(callback)` with `reallyDoStart`: the start at once with the disc; else the prompt
/// (`ExMessageBoxOkCancel`, not translated: recorded).
pub fn check_for_cd_at_game_start<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.first_cd_present() {
        h.untranslated(b"ExMessageBoxOkCancel(GUI:InsertCDPrompt)");
    } else {
        really_do_start(wm, h);
    }
}

/// `startPressed()`: the map must be in the cache and hold the players.
fn start_pressed<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(g) = h.skirmish().game.clone() else {
        wm.set_undefined("startPressed: TheSkirmishGameInfo is NULL");
        return;
    };
    let player_count = g.num_players();
    let lower_map = g.map_name.to_ascii_lowercase();
    let mmd = h.util_data().map_cache.maps.get(&lower_map).map(|m| m.num_players);
    let Some(num_players) = mmd else {
        h.skirmish().button_pushed = false;
        let (t, b) = (h.fetch_text(b"GUI:ErrorStartingGame"), h.fetch_text(b"GUI:CantFindMap"));
        message_box_ok(wm, h, &t, &b, None);
        wm.set_undefined("startPressed: the map is not in the cache (MapMetaData mmd = it->second at the end)");
        return;
    };
    let mut is_ready = false;
    if player_count > num_players {
        h.skirmish().button_pushed = false;
        let msg = format_wide(&h.fetch_text(b"GUI:TooManyPlayers"), &[FormatArg::Int(i64::from(num_players))]);
        let t = h.fetch_text(b"GUI:ErrorStartingGame");
        message_box_ok(wm, h, &t, &msg, None);
    } else {
        is_ready = true;
    }
    if is_ready {
        check_for_cd_at_game_start(wm, h);
    }
}

/// `MapSelectorTooltip(window, instData, mouse)`: the tech building's or supply dock's tooltip under the mouse.
pub fn map_selector_tooltip<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    let x = i32::from(mouse as u16 as i16);
    let y = i32::from((mouse >> 16) as u16 as i16);
    let (pixel_x, pixel_y) = wm.tree.screen_position(window);
    let image = h.find_mapped_image(b"TecBuilding");
    let image2 = h.find_mapped_image(b"Cash");
    let inside = |p: &ICoord2D| (x > pixel_x + p.x && x < pixel_x + p.x + SUPPLY_TECH_SIZE) && (y > pixel_y + p.y && y < pixel_y + p.y + SUPPLY_TECH_SIZE);
    if image.is_some() && h.skirmish().tech_pos_list.iter().any(inside) {
        let t = h.fetch_text(b"TOOLTIP:TechBuilding");
        wm.mouse.set_cursor_tooltip(&t, -1);
        return;
    }
    if image2.is_some() && h.skirmish().supply_pos_list.iter().any(inside) {
        let t = h.fetch_text(b"TOOLTIP:SupplyDock");
        wm.mouse.set_cursor_tooltip(&t, -1);
    }
}

/// `positionStartSpotControls(win, mapWindow, pos, mmd, buttonMapStartPositions)`.
pub fn position_start_spot_controls(wm: &mut GameWindowManager, win: Option<WinId>, map_window: Option<WinId>, pos: &Coord3D, mmd: Option<&MapMetaData>, buttons: &[Option<WinId>; MAX_SLOTS]) {
    let (Some(win), Some(mmd), Some(map_window)) = (win, mmd, map_window) else { return };
    let win_map_size = wm.tree.windows[map_window].size;
    let gadget_size = wm.tree.windows[win].size;
    let (ul, lr) = find_draw_positions(0, 0, win_map_size.0, win_map_size.1, &mmd.extent);
    let small_width = lr.x - ul.x;
    let small_height = lr.y - ul.y;
    let e = &mmd.extent;
    let position = (pos.x - e.lo.x) / (e.hi.x - e.lo.x);
    let mut gx = ((position * small_width as f32) - (gadget_size.0 / 2) as f32 + ul.x as f32) as i32;
    let position = (pos.y - e.lo.y) / (e.hi.y - e.lo.y);
    let mut gy = (((1.0 - position) * small_height as f32) - (gadget_size.1 / 2) as f32 + ul.y as f32) as i32;
    for b in buttons {
        if *b == Some(win) {
            break;
        }
        let Some(b) = *b else {
            wm.set_undefined("positionStartSpotControls: a start-spot button is NULL (winGetScreenPosition)");
            return;
        };
        let (tx, ty) = wm.tree.screen_position(b);
        if gx > tx && gx < tx + gadget_size.0 && gy > ty && gy < ty + gadget_size.1 {
            let closer_right = tx + gadget_size.0 - gx;
            let closer_bottom = ty + gadget_size.1 - gy;
            if closer_right < closer_bottom {
                gx = tx + gadget_size.0 + 1;
            } else if closer_bottom < closer_right {
                gy = ty + gadget_size.1 + 1;
            } else {
                gx = tx + gadget_size.0 + 1;
                gy = ty + gadget_size.1 + 1;
            }
        }
    }
    wm.win_set_position(win, gx, gy);
}

/// `positionAdditionalImages(mmd, mapWindow, force)`: the supply docks' and tech buildings' markers (their top left in
/// the map window), each pushed to the front of its list.
pub fn position_additional_images<H: SkirmishHost>(wm: &GameWindowManager, h: &mut H, mmd: Option<&MapMetaData>, map_window: Option<WinId>, _force: bool) {
    let o = h.skirmish();
    o.supply_pos_list.clear();
    o.tech_pos_list.clear();
    let (Some(mmd), Some(map_window)) = (mmd, map_window) else { return };
    if wm.tree.windows[map_window].status & win_status::HIDDEN != 0 {
        return;
    }
    // (`static MapMetaData *prevMMD`: set to NULL by `force`, never to anything else; its check never returns)
    let size = wm.tree.windows[map_window].size;
    let (ul, lr) = find_draw_positions(0, 0, size.0, size.1, &mmd.extent);
    let small_width = lr.x - ul.x;
    let small_height = lr.y - ul.y;
    let e = &mmd.extent;
    let marker = |p: &Coord3D| {
        let position = (p.x - e.lo.x) / (e.hi.x - e.lo.x);
        let x = ((position * small_width as f32) - (SUPPLY_TECH_SIZE / 2) as f32 + ul.x as f32) as i32;
        let position = (p.y - e.lo.y) / (e.hi.y - e.lo.y);
        let y = (((1.0 - position) * small_height as f32) - (SUPPLY_TECH_SIZE / 2) as f32 + ul.y as f32) as i32;
        ICoord2D { x, y }
    };
    for p in &mmd.supply_positions {
        let m = marker(p);
        o.supply_pos_list.insert(0, m);
    }
    for p in &mmd.tech_positions {
        let m = marker(p);
        o.tech_pos_list.insert(0, m);
    }
}

/// `winSetUserData((void *)TheMapCache->findMap(name))` of the map window (`None`: `NULL`).
fn set_map_user_data(wm: &mut GameWindowManager, w: WinId, key: Option<Vec<u8>>) {
    wm.ext[w].user = UserData::MapMetaData(key);
}

/// The map window's image: `image`, else `UnknownMap`, else none (`WIN_STATUS_IMAGE` cleared).
fn set_map_image<H: SkirmishHost>(wm: &mut GameWindowManager, h: &H, w: WinId, image: Option<Vec<u8>>) {
    let image = image.or_else(|| h.find_mapped_image(b"UnknownMap"));
    match image {
        Some(i) => {
            wm.win_set_status(w, win_status::IMAGE);
            wm.tree.windows[w].inst.enabled_draw_data[0].image = Some(i);
        }
        None => {
            wm.win_clear_status(w, win_status::IMAGE);
        }
    }
}

/// `positionStartSpots(mapName, buttonMapStartPositions, mapWindow)`: the map's preview and its start-spot buttons
/// placed (the others hidden).
pub fn position_start_spots<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, map_name: &[u8], buttons: &[Option<WinId>; MAX_SLOTS], map_window: Option<WinId>) {
    let lower_map = map_name.to_ascii_lowercase();
    let found = h.util_data().map_cache.maps.get(&lower_map).cloned();
    let Some(mmd) = found else {
        let Some(w) = map_window else {
            wm.set_undefined("positionStartSpots: the map window is NULL (winSetUserData)");
            return;
        };
        set_map_user_data(wm, w, None);
        set_map_image(wm, h, w, None);
        position_additional_images(wm, h, None, map_window, true);
        for b in buttons.iter().flatten() {
            wm.win_hide(*b, true);
        }
        return;
    };
    let image = get_map_preview_image(h, map_name);
    if let Some(w) = map_window {
        let key = h.util_data().map_cache.find_map(map_name).map(|_| map_name.to_ascii_lowercase());
        set_map_user_data(wm, w, key);
        set_map_image(wm, h, w, image);
    }
    position_additional_images(wm, h, Some(&mmd), map_window, true);
    let mut i = 0usize;
    while (i as i32) < mmd.num_players && mmd.is_multiplayer {
        let waypoint_name = format!("Player_{}_Start", i + 1);
        if let Some(pos) = mmd.waypoints.map.get(waypoint_name.as_bytes()) {
            let Some(&b) = buttons.get(i) else {
                wm.set_undefined("positionStartSpots: a map of more than eight players reads past the buttons");
                return;
            };
            position_start_spot_controls(wm, b, map_window, pos, Some(&mmd), buttons);
            if let Some(b) = b {
                wm.win_hide(b, false);
            }
        }
        // (else `DEBUG_ASSERTCRASH`: nothing in the release)
        i += 1;
    }
    while i < MAX_SLOTS {
        if let Some(b) = buttons[i] {
            wm.win_hide(b, true);
        }
        i += 1;
    }
}

/// `positionStartSpots(myGame, buttonMapStartPositions, mapWindow)`: the game's map, or none when the local slot has
/// not got it (outside a game in progress).
pub fn position_start_spots_for_game<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, buttons: &[Option<WinId>; MAX_SLOTS], map_window: Option<WinId>) {
    let Some(g) = h.skirmish().game.clone() else {
        wm.set_undefined("positionStartSpots: myGame is NULL");
        return;
    };
    let mut local_map_fname = g.map_name.clone();
    if !g.in_progress {
        let local_idx = g.local_slot_num();
        if local_idx == -1 || !g.slots[local_idx as usize].has_map {
            local_map_fname.clear();
        }
    }
    position_start_spots(wm, h, &local_map_fname, buttons, map_window);
}

/// `updateMapStartSpots(myGame, buttonMapStartPositions, onLoadScreen)`: each spot's button labelled with the number
/// of the slot that holds it (`NUMBER:<n>`), with its tooltip.
pub fn update_map_start_spots<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, buttons: &[Option<WinId>; MAX_SLOTS], on_load_screen: bool) {
    let Some(g) = h.skirmish().game.clone() else {
        wm.set_undefined("updateMapStartSpots: myGame is NULL");
        return;
    };
    let lower_map = g.map_name.to_ascii_lowercase();
    let Some(num_players) = h.util_data().map_cache.maps.get(&lower_map).map(|m| m.num_players) else {
        for b in buttons.iter().flatten() {
            wm.win_hide(*b, true);
        }
        return;
    };
    for b in buttons.iter().flatten() {
        button_set_text(wm, h, Some(*b), &[]);
        if !on_load_screen {
            wm.tree.windows[*b].inst.tooltip_text = Some(h.fetch_text(b"TOOLTIP:StartPosition"));
        }
    }
    for i in 0..MAX_SLOTS {
        if buttons[i].is_none() {
            continue;
        }
        let gs = &g.slots[i];
        let number = h.fetch_text(format!("NUMBER:{}", i + 1).as_bytes());
        if on_load_screen {
            let apparent = match gs.apparent_start_pos(TheGameInfo::Is(&g, Some(i)), h.util_data()) {
                Ok(p) => p,
                Err(_) => {
                    wm.set_undefined("updateMapStartSpots: getApparentStartPos");
                    return;
                }
            };
            if apparent >= 0 && apparent < num_players && gs.player_template > PLAYERTEMPLATE_MIN {
                let Some(&b) = buttons.get(apparent as usize) else {
                    wm.set_undefined("updateMapStartSpots: a start position past the eight buttons");
                    return;
                };
                button_set_text(wm, h, b, &number);
            }
        } else if gs.start_pos >= 0 && gs.start_pos < num_players && gs.player_template > PLAYERTEMPLATE_MIN {
            let Some(&b) = buttons.get(gs.start_pos as usize) else {
                wm.set_undefined("updateMapStartSpots: a start position past the eight buttons");
                return;
            };
            button_set_text(wm, h, b, &number);
            let temp = format_wide(&h.fetch_text(b"TOOLTIP:StartPositionN"), &[FormatArg::Int(i as i64 + 1)]);
            match b {
                Some(b) => wm.tree.windows[b].inst.tooltip_text = Some(temp),
                None => {
                    wm.set_undefined("updateMapStartSpots: winSetTooltip on a NULL button");
                    return;
                }
            }
        }
    }
}

/// The selected row's item data of a combo box (`GadgetComboBoxGetSelectedPos`, `GadgetComboBoxGetItemData`); `None`
/// (an uninitialised `selIndex` upstream) for a `NULL` combo box.
fn combo_selected_data<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, combo: Option<WinId>, what: &'static str) -> Option<u32> {
    let Some(c) = combo else {
        wm.set_undefined(what);
        return None;
    };
    let sel = cb::get_selected_pos(wm, h, c, -1);
    Some(cb::get_item_data(wm, h, c, sel))
}

/// `handlePlayerSelection(index)`: a computer slot's state (`setState(type, title)`).
fn handle_player_selection<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    if index == 0 || index >= MAX_SLOTS {
        return;
    }
    let combo = h.skirmish().combo_box_player[index];
    let Some(player_type) = combo_selected_data(wm, h, combo, "handlePlayerSelection: the combo box is NULL (selIndex unset)") else { return };
    let title = gu::combo_get_text(wm, h, combo);
    let (o, d) = h.skirmish_split();
    if let Some(g) = o.game.as_mut() {
        g.slots[index].set_state(player_type as i32, &title, 0, d);
    }
}

/// `handleColorSelection(index)`: the colour, if no other slot holds it (any value outside `-1 .. numColors` is set).
fn handle_color_selection<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.skirmish().combo_box_color[index];
    let Some(color) = combo_selected_data(wm, h, combo, "handleColorSelection: the combo box is NULL (selIndex unset)") else { return };
    let color = color as i32;
    let num_colors = h.util_data().settings.get_num_colors();
    let Some(g) = h.skirmish().game.as_mut() else { return };
    if color == g.slots[index].color {
        return;
    }
    if color >= -1 && color < num_colors {
        let mut color_available = true;
        if color != -1 {
            for i in 0..MAX_SLOTS {
                if color == g.slots[i].color && i != index {
                    color_available = false;
                    break;
                }
            }
        }
        if !color_available {
            return;
        }
    }
    g.slots[index].color = color;
}

/// `handlePlayerTemplateSelection(index)`.
fn handle_player_template_selection<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.skirmish().combo_box_player_template[index];
    let Some(t) = combo_selected_data(wm, h, combo, "handlePlayerTemplateSelection: the combo box is NULL (selIndex unset)") else { return };
    let Some(g) = h.skirmish().game.as_mut() else { return };
    if t as i32 == g.slots[index].player_template {
        return;
    }
    g.slots[index].set_player_template(t as i32);
}

/// `handleStartPositionSelection(index, position)`: the spot, if no other slot holds it (a negative one always).
pub fn handle_start_position_selection(g: &mut GameInfo, index: usize, position: i32) {
    if position == g.slots[index].start_pos {
        return;
    }
    if position < 0 {
        g.slots[index].start_pos = position;
        return;
    }
    let is_available = !(0..MAX_SLOTS).any(|i| i != index && g.slots[i].start_pos == position);
    if is_available {
        g.slots[index].start_pos = position;
    }
}

/// `handleTeamSelection(index)`.
fn handle_team_selection<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.skirmish().combo_box_team[index];
    let Some(team) = combo_selected_data(wm, h, combo, "handleTeamSelection: the combo box is NULL (selIndex unset)") else { return };
    let Some(g) = h.skirmish().game.as_mut() else { return };
    if team as i32 == g.slots[index].team_number {
        return;
    }
    g.slots[index].team_number = team as i32;
}

/// `handleStartingCashSelection()`.
fn handle_starting_cash_selection<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.skirmish().game.is_none() {
        return;
    }
    let combo = h.skirmish().combo_box_starting_cash;
    let Some(amount) = combo_selected_data(wm, h, combo, "handleStartingCashSelection: the combo box is NULL (selIndex unset)") else { return };
    let mut starting_cash = Money::new();
    starting_cash.deposit(amount, false, &mut Vec::new());
    if let Some(g) = h.skirmish().game.as_mut() {
        g.starting_cash = starting_cash;
    }
}

/// `handleLimitSuperweaponsClick()`: the restriction 1 when the box is checked, else 0.
fn handle_limit_superweapons_click<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.skirmish().game.is_none() {
        return;
    }
    let Some(c) = h.skirmish().check_box_limit_superweapons else {
        wm.set_undefined("handleLimitSuperweaponsClick: the check box is NULL");
        return;
    };
    let checked = check_box_is_checked(wm, c);
    if let Some(g) = h.skirmish().game.as_mut() {
        g.superweapon_restriction = u16::from(checked);
    }
}

/// `InitSkirmishGameGadgets()`.
pub fn init_skirmish_game_gadgets<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    let ids = [
        key_of(h, PARENT_NAME),
        key_of(h, "ButtonBack"),
        key_of(h, "ButtonStart"),
        key_of(h, "TextEntryMapDisplay"),
        key_of(h, "ButtonSelectMap"),
        key_of(h, "ButtonReset"),
        key_of(h, "MapWindow"),
        key_of(h, "StaticTextGameSpeed"),
        key_of(h, "CheckboxLimitSuperweapons"),
        key_of(h, "ComboBoxStartingCash"),
    ];
    let parent = wm.win_get_window_from_id(None, ids[0]);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);
    let o = h.skirmish();
    [o.parent_skirmish_game_options_id, o.button_exit_id, o.button_start_id, o.text_entry_map_display_id, o.button_select_map_id, o.button_reset_id, o.window_map_id, o.static_text_game_speed_id, o.check_box_limit_superweapons_id, o.combo_box_starting_cash_id] = ids;
    o.parent_skirmish_game_options = parent;
    o.button_select_map = find(wm, ids[4]);
    o.button_start = find(wm, ids[2]);
    o.button_exit = find(wm, ids[1]);
    o.text_entry_map_display = find(wm, ids[3]);
    o.button_reset = find(wm, ids[5]);
    o.static_text_game_speed = find(wm, ids[7]);
    o.check_box_limit_superweapons = find(wm, ids[8]);
    o.combo_box_starting_cash = find(wm, ids[9]);
    let cash = o.combo_box_starting_cash;
    gu::populate_starting_cash_combo_box(wm, h, cash);
    let name_id = key_of(h, "TextEntryPlayerName");
    let name = wm.win_get_window_from_id(None, name_id);
    let window_map = find(wm, ids[6]);
    let o = h.skirmish();
    o.text_entry_player_name_id = name_id;
    o.text_entry_player_name = name;
    o.window_map = window_map;
    let Some(window_map) = window_map else {
        wm.set_undefined("InitSkirmishGameGadgets: no MapWindow (winSetTooltipFunc on NULL)");
        return;
    };
    wm.ext[window_map].tooltip = Some(b"MapSelectorTooltip".to_vec());
    let white = game_make_color(255, 255, 255, 255);
    for i in 0..MAX_SLOTS {
        if i != 0 {
            let id = key_of(h, &format!("ComboBoxPlayer{i}"));
            let c = wm.win_get_window_from_id(parent, id);
            let o = h.skirmish();
            o.combo_box_player_id[i] = id;
            o.combo_box_player[i] = c;
            gu::combo_reset(wm, h, c);
        }
        if i == 0 {
            let t = h.fetch_text(b"GUI:Player");
            te::set_text(wm, h, name, &t);
        } else {
            let c = h.skirmish().combo_box_player[i];
            for (k, (label, state)) in [(&b"GUI:Open"[..], slot_state::OPEN), (b"GUI:Closed", slot_state::CLOSED), (b"GUI:EasyAI", slot_state::EASY_AI), (b"GUI:MediumAI", slot_state::MED_AI), (b"GUI:HardAI", slot_state::BRUTAL_AI)].into_iter().enumerate() {
                let t = h.fetch_text(label);
                gu::combo_add_entry(wm, h, c, &t, white);
                gu::combo_set_item_data(wm, h, c, k as i32, state as u32);
            }
            gu::combo_set_selected_pos(wm, h, c, 0, false);
        }
        let ids = [key_of(h, &format!("ComboBoxColor{i}")), key_of(h, &format!("ComboBoxPlayerTemplate{i}")), key_of(h, &format!("ComboBoxTeam{i}")), key_of(h, &format!("ButtonMapStartPosition{i}"))];
        let found = ids.map(|id| wm.win_get_window_from_id(parent, id));
        let o = h.skirmish();
        o.combo_box_color_id[i] = ids[0];
        o.combo_box_color[i] = found[0];
        o.combo_box_player_template_id[i] = ids[1];
        o.combo_box_player_template[i] = found[1];
        o.combo_box_team_id[i] = ids[2];
        o.combo_box_team[i] = found[2];
        o.button_map_start_position_id[i] = ids[3];
        o.button_map_start_position[i] = found[3];
        let Some(t) = found[1] else {
            wm.set_undefined("InitSkirmishGameGadgets: no ComboBoxPlayerTemplate (winSetTooltipFunc on NULL)");
            return;
        };
        wm.ext[t].tooltip = Some(b"playerTemplateComboBoxTooltip".to_vec());
        let Some(l) = cb::get_list_box(wm, t) else {
            wm.set_undefined("InitSkirmishGameGadgets: the faction combo box has no list box (winSetTooltipFunc on NULL)");
            return;
        };
        wm.ext[l].tooltip = Some(b"playerTemplateListBoxTooltip".to_vec());
    }
    for i in 0..MAX_SLOTS {
        let o = h.skirmish().clone();
        gu::populate_color_combo_box(wm, h, i, &o.combo_box_color, false);
        gu::combo_set_selected_pos(wm, h, o.combo_box_color[i], 0, false);
        gu::populate_player_template_combo_box(wm, h, i, &o.combo_box_player_template, false);
        gu::populate_team_combo_box(wm, h, i, &o.combo_box_team, false);
    }
    populate_skirmish_battle_honors(h);
}

/// `populateSkirmishBattleHonors()`: **not translated** (BattleHonors.cpp's `InsertBattleHonor`, the honours file):
/// recorded.
fn populate_skirmish_battle_honors<H: SkirmishHost>(h: &mut H) {
    h.untranslated(b"populateSkirmishBattleHonors");
}

/// `skirmishUpdateSlotList()`: the map's name, the player's name and GUIUtil's `UpdateSlotList`, then the start spots'
/// labels; guarded against re-entry (`doUpdateSlotList`).
pub fn skirmish_update_slot_list<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.skirmish().do_update_slot_list {
        return;
    }
    h.skirmish().do_update_slot_list = false;
    let Some(g) = h.skirmish().game.clone() else {
        wm.set_undefined("skirmishUpdateSlotList: TheSkirmishGameInfo is NULL (getMap)");
        return;
    };
    let lower_map = g.map_name.to_ascii_lowercase();
    let display = h.util_data().map_cache.maps.get(&lower_map).map(|m| m.display_name.str().to_vec());
    let o = h.skirmish().clone();
    if let Some(d) = display {
        static_text_set_text(wm, h, o.text_entry_map_display, &d);
    }
    te::set_text(wm, h, o.text_entry_player_name, &g.slots[0].name);
    gu::update_slot_list(wm, h, &o.combo_box_player, &o.combo_box_color, &o.combo_box_player_template, &o.combo_box_team, None, o.button_start, &o.button_map_start_position);
    update_map_start_spots(wm, h, &o.button_map_start_position, false);
    h.skirmish().do_update_slot_list = true;
}

/// `skirmishPositionStartSpots()`: the start spots on the game's map, then `updateSkirmishGameOptions()`.
pub fn skirmish_position_start_spots<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    let o = h.skirmish().clone();
    position_start_spots_for_game(wm, h, &o.button_map_start_position, o.window_map);
    update_skirmish_game_options(wm, h);
}

/// `initSkirmishGameOptions()`: empty.
pub fn init_skirmish_game_options() {}

/// `updateSkirmishGameOptions()`: the map's name; the per-player gadgets shown on a multiplayer map, hidden (and the
/// computer slots set open) on a solo map; the superweapon box and the starting money from the game.
pub fn update_skirmish_game_options<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(g) = h.skirmish().game.clone() else {
        wm.set_undefined("updateSkirmishGameOptions: TheSkirmishGameInfo is NULL");
        return;
    };
    let o = h.skirmish().clone();
    let md = h.util_data().map_cache.find_map(&g.map_name).map(|m| (m.is_multiplayer, m.display_name.str().to_vec()));
    let is_skirmish = match md {
        Some((mp, display)) => {
            static_text_set_text(wm, h, o.text_entry_map_display, &display);
            mp
        }
        None => {
            // (`DEBUG_CRASH`: nothing in the release) `mapDisplay.translate(map)`
            let d: Vec<u16> = g.map_name.iter().map(|&b| b as i8 as i16 as u16).collect();
            static_text_set_text(wm, h, o.text_entry_map_display, &d);
            true
        }
    };
    if is_skirmish {
        gu::show_underlying_gui_elements(wm, h, true, LAYOUT_FILENAME, PARENT_NAME, &GADGETS_TO_HIDE, &PER_PLAYER_GADGETS_TO_HIDE);
    } else {
        gu::show_underlying_gui_elements(wm, h, false, LAYOUT_FILENAME, PARENT_NAME, &GADGETS_TO_HIDE, &PER_PLAYER_GADGETS_TO_HIDE);
        for i in 1..MAX_SLOTS {
            gu::combo_set_selected_pos(wm, h, o.combo_box_player[i], 0, false);
        }
    }
    let Some(check) = o.check_box_limit_superweapons else {
        wm.set_undefined("updateSkirmishGameOptions: the check box is NULL (GadgetCheckBoxSetChecked)");
        return;
    };
    let restricted = h.skirmish().game.as_ref().is_some_and(|g| g.superweapon_restriction != 0);
    check_box_set_checked(wm, h, check, restricted);
    let item_count = gu::combo_get_length(wm, o.combo_box_starting_cash);
    for index in 0..item_count {
        let value = gu::combo_get_item_data(wm, h, o.combo_box_starting_cash, index) as i32;
        let cash = h.skirmish().game.as_ref().map_or(0, |g| g.starting_cash.count_money() as i32);
        if value == cash {
            gu::combo_set_selected_pos(wm, h, o.combo_box_starting_cash, index, true);
            break;
        }
    }
}

/// `SkirmishGameOptionsMenuInit(layout, userData)`.
pub fn skirmish_game_options_menu_init<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.quitting() {
        return;
    }
    h.skirmish().still_needs_to_set_options = false;
    let slider_id = key_of(h, "SliderGameSpeed");
    let o = h.skirmish();
    o.slider_game_speed_id = slider_id;
    o.sandbox_ok = false;
    o.do_update_slot_list = false;
    let now = h.time_ms();
    let (o, d) = h.skirmish_split();
    d.tick = now;
    let signal = match o.game.as_mut() {
        None => {
            o.game = Some(GameInfo::new(d));
            true
        }
        Some(g) if g.in_game => {
            g.end_game();
            false
        }
        Some(_) => true,
    };
    if signal {
        h.signal_ui_interact("SHELL_SCRIPT_HOOK_SKIRMISH_OPENED");
    }
    let (o, d) = h.skirmish_split();
    let g = o.game.as_mut().expect("made above");
    g.reset(d);
    g.clear_slot_list(d);
    g.reset(d);
    let local_ip = g.slots[0].ip;
    g.local_ip = local_ip;
    g.enter_game(d);
    gu::enable_slot_list_updates(h, false);
    init_skirmish_game_gadgets(wm, h);
    gu::enable_slot_list_updates(h, true);
    let prefs = skirmish_preferences(h);
    let user_name = prefs.get_user_name(h);
    let color = prefs.get_preferred_color(h);
    let faction = prefs.get_preferred_faction(h);
    let wins = h.skirmish_wins();
    let (o, d) = h.skirmish_split();
    let Some(g) = o.game.as_mut() else {
        wm.set_undefined("SkirmishGameOptionsMenuInit: TheSkirmishGameInfo deleted during the init");
        return;
    };
    let mut g_slot = GameSlot::new();
    g_slot.name = user_name.clone();
    g_slot.set_state(slot_state::PLAYER, &user_name, 0, d);
    g_slot.color = color;
    g_slot.set_player_template(faction);
    g.set_slot(0, g_slot.clone());
    let state = if wins > 10 {
        slot_state::BRUTAL_AI
    } else if wins > 5 {
        slot_state::MED_AI
    } else {
        slot_state::EASY_AI
    };
    g_slot.set_state(state, &[], 0, d);
    g.set_slot(1, g_slot);
    if parse_ascii_string_to_game_info(Some(g), &prefs.get_slot_list(), d).is_err() {
        wm.set_undefined("SkirmishGameOptionsMenuInit: ParseAsciiStringToGameInfo of the kept slot list");
        return;
    }
    g.seed = now as i32;
    let preorder = h.is_preorder();
    let cash = prefs.get_starting_cash(h);
    let restricted = prefs.get_superweapon_restricted();
    let preferred_map = prefs.get_preferred_map(h);
    let (o, d) = h.skirmish_split();
    let g = o.game.as_mut().expect("kept above");
    if preorder {
        g.mark_player_as_preorder(0);
    }
    g.starting_cash = cash;
    g.superweapon_restriction = u16::from(restricted);
    let r = g.set_map(&preferred_map, d).and_then(|()| {
        let md = d.map_cache.find_map(&g.map_name).map(|m| (m.crc, m.filesize));
        let (crc, size) = md.unwrap_or((0, 0));
        g.set_map_crc(crc, d)?;
        g.set_map_size(size, d)
    });
    if r.is_err() {
        wm.set_undefined("SkirmishGameOptionsMenuInit: setMap / setMapCRC / setMapSize");
        return;
    }
    let lower_map = preferred_map.to_ascii_lowercase();
    let display = d.map_cache.maps.get(&lower_map).map(|m| m.display_name.str().to_vec());
    if let Some(display) = display {
        let t = h.skirmish().text_entry_map_display;
        static_text_set_text(wm, h, t, &display);
    }
    skirmish_position_start_spots(wm, h);
    let (p, id) = (h.skirmish().parent_skirmish_game_options, h.skirmish().slider_game_speed_id);
    let fps_limit = h.util_data().frames_per_second_limit;
    let slider_pos = 15.max(61.min(prefs.get_int(b"FPS", fps_limit)));
    let Some(slider) = wm.win_get_window_from_id(p, id) else {
        wm.set_undefined("SkirmishGameOptionsMenuInit: no SliderGameSpeed (GadgetSliderSetPosition of NULL)");
        return;
    };
    slider_set_position(wm, h, slider, slider_pos);
    set_fps_text_box(wm, h, slider_pos);
    let o = h.skirmish().clone();
    let Some(start) = o.button_start else {
        wm.set_undefined("SkirmishGameOptionsMenuInit: no ButtonStart (winSetText on NULL)");
        return;
    };
    let t = h.fetch_text(b"GUI:Start");
    wm.win_set_text(start, &t);
    for i in 0..MAX_SLOTS {
        for c in [o.combo_box_color[i], o.combo_box_player_template[i], o.combo_box_team[i]] {
            match c {
                Some(c) => wm.win_enable(c, true),
                None => {
                    wm.set_undefined("SkirmishGameOptionsMenuInit: a slot's combo box is NULL (winEnable)");
                    return;
                }
            }
        }
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, o.parent_skirmish_game_options);
    shell::show_shell_map(wm, h, true);
    h.skirmish().do_update_slot_list = true;
    skirmish_update_slot_list(wm, h);
    let o = h.skirmish();
    o.just_entered = true;
    o.initial_gadget_delay = 2;
    let sub = key_of(h, "SubParent");
    if let Some(w) = wm.win_get_window_from_id(None, sub) {
        wm.win_hide(w, true);
    }
    h.skirmish().button_pushed = false;
}

/// The file's `shutdownComplete(layout)`: the layout hidden, the shell told.
fn menu_shutdown_complete<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `SkirmishGameOptionsMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn skirmish_game_options_menu_shutdown<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    h.signal_ui_interact("SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED");
    // (`TheMouse->setCursor(Mouse::ARROW)`, `setMouseText(empty)`: not modelled)
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("SkirmishGameOptionsMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        menu_shutdown_complete(wm, h, layout);
        return;
    }
    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"SkirmishGameOptionsMenuFade");
}

/// `SkirmishGameOptionsMenuUpdate(layout, userData)`.
pub fn skirmish_game_options_menu_update<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.env().in_shell_game && h.logic_frame() == 1 {
        h.signal_ui_interact("SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME");
    }
    if h.skirmish().just_entered {
        if h.skirmish().initial_gadget_delay == 1 {
            h.skirmish().still_needs_to_set_options = true;
            let p = h.skirmish().parent_skirmish_game_options;
            wm.win_set_focus(h, p);
            let o = h.skirmish();
            o.initial_gadget_delay = 2;
            o.just_entered = false;
        } else {
            h.skirmish().initial_gadget_delay -= 1;
        }
    }
    if h.skirmish().still_needs_to_set_options && !h.is_loading_map() {
        wt::set_group(wm, h, b"SkirmishGameOptionsMenuFade", false);
        h.skirmish().still_needs_to_set_options = false;
    }
    if shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shell::shutdown_complete(wm, h, Some(layout), false);
    }
}

/// `SkirmishGameOptionsMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Back*.
pub fn skirmish_game_options_menu_input<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let (b, id) = (h.skirmish().button_exit, h.skirmish().button_exit_id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(b), id as u32);
    }
    MsgHandled::Handled
}

/// A click on start-spot button `i` (`GBM_SELECTED`): the spot's holder (one of the player's slots) gives it to the
/// next such slot without one, or an empty spot is given to the first such slot (else the local one).
fn start_spot_selected<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, i: i32) {
    let Some(g) = h.skirmish().game.as_mut() else {
        wm.set_undefined("SkirmishGameOptionsMenuSystem: TheSkirmishGameInfo is NULL (a start spot)");
        return;
    };
    let player_idx_in_pos = (0..MAX_SLOTS).find(|&j| g.slots[j].start_pos == i).map_or(-1, |j| j as i32);
    if player_idx_in_pos >= 0 {
        let slot_is_ai = g.slots[player_idx_in_pos as usize].is_ai();
        if player_idx_in_pos == g.local_slot_num() || (g.am_i_host() && slot_is_ai) {
            let next_player = get_next_selectable_player(g, player_idx_in_pos + 1);
            handle_start_position_selection(g, player_idx_in_pos as usize, -1);
            if next_player >= 0 {
                handle_start_position_selection(g, next_player as usize, i);
            }
        }
    } else {
        let mut next_player = get_next_selectable_player(g, 0);
        if next_player < 0 {
            next_player = g.local_slot_num();
        }
        if next_player < 0 {
            wm.set_undefined("SkirmishGameOptionsMenuSystem: no local slot (getSlot(-1)->getStartPos())");
            return;
        }
        handle_start_position_selection(g, next_player as usize, i);
    }
    skirmish_update_slot_list(wm, h);
    h.skirmish().sandbox_ok = false;
}

/// `SkirmishGameOptionsMenuSystem(window, msg, mData1, mData2)`.
pub fn skirmish_game_options_menu_system<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let control_id = |wm: &GameWindowManager| data_win(d1).map(|c| wm.tree.windows[c].id);
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GCM_SELECTED => {
            let Some(id) = control_id(wm) else {
                wm.set_undefined("SkirmishGameOptionsMenuSystem: GCM_SELECTED with no control");
                return MsgHandled::Handled;
            };
            let o = h.skirmish().clone();
            if id == o.combo_box_starting_cash_id {
                handle_starting_cash_selection(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if id == o.combo_box_color_id[i] {
                        handle_color_selection(wm, h, i);
                    } else if id == o.combo_box_player_template_id[i] {
                        handle_player_template_selection(wm, h, i);
                    } else if id == o.combo_box_team_id[i] {
                        handle_team_selection(wm, h, i);
                    } else if id == o.combo_box_player_id[i] {
                        handle_player_selection(wm, h, i);
                    }
                }
            }
            h.skirmish().sandbox_ok = false;
            skirmish_update_slot_list(wm, h);
        }
        ggm::GSM_SLIDER_TRACK | ggm::GBM_SELECTED => {
            let Some(id) = control_id(wm) else {
                wm.set_undefined("SkirmishGameOptionsMenuSystem: a message with no control");
                return MsgHandled::Handled;
            };
            if msg == ggm::GSM_SLIDER_TRACK && id == h.skirmish().slider_game_speed_id {
                set_fps_text_box(wm, h, d2 as i32);
            }
            // (`GSM_SLIDER_TRACK` falls through into `GBM_SELECTED`)
            if h.skirmish().button_pushed {
                return MsgHandled::Handled;
            }
            let o = h.skirmish().clone();
            if id == o.button_exit_id {
                h.skirmish().button_pushed = true;
                let mut prefs = skirmish_preferences(h);
                prefs.write(wm, h);
                if let Some(l) = h.skirmish().skirmish_map_select_layout.take() {
                    wm.layout_destroy_windows(l);
                    wm.layouts[l].deleted = true;
                }
                shell::pop(wm, h);
                h.skirmish().game = None;
            } else if id == o.button_select_map_id {
                h.skirmish().sandbox_ok = false;
                let l = h.create_layout(wm, b"Menus/SkirmishMapSelectMenu.wnd");
                h.skirmish().skirmish_map_select_layout = l;
                let Some(l) = l else {
                    wm.set_undefined("SkirmishGameOptionsMenuSystem: Menus/SkirmishMapSelectMenu.wnd did not load (runInit on NULL)");
                    return MsgHandled::Handled;
                };
                shell::run_layout(wm, h, l, 0, None);
                wm.layout_hide(l, false);
                wm.layout_bring_forward(l);
            } else if id == o.button_start_id {
                h.skirmish().button_pushed = true;
                let mut prefs = skirmish_preferences(h);
                prefs.write(wm, h);
                start_pressed(wm, h);
            } else if id == o.button_reset_id {
                h.clear_battle_honors();
                populate_skirmish_battle_honors(h);
            } else if id == o.check_box_limit_superweapons_id {
                handle_limit_superweapons_click(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if id == o.button_map_start_position_id[i] {
                        start_spot_selected(wm, h, i as i32);
                    }
                }
            }
        }
        ggm::GBM_SELECTED_RIGHT => {
            let Some(id) = control_id(wm) else {
                wm.set_undefined("SkirmishGameOptionsMenuSystem: GBM_SELECTED_RIGHT with no control");
                return MsgHandled::Handled;
            };
            let o = h.skirmish().clone();
            for i in 0..MAX_SLOTS {
                if id != o.button_map_start_position_id[i] {
                    continue;
                }
                let Some(g) = h.skirmish().game.as_mut() else {
                    wm.set_undefined("SkirmishGameOptionsMenuSystem: TheSkirmishGameInfo is NULL (a start spot)");
                    return MsgHandled::Handled;
                };
                let player_idx_in_pos = (0..MAX_SLOTS).find(|&j| g.slots[j].start_pos == i as i32).map_or(-1, |j| j as i32);
                if player_idx_in_pos >= 0 {
                    let slot_is_ai = g.slots[player_idx_in_pos as usize].is_ai();
                    if player_idx_in_pos == g.local_slot_num() || (g.am_i_host() && slot_is_ai) {
                        handle_start_position_selection(g, player_idx_in_pos as usize, -1);
                    }
                    skirmish_update_slot_list(wm, h);
                    h.skirmish().sandbox_ok = false;
                }
            }
        }
        ggm::GEM_EDIT_DONE | te::gem::UPDATE_TEXT => {
            let Some(id) = control_id(wm) else {
                wm.set_undefined("SkirmishGameOptionsMenuSystem: a text message with no control");
                return MsgHandled::Handled;
            };
            if id == h.skirmish().text_entry_player_name_id {
                let e = h.skirmish().text_entry_player_name;
                let name = te::get_text(wm, h, e);
                match h.skirmish().game.as_mut() {
                    Some(g) => g.slots[0].name = name,
                    None => wm.set_undefined("SkirmishGameOptionsMenuSystem: TheSkirmishGameInfo is NULL (the player's name)"),
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::gui_util::TemplateView;
    use crate::client::save_load_menu::tests::{booted, click, enabled, hidden, named, settle, tick, H};
    use crate::client::window_manager::short_to_long;
    use crate::common::base_type::Region3D;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::map_cache::{ascii_string_to_quoted_printable, WaypointMap};
    use crate::common::multiplayer_settings::MultiplayerSettings;

    pub(crate) const F: &str = "SkirmishGameOptionsMenu.wnd";
    pub(crate) const ALPHA: &[u8] = b"maps\\alpha\\alpha.map";
    pub(crate) const BETA: &[u8] = b"maps\\beta\\beta.map";
    pub(crate) const SOLO: &[u8] = b"maps\\solo\\solo.map";

    fn settings_block(ini: &mut Ini<'_>, s: &mut MultiplayerSettings) -> IniResult<()> {
        s.parse_multiplayer_settings_definition(ini)
    }
    fn color_block(ini: &mut Ini<'_>, s: &mut MultiplayerSettings) -> IniResult<()> {
        s.parse_multiplayer_color_definition(ini)
    }
    fn money_block(ini: &mut Ini<'_>, s: &mut MultiplayerSettings) -> IniResult<()> {
        s.parse_multiplayer_starting_money_choice_definition(ini)
    }
    static BLOCKS: [BlockParse<MultiplayerSettings>; 3] = [
        BlockParse { token: "MultiplayerSettings", parse: settings_block },
        BlockParse { token: "MultiplayerColor", parse: color_block },
        BlockParse { token: "MultiplayerStartingMoneyChoice", parse: money_block },
    ];

    fn c3(x: f32, y: f32) -> Coord3D {
        Coord3D { x, y, z: 0.0 }
    }

    /// A map of the tests' cache: its display name, players, multiplayer flag, extent (from 0, 0), start waypoints,
    /// supply docks and tech buildings.
    fn map(display: &str, players: i32, mp: bool, extent: (f32, f32), starts: &[(f32, f32)], supply: &[(f32, f32)], tech: &[(f32, f32)], crc: u32) -> MapMetaData {
        let mut w = WaypointMap::default();
        for (i, p) in starts.iter().enumerate() {
            w.map.insert(format!("Player_{}_Start", i + 1).into_bytes(), c3(p.0, p.1));
        }
        MapMetaData {
            display_name: UnicodeString::try_from_chars(&display.encode_utf16().collect::<Vec<u16>>()).expect("name"),
            extent: Region3D { lo: c3(0.0, 0.0), hi: c3(extent.0, extent.1) },
            num_players: players,
            is_multiplayer: mp,
            is_official: true,
            filesize: 99,
            crc,
            waypoints: w,
            supply_positions: supply.iter().map(|p| c3(p.0, p.1)).collect(),
            tech_positions: tech.iter().map(|p| c3(p.0, p.1)).collect(),
            ..Default::default()
        }
    }

    /// The tests' World data (no EA file): three maps (Alpha, two players, 1000x500 with its two starts 10 units
    /// apart, a supply dock and a tech building; Beta, four players, 1000x1000, a start in each corner; Solo, one
    /// player, not multiplayer), three colours (Red, Blue, Green), the starting money 5,000 / 10,000 (default) /
    /// 20,000, four player templates (an observer without a building; America; China; a second America side), an FPS
    /// limit of 30, the user data folder `User\`.
    pub(crate) fn data() -> SkirmishData {
        let mut d = SkirmishData::default();
        d.map_cache.maps.insert(ALPHA.to_vec(), map("Alpha (2)", 2, true, (1000.0, 500.0), &[(100.0, 100.0), (110.0, 100.0)], &[(500.0, 250.0)], &[(200.0, 200.0)], 0x1234));
        d.map_cache.maps.insert(BETA.to_vec(), map("Beta (4)", 4, true, (1000.0, 1000.0), &[(100.0, 900.0), (900.0, 900.0), (100.0, 100.0), (900.0, 100.0)], &[], &[], 0x5678));
        d.map_cache.maps.insert(SOLO.to_vec(), map("Solo", 1, false, (500.0, 500.0), &[(250.0, 250.0)], &[], &[], 0x9));
        let ini = b"MultiplayerColor Red\n TooltipName = COLOR:Red\n RGBColor = R:255 G:0 B:0\nEnd\n\
MultiplayerColor Blue\n TooltipName = COLOR:Blue\n RGBColor = R:0 G:0 B:255\nEnd\n\
MultiplayerColor Green\n TooltipName = COLOR:Green\n RGBColor = R:0 G:255 B:0\nEnd\n\
MultiplayerStartingMoneyChoice\n Value = 5000\nEnd\n\
MultiplayerStartingMoneyChoice\n Value = 10000\n Default = Yes\nEnd\n\
MultiplayerStartingMoneyChoice\n Value = 20000\nEnd\n";
        Ini::load(b"Multiplayer.ini", ini, IniLoadType::Overwrite, None, &BLOCKS, &mut d.settings).expect("the settings read");
        let t = |name: &str, side: &str, building: &str| TemplateView { name: name.as_bytes().to_vec(), side: side.as_bytes().to_vec(), starting_building: building.as_bytes().to_vec(), ..Default::default() };
        d.templates = vec![t("FactionObserver", "Observer", ""), t("FactionAmerica", "America", "AmericaCommandCenter"), t("FactionChina", "China", "ChinaCommandCenter"), t("FactionAmericaLaser", "America", "AmericaCommandCenter")];
        d.frames_per_second_limit = 30;
        d.default_starting_cash.deposit(10000, false, &mut Vec::new());
        d.user_data_path = b"User\\".to_vec();
        d.net_crc_interval = 100;
        d
    }

    /// The tests' `Skirmish.ini`: the name `Me`, Blue, America, Beta, 45 frames a second, 20,000, superweapons limited.
    pub(crate) fn prefs() -> BTreeMap<Vec<u8>, Vec<u8>> {
        let me: Vec<u16> = "Me".encode_utf16().collect();
        [
            (&b"UserName"[..], unicode_string_to_quoted_printable(&me)),
            (b"Color", b"1".to_vec()),
            (b"PlayerTemplate", b"1".to_vec()),
            (b"Map", ascii_string_to_quoted_printable(BETA)),
            (b"FPS", b"45".to_vec()),
            (b"StartingCash", b"20000".to_vec()),
            (b"SuperweaponRestrict", b"Yes".to_vec()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_vec(), v))
        .collect()
    }

    /// The shell booted on the main menu with the tests' data and `Skirmish.ini`, the skirmish screen pushed and
    /// settled.
    pub(crate) fn opened() -> (GameWindowManager, H) {
        let (mut wm, mut h) = booted();
        h.skirmish_data = data();
        h.prefs.insert(b"Skirmish.ini".to_vec(), prefs());
        push_settled(&mut wm, &mut h, false);
        (wm, h)
    }

    /// The skirmish screen pushed and settled: its fade starts two frames after its init (until then the transitions
    /// are "finished"), and a shutdown before it has started reverses the group still holding the windows of the
    /// screen's previous opening (freed: upstream's dangling pointers).
    pub(crate) fn push_settled(wm: &mut GameWindowManager, h: &mut H, immediate: bool) {
        shell::push(wm, h, b"Menus/SkirmishGameOptionsMenu.wnd", immediate);
        settle(wm, h);
        for _ in 0..3 {
            tick(wm, h);
        }
        settle(wm, h);
    }

    fn text(wm: &GameWindowManager, w: WinId) -> String {
        String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(wm, w))
    }

    fn wtext(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// The newest message box's OK clicked, and a frame for its destruction.
    fn close_box(wm: &mut GameWindowManager, h: &mut H) {
        let ok = wm.tree.windows.iter().rposition(|w| w.name() == b"MessageBox.wnd:ButtonOk").expect("the box's OK");
        click(wm, h, ok);
        tick(wm, h);
    }

    /// The row of combo box `c` whose item data is `data`, selected as a click on the list selects it.
    pub(crate) fn choose(wm: &mut GameWindowManager, h: &mut H, c: WinId, data: i32) {
        let n = cb::get_length(wm, c);
        let row = (0..n).find(|&i| cb::get_item_data(wm, h, c, i) as i32 == data).unwrap_or_else(|| panic!("no row with {data}"));
        cb::set_selected_pos(wm, h, c, row, false);
    }

    /// #89: the skirmish screen opened from `Skirmish.ini`, and played with, predicted from SkirmishGameOptionsMenu.cpp
    /// and GUIUtil.cpp before the run:
    ///
    /// - **init**: `TheSkirmishGameInfo` made and entered (`SHELL_SCRIPT_HOOK_SKIRMISH_OPENED` signalled); slot 0 the
    ///   player `Me`, Blue (1), America (1); slot 1 an easy computer (no wins); slots 2 to 7 open (init's own
    ///   `GadgetComboBoxSetSelectedPos(comboBoxPlayer[i], 0)` set them through `handlePlayerSelection`); the map Beta
    ///   (CRC 0x5678, size 99), its name in `TextEntryMapDisplay`; the four start buttons shown at (12, 12),
    ///   (172, 12), (12, 172), (172, 172) in the map window, the other four hidden; 20,000 the starting money (the
    ///   combo box's third row), superweapons limited (the box checked); the slider at 45 and its text `45`;
    ///   `populateSkirmishBattleHonors` recorded as untranslated; slot 0's colour list `???`, Red, Blue, Green and Blue
    ///   selected; its faction list Random, America, China (the observer has no building, the second America side is
    ///   seen); the open slots' colour, faction and team boxes disabled by `UpdateSlotList`, slot 1's enabled;
    /// - **the start spots**: a click on spot 2 gives it to slot 0 (`NUMBER:1` on the button); again: slot 0 gives it
    ///   to slot 1 (the next of the player's slots without one) and has none; a right click takes it from slot 1;
    /// - **the combo boxes**: slot 1 Red, team 2 (item data 1), China; slot 2 closed; 5,000 starting money; a click on
    ///   the superweapon box clears the limit; the slider's track message at 61 writes `--`; the name entry's update
    ///   renames slot 0;
    /// - **Start**: `Skirmish.ini` written (the map, the colour, `FPS`), and the host asked for `StartSkirmish` with
    ///   `GAME_SKIRMISH`, `DIFFICULTY_NORMAL`, the speed 1000 (the slider's 61 is "no limit"), the game started
    ///   (`startGame(0)`: in progress, slots 3 to 7 closed); `buttonPushed` set: a second *Start* does nothing.
    ///
    /// **Result (2026-11-10):** as predicted, with two misses. The superweapon box's click did nothing on the first
    /// run: the synthetic check box had no `MOUSETRACK` style, so the mouse never hilited it and `GadgetCheckBoxInput`
    /// ignored the release (the test's file, fixed: `synthetic::win` gives check boxes `MOUSETRACK`). And the load
    /// screen's labels (`updateMapStartSpots(..., TRUE)`, called by the test) were predicted on slot 1's spot: they
    /// follow `getApparentStartPos`, which for an enemy computer with `ShowRandomStartPos` is its original spot (-1),
    /// so no label; the local player's own spot is labelled (the assertion now says so).
    #[test]
    fn the_skirmish_screen_sets_up_and_starts_a_game() {
        let (mut wm, mut h) = opened();
        let top = h.shell.top().expect("top");
        assert_eq!(wm.layouts[top].filename, b"Menus/SkirmishGameOptionsMenu.wnd");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(h.ui_interacts.contains(&"SHELL_SCRIPT_HOOK_SKIRMISH_OPENED"));
        assert!(h.untranslated.iter().any(|f| f == b"populateSkirmishBattleHonors"), "{:?}", h.untranslated);
        let g = h.skirmish.game.clone().expect("TheSkirmishGameInfo");
        assert!(g.in_game && !g.in_progress);
        assert_eq!(g.slots[0].state, slot_state::PLAYER);
        assert_eq!(String::from_utf16_lossy(&g.slots[0].name), "Me");
        assert_eq!((g.slots[0].color, g.slots[0].player_template), (1, 1));
        assert_eq!(g.slots[1].state, slot_state::EASY_AI);
        for s in &g.slots[2..] {
            assert_eq!(s.state, slot_state::OPEN);
        }
        assert_eq!((g.map_name.as_slice(), g.map_crc, g.map_size), (BETA, 0x5678, 99));
        assert_eq!(g.starting_cash.count_money(), 20000);
        assert_eq!(g.superweapon_restriction, 1);
        assert_eq!(text(&wm, named(&wm, F, "TextEntryMapDisplay")), "Beta (4)");
        let spots: Vec<WinId> = (0..8).map(|i| named(&wm, F, &format!("ButtonMapStartPosition{i}"))).collect();
        let map_window = named(&wm, F, "MapWindow");
        let (mx, my) = wm.tree.screen_position(map_window);
        for (i, at) in [(12, 12), (172, 12), (12, 172), (172, 172)].iter().enumerate() {
            assert!(!hidden(&wm, spots[i]));
            assert_eq!(wm.tree.screen_position(spots[i]), (mx + at.0, my + at.1), "spot {i}");
        }
        assert!(spots[4..].iter().all(|&b| hidden(&wm, b)));
        let cash = named(&wm, F, "ComboBoxStartingCash");
        assert_eq!(cb::get_selected_pos(&mut wm, &mut h, cash, -1), 2);
        assert!(check_box_is_checked(&wm, named(&wm, F, "CheckboxLimitSuperweapons")));
        let slider = named(&wm, F, "SliderGameSpeed");
        assert_eq!(slider_get_position(&wm, slider), 45);
        assert_eq!(text(&wm, named(&wm, F, "StaticTextGameSpeed")), "45");
        let c0 = named(&wm, F, "ComboBoxColor0");
        let colors: Vec<i32> = (0..cb::get_length(&wm, c0)).map(|i| cb::get_item_data(&mut wm, &mut h, c0, i) as i32).collect();
        assert_eq!(colors, [-1, 0, 1, 2]);
        assert_eq!(cb::get_selected_pos(&mut wm, &mut h, c0, -1), 2);
        let t0 = named(&wm, F, "ComboBoxPlayerTemplate0");
        let templates: Vec<i32> = (0..cb::get_length(&wm, t0)).map(|i| cb::get_item_data(&mut wm, &mut h, t0, i) as i32).collect();
        assert_eq!(templates, [-1, 1, 2]);
        for i in 2..8 {
            for n in ["ComboBoxColor", "ComboBoxPlayerTemplate", "ComboBoxTeam"] {
                assert!(!enabled(&wm, named(&wm, F, &format!("{n}{i}"))), "{n}{i}");
            }
        }
        assert!(enabled(&wm, named(&wm, F, "ComboBoxColor1")));
        // the start spots
        click(&mut wm, &mut h, spots[2]);
        assert_eq!(h.skirmish.game.as_ref().map(|g| g.slots[0].start_pos), Some(2));
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(spots[2])), "T:NUMBER:1");
        click(&mut wm, &mut h, spots[2]);
        let g = h.skirmish.game.clone().expect("game");
        assert_eq!((g.slots[0].start_pos, g.slots[1].start_pos), (-1, 2));
        let p = named(&wm, F, "SkirmishGameOptionsMenuParent");
        wm.win_send_system_msg(&mut h, Some(p), ggm::GBM_SELECTED_RIGHT, win_data(Some(spots[2])), 0);
        assert_eq!(h.skirmish.game.as_ref().map(|g| g.slots[1].start_pos), Some(-1));
        // the combo boxes
        { let c = named(&wm, F, "ComboBoxColor1"); choose(&mut wm, &mut h, c, 0); }
        { let c = named(&wm, F, "ComboBoxTeam1"); choose(&mut wm, &mut h, c, 1); }
        { let c = named(&wm, F, "ComboBoxPlayerTemplate1"); choose(&mut wm, &mut h, c, 2); }
        { let c = named(&wm, F, "ComboBoxPlayer2"); choose(&mut wm, &mut h, c, slot_state::CLOSED); }
        choose(&mut wm, &mut h, cash, 5000);
        let g = h.skirmish.game.clone().expect("game");
        assert_eq!((g.slots[1].color, g.slots[1].team_number, g.slots[1].player_template), (0, 1, 2));
        assert_eq!(g.slots[2].state, slot_state::CLOSED);
        assert_eq!(g.starting_cash.count_money(), 5000);
        { let b = named(&wm, F, "CheckboxLimitSuperweapons"); click(&mut wm, &mut h, b); }
        assert_eq!(h.skirmish.game.as_ref().map(|g| g.superweapon_restriction), Some(0));
        wm.win_send_system_msg(&mut h, Some(p), ggm::GSM_SLIDER_TRACK, win_data(Some(slider)), 61);
        assert_eq!(text(&wm, named(&wm, F, "StaticTextGameSpeed")), "--");
        slider_set_position(&mut wm, &mut h, slider, 61);
        let entry = named(&wm, F, "TextEntryPlayerName");
        te::set_text(&mut wm, &mut h, Some(entry), &wtext("Neo"));
        wm.win_send_system_msg(&mut h, Some(p), te::gem::UPDATE_TEXT, win_data(Some(entry)), 0);
        assert_eq!(h.skirmish.game.as_ref().map(|g| String::from_utf16_lossy(&g.slots[0].name)), Some("Neo".to_string()));
        // Start
        h.requests.clear();
        { let b = named(&wm, F, "ButtonStart"); click(&mut wm, &mut h, b); }
        let written = h.prefs.get(&b"Skirmish.ini"[..]).cloned().expect("written");
        assert_eq!(written.get(&b"Map"[..]).map(|v| v.as_slice()), Some(BETA));
        assert_eq!(written.get(&b"FPS"[..]).map(|v| v.as_slice()), Some(&b"61"[..]));
        assert_eq!(written.get(&b"StartingCash"[..]).map(|v| v.as_slice()), Some(&b"5000"[..]));
        assert_eq!(written.get(&b"SuperweaponRestrict"[..]).map(|v| v.as_slice()), Some(&b"No"[..]));
        let [ShellRequest::StartSkirmish { game, mode, difficulty, max_fps }] = h.requests.as_slice() else { panic!("{:?}", h.requests) };
        assert_eq!((*mode, *difficulty, *max_fps), (game_mode::SKIRMISH, DIFFICULTY_NORMAL, 1000));
        assert!(game.in_progress);
        assert!(game.slots[3..].iter().all(|s| s.state == slot_state::CLOSED));
        assert_eq!(game.map_name, BETA);
        assert!(h.skirmish.button_pushed);
        { let b = named(&wm, F, "ButtonStart"); click(&mut wm, &mut h, b); }
        assert_eq!(h.requests.len(), 1, "buttonPushed");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // the module's other entry points: no disc, the cancel, the tooltip without its images, the load screen's labels
        h.skirmish.button_pushed = false;
        h.first_cd_present = false;
        check_for_cd_at_game_start(&mut wm, &mut h);
        assert!(h.untranslated.iter().any(|f| f.starts_with(b"ExMessageBoxOkCancel")));
        h.skirmish.button_pushed = true;
        cancel_start_because_of_no_cd(&mut h);
        assert!(!h.skirmish.button_pushed);
        let mw = h.skirmish.window_map.expect("map window");
        map_selector_tooltip(&mut wm, &mut h, mw, short_to_long(mx + 1, my + 1));
        assert!(wm.mouse.is_empty);
        init_skirmish_game_options();
        // (the load screen's labels follow the apparent start: with `ShowRandomStartPos` an enemy computer's is its
        // original one, -1, so slot 1's spot 3 stays blank; the local player's own is its real one)
        if let Some(g) = h.skirmish.game.as_mut() {
            g.slots[0].start_pos = 1;
            g.slots[1].start_pos = 3;
        }
        let o = h.skirmish.clone();
        update_map_start_spots(&mut wm, &mut h, &o.button_map_start_position, true);
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(spots[1])), "T:NUMBER:1");
        assert_eq!(String::from_utf16_lossy(wm.win_get_text(spots[3])), "");
    }

    /// #89: what *Start* refuses, *Back*, Escape, a solo map, and the shutdown's immediate path, predicted before the
    /// run:
    ///
    /// - on Alpha (two players) with three occupied slots: *Too many players* in a message box, `buttonPushed`
    ///   cleared, no start asked; with a map the cache lacks: *Can't find map*, then the end iterator read (recorded
    ///   as undefined);
    /// - Solo (not multiplayer): `updateSkirmishGameOptions` hides the per-player gadgets and the labels, and sets
    ///   the computer slots' player boxes to their first row (open), the screen's handlers opening them; a start then
    ///   asks for `GAME_SINGLE_PLAYER` (one player: only slot 0 is occupied), with a game in progress cleared first;
    /// - *Back* writes `Skirmish.ini`, pops the screen (`SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED`, the fade reversed, the
    ///   pop completed by the update once done: the main menu on top) and deletes `TheSkirmishGameInfo`; opened again
    ///   from what was written, Escape's release presses *Back* the same way; a `popImmediate` takes it off at once.
    ///
    /// **Result (2026-11-10):** as predicted once the test's own steps were right; three misses in them. The message
    /// box is modal: the next click on *Start* went to nothing until the box was closed (the test closes each box).
    /// The screen's fade starts two frames after its init, after the test's `settle` had returned: the next frame's
    /// `FLASH` hid the parent, so clicks missed (the test waits for it, `push_settled`). And reopened before that
    /// fade had started, *Back* reversed the group still holding the first opening's windows, freed: upstream's
    /// dangling pointers, recorded as undefined by the transitions (an upstream quirk kept; the test waits).
    #[test]
    fn the_skirmish_screen_refuses_backs_out_and_plays_solo() {
        let (mut wm, mut h) = opened();
        // too many players on Alpha
        if let Some(g) = h.skirmish.game.as_mut() {
            g.map_name = ALPHA.to_vec();
            g.slots[2].state = slot_state::MED_AI;
        }
        h.requests.clear();
        { let b = named(&wm, F, "ButtonStart"); click(&mut wm, &mut h, b); }
        assert!(h.requests.is_empty());
        assert!(!h.skirmish.button_pushed);
        assert!(wm.tree.find_by_name(b"MessageBox.wnd:MessageBoxParent").is_some_and(|b| !hidden(&wm, b)));
        // (the box is modal: it is closed before the next click)
        close_box(&mut wm, &mut h);
        // a map the cache lacks
        if let Some(g) = h.skirmish.game.as_mut() {
            g.map_name = b"maps\\gone\\gone.map".to_vec();
        }
        { let b = named(&wm, F, "ButtonStart"); click(&mut wm, &mut h, b); }
        assert!(wm.undefined.is_some_and(|u| u.contains("cache")));
        wm.undefined = None;
        close_box(&mut wm, &mut h);
        // a solo map
        if let Some(g) = h.skirmish.game.as_mut() {
            g.map_name = SOLO.to_vec();
        }
        h.skirmish.button_pushed = false;
        skirmish_position_start_spots(&mut wm, &mut h);
        assert!(hidden(&wm, named(&wm, F, "ComboBoxColor0")) && hidden(&wm, named(&wm, F, "StaticTextTeam")));
        assert!(h.skirmish.game.as_ref().is_some_and(|g| g.slots[1..].iter().all(|s| s.state == slot_state::OPEN)));
        h.env.in_game = true;
        h.requests.clear();
        { let b = named(&wm, F, "ButtonStart"); click(&mut wm, &mut h, b); }
        assert_eq!(h.requests.first(), Some(&ShellRequest::ClearGameData));
        assert!(matches!(h.requests.get(1), Some(ShellRequest::StartSkirmish { mode: game_mode::SINGLE_PLAYER, .. })), "{:?}", h.requests);
        h.env.in_game = false;
        // Back
        h.skirmish.button_pushed = false;
        { let b = named(&wm, F, "ButtonBack"); click(&mut wm, &mut h, b); }
        assert!(h.skirmish.game.is_none() && h.shell.pending_pop);
        assert!(h.ui_interacts.contains(&"SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED"));
        settle(&mut wm, &mut h);
        tick(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        assert_eq!(h.prefs.get(&b"Skirmish.ini"[..]).and_then(|p| p.get(&b"Map"[..])).map(|v| v.as_slice()), Some(SOLO));
        // again, from what was written (Solo is not a valid multiplayer map: the cache's first, Alpha), and Escape
        push_settled(&mut wm, &mut h, false);
        assert_eq!(h.skirmish.game.as_ref().map(|g| g.map_name.clone()), Some(ALPHA.to_vec()));
        let p = h.skirmish.parent_skirmish_game_options;
        wm.win_set_focus(&mut h, p);
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(h.skirmish.game.is_none() && h.shell.pending_pop);
        settle(&mut wm, &mut h);
        tick(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        // the immediate path
        push_settled(&mut wm, &mut h, true);
        assert_eq!(h.shell.get_screen_count(), 2);
        shell::pop_immediate(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let l = h.shell.top().expect("top");
        skirmish_game_options_menu_shutdown(&mut wm, &mut h, l, None);
        assert!(wm.undefined.is_some());
        assert_eq!(skirmish_game_options_menu_input(&mut wm, &mut h, 0, gwm::CHAR, u32::from(key::ENTER), key::STATE_UP), MsgHandled::Ignored);
    }

    /// #89: the preference readers, predicted from SkirmishGameOptionsMenu.cpp and UserPreferences.cpp before the run:
    /// no entries give the machine's name, colour -1, a random faction, the cache's first multiplayer map, no limit and
    /// the settings' default money; a colour of 3 (one past the three) is -1, a faction of 0 (the observer, no
    /// building) random; a name of spaces is the machine's; `StartingCash` `-5` is `strtoul`'s 4,294,967,291;
    /// `UseSystemMapDir` `YES` is true, `no` false; `getInt` of `12abc` is 12.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_skirmish_preferences_read_as_upstream_reads_them() {
        let (_wm, mut h) = booted();
        h.skirmish_data = data();
        let p = UserPreferences::default();
        assert!(p.get_user_name(&h).is_empty());
        assert_eq!((p.get_preferred_color(&h), p.get_preferred_faction(&h)), (-1, PLAYERTEMPLATE_RANDOM));
        assert_eq!(p.get_preferred_map(&mut h), ALPHA.to_vec());
        assert!(!p.get_superweapon_restricted());
        assert_eq!(p.get_starting_cash(&h).count_money(), 10000);
        assert!(p.uses_system_map_dir());
        let mut p = UserPreferences::default();
        for (k, v) in [(&b"Color"[..], &b"3"[..]), (b"PlayerTemplate", b"0"), (b"UserName", b"_20_00_20_00"), (b"StartingCash", b"-5"), (b"UseSystemMapDir", b"YES"), (b"FPS", b"12abc")] {
            p.set_ascii_string(k, v);
        }
        assert_eq!((p.get_preferred_color(&h), p.get_preferred_faction(&h)), (-1, PLAYERTEMPLATE_RANDOM));
        assert!(p.get_user_name(&h).is_empty());
        assert_eq!(p.get_starting_cash(&h).count_money(), 4_294_967_291);
        assert!(p.uses_system_map_dir());
        assert_eq!(p.get_int(b"FPS", 30), 12);
        p.set_ascii_string(b"UseSystemMapDir", b"no");
        assert!(!p.uses_system_map_dir());
        p.set_superweapon_restricted(true);
        assert!(p.get_superweapon_restricted());
    }
}
