// SPDX-License-Identifier: GPL-3.0-only
//! The stand-in host of the quick match draft (`src/drafts/ui/wol_quick_match_menu.rs`) for the group wol7 differential tests: the methods the tests reach are written out, every
//! other method of `WolQuickMatchMenuHost` is `unreachable!` (generated from the trait, 2026-11-22).
#![allow(unused_variables, clippy::all, dead_code)]

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{GameFont, WinId};
use rts_lockstep_engine::drafts::ui::wol_quick_match_menu::*;

#[derive(Default)]
pub struct QmHost {
    pub shell: Shell,
    pub menu: WolQuickMatchMenu,
    pub random_color: i32,
    /// `TheMultiplayerSettings->getColor(c)`: `None` is a NULL definition (color, tooltip name)
    pub colors: Vec<Option<(i32, Vec<u8>)>>,
    pub pref_color: i32,
    pub templates: Vec<QmPlayerTemplate>,
    pub scene_log: String,
}

impl GuiHost for QmHost {
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
}

impl ShellHost for QmHost {
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
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        0
    }
}

impl WolQuickMatchMenuHost for QmHost {
    fn wol_quick_match_menu(&mut self) -> &mut WolQuickMatchMenu {
        &mut self.menu
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut t = b"T:".to_vec();
        t.extend_from_slice(label);
        t.iter().map(|&b| u16::from(b)).collect()
    }
    fn multiplayer_num_colors(&self) -> i32 {
        self.colors.len() as i32
    }
    fn multiplayer_color(&self, c: i32) -> Option<(i32, Vec<u8>)> {
        if c == -1 {
            return Some((self.random_color, Vec::new()));
        }
        usize::try_from(c).ok().and_then(|i| self.colors.get(i).cloned().flatten())
    }
    fn qm_pref_color(&mut self) -> i32 {
        self.pref_color
    }
    fn player_templates(&self) -> Vec<QmPlayerTemplate> {
        self.templates.clone()
    }
    fn format_text_arg(&self, fmt: &[u16], text: &[u16]) -> Vec<u16> {
        unreachable!("format_text_arg")
    }
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16> {
        unreachable!("format_int")
    }
    fn format_int_int(&self, fmt: &[u16], a: i32, b: i32) -> Vec<u16> {
        unreachable!("format_int_int")
    }
    fn gamespy_color_map_selected(&self) -> i32 {
        unreachable!("gamespy_color_map_selected")
    }
    fn gamespy_color_map_unselected(&self) -> i32 {
        unreachable!("gamespy_color_map_unselected")
    }
    fn gamespy_color_default(&self) -> i32 {
        unreachable!("gamespy_color_default")
    }
    fn ladder_find_by_index(&self, index: i32) -> Option<QmLadderInfo> {
        unreachable!("ladder_find_by_index")
    }
    fn ladder_find(&self, addr: &[u8], port: u16) -> Option<QmLadderInfo> {
        unreachable!("ladder_find")
    }
    fn ladder_special(&self) -> Vec<QmLadderInfo> {
        unreachable!("ladder_special")
    }
    fn ladder_standard(&self) -> Vec<QmLadderInfo> {
        unreachable!("ladder_standard")
    }
    fn ladder_local_count(&self) -> usize {
        unreachable!("ladder_local_count")
    }
    fn ladder_pref_recent_ladders(&mut self, profile: i32) -> Vec<(Vec<u8>, u16)> {
        unreachable!("ladder_pref_recent_ladders")
    }
    fn ladder_pref_add_recent_and_write(&mut self, profile: i32, p: &QmLadderPref) {
        unreachable!("ladder_pref_add_recent_and_write")
    }
    fn qm_pref_load(&mut self) {
        unreachable!("qm_pref_load")
    }
    fn qm_pref_write(&mut self) {
        unreachable!("qm_pref_write")
    }
    fn qm_pref_side(&mut self) -> i32 {
        unreachable!("qm_pref_side")
    }
    fn qm_pref_num_players(&mut self) -> i32 {
        unreachable!("qm_pref_num_players")
    }
    fn qm_pref_max_ping(&mut self) -> i32 {
        unreachable!("qm_pref_max_ping")
    }
    fn qm_pref_max_disconnects(&mut self) -> i32 {
        unreachable!("qm_pref_max_disconnects")
    }
    fn qm_pref_max_points(&mut self) -> i32 {
        unreachable!("qm_pref_max_points")
    }
    fn qm_pref_min_points(&mut self) -> i32 {
        unreachable!("qm_pref_min_points")
    }
    fn qm_pref_last_ladder_addr(&mut self) -> Vec<u8> {
        unreachable!("qm_pref_last_ladder_addr")
    }
    fn qm_pref_last_ladder_port(&mut self) -> u16 {
        unreachable!("qm_pref_last_ladder_port")
    }
    fn qm_pref_is_map_selected(&mut self, map: &[u8]) -> bool {
        unreachable!("qm_pref_is_map_selected")
    }
    fn qm_pref_set_last_ladder(&mut self, addr: &[u8], port: u16) {
        unreachable!("qm_pref_set_last_ladder")
    }
    fn qm_pref_set_map_selected(&mut self, map: &[u8], selected: bool) {
        unreachable!("qm_pref_set_map_selected")
    }
    fn qm_pref_set_num_players(&mut self, v: i32) {
        unreachable!("qm_pref_set_num_players")
    }
    fn qm_pref_set_max_ping(&mut self, v: i32) {
        unreachable!("qm_pref_set_max_ping")
    }
    fn qm_pref_set_side(&mut self, v: i32) {
        unreachable!("qm_pref_set_side")
    }
    fn qm_pref_set_color(&mut self, v: i32) {
        unreachable!("qm_pref_set_color")
    }
    fn qm_pref_set_max_disconnects(&mut self, v: i32) {
        unreachable!("qm_pref_set_max_disconnects")
    }
    fn option_prefs_firewall_behavior(&mut self) -> i32 {
        unreachable!("option_prefs_firewall_behavior")
    }
    fn gamespy_config_qm_maps(&self) -> Vec<Vec<u8>> {
        unreachable!("gamespy_config_qm_maps")
    }
    fn gamespy_config_ping_timeout_ms(&self) -> i32 {
        unreachable!("gamespy_config_ping_timeout_ms")
    }
    fn gamespy_config_qm_bot_id(&self) -> i32 {
        unreachable!("gamespy_config_qm_bot_id")
    }
    fn gamespy_config_qm_channel(&self) -> i32 {
        unreachable!("gamespy_config_qm_channel")
    }
    fn map_cache_find(&self, map: &[u8]) -> Option<QmMapMeta> {
        unreachable!("map_cache_find")
    }
    fn image_width(&self, image: &[u8]) -> i32 {
        unreachable!("image_width")
    }
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>> {
        unreachable!("find_image_by_name")
    }
    fn gamespy_local_profile_id(&self) -> i32 {
        unreachable!("gamespy_local_profile_id")
    }
    fn gamespy_local_name(&self) -> Vec<u16> {
        unreachable!("gamespy_local_name")
    }
    fn gamespy_ping_string(&self) -> Vec<u8> {
        unreachable!("gamespy_ping_string")
    }
    fn gamespy_max_messages_per_update(&self) -> i32 {
        unreachable!("gamespy_max_messages_per_update")
    }
    fn gamespy_is_disconnected_after_game_start(&self) -> Option<i32> {
        unreachable!("gamespy_is_disconnected_after_game_start")
    }
    fn gamespy_mark_disconnected_after_game_start(&mut self, reason: i32) {
        unreachable!("gamespy_mark_disconnected_after_game_start")
    }
    fn gamespy_reset(&mut self) {
        unreachable!("gamespy_reset")
    }
    fn gamespy_leave_group_room(&mut self) {
        unreachable!("gamespy_leave_group_room")
    }
    fn gamespy_leave_staging_room(&mut self) {
        unreachable!("gamespy_leave_staging_room")
    }
    fn gamespy_has_info(&self) -> bool {
        unreachable!("gamespy_has_info")
    }
    fn gamespy_register_text_window(&mut self, w: Option<WinId>) {
        unreachable!("gamespy_register_text_window")
    }
    fn gamespy_unregister_text_window(&mut self, w: Option<WinId>) {
        unreachable!("gamespy_unregister_text_window")
    }
    fn gamespy_add_text(&mut self, wm: &mut GameWindowManager, text: &[u16], color: i32, win: Option<WinId>) {
        unreachable!("gamespy_add_text")
    }
    fn ps_find_player_stats_by_id(&mut self, id: i32) -> QmPlayerStats {
        unreachable!("ps_find_player_stats_by_id")
    }
    fn ps_parse_and_track_player_stats(&mut self, data: &[u8], id: i32) {
        unreachable!("ps_parse_and_track_player_stats")
    }
    fn calculate_rank(&mut self, stats: &QmPlayerStats) -> i32 {
        unreachable!("calculate_rank")
    }
    fn game_spy_game_exists(&self) -> bool {
        unreachable!("game_spy_game_exists")
    }
    fn game_spy_game_is_game_in_progress(&self) -> bool {
        unreachable!("game_spy_game_is_game_in_progress")
    }
    fn game_spy_game_set_game_in_progress(&mut self, v: bool) {
        unreachable!("game_spy_game_set_game_in_progress")
    }
    fn game_spy_game_reset(&mut self) {
        unreachable!("game_spy_game_reset")
    }
    fn game_spy_game_launch_game(&mut self) {
        unreachable!("game_spy_game_launch_game")
    }
    fn game_spy_game_enter_game(&mut self) {
        unreachable!("game_spy_game_enter_game")
    }
    fn game_spy_game_set_seed(&mut self, seed: i32) {
        unreachable!("game_spy_game_set_seed")
    }
    fn game_spy_game_mark_game_as_qm(&mut self) {
        unreachable!("game_spy_game_mark_game_as_qm")
    }
    fn game_spy_game_set_ladder_ip(&mut self, ip: &[u8]) {
        unreachable!("game_spy_game_set_ladder_ip")
    }
    fn game_spy_game_set_ladder_port(&mut self, port: u16) {
        unreachable!("game_spy_game_set_ladder_port")
    }
    fn game_spy_game_set_map(&mut self, map: &[u8]) {
        unreachable!("game_spy_game_set_map")
    }
    fn game_spy_game_set_game_name(&mut self, name: &[u16]) {
        unreachable!("game_spy_game_set_game_name")
    }
    fn game_spy_game_slot_set_closed(&mut self, i: usize) {
        unreachable!("game_spy_game_slot_set_closed")
    }
    fn game_spy_game_slot_set_player(&mut self, i: usize, name: &[u16], ip: u32, color: i32, player_template: i32, nat: i32, team_number: i32) {
        unreachable!("game_spy_game_slot_set_player")
    }
    fn game_spy_game_slot_num(&self, nick: &[u8]) -> i32 {
        unreachable!("game_spy_game_slot_num")
    }
    fn game_spy_game_set_profile_id_by_login_name(&mut self, nick: &[u8], id: i32) {
        unreachable!("game_spy_game_set_profile_id_by_login_name")
    }
    fn send_stats_to_other_players(&mut self) {
        unreachable!("send_stats_to_other_players")
    }
    fn game_spy_game_start_game(&mut self) {
        unreachable!("game_spy_game_start_game")
    }
    fn nat_exists(&self) -> bool {
        unreachable!("nat_exists")
    }
    fn nat_delete(&mut self) {
        unreachable!("nat_delete")
    }
    fn nat_update(&mut self) -> QmNatState {
        unreachable!("nat_update")
    }
    fn nat_process_global_message(&mut self, slot: i32, options: &[u8]) {
        unreachable!("nat_process_global_message")
    }
    fn has_peer_message_queue(&self) -> bool {
        unreachable!("has_peer_message_queue")
    }
    fn peer_next_response(&mut self) -> Option<QmPeerResponse> {
        unreachable!("peer_next_response")
    }
    fn peer_request_stop_quick_match(&mut self) {
        unreachable!("peer_request_stop_quick_match")
    }
    fn peer_request_widen_quick_match_search(&mut self) {
        unreachable!("peer_request_widen_quick_match_search")
    }
    fn peer_request_start_quick_match(&mut self, req: &StartQuickMatchRequest) {
        unreachable!("peer_request_start_quick_match")
    }
    fn handle_buddy_responses(&mut self) {
        unreachable!("handle_buddy_responses")
    }
    fn handle_persistent_storage_responses(&mut self) {
        unreachable!("handle_persistent_storage_responses")
    }
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_close_all_overlays")
    }
    fn gamespy_close_overlay_buddy(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_close_overlay_buddy")
    }
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_toggle_overlay_buddy")
    }
    fn gamespy_open_overlay_ladder_select(&mut self, wm: &mut GameWindowManager) {
        unreachable!("gamespy_open_overlay_ladder_select")
    }
    fn raise_gs_message_box(&mut self) {
        unreachable!("raise_gs_message_box")
    }
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]) {
        unreachable!("gs_message_box_ok")
    }
    fn update_local_player_stats(&mut self, wm: &mut GameWindowManager) {
        unreachable!("update_local_player_stats")
    }
    fn game_engine_quitting(&self) -> bool {
        unreachable!("game_engine_quitting")
    }
    fn game_logic_in_shell_game_first_frame(&self) -> bool {
        unreachable!("game_logic_in_shell_game_first_frame")
    }
    fn signal_ui_interaction_entered_from_game(&mut self) {
        unreachable!("signal_ui_interaction_entered_from_game")
    }
    fn in_game_ui_message(&mut self, label: &[u8]) {
        unreachable!("in_game_ui_message")
    }
    fn game_client_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        unreachable!("game_client_random_value")
    }
    fn time_now(&self) -> i64 {
        unreachable!("time_now")
    }
    fn global_exe_crc(&self) -> u32 {
        unreachable!("global_exe_crc")
    }
    fn global_ini_crc(&self) -> u32 {
        unreachable!("global_ini_crc")
    }
}
