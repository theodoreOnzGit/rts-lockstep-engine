//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLLobbyMenu.cpp
//!     (the file's statics, `handleLobbySlashCommands`, `SetLobbyAttemptHostJoin`, `playerTooltip`,
//!      `populateGroupRoomListbox`, `LookupSmallRankImage`, `insertPlayerInListbox`, `PopulateLobbyPlayerListbox`,
//!      `WOLLobbyMenuInit/Shutdown/Update/Input/System`, `shutdownComplete`, `fillPlayerInfo`, `refreshGameList`,
//!      `refreshPlayerList`)
//!
//! Not translated: the `PERF_TEST` / `DEBUG_LOGGING` code, the `DEBUG_LOG` lines, the debug-build-only `/fakecrc` and
//! `/slots` slash commands and the debug-build `g_fakeCRC` CRC override (`_INTERNAL` / `_DEBUG` only: the release
//! branch is translated), the commented-out chat slider (`doSliderTrack`, `GSM_SLIDER_TRACK`), the `#if 0`
//! branches. The GameSpy service is gone: `TheGameSpyInfo`, `TheGameSpyConfig`, `TheGameSpyGame`, the peer and
//! persistent-storage queues, `TheLadderList`, `CustomMatchPreferences`, `TheRankPointValues` and the lobby game list
//! (`LobbyUtils.cpp`: `GrabWindowInfo`, `ReleaseWindowInfo`, `GetGameListBox`, `GetGameInfoListBox`,
//! `GetGameListBoxID`, `RefreshGameListBoxes`, `RefreshGameInfoListBox`, `ToggleGameListType`, `HandleSortButton`)
//! are the host's.
//!
//! Upstream quirks kept and flagged for the differential pass:
//! - `PEERRESPONSE_DISCONNECT` has no `break` and falls through into `PEERRESPONSE_CREATESTAGINGROOM`, which reads
//!   `resp.createStagingRoom.result` from the union the disconnect filled; [`LobbyPeerResponse::Disconnect`] carries
//!   what the host reads there (`as_create_staging_room_success`).
//! - `PopulateLobbyPlayerListbox` passes `(Int *)(&selectedIndices)` (the address of the pointer variable) to
//!   `GadgetListBoxGetSelected`, which writes the selection list over it; here the selection is read with
//!   [`lb::get_selected`] and the loop stops at the first negative entry as upstream does.
//! - `GBM_SELECTED` on the Join button reads `GadgetListBoxGetSelected(GetGameListBox(), &selected)` as a single int.

use crate::client::gadget;
use crate::logic::modules::plant;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_info::{slot_state, MAX_SLOTS};

/// `enum { COLUMN_PLAYERNAME = 1 }`.
const COLUMN_PLAYERNAME: i32 = 1;
/// `gameListRefreshInterval`, `playerListRefreshInterval`.
const GAME_LIST_REFRESH_INTERVAL: u32 = 10000;
const PLAYER_LIST_REFRESH_INTERVAL: u32 = 5000;

/// `rankNames[]`.
const RANK_NAMES: [&str; 10] = ["Private", "Corporal", "Sergeant", "Lieutenant", "Captain", "Major", "Colonel", "General", "Brigadier", "Commander"];

/// `PlayerInfo` (GameSpy/PeerDefs.h).
#[derive(Clone, Debug, Default)]
pub struct LobbyPlayerInfo {
    pub name: Vec<u8>,
    pub profile_id: i32,
    pub flags: i32,
    pub wins: i32,
    pub losses: i32,
    pub locale: Vec<u8>,
    pub rank_points: i32,
    pub side: i32,
    pub preorder: bool,
    /// `isIgnored()`.
    pub is_ignored: bool,
}

/// `GameSpyGroupRoom`.
#[derive(Clone, Debug, Default)]
pub struct LobbyGroupRoom {
    pub group_id: i32,
    pub translated_name: Vec<u16>,
}

/// `PEER_FLAG_OP`.
pub const PEER_FLAG_OP: i32 = 1 << 1;

/// A queued `PEERRESPONSE_PLAYERUTM` / `PEERRESPONSE_ROOMUTM` (`TheLobbyQueuedUTMs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbyUtm {
    pub is_room_utm: bool,
    pub command: Vec<u8>,
    pub nick: Vec<u8>,
    pub command_options: Vec<u8>,
}

/// `PEERResult` values `Join` tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinResult {
    FullRoom,
    InviteOnlyRoom,
    BannedFromRoom,
    BadPassword,
    AlreadyInRoom,
    NoConnection,
    Other,
}

/// `resp.stagingRoom.action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagingAction {
    Clear,
    Add,
    Update,
    Remove,
    Other,
}

/// The `PeerResponse` fields of a `PEERRESPONSE_STAGINGROOM`.
#[derive(Clone, Debug)]
pub struct LobbyStagingRoomResponse {
    pub action: StagingAction,
    pub id: i32,
    pub percent_complete: i32,
    pub requires_password: bool,
    pub version: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub ladder_port: u16,
    pub num_players: i32,
    pub max_players: i32,
    pub num_observers: i32,
    pub wins: [i32; MAX_SLOTS],
    pub losses: [i32; MAX_SLOTS],
    pub profile_id: [i32; MAX_SLOTS],
    pub faction: [i32; MAX_SLOTS],
    pub color: [i32; MAX_SLOTS],
    pub staging_room_map_name: Vec<u8>,
    pub staging_server_name: Vec<u8>,
    pub staging_server_ping_string: Vec<u8>,
    pub staging_server_ladder_ip: Vec<u8>,
    pub staging_room_player_names: [Vec<u8>; MAX_SLOTS],
}

/// A `GameSpyStagingRoom` as `Update` builds it (the setters' results).
#[derive(Clone, Debug, Default)]
pub struct LobbyStagingRoom {
    pub game_name: Vec<u16>,
    pub id: i32,
    pub has_password: bool,
    pub version: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub ping_string: Vec<u8>,
    pub ladder_ip: Vec<u8>,
    pub ladder_port: u16,
    pub reported_num_players: i32,
    pub reported_max_players: i32,
    pub reported_num_observers: i32,
    /// `room.setMap(TheGameState->portableMapPathToRealMapPath(mapName))`, already converted by the host.
    pub map: Vec<u8>,
    pub slots: Vec<LobbyStagingSlot>,
}

/// One `GameSpyGameSlot` as `Update` fills it.
#[derive(Clone, Debug, Default)]
pub struct LobbyStagingSlot {
    pub wins: i32,
    pub losses: i32,
    pub profile_id: i32,
    pub player_template: i32,
    pub color: i32,
    /// `slot->setState(state, name)`.
    pub state: i32,
    pub name: Vec<u16>,
}

/// The `PeerResponse` types `Update` acts on.
#[derive(Clone, Debug)]
pub enum LobbyPeerResponse {
    /// `PEERRESPONSE_JOINGROUPROOM`.
    JoinGroupRoom { ok: bool, id: i32 },
    /// `PEERRESPONSE_PLAYERCHANGEDFLAGS`, `PLAYERCHANGEDNICK`, `PLAYERINFO`: `fillPlayerInfo` then `updatePlayerInfo`.
    PlayerUpdate(LobbyPlayerInfo),
    /// `PEERRESPONSE_PLAYERJOIN`: `roomType == GroupRoom`.
    PlayerJoin { player: LobbyPlayerInfo, is_group_room: bool },
    /// `PEERRESPONSE_PLAYERUTM`, `PEERRESPONSE_ROOMUTM`.
    Utm(LobbyUtm),
    /// `PEERRESPONSE_PLAYERLEFT`: `nick`.
    PlayerLeft { nick: Vec<u8> },
    /// `PEERRESPONSE_MESSAGE`.
    Message { nick: Vec<u8>, profile_id: i32, text: Vec<u8>, is_private: bool, is_action: bool },
    /// `PEERRESPONSE_DISCONNECT` (falls through into the create-staging-room case, see the file header).
    Disconnect { reason: i32, as_create_staging_room_success: bool },
    /// `PEERRESPONSE_CREATESTAGINGROOM`: `result == PEERJoinSuccess`.
    CreateStagingRoom { success: bool },
    /// `PEERRESPONSE_JOINSTAGINGROOM`.
    JoinStagingRoom { ok: bool, result: JoinResult, staging_room_player_names: [Vec<u8>; MAX_SLOTS] },
    /// `PEERRESPONSE_STAGINGROOMLISTCOMPLETE`.
    StagingRoomListComplete,
    /// `PEERRESPONSE_STAGINGROOM`.
    StagingRoom(LobbyStagingRoomResponse),
    /// Any other response type.
    Other,
}

/// What `GBM_SELECTED` on Join reads of the room in `getStagingRoomList()`.
#[derive(Clone, Debug, Default)]
pub struct LobbyRoomToJoin {
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub ladder_ip: Vec<u8>,
    pub ladder_port: u16,
    pub num_players: i32,
    pub has_password: bool,
    pub game_name: Vec<u16>,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`) and the function-local statics of `System`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolLobbyMenu {
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<Vec<u8>>,
    pub raise_message_boxes: bool,
    pub game_list_refresh_time: u32,
    pub player_list_refresh_time: u32,
    /// `Bool DontShowMainMenu`.
    pub dont_show_main_menu: bool,
    pub parent_wol_lobby_id: i32,
    pub button_back_id: i32,
    pub button_host_id: i32,
    pub button_refresh_id: i32,
    pub button_join_id: i32,
    pub button_buddy_id: i32,
    pub button_emote_id: i32,
    pub text_entry_chat_id: i32,
    pub listbox_lobby_players_id: i32,
    pub listbox_lobby_chat_id: i32,
    pub combo_lobby_group_rooms_id: i32,
    pub parent_wol_lobby: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_host: Option<WinId>,
    pub button_refresh: Option<WinId>,
    pub button_join: Option<WinId>,
    pub button_buddy: Option<WinId>,
    pub button_emote: Option<WinId>,
    pub text_entry_chat: Option<WinId>,
    pub listbox_lobby_players: Option<WinId>,
    pub listbox_lobby_chat: Option<WinId>,
    pub combo_lobby_group_rooms: Option<WinId>,
    pub group_room_to_join: i32,
    pub initial_gadget_delay: i32,
    pub just_entered: bool,
    /// `s_tryingToHostOrJoin`.
    pub trying_to_host_or_join: bool,
    /// `buttonGameListTypeToggleID` (set at `GWM_CREATE`).
    pub button_game_list_type_toggle_id: i32,
    /// `static UnsignedInt lastFrame`, `static Int lastID` of `GLM_SELECTED`.
    pub last_frame: u32,
    pub last_id: i32,
    /// `TheLobbyQueuedUTMs`.
    pub queued_utms: Vec<LobbyUtm>,
}

impl WolLobbyMenu {
    /// The statics' initial values.
    pub fn new() -> Self {
        WolLobbyMenu { initial_gadget_delay: 2, last_id: -1, ..Default::default() }
    }
}

/// What the lobby reaches in the GameSpy layer and the engine.
pub trait WolLobbyMenuHost: ShellHost {
    fn wol_lobby_menu(&mut self) -> &mut WolLobbyMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn format_text_arg(&self, fmt: &[u16], text: &[u16]) -> Vec<u16>;
    fn format_text_int_int(&self, fmt: &[u16], text: &[u16], a: i32, b: i32) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_*]`.
    fn gamespy_color_default(&self) -> i32;
    fn gamespy_color_current_room(&self) -> i32;
    fn gamespy_color_room(&self) -> i32;
    fn gamespy_color_player_ignored(&self) -> i32;
    fn gamespy_color_player_owner(&self) -> i32;
    fn gamespy_color_player_buddy(&self) -> i32;
    fn gamespy_color_player_normal(&self) -> i32;
    /// `image->getImageWidth()`; `TheMappedImageCollection->findImageByName(name)` (the name when found).
    fn image_width(&self, image: &[u8]) -> i32;
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>>;
    /// `TheRankPointValues->m_ranks[i]`.
    fn rank_point_value(&self, i: usize) -> i32;
    /// `ThePlayerTemplateStore->getNthPlayerTemplate(n)->getSide()`.
    fn player_template_side(&self, n: i32) -> Option<Vec<u8>>;

    // TheGameSpyInfo
    fn gamespy_local_name(&self) -> Vec<u16>;
    fn gamespy_local_name_ascii(&self) -> Vec<u8>;
    fn gamespy_local_profile_id(&self) -> i32;
    fn gamespy_max_messages_per_update(&self) -> i32;
    fn gamespy_current_group_room(&self) -> i32;
    fn gamespy_set_current_group_room(&mut self, id: i32);
    fn gamespy_group_room_list(&self) -> Vec<LobbyGroupRoom>;
    fn gamespy_clear_player_info_map(&mut self);
    /// `getPlayerInfoMap()` in key order.
    fn gamespy_player_info_map(&self) -> Vec<LobbyPlayerInfo>;
    fn gamespy_player_info_by_name(&self, name: &[u8]) -> Option<LobbyPlayerInfo>;
    fn gamespy_update_player_info(&mut self, p: &LobbyPlayerInfo);
    fn gamespy_player_left_group_room(&mut self, nick: &[u8]);
    /// `getBuddyMap()->find(profileID) != end()`; `isBuddy(profileID)`.
    fn gamespy_is_buddy(&self, profile_id: i32) -> bool;
    fn gamespy_did_player_preorder(&self, profile_id: i32) -> bool;
    fn gamespy_join_group_room(&mut self, id: i32);
    fn gamespy_join_best_group_room(&mut self);
    fn gamespy_leave_group_room(&mut self);
    fn gamespy_reset(&mut self);
    fn gamespy_register_text_window(&mut self, w: Option<WinId>);
    fn gamespy_unregister_text_window(&mut self, w: Option<WinId>);
    fn gamespy_add_text(&mut self, wm: &mut GameWindowManager, text: &[u16], color: i32, win: Option<WinId>);
    fn gamespy_send_chat(&mut self, wm: &mut GameWindowManager, text: &[u16], is_emote: bool, win: Option<WinId>);
    #[allow(clippy::too_many_arguments)]
    fn gamespy_add_chat(&mut self, wm: &mut GameWindowManager, nick: &[u8], profile_id: i32, text: &[u8], is_public: bool, is_action: bool, win: Option<WinId>);
    fn gamespy_clear_staging_room_list(&mut self);
    fn gamespy_has_staging_room_list_changed(&self) -> bool;
    fn gamespy_saw_full_game_list(&mut self);
    fn gamespy_add_staging_room(&mut self, room: &LobbyStagingRoom);
    fn gamespy_update_staging_room(&mut self, room: &LobbyStagingRoom);
    fn gamespy_remove_staging_room(&mut self, id: i32);
    /// `getCurrentStagingRoom()`: the host's name (slot 0) when there is a room.
    fn gamespy_current_staging_room_host_name(&self) -> Option<Vec<u8>>;
    fn gamespy_mark_as_staging_room_host(&mut self);
    fn gamespy_set_game_options(&mut self);
    fn gamespy_mark_as_staging_room_joiner(&mut self, id: i32);
    /// `getStagingRoomList()->find(id)`.
    fn gamespy_staging_room_to_join(&self, id: i32) -> Option<LobbyRoomToJoin>;
    /// `TheLadderList->findLadder(ip, port) != NULL`.
    fn ladder_exists(&self, ip: &[u8], port: u16) -> bool;
    /// `TheGameSpyConfig->getQMChannel()`, `restrictGamesToLobby()`, `isPlayerVIP(profileID)`.
    fn gamespy_config_qm_channel(&self) -> i32;
    fn gamespy_config_restrict_games_to_lobby(&self) -> bool;
    fn gamespy_config_is_player_vip(&self, profile_id: i32) -> bool;
    /// `TheGameSpyGame`.
    fn game_spy_game_reset(&mut self);
    fn game_spy_game_set_game_name(&mut self, name: &[u16]);
    fn game_spy_game_set_ladder_ip(&mut self, ip: &[u8]);
    fn game_spy_game_set_ladder_port(&mut self, port: u16);
    /// `TheGameState->portableMapPathToRealMapPath(mapName)`.
    fn portable_map_path_to_real_map_path(&self, map_name: &[u8]) -> Vec<u8>;
    /// `TheGlobalData->m_exeCRC`, `m_iniCRC`.
    fn global_exe_crc(&self) -> u32;
    fn global_ini_crc(&self) -> u32;

    // CustomMatchPreferences
    fn custom_pref_uses_long_game_list(&mut self) -> bool;
    /// `pref.setUsesLongGameList(v); pref.write()`.
    fn custom_pref_set_uses_long_game_list_and_write(&mut self, v: bool);

    // peer queue
    fn has_peer_message_queue(&self) -> bool;
    fn peer_next_response(&mut self) -> Option<LobbyPeerResponse>;
    fn peer_request_start_game_list(&mut self, restrict_game_list: bool);
    fn peer_request_stop_game_list(&mut self);
    fn peer_request_get_extended_staging_room_info(&mut self, id: i32);
    fn peer_request_join_staging_room(&mut self, game_name: &[u16], id: i32, password: &[u8]);
    fn handle_buddy_responses(&mut self);
    fn handle_persistent_storage_responses(&mut self);

    // overlays and message boxes
    fn gamespy_open_overlay_game_options(&mut self, wm: &mut GameWindowManager);
    fn gamespy_open_overlay_game_password(&mut self, wm: &mut GameWindowManager);
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager);
    fn raise_gs_message_box(&mut self);
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]);

    // LobbyUtils.cpp
    fn grab_window_info(&mut self, wm: &mut GameWindowManager);
    fn release_window_info(&mut self);
    fn get_game_info_list_box(&self) -> Option<WinId>;
    fn get_game_list_box(&self) -> Option<WinId>;
    fn get_game_list_box_id(&self) -> i32;
    fn refresh_game_list_boxes(&mut self, wm: &mut GameWindowManager);
    fn refresh_game_info_list_box(&mut self, wm: &mut GameWindowManager, game_list: Option<WinId>, info_list: Option<WinId>);
    fn toggle_game_list_type(&mut self, wm: &mut GameWindowManager);
    fn handle_sort_button(&mut self, wm: &mut GameWindowManager, control_id: i32) -> bool;

    // right-click menus and tooltips
    /// `winSetTooltipFunc(playerTooltip)` on the player list.
    fn set_player_tooltip_func(&mut self, wm: &mut GameWindowManager, w: WinId);
    /// `TheMouse->setCursorTooltip(text, -1, NULL, width)`.
    fn mouse_set_cursor_tooltip(&mut self, text: &[u16], width: f32);
    /// `layout->getFirstWindow()`, `runInit()`.
    fn layout_first_window(&mut self, wm: &GameWindowManager, layout: usize) -> Option<WinId>;
    fn layout_run_init(&mut self, wm: &mut GameWindowManager, layout: usize);
    /// `setUnignoreText(rcLayout, nick, id)` (WOLBuddyOverlay.cpp).
    fn set_unignore_text(&mut self, wm: &mut GameWindowManager, layout: usize, nick: &[u8], id: i32);
    /// `rcMenu->winSetUserData(new GameSpyRCMenuData{id, nick, itemType})`.
    fn win_set_user_data_rc_menu(&mut self, wm: &mut GameWindowManager, w: WinId, id: i32, nick: &[u8], is_buddy: bool);
    /// `rcMenu->winSetUserData((void *)selectedID)`.
    fn win_set_user_data_int(&mut self, wm: &mut GameWindowManager, w: WinId, v: i32);
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);

    // engine
    fn game_client_frame(&self) -> u32;
    fn game_logic_in_shell_game_first_frame(&self) -> bool;
    fn signal_ui_interaction_entered_from_game(&mut self);
    /// `getQR2HostingStatus()`, `isThreadHosting`.
    fn qr2_hosting_status(&self) -> i32;
    fn is_thread_hosting(&self) -> i32;
}

fn key_of<H: WolLobbyMenuHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("WOLCustomLobby.wnd:{n}").as_bytes()) as i32
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `atoi`.
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    let neg = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let mut v: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add(i32::from(s[i] - b'0'));
        i += 1;
    }
    if neg { v.wrapping_neg() } else { v }
}

/// `handleLobbySlashCommands(uText)`.
pub fn handle_lobby_slash_commands<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, u_text: &[u16]) -> bool {
    // message.translate(uText): each unit cast to `char` and concatenated with `concat(const char c)`, which adds nothing for a zero byte.
    // ~~`u_text.iter().map(|&c| c as u8).collect()` kept a unit whose low byte is 0 as a NUL~~ **CORRECTED 2026-11-21 (#215, wol6; the differential test
    // tests/differential_draft_wol_slash.rs found 159 of 20,000 cases):** such units are dropped, as upstream's translate does.
    let message: Vec<u8> = u_text.iter().map(|&c| c as u8).filter(|&c| !(c == 0 && !plant(23461))).collect();

    if message.first() != Some(&b'/') {
        return false; // not a slash command
    }

    // remainder.nextToken(&token) with the default separators " \n\r\t"
    let remainder = &message[1..];
    let seps = b" \n\r\t";
    let start = if plant(23467) { 0 } else { remainder.iter().position(|c| !seps.contains(c)).unwrap_or(remainder.len()) };
    let end = remainder[start..].iter().position(|c| seps.contains(c)).map_or(remainder.len(), |e| start + e);
    let token = if plant(23462) { remainder[start..end].to_vec() } else { remainder[start..end].to_ascii_lowercase() };

    if token == b"host" {
        let s = if plant(23466) { utf16(&format!("Hosting qr2:{} thread:{}", h.is_thread_hosting(), h.qr2_hosting_status())) } else { utf16(&format!("Hosting qr2:{} thread:{}", h.qr2_hosting_status(), h.is_thread_hosting())) };
        let c = h.gamespy_color_default();
        h.gamespy_add_text(wm, &s, c, None);
        true // was a slash command
    } else if token == b"me" && u_text.len() > if plant(23463) { 3 } else { 4 } {
        let players = h.wol_lobby_menu().listbox_lobby_players;
        let from = if plant(23464) { 3 } else { 4 };
        h.gamespy_send_chat(wm, &u_text[from..], !plant(23465), if plant(23468) { None } else { players });
        true // was a slash command
    } else if token == b"refresh" {
        // Added 2/19/03 added the game refresh
        refresh_game_list(wm, h, !plant(23469));
        refresh_player_list(wm, h, true);
        true // was a slash command
    } else {
        // (the commented-out togglegamelist / adjustchat and the debug-only fakecrc / slots are not translated)
        false // not a slash command
    }
}

/// `SetLobbyAttemptHostJoin(start)`.
pub fn set_lobby_attempt_host_join<H: WolLobbyMenuHost>(h: &mut H, start: bool) {
    h.wol_lobby_menu().trying_to_host_or_join = start;
}

/// The rank for `rankPoints`: `while (rankPoints >= m_ranks[i + 1]) ++i`.
fn rank_for_points<H: WolLobbyMenuHost>(h: &H, rank_points: i32) -> i32 {
    let mut i: usize = 0;
    while if plant(23482) { rank_points > h.rank_point_value(i + 1) } else { rank_points >= h.rank_point_value(i + 1) } {
        i += 1;
    }
    i as i32
}

/// `playerTooltip(window, instData, mouse)` (WOLLobbyMenu.cpp:165): the player list's tooltip callback.
pub fn player_tooltip<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    let x = (mouse & 0xFFFF) as u16 as i16 as i32; // LOLONGTOSHORT
    let y = (mouse >> 16) as u16 as i16 as i32; // HILONGTOSHORT

    let (row, col) = lb::get_entry_based_on_xy(wm, h, window, x, y).unwrap_or((-1, -1));

    if row == -1 || col == -1 {
        h.mouse_set_cursor_tooltip(&[], -1.0); // TheEmptyString
        return;
    }

    let u_name = lb::get_text(wm, h, window, row, COLUMN_PLAYERNAME);
    let a_name: Vec<u8> = u_name.iter().map(|&c| c as u8).collect();

    // upstream dereferences `it->second` without checking `end()`; a missing player is treated as default info
    let info = h.gamespy_player_info_by_name(&a_name).unwrap_or_default();
    let local = h.gamespy_local_name_ascii();
    let is_local_player = local.eq_ignore_ascii_case(&info.name);

    if col == 0 {
        if info.preorder {
            let t = h.fetch_text(b"TOOLTIP:LobbyOfficersClub");
            h.mouse_set_cursor_tooltip(&t, -1.0);
        } else {
            h.mouse_set_cursor_tooltip(&[], -1.0);
        }
        return;
    }

    let player_locale = info.locale.clone();
    let locale_identifier = format!("WOL:Locale{:02}", atoi(&player_locale));
    let player_wins = info.wins;
    let player_losses = info.losses;
    let fmt = h.fetch_text(b"TOOLTIP:PlayerInfo");
    let locale_text = h.fetch_text(locale_identifier.as_bytes());
    let player_info = h.format_text_int_int(&fmt, &locale_text, player_wins, player_losses);

    let mut tooltip: Vec<u16>;
    if is_local_player {
        let fmt = h.fetch_text(b"TOOLTIP:LocalPlayer");
        tooltip = h.format_text_arg(&fmt, &u_name);
    } else {
        // not us
        if h.gamespy_is_buddy(info.profile_id) {
            // buddy
            let fmt = h.fetch_text(b"TOOLTIP:BuddyPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        } else if info.profile_id != 0 {
            // non-buddy profiled player
            let fmt = h.fetch_text(b"TOOLTIP:ProfiledPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        } else {
            // non-profiled player
            let fmt = h.fetch_text(b"TOOLTIP:GenericPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        }
    }

    if info.is_ignored {
        tooltip.extend(h.fetch_text(b"TOOLTIP:IgnoredModifier"));
    }

    if info.profile_id != 0 {
        tooltip.extend(player_info);
    }

    let rank = rank_for_points(h, info.rank_points);
    let mut side_name = b"GUI:RandomSide".to_vec();
    if info.side > 0 {
        if let Some(side) = h.player_template_side(info.side) {
            side_name = format!("SIDE:{}", String::from_utf8_lossy(&side)).into_bytes();
        }
    }
    let rank_name = format!("GUI:GSRank{rank}");
    let mut tmp = utf16("\n");
    tmp.extend(h.fetch_text(&side_name));
    tmp.extend(utf16(" "));
    tmp.extend(h.fetch_text(rank_name.as_bytes()));
    tooltip.extend(tmp);

    h.mouse_set_cursor_tooltip(&tooltip, 1.5); // the text and width are the only params used.
}

/// `populateGroupRoomListbox(lb)`.
fn populate_group_room_listbox<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, list: Option<WinId>) {
    let Some(list) = list else { return };

    cb::reset(wm, h, list);
    let mut index_to_select = -1;

    // now populate the combo box
    let qm_channel = h.gamespy_config_qm_channel();
    let current = h.gamespy_current_group_room();
    for room in h.gamespy_group_room_list() {
        if room.group_id != qm_channel {
            if room.group_id == current {
                let c = h.gamespy_color_current_room();
                let selected = cb::add_entry(wm, h, list, &room.translated_name, c);
                cb::set_item_data(wm, h, list, selected, room.group_id as u32);
                index_to_select = selected;
            } else {
                let c = h.gamespy_color_room();
                let selected = cb::add_entry(wm, h, list, &room.translated_name, c);
                cb::set_item_data(wm, h, list, selected, room.group_id as u32);
            }
        }
        // else: skipping QM groupID
    }

    cb::set_selected_pos(wm, h, list, index_to_select, false);
}

/// `LookupSmallRankImage(side, rankPoints)`: the mapped image's name.
pub fn lookup_small_rank_image<H: WolLobbyMenuHost>(h: &H, side: i32, rank_points: i32) -> Option<Vec<u8>> {
    if rank_points == 0 || (plant(23481) && rank_points < 0) {
        return None;
    }

    let rank = rank_for_points(h, rank_points);

    if !(0..if plant(23483) { 9 } else { 10 }).contains(&rank) {
        return None;
    }

    let side_str = match side {
        5 if plant(23484) => "CHA",
        13 if plant(23485) => "N",
        10 if plant(23486) => "N",
        2 | 5 | 6 | 7 => "USA",  // USA, Super Weapon, Laser, Air Force
        3 | 8 | 9 | 10 => "CHA", // China, Tank, Infantry, Nuke
        4 | 11 | 12 | 13 => "GLA", // GLA, Toxin, Demolition, Stealth
        _ => "N",
    };

    let name_at = if plant(23487) { (rank as usize).saturating_sub(1) } else { rank as usize };
    let full_image_name = if plant(23488) { format!("{}-{}", side_str, RANK_NAMES[name_at]) } else { format!("{}-{}", RANK_NAMES[name_at], side_str) };
    h.find_image_by_name(full_image_name.as_bytes())
}

/// `insertPlayerInListbox(info, color)`.
fn insert_player_in_listbox<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, info: &LobbyPlayerInfo, color: i32) -> i32 {
    let Some(players) = h.wol_lobby_menu().listbox_lobby_players else { return -1 };
    let u_str: Vec<u16> = info.name.iter().map(|&c| u16::from(c)).collect(); // uStr.translate(info.m_name)

    let current_rank = info.rank_points;
    let current_side = info.side;
    // (the commented-out PersistentStorage lookup is not translated)

    let is_preorder = h.gamespy_did_player_preorder(info.profile_id);

    let mut preorder_img = h.find_image_by_name(b"OfficersClubsmall");
    let mut w = preorder_img.as_ref().map_or(10, |i| h.image_width(i));
    w = lb::get_column_width(wm, players, 0).min(w);
    let hh = w;
    if !is_preorder {
        preorder_img = None;
    }
    let _ = preorder_img; // used only by the `#if 0` Officer's Club branch

    let rank_img = lookup_small_rank_image(h, current_side, current_rank);

    // `#if 0` Officer's Club (preorder image) no longer used in Zero Hour
    let index = lb::add_entry_image(wm, h, players, rank_img, -1, 0, w, hh, true, -1);
    lb::add_entry_text(wm, h, players, &u_str, color, index, 1, true);
    index
}

/// `PopulateLobbyPlayerListbox()` (WOLLobbyMenu.cpp:386).
pub fn populate_lobby_player_listbox<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(players_list) = h.wol_lobby_menu().listbox_lobby_players else { return };

    // Display players
    let players = h.gamespy_player_info_map();

    // save off old selection
    let max_selected_items = lb::get_num_entries(wm, players_list);
    let selected_indices = lb::get_selected(wm, h, players_list);
    let mut selected_names: Vec<Vec<u8>> = Vec::new();
    let mut indices_to_select: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let mut num_selected = 0usize;
    for i in 0..max_selected_items {
        let sel = selected_indices.get(i as usize).copied().unwrap_or(-1);
        if sel < 0 {
            break;
        }
        num_selected += 1;
        let u_str = lb::get_text(wm, h, players_list, sel, COLUMN_PLAYERNAME);
        let selected_name: Vec<u8> = u_str.iter().map(|&c| c as u8).collect();
        selected_names.push(selected_name);
    }

    // save off old top entry
    let previous_top_index = lb::get_top_visible_entry(wm, players_list);

    lb::reset(wm, h, players_list);

    // Ops
    for info in &players {
        if info.flags & PEER_FLAG_OP != 0 || h.gamespy_config_is_player_vip(info.profile_id) {
            let color = if info.is_ignored { h.gamespy_color_player_ignored() } else { h.gamespy_color_player_owner() };
            let index = insert_player_in_listbox(wm, h, info, color);

            if selected_names.contains(&info.name) {
                indices_to_select.insert(index);
            }
        }
    }

    // Buddies
    for info in &players {
        let is_buddy = h.gamespy_is_buddy(info.profile_id);
        if !(info.flags & PEER_FLAG_OP != 0 || h.gamespy_config_is_player_vip(info.profile_id)) && is_buddy {
            let color = if info.is_ignored { h.gamespy_color_player_ignored() } else { h.gamespy_color_player_buddy() };
            let index = insert_player_in_listbox(wm, h, info, color);

            if selected_names.contains(&info.name) {
                indices_to_select.insert(index);
            }
        }
    }

    // Everyone else
    for info in &players {
        let is_buddy = h.gamespy_is_buddy(info.profile_id);
        if !(info.flags & PEER_FLAG_OP != 0 || h.gamespy_config_is_player_vip(info.profile_id)) && !is_buddy {
            let color = if info.is_ignored { h.gamespy_color_player_ignored() } else { h.gamespy_color_player_normal() };
            let index = insert_player_in_listbox(wm, h, info, color);

            if selected_names.contains(&info.name) {
                indices_to_select.insert(index);
            }
        }
    }

    // restore selection
    if !indices_to_select.is_empty() {
        let new_indices: Vec<i32> = indices_to_select.iter().copied().collect();
        lb::set_selected(wm, h, players_list, &new_indices);
    }

    if indices_to_select.len() != num_selected {
        wm.win_set_lone_window(h, None);
    }

    // restore top visible entry
    lb::set_top_visible_entry(wm, h, players_list, previous_top_index);
}

/// `WOLLobbyMenuInit(layout, userData)` (WOLLobbyMenu.cpp:578).
pub fn wol_lobby_menu_init<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    {
        let m = h.wol_lobby_menu();
        m.next_screen = None;
        m.button_pushed = false;
        m.is_shutting_down = false;
    }

    set_lobby_attempt_host_join(h, false); // not trying to host or join

    {
        let m = h.wol_lobby_menu();
        m.game_list_refresh_time = 0;
        m.player_list_refresh_time = 0;
    }

    let parent_id = key_of(h, "WOLLobbyMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    let back_id = key_of(h, "ButtonBack");
    let host_id = key_of(h, "ButtonHost");
    let refresh_id = key_of(h, "ButtonRefresh");
    let join_id = key_of(h, "ButtonJoin");
    let buddy_id = key_of(h, "ButtonBuddy");
    let emote_id = key_of(h, "ButtonEmote");
    let chat_id = key_of(h, "TextEntryChat");
    let players_id = key_of(h, "ListboxPlayers");
    let chat_list_id = key_of(h, "ListboxChat");
    let rooms_id = key_of(h, "ComboBoxGroupRooms");

    let button_back = wm.win_get_window_from_id(parent, back_id);
    let button_host = wm.win_get_window_from_id(parent, host_id);
    let button_refresh = wm.win_get_window_from_id(parent, refresh_id);
    let button_join = wm.win_get_window_from_id(parent, join_id);
    if let Some(b) = button_join {
        wm.win_enable(b, false);
    }
    let button_buddy = wm.win_get_window_from_id(parent, buddy_id);
    let button_emote = wm.win_get_window_from_id(parent, emote_id);
    let text_entry_chat = wm.win_get_window_from_id(parent, chat_id);
    let listbox_players = wm.win_get_window_from_id(parent, players_id);
    if let Some(l) = listbox_players {
        h.set_player_tooltip_func(wm, l);
    }
    let listbox_chat = wm.win_get_window_from_id(parent, chat_list_id);
    h.gamespy_register_text_window(listbox_chat);
    let combo_rooms = wm.win_get_window_from_id(parent, rooms_id);

    {
        let m = h.wol_lobby_menu();
        m.parent_wol_lobby_id = parent_id;
        m.parent_wol_lobby = parent;
        m.button_back_id = back_id;
        m.button_back = button_back;
        m.button_host_id = host_id;
        m.button_host = button_host;
        m.button_refresh_id = refresh_id;
        m.button_refresh = button_refresh;
        m.button_join_id = join_id;
        m.button_join = button_join;
        m.button_buddy_id = buddy_id;
        m.button_buddy = button_buddy;
        m.button_emote_id = emote_id;
        m.button_emote = button_emote;
        m.text_entry_chat_id = chat_id;
        m.text_entry_chat = text_entry_chat;
        m.listbox_lobby_players_id = players_id;
        m.listbox_lobby_players = listbox_players;
        m.listbox_lobby_chat_id = chat_list_id;
        m.listbox_lobby_chat = listbox_chat;
        m.combo_lobby_group_rooms_id = rooms_id;
        m.combo_lobby_group_rooms = combo_rooms;
    }

    te::set_text(wm, h, text_entry_chat, &[]);

    populate_group_room_listbox(wm, h, combo_rooms);

    // Show Menu
    wm.layout_hide(layout, false);

    // if we're not in a room, this will join the best available one
    if h.gamespy_current_group_room() == 0 {
        let to_join = h.wol_lobby_menu().group_room_to_join;
        if to_join != 0 {
            h.gamespy_join_group_room(to_join);
            h.wol_lobby_menu().group_room_to_join = 0;
        } else {
            h.gamespy_join_best_group_room();
        }
    }
    // else: not joining group room because we're already in one

    h.grab_window_info(wm);

    h.gamespy_clear_staging_room_list();
    let restrict = h.gamespy_config_restrict_games_to_lobby();
    h.peer_request_start_game_list(restrict);

    // animate controls
    // (TheShell->registerWithAnimateManager is commented out upstream)
    shell::show_shell_map(wm, h, true);
    h.game_spy_game_reset();

    // (the commented-out chat slider is not translated)
    if h.custom_pref_uses_long_game_list() {
        h.toggle_game_list_type(wm);
    }

    // Set Keyboard to chat window
    wm.win_set_focus(h, text_entry_chat);
    {
        let m = h.wol_lobby_menu();
        m.raise_message_boxes = true;
        m.queued_utms.clear();
        m.just_entered = true;
        m.initial_gadget_delay = 2;
    }
    let gadget_parent_id = ShellHost::name_to_key(h, b"WOLCustomLobby.wnd:GadgetParent") as i32;
    if let Some(win) = wm.win_get_window_from_id(None, gadget_parent_id) {
        wm.win_hide(win, true);
    }
    h.wol_lobby_menu().dont_show_main_menu = true;
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_lobby_menu().is_shutting_down = false;

    // hide the layout
    wm.layout_hide(layout, true);

    // our shutdown is complete
    let has_next = h.wol_lobby_menu().next_screen.is_some();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    if let Some(next) = h.wol_lobby_menu().next_screen.clone() {
        shell::push(wm, h, &next, false);
    }

    h.wol_lobby_menu().next_screen = None;
}

/// `WOLLobbyMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn wol_lobby_menu_shutdown<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    // (the commented-out chat slider is not translated)
    if h.get_game_info_list_box().is_some() {
        h.custom_pref_set_uses_long_game_list_and_write(false);
    } else {
        h.custom_pref_set_uses_long_game_list_and_write(true);
    }

    h.release_window_info();

    let chat = h.wol_lobby_menu().listbox_lobby_chat;
    h.gamespy_unregister_text_window(chat);

    //TheGameSpyChat->stopListingGames();
    h.peer_request_stop_game_list();

    {
        let m = h.wol_lobby_menu();
        m.listbox_lobby_chat = None;
        m.listbox_lobby_players = None;

        m.is_shutting_down = true;
    }

    // if we are shutting down for an immediate pop, skip the animations
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    h.wol_lobby_menu().dont_show_main_menu = false;

    h.raise_gs_message_box();
    wt::reverse(wm, h, b"WOLCustomLobbyFade");
}

/// `refreshGameList(forceRefresh)`: the Bool is used to force refresh if the refresh button was hit.
pub fn refresh_game_list<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, force_refresh: bool) {
    let refresh_interval = GAME_LIST_REFRESH_INTERVAL;
    let now = ShellHost::time_ms(h);
    let t = h.wol_lobby_menu().game_list_refresh_time;

    if force_refresh || t == 0 || t.wrapping_add(refresh_interval) <= now {
        if h.gamespy_has_staging_room_list_changed() {
            h.refresh_game_list_boxes(wm);
            let now = ShellHost::time_ms(h);
            h.wol_lobby_menu().game_list_refresh_time = now;
        }
    }
}

/// `refreshPlayerList(forceRefresh)`.
pub fn refresh_player_list<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, force_refresh: bool) {
    let refresh_interval = PLAYER_LIST_REFRESH_INTERVAL;
    let now = ShellHost::time_ms(h);
    let t = h.wol_lobby_menu().player_list_refresh_time;

    if force_refresh || t == 0 || t.wrapping_add(refresh_interval) <= now {
        populate_lobby_player_listbox(wm, h);
        let now = ShellHost::time_ms(h);
        h.wol_lobby_menu().player_list_refresh_time = now;
    }
}

/// `allowedMessages-- && !sawImportantMessage && queue->getResponse(resp)`.
fn next_response<H: WolLobbyMenuHost>(h: &mut H, allowed_messages: &mut i32, saw_important_message: bool) -> Option<LobbyPeerResponse> {
    let go = *allowed_messages != 0;
    *allowed_messages -= 1;
    if !go || saw_important_message {
        return None;
    }
    h.peer_next_response()
}

/// The `PEERRESPONSE_CREATESTAGINGROOM` case body (also reached by falling through from `DISCONNECT`).
fn handle_create_staging_room<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, success: bool) {
    set_lobby_attempt_host_join(h, false);
    if success {
        // Woohoo!  On to our next screen!
        h.wol_lobby_menu().button_pushed = true;
        h.wol_lobby_menu().next_screen = Some(b"Menus/GameSpyGameOptionsMenu.wnd".to_vec());
        shell::pop(wm, h);
        h.gamespy_mark_as_staging_room_host();
        h.gamespy_set_game_options();
    }
}

/// `case PeerResponse::PEERRESPONSE_STAGINGROOM:`.
fn handle_staging_room<H: WolLobbyMenuHost>(h: &mut H, r: LobbyStagingRoomResponse) {
    match r.action {
        StagingAction::Clear => h.gamespy_clear_staging_room_list(),
        StagingAction::Add | StagingAction::Update => {
            if r.percent_complete == 100 {
                h.gamespy_saw_full_game_list();
            }

            let mut server_ok = true;
            if r.staging_room_map_name.is_empty() {
                server_ok = false;
            }
            // fix for ghost game problem - need to iterate over all resp.stagingRoomPlayerNames[i]
            let mut saw_self = false;
            let local = h.gamespy_local_name_ascii();
            if local == r.staging_room_player_names[0] {
                saw_self = true; // don't show ghost games for myself
            }
            if saw_self {
                server_ok = false;
            }

            if server_ok {
                let mut room = LobbyStagingRoom {
                    game_name: r.staging_server_name.iter().map(|&c| u16::from(c)).collect(),
                    id: r.id,
                    has_password: r.requires_password,
                    version: r.version,
                    exe_crc: r.exe_crc,
                    ini_crc: r.ini_crc,
                    allow_observers: r.allow_observers,
                    use_stats: r.use_stats,
                    ping_string: r.staging_server_ping_string.clone(),
                    ladder_ip: r.staging_server_ladder_ip.clone(),
                    ladder_port: r.ladder_port,
                    reported_num_players: r.num_players,
                    reported_max_players: r.max_players,
                    reported_num_observers: r.num_observers,
                    map: Vec::new(),
                    slots: vec![LobbyStagingSlot::default(); MAX_SLOTS],
                };

                let mut map_name: Vec<u8> = Vec::new();
                for &c in &r.staging_room_map_name {
                    if c != b'/' {
                        map_name.push(c);
                    } else {
                        map_name.push(b'\\');
                    }
                }
                room.map = h.portable_map_path_to_real_map_path(&map_name);

                let mut _num_players = 0;
                for i in 0..MAX_SLOTS {
                    let slot = &mut room.slots[i];
                    slot.wins = r.wins[i];
                    slot.losses = r.losses[i];
                    slot.profile_id = r.profile_id[i];
                    slot.player_template = r.faction[i];
                    slot.color = r.color[i];
                    if r.profile_id[i] == slot_state::EASY_AI {
                        slot.state = slot_state::EASY_AI;
                        _num_players += 1;
                    } else if r.profile_id[i] == slot_state::MED_AI {
                        slot.state = slot_state::MED_AI;
                        _num_players += 1;
                    } else if r.profile_id[i] == slot_state::BRUTAL_AI {
                        slot.state = slot_state::BRUTAL_AI;
                        _num_players += 1;
                    } else if !r.staging_room_player_names[i].is_empty() {
                        slot.name = r.staging_room_player_names[i].iter().map(|&c| u16::from(c)).collect(); // nameUStr.translate
                        slot.state = slot_state::PLAYER;
                        _num_players += 1;
                    } else {
                        slot.state = slot_state::OPEN;
                    }
                }
                // DEBUG_ASSERTCRASH(numPlayers, ("Game had no players!\n")) is not translated
                if r.action == StagingAction::Add {
                    h.gamespy_add_staging_room(&room);
                } else {
                    h.gamespy_update_staging_room(&room);
                }
            } else {
                h.gamespy_remove_staging_room(r.id);
            }
        }
        StagingAction::Remove => h.gamespy_remove_staging_room(r.id),
        StagingAction::Other => {}
    }
}

/// `WOLLobbyMenuUpdate(layout, userData)` (WOLLobbyMenu.cpp:899).
pub fn wol_lobby_menu_update<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.wol_lobby_menu().just_entered {
        if h.wol_lobby_menu().initial_gadget_delay == 1 {
            wt::remove(wm, h, b"MainMenuDefaultMenuLogoFade", false);
            wt::set_group(wm, h, b"WOLCustomLobbyFade", false);
            let m = h.wol_lobby_menu();
            m.initial_gadget_delay = 2;
            m.just_entered = false;
        } else {
            h.wol_lobby_menu().initial_gadget_delay -= 1;
        }
    }
    if h.game_logic_in_shell_game_first_frame() {
        h.signal_ui_interaction_entered_from_game();
    }

    // We'll only be successful if we've requested to
    if h.wol_lobby_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }

    if h.wol_lobby_menu().raise_message_boxes {
        h.raise_gs_message_box();
        h.wol_lobby_menu().raise_message_boxes = false;
    }

    if shell::is_anim_finished(wm, h) && wt::is_finished(wm) && !h.wol_lobby_menu().button_pushed && h.has_peer_message_queue() {
        h.handle_buddy_responses();
        h.handle_persistent_storage_responses();

        let mut allowed_messages = h.gamespy_max_messages_per_update();
        let mut saw_important_message = false;
        let mut _should_repopulate_players = false;
        while let Some(resp) = next_response(h, &mut allowed_messages, saw_important_message) {
            match resp {
                LobbyPeerResponse::JoinGroupRoom { ok, id } => {
                    saw_important_message = true;
                    if ok {
                        //buttonPushed = true;
                        h.gamespy_set_current_group_room(id);
                        h.gamespy_clear_player_info_map();
                        if let Some(room) = h.gamespy_group_room_list().into_iter().find(|r| r.group_id == id) {
                            let fmt = h.fetch_text(b"GUI:LobbyJoined");
                            let msg = h.format_text_arg(&fmt, &room.translated_name);
                            let c = h.gamespy_color_default();
                            h.gamespy_add_text(wm, &msg, c, None);
                        }
                    } else {
                        h.gamespy_join_best_group_room();
                    }
                    let rooms = h.wol_lobby_menu().combo_lobby_group_rooms;
                    populate_group_room_listbox(wm, h, rooms);
                    _should_repopulate_players = true;
                }
                LobbyPeerResponse::PlayerUpdate(p) => {
                    h.gamespy_update_player_info(&p);
                    _should_repopulate_players = true;
                }
                LobbyPeerResponse::PlayerJoin { player, is_group_room } => {
                    if is_group_room {
                        h.gamespy_update_player_info(&player);
                        _should_repopulate_players = true;
                    }
                }
                LobbyPeerResponse::Utm(utm) => {
                    // Putting off a UTM in the lobby
                    h.wol_lobby_menu().queued_utms.push(utm);
                }
                LobbyPeerResponse::PlayerLeft { nick } => {
                    h.gamespy_player_left_group_room(&nick);
                    _should_repopulate_players = true;
                }
                LobbyPeerResponse::Message { nick, profile_id, text, is_private, is_action } => {
                    let chat = h.wol_lobby_menu().listbox_lobby_chat;
                    h.gamespy_add_chat(wm, &nick, profile_id, &text, !is_private, is_action, chat);
                }
                LobbyPeerResponse::Disconnect { reason, as_create_staging_room_success } => {
                    saw_important_message = true;
                    let discon_munkee = format!("GUI:GSDisconReason{reason}");
                    let title = h.fetch_text(b"GUI:GSErrorTitle");
                    let body = h.fetch_text(discon_munkee.as_bytes());
                    h.gamespy_close_all_overlays(wm);
                    h.gs_message_box_ok(wm, &title, &body);
                    h.gamespy_reset();
                    shell::pop(wm, h);
                    // no `break` upstream: falls through into PEERRESPONSE_CREATESTAGINGROOM
                    saw_important_message = true;
                    handle_create_staging_room(wm, h, as_create_staging_room_success);
                }
                LobbyPeerResponse::CreateStagingRoom { success } => {
                    saw_important_message = true;
                    handle_create_staging_room(wm, h, success);
                }
                LobbyPeerResponse::JoinStagingRoom { ok, result, staging_room_player_names } => {
                    saw_important_message = true;
                    set_lobby_attempt_host_join(h, false);
                    let mut is_host_present = true;
                    if ok {
                        match h.gamespy_current_staging_room_host_name() {
                            None => is_host_present = false,
                            Some(host_name) => {
                                is_host_present = false;
                                for i in 0..MAX_SLOTS {
                                    let first_player = &staging_room_player_names[i];
                                    if host_name == *first_player {
                                        // Saw host in slot i
                                        is_host_present = true;
                                    }
                                }
                            }
                        }
                    }
                    if ok && is_host_present {
                        // Woohoo!  On to our next screen!
                        h.wol_lobby_menu().button_pushed = true;
                        h.wol_lobby_menu().next_screen = Some(b"Menus/GameSpyGameOptionsMenu.wnd".to_vec());
                        shell::pop(wm, h);
                    } else {
                        let label: &[u8] = match result {
                            JoinResult::FullRoom => b"GUI:JoinFailedRoomFull",
                            JoinResult::InviteOnlyRoom => b"GUI:JoinFailedInviteOnly",
                            JoinResult::BannedFromRoom => b"GUI:JoinFailedBannedFromRoom",
                            JoinResult::BadPassword => b"GUI:JoinFailedBadPassword",
                            JoinResult::AlreadyInRoom => b"GUI:JoinFailedAlreadyInRoom",
                            JoinResult::NoConnection => b"GUI:JoinFailedNoConnection",
                            JoinResult::Other => b"GUI:JoinFailedDefault",
                        };
                        let s = h.fetch_text(label);
                        let title = h.fetch_text(b"GUI:JoinFailedDefault");
                        h.gs_message_box_ok(wm, &title, &s);
                        let to_join = h.wol_lobby_menu().group_room_to_join;
                        if to_join != 0 {
                            h.gamespy_join_group_room(to_join);
                            h.wol_lobby_menu().group_room_to_join = 0;
                        } else {
                            h.gamespy_join_best_group_room();
                        }
                    }
                }
                LobbyPeerResponse::StagingRoomListComplete => h.gamespy_saw_full_game_list(),
                LobbyPeerResponse::StagingRoom(r) => handle_staging_room(h, r),
                LobbyPeerResponse::Other => {}
            }
        }
        // `#if 0 if (shouldRepopulatePlayers) PopulateLobbyPlayerListbox(); #else`
        refresh_player_list(wm, h, false);

        refresh_game_list(wm, h, false);
    }
}

/// `WOLLobbyMenuInput(window, msg, mData1, mData2)`.
pub fn wol_lobby_menu_input<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        if h.wol_lobby_menu().button_pushed {
            return MsgHandled::Ignored;
        }
        if (d1 & 0xFF) as u8 == key::ESC {
            // send a simulated selected event to the parent window of the back/exit button
            if (d2 & 0xFF) & key::STATE_UP != 0 {
                let m = h.wol_lobby_menu().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(m.button_back), m.button_back_id as u32);
            }

            // don't let key fall through anywhere else
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `GBM_SELECTED` on the Join button.
fn lobby_join_selected_game<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.wol_lobby_menu().queued_utms.clear();
    // Look for a game to join
    let cur = h.gamespy_current_group_room();
    h.wol_lobby_menu().group_room_to_join = cur;
    let game_list = h.get_game_list_box();
    let selected = match game_list {
        Some(l) => lb::get_selected(wm, h, l).first().copied().unwrap_or(-1),
        None => -1,
    };
    if selected >= 0 {
        let selected_id = lb::get_item_data(wm, h, game_list.expect("selected implies list"), selected, 0) as i32;
        if selected_id > 0 {
            if let Some(room_to_join) = h.gamespy_staging_room_to_join(selected_id) {
                if room_to_join.exe_crc != h.global_exe_crc() || room_to_join.ini_crc != h.global_ini_crc() {
                    // bad crc.  don't go.
                    // (release build; the _DEBUG / _INTERNAL g_fakeCRC and m_netMinPlayers branches are not translated)
                    let t = h.fetch_text(b"GUI:JoinFailedDefault");
                    let b = h.fetch_text(b"GUI:JoinFailedCRCMismatch");
                    h.gs_message_box_ok(wm, &t, &b);
                    return;
                }
                let unknown_ladder = room_to_join.ladder_port != 0 && !h.ladder_exists(&room_to_join.ladder_ip, room_to_join.ladder_port);
                if unknown_ladder {
                    let t = h.fetch_text(b"GUI:JoinFailedDefault");
                    let b = h.fetch_text(b"GUI:JoinFailedUnknownLadder");
                    h.gs_message_box_ok(wm, &t, &b);
                    return;
                }
                if room_to_join.num_players == MAX_SLOTS as i32 {
                    let t = h.fetch_text(b"GUI:JoinFailedDefault");
                    let b = h.fetch_text(b"GUI:JoinFailedRoomFull");
                    h.gs_message_box_ok(wm, &t, &b);
                    return;
                }
                h.gamespy_mark_as_staging_room_joiner(selected_id);
                h.game_spy_game_set_game_name(&room_to_join.game_name);
                h.game_spy_game_set_ladder_ip(&room_to_join.ladder_ip);
                h.game_spy_game_set_ladder_port(room_to_join.ladder_port);
                set_lobby_attempt_host_join(h, true);
                if room_to_join.has_password {
                    h.gamespy_open_overlay_game_password(wm);
                } else {
                    // no password - just join it
                    h.peer_request_join_staging_room(&room_to_join.game_name, selected_id, &[]);
                }
            }
        } else {
            let t = h.fetch_text(b"GUI:Error");
            let b = h.fetch_text(b"GUI:NoGameInfo");
            h.gs_message_box_ok(wm, &t, &b);
        }
    } else {
        let t = h.fetch_text(b"GUI:Error");
        let b = h.fetch_text(b"GUI:NoGameSelected");
        h.gs_message_box_ok(wm, &t, &b);
    }
}

/// Opens a right-click menu layout at the mouse position (the shared part of both `GLM_RIGHT_CLICKED` branches).
fn open_rc_menu<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, rc_layout: usize) -> Option<WinId> {
    let rc_menu = h.layout_first_window(wm, rc_layout)?;
    h.layout_run_init(wm, rc_layout);
    wm.win_bring_to_top(rc_menu);
    wm.win_hide(rc_menu, false);
    Some(rc_menu)
}

/// `WOLLobbyMenuSystem(window, msg, mData1, mData2)`.
pub fn wol_lobby_menu_system<H: WolLobbyMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            let id = key_of(h, "ButtonGameListToggle");
            h.wol_lobby_menu().button_game_list_type_toggle_id = id;
            // (sliderChatAdjustID is commented out upstream)
        }
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GLM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id == h.get_game_list_box_id() {
                let row_selected = d2 as i32;
                let join = h.wol_lobby_menu().button_join;
                if row_selected >= 0 {
                    if let Some(b) = join {
                        wm.win_enable(b, true);
                    }
                    let now = h.game_client_frame();

                    let id = lb::get_item_data(wm, h, control, row_selected, 0) as i32;

                    let (last_id, last_frame) = {
                        let m = h.wol_lobby_menu();
                        (m.last_id, m.last_frame)
                    };
                    if last_id != id || now > last_frame.wrapping_add(60) {
                        h.peer_request_get_extended_staging_room_info(id);
                    }

                    let m = h.wol_lobby_menu();
                    m.last_id = id;
                    m.last_frame = now;
                } else if let Some(b) = join {
                    wm.win_enable(b, false);
                }
                if let Some(info_list) = h.get_game_info_list_box() {
                    let game_list = h.get_game_list_box();
                    h.refresh_game_info_list_box(wm, game_list, Some(info_list));
                }
            }
        }
        ggm::GBM_SELECTED => {
            if h.wol_lobby_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;

            if h.handle_sort_button(wm, control_id) {
                return MsgHandled::Handled;
            }

            let m = h.wol_lobby_menu().clone();
            // If we back out, just bail - we haven't gotten far enough to need to log out
            if control_id == m.button_back_id {
                if m.trying_to_host_or_join {
                    return MsgHandled::Handled;
                }

                // Leave any group room, then pop off the screen
                h.gamespy_leave_group_room();

                set_lobby_attempt_host_join(h, true); // pretend, since we don't want to queue up another action
                h.wol_lobby_menu().button_pushed = true;
                h.wol_lobby_menu().next_screen = Some(b"Menus/WOLWelcomeMenu.wnd".to_vec());
                shell::pop(wm, h);
            } else if control_id == m.button_refresh_id {
                // Added 2/17/03 added the game refresh button
                refresh_game_list(wm, h, !plant(23469));
                refresh_player_list(wm, h, true);
            } else if control_id == m.button_host_id {
                if m.trying_to_host_or_join {
                    return MsgHandled::Handled;
                }

                set_lobby_attempt_host_join(h, true);
                h.wol_lobby_menu().queued_utms.clear();
                let cur = h.gamespy_current_group_room();
                h.wol_lobby_menu().group_room_to_join = cur;
                h.gamespy_open_overlay_game_options(wm);
            } else if control_id == m.button_join_id {
                if m.trying_to_host_or_join {
                    return MsgHandled::Handled;
                }
                lobby_join_selected_game(wm, h);
            } else if control_id == m.button_buddy_id {
                h.gamespy_toggle_overlay_buddy(wm);
            } else if control_id == m.button_game_list_type_toggle_id {
                h.toggle_game_list_type(wm);
            } else if control_id == m.button_emote_id {
                // read the user's input and clear the entry box
                let txt = te::get_text(wm, h, m.text_entry_chat);
                te::set_text(wm, h, m.text_entry_chat, &[]);
                let txt_input = crate::client::replay_menu::trim(&txt);
                if !txt_input.is_empty() {
                    // Send the message
                    h.gamespy_send_chat(wm, &txt_input, false, m.listbox_lobby_players); // 'emote' button now just sends text
                }
            }
        }
        ggm::GCM_SELECTED => {
            if h.wol_lobby_menu().trying_to_host_or_join {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_lobby_menu().clone();
            if control_id == m.combo_lobby_group_rooms_id {
                let row_selected = cb::get_selected_pos(wm, h, control, -1);

                if row_selected >= 0 {
                    let combo = m.combo_lobby_group_rooms.unwrap_or(control);
                    let group_id = cb::get_item_data(wm, h, combo, row_selected) as i32;
                    if group_id != 0 && group_id != h.gamespy_current_group_room() {
                        h.gamespy_leave_group_room();
                        h.gamespy_join_group_room(group_id);

                        if h.gamespy_config_restrict_games_to_lobby() {
                            h.gamespy_clear_staging_room_list();
                            h.refresh_game_list_boxes(wm);
                            h.peer_request_start_game_list(true);
                        }
                    }
                }
            }
        }
        crate::client::gadget_list_box::glm::DOUBLE_CLICKED => {
            if h.wol_lobby_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id == h.get_game_list_box_id() {
                let row_selected = d2 as i32;

                if row_selected >= 0 {
                    lb::set_selected(wm, h, control, &[row_selected]);
                    let join_id = h.wol_lobby_menu().button_join_id;
                    let button = wm.win_get_window_from_id(Some(window), join_id);

                    wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), join_id as u32);
                }
            }
        }
        crate::client::gadget_list_box::glm::RIGHT_CLICKED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_lobby_menu().clone();
            let (rc_pos, rc_mouse_x, rc_mouse_y) = wm.list_msg.right_click; // RightClickStruct

            if control_id == m.listbox_lobby_players_id {
                if rc_pos < 0 {
                    lb::set_selected(wm, h, control, &[-1]);
                    return MsgHandled::Handled;
                }

                let mut profile_id = 0;
                let u_name = lb::get_text(wm, h, control, rc_pos, COLUMN_PLAYERNAME);
                let a_name: Vec<u8> = u_name.iter().map(|&c| c as u8).collect();
                if let Some(info) = h.gamespy_player_info_by_name(&a_name) {
                    profile_id = info.profile_id;
                }

                let mut is_buddy = false;
                let rc_layout: Option<usize>;
                if profile_id <= 0 {
                    rc_layout = ShellHost::create_layout(h, wm, b"Menus/RCNoProfileMenu.wnd");
                } else if profile_id == h.gamespy_local_profile_id() {
                    rc_layout = ShellHost::create_layout(h, wm, b"Menus/RCLocalPlayerMenu.wnd");
                } else if h.gamespy_is_buddy(profile_id) {
                    rc_layout = ShellHost::create_layout(h, wm, b"Menus/RCBuddiesMenu.wnd");
                    is_buddy = true;
                } else {
                    rc_layout = ShellHost::create_layout(h, wm, b"Menus/RCNonBuddiesMenu.wnd");
                }
                let Some(rc_layout) = rc_layout else { return MsgHandled::Handled };

                lb::set_selected(wm, h, control, &[rc_pos]);

                let Some(rc_menu) = open_rc_menu(wm, h, rc_layout) else { return MsgHandled::Handled };
                h.set_unignore_text(wm, rc_layout, &a_name, profile_id);
                let rc_size = wm.tree.windows[rc_menu].size;
                let mut rc_pos_x = rc_mouse_x;
                let mut rc_pos_y = rc_mouse_y;
                let (dw, dh) = h.display_size();
                if rc_mouse_x + rc_size.0 > dw {
                    rc_pos_x = dw - rc_size.0;
                }
                if rc_mouse_y + rc_size.1 > dh {
                    rc_pos_y = dh - rc_size.1;
                }
                wm.win_set_position(rc_menu, rc_pos_x, rc_pos_y);

                h.win_set_user_data_rc_menu(wm, rc_menu, profile_id, &a_name, is_buddy); // ITEM_BUDDY / ITEM_NONBUDDY
                wm.win_set_lone_window(h, Some(rc_menu));
            } else if control_id == h.get_game_list_box_id() {
                if rc_pos < 0 {
                    lb::set_selected(wm, h, control, &[-1]);
                    return MsgHandled::Handled;
                }

                let selected_id = lb::get_item_data(wm, h, control, rc_pos, 0) as i32;
                if selected_id > 0 {
                    if let Some(the_room) = h.gamespy_staging_room_to_join(selected_id) {
                        if h.ladder_exists(&the_room.ladder_ip, the_room.ladder_port) {
                            let Some(rc_layout) = ShellHost::create_layout(h, wm, b"Menus/RCGameDetailsMenu.wnd") else { return MsgHandled::Handled };

                            lb::set_selected(wm, h, control, &[rc_pos]);

                            let Some(rc_menu) = open_rc_menu(wm, h, rc_layout) else { return MsgHandled::Handled };
                            wm.win_set_position(rc_menu, rc_mouse_x, rc_mouse_y);

                            h.win_set_user_data_int(wm, rc_menu, selected_id);
                            wm.win_set_lone_window(h, Some(rc_menu));
                        }
                    }
                }
            }
        }
        ggm::GEM_EDIT_DONE => {
            if h.wol_lobby_menu().button_pushed {
                return MsgHandled::Handled;
            }

            // read the user's input and clear the entry box
            let chat = h.wol_lobby_menu().text_entry_chat;
            let txt = te::get_text(wm, h, chat);
            te::set_text(wm, h, chat, &[]);
            let txt_input = crate::client::replay_menu::trim(&txt);
            if !txt_input.is_empty() {
                // Send the message
                if !handle_lobby_slash_commands(wm, h, &txt_input) {
                    let players = h.wol_lobby_menu().listbox_lobby_players;
                    h.gamespy_send_chat(wm, &txt_input, false, players);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
