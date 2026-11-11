// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/HotKey.cpp (HotKeyTranslator::translateGameMessage,
//     HotKey::HotKey, HotKeyManager::init, reset, addHotKey, executeHotKey, searchHotKey)
//   GeneralsMD/Code/GameEngine/Include/GameClient/HotKey.h (HotKeyTranslator, HotKey, HotKeyManager)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (setControlCommand's hot key: searchHotKey
//     of the button's text label, then addHotKey; switchToContext's reset)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #131: the control bar's hot keys. A command button's text, through the game's text, marks its hot key with
//! `&` (`"&Build Barracks"`); pressing that letter presses the button.
//!
//! # As upstream reads it
//!
//! - `GameClient::init` makes `TheHotKeyManager` and attaches `HotKeyTranslator` to the stream at priority 25, after
//!   `MetaEventTranslator` (20) and before `PlaceEventTranslator` (30) (GameClient.cpp:299).
//! - `ControlBar::setControlCommand(button, command)` (ControlBar.cpp:2496) asks `searchHotKey(command->getTextLabel())`:
//!   the label through `TheGameText`, and the character after its first `&` (none: no hot key); a hot key is added for
//!   the button's window. `populateCommand` calls it for every button of the command set but the script-only ones and
//!   the transport's exit buttons, before it hides the ones that need a science the player lacks or that the
//!   availability hides. `switchToContext` and `populateStructureInventory` reset the manager first.
//! - `addHotKey(win, key)`: the key lowercased; a key already mapped is kept (a `DEBUG_ASSERTCRASH`), else mapped to the
//!   window.
//! - `HotKeyTranslator::translateGameMessage` acts on `MSG_RAW_KEY_UP` only: with control, shift or alt held it keeps the
//!   key; else the key's plain printable name (`TheKeyboard->getPrintableKey(key, 0)`, [`crate::client::keyboard`]),
//!   narrowed to a byte, goes to `executeHotKey`, and the key is destroyed when that answers TRUE.
//! - `executeHotKey(key)`: the lowercased key's window; nothing for none. A window that is not hidden: enabled, its parent
//!   gets `GBM_SELECTED` (the button is pressed) and the `GUIClick` sound plays, TRUE; disabled, `GUIClickDisabled`
//!   plays, FALSE. A hidden window: FALSE, silently.
//!
//! # Upstream quirks that are kept
//!
//! - The key goes off on its release, not its press, and only without modifiers.
//! - A hot key registered for a button that is then hidden (a science the player lacks, a hidden command) still blocks
//!   a later button with the same letter; pressing it does nothing (the window is hidden).
//! - `executeHotKey` tests only the window's own hidden bit, not its parents'.
//! - A key whose name is not 7-bit keeps its low byte (`AsciiString::translate`'s `(char)` cast); lowercasing is ASCII
//!   only (`tolower` in the C locale).
//!
//! # Where this differs from upstream
//!
//! - `HotKeyTranslator` builds its key with `UnicodeString::set(&key)` on a single `WideChar`, which reads on to the next
//!   zero on the stack: here the name is the one character.
//! - The window is the caller's id (the client: the command window's slot); `GBM_SELECTED` and the sounds are the
//!   host's ([`HotKeyHost`]). The control bar's other buttons (options, idle worker, beacon, general, the up-down
//!   button) get their hot keys in `ControlBar::init`, which the first `switchToContext` resets away: not registered here
//!   either.
//! - The label's hot key is the button's own text label's (as on the first `setControlCommand`); upstream's
//!   `copyButtonTextFrom` then writes a purchase-science button's label into the shared, `const` button, which later
//!   populations would read (the port never changes the logic's buttons).

use crate::client::selection_xlat::Disposition;
use crate::common::message_stream::MessageType;
use crate::common::message_types::MSG_RAW_KEY_UP;
use crate::logic::modules::plant;
use std::collections::BTreeMap;

// `KEY_STATE_CONTROL`, `KEY_STATE_SHIFT`, `KEY_STATE_ALT` (KeyDefs.h).
use crate::client::meta_event::key_state;

/// `class HotKey`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HotKey {
    /// `m_win` (`NULL` at first).
    pub win: Option<usize>,
    /// `m_key`.
    pub key: Vec<u8>,
}

/// What `executeHotKey` asks of the window manager and the audio.
pub trait HotKeyHost {
    /// `BitTest(win->winGetStatus(), WIN_STATUS_HIDDEN)`.
    fn win_hidden(&self, win: usize) -> bool;
    /// `BitTest(win->winGetStatus(), WIN_STATUS_ENABLED)`.
    fn win_enabled(&self, win: usize) -> bool;
    /// `TheWindowManager->winSendSystemMsg(win->winGetParent(), GBM_SELECTED, win, win->winGetWindowId())`.
    fn send_selected(&mut self, win: usize);
    /// `TheAudio->addAudioEvent(&AudioEventRTS(name))`.
    fn add_audio_event(&mut self, name: &[u8]);
}

/// `class HotKeyManager`: `m_hotKeyMap`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HotKeyManager {
    pub hot_key_map: BTreeMap<Vec<u8>, HotKey>,
}

/// `AsciiString::toLower()`: `tolower` of each byte in the C locale (ASCII letters only).
fn to_lower(key: &[u8]) -> Vec<u8> {
    key.iter().map(|b| if plant(9184) { *b } else { b.to_ascii_lowercase() }).collect()
}

/// `AsciiString::translate(UnicodeString)` of one character: its low byte (`(char)`), none for a zero byte
/// (`concat('\0')` appends nothing).
fn narrow(c: u16) -> Vec<u8> {
    let b = c as u8;
    if b == 0 {
        Vec::new()
    } else {
        vec![b]
    }
}

impl HotKeyManager {
    /// `HotKeyManager()`.
    pub fn new() -> Self {
        HotKeyManager::default()
    }

    /// `init()`, `reset()`: the map cleared.
    pub fn reset(&mut self) {
        self.hot_key_map.clear();
    }

    /// `addHotKey(win, key)`: the key lowercased, mapped to `win` unless already mapped ("Hotkey %s is already mapped
    /// to window %s", a `DEBUG_ASSERTCRASH`, and the first mapping kept).
    pub fn add_hot_key(&mut self, win: usize, key_in: &[u8]) {
        let key = to_lower(key_in);
        if self.hot_key_map.contains_key(&key) && !plant(9183) {
            return;
        }
        let hk = HotKey { win: Some(win), key: key.clone() };
        self.hot_key_map.insert(key, hk);
    }

    /// `executeHotKey(key)`: whether the key pressed its window's button (see the module docs).
    pub fn execute_hot_key(&self, key_in: &[u8], h: &mut impl HotKeyHost) -> bool {
        let key = to_lower(key_in);
        let Some(hk) = self.hot_key_map.get(&key) else { return false };
        let Some(win) = hk.win else { return false };
        if !h.win_hidden(win) || plant(9185) {
            if h.win_enabled(win) {
                h.send_selected(win);
                // "here we make the same click sound that the GUI uses when you click a button"
                h.add_audio_event(b"GUIClick");
                return true;
            }
            if !plant(9186) {
                h.add_audio_event(b"GUIClickDisabled");
            }
        }
        false
    }

    /// `searchHotKey(label)` with the label already through `TheGameText` (`searchHotKey(UnicodeString)`): the character
    /// after the first `&`, narrowed to a byte; empty for none.
    pub fn search_hot_key(u: &[u16]) -> Vec<u8> {
        if u.is_empty() {
            return Vec::new();
        }
        let amp = u16::from(b'&');
        let at = if plant(9187) { u.iter().rposition(|&c| c == amp) } else { u.iter().position(|&c| c == amp) };
        match at {
            // "found a '&' - now look for the next char" (the terminator past the end)
            Some(i) => narrow(u.get(i + 1).copied().unwrap_or(0)),
            None => Vec::new(),
        }
    }
}

/// `class HotKeyTranslator` (stateless).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotKeyTranslator;

impl HotKeyTranslator {
    /// `translateGameMessage` on a key message `t` with its key and state (the message's two integer arguments);
    /// `names` is `TheKeyboard`'s ([`crate::client::keyboard::KeyNames`]), `manager` `TheHotKeyManager`.
    pub fn translate_game_message(
        &self,
        t: MessageType,
        key: i32,
        state: i32,
        names: &crate::client::keyboard::KeyNames,
        manager: &HotKeyManager,
        h: &mut impl HotKeyHost,
    ) -> Disposition {
        let mut disp = Disposition::Keep;
        if t == MSG_RAW_KEY_UP || (plant(9181) && t == crate::common::message_types::MSG_RAW_KEY_DOWN) {
            // "for our purposes here, we don't care to distinguish between right and left keys"
            let mut new_mod_state = 0;
            if state & key_state::CONTROL != 0 {
                new_mod_state |= crate::client::meta_event::mod_state::CTRL;
            }
            if state & key_state::SHIFT != 0 {
                new_mod_state |= crate::client::meta_event::mod_state::SHIFT;
            }
            if state & key_state::ALT != 0 {
                new_mod_state |= crate::client::meta_event::mod_state::ALT;
            }
            if new_mod_state != 0 && !plant(9182) {
                return disp;
            }
            // `getPrintableKey(msg->getArgument(0)->integer, 0)`: the key as an `UnsignedByte`
            let c = names.get_printable_key(key as u8, if plant(9188) { 1 } else { 0 });
            let a_key = narrow(c);
            if manager.execute_hot_key(&a_key, h) {
                disp = Disposition::Destroy;
            }
        }
        disp
    }
}

/// `setControlCommand`'s hot key for a command button: `searchHotKey` of its text label (`label`, already through the
/// game's text), added for `win` when there is one.
pub fn set_control_command_hot_key(manager: &mut HotKeyManager, win: usize, label: &[u16]) {
    let hot_key = HotKeyManager::search_hot_key(label);
    if !hot_key.is_empty() {
        manager.add_hot_key(win, &hot_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::keyboard::{language_id, KeyNames, LAYOUT_US_ENGLISH};

    #[derive(Default)]
    struct Wins {
        hidden: Vec<bool>,
        enabled: Vec<bool>,
        selected: Vec<usize>,
        sounds: Vec<Vec<u8>>,
    }
    impl HotKeyHost for Wins {
        fn win_hidden(&self, win: usize) -> bool {
            self.hidden[win]
        }
        fn win_enabled(&self, win: usize) -> bool {
            self.enabled[win]
        }
        fn send_selected(&mut self, win: usize) {
            self.selected.push(win);
        }
        fn add_audio_event(&mut self, name: &[u8]) {
            self.sounds.push(name.to_vec());
        }
    }

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// Predicted before the run: "&Barracks" gives `B`, "Power &Plant" `P`, a trailing `&` and no `&` nothing; the
    /// second `b` button does not replace the first; B's release (no modifiers) presses window 0 with `GUIClick` and is
    /// destroyed; its press does nothing; with shift held nothing; P on a disabled window plays `GUIClickDisabled` and
    /// keeps the key; on a hidden window nothing at all; after a reset nothing is mapped.
    #[test]
    fn a_buttons_letter_presses_it_on_release() {
        assert_eq!(HotKeyManager::search_hot_key(&u("&Barracks")), b"B".to_vec());
        assert_eq!(HotKeyManager::search_hot_key(&u("Power &Plant")), b"P".to_vec());
        assert_eq!(HotKeyManager::search_hot_key(&u("Trailing&")), Vec::<u8>::new());
        assert_eq!(HotKeyManager::search_hot_key(&u("None")), Vec::<u8>::new());
        let mut m = HotKeyManager::new();
        set_control_command_hot_key(&mut m, 0, &u("&Barracks"));
        set_control_command_hot_key(&mut m, 1, &u("&build"));
        set_control_command_hot_key(&mut m, 2, &u("Power &Plant"));
        assert_eq!(m.hot_key_map[&b"b".to_vec()].win, Some(0));
        let names = KeyNames::init_key_names(language_id::US, LAYOUT_US_ENGLISH);
        let mut w = Wins { hidden: vec![false, false, false], enabled: vec![true, true, false], ..Default::default() };
        let x = HotKeyTranslator;
        let (b, p) = (0x30, 0x19);
        use crate::common::message_types::MSG_RAW_KEY_DOWN;
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_DOWN, b, key_state::DOWN, &names, &m, &mut w), Disposition::Keep);
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_UP, b, key_state::UP | key_state::LSHIFT, &names, &m, &mut w), Disposition::Keep);
        assert!(w.selected.is_empty());
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_UP, b, key_state::UP, &names, &m, &mut w), Disposition::Destroy);
        assert_eq!((w.selected.clone(), w.sounds.clone()), (vec![0], vec![b"GUIClick".to_vec()]));
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_UP, p, key_state::UP, &names, &m, &mut w), Disposition::Keep);
        assert_eq!(w.sounds.last(), Some(&b"GUIClickDisabled".to_vec()));
        w.hidden[2] = true;
        let n = w.sounds.len();
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_UP, p, key_state::UP, &names, &m, &mut w), Disposition::Keep);
        assert_eq!(w.sounds.len(), n);
        m.reset();
        assert_eq!(x.translate_game_message(MSG_RAW_KEY_UP, b, key_state::UP, &names, &m, &mut w), Disposition::Keep);
        assert_eq!(w.selected, vec![0]);
    }
}
