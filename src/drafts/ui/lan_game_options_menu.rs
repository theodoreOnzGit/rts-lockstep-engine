//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanGameOptionsMenu.cpp
//!     (the file's statics, `getNextSelectablePlayer`, `getFirstSelectablePlayer`, `LanPositionStartSpots`,
//!      `playerTooltip`, `StartPressed`, `LANEnableStartButton`, `handleColorSelection`,
//!      `handlePlayerTemplateSelection`, `handleStartPositionSelection`, `handleTeamSelection`,
//!      `handleStartingCashSelection`, `handleLimitSuperweaponsClick`, `lanUpdateSlotList`, `InitLanGameGadgets`,
//!      `DeinitLanGameGadgets`, `LanGameOptionsMenuInit/Shutdown/Update/Input/System`, `updateGameOptions`,
//!      `PostToLanGameOptions`)
//!
//! `TheLAN`'s `LANGameInfo` (slots, accept flags, options) and `LANAPI` requests are the host's: this file is the
//! window logic. The shared helpers of GUIUtil.cpp (`UpdateSlotList`, `Populate*ComboBox`, `EnableAcceptControls`,
//! `positionStartSpots`, `updateMapStartSpots`) are translated in [`crate::client::gui_util`] and
//! [`crate::client::skirmish_game_options`] against the skirmish game; the host calls them for the LAN game.

use crate::client::gadget;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_info::{MAX_SLOTS, PLAYERTEMPLATE_OBSERVER};

/// `GameMakeColor(255,255,255,255)`: the file's global `white`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;
/// `SLOT_PLAYER`, `SLOT_CLOSED` (GameInfo.h SlotState).
pub const SLOT_PLAYER: i32 = 5;
pub const SLOT_CLOSED: i32 = 1;

/// `PostToLanGameType` (LANAPICallbacks.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostToLanGameType {
    SendGameOpts,
    MapBack,
}

/// What `LANGameSlot` / `GameSlot` report to this screen.
#[derive(Clone, Debug, Default)]
pub struct LanSlotInfo {
    pub start_pos: i32,
    pub color: i32,
    pub player_template: i32,
    pub team_number: i32,
    pub state: i32,
    pub is_ai: bool,
    pub is_human: bool,
    pub is_open: bool,
    pub is_occupied: bool,
    pub is_accepted: bool,
    pub is_local_player: bool,
    pub has_map: bool,
    pub name: Vec<u16>,
    /// `getUser()`'s `LANPlayer` (name, login, host), or `None`.
    pub user: Option<(Vec<u8>, Vec<u8>, Vec<u8>)>,
}

/// The file's statics and the two globals (`listboxChatWindowLanGame`, `mapSelectLayout`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanGameOptions {
    pub s_is_initing: bool,
    pub parent_lan_game_options_id: i32,
    pub combo_box_player_id: [i32; MAX_SLOTS],
    pub button_accept_id: [i32; MAX_SLOTS],
    pub combo_box_color_id: [i32; MAX_SLOTS],
    pub combo_box_player_template_id: [i32; MAX_SLOTS],
    pub combo_box_team_id: [i32; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    pub text_entry_chat_id: i32,
    pub text_entry_map_display_id: i32,
    pub button_back_id: i32,
    pub button_start_id: i32,
    pub button_emote_id: i32,
    pub button_select_map_id: i32,
    pub checkbox_limit_superweapons_id: i32,
    pub combo_box_starting_cash_id: i32,
    pub window_map_id: i32,
    pub parent_lan_game_options: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_start: Option<WinId>,
    pub button_select_map: Option<WinId>,
    pub button_emote: Option<WinId>,
    pub text_entry_chat: Option<WinId>,
    pub text_entry_map_display: Option<WinId>,
    pub checkbox_limit_superweapons: Option<WinId>,
    pub combo_box_starting_cash: Option<WinId>,
    pub window_map: Option<WinId>,
    pub combo_box_player: [Option<WinId>; MAX_SLOTS],
    pub button_accept: [Option<WinId>; MAX_SLOTS],
    pub combo_box_color: [Option<WinId>; MAX_SLOTS],
    pub combo_box_player_template: [Option<WinId>; MAX_SLOTS],
    pub combo_box_team: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub listbox_chat_window_lan_game: Option<WinId>,
    pub listbox_chat_window_lan_game_id: i32,
    pub map_select_layout: Option<usize>,
}

/// What the LAN game options screen reaches outside the window code.
pub trait LanGameOptionsHost: ShellHost {
    fn lan_game_options(&mut self) -> &mut LanGameOptions;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    // globals of LANAPICallbacks.cpp
    fn lan_next_screen_take(&mut self) -> Option<Vec<u8>>;
    fn lan_next_screen_is_set(&self) -> bool;
    fn lan_is_shutting_down(&self) -> bool;
    fn set_lan_is_shutting_down(&mut self, v: bool);
    fn lan_button_pushed(&self) -> bool;
    fn set_lan_button_pushed(&mut self, v: bool);
    // TheLAN / LANGameInfo
    fn lan_my_game_exists(&self) -> bool;
    fn lan_am_i_host(&self) -> bool;
    fn lan_game_in_progress(&self) -> bool;
    fn lan_local_slot_num(&self) -> i32;
    fn lan_slot(&self, i: usize) -> Option<LanSlotInfo>;
    fn lan_my_name(&self) -> Vec<u16>;
    fn lan_player_name(&self, i: usize) -> Vec<u16>;
    fn lan_game_map(&self) -> Vec<u8>;
    fn lan_slot_set_color(&mut self, i: usize, color: i32);
    fn lan_slot_set_player_template(&mut self, i: usize, t: i32);
    fn lan_slot_set_start_pos(&mut self, i: usize, p: i32);
    fn lan_slot_set_team_number(&mut self, i: usize, t: i32);
    fn lan_slot_set_state(&mut self, i: usize, s: i32);
    fn lan_slot_set_accept(&mut self, i: usize);
    fn lan_slot_set_nat_behavior_simple(&mut self, i: usize);
    fn lan_game_reset_accepted(&mut self);
    fn lan_game_set_starting_cash(&mut self, cash: u32);
    fn lan_game_starting_cash(&self) -> u32;
    fn lan_game_set_superweapon_restriction(&mut self, v: i32);
    fn lan_game_superweapon_restriction(&self) -> i32;
    fn lan_game_set_map(&mut self, map: &[u8]);
    fn lan_game_reset_map_flags_for_recheck(&mut self);
    fn lan_game_set_map_availability_slot0(&mut self, v: bool);
    fn lan_game_set_map_crc_and_size(&mut self, crc: u32, size: u32);
    fn lan_game_adjust_slots_for_map(&mut self);
    /// `TheLAN->RequestGameOptions(options, true)`; `GenerateGameOptionsString()` is [`Self::generate_game_options_string`].
    fn lan_request_game_options(&mut self, options: &[u8], now: bool);
    fn generate_game_options_string(&mut self) -> Vec<u8>;
    fn lan_request_game_announce(&mut self);
    fn lan_request_game_leave(&mut self);
    fn lan_request_has_map(&mut self);
    fn lan_request_accept(&mut self);
    fn lan_request_game_start(&mut self);
    fn lan_request_game_start_timer(&mut self, seconds: i32);
    fn lan_request_chat(&mut self, text: &[u16], emote: bool);
    fn lan_on_chat_system(&mut self, text: &[u16]);
    fn lan_on_player_leave(&mut self, name: &[u16]);
    fn lan_reset_game_start_timer(&mut self);
    fn lan_exists(&self) -> bool;
    // map cache
    fn map_cache_update(&mut self);
    /// `TheMapCache->findMap(map)`: `(m_numPlayers, m_displayName, m_isOfficial)`.
    fn map_cache_find_map(&self, map: &[u8]) -> Option<(i32, Vec<u16>, bool)>;
    /// `TheMapCache->find(lower)`: `(m_CRC, m_filesize)`.
    fn map_cache_find(&self, lower: &[u8]) -> Option<(u32, u32)>;
    /// `WouldMapTransfer(map)`.
    fn would_map_transfer(&self, map: &[u8]) -> bool;
    // TheGlobalData / TheMultiplayerSettings
    fn net_min_players(&self) -> i32;
    fn multiplayer_num_colors(&self) -> i32;
    fn multiplayer_start_countdown_timer_seconds(&self) -> i32;
    // LANPreferences
    fn lan_prefs_preferred_color(&mut self) -> i32;
    fn lan_prefs_preferred_faction(&mut self) -> i32;
    fn lan_prefs_preferred_map(&mut self) -> Vec<u8>;
    fn lan_prefs_starting_cash(&mut self) -> u32;
    fn lan_prefs_superweapon_restricted(&mut self) -> bool;
    // GUIUtil.cpp for the LAN game (see the module doc)
    fn gui_util_populate_starting_cash(&mut self, wm: &mut GameWindowManager, combo: Option<WinId>);
    fn gui_util_populate_color(&mut self, wm: &mut GameWindowManager, i: usize);
    fn gui_util_populate_player_template(&mut self, wm: &mut GameWindowManager, i: usize, allow_observers: bool);
    fn gui_util_populate_team(&mut self, wm: &mut GameWindowManager, i: usize);
    fn gui_util_update_slot_list(&mut self, wm: &mut GameWindowManager);
    fn gui_util_enable_accept_controls(&mut self, wm: &mut GameWindowManager, enable: bool);
    fn slot_list_updates_enabled(&self) -> bool;
    fn enable_slot_list_updates(&mut self, v: bool);
    /// `positionStartSpots(game, buttons, window)`, `updateMapStartSpots(game, buttons, FALSE)`.
    fn position_start_spots_for_game(&mut self, wm: &mut GameWindowManager);
    fn update_map_start_spots(&mut self, wm: &mut GameWindowManager);
    /// `win->winSetTooltipFunc(name)`.
    fn win_set_tooltip_func(&mut self, wm: &mut GameWindowManager, w: WinId, func: &[u8]);
    /// `GadgetButtonSetEnabledColor(w, acceptTrueColor)`.
    fn button_set_enabled_color_accept_true(&mut self, wm: &mut GameWindowManager, w: WinId);
    /// `chatSystemColor` (LobbyUtils.cpp).
    fn chat_system_color(&self) -> i32;
    /// `TheMouse->setCursor(Mouse::ARROW)`, `setMouseText(empty, NULL, NULL)`, `setCursorTooltip(text)`.
    fn mouse_reset_for_shutdown(&mut self);
    fn mouse_set_cursor_tooltip(&mut self, text: &[u16]);
    /// `UnicodeString::format(fmt, ...)` of the messages below.
    fn format_one_int(&self, fmt: &[u16], a: i32) -> Vec<u16>;
    fn format_two_text(&self, fmt: &[u16], a: &[u16], b: &[u16]) -> Vec<u16>;
    fn format_tooltip_lan_player(&self, fmt: &[u16], name: &[u8], login: &[u8], host: &[u8]) -> Vec<u16>;
}

fn key_of<H: LanGameOptionsHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("LanGameOptionsMenu.wnd:{n}").as_bytes()) as i32
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `getNextSelectablePlayer(start)`.
fn get_next_selectable_player<H: LanGameOptionsHost>(h: &H, start: i32) -> i32 {
    if !h.lan_am_i_host() {
        return -1;
    }
    let mut j = start;
    while j < MAX_SLOTS as i32 {
        if let Some(slot) = h.lan_slot(j as usize) {
            if slot.start_pos == -1 && ((j == h.lan_local_slot_num() && slot.player_template != PLAYERTEMPLATE_OBSERVER) || slot.is_ai) {
                return j;
            }
        }
        j += 1;
    }
    -1
}

/// `getFirstSelectablePlayer(game)`.
fn get_first_selectable_player<H: LanGameOptionsHost>(h: &H) -> i32 {
    let local = h.lan_local_slot_num();
    let slot = h.lan_slot(local.max(0) as usize);
    // `!game->amIHost() || slot && slot->getPlayerTemplate() != PLAYERTEMPLATE_OBSERVER`
    if !h.lan_am_i_host() || slot.as_ref().is_some_and(|s| s.player_template != PLAYERTEMPLATE_OBSERVER) {
        return local;
    }
    for i in 0..MAX_SLOTS {
        if h.lan_slot(i).is_some_and(|s| s.is_ai) {
            return i as i32;
        }
    }
    local
}

/// `LanPositionStartSpots()`.
pub fn lan_position_start_spots<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.position_start_spots_for_game(wm);
}

/// `playerTooltip(window, instData, mouse)`: the tooltip of a slot's combo box edit box.
pub fn player_tooltip<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, window: Option<WinId>) {
    let combos = h.lan_game_options().combo_box_player;
    let mut idx: i32 = -1;
    for i in 0..MAX_SLOTS {
        if let (Some(w), Some(c)) = (window, combos[i]) {
            if Some(w) == cb::get_edit_box(wm, c) {
                idx = i as i32;
                break;
            }
        }
    }
    if idx == -1 {
        return;
    }
    // (upstream indexes with `i`, which the loop leaves at `idx`)
    let Some(slot) = h.lan_slot(idx as usize) else { return };
    let Some((name, login, host)) = slot.user else {
        // DEBUG_ASSERTCRASH(TheLAN->GetMyGame()->getIP(i) == 0, ("No player info in listbox!"));
        h.mouse_set_cursor_tooltip(&[]);
        return;
    };
    let fmt = h.fetch_text(b"TOOLTIP:LANPlayer");
    let tooltip = h.format_tooltip_lan_player(&fmt, &name, &login, &host);
    h.mouse_set_cursor_tooltip(&tooltip);
}

/// A system chat line in the game's chat list (`GadgetListBoxAddEntryText(listboxChatWindowLanGame, ...)`).
fn add_chat_line<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, text: &[u16]) {
    let list = h.lan_game_options().listbox_chat_window_lan_game;
    let color = h.chat_system_color();
    if let Some(l) = list {
        lb::add_entry_text(wm, h, l, text, color, -1, 0, false);
    }
}

/// `StartPressed()` (LanGameOptionsMenu.cpp:240).
pub fn start_pressed<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut is_ready = true;
    let mut all_have_map = true;
    let mut _player_count = 0;

    if !h.lan_my_game_exists() {
        return;
    }

    h.lan_slot_set_accept(0); // cause we are, of course!

    let mut num_users = 0;
    let mut num_humans = 0;
    for i in 0..MAX_SLOTS {
        if let Some(slot) = h.lan_slot(i) {
            if slot.is_occupied && slot.player_template != PLAYERTEMPLATE_OBSERVER {
                if slot.is_human {
                    num_humans += 1;
                }
                num_users += 1;
            }
        }
    }

    // Check for too many players
    let map = h.lan_game_map();
    let md = h.map_cache_find_map(&map);
    if md.as_ref().map_or(true, |m| m.0 < num_users) {
        if h.lan_am_i_host() {
            let fmt = h.fetch_text(b"LAN:TooManyPlayers");
            let text = h.format_one_int(&fmt, md.as_ref().map_or(0, |m| m.0));
            h.lan_on_chat_system(&text);
        }
        return;
    }

    // Check for no human players
    if h.net_min_players() != 0 && num_humans == 0 {
        if h.lan_am_i_host() {
            let text = h.fetch_text(b"GUI:NeedHumanPlayers");
            h.lan_on_chat_system(&text);
        }
        return;
    }

    // Check for too few players
    if num_users < h.net_min_players() {
        if h.lan_am_i_host() {
            let fmt = h.fetch_text(b"LAN:NeedMorePlayers");
            let text = h.format_one_int(&fmt, num_users);
            h.lan_on_chat_system(&text);
        }
        return;
    }

    // Check for too few teams
    let mut num_random = 0;
    let mut teams: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for i in 0..MAX_SLOTS {
        if let Some(slot) = h.lan_slot(i) {
            if slot.is_occupied && slot.player_template != PLAYERTEMPLATE_OBSERVER {
                if slot.team_number >= 0 {
                    teams.insert(slot.team_number);
                } else {
                    num_random += 1;
                }
            }
        }
    }
    if (num_random + teams.len() as i32) < h.net_min_players() {
        if h.lan_am_i_host() {
            let text = h.fetch_text(b"LAN:NeedMoreTeams");
            h.lan_on_chat_system(&text);
        }
        return;
    }
    if num_random + (teams.len() as i32) < 2 {
        let text = h.fetch_text(b"GUI:SandboxMode");
        h.lan_on_chat_system(&text);
    }

    let map_data = h.map_cache_find_map(&map);
    let (map_display_name, will_transfer): (Vec<u16>, bool) = match map_data {
        Some((_, name, official)) => (name, !official),
        None => (map.iter().map(|&c| c as u16).collect(), h.would_map_transfer(&map)),
    };

    for i in 0..MAX_SLOTS {
        let Some(slot) = h.lan_slot(i) else { continue };
        if slot.is_human && !slot.is_accepted {
            is_ready = false;
            if !will_transfer && !slot.has_map {
                let fmt = h.fetch_text(b"GUI:PlayerNoMap");
                let msg = h.format_two_text(&fmt, &slot.name, &map_display_name);
                add_chat_line(wm, h, &msg);
                all_have_map = false;
            }
        }
        if slot.is_human && slot.player_template != PLAYERTEMPLATE_OBSERVER {
            _player_count += 1;
        }
    }

    if is_ready {
        // Defaulting to "closed" for the open slots
        for i in 0..MAX_SLOTS {
            if h.lan_slot(i).is_some_and(|s| s.is_open) {
                h.lan_slot_set_state(i, SLOT_CLOSED);
                if let Some(c) = h.lan_game_options().combo_box_player[i] {
                    cb::set_selected_pos(wm, h, c, SLOT_CLOSED, false);
                }
            }
        }
        let seconds = h.multiplayer_start_countdown_timer_seconds();
        if seconds != 0 {
            h.lan_request_game_start_timer(seconds);
        } else {
            h.lan_request_game_start();
        }
        lan_enable_start_button(wm, h, false);
    } else if all_have_map {
        let text = h.fetch_text(b"GUI:NotifiedStartIntent");
        add_chat_line(wm, h, &text);
        h.lan_request_accept();
    }
}

/// `LANEnableStartButton(enabled)`.
pub fn lan_enable_start_button<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, enabled: bool) {
    let o = h.lan_game_options().clone();
    if let Some(b) = o.button_start {
        wm.win_enable(b, enabled);
    }
    if let Some(b) = o.button_select_map {
        wm.win_enable(b, enabled);
    }
}

/// The tail the four `handle*Selection` functions share: for the host, outside init, re-send the options and refresh
/// the slot list.
fn host_resend<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.lan_game_options().s_is_initing {
        let opts = h.generate_game_options_string();
        h.lan_request_game_options(&opts, true);
        lan_update_slot_list(wm, h);
    }
}

/// The selected item data of a combo box (`GadgetComboBoxGetSelectedPos` then `GetItemData`).
fn selected_item_data<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, combo: Option<WinId>) -> i32 {
    let Some(c) = combo else { return 0 };
    let sel = cb::get_selected_pos(wm, h, c, 0);
    cb::get_item_data(wm, h, c, sel) as i32
}

/// `handleColorSelection(index)`.
fn handle_color_selection<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.lan_game_options().combo_box_color[index];
    let color = selected_item_data(wm, h, combo);
    if !h.lan_my_game_exists() {
        return;
    }
    let Some(slot) = h.lan_slot(index) else { return };
    if color == slot.color {
        return;
    }
    if color >= -1 && color < h.multiplayer_num_colors() {
        let mut color_available = true;
        if color != -1 {
            for i in 0..MAX_SLOTS {
                if let Some(check) = h.lan_slot(i) {
                    if color == check.color && i != index {
                        color_available = false;
                        break;
                    }
                }
            }
        }
        if !color_available {
            return;
        }
    }
    h.lan_slot_set_color(index, color);
    if h.lan_am_i_host() {
        host_resend(wm, h);
    } else {
        if !slot.is_local_player || !h.slot_list_updates_enabled() {
            return;
        }
        h.lan_request_game_options(format!("Color={color}").as_bytes(), true);
    }
}

/// `handlePlayerTemplateSelection(index)`.
fn handle_player_template_selection<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.lan_game_options().combo_box_player_template[index];
    let player_template = selected_item_data(wm, h, combo);
    if !h.lan_my_game_exists() {
        return;
    }
    let Some(slot) = h.lan_slot(index) else { return };
    if player_template == slot.player_template {
        return;
    }
    let old_template = slot.player_template;
    h.lan_slot_set_player_template(index, player_template);

    if old_template == PLAYERTEMPLATE_OBSERVER || player_template == PLAYERTEMPLATE_OBSERVER {
        // Was observer, is not: reset colour and team; is observer now: likewise (the two branches are identical)
        let o = h.lan_game_options().clone();
        if let Some(c) = o.combo_box_color[index] {
            cb::set_selected_pos(wm, h, c, 0, false);
        }
        if let Some(c) = o.combo_box_team[index] {
            cb::set_selected_pos(wm, h, c, 0, false);
        }
        h.lan_slot_set_start_pos(index, -1);
    }

    h.lan_game_reset_accepted();

    if h.lan_am_i_host() {
        host_resend(wm, h);
    } else if h.slot_list_updates_enabled() {
        h.lan_request_game_options(format!("PlayerTemplate={player_template}").as_bytes(), true);
    }
}

/// `handleStartPositionSelection(player, startPos)`.
fn handle_start_position_selection<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, player: i32, start_pos: i32) {
    if !h.lan_my_game_exists() {
        return;
    }
    let Some(slot) = h.lan_slot(player as usize) else { return };
    if start_pos == slot.start_pos {
        return;
    }

    let mut skip = false;
    if start_pos < 0 {
        // always allow setting to "no start pos"
        skip = true;
    }
    if !skip {
        let mut is_available = true;
        for i in 0..MAX_SLOTS as i32 {
            if i != player && h.lan_slot(i as usize).is_some_and(|s| s.start_pos == start_pos) {
                is_available = false;
                break;
            }
        }
        if !is_available {
            return;
        }
    }

    h.lan_slot_set_start_pos(player as usize, start_pos);
    if h.lan_am_i_host() {
        if !h.lan_game_options().s_is_initing {
            h.lan_game_reset_accepted();
            let opts = h.generate_game_options_string();
            h.lan_request_game_options(&opts, true);
            lan_update_slot_list(wm, h);
        }
    } else if h.slot_list_updates_enabled() {
        let sp = h.lan_slot(player as usize).map_or(start_pos, |s| s.start_pos);
        h.lan_request_game_options(format!("StartPos={sp}").as_bytes(), true);
    }
}

/// `handleTeamSelection(index)`.
fn handle_team_selection<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let combo = h.lan_game_options().combo_box_team[index];
    let team = selected_item_data(wm, h, combo);
    if !h.lan_my_game_exists() {
        return;
    }
    let Some(slot) = h.lan_slot(index) else { return };
    if team == slot.team_number {
        return;
    }
    h.lan_slot_set_team_number(index, team);
    h.lan_game_reset_accepted();

    if h.lan_am_i_host() {
        if !h.lan_game_options().s_is_initing {
            h.lan_game_reset_accepted();
            let opts = h.generate_game_options_string();
            h.lan_request_game_options(&opts, true);
            lan_update_slot_list(wm, h);
        }
    } else if h.slot_list_updates_enabled() {
        h.lan_request_game_options(format!("Team={team}").as_bytes(), true);
    }
}

/// `handleStartingCashSelection()`.
fn handle_starting_cash_selection<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.lan_my_game_exists() {
        return;
    }
    let combo = h.lan_game_options().combo_box_starting_cash;
    let cash = selected_item_data(wm, h, combo) as u32;
    h.lan_game_set_starting_cash(cash); // Money::deposit(cash, FALSE)
    h.lan_game_reset_accepted();
    if h.lan_am_i_host() {
        host_resend(wm, h); // Update the accepted button UI
    }
}

/// `handleLimitSuperweaponsClick()`.
fn handle_limit_superweapons_click<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.lan_my_game_exists() {
        return;
    }
    let checked = h.lan_game_options().checkbox_limit_superweapons.is_some_and(|c| gadget::check_box_is_checked(wm, c));
    h.lan_game_set_superweapon_restriction(if checked { 1 } else { 0 });
    h.lan_game_reset_accepted();
    if h.lan_am_i_host() {
        host_resend(wm, h);
    }
}

/// `lanUpdateSlotList()`.
pub fn lan_update_slot_list<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.slot_list_updates_enabled() || h.lan_game_options().s_is_initing {
        return;
    }
    h.gui_util_update_slot_list(wm);
    h.update_map_start_spots(wm);
}

/// `InitLanGameGadgets()` (LanGameOptionsMenu.cpp:593).
pub fn init_lan_game_gadgets<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    let ids = [
        key_of(h, "LanGameOptionsMenuParent"), key_of(h, "ButtonBack"), key_of(h, "ButtonStart"), key_of(h, "TextEntryChat"), key_of(h, "TextEntryMapDisplay"),
        key_of(h, "ListboxChatWindowLanGame"), key_of(h, "ButtonEmote"), key_of(h, "ButtonSelectMap"), key_of(h, "CheckboxLimitSuperweapons"), key_of(h, "ComboBoxStartingCash"), key_of(h, "MapWindow"),
    ];
    let parent = wm.win_get_window_from_id(None, ids[0]);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);
    let wins = [find(wm, ids[6]), find(wm, ids[7]), find(wm, ids[2]), find(wm, ids[1]), find(wm, ids[5]), find(wm, ids[3]), find(wm, ids[4]), find(wm, ids[8]), find(wm, ids[9])];
    {
        let o = h.lan_game_options();
        o.parent_lan_game_options_id = ids[0];
        o.button_back_id = ids[1];
        o.button_start_id = ids[2];
        o.text_entry_chat_id = ids[3];
        o.text_entry_map_display_id = ids[4];
        o.listbox_chat_window_lan_game_id = ids[5];
        o.button_emote_id = ids[6];
        o.button_select_map_id = ids[7];
        o.checkbox_limit_superweapons_id = ids[8];
        o.combo_box_starting_cash_id = ids[9];
        o.window_map_id = ids[10];
        o.parent_lan_game_options = parent;
        o.button_emote = wins[0];
        o.button_select_map = wins[1];
        o.button_start = wins[2];
        o.button_back = wins[3];
        o.listbox_chat_window_lan_game = wins[4];
        o.text_entry_chat = wins[5];
        o.text_entry_map_display = wins[6];
        o.checkbox_limit_superweapons = wins[7];
        o.combo_box_starting_cash = wins[8];
    }
    let combo_cash = wins[8];
    h.gui_util_populate_starting_cash(wm, combo_cash);

    let window_map = wm.win_get_window_from_id(parent, ids[10]);
    h.lan_game_options().window_map = window_map;
    let local_slot_num = h.lan_local_slot_num();
    if let Some(w) = window_map {
        h.win_set_tooltip_func(wm, w, b"MapSelectorTooltip");
    }

    for i in 0..MAX_SLOTS {
        let id_player = key_of(h, &format!("ComboBoxPlayer{i}"));
        let combo_player = wm.win_get_window_from_id(parent, id_player);
        if let Some(c) = combo_player {
            cb::reset(wm, h, c);
            if let Some(e) = cb::get_edit_box(wm, c) {
                h.win_set_tooltip_func(wm, e, b"playerTooltip");
            }
            if local_slot_num == i as i32 {
                let name = h.lan_my_name();
                cb::add_entry(wm, h, c, &name, WHITE);
            } else {
                for label in [&b"GUI:Open"[..], b"GUI:Closed", b"GUI:EasyAI", b"GUI:MediumAI", b"GUI:HardAI"] {
                    let t = h.fetch_text(label);
                    cb::add_entry(wm, h, c, &t, WHITE);
                }
                cb::set_selected_pos(wm, h, c, 0, false);
            }
        }

        let id_color = key_of(h, &format!("ComboBoxColor{i}"));
        let combo_color = wm.win_get_window_from_id(parent, id_color);
        h.lan_game_options().combo_box_color[i] = combo_color;
        h.gui_util_populate_color(wm, i);
        if let Some(c) = combo_color {
            cb::set_selected_pos(wm, h, c, 0, false);
        }

        let id_tpl = key_of(h, &format!("ComboBoxPlayerTemplate{i}"));
        let combo_tpl = wm.win_get_window_from_id(parent, id_tpl);
        h.lan_game_options().combo_box_player_template[i] = combo_tpl;
        h.gui_util_populate_player_template(wm, i, true);
        if let Some(c) = combo_tpl {
            h.win_set_tooltip_func(wm, c, b"playerTemplateComboBoxTooltip");
            if let Some(l) = cb::get_list_box(wm, c) {
                h.win_set_tooltip_func(wm, l, b"playerTemplateListBoxTooltip");
            }
        }

        let id_team = key_of(h, &format!("ComboBoxTeam{i}"));
        let combo_team = wm.win_get_window_from_id(parent, id_team);
        h.lan_game_options().combo_box_team[i] = combo_team;
        h.gui_util_populate_team(wm, i);

        let id_accept = key_of(h, &format!("ButtonAccept{i}"));
        let accept = wm.win_get_window_from_id(parent, id_accept);
        if let Some(a) = accept {
            h.win_set_tooltip_func(wm, a, b"gameAcceptTooltip");
        }

        let id_pos = key_of(h, &format!("ButtonMapStartPosition{i}"));
        let pos = wm.win_get_window_from_id(parent, id_pos);

        {
            let o = h.lan_game_options();
            o.combo_box_player_id[i] = id_player;
            o.combo_box_player[i] = combo_player;
            o.combo_box_color_id[i] = id_color;
            o.combo_box_player_template_id[i] = id_tpl;
            o.combo_box_team_id[i] = id_team;
            o.button_accept_id[i] = id_accept;
            o.button_accept[i] = accept;
            o.button_map_start_position_id[i] = id_pos;
            o.button_map_start_position[i] = pos;
        }

        if i != 0 {
            if let Some(a) = accept {
                wm.win_hide(a, true);
            }
        }
    }

    if let Some(a) = h.lan_game_options().button_accept[0] {
        h.button_set_enabled_color_accept_true(wm, a);
    }
}

/// `DeinitLanGameGadgets()`.
pub fn deinit_lan_game_gadgets<H: LanGameOptionsHost>(h: &mut H) {
    let o = h.lan_game_options();
    o.parent_lan_game_options = None;
    o.button_emote = None;
    o.button_select_map = None;
    o.button_start = None;
    o.button_back = None;
    o.listbox_chat_window_lan_game = None;
    o.text_entry_chat = None;
    o.text_entry_map_display = None;
    o.checkbox_limit_superweapons = None;
    o.combo_box_starting_cash = None;
    o.window_map = None;
    o.combo_box_player = [None; MAX_SLOTS];
    o.combo_box_color = [None; MAX_SLOTS];
    o.combo_box_player_template = [None; MAX_SLOTS];
    o.combo_box_team = [None; MAX_SLOTS];
    o.button_accept = [None; MAX_SLOTS];
    o.button_map_start_position = [None; MAX_SLOTS];
}

/// `LanGameOptionsMenuInit(layout, userData)` (LanGameOptionsMenu.cpp:752).
pub fn lan_game_options_menu_init<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.lan_my_game_exists() && h.lan_game_in_progress() {
        // DEBUG_LOG(("Popping to lobby after a game!\n"));
        shell::pop_immediate(wm, h);
        return;
    }

    h.lan_game_options().s_is_initing = true;

    h.set_lan_button_pushed(false);
    h.set_lan_is_shutting_down(false);

    h.enable_slot_list_updates(false);
    init_lan_game_gadgets(wm, h);
    h.enable_slot_list_updates(true);

    let mut start = 0;
    let o = h.lan_game_options().clone();
    if let Some(l) = o.listbox_chat_window_lan_game {
        lb::reset(wm, h, l);
    }
    te::set_text(wm, h, o.text_entry_chat, &[]);

    h.map_cache_update();

    if h.lan_am_i_host() {
        let color = h.lan_prefs_preferred_color();
        let faction = h.lan_prefs_preferred_faction();
        h.lan_slot_set_color(0, color);
        h.lan_slot_set_player_template(0, faction);
        h.lan_slot_set_nat_behavior_simple(0);
        let preferred_map = h.lan_prefs_preferred_map();
        h.lan_game_set_map(&preferred_map);
        let cash = h.lan_prefs_starting_cash();
        h.lan_game_set_starting_cash(cash);
        let sw = h.lan_prefs_superweapon_restricted();
        h.lan_game_set_superweapon_restriction(if sw { 1 } else { 0 });

        let lower_map = preferred_map.to_ascii_lowercase();
        if let Some((crc, size)) = h.map_cache_find(&lower_map) {
            h.lan_game_set_map_availability_slot0(true);
            h.lan_game_set_map_crc_and_size(crc, size);
            h.lan_game_adjust_slots_for_map(); // BGC- adjust the slots for the selected map.
        }

        lan_update_slot_list(wm, h);
        update_game_options(wm, h);

        start = 1; // leave my combo boxes usable
    } else {
        let accept = h.fetch_text(b"GUI:Accept");
        if let Some(b) = o.button_start {
            wm.win_set_text(b, &accept);
        }
        if let Some(b) = o.button_select_map {
            wm.win_enable(b, false);
        }
        if let Some(c) = o.checkbox_limit_superweapons {
            wm.win_enable(c, false); // Can look but only host can touch
        }
        if let Some(c) = o.combo_box_starting_cash {
            wm.win_enable(c, false); // Ditto
        }

        h.lan_game_reset_map_flags_for_recheck(); // force a recheck of if we have the map
        h.lan_request_has_map();

        lan_update_slot_list(wm, h);
        update_game_options(wm, h);
    }

    for i in start..MAX_SLOTS {
        if !h.lan_am_i_host() {
            if let Some(c) = o.combo_box_player[i] {
                wm.win_enable(c, false);
            }
        }
        for w in [o.combo_box_color[i], o.combo_box_player_template[i], o.combo_box_team[i]].into_iter().flatten() {
            wm.win_enable(w, false);
        }
    }

    // Show the Menu
    wm.layout_hide(layout, false);

    // Set Keyboard to Main Parent
    wm.win_set_focus(h, o.parent_lan_game_options);

    h.lan_game_options().s_is_initing = false;

    if h.lan_am_i_host() {
        let opts = h.generate_game_options_string();
        h.lan_request_game_options(&opts, true);
        h.lan_request_game_announce();
    }

    lan_update_slot_list(wm, h);
    lan_position_start_spots(wm, h);

    wt::set_group(wm, h, b"LanGameOptionsFade", false);
}

/// `updateGameOptions()`.
pub fn update_game_options<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !(h.lan_my_game_exists() && h.slot_list_updates_enabled()) {
        return;
    }
    let o = h.lan_game_options().clone();
    let local = h.lan_local_slot_num();
    let local_has_map = h.lan_slot(local.max(0) as usize).is_some_and(|s| s.has_map);
    let map = h.lan_game_map();
    let map_data = h.map_cache_find_map(&map);
    let map_display_name: Vec<u16> = match map_data {
        Some((_, name, _)) if local_has_map => name,
        _ => {
            // s = s.reverseFind('\\') + 1
            let leaf: &[u8] = match map.iter().rposition(|&c| c == b'\\') {
                Some(p) => &map[p + 1..],
                None => &map,
            };
            leaf.iter().map(|&c| c as u16).collect()
        }
    };

    let old = o.text_entry_map_display.map(|w| gadget::static_text_get_text(wm, w)).unwrap_or_default();
    if old != map_display_name {
        lan_position_start_spots(wm, h);
    }

    gadget::static_text_set_text(wm, h, o.text_entry_map_display, &map_display_name);

    if let Some(c) = o.checkbox_limit_superweapons {
        let v = h.lan_game_superweapon_restriction() != 0;
        gadget::check_box_set_checked(wm, h, c, v);
    }

    if let Some(c) = o.combo_box_starting_cash {
        let item_count = cb::get_length(wm, c);
        let cash = h.lan_game_starting_cash() as i32;
        for index in 0..item_count {
            let value = cb::get_item_data(wm, h, c, index) as i32;
            if value == cash {
                cb::set_selected_pos(wm, h, c, index, true);
                break;
            }
        }
        // DEBUG_ASSERTCRASH(index < itemCount, ("Could not find new starting cash amount %d in list", ...))
    }
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    deinit_lan_game_gadgets(h);
    h.lan_game_options().text_entry_map_display = None;

    h.set_lan_is_shutting_down(false);

    // hide menu
    wm.layout_hide(layout, true);

    // our shutdown is complete
    let has_next = h.lan_next_screen_is_set();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    if let Some(next) = h.lan_next_screen_take() {
        shell::push(wm, h, &next, false);
    }
}

/// `LanGameOptionsMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn lan_game_options_menu_shutdown<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    h.mouse_reset_for_shutdown();
    h.enable_slot_list_updates(false);
    h.set_lan_is_shutting_down(true);

    // if we are doing an immediate pop, just do it
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"LanGameOptionsFade");
    if h.lan_exists() {
        h.lan_reset_game_start_timer();
    }
}

/// `LanGameOptionsMenuUpdate(layout, userData)`.
pub fn lan_game_options_menu_update<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.lan_is_shutting_down() && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }
}

/// `LanGameOptionsMenuInput(window, msg, mData1, mData2)`.
pub fn lan_game_options_menu_input<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        if h.lan_button_pushed() {
            return MsgHandled::Ignored;
        }
        if (d1 & 0xFF) as u8 == key::ESC {
            if (d2 & 0xFF) & key::STATE_UP != 0 {
                let o = h.lan_game_options().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(o.button_back), o.button_back_id as u32);
            }
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// The player holding start position `i`, if any.
fn player_idx_in_pos<H: LanGameOptionsHost>(h: &H, i: i32) -> i32 {
    for j in 0..MAX_SLOTS {
        if h.lan_slot(j).is_some_and(|s| s.start_pos == i) {
            return j as i32;
        }
    }
    -1
}

/// `LanGameOptionsMenuSystem(window, msg, mData1, mData2)` (LanGameOptionsMenu.cpp:1000).
pub fn lan_game_options_menu_system<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    let control_id = |wm: &GameWindowManager| data_win(d1).map(|c| wm.tree.windows[c].id);
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GCM_SELECTED => {
            if h.lan_button_pushed() {
                return MsgHandled::Handled;
            }
            let Some(id) = control_id(wm) else { return MsgHandled::Handled };
            let o = h.lan_game_options().clone();
            if id == o.combo_box_starting_cash_id {
                handle_starting_cash_selection(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if id == o.combo_box_color_id[i] {
                        handle_color_selection(wm, h, i);
                    } else if id == o.combo_box_player_template_id[i] {
                        handle_player_template_selection(wm, h, i);
                    } else if id == o.combo_box_team_id[i] {
                        handle_team_selection(wm, h, i);
                    } else if id == o.combo_box_player_id[i] && h.lan_am_i_host() {
                        // We don't have anything to do if we're the host and the local slot's combo box changed
                        if i as i32 == h.lan_local_slot_num() {
                            break;
                        }
                        let pos = match o.combo_box_player[i] {
                            Some(c) => cb::get_selected_pos(wm, h, c, -1),
                            None => -1,
                        };
                        if pos != SLOT_PLAYER && pos >= 0 {
                            let state = h.lan_slot(i).map_or(0, |s| s.state);
                            if state == SLOT_PLAYER {
                                let name = h.lan_player_name(i);
                                h.lan_slot_set_state(i, pos);
                                h.lan_game_reset_accepted();
                                h.lan_on_player_leave(&name);
                            } else if state != pos {
                                let was_ai = h.lan_slot(i).is_some_and(|s| s.is_ai);
                                h.lan_slot_set_state(i, pos);
                                let is_ai = h.lan_slot(i).is_some_and(|s| s.is_ai);
                                if was_ai || is_ai {
                                    h.lan_game_reset_accepted();
                                }
                                if was_ai ^ is_ai {
                                    h.gui_util_populate_player_template(wm, i, was_ai);
                                }
                                if !h.lan_game_options().s_is_initing {
                                    let opts = h.generate_game_options_string();
                                    h.lan_request_game_options(&opts, true);
                                    lan_update_slot_list(wm, h);
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
        ggm::GBM_SELECTED => {
            if h.lan_button_pushed() {
                return MsgHandled::Handled;
            }
            let Some(id) = control_id(wm) else { return MsgHandled::Handled };
            let o = h.lan_game_options().clone();
            if id == o.button_back_id {
                // clean up the map select layout
                if let Some(l) = h.lan_game_options().map_select_layout.take() {
                    wm.layout_destroy_windows(l);
                    wm.layouts[l].deleted = true;
                }
                h.lan_request_game_leave();
            } else if id == o.button_emote_id {
                // read the user's input
                let mut txt = te::get_text(wm, h, o.text_entry_chat);
                te::set_text(wm, h, o.text_entry_chat, &[]);
                trim(&mut txt);
                if !txt.is_empty() {
                    h.lan_request_chat(&txt, true);
                }
            } else if id == o.button_select_map_id {
                let layout = h.create_layout(wm, b"Menus/LanMapSelectMenu.wnd");
                h.lan_game_options().map_select_layout = layout;
                if let Some(l) = layout {
                    wm.layout_run(h, l, 0); // runInit
                    wm.layout_hide(l, false);
                    wm.layout_bring_forward(l);
                }
            } else if id == o.button_start_id {
                if h.lan_am_i_host() {
                    start_pressed(wm, h);
                } else {
                    h.lan_request_accept();
                    h.gui_util_enable_accept_controls(wm, true);
                }
            } else if id == o.checkbox_limit_superweapons_id {
                handle_limit_superweapons_click(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if id == o.button_map_start_position_id[i] {
                        let idx = player_idx_in_pos(h, i as i32);
                        if idx >= 0 {
                            let is_ai = h.lan_slot(idx as usize).is_some_and(|s| s.is_ai);
                            if idx == h.lan_local_slot_num() || (h.lan_am_i_host() && is_ai) {
                                let next_player = get_next_selectable_player(h, idx + 1);
                                handle_start_position_selection(wm, h, idx, -1);
                                if next_player >= 0 {
                                    handle_start_position_selection(wm, h, next_player, i as i32);
                                }
                            }
                        } else {
                            let mut next_player = get_next_selectable_player(h, 0);
                            if next_player < 0 {
                                next_player = get_first_selectable_player(h);
                            }
                            handle_start_position_selection(wm, h, next_player, i as i32);
                        }
                    }
                }
            }
        }
        ggm::GBM_SELECTED_RIGHT => {
            if h.lan_button_pushed() {
                return MsgHandled::Handled;
            }
            let Some(id) = control_id(wm) else { return MsgHandled::Handled };
            let o = h.lan_game_options().clone();
            for i in 0..MAX_SLOTS {
                if id == o.button_map_start_position_id[i] {
                    let idx = player_idx_in_pos(h, i as i32);
                    if idx >= 0 {
                        let is_ai = h.lan_slot(idx as usize).is_some_and(|s| s.is_ai);
                        if idx == h.lan_local_slot_num() || (h.lan_am_i_host() && is_ai) {
                            handle_start_position_selection(wm, h, idx, -1);
                        }
                    }
                }
            }
        }
        ggm::GEM_EDIT_DONE => {
            if h.lan_button_pushed() {
                return MsgHandled::Handled;
            }
            let Some(id) = control_id(wm) else { return MsgHandled::Handled };
            let o = h.lan_game_options().clone();
            if id == o.text_entry_chat_id {
                let mut txt = te::get_text(wm, h, o.text_entry_chat);
                te::set_text(wm, h, o.text_entry_chat, &[]);
                trim(&mut txt);
                if !txt.is_empty() {
                    h.lan_request_chat(&txt, false);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `UnicodeString::trim()`: strips leading and trailing whitespace.
fn trim(s: &mut Vec<u16>) {
    let is_space = |c: &u16| *c == ' ' as u16 || (*c >= 9 && *c <= 13);
    while s.last().is_some_and(is_space) {
        s.pop();
    }
    let lead = s.iter().take_while(|c| is_space(c)).count();
    s.drain(..lead);
}

/// `PostToLanGameOptions(post)` (LanGameOptionsMenu.cpp:1387): a bridge from the map picker to this screen.
pub fn post_to_lan_game_options<H: LanGameOptionsHost>(wm: &mut GameWindowManager, h: &mut H, post: PostToLanGameType) {
    lan_position_start_spots(wm, h);
    match post {
        PostToLanGameType::SendGameOpts => {
            h.lan_game_reset_accepted();
            update_game_options(wm, h);
            lan_update_slot_list(wm, h);

            // Clear the start positions
            for i in 0..MAX_SLOTS {
                h.lan_slot_set_start_pos(i, -1);
            }

            let opts = h.generate_game_options_string();
            h.lan_request_game_options(&opts, true);
        }
        PostToLanGameType::MapBack => {}
    }
}
