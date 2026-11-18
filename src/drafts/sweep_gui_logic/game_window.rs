//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (findFirstLeaf, findLastLeaf, findPrevLeaf,
//!   findNextLeaf, winGetCursorPosition, winGetRegion, winSetDrawOffset, winGetDrawOffset, winGetTextLength, the text
//!   colour getters, winSetInstanceData, winSetWindowId, winSetLayout, winSetSystemFunc, winSetDrawFunc,
//!   winSetCallbacks, winDrawWindow, the draw data setters, winGetSystemFunc, winGetTooltipFunc, winGetDrawFunc)
//!
//! The verified port models a window as an arena entry: [`GameWindow`] (the reader's) plus [`WinExt`] (the manager's),
//! in [`GameWindowManager`], with `m_parent`, `m_child` and `m_next` / `m_prev` as a [`WinId`] and the sibling lists
//! (`parent.children`, `window_list`), head first. So `m_child` is `children.first()`, `m_next` is
//! [`GameWindowManager::next_of`] and `m_prev` is [`prev_of`] below.
//!
//! Sweep 2 findings for this file:
//! - `winSetDisabledTextColors`, `winSetHiliteTextColors` (GameWindow.cpp:897, 911): translated,
//!   `GameWindowManager::win_set_text_colors(w, 1 | 2, color, border)` (window_manager.rs:1637), which also reaches the
//!   combo box.
//! - `winSetNext`, `winSetPrev`, `winGetPrev`, `winSetNextInLayout`, `winSetPrevInLayout`, `winGetNextInLayout`,
//!   `winGetPrevInLayout` (:1217-1297): the arena derives the links from the lists (`next_of`, `unlink_window`,
//!   `insert_window_ahead_of`, and a layout's `WindowLayout::windows`), so there is no field to set. [`prev_of`] is
//!   `winGetPrev`.
//! - `winSetEditData`, `winGetEditData` (:1788, 1797): excluded, `GameWindowEditData` is the GUI editor's (WorldBuilder-side
//!   tool) and the game never sets it.
//! - `GameWindow::winGetWindowList` (GameWindowManager.cpp:687): translated, the field `GameWindowManager::window_list`.

use crate::client::window_manager::{GameWindowManager, DrawFunc, SystemFunc, WIN_ERR_INVALID_PARAMETER, WIN_ERR_OK};
use crate::client::window_script::{win_status, WinId, WinInstanceData, MAX_DRAW_DATA};

/// `m_prev`: the window before `w` in its parent's child list, or in the window list.
pub fn prev_of(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    let list = match wm.tree.windows[w].parent {
        Some(p) => &wm.tree.windows[p].children,
        None => &wm.window_list,
    };
    let i = list.iter().position(|&x| x == w)?;
    if i == 0 {
        None
    } else {
        list.get(i - 1).copied()
    }
}

/// `m_child`: the head of the child list.
fn child_of(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.tree.windows[w].children.first().copied()
}

fn is_tab_stop(wm: &GameWindowManager, w: WinId) -> bool {
    wm.tree.windows[w].status & win_status::TAB_STOP != 0
}

/// `GameWindow::findFirstLeaf()` (GameWindow.cpp:171): the root of this window's branch, then its first leaf.
pub fn find_first_leaf(wm: &GameWindowManager, this: WinId) -> WinId {
    let mut leaf = this;
    // Find the root of this branch
    while let Some(p) = wm.tree.windows[leaf].parent {
        leaf = p;
    }
    // Find the first leaf
    while let Some(c) = child_of(wm, leaf) {
        leaf = c;
    }
    leaf
}

/// `GameWindow::findLastLeaf()` (:190): the root of the branch, then down the last child of each level.
pub fn find_last_leaf(wm: &GameWindowManager, this: WinId) -> WinId {
    let mut leaf = this;
    // Find the root of this branch
    while let Some(p) = wm.tree.windows[leaf].parent {
        leaf = p;
    }
    // Find the last leaf
    while let Some(c) = child_of(wm, leaf) {
        leaf = c;
        while let Some(n) = wm.next_of(leaf) {
            leaf = n;
        }
    }
    leaf
}

/// `GameWindow::findPrevLeaf()` (:216): the previous leaf for tabbing, going down into the previous sibling's last
/// children until a tab stop; with no previous sibling, up the parents to one that has one; else the last leaf.
pub fn find_prev_leaf(wm: &GameWindowManager, this: WinId) -> Option<WinId> {
    let mut leaf = this;
    if let Some(p) = prev_of(wm, leaf) {
        leaf = p;
        while let (Some(c), false) = (child_of(wm, leaf), is_tab_stop(wm, leaf)) {
            leaf = c;
            while let Some(n) = wm.next_of(leaf) {
                leaf = n;
            }
        }
        Some(leaf)
    } else {
        while let Some(parent) = wm.tree.windows[leaf].parent {
            leaf = parent;
            if wm.tree.windows[leaf].parent.is_some() {
                if let Some(p) = prev_of(wm, leaf) {
                    leaf = p;
                    while let (Some(c), false) = (child_of(wm, leaf), is_tab_stop(wm, leaf)) {
                        leaf = c;
                        while let Some(n) = wm.next_of(leaf) {
                            leaf = n;
                        }
                    }
                    return Some(leaf);
                }
            }
        }
        // if( leaf ) return leaf->findLastLeaf(); (leaf is never NULL here)
        Some(find_last_leaf(wm, leaf))
    }
}

/// `GameWindow::findNextLeaf()` (:283): the next leaf for tabbing: the next sibling if it is a tab stop, else down its
/// first children to a leaf or a tab stop; with no next sibling, up the parents to one that has one; else the first leaf.
pub fn find_next_leaf(wm: &GameWindowManager, this: WinId) -> Option<WinId> {
    let mut leaf = this;
    if let Some(n) = wm.next_of(leaf) {
        if is_tab_stop(wm, n) {
            return Some(n);
        }
        let mut cur = Some(n);
        while let Some(l) = cur {
            if child_of(wm, l).is_none() || is_tab_stop(wm, l) {
                return Some(l);
            }
            cur = child_of(wm, l);
        }
        None
    } else {
        while let Some(parent) = wm.tree.windows[leaf].parent {
            leaf = parent;
            if wm.tree.windows[leaf].parent.is_some() {
                if let Some(n) = wm.next_of(leaf) {
                    let mut cur = Some(n);
                    while let Some(l) = cur {
                        if child_of(wm, l).is_none() || is_tab_stop(wm, l) {
                            return Some(l);
                        }
                        cur = child_of(wm, l);
                    }
                }
            }
        }
        // if( leaf ) return leaf->findFirstLeaf();
        Some(find_first_leaf(wm, leaf))
    }
}

/// `GameWindow::winGetCursorPosition(x, y)` (:528): the window's `m_cursorX`, `m_cursorY` (the text cursor / click
/// position inside it, kept by the host), `WIN_ERR_OK`.
pub fn win_get_cursor_position(cursor: (i32, i32)) -> ((i32, i32), i32) {
    (cursor, WIN_ERR_OK)
}

/// `GameWindow::winGetRegion(region)` (:570): `m_region` as (lo.x, lo.y, hi.x, hi.y).
pub fn win_get_region(wm: &GameWindowManager, w: WinId) -> ((i32, i32, i32, i32), i32) {
    (wm.tree.windows[w].region, WIN_ERR_OK)
}

/// `GameWindow::winSetDrawOffset(x, y)` (:781): `m_instData.m_imageOffset`.
pub fn win_set_draw_offset(wm: &mut GameWindowManager, w: WinId, x: i32, y: i32) {
    wm.tree.windows[w].inst.image_offset = (x, y);
}

/// `GameWindow::winGetDrawOffset(x, y)` (:792): `None` when either output was `NULL`.
pub fn win_get_draw_offset(wm: &GameWindowManager, w: WinId, have_x: bool, have_y: bool) -> Option<(i32, i32)> {
    // sanity
    if !have_x || !have_y {
        return None;
    }
    Some(wm.tree.windows[w].inst.image_offset)
}

/// `GameWindow::winGetTextLength()` (:829): the length of the window's text (`m_instData.getTextLength()`).
pub fn win_get_text_length(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.win_get_text(w).len() as i32
}

/// `winGetEnabledTextColor` (:938).
pub fn win_get_enabled_text_color(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.enabled_text.color
}
/// `winGetDisabledTextColor` (:958).
pub fn win_get_disabled_text_color(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.disabled_text.color
}
/// `winGetDisabledTextBorderColor` (:968).
pub fn win_get_disabled_text_border_color(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.disabled_text.border_color
}
/// `winGetHiliteTextColor` (:998).
pub fn win_get_hilite_text_color(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.hilite_text.color
}
/// `winGetHiliteTextBorderColor` (:1008).
pub fn win_get_hilite_text_border_color(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.hilite_text.border_color
}
/// `winGetIMECompositeTextColor` (:978) and `winGetIMECompositeBorderColor` (:988): `m_imeCompositeText`, which the
/// verified `WinInstanceData` does not carry (the IME composition is not translated, `client::ime_manager`); the
/// instance data's value is the draft's `ime_composite_text` argument.
pub fn win_get_ime_composite_text_color(ime_composite_text: (i32, i32)) -> i32 {
    ime_composite_text.0
}
pub fn win_get_ime_composite_border_color(ime_composite_text: (i32, i32)) -> i32 {
    ime_composite_text.1
}

/// `GameWindow::winSetInstanceData(data)` (:1019): "copy over all values from the inst data passed in" but keep this
/// window's own text and tooltip display strings; a copied video buffer is dropped; the text and tooltip text of `data`
/// are set if they have any. The verified `WinInstanceData` has no video buffer.
pub fn win_set_instance_data(wm: &mut GameWindowManager, w: WinId, data: &WinInstanceData) -> i32 {
    // DisplayString *text = m_instData.m_text, *tooltipText = m_instData.m_tooltip; (the window's own, kept)
    let own_tooltip_text = wm.tree.windows[w].inst.tooltip_text.clone();
    let own_text = wm.tree.windows[w].text.clone();
    // m_instData = *data;
    wm.tree.windows[w].inst = data.clone();
    // put our text instance pointers back
    wm.tree.windows[w].inst.tooltip_text = own_tooltip_text;
    wm.tree.windows[w].text = own_text;
    // set our text display instance text if present
    if !data.text_label_string.is_empty() {
        let label: Vec<u16> = data.text_label_string.iter().map(|&b| b as u16).collect();
        wm.win_set_text(w, &label);
    }
    if let Some(t) = data.tooltip_text.as_ref() {
        if !t.is_empty() {
            wm.tree.windows[w].inst.tooltip_text = Some(t.clone());
        }
    }
    WIN_ERR_OK
}

/// `GameWindow::winSetWindowId(id)` (:1092): `m_instData.m_id = id`.
pub fn win_set_window_id(wm: &mut GameWindowManager, w: WinId, id: i32) -> i32 {
    wm.tree.windows[w].inst.id = id;
    wm.tree.windows[w].id = id;
    WIN_ERR_OK
}

/// `GameWindow::winSetLayout(layout)` (:1273): `m_layout = layout`.
pub fn win_set_layout(wm: &mut GameWindowManager, w: WinId, layout: Option<usize>) {
    wm.ext[w].layout = layout;
}

/// `GameWindow::winSetSystemFunc(system)` (:1305): "if (system) m_system = system; else m_system =
/// TheWindowManager->getDefaultSystem()".
pub fn win_set_system_func(wm: &mut GameWindowManager, w: WinId, system: Option<SystemFunc>) -> i32 {
    wm.ext[w].system = system.unwrap_or(SystemFunc::Default);
    WIN_ERR_OK
}

/// `GameWindow::winSetDrawFunc(draw)` (:1334): "if (draw) m_draw = draw; else m_draw =
/// TheWindowManager->getDefaultDraw()", which on the W3D manager is `W3DGameWinDefaultDraw`.
pub fn win_set_draw_func(wm: &mut GameWindowManager, w: WinId, draw: Option<DrawFunc>) -> i32 {
    wm.ext[w].draw = draw.unwrap_or(DrawFunc::W3DDefault);
    WIN_ERR_OK
}

/// `GameWindow::winSetCallbacks(input, draw, tooltip)` (:1361): `winSetInputFunc(input)`, `winSetDrawFunc(draw)`,
/// `winSetTooltipFunc(tooltip)`. The input callback is the manager's `InputFunc` (a null one is the default).
pub fn win_set_callbacks(
    wm: &mut GameWindowManager,
    w: WinId,
    input: Option<crate::client::window_manager::InputFunc>,
    draw: Option<DrawFunc>,
    tooltip: Option<Vec<u8>>,
) -> i32 {
    wm.ext[w].input = input.unwrap_or(crate::client::window_manager::InputFunc::Default);
    win_set_draw_func(wm, w, draw);
    wm.ext[w].tooltip = tooltip;
    WIN_ERR_OK
}

/// `GameWindow::winDrawWindow()` (:1377): a shown window with a draw callback draws itself, `m_draw(this,
/// &m_instData)`. The draw callback is the host's (`GuiHost::draw`), so this answers whether it is called.
pub fn win_draw_window_should_draw(wm: &GameWindowManager, w: WinId) -> bool {
    wm.tree.windows[w].status & win_status::HIDDEN == 0
}

fn in_range(index: i32) -> bool {
    index >= 0 && index < MAX_DRAW_DATA as i32
}

/// `winSetEnabledColor(index, color)` (:1585): `WIN_ERR_INVALID_PARAMETER` outside `0..MAX_DRAW_DATA`.
pub fn win_set_enabled_color(wm: &mut GameWindowManager, w: WinId, index: i32, color: i32) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.enabled_draw_data[index as usize].color = color;
    WIN_ERR_OK
}
/// `winSetDisabledImage(index, image)` (:1627).
pub fn win_set_disabled_image(wm: &mut GameWindowManager, w: WinId, index: i32, image: Option<Vec<u8>>) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.disabled_draw_data[index as usize].image = image;
    WIN_ERR_OK
}
/// `winSetDisabledColor(index, color)` (:1648).
pub fn win_set_disabled_color(wm: &mut GameWindowManager, w: WinId, index: i32, color: i32) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.disabled_draw_data[index as usize].color = color;
    WIN_ERR_OK
}
/// `winSetDisabledBorderColor(index, color)` (:1669).
pub fn win_set_disabled_border_color(wm: &mut GameWindowManager, w: WinId, index: i32, color: i32) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.disabled_draw_data[index as usize].border_color = color;
    WIN_ERR_OK
}
/// `winSetHiliteImage(index, image)` (:1690).
pub fn win_set_hilite_image(wm: &mut GameWindowManager, w: WinId, index: i32, image: Option<Vec<u8>>) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.hilite_draw_data[index as usize].image = image;
    WIN_ERR_OK
}
/// `winSetHiliteColor(index, color)` (:1711).
pub fn win_set_hilite_color(wm: &mut GameWindowManager, w: WinId, index: i32, color: i32) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.hilite_draw_data[index as usize].color = color;
    WIN_ERR_OK
}
/// `winSetHiliteBorderColor(index, color)` (:1732).
pub fn win_set_hilite_border_color(wm: &mut GameWindowManager, w: WinId, index: i32, color: i32) -> i32 {
    if !in_range(index) {
        return WIN_ERR_INVALID_PARAMETER;
    }
    wm.tree.windows[w].inst.hilite_draw_data[index as usize].border_color = color;
    WIN_ERR_OK
}

/// `winGetSystemFunc()` (:1761).
pub fn win_get_system_func(wm: &GameWindowManager, w: WinId) -> &SystemFunc {
    &wm.ext[w].system
}
/// `winGetTooltipFunc()` (:1770): the host's tooltip callback, by name.
pub fn win_get_tooltip_func(wm: &GameWindowManager, w: WinId) -> Option<&Vec<u8>> {
    wm.ext[w].tooltip.as_ref()
}
/// `winGetDrawFunc()` (:1779).
pub fn win_get_draw_func(wm: &GameWindowManager, w: WinId) -> &DrawFunc {
    &wm.ext[w].draw
}
