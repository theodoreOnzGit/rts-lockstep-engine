//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLWelcomeMenu.cpp
//!     (the file's statics, `updateServerDisplay`, `enableControls`, `shutdownComplete`, `grabUByte`,
//!      `updateNumPlayersOnline`, `HandleNumPlayersOnline`, `FindNextNumber`, `HandleOverallStats`,
//!      `updateOverallStats`, `UpdateLocalPlayerStats`, `WOLWelcomeMenuInit/Shutdown/Update/Input/System`)
//!
//! The commented-out `updateLocalPlayerScores` is not translated. The GameSpy service is gone: `TheGameSpyInfo`,
//! the peer and buddy queues, `TheFirewallHelper` and the overlays are the host's; the peer response loop in
//! `Update` keeps upstream's structure over [`WolWelcomeMenuHost::peer_next_response`].

use crate::client::gadget;
use crate::client::gadget_list_box as lb;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;

/// `LOC_MIN`, `LOC_MAX`.
const LOC_MIN: i32 = crate::drafts::ui::wol_locale_select_popup::LOC_MIN;
const LOC_MAX: i32 = crate::drafts::ui::wol_locale_select_popup::LOC_MAX;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolWelcomeMenu {
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<Vec<u8>>,
    pub parent_wol_welcome_id: i32,
    pub button_back_id: i32,
    pub button_quick_match_id: i32,
    pub button_lobby_id: i32,
    pub button_buddies_id: i32,
    pub button_ladder_id: i32,
    pub button_my_info_id: i32,
    pub listbox_info_id: i32,
    pub button_options_id: i32,
    pub parent_wol_welcome: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_quick_match: Option<WinId>,
    pub button_lobby: Option<WinId>,
    pub button_buddies: Option<WinId>,
    pub button_ladder: Option<WinId>,
    pub button_my_info: Option<WinId>,
    pub button_button_options: Option<WinId>,
    pub welcome_layout: Option<usize>,
    pub listbox_info: Option<WinId>,
    pub static_text_server_name: Option<WinId>,
    pub static_text_last_updated: Option<WinId>,
    pub static_text_ladder_wins: Option<WinId>,
    pub static_text_ladder_losses: Option<WinId>,
    pub static_text_ladder_rank: Option<WinId>,
    pub static_text_ladder_points: Option<WinId>,
    pub static_text_ladder_disconnects: Option<WinId>,
    pub static_text_highscore_wins: Option<WinId>,
    pub static_text_highscore_losses: Option<WinId>,
    pub static_text_highscore_rank: Option<WinId>,
    pub static_text_highscore_points: Option<WinId>,
    /// `static UnicodeString gServerName`.
    pub g_server_name: Vec<u16>,
    pub last_num_players_online: i32,
    /// `s_winStats` (side, fraction) in the map's key order, and `s_totalWinPercent`.
    pub win_stats: Vec<(Vec<u8>, f32)>,
    pub total_win_percent: f32,
    pub raise_message_boxes: bool,
}

/// The `PeerResponse` types `Update` acts on.
#[derive(Clone, Debug)]
pub enum WelcomePeerResponse {
    /// `PEERRESPONSE_GROUPROOM`: the group room to add (`TheGameSpyInfo->addGroupRoom`), and its id.
    GroupRoom { group_id: i32, max_waiting: i32, name: Vec<u8>, num_games: i32, num_playing: i32, num_waiting: i32 },
    /// `PEERRESPONSE_JOINGROUPROOM`.
    JoinGroupRoom { ok: bool, id: i32 },
    /// `PEERRESPONSE_DISCONNECT`.
    Disconnect { reason: i32 },
    /// Any other response type (ignored by the switch).
    Other,
}

/// What the welcome menu reaches in the GameSpy layer and the engine.
pub trait WolWelcomeMenuHost: ShellHost {
    fn wol_welcome_menu(&mut self) -> &mut WolWelcomeMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16>;
    fn format_text_arg(&self, fmt: &[u16], text: &[u8]) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_MOTD_HEADING]`, `[GSCOLOR_MOTD]`.
    fn gamespy_color_motd_heading(&self) -> i32;
    fn gamespy_color_motd(&self) -> i32;
    /// `TheGameSpyInfo != NULL`; `getMOTD()`; `getLocalBaseName()`; `gotGroupRoomList()`;
    /// `getMaxMessagesPerUpdate()`; `getLocalProfileID()`, `getLocalName()`.
    fn has_gamespy_info(&self) -> bool;
    fn gamespy_motd(&self) -> Vec<u8>;
    fn gamespy_local_base_name(&self) -> Vec<u8>;
    fn gamespy_got_group_room_list(&self) -> bool;
    fn gamespy_max_messages_per_update(&self) -> i32;
    fn gamespy_local_profile_id(&self) -> i32;
    fn gamespy_local_name(&self) -> Vec<u16>;
    fn gamespy_add_group_room(&mut self, group_id: i32, max_waiting: i32, name: &[u8], num_games: i32, num_playing: i32, num_waiting: i32);
    fn gamespy_set_current_group_room(&mut self, id: i32);
    fn gamespy_join_best_group_room(&mut self);
    /// `MultiByteToWideCharSingleLine(line)`.
    fn multi_byte_to_wide_char(&self, line: &[u8]) -> Vec<u16>;
    /// The player templates: (side, isPlayableSide) in store order.
    fn player_template_sides(&self) -> Vec<(Vec<u8>, bool)>;
    /// `PopulatePlayerInfoWindows(parentWindowName)`.
    fn populate_player_info_windows(&mut self, wm: &mut GameWindowManager, parent_name: &[u8]);
    // firewall helper
    fn firewall_helper_exists(&self) -> bool;
    fn firewall_helper_create(&mut self);
    fn firewall_helper_detect_firewall(&mut self) -> bool;
    fn firewall_helper_delete(&mut self);
    /// `behaviorDetectionUpdate()`; when true the host stores the behaviour in the global data, writes it and
    /// flags `flagNeedToRefresh(FALSE)`.
    fn firewall_helper_behavior_detection_update_and_store(&mut self) -> bool;
    // GameSpy overlays and messages
    fn gamespy_open_overlay_locale_select(&mut self, wm: &mut GameWindowManager);
    fn gamespy_open_overlay_options(&mut self, wm: &mut GameWindowManager);
    fn gamespy_toggle_overlay_player_info(&mut self, wm: &mut GameWindowManager);
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager);
    fn raise_gs_message_box(&mut self);
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]);
    fn set_look_at_player(&mut self, profile_id: i32, name: &[u16]);
    /// `GameSpyMiscPreferences`: `getLocale()`, `getQuickMatchResLocked()`.
    fn misc_prefs_locale(&mut self) -> i32;
    fn misc_prefs_quick_match_res_locked(&mut self) -> bool;
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
    // queues
    fn handle_buddy_responses(&mut self);
    fn handle_persistent_storage_responses(&mut self);
    fn has_peer_message_queue(&self) -> bool;
    fn peer_next_response(&mut self) -> Option<WelcomePeerResponse>;
    /// Logout: `PEERREQUEST_LOGOUT`, `BUDDYREQUEST_LOGOUT`, `TearDownGameSpy()`.
    fn peer_buddy_logout_and_tear_down(&mut self);
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn key_of<H: WolWelcomeMenuHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("WOLWelcomeMenu.wnd:{n}").as_bytes()) as i32
}

/// `updateServerDisplay(serverName)`.
pub fn update_server_display<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, server_name: &[u16]) {
    let w = h.wol_welcome_menu().static_text_server_name;
    if w.is_some() {
        gadget::static_text_set_text(wm, h, w, server_name);
    }
    h.wol_welcome_menu().g_server_name = server_name.to_vec();
}

/// `enableControls(state)`.
fn enable_controls<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, state: bool) {
    let m = h.wol_welcome_menu().clone();
    if let Some(b) = m.button_quick_match {
        wm.win_enable(b, state);
    }
    if let Some(b) = m.button_lobby {
        wm.win_enable(b, state);
    }
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_welcome_menu().is_shutting_down = false;

    // hide menu
    wm.layout_hide(layout, true);

    // our shutdown is complete
    let has_next = h.wol_welcome_menu().next_screen.is_some();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    if let Some(next) = h.wol_welcome_menu().next_screen.take() {
        shell::push(wm, h, &next, false);
    }
}

/// `grabUByte(s)`: two hex digits (`strtol("0x" + s[0..2], 16)`; a bad digit stops the parse).
fn grab_ubyte(s: &[u8]) -> u8 {
    let mut v: u32 = 0;
    for &c in s.iter().take(2) {
        match (c as char).to_digit(16) {
            Some(d) => v = v * 16 + d,
            None => break,
        }
    }
    v as u8
}

/// `UnicodeString::nextToken(&line, "\n")` over a whole string: the "\n"-separated tokens (runs of the delimiter
/// are skipped, as `nextToken` does).
fn tokens<T: Copy + PartialEq>(s: &[T], nl: T) -> Vec<Vec<T>> {
    s.split(|&c| c == nl).filter(|t| !t.is_empty()).map(|t| t.to_vec()).collect()
}

/// `updateNumPlayersOnline()` (WOLWelcomeMenu.cpp:215).
fn update_num_players_online<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let id = key_of(h, "StaticTextNumPlayersOnline");
    if let Some(w) = wm.win_get_window_from_id(None, id) {
        let fmt = h.fetch_text(b"GUI:NumPlayersOnline");
        let n = h.wol_welcome_menu().last_num_players_online;
        let val_str = h.format_int(&fmt, n);
        gadget::static_text_set_text(wm, h, Some(w), &val_str);
    }

    let list = h.wol_welcome_menu().listbox_info;
    if let (Some(list), true) = (list, h.has_gamespy_info()) {
        lb::reset(wm, h, list);
        let a_motd = h.gamespy_motd();

        let heading_str = h.fetch_text(b"MOTD:NumPlayersHeading");
        let heading_color = h.gamespy_color_motd_heading();
        for mut line in tokens(&heading_str, '\n' as u16) {
            if line.last() == Some(&('\r' as u16)) {
                line.pop(); // there is a trailing '\r'
            }
            let mut line = crate::client::replay_menu::trim(&line);
            if line.is_empty() {
                line = utf16(" ");
            }
            lb::add_entry_text(wm, h, list, &line, heading_color, -1, -1, false);
        }
        lb::add_entry_text(wm, h, list, &utf16(" "), heading_color, -1, -1, false);

        for mut a_line in tokens(&a_motd, b'\n') {
            if a_line.last() == Some(&b'\r') {
                a_line.pop(); // there is a trailing '\r'
            }
            // aLine.trim()
            while a_line.last().is_some_and(|c| c.is_ascii_whitespace()) {
                a_line.pop();
            }
            let lead = a_line.iter().take_while(|c| c.is_ascii_whitespace()).count();
            a_line.drain(..lead);
            if a_line.is_empty() {
                a_line = b" ".to_vec();
            }

            let mut c = h.gamespy_color_motd();
            if a_line.starts_with(b"\\\\") {
                a_line = a_line[1..].to_vec();
            } else if a_line.starts_with(b"\\") && a_line.len() > 9 {
                let a = grab_ubyte(&a_line[1..]);
                let r = grab_ubyte(&a_line[3..]);
                let g = grab_ubyte(&a_line[5..]);
                let b = grab_ubyte(&a_line[7..]);
                c = crate::client::window_transitions::game_make_color(r, g, b, a);
                a_line = a_line[9..].to_vec();
            }
            let line = h.multi_byte_to_wide_char(&a_line);
            lb::add_entry_text(wm, h, list, &line, c, -1, -1, false);
        }
    }
}

/// `HandleNumPlayersOnline(numPlayersOnline)`.
pub fn handle_num_players_online<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, num_players_online: i32) {
    let n = if num_players_online < 1 { 1 } else { num_players_online };
    h.wol_welcome_menu().last_num_players_online = n;
    update_num_players_online(wm, h);
}

/// `FindNextNumber(pStart)`: the offset of the first digit after the next newline, or `start` itself when there
/// is no newline.
fn find_next_number(s: &[u8], start: usize) -> usize {
    let Some(nl) = s[start..].iter().position(|&c| c == b'\n') else { return start }; // error
    let mut p = start + nl;
    while p < s.len() && !s[p].is_ascii_digit() {
        p += 1; // go to next number
    }
    p
}

/// `atof` of the number at `s[p..]`.
fn atof(s: &[u8], p: usize) -> f64 {
    let end = s[p..].iter().position(|c| !(c.is_ascii_digit() || *c == b'.' || *c == b'-')).map_or(s.len(), |e| p + e);
    std::str::from_utf8(&s[p..end]).ok().and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.0)
}

/// `HandleOverallStats(szHTTPStats, len)` (WOLWelcomeMenu.cpp:318).
pub fn handle_overall_stats<H: WolWelcomeMenuHost>(h: &mut H, sz_http_stats: &[u8]) {
    let find = |hay: &[u8], from: usize, needle: &[u8]| -> Option<usize> {
        hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
    };
    let Some(p_today) = find(sz_http_stats, 0, b"Today") else {
        // error: DEBUG_LOG(( "Unable to parse win/loss stats.  Could not find 'Today' in:\n%s\n", ... ))
        return;
    };

    let mut stats: Vec<(Vec<u8>, f32)> = Vec::new();
    let mut total = 0.0f32;
    for (side, playable) in h.player_template_sides() {
        // get current side (USA, GLA, etc.)
        if !playable || side == b"Boss" {
            continue; // skip non-players
        }
        let side = if side == b"America" { b"USA".to_vec() } else { side }; // conform to GameSpy name
        let Some(p_side) = find(sz_http_stats, p_today, &side) else {
            // error, skip this side
            continue;
        };
        let p_total = find_next_number(sz_http_stats, p_side);
        let p_wins = find_next_number(sz_http_stats, p_total);
        let percent = (atof(sz_http_stats, p_wins) / atof(sz_http_stats, p_total).max(1.0)) as f32; // max prevents divide by zero
        total += percent;
        // std::map::insert keeps the first of a duplicate key and orders by key
        if !stats.iter().any(|(s, _)| *s == side) {
            stats.push((side, percent));
        }
    }
    stats.sort_by(|a, b| a.0.cmp(&b.0));
    let m = h.wol_welcome_menu();
    m.win_stats = stats;
    m.total_win_percent = total;
}

/// `updateOverallStats()`.
fn update_overall_stats<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.wol_welcome_menu().total_win_percent <= 0.0 {
        h.wol_welcome_menu().total_win_percent = 1.0; // prevent divide by zero
    }
    let m = h.wol_welcome_menu().clone();
    for (side, frac) in &m.win_stats {
        let percent = (100.0f32 * (frac / m.total_win_percent)) as i32; // REAL_TO_INT
        let fmt = h.fetch_text(b"GUI:WinPercent");
        let perc_str = h.format_int(&fmt, percent);
        let id = ShellHost::name_to_key(h, format!("WOLWelcomeMenu.wnd:Percent{}", String::from_utf8_lossy(side)).as_bytes()) as i32;
        if let Some(w) = wm.win_get_window_from_id(None, id) {
            wm.win_set_text(w, &perc_str); // GadgetCheckBoxSetText
        }
    }
}

/// `UpdateLocalPlayerStats()`.
pub fn update_local_player_stats<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let id = key_of(h, "WOLWelcomeMenuParent");
    if wm.win_get_window_from_id(None, id).is_some() {
        h.populate_player_info_windows(wm, b"WOLWelcomeMenu.wnd");
    } else {
        h.populate_player_info_windows(wm, b"WOLQuickMatchMenu.wnd");
    }
}

/// `WOLWelcomeMenuInit(layout, userData)` (WOLWelcomeMenu.cpp:462).
pub fn wol_welcome_menu_init<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    {
        let m = h.wol_welcome_menu();
        m.next_screen = None;
        m.button_pushed = false;
        m.is_shutting_down = false;
        m.welcome_layout = Some(layout);
    }

    let parent_id = key_of(h, "WOLWelcomeMenuParent");
    let back_id = key_of(h, "ButtonBack");
    let parent = wm.win_get_window_from_id(None, parent_id);
    let back = wm.win_get_window_from_id(None, back_id);
    let options_id = key_of(h, "ButtonOptions");
    let options = wm.win_get_window_from_id(None, options_id);
    let info_id = key_of(h, "InfoListbox");
    let info = wm.win_get_window_from_id(None, info_id);

    let mut under_parent = |h: &mut H, wm: &GameWindowManager, n: &str| -> Option<WinId> {
        let id = key_of(h, n);
        wm.win_get_window_from_id(parent, id)
    };
    let server_name = under_parent(h, wm, "StaticTextServerName");
    let last_updated = under_parent(h, wm, "StaticTextLastUpdated");
    let ladder_wins = under_parent(h, wm, "StaticTextLadderWins");
    let ladder_losses = under_parent(h, wm, "StaticTextLadderLosses");
    let ladder_points = under_parent(h, wm, "StaticTextLadderPoints");
    let ladder_rank = under_parent(h, wm, "StaticTextLadderRank");
    let ladder_disconnects = under_parent(h, wm, "StaticTextDisconnects");
    let highscore_wins = under_parent(h, wm, "StaticTextHighscoreWins");
    let highscore_losses = under_parent(h, wm, "StaticTextHighscoreLosses");
    let highscore_points = under_parent(h, wm, "StaticTextHighscorePoints");
    let highscore_rank = under_parent(h, wm, "StaticTextHighscoreRank");

    let quick_id = key_of(h, "ButtonQuickMatch");
    let quick = wm.win_get_window_from_id(parent, quick_id);
    let lobby_id = key_of(h, "ButtonCustomMatch");
    let lobby = wm.win_get_window_from_id(parent, lobby_id);
    let buddies_id = key_of(h, "ButtonBuddies");
    let buddies = wm.win_get_window_from_id(parent, buddies_id);
    let myinfo_id = key_of(h, "ButtonMyInfo");
    let myinfo = wm.win_get_window_from_id(parent, myinfo_id);
    let ladder_id = key_of(h, "ButtonLadder");
    let ladder = wm.win_get_window_from_id(parent, ladder_id);

    {
        let m = h.wol_welcome_menu();
        m.parent_wol_welcome_id = parent_id;
        m.button_back_id = back_id;
        m.parent_wol_welcome = parent;
        m.button_back = back;
        m.button_options_id = options_id;
        m.button_button_options = options;
        m.listbox_info_id = info_id;
        m.listbox_info = info;
        m.static_text_server_name = server_name;
        m.static_text_last_updated = last_updated;
        m.static_text_ladder_wins = ladder_wins;
        m.static_text_ladder_losses = ladder_losses;
        m.static_text_ladder_points = ladder_points;
        m.static_text_ladder_rank = ladder_rank;
        m.static_text_ladder_disconnects = ladder_disconnects;
        m.static_text_highscore_wins = highscore_wins;
        m.static_text_highscore_losses = highscore_losses;
        m.static_text_highscore_points = highscore_points;
        m.static_text_highscore_rank = highscore_rank;
        m.button_quick_match_id = quick_id;
        m.button_quick_match = quick;
        m.button_lobby_id = lobby_id;
        m.button_lobby = lobby;
        m.button_buddies_id = buddies_id;
        m.button_buddies = buddies;
        m.button_my_info_id = myinfo_id;
        m.button_my_info = myinfo;
        m.button_ladder_id = ladder_id;
        m.button_ladder = ladder;
    }

    if server_name.is_some() {
        let name = h.wol_welcome_menu().g_server_name.clone();
        gadget::static_text_set_text(wm, h, server_name, &name);
    }

    let title_id = key_of(h, "StaticTextTitle");
    let static_text_title = wm.win_get_window_from_id(parent, title_id);
    if static_text_title.is_some() && h.has_gamespy_info() {
        let fmt = h.fetch_text(b"GUI:WOLWelcome");
        let base = h.gamespy_local_base_name();
        let title = h.format_text_arg(&fmt, &base);
        gadget::static_text_set_text(wm, h, static_text_title, &title);
    }
    // (the "?" placeholders for the score windows are commented out upstream)

    // Check for firewall behaviour
    if !h.firewall_helper_exists() {
        h.firewall_helper_create();
    }
    if h.firewall_helper_detect_firewall() {
        h.firewall_helper_delete();
    }

    // Show Menu
    wm.layout_hide(layout, false);

    // Set Keyboard to Main Parent
    wm.win_set_focus(h, parent);
    let got = h.gamespy_got_group_room_list();
    enable_controls(wm, h, got);
    shell::show_shell_map(wm, h, true);

    update_num_players_online(wm, h);
    update_overall_stats(wm, h);
    update_local_player_stats(wm, h);

    let locale = h.misc_prefs_locale();
    if locale < LOC_MIN || locale > LOC_MAX {
        h.gamespy_open_overlay_locale_select(wm);
    }

    h.wol_welcome_menu().raise_message_boxes = true;
    wt::set_group(wm, h, b"WOLWelcomeMenuFade", false);
}

/// `WOLWelcomeMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn wol_welcome_menu_shutdown<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    h.wol_welcome_menu().listbox_info = None;

    if h.firewall_helper_exists() {
        h.firewall_helper_delete();
    }

    h.wol_welcome_menu().is_shutting_down = true;

    // if we are doing an immediate pop, just do it
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"WOLWelcomeMenuFade");
    h.raise_gs_message_box();
}

/// `WOLWelcomeMenuUpdate(layout, userData)` (WOLWelcomeMenu.cpp:606).
pub fn wol_welcome_menu_update<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.wol_welcome_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }

    if h.wol_welcome_menu().raise_message_boxes {
        h.raise_gs_message_box();
        h.wol_welcome_menu().raise_message_boxes = false;
    }

    if h.firewall_helper_exists() && h.firewall_helper_behavior_detection_update_and_store() {
        h.firewall_helper_delete();
    }

    if shell::is_anim_finished(wm, h) && !h.wol_welcome_menu().button_pushed && h.has_peer_message_queue() {
        h.handle_buddy_responses();
        h.handle_persistent_storage_responses();

        let mut allowed_messages = h.gamespy_max_messages_per_update();
        let mut saw_important_message = false;
        loop {
            // while (allowedMessages-- && !sawImportantMessage && queue->getResponse(resp))
            let go = allowed_messages != 0;
            allowed_messages -= 1;
            if !go || saw_important_message {
                break;
            }
            let Some(resp) = h.peer_next_response() else { break };
            match resp {
                WelcomePeerResponse::GroupRoom { group_id, max_waiting, name, num_games, num_playing, num_waiting } => {
                    // (room.m_translatedName = L"TEST" upstream)
                    h.gamespy_add_group_room(group_id, max_waiting, &name, num_games, num_playing, num_waiting);
                    if group_id == 0 {
                        enable_controls(wm, h, true);
                    }
                }
                WelcomePeerResponse::JoinGroupRoom { ok, id } => {
                    saw_important_message = true;
                    enable_controls(wm, h, true);
                    if ok {
                        h.gamespy_set_current_group_room(id);
                        h.wol_welcome_menu().button_pushed = true;
                        h.wol_welcome_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
                        shell::pop(wm, h);
                    } else {
                        let title = h.fetch_text(b"GUI:GSErrorTitle");
                        let body = h.fetch_text(b"GUI:GSGroupRoomJoinFail");
                        h.gs_message_box_ok(wm, &title, &body);
                    }
                }
                WelcomePeerResponse::Disconnect { reason } => {
                    saw_important_message = true;
                    let discon_munkee = format!("GUI:GSDisconReason{reason}");
                    let title = h.fetch_text(b"GUI:GSErrorTitle");
                    let body = h.fetch_text(discon_munkee.as_bytes());
                    h.gamespy_close_all_overlays(wm);
                    h.gs_message_box_ok(wm, &title, &body);
                    shell::pop(wm, h);
                }
                WelcomePeerResponse::Other => {}
            }
        }
    }
}

/// `WOLWelcomeMenuInput(window, msg, mData1, mData2)`.
pub fn wol_welcome_menu_input<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        // if we're showing the menu, ignore input
        if h.wol_welcome_menu().button_pushed {
            return MsgHandled::Ignored;
        }
        if (d1 & 0xFF) as u8 == key::ESC {
            if (d2 & 0xFF) & key::STATE_UP != 0 {
                let m = h.wol_welcome_menu().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(m.button_back), m.button_back_id as u32);
            }
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `WOLWelcomeMenuSystem(window, msg, mData1, mData2)`.
pub fn wol_welcome_menu_system<H: WolWelcomeMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED => {
            if h.wol_welcome_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let id = wm.tree.windows[control].id;
            let m = h.wol_welcome_menu().clone();
            if id == m.button_back_id {
                h.peer_buddy_logout_and_tear_down();

                h.wol_welcome_menu().button_pushed = true;
                shell::pop(wm, h);
            } else if id == m.button_options_id {
                h.gamespy_open_overlay_options(wm);
            } else if id == m.button_quick_match_id {
                let (w, hgt) = h.display_size();
                if (w != 800 || hgt != 600) && h.misc_prefs_quick_match_res_locked() {
                    let title = h.fetch_text(b"GUI:GSErrorTitle");
                    let body = h.fetch_text(b"GUI:QuickMatch800x600");
                    h.gs_message_box_ok(wm, &title, &body);
                } else {
                    h.wol_welcome_menu().button_pushed = true;
                    h.wol_welcome_menu().next_screen = Some(b"Menus/WOLQuickMatchMenu.wnd".to_vec());
                    shell::pop(wm, h);
                }
            } else if id == m.button_my_info_id {
                let pid = h.gamespy_local_profile_id();
                let name = h.gamespy_local_name();
                h.set_look_at_player(pid, &name);
                h.gamespy_toggle_overlay_player_info(wm);
            } else if id == m.button_lobby_id {
                h.gamespy_join_best_group_room();
                enable_controls(wm, h, false);
            } else if id == m.button_buddies_id {
                h.gamespy_toggle_overlay_buddy(wm);
            } else if id == m.button_ladder_id {
                shell::push(wm, h, b"Menus/WOLLadderScreen.wnd", false);
            }
        }
        ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
