//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLMapSelectMenu.cpp
//!     (the file's statics, `NullifyControls`, `showGameSpyGameOptionsUnderlyingGUIElements`,
//!      `WOLMapSelectMenuInit/Shutdown/Update/Input/System`)
//!
//! Same shape as [`crate::drafts::ui::lan_map_select_menu`]; the game is `TheGameSpyGame`, the options screen
//! is GameSpyGameOptionsMenu, the preferences are `CustomMatchPreferences`, and stats games (`getUseStats()`)
//! may only pick shipped maps. The GameSpy service is gone: its calls are the host's.

use crate::client::gadget::radio_set_selection;
use crate::client::gadget_list_box as lb;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::common::game_info::MAX_SLOTS;

const FILE: &str = "WOLMapSelectMenu.wnd";
const LAYOUT_FILENAME: &str = "GameSpyGameOptionsMenu.wnd";
const PARENT_NAME: &str = "GameSpyGameOptionsMenuParent";
/// `gadgetsToHide`.
const GADGETS_TO_HIDE: [&str; 8] = ["MapWindow", "StaticTextTeam", "StaticTextFaction", "StaticTextColor", "TextEntryMapDisplay", "ButtonSelectMap", "ButtonStart", "StaticTextMapPreview"];
/// `perPlayerGadgetsToHide`.
const PER_PLAYER_GADGETS_TO_HIDE: [&str; 3] = ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"];

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`); `WOLMapSelectLayout` is a global.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolMapSelect {
    pub button_back: i32,
    pub button_ok: i32,
    pub listbox_map: i32,
    pub parent: Option<WinId>,
    pub raise_message_boxes: bool,
    pub win_map_preview: Option<WinId>,
    pub win_map_preview_id: i32,
    pub radio_button_system_maps_id: i32,
    pub radio_button_user_maps_id: i32,
    pub map_list: Option<WinId>,
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    pub win_map_window: Option<WinId>,
    /// `WOLMapSelectLayout` (a layout index).
    pub wol_map_select_layout: Option<usize>,
}

/// What the GameSpy map picker reaches outside the window code.
pub trait WolMapSelectHost: ShellHost {
    fn wol_map_select(&mut self) -> &mut WolMapSelect;
    /// `CustomMatchPreferences pref; pref.usesSystemMapDir()` / `pref.setUsesSystemMapDir(v); pref.write()`.
    fn custom_prefs_uses_system_map_dir(&mut self) -> bool;
    fn custom_prefs_set_uses_system_map_dir_and_write(&mut self, v: bool);
    /// `TheGameSpyGame->getMap()`.
    fn game_spy_game_map(&self) -> Vec<u8>;
    /// `TheGameSpyInfo->getCurrentStagingRoom()->getUseStats()`.
    fn current_staging_room_use_stats(&self) -> bool;
    fn map_cache_is_official(&self, map: &[u8]) -> Option<bool>;
    fn map_cache_find(&self, lower: &[u8]) -> Option<(u32, u32)>;
    fn map_cache_update(&mut self);
    fn populate_map_listbox(&mut self, wm: &mut GameWindowManager, list: Option<WinId>, system: bool);
    fn map_list_name(&self, item_data: u32) -> Option<Vec<u8>>;
    fn get_map_preview_image(&mut self, map: &[u8]) -> Option<Vec<u8>>;
    fn set_preview_user_data_map(&mut self, wm: &mut GameWindowManager, preview: WinId, map: &[u8]);
    fn win_set_enabled_image0(&mut self, wm: &mut GameWindowManager, w: WinId, image: &[u8]);
    fn position_start_spots(&mut self, wm: &mut GameWindowManager, map: &[u8], buttons: &[Option<WinId>; MAX_SLOTS], map_window: Option<WinId>);
    /// `ShowUnderlyingGUIElements(show, layoutFilename, parentName, gadgetsToHide, perPlayerGadgetsToHide)`.
    fn show_underlying_gui_elements(&mut self, wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]);
    /// `TheGameSpyGame`: `setMap`, slot 0's `setMapAvailability`, `setMapCRC`, `setMapSize`, `adjustSlotsForMap`,
    /// `resetAccepted`, `resetStartSpots`; `TheGameSpyInfo->setGameOptions()`.
    fn game_spy_game_set_map(&mut self, map: &[u8]);
    fn game_spy_game_slot0_set_map_availability(&mut self, v: bool);
    fn game_spy_game_set_map_crc_and_size(&mut self, crc: u32, size: u32);
    fn game_spy_game_adjust_slots_for_map(&mut self);
    fn game_spy_game_reset_accepted(&mut self);
    fn game_spy_game_reset_start_spots(&mut self);
    fn game_spy_info_set_game_options(&mut self);
    /// `WOLDisplaySlotList()`, `WOLDisplayGameOptions()`, `WOLPositionStartSpots()` (WOLGameSetupMenu.cpp).
    fn wol_display_slot_list(&mut self, wm: &mut GameWindowManager);
    fn wol_display_game_options(&mut self, wm: &mut GameWindowManager);
    fn wol_position_start_spots(&mut self, wm: &mut GameWindowManager);
    /// `RaiseGSMessageBox()`.
    fn raise_gs_message_box(&mut self);
}

fn key_of<H: WolMapSelectHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("{FILE}:{n}").as_bytes()) as i32
}

/// `NullifyControls()`.
fn nullify_controls<H: WolMapSelectHost>(h: &mut H) {
    let m = h.wol_map_select();
    m.parent = None;
    m.win_map_preview = None;
    m.map_list = None;
    m.button_map_start_position = [None; MAX_SLOTS];
}

/// `showGameSpyGameOptionsUnderlyingGUIElements(show)`.
fn show_game_spy_game_options_underlying_gui_elements<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    h.show_underlying_gui_elements(wm, show, LAYOUT_FILENAME, PARENT_NAME, &GADGETS_TO_HIDE, &PER_PLAYER_GADGETS_TO_HIDE);
    let id = ShellHost::name_to_key(h, b"GameSpyGameOptionsMenu.wnd:ButtonBack") as i32;
    if let Some(win) = wm.win_get_window_from_id(None, id) {
        wm.win_enable(win, show);
    }
}

/// `WOLMapSelectMenuInit(layout, userData)` (WOLMapSelectMenu.cpp:100).
pub fn wol_map_select_menu_init<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = key_of(h, "WOLMapSelectMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.wol_map_select().parent = parent;
    wm.win_set_focus(h, parent);

    let mut uses_system_map_dir = h.custom_prefs_uses_system_map_dir();

    let preview_id = key_of(h, "WinMapPreview");
    let preview = wm.win_get_window_from_id(parent, preview_id);
    h.wol_map_select().win_map_preview_id = preview_id;
    h.wol_map_select().win_map_preview = preview;

    let game_map = h.game_spy_game_map();
    if let Some(official) = h.map_cache_is_official(&game_map) {
        uses_system_map_dir = official;
    }
    let use_stats = h.current_staging_room_use_stats();
    if use_stats {
        uses_system_map_dir = true;
    }

    let ids = [key_of(h, "ButtonBack"), key_of(h, "ButtonOK"), key_of(h, "ListboxMap"), key_of(h, "RadioButtonSystemMaps"), key_of(h, "RadioButtonUserMaps")];
    let win_map_window = wm.win_get_window_from_id(parent, ids[2]);
    {
        let m = h.wol_map_select();
        m.button_back = ids[0];
        m.button_ok = ids[1];
        m.listbox_map = ids[2];
        m.radio_button_system_maps_id = ids[3];
        m.radio_button_user_maps_id = ids[4];
        m.win_map_window = win_map_window;
    }
    let radio_system = wm.win_get_window_from_id(parent, ids[3]);
    let radio_user = wm.win_get_window_from_id(parent, ids[4]);

    if use_stats {
        // disable unofficial maps if stats are being recorded
        if let Some(r) = radio_system {
            radio_set_selection(wm, h, r, false);
        }
        if let Some(r) = radio_user {
            wm.win_enable(r, false);
        }
    } else if uses_system_map_dir {
        if let Some(r) = radio_system {
            radio_set_selection(wm, h, r, false);
        }
    } else if let Some(r) = radio_user {
        radio_set_selection(wm, h, r, false);
    }

    for i in 0..MAX_SLOTS {
        let id = key_of(h, &format!("ButtonMapStartPosition{i}"));
        let b = wm.win_get_window_from_id(preview, id);
        {
            let m = h.wol_map_select();
            m.button_map_start_position_id[i] = id;
            m.button_map_start_position[i] = b;
        }
        // DEBUG_ASSERTCRASH(buttonMapStartPosition[i], ...); upstream dereferences it unchecked
        if let Some(b) = b {
            wm.win_hide(b, true);
            wm.win_enable(b, false);
        }
    }

    h.wol_map_select().raise_message_boxes = true;
    show_game_spy_game_options_underlying_gui_elements(wm, h, false);

    // get the listbox window
    let list_id = key_of(h, "ListboxMap");
    let map_list = wm.win_get_window_from_id(parent, list_id);
    h.wol_map_select().map_list = map_list;
    if map_list.is_some() {
        h.map_cache_update();
        h.populate_map_listbox(wm, map_list, uses_system_map_dir);
    }
}

/// `WOLMapSelectMenuShutdown(layout, userData)`.
pub fn wol_map_select_menu_shutdown<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    nullify_controls(h);
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLMapSelectMenuUpdate(layout, userData)`.
pub fn wol_map_select_menu_update<H: WolMapSelectHost>(_wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    if h.wol_map_select().raise_message_boxes {
        h.raise_gs_message_box();
        h.wol_map_select().raise_message_boxes = false;
    }
}

/// `WOLMapSelectMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Back*.
pub fn wol_map_select_menu_input<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let id = key_of(h, "ButtonBack");
            let button = wm.win_get_window_from_id(Some(window), id);
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLMapSelectLayout->destroyWindows(); deleteInstance(); WOLMapSelectLayout = NULL;`.
fn destroy_layout<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(l) = h.wol_map_select().wol_map_select_layout.take() {
        wm.layout_destroy_windows(l);
        wm.layouts[l].deleted = true;
    }
}

fn row_map<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, list: WinId, row: i32, text: &[u16]) -> Vec<u8> {
    let data = lb::get_item_data(wm, h, list, row, 0);
    match h.map_list_name(data) {
        Some(n) => n,
        None => text.iter().map(|&c| c as u8).collect(), // asciiMap.translate(map)
    }
}

/// `WOLMapSelectMenuSystem(window, msg, mData1, mData2)` (WOLMapSelectMenu.cpp:258).
pub fn wol_map_select_menu_system<H: WolMapSelectHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
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
            let m = h.wol_map_select().clone();
            if id == m.listbox_map {
                let row_selected = d2 as i32;
                if row_selected >= 0 {
                    lb::set_selected(wm, h, control, &[row_selected]);
                    let button = wm.win_get_window_from_id(Some(window), m.button_ok);
                    wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), m.button_ok as u32);
                }
            }
        }
        ggm::GLM_SELECTED => {
            let Some((_, id)) = control_id(wm) else { return MsgHandled::Handled };
            let m = h.wol_map_select().clone();
            if id == m.listbox_map {
                let row_selected = d2 as i32;
                if row_selected < 0 {
                    h.position_start_spots(wm, b"", &m.button_map_start_position, m.win_map_preview);
                    return MsgHandled::Handled;
                }
                let (Some(preview), Some(mw)) = (m.win_map_preview, m.win_map_window) else { return MsgHandled::Handled };
                wm.win_set_status(preview, win_status::IMAGE);
                let map = lb::get_text(wm, h, mw, row_selected, 0);
                let ascii_map = row_map(wm, h, mw, row_selected, &map).to_ascii_lowercase();
                let image = h.get_map_preview_image(&ascii_map);
                h.set_preview_user_data_map(wm, preview, &ascii_map);
                match image {
                    Some(i) => h.win_set_enabled_image0(wm, preview, &i),
                    None => {
                        wm.win_clear_status(preview, win_status::IMAGE);
                    }
                }
                h.position_start_spots(wm, &ascii_map, &m.button_map_start_position, m.win_map_preview);
            }
        }
        ggm::GBM_SELECTED => {
            let Some((_, id)) = control_id(wm) else { return MsgHandled::Handled };
            let m = h.wol_map_select().clone();
            let game_map = h.game_spy_game_map();
            if id == m.button_back {
                show_game_spy_game_options_underlying_gui_elements(wm, h, true);
                destroy_layout(wm, h);
                h.wol_position_start_spots(wm);
            } else if id == m.radio_button_system_maps_id {
                h.map_cache_update();
                let _ = &game_map;
                h.populate_map_listbox(wm, m.map_list, true);
                h.custom_prefs_set_uses_system_map_dir_and_write(true);
            } else if id == m.radio_button_user_maps_id {
                h.map_cache_update();
                h.populate_map_listbox(wm, m.map_list, false);
                h.custom_prefs_set_uses_system_map_dir_and_write(false);
            } else if id == m.button_ok {
                let selected = match m.win_map_window {
                    Some(mw) => lb::get_selected(wm, h, mw).first().copied().unwrap_or(-1),
                    None => -1,
                };
                if selected != -1 {
                    let mw = m.win_map_window.expect("selected implies the list");
                    let map = lb::get_text(wm, h, mw, selected, 0);
                    let ascii_map = row_map(wm, h, mw, selected, &map);
                    h.game_spy_game_set_map(&ascii_map);
                    let lower = ascii_map.to_ascii_lowercase();
                    if let Some((crc, size)) = h.map_cache_find(&lower) {
                        h.game_spy_game_slot0_set_map_availability(true);
                        h.game_spy_game_set_map_crc_and_size(crc, size);
                    }
                    h.game_spy_game_adjust_slots_for_map(); // BGC- adjust the slots for the new map.
                    h.game_spy_game_reset_accepted();
                    h.game_spy_game_reset_start_spots();
                    h.game_spy_info_set_game_options();

                    h.wol_display_slot_list(wm);
                    h.wol_display_game_options(wm);

                    destroy_layout(wm, h);
                    show_game_spy_game_options_underlying_gui_elements(wm, h, true);
                    h.wol_position_start_spots(wm);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
