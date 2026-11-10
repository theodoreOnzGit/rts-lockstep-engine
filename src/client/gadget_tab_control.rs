// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetTabControl.cpp (GadgetTabControlInput,
//     GadgetTabControlSystem, GadgetTabControlComputeTabRegion, GadgetTabControlComputeSubPaneSize,
//     GadgetTabControlShowSubPane, GadgetTabControlCreateSubPanes, GadgetTabControlResizeSubPanes,
//     GadgetTabControlFixupSubPaneList)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoGadgetTabControl)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (parseChildWindows: the default
//     panes destroyed before a tab control's children are read, and the fix-up after)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Gadget.h (TabControlData, the TP_* positions, NUM_TAB_PANES)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The tab control (GitHub #72, a part of #34): a row (or column) of tabs on one edge, each showing one of up to
//! eight sub-panes.
//!
//! # As upstream reads it
//!
//! `TabControlData` keeps where the tabs are (`tabEdge`: top, right, left or bottom; `tabOrientation`: centred, top
//! left, bottom right along that edge), their size, `tabCount`, `paneBorder`, the panes (`subPanes`, eight windows),
//! which are disabled, the active tab, and the rectangle the tabs cover (`tabs*Limit`, relative to the window).
//!
//! - **Made** (`gogoGadgetTabControl`): the tab rectangle is computed; eight panes are created as children
//!   (`PassSelectedButtonsToParentSystem`, style `GWS_TAB_PANE`, named `Pane n`, enabled as the tab control is), the
//!   size of the window less the border and the tabs; the active pane is shown and the others hidden.
//! - **A left press** inside the tab rectangle picks the tab under it (along the edge, by `tabWidth` or `tabHeight`);
//!   an enabled tab that is not the active one is shown. A press outside the tabs is eaten.
//! - **Showing a pane** hides every pane and shows the asked one (if it exists), `activeTab` at most `tabCount - 1`.
//! - **Resized**: the panes and the tab rectangle follow. `GBM_SELECTED` goes to the parent.
//! - **From a file**: the default panes are destroyed before the tab control's children are read, and the children
//!   (the file's `TABPANE` windows), oldest first, become `subPanes` (`GadgetTabControlFixupSubPaneList`).
//!
//! # Upstream quirks that are kept
//!
//! - The left press shows the pane and then falls through to `default`: the input returns `MSG_IGNORED`, so the press
//!   also goes to the parent. A press outside the tabs returns `MSG_HANDLED`.
//! - A press at the far edge of the tab rectangle (`mouseX == tabsRight`) picks tab `tabCount`, which
//!   `GadgetTabControlShowSubPane` then clamps to the last tab.
//! - `GadgetTabControlFixupSubPaneList` fills the panes from the children and leaves the rest of `subPanes` as they
//!   were: pointers to the default panes, which were destroyed.
//! - With the window's border, the tab control draws its border twice (its draw and `drawWindow`; see
//!   [`crate::client::window_draw`]).
//!
//! # Upstream undefined behaviour, and what the translation does
//!
//! Set in [`GameWindowManager::undefined`], the message stopped: a tab size of zero along the edge (division by
//! zero), a tab index outside the eight panes (`subPaneDisabled[tabPressed]`, `subPanes[whichPane]`, an `activeTab`
//! below 0 when `tabCount` is 0 or less), a pane that was freed and is hidden or shown, no pane where one is shown
//! (`NULL`), and more than eight children to fix up.
//!
//! # Where this differs from upstream
//!
//! Panes are windows (ids); the pane's name (`m_decoratedNameString`) is set as upstream sets it. The script's
//! replay ([`GameWindowManager::win_create_from_script`]) destroys the default panes of a tab control that has
//! children in the file (upstream: one that has a `CHILD` block, even an empty one).
//!
//! # Representation
//!
//! [`TabRt`] in the window's [`UserData::Tab`].

use crate::client::window_manager::{gwm, DrawFunc, GameWindowManager, GuiHost, InputFunc, MsgHandled, SystemFunc, UserData, WindowMsgData};
use crate::client::window_script::{gws, win_status, GameFont, TabControlData, WinId, WinInstanceData, NUM_TAB_PANES};

use MsgHandled::{Handled, Ignored};

/// `TP_CENTER`, ... (Gadget.h).
pub mod tp {
    pub const CENTER: i32 = 0;
    pub const TOPLEFT: i32 = 1;
    pub const BOTTOMRIGHT: i32 = 2;
    pub const TOP_SIDE: i32 = 3;
    pub const RIGHT_SIDE: i32 = 4;
    pub const LEFT_SIDE: i32 = 5;
    pub const BOTTOM_SIDE: i32 = 6;
}

/// `TabControlData` as the tab control keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabRt {
    pub tab_orientation: i32,
    pub tab_edge: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub tab_count: i32,
    pub sub_panes: [Option<WinId>; NUM_TAB_PANES],
    pub sub_pane_disabled: [bool; NUM_TAB_PANES],
    pub pane_border: i32,
    pub active_tab: i32,
    pub tabs_left_limit: i32,
    pub tabs_right_limit: i32,
    pub tabs_top_limit: i32,
    pub tabs_bottom_limit: i32,
}

/// The tab control's data, if the window has some.
pub fn tab_rt(wm: &GameWindowManager, w: WinId) -> Option<&TabRt> {
    match &wm.ext[w].user {
        UserData::Tab(t) => Some(t),
        _ => None,
    }
}

struct Ub;
type R<T> = Result<T, Ub>;

fn ub<T>(wm: &mut GameWindowManager, what: &'static str) -> R<T> {
    wm.set_undefined(what);
    Err(Ub)
}

fn tab(wm: &mut GameWindowManager, w: WinId) -> R<TabRt> {
    match tab_rt(wm, w) {
        Some(t) => Ok(t.clone()),
        None => ub(wm, "tab control: no TabControlData (NULL user data)"),
    }
}

fn tab_mut(wm: &mut GameWindowManager, w: WinId) -> R<&mut TabRt> {
    tab(wm, w)?;
    match &mut wm.ext[w].user {
        UserData::Tab(t) => Ok(t),
        _ => unreachable!("checked"),
    }
}

/// An index into the eight panes.
fn pane_index(wm: &mut GameWindowManager, i: i32, what: &'static str) -> R<usize> {
    match usize::try_from(i) {
        Ok(i) if i < NUM_TAB_PANES => Ok(i),
        _ => ub(wm, what),
    }
}

/// `winHide` of a pane: a freed one is undefined.
fn hide_pane(wm: &mut GameWindowManager, p: WinId, hide: bool) -> R<()> {
    if wm.ext[p].freed {
        return ub(wm, "tab control: a pane used after it was freed");
    }
    wm.win_hide(p, hide);
    Ok(())
}

/// `LOLONGTOSHORT`, `HILONGTOSHORT` of a packed mouse position.
fn unpack(d: WindowMsgData) -> (i32, i32) {
    (i32::from(d as u16 as i16), i32::from((d >> 16) as u16 as i16))
}

/// `GadgetTabControlInput`.
pub fn tab_control_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    tab_control_input_r(wm, h, w, msg, d1, d2).unwrap_or(Handled)
}

fn tab_control_input_r<H: GuiHost>(wm: &mut GameWindowManager, _h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> R<MsgHandled> {
    let t = tab(wm, w)?;
    let (tx, ty) = wm.tree.screen_position(w);
    let (mx, my) = unpack(d1);
    let (mouse_x, mouse_y) = (mx.wrapping_sub(tx), my.wrapping_sub(ty));
    if msg == gwm::LEFT_DOWN {
        let right = if crate::logic::modules::plant(4707) { t.tabs_right_limit - 1 } else { t.tabs_right_limit };
        if mouse_x < t.tabs_left_limit || mouse_x > right || mouse_y < t.tabs_top_limit || mouse_y > t.tabs_bottom_limit {
            return Ok(Handled);
        }
        let (distance, size) = if t.tab_edge == tp::RIGHT_SIDE || t.tab_edge == tp::LEFT_SIDE { (mouse_y - t.tabs_top_limit, t.tab_height) } else { (mouse_x - t.tabs_left_limit, t.tab_width) };
        if size == 0 {
            return ub(wm, "GadgetTabControlInput: the tab size is 0 (division by zero)");
        }
        let pressed = distance.wrapping_div(size);
        let i = pane_index(wm, pressed, "GadgetTabControlInput: subPaneDisabled[tabPressed] past the array")?;
        if !t.sub_pane_disabled[i] && pressed != t.active_tab {
            show_sub_pane_r(wm, w, pressed)?;
        }
    }
    Ok(Ignored)
}

/// `GadgetTabControlSystem`.
pub fn tab_control_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::None,
        crate::client::window_manager::ggm::RESIZED => {
            resize_sub_panes(wm, h, w);
            if wm.undefined.is_none() {
                compute_tab_region(wm, w);
            }
        }
        crate::client::window_manager::ggm::GBM_SELECTED => {
            if let Some(p) = wm.tree.windows[w].parent {
                return wm.win_send_system_msg(h, Some(p), msg, d1, d2);
            }
            return Ignored;
        }
        _ => return Ignored,
    }
    Handled
}

/// `GadgetTabControlComputeTabRegion`: the rectangle the tabs cover, relative to the window.
pub fn compute_tab_region(wm: &mut GameWindowManager, w: WinId) {
    let (ww, wh) = wm.tree.windows[w].size;
    let Ok(t) = tab_mut(wm, w) else { return };
    let (mut horz, mut vert) = (0, 0);
    let along_x = t.tab_count.wrapping_mul(t.tab_width);
    let along_y = t.tab_count.wrapping_mul(t.tab_height);
    let border2 = t.pane_border.wrapping_mul(2);
    if t.tab_edge == tp::TOP_SIDE || t.tab_edge == tp::BOTTOM_SIDE {
        match t.tab_orientation {
            tp::CENTER => horz = ww.wrapping_sub(border2).wrapping_sub(along_x) / 2,
            tp::BOTTOMRIGHT => horz = ww.wrapping_sub(border2).wrapping_sub(along_x),
            _ => {}
        }
    } else {
        match t.tab_orientation {
            tp::CENTER => vert = wh.wrapping_sub(border2).wrapping_sub(along_y) / 2,
            tp::BOTTOMRIGHT => vert = wh.wrapping_sub(border2).wrapping_sub(along_y),
            _ => {}
        }
    }
    let b = t.pane_border;
    match t.tab_edge {
        tp::TOP_SIDE => {
            t.tabs_top_limit = b;
            t.tabs_bottom_limit = b.wrapping_add(t.tab_height);
            t.tabs_left_limit = b.wrapping_add(horz);
            t.tabs_right_limit = b.wrapping_add(horz).wrapping_add(along_x);
        }
        tp::BOTTOM_SIDE => {
            t.tabs_top_limit = wh.wrapping_sub(b).wrapping_sub(t.tab_height);
            t.tabs_bottom_limit = wh.wrapping_sub(b);
            t.tabs_left_limit = b.wrapping_add(horz);
            t.tabs_right_limit = b.wrapping_add(horz).wrapping_add(along_x);
        }
        tp::RIGHT_SIDE => {
            t.tabs_left_limit = ww.wrapping_sub(b).wrapping_sub(t.tab_width);
            t.tabs_right_limit = ww.wrapping_sub(b);
            t.tabs_top_limit = b.wrapping_add(vert);
            t.tabs_bottom_limit = b.wrapping_add(vert).wrapping_add(along_y);
        }
        tp::LEFT_SIDE => {
            t.tabs_left_limit = b;
            t.tabs_right_limit = b.wrapping_add(t.tab_width);
            t.tabs_top_limit = b.wrapping_add(vert);
            t.tabs_bottom_limit = b.wrapping_add(vert).wrapping_add(along_y);
        }
        _ => {}
    }
}

/// `GadgetTabControlComputeSubPaneSize`: the panes' `(width, height, x, y)`.
pub fn compute_sub_pane_size(wm: &GameWindowManager, w: WinId) -> (i32, i32, i32, i32) {
    let (ww, wh) = wm.tree.windows[w].size;
    let Some(t) = tab_rt(wm, w) else { return (0, 0, 0, 0) };
    let b2 = t.pane_border.wrapping_mul(2);
    let top_bottom = t.tab_edge == tp::TOP_SIDE || t.tab_edge == tp::BOTTOM_SIDE;
    let left_right = t.tab_edge == tp::LEFT_SIDE || t.tab_edge == tp::RIGHT_SIDE;
    let height = if top_bottom { wh.wrapping_sub(b2).wrapping_sub(t.tab_height) } else { wh.wrapping_sub(b2) };
    let width = if left_right { ww.wrapping_sub(b2).wrapping_sub(t.tab_width) } else { ww.wrapping_sub(b2) };
    let x = if t.tab_edge == tp::LEFT_SIDE { t.pane_border.wrapping_add(t.tab_width) } else { t.pane_border };
    let y = if t.tab_edge == tp::TOP_SIDE { t.pane_border.wrapping_add(t.tab_height) } else { t.pane_border };
    (width, height, x, y)
}

/// `GadgetTabControlShowSubPane(tabControl, whichPane)`.
pub fn show_sub_pane(wm: &mut GameWindowManager, w: WinId, which: i32) {
    let _ = show_sub_pane_r(wm, w, which);
}

fn show_sub_pane_r(wm: &mut GameWindowManager, w: WinId, which: i32) -> R<()> {
    let t = tab(wm, w)?;
    for p in t.sub_panes.iter().flatten() {
        hide_pane(wm, *p, true)?;
    }
    let i = pane_index(wm, which, "GadgetTabControlShowSubPane: subPanes[whichPane] past the array")?;
    let t = tab_mut(wm, w)?;
    t.active_tab = if t.sub_panes[i].is_some() { which } else { 0 };
    t.active_tab = t.active_tab.min(t.tab_count.wrapping_sub(1));
    let active = t.active_tab;
    let a = pane_index(wm, active, "GadgetTabControlShowSubPane: subPanes[activeTab] outside the array")?;
    let Some(p) = tab(wm, w)?.sub_panes[a] else { return ub(wm, "GadgetTabControlShowSubPane: subPanes[activeTab] NULL") };
    hide_pane(wm, p, false)
}

/// `GadgetTabControlCreateSubPanes`: the missing panes created, the others moved and sized; then the active one shown.
pub fn create_sub_panes<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    let (width, height, x, y) = compute_sub_pane_size(wm, w);
    for i in 0..NUM_TAB_PANES {
        let Ok(t) = tab(wm, w) else { return };
        match t.sub_panes[i] {
            None => {
                // `WIN_STATUS_NONE` (0), no instance data
                let p = wm.win_create(h, Some(w), 0, x, y, width, height, SystemFunc::PassSelectedButtonsToParent, None);
                if let UserData::Tab(t) = &mut wm.ext[w].user {
                    t.sub_panes[i] = Some(p);
                }
                let inst = &mut wm.tree.windows[p].inst;
                inst.style |= gws::TAB_PANE;
                inst.decorated_name_string = format!("Pane {i}").into_bytes();
                let enabled = wm.tree.windows[w].status & win_status::ENABLED != 0;
                wm.win_enable(p, enabled);
            }
            Some(p) => {
                if wm.ext[p].freed {
                    wm.set_undefined("GadgetTabControlCreateSubPanes: a pane used after it was freed");
                    return;
                }
                wm.win_set_size(h, p, width, height);
                wm.win_set_position(p, x, y);
            }
        }
    }
    let Ok(t) = tab(wm, w) else { return };
    show_sub_pane(wm, w, t.active_tab);
}

/// `GadgetTabControlResizeSubPanes`.
pub fn resize_sub_panes<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    let (width, height, x, y) = compute_sub_pane_size(wm, w);
    let Ok(t) = tab(wm, w) else { return };
    for p in t.sub_panes.iter().flatten() {
        if wm.ext[*p].freed {
            wm.set_undefined("GadgetTabControlResizeSubPanes: a pane used after it was freed");
            return;
        }
        wm.win_set_size(h, *p, width, height);
        wm.win_set_position(*p, x, y);
    }
}

/// `GadgetTabControlFixupSubPaneList`: the children, oldest first, as the panes (the rest of the array unchanged).
pub fn fixup_sub_pane_list(wm: &mut GameWindowManager, w: WinId) {
    let kids: Vec<WinId> = wm.tree.windows[w].children.iter().rev().copied().collect();
    if kids.len() > NUM_TAB_PANES {
        wm.set_undefined("GadgetTabControlFixupSubPaneList: more than NUM_TAB_PANES children written into subPanes");
        return;
    }
    let Ok(t) = tab_mut(wm, w) else { return };
    for (i, k) in kids.into_iter().enumerate() {
        t.sub_panes[i] = Some(k);
    }
}

/// The script's `TabControlData` (the working fields zero).
pub fn tab_template(d: &TabControlData) -> TabRt {
    TabRt { tab_orientation: d.tab_orientation, tab_edge: d.tab_edge, tab_width: d.tab_width, tab_height: d.tab_height, tab_count: d.tab_count, sub_pane_disabled: d.sub_pane_disabled, pane_border: d.pane_border, ..Default::default() }
}

/// `gogoGadgetTabControl(parent, status, x, y, width, height, instData, tData, defaultFont, defaultVisual)`
/// (`defaultFont` is the instance data's font). `None` where upstream asserts (not a tab control style).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_tab_control<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, template: TabRt, default_visual: bool) -> Option<WinId> {
    if inst.style & gws::TAB_CONTROL == 0 {
        return None;
    }
    let font = inst.font.clone();
    let t = wm.win_create(h, parent, status, x, y, width, height, SystemFunc::TabControl, Some(inst));
    tab_control_after_create(wm, h, t, parent, font, template, default_visual);
    Some(t)
}

/// `gogoGadgetTabControl` after its `winCreate`.
pub fn tab_control_after_create<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, t: WinId, parent: Option<WinId>, font: Option<GameFont>, template: TabRt, default_visual: bool) {
    wm.ext[t].user = UserData::Tab(template);
    compute_tab_region(wm, t);
    create_sub_panes(wm, h, t);
    if wm.undefined.is_some() {
        return;
    }
    show_sub_pane(wm, t, 0);
    if wm.undefined.is_some() {
        return;
    }
    wm.ext[t].input = InputFunc::TabControl;
    let image = wm.tree.windows[t].status & win_status::IMAGE != 0;
    wm.ext[t].draw = if image { DrawFunc::TabControlImage } else { DrawFunc::TabControl };
    wm.win_set_owner(t, parent);
    crate::client::gadget_look::assign_default_gadget_look(wm, h, t, font, default_visual);
}

/// `parseChildWindows` for a tab control: its default panes destroyed before the file's children are read.
pub fn destroy_default_panes(wm: &mut GameWindowManager, w: WinId, panes: &[WinId]) {
    for &p in panes {
        if wm.tree.windows[p].parent == Some(w) {
            wm.win_destroy(Some(p));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::{short_to_long, tests::Rec};

    fn tabs(wm: &mut GameWindowManager, h: &mut Rec, edge: i32, orientation: i32, count: i32) -> WinId {
        let parent = wm.win_create(h, None, win_status::ENABLED, 0, 0, 400, 300, SystemFunc::Host(b"P".to_vec()), None);
        let inst = WinInstanceData { style: gws::TAB_CONTROL, ..WinInstanceData::new() };
        let mut d = TabRt { tab_edge: edge, tab_orientation: orientation, tab_width: 30, tab_height: 20, tab_count: count, pane_border: 4, ..Default::default() };
        d.sub_pane_disabled[2] = true;
        gogo_gadget_tab_control(wm, h, Some(parent), win_status::ENABLED, 10, 10, 200, 100, inst, d, false).expect("a tab control")
    }

    /// #72: the tab rectangle and the panes. Predicted from GadgetTabControl.cpp (200x100, border 4, tabs 30x20, 3
    /// tabs): top edge, centred: left 4 + (200 - 8 - 90) / 2 = 55, right 145, top 4, bottom 24; the panes 192x72 at
    /// (4, 24), eight of them, named `Pane 0..7`, styled `TAB_PANE`, only pane 0 shown. Left edge, bottom-right: left 4,
    /// right 34, top 4 + (100 - 8 - 60) = 36, bottom 96; panes 162x92 at (34, 4).
    #[test]
    fn a_tab_control_computes_its_tabs_and_panes_as_upstream_does() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let t = tabs(&mut wm, &mut h, tp::TOP_SIDE, tp::CENTER, 3);
        let d = tab_rt(&wm, t).expect("data").clone();
        assert_eq!((d.tabs_left_limit, d.tabs_right_limit, d.tabs_top_limit, d.tabs_bottom_limit), (55, 145, 4, 24));
        let panes: Vec<WinId> = d.sub_panes.iter().map(|p| p.expect("a pane")).collect();
        for (i, &p) in panes.iter().enumerate() {
            assert_eq!((wm.win(p).region.0, wm.win(p).region.1, wm.win(p).size), (4, 24, (192, 72)));
            assert_eq!(wm.win(p).inst.decorated_name_string, format!("Pane {i}").into_bytes());
            assert_ne!(wm.win(p).inst.style & gws::TAB_PANE, 0);
            assert_eq!(wm.win(p).status & win_status::HIDDEN != 0, i != 0);
        }
        let l = tabs(&mut wm, &mut h, tp::LEFT_SIDE, tp::BOTTOMRIGHT, 3);
        let d = tab_rt(&wm, l).expect("data").clone();
        assert_eq!((d.tabs_left_limit, d.tabs_right_limit, d.tabs_top_limit, d.tabs_bottom_limit), (4, 34, 36, 96));
        assert_eq!(compute_sub_pane_size(&wm, l), (162, 92, 34, 4));
    }

    /// #72: the hit test. Predicted (the top, centred control at (10,10)): a press at x 10 + 55 + 30 (tab 1) shows
    /// pane 1 and returns `MSG_IGNORED`; at tab 2 (disabled) nothing changes; at the right edge, x 10 + 145, tab 3,
    /// clamped to tab 2 (shown, since the disabled check is on tab 3); outside the tabs, eaten (`MSG_HANDLED`); a
    /// resize moves the panes and the tabs; `GBM_SELECTED` goes to the parent.
    #[test]
    fn a_tab_control_picks_tabs_as_upstream_does() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let t = tabs(&mut wm, &mut h, tp::TOP_SIDE, tp::CENTER, 3);
        let at = |x: i32, y: i32| short_to_long(x, y);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(t), gwm::LEFT_DOWN, at(10 + 55 + 30, 20), 0), Ignored);
        assert_eq!(tab_rt(&wm, t).expect("data").active_tab, 1);
        wm.win_send_input_msg(&mut h, Some(t), gwm::LEFT_DOWN, at(10 + 55 + 61, 20), 0);
        assert_eq!(tab_rt(&wm, t).expect("data").active_tab, 1);
        wm.win_send_input_msg(&mut h, Some(t), gwm::LEFT_DOWN, at(10 + 145, 20), 0);
        assert_eq!(tab_rt(&wm, t).expect("data").active_tab, 2);
        assert_eq!(wm.win_send_input_msg(&mut h, Some(t), gwm::LEFT_DOWN, at(5, 5), 0), Handled);
        wm.win_set_size(&mut h, t, 300, 100);
        let d = tab_rt(&wm, t).expect("data").clone();
        assert_eq!((d.tabs_left_limit, wm.win(d.sub_panes[0].expect("pane")).size), (105, (292, 72)));
        let p = wm.tree.windows[t].parent;
        h.sys.clear();
        wm.win_send_system_msg(&mut h, Some(t), crate::client::window_manager::ggm::GBM_SELECTED, 5, 0);
        assert_eq!(h.sys.iter().map(|s| (s.1, s.2)).collect::<Vec<_>>(), vec![(p.expect("parent"), crate::client::window_manager::ggm::GBM_SELECTED)]);
        // a zero tab width is a division by zero upstream
        if let UserData::Tab(d) = &mut wm.ext[t].user {
            d.tab_width = 0;
            d.tabs_right_limit = d.tabs_left_limit;
        }
        wm.win_send_input_msg(&mut h, Some(t), gwm::LEFT_DOWN, at(10 + 105, 20), 0);
        assert!(wm.undefined.is_some());
    }

    /// #72: the panes from a file. Predicted: destroying the default panes and fixing up three children makes them
    /// panes 0..2 (oldest first) and leaves 3..7 pointing at the destroyed defaults; showing pane 1 after the update
    /// (the defaults freed) is a use of freed memory.
    #[test]
    fn a_tab_control_takes_its_panes_from_its_children() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let t = tabs(&mut wm, &mut h, tp::TOP_SIDE, tp::TOPLEFT, 3);
        let defaults: Vec<WinId> = tab_rt(&wm, t).expect("data").sub_panes.iter().flatten().copied().collect();
        destroy_default_panes(&mut wm, t, &defaults);
        assert!(wm.win(t).children.is_empty());
        let kids: Vec<WinId> = (0..3).map(|_| wm.win_create(&mut h, Some(t), 0, 0, 0, 5, 5, SystemFunc::PassSelectedButtonsToParent, None)).collect();
        fixup_sub_pane_list(&mut wm, t);
        let d = tab_rt(&wm, t).expect("data").clone();
        assert_eq!(&d.sub_panes[..3], &[Some(kids[0]), Some(kids[1]), Some(kids[2])]);
        assert_eq!(d.sub_panes[3], Some(defaults[3]));
        wm.update(&mut h);
        show_sub_pane(&mut wm, t, 1);
        assert!(wm.undefined.is_some());
    }

    /// #72: combo boxes, text entries and tab controls read from a file (`createGadget`, `setWindowText`,
    /// `parseChildWindows`). Predicted: the entry `E` (label `Lbl`) shows the label itself (`Lbl`, 3 characters),
    /// `maxTextLen` 10, with the entry's callbacks; the combo box `CB` (editable, 16 characters, 5 rows) has its three
    /// children, its edit box with `Entry`, not `NO_INPUT`, `maxTextLen` 16, its list of 10 rows; the tab control `TC`
    /// with the panes `P1`, `P2` in the file loses its eight default panes (destroyed, off its child list) and its
    /// panes are `P1`, `P2` (oldest first), the rest the destroyed defaults, which the next update frees.
    #[test]
    fn script_gadgets_are_created_as_upstream_creates_them() {
        use crate::client::window_manager::tests::WndHost800;
        use crate::client::window_script::tests::Host as WndTestHost;
        use crate::client::window_script::WindowScript;
        let data = b"FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 400 300, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  CHILD\n\
  WINDOW\n  WINDOWTYPE = ENTRYFIELD;\n  SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 110 30, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:E\";\n  STATUS = ENABLED;\n  STYLE = ENTRYFIELD;\n  TEXT = \"Lbl\";\n  TEXTENTRYDATA = MAXLEN: 10, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n  END\n\
  WINDOW\n  WINDOWTYPE = COMBOBOX;\n  SCREENRECT = UPPERLEFT: 10 40, BOTTOMRIGHT: 130 60, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:CB\";\n  STATUS = ENABLED;\n  STYLE = COMBOBOX;\n  COMBOBOXDATA = ISEDITABLE: 1, MAXCHARS: 16, MAXDISPLAY: 5, ASCIIONLY: 0, LETTERSANDNUMBERS: 0;\n  END\n\
  WINDOW\n  WINDOWTYPE = TABCONTROL;\n  SCREENRECT = UPPERLEFT: 10 70, BOTTOMRIGHT: 210 170, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:TC\";\n  STATUS = ENABLED;\n  STYLE = TABCONTROL;\n  TABCONTROLDATA = TABORIENTATION: 0, TABEDGE: 3, TABWIDTH: 30, TABHEIGHT: 20, TABCOUNT: 2, PANEBORDER: 4, PANEDISABLED: 2, 0, 0;\n  CHILD\n\
    WINDOW\n    WINDOWTYPE = TABPANE;\n    SCREENRECT = UPPERLEFT: 14 94, BOTTOMRIGHT: 206 166, CREATIONRESOLUTION: 800 600;\n    NAME = \"T.wnd:P1\";\n    STATUS = ENABLED;\n    STYLE = TABPANE;\n    END\n\
    WINDOW\n    WINDOWTYPE = TABPANE;\n    SCREENRECT = UPPERLEFT: 14 94, BOTTOMRIGHT: 206 166, CREATIONRESOLUTION: 800 600;\n    NAME = \"T.wnd:P2\";\n    STATUS = ENABLED;\n    STYLE = TABPANE;\n    END\n\
  ENDALLCHILDREN\n  END\n\
  ENDALLCHILDREN\nEND\n";
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mut wnd = WndHost800(WndTestHost::default());
        wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", data, &mut wnd, &mut h).expect("reads");
        let f = |n: &str| wm.tree.find_by_name(format!("T.wnd:{n}").as_bytes()).expect(n);
        let (e, cb, tc, p1, p2) = (f("E"), f("CB"), f("TC"), f("P1"), f("P2"));
        assert_eq!(wm.undefined, None);
        let ed = crate::client::gadget_text_entry::entry_rt(&wm, e).expect("entry data");
        assert_eq!((ed.text.text.clone(), ed.char_pos, ed.max_text_len), ("Lbl".encode_utf16().collect::<Vec<u16>>(), 3, 10));
        assert_eq!((wm.ext[e].input.clone(), wm.ext[e].draw.clone()), (InputFunc::TextEntry, DrawFunc::TextEntry));
        let c = crate::client::gadget_combo_box::combo_rt(&wm, cb).expect("combo data").clone();
        assert_eq!(wm.win(cb).children.len(), 3);
        let eb = c.edit_box.expect("edit box");
        assert_eq!(crate::client::gadget_text_entry::entry_rt(&wm, eb).expect("entry").text.text, "Entry".encode_utf16().collect::<Vec<u16>>());
        assert_eq!(wm.win(eb).status & win_status::NO_INPUT, 0);
        assert_eq!(crate::client::gadget_text_entry::entry_rt(&wm, eb).expect("entry").max_text_len, 16);
        assert_eq!(crate::client::gadget_list_box::listbox_rt(&wm, c.list_box.expect("list")).expect("list").list_length, 10);
        let t = tab_rt(&wm, tc).expect("tab data").clone();
        assert_eq!(wm.win(tc).children, vec![p2, p1]);
        assert_eq!(&t.sub_panes[..2], &[Some(p1), Some(p2)]);
        let d = t.sub_panes[2].expect("a default pane");
        assert_ne!(wm.win(d).status & win_status::DESTROYED, 0);
        wm.update(&mut h);
        assert!(wm.ext[d].freed);
    }
}
