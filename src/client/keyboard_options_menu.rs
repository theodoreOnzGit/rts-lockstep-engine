// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/KeyboardOptionsMenu.cpp (the whole file:
//     populateCategoryBox, setKeyDown, fillCommandListBox, doKeyUp, doKeyDown, KeyboardOptionsMenuInit,
//     KeyboardOptionsMenuShutdown, KeyboardOptionsMenuUpdate, KeyboardOptionsMenuInput, KeyboardOptionsMenuSystem,
//     KeyboardTextEntryInput)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The keyboard options screen (GitHub #160, a part of #100): `Menus/KeyboardOptionsMenu.wnd`, which the options
//! screen's *Keyboard Options* pushes.
//!
//! # As upstream reads it
//!
//! A combo box of the eight command categories (`CategoryListName`: control, information, interface, selection,
//! taunt, team, misc, debug), a list of the commands of the chosen category (`TheMetaMap`'s records, by their display
//! names), the chosen command's description and current key (its `KeyNames` name), and a text entry where a new key
//! combination is typed: the modifier keys (left control, either shift, left alt) build a prefix such as
//! `Ctrl+Shift+` (the game's text `KEYBOARD:Ctrl+`, ...), a character typed after it completes it.
//!
//! **Upstream never assigns or saves a key here.** *Assign*'s branch is empty (`// check grammar in text field`), and
//! nothing writes the meta map or a file: the screen shows the bindings and lets a combination be typed, no more.
//! *Reset All* refills the screen (it resets no binding either). This translation does the same.
//!
//! # Upstream quirks that are kept
//!
//! - The modifier flags (`shiftDown`, `altDown`, `ctrlDown`, `absolute`) and the three prefixes are globals: they are
//!   kept from one visit of the screen to the next (only *Reset All*, backspace and the keys themselves clear them).
//! - `setKeyDown(mod, b)` compares `mod` with the game's `KEYBOARD:Shift+` and `KEYBOARD:Ctrl+` texts, anything else
//!   being alt.
//! - Releasing alt while alt and control are down leaves `Ctrl+` (the same as releasing control): `doKeyUp`'s second
//!   branch sets `ctrl` in both cases.
//! - The last character of the entry is read as a `char` (`WideChar` truncated to its low byte): a character whose
//!   low byte is 0x2B counts as `+`; and `test != ch` compares the signed `char` with the `WideChar`, so a character
//!   of 0x80 or more is always replaced.
//! - `KeyboardOptionsMenuUpdate` is not in the function lexicon, so no WND can name it; `KeyboardTextEntryInput` is set
//!   by `winSetInputFunc`, here the window's input callback named `KeyboardTextEntryInput` ([`InputFunc::Host`]).
//! - Escape is handled whatever its state; a `GWM_CHAR` of another key falls out of the inner switch and is ignored.
//!
//! # Undefined upstream (recorded with [`GameWindowManager::set_undefined`], not acted on)
//!
//! - `populateCategoryBox` formats each label with `"GUI:%s"` and the `LookupListRec` itself (a struct passed through
//!   `...` and read back as `char *`): MSVC's x86 build reads the record's name, which is what this translation
//!   fetches; the differential test does not compare the labels' text.
//! - The last character read of an empty entry whose buffer is kept (`getCharAt(-1)` after the last character was
//!   removed: [`KeyboardOptionsMenu::text_buffer_kept`]); with no buffer it reads 0, as here.
//! - A control that is missing (`textEntryAssignHotkey->winSetInputFunc`, `winGetUserData` of `NULL`), a category
//!   selection of -1 (`CategoryListName[-1]`), and a message whose control is `NULL`.
//!
//! # Verified against the compiled upstream (#160)
//!
//! `tests/differential_keyboard_options.rs`: KeyboardOptionsMenu.cpp cut verbatim, against this translation on the real
//! window manager and gadgets, 20,000 random cases of up to 40 operations: 0 mismatches; 10 of 10 planted bugs
//! (9561-9570) caught.

use crate::client::gadget::static_text_set_text;
use crate::client::gadget_combo_box as combo;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry::{gem, win_is_al_num, win_is_ascii, win_is_digit, EntryRt, VK_RETURN};
use crate::client::meta_event::{MetaMap, CATEGORY_LIST_NAME, KEY_NAMES};
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, InputFunc, MsgHandled, UserData, WindowMsgData, WIN_STATE_HILITED};
use crate::client::window_script::{gws, WinId};
use crate::logic::modules::plant;

use MsgHandled::{Handled, Ignored};

/// `CATEGORY_NUM_CATEGORIES`.
pub const CATEGORY_NUM_CATEGORIES: usize = 8;
/// `CATEGORY_CONTROL`.
pub const CATEGORY_CONTROL: i32 = 0;
/// `GameMakeColor(255,255,255,255)`.
const WHITE: i32 = -1;

/// The keys `KeyboardTextEntryInput` looks at (`KeyDefs.h`).
pub mod keys {
    pub const LCTRL: u32 = 0x1D;
    pub const LSHIFT: u32 = 0x2A;
    pub const RSHIFT: u32 = 0x36;
    pub const LALT: u32 = 0x38;
    pub const CAPS: u32 = 0x3A;
    pub const PGUP: u32 = 0xC9;
    pub const PGDN: u32 = 0xD1;
    /// `KEY_F1` .. `KEY_F10`, `KEY_F11`, `KEY_F12`.
    pub const F: [u32; 12] = [0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x57, 0x58];
}

/// KeyboardOptionsMenu.cpp's statics and globals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyboardOptionsMenu {
    pub button_back_id: i32,
    pub button_back: Option<WinId>,
    pub parent_id: i32,
    pub parent: Option<WinId>,
    pub combo_box_category_list_id: i32,
    pub combo_box_category_list: Option<WinId>,
    pub list_box_command_list_id: i32,
    pub list_box_command_list: Option<WinId>,
    pub static_text_description_id: i32,
    pub static_text_description: Option<WinId>,
    pub static_text_current_hotkey_id: i32,
    pub static_text_current_hotkey: Option<WinId>,
    pub button_reset_all_id: i32,
    pub button_reset_all: Option<WinId>,
    pub text_entry_assign_hotkey_id: i32,
    pub text_entry_assign_hotkey: Option<WinId>,
    pub button_assign_id: i32,
    pub button_assign: Option<WinId>,
    /// `shiftDown`, `altDown`, `ctrlDown`.
    pub shift_down: bool,
    pub alt_down: bool,
    pub ctrl_down: bool,
    /// `absolute`: a complete combination is in the entry.
    pub absolute: bool,
    /// `alt`, `ctrl`, `shift`: the game's `KEYBOARD:Alt+`, `KEYBOARD:Ctrl+`, `KEYBOARD:Shift+` (empty before the first
    /// init).
    pub alt: Vec<u16>,
    pub ctrl: Vec<u16>,
    pub shift: Vec<u16>,
    /// The entry's text is empty but keeps its buffer (its last character was removed): `getCharAt(-1)` of it is
    /// undefined. A new entry (each init) has none.
    pub text_buffer_kept: bool,
}

/// What the screen asks of its host.
pub trait KeyboardOptionsHost: ShellHost {
    /// The file's statics.
    fn keyboard_options(&mut self) -> &mut KeyboardOptionsMenu;
    /// `TheMetaMap`.
    fn meta_map(&self) -> &MetaMap;
    /// `TheGameText->fetch(label)`.
    fn keyboard_text(&self, label: &[u8]) -> Vec<u16>;
}

fn entry_mut(wm: &mut GameWindowManager, w: WinId) -> Option<&mut EntryRt> {
    match &mut wm.ext[w].user {
        UserData::Entry(e) => Some(e),
        _ => None,
    }
}

/// `e->text->setText(text)`: nothing when the text is the same (`DisplayString::setText`'s early return, which keeps
/// an emptied buffer); `sText` too when `secret` is given.
fn set_text(m: &mut KeyboardOptionsMenu, e: &mut EntryRt, text: &[u16]) {
    if e.text.text != text {
        e.text.text = text.to_vec();
        m.text_buffer_kept = false;
    }
}

/// `e->text->getText().getCharAt(len - 1)` as a `char`: `None` where undefined (an empty text with its buffer kept).
fn last_char(m: &KeyboardOptionsMenu, e: &EntryRt) -> Option<i8> {
    match e.text.text.last() {
        Some(&c) => Some(c as u8 as i8),
        None if m.text_buffer_kept => None,
        None => Some(0),
    }
}

/// `populateCategoryBox()`.
pub fn populate_category_box<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(c) = h.keyboard_options().combo_box_category_list else { return };
    combo::reset(wm, h, c);
    for r in CATEGORY_LIST_NAME.iter().take(CATEGORY_NUM_CATEGORIES) {
        // (`temp.format("GUI:%s", CategoryListName[i])`: see the module docs)
        let s = h.keyboard_text(format!("GUI:{}", r.name).as_bytes());
        combo::add_entry(wm, h, c, &s, WHITE);
    }
    combo::set_selected_pos(wm, h, c, if plant(9561) { 1 } else { 0 }, false);
}

/// `setKeyDown(mod, b)`.
pub fn set_key_down<H: KeyboardOptionsHost>(h: &mut H, m: &[u16], b: bool) {
    let shift = h.keyboard_text(b"KEYBOARD:Shift+");
    let ctrl = h.keyboard_text(b"KEYBOARD:Ctrl+");
    let k = h.keyboard_options();
    if m == shift {
        k.shift_down = b;
    } else if m == ctrl {
        k.ctrl_down = b;
    } else {
        k.alt_down = b;
    }
}

/// `fillCommandListBox(cat)`: the list made anew with the display names of the meta map's records of `cat`.
pub fn fill_command_list_box<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, cat: i32) {
    let Some(l) = h.keyboard_options().list_box_command_list else { return };
    lb::reset(wm, h, l);
    let names: Vec<Vec<u16>> = h.meta_map().recs.iter().filter(|r| r.category == cat).map(|r| r.display_name.str().to_vec()).collect();
    for n in names {
        lb::add_entry_text(wm, h, l, &n, WHITE, -1, -1, true);
    }
}

fn concat(parts: &[&[u16]]) -> Vec<u16> {
    parts.concat()
}

/// `doKeyUp(e, mod)`: a modifier released.
fn do_key_up<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, m: &[u16]) {
    let mut k = std::mem::take(h.keyboard_options());
    let Some(e) = entry_mut(wm, w) else {
        *h.keyboard_options() = k;
        wm.set_undefined("KeyboardTextEntryInput: the window has no EntryData");
        return;
    };
    let Some(c) = last_char(&k, e) else {
        *h.keyboard_options() = k;
        wm.set_undefined("doKeyUp: getCharAt(-1) of an empty text with its buffer");
        return;
    };
    let (alt, ctrl, shift) = (k.alt.clone(), k.ctrl.clone(), k.shift.clone());
    let mut release = false;
    if c == b'+' as i8 {
        let mut set = |k: &mut KeyboardOptionsMenu, e: &mut EntryRt, t: &[u16]| {
            set_text(k, e, t);
            e.char_pos = e.text.text.len() as u16;
            release = true;
        };
        if k.alt_down && k.ctrl_down && k.shift_down {
            if m == shift {
                set(&mut k, e, &concat(&[&alt, &ctrl]));
            } else if m == alt {
                set(&mut k, e, &concat(&[&ctrl, &shift]));
            } else if m == ctrl {
                set(&mut k, e, &concat(&[&alt, &shift]));
            }
        } else if k.alt_down && k.ctrl_down {
            if m == alt {
                let t = if plant(9562) { alt.clone() } else { ctrl.clone() };
                set(&mut k, e, &t);
            } else if m == ctrl {
                set(&mut k, e, &ctrl);
            }
        } else if k.alt_down && k.shift_down {
            if m == alt {
                set(&mut k, e, &shift);
            } else if m == shift {
                set(&mut k, e, &alt);
            }
        } else if k.ctrl_down && k.shift_down {
            if m == ctrl {
                set(&mut k, e, &shift);
            } else if m == shift {
                set(&mut k, e, &ctrl);
            }
        } else {
            set_text(&mut k, e, &[]);
            e.s_text.text.clear();
            e.char_pos = e.text.text.len() as u16;
            release = true;
        }
    } else {
        k.absolute = true;
    }
    *h.keyboard_options() = k;
    if release {
        set_key_down(h, m, false);
    }
}

/// `doKeyDown(e, mod)`: a modifier pressed.
fn do_key_down<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, m: &[u16]) {
    let mut k = std::mem::take(h.keyboard_options());
    let Some(e) = entry_mut(wm, w) else {
        *h.keyboard_options() = k;
        wm.set_undefined("KeyboardTextEntryInput: the window has no EntryData");
        return;
    };
    let (alt, ctrl, shift) = (k.alt.clone(), k.ctrl.clone(), k.shift.clone());
    // what is pressed after the text is set: (reset all modifiers first, then mod's)
    let mut press: Option<bool> = None;
    if e.text.text.len() <= 1 {
        set_text(&mut k, e, m);
        e.s_text.text = m.to_vec();
        e.char_pos = e.text.text.len() as u16;
        press = Some(false);
    } else {
        // (the length is 2 or more: the character is defined)
        let c = e.text.text[e.text.text.len() - 1] as u8 as i8;
        if c != b'+' as i8 && k.absolute {
            set_text(&mut k, e, m);
            e.s_text.text = m.to_vec();
            e.char_pos = e.text.text.len() as u16;
            press = Some(true);
            k.absolute = false;
        } else if (m == shift && k.shift_down) || (m == ctrl && k.ctrl_down) || (m == alt && k.alt_down) {
        } else {
            let t: Option<Vec<u16>> = if k.alt_down && k.ctrl_down {
                Some(concat(&[&alt, &ctrl, m]))
            } else if k.alt_down && k.shift_down {
                Some(concat(&[&alt, &ctrl, &shift]))
            } else if k.alt_down {
                Some(concat(&[&alt, m]))
            } else if k.ctrl_down && k.shift_down {
                Some(concat(&[&alt, &ctrl, &shift]))
            } else if k.ctrl_down {
                if m == alt {
                    Some(concat(&[m, &ctrl]))
                } else {
                    Some(concat(&[&ctrl, m]))
                }
            } else if k.shift_down {
                Some(if plant(9563) { concat(&[&shift, m]) } else { concat(&[m, &shift]) })
            } else {
                None
            };
            if let Some(t) = t {
                set_text(&mut k, e, &t);
                e.char_pos = e.text.text.len() as u16;
                press = Some(false);
            }
        }
    }
    *h.keyboard_options() = k;
    match press {
        Some(true) => {
            let (shift, alt, ctrl) = {
                let k = h.keyboard_options();
                (k.shift.clone(), k.alt.clone(), k.ctrl.clone())
            };
            set_key_down(h, &shift, false);
            set_key_down(h, &alt, false);
            set_key_down(h, &ctrl, false);
            set_key_down(h, m, true);
        }
        Some(false) => set_key_down(h, m, true),
        None => {}
    }
}

/// The window named `name` (`TheWindowManager->winGetWindowFromId(parent, TheNameKeyGenerator->nameToKey(name))`).
fn find<H: KeyboardOptionsHost>(wm: &GameWindowManager, h: &mut H, parent: Option<WinId>, name: &str) -> (i32, Option<WinId>) {
    let id = h.name_to_key(format!("KeyboardOptionsMenu.wnd:{name}").as_bytes()) as i32;
    (id, wm.win_get_window_from_id(parent, id))
}

/// The entry emptied (`e->text->setText(TheEmptyString); e->charPos = e->text->getTextLength();`), and disabled.
fn clear_entry<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, what: &'static str) -> bool {
    let Some(t) = h.keyboard_options().text_entry_assign_hotkey else {
        wm.set_undefined(what);
        return false;
    };
    let mut k = std::mem::take(h.keyboard_options());
    let ok = match entry_mut(wm, t) {
        Some(e) => {
            set_text(&mut k, e, &[]);
            e.char_pos = e.text.text.len() as u16;
            true
        }
        None => false,
    };
    *h.keyboard_options() = k;
    if !ok {
        wm.set_undefined(what);
    }
    ok
}

/// `KeyboardOptionsMenuInit(layout, userData)`.
pub fn keyboard_options_menu_init<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let (pid, parent) = find(wm, h, None, "ParentKeyboardOptionsMenu");
    let (bid, back) = find(wm, h, parent, "ButtonBack");
    let (cid, cbox) = find(wm, h, None, "ComboBoxCategoryList");
    let (lid, list) = find(wm, h, None, "ListBoxCommandList");
    let (did, desc) = find(wm, h, None, "StaticTextDescription");
    let (hid, hot) = find(wm, h, None, "StaticTextCurrentHotkey");
    let (rid, reset) = find(wm, h, None, "ButtonResetAll");
    let (tid, entry) = find(wm, h, None, "TextEntryAssignHotkey");
    let (aid, assign) = find(wm, h, None, "ButtonAssign");
    {
        let k = h.keyboard_options();
        k.parent_id = pid;
        k.parent = parent;
        k.button_back_id = bid;
        k.button_back = back;
        k.combo_box_category_list_id = cid;
        k.combo_box_category_list = cbox;
        k.list_box_command_list_id = lid;
        k.list_box_command_list = list;
        k.static_text_description_id = did;
        k.static_text_description = desc;
        k.static_text_current_hotkey_id = hid;
        k.static_text_current_hotkey = hot;
        k.button_reset_all_id = rid;
        k.button_reset_all = reset;
        k.text_entry_assign_hotkey_id = tid;
        k.text_entry_assign_hotkey = entry;
        k.button_assign_id = aid;
        k.button_assign = assign;
        // (a new entry: its text has no buffer)
        k.text_buffer_kept = false;
    }
    let Some(entry) = entry else {
        wm.set_undefined("KeyboardOptionsMenuInit: no TextEntryAssignHotkey (winSetInputFunc of NULL)");
        return;
    };
    wm.ext[entry].input = InputFunc::Host(b"KeyboardTextEntryInput".to_vec());
    populate_category_box(wm, h);
    fill_command_list_box(wm, h, CATEGORY_CONTROL);
    wm.win_enable(entry, false);
    if !clear_entry(wm, h, "KeyboardOptionsMenuInit: TextEntryAssignHotkey has no EntryData") {
        return;
    }
    let (alt, ctrl, shift) = (h.keyboard_text(b"KEYBOARD:Alt+"), h.keyboard_text(b"KEYBOARD:Ctrl+"), h.keyboard_text(b"KEYBOARD:Shift+"));
    let k = h.keyboard_options();
    k.alt = alt;
    k.ctrl = ctrl;
    k.shift = shift;
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
}

/// `KeyboardOptionsMenuShutdown(layout, userData)`: hidden, the shutdown complete.
pub fn keyboard_options_menu_shutdown<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `KeyboardOptionsMenuUpdate(layout, userData)`: nothing (and not in the function lexicon).
pub fn keyboard_options_menu_update<H: KeyboardOptionsHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `KeyboardOptionsMenuInput(window, msg, mData1, mData2)`: Escape (handled whatever its state) presses *Back* on its
/// release, the button looked up under `window`; anything else is ignored.
pub fn keyboard_options_menu_input<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 && !plant(9564) {
        let id = h.name_to_key(b"KeyboardOptionsMenu.wnd:ButtonBack") as i32;
        let button = wm.win_get_window_from_id(Some(window), id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    Handled
}

/// `KeyboardOptionsMenuSystem(window, msg, mData1, mData2)`.
pub fn keyboard_options_menu_system<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return Handled;
        }
        ggm::GCM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("KeyboardOptionsMenuSystem: GCM_SELECTED with no control");
                return Handled;
            };
            let k = h.keyboard_options().clone();
            if wm.tree.windows[control].id == k.combo_box_category_list_id {
                let Some(c) = k.combo_box_category_list else {
                    wm.set_undefined("KeyboardOptionsMenuSystem: no category combo box (its selection uninitialised)");
                    return Handled;
                };
                let selected = combo::get_selected_pos(wm, h, c, -1);
                let Some(rec) = usize::try_from(selected).ok().and_then(|i| CATEGORY_LIST_NAME.get(i)) else {
                    // (index 8, the table's `{NULL, 0}`, is CATEGORY_CONTROL's value; past it, or -1: undefined)
                    if selected == CATEGORY_LIST_NAME.len() as i32 {
                        fill_after_category(wm, h, 0);
                        return Handled;
                    }
                    wm.set_undefined("KeyboardOptionsMenuSystem: CategoryListName[selected] outside the table");
                    return Handled;
                };
                fill_after_category(wm, h, rec.value);
            }
        }
        ggm::GLM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("KeyboardOptionsMenuSystem: GLM_SELECTED with no control");
                return Handled;
            };
            let k = h.keyboard_options().clone();
            if wm.tree.windows[control].id == k.list_box_command_list_id {
                let Some(l) = k.list_box_command_list else {
                    wm.set_undefined("KeyboardOptionsMenuSystem: no command list (its selection uninitialised)");
                    return Handled;
                };
                let selected = lb::get_selected(wm, h, l).first().copied().unwrap_or(-1);
                let s = lb::get_text(wm, h, l, selected, 0);
                let rec = h.meta_map().recs.iter().find(|r| r.display_name.str() == s.as_slice()).map(|r| (r.description.str().to_vec(), r.key));
                if let Some((description, key)) = rec {
                    static_text_set_text(wm, h, k.static_text_description, &description);
                    let Some(t) = k.text_entry_assign_hotkey else {
                        wm.set_undefined("KeyboardOptionsMenuSystem: no TextEntryAssignHotkey (winEnable of NULL)");
                        return Handled;
                    };
                    wm.win_enable(t, !plant(9565));
                    if let Some(n) = KEY_NAMES.iter().find(|n| n.value == key) {
                        // (`aStr.format(cptr)`: the name as a format, none holds a `%`; `uStr.translate(aStr)`)
                        let u: Vec<u16> = n.name.bytes().map(u16::from).collect();
                        static_text_set_text(wm, h, k.static_text_current_hotkey, &u);
                    }
                }
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("KeyboardOptionsMenuSystem: GBM_SELECTED with no control (control->winGetWindowId)");
                return Handled;
            };
            let id = wm.tree.windows[control].id;
            let k = h.keyboard_options().clone();
            if id == k.button_back_id {
                shell::pop(wm, h);
            } else if id == k.button_assign_id {
                // (check grammar in text field: nothing)
            } else if id == k.button_reset_all_id {
                populate_category_box(wm, h);
                fill_command_list_box(wm, h, CATEGORY_CONTROL);
                let null = h.keyboard_text(b"GUI:NULL");
                let hot = h.keyboard_options().static_text_current_hotkey;
                static_text_set_text(wm, h, hot, &null);
                if !clear_entry(wm, h, "KeyboardOptionsMenuSystem: Reset All with no TextEntryAssignHotkey") {
                    return Handled;
                }
                let (alt, ctrl, shift) = (k.alt.clone(), k.ctrl.clone(), k.shift.clone());
                set_key_down(h, &alt, false);
                set_key_down(h, &ctrl, false);
                if !plant(9566) {
                    set_key_down(h, &shift, false);
                }
                if let Some(t) = h.keyboard_options().text_entry_assign_hotkey {
                    wm.win_enable(t, false);
                }
            }
        }
        _ => return Ignored,
    }
    Handled
}

/// GCM_SELECTED's tail for the category `cat`: the list filled, the description and key reset, the entry emptied and
/// disabled.
fn fill_after_category<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, cat: i32) {
    fill_command_list_box(wm, h, cat);
    let null = h.keyboard_text(b"GUI:NULL");
    let k = h.keyboard_options().clone();
    static_text_set_text(wm, h, k.static_text_description, &null);
    static_text_set_text(wm, h, k.static_text_current_hotkey, &null);
    if !clear_entry(wm, h, "KeyboardOptionsMenuSystem: GCM_SELECTED with no TextEntryAssignHotkey") {
        return;
    }
    if let Some(t) = k.text_entry_assign_hotkey {
        wm.win_enable(t, plant(9567));
    }
}

/// `TheWindowManager->winSendSystemMsg(window->winGetOwner(), msg, (WindowMsgData)window, 0)`.
fn tell_owner<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32) {
    let o = wm.ext[w].owner;
    wm.win_send_system_msg(h, o, msg, win_data(Some(w)), 0);
}

/// `KeyboardTextEntryInput(window, msg, mData1, mData2)`: the hotkey entry's input (see the module docs).
pub fn keyboard_text_entry_input<H: KeyboardOptionsHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if !matches!(wm.ext[window].user, UserData::Entry(_)) {
        wm.set_undefined("KeyboardTextEntryInput: the window has no EntryData");
        return Handled;
    }
    // `TheIMEManager->isAttachedTo(window) && isComposing()`: never (the manager never composes)
    if wm.ime.is_attached_to(Some(window)) && wm.ime.is_composing() {
        return Handled;
    }
    let style = wm.tree.windows[window].inst.style;
    match msg {
        gwm::IME_CHAR => {
            let ch = d1 as u16;
            if ch == VK_RETURN {
                tell_owner(wm, h, window, gem::EDIT_DONE);
                return Handled;
            }
            if ch != 0 {
                let mut k = std::mem::take(h.keyboard_options());
                let e = entry_mut(wm, window).expect("checked");
                if (e.numerical_only && !win_is_digit(ch)) || (!e.numerical_only && e.alpha_numerical_only && !win_is_al_num(ch)) || (!e.numerical_only && !e.alpha_numerical_only && e.ascii_only && !win_is_ascii(ch)) {
                    *h.keyboard_options() = k;
                    return Handled;
                }
                let send = if e.text.text.len() <= 1 {
                    set_text(&mut k, e, &[]);
                    e.text.text.push(ch);
                    k.text_buffer_kept = false;
                    e.char_pos = e.text.text.len() as u16;
                    true
                } else {
                    let c = e.text.text[e.text.text.len() - 1] as u8 as i8;
                    if c == b'+' as i8 {
                        e.text.text.push(ch);
                        e.char_pos = e.char_pos.wrapping_add(1);
                        true
                    } else if (k.shift_down | k.ctrl_down | k.alt_down) && !k.absolute {
                        // (`char test` against the `WideChar`: the char sign-extended)
                        if i32::from(c) != i32::from(ch) || plant(9568) {
                            e.text.text.pop();
                            e.text.text.push(ch);
                            true
                        } else {
                            false
                        }
                    } else {
                        set_text(&mut k, e, &[]);
                        e.text.text.push(ch);
                        k.text_buffer_kept = false;
                        e.char_pos = 1;
                        true
                    }
                };
                *h.keyboard_options() = k;
                if send {
                    tell_owner(wm, h, window, gem::UPDATE_TEXT);
                }
                return Handled;
            }
        }
        gwm::CHAR => {
            let down = d2 & key::STATE_DOWN != 0;
            let up = d2 & key::STATE_UP != 0;
            let modifier = |h: &mut H| -> Option<Vec<u16>> {
                let k = h.keyboard_options();
                match d1 {
                    keys::LCTRL => Some(k.ctrl.clone()),
                    keys::RSHIFT | keys::LSHIFT => Some(k.shift.clone()),
                    keys::LALT => Some(k.alt.clone()),
                    _ => None,
                }
            };
            if let Some(m) = modifier(h) {
                if down {
                    do_key_down(wm, h, window, &m);
                    tell_owner(wm, h, window, gem::UPDATE_TEXT);
                    return Handled;
                }
                if up {
                    do_key_up(wm, h, window, &m);
                    tell_owner(wm, h, window, gem::UPDATE_TEXT);
                    return Handled;
                }
            } else if d1 == u32::from(key::ESC) || d1 == keys::PGUP || d1 == keys::PGDN || d1 == u32::from(key::HOME) || d1 == u32::from(key::END) || keys::F.contains(&d1) || d1 == keys::CAPS {
                return Ignored;
            } else if d1 == u32::from(key::DOWN) || d1 == u32::from(key::RIGHT) || d1 == u32::from(key::TAB) {
                if down {
                    wm.win_next_tab(h, window);
                }
            } else if d1 == u32::from(key::UP) || d1 == u32::from(key::LEFT) {
                if down {
                    wm.win_prev_tab(h, window);
                }
            } else if d1 == u32::from(key::BACKSPACE) {
                let mut k = std::mem::take(h.keyboard_options());
                let e = entry_mut(wm, window).expect("checked");
                set_text(&mut k, e, &[]);
                e.char_pos = e.text.text.len() as u16;
                *h.keyboard_options() = k;
                tell_owner(wm, h, window, gem::UPDATE_TEXT);
                let (shift, ctrl, alt) = {
                    let k = h.keyboard_options();
                    (k.shift.clone(), k.ctrl.clone(), k.alt.clone())
                };
                set_key_down(h, &shift, false);
                set_key_down(h, &ctrl, false);
                if !plant(9569) {
                    set_key_down(h, &alt, false);
                }
                return Handled;
            } else if d1 == u32::from(key::DEL) && down {
                let e = entry_mut(wm, window).expect("checked");
                if e.con_char_pos == 0 && e.char_pos > 0 {
                    // (`removeLastChar` of the last character keeps the buffer; of an empty text, nothing)
                    let emptied = e.text.text.pop().is_some() && e.text.text.is_empty();
                    e.s_text.text.pop();
                    e.char_pos -= 1;
                    if emptied {
                        h.keyboard_options().text_buffer_kept = true;
                    }
                    tell_owner(wm, h, window, gem::UPDATE_TEXT);
                }
            }
        }
        gwm::LEFT_DOWN => {
            wm.tree.windows[window].inst.state |= WIN_STATE_HILITED;
            wm.win_set_focus(h, Some(window));
        }
        gwm::MOUSE_ENTERING => {
            if style & gws::MOUSE_TRACK != 0 {
                wm.tree.windows[window].inst.state |= WIN_STATE_HILITED;
                tell_owner(wm, h, window, ggm::GBM_MOUSE_ENTERING);
                wm.win_set_focus(h, Some(window));
            }
        }
        gwm::MOUSE_LEAVING => {
            if style & gws::MOUSE_TRACK != 0 {
                wm.tree.windows[window].inst.state &= !WIN_STATE_HILITED;
                tell_owner(wm, h, window, ggm::GBM_MOUSE_LEAVING);
            }
        }
        gwm::LEFT_DRAG => {
            if style & gws::MOUSE_TRACK != 0 && !plant(9570) {
                tell_owner(wm, h, window, ggm::LEFT_DRAG);
            }
        }
        _ => return Ignored,
    }
    Handled
}
