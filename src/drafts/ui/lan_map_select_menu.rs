//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 3,000 sequences (test: tests/differential_draft_lan_map_select.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanMapSelectMenu.cpp
//!     (the file's statics, `showLANGameOptionsUnderlyingGUIElements`, `NullifyControls`, `LanMapSelectMenuInit`,
//!      `LanMapSelectMenuShutdown`, `LanMapSelectMenuUpdate`, `LanMapSelectMenuInput`, `LanMapSelectMenuSystem`)
//!
//! Same shape as the skirmish picker already in the port ([`crate::client::skirmish_map_select`]); the differences
//! are `TheLAN->GetMyGame()` for the game, `PostToLanGameOptions(...)`, and that the radio buttons DO write
//! `UseSystemMapDir` here. The LAN game, the map cache and the preview are the host's.

use crate::client::gadget::radio_set_selection;
use crate::client::gadget_list_box as lb;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::common::game_info::MAX_SLOTS;
use crate::logic::modules::plant;

const FILE: &str = "LanMapSelectMenu.wnd";

/// `LanGameOptionsMenu.wnd` layout name and parent (the file's `layoutFilename`, `parentName`).
const LAYOUT_FILENAME: &str = "LanGameOptionsMenu.wnd";
const PARENT_NAME: &str = "LanGameOptionsMenuParent";
/// `gadgetsToHide`.
const GADGETS_TO_HIDE: [&str; 7] = ["MapWindow", "StaticTextTeam", "StaticTextFaction", "StaticTextColor", "TextEntryMapDisplay", "ButtonSelectMap", "ButtonStart"];
/// `gadgetsToHide` has `StaticTextMapPreview` after `ButtonStart`: kept as the eighth entry.
const GADGETS_TO_HIDE_LAST: &str = "StaticTextMapPreview";
/// `perPlayerGadgetsToHide`.
const PER_PLAYER_GADGETS_TO_HIDE: [&str; 3] = ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"];

/// `PostToLanGameOptions(...)` codes used here (LANAPICallbacks.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostToLanGameOptionsType {
    /// `MAP_BACK`.
    MapBack,
    /// `SEND_GAME_OPTS`.
    SendGameOpts,
}

/// LanMapSelectMenu.cpp's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanMapSelect {
    pub button_back: i32,
    pub button_ok: i32,
    pub listbox_map: i32,
    pub win_map_preview_id: i32,
    pub parent: Option<WinId>,
    pub map_list: Option<WinId>,
    pub win_map_preview: Option<WinId>,
    pub radio_button_system_maps_id: i32,
    pub radio_button_user_maps_id: i32,
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    /// `mapSelectLayout` (defined in LanGameOptionsMenu.cpp): a layout index.
    pub map_select_layout: Option<usize>,
}

/// What the LAN map picker reaches outside the window code.
pub trait LanMapSelectHost: ShellHost {
    fn lan_map_select(&mut self) -> &mut LanMapSelect;
    /// `LANPreferences pref; pref.usesSystemMapDir()`.
    fn lan_prefs_uses_system_map_dir(&mut self) -> bool;
    /// `LANPreferences pref; pref["UseSystemMapDir"] = yes/no; pref.write()`.
    fn lan_prefs_write_use_system_map_dir(&mut self, yes: bool);
    /// `TheLAN->GetMyGame()->getMap()`.
    fn lan_my_game_map(&self) -> Vec<u8>;
    /// `TheMapCache->findMap(map)->m_isOfficial` (`None`: not in the cache).
    fn map_cache_is_official(&self, map: &[u8]) -> Option<bool>;
    /// `TheMapCache->find(map)`: `Some((m_CRC, m_filesize))`.
    fn map_cache_find(&self, map: &[u8]) -> Option<(u32, u32)>;
    /// `TheMapCache->updateCache()`.
    fn map_cache_update(&mut self);
    /// `populateMapListbox(listbox, useSystemMaps, isMultiplayer, mapToSelect)` (the menu passes `TRUE` and `TheLAN->GetMyGame()->getMap()`).
    fn populate_map_listbox(&mut self, wm: &mut GameWindowManager, list: Option<WinId>, system: bool, is_multiplayer: bool, map_to_select: &[u8]);
    /// `(const char *)GadgetListBoxGetItemData(list, row)`: the row's map file, if it has one.
    fn map_list_name(&self, item_data: u32) -> Option<Vec<u8>>;
    /// `getMapPreviewImage(map)`: the image name; `winMapPreview->winSetUserData(TheMapCache->findMap(map))`.
    fn get_map_preview_image(&mut self, map: &[u8]) -> Option<Vec<u8>>;
    fn set_preview_user_data_map(&mut self, wm: &mut GameWindowManager, preview: WinId, map: &[u8]);
    /// `win->winSetEnabledImage(0, image)`.
    fn win_set_enabled_image0(&mut self, wm: &mut GameWindowManager, w: WinId, image: &[u8]);
    /// `positionStartSpots(mapName, buttons, mapWindow)`.
    fn position_start_spots(&mut self, wm: &mut GameWindowManager, map: &[u8], buttons: &[Option<WinId>; MAX_SLOTS], map_window: Option<WinId>);
    /// `TheLAN->GetMyGame()`: `setMap`, `getSlot(0)->setMapAvailability(true)`, `setMapCRC`, `setMapSize`,
    /// `resetStartSpots()`, `adjustSlotsForMap()`.
    fn lan_game_set_map(&mut self, map: &[u8]);
    fn lan_game_slot0_set_map_availability(&mut self, available: bool);
    fn lan_game_set_map_crc_and_size(&mut self, crc: u32, size: u32);
    fn lan_game_reset_start_spots(&mut self);
    fn lan_game_adjust_slots_for_map(&mut self);
    /// `PostToLanGameOptions(t)`.
    fn post_to_lan_game_options(&mut self, t: PostToLanGameOptionsType);
}

fn key_of<H: LanMapSelectHost>(h: &mut H, name: &str) -> i32 {
    ShellHost::name_to_key(h, format!("{FILE}:{name}").as_bytes()) as i32
}

/// `ShowUnderlyingGUIElements(show, layoutFilename, parentName, gadgetsToHide, perPlayerGadgetsToHide)`
/// (GUIUtil.cpp), already translated at [`crate::client::gui_util::show_underlying_gui_elements`]; reached through
/// the host because that function wants a `GuiUtilHost`.
pub trait LanUnderlyingGui: LanMapSelectHost {
    fn show_underlying_gui_elements(&mut self, wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]);
}

/// `showLANGameOptionsUnderlyingGUIElements(show)` (LanMapSelectMenu.cpp:79).
pub fn show_lan_game_options_underlying_gui_elements<H: LanUnderlyingGui>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    let mut gadgets: Vec<&str> = GADGETS_TO_HIDE.to_vec();
    gadgets.push(GADGETS_TO_HIDE_LAST);
    h.show_underlying_gui_elements(wm, show, LAYOUT_FILENAME, PARENT_NAME, &gadgets, &PER_PLAYER_GADGETS_TO_HIDE);
    let id = ShellHost::name_to_key(h, b"LanGameOptionsMenu.wnd:ButtonBack") as i32;
    if let Some(win) = wm.win_get_window_from_id(None, id) {
        wm.win_enable(win, show);
    }
}

/// `NullifyControls()`.
fn nullify_controls<H: LanMapSelectHost>(h: &mut H) {
    let m = h.lan_map_select();
    m.map_list = None;
    m.win_map_preview = None;
    m.parent = None;
    m.button_map_start_position = [None; MAX_SLOTS];
}

/// `LanMapSelectMenuInit(layout, userData)` (LanMapSelectMenu.cpp:103).
pub fn lan_map_select_menu_init<H: LanUnderlyingGui>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    show_lan_game_options_underlying_gui_elements(wm, h, false);

    // set keyboard focus to main parent
    let parent_id = key_of(h, "LanMapSelectMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.lan_map_select().parent = parent;
    wm.win_set_focus(h, parent);

    let mut uses_system_map_dir = h.lan_prefs_uses_system_map_dir();
    let my_map = h.lan_my_game_map();
    if let Some(official) = h.map_cache_is_official(&my_map) {
        uses_system_map_dir = if plant(23_861) { !official } else { official };
    }

    let ids = [key_of(h, "ButtonBack"), key_of(h, "ButtonOK"), key_of(h, "ListboxMap"), key_of(h, "WinMapPreview"), key_of(h, "RadioButtonSystemMaps"), key_of(h, "RadioButtonUserMaps")];
    {
        let m = h.lan_map_select();
        m.button_back = ids[0];
        m.button_ok = ids[1];
        m.listbox_map = ids[2];
        m.win_map_preview_id = ids[3];
        m.radio_button_system_maps_id = ids[4];
        m.radio_button_user_maps_id = ids[5];
    }

    let radio_system = wm.win_get_window_from_id(parent, ids[4]);
    let radio_user = wm.win_get_window_from_id(parent, ids[5]);
    let win_map_preview = wm.win_get_window_from_id(parent, ids[3]);
    h.lan_map_select().win_map_preview = win_map_preview;

    if let Some(r) = if uses_system_map_dir != plant(23_862) { radio_system } else { radio_user } {
        radio_set_selection(wm, h, r, false);
    }

    for i in 0..MAX_SLOTS {
        let id = key_of(h, &format!("ButtonMapStartPosition{i}"));
        let b = wm.win_get_window_from_id(win_map_preview, id);
        {
            let m = h.lan_map_select();
            m.button_map_start_position_id[i] = id;
            m.button_map_start_position[i] = b;
        }
        // DEBUG_ASSERTCRASH(buttonMapStartPosition[i], ...); upstream dereferences it unchecked
        if let Some(b) = b {
            wm.win_hide(b, true);
            wm.win_enable(b, false);
        } else {
            wm.set_undefined("LanMapSelectMenuInit: ButtonMapStartPosition missing (null dereference)");
        }
    }

    // get the listbox window
    let list_id = key_of(h, "ListboxMap");
    let map_list = wm.win_get_window_from_id(parent, list_id);
    h.lan_map_select().map_list = map_list;
    if map_list.is_some() {
        if !plant(23_863) {
            h.map_cache_update();
        }
        let map = h.lan_my_game_map();
        h.populate_map_listbox(wm, map_list, uses_system_map_dir, true, &map);
    }
}

/// `LanMapSelectMenuShutdown(layout, userData)`.
pub fn lan_map_select_menu_shutdown<H: LanMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    // hide menu
    if wm.layouts[layout].deleted {
        wm.set_undefined("LanMapSelectMenuShutdown: the layout was deleted (use after free)");
    }
    if !plant(23_872) {
        wm.layout_hide(layout, true);
    }
    nullify_controls(h);

    // our shutdown is complete
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `LanMapSelectMenuUpdate(layout, userData)`: empty.
pub fn lan_map_select_menu_update<H: LanMapSelectHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `LanMapSelectMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Back*.
pub fn lan_map_select_menu_input<H: LanMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & (if plant(23_866) { key::STATE_DOWN } else { key::STATE_UP }) != 0 {
        let id = key_of(h, "ButtonBack");
        let button = wm.win_get_window_from_id(Some(window), id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    MsgHandled::Handled
}

/// `mapSelectLayout->destroyWindows(); mapSelectLayout->deleteInstance(); mapSelectLayout = NULL;`.
fn destroy_map_select_layout<H: LanMapSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    match h.lan_map_select().map_select_layout.take() {
        Some(l) if !wm.layouts[l].deleted => {
            wm.layout_destroy_windows(l);
            wm.layouts[l].deleted = true;
        }
        // `mapSelectLayout` is NULL (or deleted): upstream calls through it
        _ => wm.set_undefined("LanMapSelectMenuSystem: mapSelectLayout is NULL (null dereference)"),
    }
}

/// The row's map file (its item data), else the text translated (`asciiMap.translate(map)`).
fn row_map<H: LanMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, list: Option<WinId>, row: i32, text: &[u16]) -> Vec<u8> {
    // (`GadgetListBoxGetItemData(NULL, ...)` is NULL)
    let data = list.map_or(0, |l| lb::get_item_data(wm, h, l, row, 0));
    match h.map_list_name(data) {
        Some(n) => n,
        // `asciiMap.translate(map)`: each unit's low byte appended, a NUL appending nothing (`concat((char)c)`)
        None => text.iter().map(|&c| c as u8).filter(|&b| b != 0).collect(),
    }
}

/// `LanMapSelectMenuSystem(window, msg, mData1, mData2)` (LanMapSelectMenu.cpp:290).
pub fn lan_map_select_menu_system<H: LanUnderlyingGui>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let m0 = h.lan_map_select().clone();
    let mut map_window = None;
    if m0.listbox_map != 0 {
        map_window = wm.win_get_window_from_id(m0.parent, m0.listbox_map);
    }
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
            let Some((control, id)) = control_id(wm) else {
                wm.set_undefined("LanMapSelectMenuSystem: a NULL control (null dereference)");
                return MsgHandled::Handled;
            };
            if id == m0.listbox_map {
                let row_selected = d2 as i32;
                if row_selected >= 0 {
                    lb::set_selected(wm, h, control, &[row_selected]);
                    let button = wm.win_get_window_from_id(Some(window), m0.button_ok);
                    wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), m0.button_ok as u32);
                }
            }
        }
        ggm::GBM_SELECTED => {
            let Some((_, id)) = control_id(wm) else {
                wm.set_undefined("LanMapSelectMenuSystem: a NULL control (null dereference)");
                return MsgHandled::Handled;
            };
            if id == m0.radio_button_system_maps_id {
                h.map_cache_update();
                let map = h.lan_my_game_map();
                h.populate_map_listbox(wm, m0.map_list, true, true, &map);
                h.lan_prefs_write_use_system_map_dir(!plant(23_864));
            } else if id == m0.radio_button_user_maps_id {
                h.map_cache_update();
                let map = h.lan_my_game_map();
                h.populate_map_listbox(wm, m0.map_list, plant(23_865), true, &map);
                h.lan_prefs_write_use_system_map_dir(false);
            } else if id == m0.button_back {
                destroy_map_select_layout(wm, h);
                nullify_controls(h);
                show_lan_game_options_underlying_gui_elements(wm, h, true);
                h.post_to_lan_game_options(if plant(23_869) { PostToLanGameOptionsType::SendGameOpts } else { PostToLanGameOptionsType::MapBack });
            } else if id == m0.button_ok {
                let mut selected = -1;
                if let Some(mw) = map_window {
                    selected = lb::get_selected(wm, h, mw).first().copied().unwrap_or(-1);
                }
                if selected != -1 {
                    let mw = map_window.expect("selected implies the list");
                    let map = lb::get_text(wm, h, mw, selected, 0);
                    let ascii_map = row_map(wm, h, Some(mw), selected, &map);
                    h.lan_game_set_map(&ascii_map);
                    let ascii_map = if plant(23_867) { ascii_map } else { ascii_map.to_ascii_lowercase() }; // asciiMap.toLower()
                    if let Some((crc, size)) = h.map_cache_find(&ascii_map) {
                        h.lan_game_slot0_set_map_availability(true);
                        h.lan_game_set_map_crc_and_size(crc, size);
                        if !plant(23_868) {
                            h.lan_game_reset_start_spots();
                        }
                        h.lan_game_adjust_slots_for_map(); // BGC- adjust the slots for the new map.
                    }

                    destroy_map_select_layout(wm, h);
                    nullify_controls(h);
                    show_lan_game_options_underlying_gui_elements(wm, h, true);
                    h.post_to_lan_game_options(PostToLanGameOptionsType::SendGameOpts);
                }
            }
        }
        ggm::GLM_SELECTED => {
            let Some((_, id)) = control_id(wm) else {
                wm.set_undefined("LanMapSelectMenuSystem: a NULL control (null dereference)");
                return MsgHandled::Handled;
            };
            if id != m0.listbox_map {
                return MsgHandled::Handled;
            }
            let row_selected = d2 as i32;
            if row_selected < 0 {
                let old = h.lan_my_game_map();
                h.position_start_spots(wm, if plant(23_871) { &old } else { b"" }, &m0.button_map_start_position, m0.win_map_preview);
                return MsgHandled::Handled;
            }
            let Some(preview) = m0.win_map_preview else {
                wm.set_undefined("LanMapSelectMenuSystem: winMapPreview is NULL (null dereference)");
                return MsgHandled::Handled;
            };
            wm.win_set_status(preview, win_status::IMAGE);
            // (`GadgetListBoxGetText(NULL, ...)` is the empty text)
            let map = map_window.map_or_else(Vec::new, |mw| lb::get_text(wm, h, mw, row_selected, 0));
            let ascii_map = row_map(wm, h, map_window, row_selected, &map).to_ascii_lowercase();
            let image = h.get_map_preview_image(&ascii_map);
            h.set_preview_user_data_map(wm, preview, &ascii_map);
            match image {
                Some(i) => h.win_set_enabled_image0(wm, preview, &i),
                None => {
                    if !plant(23_870) {
                        wm.win_clear_status(preview, win_status::IMAGE);
                    }
                }
            }
            h.position_start_spots(wm, &ascii_map, &m0.button_map_start_position, m0.win_map_preview);
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
