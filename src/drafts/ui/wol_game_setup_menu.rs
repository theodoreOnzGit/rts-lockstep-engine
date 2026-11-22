//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Differential-tested so far (#215, group wol8; tests/differential_draft_wol_game_setup_display.rs, 300 sequences, 0 mismatches, on upstream's real window manager and gadgets, upstream's
//! text cut verbatim): `wol_display_game_options`, `wol_display_slot_list`, `wol_position_start_spots` and the private `handle_player_template_selection` (reached from the first) and
//! `ascii_to_wide`/`set_map_display_from_path`. The first run found one defect (#232), fixed: `ascii_to_wide` zero-extended (`u16::from(c)`) where `UnicodeString::translate` sign-extends.
//! The stand-ins (staging room, GameSpy layer, GUI helpers, map preview, peer queue) are listed in differential/wrap_draft_wol8_setup.cpp. Everything else in this file is NOT covered:
//! Init, Shutdown, Update, Input, System, the other handlers and tooltips, `StartPressed`, `savePlayerInfo`, the slash commands (those of group wol6 excepted).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLGameSetupMenu.cpp
//!     (the file's statics, `SendStatsToOtherPlayers`, `PopBackToLobby`, `WOLPositionStartSpots`, `savePlayerInfo`,
//!      `playerTooltip`, `gameAcceptTooltip`, `pingTooltip`, `handleColorSelection`,
//!      `handlePlayerTemplateSelection`, `handleStartPositionSelection`, `handleTeamSelection`,
//!      `handleStartingCashSelection`, `handleLimitSuperweaponsClick`, `StartPressed`, `WOLDisplayGameOptions`,
//!      `WOLDisplaySlotList`, `InitWOLGameGadgets`, `DeinitWOLGameGadgets`, `WOLGameSetupMenuInit/Shutdown/Update/
//!      Input/System`, `shutdownComplete`, `fillPlayerInfo`, `handleGameSetupSlashCommands`,
//!      `getNextSelectablePlayer`, `getFirstSelectablePlayer`)
//!
//! Not translated: `slotListDebugLog` / `SLOTLIST_DEBUG_LOG` and the other `DEBUG_LOG` lines (debug builds only; the
//! release macro is `DEBUG_LOG`, empty), the debug-only `/slots` and `/discon` slash commands, the commented-out
//! `buttonStartPosition` arrays, the commented-out body of `WOLGameSetupMenuInput`. The GameSpy service is gone:
//! `TheGameSpyInfo`, the current staging room and `TheGameSpyGame` (`GameSpyStagingRoom`, `GameSpyGameSlot`), the
//! peer and persistent-storage queues, `TheNAT`, `TheMapCache`, the preferences files and the GUIUtil helpers
//! (translated in [`crate::client::gui_util`]) are the host's. This file is the window logic, as
//! [`crate::drafts::ui::lan_game_options_menu`] is for the LAN game.
//!
//! Upstream quirks kept and flagged for the differential pass:
//! - `PEERRESPONSE_DISCONNECT` has no `break` and falls through into `PEERRESPONSE_ROOMUTM`, which compares
//!   `resp.command` (empty for a disconnect) with "SL", "HWS", "NAT" and "Pings": none match, so only
//!   `sawImportantMessage = TRUE` (already set) happens.
//! - In `StartPressed`, `*TheGameSpyGame = *myGame` copies the whole staging room (host call).
//! - The `Request` branch reads `options.str()+1` after `nextToken(&key, "=")`, i.e. the text after the `=`.

use crate::client::gadget;
use crate::logic::modules::plant;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_info::{slot_state, MAX_SLOTS, PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM};
use crate::drafts::ui::wol_lobby_menu::{LobbyPlayerInfo, LobbyUtm};

/// `FirewallHelperClass::FIREWALL_MIN`, `FIREWALL_MAX`.
const FIREWALL_MIN: i32 = 0;
const FIREWALL_MAX: i32 = 128;

/// What `GameSpyGameSlot` reports to this screen.
#[derive(Clone, Debug, Default)]
pub struct StagingSlotInfo {
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
    pub has_map: bool,
    pub name: Vec<u16>,
    pub ip: u32,
    /// `getPingAsInt()`.
    pub ping: i32,
    /// `isPlayer(localName)`.
    pub is_local_name: bool,
}

/// The staging room's own fields this screen reads.
#[derive(Clone, Debug, Default)]
pub struct StagingRoomInfo {
    pub map: Vec<u8>,
    pub am_i_host: bool,
    pub is_in_game: bool,
    pub local_slot_num: i32,
    pub map_crc: u32,
    pub map_size: u32,
    pub use_stats: bool,
    pub old_factions_only: bool,
    pub allow_observers: bool,
    pub superweapon_restriction: i32,
    /// `getStartingCash().countMoney()`.
    pub starting_cash: u32,
}

/// `PSPlayerStats` as `playerTooltip` reads it.
#[derive(Clone, Debug, Default)]
pub struct SetupPlayerStats {
    pub id: i32,
    pub locale: i32,
    pub wins: Vec<i32>,
    pub losses: Vec<i32>,
    pub discons: Vec<i32>,
    pub desyncs: Vec<i32>,
    /// `games`: (side, count) in key order.
    pub games: Vec<(i32, i32)>,
    pub games_as_random: i32,
}

/// The `PeerResponse` types `Update` acts on.
#[derive(Clone, Debug)]
pub enum SetupPeerResponse {
    FailedToHost,
    GameStart,
    PlayerChangedFlags(LobbyPlayerInfo),
    PlayerInfo(LobbyPlayerInfo),
    /// `PEERRESPONSE_PLAYERJOIN`: `roomType == StagingRoom`, `resp.player.IP` (network order).
    PlayerJoin { player: LobbyPlayerInfo, is_staging_room: bool, ip: u32 },
    PlayerLeft { nick: Vec<u8> },
    Message { nick: Vec<u8>, profile_id: i32, text: Vec<u8>, is_private: bool, is_action: bool },
    Disconnect { reason: i32 },
    RoomUtm { nick: Vec<u8>, command: Vec<u8>, command_options: Vec<u8> },
    PlayerUtm { nick: Vec<u8>, command: Vec<u8>, command_options: Vec<u8> },
    Other,
}

/// `NATStateType` as `Update` tests it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupNatState {
    Done,
    Failed,
    Other,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolGameSetupMenu {
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<Vec<u8>>,
    pub raise_message_boxes: bool,
    pub launch_game_next: bool,
    pub parent_wol_game_setup_id: i32,
    pub combo_box_player_id: [i32; MAX_SLOTS],
    pub static_text_player_id: [i32; MAX_SLOTS],
    pub button_accept_id: [i32; MAX_SLOTS],
    pub combo_box_color_id: [i32; MAX_SLOTS],
    pub combo_box_player_template_id: [i32; MAX_SLOTS],
    pub combo_box_team_id: [i32; MAX_SLOTS],
    pub button_map_start_position_id: [i32; MAX_SLOTS],
    pub generic_ping_window_id: [i32; MAX_SLOTS],
    pub text_entry_chat_id: i32,
    pub text_entry_map_display_id: i32,
    pub button_back_id: i32,
    pub button_start_id: i32,
    pub button_emote_id: i32,
    pub button_select_map_id: i32,
    pub window_map_id: i32,
    pub window_map_select_map_id: i32,
    pub check_box_use_stats_id: i32,
    pub check_box_limit_superweapons_id: i32,
    pub combo_box_starting_cash_id: i32,
    pub check_box_limit_armies_id: i32,
    pub parent_wol_game_setup: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_start: Option<WinId>,
    pub button_select_map: Option<WinId>,
    pub button_emote: Option<WinId>,
    pub text_entry_chat: Option<WinId>,
    pub text_entry_map_display: Option<WinId>,
    pub window_map: Option<WinId>,
    pub check_box_use_stats: Option<WinId>,
    pub check_box_limit_superweapons: Option<WinId>,
    pub combo_box_starting_cash: Option<WinId>,
    pub check_box_limit_armies: Option<WinId>,
    pub combo_box_player: [Option<WinId>; MAX_SLOTS],
    pub static_text_player: [Option<WinId>; MAX_SLOTS],
    pub button_accept: [Option<WinId>; MAX_SLOTS],
    pub combo_box_color: [Option<WinId>; MAX_SLOTS],
    pub combo_box_player_template: [Option<WinId>; MAX_SLOTS],
    pub combo_box_team: [Option<WinId>; MAX_SLOTS],
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub generic_ping_window: [Option<WinId>; MAX_SLOTS],
    /// `pingImages[3]` (mapped image names).
    pub ping_images: [Option<Vec<u8>>; 3],
    /// `listboxGameSetupChat`, `listboxGameSetupChatID` (not static).
    pub listbox_game_setup_chat: Option<WinId>,
    pub listbox_game_setup_chat_id: i32,
    pub init_done: bool,
    pub last_slotlist_time: u32,
    pub enter_time: u32,
    pub initial_accept_enable: bool,
    /// `static buttonCommunicatorID` of `System`.
    pub button_communicator_id: i32,
}

/// What the game setup screen reaches in the GameSpy layer and the engine.
pub trait WolGameSetupMenuHost: ShellHost {
    fn wol_game_setup_menu(&mut self) -> &mut WolGameSetupMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn format_text_arg(&self, fmt: &[u16], a: &[u16]) -> Vec<u16>;
    fn format_two_text(&self, fmt: &[u16], a: &[u16], b: &[u16]) -> Vec<u16>;
    fn format_one_int(&self, fmt: &[u16], a: i32) -> Vec<u16>;
    /// `format(fmt, locale text, ping, wins, losses, discons, favorite side)`.
    #[allow(clippy::too_many_arguments)]
    fn format_staging_player_info(&self, fmt: &[u16], locale: &[u16], ping: i32, wins: i32, losses: i32, discons: i32, favorite_side: &[u16]) -> Vec<u16>;
    fn gamespy_color_default(&self) -> i32;
    fn gamespy_color_player_owner(&self) -> i32;
    fn gamespy_color_player_normal(&self) -> i32;
    fn time_now_ms(&self) -> u32;

    // TheGameSpyInfo
    fn has_gamespy_info(&self) -> bool;
    fn gamespy_local_name(&self) -> Vec<u16>;
    fn gamespy_local_name_ascii(&self) -> Vec<u8>;
    fn gamespy_local_profile_id(&self) -> i32;
    fn gamespy_ping_string(&self) -> Vec<u8>;
    fn gamespy_max_messages_per_update(&self) -> i32;
    fn gamespy_am_i_host(&self) -> bool;
    fn gamespy_set_current_group_room(&mut self, id: i32);
    fn gamespy_set_game_options(&mut self);
    fn gamespy_leave_staging_room(&mut self);
    fn gamespy_reset(&mut self);
    fn gamespy_is_disconnected_after_game_start(&self) -> Option<i32>;
    fn gamespy_mark_disconnected_after_game_start(&mut self, reason: i32);
    fn gamespy_register_text_window(&mut self, w: Option<WinId>);
    fn gamespy_unregister_text_window(&mut self, w: Option<WinId>);
    fn gamespy_add_text(&mut self, wm: &mut GameWindowManager, text: &[u16], color: i32, win: Option<WinId>);
    fn gamespy_send_chat(&mut self, wm: &mut GameWindowManager, text: &[u16], is_emote: bool, win: Option<WinId>);
    #[allow(clippy::too_many_arguments)]
    fn gamespy_add_chat(&mut self, wm: &mut GameWindowManager, nick: &[u8], profile_id: i32, text: &[u8], is_public: bool, is_action: bool, win: Option<WinId>);
    fn gamespy_update_player_info(&mut self, p: &LobbyPlayerInfo);
    fn gamespy_player_left_group_room(&mut self, nick: &[u8]);
    fn gamespy_player_info_by_name(&self, name: &[u8]) -> Option<LobbyPlayerInfo>;
    fn gamespy_is_buddy(&self, id: i32) -> bool;
    fn gamespy_config_ping_cutoff_good(&self) -> i32;
    fn gamespy_config_ping_cutoff_bad(&self) -> i32;

    // the current staging room and TheGameSpyGame
    fn staging_room_exists(&self) -> bool;
    fn staging_room_info(&self) -> StagingRoomInfo;
    fn staging_slot(&self, i: usize) -> Option<StagingSlotInfo>;
    fn staging_slot_set_color(&mut self, i: usize, v: i32);
    fn staging_slot_set_player_template(&mut self, i: usize, v: i32);
    fn staging_slot_set_start_pos(&mut self, i: usize, v: i32);
    fn staging_slot_set_team_number(&mut self, i: usize, v: i32);
    fn staging_slot_set_state(&mut self, i: usize, state: i32);
    fn staging_slot_set_accept(&mut self, i: usize);
    fn staging_slot_set_map_availability(&mut self, i: usize, v: bool);
    fn staging_slot_set_ip(&mut self, i: usize, v: u32);
    fn staging_slot_set_port(&mut self, i: usize, v: u16);
    fn staging_slot_port(&self, i: usize) -> u16;
    fn staging_slot_set_nat_behavior(&mut self, i: usize, v: i32);
    fn staging_slot_set_ping_string(&mut self, i: usize, s: &[u8]);
    /// `GameSlot newSlot; newSlot.setState(SLOT_PLAYER, name); newSlot.setIP(ip); game->setSlot(i, newSlot)`.
    fn staging_set_new_player_slot(&mut self, i: usize, name: &[u16], ip: u32);
    fn staging_reset_accepted(&mut self);
    fn staging_reset(&mut self);
    fn staging_slot_num(&self, nick: &[u8]) -> i32;
    fn staging_set_map(&mut self, map: &[u8]);
    fn staging_set_map_crc(&mut self, crc: u32);
    fn staging_set_map_size(&mut self, size: u32);
    fn staging_adjust_slots_for_map(&mut self);
    fn staging_set_starting_cash(&mut self, cash: u32);
    fn staging_set_superweapon_restriction(&mut self, v: i32);
    fn staging_set_old_factions_only(&mut self, v: bool);
    /// `TheMultiplayerSettings->getDefaultStartingMoney()` as a count.
    fn multiplayer_default_starting_money(&self) -> u32;
    fn multiplayer_num_colors(&self) -> i32;
    fn player_template_count(&self) -> i32;
    /// `getNthPlayerTemplate(n)`: `isOldFaction()`, `getSide()`; `None` when null.
    fn player_template_is_old_faction(&self, n: i32) -> Option<bool>;
    fn player_template_side(&self, n: i32) -> Option<Vec<u8>>;
    /// `ParseAsciiStringToGameInfo(staging room, options)`.
    fn parse_options_into_staging_room(&mut self, options: &[u8]) -> bool;
    /// `TheGameSpyGame`.
    fn game_spy_game_exists(&self) -> bool;
    fn game_spy_game_is_game_in_progress(&self) -> bool;
    fn game_spy_game_set_game_in_progress(&mut self, v: bool);
    fn game_spy_game_is_in_game_and_am_host(&self) -> bool;
    fn game_spy_game_reset(&mut self);
    fn game_spy_game_launch_game(&mut self);
    fn game_spy_game_get_use_stats(&self) -> bool;
    fn game_spy_game_get_game_name(&self) -> Vec<u16>;
    /// `*TheGameSpyGame = *myGame; TheGameSpyGame->startGame(0)`.
    fn game_spy_game_copy_staging_room_and_start(&mut self);
    /// `savePlayerInfo`'s reads of `TheGameSpyGame`: local slot, its color/template, `amIHost`, map,
    /// `getSuperweaponRestriction`, `getStartingCash`.
    fn game_spy_game_local_slot_num(&self) -> i32;
    fn game_spy_game_slot_color_and_template(&self, i: usize) -> Option<(i32, i32)>;
    fn game_spy_game_am_i_host(&self) -> bool;
    /// `(const char *)GadgetListBoxGetItemData(listboxMap, row)` of the WOL map list: the map's file name.
    fn map_file_name_for_item_data(&self, item: u32) -> Vec<u8>;
    fn game_spy_game_map(&self) -> Vec<u8>;
    fn game_spy_game_superweapon_restriction(&self) -> i32;
    fn game_spy_game_starting_cash(&self) -> u32;

    // preferences
    fn custom_pref_set_preferred_color(&mut self, v: i32);
    fn custom_pref_set_preferred_faction(&mut self, v: i32);
    fn custom_pref_set_preferred_map(&mut self, map: &[u8]);
    fn custom_pref_set_superweapon_restricted(&mut self, v: bool);
    fn custom_pref_set_starting_cash(&mut self, v: u32);
    fn custom_pref_write(&mut self);
    fn custom_pref_preferred_color(&mut self) -> i32;
    fn custom_pref_preferred_faction(&mut self) -> i32;
    fn custom_pref_preferred_map(&mut self) -> Vec<u8>;
    fn custom_pref_starting_cash(&mut self) -> u32;
    fn custom_pref_superweapon_restricted(&mut self) -> bool;
    fn option_prefs_firewall_behavior(&mut self) -> i32;

    // map cache
    fn map_cache_update(&mut self);
    /// `TheMapCache->findMap(map)`: `(m_numPlayers, m_displayName, m_isOfficial)`.
    fn map_cache_find_map(&self, map: &[u8]) -> Option<(i32, Vec<u16>, bool)>;
    /// `TheMapCache->find(lower)`: `(m_CRC, m_filesize)`.
    fn map_cache_find(&self, lower: &[u8]) -> Option<(u32, u32)>;
    fn would_map_transfer(&self, map: &[u8]) -> bool;
    /// `TheGameState->getMapLeafName(map)`.
    fn map_leaf_name(&self, map: &[u8]) -> Vec<u8>;
    /// `TheGlobalData->m_netMinPlayers`.
    fn net_min_players(&self) -> i32;

    // persistent storage
    fn ps_find_player_stats_by_id(&mut self, id: i32) -> SetupPlayerStats;
    /// `stats = parsePlayerKVPairs(data); if (stats.id && findPlayerStatsByID(stats.id).id == 0) trackPlayerStats(stats)`.
    fn ps_parse_and_track_player_stats(&mut self, data: &[u8]);
    fn ps_request_read_player_stats(&mut self, id: i32);
    /// The `STATS/` UTM payload: `"%d %s"` of the local profile id and `formatPlayerKVPairs(subStats)` where `subStats`
    /// copies `id, wins, losses, discons, desyncs, games, locale, gamesAsRandom` of the local stats and is passed to
    /// `GetAdditionalDisconnectsFromUserFile`.
    fn ps_format_local_stats_for_utm(&mut self) -> Vec<u8>;

    // peer queue
    fn has_peer_message_queue(&self) -> bool;
    fn peer_queue_is_connected(&self) -> bool;
    fn peer_next_response(&mut self) -> Option<SetupPeerResponse>;
    /// `PEERREQUEST_UTMPLAYER` with `isStagingRoom = TRUE`, `id`, `nick`, `options`.
    fn peer_request_utm_player(&mut self, id: &[u8], nick: &[u8], options: &[u8]);
    /// `PEERREQUEST_UTMROOM` with `isStagingRoom = TRUE`.
    fn peer_request_utm_room(&mut self, id: &[u8], options: &[u8]);
    fn peer_request_start_game(&mut self);
    fn handle_buddy_responses(&mut self);
    fn handle_persistent_storage_responses(&mut self);
    /// `TheLobbyQueuedUTMs`: `front()` then `pop_front()`.
    fn lobby_queued_utm_pop_front(&mut self) -> Option<LobbyUtm>;

    // TheNAT
    fn nat_exists(&self) -> bool;
    fn nat_delete(&mut self);
    fn nat_update(&mut self) -> SetupNatState;
    fn nat_process_global_message(&mut self, slot: i32, options: &[u8]);

    // overlays, message boxes, other screens
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn raise_gs_message_box(&mut self);
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]);
    /// `TheEstablishConnectionsMenu->endMenu()` when it exists.
    fn establish_connections_menu_end(&mut self);
    /// `WOLMapSelectLayout` (a global of WOLGameSetupMenu.cpp, used by [`crate::drafts::ui::wol_map_select_menu`]).
    fn wol_map_select_layout(&self) -> Option<usize>;
    fn set_wol_map_select_layout(&mut self, l: Option<usize>);
    /// `positionStartSpots(map, buttons, window)` / `positionStartSpots(game, buttons, window)` /
    /// `updateMapStartSpots(game, buttons)`.
    fn position_start_spots_for_map(&mut self, wm: &mut GameWindowManager, map: &[u8], map_window: Option<WinId>);
    fn position_start_spots_for_staging_room(&mut self, wm: &mut GameWindowManager, map_window: Option<WinId>);
    fn update_map_start_spots(&mut self, wm: &mut GameWindowManager);
    // GUIUtil.cpp for the staging room
    fn gui_util_populate_starting_cash(&mut self, wm: &mut GameWindowManager, combo: Option<WinId>);
    fn gui_util_populate_color(&mut self, wm: &mut GameWindowManager, i: usize);
    fn gui_util_populate_player_template(&mut self, wm: &mut GameWindowManager, i: usize, allow_observers: bool);
    fn gui_util_populate_team(&mut self, wm: &mut GameWindowManager, i: usize);
    fn gui_util_update_slot_list(&mut self, wm: &mut GameWindowManager);
    fn slot_list_updates_enabled(&self) -> bool;
    fn enable_slot_list_updates(&mut self, v: bool);
    /// `win->winSetTooltipFunc(name)`; `winSetTooltip(text)`.
    fn win_set_tooltip_func(&mut self, wm: &mut GameWindowManager, w: WinId, func: &[u8]);
    fn win_set_tooltip(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]);
    /// `GadgetComboBoxGetListBox(combo)->winSetTooltipFunc(name)`.
    fn combo_list_box_set_tooltip_func(&mut self, wm: &mut GameWindowManager, combo: WinId, func: &[u8]);
    /// `window->winGetScreenPosition`, `winGetSize`.
    fn win_screen_rect(&self, wm: &GameWindowManager, w: WinId) -> (i32, i32, i32, i32);
    /// `win->winSetEnabledImage(0, image)`.
    fn win_set_enabled_image0(&mut self, wm: &mut GameWindowManager, w: WinId, image: &Option<Vec<u8>>);
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>>;
    /// `TheMouse->setCursorTooltip(text, -1, NULL, width)`.
    fn mouse_set_cursor_tooltip(&mut self, text: &[u16], width: f32);
    // engine
    fn in_game_ui_message(&mut self, label: &[u8]);
    fn qr2_hosting_status(&self) -> i32;
    fn is_thread_hosting(&self) -> i32;
}

fn key_of<H: WolGameSetupMenuHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("GameSpyGameOptionsMenu.wnd:{n}").as_bytes()) as i32
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `UnicodeString::translate(const AsciiString&)` (UnicodeString.cpp:200): each `char` sign-extended to a `WideChar` (#232: this was `u16::from(c)`, which zero-extends: 0xE9 became
/// 0x00E9 where upstream gives 0xFFE9).
fn ascii_to_wide(s: &[u8]) -> Vec<u16> {
    s.iter().map(|&c| if plant(25071) { u16::from(c) } else { c as i8 as i16 as u16 }).collect()
}

/// `atoi`.
pub fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    // C's `isspace`: space, \t, \n, \v, \f, \r (Rust's `is_ascii_whitespace` has no \v)
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r') {
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

/// `AsciiString::trim()`.
fn trim_ascii(s: &[u8]) -> Vec<u8> {
    let start = s.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(start, |e| e + 1);
    s[start..end].to_vec()
}

/// The map's display name and whether it will transfer, as `StartPressed` and the slotlist handler compute them
/// (`TheMapCache->findMap(map)`: the display name and `!m_isOfficial`, else the map's path/leaf name and
/// `WouldMapTransfer`).
fn map_display_and_transfer<H: WolGameSetupMenuHost>(h: &H, map: &[u8], use_leaf_name: bool) -> (Vec<u16>, bool) {
    match h.map_cache_find_map(map) {
        Some((_, display, official)) => (display, !official),
        None => {
            let name = if use_leaf_name { h.map_leaf_name(map) } else { map.to_vec() };
            (ascii_to_wide(&name), h.would_map_transfer(map))
        }
    }
}

/// `SendStatsToOtherPlayers(game)`: `slots` are the game's slots (names) and `local_index` its `getLocalSlotNum()`.
pub fn send_stats_to_other_players<H: WolGameSetupMenuHost>(h: &mut H, slots: &[StagingSlotInfo], local_index: i32) {
    let full_str = h.ps_format_local_stats_for_utm(); // "%d %s": profile id and the formatted sub-stats
    for (i, slot) in slots.iter().enumerate().take(MAX_SLOTS) {
        if slot.is_human && i as i32 != local_index {
            let host_name: Vec<u8> = slot.name.iter().map(|&c| c as u8).collect(); // hostName.translate(slot->getName())
            h.peer_request_utm_player(b"STATS/", &host_name, &full_str);
        }
    }
}

/// `PopBackToLobby()`.
pub fn pop_back_to_lobby<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    // delete TheNAT, its no good for us anymore.
    if h.nat_exists() {
        h.nat_delete();
    }

    if h.has_gamespy_info() {
        // this can be blown away by a disconnect on the map transfer screen
        h.staging_reset();
        h.gamespy_leave_staging_room();
        //TheGameSpyInfo->joinBestGroupRoom();
    }

    if h.wol_game_setup_menu().parent_wol_game_setup.is_some() {
        h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
        shell::pop(wm, h);
    }
}

/// `WOLPositionStartSpots()`.
pub fn wol_position_start_spots<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_game_setup_menu().clone();
    let mut win = m.window_map;
    if h.wol_map_select_layout().is_some() != plant(25080) {
        win = wm.win_get_window_from_id(None, m.window_map_select_map_id);

        // get the controls.
        let list_id = ShellHost::name_to_key(h, b"WOLMapSelectMenu.wnd:ListboxMap") as i32;
        if let Some(listbox_map) = wm.win_get_window_from_id(None, list_id) {
            // get the selected index
            let selected = lb::get_selected(wm, h, listbox_map).first().copied().unwrap_or(-1);

            if selected != -1 {
                // get text of the map to load
                let map = lb::get_text(wm, h, listbox_map, selected, 0);

                // set the map name in the global data map name
                // `const char *mapFname = (const char *)GadgetListBoxGetItemData(listboxMap, selected)`: a pointer to
                // the map's file name, which the host's map list resolves (`row_map_file_name`, as
                // `wol_map_select_menu` does with its `row_map`)
                let item = lb::get_item_data(wm, h, listbox_map, selected, 0);
                let ascii_map: Vec<u8> = if item != 0 {
                    h.map_file_name_for_item_data(item)
                } else {
                    map.iter().map(|&c| c as u8).collect() // asciiMap.translate(map)
                };

                h.position_start_spots_for_map(wm, &ascii_map, win);
            }
        }
    } else {
        // DEBUG_ASSERTCRASH(win != NULL, ("no map preview window"))
        h.position_start_spots_for_staging_room(wm, win);
    }
}

/// `savePlayerInfo()`.
fn save_player_info<H: WolGameSetupMenuHost>(h: &mut H) {
    if h.game_spy_game_exists() {
        let slot_num = h.game_spy_game_local_slot_num();
        if slot_num >= 0 {
            if let Some((color, template)) = h.game_spy_game_slot_color_and_template(slot_num as usize) {
                // save off some prefs
                h.custom_pref_set_preferred_color(color);
                h.custom_pref_set_preferred_faction(template);
                if h.game_spy_game_am_i_host() {
                    let map = h.game_spy_game_map();
                    h.custom_pref_set_preferred_map(&map);
                    let restricted = h.game_spy_game_superweapon_restriction() != 0;
                    h.custom_pref_set_superweapon_restricted(restricted);
                    let cash = h.game_spy_game_starting_cash();
                    h.custom_pref_set_starting_cash(cash);
                }
                h.custom_pref_write();
            }
        }
    }
}

/// `playerTooltip(window, instData, mouse)` (WOLGameSetupMenu.cpp:300): the player boxes' tooltip callback.
pub fn player_tooltip<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, _mouse: u32) {
    let m = h.wol_game_setup_menu().clone();
    let mut slot_idx: i32 = -1;
    for i in 0..MAX_SLOTS {
        if Some(window) == m.combo_box_player[i] || Some(window) == m.static_text_player[i] {
            slot_idx = i as i32;
            break;
        }
    }
    if slot_idx < 0 {
        h.mouse_set_cursor_tooltip(&[], 1.5);
        return;
    }

    if !h.staging_room_exists() {
        h.mouse_set_cursor_tooltip(&[], 1.5);
        return;
    }

    let slot = h.staging_slot(slot_idx as usize);
    let Some(slot) = slot.filter(|s| s.is_human) else {
        h.mouse_set_cursor_tooltip(&[], 1.5);
        return;
    };

    // for tooltip, we want: player name, ping, locale, win/loss history, discons/desyncs as one var, favorite army
    let u_name = slot.name.clone();
    let a_name: Vec<u8> = u_name.iter().map(|&c| c as u8).collect(); // aName.translate(uName)
    let Some(info) = h.gamespy_player_info_by_name(&a_name) else {
        h.mouse_set_cursor_tooltip(&u_name, 1.5);
        return;
    };
    let profile_id = info.profile_id;

    let stats = h.ps_find_player_stats_by_id(profile_id);
    if stats.id == 0 {
        h.mouse_set_cursor_tooltip(&u_name, 1.5);
        return;
    }

    let room = h.staging_room_info();
    let is_local_player = slot_idx == room.local_slot_num;

    let locale_identifier = format!("WOL:Locale{:02}", stats.locale);
    let total_wins: i32 = stats.wins.iter().sum();
    let total_losses: i32 = stats.losses.iter().sum();
    let total_discons: i32 = stats.discons.iter().sum::<i32>() + stats.desyncs.iter().sum::<i32>();
    let mut favorite_side: Vec<u16> = Vec::new();
    let mut num_games = 0;
    let mut favorite = 0;
    for &(side, count) in &stats.games {
        if count >= num_games {
            num_games = count;
            favorite = side;
        }
    }
    if num_games == 0 {
        favorite_side = h.fetch_text(b"GUI:None");
    } else if stats.games_as_random >= num_games {
        favorite_side = h.fetch_text(b"GUI:Random");
    } else if let Some(side) = h.player_template_side(favorite) {
        let s = format!("SIDE:{}", String::from_utf8_lossy(&side));
        favorite_side = h.fetch_text(s.as_bytes());
    }

    let fmt = h.fetch_text(b"TOOLTIP:StagingPlayerInfo");
    let locale_text = h.fetch_text(locale_identifier.as_bytes());
    let player_info = h.format_staging_player_info(&fmt, &locale_text, slot.ping, total_wins, total_losses, total_discons, &favorite_side);

    let mut tooltip: Vec<u16>;
    if is_local_player {
        let fmt = h.fetch_text(b"TOOLTIP:LocalPlayer");
        tooltip = h.format_text_arg(&fmt, &u_name);
    } else {
        // not us
        if h.gamespy_is_buddy(profile_id) {
            // buddy
            let fmt = h.fetch_text(b"TOOLTIP:BuddyPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        } else if profile_id != 0 {
            // non-buddy profiled player
            let fmt = h.fetch_text(b"TOOLTIP:ProfiledPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        } else {
            // non-profiled player
            let fmt = h.fetch_text(b"TOOLTIP:GenericPlayer");
            tooltip = h.format_text_arg(&fmt, &u_name);
        }
    }

    tooltip.extend(player_info);

    h.mouse_set_cursor_tooltip(&tooltip, 1.5); // the text and width are the only params used.
}

/// `gameAcceptTooltip(window, instData, mouse)`.
pub fn game_accept_tooltip<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    let x = (mouse & 0xFFFF) as u16 as i16 as i32; // LOLONGTOSHORT
    let y = (mouse >> 16) as u16 as i16 as i32; // HILONGTOSHORT
    let (win_pos_x, win_pos_y, win_width, win_height) = h.win_screen_rect(wm, window);

    if (x > win_pos_x && x < (win_pos_x + win_width)) && (y > win_pos_y && y < (win_pos_y + win_height)) {
        let t = h.fetch_text(b"TOOLTIP:GameAcceptance");
        h.mouse_set_cursor_tooltip(&t, 1.0);
    }
}

/// `pingTooltip(window, instData, mouse)`.
pub fn ping_tooltip<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    let x = (mouse & 0xFFFF) as u16 as i16 as i32;
    let y = (mouse >> 16) as u16 as i16 as i32;
    let (win_pos_x, win_pos_y, win_width, win_height) = h.win_screen_rect(wm, window);

    if (x > win_pos_x && x < (win_pos_x + win_width)) && (y > win_pos_y && y < (win_pos_y + win_height)) {
        let t = h.fetch_text(b"TOOLTIP:ConnectionSpeed");
        h.mouse_set_cursor_tooltip(&t, 1.0);
    }
}

/// The host's request to the game's host player: `PEERREQUEST_UTMPLAYER` "REQ/" with `options`.
fn request_to_host<H: WolGameSetupMenuHost>(h: &mut H, options: &str) {
    let host_name: Vec<u8> = match h.staging_slot(0) {
        Some(s) => s.name.iter().map(|&c| c as u8).collect(),
        None => Vec::new(),
    };
    h.peer_request_utm_player(b"REQ/", &host_name, options.as_bytes());
}

/// `handleColorSelection(index)`.
fn handle_color_selection<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let Some(combo) = h.wol_game_setup_menu().combo_box_color[index] else { return };
    let sel_index = cb::get_selected_pos(wm, h, combo, 0);
    let color = cb::get_item_data(wm, h, combo, sel_index) as i32;

    if h.staging_room_exists() {
        let Some(slot) = h.staging_slot(index) else { return };
        if color == slot.color {
            return;
        }

        if color >= -1 && color < h.multiplayer_num_colors() {
            let mut color_available = true;
            if color != -1 {
                for i in 0..MAX_SLOTS {
                    if let Some(check_slot) = h.staging_slot(i) {
                        if color == check_slot.color && i != index {
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

        h.staging_slot_set_color(index, color);

        if h.gamespy_am_i_host() {
            // send around a new slotlist
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h);
        } else {
            // request the color from the host
            if !slot.is_local_name {
                return;
            }
            request_to_host(h, &format!("Color={color}"));
        }
    }
}

/// `handlePlayerTemplateSelection(index)`.
fn handle_player_template_selection<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let m = h.wol_game_setup_menu().clone();
    let Some(combo) = m.combo_box_player_template[index] else { return };
    let sel_index = cb::get_selected_pos(wm, h, combo, 0);
    let player_template = cb::get_item_data(wm, h, combo, sel_index) as i32;

    if h.staging_room_exists() {
        let Some(slot) = h.staging_slot(index) else { return };
        if player_template == slot.player_template {
            return;
        }

        let old_template = slot.player_template;
        h.staging_slot_set_player_template(index, player_template);

        if old_template == PLAYERTEMPLATE_OBSERVER {
            // was observer, so populate color & team with all, and enable
            if let Some(c) = m.combo_box_color[index] {
                cb::set_selected_pos(wm, h, c, 0, false);
            }
            if let Some(c) = m.combo_box_team[index] {
                cb::set_selected_pos(wm, h, c, 0, false);
            }
            h.staging_slot_set_start_pos(index, -1);
        } else if player_template == PLAYERTEMPLATE_OBSERVER {
            // is becoming observer, so populate color & team with random only, and disable
            if let Some(c) = m.combo_box_color[index] {
                cb::set_selected_pos(wm, h, c, 0, false);
            }
            if let Some(c) = m.combo_box_team[index] {
                cb::set_selected_pos(wm, h, c, 0, false);
            }
            h.staging_slot_set_start_pos(index, -1);
        }

        if h.gamespy_am_i_host() != plant(25081) {
            // send around a new slotlist
            h.staging_reset_accepted();
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h);
        } else {
            // request the playerTemplate from the host
            request_to_host(h, &format!("PlayerTemplate={player_template}"));
        }
    }
}

/// `handleStartPositionSelection(player, startPos)`.
fn handle_start_position_selection<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, player: i32, start_pos: i32) {
    if h.staging_room_exists() {
        if player < 0 {
            return;
        }
        let Some(slot) = h.staging_slot(player as usize) else { return };

        if start_pos == slot.start_pos {
            return;
        }
        let mut skip = false;
        if start_pos < 0 {
            skip = true;
        }

        if !skip {
            let mut is_available = true;
            for i in 0..MAX_SLOTS {
                if i as i32 != player && h.staging_slot(i).is_some_and(|s| s.start_pos == start_pos) {
                    is_available = false;
                    break;
                }
            }
            if !is_available {
                return;
            }
        }
        h.staging_slot_set_start_pos(player as usize, start_pos);

        if h.staging_room_info().am_i_host {
            // send around a new slotlist
            h.staging_reset_accepted();
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h);
        } else {
            // request the color from the host
            if h.slot_list_updates_enabled() {
                // request the playerTemplate from the host
                request_to_host(h, &format!("StartPos={start_pos}"));
            }
        }
    }
}

/// `handleTeamSelection(index)`.
fn handle_team_selection<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, index: usize) {
    let Some(combo) = h.wol_game_setup_menu().combo_box_team[index] else { return };
    let sel_index = cb::get_selected_pos(wm, h, combo, 0);
    let team = cb::get_item_data(wm, h, combo, sel_index) as i32;

    if h.staging_room_exists() {
        let Some(slot) = h.staging_slot(index) else { return };
        if team == slot.team_number {
            return;
        }

        h.staging_slot_set_team_number(index, team);

        if h.gamespy_am_i_host() {
            // send around a new slotlist
            h.staging_reset_accepted();
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h);
        } else {
            // request the team from the host
            request_to_host(h, &format!("Team={team}"));
        }
    }
}

/// `handleStartingCashSelection()`.
fn handle_starting_cash_selection<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.staging_room_exists() {
        let Some(combo) = h.wol_game_setup_menu().combo_box_starting_cash else { return };
        let sel_index = cb::get_selected_pos(wm, h, combo, 0);

        let cash = cb::get_item_data(wm, h, combo, sel_index);
        h.staging_set_starting_cash(cash);
        h.staging_reset_accepted();

        if h.staging_room_info().am_i_host {
            // send around the new data
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h); // Update the accepted button UI
        }
    }
}

/// `handleLimitSuperweaponsClick()`.
fn handle_limit_superweapons_click<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.staging_room_exists() {
        // At the moment, 1 and 0 are the only choices supported in the GUI, though the system could support more.
        let checked = h.wol_game_setup_menu().check_box_limit_superweapons.is_some_and(|c| gadget::check_box_is_checked(wm, c));
        h.staging_set_superweapon_restriction(if checked { 1 } else { 0 });
        h.staging_reset_accepted();

        if h.staging_room_info().am_i_host {
            // send around a new slotlist
            h.gamespy_set_game_options();
            wol_display_slot_list(wm, h); // Update the accepted button UI
        }
    }
}

/// `StartPressed()`.
fn start_pressed<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut is_ready = true;
    let mut all_have_map = true;
    let mut player_count = 0;
    let mut human_count = 0;
    if !h.staging_room_exists() {
        return;
    }
    let room = h.staging_room_info();
    let chat = h.wol_game_setup_menu().listbox_game_setup_chat;

    // see if everyone's accepted and count the number of players in the game
    let (map_display_name, will_transfer) = map_display_and_transfer(h, &room.map, false);
    for i in 0..MAX_SLOTS {
        let Some(slot) = h.staging_slot(i) else { continue };
        if !slot.is_accepted && slot.is_human {
            is_ready = false;
            if !slot.has_map && !will_transfer {
                let fmt = h.fetch_text(b"GUI:PlayerNoMap");
                let msg = h.format_two_text(&fmt, &slot.name, &map_display_name);
                let c = h.gamespy_color_default();
                h.gamespy_add_text(wm, &msg, c, chat);
                all_have_map = false;
            }
        }
        if slot.is_occupied && slot.player_template != PLAYERTEMPLATE_OBSERVER {
            if slot.is_human {
                human_count += 1;
            }
            player_count += 1;
        }
    }

    // Check for too many players
    let md = h.map_cache_find_map(&room.map);
    if md.as_ref().map_or(true, |(n, _, _)| *n < player_count) {
        if room.am_i_host {
            let fmt = h.fetch_text(b"LAN:TooManyPlayers");
            let text = h.format_one_int(&fmt, md.as_ref().map_or(0, |m| m.0));
            let c = h.gamespy_color_default();
            h.gamespy_add_text(wm, &text, c, chat);
        }
        return;
    }

    // Check for observer + AI players
    if h.net_min_players() != 0 && human_count == 0 {
        if room.am_i_host {
            let text = h.fetch_text(b"GUI:NeedHumanPlayers");
            let c = h.gamespy_color_default();
            h.gamespy_add_text(wm, &text, c, chat);
        }
        return;
    }

    // Check for too few players
    if player_count < h.net_min_players() {
        if room.am_i_host {
            let fmt = h.fetch_text(b"LAN:NeedMorePlayers");
            let text = h.format_one_int(&fmt, player_count);
            let c = h.gamespy_color_default();
            h.gamespy_add_text(wm, &text, c, chat);
        }
        return;
    }

    // Check for too few teams
    let mut num_random = 0;
    let mut teams: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for i in 0..MAX_SLOTS {
        if let Some(slot) = h.staging_slot(i) {
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
        if room.am_i_host {
            let text = h.fetch_text(b"LAN:NeedMoreTeams");
            let c = h.gamespy_color_default();
            h.gamespy_add_text(wm, &text, c, chat);
        }
        return;
    }

    if num_random + (teams.len() as i32) < 2 {
        let text = h.fetch_text(b"GUI:SandboxMode");
        let c = h.gamespy_color_default();
        h.gamespy_add_text(wm, &text, c, chat);
    }

    if is_ready {
        h.peer_request_start_game();

        let slots: Vec<StagingSlotInfo> = (0..MAX_SLOTS).map(|i| h.staging_slot(i).unwrap_or_default()).collect();
        send_stats_to_other_players(h, &slots, room.local_slot_num);

        // we've started, there's no going back, i.e. disable the back button.
        if let Some(b) = h.wol_game_setup_menu().button_back {
            wm.win_enable(b, false);
        }
        let id = ShellHost::name_to_key(h, b"GameSpyGameOptionsMenu.wnd:ButtonCommunicator") as i32;
        if let Some(button_buddy) = wm.win_get_window_from_id(None, id) {
            wm.win_enable(button_buddy, false);
        }
        h.gamespy_close_overlay_buddy(wm);

        h.game_spy_game_copy_staging_room_and_start();
    } else if all_have_map {
        let t = h.fetch_text(b"GUI:NotifiedStartIntent");
        let c = h.gamespy_color_default();
        h.gamespy_add_text(wm, &t, c, chat);
        h.peer_request_utm_room(b"HWS/", b"true");
    }
}

/// `WOLDisplayGameOptions()` (WOLGameSetupMenu.cpp:906): update options on screen.
pub fn wol_display_game_options<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_game_setup_menu().clone();
    if m.parent_wol_game_setup.is_none() || !h.staging_room_exists() {
        return;
    }
    let room = h.staging_room_info();

    let local_slot = if room.local_slot_num >= 0 { h.staging_slot(room.local_slot_num as usize) } else { None };

    let md = h.map_cache_find_map(&room.map);
    if let (Some((_, display, _)), Some(ls)) = (&md, &local_slot) {
        if ls.has_map || plant(25072) {
            gadget::static_text_set_text(wm, h, m.text_entry_map_display, display);
        } else {
            set_map_display_from_path(wm, h, &m, &room.map);
        }
    } else {
        set_map_display_from_path(wm, h, &m, &room.map);
    }
    wol_position_start_spots(wm, h);
    h.update_map_start_spots(wm);

    // If our display does not match the current state of game settings, update the checkbox.
    let is_using_stats = room.use_stats;
    if let Some(c) = m.check_box_use_stats {
        if gadget::check_box_is_checked(wm, c) != is_using_stats {
            gadget::check_box_set_checked(wm, h, c, is_using_stats);
            let t = h.fetch_text(if is_using_stats { b"TOOLTIP:UseStatsOn" } else { b"TOOLTIP:UseStatsOff" });
            h.win_set_tooltip(wm, c, &t);
        }
    }

    let old_factions_only = room.old_factions_only;
    if let Some(c) = m.check_box_limit_armies {
        if gadget::check_box_is_checked(wm, c) != old_factions_only {
            gadget::check_box_set_checked(wm, h, c, old_factions_only);
            // Repopulate the lists of available armies, since the old list is now wrong
            for i in 0..MAX_SLOTS {
                h.gui_util_populate_player_template(wm, i, room.allow_observers);

                // Make sure selections are up to date on all machines
                handle_player_template_selection(wm, h, i);
            }
        }
    }

    // Note: must check if checkbox is already correct to avoid infinite recursion
    let limit_superweapons = if plant(25074) { room.superweapon_restriction > 0 } else { room.superweapon_restriction != 0 };
    if let Some(c) = m.check_box_limit_superweapons {
        if limit_superweapons != gadget::check_box_is_checked(wm, c) {
            gadget::check_box_set_checked(wm, h, c, limit_superweapons);
        }
    }

    if let Some(combo) = m.combo_box_starting_cash {
        let item_count = cb::get_length(wm, combo);
        for index in 0..(if plant(25076) { item_count - 1 } else { item_count }) {
            let value = cb::get_item_data(wm, h, combo, index);
            if value == room.starting_cash {
                // Note: must check if combobox is already correct to avoid infinite recursion
                let selected_index = cb::get_selected_pos(wm, h, combo, 0);
                if index != selected_index {
                    cb::set_selected_pos(wm, h, combo, index, true);
                }

                if !plant(25075) {
                    break;
                }
            }
        }
        // DEBUG_ASSERTCRASH(index < itemCount, ...) is not translated
    }
}

/// The `else` of `WOLDisplayGameOptions`' map name: the map's path after the last backslash.
fn set_map_display_from_path<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, m: &WolGameSetupMenu, map: &[u8]) {
    let mut s: &[u8] = map;
    if let Some(p) = if plant(25073) { map.iter().position(|&c| c == b'\\') } else { map.iter().rposition(|&c| c == b'\\') } {
        s = &map[p + 1..];
    }
    let map_display = ascii_to_wide(s);
    gadget::static_text_set_text(wm, h, m.text_entry_map_display, &map_display);
}

/// `WOLDisplaySlotList()` (WOLGameSetupMenu.cpp:985): the bad munkee slot list displaying function.
pub fn wol_display_slot_list<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.wol_game_setup_menu().parent_wol_game_setup.is_none() || !h.staging_room_exists() {
        return;
    }

    if !h.staging_room_info().is_in_game && !plant(25082) {
        return;
    }

    // DEBUG_ASSERTCRASH(!game->getConstSlot(0)->isOpen(), ("Open host!"))

    h.gui_util_update_slot_list(wm);

    wol_display_game_options(wm, h);

    let m = h.wol_game_setup_menu().clone();
    let local = h.staging_room_info().local_slot_num;
    for i in 0..MAX_SLOTS {
        let slot = h.staging_slot(i);
        if let Some(slot) = slot.filter(|s| s.is_human) {
            let mut ping = slot.ping;
            if i as i32 == local {
                // set up my own ping...
                let p = h.gamespy_ping_string();
                h.staging_slot_set_ping_string(i, &p);
                if !plant(25078) {
                    ping = h.staging_slot(i).map_or(ping, |s| s.ping);
                }
            }

            if let Some(w) = m.generic_ping_window[i] {
                wm.win_hide(w, false);
                if if plant(25077) { ping <= h.gamespy_config_ping_cutoff_good() } else { ping < h.gamespy_config_ping_cutoff_good() } {
                    h.win_set_enabled_image0(wm, w, &m.ping_images[0]);
                } else if ping < h.gamespy_config_ping_cutoff_bad() {
                    h.win_set_enabled_image0(wm, w, &m.ping_images[1]);
                } else {
                    h.win_set_enabled_image0(wm, w, &m.ping_images[2]);
                }
            }
        } else if let Some(w) = m.generic_ping_window[i] {
            wm.win_hide(w, !plant(25079));
        }
    }
}

/// `InitWOLGameGadgets()` (WOLGameSetupMenu.cpp:1056).
pub fn init_wol_game_gadgets<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let the_game_info_exists = h.staging_room_exists();
    let ping_images = [h.find_image_by_name(b"Ping03"), h.find_image_by_name(b"Ping02"), h.find_image_by_name(b"Ping01")];
    // DEBUG_ASSERTCRASH on the images is not translated

    // Initialize the gadget IDs
    let parent_id = key_of(h, "GameSpyGameOptionsMenuParent");
    let back_id = key_of(h, "ButtonBack");
    let start_id = key_of(h, "ButtonStart");
    let chat_id = key_of(h, "TextEntryChat");
    let map_display_id = key_of(h, "TextEntryMapDisplay");
    let chat_list_id = key_of(h, "ListboxChatWindowGameSpyGameSetup");
    let emote_id = key_of(h, "ButtonEmote");
    let select_map_id = key_of(h, "ButtonSelectMap");
    let use_stats_id = key_of(h, "CheckBoxUseStats");
    let window_map_id = key_of(h, "MapWindow");
    let limit_sw_id = key_of(h, "CheckboxLimitSuperweapons");
    let cash_id = key_of(h, "ComboBoxStartingCash");
    let limit_armies_id = key_of(h, "CheckBoxLimitArmies");
    let map_select_map_id = ShellHost::name_to_key(h, b"WOLMapSelectMenu.wnd:WinMapPreview") as i32;
    let title_id = key_of(h, "StaticTextGameName");

    // Initialize the pointers to our gadgets
    let parent = wm.win_get_window_from_id(None, parent_id);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);
    let button_emote = find(wm, emote_id);
    let button_select_map = find(wm, select_map_id);
    let check_box_use_stats = find(wm, use_stats_id);
    let button_start = find(wm, start_id);
    let button_back = find(wm, back_id);
    let listbox_chat = find(wm, chat_list_id);
    let text_entry_chat = find(wm, chat_id);
    let text_entry_map_display = find(wm, map_display_id);
    let window_map = find(wm, window_map_id);
    let check_box_limit_superweapons = find(wm, limit_sw_id);
    let combo_box_starting_cash = find(wm, cash_id);
    let check_box_limit_armies = find(wm, limit_armies_id);

    {
        let m = h.wol_game_setup_menu();
        m.ping_images = ping_images;
        m.parent_wol_game_setup_id = parent_id;
        m.button_back_id = back_id;
        m.button_start_id = start_id;
        m.text_entry_chat_id = chat_id;
        m.text_entry_map_display_id = map_display_id;
        m.listbox_game_setup_chat_id = chat_list_id;
        m.button_emote_id = emote_id;
        m.button_select_map_id = select_map_id;
        m.check_box_use_stats_id = use_stats_id;
        m.window_map_id = window_map_id;
        m.check_box_limit_superweapons_id = limit_sw_id;
        m.combo_box_starting_cash_id = cash_id;
        m.check_box_limit_armies_id = limit_armies_id;
        m.window_map_select_map_id = map_select_map_id;
        m.parent_wol_game_setup = parent;
        m.button_emote = button_emote;
        m.button_select_map = button_select_map;
        m.check_box_use_stats = check_box_use_stats;
        m.button_start = button_start;
        m.button_back = button_back;
        m.listbox_game_setup_chat = listbox_chat;
        m.text_entry_chat = text_entry_chat;
        m.text_entry_map_display = text_entry_map_display;
        m.window_map = window_map;
        m.check_box_limit_superweapons = check_box_limit_superweapons;
        m.combo_box_starting_cash = combo_box_starting_cash;
        m.check_box_limit_armies = check_box_limit_armies;
    }

    h.gui_util_populate_starting_cash(wm, combo_box_starting_cash);

    // Limit Armies can ONLY be set in the Host Game window (PopupHostGame.wnd)
    if let Some(c) = check_box_limit_armies {
        wm.win_enable(c, false);
    }
    // Ditto use stats
    if let Some(c) = check_box_use_stats {
        wm.win_enable(c, false);
    }
    let is_using_stats = h.game_spy_game_get_use_stats();
    if let Some(c) = check_box_use_stats {
        gadget::check_box_set_checked(wm, h, c, is_using_stats);
        let t = h.fetch_text(if is_using_stats { b"TOOLTIP:UseStatsOn" } else { b"TOOLTIP:UseStatsOff" });
        h.win_set_tooltip(wm, c, &t);
    }

    let label_id = key_of(h, "StartingCashLabel");
    if !h.game_spy_game_am_i_host() {
        if let Some(c) = check_box_limit_superweapons {
            wm.win_enable(c, false);
        }
        if let Some(c) = combo_box_starting_cash {
            wm.win_enable(c, false);
        }
        if let Some(l) = wm.win_get_window_from_id(parent, label_id) {
            wm.win_enable(l, false);
        }
    }

    if is_using_stats {
        // Recorded stats games can never limit superweapons, limit armies, or have inflated starting cash.
        // This should probably be enforced at the gamespy level as well, to prevent expoits.
        if let Some(c) = check_box_limit_superweapons {
            wm.win_enable(c, false);
        }
        if let Some(c) = combo_box_starting_cash {
            wm.win_enable(c, false);
        }
        if let Some(c) = check_box_limit_armies {
            wm.win_enable(c, false);
        }
        if let Some(l) = wm.win_get_window_from_id(parent, label_id) {
            wm.win_enable(l, false);
        }
    }

    // Added By Sadullah Nader: Tooltip Function set
    if let Some(w) = window_map {
        h.win_set_tooltip_func(wm, w, b"MapSelectorTooltip");
    }

    if let Some(title) = wm.win_get_window_from_id(parent, title_id) {
        let name = h.game_spy_game_get_game_name();
        gadget::static_text_set_text(wm, h, Some(title), &name);
    }

    if !the_game_info_exists {
        // DEBUG_CRASH(("No staging room!"))
        return;
    }
    let allow_observers = h.staging_room_info().allow_observers;
    let am_i_host = h.gamespy_am_i_host();

    for i in 0..MAX_SLOTS {
        let combo_player_id = key_of(h, &format!("ComboBoxPlayer{i}"));
        let combo_player = wm.win_get_window_from_id(parent, combo_player_id);
        if let Some(c) = combo_player {
            cb::reset(wm, h, c);
            h.win_set_tooltip_func(wm, c, b"playerTooltip");
        }

        let static_player_id = key_of(h, &format!("StaticTextPlayer{i}"));
        let static_player = wm.win_get_window_from_id(parent, static_player_id);
        if let Some(s) = static_player {
            h.win_set_tooltip_func(wm, s, b"playerTooltip");
            if am_i_host {
                wm.win_hide(s, true);
            }
        }

        if let Some(c) = combo_player {
            let normal = h.gamespy_color_player_normal();
            if i == 0 && am_i_host {
                let u_name = h.gamespy_local_name(); // uName.translate(TheGameSpyInfo->getLocalName())
                let owner = h.gamespy_color_player_owner();
                cb::add_entry(wm, h, c, &u_name, owner);
                if let Some(c0) = combo_player {
                    cb::set_selected_pos(wm, h, c0, 0, false);
                }
            } else {
                for label in [&b"GUI:Open"[..], b"GUI:Closed", b"GUI:EasyAI", b"GUI:MediumAI", b"GUI:HardAI"] {
                    let t = h.fetch_text(label);
                    cb::add_entry(wm, h, c, &t, normal);
                }
                cb::set_selected_pos(wm, h, c, 0, false);
            }
        }

        let color_id = key_of(h, &format!("ComboBoxColor{i}"));
        let combo_color = wm.win_get_window_from_id(parent, color_id);
        h.wol_game_setup_menu().combo_box_color[i] = combo_color;
        h.wol_game_setup_menu().combo_box_color_id[i] = color_id;
        h.gui_util_populate_color(wm, i);
        if let Some(c) = combo_color {
            cb::set_selected_pos(wm, h, c, 0, false);
        }

        let template_id = key_of(h, &format!("ComboBoxPlayerTemplate{i}"));
        let combo_template = wm.win_get_window_from_id(parent, template_id);
        h.wol_game_setup_menu().combo_box_player_template[i] = combo_template;
        h.wol_game_setup_menu().combo_box_player_template_id[i] = template_id;
        h.gui_util_populate_player_template(wm, i, allow_observers);

        // add tooltips to the player template combobox and listbox
        if let Some(c) = combo_template {
            h.win_set_tooltip_func(wm, c, b"playerTemplateComboBoxTooltip");
            h.combo_list_box_set_tooltip_func(wm, c, b"playerTemplateListBoxTooltip");
        }

        let team_id = key_of(h, &format!("ComboBoxTeam{i}"));
        let combo_team = wm.win_get_window_from_id(parent, team_id);
        h.wol_game_setup_menu().combo_box_team[i] = combo_team;
        h.wol_game_setup_menu().combo_box_team_id[i] = team_id;
        h.gui_util_populate_team(wm, i);

        let accept_id = key_of(h, &format!("ButtonAccept{i}"));
        let button_accept = wm.win_get_window_from_id(parent, accept_id);
        if let Some(b) = button_accept {
            h.win_set_tooltip_func(wm, b, b"gameAcceptTooltip");
        }

        let ping_id = key_of(h, &format!("GenericPing{i}"));
        let generic_ping = wm.win_get_window_from_id(parent, ping_id);
        if let Some(w) = generic_ping {
            h.win_set_tooltip_func(wm, w, b"pingTooltip");
        }

        let start_pos_id = key_of(h, &format!("ButtonMapStartPosition{i}"));
        let button_map_start_position = wm.win_get_window_from_id(parent, start_pos_id);

        if i != 0 {
            if let Some(b) = button_accept {
                wm.win_hide(b, true);
            }
        }

        let o = h.wol_game_setup_menu();
        o.combo_box_player_id[i] = combo_player_id;
        o.combo_box_player[i] = combo_player;
        o.static_text_player_id[i] = static_player_id;
        o.static_text_player[i] = static_player;
        o.button_accept_id[i] = accept_id;
        o.button_accept[i] = button_accept;
        o.generic_ping_window_id[i] = ping_id;
        o.generic_ping_window[i] = generic_ping;
        o.button_map_start_position_id[i] = start_pos_id;
        o.button_map_start_position[i] = button_map_start_position;
    }

    if let Some(b) = h.wol_game_setup_menu().button_accept[0] {
        wm.win_enable(b, true);
    }

    if let Some(b) = h.wol_game_setup_menu().button_back {
        wm.win_enable(b, true);
    }
    //GadgetButtonSetEnabledColor(buttonAccept[0], GameSpyColor[GSCOLOR_ACCEPT_TRUE]);
}

/// `DeinitWOLGameGadgets()`.
pub fn deinit_wol_game_gadgets<H: WolGameSetupMenuHost>(h: &mut H) {
    let m = h.wol_game_setup_menu();
    m.parent_wol_game_setup = None;
    m.button_emote = None;
    m.button_select_map = None;
    m.button_start = None;
    m.button_back = None;
    m.listbox_game_setup_chat = None;
    m.text_entry_chat = None;
    m.text_entry_map_display = None;
    m.window_map = None;
    m.check_box_use_stats = None;
    m.check_box_limit_superweapons = None;
    m.combo_box_starting_cash = None;

    for i in 0..MAX_SLOTS {
        m.combo_box_player[i] = None;
        m.static_text_player[i] = None;
        m.combo_box_color[i] = None;
        m.combo_box_player_template[i] = None;
        m.combo_box_team[i] = None;
        m.button_accept[i] = None;
        m.button_map_start_position[i] = None;
        m.generic_ping_window[i] = None;
    }
}

/// `WOLGameSetupMenuInit(layout, userData)` (WOLGameSetupMenu.cpp:1337).
pub fn wol_game_setup_menu_init<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.game_spy_game_exists() && h.game_spy_game_is_game_in_progress() {
        h.game_spy_game_set_game_in_progress(false);

        // check if we were disconnected
        if let Some(discon_reason) = h.gamespy_is_disconnected_after_game_start() {
            let discon_munkee = format!("GUI:GSDisconReason{discon_reason}");
            let title = h.fetch_text(b"GUI:GSErrorTitle");
            let body = h.fetch_text(discon_munkee.as_bytes());
            h.gamespy_close_all_overlays(wm);
            h.gs_message_box_ok(wm, &title, &body);
            h.gamespy_reset();
            // game was in progress, and we were disconnected, so pop immediate back to main menu
            shell::pop_immediate(wm, h);
            return;
        }

        // If we init while the game is in progress, we are really returning to the menu
        // after the game.  So, we pop the menu and go back to the lobby.  Whee!
        shell::pop_immediate(wm, h);
        if h.has_peer_message_queue() && h.peer_queue_is_connected() {
            // We're still connected, so pushing back on the lobby
            shell::push(wm, h, b"Menus/WOLCustomLobby.wnd", true);
        }
        return;
    }
    h.gamespy_set_current_group_room(0);

    if h.nat_exists() {
        h.nat_delete();
    }

    {
        let m = h.wol_game_setup_menu();
        m.next_screen = None;
        m.button_pushed = false;
        m.is_shutting_down = false;
        m.launch_game_next = false;
    }

    // initialize the gadgets
    h.enable_slot_list_updates(false);
    init_wol_game_gadgets(wm, h);
    h.enable_slot_list_updates(true);
    let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
    h.gamespy_register_text_window(chat);

    // The dialog needs to react differently depending on whether it's the host or not.
    h.map_cache_update();
    let host_slot = h.staging_slot(0).unwrap_or_default();
    h.staging_slot_set_accept(0);
    let m = h.wol_game_setup_menu().clone();
    if h.gamespy_am_i_host() {
        let color = h.custom_pref_preferred_color();
        h.staging_slot_set_color(0, color);
        let faction = h.custom_pref_preferred_faction();
        h.staging_slot_set_player_template(0, faction);
        let nat = h.option_prefs_firewall_behavior();
        h.staging_slot_set_nat_behavior(0, nat);
        let ping = h.gamespy_ping_string();
        h.staging_slot_set_ping_string(0, &ping);
        let preferred_map = h.custom_pref_preferred_map();
        h.staging_set_map(&preferred_map);

        // Recorded stats games can never limit superweapons, limit armies, or have inflated starting cash.
        // This should probably be enforced at the gamespy level as well, to prevent expoits.
        let is_using_stats = h.game_spy_game_get_use_stats();
        let cash = if is_using_stats { h.multiplayer_default_starting_money() } else { h.custom_pref_starting_cash() };
        h.staging_set_starting_cash(cash);
        let restriction = if is_using_stats { 0 } else if h.custom_pref_superweapon_restricted() { 1 } else { 0 };
        h.staging_set_superweapon_restriction(restriction);
        if is_using_stats {
            h.staging_set_old_factions_only(false);
        }

        //game->setOldFactionsOnly( customPref.getFactionsLimited() );
        if h.staging_room_info().old_factions_only {
            // Make sure host follows the old factions only restrictions!
            let template = h.staging_slot(0).map_or(0, |s| s.player_template);
            if let Some(false) = h.player_template_is_old_faction(template) {
                h.staging_slot_set_player_template(0, PLAYERTEMPLATE_RANDOM);
            }
        }

        for i in 1..MAX_SLOTS {
            h.staging_slot_set_state(i, slot_state::OPEN);
        }

        let lower_map = preferred_map.to_ascii_lowercase();
        if let Some((crc, filesize)) = h.map_cache_find(&lower_map) {
            h.staging_slot_set_map_availability(0, true);
            h.staging_set_map_crc(crc);
            h.staging_set_map_size(filesize);

            h.staging_adjust_slots_for_map(); // BGC- adjust the slots for the new map.
        }

        wol_display_slot_list(wm, h);
        wol_display_game_options(wm, h);
    } else {
        let nat = h.option_prefs_firewall_behavior();
        let faction = h.custom_pref_preferred_faction();
        let color = h.custom_pref_preferred_color();
        let a_name: Vec<u8> = host_slot.name.iter().map(|&c| c as u8).collect(); // aName.translate(uName)
        h.peer_request_utm_player(b"REQ/", &a_name, format!("PlayerTemplate={faction}").as_bytes());
        h.peer_request_utm_player(b"REQ/", &a_name, format!("Color={color}").as_bytes());
        h.peer_request_utm_player(b"REQ/", &a_name, format!("NAT={nat}").as_bytes());
        let ping = h.gamespy_ping_string();
        let mut options = b"Ping=".to_vec();
        options.extend(ping);
        h.peer_request_utm_player(b"REQ/", &a_name, &options);

        let info = h.staging_room_info();
        h.staging_set_map_crc(info.map_crc); // force a recheck
        h.staging_set_map_size(info.map_size); // of if we have the map

        for i in 0..MAX_SLOTS {
            // I'm a client, disable the controls I can't touch.
            for w in [m.combo_box_player[i], m.combo_box_color[i], m.combo_box_player_template[i], m.combo_box_team[i], m.button_map_start_position[i]] {
                if let Some(w) = w {
                    wm.win_enable(w, false);
                }
            }
        }
        if let Some(b) = m.button_start {
            let t = h.fetch_text(b"GUI:Accept");
            wm.win_set_text(b, &t);
            wm.win_enable(b, false);
        }
        if let Some(b) = m.button_select_map {
            wm.win_enable(b, false);
        }
        if let Some(c) = m.check_box_limit_superweapons {
            wm.win_enable(c, false); // Can look but only host can touch
        }
        if let Some(c) = m.combo_box_starting_cash {
            wm.win_enable(c, false); // Ditto
        }
        h.wol_game_setup_menu().initial_accept_enable = false;
    }

    // Show the Menu
    wm.layout_hide(layout, false);

    // Make sure the text fields are clear
    if let Some(l) = m.listbox_game_setup_chat {
        lb::reset(wm, h, l);
    }
    te::set_text(wm, h, m.text_entry_chat, &[]);

    h.wol_game_setup_menu().init_done = true;
    h.gamespy_set_game_options();
    //TheShell->registerWithAnimateManager(parentWOLGameSetup, WIN_ANIMATION_SLIDE_TOP, TRUE);
    wol_position_start_spots(wm, h);

    let now = h.time_now_ms();
    {
        let o = h.wol_game_setup_menu();
        o.last_slotlist_time = 0;
        o.enter_time = now;
    }

    // Set Keyboard to chat entry
    wm.win_set_focus(h, m.text_entry_chat);
    h.wol_game_setup_menu().raise_message_boxes = true;
    wt::set_group(wm, h, b"GameSpyGameOptionsMenuFade", false);
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_game_setup_menu().is_shutting_down = false;

    // hide the layout
    wm.layout_hide(layout, true);

    // our shutdown is complete
    let has_next = h.wol_game_setup_menu().next_screen.is_some();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    if let Some(next) = h.wol_game_setup_menu().next_screen.clone() {
        if !h.has_peer_message_queue() || !h.peer_queue_is_connected() {
            // skipping push because we're disconnected
        } else {
            shell::push(wm, h, &next, false);
        }
    }

    // (the commented-out launchGameNext block is not translated)
    h.wol_game_setup_menu().next_screen = None;
}

/// `WOLGameSetupMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn wol_game_setup_menu_shutdown<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
    h.gamespy_unregister_text_window(chat);

    if let Some(l) = h.wol_map_select_layout() {
        wm.layout_destroy_windows(l);
        wm.layouts[l].deleted = true; // deleteInstance()
        h.set_wol_map_select_layout(None);
    }
    h.wol_game_setup_menu().parent_wol_game_setup = None;
    h.enable_slot_list_updates(false);
    deinit_wol_game_gadgets(h);
    h.establish_connections_menu_end();
    h.wol_game_setup_menu().init_done = false;

    h.wol_game_setup_menu().is_shutting_down = true;

    // if we are shutting down for an immediate pop, skip the animations
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);

    h.raise_gs_message_box();
    wt::reverse(wm, h, b"GameSpyGameOptionsMenuFade");
}

/// `allowedMessages-- && !sawImportantMessage`, the response loop's condition (the response is fetched in the body).
fn more_messages(allowed_messages: &mut i32, saw_important_message: bool) -> bool {
    let go = *allowed_messages != 0;
    *allowed_messages -= 1;
    go && !saw_important_message
}

/// The kick-out shared by the "host left", "kicked" and "can't see ourselves" cases:
/// `buttonPushed = true; reset; leaveStagingRoom; message box; nextScreen = lobby; pop`.
fn leave_for_lobby_with_message<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16]) {
    h.wol_game_setup_menu().button_pushed = true;
    h.staging_reset();
    h.gamespy_leave_staging_room();
    //TheGameSpyInfo->joinBestGroupRoom();
    h.gs_message_box_ok(wm, title, body);
    h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
    shell::pop(wm, h);
}

/// `case PEERRESPONSE_ROOMUTM` / command "SL" (the slot list from the host).
fn handle_slot_list_utm<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, nick: &[u8], command_options: &[u8]) {
    let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
    // slotlist
    let host_slot = h.staging_slot(0);
    let host_named = host_slot.as_ref().is_some_and(|s| {
        let name: Vec<u8> = s.name.iter().map(|&c| c as u8).collect();
        name == nick
    });
    let is_valid_slot_list = h.staging_room_exists() && host_slot.is_some() && host_named && !h.gamespy_am_i_host();
    if !is_valid_slot_list {
        // Not a valid slotlist
        return;
    }
    let room = h.staging_room_info();
    let old_local_slot_num = if room.is_in_game { room.local_slot_num } else { -1 };
    let was_in_game = old_local_slot_num >= 0;
    let old_map_crc = room.map_crc;

    let options = trim_ascii(command_options);
    let mut ports = [0u16; MAX_SLOTS];
    let mut ips = [0u32; MAX_SLOTS];
    for i in 0..MAX_SLOTS {
        if let Some(s) = h.staging_slot(i) {
            ips[i] = s.ip;
            ports[i] = h.staging_slot_port(i);
        } else {
            ips[i] = 0;
            ports[i] = 0;
        }
    }
    let options_ok = h.parse_options_into_staging_room(&options);
    if h.nat_exists() {
        for i in 0..MAX_SLOTS {
            if h.staging_slot(i).is_some() {
                h.staging_slot_set_port(i, ports[i]);
                h.staging_slot_set_ip(i, ips[i]);
            }
        }
    }
    let room = h.staging_room_info();
    let new_local_slot_num = if room.is_in_game { room.local_slot_num } else { -1 };
    let is_in_game = new_local_slot_num >= 0;
    if !options_ok {
        // Options are bad!  bailing!
        return;
    }
    wol_display_slot_list(wm, h);

    // if I changed map availability, send it across
    let new_map_crc = h.staging_room_info().map_crc;
    if is_in_game {
        let now = h.time_now_ms();
        h.wol_game_setup_menu().last_slotlist_time = now;
        if (old_map_crc ^ new_map_crc) != 0 || (!was_in_game && is_in_game) {
            // it changed.  send it
            let host_name: Vec<u8> = h.staging_slot(0).map_or(Vec::new(), |s| s.name.iter().map(|&c| c as u8).collect());
            let local = h.staging_slot(new_local_slot_num as usize).unwrap_or_default();
            h.peer_request_utm_player(b"MAP", &host_name, if local.has_map { b"1" } else { b"0" });
            if !local.has_map {
                let room = h.staging_room_info();
                let (map_display_name, will_transfer) = map_display_and_transfer(h, &room.map, true);
                let fmt = h.fetch_text(if will_transfer { b"GUI:LocalPlayerNoMapWillTransfer" } else { b"GUI:LocalPlayerNoMap" });
                let text = h.format_text_arg(&fmt, &map_display_name);
                let c = h.gamespy_color_default();
                h.gamespy_add_text(wm, &text, c, chat);
            }
        }
        if !h.wol_game_setup_menu().initial_accept_enable {
            if let Some(b) = h.wol_game_setup_menu().button_start {
                wm.win_enable(b, true);
            }
            h.wol_game_setup_menu().initial_accept_enable = true;
        }
    } else if h.wol_game_setup_menu().last_slotlist_time != 0 {
        // can't see ourselves
        let t = h.fetch_text(b"GUI:GSErrorTitle");
        let b = h.fetch_text(b"GUI:GSKicked");
        leave_for_lobby_with_message(wm, h, &t, &b);
    }
}

/// `case PEERRESPONSE_PLAYERUTM` / command "REQ" (a client's request to the host).
fn handle_req_utm<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, slot_num: i32, command_options: &[u8]) {
    let options = trim_ascii(command_options);

    let mut change = false;
    let mut should_unaccept = false;
    // options.nextToken(&key, "="): the key, and the rest starting at the '='
    let start = options.iter().position(|&c| c != b'=').unwrap_or(options.len());
    let end = options[start..].iter().position(|&c| c == b'=').map_or(options.len(), |e| start + e);
    let key = options[start..end].to_vec();
    let rest: &[u8] = if options.len() > end { &options[end..] } else { &[] };
    let value: &[u8] = if rest.is_empty() { &[] } else { &rest[1..] }; // options.str()+1
    let mut val = atoi(value);
    let u_val = atoi(value) as u32;

    let idx = slot_num as usize;
    let Some(slot) = h.staging_slot(idx) else { return };
    let room = h.staging_room_info();

    if key == b"Color" {
        if val >= -1 && val < h.multiplayer_num_colors() && val != slot.color && slot.player_template != PLAYERTEMPLATE_OBSERVER {
            let mut color_available = true;
            if val != -1 {
                for i in 0..MAX_SLOTS {
                    if let Some(check_slot) = h.staging_slot(i) {
                        if val == check_slot.color && i != idx {
                            color_available = false;
                            break;
                        }
                    }
                }
            }
            if color_available {
                h.staging_slot_set_color(idx, val);
            }
            change = true;
        }
        // else: Rejecting invalid color
    } else if key == b"PlayerTemplate" {
        if val >= PLAYERTEMPLATE_MIN && val < h.player_template_count() && val != slot.player_template {
            // Validate for LimitArmies checkbox
            if room.old_factions_only {
                if let Some(false) = h.player_template_is_old_faction(val) {
                    val = PLAYERTEMPLATE_RANDOM;
                }
            }

            h.staging_slot_set_player_template(idx, val);
            if val == PLAYERTEMPLATE_OBSERVER {
                h.staging_slot_set_color(idx, -1);
                h.staging_slot_set_start_pos(idx, -1);
                h.staging_slot_set_team_number(idx, -1);
            }
            change = true;
            should_unaccept = true;
        }
        // else: Rejecting invalid PlayerTemplate
    } else if key == b"StartPos" {
        if val >= -1 && val < MAX_SLOTS as i32 && val != slot.start_pos && slot.player_template != PLAYERTEMPLATE_OBSERVER {
            let mut start_pos_available = true;
            if val != -1 {
                for i in 0..MAX_SLOTS {
                    if let Some(check_slot) = h.staging_slot(i) {
                        if val == check_slot.start_pos && i != idx {
                            start_pos_available = false;
                            break;
                        }
                    }
                }
            }
            if start_pos_available {
                h.staging_slot_set_start_pos(idx, val);
            }
            change = true;
            should_unaccept = true;
        }
        // else: Rejecting invalid startPos
    } else if key == b"Team" {
        if val >= -1 && val < MAX_SLOTS as i32 / 2 && val != slot.team_number && slot.player_template != PLAYERTEMPLATE_OBSERVER {
            h.staging_slot_set_team_number(idx, val);
            change = true;
            should_unaccept = true;
        }
        // else: Rejecting invalid team
    } else if key == b"IP" {
        if u_val != slot.ip {
            h.staging_slot_set_ip(idx, u_val);
            change = true;
            should_unaccept = true;
        }
        // else: Rejecting invalid IP
    } else if key == b"NAT" {
        if val >= FIREWALL_MIN && val <= FIREWALL_MAX {
            h.staging_slot_set_nat_behavior(idx, val);
            change = true;
        }
        // else: Rejecting invalid NAT behavior
    } else if key == b"Ping" {
        h.staging_slot_set_ping_string(idx, value);
        h.gamespy_set_game_options();
    }

    if change {
        if should_unaccept {
            h.staging_reset_accepted();
        }

        h.gamespy_set_game_options();

        wol_display_slot_list(wm, h);
        // DEBUG_LOG of the slot values and GameInfoToAsciiString is not translated
    }
    let _ = wm;
}

/// `case PEERRESPONSE_ROOMUTM` / `case PEERRESPONSE_PLAYERUTM` bodies, split by command.
fn handle_room_utm<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, nick: &[u8], command: &[u8], command_options: &[u8]) {
    let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
    if command == b"SL" {
        handle_slot_list_utm(wm, h, nick, command_options);
    } else if command == b"HWS" {
        // host wants to start
        let room = h.staging_room_info();
        let host_named = h.staging_slot(0).is_some_and(|s| s.name.iter().map(|&c| c as u8).collect::<Vec<u8>>() == nick);
        if h.staging_room_exists() && room.is_in_game && host_named {
            let slot = h.staging_slot(room.local_slot_num.max(0) as usize);
            if slot.is_some_and(|s| !s.is_accepted) {
                let t = h.fetch_text(b"GUI:HostWantsToStart");
                let c = h.gamespy_color_default();
                h.gamespy_add_text(wm, &t, c, chat);
            }
        }
    } else if command.eq_ignore_ascii_case(b"NAT") {
        if h.nat_exists() {
            h.nat_process_global_message(-1, command_options);
        }
    } else if command.eq_ignore_ascii_case(b"Pings") {
        if !h.gamespy_am_i_host() {
            let pings = command_options;
            let mut rest: &[u8] = pings;
            for i in 0..MAX_SLOTS {
                // pings.nextToken(&token, ","): skip leading ',', take up to the next ','
                let start = rest.iter().position(|&c| c != b',').unwrap_or(rest.len());
                let end = rest[start..].iter().position(|&c| c == b',').map_or(rest.len(), |e| start + e);
                if !rest.is_empty() && end > start {
                    let token = trim_ascii(&rest[start..end]);
                    rest = &rest[end..];
                    h.staging_slot_set_ping_string(i, &token);
                } else {
                    h.staging_slot_set_ping_string(i, b"");
                }
            }
        }
    }
}

/// `case PEERRESPONSE_PLAYERUTM` body.
fn handle_player_utm<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, nick: &[u8], command: &[u8], command_options: &[u8]) {
    if command == b"STATS" {
        h.ps_parse_and_track_player_stats(command_options);
        return;
    }
    if h.staging_room_exists() {
        let slot_num = h.staging_slot_num(nick);
        if slot_num >= 0 && (slot_num as usize) < MAX_SLOTS && command.eq_ignore_ascii_case(b"NAT") {
            // this is a command for NAT negotiations, pass if off to TheNAT
            if h.nat_exists() {
                h.nat_process_global_message(slot_num, command_options);
            }
        }
        if slot_num == 0 && !h.gamespy_am_i_host() {
            if command == b"KICK" {
                // oops - we've been kicked.  bail.
                h.wol_game_setup_menu().button_pushed = true;
                h.staging_reset();
                h.gamespy_leave_staging_room();
                //TheGameSpyInfo->joinBestGroupRoom();
                let mut message = h.fetch_text(b"GUI:GSKicked");
                let command_message = trim_ascii(command_options);
                if command_message == b"GameStarted" {
                    message = h.fetch_text(b"GUI:GSKickedGameStarted");
                } else if command_message == b"GameFull" {
                    message = h.fetch_text(b"GUI:GSKickedGameFull");
                }
                let title = h.fetch_text(b"GUI:GSErrorTitle");
                h.gs_message_box_ok(wm, &title, &message);
                h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
                shell::pop(wm, h);
            }
        } else if slot_num > 0 && h.gamespy_am_i_host() {
            let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
            if command == b"accept" {
                h.staging_slot_set_accept(slot_num as usize);
                h.gamespy_set_game_options();
                wol_display_slot_list(wm, h);
            } else if command == b"MAP" {
                let has_map = atoi(command_options) != 0;
                h.staging_slot_set_map_availability(slot_num as usize, has_map);
                if !has_map {
                    // tell the host the user doesn't have the map
                    let room = h.staging_room_info();
                    let (map_display_name, will_transfer) = map_display_and_transfer(h, &room.map, false);
                    let name = h.staging_slot(slot_num as usize).map_or(Vec::new(), |s| s.name);
                    let fmt = h.fetch_text(if will_transfer { b"GUI:PlayerNoMapWillTransfer" } else { b"GUI:PlayerNoMap" });
                    let text = h.format_two_text(&fmt, &name, &map_display_name);
                    let c = h.gamespy_color_default();
                    h.gamespy_add_text(wm, &text, c, chat);
                }
                wol_display_slot_list(wm, h);
            } else if command == b"REQ" {
                handle_req_utm(wm, h, slot_num, command_options);
            }
        }
    }
}

/// `WOLGameSetupMenuUpdate(layout, userData)` (WOLGameSetupMenu.cpp:1520).
pub fn wol_game_setup_menu_update<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    // We'll only be successful if we've requested to
    if h.wol_game_setup_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
        return;
    }

    if h.wol_game_setup_menu().raise_message_boxes {
        h.raise_gs_message_box();
        h.wol_game_setup_menu().raise_message_boxes = false;
    }

    if !(shell::is_anim_finished(wm, h) && !h.wol_game_setup_menu().button_pushed && h.has_peer_message_queue()) {
        return;
    }
    h.handle_buddy_responses();
    h.handle_persistent_storage_responses();

    if h.game_spy_game_exists() && h.game_spy_game_is_game_in_progress() {
        if h.gamespy_is_disconnected_after_game_start().is_some() {
            return; // already been disconnected, so don't worry.
        }

        let mut allowed_messages = h.gamespy_max_messages_per_update();
        let mut saw_important_message = false;
        while more_messages(&mut allowed_messages, saw_important_message) {
            let Some(resp) = h.peer_next_response() else { break };
            if let SetupPeerResponse::Disconnect { reason } = resp {
                saw_important_message = true;
                let discon_munkee = format!("GUI:GSDisconReason{reason}");

                // check for scorescreen
                let id = ShellHost::name_to_key(h, b"ScoreScreen.wnd:ListboxChatWindowScoreScreen") as i32;
                if let Some(score_list) = wm.win_get_window_from_id(None, id) {
                    let t = h.fetch_text(discon_munkee.as_bytes());
                    let c = h.gamespy_color_default();
                    lb::add_entry_text(wm, h, score_list, &t, c, -1, -1, false);
                } else {
                    // still ingame
                    h.in_game_ui_message(discon_munkee.as_bytes());
                }
                h.gamespy_mark_disconnected_after_game_start(reason);
            }
        }

        return; // if we're in game, all we care about is if we've been disconnected from the chat server
    }

    let mut is_hosting = h.gamespy_am_i_host(); // only while in game setup screen
    is_hosting = is_hosting || h.game_spy_game_is_in_game_and_am_host(); // while in game
    let (last_slotlist_time, enter_time) = {
        let m = h.wol_game_setup_menu();
        (m.last_slotlist_time, m.enter_time)
    };
    if !is_hosting && last_slotlist_time == 0 && h.time_now_ms() > enter_time.wrapping_add(10000) {
        // don't do this if we're disconnected
        if h.peer_queue_is_connected() {
            // haven't seen ourselves
            h.wol_game_setup_menu().button_pushed = true;
            if h.game_spy_game_exists() {
                h.game_spy_game_reset();
            }
            h.gamespy_leave_staging_room();
            //TheGameSpyInfo->joinBestGroupRoom();
            let t = h.fetch_text(b"GUI:HostLeftTitle");
            let b = h.fetch_text(b"GUI:HostLeft");
            h.gs_message_box_ok(wm, &t, &b);
            h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
            shell::pop(wm, h);
        }
        return;
    }

    if h.nat_exists() {
        let nat_state = h.nat_update();
        if nat_state == SetupNatState::Done {
            //launchGameNext = TRUE; TheShell->pop();
            h.game_spy_game_launch_game();
            if h.has_gamespy_info() {
                // this can be blown away by a disconnect on the map transfer screen
                h.gamespy_leave_staging_room();
            }
            return;
        } else if nat_state == SetupNatState::Failed {
            // Just back out.  This cleans up some slot list problems
            h.wol_game_setup_menu().button_pushed = true;

            // delete TheNAT, its no good for us anymore.
            h.nat_delete();

            h.staging_reset();
            h.gamespy_leave_staging_room();
            //TheGameSpyInfo->joinBestGroupRoom();
            let t = h.fetch_text(b"GUI:Error");
            let b = h.fetch_text(b"GUI:NATNegotiationFailed");
            h.gs_message_box_ok(wm, &t, &b);
            h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
            shell::pop(wm, h);
            return;
        }
    }

    let mut allowed_messages = h.gamespy_max_messages_per_update();
    let mut saw_important_message = false;
    while more_messages(&mut allowed_messages, saw_important_message) {
        let resp = if let Some(utm) = h.lobby_queued_utm_pop_front() {
            // Got response from queued lobby UTM list
            if utm.is_room_utm {
                SetupPeerResponse::RoomUtm { nick: utm.nick, command: utm.command, command_options: utm.command_options }
            } else {
                SetupPeerResponse::PlayerUtm { nick: utm.nick, command: utm.command, command_options: utm.command_options }
            }
        } else if let Some(r) = h.peer_next_response() {
            r
        } else {
            break;
        };
        match resp {
            SetupPeerResponse::FailedToHost => {
                // oops - we've not heard from the qr server.  bail.
                let t = h.fetch_text(b"GUI:GSFailedToHost");
                let c = h.gamespy_color_default();
                h.gamespy_add_text(wm, &t, c, None);
            }
            SetupPeerResponse::GameStart => {
                saw_important_message = true;
                if !h.staging_room_exists() || !h.staging_room_info().is_in_game {
                    continue;
                }

                if !h.game_spy_game_exists() {
                    continue;
                }

                // SendStatsToOtherPlayers(TheGameSpyGame)
                let local = h.game_spy_game_local_slot_num();
                let slots: Vec<StagingSlotInfo> = (0..MAX_SLOTS).map(|i| h.staging_slot(i).unwrap_or_default()).collect();
                send_stats_to_other_players(h, &slots, local);

                // we've started, there's no going back, i.e. disable the back button.
                if let Some(b) = h.wol_game_setup_menu().button_back {
                    wm.win_enable(b, false);
                }
                let id = ShellHost::name_to_key(h, b"GameSpyGameOptionsMenu.wnd:ButtonCommunicator") as i32;
                if let Some(button_buddy) = wm.win_get_window_from_id(None, id) {
                    wm.win_enable(button_buddy, false);
                }
                h.gamespy_close_overlay_buddy(wm);

                h.game_spy_game_copy_staging_room_and_start();
            }
            SetupPeerResponse::PlayerChangedFlags(p) => {
                h.gamespy_update_player_info(&p);
                wol_display_slot_list(wm, h);
            }
            SetupPeerResponse::PlayerInfo(p) => {
                h.gamespy_update_player_info(&p);
                wol_display_slot_list(wm, h);
                // send out new slotlist if I'm host
                h.gamespy_set_game_options();
            }
            SetupPeerResponse::PlayerJoin { player: p, is_staging_room, ip } => {
                if !is_staging_room {
                    continue;
                }
                saw_important_message = true;
                h.gamespy_update_player_info(&p);

                if p.profile_id != 0 && h.ps_find_player_stats_by_id(p.profile_id).id == 0 {
                    h.ps_request_read_player_stats(p.profile_id);
                }

                // check if we have room for the dude
                if h.gamespy_am_i_host() && h.staging_room_exists() {
                    if h.nat_exists() {
                        // ditch him
                        h.peer_request_utm_player(b"KICK/", &p.name, b"GameStarted");
                    } else {
                        // look for room for him; See if there's room. First get the number of players currently in
                        // the room (the count, and the map's starting spots, are computed but unused upstream).
                        let mut open_slot_index: i32 = -1;
                        for i in 0..MAX_SLOTS {
                            if h.staging_slot(i).is_some_and(|s| s.is_open) {
                                open_slot_index = i as i32;
                                break;
                            }
                        }

                        if open_slot_index >= 0 {
                            // add him
                            let u_name = ascii_to_wide(&p.name);
                            h.staging_set_new_player_slot(open_slot_index as usize, &u_name, u32::from_be(ip)); // ntohl
                            h.staging_reset_accepted(); // BGC - need to unaccept everyone if someone joins the game.
                        } else {
                            // ditch him
                            h.peer_request_utm_player(b"KICK/", &p.name, b"GameFull");
                        }

                        // send out new slotlist if I'm host
                        h.gamespy_set_game_options();
                    }
                }
                wol_display_slot_list(wm, h);
            }
            SetupPeerResponse::PlayerLeft { nick } => {
                saw_important_message = true;
                h.gamespy_player_left_group_room(&nick);

                if h.game_spy_game_exists() && h.game_spy_game_is_game_in_progress() {
                    continue;
                }

                if !h.nat_exists() {
                    // don't update slot list if we're trying to start a game
                    if h.staging_room_exists() && h.gamespy_am_i_host() {
                        let idx = h.staging_slot_num(&nick);
                        if idx >= 0 {
                            h.staging_slot_set_state(idx as usize, slot_state::OPEN);
                            h.staging_reset_accepted(); // BGC - need to unaccept everyone if someone leaves the game.
                        }
                    }

                    // send out new slotlist if I'm host
                    h.gamespy_set_game_options();
                    wol_display_slot_list(wm, h);

                    if h.staging_room_exists() && !h.gamespy_am_i_host() {
                        let idx = h.staging_slot_num(&nick);
                        if idx == 0 {
                            // host left
                            let t = h.fetch_text(b"GUI:HostLeftTitle");
                            let b = h.fetch_text(b"GUI:HostLeft");
                            leave_for_lobby_with_message(wm, h, &t, &b);
                        }
                    }
                }
            }
            SetupPeerResponse::Message { nick, profile_id, text, is_private, is_action } => {
                let chat = h.wol_game_setup_menu().listbox_game_setup_chat;
                h.gamespy_add_chat(wm, &nick, profile_id, &text, !is_private, is_action, chat);
            }
            SetupPeerResponse::Disconnect { reason } => {
                saw_important_message = true;
                let discon_munkee = format!("GUI:GSDisconReason{reason}");
                let title = h.fetch_text(b"GUI:GSErrorTitle");
                let body = h.fetch_text(discon_munkee.as_bytes());
                h.gamespy_close_all_overlays(wm);
                h.gs_message_box_ok(wm, &title, &body);
                h.gamespy_reset();
                shell::pop(wm, h);
                // no `break` upstream: falls through into PEERRESPONSE_ROOMUTM with an empty command (see the file header)
                saw_important_message = true;
            }
            SetupPeerResponse::RoomUtm { nick, command, command_options } => {
                saw_important_message = true;
                handle_room_utm(wm, h, &nick, &command, &command_options);
            }
            SetupPeerResponse::PlayerUtm { nick, command, command_options } => {
                saw_important_message = true;
                handle_player_utm(wm, h, &nick, &command, &command_options);
            }
            SetupPeerResponse::Other => {}
        }
    }
}

/// `WOLGameSetupMenuInput(window, msg, mData1, mData2)`: the body is commented out upstream.
pub fn wol_game_setup_menu_input() -> MsgHandled {
    MsgHandled::Ignored
}

/// `handleGameSetupSlashCommands(uText)`.
pub fn handle_game_setup_slash_commands<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, u_text: &[u16]) -> bool {
    // message.translate(uText): each unit cast to `char` and concatenated with `concat(const char c)`, which adds nothing for a zero byte.
    // ~~`u_text.iter().map(|&c| c as u8).collect()` kept a unit whose low byte is 0 as a NUL~~ **CORRECTED 2026-11-21 (#215, wol6; the differential test
    // tests/differential_draft_wol_slash.rs found 159 of 20,000 cases):** such units are dropped, as upstream's translate does.
    let message: Vec<u8> = u_text.iter().map(|&c| c as u8).filter(|&c| !(c == 0 && !plant(23471))).collect();

    if message.first() != Some(&b'/') {
        return false; // not a slash command
    }

    // remainder.nextToken(&token) with the default separators " \n\r\t"
    let remainder = &message[1..];
    let seps = b" \n\r\t";
    let start = if plant(23477) { 0 } else { remainder.iter().position(|c| !seps.contains(c)).unwrap_or(remainder.len()) };
    let end = remainder[start..].iter().position(|c| seps.contains(c)).map_or(remainder.len(), |e| start + e);
    let token = if plant(23472) { remainder[start..end].to_vec() } else { remainder[start..end].to_ascii_lowercase() };

    if token == b"host" {
        let s = if plant(23476) { utf16(&format!("Hosting qr2:{} thread:{}", h.is_thread_hosting(), h.qr2_hosting_status())) } else { utf16(&format!("Hosting qr2:{} thread:{}", h.qr2_hosting_status(), h.is_thread_hosting())) };
        let c = h.gamespy_color_default();
        h.gamespy_add_text(wm, &s, c, None);
        true // was a slash command
    } else if token == b"me" && u_text.len() > if plant(23473) { 3 } else { 4 } {
        let from = if plant(23474) { 3 } else { 4 };
        h.gamespy_send_chat(wm, &u_text[from..], !plant(23475), None);
        true // was a slash command
    } else {
        // (the debug-only /slots and /discon are not translated)
        false // not a slash command
    }
}

/// `getNextSelectablePlayer(start)`.
fn get_next_selectable_player<H: WolGameSetupMenuHost>(h: &H, start: i32) -> i32 {
    let room = h.staging_room_info();
    if !room.am_i_host {
        return -1;
    }
    let mut j = start;
    while j < MAX_SLOTS as i32 {
        if let Some(slot) = h.staging_slot(j as usize) {
            if slot.start_pos == -1 && ((j == room.local_slot_num && slot.player_template != PLAYERTEMPLATE_OBSERVER) || slot.is_ai) {
                return j;
            }
        }
        j += 1;
    }
    -1
}

/// `getFirstSelectablePlayer(game)`.
fn get_first_selectable_player<H: WolGameSetupMenuHost>(h: &H) -> i32 {
    let room = h.staging_room_info();
    let slot = h.staging_slot(room.local_slot_num.max(0) as usize);
    // `!game->amIHost() || slot && slot->getPlayerTemplate() != PLAYERTEMPLATE_OBSERVER`
    if !room.am_i_host || slot.as_ref().is_some_and(|s| s.player_template != PLAYERTEMPLATE_OBSERVER) {
        return room.local_slot_num;
    }

    for i in 0..MAX_SLOTS {
        if h.staging_slot(i).is_some_and(|s| s.is_ai) {
            return i as i32;
        }
    }

    room.local_slot_num
}

/// The start-position button branch of `GBM_SELECTED` for button `i`.
fn start_position_button_pressed<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, i: i32) {
    let room = h.staging_room_info();
    let mut player_idx_in_pos: i32 = -1;
    for j in 0..MAX_SLOTS {
        if h.staging_slot(j).is_some_and(|s| s.start_pos == i) {
            player_idx_in_pos = j as i32;
            break;
        }
    }
    if player_idx_in_pos >= 0 {
        let slot = h.staging_slot(player_idx_in_pos as usize);
        if player_idx_in_pos == room.local_slot_num || (room.am_i_host && slot.is_some_and(|s| s.is_ai)) {
            // it's one of my type.  Try to change it.
            let next_player = get_next_selectable_player(h, player_idx_in_pos + 1);
            handle_start_position_selection(wm, h, player_idx_in_pos, -1);
            if next_player >= 0 {
                handle_start_position_selection(wm, h, next_player, i);
            }
        }
    } else {
        // nobody in the slot - put us in
        let mut next_player = get_next_selectable_player(h, 0);
        if next_player < 0 {
            next_player = get_first_selectable_player(h);
        }
        handle_start_position_selection(wm, h, next_player, i);
    }
}

/// `WOLGameSetupMenuSystem(window, msg, mData1, mData2)` (WOLGameSetupMenu.cpp:2555).
pub fn wol_game_setup_menu_system<H: WolGameSetupMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            let id = key_of(h, "ButtonCommunicator");
            h.wol_game_setup_menu().button_communicator_id = id;
        }
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GCM_SELECTED => {
            let m = h.wol_game_setup_menu().clone();
            if !m.init_done || m.button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id == m.combo_box_starting_cash_id {
                handle_starting_cash_selection(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if control_id == m.combo_box_color_id[i] {
                        handle_color_selection(wm, h, i);
                    } else if control_id == m.combo_box_player_template_id[i] {
                        handle_player_template_selection(wm, h, i);
                    } else if control_id == m.combo_box_team_id[i] {
                        handle_team_selection(wm, h, i);
                    } else if control_id == m.combo_box_player_id[i] && h.gamespy_am_i_host() {
                        // We don't have anything that'll happen if we click on ourselves
                        if i as i32 == h.staging_room_info().local_slot_num {
                            break;
                        }
                        // Get
                        let pos = match m.combo_box_player[i] {
                            Some(c) => cb::get_selected_pos(wm, h, c, -1),
                            None => -1,
                        };
                        if pos != slot_state::PLAYER && pos >= 0 {
                            let slot = h.staging_slot(i).unwrap_or_default();
                            if slot.state == slot_state::PLAYER {
                                let a_name: Vec<u8> = slot.name.iter().map(|&c| c as u8).collect();
                                h.peer_request_utm_player(b"KICK/", &a_name, b"true");

                                h.staging_slot_set_state(i, pos);
                                h.staging_reset_accepted();
                                h.gamespy_set_game_options();
                                wol_display_slot_list(wm, h);
                                //TheLAN->OnPlayerLeave(name);
                            } else if slot.state != pos {
                                let was_ai = slot.is_ai;
                                h.staging_slot_set_state(i, pos);
                                let is_ai = h.staging_slot(i).is_some_and(|s| s.is_ai);
                                h.staging_reset_accepted();
                                if was_ai ^ is_ai {
                                    let allow = was_ai && h.staging_room_info().allow_observers;
                                    h.gui_util_populate_player_template(wm, i, allow);
                                }
                                h.gamespy_set_game_options();
                                wol_display_slot_list(wm, h);
                            }
                        }
                        break;
                    }
                }
            }
        }
        ggm::GBM_SELECTED => {
            if h.wol_game_setup_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_game_setup_menu().clone();
            let button_communicator_id = key_of(h, "ButtonCommunicator");
            if control_id == m.button_back_id {
                save_player_info(h);
                if let Some(l) = h.wol_map_select_layout() {
                    wm.layout_destroy_windows(l);
                    wm.layouts[l].deleted = true; // deleteInstance()
                    h.set_wol_map_select_layout(None);
                }

                h.staging_reset();
                //peerLeaveRoom(TheGameSpyChat->getPeer(), StagingRoom, NULL);
                h.gamespy_leave_staging_room();
                h.wol_game_setup_menu().button_pushed = true;
                h.wol_game_setup_menu().next_screen = Some(b"Menus/WOLCustomLobby.wnd".to_vec());
                shell::pop(wm, h);
            } else if control_id == button_communicator_id {
                h.gamespy_toggle_overlay_buddy(wm);
            } else if control_id == m.button_emote_id {
                // read the user's input, clear the text entry line, clean up the text
                let txt = te::get_text(wm, h, m.text_entry_chat);
                te::set_text(wm, h, m.text_entry_chat, &[]);
                let txt_input = crate::client::replay_menu::trim(&txt);
                // Echo the user's input to the chat window
                if !txt_input.is_empty() {
                    h.gamespy_send_chat(wm, &txt_input, false, None); // 'emote' button is now carriage-return
                }
            } else if control_id == m.button_select_map_id {
                if let Some(l) = ShellHost::create_layout(h, wm, b"Menus/WOLMapSelectMenu.wnd") {
                    h.set_wol_map_select_layout(Some(l));
                    wm.layout_run(h, l, 0); // runInit()
                    wm.layout_hide(l, false);
                    wm.layout_bring_forward(l);
                }
            } else if control_id == m.button_start_id {
                save_player_info(h);
                if h.gamespy_am_i_host() {
                    start_pressed(wm, h);
                } else {
                    // I'm the Client... send an accept message to the host.
                    let local = h.staging_room_info().local_slot_num;
                    if local >= 0 && h.staging_slot(local as usize).is_some() {
                        h.staging_slot_set_accept(local as usize);
                    }
                    let ascii_name: Vec<u8> = h.staging_slot(0).map_or(Vec::new(), |s| s.name.iter().map(|&c| c as u8).collect());
                    h.peer_request_utm_player(b"accept", &ascii_name, b"true");
                    //peerSetReady( PEER, PEERTrue );
                    wol_display_slot_list(wm, h);
                }
            } else if control_id == m.check_box_limit_superweapons_id {
                handle_limit_superweapons_click(wm, h);
            } else {
                for i in 0..MAX_SLOTS {
                    if control_id == m.button_map_start_position_id[i] {
                        start_position_button_pressed(wm, h, i as i32);
                    }
                }
            }
        }
        ggm::GBM_SELECTED_RIGHT => {
            if h.wol_game_setup_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_game_setup_menu().clone();
            for i in 0..MAX_SLOTS {
                if control_id == m.button_map_start_position_id[i] {
                    let room = h.staging_room_info();
                    let mut player_idx_in_pos: i32 = -1;
                    for j in 0..MAX_SLOTS {
                        if h.staging_slot(j).is_some_and(|s| s.start_pos == i as i32) {
                            player_idx_in_pos = j as i32;
                            break;
                        }
                    }
                    if player_idx_in_pos >= 0 {
                        let slot = h.staging_slot(player_idx_in_pos as usize);
                        if player_idx_in_pos == room.local_slot_num || (room.am_i_host && slot.is_some_and(|s| s.is_ai)) {
                            // it's one of my type.  Remove it.
                            handle_start_position_selection(wm, h, player_idx_in_pos, -1);
                        }
                    }
                }
            }
        }
        ggm::GEM_EDIT_DONE => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_game_setup_menu().clone();
            // Take the user's input and echo it into the chat window as well as send it to the other clients
            if control_id == m.text_entry_chat_id {
                // read the user's input, clear the text entry line, clean up the text
                let txt = te::get_text(wm, h, m.text_entry_chat);
                te::set_text(wm, h, m.text_entry_chat, &[]);
                let txt_input = crate::client::replay_menu::trim(&txt);
                // Echo the user's input to the chat window
                if !txt_input.is_empty() && !handle_game_setup_slash_commands(wm, h, &txt_input) {
                    h.gamespy_send_chat(wm, &txt_input, false, None);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
