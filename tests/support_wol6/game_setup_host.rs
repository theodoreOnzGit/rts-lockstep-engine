// SPDX-License-Identifier: GPL-3.0-only
//! The stand-in host of the game setup draft (`src/drafts/ui/wol_game_setup_menu.rs`) for the group wol6 differential tests. The methods the tests
//! reach log their calls; every other method of `WolGameSetupMenuHost` is `unreachable!` (generated from the trait, 2026-11-21).
#![allow(unused_variables, clippy::all)]

use super::hex;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::wol_game_setup_menu::*;
use rts_lockstep_engine::drafts::ui::wol_lobby_menu::{LobbyPlayerInfo, LobbyUtm};
use std::cell::RefCell;

pub struct GameSetupHost {
    pub shell: Shell,
    pub menu: WolGameSetupMenu,
    pub log: RefCell<String>,
    pub color_default: i32,
    pub qr2: i32,
    pub thread: i32,
    pub now: u32,
    /// the scene operations' log (ui4_common::HasLog) and the declared image names (group wol8)
    pub scene_log: String,
    pub w8_images: std::collections::BTreeSet<Vec<u8>>,
    /// group wol8, the staging room's display functions: the case's room and slots, the SDK's answers and the key table (`keys_on`: a name's key is its first-asked number, as
    /// the C++ `NameKeyGenerator`; off, 0 as before)
    pub w8: W8State,
}

#[derive(Default)]
pub struct W8State {
    pub keys_on: bool,
    pub keys: std::collections::BTreeMap<Vec<u8>, u32>,
    pub room_exists: bool,
    pub am_i_host: bool,
    pub room: StagingRoomInfo,
    pub slots: Vec<StagingSlotInfo>,
    pub ping_string: Vec<u8>,
    pub good: i32,
    pub bad: i32,
    pub maps: std::collections::BTreeMap<Vec<u8>, (i32, Vec<u16>, bool)>,
    pub map_select_layout: bool,
    pub map_names: Vec<Vec<u8>>,
}

impl GameSetupHost {
    pub fn new() -> Self {
        GameSetupHost { shell: Shell::new(), menu: WolGameSetupMenu::default(), log: RefCell::new(String::new()), color_default: 0, qr2: 0, thread: 0, now: 0, scene_log: String::new(), w8_images: Default::default(), w8: W8State::default() }
    }
    pub fn line(&self, s: String) {
        let mut l = self.log.borrow_mut();
        l.push_str(&s);
        l.push('\n');
    }
}

impl GuiHost for GameSetupHost {
    fn font_height(&self, font: Option<&rts_lockstep_engine::client::window_script::GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
}

impl ShellHost for GameSetupHost {
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
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        if !self.w8.keys_on {
            return 0;
        }
        let n = self.w8.keys.len() as u32 + 1;
        *self.w8.keys.entry(name.to_vec()).or_insert(n)
    }
}

fn win_id(w: Option<WinId>) -> i64 {
    w.map_or(-1, |w| w as i64)
}

impl WolGameSetupMenuHost for GameSetupHost {
    fn wol_game_setup_menu(&mut self) -> &mut WolGameSetupMenu {
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
    fn format_text_arg(&self, fmt: &[u16], a: &[u16]) -> Vec<u16> {
        unreachable!("format_text_arg")
    }
    fn format_two_text(&self, fmt: &[u16], a: &[u16], b: &[u16]) -> Vec<u16> {
        unreachable!("format_two_text")
    }
    fn format_one_int(&self, fmt: &[u16], a: i32) -> Vec<u16> {
        unreachable!("format_one_int")
    }
    fn format_staging_player_info(&self, fmt: &[u16], locale: &[u16], ping: i32, wins: i32, losses: i32, discons: i32, favorite_side: &[u16]) -> Vec<u16> {
        unreachable!("format_staging_player_info")
    }
    fn gamespy_color_player_owner(&self) -> i32 {
        unreachable!("gamespy_color_player_owner")
    }
    fn gamespy_color_player_normal(&self) -> i32 {
        unreachable!("gamespy_color_player_normal")
    }
    fn time_now_ms(&self) -> u32 {
        unreachable!("time_now_ms")
    }
    fn has_gamespy_info(&self) -> bool {
        unreachable!("has_gamespy_info")
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
    fn gamespy_ping_string(&self) -> Vec<u8> {
        self.w8.ping_string.clone()
    }
    fn gamespy_max_messages_per_update(&self) -> i32 {
        unreachable!("gamespy_max_messages_per_update")
    }
    fn gamespy_am_i_host(&self) -> bool {
        self.w8.am_i_host
    }
    fn gamespy_set_current_group_room(&mut self, id: i32) {
        unreachable!("gamespy_set_current_group_room")
    }
    fn gamespy_set_game_options(&mut self) {
        self.line("setGameOptions".to_string());
    }
    fn gamespy_leave_staging_room(&mut self) {
        unreachable!("gamespy_leave_staging_room")
    }
    fn gamespy_reset(&mut self) {
        unreachable!("gamespy_reset")
    }
    fn gamespy_is_disconnected_after_game_start(&self) -> Option<i32> {
        unreachable!("gamespy_is_disconnected_after_game_start")
    }
    fn gamespy_mark_disconnected_after_game_start(&mut self, reason: i32) {
        unreachable!("gamespy_mark_disconnected_after_game_start")
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
    fn gamespy_update_player_info(&mut self, p: &LobbyPlayerInfo) {
        unreachable!("gamespy_update_player_info")
    }
    fn gamespy_player_left_group_room(&mut self, nick: &[u8]) {
        unreachable!("gamespy_player_left_group_room")
    }
    fn gamespy_player_info_by_name(&self, name: &[u8]) -> Option<LobbyPlayerInfo> {
        unreachable!("gamespy_player_info_by_name")
    }
    fn gamespy_is_buddy(&self, id: i32) -> bool {
        unreachable!("gamespy_is_buddy")
    }
    fn gamespy_config_ping_cutoff_good(&self) -> i32 {
        self.w8.good
    }
    fn gamespy_config_ping_cutoff_bad(&self) -> i32 {
        self.w8.bad
    }
    fn staging_room_exists(&self) -> bool {
        self.w8.room_exists
    }
    fn staging_room_info(&self) -> StagingRoomInfo {
        self.w8.room.clone()
    }
    fn staging_slot(&self, i: usize) -> Option<StagingSlotInfo> {
        self.w8.slots.get(i).cloned()
    }
    fn staging_slot_set_color(&mut self, i: usize, v: i32) {
        unreachable!("staging_slot_set_color")
    }
    fn staging_slot_set_player_template(&mut self, i: usize, v: i32) {
        self.w8.slots[i].player_template = v;
    }
    fn staging_slot_set_start_pos(&mut self, i: usize, v: i32) {
        self.w8.slots[i].start_pos = v;
    }
    fn staging_slot_set_team_number(&mut self, i: usize, v: i32) {
        unreachable!("staging_slot_set_team_number")
    }
    fn staging_slot_set_state(&mut self, i: usize, state: i32) {
        unreachable!("staging_slot_set_state")
    }
    fn staging_slot_set_accept(&mut self, i: usize) {
        unreachable!("staging_slot_set_accept")
    }
    fn staging_slot_set_map_availability(&mut self, i: usize, v: bool) {
        unreachable!("staging_slot_set_map_availability")
    }
    fn staging_slot_set_ip(&mut self, i: usize, v: u32) {
        unreachable!("staging_slot_set_ip")
    }
    fn staging_slot_set_port(&mut self, i: usize, v: u16) {
        unreachable!("staging_slot_set_port")
    }
    fn staging_slot_port(&self, i: usize) -> u16 {
        unreachable!("staging_slot_port")
    }
    fn staging_slot_set_nat_behavior(&mut self, i: usize, v: i32) {
        unreachable!("staging_slot_set_nat_behavior")
    }
    fn staging_slot_set_ping_string(&mut self, i: usize, s: &[u8]) {
        // `GameSpyGameSlot::setPingString` (StagingRoomGameInfo.cpp:449): the SDK's `getPingValue` is a stand-in on both sides, `atoi`
        self.w8.slots[i].ping = atoi(s);
    }
    fn staging_set_new_player_slot(&mut self, i: usize, name: &[u16], ip: u32) {
        unreachable!("staging_set_new_player_slot")
    }
    fn staging_reset_accepted(&mut self) {
        self.line("resetAccepted".to_string());
    }
    fn staging_reset(&mut self) {
        unreachable!("staging_reset")
    }
    fn staging_slot_num(&self, nick: &[u8]) -> i32 {
        unreachable!("staging_slot_num")
    }
    fn staging_set_map(&mut self, map: &[u8]) {
        unreachable!("staging_set_map")
    }
    fn staging_set_map_crc(&mut self, crc: u32) {
        unreachable!("staging_set_map_crc")
    }
    fn staging_set_map_size(&mut self, size: u32) {
        unreachable!("staging_set_map_size")
    }
    fn staging_adjust_slots_for_map(&mut self) {
        unreachable!("staging_adjust_slots_for_map")
    }
    fn staging_set_starting_cash(&mut self, cash: u32) {
        unreachable!("staging_set_starting_cash")
    }
    fn staging_set_superweapon_restriction(&mut self, v: i32) {
        unreachable!("staging_set_superweapon_restriction")
    }
    fn staging_set_old_factions_only(&mut self, v: bool) {
        unreachable!("staging_set_old_factions_only")
    }
    fn multiplayer_default_starting_money(&self) -> u32 {
        unreachable!("multiplayer_default_starting_money")
    }
    fn multiplayer_num_colors(&self) -> i32 {
        unreachable!("multiplayer_num_colors")
    }
    fn player_template_count(&self) -> i32 {
        unreachable!("player_template_count")
    }
    fn player_template_is_old_faction(&self, n: i32) -> Option<bool> {
        unreachable!("player_template_is_old_faction")
    }
    fn player_template_side(&self, n: i32) -> Option<Vec<u8>> {
        unreachable!("player_template_side")
    }
    fn parse_options_into_staging_room(&mut self, options: &[u8]) -> bool {
        unreachable!("parse_options_into_staging_room")
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
    fn game_spy_game_is_in_game_and_am_host(&self) -> bool {
        unreachable!("game_spy_game_is_in_game_and_am_host")
    }
    fn game_spy_game_reset(&mut self) {
        unreachable!("game_spy_game_reset")
    }
    fn game_spy_game_launch_game(&mut self) {
        unreachable!("game_spy_game_launch_game")
    }
    fn game_spy_game_get_use_stats(&self) -> bool {
        unreachable!("game_spy_game_get_use_stats")
    }
    fn game_spy_game_get_game_name(&self) -> Vec<u16> {
        unreachable!("game_spy_game_get_game_name")
    }
    fn game_spy_game_copy_staging_room_and_start(&mut self) {
        unreachable!("game_spy_game_copy_staging_room_and_start")
    }
    fn game_spy_game_local_slot_num(&self) -> i32 {
        unreachable!("game_spy_game_local_slot_num")
    }
    fn game_spy_game_slot_color_and_template(&self, i: usize) -> Option<(i32, i32)> {
        unreachable!("game_spy_game_slot_color_and_template")
    }
    fn game_spy_game_am_i_host(&self) -> bool {
        unreachable!("game_spy_game_am_i_host")
    }
    fn map_file_name_for_item_data(&self, item: u32) -> Vec<u8> {
        self.w8.map_names[item as usize - 1].clone()
    }
    fn game_spy_game_map(&self) -> Vec<u8> {
        unreachable!("game_spy_game_map")
    }
    fn game_spy_game_superweapon_restriction(&self) -> i32 {
        unreachable!("game_spy_game_superweapon_restriction")
    }
    fn game_spy_game_starting_cash(&self) -> u32 {
        unreachable!("game_spy_game_starting_cash")
    }
    fn custom_pref_set_preferred_color(&mut self, v: i32) {
        unreachable!("custom_pref_set_preferred_color")
    }
    fn custom_pref_set_preferred_faction(&mut self, v: i32) {
        unreachable!("custom_pref_set_preferred_faction")
    }
    fn custom_pref_set_preferred_map(&mut self, map: &[u8]) {
        unreachable!("custom_pref_set_preferred_map")
    }
    fn custom_pref_set_superweapon_restricted(&mut self, v: bool) {
        unreachable!("custom_pref_set_superweapon_restricted")
    }
    fn custom_pref_set_starting_cash(&mut self, v: u32) {
        unreachable!("custom_pref_set_starting_cash")
    }
    fn custom_pref_write(&mut self) {
        unreachable!("custom_pref_write")
    }
    fn custom_pref_preferred_color(&mut self) -> i32 {
        unreachable!("custom_pref_preferred_color")
    }
    fn custom_pref_preferred_faction(&mut self) -> i32 {
        unreachable!("custom_pref_preferred_faction")
    }
    fn custom_pref_preferred_map(&mut self) -> Vec<u8> {
        unreachable!("custom_pref_preferred_map")
    }
    fn custom_pref_starting_cash(&mut self) -> u32 {
        unreachable!("custom_pref_starting_cash")
    }
    fn custom_pref_superweapon_restricted(&mut self) -> bool {
        unreachable!("custom_pref_superweapon_restricted")
    }
    fn option_prefs_firewall_behavior(&mut self) -> i32 {
        unreachable!("option_prefs_firewall_behavior")
    }
    fn map_cache_update(&mut self) {
        unreachable!("map_cache_update")
    }
    fn map_cache_find_map(&self, map: &[u8]) -> Option<(i32, Vec<u16>, bool)> {
        self.w8.maps.get(map).cloned()
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<(u32, u32)> {
        unreachable!("map_cache_find")
    }
    fn would_map_transfer(&self, map: &[u8]) -> bool {
        unreachable!("would_map_transfer")
    }
    fn map_leaf_name(&self, map: &[u8]) -> Vec<u8> {
        unreachable!("map_leaf_name")
    }
    fn net_min_players(&self) -> i32 {
        unreachable!("net_min_players")
    }
    fn ps_find_player_stats_by_id(&mut self, id: i32) -> SetupPlayerStats {
        unreachable!("ps_find_player_stats_by_id")
    }
    fn ps_parse_and_track_player_stats(&mut self, data: &[u8]) {
        unreachable!("ps_parse_and_track_player_stats")
    }
    fn ps_request_read_player_stats(&mut self, id: i32) {
        unreachable!("ps_request_read_player_stats")
    }
    fn ps_format_local_stats_for_utm(&mut self) -> Vec<u8> {
        unreachable!("ps_format_local_stats_for_utm")
    }
    fn has_peer_message_queue(&self) -> bool {
        unreachable!("has_peer_message_queue")
    }
    fn peer_queue_is_connected(&self) -> bool {
        unreachable!("peer_queue_is_connected")
    }
    fn peer_next_response(&mut self) -> Option<SetupPeerResponse> {
        unreachable!("peer_next_response")
    }
    fn peer_request_utm_player(&mut self, id: &[u8], nick: &[u8], options: &[u8]) {
        let n = if nick.is_empty() { "-".to_string() } else { nick.iter().map(|b| format!("{b:02x}")).collect::<String>() };
        self.line(format!("utm {} {} {}", String::from_utf8_lossy(id), n, String::from_utf8_lossy(options)));
    }
    fn peer_request_utm_room(&mut self, id: &[u8], options: &[u8]) {
        unreachable!("peer_request_utm_room")
    }
    fn peer_request_start_game(&mut self) {
        unreachable!("peer_request_start_game")
    }
    fn handle_buddy_responses(&mut self) {
        unreachable!("handle_buddy_responses")
    }
    fn handle_persistent_storage_responses(&mut self) {
        unreachable!("handle_persistent_storage_responses")
    }
    fn lobby_queued_utm_pop_front(&mut self) -> Option<LobbyUtm> {
        unreachable!("lobby_queued_utm_pop_front")
    }
    fn nat_exists(&self) -> bool {
        unreachable!("nat_exists")
    }
    fn nat_delete(&mut self) {
        unreachable!("nat_delete")
    }
    fn nat_update(&mut self) -> SetupNatState {
        unreachable!("nat_update")
    }
    fn nat_process_global_message(&mut self, slot: i32, options: &[u8]) {
        unreachable!("nat_process_global_message")
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
    fn raise_gs_message_box(&mut self) {
        unreachable!("raise_gs_message_box")
    }
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]) {
        unreachable!("gs_message_box_ok")
    }
    fn establish_connections_menu_end(&mut self) {
        unreachable!("establish_connections_menu_end")
    }
    fn wol_map_select_layout(&self) -> Option<usize> {
        self.w8.map_select_layout.then_some(1)
    }
    fn set_wol_map_select_layout(&mut self, l: Option<usize>) {
        unreachable!("set_wol_map_select_layout")
    }
    fn position_start_spots_for_map(&mut self, wm: &mut GameWindowManager, map: &[u8], map_window: Option<WinId>) {
        let n = if map.is_empty() { "-".to_string() } else { map.iter().map(|b| format!("{b:02x}")).collect::<String>() };
        self.line(format!("positionStartSpots map {n} win {}", win_id(map_window)));
    }
    fn position_start_spots_for_staging_room(&mut self, wm: &mut GameWindowManager, map_window: Option<WinId>) {
        self.line(format!("positionStartSpots room win {}", win_id(map_window)));
    }
    fn update_map_start_spots(&mut self, wm: &mut GameWindowManager) {
        self.line("updateMapStartSpots".to_string());
    }
    fn gui_util_populate_starting_cash(&mut self, wm: &mut GameWindowManager, combo: Option<WinId>) {
        unreachable!("gui_util_populate_starting_cash")
    }
    fn gui_util_populate_color(&mut self, wm: &mut GameWindowManager, i: usize) {
        unreachable!("gui_util_populate_color")
    }
    fn gui_util_populate_player_template(&mut self, wm: &mut GameWindowManager, i: usize, allow_observers: bool) {
        self.line(format!("populateTpl {i} {}", i32::from(allow_observers)));
    }
    fn gui_util_populate_team(&mut self, wm: &mut GameWindowManager, i: usize) {
        unreachable!("gui_util_populate_team")
    }
    fn gui_util_update_slot_list(&mut self, wm: &mut GameWindowManager) {
        self.line("UpdateSlotList".to_string());
    }
    fn slot_list_updates_enabled(&self) -> bool {
        unreachable!("slot_list_updates_enabled")
    }
    fn enable_slot_list_updates(&mut self, v: bool) {
        unreachable!("enable_slot_list_updates")
    }
    fn win_set_tooltip_func(&mut self, wm: &mut GameWindowManager, w: WinId, func: &[u8]) {
        unreachable!("win_set_tooltip_func")
    }
    fn win_set_tooltip(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]) {
        wm.tree.windows[w].inst.tooltip_text = Some(text.to_vec());
    }
    fn combo_list_box_set_tooltip_func(&mut self, wm: &mut GameWindowManager, combo: WinId, func: &[u8]) {
        unreachable!("combo_list_box_set_tooltip_func")
    }
    fn win_screen_rect(&self, wm: &GameWindowManager, w: WinId) -> (i32, i32, i32, i32) {
        unreachable!("win_screen_rect")
    }
    fn win_set_enabled_image0(&mut self, wm: &mut GameWindowManager, w: WinId, image: &Option<Vec<u8>>) {
        wm.tree.windows[w].inst.enabled_draw_data[0].image = image.clone();
    }
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>> {
        unreachable!("find_image_by_name")
    }
    fn mouse_set_cursor_tooltip(&mut self, text: &[u16], width: f32) {
        unreachable!("mouse_set_cursor_tooltip")
    }
    fn in_game_ui_message(&mut self, label: &[u8]) {
        unreachable!("in_game_ui_message")
    }
}
