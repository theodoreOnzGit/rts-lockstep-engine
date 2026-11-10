// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishMapSelectMenu.cpp (the file's statics,
//     NullifyControls, mapListTooltipFunc, showSkirmishGameOptionsUnderlyingGUIElements, SkirmishMapSelectMenuInit,
//     SkirmishMapSelectMenuShutdown, SkirmishMapSelectMenuUpdate, SkirmishMapSelectMenuInput,
//     SkirmishMapSelectMenuSystem)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   SkirmishMapSelectMenu.cpp: Chris Brue, August 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The skirmish map picker (GitHub #89): `Menus/SkirmishMapSelectMenu.wnd`'s callbacks, the layout the skirmish
//! options screen opens over itself (*Select Map*).
//!
//! # As upstream reads it
//!
//! **Init** gives `SkrimishMapSelectMenuParent` the focus; picks the shipped or the user's maps (`Network.ini`'s
//! `UseSystemMapDir`, overruled by the current map being official or not) and selects that radio button; hides the
//! eight start-spot buttons of the preview (`WinMapPreview`), hides the options screen's gadgets under it
//! (`showSkirmishGameOptionsUnderlyingGUIElements(FALSE)`), and fills `ListboxMap` (`populateMapListbox`: the shipped
//! multiplayer maps, or the user's solo then multiplayer maps), with its tooltip. **System**: selecting a row shows the
//! map's preview and start spots (`positionStartSpots`); a double click presses *OK*; the radio buttons refill the
//! list; *OK* sets the game's map (its CRC and size from the cache, every slot's start spot cleared, the options
//! screen's map name), shows the options screen's gadgets again, places its start spots, refreshes its slot list and
//! destroys this layout; *Back* shows the gadgets, destroys the layout and places the start spots. **Input**: Escape's
//! release presses *Back*. **Shutdown** hides the layout and tells the shell; **Update** is empty.
//!
//! # Upstream quirks that are kept
//!
//! - The parent's name is misspelt in the code (`SkrimishMapSelectMenuParent`): a file whose parent is spelt right is
//!   not found, and the focus goes nowhere (`winSetFocus(NULL)`).
//! - A row's map name is read from column 0 (`GadgetListBoxGetText(mapWindow, row, 0)`), the star's column when the
//!   list has two: it is used only when the row has no item data, and *OK*'s fallback translates an empty string
//!   (`map` is never set there).
//! - *Back* places the start spots after the layout is gone; *OK* sets the map without `TheSkirmishGameInfo`'s
//!   `NULL` check.
//! - The radio buttons do not write `UseSystemMapDir` (commented out upstream).
//! - `SkirmishMapSelectMenuShutdown` is never called by the game (the layout is not on the shell's stack: *OK* and
//!   *Back* destroy it): translated as the lexicon names it.
//! - `showSkirmishGameOptionsUnderlyingGUIElements` dereferences each of the options screen's gadgets unchecked
//!   (recorded as undefined when one is missing); without the options screen's parent it does nothing.
//!
//! # Where this differs from upstream
//!
//! - `LANPreferences` (`Network.ini`) is the host's map ([`crate::client::skirmish_game_options::SkirmishHost::prefs_load`]).
//! - The map cache's update and the list's item data are the host's (see [`crate::client::map_util`]).
//! - `mapListTooltipFunc` sets the mouse's tooltip on the window manager ([`GameWindowManager::mouse`]).
//!
//! # Representation
//!
//! [`SkirmishMapSelect`]: the file's statics. `skirmishMapSelectLayout` is the options screen's
//! ([`crate::client::skirmish_game_options::SkirmishGameOptions::skirmish_map_select_layout`]).

use crate::client::gadget::radio_set_selection;
use crate::client::gadget::static_text_set_text;
use crate::client::gadget_list_box as lb;
use crate::client::map_util::{get_map_preview_image, populate_map_listbox, populate_map_listbox_no_reset};
use crate::client::skirmish_game_options::{self as sgo, SkirmishHost};
use crate::client::shell;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, UserData, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::common::game_info::MAX_SLOTS;

const FILE: &str = "SkirmishMapSelectMenu.wnd";

/// SkirmishMapSelectMenu.cpp's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkirmishMapSelect {
    pub button_back: i32,
    pub button_ok: i32,
    pub listbox_map: i32,
    pub parent: Option<WinId>,
    pub map_list: Option<WinId>,
    pub radio_button_system_maps_id: i32,
    pub radio_button_user_maps_id: i32,
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    pub win_map_preview: Option<WinId>,
    pub win_map_preview_id: i32,
}

fn key_of<H: SkirmishHost>(h: &mut H, name: &str) -> i32 {
    h.name_to_key(format!("{FILE}:{name}").as_bytes()) as i32
}

/// `NullifyControls()`.
fn nullify_controls<H: SkirmishHost>(h: &mut H) {
    let m = h.map_select();
    m.map_list = None;
    m.win_map_preview = None;
    m.parent = None;
    m.button_map_start_position = [None; MAX_SLOTS];
}

/// `mapListTooltipFunc(window, instData, mouse)`: the medal's tooltip of the row under the mouse (its column 1 item
/// data, 0 to 4), else none.
pub fn map_list_tooltip_func<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    let x = i32::from(mouse as u16 as i16);
    let y = i32::from((mouse >> 16) as u16 as i16);
    let (row, col) = lb::get_entry_based_on_xy(wm, h, window, x, y).unwrap_or((-1, -1));
    if row == -1 || col == -1 {
        wm.mouse.set_cursor_tooltip(&[], -1);
        return;
    }
    let image_item_data = lb::get_item_data(wm, h, window, row, 1) as i32;
    let label: Option<&[u8]> = match image_item_data {
        0 => Some(b"TOOLTIP:MapNoSuccess"),
        1 => Some(b"TOOLTIP:MapEasySuccess"),
        2 => Some(b"TOOLTIP:MapMediumSuccess"),
        3 => Some(b"TOOLTIP:MapHardSuccess"),
        4 => Some(b"TOOLTIP:MapMaxBrutalSuccess"),
        _ => None,
    };
    let tooltip = label.map_or_else(Vec::new, |l| h.fetch_text(l));
    wm.mouse.set_cursor_tooltip(&tooltip, -1);
}

/// The options screen's gadgets `showSkirmishGameOptionsUnderlyingGUIElements` hides (`winHide`), in its order.
const UNDERLYING_HIDDEN: [&str; 30] = [
    "MapWindow", "StaticTextTeam", "StaticTextFaction", "StaticTextColor", "ComboBoxTeam0", "ComboBoxTeam1", "ComboBoxTeam2", "ComboBoxTeam3", "ComboBoxTeam4", "ComboBoxTeam5",
    "ComboBoxTeam6", "ComboBoxTeam7", "ComboBoxColor0", "ComboBoxColor1", "ComboBoxColor2", "ComboBoxColor3", "ComboBoxColor4", "ComboBoxColor5", "ComboBoxColor6", "ComboBoxColor7",
    "ComboBoxPlayerTemplate0", "ComboBoxPlayerTemplate1", "ComboBoxPlayerTemplate2", "ComboBoxPlayerTemplate3", "ComboBoxPlayerTemplate4", "ComboBoxPlayerTemplate5",
    "ComboBoxPlayerTemplate6", "ComboBoxPlayerTemplate7", "TextEntryMapDisplay", "ButtonSelectMap",
];

/// `showSkirmishGameOptionsUnderlyingGUIElements(show)`: the options screen's map, labels, per-slot combo boxes, map
/// name and buttons shown or hidden; *Reset* and *Back* enabled or disabled.
pub fn show_skirmish_game_options_underlying_gui_elements<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    let parent_id = h.name_to_key(b"SkirmishGameOptionsMenu.wnd:SkirmishGameOptionsMenuParent") as i32;
    let Some(parent) = wm.win_get_window_from_id(None, parent_id) else { return };
    let find = |wm: &mut GameWindowManager, h: &mut H, name: &str| {
        let id = h.name_to_key(format!("SkirmishGameOptionsMenu.wnd:{name}").as_bytes()) as i32;
        let w = wm.win_get_window_from_id(Some(parent), id);
        if w.is_none() {
            wm.set_undefined("showSkirmishGameOptionsUnderlyingGUIElements: a gadget of the options screen is missing (winHide on NULL)");
        }
        w
    };
    for n in UNDERLYING_HIDDEN.iter().chain(["ButtonStart", "StaticTextMapPreview"].iter()) {
        let Some(w) = find(wm, h, n) else { return };
        wm.win_hide(w, !show);
    }
    for n in ["ButtonReset", "ButtonBack"] {
        let Some(w) = find(wm, h, n) else { return };
        wm.win_enable(w, show);
    }
}

/// The map list filled for the shipped maps (`TRUE, TRUE`) or the user's (`FALSE, FALSE` then `FALSE, TRUE`), after
/// `TheMapCache->updateCache()`.
fn fill_map_list<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, system: bool) {
    let map_list = h.map_select().map_list;
    let map = h.skirmish().game.as_ref().map(|g| g.map_name.clone());
    let Some(map) = map else {
        wm.set_undefined("SkirmishMapSelectMenu: TheSkirmishGameInfo is NULL (getMap)");
        return;
    };
    h.update_map_cache();
    if system {
        populate_map_listbox(wm, h, map_list, true, true, &map);
    } else {
        populate_map_listbox(wm, h, map_list, false, false, &map);
        populate_map_listbox_no_reset(wm, h, map_list, false, true, &map);
    }
}

/// `SkirmishMapSelectMenuInit(layout, userData)`.
pub fn skirmish_map_select_menu_init<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = key_of(h, "SkrimishMapSelectMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.map_select().parent = parent;
    wm.win_set_focus(h, parent);
    let pref = crate::client::user_preferences::load_preferences(wm, h, b"Network.ini");
    let mut uses_system_map_dir = pref.uses_system_map_dir();
    let Some(map) = h.skirmish().game.as_ref().map(|g| g.map_name.clone()) else {
        wm.set_undefined("SkirmishMapSelectMenuInit: TheSkirmishGameInfo is NULL (getMap)");
        return;
    };
    if let Some(m) = h.util_data().map_cache.find_map(&map) {
        uses_system_map_dir = m.is_official;
    }
    let preview_id = key_of(h, "WinMapPreview");
    let preview = wm.win_get_window_from_id(parent, preview_id);
    let ids = [key_of(h, "ButtonBack"), key_of(h, "ButtonOK"), key_of(h, "ListboxMap"), key_of(h, "RadioButtonSystemMaps"), key_of(h, "RadioButtonUserMaps")];
    let m = h.map_select();
    m.win_map_preview_id = preview_id;
    m.win_map_preview = preview;
    m.button_back = ids[0];
    m.button_ok = ids[1];
    m.listbox_map = ids[2];
    m.radio_button_system_maps_id = ids[3];
    m.radio_button_user_maps_id = ids[4];
    let radio_system = wm.win_get_window_from_id(parent, ids[3]);
    let radio_user = wm.win_get_window_from_id(parent, ids[4]);
    if let Some(r) = if uses_system_map_dir { radio_system } else { radio_user } {
        radio_set_selection(wm, h, r, false);
    }
    for i in 0..MAX_SLOTS {
        let id = key_of(h, &format!("ButtonMapStartPosition{i}"));
        let b = wm.win_get_window_from_id(preview, id);
        let m = h.map_select();
        m.button_map_start_position_id[i] = id;
        m.button_map_start_position[i] = b;
        let Some(b) = b else {
            wm.set_undefined("SkirmishMapSelectMenuInit: a ButtonMapStartPosition is missing (winHide on NULL)");
            return;
        };
        wm.win_hide(b, true);
        wm.win_enable(b, false);
    }
    show_skirmish_game_options_underlying_gui_elements(wm, h, false);
    let list_id = key_of(h, "ListboxMap");
    let map_list = wm.win_get_window_from_id(parent, list_id);
    h.map_select().map_list = map_list;
    if let Some(l) = map_list {
        fill_map_list(wm, h, uses_system_map_dir);
        wm.ext[l].tooltip = Some(b"mapListTooltipFunc".to_vec());
    }
}

/// `SkirmishMapSelectMenuShutdown(layout, userData)`.
pub fn skirmish_map_select_menu_shutdown<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    nullify_controls(h);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `SkirmishMapSelectMenuUpdate(layout, userData)`: empty.
pub fn skirmish_map_select_menu_update<H: SkirmishHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `SkirmishMapSelectMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Back*.
pub fn skirmish_map_select_menu_input<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let id = key_of(h, "ButtonBack");
        let button = wm.win_get_window_from_id(Some(window), id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    MsgHandled::Handled
}

/// `skirmishMapSelectLayout->destroyWindows(); deleteInstance(); skirmishMapSelectLayout = NULL;`.
fn destroy_map_select_layout<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H) {
    match h.skirmish().skirmish_map_select_layout.take() {
        Some(l) => {
            wm.layout_destroy_windows(l);
            wm.layouts[l].deleted = true;
        }
        None => wm.set_undefined("SkirmishMapSelectMenuSystem: skirmishMapSelectLayout is NULL (destroyWindows)"),
    }
}

/// The row's map file (its item data), else the text translated (`asciiMap.translate(map)`).
fn row_map<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, list: WinId, row: i32, text: &[u16]) -> Vec<u8> {
    let data = lb::get_item_data(wm, h, list, row, 0);
    match h.map_list_name(data) {
        Some(n) => n,
        // (`DEBUG_ASSERTCRASH`: nothing in the release) `AsciiString::translate`: each unit's low byte
        None => text.iter().map(|&c| c as u8).collect(),
    }
}

/// `SkirmishMapSelectMenuSystem(window, msg, mData1, mData2)`.
pub fn skirmish_map_select_menu_system<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let control_id = |wm: &GameWindowManager| data_win(d1).map(|c| (c, wm.tree.windows[c].id));
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => nullify_controls(h),
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        lb::glm::DOUBLE_CLICKED => {
            let Some((control, id)) = control_id(wm) else { return MsgHandled::Handled };
            if id == h.map_select().listbox_map {
                let row_selected = d2 as i32;
                if row_selected >= 0 {
                    lb::set_selected(wm, h, control, &[row_selected]);
                    let ok = h.map_select().button_ok;
                    let button = wm.win_get_window_from_id(Some(window), ok);
                    wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), ok as u32);
                }
            }
        }
        ggm::GLM_SELECTED => {
            let m = h.map_select().clone();
            let map_window = wm.win_get_window_from_id(m.parent, m.listbox_map);
            let Some((_, id)) = control_id(wm) else { return MsgHandled::Handled };
            if id != m.listbox_map {
                return MsgHandled::Handled;
            }
            let row_selected = d2 as i32;
            if row_selected < 0 {
                sgo::position_start_spots(wm, h, b"", &m.button_map_start_position, m.win_map_preview);
                return MsgHandled::Handled;
            }
            let Some(preview) = m.win_map_preview else {
                wm.set_undefined("SkirmishMapSelectMenuSystem: WinMapPreview is NULL (winSetStatus)");
                return MsgHandled::Handled;
            };
            wm.win_set_status(preview, win_status::IMAGE);
            let Some(mw) = map_window else {
                wm.set_undefined("SkirmishMapSelectMenuSystem: ListboxMap is not under the parent (GadgetListBoxGetText of NULL)");
                return MsgHandled::Handled;
            };
            let map = lb::get_text(wm, h, mw, row_selected, 0);
            let ascii_map = row_map(wm, h, mw, row_selected, &map).to_ascii_lowercase();
            let image = get_map_preview_image(h, &ascii_map);
            let found = h.util_data().map_cache.find_map(&ascii_map).map(|_| ascii_map.clone());
            wm.ext[preview].user = UserData::MapMetaData(found);
            match image {
                Some(i) => wm.tree.windows[preview].inst.enabled_draw_data[0].image = Some(i),
                None => {
                    wm.win_clear_status(preview, win_status::IMAGE);
                }
            }
            sgo::position_start_spots(wm, h, &ascii_map, &m.button_map_start_position, m.win_map_preview);
        }
        ggm::GBM_SELECTED => {
            let Some((_, id)) = control_id(wm) else { return MsgHandled::Handled };
            let m = h.map_select().clone();
            if id == m.radio_button_system_maps_id {
                fill_map_list(wm, h, true);
            } else if id == m.radio_button_user_maps_id {
                fill_map_list(wm, h, false);
            } else if id == m.button_back {
                show_skirmish_game_options_underlying_gui_elements(wm, h, true);
                destroy_map_select_layout(wm, h);
                sgo::skirmish_position_start_spots(wm, h);
            } else if id == m.button_ok {
                let Some(map_window) = wm.win_get_window_from_id(m.parent, m.listbox_map) else {
                    wm.set_undefined("SkirmishMapSelectMenuSystem: ListboxMap is not under the parent (GadgetListBoxGetSelected of NULL)");
                    return MsgHandled::Handled;
                };
                let selected = lb::get_selected(wm, h, map_window).first().copied().unwrap_or(-1);
                if selected != -1 {
                    // (`map` is never set here: the fallback translates an empty string)
                    let ascii_map = row_map(wm, h, map_window, selected, &[]);
                    let (o, d) = h.skirmish_split();
                    let Some(g) = o.game.as_mut() else {
                        wm.set_undefined("SkirmishMapSelectMenuSystem: TheSkirmishGameInfo is NULL (setMap)");
                        return MsgHandled::Handled;
                    };
                    let md = d.map_cache.find_map(&ascii_map).map(|m| (m.crc, m.filesize, m.display_name.str().to_vec()));
                    let r = g.set_map(&ascii_map, d).and_then(|()| {
                        let (crc, size) = md.as_ref().map_or((0, 0), |m| (m.0, m.1));
                        g.set_map_crc(crc, d)?;
                        g.set_map_size(size, d)
                    });
                    if r.is_err() {
                        wm.set_undefined("SkirmishMapSelectMenuSystem: setMap / setMapCRC / setMapSize");
                        return MsgHandled::Handled;
                    }
                    for s in g.slots.iter_mut() {
                        s.start_pos = -1;
                    }
                    let display_id = h.name_to_key(b"SkirmishGameOptionsMenu.wnd:TextEntryMapDisplay") as i32;
                    let win = wm.win_get_window_from_id(m.parent, display_id);
                    if let (Some(win), Some(md)) = (win, md) {
                        static_text_set_text(wm, h, Some(win), &md.2);
                    }
                    show_skirmish_game_options_underlying_gui_elements(wm, h, true);
                    sgo::skirmish_position_start_spots(wm, h);
                    sgo::skirmish_update_slot_list(wm, h);
                    destroy_map_select_layout(wm, h);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::map_util::MapUtilHost;
    use crate::client::save_load_menu::tests::{click, enabled, hidden, named, H};
    use crate::client::skirmish_game_options::tests::{opened, ALPHA, BETA, F};
    use crate::client::window_manager::short_to_long;

    const M: &str = "SkirmishMapSelectMenu.wnd";

    /// The newest window `SkirmishMapSelectMenu.wnd:<name>` (the picker's earlier openings' windows are freed, not
    /// removed).
    fn m(wm: &GameWindowManager, name: &str) -> WinId {
        let full = format!("{M}:{name}");
        wm.tree.windows.iter().rposition(|w| w.name() == full.as_bytes()).unwrap_or_else(|| panic!("{name}"))
    }

    fn open(wm: &mut GameWindowManager, h: &mut H) -> WinId {
        let b = named(wm, F, "ButtonSelectMap");
        click(wm, h, b);
        m(wm, "SkrimishMapSelectMenuParent")
    }

    fn row_text(wm: &mut GameWindowManager, h: &mut H, l: WinId, row: i32) -> String {
        String::from_utf16_lossy(&lb::get_text(wm, h, l, row, 1))
    }

    /// #89: the map picker over the skirmish screen (opened on Beta), predicted from SkirmishMapSelectMenu.cpp and
    /// MapUtil.cpp before the run:
    ///
    /// - **init**: the shipped maps (Beta is official) and their radio button selected; the preview's eight start
    ///   buttons hidden and disabled; the options screen's map, labels, per-slot boxes, map name, *Select Map*,
    ///   *Start* and preview label hidden, *Reset* and *Back* disabled; the list's two columns: Alpha (2) then
    ///   Beta (4) (by player count; Solo is not multiplayer), each with the empty star (item data 0) and its file as
    ///   the item data; Beta's row (1) selected; the tooltip over row 0 is `TOOLTIP:MapNoSuccess`;
    /// - **a row selected** (Alpha): the preview's image cleared (no preview image: the copy from the archive is
    ///   recorded as untranslated, and no `UnknownMap`), its user data Alpha; its start buttons 0 and 1 shown at
    ///   (12, 122) and (14, 122) in it: the second is **not** nudged off the first, as upstream's check compares it
    ///   with the first's screen position; the supply dock's marker at (93, 93), the tech building's at (33, 103);
    ///   a row of -1: no map, the buttons hidden;
    /// - **a double click** on Alpha presses *OK*: the game's map Alpha (CRC 0x1234, size 99), every start cleared, the
    ///   options screen's map name `Alpha (2)`, its gadgets shown again and its start buttons 0 and 1 placed (2 to 7
    ///   hidden), the picker's layout destroyed and its controls forgotten (`GWM_DESTROY`);
    /// - opened again, *Back* only closes it; again, Escape's release does the same; again, the user's maps: none in
    ///   `User\Maps`, an empty list whose "selected" row 0 makes the list box read `listData[-1]` (undefined).
    ///
    /// **Result (2026-11-10):** four misses. (1) **A defect**: the list had four rows, not two: the translation passed
    /// `overwrite` `FALSE` to `GadgetListBoxAddEntryText`, whose default upstream is `TRUE`, so each name went into a
    /// row of its own after the star's (fixed in `map_util`; issue drafted). (2) After the init Beta's four start
    /// buttons are shown (still disabled): the list's selection of Beta's row sends `GLM_SELECTED` to the picker
    /// (GadgetListBox.cpp's `GLM_SET_SELECTION` does), which places them; upstream does the same, the prediction
    /// missed it. (3) The picker's controls are forgotten (`GWM_DESTROY`) on the next frame's `processDestroyList`,
    /// not at *OK*. (4) The test found the first opening's freed windows by name (it now takes the newest).
    #[test]
    fn the_map_picker_lists_previews_and_sets_the_map() {
        let (mut wm, mut h) = opened();
        let p = open(&mut wm, &mut h);
        let l = h.skirmish.skirmish_map_select_layout.expect("the picker's layout");
        assert!(!wm.layouts[l].hidden);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let radio = m(&wm, "RadioButtonSystemMaps");
        assert!(wm.tree.windows[radio].inst.state & crate::client::window_manager::WIN_STATE_SELECTED != 0);
        // (the list's selection of Beta's row tells the screen, `GLM_SELECTED`, which places Beta's four spots: shown,
        // still disabled; see the result below)
        for i in 0..8 {
            let b = m(&wm, &format!("ButtonMapStartPosition{i}"));
            assert_eq!(hidden(&wm, b), i >= 4, "{i}");
            assert!(!enabled(&wm, b));
        }
        assert_eq!(wm.ext[m(&wm, "WinMapPreview")].user, UserData::MapMetaData(Some(BETA.to_vec())));
        for n in ["MapWindow", "StaticTextTeam", "ComboBoxColor3", "TextEntryMapDisplay", "ButtonSelectMap", "ButtonStart", "StaticTextMapPreview"] {
            assert!(hidden(&wm, named(&wm, F, n)), "{n}");
        }
        assert!(!enabled(&wm, named(&wm, F, "ButtonReset")) && !enabled(&wm, named(&wm, F, "ButtonBack")));
        let list = m(&wm, "ListboxMap");
        assert_eq!(lb::get_num_entries(&wm, list), 2);
        assert_eq!((row_text(&mut wm, &mut h, list, 0), row_text(&mut wm, &mut h, list, 1)), ("Alpha (2)".to_string(), "Beta (4)".to_string()));
        for (row, file) in [(0, ALPHA), (1, BETA)] {
            let d = lb::get_item_data(&mut wm, &mut h, list, row, 0);
            assert_eq!(h.map_list_name(d).as_deref(), Some(file));
            assert_eq!(lb::get_item_data(&mut wm, &mut h, list, row, 1), 0);
        }
        assert_eq!(lb::get_selected(&mut wm, &mut h, list), [1]);
        let (lx, ly) = wm.tree.screen_position(list);
        map_list_tooltip_func(&mut wm, &mut h, list, short_to_long(lx + 50, ly + 3));
        assert_eq!(String::from_utf16_lossy(&wm.mouse.display_string), "T:TOOLTIP:MapNoSuccess");
        // a row selected: Alpha
        wm.win_send_system_msg(&mut h, Some(p), ggm::GLM_SELECTED, win_data(Some(list)), 0);
        let preview = m(&wm, "WinMapPreview");
        assert_eq!(wm.tree.windows[preview].status & win_status::IMAGE, 0);
        assert_eq!(wm.ext[preview].user, UserData::MapMetaData(Some(ALPHA.to_vec())));
        assert!(h.untranslated.iter().any(|f| f.starts_with(b"getMapPreviewImage")));
        let (px, py) = wm.tree.screen_position(preview);
        let b0 = m(&wm, "ButtonMapStartPosition0");
        let b1 = m(&wm, "ButtonMapStartPosition1");
        assert_eq!((wm.tree.screen_position(b0), wm.tree.screen_position(b1)), ((px + 12, py + 122), (px + 14, py + 122)));
        assert!(!hidden(&wm, b0) && !hidden(&wm, b1) && hidden(&wm, m(&wm, "ButtonMapStartPosition2")));
        assert_eq!(h.skirmish.supply_pos_list, [crate::common::base_type::ICoord2D { x: 93, y: 93 }]);
        assert_eq!(h.skirmish.tech_pos_list, [crate::common::base_type::ICoord2D { x: 33, y: 103 }]);
        wm.win_send_system_msg(&mut h, Some(p), ggm::GLM_SELECTED, win_data(Some(list)), -1i32 as u32);
        assert_eq!(wm.ext[preview].user, UserData::MapMetaData(None));
        assert!(hidden(&wm, b0) && hidden(&wm, b1));
        // a double click on Alpha: OK
        wm.win_send_system_msg(&mut h, Some(p), lb::glm::DOUBLE_CLICKED, win_data(Some(list)), 0);
        let g = h.skirmish.game.clone().expect("game");
        assert_eq!((g.map_name.as_slice(), g.map_crc, g.map_size), (ALPHA, 0x1234, 99));
        assert!(g.slots.iter().all(|s| s.start_pos == -1));
        let display = named(&wm, F, "TextEntryMapDisplay");
        assert_eq!(String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(&wm, display)), "Alpha (2)");
        assert!(!hidden(&wm, named(&wm, F, "MapWindow")) && enabled(&wm, named(&wm, F, "ButtonBack")));
        assert!(!hidden(&wm, named(&wm, F, "ButtonMapStartPosition1")) && hidden(&wm, named(&wm, F, "ButtonMapStartPosition2")));
        assert!(wm.layouts[l].deleted && h.skirmish.skirmish_map_select_layout.is_none());
        // (`GWM_DESTROY` reaches the picker's parent when the manager frees it: the next frame's `processDestroyList`)
        assert!(h.map_select.parent.is_some());
        crate::client::save_load_menu::tests::tick(&mut wm, &mut h);
        assert_eq!(h.map_select.parent, None);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // Back
        open(&mut wm, &mut h);
        let back = m(&wm, "ButtonBack");
        click(&mut wm, &mut h, back);
        assert!(h.skirmish.skirmish_map_select_layout.is_none() && !hidden(&wm, named(&wm, F, "MapWindow")));
        assert_eq!(h.skirmish.game.as_ref().map(|g| g.map_name.clone()), Some(ALPHA.to_vec()));
        // Escape
        let p = open(&mut wm, &mut h);
        wm.win_set_focus(&mut h, Some(p));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(h.skirmish.skirmish_map_select_layout.is_none());
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // the user's maps: none
        let p = open(&mut wm, &mut h);
        let user = m(&wm, "RadioButtonUserMaps");
        click(&mut wm, &mut h, user);
        let list = m(&wm, "ListboxMap");
        assert_eq!(lb::get_num_entries(&wm, list), 0);
        assert!(wm.undefined.is_some_and(|u| u.contains("listData[-1]")), "{:?}", wm.undefined);
        // the entry points the game never calls
        let l = h.skirmish.skirmish_map_select_layout.expect("the picker's layout");
        skirmish_map_select_menu_update(&mut wm, &mut h, l);
        skirmish_map_select_menu_shutdown(&mut wm, &mut h, l);
        assert!(wm.layouts[l].hidden && h.map_select.parent.is_none());
        assert_eq!(skirmish_map_select_menu_input(&mut wm, &mut h, p, gwm::CHAR, u32::from(key::ENTER), key::STATE_UP), MsgHandled::Ignored);
    }
}
