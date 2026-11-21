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
}

impl LobbyHost {
    pub fn new() -> Self {
        LobbyHost { shell: Shell::new(), menu: WolLobbyMenu::new(), log: RefCell::new(String::new()), color_default: 0, qr2: 0, thread: 0, changed: false, now: 0 }
    }
    pub fn line(&self, s: String) {
        let mut l = self.log.borrow_mut();
        l.push_str(&s);
        l.push('\n');
    }
}

impl GuiHost for LobbyHost {}

impl ShellHost for LobbyHost {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<usize> {
        None
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
    fn qr2_hosting_status(&self) -> i32 {
        self.qr2
    }
    fn is_thread_hosting(&self) -> i32 {
        self.thread
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        unreachable!("fetch_text")
    }
    fn format_text_arg(&self, fmt: &[u16], text: &[u16]) -> Vec<u16> {
        unreachable!("format_text_arg")
    }
    fn format_text_int_int(&self, fmt: &[u16], text: &[u16], a: i32, b: i32) -> Vec<u16> {
        unreachable!("format_text_int_int")
    }
    fn gamespy_color_current_room(&self) -> i32 {
        unreachable!("gamespy_color_current_room")
    }
    fn gamespy_color_room(&self) -> i32 {
        unreachable!("gamespy_color_room")
    }
    fn gamespy_color_player_ignored(&self) -> i32 {
        unreachable!("gamespy_color_player_ignored")
    }
    fn gamespy_color_player_owner(&self) -> i32 {
        unreachable!("gamespy_color_player_owner")
    }
    fn gamespy_color_player_buddy(&self) -> i32 {
        unreachable!("gamespy_color_player_buddy")
    }
    fn gamespy_color_player_normal(&self) -> i32 {
        unreachable!("gamespy_color_player_normal")
    }
    fn image_width(&self, image: &[u8]) -> i32 {
        unreachable!("image_width")
    }
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>> {
        unreachable!("find_image_by_name")
    }
    fn rank_point_value(&self, i: usize) -> i32 {
        unreachable!("rank_point_value")
    }
    fn player_template_side(&self, n: i32) -> Option<Vec<u8>> {
        unreachable!("player_template_side")
    }
    fn gamespy_local_name(&self) -> Vec<u16> {
        unreachable!("gamespy_local_name")
    }
    fn gamespy_local_name_ascii(&self) -> Vec<u8> {
        unreachable!("gamespy_local_name_ascii")
    }
    fn gamespy_local_profile_id(&self) -> i32 {
        unreachable!("gamespy_local_profile_id")
    }
    fn gamespy_max_messages_per_update(&self) -> i32 {
        unreachable!("gamespy_max_messages_per_update")
    }
    fn gamespy_current_group_room(&self) -> i32 {
        unreachable!("gamespy_current_group_room")
    }
    fn gamespy_set_current_group_room(&mut self, id: i32) {
        unreachable!("gamespy_set_current_group_room")
    }
    fn gamespy_group_room_list(&self) -> Vec<LobbyGroupRoom> {
        unreachable!("gamespy_group_room_list")
    }
    fn gamespy_clear_player_info_map(&mut self) {
        unreachable!("gamespy_clear_player_info_map")
    }
    fn gamespy_player_info_map(&self) -> Vec<LobbyPlayerInfo> {
        unreachable!("gamespy_player_info_map")
    }
    fn gamespy_player_info_by_name(&self, name: &[u8]) -> Option<LobbyPlayerInfo> {
        unreachable!("gamespy_player_info_by_name")
    }
    fn gamespy_update_player_info(&mut self, p: &LobbyPlayerInfo) {
        unreachable!("gamespy_update_player_info")
    }
    fn gamespy_player_left_group_room(&mut self, nick: &[u8]) {
        unreachable!("gamespy_player_left_group_room")
    }
    fn gamespy_is_buddy(&self, profile_id: i32) -> bool {
        unreachable!("gamespy_is_buddy")
    }
    fn gamespy_did_player_preorder(&self, profile_id: i32) -> bool {
        unreachable!("gamespy_did_player_preorder")
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
        unreachable!("gamespy_staging_room_to_join")
    }
    fn ladder_exists(&self, ip: &[u8], port: u16) -> bool {
        unreachable!("ladder_exists")
    }
    fn gamespy_config_qm_channel(&self) -> i32 {
        unreachable!("gamespy_config_qm_channel")
    }
    fn gamespy_config_restrict_games_to_lobby(&self) -> bool {
        unreachable!("gamespy_config_restrict_games_to_lobby")
    }
    fn gamespy_config_is_player_vip(&self, profile_id: i32) -> bool {
        unreachable!("gamespy_config_is_player_vip")
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
        unreachable!("get_game_list_box_id")
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
        unreachable!("mouse_set_cursor_tooltip")
    }
    fn layout_first_window(&mut self, wm: &GameWindowManager, layout: usize) -> Option<WinId> {
        unreachable!("layout_first_window")
    }
    fn layout_run_init(&mut self, wm: &mut GameWindowManager, layout: usize) {
        unreachable!("layout_run_init")
    }
    fn set_unignore_text(&mut self, wm: &mut GameWindowManager, layout: usize, nick: &[u8], id: i32) {
        unreachable!("set_unignore_text")
    }
    fn win_set_user_data_rc_menu(&mut self, wm: &mut GameWindowManager, w: WinId, id: i32, nick: &[u8], is_buddy: bool) {
        unreachable!("win_set_user_data_rc_menu")
    }
    fn win_set_user_data_int(&mut self, wm: &mut GameWindowManager, w: WinId, v: i32) {
        unreachable!("win_set_user_data_int")
    }
    fn display_size(&self) -> (i32, i32) {
        unreachable!("display_size")
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
