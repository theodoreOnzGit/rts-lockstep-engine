//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupHostGame.cpp
//!     (the file's statics, `CustomMatchHideHostPopup`, `HandleCustomLadderSelection`,
//!      `PopulateCustomLadderListBox`, `PopulateCustomLadderComboBox`, `PopupHostGameInit/Update/Input/System`,
//!      `createGame`)
//!
//! The GameSpy service is gone: `TheLadderList`, `CustomMatchPreferences`, `LadderPreferences`,
//! `TheGameSpyInfo`, `TheGameSpyGame` and the peer queue are the host's. The ladder list's pointers (`const
//! LadderInfo *`, compared by identity in `usedLadders`) are modelled by the ladder's `index`, which is unique.

use crate::client::gadget;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// `LadderInfo` as this file reads it (LadderDefs.h).
#[derive(Clone, Debug, Default)]
pub struct LadderInfoLite {
    pub index: i32,
    pub name: Vec<u16>,
    pub address: Vec<u8>,
    pub port: u16,
    pub valid_custom: bool,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopupHostGame {
    pub parent_popup_id: i32,
    pub text_entry_game_name_id: i32,
    pub button_create_game_id: i32,
    pub check_box_allow_observers_id: i32,
    pub text_entry_game_description_id: i32,
    pub button_cancel_id: i32,
    pub text_entry_ladder_password_id: i32,
    pub combo_box_ladder_name_id: i32,
    pub text_entry_game_password_id: i32,
    pub check_box_limit_armies_id: i32,
    pub check_box_use_stats_id: i32,
    pub parent_popup: Option<WinId>,
    pub text_entry_game_name: Option<WinId>,
    pub button_create_game: Option<WinId>,
    pub check_box_allow_observers: Option<WinId>,
    pub text_entry_game_description: Option<WinId>,
    pub button_cancel: Option<WinId>,
    pub combo_box_ladder_name: Option<WinId>,
    pub text_entry_ladder_password: Option<WinId>,
    pub text_entry_game_password: Option<WinId>,
    pub check_box_limit_armies: Option<WinId>,
    pub check_box_use_stats: Option<WinId>,
    pub is_populating_ladder_box: bool,
}

/// What the host popup reaches in the GameSpy layer.
pub trait PopupHostGameHost: ShellHost {
    fn popup_host_game(&mut self) -> &mut PopupHostGame;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_MAP_SELECTED]`, `[GSCOLOR_MAP_UNSELECTED]`.
    fn gamespy_color_map_selected(&self) -> i32;
    fn gamespy_color_map_unselected(&self) -> i32;
    // CustomMatchPreferences
    fn custom_pref_set_last_ladder_and_write(&mut self, addr: &[u8], port: u16);
    fn custom_pref_last_ladder_addr(&mut self) -> Vec<u8>;
    fn custom_pref_last_ladder_port(&mut self) -> u16;
    /// `pref.load("GeneralsOnline\\CustomPref%d.ini")` for the local profile (the combo box variant).
    fn custom_pref_load_for_local_profile(&mut self);
    fn custom_pref_allows_observers(&mut self) -> bool;
    fn custom_pref_use_stats(&mut self) -> bool;
    fn custom_pref_factions_limited(&mut self) -> bool;
    fn custom_pref_set_and_write(&mut self, allow_observers: bool, factions_limited: bool, use_stats: bool);
    // TheLadderList
    fn ladder_find_by_index(&self, index: i32) -> Option<LadderInfoLite>;
    fn ladder_find(&self, addr: &[u8], port: u16) -> Option<LadderInfoLite>;
    fn ladder_local(&self) -> Vec<LadderInfoLite>;
    fn ladder_special(&self) -> Vec<LadderInfoLite>;
    fn ladder_standard(&self) -> Vec<LadderInfoLite>;
    /// `LadderPreferences::loadProfile(localProfile)` then `getRecentLadders()`: (address, port) in map order.
    fn ladder_pref_recent_ladders(&mut self) -> Vec<(Vec<u8>, u16)>;
    // TheGameSpyInfo
    fn gamespy_local_name(&self) -> Vec<u16>;
    fn gamespy_set_current_group_room(&mut self, room: i32);
    fn gamespy_internal_ip(&self) -> u32;
    fn gamespy_ping_string(&self) -> Vec<u8>;
    fn gamespy_restrict_games_to_lobby(&self) -> bool;
    fn global_exe_crc(&self) -> u32;
    fn global_ini_crc(&self) -> u32;
    /// `winSetModal`.
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>);
    /// `GameSpyOpenOverlay(GSOVERLAY_LADDERSELECT)`, `GameSpyCloseOverlay(GSOVERLAY_GAMEOPTIONS)`.
    fn gamespy_open_overlay_ladder_select(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_overlay_game_options(&mut self, wm: &mut GameWindowManager);
    /// `SetLobbyAttemptHostJoin(FALSE)`.
    fn set_lobby_attempt_host_join_false(&mut self);
    /// `TheGameSpyGame`'s setters used by `createGame`.
    fn game_spy_game_set_game_name(&mut self, name: &[u16]);
    fn game_spy_game_set_options(&mut self, allow_observers: bool, old_factions_only: bool, use_stats: bool);
    fn game_spy_game_set_ladder(&mut self, ip: &[u8], port: u16);
    /// `PeerRequest` of `PEERREQUEST_CREATESTAGINGROOM` queued with `TheGameSpyPeerMessageQueue->addRequest`.
    fn peer_request_create_staging_room(&mut self, req: &CreateStagingRoomRequest);
}

/// The fields of the `PeerRequest` `createGame` fills.
#[derive(Clone, Debug, Default)]
pub struct CreateStagingRoomRequest {
    pub text: Vec<u16>,
    pub password: Vec<u8>,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub game_version: u32,
    pub restrict_game_list: bool,
    pub ladder_ip: Vec<u8>,
    pub lad_port: u16,
    pub host_ping_str: Vec<u8>,
}

fn key_of<H: PopupHostGameHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("PopupHostGame.wnd:{n}").as_bytes()) as i32
}

/// `CustomMatchHideHostPopup(hide)`.
pub fn custom_match_hide_host_popup<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, hide: bool) {
    let Some(p) = h.popup_host_game().parent_popup else { return };
    wm.win_hide(p, hide);
}

/// `HandleCustomLadderSelection(ladderID)`.
pub fn handle_custom_ladder_selection<H: PopupHostGameHost>(h: &mut H, ladder_id: i32) {
    if h.popup_host_game().parent_popup.is_none() {
        return;
    }
    if ladder_id == 0 {
        h.custom_pref_set_last_ladder_and_write(&[], 0);
        return;
    }
    match h.ladder_find_by_index(ladder_id) {
        None => h.custom_pref_set_last_ladder_and_write(&[], 0),
        Some(info) => h.custom_pref_set_last_ladder_and_write(&info.address, info.port),
    }
}

/// Adds one ladder to the list box when it is not used yet; the item data is its index (set on the row).
fn list_add_ladder<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, win: WinId, used: &mut Vec<i32>, info: &LadderInfoLite, color: i32) -> i32 {
    used.push(info.index);
    let index = lb::add_entry_text(wm, h, win, &info.name, color, -1, 0, false);
    lb::set_item_data(wm, h, win, info.index as u32, index, 0);
    index
}

/// `PopulateCustomLadderListBox(win)` (PopupHostGame.cpp:107). Upstream calls
/// `GadgetListBoxSetItemData(win, data, index)` with the arguments in the listbox's own order (data, row).
pub fn populate_custom_ladder_list_box<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, win: Option<WinId>) {
    let Some(win) = win else { return };
    if h.popup_host_game().parent_popup.is_none() {
        return;
    }
    h.popup_host_game().is_populating_ladder_box = true;

    let special_color = h.gamespy_color_map_selected();
    let normal_color = h.gamespy_color_map_unselected();
    let favorite_color = h.gamespy_color_map_unselected();
    let local_color = h.gamespy_color_map_unselected();

    lb::reset(wm, h, win);
    let mut used: Vec<i32> = Vec::new();

    let t = h.fetch_text(b"GUI:NoLadder");
    let index = lb::add_entry_text(wm, h, win, &t, normal_color, -1, 0, false);
    lb::set_item_data(wm, h, win, 0, index, 0);

    let mut selected_pos = 0;

    let last_addr = h.custom_pref_last_ladder_addr();
    let last_port = h.custom_pref_last_ladder_port();
    if let Some(info) = h.ladder_find(&last_addr, last_port) {
        if info.index > 0 && info.valid_custom {
            selected_pos = list_add_ladder(wm, h, win, &mut used, &info, favorite_color);
        }
    }

    let recent = h.ladder_pref_recent_ladders();
    for (addr, port) in recent {
        if addr == last_addr && port == last_port {
            continue;
        }
        if let Some(info) = h.ladder_find(&addr, port) {
            if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
                list_add_ladder(wm, h, win, &mut used, &info, favorite_color);
            }
        }
    }

    for info in h.ladder_local() {
        if info.index < 0 && info.valid_custom && !used.contains(&info.index) {
            list_add_ladder(wm, h, win, &mut used, &info, local_color);
        }
    }
    for info in h.ladder_special() {
        if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
            list_add_ladder(wm, h, win, &mut used, &info, special_color);
        }
    }
    for info in h.ladder_standard() {
        if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
            list_add_ladder(wm, h, win, &mut used, &info, normal_color);
        }
    }

    lb::set_selected(wm, h, win, &[selected_pos]);
    h.popup_host_game().is_populating_ladder_box = false;
}

/// Adds one ladder to the combo box; the item data is its index.
fn combo_add_ladder<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, combo: WinId, info: &LadderInfoLite, color: i32) -> i32 {
    let index = cb::add_entry(wm, h, combo, &info.name, color);
    cb::set_item_data(wm, h, combo, index, info.index as u32);
    index
}

/// `PopulateCustomLadderComboBox()` (PopupHostGame.cpp:215).
pub fn populate_custom_ladder_combo_box<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H) {
    let p = h.popup_host_game().clone();
    let Some(combo) = p.combo_box_ladder_name else { return };
    if p.parent_popup.is_none() {
        return;
    }
    h.popup_host_game().is_populating_ladder_box = true;

    h.custom_pref_load_for_local_profile();

    let mut used: Vec<i32> = Vec::new();
    let special_color = h.gamespy_color_map_selected();
    let normal_color = h.gamespy_color_map_unselected();

    cb::reset(wm, h, combo);
    let t = h.fetch_text(b"GUI:NoLadder");
    let index = cb::add_entry(wm, h, combo, &t, normal_color);
    cb::set_item_data(wm, h, combo, index, 0);

    let mut selected_pos = 0;

    let last_addr = h.custom_pref_last_ladder_addr();
    let last_port = h.custom_pref_last_ladder_port();
    if let Some(info) = h.ladder_find(&last_addr, last_port) {
        if info.valid_custom {
            used.push(info.index);
            selected_pos = combo_add_ladder(wm, h, combo, &info, special_color);
        }
    }

    let recent = h.ladder_pref_recent_ladders();
    for (addr, port) in recent {
        if addr == last_addr && port == last_port {
            continue;
        }
        if let Some(info) = h.ladder_find(&addr, port) {
            if info.valid_custom && !used.contains(&info.index) {
                used.push(info.index);
                combo_add_ladder(wm, h, combo, &info, normal_color);
            }
        }
    }

    let t = h.fetch_text(b"GUI:ChooseLadder");
    let index = cb::add_entry(wm, h, combo, &t, normal_color);
    cb::set_item_data(wm, h, combo, index, -1i32 as u32);

    cb::set_selected_pos(wm, h, combo, selected_pos, false);
    h.popup_host_game().is_populating_ladder_box = false;
}

/// `PopupHostGameInit(layout, userData)` (PopupHostGame.cpp:288).
pub fn popup_host_game_init<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = key_of(h, "ParentHostPopUp");
    let parent = wm.win_get_window_from_id(None, parent_id);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);

    let name_id = key_of(h, "TextEntryGameName");
    let name_win = find(wm, name_id);
    let name = h.gamespy_local_name(); // name.translate(TheGameSpyInfo->getLocalName())
    te::set_text(wm, h, name_win, &name);

    let desc_id = key_of(h, "TextEntryGameDescription");
    let desc_win = find(wm, desc_id);
    te::set_text(wm, h, desc_win, &[]);

    let ladder_pw_id = key_of(h, "TextEntryLadderPassword");
    let ladder_pw = find(wm, ladder_pw_id);
    te::set_text(wm, h, ladder_pw, &[]);

    let game_pw_id = key_of(h, "TextEntryGamePassword");
    let game_pw = find(wm, game_pw_id);
    te::set_text(wm, h, game_pw, &[]);

    let create_id = key_of(h, "ButtonCreateGame");
    let create = find(wm, create_id);
    let cancel_id = key_of(h, "ButtonCancel");
    let cancel = find(wm, cancel_id);

    let obs_id = key_of(h, "CheckBoxAllowObservers");
    let obs = find(wm, obs_id);
    let allows_observers = h.custom_pref_allows_observers();
    if let Some(o) = obs {
        gadget::check_box_set_checked(wm, h, o, allows_observers);
    }

    let combo_id = key_of(h, "ComboBoxLadderName");
    let combo = find(wm, combo_id);
    {
        let p = h.popup_host_game();
        p.parent_popup_id = parent_id;
        p.parent_popup = parent;
        p.text_entry_game_name_id = name_id;
        p.text_entry_game_name = name_win;
        p.text_entry_game_description_id = desc_id;
        p.text_entry_game_description = desc_win;
        p.text_entry_ladder_password_id = ladder_pw_id;
        p.text_entry_ladder_password = ladder_pw;
        p.text_entry_game_password_id = game_pw_id;
        p.text_entry_game_password = game_pw;
        p.button_create_game_id = create_id;
        p.button_create_game = create;
        p.button_cancel_id = cancel_id;
        p.button_cancel = cancel;
        p.check_box_allow_observers_id = obs_id;
        p.check_box_allow_observers = obs;
        p.combo_box_ladder_name_id = combo_id;
        p.combo_box_ladder_name = combo;
    }
    if let Some(c) = combo {
        cb::reset(wm, h, c);
    }
    populate_custom_ladder_combo_box(wm, h);

    let use_stats_id = key_of(h, "CheckBoxUseStats");
    let use_stats_win = find(wm, use_stats_id);
    let using_stats = h.custom_pref_use_stats();
    if let Some(u) = use_stats_win {
        gadget::check_box_set_checked(wm, h, u, using_stats);
    }

    let limit_id = key_of(h, "CheckBoxLimitArmies");
    let limit_win = find(wm, limit_id);
    {
        let p = h.popup_host_game();
        p.check_box_use_stats_id = use_stats_id;
        p.check_box_use_stats = use_stats_win;
        p.check_box_limit_armies_id = limit_id;
        p.check_box_limit_armies = limit_win;
    }
    let limited = if using_stats { false } else { h.custom_pref_factions_limited() };
    if let Some(l) = limit_win {
        wm.win_enable(l, !using_stats);
        gadget::check_box_set_checked(wm, h, l, limited);
    }

    wm.win_set_focus(h, parent);
    h.win_set_modal(wm, parent);
}

/// `PopupHostGameUpdate(layout, userData)`.
pub fn popup_host_game_update<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let p = h.popup_host_game().clone();
    let use_stats = p.check_box_use_stats.is_some_and(|c| gadget::check_box_is_checked(wm, c));
    let Some(limit) = p.check_box_limit_armies else { return };
    if use_stats {
        wm.win_enable(limit, false);
        gadget::check_box_set_checked(wm, h, limit, false);
    } else {
        wm.win_enable(limit, true);
    }
}

/// `PopupHostGameInput(window, msg, mData1, mData2)`: Escape's release presses *Cancel*.
pub fn popup_host_game_input<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let p = h.popup_host_game().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(p.button_cancel), p.button_cancel_id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `PopupHostGameSystem(window, msg, mData1, mData2)`.
pub fn popup_host_game_system<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => h.popup_host_game().parent_popup = None,
        gwm::INPUT_FOCUS => {
            // (break, not return: handled all the same)
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        crate::client::gadget_text_entry::gem::UPDATE_TEXT => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let p = h.popup_host_game().clone();
            if wm.tree.windows[control].id == p.text_entry_game_name_id {
                // strip the leading white space of the game name
                let txt = te::get_text(wm, h, p.text_entry_game_name);
                let lead = txt.iter().take_while(|&&c| c == ' ' as u16 || (c >= 9 && c <= 13)).count();
                let txt: Vec<u16> = txt[lead..].to_vec();
                te::set_text(wm, h, p.text_entry_game_name, &txt);
            }
        }
        ggm::GCM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let pos = cb::get_selected_pos(wm, h, control, -1);
            let p = h.popup_host_game().clone();
            if control_id == p.combo_box_ladder_name_id && !p.is_populating_ladder_box && pos >= 0 {
                let ladder_id = cb::get_item_data(wm, h, control, pos) as i32;
                if ladder_id < 0 {
                    populate_custom_ladder_combo_box(wm, h); // this restores the non-"Choose a ladder" selection
                    h.gamespy_open_overlay_ladder_select(wm);
                }
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let p = h.popup_host_game().clone();
            if control_id == p.button_cancel_id {
                h.popup_host_game().parent_popup = None;
                h.gamespy_close_overlay_game_options(wm);
                h.set_lobby_attempt_host_join_false();
            } else if control_id == p.button_create_game_id {
                let name = te::get_text(wm, h, p.text_entry_game_name);
                let name = crate::client::replay_menu::trim(&name);
                if name.is_empty() {
                    let local = h.gamespy_local_name();
                    te::set_text(wm, h, p.text_entry_game_name, &local);
                }
                create_game(wm, h);
                h.popup_host_game().parent_popup = None;
                h.gamespy_close_overlay_game_options(wm);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `createGame()` (PopupHostGame.cpp:565).
pub fn create_game<H: PopupHostGameHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.gamespy_set_current_group_room(0);

    let p = h.popup_host_game().clone();
    let game_name = te::get_text(wm, h, p.text_entry_game_name);
    h.game_spy_game_set_game_name(&game_name);

    let passwd_w = te::get_text(wm, h, p.text_entry_game_password);
    let passwd: Vec<u8> = passwd_w.iter().map(|&c| c as u8).collect(); // passwd.translate(...)

    let a_o = p.check_box_allow_observers.is_some_and(|c| gadget::check_box_is_checked(wm, c));
    let limit_armies = p.check_box_limit_armies.is_some_and(|c| gadget::check_box_is_checked(wm, c));
    let use_stats = p.check_box_use_stats.is_some_and(|c| gadget::check_box_is_checked(wm, c));
    h.custom_pref_set_and_write(a_o, limit_armies, use_stats);
    h.game_spy_game_set_options(a_o, limit_armies, use_stats);

    let mut req = CreateStagingRoomRequest {
        text: game_name,
        password: passwd,
        allow_observers: a_o,
        use_stats,
        exe_crc: h.global_exe_crc(),
        ini_crc: h.global_ini_crc(),
        game_version: h.gamespy_internal_ip(),
        restrict_game_list: h.gamespy_restrict_games_to_lobby(),
        ladder_ip: b"localhost".to_vec(),
        lad_port: 0,
        host_ping_str: Vec::new(),
    };

    let ladder_select_pos = match p.combo_box_ladder_name {
        Some(c) => cb::get_selected_pos(wm, h, c, -1),
        None => -1,
    };
    if ladder_select_pos >= 0 {
        let ladder_id = cb::get_item_data(wm, h, p.combo_box_ladder_name.expect("pos implies combo"), ladder_select_pos) as i32;
        if ladder_id != 0 {
            if let Some(info) = h.ladder_find_by_index(ladder_id) {
                req.ladder_ip = info.address.clone();
                req.lad_port = info.port;
            }
        }
    }
    h.game_spy_game_set_ladder(&req.ladder_ip, req.lad_port);

    req.host_ping_str = h.gamespy_ping_string();
    h.peer_request_create_staging_room(&req);
}
