//! UNTESTED DRAFT (#202, #203, #214): compiles; PARTLY tested against the upstream C++ (#215, group ui5): `populateLadderComboBox`, `populateLadderListBox`, `handleLadderSelection`,
//! `setPasswordMode`, `PopupLadderSelectInit/Input/System`, `ladderSelectedCallback` and `updateLadderDetails` match it on 3,000 cases (test:
//! tests/differential_draft_popup_ladder_select.rs). NOT covered, so the file keeps this banner: `RCGameDetailsMenuInit`, `RCGameDetailsMenuSystem` and `closeRightClickMenu` (they need
//! layouts made from WND files, the lone window and the GameSpy staging rooms).
//! Corrected 2026-11-21 (#215, group ui5): the password's `translate` drops a unit whose low byte is 0; the places where upstream is undefined (the ladder list box missing, a NULL control)
//! report through [`PopupLadderSelectHost::undefined`].
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupLadderSelect.cpp
//!     (the file's statics, `populateLadderComboBox`, `populateLadderListBox`, `handleLadderSelection`,
//!      `setPasswordMode`, `PopupLadderSelectInit/Input/System`, `ladderSelectedCallback`, `updateLadderDetails`,
//!      `closeRightClickMenu`, `RCGameDetailsMenuInit/System`)
//!
//! The GameSpy service is gone: `TheLadderList`, `TheGameSpyInfo`'s staging rooms and `EncryptString` are the
//! host's. `PopulateQMLadderComboBox/ListBox` and `HandleQMLadderSelection` belong to WOLQuickMatchMenu.cpp
//! ([`crate::drafts::ui::wol_quick_match_menu`]); `PopulateCustomLadderComboBox/ListBox` and
//! `HandleCustomLadderSelection` to PopupHostGame.cpp ([`crate::drafts::ui::popup_host_game`]). Both are reached
//! through the host so this file does not depend on a host trait of either.

use crate::client::gadget;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `GameMakeColor(255,255,255,255)`, `GameMakeColor(0,255,255,255)`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;
const CAPTION_COLOR: i32 = 0xFF00_FFFFu32 as i32;

/// `enum PasswordMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PasswordMode {
    #[default]
    PassNone,
    PassEntry,
    PassError,
}

/// `LadderInfo` as this file reads it (LadderDefs.h).
#[derive(Clone, Debug, Default)]
pub struct LadderInfoFull {
    pub index: i32,
    pub name: Vec<u16>,
    pub players_per_team: i32,
    pub location: Vec<u16>,
    pub homepage_url: Vec<u16>,
    pub description: Vec<u16>,
    pub crypted_password: Vec<u8>,
    pub min_wins: i32,
    pub max_wins: i32,
    pub random_factions: bool,
    pub random_maps: bool,
    pub valid_factions: Vec<Vec<u8>>,
    pub valid_maps: Vec<Vec<u8>>,
    pub address: Vec<u8>,
    pub port: u16,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`), `s_currentMode` and `ladderIndex`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopupLadderSelect {
    pub parent_id: i32,
    pub listbox_ladder_select_id: i32,
    pub listbox_ladder_details_id: i32,
    pub static_text_ladder_name_id: i32,
    pub button_ok_id: i32,
    pub button_cancel_id: i32,
    pub parent: Option<WinId>,
    pub listbox_ladder_select: Option<WinId>,
    pub listbox_ladder_details: Option<WinId>,
    pub static_text_ladder_name: Option<WinId>,
    pub button_ok: Option<WinId>,
    pub button_cancel: Option<WinId>,
    pub password_parent_id: i32,
    pub button_password_ok_id: i32,
    pub button_password_cancel_id: i32,
    pub text_entry_password_id: i32,
    pub password_parent: Option<WinId>,
    pub text_entry_password: Option<WinId>,
    pub bad_password_parent_id: i32,
    pub button_bad_password_ok_id: i32,
    pub bad_password_parent: Option<WinId>,
    pub current_mode: PasswordMode,
    pub ladder_index: i32,
}

/// What the ladder popup reaches in the GameSpy layer.
pub trait PopupLadderSelectHost: ShellHost {
    fn popup_ladder_select(&mut self) -> &mut PopupLadderSelect;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn ladder_find_by_index(&self, index: i32) -> Option<LadderInfoFull>;
    /// `PopulateQMLadderComboBox()`, `PopulateCustomLadderComboBox()`.
    fn populate_qm_ladder_combo_box(&mut self, wm: &mut GameWindowManager);
    fn populate_custom_ladder_combo_box(&mut self, wm: &mut GameWindowManager);
    /// `PopulateQMLadderListBox(win)`, `PopulateCustomLadderListBox(win)`.
    fn populate_qm_ladder_list_box(&mut self, wm: &mut GameWindowManager, win: Option<WinId>);
    fn populate_custom_ladder_list_box(&mut self, wm: &mut GameWindowManager, win: Option<WinId>);
    /// `HandleQMLadderSelection(id)`, `HandleCustomLadderSelection(id)`.
    fn handle_qm_ladder_selection(&mut self, id: i32);
    fn handle_custom_ladder_selection(&mut self, id: i32);
    /// `CustomMatchHideHostPopup(hide)`.
    fn custom_match_hide_host_popup(&mut self, wm: &mut GameWindowManager, hide: bool);
    /// `winSetModal(window)`.
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>);
    /// `GameSpyCloseOverlay(GSOVERLAY_LADDERSELECT)`.
    fn gamespy_close_overlay_ladder_select(&mut self, wm: &mut GameWindowManager);
    /// `EncryptString(pass)`.
    fn encrypt_string(&self, pass: &[u8]) -> Vec<u8>;
    /// `UnicodeString::format` of the ladder lines.
    fn format_ladder_name_and_size(&self, fmt: &[u16], name: &[u16], players_a: i32, players_b: i32) -> Vec<u16>;
    fn format_text(&self, fmt: &[u16], text: &[u16]) -> Vec<u16>;
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16>;
    /// `TheMapCache->findMap(map)->m_displayName`.
    fn map_display_name(&self, map: &[u8]) -> Option<Vec<u16>>;
    // RCGameDetailsMenu
    /// `TheGameSpyInfo->getStagingRoomList()->find(id)` then `TheLadderList->findLadder(room ip, port)`.
    fn ladder_for_staging_room(&self, room_id: i32) -> Option<LadderInfoFull>;
    /// `window->winGetUserData()` of a right-click menu window as an int.
    fn win_user_data_int(&self, wm: &GameWindowManager, w: WinId) -> i32;
    fn win_set_user_data_int(&mut self, wm: &mut GameWindowManager, w: WinId, v: i32);
    /// `winSetLoneWindow(w)`.
    fn win_set_lone_window(&mut self, wm: &mut GameWindowManager, w: WinId);
    /// `GGM_CLOSE` for the right-click menu and the id of its `ButtonLadderDetails` are kept in these statics.
    fn rc_game_details_ids(&mut self) -> &mut (i32, i32);
    /// Upstream does something undefined here (a NULL dereference, an uninitialised `Int`): `what` says what. The draft carries on with its own reading; a test excludes the input.
    fn undefined(&mut self, _what: &str) {}
}

fn key_of<H: PopupLadderSelectHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("PopupLadderSelect.wnd:{n}").as_bytes()) as i32
}

/// `populateLadderComboBox()`.
fn populate_ladder_combo_box<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.populate_qm_ladder_combo_box(wm);
    h.populate_custom_ladder_combo_box(wm);
}

/// `populateLadderListBox()`.
fn populate_ladder_list_box<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    let p = h.popup_ladder_select().clone();
    h.populate_qm_ladder_list_box(wm, p.listbox_ladder_select);
    h.populate_custom_ladder_list_box(wm, p.listbox_ladder_select);

    let Some(l) = p.listbox_ladder_select else {
        h.undefined("populateLadderListBox: GadgetListBoxGetSelected(NULL) leaves selIndex uninitialised");
        return;
    };
    let sel_index = lb::get_selected(wm, h, l).first().copied().unwrap_or(-1);
    if sel_index < 0 {
        return;
    }
    let sel_id = lb::get_item_data(wm, h, l, sel_index, 0) as i32;
    if sel_id == 0 {
        return;
    }
    update_ladder_details(wm, h, sel_id, p.static_text_ladder_name, p.listbox_ladder_details);
}

/// `handleLadderSelection(ladderID)`.
fn handle_ladder_selection<H: PopupLadderSelectHost>(h: &mut H, ladder_id: i32) {
    if plant(22755) {
        h.handle_custom_ladder_selection(ladder_id);
        h.handle_qm_ladder_selection(ladder_id);
        return;
    }
    h.handle_qm_ladder_selection(ladder_id);
    h.handle_custom_ladder_selection(ladder_id);
}

/// `setPasswordMode(mode)` (PopupLadderSelect.cpp:122).
fn set_password_mode<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, mode: PasswordMode) {
    h.popup_ladder_select().current_mode = mode;
    let p = h.popup_ladder_select().clone();
    let hide = |wm: &mut GameWindowManager, w: Option<WinId>, v: bool| {
        if let Some(w) = w {
            wm.win_hide(w, v);
        }
    };
    let enable = |wm: &mut GameWindowManager, w: Option<WinId>, v: bool| {
        if let Some(w) = w {
            wm.win_enable(w, v);
        }
    };
    match mode {
        PasswordMode::PassNone => {
            hide(wm, p.password_parent, true);
            hide(wm, p.bad_password_parent, true);
            enable(wm, p.button_ok, !plant(22741));
            enable(wm, p.button_cancel, true);
            enable(wm, p.text_entry_password, false);
            enable(wm, p.listbox_ladder_select, true);
            wm.win_set_focus(h, p.listbox_ladder_select);
        }
        PasswordMode::PassEntry => {
            hide(wm, p.password_parent, false);
            hide(wm, p.bad_password_parent, true);
            enable(wm, p.button_ok, false);
            enable(wm, p.button_cancel, false);
            if let Some(t) = p.text_entry_password {
                wm.win_enable(t, true);
                if !plant(22742) {
                    te::set_text(wm, h, Some(t), &[]);
                }
            }
            enable(wm, p.listbox_ladder_select, false);
            wm.win_set_focus(h, p.text_entry_password);
        }
        PasswordMode::PassError => {
            hide(wm, p.password_parent, true);
            hide(wm, p.bad_password_parent, false);
            enable(wm, p.button_ok, false);
            enable(wm, p.button_cancel, false);
            enable(wm, p.text_entry_password, false);
            enable(wm, p.listbox_ladder_select, false);
            wm.win_set_focus(h, if plant(22743) { None } else { p.parent });
        }
    }
}

/// `PopupLadderSelectInit(layout, userData)`.
pub fn popup_ladder_select_init<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = key_of(h, "Parent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    let ls_id = key_of(h, "ListBoxLadderSelect");
    let ld_id = key_of(h, "ListBoxLadderDetails");
    let name_id = key_of(h, "StaticTextLadderName");
    let ok_id = key_of(h, "ButtonOk");
    let cancel_id = key_of(h, "ButtonCancel");
    let pw_parent_id = key_of(h, "PasswordParent");
    let pw_ok_id = key_of(h, "ButtonPasswordOk");
    let pw_cancel_id = key_of(h, "ButtonPasswordCancel");
    let pw_entry_id = key_of(h, "PasswordEntry");
    let bad_parent_id = key_of(h, "BadPasswordParent");
    let bad_ok_id = key_of(h, "ButtonBadPasswordOk");
    let wins = [ls_id, ld_id, name_id, ok_id, cancel_id, pw_parent_id, pw_entry_id, bad_parent_id].map(|id| wm.win_get_window_from_id(parent, id));
    {
        let p = h.popup_ladder_select();
        p.parent_id = parent_id;
        p.parent = parent;
        p.listbox_ladder_select_id = ls_id;
        p.listbox_ladder_details_id = ld_id;
        p.static_text_ladder_name_id = name_id;
        p.button_ok_id = ok_id;
        p.button_cancel_id = cancel_id;
        p.listbox_ladder_select = wins[0];
        p.listbox_ladder_details = wins[1];
        p.static_text_ladder_name = wins[2];
        p.button_ok = wins[3];
        p.button_cancel = wins[4];
        p.password_parent_id = pw_parent_id;
        p.password_parent = wins[5];
        p.button_password_ok_id = pw_ok_id;
        p.button_password_cancel_id = pw_cancel_id;
        p.text_entry_password_id = pw_entry_id;
        p.text_entry_password = wins[6];
        p.bad_password_parent_id = bad_parent_id;
        p.bad_password_parent = wins[7];
        p.button_bad_password_ok_id = bad_ok_id;
    }

    wm.win_set_focus(h, parent);
    if !plant(22744) {
        h.win_set_modal(wm, parent);
    }

    set_password_mode(wm, h, PasswordMode::PassNone);

    h.custom_match_hide_host_popup(wm, !plant(22745));

    populate_ladder_list_box(wm, h);
}

/// `PopupLadderSelectInput(window, msg, mData1, mData2)`.
pub fn popup_ladder_select_input<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            match h.popup_ladder_select().current_mode {
                PasswordMode::PassNone if !plant(22746) => {
                    populate_ladder_combo_box(wm, h);
                    h.gamespy_close_overlay_ladder_select(wm);
                }
                _ => set_password_mode(wm, h, PasswordMode::PassNone),
            }
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `ladderSelectedCallback()`.
pub fn ladder_selected_callback<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    let idx = h.popup_ladder_select().ladder_index;
    handle_ladder_selection(h, idx);
    if plant(22756) {
        h.gamespy_close_overlay_ladder_select(wm);
        populate_ladder_combo_box(wm, h);
        return;
    }
    populate_ladder_combo_box(wm, h);
    h.gamespy_close_overlay_ladder_select(wm);
}

/// `PopupLadderSelectSystem(window, msg, mData1, mData2)`.
pub fn popup_ladder_select_system<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => {
            {
                let p = h.popup_ladder_select();
                p.parent = None;
                p.listbox_ladder_select = None;
                p.listbox_ladder_details = None;
            }
            if !plant(22747) {
                h.custom_match_hide_host_popup(wm, false);
            }
        }
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                h.undefined("control->winGetWindowId() of a NULL control");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let p = h.popup_ladder_select().clone();
            if id == p.button_ok_id {
                let select_pos = match p.listbox_ladder_select {
                    Some(l) => lb::get_selected(wm, h, l).first().copied().unwrap_or(-1),
                    None => {
                        h.undefined("GBM_SELECTED: GadgetListBoxGetSelected(NULL) leaves selectPos as -1; then GetItemData(NULL)");
                        -1
                    }
                };
                if select_pos < 0 {
                    return MsgHandled::Handled;
                }
                let ladder_index = lb::get_item_data(wm, h, p.listbox_ladder_select.expect("pos implies list"), select_pos, 0) as i32;
                if !plant(22750) {
                    h.popup_ladder_select().ladder_index = ladder_index;
                }
                let li = h.ladder_find_by_index(ladder_index);
                if li.is_some_and(|l| l.crypted_password.is_empty() == plant(22748)) {
                    set_password_mode(wm, h, PasswordMode::PassEntry);
                } else {
                    ladder_selected_callback(wm, h);
                }
            } else if id == p.button_cancel_id {
                populate_ladder_combo_box(wm, h);
                h.gamespy_close_overlay_ladder_select(wm);
            } else if id == p.button_password_ok_id {
                let li = h.ladder_find_by_index(p.ladder_index);
                let Some(li) = li.filter(|l| !l.crypted_password.is_empty()) else {
                    set_password_mode(wm, h, PasswordMode::PassError);
                    return MsgHandled::Handled;
                };
                let pass_w = te::get_text(wm, h, p.text_entry_password);
                let pass: Vec<u8> = pass_w.iter().map(|&c| c as u8).filter(|&c| c != 0).collect(); // pass.translate(...): a 0 byte appends nothing
                if !pass.is_empty() {
                    // password ok
                    let crypt_pass = h.encrypt_string(&pass);
                    if (crypt_pass == li.crypted_password) != plant(22749) {
                        ladder_selected_callback(wm, h);
                    } else {
                        set_password_mode(wm, h, PasswordMode::PassError);
                    }
                } else {
                    set_password_mode(wm, h, PasswordMode::PassError);
                }
            } else if id == p.button_password_cancel_id || id == p.button_bad_password_ok_id {
                set_password_mode(wm, h, PasswordMode::PassNone);
            }
        }
        lb::glm::SELECTED => {
            let p = h.popup_ladder_select().clone();
            let Some(l) = p.listbox_ladder_select else {
                h.undefined("GLM_SELECTED: GadgetListBoxGetSelected(NULL) leaves selIndex uninitialised");
                return MsgHandled::Handled;
            };
            let sel_index = lb::get_selected(wm, h, l).first().copied().unwrap_or(-1);
            if sel_index < 0 {
                return MsgHandled::Handled;
            }
            let sel_id = lb::get_item_data(wm, h, l, sel_index, 0) as i32;
            if sel_id == 0 && !plant(22752) {
                return MsgHandled::Handled;
            }
            update_ladder_details(wm, h, sel_id, p.static_text_ladder_name, p.listbox_ladder_details);
        }
        lb::glm::DOUBLE_CLICKED => {
            let Some(control) = data_win(d1) else {
                h.undefined("control->winGetWindowId() of a NULL control");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let select_pos = d2 as i32;
            lb::set_selected(wm, h, control, &[select_pos]);
            let p = h.popup_ladder_select().clone();
            if id == p.listbox_ladder_select_id || plant(22751) {
                match p.button_ok {
                    Some(b) => {
                        let bid = wm.tree.windows[b].id;
                        wm.win_send_system_msg(h, p.parent, ggm::GBM_SELECTED, win_data(Some(b)), bid as u32);
                    }
                    None => h.undefined("GLM_DOUBLE_CLICKED: buttonOk->winGetWindowId() on NULL"),
                }
            }
        }
        ggm::GEM_EDIT_DONE => {
            let Some(control) = data_win(d1) else {
                h.undefined("control->winGetWindowId() of a NULL control");
                return MsgHandled::Handled;
            };
            let p = h.popup_ladder_select().clone();
            if wm.tree.windows[control].id == p.text_entry_password_id || plant(22758) {
                let ok = wm.win_get_window_from_id(p.password_parent, p.button_password_ok_id);
                wm.win_send_system_msg(h, p.parent, ggm::GBM_SELECTED, win_data(ok), p.button_password_ok_id as u32);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `updateLadderDetails(selID, staticTextLadderName, listboxLadderDetails)` (PopupLadderSelect.cpp:430).
pub fn update_ladder_details<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, sel_id: i32, static_text_ladder_name: Option<WinId>, listbox_ladder_details: Option<WinId>) {
    let (Some(name_win), Some(details)) = (static_text_ladder_name, listbox_ladder_details) else { return };

    gadget::static_text_set_text(wm, h, Some(name_win), &[]);
    lb::reset(wm, h, details);

    let Some(info) = h.ladder_find_by_index(sel_id) else { return };

    let color = WHITE;
    let caption_color = CAPTION_COLOR;

    let fmt = h.fetch_text(b"GUI:LadderNameAndSize");
    let line = h.format_ladder_name_and_size(&fmt, &info.name, info.players_per_team, info.players_per_team);
    gadget::static_text_set_text(wm, h, Some(name_win), &line);

    if !info.location.is_empty() || plant(22757) {
        lb::add_entry_text(wm, h, details, &info.location, caption_color, -1, 0, false);
    }

    let fmt = h.fetch_text(b"GUI:LadderURL");
    let line = h.format_text(&fmt, &info.homepage_url);
    lb::add_entry_text(wm, h, details, &line, caption_color, -1, 0, false);

    if !info.description.is_empty() {
        lb::add_entry_text(wm, h, details, &info.description, color, -1, 0, false);
    }

    if !info.crypted_password.is_empty() {
        let t = h.fetch_text(b"GUI:LadderHasPassword");
        lb::add_entry_text(wm, h, details, &t, caption_color, -1, 0, false);
    }

    if info.min_wins != 0 {
        let fmt = h.fetch_text(b"GUI:LadderMinWins");
        let line = h.format_int(&fmt, if plant(22753) { info.max_wins } else { info.min_wins });
        lb::add_entry_text(wm, h, details, &line, caption_color, -1, 0, false);
    }

    if info.max_wins != 0 {
        let fmt = h.fetch_text(b"GUI:LadderMaxWins");
        let line = h.format_int(&fmt, info.max_wins);
        lb::add_entry_text(wm, h, details, &line, caption_color, -1, 0, false);
    }

    let label: &[u8] = if info.random_factions != plant(22754) { b"GUI:LadderRandomFactions" } else { b"GUI:LadderFactions" };
    let t = h.fetch_text(label);
    lb::add_entry_text(wm, h, details, &t, caption_color, -1, 0, false);

    for f in &info.valid_factions {
        let mut marker = b"INI:Faction".to_vec();
        marker.extend_from_slice(f);
        let t = h.fetch_text(&marker);
        lb::add_entry_text(wm, h, details, &t, color, -1, 0, false);
    }

    let label: &[u8] = if info.random_maps { b"GUI:LadderRandomMaps" } else { b"GUI:LadderMaps" };
    let t = h.fetch_text(label);
    lb::add_entry_text(wm, h, details, &t, caption_color, -1, 0, false);

    for m in &info.valid_maps {
        if let Some(display) = h.map_display_name(m) {
            lb::add_entry_text(wm, h, details, &display, color, -1, 0, false);
        }
    }
}

/// `closeRightClickMenu(win)`: the window's layout is destroyed and freed.
fn close_right_click_menu<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, _h: &mut H, win: WinId) {
    if let Some(l) = wm.ext[win].layout {
        wm.layout_destroy_windows(l);
        wm.layouts[l].deleted = true;
    }
}

/// `RCGameDetailsMenuInit(layout, userData)`: empty.
pub fn rc_game_details_menu_init<H: PopupLadderSelectHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `RCGameDetailsMenuSystem(window, msg, mData1, mData2)` (PopupLadderSelect.cpp:600). The two function-local
/// statics (`ladderInfoID`, `buttonOkID`) are [`PopupLadderSelectHost::rc_game_details_ids`].
pub fn rc_game_details_menu_system<H: PopupLadderSelectHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            let a = ShellHost::name_to_key(h, b"RCGameDetailsMenu.wnd:ButtonLadderDetails") as i32;
            let b = ShellHost::name_to_key(h, b"PopupLadderDetails.wnd:ButtonOk") as i32;
            *h.rc_game_details_ids() = (a, b);
        }
        ggm::CLOSE => close_right_click_menu(wm, h, window),
        gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                h.undefined("control->winGetWindowId() of a NULL control");
                return MsgHandled::Handled;
            };
            let control_id = wm.tree.windows[control].id;
            let selected_id = h.win_user_data_int(wm, window);
            if selected_id == 0 {
                return MsgHandled::Handled;
            }

            close_right_click_menu(wm, h, window);

            let (ladder_info_id, _button_ok_id) = *h.rc_game_details_ids();
            if control_id == ladder_info_id {
                // look up the staging room, then its ladder
                if let Some(_linfo) = h.ladder_for_staging_room(selected_id) {
                    let Some(rc_layout) = h.create_layout(wm, b"Menus/PopupLadderDetails.wnd") else { return MsgHandled::Handled };
                    wm.layout_run(h, rc_layout, 0); // rcMenu->winGetLayout()->runInit()
                    let rc_menu = wm.layouts[rc_layout].windows.first().copied(); // getFirstWindow()
                    if let Some(rc_menu) = rc_menu {
                        wm.win_bring_to_top(rc_menu);
                        wm.win_hide(rc_menu, false);
                        h.win_set_user_data_int(wm, rc_menu, selected_id);
                        h.win_set_lone_window(wm, rc_menu);
                    }
                    let st_id = ShellHost::name_to_key(h, b"PopupLadderDetails.wnd:StaticTextLadderName") as i32;
                    let lb_id = ShellHost::name_to_key(h, b"PopupLadderDetails.wnd:ListBoxLadderDetails") as i32;
                    let st = wm.win_get_window_from_id(None, st_id);
                    let lbx = wm.win_get_window_from_id(None, lb_id);
                    update_ladder_details(wm, h, selected_id, st, lbx);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
