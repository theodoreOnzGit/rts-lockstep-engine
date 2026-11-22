// SPDX-License-Identifier: GPL-3.0-only
//! The stand-in host of the lobby draft (`src/drafts/ui/wol_lobby_menu.rs`) for the group wol6 differential tests. The methods the tests reach log
//! their calls; every other method of `WolLobbyMenuHost` is `unreachable!` (generated from the trait, 2026-11-21).
#![allow(unused_variables, clippy::all)]

use super::hex;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::wol_lobby_menu::*;
use std::cell::RefCell;

pub struct LobbyHost {
    pub shell: Shell,
    pub menu: WolLobbyMenu,
    pub log: RefCell<String>,
    pub color_default: i32,
    pub qr2: i32,
    pub thread: i32,
    pub changed: bool,
    pub now: u32,
    pub rank_values: [i32; 14],
    pub images: Vec<Vec<u8>>,
    /// the GameSpy state the list functions read (group wol7)
    pub gs: Gs,
    /// the scene operations' log (ui4_common::HasLog)
    pub scene_log: String,
}

/// The case's GameSpy state of the lobby's list functions (group wol7): what the C++ wrapper's `@ w7` operations set.
#[derive(Default)]
pub struct Gs {
    pub colors: [i32; 32],
    pub image_widths: std::collections::BTreeMap<Vec<u8>, i32>,
    pub templates: Vec<Vec<u8>>,
    pub local_name: Vec<u8>,
    pub current_room: i32,
    pub qm: i32,
    pub rooms: std::collections::BTreeMap<i32, LobbyGroupRoom>,
    /// keyed by the ASCII-lower-cased name (`AsciiComparator`: stricmp)
    pub players: std::collections::BTreeMap<Vec<u8>, LobbyPlayerInfo>,
    pub buddies: std::collections::BTreeSet<i32>,
    pub preorder: std::collections::BTreeSet<i32>,
    pub vip: std::collections::BTreeSet<i32>,
    pub saved_ignored: std::collections::BTreeSet<i32>,
    pub ignored_names: std::collections::BTreeSet<Vec<u8>>,
    /// group wol8, the right-click menu: the windows the next `create_layout` holds (empty: none made, as before), the local profile, the game list's id,
    /// the staging rooms (None: a NULL pointer in the map) and the ladders (ip, port)
    pub lay_members: Vec<usize>,
    pub local_profile: i32,
    pub game_list_id: i32,
    pub rooms_to_join: std::collections::BTreeMap<i32, Option<LobbyRoomToJoin>>,
    pub ladders: std::collections::BTreeSet<(Vec<u8>, u16)>,
}

impl Gs {
    /// `PlayerInfo::isIgnored()` (PeerDefs.cpp:827).
    pub fn with_ignored(&self, p: &LobbyPlayerInfo) -> LobbyPlayerInfo {
        let mut q = p.clone();
        q.is_ignored = if p.profile_id != 0 { self.saved_ignored.contains(&p.profile_id) } else { self.ignored_names.contains(&p.name) };
        q
    }
}

impl LobbyHost {
    pub fn new() -> Self {
        LobbyHost { shell: Shell::new(), menu: WolLobbyMenu::new(), log: RefCell::new(String::new()), color_default: 0, qr2: 0, thread: 0, changed: false, now: 0, rank_values: [0; 14], images: Vec::new(), gs: Gs::default(), scene_log: String::new() }
    }
    pub fn line(&self, s: String) {
        let mut l = self.log.borrow_mut();
        l.push_str(&s);
        l.push('\n');
    }
}

impl GuiHost for LobbyHost {
    fn font_height(&self, font: Option<&rts_lockstep_engine::client::window_script::GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
}

impl ShellHost for LobbyHost {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        if self.gs.lay_members.is_empty() {
            return None;
        }
        // winCreateLayout of differential/ui4_host.h: a layout of the case's windows, head first
        self.line(format!("layout {}", String::from_utf8_lossy(filename)));
        let l = wm.layouts.len();
        wm.layouts.push(rts_lockstep_engine::client::window_manager::WindowLayout::default());
        for &m in self.gs.lay_members.clone().iter().rev() {
            wm.layout_add_window(l, m);
        }
        Some(l)
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _layout: usize, _user: Option<bool>) {}
    fn time_ms(&self) -> u32 {
        self.now
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        0
    }
}

fn win_id(w: Option<WinId>) -> i64 {
    w.map_or(-1, |w| w as i64)
}

impl WolLobbyMenuHost for LobbyHost {
    fn wol_lobby_menu(&mut self) -> &mut WolLobbyMenu {
        &mut self.menu
    }
    fn gamespy_color_default(&self) -> i32 {
        self.color_default
    }
    fn gamespy_add_text(&mut self, wm: &mut GameWindowManager, text: &[u16], color: i32, win: Option<WinId>) {
        self.line(format!("addText {} color {} win {}", hex(text), color, win_id(win)));
    }
    fn gamespy_send_chat(&mut self, wm: &mut GameWindowManager, text: &[u16], is_emote: bool, win: Option<WinId>) {
        self.line(format!("sendChat {} emote {} win {}", hex(text), i32::from(is_emote), win_id(win)));
    }
    fn gamespy_has_staging_room_list_changed(&self) -> bool {
        self.line(format!("hasStagingRoomListChanged -> {}", i32::from(self.changed)));
        self.changed
    }
    fn refresh_game_list_boxes(&mut self, wm: &mut GameWindowManager) {
        self.line("RefreshGameListBoxes".to_string());
    }
    fn rank_point_value(&self, i: usize) -> i32 {
        self.rank_values[i]
    }
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.line(format!("find {}", String::from_utf8_lossy(name)));
        self.images.iter().find(|i| i.as_slice() == name).cloned()
    }
    fn qr2_hosting_status(&self) -> i32 {
        self.qr2
    }
    fn is_thread_hosting(&self) -> i32 {
        self.thread
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut t = b"T:".to_vec();
        t.extend_from_slice(label);
        t.iter().map(|&b| u16::from(b)).collect()
    }
    fn format_text_arg(&self, fmt: &[u16], text: &[u16]) -> Vec<u16> {
        let mut v = fmt.to_vec();
        v.push(u16::from(b'|'));
        v.extend_from_slice(text);
        v
    }
    fn format_text_int_int(&self, fmt: &[u16], text: &[u16], a: i32, b: i32) -> Vec<u16> {
        let mut v = fmt.to_vec();
        v.push(u16::from(b'|'));
        v.extend_from_slice(text);
        v.extend(format!("|{a}|{b}").encode_utf16());
        v
    }
    fn gamespy_color_current_room(&self) -> i32 {
        self.gs.colors[1]
    }
    fn gamespy_color_room(&self) -> i32 {
        self.gs.colors[2]
    }
    fn gamespy_color_player_ignored(&self) -> i32 {
        self.gs.colors[10]
    }
    fn gamespy_color_player_owner(&self) -> i32 {
        self.gs.colors[7]
    }
    fn gamespy_color_player_buddy(&self) -> i32 {
        self.gs.colors[8]
    }
    fn gamespy_color_player_normal(&self) -> i32 {
        self.gs.colors[6]
    }
    fn image_width(&self, image: &[u8]) -> i32 {
        self.gs.image_widths.get(image).copied().unwrap_or(0)
    }
    fn player_template_side(&self, n: i32) -> Option<Vec<u8>> {
        usize::try_from(n).ok().and_then(|i| self.gs.templates.get(i).cloned())
    }
    fn gamespy_local_name(&self) -> Vec<u16> {
        unreachable!("gamespy_local_name")
    }
    fn gamespy_local_name_ascii(&self) -> Vec<u8> {
        self.gs.local_name.clone()
    }
    fn gamespy_local_profile_id(&self) -> i32 {
        self.gs.local_profile
    }
    fn gamespy_max_messages_per_update(&self) -> i32 {
        unreachable!("gamespy_max_messages_per_update")
    }
    fn gamespy_current_group_room(&self) -> i32 {
        self.gs.current_room
    }
    fn gamespy_set_current_group_room(&mut self, id: i32) {
        unreachable!("gamespy_set_current_group_room")
    }
    fn gamespy_group_room_list(&self) -> Vec<LobbyGroupRoom> {
        self.gs.rooms.values().cloned().collect()
    }
    fn gamespy_clear_player_info_map(&mut self) {
        unreachable!("gamespy_clear_player_info_map")
    }
    fn gamespy_player_info_map(&self) -> Vec<LobbyPlayerInfo> {
        self.gs.players.values().map(|p| self.gs.with_ignored(p)).collect()
    }
    fn gamespy_player_info_by_name(&self, name: &[u8]) -> Option<LobbyPlayerInfo> {
        self.gs.players.get(&name.to_ascii_lowercase()).map(|p| self.gs.with_ignored(p))
    }
    fn gamespy_update_player_info(&mut self, p: &LobbyPlayerInfo) {
        unreachable!("gamespy_update_player_info")
    }
    fn gamespy_player_left_group_room(&mut self, nick: &[u8]) {
        unreachable!("gamespy_player_left_group_room")
    }
    fn gamespy_is_buddy(&self, profile_id: i32) -> bool {
        self.gs.buddies.contains(&profile_id)
    }
    fn gamespy_did_player_preorder(&self, profile_id: i32) -> bool {
        self.gs.preorder.contains(&profile_id)
    }
    fn gamespy_join_group_room(&mut self, id: i32) {
        unreachable!("gamespy_join_group_room")
    }
    fn gamespy_join_best_group_room(&mut self) {
        unreachable!("gamespy_join_best_group_room")
    }
    fn gamespy_leave_group_room(&mut self) {
        unreachable!("gamespy_leave_group_room")
    }
    fn gamespy_reset(&mut self) {
        unreachable!("gamespy_reset")
    }
    fn gamespy_register_text_window(&mut self, w: Option<WinId>) {
        unreachable!("gamespy_register_text_window")
    }
    fn gamespy_unregister_text_window(&mut self, w: Option<WinId>) {
        unreachable!("gamespy_unregister_text_window")
    }
    fn gamespy_add_chat(&mut self, wm: &mut GameWindowManager, nick: &[u8], profile_id: i32, text: &[u8], is_public: bool, is_action: bool, win: Option<WinId>) {
        unreachable!("gamespy_add_chat")
    }
    fn gamespy_clear_staging_room_list(&mut self) {
        unreachable!("gamespy_clear_staging_room_list")
    }
    fn gamespy_saw_full_game_list(&mut self) {
        unreachable!("gamespy_saw_full_game_list")
    }
    fn gamespy_add_staging_room(&mut self, room: &LobbyStagingRoom) {
        unreachable!("gamespy_add_staging_room")
    }
    fn gamespy_update_staging_room(&mut self, room: &LobbyStagingRoom) {
        unreachable!("gamespy_update_staging_room")
    }
    fn gamespy_remove_staging_room(&mut self, id: i32) {
        unreachable!("gamespy_remove_staging_room")
    }
    fn gamespy_current_staging_room_host_name(&self) -> Option<Vec<u8>> {
        unreachable!("gamespy_current_staging_room_host_name")
    }
    fn gamespy_mark_as_staging_room_host(&mut self) {
        unreachable!("gamespy_mark_as_staging_room_host")
    }
    fn gamespy_set_game_options(&mut self) {
        unreachable!("gamespy_set_game_options")
    }
    fn gamespy_mark_as_staging_room_joiner(&mut self, id: i32) {
        unreachable!("gamespy_mark_as_staging_room_joiner")
    }
    fn gamespy_staging_room_to_join(&self, id: i32) -> Option<LobbyRoomToJoin> {
        self.gs.rooms_to_join.get(&id).cloned().flatten()
    }
    fn ladder_exists(&self, ip: &[u8], port: u16) -> bool {
        self.gs.ladders.contains(&(ip.to_vec(), port))
    }
    fn gamespy_config_qm_channel(&self) -> i32 {
        self.gs.qm
    }
    fn gamespy_config_restrict_games_to_lobby(&self) -> bool {
        unreachable!("gamespy_config_restrict_games_to_lobby")
    }
    fn gamespy_config_is_player_vip(&self, profile_id: i32) -> bool {
        self.gs.vip.contains(&profile_id)
    }
    fn game_spy_game_reset(&mut self) {
        unreachable!("game_spy_game_reset")
    }
    fn game_spy_game_set_game_name(&mut self, name: &[u16]) {
        unreachable!("game_spy_game_set_game_name")
    }
    fn game_spy_game_set_ladder_ip(&mut self, ip: &[u8]) {
        unreachable!("game_spy_game_set_ladder_ip")
    }
    fn game_spy_game_set_ladder_port(&mut self, port: u16) {
        unreachable!("game_spy_game_set_ladder_port")
    }
    fn portable_map_path_to_real_map_path(&self, map_name: &[u8]) -> Vec<u8> {
        unreachable!("portable_map_path_to_real_map_path")
    }
    fn global_exe_crc(&self) -> u32 {
        unreachable!("global_exe_crc")
    }
    fn global_ini_crc(&self) -> u32 {
        unreachable!("global_ini_crc")
    }
    fn custom_pref_uses_long_game_list(&mut self) -> bool {
        unreachable!("custom_pref_uses_long_game_list")
    }
    fn custom_pref_set_uses_long_game_list_and_write(&mut self, v: bool) {
        unreachable!("custom_pref_set_uses_long_game_list_and_write")
    }
    fn has_peer_message_queue(&self) -> bool {
        unreachable!("has_peer_message_queue")
    }
    fn peer_next_response(&mut self) -> Option<LobbyPeerResponse> {
        unreachable!("peer_next_response")
    }
    fn peer_request_start_game_list(&mut self, restrict_game_list: bool) {
        unreachable!("peer_request_start_game_list")
    }
    fn peer_request_stop_game_list(&mut self) {
        unreachable!("peer_request_stop_game_list")
    }
    fn peer_request_get_extended_staging_room_info(&mut self, id: i32) {
        unreachable!("peer_request_get_extended_staging_room_info")
    }
    fn peer_request_join_staging_room(&mut self, game_name: &[u16], id: i32, password: &[u8]) {
        unreachable!("peer_request_join_staging_room")
    }
    fn handle_buddy_responses(&mut self) {
        unreachable!("handle_buddy_responses")
    }
    fn handle_persistent_storage_responses(&mut self) {
        unreachable!("handle_persistent_storage_responses")
    }
    fn gamespy_open_overlay_game_options(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_open_overlay_game_options")
    }
    fn gamespy_open_overlay_game_password(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_open_overlay_game_password")
    }
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_toggle_overlay_buddy")
    }
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_close_all_overlays")
    }
    fn raise_gs_message_box(&mut self) {
        unreachable!("raise_gs_message_box")
    }
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]) {
        unreachable!("gs_message_box_ok")
    }
    fn grab_window_info(&mut self, wm: &mut GameWindowManager) {
        unreachable!("grab_window_info")
    }
    fn release_window_info(&mut self) {
        unreachable!("release_window_info")
    }
    fn get_game_info_list_box(&self) -> Option<WinId> {
        unreachable!("get_game_info_list_box")
    }
    fn get_game_list_box(&self) -> Option<WinId> {
        unreachable!("get_game_list_box")
    }
    fn get_game_list_box_id(&self) -> i32 {
        self.gs.game_list_id
    }
    fn refresh_game_info_list_box(&mut self, wm: &mut GameWindowManager, game_list: Option<WinId>, info_list: Option<WinId>) {
        unreachable!("refresh_game_info_list_box")
    }
    fn toggle_game_list_type(&mut self, wm: &mut GameWindowManager) {
        unreachable!("toggle_game_list_type")
    }
    fn handle_sort_button(&mut self, wm: &mut GameWindowManager, control_id: i32) -> bool {
        unreachable!("handle_sort_button")
    }
    fn set_player_tooltip_func(&mut self, wm: &mut GameWindowManager, w: WinId) {
        unreachable!("set_player_tooltip_func")
    }
    fn mouse_set_cursor_tooltip(&mut self, text: &[u16], width: f32) {
        let t = format!("tip [{}]", text.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","));
        self.line(format!("{t} delay -1 width {}", width.to_bits()));
    }
    fn layout_first_window(&mut self, wm: &GameWindowManager, layout: usize) -> Option<WinId> {
        wm.layouts[layout].windows.first().copied()
    }
    fn layout_run_init(&mut self, wm: &mut GameWindowManager, layout: usize) {
        // the layouts of the C++ program have no init callback (`u4_layout_init` is NULL): `runInit` does nothing
    }
    fn set_unignore_text(&mut self, wm: &mut GameWindowManager, layout: usize, nick: &[u8], id: i32) {
        let n = if nick.is_empty() { "-".to_string() } else { nick.iter().map(|b| format!("{b:02x}")).collect() };
        self.line(format!("unignore {layout} {n} {id}"));
    }
    fn win_set_user_data_rc_menu(&mut self, wm: &mut GameWindowManager, w: WinId, id: i32, nick: &[u8], is_buddy: bool) {
        let n = if nick.is_empty() { "-".to_string() } else { nick.iter().map(|b| format!("{b:02x}")).collect() };
        // RCItemType: ITEM_BUDDY 0, ITEM_REQUEST 1, ITEM_NONBUDDY 2
        self.line(format!("rcdata {w} {id} {n} {}", if is_buddy { 0 } else { 2 }));
    }
    fn win_set_user_data_int(&mut self, wm: &mut GameWindowManager, w: WinId, v: i32) {
        self.line(format!("ud {w} {v}"));
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
    fn game_client_frame(&self) -> u32 {
        unreachable!("game_client_frame")
    }
    fn game_logic_in_shell_game_first_frame(&self) -> bool {
        unreachable!("game_logic_in_shell_game_first_frame")
    }
    fn signal_ui_interaction_entered_from_game(&mut self) {
        unreachable!("signal_ui_interaction_entered_from_game")
    }
}
