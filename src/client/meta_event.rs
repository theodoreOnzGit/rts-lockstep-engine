// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/MetaEvent.cpp (GameMessageMetaTypeNames,
//     TheMetaMapFieldParseTable, MetaEventTranslator::translateGameMessage's key part, MetaMap::findGameMessageMetaType,
//     MetaMap::getMetaMapRec, MetaMap::parseMetaMap, INI::parseMetaMapDefinition)
//   GeneralsMD/Code/GameEngine/Include/GameClient/MetaEvent.h (MappableKeyCategories, MappableKeyType, KeyNames,
//     MappableKeyTransition, MappableKeyModState, CommandUsableInType and their names, MetaMapRec)
//   GeneralsMD/Code/GameEngine/Include/GameClient/KeyDefs.h (the KEY_STATE_* bits)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Keyboard.cpp (the modifier bits a key carries)
//   GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp (the two CommandMap files TheMetaMap loads)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #125: the key map (`TheMetaMap`, from `CommandMap.ini`) and `MetaEventTranslator`'s key part: a raw key message
//! becomes the meta message its binding names (`MSG_META_SELECT_NEXT_UNIT` for the tab key, ...), which the command
//! translator's cases then carry out ([`crate::client::command_xlat_meta`]).
//!
//! # As upstream reads it
//!
//! - `GameEngine::init` (GameEngine.cpp:524) loads `TheMetaMap` from `Data\<language>\CommandMap.ini`, then
//!   `Data\INI\CommandMap.ini`, each if present ([`MetaMap::load_files`]). A `CommandMap <NAME>` block names a meta message
//!   by `GameMessageMetaTypeNames` (unknown: `INI_INVALID_DATA`); `getMetaMapRec` finds the record of that message or
//!   **prepends** a new one (key `MK_NONE`, `DOWN`, no modifiers, usable nowhere, `CATEGORY_MISC`), and the block's
//!   fields (`Key`, `Transition`, `Modifiers`, `UseableIn`, `Category`, `Description`, `DisplayName`) fill it.
//! - `MetaEventTranslator::translateGameMessage` (priority 20) on `MSG_RAW_KEY_DOWN` / `UP` ([`MetaEventTranslator::translate_key`]):
//!   the key state's control, shift and alt bits (either side) make the modifier state; the records are tried in the list's
//!   order, skipping a game-only command before client frame 1 and one not usable where the shell is (shown or not).
//!   A record without a key fires on a change of the modifiers alone (an `UP` one when the old modifiers were its, a
//!   `DOWN` one when the new are); a record with the key and the same modifiers fires on its transition. A firing record
//!   appends its meta message (not for an auto-repeat) and destroys the key; `MSG_META_TOGGLE_FAST_FORWARD_REPLAY` instead
//!   toggles the global data's TiVO fast mode in a replay and keeps the key. A key down is kept as the last key; the
//!   modifiers become the last modifiers.
//!
//! # Upstream quirks that are kept
//!
//! - The list is in reverse order of the file (each new record is prepended), so of two records the later block wins.
//! - A record's `Transition = DOUBLEDOWN` never fires (the double-down test is commented out).
//! - The modifiers-only rule needs no key match: a modifier key's own message (or any key at a changed modifier state)
//!   fires it, before any record with a key is tried.
//!
//! # Where this differs from upstream
//!
//! - The keyboard is the application's (Bevy's events, natively and in the browser): [`KeyboardModifiers`] keeps the
//!   modifier bits as Keyboard.cpp's `translateKey` does, but per event, not per frame's batch; an auto-repeat is the
//!   window system's (Bevy's `repeat`), not `checkKeyRepeat`'s 10-frame delay.
//! - The TiVO fast mode is the replay viewer's: [`KeyResult::fast_forward_toggled`] says the key asked for it, and the
//!   message label; the client has no fast-forward yet (#125's gaps).
//! - The records' description and display name are translated with the game's text when one is given, as upstream; the
//!   key-mapping dialog that shows them is not translated.
//!
//! # Representation
//!
//! [`MetaMap`] holds the records in the list's order (index 0 is `getFirstMetaMapRec()`). The translator takes the key
//! message's two arguments and the frame, shell and replay it reads ([`KeyContext`]) and answers what upstream's stream would
//! get ([`KeyResult`]).

use crate::client::selection_xlat::Disposition;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, LookupListRec, UserData};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::common::science::{parse_and_translate_label, GameTextFetch};
use crate::common::unicode_string::UnicodeString;
use crate::logic::modules::plant;

/// `KEY_STATE_*` (KeyDefs.h).
pub mod key_state {
    pub const NONE: i32 = 0x0000;
    pub const UP: i32 = 0x0001;
    pub const DOWN: i32 = 0x0002;
    pub const LCONTROL: i32 = 0x0004;
    pub const RCONTROL: i32 = 0x0008;
    pub const LSHIFT: i32 = 0x0010;
    pub const RSHIFT: i32 = 0x0020;
    pub const LALT: i32 = 0x0040;
    pub const RALT: i32 = 0x0080;
    pub const AUTOREPEAT: i32 = 0x0100;
    pub const CAPSLOCK: i32 = 0x0200;
    pub const SHIFT2: i32 = 0x0400;
    pub const CONTROL: i32 = LCONTROL | RCONTROL;
    pub const SHIFT: i32 = LSHIFT | RSHIFT | SHIFT2;
    pub const ALT: i32 = LALT | RALT;
}

/// `MK_NONE` (`KEY_NONE`).
pub const MK_NONE: i32 = 0x00;

/// `enum MappableKeyTransition`.
pub mod transition {
    pub const DOWN: i32 = 0;
    pub const UP: i32 = 1;
    /// "if a key transition is repeated immediately, we generate this" (never fires: see the module docs).
    pub const DOUBLEDOWN: i32 = 2;
}

/// `enum MappableKeyModState`.
pub mod mod_state {
    use super::key_state;
    pub const NONE: i32 = 0;
    pub const CTRL: i32 = key_state::LCONTROL;
    pub const ALT: i32 = key_state::LALT;
    pub const SHIFT: i32 = key_state::LSHIFT;
    pub const CTRL_ALT: i32 = CTRL | ALT;
    pub const SHIFT_CTRL: i32 = SHIFT | CTRL;
    pub const SHIFT_ALT: i32 = SHIFT | ALT;
    pub const SHIFT_ALT_CTRL: i32 = SHIFT | ALT | CTRL;
}

/// `enum CommandUsableInType`.
pub mod usable_in {
    pub const NONE: u32 = 0;
    pub const SHELL: u32 = 1 << 0;
    pub const GAME: u32 = 1 << 1;
}

/// `CATEGORY_MISC` (`enum MappableKeyCategories`).
pub const CATEGORY_MISC: i32 = 6;

/// `CategoryListName`.
pub const CATEGORY_LIST_NAME: &[LookupListRec] = &[
    LookupListRec { name: "CONTROL", value: 0 },
    LookupListRec { name: "INFORMATION", value: 1 },
    LookupListRec { name: "INTERFACE", value: 2 },
    LookupListRec { name: "SELECTION", value: 3 },
    LookupListRec { name: "TAUNT", value: 4 },
    LookupListRec { name: "TEAM", value: 5 },
    LookupListRec { name: "MISC", value: 6 },
    LookupListRec { name: "DEBUG", value: 7 },
];

/// `TransitionNames`.
pub const TRANSITION_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "DOWN", value: transition::DOWN },
    LookupListRec { name: "UP", value: transition::UP },
    LookupListRec { name: "DOUBLEDOWN", value: transition::DOUBLEDOWN },
];

/// `ModifierNames`.
pub const MODIFIER_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "NONE", value: mod_state::NONE },
    LookupListRec { name: "CTRL", value: mod_state::CTRL },
    LookupListRec { name: "ALT", value: mod_state::ALT },
    LookupListRec { name: "SHIFT", value: mod_state::SHIFT },
    LookupListRec { name: "CTRL_ALT", value: mod_state::CTRL_ALT },
    LookupListRec { name: "SHIFT_CTRL", value: mod_state::SHIFT_CTRL },
    LookupListRec { name: "SHIFT_ALT", value: mod_state::SHIFT_ALT },
    LookupListRec { name: "SHIFT_ALT_CTRL", value: mod_state::SHIFT_ALT_CTRL },
];

/// `TheCommandUsableInNames`.
pub const THE_COMMAND_USABLE_IN_NAMES: &[&str] = &["SHELL", "GAME"];

/// `KeyNames` (MetaEvent.h): the `MappableKeyType` names, each `KEY_*` (`KeyDefs.h`, a DirectInput scan code).
pub const KEY_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "KEY_ESC", value: 0x01 },
    LookupListRec { name: "KEY_BACKSPACE", value: 0x0E },
    LookupListRec { name: "KEY_ENTER", value: 0x1C },
    LookupListRec { name: "KEY_SPACE", value: 0x39 },
    LookupListRec { name: "KEY_TAB", value: 0x0F },
    LookupListRec { name: "KEY_F1", value: 0x3B },
    LookupListRec { name: "KEY_F2", value: 0x3C },
    LookupListRec { name: "KEY_F3", value: 0x3D },
    LookupListRec { name: "KEY_F4", value: 0x3E },
    LookupListRec { name: "KEY_F5", value: 0x3F },
    LookupListRec { name: "KEY_F6", value: 0x40 },
    LookupListRec { name: "KEY_F7", value: 0x41 },
    LookupListRec { name: "KEY_F8", value: 0x42 },
    LookupListRec { name: "KEY_F9", value: 0x43 },
    LookupListRec { name: "KEY_F10", value: 0x44 },
    LookupListRec { name: "KEY_F11", value: 0x57 },
    LookupListRec { name: "KEY_F12", value: 0x58 },
    LookupListRec { name: "KEY_A", value: 0x1E },
    LookupListRec { name: "KEY_B", value: 0x30 },
    LookupListRec { name: "KEY_C", value: 0x2E },
    LookupListRec { name: "KEY_D", value: 0x20 },
    LookupListRec { name: "KEY_E", value: 0x12 },
    LookupListRec { name: "KEY_F", value: 0x21 },
    LookupListRec { name: "KEY_G", value: 0x22 },
    LookupListRec { name: "KEY_H", value: 0x23 },
    LookupListRec { name: "KEY_I", value: 0x17 },
    LookupListRec { name: "KEY_J", value: 0x24 },
    LookupListRec { name: "KEY_K", value: 0x25 },
    LookupListRec { name: "KEY_L", value: 0x26 },
    LookupListRec { name: "KEY_M", value: 0x32 },
    LookupListRec { name: "KEY_N", value: 0x31 },
    LookupListRec { name: "KEY_O", value: 0x18 },
    LookupListRec { name: "KEY_P", value: 0x19 },
    LookupListRec { name: "KEY_Q", value: 0x10 },
    LookupListRec { name: "KEY_R", value: 0x13 },
    LookupListRec { name: "KEY_S", value: 0x1F },
    LookupListRec { name: "KEY_T", value: 0x14 },
    LookupListRec { name: "KEY_U", value: 0x16 },
    LookupListRec { name: "KEY_V", value: 0x2F },
    LookupListRec { name: "KEY_W", value: 0x11 },
    LookupListRec { name: "KEY_X", value: 0x2D },
    LookupListRec { name: "KEY_Y", value: 0x15 },
    LookupListRec { name: "KEY_Z", value: 0x2C },
    LookupListRec { name: "KEY_1", value: 0x02 },
    LookupListRec { name: "KEY_2", value: 0x03 },
    LookupListRec { name: "KEY_3", value: 0x04 },
    LookupListRec { name: "KEY_4", value: 0x05 },
    LookupListRec { name: "KEY_5", value: 0x06 },
    LookupListRec { name: "KEY_6", value: 0x07 },
    LookupListRec { name: "KEY_7", value: 0x08 },
    LookupListRec { name: "KEY_8", value: 0x09 },
    LookupListRec { name: "KEY_9", value: 0x0A },
    LookupListRec { name: "KEY_0", value: 0x0B },
    LookupListRec { name: "KEY_KP1", value: 0x4F },
    LookupListRec { name: "KEY_KP2", value: 0x50 },
    LookupListRec { name: "KEY_KP3", value: 0x51 },
    LookupListRec { name: "KEY_KP4", value: 0x4B },
    LookupListRec { name: "KEY_KP5", value: 0x4C },
    LookupListRec { name: "KEY_KP6", value: 0x4D },
    LookupListRec { name: "KEY_KP7", value: 0x47 },
    LookupListRec { name: "KEY_KP8", value: 0x48 },
    LookupListRec { name: "KEY_KP9", value: 0x49 },
    LookupListRec { name: "KEY_KP0", value: 0x52 },
    LookupListRec { name: "KEY_MINUS", value: 0x0C },
    LookupListRec { name: "KEY_EQUAL", value: 0x0D },
    LookupListRec { name: "KEY_LBRACKET", value: 0x1A },
    LookupListRec { name: "KEY_RBRACKET", value: 0x1B },
    LookupListRec { name: "KEY_SEMICOLON", value: 0x27 },
    LookupListRec { name: "KEY_APOSTROPHE", value: 0x28 },
    LookupListRec { name: "KEY_TICK", value: 0x29 },
    LookupListRec { name: "KEY_BACKSLASH", value: 0x2B },
    LookupListRec { name: "KEY_COMMA", value: 0x33 },
    LookupListRec { name: "KEY_PERIOD", value: 0x34 },
    LookupListRec { name: "KEY_SLASH", value: 0x35 },
    LookupListRec { name: "KEY_UP", value: 0xC8 },
    LookupListRec { name: "KEY_DOWN", value: 0xD0 },
    LookupListRec { name: "KEY_LEFT", value: 0xCB },
    LookupListRec { name: "KEY_RIGHT", value: 0xCD },
    LookupListRec { name: "KEY_HOME", value: 0xC7 },
    LookupListRec { name: "KEY_END", value: 0xCF },
    LookupListRec { name: "KEY_PGUP", value: 0xC9 },
    LookupListRec { name: "KEY_PGDN", value: 0xD1 },
    LookupListRec { name: "KEY_INS", value: 0xD2 },
    LookupListRec { name: "KEY_DEL", value: 0xD3 },
    LookupListRec { name: "KEY_KPSLASH", value: 0xB5 },
    LookupListRec { name: "KEY_NONE", value: 0x00 },
];

/// `GameMessageMetaTypeNames` (MetaEvent.cpp) of the release build (none of `_ALLOW_DEBUG_CHEATS_IN_RELEASE`, `_DEBUG`,
/// `_INTERNAL`, `ALLOW_SURRENDER`, `DUMP_PERF_STATS`), in order.
pub const GAME_MESSAGE_META_TYPE_NAMES: &[(&str, MessageType)] = &[
    ("SAVE_VIEW1", MSG_META_SAVE_VIEW1),
    ("SAVE_VIEW2", MSG_META_SAVE_VIEW2),
    ("SAVE_VIEW3", MSG_META_SAVE_VIEW3),
    ("SAVE_VIEW4", MSG_META_SAVE_VIEW4),
    ("SAVE_VIEW5", MSG_META_SAVE_VIEW5),
    ("SAVE_VIEW6", MSG_META_SAVE_VIEW6),
    ("SAVE_VIEW7", MSG_META_SAVE_VIEW7),
    ("SAVE_VIEW8", MSG_META_SAVE_VIEW8),
    ("VIEW_VIEW1", MSG_META_VIEW_VIEW1),
    ("VIEW_VIEW2", MSG_META_VIEW_VIEW2),
    ("VIEW_VIEW3", MSG_META_VIEW_VIEW3),
    ("VIEW_VIEW4", MSG_META_VIEW_VIEW4),
    ("VIEW_VIEW5", MSG_META_VIEW_VIEW5),
    ("VIEW_VIEW6", MSG_META_VIEW_VIEW6),
    ("VIEW_VIEW7", MSG_META_VIEW_VIEW7),
    ("VIEW_VIEW8", MSG_META_VIEW_VIEW8),
    ("CREATE_TEAM0", MSG_META_CREATE_TEAM0),
    ("CREATE_TEAM1", MSG_META_CREATE_TEAM1),
    ("CREATE_TEAM2", MSG_META_CREATE_TEAM2),
    ("CREATE_TEAM3", MSG_META_CREATE_TEAM3),
    ("CREATE_TEAM4", MSG_META_CREATE_TEAM4),
    ("CREATE_TEAM5", MSG_META_CREATE_TEAM5),
    ("CREATE_TEAM6", MSG_META_CREATE_TEAM6),
    ("CREATE_TEAM7", MSG_META_CREATE_TEAM7),
    ("CREATE_TEAM8", MSG_META_CREATE_TEAM8),
    ("CREATE_TEAM9", MSG_META_CREATE_TEAM9),
    ("SELECT_TEAM0", MSG_META_SELECT_TEAM0),
    ("SELECT_TEAM1", MSG_META_SELECT_TEAM1),
    ("SELECT_TEAM2", MSG_META_SELECT_TEAM2),
    ("SELECT_TEAM3", MSG_META_SELECT_TEAM3),
    ("SELECT_TEAM4", MSG_META_SELECT_TEAM4),
    ("SELECT_TEAM5", MSG_META_SELECT_TEAM5),
    ("SELECT_TEAM6", MSG_META_SELECT_TEAM6),
    ("SELECT_TEAM7", MSG_META_SELECT_TEAM7),
    ("SELECT_TEAM8", MSG_META_SELECT_TEAM8),
    ("SELECT_TEAM9", MSG_META_SELECT_TEAM9),
    ("ADD_TEAM0", MSG_META_ADD_TEAM0),
    ("ADD_TEAM1", MSG_META_ADD_TEAM1),
    ("ADD_TEAM2", MSG_META_ADD_TEAM2),
    ("ADD_TEAM3", MSG_META_ADD_TEAM3),
    ("ADD_TEAM4", MSG_META_ADD_TEAM4),
    ("ADD_TEAM5", MSG_META_ADD_TEAM5),
    ("ADD_TEAM6", MSG_META_ADD_TEAM6),
    ("ADD_TEAM7", MSG_META_ADD_TEAM7),
    ("ADD_TEAM8", MSG_META_ADD_TEAM8),
    ("ADD_TEAM9", MSG_META_ADD_TEAM9),
    ("VIEW_TEAM0", MSG_META_VIEW_TEAM0),
    ("VIEW_TEAM1", MSG_META_VIEW_TEAM1),
    ("VIEW_TEAM2", MSG_META_VIEW_TEAM2),
    ("VIEW_TEAM3", MSG_META_VIEW_TEAM3),
    ("VIEW_TEAM4", MSG_META_VIEW_TEAM4),
    ("VIEW_TEAM5", MSG_META_VIEW_TEAM5),
    ("VIEW_TEAM6", MSG_META_VIEW_TEAM6),
    ("VIEW_TEAM7", MSG_META_VIEW_TEAM7),
    ("VIEW_TEAM8", MSG_META_VIEW_TEAM8),
    ("VIEW_TEAM9", MSG_META_VIEW_TEAM9),
    ("SELECT_MATCHING_UNITS", MSG_META_SELECT_MATCHING_UNITS),
    ("SELECT_NEXT_UNIT", MSG_META_SELECT_NEXT_UNIT),
    ("SELECT_PREV_UNIT", MSG_META_SELECT_PREV_UNIT),
    ("SELECT_NEXT_WORKER", MSG_META_SELECT_NEXT_WORKER),
    ("SELECT_PREV_WORKER", MSG_META_SELECT_PREV_WORKER),
    ("SELECT_HERO", MSG_META_SELECT_HERO),
    ("SELECT_ALL", MSG_META_SELECT_ALL),
    ("SELECT_ALL_AIRCRAFT", MSG_META_SELECT_ALL_AIRCRAFT),
    ("VIEW_COMMAND_CENTER", MSG_META_VIEW_COMMAND_CENTER),
    ("VIEW_LAST_RADAR_EVENT", MSG_META_VIEW_LAST_RADAR_EVENT),
    ("SCATTER", MSG_META_SCATTER),
    ("STOP", MSG_META_STOP),
    ("DEPLOY", MSG_META_DEPLOY),
    ("CREATE_FORMATION", MSG_META_CREATE_FORMATION),
    ("FOLLOW", MSG_META_FOLLOW),
    ("CHAT_PLAYERS", MSG_META_CHAT_PLAYERS),
    ("CHAT_ALLIES", MSG_META_CHAT_ALLIES),
    ("CHAT_EVERYONE", MSG_META_CHAT_EVERYONE),
    ("DIPLOMACY", MSG_META_DIPLOMACY),
    ("PLACE_BEACON", MSG_META_PLACE_BEACON),
    ("DELETE_BEACON", MSG_META_REMOVE_BEACON),
    ("OPTIONS", MSG_META_OPTIONS),
    ("TOGGLE_LOWER_DETAILS", MSG_META_TOGGLE_LOWER_DETAILS),
    ("TOGGLE_CONTROL_BAR", MSG_META_TOGGLE_CONTROL_BAR),
    ("BEGIN_PATH_BUILD", MSG_META_BEGIN_PATH_BUILD),
    ("END_PATH_BUILD", MSG_META_END_PATH_BUILD),
    ("BEGIN_FORCEATTACK", MSG_META_BEGIN_FORCEATTACK),
    ("END_FORCEATTACK", MSG_META_END_FORCEATTACK),
    ("BEGIN_FORCEMOVE", MSG_META_BEGIN_FORCEMOVE),
    ("END_FORCEMOVE", MSG_META_END_FORCEMOVE),
    ("BEGIN_WAYPOINTS", MSG_META_BEGIN_WAYPOINTS),
    ("END_WAYPOINTS", MSG_META_END_WAYPOINTS),
    ("BEGIN_PREFER_SELECTION", MSG_META_BEGIN_PREFER_SELECTION),
    ("END_PREFER_SELECTION", MSG_META_END_PREFER_SELECTION),
    ("TAKE_SCREENSHOT", MSG_META_TAKE_SCREENSHOT),
    ("ALL_CHEER", MSG_META_ALL_CHEER),
    ("BEGIN_CAMERA_ROTATE_LEFT", MSG_META_BEGIN_CAMERA_ROTATE_LEFT),
    ("END_CAMERA_ROTATE_LEFT", MSG_META_END_CAMERA_ROTATE_LEFT),
    ("BEGIN_CAMERA_ROTATE_RIGHT", MSG_META_BEGIN_CAMERA_ROTATE_RIGHT),
    ("END_CAMERA_ROTATE_RIGHT", MSG_META_END_CAMERA_ROTATE_RIGHT),
    ("BEGIN_CAMERA_ZOOM_IN", MSG_META_BEGIN_CAMERA_ZOOM_IN),
    ("END_CAMERA_ZOOM_IN", MSG_META_END_CAMERA_ZOOM_IN),
    ("BEGIN_CAMERA_ZOOM_OUT", MSG_META_BEGIN_CAMERA_ZOOM_OUT),
    ("END_CAMERA_ZOOM_OUT", MSG_META_END_CAMERA_ZOOM_OUT),
    ("CAMERA_RESET", MSG_META_CAMERA_RESET),
    ("TOGGLE_CAMERA_TRACKING_DRAWABLE", MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE),
    ("TOGGLE_FAST_FORWARD_REPLAY", MSG_META_TOGGLE_FAST_FORWARD_REPLAY),
    ("DEMO_INSTANT_QUIT", MSG_META_DEMO_INSTANT_QUIT),
];

/// `class MetaMapRec`.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaMapRec {
    /// `m_meta`: the meta-event to emit.
    pub meta: MessageType,
    /// `m_key`: the key we want (`MappableKeyType`).
    pub key: i32,
    /// `m_transition`.
    pub transition: i32,
    /// `m_modState`: the required state of the ctrl-alt-shift keys.
    pub mod_state: i32,
    /// `m_usableIn`.
    pub usable_in: u32,
    /// `m_category`.
    pub category: i32,
    /// `m_description`, `m_displayName`: for the key mapping dialog.
    pub description: UnicodeString,
    pub display_name: UnicodeString,
}

impl MetaMapRec {
    /// `getMetaMapRec`'s new record: no key, `DOWN`, no modifiers, usable nowhere, `CATEGORY_MISC`.
    pub fn new(meta: MessageType) -> Self {
        MetaMapRec {
            meta,
            key: MK_NONE,
            transition: transition::DOWN,
            mod_state: mod_state::NONE,
            usable_in: usable_in::NONE,
            category: CATEGORY_MISC,
            description: UnicodeString::new(),
            display_name: UnicodeString::new(),
        }
    }
}

/// `class MetaMap`: the records, `m_metaMaps`'s list in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetaMap {
    pub recs: Vec<MetaMapRec>,
}

/// A record with the game's text, while its block is read.
struct RecParse<'a> {
    rec: MetaMapRec,
    text: Option<&'a dyn GameTextFetch>,
}

/// `parseAndTranslateLabel` with the game's text; without it the label is kept as it reads.
fn label(ini: &mut Ini<'_>, text: Option<&dyn GameTextFetch>) -> IniResult<UnicodeString> {
    match text {
        Some(t) => parse_and_translate_label(ini, t),
        None => {
            let token = ini.get_next_token(None)?;
            let mut s = UnicodeString::new();
            for &b in &token {
                let _ = s.concat_char(u16::from(b));
            }
            Ok(s)
        }
    }
}

/// `TheMetaMapFieldParseTable`.
fn the_meta_map_field_parse_table<'a>() -> Vec<FieldParse<RecParse<'a>>> {
    vec![
        FieldParse::new("Key", |ini, r: &mut RecParse<'a>, ud| ini.parse_lookup_list(ud.lookup()).map(|v| r.rec.key = v), UserData::Lookup(KEY_NAMES)),
        FieldParse::new("Transition", |ini, r: &mut RecParse<'a>, ud| ini.parse_lookup_list(ud.lookup()).map(|v| r.rec.transition = v), UserData::Lookup(TRANSITION_NAMES)),
        FieldParse::new("Modifiers", |ini, r: &mut RecParse<'a>, ud| ini.parse_lookup_list(ud.lookup()).map(|v| r.rec.mod_state = v), UserData::Lookup(MODIFIER_NAMES)),
        FieldParse::new("UseableIn", |ini, r: &mut RecParse<'a>, ud| ini.parse_bit_string32(&mut r.rec.usable_in, ud.names()), UserData::Names(THE_COMMAND_USABLE_IN_NAMES)),
        FieldParse::new("Category", |ini, r: &mut RecParse<'a>, ud| ini.parse_lookup_list(ud.lookup()).map(|v| r.rec.category = v), UserData::Lookup(CATEGORY_LIST_NAME)),
        FieldParse::new("Description", |ini, r: &mut RecParse<'a>, _| label(ini, r.text).map(|s| r.rec.description = s), UserData::None),
        FieldParse::new("DisplayName", |ini, r: &mut RecParse<'a>, _| label(ini, r.text).map(|s| r.rec.display_name = s), UserData::None),
    ]
}

/// The context the `CommandMap` block fills.
pub struct MetaMapLoad<'a> {
    pub map: &'a mut MetaMap,
    pub text: Option<&'a dyn GameTextFetch>,
}

/// `INI::parseMetaMapDefinition` / `MetaMap::parseMetaMap(ini)`: "read and ignore the meta-map name" (it names the meta
/// message), the record of that message, then its fields.
fn parse_meta_map_definition(ini: &mut Ini<'_>, ctx: &mut MetaMapLoad<'_>) -> IniResult<()> {
    let c = ini.get_next_token(None)?;
    let t = MetaMap::find_game_message_meta_type(&c);
    if t == MSG_INVALID {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let i = ctx.map.get_meta_map_rec(t);
    let mut r = RecParse { rec: ctx.map.recs[i].clone(), text: ctx.text };
    let table = the_meta_map_field_parse_table();
    let res = ini.init_from_ini(&mut r, &table);
    // (upstream fills the record in place: what was read before an error stays)
    ctx.map.recs[i] = r.rec;
    res
}

impl MetaMap {
    /// `MetaMap()`.
    pub fn new() -> Self {
        MetaMap::default()
    }

    /// `getFirstMetaMapRec()`'s list.
    pub fn records(&self) -> &[MetaMapRec] {
        &self.recs
    }

    /// `findGameMessageMetaType(name)`: the meta message of a name, without case; `MSG_INVALID` for none ("did you remember
    /// to add it to GameMessageMetaTypeNames[] ?", a `DEBUG_CRASH`).
    pub fn find_game_message_meta_type(name: &[u8]) -> MessageType {
        GAME_MESSAGE_META_TYPE_NAMES.iter().find(|(n, _)| n.as_bytes().eq_ignore_ascii_case(name)).map_or(MSG_INVALID, |&(_, t)| t)
    }

    /// `getMetaMapRec(t)`: the index of `t`'s record; "not found.. create a new one", at the list's head.
    pub fn get_meta_map_rec(&mut self, t: MessageType) -> usize {
        if let Some(i) = self.recs.iter().position(|r| r.meta == t) {
            return i;
        }
        if plant(9144) {
            self.recs.push(MetaMapRec::new(t));
            return self.recs.len() - 1;
        }
        self.recs.insert(0, MetaMapRec::new(t));
        0
    }

    /// `INI::load(path, INI_LOAD_OVERWRITE)` of one CommandMap file (`text`: the game's, for the labels).
    pub fn load_text(&mut self, path: &[u8], data: &[u8], text: Option<&dyn GameTextFetch>) -> IniResult<()> {
        let blocks = [BlockParse { token: "CommandMap", parse: parse_meta_map_definition }];
        let mut ctx = MetaMapLoad { map: self, text };
        Ini::load(path, data, IniLoadType::Overwrite, None, &blocks, &mut ctx)
    }

    /// `initSubsystem(TheMetaMap, ..., "Data\\<language>\\CommandMap.ini", "Data\\INI\\CommandMap.ini")`: both files, each
    /// if `open` has it (upstream's `INI::load` of a missing file throws, which `initSubsystem` lets through: the game
    /// does not start without its CommandMap). Whether any file was read.
    pub fn load_files(&mut self, language: &str, open: impl Fn(&[u8]) -> Option<Vec<u8>>, text: Option<&dyn GameTextFetch>) -> IniResult<bool> {
        let mut any = false;
        for p in [format!("Data\\{language}\\CommandMap.ini").into_bytes(), b"Data\\INI\\CommandMap.ini".to_vec()] {
            if let Some(data) = open(&p) {
                self.load_text(&p, &data, text)?;
                any = true;
            }
        }
        Ok(any)
    }
}

/// What the translator reads beyond the message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyContext {
    /// `TheGameClient->getFrame()`.
    pub client_frame: u32,
    /// `TheShell->isShellActive()`.
    pub shell_active: bool,
    /// `TheGameLogic->isInReplayGame()`.
    pub in_replay: bool,
}

/// What a key message gives the stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyResult {
    /// The key message's disposition.
    pub disposition: Option<Disposition>,
    /// The meta messages appended (`TheMessageStream->appendMessage(map->m_meta)`), in order.
    pub metas: Vec<MessageType>,
    /// `TheWritableGlobalData->m_TiVOFastMode = 1 - ...` was asked (with `TheInGameUI->message(GUI:FF_ON / FF_OFF)`).
    pub fast_forward_toggled: bool,
}

/// `class MetaEventTranslator`'s key members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MetaEventTranslator {
    /// `m_lastKeyDown` (`MK_NONE` at first).
    pub last_key_down: i32,
    /// `m_lastModState`.
    pub last_mod_state: i32,
}

impl MetaEventTranslator {
    /// `translateGameMessage` on `MSG_RAW_KEY_DOWN` (`down`) / `MSG_RAW_KEY_UP` with the key and its state (the message's
    /// two integer arguments).
    pub fn translate_key(&mut self, map: &MetaMap, down: bool, key: i32, key_state: i32, ctx: &KeyContext) -> KeyResult {
        let mut out = KeyResult { disposition: Some(Disposition::Keep), ..Default::default() };
        // "for our purposes here, we don't care to distinguish between right and left keys, so just fudge a little to
        // simplify things."
        let mut new_mod_state = 0;
        if key_state & key_state::CONTROL != 0 {
            new_mod_state |= mod_state::CTRL;
        }
        if key_state & key_state::SHIFT != 0 {
            new_mod_state |= mod_state::SHIFT;
        }
        if key_state & key_state::ALT != 0 {
            new_mod_state |= mod_state::ALT;
        }
        for map in map.records() {
            // "if this command is *only* usable in the game, we will ignore it if the game client has not yet incremented
            // to frame 1"
            if map.usable_in == usable_in::GAME && ctx.client_frame < 1 && !plant(9143) {
                continue;
            }
            // "if the shell is active, and this command is not usable in shell, continue"
            if ctx.shell_active && map.usable_in & usable_in::SHELL == 0 {
                continue;
            }
            // "if the shell is not active and this command is not usable in the game, continue"
            if !ctx.shell_active && map.usable_in & usable_in::GAME == 0 {
                continue;
            }
            // "check for the special case of mods-only-changed."
            let up_mods = if plant(9142) { new_mod_state } else { self.last_mod_state };
            if map.key == MK_NONE
                && new_mod_state != self.last_mod_state
                && ((map.transition == transition::UP && map.mod_state == up_mods) || (map.transition == transition::DOWN && map.mod_state == new_mod_state))
            {
                out.metas.push(map.meta);
                out.disposition = Some(Disposition::Destroy);
                break;
            }
            // "ok, now check for "normal" key transitions."
            if map.key == key
                && map.mod_state == new_mod_state
                && ((map.transition == transition::UP && key_state & key_state::UP != 0) || (map.transition == transition::DOWN && key_state & key_state::DOWN != 0))
            {
                if key_state & key_state::AUTOREPEAT != 0 && !plant(9141) {
                    // "if it's an autorepeat of a "known" key, don't generate the meta-event, but DO eat the keystroke so
                    // no one else can mess with it"
                } else {
                    // "THIS IS A GREASY HACK... MESSAGE SHOULD BE HANDLED IN A TRANSLATOR, BUT DURING CINEMATICS THE
                    // TRANSLATOR IS DISABLED"
                    if map.meta == MSG_META_TOGGLE_FAST_FORWARD_REPLAY {
                        if ctx.in_replay {
                            out.fast_forward_toggled = true;
                        }
                        // "cause for goodness sake, this key gets used a lot by non-replay hotkeys"
                        out.disposition = Some(Disposition::Keep);
                        break;
                    }
                    out.metas.push(map.meta);
                }
                out.disposition = Some(Disposition::Destroy);
                break;
            }
        }
        if down {
            self.last_key_down = key;
        }
        self.last_mod_state = new_mod_state;
        out
    }
}

/// Keyboard.cpp's modifier bits (`m_modifiers`), which every key message of a frame carries: kept per key event here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardModifiers {
    pub bits: i32,
}

/// The DirectInput codes of the modifier keys (`KEY_LSHIFT`, ...).
pub mod modifier_key {
    pub const LSHIFT: u8 = 0x2A;
    pub const RSHIFT: u8 = 0x36;
    pub const LCTRL: u8 = 0x1D;
    pub const RCTRL: u8 = 0x9D;
    pub const LALT: u8 = 0x38;
    pub const RALT: u8 = 0xB8;
}

impl KeyboardModifiers {
    /// `Keyboard::translateKey`'s modifier keys and `updateKeys`' `BitSet(state, m_modifiers)`: a key event's state
    /// (`KEY_STATE_DOWN` / `UP`, `KEY_STATE_AUTOREPEAT` for a repeat, and the modifiers after this key).
    pub fn key_state(&mut self, key: u8, down: bool, repeat: bool) -> i32 {
        let bit = match key {
            modifier_key::LSHIFT => key_state::LSHIFT,
            modifier_key::RSHIFT => key_state::RSHIFT,
            modifier_key::LCTRL => key_state::LCONTROL,
            modifier_key::RCTRL => key_state::RCONTROL,
            modifier_key::LALT => key_state::LALT,
            modifier_key::RALT => key_state::RALT,
            _ => 0,
        };
        if down {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
        let base = if repeat {
            // "note: not a bitset; this is an assignment" (the modifiers are set into it after)
            key_state::DOWN | key_state::AUTOREPEAT
        } else if down {
            key_state::DOWN
        } else {
            key_state::UP
        };
        base | self.bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Predicted before the run: a CommandMap with the tab key's `SELECT_NEXT_UNIT` (game), `BEGIN_FORCEATTACK` on CTRL
    /// alone (DOWN) and `END_FORCEATTACK` (UP), and a later block for `SELECT_NEXT_UNIT` again (its record is reused):
    /// the records are in reverse order of first appearance (`END`, `BEGIN`, `NEXT`); tab at frame 0 gives nothing (game
    /// only), at frame 1 `MSG_META_SELECT_NEXT_UNIT` and the key destroyed; a repeat is eaten without a meta; ctrl down
    /// gives `BEGIN_FORCEATTACK`, ctrl up `END_FORCEATTACK`; an unknown block name is an error.
    #[test]
    fn the_command_map_turns_keys_into_meta_messages() {
        let ini = b"CommandMap SELECT_NEXT_UNIT\n  Key = KEY_TAB\n  Transition = DOWN\n  Modifiers = NONE\n  UseableIn = GAME\n  Category = SELECTION\nEnd\n\
CommandMap BEGIN_FORCEATTACK\n  Key = KEY_NONE\n  Transition = DOWN\n  Modifiers = CTRL\n  UseableIn = GAME\nEnd\n\
CommandMap END_FORCEATTACK\n  Key = KEY_NONE\n  Transition = UP\n  Modifiers = CTRL\n  UseableIn = GAME\nEnd\n\
CommandMap select_next_unit\n  Key = KEY_TAB\nEnd\n";
        let mut m = MetaMap::new();
        m.load_text(b"Data\\INI\\CommandMap.ini", ini, None).expect("reads");
        let order: Vec<MessageType> = m.records().iter().map(|r| r.meta).collect();
        assert_eq!(order, vec![MSG_META_END_FORCEATTACK, MSG_META_BEGIN_FORCEATTACK, MSG_META_SELECT_NEXT_UNIT]);
        assert_eq!(m.records()[2].category, 3);
        let mut x = MetaEventTranslator::default();
        let mut kb = KeyboardModifiers::default();
        let tab = 0x0F;
        let game0 = KeyContext { client_frame: 0, ..Default::default() };
        let game = KeyContext { client_frame: 1, ..Default::default() };
        let s = kb.key_state(tab, true, false);
        assert_eq!(x.translate_key(&m, true, i32::from(tab), s, &game0).metas, vec![]);
        let r = x.translate_key(&m, true, i32::from(tab), s, &game);
        assert_eq!((r.metas.clone(), r.disposition), (vec![MSG_META_SELECT_NEXT_UNIT], Some(Disposition::Destroy)));
        let s = kb.key_state(tab, true, true);
        let r = x.translate_key(&m, true, i32::from(tab), s, &game);
        assert_eq!((r.metas.clone(), r.disposition), (vec![], Some(Disposition::Destroy)));
        let s = kb.key_state(modifier_key::LCTRL, true, false);
        assert_eq!(x.translate_key(&m, true, i32::from(modifier_key::LCTRL), s, &game).metas, vec![MSG_META_BEGIN_FORCEATTACK]);
        let s = kb.key_state(modifier_key::LCTRL, false, false);
        assert_eq!(x.translate_key(&m, false, i32::from(modifier_key::LCTRL), s, &game).metas, vec![MSG_META_END_FORCEATTACK]);
        assert!(MetaMap::new().load_text(b"x", b"CommandMap NO_SUCH_THING\nEnd\n", None).is_err());
    }
}
