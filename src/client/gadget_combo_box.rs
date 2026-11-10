// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetComboBox.cpp (GadgetComboBoxInput, HideListBox,
//     GadgetComboBoxSystem, GadgetComboBoxSetColors, GadgetComboBoxSetIsEditable,
//     GadgetComboBoxSetLettersAndNumbersOnly, GadgetComboBoxSetAsciiOnly, GadgetComboBoxSetMaxChars,
//     GadgetComboBoxSetMaxDisplay, GadgetComboBoxGetText, GadgetComboBoxSetText, GadgetComboBoxAddEntry,
//     GadgetComboBoxReset, GadgetComboBoxHideList, GadgetComboBoxSetFont, GadgetComboBoxSetEnabledTextColors,
//     GadgetComboBoxSetDisabledTextColors, GadgetComboBoxSetHiliteTextColors, GadgetComboBoxGetSelectedPos,
//     GadgetComboBoxSetSelectedPos, GadgetComboBoxSetItemData, GadgetComboBoxGetItemData, GadgetComboBoxGetLength)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GadgetComboBox.h (GadgetComboBoxGetDropDownButton,
//     GadgetComboBoxGetListBox, GadgetComboBoxGetEditBox)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoGadgetComboBox)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (createGadget's combo box branch:
//     the data it gives gogoGadgetComboBox, and the parts' draw data)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Gadget.h (ComboBoxData, the GCM_* messages)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The combo box (GitHub #72, a part of #34): a drop-down button, an edit box (a text entry) and a list box that opens
//! below them.
//!
//! # As upstream reads it
//!
//! `gogoGadgetComboBox` makes three children: the **drop-down button** (21 pixels wide, the combo box's height, at
//! its right edge), the **edit box** (the rest; `NO_INPUT` unless the combo box is editable, and then its text is
//! drawn from its start) and the **list box** (as wide and as high as the combo box, below it, `ABOVE` and
//! `ONE_LINE`, hidden), all owned by the combo box. `ComboBoxData` keeps them, `isEditable`, `maxDisplay` (the rows
//! shown before the list scrolls), `maxChars` (the edit box's `maxTextLen`), the filters and `entryCount`.
//!
//! - **Opening** (the drop-down button's `GBM_SELECTED`, or a left release on the combo box itself, which a
//!   non-editable edit box passes on): the combo box becomes the manager's lone window; a hidden list is shown,
//!   `min(entryCount, maxDisplay)` rows high (`fontHeight * n + 2n + 4`), its scroll bar hidden when every row fits,
//!   and the combo box grows by its height. A shown list is hidden (`HideListBox`: the combo box goes back to the
//!   edit box's height). Any click another window takes, or `GGM_CLOSE`, hides it.
//! - **Choosing**: the list's `GLM_SELECTED` hides it (unless `dontHide`, set by `GCM_SET_SELECTION` on an open list)
//!   and, for a row, copies the row's text and colour into the edit box and tells the owner `GCM_SELECTED`. Typing in
//!   the edit box (`GEM_UPDATE_TEXT`) tells the owner `GCM_UPDATE_TEXT`, clears the list's selection and hides it;
//!   return (`GEM_EDIT_DONE`) hides it and sends `GCM_SELECTED`.
//! - **Adding** (`GCM_ADD_ENTRY`): `entryCount` grows; when it reaches the list's length the length doubles; the text
//!   goes to the end of the list (column 0, overwrite); the list is placed below the edit box and sized as when
//!   opening.
//! - **Keys**: down, right and tab go to the next window of the tab list, up and left to the previous; any other key
//!   goes to the edit box.
//!
//! # Upstream quirks that are kept
//!
//! - The list box's instance data gets `WIN_STATUS_HIDDEN` in its **style** (`BitSet( winInstData.m_style,
//!   WIN_STATUS_HIDDEN )`): that is `GWS_HORZ_SLIDER`, so the combo box's list is styled a horizontal slider too. Its
//!   scroll buttons' `GWM_MOUSE_ENTERING` then hilites it (the push button hilites a horizontal slider parent), and
//!   its border is a slider's (none).
//! - The edit box is made with the label `Entry`, which `gogoGadgetTextEntry` sets as its text: every combo box starts
//!   with `Entry` in its edit box.
//! - `GadgetComboBoxSetMaxChars` sets the edit box's `maxTextLen` without `gogoGadgetTextEntry`'s clamp to 256 (and
//!   truncated to a `Short`).
//! - `GadgetComboBoxSetIsEditable(TRUE)` cannot clear `NO_INPUT`: `winSetStatus` only sets bits.
//! - `GCM_ADD_ENTRY` makes a short list (all rows shown) 16 pixels **wider** than the combo box; opening it makes it
//!   as wide. While the list is hidden, `GGM_RESIZED` gives it the combo box's whole size.
//! - `GCM_ADD_ENTRY` returns the added index **as** the message's `WindowMsgHandledType`.
//! - `GCM_DEL_ALL` sets the edit text empty and `entryCount` 0, but keeps the list's length and leaves the list open.
//! - Opening from a left release plays `GUIClick`; from the drop-down button's `GBM_SELECTED` it plays nothing more
//!   (the button played its own).
//!
//! # Upstream undefined behaviour, and what the translation does
//!
//! The combo box keeps its children as pointers: once one of them is destroyed and freed, a use of it is a use of
//! freed memory (the manager's [`GameWindowManager::undefined`] is set and the message stops). So is a combo box whose
//! data was deleted (`GWM_DESTROY`) and a `GCM_ADD_ENTRY` or `GGM_RESIZED` without a list box or a drop-down button.
//!
//! # Where this differs from upstream
//!
//! The pointers its messages pass (`UnicodeString*`, `Int*`, `void**`) are the manager's [`ComboMsg`]; the return
//! value of `GCM_ADD_ENTRY` is [`ComboMsg::added`] (the message returns `MSG_HANDLED`). The IME composite text
//! colours (`GadgetComboBoxSetIMECompositeTextColors`) are not modelled: nothing draws them without an IME. The combo
//! box's `ComboBoxData` points to its edit box's `EntryData` and list box's `ListboxData` (`entryData`,
//! `listboxData`); the translation reaches them through the windows.
//!
//! # Representation
//!
//! [`ComboRt`] in the window's [`UserData::Combo`].

use crate::client::gadget_list_box::{self as lbx, listbox_rt, ListboxRt};
use crate::client::gadget_text_entry::{self as te, EntryRt};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, DrawFunc, GameWindowManager, GuiHost, InputFunc, MsgHandled, SystemFunc, UserData, WindowMsgData, WIN_STATE_HILITED};
use crate::client::window_script::{gws, win_status, ComboBoxData, GadgetPart, GameFont, WinId, WinInstanceData};

use MsgHandled::{Handled, Ignored};

/// The combo box's messages (Gadget.h, after the list box's).
pub mod gcm {
    pub const ADD_ENTRY: u32 = 16416;
    pub const DEL_ENTRY: u32 = 16417;
    pub const DEL_ALL: u32 = 16418;
    pub const SELECTED: u32 = 16419;
    pub const GET_TEXT: u32 = 16420;
    pub const SET_TEXT: u32 = 16421;
    pub const EDIT_DONE: u32 = 16422;
    pub const GET_ITEM_DATA: u32 = 16423;
    pub const SET_ITEM_DATA: u32 = 16424;
    pub const GET_SELECTION: u32 = 16425;
    pub const SET_SELECTION: u32 = 16426;
    pub const UPDATE_TEXT: u32 = 16427;
}

/// The drop-down button's size (`buttonWidth`; the height is the combo box's).
pub const DROP_DOWN_WIDTH: i32 = 21;

/// `ComboBoxData` as the combo box keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboRt {
    pub is_editable: bool,
    pub max_display: i32,
    pub max_chars: i32,
    pub ascii_only: bool,
    pub letters_and_numbers_only: bool,
    /// `entryData`: the edit box whose `EntryData` it points to.
    pub entry_data: Option<WinId>,
    pub dont_hide: bool,
    pub entry_count: i32,
    pub drop_down_button: Option<WinId>,
    pub edit_box: Option<WinId>,
    pub list_box: Option<WinId>,
}

/// What `gogoGadgetComboBox` is given: the `ComboBoxData` and the templates its `entryData` and `listboxData` point to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboTemplate {
    pub combo: ComboRt,
    pub entry: EntryRt,
    pub listbox: ListboxRt,
}

/// The data the combo box's messages point to (upstream passes pointers).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboMsg {
    /// `GCM_ADD_ENTRY`'s, `GCM_SET_TEXT`'s string; `GCM_GET_TEXT`'s answer.
    pub text: Vec<u16>,
    /// `GCM_ADD_ENTRY`'s return value.
    pub added: i32,
    /// `GCM_GET_SELECTION`'s `Int`.
    pub selection: i32,
    /// `GCM_GET_ITEM_DATA`'s `void*`.
    pub item_data: u32,
}

/// The combo box's data, if the window has some.
pub fn combo_rt(wm: &GameWindowManager, w: WinId) -> Option<&ComboRt> {
    match &wm.ext[w].user {
        UserData::Combo(c) => Some(c),
        _ => None,
    }
}

/// An undefined operation upstream: recorded, and the caller stops.
struct Ub;
type R<T> = Result<T, Ub>;

fn ub<T>(wm: &mut GameWindowManager, what: &'static str) -> R<T> {
    wm.set_undefined(what);
    Err(Ub)
}

/// The combo box's data (upstream dereferences it).
fn cb(wm: &mut GameWindowManager, w: WinId) -> R<ComboRt> {
    match combo_rt(wm, w) {
        Some(c) => Ok(c.clone()),
        None => ub(wm, "combo box: no ComboBoxData (deleted, or NULL user data)"),
    }
}

fn cb_mut(wm: &mut GameWindowManager, w: WinId) -> R<&mut ComboRt> {
    if combo_rt(wm, w).is_none() {
        return ub(wm, "combo box: no ComboBoxData (deleted, or NULL user data)");
    }
    match &mut wm.ext[w].user {
        UserData::Combo(c) => Ok(c),
        _ => unreachable!("checked"),
    }
}

/// One of the combo box's child pointers, used: freed is undefined.
fn live(wm: &mut GameWindowManager, w: Option<WinId>) -> R<Option<WinId>> {
    match w {
        Some(x) if wm.ext[x].freed => ub(wm, "combo box: a child window used after it was freed"),
        _ => Ok(w),
    }
}

/// `GadgetComboBoxGetDropDownButton`.
pub fn get_drop_down_button(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    combo_rt(wm, w).and_then(|c| c.drop_down_button)
}
/// `GadgetComboBoxGetListBox`.
pub fn get_list_box(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    combo_rt(wm, w).and_then(|c| c.list_box)
}
/// `GadgetComboBoxGetEditBox`.
pub fn get_edit_box(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    combo_rt(wm, w).and_then(|c| c.edit_box)
}

/// The list box's `ListboxData` (upstream dereferences it).
fn list_data(wm: &mut GameWindowManager, l: WinId) -> R<ListboxRt> {
    match listbox_rt(wm, l) {
        Some(d) => Ok(d.clone()),
        None => ub(wm, "combo box: the list box has no ListboxData"),
    }
}

fn hidden(wm: &GameWindowManager, w: WinId) -> bool {
    wm.tree.windows[w].status & win_status::HIDDEN != 0
}

/// Shows or hides the list's scroll bar (`upButton`, `downButton`, `slider`).
fn hide_scroll_bar(wm: &mut GameWindowManager, l: &ListboxRt, hide: bool) -> R<()> {
    for b in [l.up_button, l.down_button, l.slider] {
        if let Some(b) = live(wm, b)? {
            wm.win_hide(b, hide);
        }
    }
    Ok(())
}

/// The list's height for `n` rows: `winFontHeight(listFont) * n + n * 2 + 4`.
fn list_height<H: GuiHost>(wm: &GameWindowManager, h: &H, l: WinId, n: i32) -> i32 {
    let fh = h.font_height(wm.tree.windows[l].inst.font.as_ref());
    fh.wrapping_mul(n).wrapping_add(n.wrapping_mul(2)).wrapping_add(4)
}

/// The left release's and the drop-down button's toggle: open a hidden list, hide a shown one.
fn toggle<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) -> R<()> {
    let c = cb_mut(wm, w)?;
    c.dont_hide = false;
    let lb = c.list_box;
    let Some(l) = live(wm, lb)? else { return Ok(()) };
    wm.win_set_lone_window(h, Some(w));
    if hidden(wm, l) {
        wm.win_hide(l, false);
        let (wx, wy) = wm.tree.windows[w].size;
        let ld = list_data(wm, l)?;
        let c = cb(wm, w)?;
        let multiplier = if c.entry_count <= c.max_display {
            hide_scroll_bar(wm, &ld, true)?;
            c.entry_count
        } else {
            hide_scroll_bar(wm, &ld, false)?;
            c.max_display
        };
        let ny = list_height(wm, h, l, multiplier);
        wm.win_set_size(h, w, wx, wy.wrapping_add(ny));
        if wm.undefined.is_some() {
            return Err(Ub);
        }
        wm.win_set_position(l, 0, wy);
        wm.win_set_size(h, l, wx, ny);
    } else {
        hide_list_box(wm, h, w)?;
    }
    Ok(())
}

/// `HideListBox(window)`: hides a shown list and shrinks the combo box to the edit box's height.
fn hide_list_box<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) -> R<()> {
    let lb = cb(wm, w)?.list_box;
    let Some(l) = live(wm, lb)? else { return Ok(()) };
    if !hidden(wm, l) {
        wm.win_hide(l, true);
        let eb = cb(wm, w)?.edit_box;
        let Some(e) = live(wm, eb)? else { return ub(wm, "HideListBox: editBox NULL") };
        let ey = wm.tree.windows[e].size.1;
        let wx = wm.tree.windows[w].size.0;
        wm.win_set_size(h, w, wx, ey);
    }
    Ok(())
}

fn owner(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.ext[w].owner
}

/// `GadgetComboBoxInput`.
pub fn combo_box_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    combo_box_input_r(wm, h, w, msg, d1, d2).unwrap_or(Handled)
}

fn combo_box_input_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> R<MsgHandled> {
    let edit = get_edit_box(wm, w);
    match msg {
        gwm::CHAR => match d1 {
            x if x == u32::from(key::DOWN) || x == u32::from(key::RIGHT) || x == u32::from(key::TAB) => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_next_tab(h, w);
                }
            }
            x if x == u32::from(key::UP) || x == u32::from(key::LEFT) => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_prev_tab(h, w);
                }
            }
            _ => {
                let e = live(wm, edit)?;
                return Ok(wm.win_send_input_msg(h, e, gwm::CHAR, d1, d2));
            }
        },
        gwm::WHEEL_DOWN | gwm::WHEEL_UP | gwm::RIGHT_UP => {}
        gwm::LEFT_UP => {
            cb(wm, w)?;
            h.add_audio_event(b"GUIClick");
            toggle(wm, h, w)?;
        }
        gwm::LEFT_DRAG => {
            if wm.tree.windows[w].inst.style & gws::MOUSE_TRACK != 0 {
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, ggm::LEFT_DRAG, win_data(Some(w)), 0);
            }
        }
        _ => return Ok(Ignored),
    }
    Ok(Handled)
}

/// `GadgetComboBoxSystem`.
pub fn combo_box_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    combo_box_system_r(wm, h, w, msg, d1, d2).unwrap_or(Handled)
}

fn combo_box_system_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> R<MsgHandled> {
    match msg {
        ggm::SET_LABEL => {
            let t = wm.msg_text.clone();
            wm.win_set_text(w, &t);
        }
        gcm::GET_TEXT => {
            let eb = cb(wm, w)?.edit_box;
            if let Some(e) = live(wm, eb)? {
                wm.combo_msg.text = te::get_text(wm, h, Some(e));
            }
        }
        gcm::SET_TEXT => {
            let eb = cb(wm, w)?.edit_box;
            if let Some(e) = live(wm, eb)? {
                let t = wm.combo_msg.text.clone();
                te::set_text(wm, h, Some(e), &t);
            }
        }
        te::gem::UPDATE_TEXT => {
            let c = cb(wm, w)?;
            let o = owner(wm, w);
            wm.win_send_system_msg(h, o, gcm::UPDATE_TEXT, win_data(Some(w)), 0);
            if let Some(l) = live(wm, c.list_box)? {
                lbx::set_selected(wm, h, l, &[-1]);
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
                hide_list_box(wm, h, w)?;
            }
        }
        te::gem::EDIT_DONE => {
            let c = cb(wm, w)?;
            if data_win(d1) == c.edit_box {
                hide_list_box(wm, h, w)?;
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, gcm::SELECTED, win_data(Some(w)), 0);
            }
        }
        gcm::SET_SELECTION => {
            let lb = get_list_box(wm, w);
            if let Some(l) = live(wm, lb)? {
                if !hidden(wm, l) && d2 == 1 {
                    cb_mut(wm, w)?.dont_hide = true;
                }
                lbx::set_selected(wm, h, l, &[d1 as i32]);
            }
        }
        gcm::GET_SELECTION => {
            let c = cb(wm, w)?;
            if let Some(l) = live(wm, c.list_box)? {
                // `GadgetListBoxGetSelected(listBox, (Int *)mData2)`: a single-select list writes its selection
                let s = lbx::get_selected(wm, h, l);
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
                if let Some(&v) = s.first() {
                    if listbox_rt(wm, l).is_some_and(|d| d.multi_select) {
                        return ub(wm, "GCM_GET_SELECTION: a multi-select list writes a pointer into the Int");
                    }
                    wm.combo_msg.selection = v;
                }
            } else {
                wm.combo_msg.selection = -1;
            }
        }
        gcm::SET_ITEM_DATA => {
            let c = cb(wm, w)?;
            if let Some(l) = live(wm, c.list_box)? {
                lbx::set_item_data(wm, h, l, d2, d1 as i32, 0);
            }
        }
        gcm::GET_ITEM_DATA => {
            let c = cb(wm, w)?;
            if let Some(l) = live(wm, c.list_box)? {
                wm.combo_msg.item_data = lbx::get_item_data(wm, h, l, d1 as i32, 0);
            }
        }
        lbx::glm::SELECTED => {
            let c = cb(wm, w)?;
            if data_win(d1) == c.list_box {
                if c.dont_hide {
                    cb_mut(wm, w)?.dont_hide = false;
                } else {
                    hide_list_box(wm, h, w)?;
                }
                if d2 as i32 == -1 {
                    return Ok(Handled);
                }
                let l = live(wm, c.list_box)?.expect("compared with mData1");
                let (text, color) = lbx::get_text_and_color(wm, h, l, d2 as i32, 0);
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
                let eb = live(wm, c.edit_box)?;
                let Some(e) = eb else { return ub(wm, "GLM_SELECTED: GadgetTextEntrySetTextColor(NULL)") };
                te::set_text_color(wm, e, color);
                te::set_text(wm, h, Some(e), &text);
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, gcm::SELECTED, win_data(Some(w)), 0);
            }
        }
        ggm::LEFT_DRAG => {}
        gcm::DEL_ALL => {
            let c = cb(wm, w)?;
            if let Some(l) = live(wm, c.list_box)? {
                lbx::reset(wm, h, l);
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
            }
            if let Some(e) = live(wm, c.edit_box)? {
                te::set_text(wm, h, Some(e), &[]);
            }
            cb_mut(wm, w)?.entry_count = 0;
        }
        gcm::DEL_ENTRY => {}
        ggm::CLOSE => hide_list_box(wm, h, w)?,
        gcm::ADD_ENTRY => {
            let lb = get_list_box(wm, w);
            let Some(l) = live(wm, lb)? else { return ub(wm, "GCM_ADD_ENTRY: listBox NULL dereferenced") };
            let ld = list_data(wm, l)?;
            let c = cb_mut(wm, w)?;
            c.entry_count = c.entry_count.wrapping_add(1);
            let count = c.entry_count;
            if count >= i32::from(ld.list_length) {
                lbx::set_list_length(wm, h, l, i32::from(ld.list_length).wrapping_mul(2));
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
            }
            let text = wm.combo_msg.text.clone();
            let added = lbx::add_entry_text(wm, h, l, &text, d2 as i32, -1, 0, true);
            if wm.undefined.is_some() {
                return Err(Ub);
            }
            let ld = list_data(wm, l)?;
            let c = cb(wm, w)?;
            let eb = live(wm, c.edit_box)?;
            let Some(e) = eb else { return ub(wm, "GCM_ADD_ENTRY: editBox NULL dereferenced") };
            let wx = wm.tree.windows[w].size.0;
            let ey = wm.tree.windows[e].size.1;
            let (multiplier, list_x) = if c.entry_count <= c.max_display {
                hide_scroll_bar(wm, &ld, true)?;
                (c.entry_count, wx.wrapping_add(if crate::logic::modules::plant(4706) { 0 } else { 16 }))
            } else {
                hide_scroll_bar(wm, &ld, false)?;
                (c.max_display, wx)
            };
            let ny = list_height(wm, h, l, multiplier);
            wm.win_set_position(l, 0, ey);
            wm.win_set_size(h, l, list_x, ny);
            wm.combo_msg.added = added;
        }
        gwm::CREATE => {}
        ggm::RESIZED => {
            let (width, height) = (d1 as i32, d2 as i32);
            let c = cb(wm, w)?;
            let Some(dd) = live(wm, c.drop_down_button)? else { return ub(wm, "GGM_RESIZED: dropDownButton NULL dereferenced") };
            let dx = wm.tree.windows[dd].size.0;
            let Some(l) = live(wm, c.list_box)? else { return ub(wm, "GGM_RESIZED: listBox NULL dereferenced") };
            if hidden(wm, l) {
                wm.win_set_size(h, l, width, height);
                if wm.undefined.is_some() {
                    return Err(Ub);
                }
                wm.win_set_position(dd, width.wrapping_sub(dx), 0);
                if let Some(e) = live(wm, c.edit_box)? {
                    wm.win_set_position(e, 0, 0);
                    wm.win_set_size(h, e, width.wrapping_sub(dx), height);
                }
            }
        }
        gwm::DESTROY => {
            wm.win_set_lone_window(h, None);
            wm.ext[w].user = UserData::None;
        }
        gwm::INPUT_FOCUS => {
            if d1 == 0 {
                wm.tree.windows[w].inst.state &= !WIN_STATE_HILITED;
            } else {
                wm.tree.windows[w].inst.state |= WIN_STATE_HILITED;
            }
            let o = owner(wm, w);
            let id = wm.tree.windows[w].id;
            wm.win_send_system_msg(h, o, ggm::FOCUS_CHANGE, d1, id as u32);
            // the edit box answers into a local `wantsFocus`; the combo box always wants the focus
            let e = get_edit_box(wm, w);
            let e = live(wm, e)?;
            wm.win_send_system_msg(h, e, gwm::INPUT_FOCUS, d1, 1);
            wm.focus_reply = true;
        }
        ggm::GBM_SELECTED => {
            let c = cb(wm, w)?;
            if data_win(d1) == c.drop_down_button {
                toggle(wm, h, w)?;
            }
        }
        _ => return Ok(Ignored),
    }
    Ok(Handled)
}

/// `GadgetComboBoxSetIsEditable(comboBox, isEditable)`: `NO_INPUT` on the edit box (`winSetStatus` cannot clear it).
pub fn set_is_editable(wm: &mut GameWindowManager, w: WinId, editable: bool) {
    let _ = (|| -> R<()> {
        let Some(e) = live(wm, get_edit_box(wm, w))? else { return Ok(()) };
        cb_mut(wm, w)?.is_editable = editable;
        let mut status = wm.tree.windows[e].status;
        if editable {
            status &= !win_status::NO_INPUT;
        } else {
            status |= win_status::NO_INPUT;
        }
        wm.win_set_status(e, status);
        Ok(())
    })();
}

/// The edit box's `EntryData` the combo box points to (`comboData->entryData`), mutable.
fn entry_data(wm: &mut GameWindowManager, w: WinId) -> R<Option<&mut EntryRt>> {
    let Some(e) = cb(wm, w)?.entry_data else { return Ok(None) };
    if wm.ext[e].freed || !matches!(wm.ext[e].user, UserData::Entry(_)) {
        return ub(wm, "combo box: entryData used after it was freed");
    }
    match &mut wm.ext[e].user {
        UserData::Entry(d) => Ok(Some(d)),
        _ => unreachable!("checked"),
    }
}

/// `GadgetComboBoxSetLettersAndNumbersOnly`.
pub fn set_letters_and_numbers_only(wm: &mut GameWindowManager, w: WinId, on: bool) {
    let _ = (|| -> R<()> {
        cb_mut(wm, w)?.letters_and_numbers_only = on;
        if let Some(d) = entry_data(wm, w)? {
            d.alpha_numerical_only = on;
        }
        Ok(())
    })();
}

/// `GadgetComboBoxSetAsciiOnly`.
pub fn set_ascii_only(wm: &mut GameWindowManager, w: WinId, on: bool) {
    let _ = (|| -> R<()> {
        cb_mut(wm, w)?.ascii_only = on;
        if let Some(d) = entry_data(wm, w)? {
            d.ascii_only = on;
        }
        Ok(())
    })();
}

/// `GadgetComboBoxSetMaxChars` (the edit box's `maxTextLen`, a `Short`, unclamped).
pub fn set_max_chars(wm: &mut GameWindowManager, w: WinId, max_chars: i32) {
    let _ = (|| -> R<()> {
        cb_mut(wm, w)?.max_chars = max_chars;
        match entry_data(wm, w)? {
            Some(d) => d.max_text_len = max_chars as i16,
            None => return ub(wm, "GadgetComboBoxSetMaxChars: entryData NULL dereferenced"),
        }
        Ok(())
    })();
}

/// `GadgetComboBoxSetMaxDisplay`.
pub fn set_max_display(wm: &mut GameWindowManager, w: WinId, max_display: i32) {
    let _ = cb_mut(wm, w).map(|c| c.max_display = max_display);
}

/// `GadgetComboBoxGetText`: the edit box's text (empty if not a combo box).
pub fn get_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) -> Vec<u16> {
    if wm.tree.windows[w].inst.style & gws::COMBO_BOX == 0 {
        return Vec::new();
    }
    let e = get_edit_box(wm, w);
    match live(wm, e) {
        Ok(e) => te::get_text(wm, h, e),
        Err(Ub) => Vec::new(),
    }
}

/// `GadgetComboBoxSetText`: the edit box's text.
pub fn set_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, text: &[u16]) {
    let e = get_edit_box(wm, w);
    if let Ok(e) = live(wm, e) {
        te::set_text(wm, h, e, text);
    }
}

/// `GadgetComboBoxAddEntry(comboBox, text, color)`: the list's index of the new row.
pub fn add_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, text: &[u16], color: i32) -> i32 {
    wm.combo_msg.text = text.to_vec();
    // a message that does not handle it returns MSG_IGNORED, 0
    wm.combo_msg.added = 0;
    wm.win_send_system_msg(h, Some(w), gcm::ADD_ENTRY, 1, color as u32);
    wm.combo_msg.added
}

/// `GadgetComboBoxReset` (`GCM_DEL_ALL`).
pub fn reset<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    wm.win_send_system_msg(h, Some(w), gcm::DEL_ALL, 0, 0);
}

/// `GadgetComboBoxHideList` (`GGM_CLOSE`).
pub fn hide_list<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    wm.win_send_system_msg(h, Some(w), ggm::CLOSE, 0, 0);
}

/// `GadgetComboBoxSetFont(comboBox, font)`: the list box's and the edit box's (`winSetFont`); the combo box's own
/// display strings take the window's font when drawn.
pub fn set_font(wm: &mut GameWindowManager, w: WinId, font: Option<GameFont>) {
    if let Some(l) = get_list_box(wm, w) {
        if !wm.ext[l].freed {
            wm.win_set_font(l, font.clone());
        }
    }
    if let Some(e) = get_edit_box(wm, w) {
        if !wm.ext[e].freed {
            wm.win_set_font(e, font);
        }
    }
}

/// `GadgetComboBoxSet{Enabled,Disabled,Hilite}TextColors`: the list box's and the edit box's (`which` 0, 1, 2).
pub fn set_text_colors(wm: &mut GameWindowManager, w: WinId, which: usize, color: i32, border: i32) {
    let Some(c) = combo_rt(wm, w).cloned() else { return };
    for x in [c.list_box, c.edit_box].into_iter().flatten() {
        if !wm.ext[x].freed {
            wm.win_set_text_colors(x, which, color, border);
        }
    }
}

/// `GadgetComboBoxGetSelectedPos(comboBox, &index)`: `None` when nothing answered (the caller's `Int` unchanged).
pub fn get_selected_pos<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, initial: i32) -> i32 {
    wm.combo_msg.selection = initial;
    wm.win_send_system_msg(h, Some(w), gcm::GET_SELECTION, 0, 1);
    wm.combo_msg.selection
}

/// `GadgetComboBoxSetSelectedPos(comboBox, index, dontHide)`.
pub fn set_selected_pos<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, index: i32, dont_hide: bool) {
    wm.win_send_system_msg(h, Some(w), gcm::SET_SELECTION, index as u32, u32::from(dont_hide));
}

/// `GadgetComboBoxSetItemData(comboBox, index, data)`.
pub fn set_item_data<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, index: i32, data: u32) {
    wm.win_send_system_msg(h, Some(w), gcm::SET_ITEM_DATA, index as u32, data);
}

/// `GadgetComboBoxGetItemData(comboBox, index)`.
pub fn get_item_data<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, index: i32) -> u32 {
    wm.combo_msg.item_data = 0;
    wm.win_send_system_msg(h, Some(w), gcm::GET_ITEM_DATA, index as u32, 1);
    wm.combo_msg.item_data
}

/// `GadgetComboBoxGetLength`: `entryCount`.
pub fn get_length(wm: &GameWindowManager, w: WinId) -> i32 {
    combo_rt(wm, w).map_or(0, |c| c.entry_count)
}

/// The script's `ComboBoxData` with the data `createGadget` gives it: an `EntryData` with its filters and `maxChars`,
/// and a `ListboxData` of 10 rows, one column, a scroll bar and force-select.
pub fn combo_template(d: &ComboBoxData) -> ComboTemplate {
    ComboTemplate {
        combo: ComboRt { is_editable: d.is_editable, max_display: d.max_display, max_chars: d.max_chars, ascii_only: d.ascii_only, letters_and_numbers_only: d.letters_and_numbers_only, ..Default::default() },
        entry: EntryRt { ascii_only: d.ascii_only, alpha_numerical_only: d.letters_and_numbers_only, max_text_len: d.max_chars as i16, ..Default::default() },
        listbox: ListboxRt { list_length: 10, scroll_bar: true, force_select: true, columns: 1, ..Default::default() },
    }
}

/// `gogoGadgetComboBox(parent, status, x, y, width, height, instData, comboBoxData, defaultFont, defaultVisual)`
/// (`defaultFont` is the instance data's font). `None` where upstream asserts (not a combo box style).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_combo_box<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, template: ComboTemplate, default_visual: bool) -> Option<WinId> {
    if inst.style & gws::COMBO_BOX == 0 {
        return None;
    }
    let font = inst.font.clone();
    let c = wm.win_create(h, parent, status, x, y, width, height, SystemFunc::ComboBox, Some(inst));
    combo_box_after_create(wm, h, c, parent, status, font, template, default_visual);
    Some(c)
}

/// `gogoGadgetComboBox` after its `winCreate`: the data, owner, callbacks, the three children, the settings, the text
/// colours, the look.
#[allow(clippy::too_many_arguments)]
pub fn combo_box_after_create<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, c: WinId, parent: Option<WinId>, status: u32, font: Option<GameFont>, template: ComboTemplate, default_visual: bool) {
    let (width, height) = wm.tree.windows[c].size;
    let mut data = template.combo;
    data.entry_data = None;
    data.drop_down_button = None;
    data.edit_box = None;
    data.list_box = None;
    wm.ext[c].user = UserData::Combo(data.clone());
    wm.win_set_owner(c, parent);
    let image = wm.tree.windows[c].status & win_status::IMAGE != 0;
    wm.ext[c].draw = if image { DrawFunc::ComboBoxImage } else { DrawFunc::ComboBox };
    wm.ext[c].input = InputFunc::ComboBox;
    let mut status = status & !(win_status::BORDER | win_status::HIDDEN);
    let track = wm.tree.windows[c].inst.style & gws::MOUSE_TRACK;
    let tip_func = wm.ext[c].tooltip.clone();
    let tip = wm.tree.windows[c].inst.tooltip_text.clone();
    let delay = wm.tree.windows[c].inst.tooltip_delay;
    let tooltip = |wm: &mut GameWindowManager, x: WinId| {
        wm.ext[x].tooltip = tip_func.clone();
        wm.tree.windows[x].inst.tooltip_text = Some(tip.clone().unwrap_or_default());
        wm.tree.windows[x].inst.tooltip_delay = delay;
    };
    // the drop-down button
    let bi = WinInstanceData { style: gws::PUSH_BUTTON | track, ..WinInstanceData::new() };
    let dd = wm.gogo_push_button(h, Some(c), status | win_status::ACTIVE | win_status::ENABLED, width - DROP_DOWN_WIDTH, 0, DROP_DOWN_WIDTH, height, bi, true);
    tooltip(wm, dd);
    if let UserData::Combo(d) = &mut wm.ext[c].user {
        d.drop_down_button = Some(dd);
    }
    // the edit box
    let ei = WinInstanceData { style: gws::ENTRY_FIELD | track, text_label_string: b"Entry".to_vec(), ..WinInstanceData::new() };
    let mut entry = template.entry;
    let status_text = if data.is_editable {
        status
    } else {
        entry.draw_text_from_start = true;
        status | win_status::NO_INPUT
    };
    let e = te::gogo_gadget_text_entry(wm, h, Some(c), status_text, 0, 0, width - DROP_DOWN_WIDTH, height, ei, entry, false).expect("an entry field style");
    tooltip(wm, e);
    if let UserData::Combo(d) = &mut wm.ext[c].user {
        d.edit_box = Some(e);
        d.entry_data = Some(e);
    }
    // the list box (`WIN_STATUS_HIDDEN` set in its **style**: `GWS_HORZ_SLIDER`)
    let li = WinInstanceData { style: track | win_status::HIDDEN | gws::SCROLL_LISTBOX, ..WinInstanceData::new() };
    status &= !win_status::IMAGE;
    let l = lbx::gogo_gadget_list_box(wm, h, Some(c), status | win_status::ABOVE | win_status::ONE_LINE, 0, height, width, height, li, template.listbox);
    let Some(l) = l.filter(|_| wm.undefined.is_none()) else {
        wm.set_undefined("gogoGadgetComboBox: gogoGadgetListBox returned NULL (dereferenced)");
        return;
    };
    wm.win_hide(l, true);
    tooltip(wm, l);
    lbx::set_audio_feedback(wm, l, true);
    if let UserData::Combo(d) = &mut wm.ext[c].user {
        d.list_box = Some(l);
    }
    set_is_editable(wm, c, data.is_editable);
    set_max_chars(wm, c, data.max_chars);
    set_max_display(wm, c, data.max_display);
    let inst = wm.tree.windows[c].inst.clone();
    for (which, t) in [(0, inst.enabled_text), (1, inst.disabled_text), (2, inst.hilite_text)] {
        wm.win_set_text_colors(l, which, t.color, t.border_color);
        wm.win_set_text_colors(e, which, t.color, t.border_color);
    }
    if let UserData::Combo(d) = &mut wm.ext[c].user {
        d.dont_hide = false;
    }
    crate::client::gadget_look::assign_default_gadget_look(wm, h, c, font, default_visual);
}

/// createGadget's combo box branch after `gogoGadgetComboBox`: the file's part draw data to the drop-down button, the
/// edit box, the list box, its up and down buttons, its slider and the slider's thumb (all nine of each state).
pub fn assign_part_draw_data(wm: &mut GameWindowManager, w: WinId) {
    let parts = wm.tree.windows[w].parts.clone();
    let Some(c) = combo_rt(wm, w).cloned() else { return };
    let l = c.list_box.and_then(|l| listbox_rt(wm, l).cloned());
    let slider = l.as_ref().and_then(|l| l.slider);
    let thumb = slider.and_then(|s| wm.tree.windows[s].children.first().copied());
    for (part, d) in parts {
        let target = match part {
            GadgetPart::ComboBoxDropDownButton => c.drop_down_button,
            GadgetPart::ComboBoxEditBox => c.edit_box,
            GadgetPart::ComboBoxListBox => c.list_box,
            GadgetPart::ListBoxUpButton => l.as_ref().and_then(|l| l.up_button),
            GadgetPart::ListBoxDownButton => l.as_ref().and_then(|l| l.down_button),
            GadgetPart::ListBoxSlider => slider,
            GadgetPart::SliderThumb => thumb,
        };
        if let Some(t) = target {
            let i = &mut wm.tree.windows[t].inst;
            i.enabled_draw_data = d.enabled.clone();
            i.disabled_draw_data = d.disabled.clone();
            i.hilite_draw_data = d.hilite.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::tests::Rec;

    fn u(t: &str) -> Vec<u16> {
        t.encode_utf16().collect()
    }

    fn sys(h: &Rec) -> Vec<(WinId, u32, u32)> {
        h.sys.iter().map(|(_, w, m, d)| (*w, *m, *d)).collect()
    }

    fn combo(wm: &mut GameWindowManager, h: &mut Rec, editable: bool) -> (WinId, WinId) {
        let owner = wm.win_create(h, None, win_status::ENABLED, 0, 0, 400, 300, SystemFunc::Host(b"Owner".to_vec()), None);
        let inst = WinInstanceData { style: gws::COMBO_BOX | gws::MOUSE_TRACK, ..WinInstanceData::new() };
        let t = combo_template(&ComboBoxData { is_editable: editable, max_display: 3, max_chars: 300, ascii_only: false, letters_and_numbers_only: false });
        let c = gogo_gadget_combo_box(wm, h, Some(owner), win_status::ENABLED, 10, 20, 120, 20, inst, t, false).expect("a combo box");
        (owner, c)
    }

    /// #72: creation. Predicted from gogoGadgetComboBox (combo box 120x20 at (10,20), not editable, the default font
    /// 14 high): the children, newest first, are the list box, the edit box and the drop-down button; the button is
    /// 21x20 at (99,0); the edit box 99x20 at (0,0), `NO_INPUT`, its text `Entry` drawn from its start, `maxTextLen`
    /// 300 (unclamped); the list box 120x20 at (0,20), hidden, `ABOVE | ONE_LINE`, styled `SCROLL_LISTBOX |
    /// HORZ_SLIDER | MOUSE_TRACK`, owned by the combo box, with audio feedback, its length 10.
    #[test]
    fn a_combo_box_is_made_as_upstream_makes_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (_owner, c) = combo(&mut wm, &mut h, false);
        let d = combo_rt(&wm, c).expect("data").clone();
        let (dd, e, l) = (d.drop_down_button.expect("button"), d.edit_box.expect("edit"), d.list_box.expect("list"));
        assert_eq!(wm.win(c).children, vec![l, e, dd]);
        assert_eq!((wm.win(dd).region, wm.win(dd).size), ((99, 0, 120, 20), (21, 20)));
        assert_eq!((wm.win(e).region, wm.win(e).size), ((0, 0, 99, 20), (99, 20)));
        assert_ne!(wm.win(e).status & win_status::NO_INPUT, 0);
        let ed = te::entry_rt(&wm, e).expect("entry");
        assert_eq!((ed.text.text.clone(), ed.char_pos, ed.max_text_len, ed.draw_text_from_start), (u("Entry"), 5, 300, true));
        assert_eq!((wm.win(l).region, wm.win(l).size), ((0, 20, 120, 40), (120, 20)));
        assert_eq!(wm.win(l).status & (win_status::HIDDEN | win_status::ABOVE | win_status::ONE_LINE), win_status::HIDDEN | win_status::ABOVE | win_status::ONE_LINE);
        assert_eq!(wm.win(l).inst.style, gws::SCROLL_LISTBOX | gws::HORZ_SLIDER | gws::MOUSE_TRACK);
        assert_eq!((wm.ext[l].owner, wm.ext[e].owner, wm.ext[dd].owner), (Some(c), Some(c), Some(c)));
        let ld = listbox_rt(&wm, l).expect("list");
        assert_eq!((ld.list_length, ld.audio_feedback, ld.scroll_bar), (10, true, true));
        assert_eq!(get_text(&mut wm, &mut h, c), u("Entry"));
        // editable cannot clear NO_INPUT
        set_is_editable(&mut wm, c, true);
        assert_ne!(wm.win(e).status & win_status::NO_INPUT, 0);
        assert!(combo_rt(&wm, c).expect("data").is_editable);
    }

    /// #72: adding, opening and choosing. Predicted: two entries (`A`, `B`) make `entryCount` 2, rows 0 and 1; the
    /// list (below the edit box at y 20) is `14 * 2 + 4 + 4` = 36 high and 136 wide (16 wider: both fit `maxDisplay`
    /// 3), its scroll bar hidden. The drop-down button's `GBM_SELECTED` opens it: the combo box is the lone window,
    /// grows to 20 + 36 = 56 and the list is 120 wide. Selecting row 1 hides it (the combo box back to 20 high), puts
    /// `B` in the edit box and tells the owner `GCM_SELECTED`. A fourth entry past `maxDisplay` shows the scroll bar
    /// and sizes the list for 3 rows (3 * 14 + 6 + 4 = 52).
    #[test]
    fn a_combo_box_opens_and_chooses_as_upstream_does() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (owner, c) = combo(&mut wm, &mut h, false);
        let d = combo_rt(&wm, c).expect("data").clone();
        let (dd, e, l) = (d.drop_down_button.expect("button"), d.edit_box.expect("edit"), d.list_box.expect("list"));
        assert_eq!(add_entry(&mut wm, &mut h, c, &u("A"), 5), 0);
        assert_eq!(add_entry(&mut wm, &mut h, c, &u("B"), 6), 1);
        assert_eq!(get_length(&wm, c), 2);
        assert_eq!((wm.win(l).region.1, wm.win(l).size), (20, (136, 36)));
        let ld = listbox_rt(&wm, l).expect("list").clone();
        assert!(hidden(&wm, ld.slider.expect("slider")));
        h.sys.clear();
        wm.win_send_system_msg(&mut h, Some(c), ggm::GBM_SELECTED, win_data(Some(dd)), 0);
        assert_eq!(wm.lone_window, Some(c));
        assert!(!hidden(&wm, l));
        assert_eq!(wm.win(c).size, (120, 56));
        assert_eq!(wm.win(l).size, (120, 36));
        lbx::set_selected(&mut wm, &mut h, l, &[1]);
        assert!(hidden(&wm, l));
        assert_eq!(wm.win(c).size, (120, 20));
        assert_eq!(te::get_text(&mut wm, &mut h, Some(e)), u("B"));
        assert_eq!(wm.win(e).inst.enabled_text.color, 6);
        assert_eq!(sys(&h).last(), Some(&(owner, gcm::SELECTED, win_data(Some(c)))));
        assert_eq!(get_selected_pos(&mut wm, &mut h, c, -7), 1);
        set_item_data(&mut wm, &mut h, c, 1, 99);
        assert_eq!(get_item_data(&mut wm, &mut h, c, 1), 99);
        add_entry(&mut wm, &mut h, c, &u("C"), 0);
        add_entry(&mut wm, &mut h, c, &u("D"), 0);
        assert_eq!(wm.win(l).size, (120, 52));
        assert!(!hidden(&wm, ld.slider.expect("slider")));
        // a left release on the combo box opens it with a click; GGM_CLOSE hides it
        h.audio.clear();
        wm.win_send_input_msg(&mut h, Some(c), gwm::LEFT_UP, 0, 0);
        assert_eq!(h.audio, vec![b"GUIClick".to_vec()]);
        assert!(!hidden(&wm, l));
        hide_list(&mut wm, &mut h, c);
        assert!(hidden(&wm, l));
        // reset: the edit text empty, no entries
        reset(&mut wm, &mut h, c);
        assert_eq!((get_text(&mut wm, &mut h, c), get_length(&wm, c)), (Vec::new(), 0));
        set_text(&mut wm, &mut h, c, &u("Z"));
        assert_eq!(get_text(&mut wm, &mut h, c), u("Z"));
    }

    /// #72: the edit box's messages and the keys. Predicted: typing in an editable combo box's edit box sends the
    /// combo box `GEM_UPDATE_TEXT`, which tells the owner `GCM_UPDATE_TEXT`; return sends `GEM_EDIT_DONE` and the
    /// owner gets `GCM_SELECTED`; a letter key given to the combo box goes to the edit box (eaten: handled); the focus
    /// hilites the combo box and selects the edit box; destroying the combo box frees its data and clears the lone
    /// window.
    #[test]
    fn a_combo_box_passes_its_edit_box_on_as_upstream_does() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let (owner, c) = combo(&mut wm, &mut h, true);
        let e = get_edit_box(&wm, c).expect("edit");
        h.sys.clear();
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(b'x'), 0);
        assert_eq!(sys(&h), vec![(owner, gcm::UPDATE_TEXT, win_data(Some(c)))]);
        wm.win_send_input_msg(&mut h, Some(e), gwm::IME_CHAR, u32::from(te::VK_RETURN), 0);
        assert_eq!(sys(&h)[1..], [(owner, gcm::SELECTED, win_data(Some(c)))]);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(c), gwm::CHAR, 0x1E, key::STATE_DOWN), Handled);
        wm.win_set_focus(&mut h, Some(c));
        assert_eq!(wm.keyboard_focus, Some(c));
        assert_ne!(wm.win(c).inst.state & WIN_STATE_HILITED, 0);
        assert_ne!(wm.win(e).inst.state & crate::client::window_manager::WIN_STATE_SELECTED, 0);
        wm.win_send_system_msg(&mut h, Some(c), ggm::GBM_SELECTED, win_data(get_drop_down_button(&wm, c)), 0);
        assert_eq!(wm.lone_window, Some(c));
        set_ascii_only(&mut wm, c, true);
        set_letters_and_numbers_only(&mut wm, c, true);
        set_max_display(&mut wm, c, 9);
        let ed = te::entry_rt(&wm, e).expect("entry");
        assert!(ed.ascii_only && ed.alpha_numerical_only);
        assert_eq!(combo_rt(&wm, c).expect("data").max_display, 9);
        set_text_colors(&mut wm, c, 2, 11, 12);
        assert_eq!(wm.win(e).inst.hilite_text.color, 11);
        set_font(&mut wm, c, Some(GameFont { name: b"F".to_vec(), point_size: 9, bold: false }));
        assert_eq!(wm.win(get_list_box(&wm, c).expect("list")).inst.font.as_ref().map(|f| f.point_size), Some(9));
        wm.win_destroy(Some(c));
        wm.update(&mut h);
        assert_eq!(wm.lone_window, None);
        assert!(combo_rt(&wm, c).is_none());
    }
}
