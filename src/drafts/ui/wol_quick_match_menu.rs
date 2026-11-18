//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLQuickMatchMenu.cpp
//!     (the file's statics, `isInfoShown`, `hideInfoGadgets`, `hideOptionsGadgets`, `enableOptionsGadgets`,
//!      `UpdateStartButton`, `populateQMColorComboBox`, `populateQMSideComboBox`, `HandleQMLadderSelection`,
//!      `isValidLadder`, `PopulateQMLadderListBox`, `getLadderInfo`, `PopulateQMLadderComboBox`,
//!      `populateQuickMatchMapSelectListbox`, `saveQuickMatchOptions`, `WOLQuickMatchMenuInit/Shutdown/Update/Input/
//!      System`, `shutdownComplete`)
//!
//! Not translated: the `PERF_TEST` / `DEBUG_LOGGING` code (`s_perfLog`, `s_inQM`, `getMessageString`, the timing
//! block in `Update`), which is compiled out of a release build; the `DEBUG_LOG` lines; the `DEBUG_ASSERTCRASH` on
//! `numPlayersPerTeam`; the commented-out text entries. The GameSpy service is gone: `TheGameSpyInfo`,
//! `TheGameSpyGame`, the peer and persistent-storage queues, `TheNAT`, `TheLadderList`, the preferences files, the
//! map cache and the player-template store are the host's.
//!
//! Pointers: `const LadderInfo *` is modelled by the ladder's `index` (unique; `usedLadders` is compared by
//! identity upstream), and the `const MapMetaData *` stored in the map list's column 1 item data by `1 +` an index
//! into [`WolQuickMatchMenu::map_metas`] (0 is `NULL`). `GadgetComboBoxGetSelectedPos(NULL, ..)` for the disabled
//! ladder combo (`comboBoxLadder` is `NULL` when there are no ladders) leaves upstream's `selected` uninitialised;
//! [`get_ladder_info`] answers "no ladder" there instead (flagged for the differential pass).

use crate::client::gadget;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_info::{MAX_SLOTS, PLAYERTEMPLATE_RANDOM};

/// `MAX_DISCONNECTS[]`, `MAX_DISCONNECTS_COUNT`.
const MAX_DISCONNECTS: [i32; 5] = [0, 5, 10, 25, 50];
const MAX_DISCONNECTS_COUNT: usize = 5;

/// `LadderInfo` as this file reads it (LadderDefs.h).
#[derive(Clone, Debug, Default)]
pub struct QmLadderInfo {
    pub index: i32,
    pub name: Vec<u16>,
    pub players_per_team: i32,
    pub address: Vec<u8>,
    pub port: u16,
    pub valid_qm: bool,
    pub min_wins: i32,
    pub max_wins: i32,
    pub random_factions: bool,
    pub random_maps: bool,
    pub valid_factions: Vec<Vec<u8>>,
    pub valid_maps: Vec<Vec<u8>>,
}

/// `MapMetaData` as this file reads it: `m_fileName`, `m_displayName`, `m_numPlayers`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QmMapMeta {
    pub file_name: Vec<u8>,
    pub display_name: Vec<u16>,
    pub num_players: i32,
}

/// A `PlayerTemplate` as `populateQMSideComboBox` reads it.
#[derive(Clone, Debug, Default)]
pub struct QmPlayerTemplate {
    pub name: Vec<u8>,
    pub side: Vec<u8>,
    /// `getStartingBuilding().isEmpty()`.
    pub starting_building_empty: bool,
    /// `TheChallengeGenerals->getGeneralByTemplateName(name)`: `None` when there is no such general, else
    /// `isStartingEnabled()`.
    pub general_starting_enabled: Option<bool>,
}

/// The `PSPlayerStats` fields this file sums (`wins`, `discons`, `desyncs` are per-general maps; only the values
/// are summed).
#[derive(Clone, Debug, Default)]
pub struct QmPlayerStats {
    pub id: i32,
    pub wins: Vec<i32>,
    pub discons: Vec<i32>,
    pub desyncs: Vec<i32>,
}

/// `LadderPref` (LadderPreferences.h) as `Start` writes it.
#[derive(Clone, Debug, Default)]
pub struct QmLadderPref {
    pub last_play_date: i64,
    pub address: Vec<u8>,
    pub port: u16,
    pub name: Vec<u16>,
}

/// The fields of the `PEERREQUEST_STARTQUICKMATCH` `PeerRequest` that `Start` fills.
#[derive(Clone, Debug, Default)]
pub struct StartQuickMatchRequest {
    /// `req.qmMaps`: the map list's column 0 item data per row.
    pub qm_maps: Vec<u32>,
    pub max_point_percentage: i32,
    pub min_point_percentage: i32,
    pub widen_time: i32,
    pub max_discons: i32,
    pub max_ping: i32,
    pub points: i32,
    pub ladder_id: i32,
    pub ladder_pass_crc: u32,
    pub side: i32,
    pub color: i32,
    pub nat: i32,
    pub num_players: i32,
    pub discons: i32,
    /// `pings[17]`, at most 16 characters.
    pub pings: Vec<u8>,
    pub bot_id: i32,
    pub room_id: i32,
    pub exe_crc: u32,
    pub ini_crc: u32,
}

/// `QM_*` of `resp.qmStatus.status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QmState {
    Idle,
    JoiningQmChannel,
    LookingForBot,
    SentInfo,
    Working,
    PoolSize,
    WideningSearch,
    Matched,
    InChannel,
    NegotiatingFirewalls,
    StartingGame,
    CouldNotFindBot,
    CouldNotFindChannel,
    CouldNotNegotiateFirewalls,
    Stopped,
}

/// `resp.qmStatus` and `resp.stagingRoomPlayerNames` of a `PEERRESPONSE_QUICKMATCHSTATUS`.
#[derive(Clone, Debug)]
pub struct QmStatusResponse {
    pub status: QmState,
    pub pool_size: i32,
    pub seed: i32,
    pub map_idx: i32,
    pub ip: [u32; MAX_SLOTS],
    pub color: [i32; MAX_SLOTS],
    pub side: [i32; MAX_SLOTS],
    pub nat: [i32; MAX_SLOTS],
    pub staging_room_player_names: [Vec<u8>; MAX_SLOTS],
}

/// The `PeerResponse` types `Update` acts on.
#[derive(Clone, Debug)]
pub enum QmPeerResponse {
    /// `PEERRESPONSE_PLAYERUTM`.
    PlayerUtm { command: Vec<u8>, nick: Vec<u8>, command_options: Vec<u8> },
    /// `PEERRESPONSE_DISCONNECT`.
    Disconnect { reason: i32 },
    /// `PEERRESPONSE_CREATESTAGINGROOM`: `result == PEERJoinSuccess` and the raw result.
    CreateStagingRoom { success: bool, result: i32 },
    /// `PEERRESPONSE_JOINSTAGINGROOM`: the raw `ok` (both branches print the same).
    JoinStagingRoom { ok: i32 },
    /// `PEERRESPONSE_STAGINGROOM`.
    StagingRoom,
    /// `PEERRESPONSE_QUICKMATCHSTATUS`.
    QuickMatchStatus(QmStatusResponse),
    /// `PEERRESPONSE_JOINGROUPROOM`, `PLAYERJOIN`, `PLAYERLEFT`, `MESSAGE` (bodies commented out) and any other
    /// type: nothing happens.
    Other,
}

/// `NATStateType` as `Update` tests it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QmNatState {
    Done,
    Failed,
    Other,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolQuickMatchMenu {
    pub parent_wol_quick_match_id: i32,
    pub button_back_id: i32,
    pub button_start_id: i32,
    pub button_stop_id: i32,
    pub button_widen_id: i32,
    pub button_buddies_id: i32,
    pub listbox_quick_match_id: i32,
    pub listbox_map_select_id: i32,
    pub button_select_all_maps_id: i32,
    pub button_select_no_maps_id: i32,
    pub text_entry_wait_time_id: i32,
    pub combo_box_num_players_id: i32,
    pub combo_box_max_ping_id: i32,
    pub combo_box_ladder_id: i32,
    pub combo_box_max_disconnects_id: i32,
    pub static_text_num_players_id: i32,
    pub combo_box_side_id: i32,
    pub combo_box_color_id: i32,

    pub parent_wol_quick_match: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_start: Option<WinId>,
    pub button_stop: Option<WinId>,
    pub button_widen: Option<WinId>,
    /// `GameWindow *quickmatchTextWindow` (not static upstream).
    pub quickmatch_text_window: Option<WinId>,
    pub listbox_map_select: Option<WinId>,
    pub text_entry_wait_time: Option<WinId>,
    pub combo_box_num_players: Option<WinId>,
    pub combo_box_max_ping: Option<WinId>,
    pub combo_box_ladder: Option<WinId>,
    /// "enable and disable this, but never use it. it is a stand-in for comboBoxLadder for when there are no
    /// ladders".
    pub combo_box_disabled_ladder: Option<WinId>,
    pub combo_box_max_disconnects: Option<WinId>,
    pub static_text_num_players: Option<WinId>,
    pub combo_box_side: Option<WinId>,
    pub combo_box_color: Option<WinId>,

    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<Vec<u8>>,
    pub raise_message_boxes: bool,
    pub is_in_init: bool,
    /// `selectedImage`, `unselectedImage` (mapped image names).
    pub selected_image: Option<Vec<u8>>,
    pub unselected_image: Option<Vec<u8>>,
    pub is_populating_ladder_box: bool,
    pub max_ping_entries: i32,
    pub max_points: i32,
    pub min_points: i32,
    /// The `MapMetaData` the map list's column 1 item data points at (item data is `1 +` the index here).
    pub map_metas: Vec<QmMapMeta>,
}

impl WolQuickMatchMenu {
    /// The statics' initial values.
    pub fn new() -> Self {
        WolQuickMatchMenu { max_points: 100, min_points: 0, ..Default::default() }
    }
}

/// What the quick match menu reaches in the GameSpy layer and the engine.
pub trait WolQuickMatchMenuHost: ShellHost {
    fn wol_quick_match_menu(&mut self) -> &mut WolQuickMatchMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn format_text_arg(&self, fmt: &[u16], text: &[u16]) -> Vec<u16>;
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16>;
    fn format_int_int(&self, fmt: &[u16], a: i32, b: i32) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_MAP_SELECTED]`, `[GSCOLOR_MAP_UNSELECTED]`, `[GSCOLOR_DEFAULT]`.
    fn gamespy_color_map_selected(&self) -> i32;
    fn gamespy_color_map_unselected(&self) -> i32;
    fn gamespy_color_default(&self) -> i32;

    // TheLadderList
    fn ladder_find_by_index(&self, index: i32) -> Option<QmLadderInfo>;
    fn ladder_find(&self, addr: &[u8], port: u16) -> Option<QmLadderInfo>;
    fn ladder_special(&self) -> Vec<QmLadderInfo>;
    fn ladder_standard(&self) -> Vec<QmLadderInfo>;
    fn ladder_local_count(&self) -> usize;
    /// `LadderPreferences::loadProfile(localProfile)` then `getRecentLadders()`: (address, port) in map order.
    fn ladder_pref_recent_ladders(&mut self, profile: i32) -> Vec<(Vec<u8>, u16)>;
    /// `LadderPreferences::loadProfile`, `addRecentLadder(p)`, `write()`.
    fn ladder_pref_add_recent_and_write(&mut self, profile: i32, p: &QmLadderPref);

    // QuickMatchPreferences: `QuickMatchPreferences pref;` is `qm_pref_load`.
    fn qm_pref_load(&mut self);
    fn qm_pref_write(&mut self);
    fn qm_pref_color(&mut self) -> i32;
    fn qm_pref_side(&mut self) -> i32;
    fn qm_pref_num_players(&mut self) -> i32;
    fn qm_pref_max_ping(&mut self) -> i32;
    fn qm_pref_max_disconnects(&mut self) -> i32;
    fn qm_pref_max_points(&mut self) -> i32;
    fn qm_pref_min_points(&mut self) -> i32;
    fn qm_pref_last_ladder_addr(&mut self) -> Vec<u8>;
    fn qm_pref_last_ladder_port(&mut self) -> u16;
    fn qm_pref_is_map_selected(&mut self, map: &[u8]) -> bool;
    fn qm_pref_set_last_ladder(&mut self, addr: &[u8], port: u16);
    fn qm_pref_set_map_selected(&mut self, map: &[u8], selected: bool);
    fn qm_pref_set_num_players(&mut self, v: i32);
    fn qm_pref_set_max_ping(&mut self, v: i32);
    fn qm_pref_set_side(&mut self, v: i32);
    fn qm_pref_set_color(&mut self, v: i32);
    fn qm_pref_set_max_disconnects(&mut self, v: i32);
    /// `OptionPreferences::getFirewallBehavior()`.
    fn option_prefs_firewall_behavior(&mut self) -> i32;

    // TheMultiplayerSettings, ThePlayerTemplateStore
    fn multiplayer_num_colors(&self) -> i32;
    /// `TheMultiplayerSettings->getColor(c)`: (`getColor()`, `getTooltipName()`), `None` when null.
    fn multiplayer_color(&self, c: i32) -> Option<(i32, Vec<u8>)>;
    fn player_templates(&self) -> Vec<QmPlayerTemplate>;

    // TheGameSpyConfig
    fn gamespy_config_qm_maps(&self) -> Vec<Vec<u8>>;
    fn gamespy_config_ping_timeout_ms(&self) -> i32;
    fn gamespy_config_qm_bot_id(&self) -> i32;
    fn gamespy_config_qm_channel(&self) -> i32;
    /// `TheMapCache->findMap(map)`.
    fn map_cache_find(&self, map: &[u8]) -> Option<QmMapMeta>;
    /// `image->getImageWidth()` of the mapped image.
    fn image_width(&self, image: &[u8]) -> i32;
    /// `TheMappedImageCollection->findImageByName(name)`: the name when the image exists.
    fn find_image_by_name(&self, name: &[u8]) -> Option<Vec<u8>>;

    // TheGameSpyInfo
    fn gamespy_local_profile_id(&self) -> i32;
    fn gamespy_local_name(&self) -> Vec<u16>;
    fn gamespy_ping_string(&self) -> Vec<u8>;
    fn gamespy_max_messages_per_update(&self) -> i32;
    /// `isDisconnectedAfterGameStart(&reason)`.
    fn gamespy_is_disconnected_after_game_start(&self) -> Option<i32>;
    fn gamespy_mark_disconnected_after_game_start(&mut self, reason: i32);
    fn gamespy_reset(&mut self);
    fn gamespy_leave_group_room(&mut self);
    fn gamespy_leave_staging_room(&mut self);
    fn gamespy_has_info(&self) -> bool;
    fn gamespy_register_text_window(&mut self, w: Option<WinId>);
    fn gamespy_unregister_text_window(&mut self, w: Option<WinId>);
    /// `addText(text, color, win)`.
    fn gamespy_add_text(&mut self, wm: &mut GameWindowManager, text: &[u16], color: i32, win: Option<WinId>);
    // TheGameSpyPSMessageQueue
    fn ps_find_player_stats_by_id(&mut self, id: i32) -> QmPlayerStats;
    /// `stats = parsePlayerKVPairs(data); stats.id = id; trackPlayerStats(stats)`.
    fn ps_parse_and_track_player_stats(&mut self, data: &[u8], id: i32);
    /// `CalculateRank(stats)`.
    fn calculate_rank(&mut self, stats: &QmPlayerStats) -> i32;

    // TheGameSpyGame
    fn game_spy_game_exists(&self) -> bool;
    fn game_spy_game_is_game_in_progress(&self) -> bool;
    fn game_spy_game_set_game_in_progress(&mut self, v: bool);
    fn game_spy_game_reset(&mut self);
    fn game_spy_game_launch_game(&mut self);
    fn game_spy_game_enter_game(&mut self);
    fn game_spy_game_set_seed(&mut self, seed: i32);
    fn game_spy_game_mark_game_as_qm(&mut self);
    fn game_spy_game_set_ladder_ip(&mut self, ip: &[u8]);
    fn game_spy_game_set_ladder_port(&mut self, port: u16);
    fn game_spy_game_set_map(&mut self, map: &[u8]);
    fn game_spy_game_set_game_name(&mut self, name: &[u16]);
    fn game_spy_game_slot_set_closed(&mut self, i: usize);
    /// `slot->setState(SLOT_PLAYER, uName, ip)`, `setColor`, `setPlayerTemplate`, `setNATBehavior`,
    /// `setLocale("")`, `setTeamNumber`.
    #[allow(clippy::too_many_arguments)]
    fn game_spy_game_slot_set_player(&mut self, i: usize, name: &[u16], ip: u32, color: i32, player_template: i32, nat: i32, team_number: i32);
    /// `getSlotNum(nick)`.
    fn game_spy_game_slot_num(&self, nick: &[u8]) -> i32;
    /// For each slot: `slot && slot->isHuman() && slot->getLoginName().compareNoCase(nick) == 0` then
    /// `setProfileID(id)` on the first match.
    fn game_spy_game_set_profile_id_by_login_name(&mut self, nick: &[u8], id: i32);
    /// `SendStatsToOtherPlayers(TheGameSpyGame)`, `TheGameSpyGame->startGame(0)`.
    fn send_stats_to_other_players(&mut self);
    fn game_spy_game_start_game(&mut self);

    // TheNAT
    fn nat_exists(&self) -> bool;
    fn nat_delete(&mut self);
    fn nat_update(&mut self) -> QmNatState;
    fn nat_process_global_message(&mut self, slot: i32, options: &[u8]);

    // peer queue and overlays
    fn has_peer_message_queue(&self) -> bool;
    fn peer_next_response(&mut self) -> Option<QmPeerResponse>;
    /// `PEERREQUEST_STOPQUICKMATCH`, `PEERREQUEST_WIDENQUICKMATCHSEARCH`, `PEERREQUEST_STARTQUICKMATCH`.
    fn peer_request_stop_quick_match(&mut self);
    fn peer_request_widen_quick_match_search(&mut self);
    fn peer_request_start_quick_match(&mut self, req: &StartQuickMatchRequest);
    fn handle_buddy_responses(&mut self);
    fn handle_persistent_storage_responses(&mut self);
    fn gamespy_close_all_overlays(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_toggle_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_open_overlay_ladder_select(&mut self, wm: &mut GameWindowManager);
    fn raise_gs_message_box(&mut self);
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]);
    /// `UpdateLocalPlayerStats()` (WOLWelcomeMenu.cpp, [`crate::drafts::ui::wol_welcome_menu::update_local_player_stats`]).
    fn update_local_player_stats(&mut self, wm: &mut GameWindowManager);

    // engine
    /// `TheGameEngine->getQuitting()`.
    fn game_engine_quitting(&self) -> bool;
    /// `TheGameLogic->isInShellGame() && TheGameLogic->getFrame() == 1`.
    fn game_logic_in_shell_game_first_frame(&self) -> bool;
    /// `SignalUIInteraction(SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME)`.
    fn signal_ui_interaction_entered_from_game(&mut self);
    /// `TheInGameUI->message(label)`.
    fn in_game_ui_message(&mut self, label: &[u8]);
    /// `GameClientRandomValue(lo, hi)`.
    fn game_client_random_value(&mut self, lo: i32, hi: i32) -> i32;
    /// `time(NULL)`.
    fn time_now(&self) -> i64;
    fn global_exe_crc(&self) -> u32;
    fn global_ini_crc(&self) -> u32;
}

fn key_of<H: WolQuickMatchMenuHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("WOLQuickMatchMenu.wnd:{n}").as_bytes()) as i32
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `isInfoShown()`.
fn is_info_shown<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) -> bool {
    let id = key_of(h, "ParentStats");
    let parent = h.wol_quick_match_menu().parent_wol_quick_match;
    match wm.win_get_window_from_id(parent, id) {
        Some(p) => !wm.is_hidden(Some(p)),
        None => false,
    }
}

/// `hideInfoGadgets(doIt)`.
fn hide_info_gadgets<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, do_it: bool) {
    let id = key_of(h, "ParentStats");
    let parent = h.wol_quick_match_menu().parent_wol_quick_match;
    if let Some(p) = wm.win_get_window_from_id(parent, id) {
        wm.win_hide(p, do_it);
    }
}

/// `hideOptionsGadgets(doIt)`.
fn hide_options_gadgets<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, do_it: bool) {
    let id = key_of(h, "ParentOptions");
    let m = h.wol_quick_match_menu().clone();
    if let Some(p) = wm.win_get_window_from_id(m.parent_wol_quick_match, id) {
        wm.win_hide(p, do_it);
        for w in [m.combo_box_side, m.combo_box_color, m.combo_box_num_players, m.combo_box_ladder, m.combo_box_disabled_ladder, m.combo_box_max_ping, m.combo_box_max_disconnects] {
            if let Some(w) = w {
                wm.win_hide(w, do_it);
            }
        }
    }
}

/// `enableOptionsGadgets(doIt)`.
fn enable_options_gadgets<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, do_it: bool) {
    let id = key_of(h, "ParentOptions");
    let m = h.wol_quick_match_menu().clone();
    let li = get_ladder_info(wm, h);
    if let Some(p) = wm.win_get_window_from_id(m.parent_wol_quick_match, id) {
        wm.win_enable(p, do_it);
        if let Some(w) = m.combo_box_side {
            wm.win_enable(w, do_it && li.as_ref().map_or(true, |l| !l.random_factions));
        }
        if let Some(w) = m.combo_box_color {
            wm.win_enable(w, do_it);
        }
        if let Some(w) = m.combo_box_num_players {
            wm.win_enable(w, do_it);
        }
        if let Some(w) = m.combo_box_ladder {
            wm.win_enable(w, do_it);
        }
        if let Some(w) = m.combo_box_disabled_ladder {
            wm.win_enable(w, false);
        }
        if let Some(w) = m.combo_box_max_ping {
            wm.win_enable(w, do_it);
        }
        if let Some(w) = m.combo_box_max_disconnects {
            wm.win_enable(w, do_it);
        }
    }
}

/// `UpdateStartButton()`.
pub fn update_start_button<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_quick_match_menu().clone();
    let (Some(combo), Some(start), Some(list)) = (m.combo_box_ladder, m.button_start, m.listbox_map_select) else { return };

    let selected = cb::get_selected_pos(wm, h, combo, 0);
    let index = cb::get_item_data(wm, h, combo, selected) as i32;
    if h.ladder_find_by_index(index).is_some() {
        wm.win_enable(start, true);
        return;
    }

    let num_maps = lb::get_num_entries(wm, list);
    for i in 0..num_maps {
        if lb::get_item_data(wm, h, list, i, 0) != 0 {
            wm.win_enable(start, true);
            return;
        }
    }
    wm.win_enable(start, false);
}

/// `populateQMColorComboBox(pref)`.
fn populate_qm_color_combo_box<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(combo) = h.wol_quick_match_menu().combo_box_color else { return };
    let num_colors = h.multiplayer_num_colors();

    cb::reset(wm, h, combo);

    // def = getColor(PLAYERTEMPLATE_RANDOM)
    let random_color = h.multiplayer_color(PLAYERTEMPLATE_RANDOM).map_or(0, |d| d.0);
    let t = h.fetch_text(b"GUI:???");
    let new_index = cb::add_entry(wm, h, combo, &t, random_color);
    cb::set_item_data(wm, h, combo, new_index, -1i32 as u32);

    for c in 0..num_colors {
        let Some((color, tooltip)) = h.multiplayer_color(c) else { continue };
        let color_name = h.fetch_text(&tooltip);
        let new_index = cb::add_entry(wm, h, combo, &color_name, color);
        cb::set_item_data(wm, h, combo, new_index, c as u32);
    }
    let pref_color = h.qm_pref_color();
    cb::set_selected_pos(wm, h, combo, pref_color, false);
}

/// `populateQMSideComboBox(favSide, li)`.
fn populate_qm_side_combo_box<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, fav_side: i32, li: Option<&QmLadderInfo>) {
    let Some(combo) = h.wol_quick_match_menu().combo_box_side else { return };
    let templates = h.player_templates();

    cb::reset(wm, h, combo);

    let random_color = h.multiplayer_color(PLAYERTEMPLATE_RANDOM).map_or(0, |d| d.0);
    let t = h.fetch_text(b"GUI:Random");
    let new_index = cb::add_entry(wm, h, combo, &t, random_color);
    cb::set_item_data(wm, h, combo, new_index, PLAYERTEMPLATE_RANDOM as u32);

    let mut seen_sides: Vec<Vec<u8>> = Vec::new();

    let mut entry_to_select = 0; // select Random by default

    for (c, fac) in templates.iter().enumerate() {
        let c = c as i32;
        if fac.starting_building_empty {
            continue;
        }

        let side = format!("SIDE:{}", String::from_utf8_lossy(&fac.side)).into_bytes();
        if seen_sides.contains(&side) {
            continue;
        }

        if let Some(li) = li {
            if !li.valid_factions.contains(&fac.side) {
                continue; // ladder doesn't allow it.
            }
        }

        // Remove disallowed generals from the choice list.
        // This is also enforced at GUI setup (GUIUtil.cpp and UserPreferences.cpp).
        // @todo: unlock these when something rad happens
        let disallow_locked_generals = true;
        let starts_locked = fac.general_starting_enabled.map_or(false, |enabled| !enabled);
        if disallow_locked_generals && starts_locked {
            continue;
        }

        seen_sides.push(side.clone());

        let t = h.fetch_text(&side);
        let new_index = cb::add_entry(wm, h, combo, &t, random_color);
        cb::set_item_data(wm, h, combo, new_index, c as u32);

        if c == fav_side {
            entry_to_select = new_index;
        }
    }

    cb::set_selected_pos(wm, h, combo, entry_to_select, false);
    wm.win_enable(combo, !li.is_some_and(|l| l.random_factions));
}

/// `HandleQMLadderSelection(ladderID)`.
pub fn handle_qm_ladder_selection<H: WolQuickMatchMenuHost>(h: &mut H, ladder_id: i32) {
    if h.wol_quick_match_menu().parent_wol_quick_match.is_none() {
        return;
    }

    h.qm_pref_load();

    if ladder_id < 1 {
        h.qm_pref_set_last_ladder(&[], 0);
        h.qm_pref_write();
        return;
    }

    match h.ladder_find_by_index(ladder_id) {
        None => h.qm_pref_set_last_ladder(&[], 0),
        Some(info) => h.qm_pref_set_last_ladder(&info.address, info.port),
    }

    h.qm_pref_write();
}

/// `isValidLadder(lad)`.
fn is_valid_ladder<H: WolQuickMatchMenuHost>(h: &mut H, lad: Option<&QmLadderInfo>) -> bool {
    if let Some(lad) = lad {
        if lad.index > 0 && lad.valid_qm {
            let pid = h.gamespy_local_profile_id();
            let stats = h.ps_find_player_stats_by_id(pid);
            let num_wins: i32 = stats.wins.iter().sum();
            if lad.max_wins == 0 || lad.max_wins >= num_wins {
                if lad.min_wins == 0 || lad.min_wins <= num_wins {
                    return true;
                }
            }
        }
    }
    false
}

/// `PopulateQMLadderListBox(win)` (WOLQuickMatchMenu.cpp:300).
pub fn populate_qm_ladder_list_box<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, win: Option<WinId>) {
    let m = h.wol_quick_match_menu().clone();
    if m.parent_wol_quick_match.is_none() || m.combo_box_ladder.is_none() {
        return;
    }
    let Some(win) = win else { return };

    h.wol_quick_match_menu().is_populating_ladder_box = true;

    h.qm_pref_load();
    let local_profile = h.gamespy_local_profile_id();

    let special_color = h.gamespy_color_map_selected();
    let normal_color = h.gamespy_color_map_unselected();
    let favorite_color = h.gamespy_color_map_unselected();
    lb::reset(wm, h, win);

    let mut used_ladders: Vec<i32> = Vec::new();

    // start with "No Ladder"
    let t = h.fetch_text(b"GUI:NoLadder");
    let index = lb::add_entry_text(wm, h, win, &t, normal_color, -1, -1, false);
    lb::set_item_data(wm, h, win, 0, index, 0);

    // add the last ladder
    let mut selected_pos = 0;
    let last_ladder_addr = h.qm_pref_last_ladder_addr();
    let last_ladder_port = h.qm_pref_last_ladder_port();
    let info = h.ladder_find(&last_ladder_addr, last_ladder_port);
    if is_valid_ladder(h, info.as_ref()) {
        let info = info.expect("valid implies present");
        used_ladders.push(info.index);
        let index = lb::add_entry_text(wm, h, win, &info.name, favorite_color, -1, -1, false);
        lb::set_item_data(wm, h, win, info.index as u32, index, 0);
        selected_pos = index;
    }

    // our recent ladders
    for (addr, port) in h.ladder_pref_recent_ladders(local_profile) {
        if addr == last_ladder_addr && port == last_ladder_port {
            continue;
        }
        let info = h.ladder_find(&addr, port);
        if is_valid_ladder(h, info.as_ref()) {
            let info = info.expect("valid implies present");
            if !used_ladders.contains(&info.index) {
                used_ladders.push(info.index);
                let index = lb::add_entry_text(wm, h, win, &info.name, favorite_color, -1, -1, false);
                lb::set_item_data(wm, h, win, info.index as u32, index, 0);
            }
        }
    }

    // special ladders
    for info in h.ladder_special() {
        if is_valid_ladder(h, Some(&info)) && !used_ladders.contains(&info.index) {
            used_ladders.push(info.index);
            let index = lb::add_entry_text(wm, h, win, &info.name, special_color, -1, -1, false);
            lb::set_item_data(wm, h, win, info.index as u32, index, 0);
        }
    }

    // standard ladders
    for info in h.ladder_standard() {
        if is_valid_ladder(h, Some(&info)) && !used_ladders.contains(&info.index) {
            used_ladders.push(info.index);
            let index = lb::add_entry_text(wm, h, win, &info.name, normal_color, -1, -1, false);
            lb::set_item_data(wm, h, win, info.index as u32, index, 0);
        }
    }

    lb::set_selected(wm, h, win, &[selected_pos]);
    h.wol_quick_match_menu().is_populating_ladder_box = false;
}

/// `getLadderInfo()`.
fn get_ladder_info<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) -> Option<QmLadderInfo> {
    let combo = h.wol_quick_match_menu().combo_box_ladder?; // (NULL upstream: see the file header)
    let selected = cb::get_selected_pos(wm, h, combo, 0);
    let index = cb::get_item_data(wm, h, combo, selected) as i32;
    h.ladder_find_by_index(index)
}

/// `PopulateQMLadderComboBox()` (WOLQuickMatchMenu.cpp:425).
pub fn populate_qm_ladder_combo_box<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_quick_match_menu().clone();
    let (Some(_), Some(combo)) = (m.parent_wol_quick_match, m.combo_box_ladder) else { return };

    h.wol_quick_match_menu().is_populating_ladder_box = true;

    h.qm_pref_load();
    let local_profile = h.gamespy_local_profile_id();

    let special_color = h.gamespy_color_map_selected();
    let normal_color = h.gamespy_color_map_unselected();
    cb::reset(wm, h, combo);
    let t = h.fetch_text(b"GUI:NoLadder");
    let index = cb::add_entry(wm, h, combo, &t, normal_color);
    cb::set_item_data(wm, h, combo, index, 0);

    let mut used_ladders: Vec<i32> = Vec::new();

    let mut selected_pos = 0;
    let last_ladder_addr = h.qm_pref_last_ladder_addr();
    let last_ladder_port = h.qm_pref_last_ladder_port();
    let info = h.ladder_find(&last_ladder_addr, last_ladder_port);
    if is_valid_ladder(h, info.as_ref()) {
        let info = info.expect("valid implies present");
        used_ladders.push(info.index);
        let index = cb::add_entry(wm, h, combo, &info.name, special_color);
        cb::set_item_data(wm, h, combo, index, info.index as u32);
        selected_pos = index;

        // we selected a ladder?  No game size choice for us...
        if let Some(np) = m.combo_box_num_players {
            cb::set_selected_pos(wm, h, np, info.players_per_team - 1, false);
            wm.win_enable(np, false);
        }
    } else if let Some(np) = m.combo_box_num_players {
        wm.win_enable(np, true);
    }

    for (addr, port) in h.ladder_pref_recent_ladders(local_profile) {
        if addr == last_ladder_addr && port == last_ladder_port {
            continue;
        }
        let info = h.ladder_find(&addr, port);
        if is_valid_ladder(h, info.as_ref()) {
            let info = info.expect("valid implies present");
            if !used_ladders.contains(&info.index) {
                used_ladders.push(info.index);
                let index = cb::add_entry(wm, h, combo, &info.name, normal_color);
                cb::set_item_data(wm, h, combo, index, info.index as u32);
            }
        }
    }

    let t = h.fetch_text(b"GUI:ChooseLadder");
    let index = cb::add_entry(wm, h, combo, &t, normal_color);
    cb::set_item_data(wm, h, combo, index, -1i32 as u32);

    cb::set_selected_pos(wm, h, combo, selected_pos, false);
    h.wol_quick_match_menu().is_populating_ladder_box = false;

    let fav_side = h.qm_pref_side();
    let li = get_ladder_info(wm, h);
    populate_qm_side_combo_box(wm, h, fav_side, li.as_ref()); // this will set side to random and disable if necessary
}

/// The map list's row image: `width = min(GadgetListBoxGetColumnWidth(list, 0), img->getImageWidth())`,
/// `height = width`, or 10 x 10 without an image.
fn map_image_size<H: WolQuickMatchMenuHost>(wm: &GameWindowManager, h: &H, list: WinId, img: &Option<Vec<u8>>) -> (i32, i32) {
    match img {
        Some(i) => {
            let width = lb::get_column_width(wm, list, 0).min(h.image_width(i));
            (width, width) // (height, width)
        }
        None => (10, 10),
    }
}

/// `populateQuickMatchMapSelectListbox(pref)`.
fn populate_quick_match_map_select_listbox<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_quick_match_menu().clone();
    let (Some(combo), Some(list)) = (m.combo_box_ladder, m.listbox_map_select) else { return };
    let mut maps = h.gamespy_config_qm_maps();

    // enable/disable box based on ladder status
    let selected = cb::get_selected_pos(wm, h, combo, 0);
    let index = cb::get_item_data(wm, h, combo, selected) as i32;
    let li = h.ladder_find_by_index(index);
    // listboxMapSelect->winEnable( li == NULL || li->randomMaps == FALSE );  (commented out upstream)

    let num_players;
    if let Some(li) = &li {
        num_players = li.players_per_team * 2;
        maps = li.valid_maps.clone();
    } else {
        let mut selected = match m.combo_box_num_players {
            Some(np) => cb::get_selected_pos(wm, h, np, 0),
            None => 0,
        };
        if selected < 0 {
            selected = 0;
        }
        num_players = (selected + 1) * 2;
    }

    lb::reset(wm, h, list);
    h.wol_quick_match_menu().map_metas.clear();
    for the_map in maps {
        let Some(md) = h.map_cache_find(&the_map) else { continue };
        if md.num_players >= num_players {
            let display_name = md.display_name.clone();
            let mut is_selected = h.qm_pref_is_map_selected(&the_map);
            if li.as_ref().is_some_and(|l| l.random_maps) {
                is_selected = true;
            }
            let img = if is_selected { m.selected_image.clone() } else { m.unselected_image.clone() };
            let (height, width) = map_image_size(wm, h, list, &img);
            let index = lb::add_entry_image(wm, h, list, img, -1, 0, height, width, true, -1);
            let color = if is_selected { h.gamespy_color_map_selected() } else { h.gamespy_color_map_unselected() };
            lb::add_entry_text(wm, h, list, &display_name, color, index, 1, true);
            lb::set_item_data(wm, h, list, u32::from(is_selected), index, 0);
            let menu = h.wol_quick_match_menu();
            menu.map_metas.push(md);
            let handle = menu.map_metas.len() as u32; // 1 + index
            lb::set_item_data(wm, h, list, handle, index, 1);
        }
    }
}

/// `saveQuickMatchOptions()`.
fn save_quick_match_options<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.wol_quick_match_menu().is_in_init {
        return;
    }
    h.qm_pref_load();
    let m = h.wol_quick_match_menu().clone();

    let li = get_ladder_info(wm, h);

    if let Some(li) = &li {
        h.qm_pref_set_last_ladder(&li.address, li.port);
        h.qm_pref_write();
        // (numPlayers = li->playersPerTeam*2 is computed but unused here)
    } else {
        h.qm_pref_set_last_ladder(&[], 0);
        // (numPlayers from comboBoxNumPlayers is computed but unused here)
    }

    if li.as_ref().map_or(true, |l| !l.random_maps) {
        // don't save the map as selected if we couldn't choose
        if let Some(list) = m.listbox_map_select {
            let entries = lb::get_num_entries(wm, list);
            let mut row = 0;
            while row < entries {
                let handle = lb::get_item_data(wm, h, list, row, 1) as usize;
                if handle != 0 {
                    if let Some(md) = m.map_metas.get(handle - 1) {
                        let sel = lb::get_item_data(wm, h, list, row, 0) != 0;
                        h.qm_pref_set_map_selected(&md.file_name, sel);
                    }
                }
                row += 1;
            }
        }
    }

    // (the text entries for max disconnects / points / wait time are commented out upstream)

    if let Some(c) = m.combo_box_num_players {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        h.qm_pref_set_num_players(selected);
    }
    if let Some(c) = m.combo_box_max_ping {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        h.qm_pref_set_max_ping(selected);
    }

    if let Some(c) = m.combo_box_side {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        let item = cb::get_item_data(wm, h, c, selected) as i32;
        h.qm_pref_set_side(item.max(0));
    }
    if let Some(c) = m.combo_box_color {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        h.qm_pref_set_color(selected.max(0));
    }

    if let Some(c) = m.combo_box_max_disconnects {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        h.qm_pref_set_max_disconnects(selected);
    }

    h.qm_pref_write();
}

/// `WOLQuickMatchMenuInit(layout, userData)` (WOLQuickMatchMenu.cpp:614).
pub fn wol_quick_match_menu_init<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_quick_match_menu().is_in_init = true;
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
            // pop immediate back to main menu
            shell::pop_immediate(wm, h);
            return;
        }
    }

    {
        let m = h.wol_quick_match_menu();
        m.next_screen = None;
        m.button_pushed = false;
        m.is_shutting_down = false;
        m.raise_message_boxes = true;
    }

    if h.nat_exists() {
        h.nat_delete();
    }

    let parent_id = key_of(h, "WOLQuickMatchMenuParent");
    let back_id = key_of(h, "ButtonBack");
    let buddies_id = key_of(h, "ButtonBuddies");
    let start_id = key_of(h, "ButtonStart");
    let stop_id = key_of(h, "ButtonStop");
    let widen_id = key_of(h, "ButtonWiden");
    let list_qm_id = key_of(h, "ListboxQuickMatch");
    let list_map_id = key_of(h, "ListBoxMapSelect");
    let all_maps_id = key_of(h, "ButtonSelectAllMaps");
    let no_maps_id = key_of(h, "ButtonSelectNoMaps");
    let wait_time_id = key_of(h, "TextEntryWaitTime");
    let max_ping_id = key_of(h, "ComboBoxMaxPing");
    let num_players_id = key_of(h, "ComboBoxNumPlayers");
    let ladder_id = key_of(h, "ComboBoxLadder");
    let max_discons_id = key_of(h, "ComboBoxMaxDisconnects");
    let static_np_id = key_of(h, "StaticTextNumPlayers");
    let side_id = key_of(h, "ComboBoxSide");
    let color_id = key_of(h, "ComboBoxColor");

    let parent = wm.win_get_window_from_id(None, parent_id);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);
    let button_back = find(wm, back_id);
    let button_start = find(wm, start_id);
    let button_stop = find(wm, stop_id);
    let button_widen = find(wm, widen_id);
    let quickmatch_text_window = find(wm, list_qm_id);
    let listbox_map_select = find(wm, list_map_id);
    let text_entry_wait_time = find(wm, wait_time_id);
    let combo_box_max_ping = find(wm, max_ping_id);
    let combo_box_num_players = find(wm, num_players_id);
    let mut combo_box_ladder = find(wm, ladder_id);
    let combo_box_max_disconnects = find(wm, max_discons_id);
    h.gamespy_register_text_window(quickmatch_text_window);
    let static_text_num_players = find(wm, static_np_id);
    let combo_box_side = find(wm, side_id);
    let combo_box_color = find(wm, color_id);

    {
        let m = h.wol_quick_match_menu();
        m.parent_wol_quick_match_id = parent_id;
        m.button_back_id = back_id;
        m.button_buddies_id = buddies_id;
        m.button_start_id = start_id;
        m.button_stop_id = stop_id;
        m.button_widen_id = widen_id;
        m.listbox_quick_match_id = list_qm_id;
        m.listbox_map_select_id = list_map_id;
        m.button_select_all_maps_id = all_maps_id;
        m.button_select_no_maps_id = no_maps_id;
        m.text_entry_wait_time_id = wait_time_id;
        m.combo_box_max_ping_id = max_ping_id;
        m.combo_box_num_players_id = num_players_id;
        m.combo_box_ladder_id = ladder_id;
        m.combo_box_max_disconnects_id = max_discons_id;
        m.static_text_num_players_id = static_np_id;
        m.combo_box_side_id = side_id;
        m.combo_box_color_id = color_id;
        m.parent_wol_quick_match = parent;
        m.button_back = button_back;
        m.button_start = button_start;
        m.button_stop = button_stop;
        m.button_widen = button_widen;
        m.quickmatch_text_window = quickmatch_text_window;
        m.listbox_map_select = listbox_map_select;
        m.text_entry_wait_time = text_entry_wait_time;
        m.combo_box_max_ping = combo_box_max_ping;
        m.combo_box_num_players = combo_box_num_players;
        m.combo_box_max_disconnects = combo_box_max_disconnects;
        m.static_text_num_players = static_text_num_players;
        m.combo_box_side = combo_box_side;
        m.combo_box_color = combo_box_color;
        m.combo_box_disabled_ladder = None;
    }

    if h.ladder_standard().is_empty() && h.ladder_special().is_empty() && h.ladder_local_count() == 0 {
        // no ladders, so just disable them
        let disabled = combo_box_ladder;
        combo_box_ladder = None;
        {
            let m = h.wol_quick_match_menu();
            m.combo_box_disabled_ladder = disabled;
            m.is_populating_ladder_box = true;
        }

        let normal_color = h.gamespy_color_map_unselected();
        if let Some(d) = disabled {
            cb::reset(wm, h, d);
            let t = h.fetch_text(b"GUI:NoLadder");
            let index = cb::add_entry(wm, h, d, &t, normal_color);
            cb::set_item_data(wm, h, d, index, 0);
            cb::set_selected_pos(wm, h, d, index, false);
        }

        h.wol_quick_match_menu().is_populating_ladder_box = false;
        // (the commented-out block that would hide the combo box instead is not translated)
    }
    h.wol_quick_match_menu().combo_box_ladder = combo_box_ladder;

    if let Some(b) = wm.win_get_window_from_id(None, buddies_id) {
        wm.win_enable(b, true);
    }

    let static_text_title_id = key_of(h, "StaticTextTitle");
    if let Some(title_win) = wm.win_get_window_from_id(parent, static_text_title_id) {
        let fmt = h.fetch_text(b"GUI:QuickMatchTitle");
        let name = h.gamespy_local_name();
        let tmp = h.format_text_arg(&fmt, &name);
        gadget::static_text_set_text(wm, h, Some(title_win), &tmp);
    }

    // QM is not going yet, so disable the Widen Search button
    if let Some(b) = button_widen {
        wm.win_enable(b, false);
    }
    if let Some(b) = button_stop {
        wm.win_hide(b, true);
    }
    if let Some(b) = button_start {
        wm.win_hide(b, false);
    }
    if let Some(w) = quickmatch_text_window {
        lb::reset(wm, h, w);
    }
    enable_options_gadgets(wm, h, true);

    // Show Menu
    wm.layout_hide(layout, false);

    // Set Keyboard to Main Parent
    wm.win_set_focus(h, parent);

    // fill in preferences
    let selected_image = h.find_image_by_name(b"CustomMatch_selected");
    let unselected_image = h.find_image_by_name(b"CustomMatch_deselected");
    h.qm_pref_load();

    // (the commented-out text entry fills are not translated)
    let max_points = h.qm_pref_max_points();
    let min_points = h.qm_pref_min_points();
    {
        let m = h.wol_quick_match_menu();
        m.selected_image = selected_image;
        m.unselected_image = unselected_image;
        m.max_points = max_points;
        m.min_points = min_points;
    }

    let c = h.gamespy_color_default();
    if let Some(np) = combo_box_num_players {
        cb::reset(wm, h, np);
        for i in 1..5 {
            let fmt = h.fetch_text(b"GUI:PlayersVersusPlayers");
            let s = h.format_int_int(&fmt, i, i);
            cb::add_entry(wm, h, np, &s, c);
        }
        let n = h.qm_pref_num_players();
        cb::set_selected_pos(wm, h, np, n.max(0), false);
    }

    if let Some(md) = combo_box_max_disconnects {
        cb::reset(wm, h, md);
        let t = h.fetch_text(b"GUI:Any");
        cb::add_entry(wm, h, md, &t, c);
        for i in 1..MAX_DISCONNECTS_COUNT {
            let s = utf16(&MAX_DISCONNECTS[i].to_string());
            cb::add_entry(wm, h, md, &s, c);
        }
        let max_discon_index = h.qm_pref_max_disconnects().max(0);
        cb::set_selected_pos(wm, h, md, max_discon_index, false);
    }

    if let Some(mp) = combo_box_max_ping {
        cb::reset(wm, h, mp);
        let mut max_ping_entries = (h.gamespy_config_ping_timeout_ms() - 1) / 100;
        max_ping_entries += 1; // need to add the entry for the actual timeout
        h.wol_quick_match_menu().max_ping_entries = max_ping_entries;
        for i in 1..max_ping_entries {
            let fmt = h.fetch_text(b"GUI:TimeInMilliseconds");
            let s = h.format_int(&fmt, i * 100);
            cb::add_entry(wm, h, mp, &s, c);
        }
        let t = h.fetch_text(b"GUI:ANY");
        cb::add_entry(wm, h, mp, &t, c);
        let mut i = h.qm_pref_max_ping();
        if i < 0 {
            i = 0;
        }
        if i >= max_ping_entries {
            i = max_ping_entries - 1;
        }
        cb::set_selected_pos(wm, h, mp, i, false);
    }

    populate_qm_color_combo_box(wm, h);
    let fav_side = h.qm_pref_side();
    let li = get_ladder_info(wm, h);
    populate_qm_side_combo_box(wm, h, fav_side, li.as_ref());

    populate_qm_ladder_combo_box(wm, h);
    shell::show_shell_map(wm, h, true);
    h.game_spy_game_reset();

    if let Some(list) = listbox_map_select {
        lb::reset(wm, h, list);
    }
    populate_quick_match_map_select_listbox(wm, h);

    h.update_local_player_stats(wm);
    update_start_button(wm, h);
    wt::set_group(wm, h, b"WOLQuickMatchMenuFade", false);
    h.wol_quick_match_menu().is_in_init = false;
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_quick_match_menu().is_shutting_down = false;

    // hide the layout
    wm.layout_hide(layout, true);

    // our shutdown is complete
    let has_next = h.wol_quick_match_menu().next_screen.is_some();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    if let Some(next) = h.wol_quick_match_menu().next_screen.clone() {
        shell::push(wm, h, &next, false);
    }

    h.wol_quick_match_menu().next_screen = None;
}

/// `WOLQuickMatchMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn wol_quick_match_menu_shutdown<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    let text_window = h.wol_quick_match_menu().quickmatch_text_window;
    h.gamespy_unregister_text_window(text_window);

    if !h.game_engine_quitting() {
        save_quick_match_options(wm, h);
    }

    {
        let m = h.wol_quick_match_menu();
        m.parent_wol_quick_match = None;
        m.button_back = None;
        m.quickmatch_text_window = None;
        m.selected_image = None;
        m.unselected_image = None;

        m.is_shutting_down = true;
    }

    // if we are shutting down for an immediate pop, skip the animations
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"WOLQuickMatchMenuFade");

    h.raise_gs_message_box();
}

/// `allowedMessages-- && !sawImportantMessage && queue->getResponse(resp)`, the response loop's condition.
fn next_response<H: WolQuickMatchMenuHost>(h: &mut H, allowed_messages: &mut i32, saw_important_message: bool) -> Option<QmPeerResponse> {
    let go = *allowed_messages != 0;
    *allowed_messages -= 1;
    if !go || saw_important_message {
        return None;
    }
    h.peer_next_response()
}

/// `AsciiString::nextToken(&tok, " ")`: skips leading separators, takes the token, leaves the rest (from the
/// separator on) in `data`.
fn next_token_space(data: &mut Vec<u8>) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let start = data.iter().position(|&c| c != b' ').unwrap_or(data.len());
    let end = data[start..].iter().position(|&c| c == b' ').map_or(data.len(), |e| start + e);
    if end > start {
        let tok = data[start..end].to_vec();
        *data = data[end..].to_vec();
        Some(tok)
    } else {
        data.clear();
        None
    }
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

fn qm_text<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, label: &str) {
    let t = h.fetch_text(label.as_bytes());
    let c = h.gamespy_color_default();
    let w = h.wol_quick_match_menu().quickmatch_text_window;
    h.gamespy_add_text(wm, &t, c, w);
}

/// The tail shared by `QM_COULDNOTFINDBOT`, `QM_COULDNOTFINDCHANNEL`, `QM_COULDNOTNEGOTIATEFIREWALLS`, `QM_STOPPED`.
fn qm_back_to_start<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_quick_match_menu().clone();
    if let Some(b) = m.button_widen {
        wm.win_enable(b, false);
    }
    if let Some(b) = m.button_start {
        wm.win_hide(b, false);
    }
    if let Some(b) = m.button_stop {
        wm.win_hide(b, true);
    }
    enable_options_gadgets(wm, h, true);
}

/// `case QM_MATCHED:` of `WOLQuickMatchMenuUpdate`.
fn handle_qm_matched<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, mut status: QmStatusResponse) {
    qm_text(wm, h, "QM:MATCHED");
    let m = h.wol_quick_match_menu().clone();
    if let Some(b) = m.button_widen {
        wm.win_enable(b, false);
    }

    h.game_spy_game_enter_game();
    h.game_spy_game_set_seed(status.seed);

    h.game_spy_game_mark_game_as_qm();

    match get_ladder_info(wm, h) {
        None => {
            h.game_spy_game_set_ladder_ip(b"localhost");
            h.game_spy_game_set_ladder_port(0);
        }
        Some(info) => {
            h.game_spy_game_set_ladder_ip(&info.address);
            h.game_spy_game_set_ladder_port(info.port);
        }
    }

    let mut num_players = 0;
    for i in 0..MAX_SLOTS {
        if !status.staging_room_player_names[i].is_empty() {
            num_players += 1;
        }
    }

    for it in h.gamespy_config_qm_maps() {
        let the_map = it.to_ascii_lowercase();
        if let Some(md) = h.map_cache_find(&the_map) {
            if md.num_players >= num_players {
                h.game_spy_game_set_map(&it);
                let before = status.map_idx;
                status.map_idx -= 1;
                if before == 0 {
                    break;
                }
            }
        }
    }

    let mut num_players_per_team = num_players / 2;
    // DEBUG_ASSERTCRASH(numPlayersPerTeam, ("0 players per team???")) is not translated
    if num_players_per_team == 0 {
        num_players_per_team = 1;
    }

    for i in 0..MAX_SLOTS {
        if status.staging_room_player_names[i].is_empty() {
            h.game_spy_game_slot_set_closed(i);
        } else {
            let a_name = &status.staging_room_player_names[i];
            let u_name: Vec<u16> = a_name.iter().map(|&c| u16::from(c)).collect(); // uName.translate(aName)
            h.game_spy_game_slot_set_player(i, &u_name, status.ip[i], status.color[i], status.side[i], status.nat[i], i as i32 / num_players_per_team);
            if i == 0 {
                h.game_spy_game_set_game_name(&u_name);
            }
        }
    }

    // DEBUG_LOG of GameInfoToAsciiString is not translated
    h.send_stats_to_other_players();
    h.game_spy_game_start_game();
    let buddies_id = h.wol_quick_match_menu().button_buddies_id;
    if let Some(b) = wm.win_get_window_from_id(None, buddies_id) {
        wm.win_enable(b, false);
    }
    h.gamespy_close_overlay_buddy(wm);
}

/// `WOLQuickMatchMenuUpdate(layout, userData)` (WOLQuickMatchMenu.cpp:830).
pub fn wol_quick_match_menu_update<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.game_logic_in_shell_game_first_frame() {
        h.signal_ui_interaction_entered_from_game();
    }

    // We'll only be successful if we've requested to
    if h.wol_quick_match_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }

    if h.wol_quick_match_menu().raise_message_boxes {
        h.raise_gs_message_box();
        h.wol_quick_match_menu().raise_message_boxes = false;
    }

    // @todo: MDC handle disconnects in-game the same way as Custom Match!

    if shell::is_anim_finished(wm, h) && !h.wol_quick_match_menu().button_pushed && h.has_peer_message_queue() {
        h.handle_buddy_responses();
        h.handle_persistent_storage_responses();

        if h.game_spy_game_exists() && h.game_spy_game_is_game_in_progress() {
            if h.gamespy_is_disconnected_after_game_start().is_some() {
                return; // already been disconnected, so don't worry.
            }

            let mut allowed_messages = h.gamespy_max_messages_per_update();
            let mut saw_important_message = false;
            while let Some(resp) = next_response(h, &mut allowed_messages, saw_important_message) {
                if let QmPeerResponse::Disconnect { reason } = resp {
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

        if h.nat_exists() {
            let nat_state = h.nat_update();
            if nat_state == QmNatState::Done {
                h.game_spy_game_launch_game();
                if h.gamespy_has_info() {
                    // this can be blown away by a disconnect on the map transfer screen
                    h.gamespy_leave_staging_room();
                }
                return; // don't do any more processing this frame, in case the screen goes away
            } else if nat_state == QmNatState::Failed {
                // delete TheNAT, its no good for us anymore.
                h.nat_delete();

                // Just back out.  This cleans up some slot list problems
                h.wol_quick_match_menu().button_pushed = true;
                let title = h.fetch_text(b"GUI:Error");
                let body = h.fetch_text(b"GUI:NATNegotiationFailed");
                h.gs_message_box_ok(wm, &title, &body);
                h.wol_quick_match_menu().next_screen = Some(b"Menus/WOLWelcomeMenu.wnd".to_vec());
                shell::pop(wm, h);
                return; // don't do any more processing this frame, in case the screen goes away
            }
        }

        let mut allowed_messages = h.gamespy_max_messages_per_update();
        let mut saw_important_message = false;
        while let Some(resp) = next_response(h, &mut allowed_messages, saw_important_message) {
            match resp {
                QmPeerResponse::PlayerUtm { command, nick, command_options } => {
                    if command.eq_ignore_ascii_case(b"STATS") {
                        // DEBUG_LOG of the STATS data is not translated
                        let mut data = command_options.clone();
                        let id_str = next_token_space(&mut data).unwrap_or_default();
                        let id = atoi(&id_str);

                        let old_stats = h.ps_find_player_stats_by_id(id);
                        // stats.id = id; if (stats.id && (oldStats.id == 0)) trackPlayerStats(stats)
                        if id != 0 && old_stats.id == 0 {
                            h.ps_parse_and_track_player_stats(&data, id);
                        }

                        // now fill in the profileID in the game slot
                        h.game_spy_game_set_profile_id_by_login_name(&nick, id);
                    }
                    let slot_num = h.game_spy_game_slot_num(&nick);
                    if slot_num >= 0 && (slot_num as usize) < MAX_SLOTS && command.eq_ignore_ascii_case(b"NAT") {
                        // this is a command for NAT negotiations, pass if off to TheNAT
                        saw_important_message = true;
                        if h.nat_exists() {
                            h.nat_process_global_message(slot_num, &command_options);
                        }
                    }
                    // (the commented-out `key == "NAT"` block is not translated)
                }
                QmPeerResponse::Disconnect { reason } => {
                    saw_important_message = true;
                    let discon_munkee = format!("GUI:GSDisconReason{reason}");
                    let title = h.fetch_text(b"GUI:GSErrorTitle");
                    let body = h.fetch_text(discon_munkee.as_bytes());
                    h.gamespy_close_all_overlays(wm);
                    h.gs_message_box_ok(wm, &title, &body);
                    h.gamespy_reset();
                    shell::pop(wm, h);
                }
                QmPeerResponse::CreateStagingRoom { success, result } => {
                    let text = if success { utf16("Created staging room") } else { utf16(&format!("createStagingRoom result: {result}")) };
                    let c = h.gamespy_color_default();
                    let w = h.wol_quick_match_menu().quickmatch_text_window;
                    h.gamespy_add_text(wm, &text, c, w);
                }
                QmPeerResponse::JoinStagingRoom { ok } => {
                    // both branches print the same text
                    let text = utf16(&format!("joinStagingRoom result: {ok}"));
                    let c = h.gamespy_color_default();
                    let w = h.wol_quick_match_menu().quickmatch_text_window;
                    h.gamespy_add_text(wm, &text, c, w);
                }
                QmPeerResponse::StagingRoom => {
                    let text = utf16("Staging room list callback");
                    let c = h.gamespy_color_default();
                    let w = h.wol_quick_match_menu().quickmatch_text_window;
                    h.gamespy_add_text(wm, &text, c, w);
                }
                QmPeerResponse::QuickMatchStatus(status) => {
                    saw_important_message = true;
                    let widen = h.wol_quick_match_menu().button_widen;
                    match status.status {
                        QmState::Idle => {}
                        QmState::JoiningQmChannel => qm_text(wm, h, "QM:JOININGQMCHANNEL"),
                        QmState::LookingForBot => qm_text(wm, h, "QM:LOOKINGFORBOT"),
                        QmState::SentInfo => qm_text(wm, h, "QM:SENTINFO"),
                        QmState::Working => {
                            let fmt = h.fetch_text(b"QM:WORKING");
                            let s = h.format_int(&fmt, status.pool_size);
                            let c = h.gamespy_color_default();
                            let w = h.wol_quick_match_menu().quickmatch_text_window;
                            h.gamespy_add_text(wm, &s, c, w);
                            if let Some(b) = widen {
                                wm.win_enable(b, true);
                            }
                        }
                        QmState::PoolSize => {
                            let fmt = h.fetch_text(b"QM:POOLSIZE");
                            let s = h.format_int(&fmt, status.pool_size);
                            let c = h.gamespy_color_default();
                            let w = h.wol_quick_match_menu().quickmatch_text_window;
                            h.gamespy_add_text(wm, &s, c, w);
                        }
                        QmState::WideningSearch => {
                            qm_text(wm, h, "QM:WIDENINGSEARCH");
                            if let Some(b) = widen {
                                wm.win_enable(b, false);
                            }
                        }
                        QmState::Matched => handle_qm_matched(wm, h, status),
                        QmState::InChannel => qm_text(wm, h, "QM:INCHANNEL"),
                        QmState::NegotiatingFirewalls => qm_text(wm, h, "QM:NEGOTIATINGFIREWALLS"),
                        QmState::StartingGame => qm_text(wm, h, "QM:STARTINGGAME"),
                        QmState::CouldNotFindBot => {
                            qm_text(wm, h, "QM:COULDNOTFINDBOT");
                            qm_back_to_start(wm, h);
                        }
                        QmState::CouldNotFindChannel => {
                            qm_text(wm, h, "QM:COULDNOTFINDCHANNEL");
                            qm_back_to_start(wm, h);
                        }
                        QmState::CouldNotNegotiateFirewalls => {
                            qm_text(wm, h, "QM:COULDNOTNEGOTIATEFIREWALLS");
                            qm_back_to_start(wm, h);
                        }
                        QmState::Stopped => {
                            qm_text(wm, h, "QM:STOPPED");
                            qm_back_to_start(wm, h);
                        }
                    }
                }
                QmPeerResponse::Other => {}
            }
        }
    }
}

/// `WOLQuickMatchMenuInput(window, msg, mData1, mData2)`.
pub fn wol_quick_match_menu_input<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        if h.wol_quick_match_menu().button_pushed {
            return MsgHandled::Ignored;
        }
        if (d1 & 0xFF) as u8 == key::ESC {
            // send a simulated selected event to the parent window of the back/exit button
            if (d2 & 0xFF) & key::STATE_UP != 0 {
                let m = h.wol_quick_match_menu().clone();
                if let Some(back) = m.button_back {
                    if !wm.is_hidden(Some(back)) {
                        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(Some(back)), m.button_back_id as u32);
                    }
                }
            }

            // don't let key fall through anywhere else
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `Start`: the `GBM_SELECTED` branch for `buttonStartID` (WOLQuickMatchMenu.cpp:1615).
fn start_quick_match<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.wol_quick_match_menu().clone();
    let mut req = StartQuickMatchRequest::default();
    if let Some(list) = m.listbox_map_select {
        let num_maps = lb::get_num_entries(wm, list);
        for i in 0..num_maps {
            req.qm_maps.push(lb::get_item_data(wm, h, list, i, 0));
        }
    }
    req.max_point_percentage = 100.max(m.max_points);
    req.min_point_percentage = 100.min(m.min_points);
    req.widen_time = 0;

    let mut val = match m.combo_box_max_disconnects {
        Some(c) => cb::get_selected_pos(wm, h, c, 0),
        None => 0,
    };
    if val < 0 {
        val = 0;
    }
    req.max_discons = MAX_DISCONNECTS[val as usize];

    let mut val = match m.combo_box_max_ping {
        Some(c) => cb::get_selected_pos(wm, h, c, 0),
        None => 0,
    };
    if val < 0 {
        val = 0;
    }
    if val >= m.max_ping_entries - 1 {
        req.max_ping = h.gamespy_config_ping_timeout_ms();
    } else {
        req.max_ping = (val + 1) * 100;
    }

    let pid = h.gamespy_local_profile_id();
    let stats = h.ps_find_player_stats_by_id(pid);
    req.points = h.calculate_rank(&stats);

    let mut ladder_index = match m.combo_box_ladder {
        Some(c) => {
            let selected = cb::get_selected_pos(wm, h, c, 0);
            cb::get_item_data(wm, h, c, selected) as i32
        }
        None => 0,
    };
    let mut ladder_info: Option<QmLadderInfo> = None;
    if ladder_index < 0 {
        ladder_index = 0;
    }
    if ladder_index != 0 {
        ladder_info = h.ladder_find_by_index(ladder_index);
        if ladder_info.is_none() {
            ladder_index = 0; // sanity
        }
    }
    req.ladder_id = ladder_index;

    req.ladder_pass_crc = 0;

    let mut index = -1;
    if let Some(c) = m.combo_box_side {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        if selected >= 0 {
            index = cb::get_item_data(wm, h, c, selected) as i32;
        }
    }
    req.side = index;
    if let Some(li) = ladder_info.as_ref().filter(|l| l.random_factions) {
        let side_num = h.game_client_random_value(0, li.valid_factions.len() as i32 - 1);
        // Looking for sideNum out of the random sides
        let cit = li.valid_factions.get(side_num as usize); // the loop advances sideNum times; end() when past
        if let Some(side_str) = cit {
            let templates = h.player_templates();
            for (c, fac) in templates.iter().enumerate() {
                if fac.side == *side_str {
                    req.side = c as i32;
                    break;
                }
            }
        }
    } else if index == PLAYERTEMPLATE_RANDOM {
        // If not a forced random ladder, then we need to resolve our pick of random right now anyway, or else
        // we will get the same pick every darn time.
        let mut random_tries = 0; // Rare to hit Random 10 times in a row, but if it does then random will be converted to a set side by the very bug this tries to fix, so no harm done.

        while random_tries < 10 && index == PLAYERTEMPLATE_RANDOM {
            let number_combo_box_entries = m.combo_box_side.map_or(0, |c| cb::get_length(wm, c));
            let random_pick = h.game_client_random_value(0, number_combo_box_entries - 1);
            if let Some(c) = m.combo_box_side {
                index = cb::get_item_data(wm, h, c, random_pick) as i32;
            }
            req.side = index;

            random_tries += 1;
        }
    }

    let mut index = -1;
    if let Some(c) = m.combo_box_color {
        let selected = cb::get_selected_pos(wm, h, c, 0);
        if selected >= 0 {
            index = cb::get_item_data(wm, h, c, selected) as i32;
        }
    }
    req.color = index;

    req.nat = h.option_prefs_firewall_behavior();

    if ladder_index != 0 {
        req.num_players = match &ladder_info {
            Some(l) => l.players_per_team * 2,
            None => 2,
        };
    } else {
        let mut val = match m.combo_box_num_players {
            Some(c) => cb::get_selected_pos(wm, h, c, 0),
            None => 0,
        };
        if val < 0 {
            val = 0;
        }
        req.num_players = (val + 1) * 2;
    }

    let mut num_discons = 0;
    for v in &stats.discons {
        num_discons += v;
    }
    for v in &stats.desyncs {
        num_discons += v;
    }
    req.discons = num_discons;

    // strncpy(req.QM.pings, ..., 17); pings[16] = 0
    let mut pings = h.gamespy_ping_string();
    pings.truncate(16);
    req.pings = pings;

    req.bot_id = h.gamespy_config_qm_bot_id();
    req.room_id = h.gamespy_config_qm_channel();

    req.exe_crc = h.global_exe_crc();
    req.ini_crc = h.global_ini_crc();

    h.peer_request_start_quick_match(&req);
    if let Some(b) = m.button_widen {
        wm.win_enable(b, false);
    }
    if let Some(b) = m.button_start {
        wm.win_hide(b, true);
    }
    if let Some(b) = m.button_stop {
        wm.win_hide(b, false);
    }
    enable_options_gadgets(wm, h, false);

    if ladder_index > 0 {
        // save the ladder as being played upon even if we cancel out of matching early...
        if let Some(li) = &ladder_info {
            let p = QmLadderPref { last_play_date: h.time_now(), address: li.address.clone(), port: li.port, name: li.name.clone() };
            let profile = h.gamespy_local_profile_id();
            h.ladder_pref_add_recent_and_write(profile, &p);
        }
    }
}

/// Re-sets one map list row to `image` / `color` (`GadgetListBoxAddEntryImage` and `AddEntryText` as the
/// select-all, select-none and row-toggle branches do).
fn set_map_row<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, list: WinId, row: i32, image: Option<Vec<u8>>, height: i32, width: i32, selected: bool) {
    lb::add_entry_image(wm, h, list, image, row, 0, height, width, true, -1);
    lb::set_item_data(wm, h, list, u32::from(selected), row, 0);
    let text = lb::get_text(wm, h, list, row, 1);
    let color = if selected { h.gamespy_color_map_selected() } else { h.gamespy_color_map_unselected() };
    lb::add_entry_text(wm, h, list, &text, color, row, 1, true);
}

/// `WOLQuickMatchMenuSystem(window, msg, mData1, mData2)`.
pub fn wol_quick_match_menu_system<H: WolQuickMatchMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GCM_SELECTED => {
            if h.wol_quick_match_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let pos = cb::get_selected_pos(wm, h, control, -1);

            save_quick_match_options(wm, h);
            let m = h.wol_quick_match_menu().clone();
            if control_id == m.combo_box_ladder_id && !m.is_populating_ladder_box && pos >= 0 {
                h.qm_pref_load();
                let ladder_id = cb::get_item_data(wm, h, control, pos) as i32;
                if ladder_id == 0 {
                    // no ladder selected - enable buttons
                    if let Some(np) = m.combo_box_num_players {
                        let n = (h.qm_pref_num_players() / 2 - 1).max(0);
                        cb::set_selected_pos(wm, h, np, n, false);
                        wm.win_enable(np, true);
                    }
                    let side = h.qm_pref_side();
                    populate_qm_side_combo_box(wm, h, side, None); // this will set side to random and disable if necessary
                } else if ladder_id > 0 {
                    // ladder selected - disable buttons
                    let li = h.ladder_find_by_index(ladder_id);
                    if let Some(np) = m.combo_box_num_players {
                        match &li {
                            Some(li) => cb::set_selected_pos(wm, h, np, li.players_per_team - 1, false),
                            None => cb::set_selected_pos(wm, h, np, 0, false),
                        }
                        wm.win_enable(np, false);
                    }

                    let side = h.qm_pref_side();
                    populate_qm_side_combo_box(wm, h, side, li.as_ref()); // this will set side to random and disable if necessary
                } else {
                    // "Choose a ladder" selected - open overlay
                    populate_qm_ladder_combo_box(wm, h); // this restores the non-"Choose a ladder" selection
                    h.gamespy_open_overlay_ladder_select(wm);
                }
            }
            if !h.wol_quick_match_menu().is_in_init {
                populate_quick_match_map_select_listbox(wm, h);
                update_start_button(wm, h);
            }
        }
        ggm::GBM_SELECTED => {
            if h.wol_quick_match_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let button_options_id = key_of(h, "ButtonOptions");
            let m = h.wol_quick_match_menu().clone();

            if control_id == m.button_stop_id {
                h.peer_request_stop_quick_match();
                if let Some(b) = m.button_widen {
                    wm.win_enable(b, false);
                }
                if let Some(b) = m.button_start {
                    wm.win_hide(b, false);
                }
                if let Some(b) = m.button_stop {
                    wm.win_hide(b, true);
                }
                enable_options_gadgets(wm, h, true);
                qm_text(wm, h, "GUI:QMAborted");
            } else if control_id == button_options_id {
                let win = wm.win_get_window_from_id(m.parent_wol_quick_match, button_options_id);
                if is_info_shown(wm, h) {
                    hide_info_gadgets(wm, h, true);
                    hide_options_gadgets(wm, h, false);
                    let t = h.fetch_text(b"GUI:PlayerInfo");
                    gadget::button_set_text(wm, h, win, &t);
                } else {
                    hide_info_gadgets(wm, h, false);
                    hide_options_gadgets(wm, h, true);
                    let t = h.fetch_text(b"GUI:Setup");
                    gadget::button_set_text(wm, h, win, &t);
                }
            } else if control_id == m.button_widen_id {
                h.peer_request_widen_quick_match_search();
                if let Some(b) = m.button_widen {
                    wm.win_enable(b, false);
                }
            } else if control_id == m.button_start_id {
                start_quick_match(wm, h);
            } else if control_id == m.button_buddies_id {
                h.gamespy_toggle_overlay_buddy(wm);
            } else if control_id == m.button_back_id {
                h.wol_quick_match_menu().button_pushed = true;
                h.gamespy_leave_group_room();
                h.wol_quick_match_menu().next_screen = Some(b"Menus/WOLWelcomeMenu.wnd".to_vec());
                shell::pop(wm, h);
            } else if control_id == m.button_select_all_maps_id {
                if let Some(list) = m.listbox_map_select {
                    let num_maps = lb::get_num_entries(wm, list);
                    for i in 0..num_maps {
                        set_map_row(wm, h, list, i, m.selected_image.clone(), -1, -1, true);
                    }
                }
            } else if control_id == m.button_select_no_maps_id {
                if let Some(list) = m.listbox_map_select {
                    let num_maps = lb::get_num_entries(wm, list);
                    for i in 0..num_maps {
                        set_map_row(wm, h, list, i, m.unselected_image.clone(), -1, -1, false);
                    }
                }
            }
        }
        ggm::GLM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let selected = d2 as i32;
            let m = h.wol_quick_match_menu().clone();

            if control_id == m.listbox_map_select_id {
                let li = get_ladder_info(wm, h);
                if selected >= 0 && li.as_ref().map_or(true, |l| !l.random_maps) {
                    let was_selected = lb::get_item_data(wm, h, control, selected, 0) != 0;
                    let img = if !was_selected { m.selected_image.clone() } else { m.unselected_image.clone() };
                    let (height, width) = map_image_size(wm, h, control, &img);
                    set_map_row(wm, h, control, selected, img, height, width, !was_selected);
                }
                if selected >= 0 {
                    lb::set_selected(wm, h, control, &[-1]);
                }
            }
            update_start_button(wm, h);
        }
        ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
