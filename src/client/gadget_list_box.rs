// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetListBox.cpp (doAudioFeedback,
//     getListboxEntryBasedOnCoord, GadgetListBoxGetEntryBasedOnXY, getListboxTopEntry, getListboxBottomEntry,
//     removeSelection, adjustDisplay, computeTotalHeight, addImageEntry, moveRowsDown, addEntry, GadgetListBoxInput,
//     GadgetListBoxMultiInput, GadgetListBoxSystem, GadgetListBoxGetText, GadgetListBoxGetTextAndColor,
//     GadgetListBoxAddEntryText, GadgetListBoxAddEntryImage, GadgetListBoxSetFont, GadgetListboxCreateScrollbar,
//     GadgetListBoxAddMultiSelect, GadgetListBoxRemoveMultiSelect, GadgetListBoxSetListLength,
//     GadgetListBoxGetListLength, GadgetListBoxGetNumEntries, GadgetListBoxGetSelected, GadgetListBoxSetSelected,
//     GadgetListBoxReset, GadgetListBoxSetItemData, GadgetListBoxGetItemData, GadgetListBoxGetBottomVisibleEntry,
//     GadgetListBoxIsFull, GadgetListBoxSetBottomVisibleEntry, GadgetListBoxGetTopVisibleEntry,
//     GadgetListBoxSetTopVisibleEntry, GadgetListBoxSetAudioFeedback, GadgetListBoxGetNumColumns,
//     GadgetListBoxGetColumnWidth)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoGadgetListBox)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (createGadget's list box branch: the
//     LISTBOX*DRAWDATA and SLIDERTHUMB*DRAWDATA given to the inner windows)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Gadget.h (ListEntryCell, ListEntryRow, ListboxData, the offsets)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The list box (GitHub #70, a part of #34): rows of cells (text or image) in columns, a scroll buffer, single or
//! multiple selection, and a scroll bar made of an up button, a down button and a vertical slider.
//!
//! # As upstream reads it
//!
//! `ListboxData` holds `listLength` rows (`listData`), of which `endPos` are used; each used row has an array of
//! `columns` cells, allocated when the row is first written. A row's `height` is the tallest cell's (text: the
//! display string's height, word wrapped at the column's width less 7, or the font's height with `ONE_LINE`; image:
//! its height + 1, or the font's height), and its `listHeight` the running total of `height + 1` down to its bottom.
//! `displayPos` is the pixel offset of the view and `displayHeight` the view's height.
//!
//! - **Adding** (`GLM_ADD_ENTRY`, through `GadgetListBoxAddEntryText` / `Image`): at the end (`row` -1, or any row at
//!   or past `insertPos`), or at a row, pushing the rows below down unless overwriting. A full list drops its top row
//!   first (`GLM_SCROLL_BUFFER`) with `autoPurge`, else refuses. `autoScroll` scrolls until the new row shows; the
//!   selection at the added row is dropped.
//! - **Scrolling** (`adjustDisplay(n)`): to the top of the row `n` rows from the top row, and the slider follows
//!   (`maxVal = totalHeight - (displayHeight - 2) + 1`, at least 0; the slider's `numTicks` recomputed; `GSM_SET_SLIDER`
//!   at `maxVal - displayPos`). The up and down buttons (`GBM_SELECTED` or a drag on them) scroll by a row, the wheel
//!   too, and the slider's `GSM_SLIDER_TRACK` sets `displayPos` from its position.
//! - **Selecting**: a left release picks the row under the mouse (or -1), a second release on the same row clears it
//!   unless `forceSelect`, and two releases inside the double-click time send `GLM_DOUBLE_CLICKED`; every release sends
//!   `GLM_SELECTED` with the selection. Up and down keys move it, scrolling it into view; a printable key jumps to the
//!   next row whose text starts with it. A multi-select list box toggles the row in `selections`, an array ended by
//!   -1.
//! - **The scroll bar** (`GadgetListboxCreateScrollbar`): the up button (21x22) at the top right, the down button at
//!   the bottom right, the slider between them; their status is the list box's without `BORDER`, `HIDDEN` and
//!   `NO_INPUT`, with `IMAGE`, `ACTIVE` and `ENABLED`; they track the mouse if the list box does. The file's
//!   `LISTBOX*UPBUTTONDRAWDATA`, `...DOWNBUTTON...`, `...SLIDER...` and `SLIDERTHUMB...` then replace all nine draw
//!   data of each (createGadget).
//!
//! # Upstream quirks that are kept
//!
//! - A row's `height` is a `Byte` (signed `char`): a row taller than 127 pixels wraps negative; `displayPos`,
//!   `displayHeight`, `endPos`, `insertPos` and `listLength` are `Short`s and truncate.
//! - `gogoGadgetListBox` gives one column `width - (slider width + 2)`; `GGM_RESIZED` gives it `width - slider width`.
//! - A script's list box gets its title text after creation: `displayHeight` and the scroll bar were made as if it had
//!   none, while the hit test and the draw step past the title.
//! - `GLM_ADD_ENTRY` on a multi-select list box clears every selection but the ones equal to 0, stopping at the first it
//!   clears (`if( (row = list->selections[i]) != 0 ) list->selections[i] = -1;`).
//! - `GLM_SCROLL_BUFFER` on a multi-select list box removes the selections **below** the scroll and keeps (shifted to
//!   negative or zero) those it scrolled off; on a single-select one it moves `selectPos` up even past 0.
//! - `GadgetListBoxSetListLength` shrinking the list sets `endPos` to the new length even when fewer rows were used:
//!   the rows in between have no cells.
//! - `getListboxTopEntry` returns 0 past the end; the up key on a selection out of view sets `displayPos` itself before
//!   `adjustDisplay`.
//! - The double click compares `timeGetTime()` with the last release plus the system's double-click time (500 ms) and
//!   sends `GLM_DOUBLE_CLICKED` before `GLM_SELECTED`.
//! - `GLM_DEL_ENTRY` leaves the deleted row's cell pointer in the row past the new end (see below).
//!
//! # Upstream undefined behaviour, and what the translation does
//!
//! The translation keeps pointers as upstream has them: a row's cells are an array in an arena ([`ListboxRt::arrays`]),
//! and a row holds its index, so rows that alias one array (after `GLM_DEL_ENTRY`'s copy) and arrays already freed
//! are what they are upstream. Where upstream is undefined the manager's [`GameWindowManager::undefined`] is set and
//! the message stops (the differential skips the rest of the session):
//!
//! - `GLM_DEL_ENTRY` writes `cells[columns]`, one past the row's array (`for( i = 0; i <= list->columns; i++ )`), on
//!   **every** row that has cells; it also leaves the old last row's pointer behind the new end, so a later
//!   `GLM_DEL_ALL`, `GWM_DESTROY` or list length change frees it twice, and an add there reads freed memory;
//! - an index outside `listData` (a selection array full to its end, an `mData1` past the list, `getListboxBottomEntry`
//!   on an empty list reading `listData[-1]`, `GadgetListBoxIsFull` on an empty list), a row without cells read
//!   (`GLM_GET_TEXT`, the key search with more than one column), a cell array read or freed after it was freed, a text
//!   added into an image cell (its image is used as a `DisplayString`), no `listData` (no columns), a negative list
//!   length, no scroll bar slider where `GSM_SLIDER_TRACK` or the border reads it, no columns at creation (upstream
//!   returns `NULL` and `createGadget` dereferences it).
//!
//! What C++ leaves undefined but both of upstream's x86 toolchains define is kept, not skipped: `memcpy` of
//! overlapping rows (MSVC's `memcpy` is `memmove`; the translation moves), `delete` of a `new[]` array (trivially
//! destructible cells: the same as `delete[]`), and a float to `Int` conversion out of range (`0x80000000`, see
//! [`crate::client::gadget_slider`]).
//!
//! # Where this differs from upstream
//!
//! A cell's `DisplayString` is its text, word wrap and font ([`DisplayString`]); its size is the host's
//! ([`GuiHost::text_size`]). Pointers passed in messages (`AddMessageStruct`, `ICoord2D`, `TextAndColor`, the
//! selection list, `RightClickStruct`) are the manager's message fields ([`ListMsg`]). An image is its name. The
//! cells' user data is an integer.
//!
//! # Representation
//!
//! [`ListboxRt`] in the window's [`UserData::Listbox`].

use crate::client::gadget_slider::{slider_rt, SliderRt};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, DrawFunc, GameWindowManager, GuiHost, InputFunc, MsgHandled, SystemFunc, UserData, WindowMsgData, WIN_STATE_HILITED};
use crate::client::window_script::{gws, win_status, GameFont, GadgetPart, ListboxData, WinId, WinInstanceData};

use MsgHandled::{Handled, Ignored};

/// `LISTBOX_TEXT`.
pub const LISTBOX_TEXT: i32 = 1;
/// `LISTBOX_IMAGE`.
pub const LISTBOX_IMAGE: i32 = 2;
/// `TEXT_X_OFFSET`.
pub const TEXT_X_OFFSET: i32 = 5;
/// `TEXT_WIDTH_OFFSET`.
pub const TEXT_WIDTH_OFFSET: i32 = 7;
/// `TOTAL_OUTLINE_HEIGHT`.
pub const TOTAL_OUTLINE_HEIGHT: i32 = 2;
/// `GP_DONT_UPDATE`.
pub const GP_DONT_UPDATE: u32 = 1;
/// Where a loop that upstream would run forever is cut (the translation records it as undefined).
const LOOP_LIMIT: i32 = 100_000;
/// `GetDoubleClickTime()`: Windows' default, 500 ms.
pub const DOUBLE_CLICK_TIME: u32 = 500;
/// `Keyboard::MAX_KEY_STATES`.
pub const MAX_KEY_STATES: i32 = 3;

/// The list box's own messages (Gadget.h, after `GLM_ADD_ENTRY`).
pub mod glm {
    pub const ADD_ENTRY: u32 = 16399;
    pub const DEL_ENTRY: u32 = 16400;
    pub const DEL_ALL: u32 = 16401;
    pub const SELECTED: u32 = 16402;
    pub const DOUBLE_CLICKED: u32 = 16403;
    pub const RIGHT_CLICKED: u32 = 16404;
    pub const SET_SELECTION: u32 = 16405;
    pub const GET_SELECTION: u32 = 16406;
    pub const TOGGLE_MULTI_SELECTION: u32 = 16407;
    pub const GET_TEXT: u32 = 16408;
    pub const SET_UP_BUTTON: u32 = 16409;
    pub const SET_DOWN_BUTTON: u32 = 16410;
    pub const SET_SLIDER: u32 = 16411;
    pub const SCROLL_BUFFER: u32 = 16412;
    pub const UPDATE_DISPLAY: u32 = 16413;
    pub const GET_ITEM_DATA: u32 = 16414;
    pub const SET_ITEM_DATA: u32 = 16415;
}

/// A `DisplayString`: its text, word wrap width (0: none) and font.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayString {
    pub text: Vec<u16>,
    pub word_wrap: i32,
    pub font: Option<GameFont>,
}

/// What a cell's `data` points to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CellData {
    /// `NULL`.
    #[default]
    None,
    Text(DisplayString),
    /// An image, by name.
    Image(Vec<u8>),
}

/// `ListEntryCell`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListEntryCell {
    pub cell_type: i32,
    pub color: i32,
    pub data: CellData,
    pub user_data: u32,
    pub width: i32,
    pub height: i32,
}

/// `ListEntryRow`: `cell` is the index of its array in [`ListboxRt::arrays`] (`None`: `NULL`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListEntryRow {
    pub list_height: i32,
    /// `Byte`.
    pub height: i8,
    pub cell: Option<usize>,
}

/// `ListboxData` as the list box keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListboxRt {
    pub list_length: i16,
    pub columns: i16,
    /// `columnWidthPercentage` (`None`: `NULL`).
    pub column_width_percentage: Option<Vec<i32>>,
    pub auto_scroll: bool,
    pub auto_purge: bool,
    pub scroll_bar: bool,
    pub multi_select: bool,
    pub force_select: bool,
    pub scroll_if_at_end: bool,
    pub audio_feedback: bool,
    /// `columnWidth` (`None`: `NULL`).
    pub column_width: Option<Vec<i32>>,
    /// `listData` (`None`: `NULL`).
    pub list_data: Option<Vec<ListEntryRow>>,
    pub up_button: Option<WinId>,
    pub down_button: Option<WinId>,
    pub slider: Option<WinId>,
    pub total_height: i32,
    pub end_pos: i16,
    pub insert_pos: i16,
    pub select_pos: i32,
    /// `selections` (`None`: `NULL`), `listLength` long.
    pub selections: Option<Vec<i32>>,
    pub display_height: i16,
    pub double_click_time: u32,
    pub display_pos: i16,
    /// The cell arrays rows point to (`None`: freed).
    pub arrays: Vec<Option<Vec<ListEntryCell>>>,
}

/// `AddMessageStruct`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddMessage {
    pub row: i32,
    pub column: i32,
    /// The text (`LISTBOX_TEXT`) or the image (`LISTBOX_IMAGE`, `None`: `NULL`).
    pub text: Vec<u16>,
    pub image: Option<Vec<u8>>,
    pub entry_type: i32,
    pub overwrite: bool,
    pub width: i32,
    pub height: i32,
}

/// The data the list box's messages point to (upstream passes pointers).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListMsg {
    /// `GLM_ADD_ENTRY`'s `AddMessageStruct`.
    pub add: AddMessage,
    /// `GLM_GET_TEXT`'s, `GLM_SET_ITEM_DATA`'s and `GLM_GET_ITEM_DATA`'s `ICoord2D` (column, row).
    pub pos: (i32, i32),
    /// `GLM_GET_TEXT`'s `TextAndColor` answer.
    pub text: Vec<u16>,
    pub color: i32,
    /// `GLM_SET_SELECTION`'s list.
    pub select: Vec<i32>,
    /// `GLM_GET_SELECTION`'s answer: the selection, or the multi-select array.
    pub selection: Vec<i32>,
    /// `GLM_GET_ITEM_DATA`'s answer.
    pub item_data: u32,
    /// `GLM_RIGHT_CLICKED`'s `RightClickStruct` (pos, mouseX, mouseY).
    pub right_click: (i32, i32, i32),
    /// `GLM_ADD_ENTRY`'s return value (the message's result cast to `Int`).
    pub added: i32,
}

/// The list box's data, if the window has some.
pub fn listbox_rt(wm: &GameWindowManager, w: WinId) -> Option<&ListboxRt> {
    match &wm.ext[w].user {
        UserData::Listbox(l) => Some(l),
        _ => None,
    }
}

fn lb_opt(wm: &mut GameWindowManager, w: WinId) -> Option<&mut ListboxRt> {
    match &mut wm.ext[w].user {
        UserData::Listbox(l) => Some(l),
        _ => None,
    }
}

/// An undefined operation upstream: recorded, and the caller stops.
#[derive(Debug)]
pub struct Ub;

type R<T> = Result<T, Ub>;

fn ub<T>(wm: &mut GameWindowManager, what: &'static str) -> R<T> {
    wm.set_undefined(what);
    Err(Ub)
}

fn lb(wm: &mut GameWindowManager, w: WinId) -> R<&mut ListboxRt> {
    if !matches!(wm.ext[w].user, UserData::Listbox(_)) {
        wm.set_undefined("list box: no ListboxData (NULL user data)");
        return Err(Ub);
    }
    Ok(lb_opt(wm, w).expect("checked"))
}

fn lbr(wm: &mut GameWindowManager, w: WinId) -> R<ListboxRt> {
    lb(wm, w).map(|l| l.clone())
}

/// `listData[i]` (a copy).
fn row(wm: &mut GameWindowManager, w: WinId, i: i32) -> R<ListEntryRow> {
    let r = lb(wm, w)?.list_data.as_ref().and_then(|d| usize::try_from(i).ok().and_then(|i| d.get(i)).copied());
    match r {
        Some(r) => Ok(r),
        None => ub(wm, "list box: listData index out of range (or listData NULL)"),
    }
}

fn row_mut(wm: &mut GameWindowManager, w: WinId, i: i32) -> R<&mut ListEntryRow> {
    let ok = lb(wm, w)?.list_data.as_ref().is_some_and(|d| i >= 0 && (i as usize) < d.len());
    if !ok {
        return ub(wm, "list box: listData index out of range (or listData NULL)");
    }
    Ok(&mut lb(wm, w)?.list_data.as_mut().expect("checked")[i as usize])
}

/// The cell array a row points to, live.
fn cells(wm: &mut GameWindowManager, w: WinId, a: usize) -> R<&mut Vec<ListEntryCell>> {
    if lb(wm, w)?.arrays.get(a).is_none_or(|x| x.is_none()) {
        return ub(wm, "list box: a cell array used after it was freed");
    }
    Ok(lb(wm, w)?.arrays[a].as_mut().expect("checked"))
}

/// `cells[j]` of the array `a`.
fn cell(wm: &mut GameWindowManager, w: WinId, a: usize, j: i32) -> R<&mut ListEntryCell> {
    let n = cells(wm, w, a)?.len();
    if j < 0 || j as usize >= n {
        return ub(wm, "list box: a cell index past its row's array");
    }
    Ok(&mut cells(wm, w, a)?[j as usize])
}

/// `delete[]` of a row's array: twice is undefined.
fn free_array(wm: &mut GameWindowManager, w: WinId, a: usize) -> R<()> {
    cells(wm, w, a)?;
    lb(wm, w)?.arrays[a] = None;
    Ok(())
}

/// `NEW ListEntryCell[columns]`, zeroed.
fn new_array(wm: &mut GameWindowManager, w: WinId) -> R<usize> {
    let l = lb(wm, w)?;
    let n = usize::try_from(l.columns).unwrap_or(0);
    l.arrays.push(Some(vec![ListEntryCell::default(); n]));
    Ok(l.arrays.len() - 1)
}

/// `list->selections[i]`.
fn sel(wm: &mut GameWindowManager, w: WinId, i: i32) -> R<i32> {
    let v = lb(wm, w)?.selections.as_ref().and_then(|s| usize::try_from(i).ok().and_then(|i| s.get(i)).copied());
    match v {
        Some(v) => Ok(v),
        None => ub(wm, "list box: selections index past the array (or selections NULL)"),
    }
}

fn set_sel(wm: &mut GameWindowManager, w: WinId, i: i32, v: i32) -> R<()> {
    let ok = lb(wm, w)?.selections.as_ref().is_some_and(|s| i >= 0 && (i as usize) < s.len());
    if !ok {
        return ub(wm, "list box: selections index past the array (or selections NULL)");
    }
    lb(wm, w)?.selections.as_mut().expect("checked")[i as usize] = v;
    Ok(())
}

/// `memset(selections, -1, listLength * sizeof(Int))`.
fn clear_sels(wm: &mut GameWindowManager, w: WinId) -> R<()> {
    let l = lb(wm, w)?;
    let n = usize::try_from(l.list_length).unwrap_or(0);
    match l.selections.as_mut() {
        Some(s) if s.len() >= n => {
            s[..n].iter_mut().for_each(|x| *x = -1);
            Ok(())
        }
        _ => ub(wm, "list box: selections NULL or shorter than listLength"),
    }
}

fn column_width(wm: &mut GameWindowManager, w: WinId, j: i32) -> R<i32> {
    let v = lb(wm, w)?.column_width.as_ref().and_then(|c| usize::try_from(j).ok().and_then(|j| c.get(j)).copied());
    match v {
        Some(v) => Ok(v),
        None => ub(wm, "list box: columnWidth index out of range (or columnWidth NULL)"),
    }
}

/// `winFontHeight(font)`.
fn font_height<H: GuiHost>(wm: &mut GameWindowManager, h: &H, font: Option<&GameFont>) -> R<i32> {
    match font {
        Some(f) => Ok(h.font_height(Some(f))),
        None => ub(wm, "winFontHeight(NULL)"),
    }
}

/// The title's height: `winFontHeight(font) + 1` when the window has text.
fn title_adjust<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId) -> R<i32> {
    if wm.win_get_text(w).is_empty() {
        return Ok(0);
    }
    let f = wm.tree.windows[w].inst.font.clone();
    Ok(font_height(wm, h, f.as_ref())?.wrapping_add(1))
}

/// `doAudioFeedback`.
fn do_audio_feedback<H: GuiHost>(wm: &GameWindowManager, h: &mut H, w: WinId) {
    if listbox_rt(wm, w).is_some_and(|l| l.audio_feedback) {
        h.add_audio_event(b"GUIComboBoxClick");
    }
}

/// The row under a screen `y` (the loop shared by the hit tests): `(-1 or -2, i)`: -1 when below the shown rows or
/// past the end; else -2 and the row `i`.
fn row_at<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, mousey: i32) -> R<(i32, i32)> {
    let (_, mut y) = wm.tree.screen_position(w);
    y = y.wrapping_add(title_adjust(wm, h, w)?);
    let mut i = 0i32;
    loop {
        let l = lbr(wm, w)?;
        if i > 0 && row(wm, w, i - 1)?.list_height > i32::from(l.display_pos) + i32::from(l.display_height) {
            return Ok((-1, i));
        }
        if i == i32::from(l.end_pos) {
            return Ok((-1, i));
        }
        if row(wm, w, i)?.list_height > mousey.wrapping_sub(y).wrapping_add(i32::from(l.display_pos)) {
            return Ok((-2, i));
        }
        i += 1;
    }
}

/// `getListboxEntryBasedOnCoord` / `GadgetListBoxGetEntryBasedOnXY`: (row, column), -1 for none.
pub fn get_entry_based_on_xy<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, x: i32, y: i32) -> Option<(i32, i32)> {
    let r: R<(i32, i32)> = (|| {
        let (winx, _) = wm.tree.screen_position(w);
        let (pos, i) = row_at(wm, h, w, y)?;
        let mut column = -1;
        let mut out = pos;
        if pos == -2 {
            out = i;
            let mut total = 0i32;
            let cols = lbr(wm, w)?.columns;
            for c in 0..i32::from(cols) {
                total = total.wrapping_add(column_width(wm, w, c)?);
                if x.wrapping_sub(winx) < total {
                    column = c;
                    break;
                }
            }
        }
        Ok((out, column))
    })();
    r.ok()
}

/// `getListboxTopEntry`.
fn top_entry(wm: &mut GameWindowManager, w: WinId) -> R<i32> {
    let mut e = 0i32;
    loop {
        let l = lbr(wm, w)?;
        if row(wm, w, e)?.list_height > i32::from(l.display_pos) {
            return Ok(e);
        }
        if e >= i32::from(l.end_pos) {
            return Ok(0);
        }
        e += 1;
    }
}

/// `getListboxBottomEntry`.
fn bottom_entry(wm: &mut GameWindowManager, w: WinId) -> R<i32> {
    let l = lbr(wm, w)?;
    let bottom = i32::from(l.display_pos) + i32::from(l.display_height);
    let mut e = i32::from(l.end_pos) - 1;
    loop {
        if e < 0 {
            return ub(wm, "getListboxBottomEntry: an empty list reads listData[-1]");
        }
        let lh = row(wm, w, e)?.list_height;
        if lh == bottom {
            return Ok(e);
        }
        if lh < bottom && e != i32::from(l.end_pos) - 1 {
            return Ok(e + 1);
        }
        if lh < bottom {
            return Ok(e);
        }
        if e < 0 {
            return Ok(0);
        }
        e -= 1;
    }
}

/// `removeSelection(list, i)`: `memcpy(&sel[i], &sel[i+1], (listLength - i) * sizeof(Int))`, then -1 at the end.
/// The copy reads one `Int` past the array, into the last slot, which the -1 then overwrites: the value read never
/// matters, so this over-read is kept, not treated as undefined.
fn remove_selection(wm: &mut GameWindowManager, w: WinId, i: i32) -> R<()> {
    let n = i32::from(lbr(wm, w)?.list_length);
    let len = lbr(wm, w)?.selections.as_ref().map_or(0, |s| s.len() as i32);
    if i < 0 || i >= n || len < n {
        return ub(wm, "removeSelection: outside the selections array");
    }
    let s = lb(wm, w)?.selections.as_mut().expect("checked");
    for k in i..n - 1 {
        s[k as usize] = s[k as usize + 1];
    }
    s[(n - 1) as usize] = -1;
    Ok(())
}

/// `adjustDisplay(window, adjustment, updateSlider)`.
fn adjust_display<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, adjustment: i32, update_slider: bool) -> R<()> {
    let mut entry = top_entry(wm, w)?.wrapping_add(adjustment);
    let l = lbr(wm, w)?;
    if entry < 0 {
        entry = 0;
    } else if entry >= i32::from(l.end_pos) {
        entry = i32::from(l.end_pos) - 1;
    }
    if update_slider {
        let p = if entry > 0 { row(wm, w, entry - 1)?.list_height.wrapping_add(1) } else { 0 };
        lb(wm, w)?.display_pos = p as i16;
    }
    let l = lbr(wm, w)?;
    if let Some(s) = l.slider {
        let Some(mut sd) = slider_rt(wm, s) else { return ub(wm, "adjustDisplay: the slider has no SliderData") };
        let ssize = wm.tree.windows[s].size;
        // (plant 4701, differential builds only: the `+ 1` dropped)
        sd.max_val = l.total_height.wrapping_sub(i32::from(l.display_height) - TOTAL_OUTLINE_HEIGHT).wrapping_add(if crate::logic::modules::plant(4701) { 0 } else { 1 });
        if sd.max_val < 0 {
            sd.max_val = 0;
        }
        let Some(&c) = wm.tree.windows[s].children.first() else { return ub(wm, "adjustDisplay: the slider has no thumb") };
        let csize = wm.tree.windows[c].size;
        sd.num_ticks = ssize.1.wrapping_sub(csize.1) as f32 / sd.max_val as f32;
        wm.ext[s].user = UserData::Slider(sd);
        if update_slider {
            let dp = lbr(wm, w)?.display_pos;
            wm.win_send_system_msg(h, Some(s), ggm::GSM_SET_SLIDER, sd.max_val.wrapping_sub(i32::from(dp)) as u32, 0);
        }
    }
    Ok(())
}

/// `computeTotalHeight`.
fn compute_total_height<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) -> R<()> {
    let one_line = wm.tree.windows[w].status & win_status::ONE_LINE != 0;
    let font = wm.tree.windows[w].inst.font.clone();
    let mut height = 0i32;
    let end = i32::from(lbr(wm, w)?.end_pos);
    for i in 0..end {
        let r = row(wm, w, i)?;
        let Some(a) = r.cell else { continue };
        let mut temp = 0i32;
        let cols = i32::from(lbr(wm, w)?.columns);
        for j in 0..cols {
            let c = cell(wm, w, a, j)?.clone();
            let mut ch = 0;
            if c.cell_type == LISTBOX_TEXT {
                if one_line {
                    ch = font_height(wm, h, font.as_ref())?;
                } else if let CellData::Text(d) = &c.data {
                    ch = h.text_size(d.font.as_ref(), &d.text, (d.word_wrap > 0).then_some(d.word_wrap)).1;
                } else if c.data != CellData::None {
                    return ub(wm, "computeTotalHeight: an image used as a DisplayString");
                }
            } else if c.cell_type == LISTBOX_IMAGE {
                ch = if c.height > 0 { c.height.wrapping_add(1) } else { font_height(wm, h, font.as_ref())? };
            }
            if ch > temp {
                temp = ch;
            }
        }
        let rm = row_mut(wm, w, i)?;
        rm.height = temp as i8;
        height = height.wrapping_add(i32::from(rm.height) + 1);
        rm.list_height = height;
    }
    lb(wm, w)?.total_height = height;
    adjust_display(wm, h, w, 0, true)
}

/// `addImageEntry`.
#[allow(clippy::too_many_arguments)]
fn add_image_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, image: Option<Vec<u8>>, color: i32, mut r: i32, mut column: i32, w: WinId, width: i32, height: i32) -> R<i32> {
    let l = lbr(wm, w)?;
    if column >= i32::from(l.columns) || r >= i32::from(l.list_length) {
        return Ok(-1);
    }
    if r == -1 {
        let l = lb(wm, w)?;
        r = i32::from(l.insert_pos);
        l.insert_pos = l.insert_pos.wrapping_add(1);
        l.end_pos = l.end_pos.wrapping_add(1);
    }
    if column == -1 {
        column = 0;
    }
    let a = match row(wm, w, r)?.cell {
        Some(a) => a,
        None => {
            let a = new_array(wm, w)?;
            row_mut(wm, w, r)?.cell = Some(a);
            a
        }
    };
    let c = cell(wm, w, a, column)?;
    c.cell_type = LISTBOX_IMAGE;
    c.data = match image {
        Some(i) => CellData::Image(i),
        None => CellData::None,
    };
    c.color = color;
    c.height = height;
    c.width = width;
    compute_total_height(wm, h, w)?;
    Ok(r)
}

/// `moveRowsDown(list, startingRow)`: the rows from `startingRow` to `endPos` one down (the cell pointers copied).
fn move_rows_down(wm: &mut GameWindowManager, w: WinId, start: i32) -> R<()> {
    let l = lbr(wm, w)?;
    let end = i32::from(l.end_pos);
    let n = l.list_data.as_ref().map_or(0, |d| d.len() as i32);
    if start < 0 || end + 1 > n || start > end {
        return ub(wm, "moveRowsDown: copies past listData");
    }
    let d = lb(wm, w)?.list_data.as_mut().expect("checked");
    for k in (start..end).rev() {
        d[(k + 1) as usize] = d[k as usize];
    }
    let l = lb(wm, w)?;
    l.end_pos = l.end_pos.wrapping_add(1);
    l.insert_pos = l.end_pos;
    let rr = row_mut(wm, w, start)?;
    rr.cell = None;
    rr.height = 0;
    rr.list_height = 0;
    if lbr(wm, w)?.multi_select {
        let mut i = 0;
        loop {
            let s = sel(wm, w, i)?;
            if s < 0 {
                break;
            }
            if start <= s {
                set_sel(wm, w, i, s + 1)?;
            }
            i += 1;
        }
    } else {
        let l = lb(wm, w)?;
        if l.select_pos >= start {
            l.select_pos += 1;
        }
    }
    Ok(())
}

/// `addEntry`.
#[allow(clippy::too_many_arguments)]
fn add_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, text: &[u16], color: i32, mut r: i32, mut column: i32, w: WinId, overwrite: bool) -> R<i32> {
    let l = lbr(wm, w)?;
    if column >= i32::from(l.columns) || r >= i32::from(l.list_length) {
        return Ok(-1);
    }
    if r == -1 {
        let l = lb(wm, w)?;
        r = i32::from(l.insert_pos);
        l.insert_pos = l.insert_pos.wrapping_add(1);
        l.end_pos = l.end_pos.wrapping_add(1);
    }
    if column == -1 {
        column = 0;
    }
    // (plant 4702, differential builds only: the text offset one short)
    let width = column_width(wm, w, column)?.wrapping_sub(if crate::logic::modules::plant(4702) { TEXT_WIDTH_OFFSET - 1 } else { TEXT_WIDTH_OFFSET });
    let mut rows_added = 0;
    let existing = row(wm, w, r)?.cell;
    match existing {
        None => {
            let a = new_array(wm, w)?;
            row_mut(wm, w, r)?.cell = Some(a);
            rows_added = 1;
        }
        Some(_) if !overwrite => {
            move_rows_down(wm, w, r)?;
            let a = new_array(wm, w)?;
            row_mut(wm, w, r)?.cell = Some(a);
            rows_added = 1;
        }
        Some(_) => {}
    }
    let a = row(wm, w, r)?.cell.expect("set above");
    let one_line = wm.tree.windows[w].status & win_status::ONE_LINE != 0;
    let font = wm.tree.windows[w].inst.font.clone();
    {
        let c = cell(wm, w, a, column)?;
        c.cell_type = LISTBOX_TEXT;
        c.color = color;
        if matches!(c.data, CellData::Image(_)) {
            return ub(wm, "addEntry: text written into an image cell (the image used as a DisplayString)");
        }
        if c.data == CellData::None {
            c.data = CellData::Text(DisplayString::default());
        }
        let CellData::Text(d) = &mut c.data else { unreachable!("text") };
        if !one_line {
            d.word_wrap = width;
        }
        d.text = text.to_vec();
        d.font = font;
    }
    if overwrite {
        let rr = row(wm, w, r)?;
        let old_row_height = i32::from(rr.height);
        let mut old_total = rr.list_height;
        if old_total == 0 && r != 0 {
            old_total = row(wm, w, r - 1)?.list_height;
        }
        let CellData::Text(d) = cell(wm, w, a, column)?.data.clone() else { unreachable!("text") };
        let rh = h.text_size(d.font.as_ref(), &d.text, (d.word_wrap > 0).then_some(d.word_wrap)).1;
        if rh > old_row_height {
            let total = old_total.wrapping_add(rh - old_row_height);
            let rm = row_mut(wm, w, r)?;
            rm.height = rh as i8;
            rm.list_height = total.wrapping_add(rows_added);
            let l = lb(wm, w)?;
            l.total_height = l.total_height.wrapping_add(rh - old_row_height + rows_added);
            adjust_display(wm, h, w, 0, true)?;
        }
    } else {
        compute_total_height(wm, h, w)?;
    }
    Ok(r)
}

fn owner(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.ext[w].owner
}

fn set_hilite(wm: &mut GameWindowManager, w: WinId, on: bool) {
    if on {
        wm.tree.windows[w].inst.state |= WIN_STATE_HILITED;
    } else {
        wm.tree.windows[w].inst.state &= !WIN_STATE_HILITED;
    }
}

/// The input shared by both list box inputs: enter/leave, drag, left down.
fn common_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32) -> Option<MsgHandled> {
    let track = wm.tree.windows[w].inst.style & gws::MOUSE_TRACK != 0;
    let me = win_data(Some(w));
    match msg {
        gwm::MOUSE_ENTERING | gwm::MOUSE_LEAVING => {
            if track {
                let on = msg == gwm::MOUSE_ENTERING;
                set_hilite(wm, w, on);
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, if on { ggm::GBM_MOUSE_ENTERING } else { ggm::GBM_MOUSE_LEAVING }, me, 0);
            }
            Some(Handled)
        }
        gwm::LEFT_DRAG => {
            if track {
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, ggm::LEFT_DRAG, me, 0);
            }
            Some(Handled)
        }
        gwm::LEFT_DOWN => {
            do_audio_feedback(wm, h, w);
            Some(Handled)
        }
        _ => None,
    }
}

/// The right release of both inputs: the focus, the row under the mouse, `GLM_RIGHT_CLICKED`.
fn right_up<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, d1: WindowMsgData) -> R<()> {
    wm.win_set_focus(h, Some(w));
    let mousex = (d1 & 0xFFFF) as i32;
    let mousey = (d1 >> 16) as i32;
    let (p, i) = row_at(wm, h, w, mousey)?;
    let pos = if p == -2 { i } else { p };
    wm.list_msg.right_click = (pos, mousex, mousey);
    let o = owner(wm, w);
    wm.win_send_system_msg(h, o, glm::RIGHT_CLICKED, win_data(Some(w)), 0);
    Ok(())
}

/// `GadgetListBoxInput`.
pub fn list_box_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match list_box_input_r(wm, h, w, msg, d1, d2) {
        Ok(m) => m,
        Err(Ub) => Handled,
    }
}

fn list_box_input_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> R<MsgHandled> {
    if let Some(m) = common_input(wm, h, w, msg) {
        return Ok(m);
    }
    let me = win_data(Some(w));
    match msg {
        gwm::CHAR => match d1 as u8 {
            key::ENTER | key::SPACE => {
                if d2 & key::STATE_UP != 0 {
                    do_audio_feedback(wm, h, w);
                    let sp = lbr(wm, w)?.select_pos;
                    let o = owner(wm, w);
                    wm.win_send_system_msg(h, o, glm::DOUBLE_CLICKED, me, sp as u32);
                }
            }
            key::DOWN => {
                if d2 & key::STATE_DOWN != 0 {
                    let l = lbr(wm, w)?;
                    if l.select_pos == -1 {
                        lb(wm, w)?.select_pos = 0;
                        adjust_display(wm, h, w, 0, true)?;
                    } else if l.select_pos < i32::from(l.end_pos) - 1 {
                        lb(wm, w)?.select_pos += 1;
                        let mut guard = 0;
                        loop {
                            let l = lbr(wm, w)?;
                            let r = row(wm, w, l.select_pos)?;
                            let cell_bottom = r.list_height;
                            let cell_top = cell_bottom - i32::from(r.height);
                            let top = i32::from(l.display_pos);
                            let bottom = i32::from(l.display_pos) + i32::from(l.display_height) - 1;
                            if cell_top < top {
                                adjust_display(wm, h, w, -1, true)?;
                            } else if cell_bottom < bottom {
                                adjust_display(wm, h, w, 0, true)?;
                                break;
                            } else {
                                adjust_display(wm, h, w, 1, true)?;
                            }
                            guard += 1;
                            if guard > LOOP_LIMIT {
                                return ub(wm, "GadgetListBoxInput: KEY_DOWN's scroll loop never ends");
                            }
                        }
                    }
                    let sp = lbr(wm, w)?.select_pos;
                    let o = owner(wm, w);
                    wm.win_send_system_msg(h, o, glm::SELECTED, me, sp as u32);
                }
            }
            key::UP => {
                if d2 & key::STATE_DOWN != 0 {
                    let l = lbr(wm, w)?;
                    if l.select_pos == -1 {
                        lb(wm, w)?.select_pos = 0;
                        adjust_display(wm, h, w, 0, true)?;
                    } else if l.select_pos > 0 {
                        lb(wm, w)?.select_pos -= 1;
                        let mut guard = 0;
                        loop {
                            let l = lbr(wm, w)?;
                            let r = row(wm, w, l.select_pos)?;
                            if r.list_height - i32::from(r.height) < i32::from(l.display_pos) {
                                lb(wm, w)?.display_pos = r.list_height.wrapping_add(1) as i16;
                                adjust_display(wm, h, w, -1, true)?;
                            } else if r.list_height > i32::from(l.display_pos) + i32::from(l.display_height) {
                                lb(wm, w)?.display_pos = r.list_height.wrapping_sub(i32::from(l.display_height)) as i16;
                                adjust_display(wm, h, w, 1, true)?;
                            } else {
                                adjust_display(wm, h, w, 0, true)?;
                                break;
                            }
                            guard += 1;
                            if guard > LOOP_LIMIT {
                                return ub(wm, "GadgetListBoxInput: KEY_UP's scroll loop never ends");
                            }
                        }
                    }
                    let sp = lbr(wm, w)?.select_pos;
                    let o = owner(wm, w);
                    wm.win_send_system_msg(h, o, glm::SELECTED, me, sp as u32);
                }
            }
            key::RIGHT | key::TAB => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_next_tab(h, w);
                }
            }
            key::LEFT => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_prev_tab(h, w);
                }
            }
            k => {
                let mut found = false;
                if d2 & key::STATE_DOWN != 0 {
                    let l = lbr(wm, w)?;
                    let mut position = l.select_pos;
                    for _ in 0..i32::from(l.end_pos) {
                        position += 1;
                        let l = lbr(wm, w)?;
                        if position >= i32::from(l.end_pos) {
                            position = 0;
                        }
                        let r = row(wm, w, position)?;
                        // `cell = &listData[position].cell[j]` until a text cell with data
                        let mut found_cell: Option<ListEntryCell> = None;
                        match r.cell {
                            None => {
                                if l.columns > 1 {
                                    return ub(wm, "GadgetListBoxInput: the key search reads a row without cells");
                                }
                                // `&NULL[0]` is NULL: `if(!cell) continue`
                                if l.columns < 1 {
                                    // the cell stays NULL
                                }
                                continue;
                            }
                            Some(a) => {
                                for j in 0..i32::from(l.columns) {
                                    let c = cell(wm, w, a, j)?.clone();
                                    let stop = c.cell_type == LISTBOX_TEXT && c.data != CellData::None;
                                    found_cell = Some(c);
                                    if stop {
                                        break;
                                    }
                                }
                            }
                        }
                        let Some(c) = found_cell else { continue };
                        if c.cell_type != LISTBOX_TEXT {
                            continue;
                        }
                        let d = match &c.data {
                            CellData::None => continue,
                            CellData::Text(d) => d.clone(),
                            CellData::Image(_) => return ub(wm, "GadgetListBoxInput: an image used as a DisplayString"),
                        };
                        let first = d.text.first().copied().unwrap_or(0);
                        for st in 0..MAX_KEY_STATES {
                            if first == h.printable_key(k, st) {
                                lb(wm, w)?.select_pos = position;
                                let prev = top_entry(wm, w)?;
                                adjust_display(wm, h, w, position - prev, true)?;
                                found = true;
                                break;
                            }
                        }
                        if found {
                            do_audio_feedback(wm, h, w);
                            break;
                        }
                    }
                }
                if !found {
                    return Ok(Ignored);
                }
            }
        },
        gwm::WHEEL_DOWN => {
            let l = lbr(wm, w)?;
            if l.end_pos > 0 {
                let r = row(wm, w, i32::from(l.end_pos) - 1)?;
                if r.list_height > i32::from(l.display_height) + i32::from(l.display_pos) {
                    adjust_display(wm, h, w, 1, true)?;
                }
            }
        }
        gwm::WHEEL_UP => {
            if lbr(wm, w)?.end_pos > 0 {
                adjust_display(wm, h, w, -1, true)?;
            }
        }
        gwm::LEFT_UP => {
            wm.win_set_focus(h, Some(w));
            let mousey = (d1 >> 16) as i32;
            let old = lbr(wm, w)?.select_pos;
            let (p, i) = row_at(wm, h, w, mousey)?;
            lb(wm, w)?.select_pos = p;
            let l = lbr(wm, w)?;
            let now = wm.time_ms;
            if l.double_click_time.wrapping_add(DOUBLE_CLICK_TIME) > now && (i == old || (old == -1 && i >= 0 && i < i32::from(l.end_pos))) {
                lb(wm, w)?.double_click_time = 0;
                let temp = if old == -1 { i } else { old };
                let o = owner(wm, w);
                wm.win_send_system_msg(h, o, glm::DOUBLE_CLICKED, me, temp as u32);
            }
            let l = lb(wm, w)?;
            if i == old && !l.force_select {
                l.select_pos = -1;
            }
            if l.select_pos == -2 && i < i32::from(l.end_pos) {
                l.select_pos = i;
            }
            if l.select_pos < 0 && l.force_select {
                l.select_pos = old;
            }
            l.double_click_time = now;
            let sp = l.select_pos;
            let o = owner(wm, w);
            wm.win_send_system_msg(h, o, glm::SELECTED, me, sp as u32);
        }
        gwm::RIGHT_DOWN => do_audio_feedback(wm, h, w),
        gwm::RIGHT_UP => right_up(wm, h, w, d1)?,
        _ => return Ok(Ignored),
    }
    Ok(Handled)
}

/// `GadgetListBoxMultiInput`.
pub fn list_box_multi_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match list_box_multi_input_r(wm, h, w, msg, d1, d2) {
        Ok(m) => m,
        Err(Ub) => Handled,
    }
}

fn list_box_multi_input_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> R<MsgHandled> {
    if let Some(m) = common_input(wm, h, w, msg) {
        return Ok(m);
    }
    match msg {
        gwm::CHAR => match d1 as u8 {
            // `window->winNextTab()`: empty upstream
            key::TAB => {
                let _ = d2;
            }
            _ => return Ok(Ignored),
        },
        gwm::LEFT_UP => {
            wm.win_set_focus(h, Some(w));
            let mousey = (d1 >> 16) as i32;
            let (p, i) = row_at(wm, h, w, mousey)?;
            let select_pos = if p == -2 { i } else { p };
            let mut i = 0;
            let mut removed = false;
            loop {
                let s = sel(wm, w, i)?;
                if s < 0 {
                    break;
                }
                if s == select_pos {
                    remove_selection(wm, w, i)?;
                    removed = true;
                    break;
                }
                i += 1;
            }
            if !removed {
                set_sel(wm, w, i, select_pos)?;
                set_sel(wm, w, i + 1, -1)?;
            }
            let o = owner(wm, w);
            wm.win_send_system_msg(h, o, glm::SELECTED, win_data(Some(w)), select_pos as u32);
        }
        gwm::RIGHT_UP => right_up(wm, h, w, d1)?,
        gwm::WHEEL_DOWN => {
            let l = lbr(wm, w)?;
            if l.down_button.is_some() && i32::from(l.display_pos) + i32::from(l.display_height) <= l.total_height {
                adjust_display(wm, h, w, 1, true)?;
            }
        }
        gwm::WHEEL_UP => {
            let l = lbr(wm, w)?;
            if l.up_button.is_some() && l.display_pos > 0 {
                adjust_display(wm, h, w, -1, true)?;
            }
        }
        _ => return Ok(Ignored),
    }
    Ok(Handled)
}

/// `GadgetListBoxSystem`.
pub fn list_box_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match list_box_system_r(wm, h, w, msg, d1, d2) {
        Ok(m) => m,
        Err(Ub) => Handled,
    }
}

/// Frees the display strings and arrays of rows `0..listLength` (`GLM_DEL_ALL`, `GWM_DESTROY`): `cells[j].data` and
/// `userData` cleared, then the array freed and the row's pointer cleared.
fn free_all_rows(wm: &mut GameWindowManager, w: WinId) -> R<()> {
    let n = i32::from(lbr(wm, w)?.list_length);
    for i in 0..n {
        let r = row(wm, w, i)?;
        if let Some(a) = r.cell {
            let cols = i32::from(lbr(wm, w)?.columns);
            for j in (0..cols).rev() {
                let c = cell(wm, w, a, j)?;
                c.user_data = 0;
                c.data = CellData::None;
            }
            free_array(wm, w, a)?;
        }
        row_mut(wm, w, i)?.cell = None;
    }
    Ok(())
}

fn list_box_system_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> R<MsgHandled> {
    let me = win_data(Some(w));
    match msg {
        ggm::SET_LABEL => {
            let t = wm.msg_text.clone();
            wm.win_set_text(w, &t);
        }
        glm::GET_TEXT => {
            let (x, y) = wm.list_msg.pos;
            let l = lbr(wm, w)?;
            let mut out = (Vec::new(), 0);
            if !(x >= i32::from(l.columns) || y >= i32::from(l.list_length)) {
                let r = row(wm, w, y)?;
                let Some(a) = r.cell else { return ub(wm, "GLM_GET_TEXT: the row has no cells") };
                let c = cell(wm, w, a, x)?.clone();
                if c.cell_type == LISTBOX_TEXT {
                    match c.data {
                        CellData::Text(d) => out = (d.text, c.color),
                        _ => return ub(wm, "GLM_GET_TEXT: a text cell without a DisplayString"),
                    }
                }
            }
            wm.list_msg.text = out.0;
            wm.list_msg.color = out.1;
        }
        ggm::GBM_SELECTED | ggm::LEFT_DRAG => {
            // `(GameWindow *)mData1 == list->upButton`: a NULL sender matches a missing button
            let l = lbr(wm, w)?;
            if data_win(d1) == l.up_button {
                if l.display_pos > 0 {
                    adjust_display(wm, h, w, -1, true)?;
                }
            } else if data_win(d1) == l.down_button && i32::from(l.display_pos) + i32::from(l.display_height) <= l.total_height {
                adjust_display(wm, h, w, 1, true)?;
            }
        }
        glm::DEL_ALL => {
            free_all_rows(wm, w)?;
            let l = lb(wm, w)?;
            if let Some(d) = l.list_data.as_mut() {
                let n = usize::try_from(l.list_length).unwrap_or(0).min(d.len());
                d[..n].iter_mut().for_each(|r| *r = ListEntryRow::default());
            }
            if d1 != GP_DONT_UPDATE {
                l.display_pos = 0;
            }
            if l.multi_select {
                clear_sels(wm, w)?;
            } else {
                lb(wm, w)?.select_pos = -1;
            }
            let l = lb(wm, w)?;
            l.insert_pos = 0;
            l.end_pos = 0;
            l.total_height = 0;
            adjust_display(wm, h, w, 0, true)?;
        }
        glm::DEL_ENTRY => {
            let l = lbr(wm, w)?;
            if i32::from(l.end_pos) <= d1 as i32 {
                return Ok(Handled);
            }
            if (d1 as i32) < 0 {
                return ub(wm, "GLM_DEL_ENTRY: a negative row indexes listData out of range");
            }
            let i0 = d1 as i32;
            if let Some(a) = row(wm, w, i0)?.cell {
                // `for( i = 0; i <= list->columns; i++ )`: one past the array
                let _ = a;
                return ub(wm, "GLM_DEL_ENTRY: writes cells[columns], one past the row's array");
            }
            // `delete [] NULL`; the rows below move up (memcpy of overlapping rows: MSVC moves)
            let end = i32::from(l.end_pos);
            let d = lb(wm, w)?.list_data.as_mut().expect("row() checked");
            for k in i0..end - 1 {
                d[k as usize] = d[(k + 1) as usize];
            }
            let l = lb(wm, w)?;
            l.end_pos -= 1;
            l.insert_pos = l.end_pos;
            if l.multi_select {
                let mut i = 0;
                loop {
                    let s = sel(wm, w, i)?;
                    if s < 0 {
                        break;
                    }
                    if i0 < s {
                        set_sel(wm, w, i, s - 1)?;
                    } else if i0 == s {
                        remove_selection(wm, w, i)?;
                        i -= 1;
                    }
                    i += 1;
                }
            } else if i0 < l.select_pos {
                l.select_pos -= 1;
            } else if i0 == l.select_pos {
                l.select_pos = -1;
            }
            compute_total_height(wm, h, w)?;
        }
        glm::ADD_ENTRY => {
            let mut success = true;
            let mut added = -1;
            let mut add = wm.list_msg.add.clone();
            let l = lbr(wm, w)?;
            if add.row >= i32::from(l.insert_pos) {
                add.row = -1;
                wm.list_msg.add.row = -1;
            }
            let mut r = add.row;
            if add.row == -1 && l.insert_pos == l.list_length {
                r = i32::from(l.insert_pos);
                if l.auto_purge {
                    wm.win_send_system_msg(h, Some(w), glm::SCROLL_BUFFER, 1, 0);
                    if wm.undefined.is_some() {
                        return Err(Ub);
                    }
                } else {
                    success = false;
                }
            } else if add.row != -1 && !add.overwrite && l.insert_pos == l.list_length {
                if l.auto_purge {
                    wm.win_send_system_msg(h, Some(w), glm::SCROLL_BUFFER, 1, 0);
                    if wm.undefined.is_some() {
                        return Err(Ub);
                    }
                } else {
                    success = false;
                }
            }
            if success {
                if add.entry_type == LISTBOX_TEXT {
                    added = add_entry(wm, h, &add.text, d2 as i32, add.row, add.column, w, add.overwrite)?;
                } else if add.entry_type == LISTBOX_IMAGE {
                    added = add_image_entry(wm, h, add.image.clone(), d2 as i32, add.row, add.column, w, add.width, add.height)?;
                } else {
                    success = false;
                }
            }
            if success {
                if lbr(wm, w)?.auto_scroll {
                    let mut guard = 0;
                    loop {
                        let l = lbr(wm, w)?;
                        let at = if r == -1 { i32::from(l.insert_pos) - 1 } else { r };
                        if at >= i32::from(l.list_length) {
                            return ub(wm, "GLM_ADD_ENTRY: autoScroll reads listData[listLength] (a full auto-purged list)");
                        }
                        if row(wm, w, at)?.list_height >= i32::from(l.display_pos) + i32::from(l.display_height) {
                            adjust_display(wm, h, w, 1, true)?;
                        } else {
                            break;
                        }
                        guard += 1;
                        if guard > LOOP_LIMIT {
                            return ub(wm, "GLM_ADD_ENTRY: autoScroll's loop never ends");
                        }
                    }
                }
                if lbr(wm, w)?.multi_select {
                    let mut i = 0;
                    loop {
                        let s = sel(wm, w, i)?;
                        if s < 0 {
                            break;
                        }
                        if s != 0 {
                            set_sel(wm, w, i, -1)?;
                        }
                        i += 1;
                    }
                } else {
                    let l = lb(wm, w)?;
                    if r == l.select_pos {
                        l.select_pos = -1;
                    }
                }
            }
            wm.list_msg.added = added;
            return Ok(if added == 1 { Handled } else { Ignored });
        }
        glm::TOGGLE_MULTI_SELECTION => {
            if (d1 as i32) < 0 {
                if lbr(wm, w)?.multi_select {
                    clear_sels(wm, w)?;
                }
                return Ok(Handled);
            }
            if row(wm, w, d1 as i32)?.cell.is_none() {
                return Ok(Handled);
            }
            if lbr(wm, w)?.multi_select {
                let mut i = 0;
                let mut removed = false;
                loop {
                    let s = sel(wm, w, i)?;
                    if s < 0 {
                        break;
                    }
                    if s == d1 as i32 {
                        remove_selection(wm, w, i)?;
                        removed = true;
                        break;
                    }
                    i += 1;
                }
                if !removed {
                    set_sel(wm, w, i, d1 as i32)?;
                    set_sel(wm, w, i + 1, -1)?;
                }
            }
        }
        glm::SET_SELECTION => {
            let list = wm.list_msg.select.clone();
            let count = d2 as i32;
            let Some(&first) = list.first() else { return ub(wm, "GLM_SET_SELECTION: an empty list read at [0]") };
            let l = lbr(wm, w)?;
            let o = owner(wm, w);
            if first < 0 || i32::from(l.list_length) <= first {
                if l.multi_select {
                    clear_sels(wm, w)?;
                } else {
                    lb(wm, w)?.select_pos = -1;
                }
                let sp = lbr(wm, w)?.select_pos;
                wm.win_send_system_msg(h, o, glm::SELECTED, me, sp as u32);
                return Ok(Handled);
            }
            if l.multi_select {
                let mut i = 0;
                while i < count && i < i32::from(l.end_pos) {
                    let Some(&v) = list.get(i as usize) else { return ub(wm, "GLM_SET_SELECTION: the list read past its count") };
                    if i32::from(l.list_length) <= v {
                        break;
                    }
                    if row(wm, w, v)?.cell.is_none() {
                        break;
                    }
                    set_sel(wm, w, i, v)?;
                    i += 1;
                }
                set_sel(wm, w, i, -1)?;
            } else {
                if row(wm, w, first)?.cell.is_none() {
                    return Ok(Handled);
                }
                lb(wm, w)?.select_pos = first;
                let parent = wm.tree.windows[w].parent;
                if parent.is_some_and(|p| wm.tree.windows[p].inst.style & gws::COMBO_BOX != 0) {
                    wm.win_send_system_msg(h, o, glm::SELECTED, me, first as u32);
                    return Ok(Handled);
                }
                let l = lbr(wm, w)?;
                let lh = row(wm, w, first)?.list_height;
                if lh < i32::from(l.display_pos) {
                    wm.win_send_system_msg(h, Some(w), glm::UPDATE_DISPLAY, first as u32, 0);
                    if wm.undefined.is_some() {
                        return Err(Ub);
                    }
                } else if lh > i32::from(l.display_pos) + i32::from(l.display_height) {
                    let p = if first > 0 { lh.wrapping_sub(i32::from(l.display_height)) } else { 0 };
                    lb(wm, w)?.display_pos = p as i16;
                    adjust_display(wm, h, w, 0, true)?;
                }
            }
            let sp = lbr(wm, w)?.select_pos;
            wm.win_send_system_msg(h, o, glm::SELECTED, me, sp as u32);
        }
        glm::SCROLL_BUFFER => {
            let l = lbr(wm, w)?;
            if i32::from(l.end_pos) < d1 as i32 {
                return Ok(Handled);
            }
            let n = d1 as i32;
            if n < 0 {
                return ub(wm, "GLM_SCROLL_BUFFER: a negative count copies from outside listData");
            }
            for i in 0..n {
                if let Some(a) = row(wm, w, i)?.cell {
                    let cols = i32::from(lbr(wm, w)?.columns);
                    for j in 0..cols {
                        let c = cell(wm, w, a, j)?;
                        c.data = CellData::None;
                        c.user_data = 0;
                        c.color = 0;
                        c.cell_type = 0;
                    }
                    free_array(wm, w, a)?;
                }
                row_mut(wm, w, i)?.cell = None;
            }
            let end = i32::from(l.end_pos);
            // `memcpy(listData, &listData[mData1], (endPos - mData1) * ...)`: overlapping, moved
            for k in 0..end - n {
                let r = row(wm, w, k + n)?;
                *row_mut(wm, w, k)? = r;
            }
            let l = lb(wm, w)?;
            l.end_pos = (i32::from(l.end_pos) - n) as i16;
            l.insert_pos = l.end_pos;
            let e = i32::from(l.end_pos);
            for i in 0..n {
                row_mut(wm, w, e + i)?.cell = None;
            }
            if lbr(wm, w)?.multi_select {
                let mut i = 0;
                loop {
                    let s = sel(wm, w, i)?;
                    if s < 0 {
                        break;
                    }
                    if n >= s {
                        set_sel(wm, w, i, s - n)?;
                    } else {
                        remove_selection(wm, w, i)?;
                        i -= 1;
                    }
                    i += 1;
                }
            } else {
                let l = lb(wm, w)?;
                // (plant 4704: `>= 0` for `> 0`)
                if l.select_pos > 0 || (crate::logic::modules::plant(4704) && l.select_pos == 0) {
                    l.select_pos -= n;
                }
            }
            if lbr(wm, w)?.display_pos > 0 {
                adjust_display(wm, h, w, -n, true)?;
            }
            compute_total_height(wm, h, w)?;
        }
        glm::GET_SELECTION => {
            let l = lbr(wm, w)?;
            wm.list_msg.selection = if l.multi_select { l.selections.clone().unwrap_or_default() } else { vec![l.select_pos] };
        }
        glm::SET_UP_BUTTON => lb(wm, w)?.up_button = data_win(d1),
        glm::SET_DOWN_BUTTON => lb(wm, w)?.down_button = data_win(d1),
        glm::SET_SLIDER => lb(wm, w)?.slider = data_win(d1),
        gwm::CREATE => {}
        ggm::RESIZED => {
            let width = d1 as i32;
            let height = d2 as i32;
            let l = lbr(wm, w)?;
            let size = |o: Option<WinId>| o.map_or((0, 0), |x| wm.tree.windows[x].size);
            let (down, up, ssize) = (size(l.down_button), size(l.up_button), size(l.slider));
            if let Some(u) = l.up_button {
                wm.win_set_position(u, width - up.0 - 2, 2);
            }
            if let Some(d) = l.down_button {
                wm.win_set_position(d, width - down.0 - 2, height - down.1 - 2);
            }
            if let Some(s) = l.slider {
                wm.win_set_size(h, s, ssize.0, height - 2 * up.1 - 6);
                wm.win_set_position(s, width - ssize.0 - 2, up.1 + 3);
            }
            let ta = if wm.win_get_text(w).is_empty() { 0 } else { title_adjust(wm, h, w)? - 1 };
            let l = lb(wm, w)?;
            l.display_height = height as i16;
            l.display_height = l.display_height.wrapping_sub(ta as i16);
            let slider_w = l.slider.map(|s| wm.tree.windows[s].size.0);
            let l = lb(wm, w)?;
            if l.columns == 1 {
                let Some(cw) = l.column_width.as_mut().filter(|c| !c.is_empty()) else { return ub(wm, "GGM_RESIZED: columnWidth NULL") };
                cw[0] = width - slider_w.unwrap_or(0);
            } else {
                let Some(p) = l.column_width_percentage.clone() else { return Ok(Handled) };
                let Some(cw) = l.column_width.as_mut() else { return Ok(Handled) };
                let total = width - slider_w.unwrap_or(0);
                for i in 0..usize::try_from(l.columns).unwrap_or(0) {
                    let (Some(&pc), Some(c)) = (p.get(i), cw.get_mut(i)) else { return ub(wm, "GGM_RESIZED: column arrays shorter than columns") };
                    *c = pc.wrapping_mul(total) / 100;
                }
            }
            compute_total_height(wm, h, w)?;
        }
        glm::UPDATE_DISPLAY => {
            let p = if d1 > 0 { row(wm, w, (d1 as i32).wrapping_sub(1))?.list_height.wrapping_add(1) } else { 0 };
            let l = lb(wm, w)?;
            l.display_pos = p as i16;
            if i32::from(l.display_pos) + i32::from(l.display_height) >= l.total_height {
                l.display_pos = l.total_height.wrapping_sub(i32::from(l.display_height)) as i16;
            }
            adjust_display(wm, h, w, 0, true)?;
        }
        gwm::DESTROY => {
            free_all_rows(wm, w)?;
            wm.ext[w].user = UserData::None;
        }
        gwm::INPUT_FOCUS => {
            set_hilite(wm, w, d1 != 0);
            let o = owner(wm, w);
            let id = wm.tree.windows[w].id;
            wm.win_send_system_msg(h, o, ggm::FOCUS_CHANGE, d1, id as u32);
            wm.focus_reply = true;
        }
        ggm::GSM_SLIDER_TRACK => {
            let l = lbr(wm, w)?;
            let Some(s) = l.slider else { return ub(wm, "GSM_SLIDER_TRACK: list->slider NULL") };
            let Some(sd) = slider_rt(wm, s) else { return ub(wm, "GSM_SLIDER_TRACK: the slider has no SliderData") };
            let l = lb(wm, w)?;
            l.display_pos = sd.max_val.wrapping_sub(d2 as i32) as i16;
            if i32::from(l.display_pos) > l.total_height - i32::from(l.display_height) + 1 {
                l.display_pos = (l.total_height - i32::from(l.display_height) + 1) as i16;
            }
            if l.display_pos < 0 {
                l.display_pos = 0;
            }
            adjust_display(wm, h, w, 0, false)?;
        }
        glm::SET_ITEM_DATA => {
            let (x, y) = wm.list_msg.pos;
            let l = lbr(wm, w)?;
            if y >= 0 && y < i32::from(l.end_pos) {
                if let Some(a) = row(wm, w, y)?.cell {
                    cell(wm, w, a, x)?.user_data = d2;
                }
            }
        }
        glm::GET_ITEM_DATA => {
            let (x, y) = wm.list_msg.pos;
            wm.list_msg.item_data = 0;
            let l = lbr(wm, w)?;
            if y >= 0 && y < i32::from(l.end_pos) {
                if let Some(a) = row(wm, w, y)?.cell {
                    wm.list_msg.item_data = cell(wm, w, a, x)?.user_data;
                }
            }
        }
        _ => return Ok(Ignored),
    }
    Ok(Handled)
}

// ---------------------------------------------------------------------------
// the public functions
// ---------------------------------------------------------------------------

/// `GadgetListBoxGetTextAndColor(listbox, &color, row, column)`.
pub fn get_text_and_color<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, r: i32, column: i32) -> (Vec<u16>, i32) {
    if r == -1 || column == -1 || wm.tree.windows[w].inst.style & gws::SCROLL_LISTBOX == 0 {
        return (Vec::new(), 0);
    }
    wm.list_msg.pos = (column, r);
    // `TextAndColor tAndC;` is not initialised: a message that does not answer leaves it as it was
    wm.win_send_system_msg(h, Some(w), glm::GET_TEXT, 0, 0);
    (wm.list_msg.text.clone(), wm.list_msg.color)
}

/// `GadgetListBoxGetText`.
pub fn get_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, r: i32, column: i32) -> Vec<u16> {
    get_text_and_color(wm, h, w, r, column).0
}

/// `GadgetListBoxAddEntryText(listbox, text, color, row, column, overwrite)`: the row added at, or -1.
#[allow(clippy::too_many_arguments)]
pub fn add_entry_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, text: &[u16], color: i32, r: i32, column: i32, overwrite: bool) -> i32 {
    let text: Vec<u16> = if text.is_empty() { vec![u16::from(b' ')] } else { text.to_vec() };
    let Ok(l) = lbr(wm, w) else { return -1 };
    let was_full = l.list_length <= l.end_pos;
    let offset = if was_full { 0 } else { 1 };
    // `getListboxBottomEntry` reads `listData[-1]` (the allocation's header) on an empty list, or when every row
    // ends below the view: the value is only used with `scrollIfAtEnd`, and then it is undefined
    let old_bottom = if !l.scroll_if_at_end {
        0
    } else {
        let Ok(b) = bottom_entry(wm, w) else { return -1 };
        b
    };
    wm.list_msg.add = AddMessage { row: r, column, text, image: None, entry_type: LISTBOX_TEXT, overwrite, width: -1, height: -1 };
    wm.list_msg.added = 0;
    wm.win_send_system_msg(h, Some(w), glm::ADD_ENTRY, 0, color as u32);
    if wm.undefined.is_some() {
        return -1;
    }
    let index = wm.list_msg.added;
    if lbr(wm, w).is_ok_and(|l| l.scroll_if_at_end) && index - old_bottom == offset && is_full(wm, w) {
        set_bottom_visible_entry(wm, h, w, index);
    }
    index
}

/// `GadgetListBoxAddEntryImage(listbox, image, row, column, height, width, overwrite, color)`.
#[allow(clippy::too_many_arguments)]
pub fn add_entry_image<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, image: Option<Vec<u8>>, r: i32, column: i32, height: i32, width: i32, overwrite: bool, color: i32) -> i32 {
    wm.list_msg.add = AddMessage { row: r, column, text: Vec::new(), image, entry_type: LISTBOX_IMAGE, overwrite, width, height };
    wm.list_msg.added = 0;
    wm.win_send_system_msg(h, Some(w), glm::ADD_ENTRY, 0, color as u32);
    wm.list_msg.added
}

/// `GadgetListBoxSetFont(g, font)`: the window's text and every text cell.
pub fn set_font(wm: &mut GameWindowManager, w: WinId, font: Option<GameFont>) {
    let Some(l) = lb_opt(wm, w) else { return };
    let n = usize::try_from(l.list_length).unwrap_or(0);
    let cols = usize::try_from(l.columns).unwrap_or(0);
    let rows: Vec<Option<usize>> = l.list_data.as_ref().map(|d| d.iter().take(n).map(|r| r.cell).collect()).unwrap_or_default();
    for a in rows.into_iter().flatten() {
        let Some(Some(arr)) = l.arrays.get_mut(a) else { continue };
        for c in arr.iter_mut().take(cols) {
            if c.cell_type == LISTBOX_TEXT {
                if let CellData::Text(d) = &mut c.data {
                    d.font = font.clone();
                }
            }
        }
    }
}

/// `GadgetListboxCreateScrollbar`.
pub fn create_scrollbar<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    let (width, height) = wm.tree.windows[w].size;
    let title = !wm.win_get_text(w).is_empty();
    let mut status = wm.tree.windows[w].status & !(win_status::BORDER | win_status::HIDDEN | win_status::NO_INPUT);
    let font = wm.tree.windows[w].inst.font.clone();
    let fh = h.font_height(font.as_ref());
    let top = if title { fh + 1 } else { 0 };
    let bottom = if title { height - (fh + 1) } else { height };
    let (bw, bh) = (21, 22);
    status |= win_status::IMAGE;
    let track = wm.tree.windows[w].inst.style & gws::MOUSE_TRACK;
    let inst = |style: u32| WinInstanceData { style: style | track, ..WinInstanceData::new() };
    let st = status | win_status::ACTIVE | win_status::ENABLED;
    let up = wm.gogo_push_button(h, Some(w), st, width - bw - 2, top + 2, bw, bh, inst(gws::PUSH_BUTTON), true);
    let down = wm.gogo_push_button(h, Some(w), st, width - bw - 2, top + bottom - bh - 2, bw, bh, inst(gws::PUSH_BUTTON), true);
    let mut sd = SliderRt::default();
    let slider = crate::client::gadget_slider::gogo_gadget_slider(wm, h, Some(w), st, width - bw - 2, top + bh + 3, bw, bottom - 2 * bh - 6, inst(gws::VERT_SLIDER), &mut sd, true);
    if let Some(l) = lb_opt(wm, w) {
        l.up_button = Some(up);
        l.down_button = Some(down);
        l.slider = slider;
        l.scroll_bar = true;
    }
}

/// `GadgetListBoxAddMultiSelect`.
pub fn add_multi_select(wm: &mut GameWindowManager, w: WinId) {
    let Some(l) = lb_opt(wm, w) else { return };
    let n = usize::try_from(l.list_length).unwrap_or(0);
    l.selections = Some(vec![-1; n]);
    l.multi_select = true;
    wm.ext[w].input = InputFunc::ListBoxMulti;
}

/// `GadgetListBoxRemoveMultiSelect`.
pub fn remove_multi_select(wm: &mut GameWindowManager, w: WinId) {
    let Some(l) = lb_opt(wm, w) else { return };
    l.selections = None;
    l.multi_select = false;
    wm.ext[w].input = InputFunc::ListBox;
}

/// `GadgetListBoxSetListLength(listbox, newLength)`.
pub fn set_list_length<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, new_length: i32) {
    let _ = set_list_length_r(wm, h, w, new_length);
}

fn set_list_length_r<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, new_length: i32) -> R<()> {
    let Some(l) = lb_opt(wm, w) else { return Ok(()) };
    if l.columns < 1 {
        return Ok(());
    }
    if new_length < 0 {
        return ub(wm, "GadgetListBoxSetListLength: NEW ListEntryRow[negative]");
    }
    let l = lb(wm, w)?;
    let old_len = i32::from(l.list_length);
    let mut new_data = vec![ListEntryRow::default(); new_length as usize];
    let old = l.list_data.clone();
    let copy = if new_length >= old_len {
        old_len
    } else {
        if i32::from(l.display_pos) > new_length {
            l.display_pos = new_length as i16;
        }
        if l.select_pos > new_length || l.multi_select {
            l.select_pos = -1;
        }
        if i32::from(l.insert_pos) > new_length {
            l.insert_pos = new_length as i16;
        }
        l.end_pos = new_length as i16;
        new_length
    };
    if copy > 0 {
        let Some(o) = old.as_ref().filter(|o| o.len() >= copy as usize) else { return ub(wm, "GadgetListBoxSetListLength: memcpy from a NULL or short listData") };
        new_data[..copy as usize].copy_from_slice(&o[..copy as usize]);
    }
    for i in 0..old_len {
        let r = row(wm, w, i)?;
        if i >= new_length {
            if let Some(a) = r.cell {
                free_array(wm, w, a)?;
            }
        }
        row_mut(wm, w, i)?.cell = None;
    }
    let l = lb(wm, w)?;
    l.list_length = new_length as i16;
    l.list_data = Some(new_data);
    compute_total_height(wm, h, w)?;
    if lbr(wm, w)?.multi_select {
        remove_multi_select(wm, w);
        add_multi_select(wm, w);
    }
    Ok(())
}

/// `GadgetListBoxGetListLength`: the list length of a multi-select list box, else 1.
pub fn get_list_length(wm: &GameWindowManager, w: WinId) -> i32 {
    listbox_rt(wm, w).map_or(0, |l| if l.multi_select { i32::from(l.list_length) } else { 1 })
}

/// `GadgetListBoxGetNumEntries`.
pub fn get_num_entries(wm: &GameWindowManager, w: WinId) -> i32 {
    listbox_rt(wm, w).map_or(0, |l| i32::from(l.end_pos))
}

/// `GadgetListBoxGetSelected`: the selection (one value), or the multi-select array.
pub fn get_selected<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) -> Vec<i32> {
    wm.list_msg.selection.clear();
    wm.win_send_system_msg(h, Some(w), glm::GET_SELECTION, 0, 0);
    wm.list_msg.selection.clone()
}

/// `GadgetListBoxSetSelected(listbox, selectList, selectCount)` (one index: a list of one).
pub fn set_selected<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, list: &[i32]) {
    wm.list_msg.select = list.to_vec();
    wm.win_send_system_msg(h, Some(w), glm::SET_SELECTION, 0, list.len() as u32);
}

/// `GadgetListBoxReset`.
pub fn reset<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    wm.win_send_system_msg(h, Some(w), glm::DEL_ALL, 0, 0);
}

/// `GadgetListBoxSetItemData`.
pub fn set_item_data<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, data: u32, r: i32, column: i32) {
    wm.list_msg.pos = (column, r);
    wm.win_send_system_msg(h, Some(w), glm::SET_ITEM_DATA, 0, data);
}

/// `GadgetListBoxGetItemData`.
pub fn get_item_data<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, r: i32, column: i32) -> u32 {
    wm.list_msg.pos = (column, r);
    wm.list_msg.item_data = 0;
    wm.win_send_system_msg(h, Some(w), glm::GET_ITEM_DATA, 0, 0);
    wm.list_msg.item_data
}

/// `GadgetListBoxGetBottomVisibleEntry`.
pub fn get_bottom_visible_entry(wm: &mut GameWindowManager, w: WinId) -> i32 {
    if listbox_rt(wm, w).is_none() {
        return 0;
    }
    bottom_entry(wm, w).unwrap_or(0)
}

/// `GadgetListBoxIsFull`.
pub fn is_full(wm: &mut GameWindowManager, w: WinId) -> bool {
    let r: R<bool> = (|| {
        let e = bottom_entry(wm, w)?;
        let l = lbr(wm, w)?;
        Ok(row(wm, w, e)?.list_height >= i32::from(l.display_pos) + i32::from(l.display_height) - 5)
    })();
    r.unwrap_or(false)
}

/// `GadgetListBoxSetBottomVisibleEntry`.
pub fn set_bottom_visible_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, new_pos: i32) {
    if listbox_rt(wm, w).is_none() {
        return;
    }
    let Ok(prev) = bottom_entry(wm, w) else { return };
    let _ = adjust_display(wm, h, w, new_pos - prev + 1, true);
}

/// `GadgetListBoxGetTopVisibleEntry`.
pub fn get_top_visible_entry(wm: &mut GameWindowManager, w: WinId) -> i32 {
    if listbox_rt(wm, w).is_none() {
        return 0;
    }
    top_entry(wm, w).unwrap_or(0)
}

/// `GadgetListBoxSetTopVisibleEntry`.
pub fn set_top_visible_entry<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, new_pos: i32) {
    if listbox_rt(wm, w).is_none() {
        return;
    }
    let Ok(prev) = top_entry(wm, w) else { return };
    let _ = adjust_display(wm, h, w, new_pos - prev, true);
}

/// `GadgetListBoxSetAudioFeedback`.
pub fn set_audio_feedback(wm: &mut GameWindowManager, w: WinId, enable: bool) {
    if let Some(l) = lb_opt(wm, w) {
        l.audio_feedback = enable;
    }
}

/// `GadgetListBoxGetNumColumns`.
pub fn get_num_columns(wm: &GameWindowManager, w: WinId) -> i32 {
    listbox_rt(wm, w).map_or(0, |l| i32::from(l.columns))
}

/// `GadgetListBoxGetColumnWidth`.
pub fn get_column_width(wm: &GameWindowManager, w: WinId, column: i32) -> i32 {
    let Some(l) = listbox_rt(wm, w) else { return 0 };
    if i32::from(l.columns) <= column || column < 0 {
        return 0;
    }
    l.column_width.as_ref().and_then(|c| c.get(column as usize)).copied().unwrap_or(0)
}

/// The script's `ListboxData` as `gogoGadgetListBox` copies it (the internal fields zero).
pub fn listbox_template(d: &ListboxData) -> ListboxRt {
    ListboxRt {
        list_length: d.list_length,
        columns: d.columns,
        column_width_percentage: (!d.column_width_percentage.is_empty()).then(|| d.column_width_percentage.clone()),
        auto_scroll: d.auto_scroll,
        auto_purge: d.auto_purge,
        scroll_bar: d.scroll_bar,
        multi_select: d.multi_select,
        force_select: d.force_select,
        scroll_if_at_end: d.scroll_if_at_end,
        ..Default::default()
    }
}

/// `gogoGadgetListBox(parent, status, x, y, width, height, instData, listboxData, NULL, FALSE)`. `None` where upstream
/// asserts (not a list box style).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_list_box<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, template: ListboxRt) -> Option<WinId> {
    gogo_gadget_list_box_look(wm, h, parent, status, x, y, width, height, inst, template, false)
}

/// `gogoGadgetListBox(..., defaultVisual)` (`defaultFont` is the instance data's font).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_list_box_look<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, template: ListboxRt, default_visual: bool) -> Option<WinId> {
    if inst.style & gws::SCROLL_LISTBOX == 0 {
        return None;
    }
    let inst_font = inst.font.clone();
    let lw = wm.win_create(h, parent, status, x, y, width, height, SystemFunc::ListBox, Some(inst));
    list_box_after_create(wm, h, lw, parent, inst_font, template, default_visual);
    Some(lw)
}

/// `gogoGadgetListBox` after its `winCreate`: the data, owner, callbacks, list length, display, scroll bar, columns
/// and font. Where upstream returns `NULL` (more than one column without percentages, or no columns: `createGadget`
/// then dereferences it) the manager's `undefined` is set.
pub fn list_box_after_create<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, lw: WinId, parent: Option<WinId>, inst_font: Option<GameFont>, template: ListboxRt, default_visual: bool) {
    let (width, height) = wm.tree.windows[lw].size;
    // `instData->getTextLength()`: the script sets the text after creation (setWindowText), so never
    let title = false;
    let mut data = template;
    data.list_data = None;
    data.selections = None;
    data.column_width = None;
    data.arrays.clear();
    wm.ext[lw].user = UserData::Listbox(data);
    wm.win_set_owner(lw, parent);
    let image = wm.tree.windows[lw].status & win_status::IMAGE != 0;
    wm.ext[lw].draw = if image { DrawFunc::ListBoxImage } else { DrawFunc::ListBox };
    let multi = listbox_rt(wm, lw).is_some_and(|l| l.multi_select);
    wm.ext[lw].input = if multi { InputFunc::ListBoxMulti } else { InputFunc::ListBox };
    let length = listbox_rt(wm, lw).map_or(0, |l| i32::from(l.list_length));
    if let Some(l) = lb_opt(wm, lw) {
        l.list_length = 0;
    }
    set_list_length(wm, h, lw, length);
    if wm.undefined.is_some() {
        return;
    }
    let fh = if title { h.font_height(inst_font.as_ref()) } else { 0 };
    if let Some(l) = lb_opt(wm, lw) {
        l.display_height = height as i16;
        if title {
            l.display_height = l.display_height.wrapping_sub(fh as i16);
        }
        l.display_pos = 0;
        l.select_pos = -1;
        l.double_click_time = 0;
        l.insert_pos = 0;
        l.end_pos = 0;
        l.total_height = 0;
    }
    if listbox_rt(wm, lw).is_some_and(|l| l.scroll_bar) {
        create_scrollbar(wm, h, lw);
    }
    let slider_w = listbox_rt(wm, lw).and_then(|l| l.slider).map(|s| wm.tree.windows[s].size.0);
    let l = lb_opt(wm, lw).expect("set above");
    if l.columns == 1 {
        l.column_width = Some(vec![width - slider_w.map_or(0, |s| s + 2)]);
    } else {
        let n = usize::try_from(l.columns).unwrap_or(0);
        let Some(p) = l.column_width_percentage.clone().filter(|p| n > 0 && p.len() >= n) else {
            wm.set_undefined("gogoGadgetListBox: no columns or no column percentages (returns NULL; createGadget dereferences it)");
            return;
        };
        let total = width - slider_w.map_or(0, |s| s + 2);
        l.column_width = Some((0..n).map(|i| p[i].wrapping_mul(total) / 100).collect());
    }
    crate::client::gadget_look::assign_default_gadget_look(wm, h, lw, inst_font, default_visual);
}

/// createGadget's list box branch after `gogoGadgetListBox`: the file's part draw data to the up button, the down
/// button, the slider and its thumb (all nine of each state).
pub fn assign_part_draw_data(wm: &mut GameWindowManager, w: WinId) {
    let parts = wm.tree.windows[w].parts.clone();
    let Some(l) = listbox_rt(wm, w) else { return };
    let (up, down, slider) = (l.up_button, l.down_button, l.slider);
    let thumb = slider.and_then(|s| wm.tree.windows[s].children.first().copied());
    for (part, d) in parts {
        let target = match part {
            GadgetPart::ListBoxUpButton => up,
            GadgetPart::ListBoxDownButton => down,
            GadgetPart::ListBoxSlider => slider,
            GadgetPart::SliderThumb => thumb,
            _ => None,
        };
        if let Some(t) = target {
            let i = &mut wm.tree.windows[t].inst;
            i.enabled_draw_data = d.enabled.clone();
            i.disabled_draw_data = d.disabled.clone();
            i.hilite_draw_data = d.hilite.clone();
        }
    }
}

/// The selected rows (the single selection, or the multi-select array up to its -1).
pub fn selected_rows(wm: &GameWindowManager, w: WinId) -> Vec<i32> {
    let Some(l) = listbox_rt(wm, w) else { return Vec::new() };
    if l.multi_select {
        l.selections.as_ref().map(|s| s.iter().copied().take_while(|&x| x >= 0).collect()).unwrap_or_default()
    } else {
        vec![l.select_pos]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::gadget_slider::{slider_rt, HORIZONTAL_SLIDER_THUMB_POSITION};
    use crate::client::window_manager::tests::{Rec, WndHost800};
    use crate::client::window_manager::{DrawFunc, DrawItem};
    use crate::client::window_script::tests::Host as WndTestHost;
    use crate::client::window_script::WindowScript;

    /// Nine draw data with the image `name` first (`ImgX` names are images to the test host).
    fn dd9(field: &str, name: &str) -> String {
        let mut s = format!("  {field} = IMAGE: {name}, COLOR: 1 2 3 255, BORDERCOLOR: 4 5 6 255");
        for _ in 1..9 {
            s.push_str(", IMAGE: NoImage, COLOR: 1 2 3 255, BORDERCOLOR: 4 5 6 255");
        }
        s.push_str(";\n");
        s
    }

    /// A parent (0,0)-(400,300) at 800x600 with a host system and input, and one gadget `G` whose fields are `fields`.
    fn layout(ty: &str, rect: &str, status: &str, style: &str, fields: &str) -> Vec<u8> {
        format!(
            "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 400 300, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"TestSystem\";\n  INPUTCALLBACK = \"TestInput\";\n  CHILD\n\
  WINDOW\n  WINDOWTYPE = {ty};\n  SCREENRECT = {rect}, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:G\";\n  STATUS = {status};\n  STYLE = {style};\n{fields}  END\n  ENDALLCHILDREN\nEND\n"
        )
        .into_bytes()
    }

    fn load(h: &mut Rec, data: &[u8]) -> (GameWindowManager, WinId, WinId) {
        let mut wm = GameWindowManager::new((800, 600));
        let mut wnd = WndHost800(WndTestHost::default());
        wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", data, &mut wnd, h).expect("reads");
        let p = wm.tree.find_by_name(b"T.wnd:Parent").expect("parent");
        let g = wm.tree.find_by_name(b"T.wnd:G").expect("gadget");
        (wm, p, g)
    }

    fn listbox(h: &mut Rec, data: &str) -> (GameWindowManager, WinId, WinId) {
        let mut f = format!("  {data}\n");
        f += &dd9("LISTBOXENABLEDUPBUTTONDRAWDATA", "ImgUp");
        f += &dd9("LISTBOXENABLEDDOWNBUTTONDRAWDATA", "ImgDown");
        f += &dd9("LISTBOXENABLEDSLIDERDRAWDATA", "ImgSlider");
        f += &dd9("SLIDERTHUMBENABLEDDRAWDATA", "ImgThumb");
        load(h, &layout("SCROLLLISTBOX", "UPPERLEFT: 10 10, BOTTOMRIGHT: 210 110", "ENABLED", "SCROLLLISTBOX+MOUSETRACK", &f))
    }

    fn u(t: &str) -> Vec<u16> {
        t.encode_utf16().collect()
    }

    fn sys(h: &Rec, msg: u32) -> Vec<(u32, u32)> {
        h.sys.iter().filter(|m| m.2 == msg).map(|m| (m.2, m.3)).collect()
    }

    /// #70: a list box read from a WND file with a scroll bar. Predicted from gogoGadgetListBox,
    /// GadgetListboxCreateScrollbar, gogoGadgetSlider and createGadget, before the run (200x100 at (10,10), one
    /// column, LENGTH 10):
    /// - the inner windows, newest first: [slider, down, up]; up at (177,2), down at (177,76), both 21x22; the slider at
    ///   (177,25), 21x50, with the list box's status plus `IMAGE`, `ACTIVE` and `TAB_STOP`; its thumb 21x22 at (0,0),
    ///   `DRAGABLE`; all four track the mouse; the up button and thumb draw as image push buttons, the slider with the
    ///   **colour** draw (a list box's slider never uses the image draw);
    /// - the file's part draw data: the up button's image 0 `ImgUp`, the down button's `ImgDown`, the slider's
    ///   `ImgSlider`, the thumb's `ImgThumb`;
    /// - the slider's data: min 0, max 1 (min == max made max = min + 1), `numTicks` (50 - 16) / 1 = 34;
    /// - the column 200 - (21 + 2) = 177 wide; `displayHeight` 100; no title; selection -1;
    /// - three entries `A`, `BB`, `CCC` (font Times New Roman 14: 14 high): rows 0, 1, 2, `listHeight` 15, 30, 45; the
    ///   slider's max `45 - (100 - 2) + 1 < 0` so 0, `numTicks = 28 / 0 = inf`, and `GSM_SET_SLIDER(0)` puts the thumb at
    ///   `(Int)(0 * inf)` = `(Int)NaN` = **-2147483648** (x86's integer indefinite);
    /// - `GLM_GET_TEXT` of (1, 0) is `BB` with colour 7.
    #[test]
    fn a_list_box_is_created_with_its_scroll_bar_as_upstream_creates_it() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 10, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        assert_eq!(wm.undefined, None);
        let lr = listbox_rt(&wm, l).expect("data").clone();
        let (up, down, slider) = (lr.up_button.expect("up"), lr.down_button.expect("down"), lr.slider.expect("slider"));
        assert_eq!(wm.win(l).children, vec![slider, down, up]);
        assert_eq!((wm.win(up).region.0, wm.win(up).region.1, wm.win(up).size), (177, 2, (21, 22)));
        assert_eq!((wm.win(down).region.0, wm.win(down).region.1, wm.win(down).size), (177, 76, (21, 22)));
        assert_eq!((wm.win(slider).region.0, wm.win(slider).region.1, wm.win(slider).size), (177, 25, (21, 50)));
        let ss = wm.win(slider).status;
        assert_eq!(ss, win_status::ENABLED | win_status::IMAGE | win_status::ACTIVE | win_status::TAB_STOP);
        let thumb = wm.win(slider).children[0];
        assert_eq!((wm.win(thumb).region.0, wm.win(thumb).region.1, wm.win(thumb).size), (0, 0, (21, 22)));
        assert!(wm.win(thumb).status & win_status::DRAGABLE != 0);
        for w in [up, down, slider, thumb] {
            assert!(wm.win(w).inst.style & gws::MOUSE_TRACK != 0);
        }
        assert_eq!((wm.ext[up].draw.clone(), wm.ext[thumb].draw.clone(), wm.ext[slider].draw.clone()), (DrawFunc::PushButtonImage, DrawFunc::PushButtonImage, DrawFunc::VerticalSlider));
        let img = |w: WinId| wm.win(w).inst.enabled_draw_data[0].image.clone();
        assert_eq!([img(up), img(down), img(slider), img(thumb)], [Some(b"ImgUp".to_vec()), Some(b"ImgDown".to_vec()), Some(b"ImgSlider".to_vec()), Some(b"ImgThumb".to_vec())]);
        let s = slider_rt(&wm, slider).expect("slider data");
        assert_eq!((s.min_val, s.max_val, s.num_ticks), (0, 1, 34.0));
        assert_eq!((lr.column_width.clone(), lr.display_height, lr.select_pos), (Some(vec![177]), 100, -1));
        for (i, t) in ["A", "BB", "CCC"].iter().enumerate() {
            assert_eq!(add_entry_text(&mut wm, &mut h, l, &u(t), 7, -1, -1, false), i as i32);
        }
        let lr = listbox_rt(&wm, l).expect("data");
        let hs: Vec<i32> = lr.list_data.as_ref().expect("rows")[..3].iter().map(|r| r.list_height).collect();
        assert_eq!((hs, lr.total_height, lr.end_pos), (vec![15, 30, 45], 45, 3));
        let s = slider_rt(&wm, slider).expect("slider data");
        assert_eq!((s.max_val, s.position), (0, 0));
        assert!(s.num_ticks.is_infinite());
        assert_eq!(wm.win(thumb).region.1, i32::MIN);
        assert_eq!(get_text_and_color(&mut wm, &mut h, l, 1, 0), (u("BB"), 7));
        assert_eq!(wm.undefined, None);
    }

    /// #70: selecting with the mouse and the keys. Predicted from GadgetListBoxInput (rows of 15 at screen y 10):
    /// - a click at y 30 (20 into the list) selects row 1: the list box takes the focus (`GGM_FOCUS_CHANGE` 1 to the
    ///   parent), the parent gets `GLM_SELECTED` 1;
    /// - the same row again 200 ms later: `GLM_DOUBLE_CLICKED` 1 and then, not forced, `GLM_SELECTED` -1.
    ///   ~~Only those two messages~~ **Missed on the first run:** `GGM_FOCUS_CHANGE` 1 comes first again: the release
    ///   calls `winSetFocus` on the window that has the focus, which asks it again (`GWM_INPUT_FOCUS`, TRUE);
    /// - a click below the rows (y 70): -1;
    /// - with the focus: key down from -1 selects 0, then 1; key `C` (the test keyboard's shifted `C`) jumps to `CCC`,
    ///   row 2; key `Z` finds nothing and is not used (goes to the parent);
    /// - `FORCESELECT` set: a second click on the selected row keeps it.
    #[test]
    fn a_list_box_selects_as_upstream_selects() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 10, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for t in ["A", "BB", "CCC"] {
            add_entry_text(&mut wm, &mut h, l, &u(t), 7, -1, -1, false);
        }
        h.sys.clear();
        let click = |wm: &mut GameWindowManager, h: &mut Rec, y: i32, t: u32| {
            wm.time_ms = t;
            wm.win_process_mouse_event(h, gwm::LEFT_DOWN, (20, y), (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_UP, (20, y), (0, 0));
        };
        let ld = win_data(Some(l));
        click(&mut wm, &mut h, 30, 1000);
        assert_eq!(wm.keyboard_focus, Some(l));
        assert_eq!(sys(&h, ggm::FOCUS_CHANGE), vec![(ggm::FOCUS_CHANGE, 1)]);
        assert_eq!(h.sys.last(), Some(&(b"TestSystem".to_vec(), _p, glm::SELECTED, ld)));
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(1));
        h.sys.clear();
        click(&mut wm, &mut h, 30, 1200);
        assert_eq!(h.sys.iter().map(|m| m.2).collect::<Vec<_>>(), vec![ggm::FOCUS_CHANGE, glm::DOUBLE_CLICKED, glm::SELECTED]);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(-1));
        click(&mut wm, &mut h, 70, 3000);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(-1));
        wm.win_process_key(&mut h, key::DOWN, key::STATE_DOWN);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(0));
        wm.win_process_key(&mut h, key::DOWN, key::STATE_DOWN);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(1));
        assert_eq!(wm.win_process_key(&mut h, b'C', key::STATE_DOWN), crate::client::window_manager::WinInputReturnCode::Used);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(2));
        h.inp.clear();
        wm.win_process_key(&mut h, b'Z', key::STATE_DOWN);
        assert_eq!(h.inp.iter().map(|m| m.2).collect::<Vec<_>>(), vec![gwm::CHAR], "Z goes up to the parent");
        // forced
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 10, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 1;");
        add_entry_text(&mut wm, &mut h, l, &u("A"), 7, -1, -1, false);
        click(&mut wm, &mut h, 15, 1000);
        click(&mut wm, &mut h, 15, 5000);
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(0));
    }

    /// #70: scrolling through the scroll bar. Predicted (ten rows of 15: total 150; the slider's max 150 - 98 + 1 = 53,
    /// `numTicks` (50 - 22) / 53):
    /// - after the adds: `displayPos` 0, the slider at 53, its thumb at 0;
    /// - a click on the down button (screen (187,86)): `GUIClick`, then its `GBM_SELECTED` scrolls a row: `displayPos`
    ///   16, the slider at 53 - 16 = 37, the thumb at `(Int)(16 * 0.5283)` = 8; the top row 1;
    /// - the wheel up: back to 0, the slider at 53, the thumb at 0;
    /// - the thumb (screen (187,35)) pressed and dragged 20 down: the manager moves it to y 20; the slider's
    ///   `GGM_LEFT_DRAG`: centre 66, `delta` 20, `(Int)(20 / 0.5283)` = 37, position 53 - 37 = 16; the list box's
    ///   `GSM_SLIDER_TRACK` 16: `displayPos` 53 - 16 = 37 (not snapped to a row: `adjustDisplay(0, FALSE)`), top row 2;
    /// - the release on the thumb: its `GBM_SELECTED` reaches the slider, which tells the list box `GSM_SLIDER_DONE`
    ///   (ignored);
    /// - the border (`BORDER`): the slider adjustment is the thumb's **height** 22: the right corner at
    ///   7 + (200 + 3 - 22) - 5 = 183.
    #[test]
    fn a_list_box_scrolls_through_its_scroll_bar() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 10, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for i in 0..10 {
            add_entry_text(&mut wm, &mut h, l, &u(&format!("r{i}")), 7, -1, -1, false);
        }
        let lr = listbox_rt(&wm, l).expect("data").clone();
        let slider = lr.slider.expect("slider");
        let thumb = wm.win(slider).children[0];
        let st = |wm: &GameWindowManager| (listbox_rt(wm, l).expect("data").display_pos, slider_rt(wm, slider).expect("s").position, wm.win(thumb).region.1);
        assert_eq!(st(&wm), (0, 53, 0));
        h.audio.clear();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (190, 90), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (190, 90), (0, 0));
        assert_eq!(h.audio, vec![b"GUIClick".to_vec()]);
        assert_eq!(st(&wm), (16, 37, 8));
        assert_eq!(get_top_visible_entry(&mut wm, l), 1);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (20, 30), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::WHEEL_UP, (20, 30), (0, 0));
        assert_eq!(st(&wm), (0, 53, 0));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (190, 40), (0, 0));
        assert_eq!(wm.grab_window, Some(thumb));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DRAG, (190, 60), (0, 20));
        assert_eq!(st(&wm), (37, 16, 20));
        assert_eq!(get_top_visible_entry(&mut wm, l), 2);
        h.sys.clear();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (190, 60), (0, 0));
        assert_eq!(wm.grab_window, None);
        assert_eq!(wm.undefined, None);
        // the border
        wm.win_set_status(l, win_status::BORDER);
        let mut out = Vec::new();
        crate::client::window_draw::win_draw_border(&wm, &h, l, &mut out);
        let ur = out.iter().find_map(|d| if let DrawItem::Image { image, x0, .. } = d { (image == b"BorderCornerUR").then_some(*x0) } else { None });
        assert_eq!(ur, Some(183));
    }

    /// #70: the scroll buffer. Predicted from GLM_ADD_ENTRY and GLM_SCROLL_BUFFER (LENGTH 4, no scroll bar):
    /// - with `AUTOPURGE` only: a fifth entry drops the first: it is added at row 3 (returned), the texts are 2, 3, 4,
    ///   5; with the selection on row 2 before, it moves to row 1;
    /// - without `AUTOPURGE`: a fifth entry is refused (-1), nothing changes;
    /// - with `AUTOPURGE` and `AUTOSCROLL`: the fifth entry's scroll loop reads `listData[4]`, one past the list: the
    ///   translation records upstream's undefined behaviour and returns -1.
    #[test]
    fn a_full_list_box_drops_its_top_row_or_refuses() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 4, AUTOSCROLL: 0, AUTOPURGE: 1, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for i in 1..=4 {
            add_entry_text(&mut wm, &mut h, l, &u(&i.to_string()), 7, -1, -1, false);
        }
        set_selected(&mut wm, &mut h, l, &[2]);
        assert_eq!(add_entry_text(&mut wm, &mut h, l, &u("5"), 7, -1, -1, false), 3);
        let texts: Vec<Vec<u16>> = (0..4).map(|r| get_text(&mut wm, &mut h, l, r, 0)).collect();
        assert_eq!(texts, ["2", "3", "4", "5"].map(u).to_vec());
        assert_eq!(listbox_rt(&wm, l).map(|x| x.select_pos), Some(1));
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 4, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for i in 1..=4 {
            add_entry_text(&mut wm, &mut h, l, &u(&i.to_string()), 7, -1, -1, false);
        }
        assert_eq!(add_entry_text(&mut wm, &mut h, l, &u("5"), 7, -1, -1, false), -1);
        assert_eq!(get_text(&mut wm, &mut h, l, 0, 0), u("1"));
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 4, AUTOSCROLL: 1, AUTOPURGE: 1, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for i in 1..=4 {
            add_entry_text(&mut wm, &mut h, l, &u(&i.to_string()), 7, -1, -1, false);
        }
        assert_eq!(wm.undefined, None);
        assert_eq!(add_entry_text(&mut wm, &mut h, l, &u("5"), 7, -1, -1, false), -1);
        assert!(wm.undefined.is_some_and(|u| u.contains("listData[listLength]")), "{:?}", wm.undefined);
    }

    /// #70: multiple selection and columns. Predicted (MULTISELECT, LENGTH 4, two columns 30% / 70% of 200 - 23 = 177:
    /// 53 and 123):
    /// - clicks on rows 0 and 2 select [0, 2]; a click on row 0 removes it: [2] (removeSelection's copy reads one `Int`
    ///   past the array into the last slot, which it then overwrites with -1: kept, not undefined);
    /// - the input is `GadgetListBoxMultiInput`; `GLM_SELECTED` carries the clicked row;
    /// - adding an entry clears the selections that are not 0, stopping at the first cleared: [0, 2] becomes [0];
    /// - `GadgetListBoxGetEntryBasedOnXY` at x 10 + 60: column 1 (53 <= 60 < 176), row 1 at y 30.
    #[test]
    fn a_multi_select_list_box_toggles_rows() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 4, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 1, COLUMNS: 2, COLUMNSWIDTH%: 30, COLUMNSWIDTH%: 70, FORCESELECT: 0;");
        assert_eq!(wm.ext[l].input, InputFunc::ListBoxMulti);
        assert_eq!(listbox_rt(&wm, l).and_then(|x| x.column_width.clone()), Some(vec![53, 123]));
        for t in ["A", "B", "C"] {
            add_entry_text(&mut wm, &mut h, l, &u(t), 7, -1, -1, false);
        }
        let click = |wm: &mut GameWindowManager, h: &mut Rec, y: i32| {
            wm.win_process_mouse_event(h, gwm::LEFT_DOWN, (20, y), (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_UP, (20, y), (0, 0));
        };
        click(&mut wm, &mut h, 15);
        click(&mut wm, &mut h, 45);
        assert_eq!(selected_rows(&wm, l), vec![0, 2]);
        assert_eq!(h.sys.last().map(|m| (m.2, m.3)), Some((glm::SELECTED, win_data(Some(l)))));
        click(&mut wm, &mut h, 15);
        assert_eq!(selected_rows(&wm, l), vec![2]);
        click(&mut wm, &mut h, 15);
        assert_eq!(selected_rows(&wm, l), vec![2, 0]);
        add_entry_text(&mut wm, &mut h, l, &u("D"), 7, -1, -1, false);
        assert_eq!(selected_rows(&wm, l), Vec::<i32>::new(), "2 cleared first: the scan stops at the -1 it wrote");
        assert_eq!(wm.undefined, None);
        assert_eq!(get_entry_based_on_xy(&mut wm, &h, l, 70, 30), Some((1, 1)));
    }

    /// #70: the list box's colour draw. Predicted from W3DGadgetListBoxDraw / drawListBoxText (rows `A`, `BB` at
    /// colour 7): the background border and fill at (10,10)-(210,110) (the file's colours); the text of row 0 at
    /// (10 + 5, 10 + 4) clipped to (10, 14)-(10 + 177, 14 + 14), row 1 at y 29; the drop colour black (0xFF000000).
    #[test]
    fn a_list_box_draws_its_rows() {
        let mut h = Rec::default();
        let (mut wm, _p, l) = listbox(&mut h, "LISTBOXDATA = LENGTH: 10, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 1, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;");
        for t in ["A", "BB"] {
            add_entry_text(&mut wm, &mut h, l, &u(t), 7, -1, -1, false);
        }
        let mut out = Vec::new();
        crate::client::window_draw::draw(&mut wm, &mut h, l, &mut out);
        let texts: Vec<_> = out.iter().filter_map(|d| if let DrawItem::Text { x, y, clip, color, drop_color, .. } = d { Some((*x, *y, *clip, *color, *drop_color)) } else { None }).collect();
        let black = 0xFF00_0000u32 as i32;
        assert_eq!(texts, vec![(15, 14, Some((10, 14, 187, 28)), 7, black), (15, 29, Some((10, 29, 187, 43)), 7, black)]);
    }

    /// #70: a horizontal slider from a WND file. Predicted from gogoGadgetSlider and GadgetHorizontalSlider.cpp (200x20
    /// at (10,200), min 0, max 100):
    /// - the thumb 13x16 at (0, 10); `numTicks` 187 / 100 = 1.87;
    /// - `GSM_SET_SLIDER` 50: position 50, the thumb at x `(Int)(50 * 1.87)` = 93;
    /// - key left (focus on it) **increases** (the swapped keys): 52, the parent told `GSM_SLIDER_TRACK` 52, the thumb
    ///   at 97; key right: back to 50 and 93;
    /// - the thumb pressed at (105,215) and dragged 20 right: moved to 113; the slider: centre 10 + 113 + 6 = 129,
    ///   `delta` 113, `(Int)(113 / 1.87)` = 60; `GSM_SLIDER_TRACK` 60; the release: the thumb's `GBM_SELECTED` reaches
    ///   the slider, which ignores it (no `GSM_SLIDER_DONE`, unlike the vertical slider);
    /// - entering the slider hilites its thumb too.
    #[test]
    fn a_horizontal_slider_moves_as_upstream_moves_it() {
        let mut h = Rec::default();
        let (mut wm, p, s) = load(&mut h, &layout("HORZSLIDER", "UPPERLEFT: 10 200, BOTTOMRIGHT: 210 220", "ENABLED", "HORZSLIDER+MOUSETRACK", "  SLIDERDATA = MINVALUE: 0, MAXVALUE: 100;\n"));
        let thumb = wm.win(s).children[0];
        assert_eq!((wm.win(thumb).region.0, wm.win(thumb).region.1, wm.win(thumb).size), (0, HORIZONTAL_SLIDER_THUMB_POSITION, (13, 16)));
        assert_eq!(slider_rt(&wm, s).map(|x| x.num_ticks), Some(187.0 / 100.0));
        crate::client::gadget_slider::slider_set_position(&mut wm, &mut h, s, 50);
        assert_eq!((slider_rt(&wm, s).map(|x| x.position), wm.win(thumb).region.0), (Some(50), 93));
        wm.win_set_focus(&mut h, Some(s));
        h.sys.clear();
        wm.win_process_key(&mut h, key::LEFT, key::STATE_DOWN);
        assert_eq!(h.sys, vec![(b"TestSystem".to_vec(), p, ggm::GSM_SLIDER_TRACK, win_data(Some(s)))]);
        assert_eq!((slider_rt(&wm, s).map(|x| x.position), wm.win(thumb).region.0), (Some(52), 97));
        wm.win_process_key(&mut h, key::RIGHT, key::STATE_DOWN);
        assert_eq!((slider_rt(&wm, s).map(|x| x.position), wm.win(thumb).region.0), (Some(50), 93));
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (105, 215), (0, 0));
        assert!(wm.win(thumb).inst.state & WIN_STATE_HILITED != 0);
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (105, 215), (0, 0));
        assert_eq!(wm.grab_window, Some(thumb));
        h.sys.clear();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DRAG, (125, 215), (20, 0));
        assert_eq!((slider_rt(&wm, s).map(|x| x.position), wm.win(thumb).region.0), (Some(60), 113));
        assert_eq!(h.sys.iter().map(|m| m.2).collect::<Vec<_>>(), vec![ggm::GSM_SLIDER_TRACK]);
        h.sys.clear();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (125, 215), (0, 0));
        assert!(!h.sys.iter().any(|m| m.2 == ggm::GSM_SLIDER_DONE));
        assert_eq!(wm.undefined, None);
    }
}
