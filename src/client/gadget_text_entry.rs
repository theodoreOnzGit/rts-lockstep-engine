// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetTextEntry.cpp (GadgetTextEntryInput,
//     GadgetTextEntrySystem, GadgetTextEntrySetFont, GadgetTextEntryGetText)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoGadgetTextEntry)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp (winIsDigit, winIsAscii, winIsAlNum)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GadgetTextEntry.h (GadgetTextEntrySetText,
//     GadgetTextEntrySetTextColor)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Gadget.h (EntryData, ENTRY_TEXT_LEN, the GEM_* messages)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Color.cpp (GameDarkenColor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The text entry (GitHub #72, a part of #34): a one-line edit field, alone or as a combo box's edit box.
//!
//! # As upstream reads it
//!
//! `EntryData` keeps the text (`text`), the same number of `*` for secret entries (`sText`), the IME's construction
//! text, the filters (`numericalOnly`, `alphaNumericalOnly`, `aSCIIOnly`), `maxTextLen` (at most `ENTRY_TEXT_LEN`,
//! 256, when made) and `charPos`, the number of characters.
//!
//! - **Characters** arrive as `GWM_IME_CHAR` (upstream's `IMEManager` sends one for each `WM_CHAR` of 32 or more, or
//!   return, to the window it is attached to: the text entry that has the focus). Return sends the owner
//!   `GEM_EDIT_DONE`; any other character passes the filters and, while `charPos < maxTextLen - 1`, is appended (a `*`
//!   to the secret text) and the owner gets `GEM_UPDATE_TEXT`.
//! - **Keys** (`GWM_CHAR`): with control or alt down, ignored; escape, page up and down, home, end, F1 to F12, caps
//!   lock and delete are ignored (passed to the parent); down, right and tab go to the next window of the manager's tab
//!   list (from the parent combo box, if the entry is one's), up and left to the previous; backspace removes the last
//!   character and sends `GEM_UPDATE_TEXT`; any other key is eaten.
//! - **The mouse**: a left press hilites the entry and gives it the focus; with `GWS_MOUSE_TRACK`, entering and
//!   leaving hilite it and tell the owner (`GBM_MOUSE_ENTERING` / `LEAVING`), and a drag sends `GGM_LEFT_DRAG`.
//! - **The focus** (`GWM_INPUT_FOCUS`): selected and hilited while it has it; the owner gets `GGM_FOCUS_CHANGE`; it
//!   always wants the focus. Taking it attaches `TheIMEManager` to the entry; losing it detaches the manager if it is
//!   attached to this entry (#74).
//! - `GEM_SET_TEXT` replaces the text (and the secret text by as many `*`) and puts `charPos` at its end;
//!   `GEM_GET_TEXT` reads it.
//!
//! # Upstream quirks that are kept
//!
//! - Enter in `GWM_CHAR` does nothing (its case is commented out): only `GWM_IME_CHAR` return ends an edit.
//! - `GEM_SET_TEXT` does not check `maxTextLen`: a text set by code (or the label) may be longer than typing allows,
//!   and then no character can be typed until enough are erased.
//! - `charPos` is an `UnsignedShort` set from the text's length (`GEM_SET_TEXT`), and compared with `maxTextLen - 1`
//!   as `Int`: a negative or zero `maxTextLen` allows no typing at all.
//! - The secret text is kept whether or not the entry is secret.
//! - `gogoGadgetTextEntry` sets the text from the **label** (`winTextLabelToText`: the bytes as they are), so an entry
//!   made with a label starts with it; a combo box's edit box is made with the label `Entry` and starts with the text
//!   `Entry`.
//!
//! # Not translated
//!
//! ~~The IME (`TheIMEManager` is never attached in this translation, so it never composes): its construction text and
//! candidate list (`constructList`, made only for Korean and Japanese) stay empty, and its `WM_CHAR` handling, which is
//! what turns typed characters into `GWM_IME_CHAR`, is the host's to do (it sends `GWM_IME_CHAR` to the focus window).~~
//! **Corrected 2026-11-10 (#74):** `TheIMEManager` is translated as far as typing goes
//! ([`crate::client::ime_manager`]): the entry attaches it when it takes the focus and detaches it (`attach(NULL)`)
//! when it loses it while attached, and its `WM_CHAR` handling sends the attached entry `GWM_IME_CHAR`. It never
//! composes, so the construction text and candidate list (`constructList`, made only for Korean and Japanese) stay
//! empty. `winSetCursorPosition` (where the IME's window goes) is not modelled.
//!
//! # Where this differs from upstream
//!
//! `winIsDigit`, `winIsAlNum` and `winIsAscii` are `iswdigit`, `iswalnum` and `iswascii` of the CRT's C locale.
//! The translation classifies ASCII only (`0-9`, `A-Z`, `a-z`); upstream's MSVC CRT also counts some characters
//! above 0x7F as letters or digits (the differential's glibc agrees with the translation; MSVC is unverified there).
//! The text passed by pointer (`GEM_SET_TEXT`, `GEM_GET_TEXT`) is the manager's [`EntryMsg`].
//!
//! # Representation
//!
//! [`EntryRt`] in the window's [`UserData::Entry`].

use crate::client::gadget_list_box::DisplayString;
use crate::client::window_manager::{gwm, key, win_data, DrawFunc, GameWindowManager, GuiHost, InputFunc, MsgHandled, SystemFunc, UserData, WindowMsgData, WIN_STATE_HILITED, WIN_STATE_SELECTED};
use crate::client::window_script::{gws, win_status, EntryData, GameFont, WinId, WinInstanceData};

use MsgHandled::{Handled, Ignored};

/// `ENTRY_TEXT_LEN`.
pub const ENTRY_TEXT_LEN: i16 = 256;
/// `VK_RETURN`.
pub const VK_RETURN: u16 = 0x0D;

/// The text entry's messages (Gadget.h, after the combo box's).
pub mod gem {
    pub const GET_TEXT: u32 = 16428;
    pub const SET_TEXT: u32 = 16429;
    pub const EDIT_DONE: u32 = 16430;
    pub const UPDATE_TEXT: u32 = 16431;
}

/// The keys `GWM_CHAR` passes on (`KeyDefs.h`).
const IGNORED_KEYS: [u8; 19] = [
    key::ESC,
    0xC9, // KEY_PGUP
    0xD1, // KEY_PGDN
    key::HOME,
    key::END,
    0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, // KEY_F1 .. KEY_F10
    0x57, 0x58, // KEY_F11, KEY_F12
    0x3A, // KEY_CAPS
    key::DEL,
];
/// `KEY_STATE_CONTROL | KEY_STATE_ALT`.
const KEY_STATE_ALT_CONTROL: u32 = 0x0004 | 0x0008 | 0x0040 | 0x0080;

/// `EntryData` as the text entry keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryRt {
    /// `text`.
    pub text: DisplayString,
    /// `sText`: a `*` per character.
    pub s_text: DisplayString,
    /// `constructText` (the IME's; always empty here).
    pub construct_text: DisplayString,
    pub secret_text: bool,
    pub numerical_only: bool,
    pub alpha_numerical_only: bool,
    pub ascii_only: bool,
    /// `Short`.
    pub max_text_len: i16,
    pub received_unichar: bool,
    pub draw_text_from_start: bool,
    /// `UnsignedShort`.
    pub char_pos: u16,
    /// `UnsignedShort` (the IME's; always 0 here).
    pub con_char_pos: u16,
}

/// The data the text entry's messages point to (upstream passes `UnicodeString*`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryMsg {
    /// `GEM_SET_TEXT`'s string, `GEM_GET_TEXT`'s answer.
    pub text: Vec<u16>,
}

/// The entry's data, if the window has some.
pub fn entry_rt(wm: &GameWindowManager, w: WinId) -> Option<&EntryRt> {
    match &wm.ext[w].user {
        UserData::Entry(e) => Some(e),
        _ => None,
    }
}

/// The entry's data, mutable; `None` (and upstream's `NULL` dereference recorded) without it.
fn entry_mut<'a>(wm: &'a mut GameWindowManager, w: WinId, what: &'static str) -> Option<&'a mut EntryRt> {
    if !matches!(wm.ext[w].user, UserData::Entry(_)) {
        wm.set_undefined(what);
        return None;
    }
    match &mut wm.ext[w].user {
        UserData::Entry(e) => Some(e),
        _ => None,
    }
}

/// `GameIsDigit` (`iswdigit`), ASCII only (see the module docs).
pub fn win_is_digit(c: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&c)
}

/// `GameIsAlNum` (`iswalnum`), ASCII only.
pub fn win_is_al_num(c: u16) -> bool {
    c < 0x80 && (c as u8).is_ascii_alphanumeric()
}

/// `GameIsAscii` (`iswascii`).
pub fn win_is_ascii(c: u16) -> bool {
    c < 0x80
}

/// `GameDarkenColor(color, percent)` (Color.cpp).
pub fn game_darken_color(color: i32, percent: i32) -> i32 {
    if !(1..90).contains(&percent) {
        return color;
    }
    let c = color as u32;
    let (a, r, g, b) = ((c >> 24) & 0xFF, (c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF);
    let darken = |v: u32| -> u32 { (v as i32 - (v as i32 * percent / 100)) as u32 & 0xFF };
    ((a << 24) | (darken(r) << 16) | (darken(g) << 8) | darken(b)) as i32
}

fn owner(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.ext[w].owner
}

/// The parent if it is a combo box (the tab keys and the cursor use it).
pub(crate) fn combo_parent(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.tree.windows[w].parent.filter(|&p| wm.tree.windows[p].inst.style & gws::COMBO_BOX != 0)
}

fn set_state(wm: &mut GameWindowManager, w: WinId, bits: i32, on: bool) {
    if on {
        wm.tree.windows[w].inst.state |= bits;
    } else {
        wm.tree.windows[w].inst.state &= !bits;
    }
}

/// `GadgetTextEntryInput`.
pub fn text_entry_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    // `TheIMEManager->isAttachedTo(window) && isComposing()`: never (#74: the manager never composes)
    debug_assert!(!(wm.ime.is_attached_to(Some(w)) && wm.ime.is_composing()));
    let style = wm.tree.windows[w].inst.style;
    match msg {
        gwm::IME_CHAR => {
            let ch = d1 as u16;
            if ch == VK_RETURN {
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, gem::EDIT_DONE, win_data(Some(w)), 0);
                return Handled;
            }
            if ch != 0 {
                let Some(e) = entry_mut(wm, w, "GWM_IME_CHAR: the text entry has no EntryData") else { return Handled };
                if e.numerical_only && !win_is_digit(ch) {
                    return Handled;
                }
                if e.alpha_numerical_only && !win_is_al_num(ch) {
                    return Handled;
                }
                if e.ascii_only && !win_is_ascii(ch) {
                    return Handled;
                }
                let limit = i32::from(e.max_text_len) - 1 + i32::from(crate::logic::modules::plant(4705));
                if i32::from(e.char_pos) < limit {
                    e.text.text.push(ch);
                    e.s_text.text.push(u16::from(b'*'));
                    e.char_pos = e.char_pos.wrapping_add(1);
                    let o = owner(wm, w);
                    wm.win_send_system_msg(h, o, gem::UPDATE_TEXT, win_data(Some(w)), 0);
                }
            }
        }
        gwm::CHAR => {
            if d2 & key::STATE_DOWN != 0 && d2 & KEY_STATE_ALT_CONTROL != 0 {
                return Ignored;
            }
            let k = d1 as u8;
            if d1 <= 0xFF && IGNORED_KEYS.contains(&k) {
                return Ignored;
            }
            match d1 {
                x if x == u32::from(key::DOWN) || x == u32::from(key::RIGHT) || x == u32::from(key::TAB) => {
                    if d2 & key::STATE_DOWN != 0 {
                        let to = combo_parent(wm, w).unwrap_or(w);
                        wm.win_next_tab(h, to);
                    }
                }
                x if x == u32::from(key::UP) || x == u32::from(key::LEFT) => {
                    if d2 & key::STATE_DOWN != 0 {
                        let to = combo_parent(wm, w).unwrap_or(w);
                        wm.win_prev_tab(h, to);
                    }
                }
                // (backspace up: nothing, as any other key)
                x if x == u32::from(key::BACKSPACE) && d2 & key::STATE_DOWN != 0 => {
                    let Some(e) = entry_mut(wm, w, "GWM_CHAR backspace: the text entry has no EntryData") else { return Handled };
                    if e.con_char_pos == 0 && e.char_pos > 0 {
                        e.text.text.pop();
                        e.s_text.text.pop();
                        e.char_pos -= 1;
                        let o = owner(wm, w);
                        wm.win_send_system_msg(h, o, gem::UPDATE_TEXT, win_data(Some(w)), 0);
                    }
                }
                _ => {}
            }
        }
        gwm::LEFT_DOWN => {
            set_state(wm, w, WIN_STATE_HILITED, true);
            wm.win_set_focus(h, Some(w));
        }
        gwm::MOUSE_ENTERING => {
            if style & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, true);
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, crate::client::window_manager::ggm::GBM_MOUSE_ENTERING, win_data(Some(w)), 0);
            }
        }
        gwm::MOUSE_LEAVING => {
            if style & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, false);
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, crate::client::window_manager::ggm::GBM_MOUSE_LEAVING, win_data(Some(w)), 0);
            }
        }
        gwm::LEFT_DRAG => {
            if style & gws::MOUSE_TRACK != 0 {
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, crate::client::window_manager::ggm::LEFT_DRAG, win_data(Some(w)), 0);
            }
        }
        _ => return Ignored,
    }
    Handled
}

/// `GadgetTextEntrySystem`.
pub fn text_entry_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gem::GET_TEXT => {
            let Some(e) = entry_mut(wm, w, "GEM_GET_TEXT: the text entry has no EntryData") else { return Handled };
            let t = e.text.text.clone();
            wm.entry_msg.text = t;
        }
        gem::SET_TEXT => {
            let s = wm.entry_msg.text.clone();
            let Some(e) = entry_mut(wm, w, "GEM_SET_TEXT: the text entry has no EntryData") else { return Handled };
            e.char_pos = s.len() as u16;
            e.s_text.text = vec![u16::from(b'*'); s.len()];
            e.text.text = s;
            e.construct_text.text.clear();
            e.con_char_pos = 0;
        }
        gwm::CREATE => {}
        gwm::DESTROY => {
            // the display strings and the data are freed (no construct list: no IME)
            wm.ext[w].user = UserData::None;
        }
        gwm::INPUT_FOCUS => {
            if d1 == 0 {
                set_state(wm, w, WIN_STATE_SELECTED | WIN_STATE_HILITED, false);
                let Some(e) = entry_mut(wm, w, "GWM_INPUT_FOCUS: the text entry has no EntryData") else { return Handled };
                e.construct_text.text.clear();
                e.con_char_pos = 0;
                // (#74) `if (TheIMEManager && TheIMEManager->isAttachedTo(window)) TheIMEManager->attach(NULL)`
                if wm.ime.is_attached_to(Some(w)) && !crate::logic::modules::plant(4710) {
                    wm.ime.attach(None);
                }
            } else {
                // (#74) `TheIMEManager->attach(window)`
                wm.ime.attach(Some(w));
                set_state(wm, w, WIN_STATE_SELECTED | WIN_STATE_HILITED, true);
            }
            let o = owner(wm, w);
            let id = wm.tree.windows[w].id;
            wm.win_send_system_msg(h, o, crate::client::window_manager::ggm::FOCUS_CHANGE, d1, id as u32);
            wm.focus_reply = true;
        }
        _ => return Ignored,
    }
    Handled
}

/// `GadgetTextEntrySetText(g, text)` (`GEM_SET_TEXT`).
pub fn set_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>, text: &[u16]) {
    wm.entry_msg.text = text.to_vec();
    wm.win_send_system_msg(h, w, gem::SET_TEXT, 1, 0);
}

/// `GadgetTextEntryGetText(textentry)`: empty for no window or one that is not an entry field.
pub fn get_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>) -> Vec<u16> {
    let Some(w) = w else { return Vec::new() };
    if wm.tree.windows[w].inst.style & gws::ENTRY_FIELD == 0 {
        return Vec::new();
    }
    wm.entry_msg.text.clear();
    wm.win_send_system_msg(h, Some(w), gem::GET_TEXT, 0, 1);
    std::mem::take(&mut wm.entry_msg.text)
}

/// `GadgetTextEntrySetFont(g, font)`: the entry's two display strings (the window's own text and tooltip strings
/// are not modelled: they take the window's font when drawn).
pub fn set_font(wm: &mut GameWindowManager, w: WinId, font: Option<GameFont>) {
    if let UserData::Entry(e) = &mut wm.ext[w].user {
        e.text.font = font.clone();
        e.s_text.font = font;
    }
}

/// `GadgetTextEntrySetTextColor(g, color)`: the enabled text colour, and the disabled one darkened by 25 %, both with
/// the enabled border colour.
pub fn set_text_color(wm: &mut GameWindowManager, w: WinId, color: i32) {
    let back = wm.tree.windows[w].inst.enabled_text.border_color;
    wm.win_set_text_colors(w, 0, color, back);
    wm.win_set_text_colors(w, 1, game_darken_color(color, 25), back);
}

/// The script's `EntryData` (or a combo box's) as `gogoGadgetTextEntry` copies it.
pub fn entry_template(d: &EntryData) -> EntryRt {
    EntryRt { secret_text: d.secret_text, numerical_only: d.numerical_only, alpha_numerical_only: d.alpha_numerical_only, ascii_only: d.ascii_only, max_text_len: d.max_text_len, ..Default::default() }
}

/// `gogoGadgetTextEntry(parent, status, x, y, width, height, instData, entryData, defaultFont, defaultVisual)`
/// (`defaultFont` is the instance data's font). `None` where upstream asserts (not an entry field style).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_text_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, template: EntryRt, default_visual: bool) -> Option<WinId> {
    if inst.style & gws::ENTRY_FIELD == 0 {
        return None;
    }
    let font = inst.font.clone();
    let label = inst.text_label_string.clone();
    let e = wm.win_create(h, parent, status, x, y, width, height, SystemFunc::TextEntry, Some(inst));
    text_entry_after_create(wm, h, e, parent, font, template, default_visual);
    // the text from the label (`winTextLabelToText`: the bytes as they are)
    let text: Vec<u16> = label.iter().map(|&b| b as i8 as i16 as u16).collect();
    if !text.is_empty() {
        set_text(wm, h, Some(e), &text);
    }
    Some(e)
}

/// `gogoGadgetTextEntry` after its `winCreate`, up to the label: the owner, callbacks, data and look.
pub fn text_entry_after_create<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, e: WinId, parent: Option<WinId>, font: Option<GameFont>, template: EntryRt, default_visual: bool) {
    wm.win_set_owner(e, parent);
    wm.ext[e].input = InputFunc::TextEntry;
    let image = wm.tree.windows[e].status & win_status::IMAGE != 0;
    wm.ext[e].draw = if image { DrawFunc::TextEntryImage } else { DrawFunc::TextEntry };
    let mut d = template;
    // the template's display strings are new ones (copied from the template's: none, from the script or a combo box)
    d.char_pos = d.text.text.len() as u16;
    d.con_char_pos = 0;
    d.received_unichar = false;
    if d.max_text_len >= ENTRY_TEXT_LEN {
        d.max_text_len = ENTRY_TEXT_LEN;
    }
    d.text.font = None;
    d.s_text.font = None;
    d.construct_text = DisplayString::default();
    wm.ext[e].user = UserData::Entry(d);
    // `OurLanguage` is never Korean or Japanese here: no construct list
    crate::client::gadget_look::assign_default_gadget_look(wm, h, e, font, default_visual);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::tests::Rec;
    use crate::client::window_manager::ggm;

    fn u(t: &str) -> Vec<u16> {
        t.encode_utf16().collect()
    }

    /// The system messages the host got: (window, message, `mData1`).
    fn sys(h: &Rec) -> Vec<(WinId, u32, u32)> {
        h.sys.iter().map(|(_, w, m, d)| (*w, *m, *d)).collect()
    }

    fn entry(wm: &mut GameWindowManager, h: &mut Rec, max: i16, label: &str) -> (WinId, WinId) {
        let owner = wm.win_create(h, None, win_status::ENABLED, 0, 0, 400, 300, SystemFunc::Host(b"Owner".to_vec()), None);
        let inst = WinInstanceData { style: gws::ENTRY_FIELD | gws::MOUSE_TRACK, text_label_string: label.as_bytes().to_vec(), ..WinInstanceData::new() };
        let d = EntryRt { max_text_len: max, ..Default::default() };
        let e = gogo_gadget_text_entry(wm, h, Some(owner), win_status::ENABLED, 10, 20, 100, 20, inst, d, false).expect("an entry");
        (owner, e)
    }

    /// #72: typing. Predicted from GadgetTextEntry.cpp: an entry with `maxTextLen` 4 and the label `ab` starts with
    /// `ab` (charPos 2); `GWM_IME_CHAR` `c` appends it (`abc`, secret `***`) and tells the owner `GEM_UPDATE_TEXT`;
    /// `d` is refused (charPos 3 is not below 4 - 1); backspace (down) removes `c` with another `GEM_UPDATE_TEXT`;
    /// backspace up does nothing; return sends `GEM_EDIT_DONE` and changes nothing; control+A is ignored (the message
    /// passes to the parent); an ordinary key is eaten; `numericalOnly` refuses a letter.
    #[test]
    fn a_text_entry_takes_characters_as_upstream_takes_them() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (owner, e) = entry(&mut wm, &mut h, 4, "ab");
        let ed = win_data(Some(e));
        assert_eq!(get_text(&mut wm, &mut h, Some(e)), u("ab"));
        assert_eq!(entry_rt(&wm, e).expect("data").char_pos, 2);
        h.sys.clear();
        assert_eq!(wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(b'c'), 0), Handled);
        assert_eq!(entry_rt(&wm, e).expect("data").text.text, u("abc"));
        assert_eq!(entry_rt(&wm, e).expect("data").s_text.text, u("***"));
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(b'd'), 0);
        assert_eq!(get_text(&mut wm, &mut h, Some(e)), u("abc"));
        assert_eq!(sys(&h), vec![(owner, gem::UPDATE_TEXT, ed)]);
        wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, u32::from(key::BACKSPACE), key::STATE_UP);
        wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, u32::from(key::BACKSPACE), key::STATE_DOWN);
        assert_eq!(get_text(&mut wm, &mut h, Some(e)), u("ab"));
        assert_eq!(entry_rt(&wm, e).expect("data").char_pos, 2);
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(VK_RETURN), 0);
        assert_eq!(sys(&h)[1..], [(owner, gem::UPDATE_TEXT, ed), (owner, gem::EDIT_DONE, ed)]);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, 0x1E, key::STATE_DOWN | 0x0004), Ignored);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, u32::from(key::DEL), key::STATE_DOWN), Ignored);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, 0x1E, key::STATE_DOWN), Handled);
        if let UserData::Entry(d) = &mut wm.ext[e].user {
            d.numerical_only = true;
        }
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(b'x'), 0);
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(b'7'), 0);
        assert_eq!(get_text(&mut wm, &mut h, Some(e)), u("ab7"));
        // GEM_SET_TEXT ignores maxTextLen
        set_text(&mut wm, &mut h, Some(e), &u("long text"));
        assert_eq!(entry_rt(&wm, e).expect("data").char_pos, 9);
        assert_eq!(entry_rt(&wm, e).expect("data").s_text.text.len(), 9);
    }

    /// #72: the focus and the mouse. Predicted: a left press hilites the entry and gives it the focus, which selects
    /// it and tells the owner `GGM_FOCUS_CHANGE` (1, the window id); losing it clears both bits; with mouse tracking,
    /// entering hilites it and sends `GBM_MOUSE_ENTERING`.
    #[test]
    fn a_text_entry_takes_the_focus_as_upstream_takes_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (owner, e) = entry(&mut wm, &mut h, 10, "");
        wm.win_mut(e).id = 77;
        h.sys.clear();
        wm.win_send_input_msg(&mut h, Some(e), gwm::LEFT_DOWN, 0, 0);
        assert_eq!(wm.keyboard_focus, Some(e));
        assert_eq!(wm.win(e).inst.state & (WIN_STATE_HILITED | WIN_STATE_SELECTED), WIN_STATE_HILITED | WIN_STATE_SELECTED);
        assert_eq!(sys(&h), vec![(owner, ggm::FOCUS_CHANGE, 1)]);
        wm.win_set_focus(&mut h, None);
        assert_eq!(wm.win(e).inst.state & (WIN_STATE_HILITED | WIN_STATE_SELECTED), 0);
        wm.win_send_input_msg(&mut h, Some(e), gwm::MOUSE_ENTERING, 0, 0);
        assert_ne!(wm.win(e).inst.state & WIN_STATE_HILITED, 0);
        assert_eq!(sys(&h).last(), Some(&(owner, ggm::GBM_MOUSE_ENTERING, win_data(Some(e)))));
        wm.win_send_input_msg(&mut h, Some(e), gwm::MOUSE_LEAVING, 0, 0);
        assert_eq!(wm.win(e).inst.state & WIN_STATE_HILITED, 0);
        wm.win_send_input_msg(&mut h, Some(e), gwm::LEFT_DRAG, 0, 0);
        assert_eq!(sys(&h).last(), Some(&(owner, ggm::LEFT_DRAG, win_data(Some(e)))));
        // the tab keys go to the manager's tab list
        let other = wm.win_create(&mut h, None, win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Host(b"Other".to_vec()), None);
        wm.register_tab_list(vec![e, other]);
        wm.win_set_focus(&mut h, Some(e));
        wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, u32::from(key::TAB), key::STATE_DOWN);
        assert_eq!(wm.keyboard_focus, None, "the host's window does not want the focus");
        wm.win_send_input_msg(&mut h, Some(e), gwm::CHAR, u32::from(key::UP), key::STATE_DOWN);
        assert_eq!(wm.keyboard_focus, None);
        // destruction frees the data
        wm.win_destroy(Some(e));
        wm.update(&mut h);
        assert!(entry_rt(&wm, e).is_none());
    }

    /// #72: the character classes and the darkened colour. Predicted: digits only `0-9`; letters and digits ASCII
    /// only; `GameDarkenColor(0xFF804020, 25)` takes a quarter off each of r, g, b: 0xFF603018; 90 % or more, or 0,
    /// leaves it.
    #[test]
    fn character_classes_and_darkening_follow_upstream() {
        assert!(win_is_digit(u16::from(b'5')) && !win_is_digit(u16::from(b'a')) && !win_is_digit(0xFF10));
        assert!(win_is_al_num(u16::from(b'Z')) && !win_is_al_num(u16::from(b'-')) && !win_is_al_num(0xE9));
        assert!(win_is_ascii(0x7F) && !win_is_ascii(0x80));
        assert_eq!(game_darken_color(0xFF80_4020_u32 as i32, 25), 0xFF60_3018_u32 as i32);
        assert_eq!(game_darken_color(0x1234_5678, 90), 0x1234_5678);
        assert_eq!(game_darken_color(0x1234_5678, 0), 0x1234_5678);
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (_, e) = entry(&mut wm, &mut h, 10, "");
        wm.win_mut(e).inst.enabled_text.border_color = 7;
        set_text_color(&mut wm, e, 0xFF80_4020_u32 as i32);
        assert_eq!((wm.win(e).inst.enabled_text.color, wm.win(e).inst.enabled_text.border_color), (0xFF80_4020_u32 as i32, 7));
        assert_eq!((wm.win(e).inst.disabled_text.color, wm.win(e).inst.disabled_text.border_color), (0xFF60_3018_u32 as i32, 7));
        set_font(&mut wm, e, Some(GameFont { name: b"F".to_vec(), point_size: 9, bold: false }));
        assert_eq!(entry_rt(&wm, e).expect("data").s_text.font.as_ref().map(|f| f.point_size), Some(9));
    }
}
