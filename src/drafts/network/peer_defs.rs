//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#215, group network3): matches the upstream C++ on 20,000 programs (about 800,000 operations; tests/differential_draft_peerdefs.rs,
//! 11 of 11 planted bugs caught): `ascii_compare` (20,000 string pairs), the `GameSpyColors` values, and `GameSpyInfo`'s `new` state, `add_group_room`,
//! `join_group_room`, `leave_group_room`, `join_best_group_room`, `update_player_info`, `player_left_group_room`, the staging-room list members
//! (`clear`, `add`, `update`, `remove`, `has_..._changed`, `find_..._by_id`, `leave_staging_room`), the ignore lists, the saved ignore list, the preorder
//! set, `get_ping_value` (with `grab_hex_int`), `player_is_ignored`, the MOTD, config, ping-string, IP, disallow-text and max-messages accessors.
//! NOT covered: `reset`, `mark_as_staging_room_host`, `mark_as_staging_room_joiner`, `set_game_options`, `update_additional_game_spy_disconnections`,
//! `read_additional_disconnects`, `get_current_staging_room` (its None path only), `set_up_game_spy`, `tear_down_game_spy` (they need the staging room's
//! `GameInfo`, the logic's globals, threads or files).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PeerDefs.h
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PeerDefsImplementation.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/PeerDefs.cpp
//!
//! `GameSpyInfo` (the main-thread side of the online lobby): group rooms, the player info map, buddy lists,
//! staging rooms, the local staging room, ignore lists, ping strings, preorder players, and `SetUpGameSpy` /
//! `TearDownGameSpy`, which create and destroy the queues, the pinger, the config and the ladder list.
//!
//! # Not in this file
//!
//! The chat members of `GameSpyInfo` (`registerTextWindow`, `unregisterTextWindow`, `addText`, `addChat`,
//! `sendChat`) are defined in `GameSpy/Chat.cpp`, not `PeerDefs.cpp`; they are not in the area list of this
//! draft (see the report). `GameSpyColor[]` (the colour table) and `Color` come from the GUI; only the
//! [`GameSpyColors`] indices are here. `GetLocalChatConnectionAddress` and the `AsciiComparator` operator are
//! translated ([`ascii_compare`]).
//!
//! # Upstream quirks kept
//!
//! * `joinBestGroupRoom` bails (and zeroes the current room) when already in a room; its "best" room needs
//!   `minPlayers > 25` as part of the test, so the first non-QM room is taken whatever its size and later ones
//!   only if `numWaiting < minPlayers`, with `minPlayers` starting at 1000 and then set to that room's count:
//!   once a room is picked with 25 or fewer waiting, no other is ever picked.
//! * `getAdditionalDisconnects` logs what it would have returned and returns 0.
//! * `markAsStagingRoomJoiner` copies the joined room's game options through `GameInfoToAsciiString` /
//!   `ParseAsciiStringToGameInfo` and asserts (debug only) that the parse succeeded.
//! * `updateStagingRoom` is `addStagingRoom` (remove then add).
//! * The stats update of `updateAdditionalGameSpyDisconnections` indexes `stats.discons[ptIdx]` with the position
//!   the template search stopped at; if the local template is not found `ptIdx` is the template count.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::common::ascii_string::AsciiString;
use crate::logic::modules::plant;
use crate::common::game_info::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, slot_state, GameInfoHost, PLAYERTEMPLATE_OBSERVER,
};

use super::buddy_thread::{BuddyMessageQueue, GpProfile, GpStatus};
use super::gs_config::GameSpyConfig;
use super::ladder_defs::{LadderHost, LadderList};
use super::net_command_msg::MAX_SLOTS;
use super::peer_thread::{PeerMessageQueue, PeerRequest, PeerRequestType};
use super::persistent_storage_thread::{
    format_player_kv_pairs, PsMessageQueue, PsPlayerStats, PsRequest, PsRequestType, PsResponse, PsResponseType,
};
use super::ping_thread::Pinger;
use super::staging_room_game_info::GameSpyStagingRoom;

const SLOTS: usize = MAX_SLOTS as usize;

/// `enum RCItemType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RcItemType {
    /// `ITEM_BUDDY`.
    Buddy,
    /// `ITEM_REQUEST`.
    Request,
    /// `ITEM_NONBUDDY`.
    NonBuddy,
    /// `ITEM_NONE`.
    None,
}

/// `class GameSpyRCMenuData`.
#[derive(Clone, Debug)]
pub struct GameSpyRcMenuData {
    /// `m_nick`.
    pub nick: AsciiString,
    /// `m_id`.
    pub id: GpProfile,
    /// `m_itemType`.
    pub item_type: RcItemType,
}

/// `class BuddyInfo`.
#[derive(Clone, Debug, Default)]
pub struct BuddyInfo {
    /// `m_id`.
    pub id: GpProfile,
    /// `m_name`.
    pub name: AsciiString,
    /// `m_email`.
    pub email: AsciiString,
    /// `m_countryCode`.
    pub country_code: AsciiString,
    /// `m_status`.
    pub status: GpStatus,
    /// `m_statusString`.
    pub status_string: Vec<u16>,
    /// `m_locationString`.
    pub location_string: Vec<u16>,
}

/// `BuddyInfoMap`.
pub type BuddyInfoMap = BTreeMap<GpProfile, BuddyInfo>;

/// `class BuddyMessage`.
#[derive(Clone, Debug, Default)]
pub struct BuddyMessage {
    /// `m_timestamp`.
    pub timestamp: u32,
    /// `m_senderID`.
    pub sender_id: GpProfile,
    /// `m_senderNick`.
    pub sender_nick: AsciiString,
    /// `m_recipientID`.
    pub recipient_id: GpProfile,
    /// `m_recipientNick`.
    pub recipient_nick: AsciiString,
    /// `m_message`.
    pub message: Vec<u16>,
}

/// `class GameSpyGroupRoom`.
#[derive(Clone, Debug, Default)]
pub struct GameSpyGroupRoom {
    /// `m_name`.
    pub name: AsciiString,
    /// `m_translatedName`.
    pub translated_name: Vec<u16>,
    /// `m_groupID`.
    pub group_id: i32,
    /// `m_numWaiting`.
    pub num_waiting: i32,
    /// `m_maxWaiting`.
    pub max_waiting: i32,
    /// `m_numGames`.
    pub num_games: i32,
    /// `m_numPlaying`.
    pub num_playing: i32,
}

/// `GroupRoomMap`.
pub type GroupRoomMap = BTreeMap<i32, GameSpyGroupRoom>;

/// `class PlayerInfo`.
#[derive(Clone, Debug, Default)]
pub struct PlayerInfo {
    /// `m_name`.
    pub name: AsciiString,
    /// `m_locale`.
    pub locale: AsciiString,
    /// `m_wins`.
    pub wins: i32,
    /// `m_losses`.
    pub losses: i32,
    /// `m_profileID`.
    pub profile_id: i32,
    /// `m_flags`.
    pub flags: i32,
    /// `m_rankPoints`.
    pub rank_points: i32,
    /// `m_side`.
    pub side: i32,
    /// `m_preorder`.
    pub preorder: i32,
}

/// `AsciiComparator::operator()`: `stricmp(s1, s2) < 0`, as an ordering over the bytes up to the first NUL.
pub fn ascii_compare(a: &[u8], b: &[u8]) -> Ordering {
    let cut = |s: &[u8]| -> Vec<u8> {
        let e = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        s[..e].iter().map(|c| if plant(16364) { *c } else { c.to_ascii_lowercase() }).collect()
    };
    cut(a).cmp(&cut(b))
}

/// `PlayerInfoMap` (`std::map<AsciiString, PlayerInfo, AsciiComparator>`): keys compare case-insensitively; the
/// key spelling kept is that of the first insertion, as `std::map` keeps its key when an equal one is assigned.
#[derive(Clone, Debug, Default)]
pub struct PlayerInfoMap {
    map: BTreeMap<Vec<u8>, (AsciiString, PlayerInfo)>,
}

impl PlayerInfoMap {
    fn key(name: &AsciiString) -> Vec<u8> {
        let s = name.str();
        let e = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        s[..e].iter().map(|c| if plant(16371) { *c } else { c.to_ascii_lowercase() }).collect()
    }
    /// `find`.
    pub fn find(&self, name: &AsciiString) -> Option<&PlayerInfo> {
        self.map.get(&Self::key(name)).map(|(_, p)| p)
    }
    /// `operator[]=`: replaces the value, keeps the first key.
    pub fn insert(&mut self, name: AsciiString, pi: PlayerInfo) {
        let k = Self::key(&name);
        match self.map.get_mut(&k) {
            Some(e) => e.1 = pi,
            None => {
                self.map.insert(k, (name, pi));
            }
        }
    }
    /// `erase`.
    pub fn erase(&mut self, name: &AsciiString) {
        self.map.remove(&Self::key(name));
    }
    /// `clear`.
    pub fn clear(&mut self) {
        self.map.clear();
    }
    /// Iteration in `AsciiComparator` order.
    pub fn iter(&self) -> impl Iterator<Item = (&AsciiString, &PlayerInfo)> {
        self.map.values().map(|(k, v)| (k, v))
    }
}

/// `enum GameSpyColors`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSpyColors {
    Default,
    CurrentRoom,
    Room,
    Game,
    GameFull,
    GameCrcMismatch,
    PlayerNormal,
    PlayerOwner,
    PlayerBuddy,
    PlayerSelf,
    PlayerIgnored,
    ChatNormal,
    ChatEmote,
    ChatOwner,
    ChatOwnerEmote,
    ChatPrivate,
    ChatPrivateEmote,
    ChatPrivateOwner,
    ChatPrivateOwnerEmote,
    ChatBuddy,
    ChatSelf,
    AcceptTrue,
    AcceptFalse,
    MapSelected,
    MapUnselected,
    Motd,
    MotdHeading,
}

/// `enum GameSpyBuddyStatus` is [`super::peer_thread::BuddyStatus`].
pub use super::peer_thread::BuddyStatus as GameSpyBuddyStatus;

/// What `GameSpyInfo` and `SetUpGameSpy` reach outside the lobby state.
pub trait PeerDefsHost: GameInfoHost {
    // ---- text, boxes ----
    /// `TheGameText->fetch(label, &exists)`.
    fn game_text_fetch_exists(&self, label: &[u8]) -> (bool, Vec<u16>);
    /// `GSMessageBoxOk(fetch("GUI:Error"), fetch(text_label), NULL)`.
    fn gs_message_box_error(&mut self, text_label: &str);
    // ---- config ----
    /// `TheGameSpyConfig->getQMChannel()`.
    fn config_qm_channel(&self) -> i32;
    /// `TheGameSpyConfig->setQMChannel(id)`.
    fn config_set_qm_channel(&mut self, id: i32);
    /// `TheGameSpyConfig->getPingTimeoutInMs()`.
    fn config_ping_timeout_ms(&self) -> i32;
    // ---- queues ----
    /// `TheGameSpyPeerMessageQueue`.
    fn peer_queue(&self) -> PeerMessageQueue;
    /// `TheGameSpyPSMessageQueue`.
    fn ps_queue(&self) -> PsMessageQueue;
    // ---- maps ----
    /// `getDefaultMap(TRUE)`.
    fn default_map(&mut self) -> Vec<u8>;
    /// `TheMapCache->updateCache()`.
    fn map_cache_update(&mut self);
    // ---- ignore prefs ----
    /// `IgnorePreferences pref; pref.setIgnore(nick, profileID, ignore); pref.write()`.
    fn ignore_prefs_set_and_write(&mut self, nick: &[u8], profile_id: i32, ignore: bool);
    /// `IgnorePreferences::getIgnores()`.
    fn ignore_prefs_get(&self) -> BTreeMap<i32, AsciiString>;
    // ---- disconnect bookkeeping ----
    /// `GetAdditionalDisconnectsFromUserFile(playerID)`.
    fn additional_disconnects_from_user_file(&mut self, profile_id: i32) -> i32;
    /// `GameSpyMiscPreferences mPref; mPref.setCachedStats(kv); mPref.write()`.
    fn misc_prefs_set_cached_stats_and_write(&mut self, kv: &[u8]);
    /// `GetTickCount()` for `markAsStagingRoomHost`'s seed (same as `GameInfoHost::tick_count`).
    fn get_tick_count(&mut self) -> u32 {
        self.tick_count()
    }
}

/// The local-player facts `updateAdditionalGameSpyDisconnections` reads from the logic.
#[derive(Clone, Debug, Default)]
pub struct LocalPlayerFacts {
    /// `TheRecorder->isMultiplayer() && TheGameLogic->isInInternetGame()`.
    pub in_internet_multiplayer: bool,
    /// The position of the local player's template in the store (`ptIdx` after the search loop).
    pub template_index: i32,
    /// `myTemplate->isObserver()`.
    pub template_is_observer: bool,
    /// `player->getPlayerType() != PLAYER_HUMAN`.
    pub player_is_not_human: bool,
    /// `player->isPlayerObserver()`.
    pub player_is_observer: bool,
    /// Any slot of `TheGameInfo` is an AI.
    pub any_ai: bool,
}

/// `class GameSpyInfo`.
#[derive(Clone, Debug)]
pub struct GameSpyInfo {
    /// `m_sawFullGameList`.
    saw_full_game_list: bool,
    /// `m_isDisconAfterGameStart`.
    is_discon_after_game_start: bool,
    /// `m_disconReason`.
    discon_reason: i32,
    /// `m_rawMotd`.
    raw_motd: AsciiString,
    /// `m_rawConfig`.
    raw_config: AsciiString,
    /// `m_pingString`.
    ping_string: AsciiString,
    /// `m_groupRooms`.
    group_rooms: GroupRoomMap,
    /// `m_stagingRooms`.
    staging_rooms: BTreeMap<i32, GameSpyStagingRoom>,
    /// `m_stagingRoomsDirty`.
    staging_rooms_dirty: bool,
    /// `m_buddyMap`.
    buddy_map: BuddyInfoMap,
    /// `m_buddyRequestMap`.
    buddy_request_map: BuddyInfoMap,
    /// `m_playerInfoMap`.
    player_info_map: PlayerInfoMap,
    /// `m_buddyMessages`.
    buddy_messages: Vec<BuddyMessage>,
    /// `m_currentGroupRoomID`.
    current_group_room_id: i32,
    /// `m_gotGroupRoomList`.
    got_group_room_list: bool,
    /// `m_localName`.
    local_name: AsciiString,
    /// `m_localProfileID`.
    local_profile_id: i32,
    /// `m_localPasswd`.
    local_passwd: AsciiString,
    /// `m_localEmail`.
    local_email: AsciiString,
    /// `m_localBaseName`.
    local_base_name: AsciiString,
    /// `m_cachedLocalPlayerStats`.
    cached_local_player_stats: PsPlayerStats,
    /// `m_disallowAsainText`.
    disallow_asian_text: bool,
    /// `m_disallowNonAsianText`.
    disallow_non_asian_text: bool,
    /// `m_internalIP`.
    internal_ip: u32,
    /// `m_externalIP`.
    external_ip: u32,
    /// `m_maxMessagesPerUpdate`.
    max_messages_per_update: i32,
    /// `m_joinedStagingRoom`.
    joined_staging_room: i32,
    /// `m_isHosting`.
    is_hosting: bool,
    /// `m_localStagingRoom` (`TheGameSpyGame` points here).
    pub local_staging_room: GameSpyStagingRoom,
    /// `m_localStagingRoomID`.
    local_staging_room_id: i32,
    /// `m_ignoreList`.
    ignore_list: BTreeSet<Vec<u8>>,
    /// `m_savedIgnoreMap`.
    saved_ignore_map: BTreeMap<i32, AsciiString>,
    /// `m_preorderPlayers`.
    preorder_players: BTreeSet<i32>,
    /// `m_additionalDisconnects`.
    additional_disconnects: i32,
}

impl GameSpyInfo {
    /// `GameSpyInfo::GameSpyInfo()` (PeerDefs.cpp): `reset()` and `TheGameSpyGame = &m_localStagingRoom`;
    /// `m_isDisconAfterGameStart = FALSE`.
    pub fn new<H: PeerDefsHost>(host: &mut H) -> GameSpyInfo {
        let mut s = GameSpyInfo {
            saw_full_game_list: false,
            is_discon_after_game_start: false,
            discon_reason: 0,
            raw_motd: AsciiString::default(),
            raw_config: AsciiString::default(),
            ping_string: AsciiString::default(),
            group_rooms: GroupRoomMap::new(),
            staging_rooms: BTreeMap::new(),
            staging_rooms_dirty: false,
            buddy_map: BuddyInfoMap::new(),
            buddy_request_map: BuddyInfoMap::new(),
            player_info_map: PlayerInfoMap::default(),
            buddy_messages: Vec::new(),
            current_group_room_id: 0,
            got_group_room_list: false,
            local_name: AsciiString::default(),
            local_profile_id: 0,
            local_passwd: AsciiString::default(),
            local_email: AsciiString::default(),
            local_base_name: AsciiString::default(),
            cached_local_player_stats: PsPlayerStats::new(),
            disallow_asian_text: false,
            disallow_non_asian_text: false,
            internal_ip: 0,
            external_ip: 0,
            max_messages_per_update: 100,
            joined_staging_room: 0,
            is_hosting: false,
            local_staging_room: GameSpyStagingRoom::new(host),
            local_staging_room_id: 0,
            ignore_list: BTreeSet::new(),
            saved_ignore_map: BTreeMap::new(),
            preorder_players: BTreeSet::new(),
            additional_disconnects: -1,
        };
        s.reset(host);
        s.is_discon_after_game_start = false;
        s
    }

    /// `GameSpyInfo::reset` (PeerDefs.cpp): forgets the rooms, buddies, local staging room, names, ignore and
    /// preorder sets and the cached stats; `m_maxMessagesPerUpdate = 100`, `m_additionalDisconnects = -1`. The
    /// ignore list `m_ignoreList` is not cleared (not in upstream's reset).
    pub fn reset<H: PeerDefsHost>(&mut self, host: &mut H) {
        self.saw_full_game_list = false;
        self.is_discon_after_game_start = false;
        self.current_group_room_id = 0;
        self.clear_group_room_list();
        self.clear_staging_room_list();
        self.local_staging_room_id = 0;
        self.buddy_request_map.clear();
        self.buddy_map.clear();
        self.buddy_messages.clear();
        self.joined_staging_room = 0;
        self.is_hosting = false;
        self.local_staging_room_id = 0;
        self.local_staging_room.reset(host);
        self.got_group_room_list = false;
        self.local_name = AsciiString::default();
        self.local_profile_id = 0;
        self.max_messages_per_update = 100;

        self.disallow_asian_text = false;
        self.disallow_non_asian_text = false;
        self.discon_reason = 0;
        self.local_base_name.clear();
        self.local_email.clear();
        self.local_passwd.clear();
        self.ping_string.clear();
        self.raw_config.clear();
        self.raw_motd.clear();

        self.internal_ip = 0;
        self.external_ip = 0;

        self.saved_ignore_map.clear();
        self.preorder_players.clear();

        self.cached_local_player_stats.reset();

        self.additional_disconnects = -1;
    }

    // ---- simple accessors (PeerDefsImplementation.h) ----
    /// `clearGroupRoomList`.
    pub fn clear_group_room_list(&mut self) {
        self.group_rooms.clear();
        self.got_group_room_list = false;
    }
    /// `getGroupRoomList`.
    pub fn get_group_room_list(&mut self) -> &mut GroupRoomMap {
        &mut self.group_rooms
    }
    /// `gotGroupRoomList`.
    pub fn got_group_room_list(&self) -> bool {
        self.got_group_room_list
    }
    /// `setCurrentGroupRoom`: also forgets the player info map.
    pub fn set_current_group_room(&mut self, group_id: i32) {
        self.current_group_room_id = group_id;
        self.player_info_map.clear();
    }
    /// `getCurrentGroupRoom`.
    pub fn get_current_group_room(&self) -> i32 {
        self.current_group_room_id
    }
    /// `getPlayerInfoMap`.
    pub fn get_player_info_map(&mut self) -> &mut PlayerInfoMap {
        &mut self.player_info_map
    }
    /// `setLocalName`.
    pub fn set_local_name(&mut self, name: AsciiString) {
        self.local_name = name;
    }
    /// `getLocalName`.
    pub fn get_local_name(&self) -> AsciiString {
        self.local_name.clone()
    }
    /// `setLocalProfileID`.
    pub fn set_local_profile_id(&mut self, id: i32) {
        self.local_profile_id = id;
    }
    /// `getLocalProfileID`.
    pub fn get_local_profile_id(&self) -> i32 {
        self.local_profile_id
    }
    /// `getLocalEmail`.
    pub fn get_local_email(&self) -> AsciiString {
        self.local_email.clone()
    }
    /// `setLocalEmail`.
    pub fn set_local_email(&mut self, v: AsciiString) {
        self.local_email = v;
    }
    /// `getLocalPassword`.
    pub fn get_local_password(&self) -> AsciiString {
        self.local_passwd.clone()
    }
    /// `setLocalPassword`.
    pub fn set_local_password(&mut self, v: AsciiString) {
        self.local_passwd = v;
    }
    /// `setLocalBaseName`.
    pub fn set_local_base_name(&mut self, v: AsciiString) {
        self.local_base_name = v;
    }
    /// `getLocalBaseName`.
    pub fn get_local_base_name(&self) -> AsciiString {
        self.local_base_name.clone()
    }
    /// `setCachedLocalPlayerStats`.
    pub fn set_cached_local_player_stats(&mut self, stats: PsPlayerStats) {
        self.cached_local_player_stats = stats;
    }
    /// `getCachedLocalPlayerStats` (a copy through the copy constructor).
    pub fn get_cached_local_player_stats(&self) -> PsPlayerStats {
        PsPlayerStats::copy_constructed(&self.cached_local_player_stats)
    }
    /// `getBuddyMap`.
    pub fn get_buddy_map(&mut self) -> &mut BuddyInfoMap {
        &mut self.buddy_map
    }
    /// `getBuddyRequestMap`.
    pub fn get_buddy_request_map(&mut self) -> &mut BuddyInfoMap {
        &mut self.buddy_request_map
    }
    /// `getBuddyMessages`.
    pub fn get_buddy_messages(&mut self) -> &mut Vec<BuddyMessage> {
        &mut self.buddy_messages
    }
    /// `isBuddy`.
    pub fn is_buddy(&self, id: i32) -> bool {
        self.buddy_map.contains_key(&id)
    }
    /// `getStagingRoomList`.
    pub fn get_staging_room_list(&mut self) -> &mut BTreeMap<i32, GameSpyStagingRoom> {
        &mut self.staging_rooms
    }
    /// `getCurrentStagingRoomID`.
    pub fn get_current_staging_room_id(&self) -> i32 {
        self.local_staging_room_id
    }
    /// `sawFullGameList`.
    pub fn saw_full_game_list(&mut self) {
        self.saw_full_game_list = true;
    }
    /// `setDisallowAsianText`.
    pub fn set_disallow_asian_text(&mut self, val: bool) {
        self.disallow_asian_text = val;
    }
    /// `setDisallowNonAsianText`.
    pub fn set_disallow_non_asian_text(&mut self, val: bool) {
        self.disallow_non_asian_text = val;
    }
    /// `getDisallowAsianText`.
    pub fn get_disallow_asian_text(&self) -> bool {
        self.disallow_asian_text
    }
    /// `getDisallowNonAsianText`.
    pub fn get_disallow_non_asian_text(&self) -> bool {
        self.disallow_non_asian_text
    }
    /// `setPingString`.
    pub fn set_ping_string(&mut self, ping: AsciiString) {
        self.ping_string = ping;
    }
    /// `getPingString`.
    pub fn get_ping_string(&self) -> AsciiString {
        self.ping_string.clone()
    }
    /// `setMOTD`.
    pub fn set_motd(&mut self, motd: AsciiString) {
        self.raw_motd = motd;
    }
    /// `getMOTD`.
    pub fn get_motd(&self) -> AsciiString {
        self.raw_motd.clone()
    }
    /// `setConfig`.
    pub fn set_config(&mut self, config: AsciiString) {
        self.raw_config = config;
    }
    /// `getConfig`.
    pub fn get_config(&self) -> AsciiString {
        self.raw_config.clone()
    }
    /// `getInternalIP`.
    pub fn get_internal_ip(&self) -> u32 {
        self.internal_ip
    }
    /// `getExternalIP`.
    pub fn get_external_ip(&self) -> u32 {
        self.external_ip
    }
    /// `setLocalIPs`.
    pub fn set_local_ips(&mut self, internal_ip: u32, external_ip: u32) {
        self.internal_ip = internal_ip;
        self.external_ip = external_ip;
    }
    /// `isDisconnectedAfterGameStart(reason)`: (flag, reason).
    pub fn is_disconnected_after_game_start(&self) -> (bool, i32) {
        (self.is_discon_after_game_start, self.discon_reason)
    }
    /// `markAsDisconnectedAfterGameStart`.
    pub fn mark_as_disconnected_after_game_start(&mut self, reason: i32) {
        self.is_discon_after_game_start = true;
        self.discon_reason = reason;
    }
    /// `didPlayerPreorder`.
    pub fn did_player_preorder(&self, profile_id: i32) -> bool {
        self.preorder_players.contains(&profile_id)
    }
    /// `markPlayerAsPreorder`.
    pub fn mark_player_as_preorder(&mut self, profile_id: i32) {
        self.preorder_players.insert(profile_id);
    }
    /// `setMaxMessagesPerUpdate`.
    pub fn set_max_messages_per_update(&mut self, num: i32) {
        self.max_messages_per_update = num;
    }
    /// `getMaxMessagesPerUpdate`.
    pub fn get_max_messages_per_update(&self) -> i32 {
        self.max_messages_per_update
    }
    /// `amIHost`.
    pub fn am_i_host(&self) -> bool {
        self.is_hosting
    }

    // ---- additional disconnects ----
    /// `readAdditionalDisconnects`.
    pub fn read_additional_disconnects<H: PeerDefsHost>(&mut self, host: &mut H) {
        self.additional_disconnects = host.additional_disconnects_from_user_file(self.local_profile_id);
    }
    /// `getAdditionalDisconnects`: logs what it would have returned and returns 0.
    pub fn get_additional_disconnects(&self) -> i32 {
        0
    }
    /// `clearAdditionalDisconnects`.
    pub fn clear_additional_disconnects(&mut self) {
        self.additional_disconnects = 0;
    }

    /// `getCurrentStagingRoom`: the local staging room if hosting or joined, otherwise the listed room whose id is
    /// `m_joinedStagingRoom` (which is 0 here, so normally None).
    pub fn get_current_staging_room(&mut self) -> Option<&mut GameSpyStagingRoom> {
        if self.is_hosting || self.joined_staging_room != 0 {
            return Some(&mut self.local_staging_room);
        }
        let id = self.joined_staging_room;
        self.staging_rooms.get_mut(&id)
    }

    /// `setGameOptions` (PeerDefs.cpp): when hosting, queues `PEERREQUEST_SETGAMEOPTIONS` with the staging room's
    /// option string, its portable map name (backslashes to slashes), per-slot player names and the wins, losses,
    /// profile ids, factions and colours of the humans (observers counted apart) and AIs (named CE/CM/CH, profile id
    /// = slot state), with `maxPlayers` = open slots + players + observers; then queues a `Pings/` UTM of the
    /// per-slot ping strings ("0" for non-humans).
    pub fn set_game_options<H: PeerDefsHost>(&mut self, host: &mut H) {
        if !self.is_hosting {
            return;
        }
        let mut req = PeerRequest::default();
        req.peer_request_type = PeerRequestType::SetGameOptions;
        req.options = game_info_to_ascii_string(Some(&self.local_staging_room.info), host).unwrap_or_default();

        let map_name = host.real_map_path_to_portable(&self.local_staging_room.info.map_name);
        let mut new_map_name = Vec::new();
        for &c in map_name.iter() {
            new_map_name.push(if c != b'\\' { c } else { b'/' });
        }
        req.game_opts_map_name = new_map_name;

        req.game_options.num_players = 0;
        req.game_options.num_observers = 0;
        let mut num_open_slots = 0;
        for i in 0..SLOTS {
            let mut wins = 0;
            let mut losses = 0;
            let mut profile_id = 0;
            req.game_opts_player_names[i] = Vec::new();
            let slot = &self.local_staging_room.info.slots[i];
            if !slot.is_occupied() {
                if slot.is_open() {
                    num_open_slots += 1;
                }
            } else {
                let mut player_name = AsciiString::default();
                let idx = (req.game_options.num_observers + req.game_options.num_players) as usize;
                if slot.is_human() {
                    let _ = player_name.translate(&slot.name);
                    req.game_opts_player_names[i] = player_name.str().to_vec();
                    if let Some(pi) = self.player_info_map.find(&player_name) {
                        wins = pi.wins;
                        losses = pi.losses;
                        profile_id = pi.profile_id;
                    }
                    req.game_options.wins[idx] = wins;
                    req.game_options.losses[idx] = losses;
                    req.game_options.profile_id[idx] = profile_id;
                    req.game_options.faction[idx] = slot.player_template;
                    req.game_options.color[idx] = slot.color;
                    if slot.player_template == PLAYERTEMPLATE_OBSERVER {
                        req.game_options.num_observers += 1;
                    } else {
                        req.game_options.num_players += 1;
                    }
                } else if slot.is_ai() {
                    // add in AI players
                    let name: &[u8] = match slot.state {
                        slot_state::EASY_AI => b"CE",
                        slot_state::MED_AI => b"CM",
                        slot_state::BRUTAL_AI => b"CH",
                        _ => b"",
                    };
                    // name is unused - we go off of the profileID
                    req.game_opts_player_names[i] = name.to_vec();
                    req.game_options.wins[idx] = 0;
                    req.game_options.losses[idx] = 0;
                    req.game_options.profile_id[idx] = slot.state;
                    req.game_options.faction[idx] = slot.player_template;
                    req.game_options.color[idx] = slot.color;
                    req.game_options.num_players += 1;
                }
            }
        }
        req.game_options.max_players = num_open_slots + req.game_options.num_players + req.game_options.num_observers;
        let peer = host.peer_queue();
        peer.add_request(req.clone());

        req.peer_request_type = PeerRequestType::UtmRoom;
        req.utm_is_staging_room = true;
        req.id = b"Pings/".to_vec();
        let mut pings: Vec<u8> = Vec::new();
        for i in 0..SLOTS {
            if i != 0 {
                pings.push(b',');
            }
            if self.local_staging_room.info.slots[i].is_human() {
                pings.extend_from_slice(self.local_staging_room.gs_slots[i].ping_str.str());
            } else {
                pings.push(b'0');
            }
        }
        req.options = pings;
        peer.add_request(req);
    }

    /// `addGroupRoom` (PeerDefs.cpp): `groupID == 0` is the end-of-list marker: marks the list as received and, if
    /// the localized `GUI:<name>` labels exist for some but not all non-quick-match rooms, renames every such room
    /// "<label> <n>" cycling through the labels; otherwise stores the room with its translated name and notes the
    /// quick-match channel when the room is called "quickmatch".
    pub fn add_group_room<H: PeerDefsHost>(&mut self, host: &mut H, mut room: GameSpyGroupRoom) {
        if room.group_id == 0 {
            self.got_group_room_list = true;

            // figure out how many good strings we've got
            let mut names: Vec<Vec<u16>> = Vec::new();
            let mut num_rooms = 0usize;
            let qm = host.config_qm_channel();
            for (_, r) in self.group_rooms.iter() {
                if r.group_id != qm {
                    num_rooms += 1;
                    let mut label = b"GUI:".to_vec();
                    label.extend_from_slice(r.name.str());
                    let (exists, group_name) = host.game_text_fetch_exists(&label);
                    if exists {
                        names.push(group_name);
                    }
                }
            }

            if !names.is_empty() && names.len() != num_rooms {
                // didn't get all names.  fix up
                let mut name_index = 0usize;
                let mut times_through = 1; // start with USA Lobby 1
                let ids: Vec<i32> = self.group_rooms.keys().copied().collect();
                for id in ids {
                    let mut r = self.group_rooms.get(&id).cloned().unwrap_or_default();
                    if r.group_id != qm {
                        let mut t = names[name_index].clone();
                        t.push(b' ' as u16);
                        t.extend(times_through.to_string().encode_utf16());
                        r.translated_name = t;
                        name_index = (name_index + 1) % names.len();
                        self.group_rooms.insert(r.group_id, r);
                        if name_index == 0 {
                            // we've looped through the name list already.  increment the timesThrough counter
                            times_through += 1;
                        }
                    }
                }
            }
        } else {
            let mut label = b"GUI:".to_vec();
            label.extend_from_slice(room.name.str());
            let (_exists, text) = host.game_text_fetch_exists(&label);
            room.translated_name = text;
            let is_qm = if plant(16366) { room.name.str() == b"quickmatch" } else { room.name.str().eq_ignore_ascii_case(b"quickmatch") };
            let gid = room.group_id;
            self.group_rooms.insert(gid, room);
            if is_qm {
                host.config_set_qm_channel(gid);
            }
        }
    }

    /// `joinGroupRoom`: for a positive id queues `PEERREQUEST_JOINGROUPROOM` and forgets the player info map.
    pub fn join_group_room<H: PeerDefsHost>(&mut self, host: &mut H, group_id: i32) {
        if group_id > 0 {
            let mut req = PeerRequest::default();
            req.peer_request_type = PeerRequestType::JoinGroupRoom;
            req.group_room_id = group_id;
            host.peer_queue().add_request(req);
            self.player_info_map.clear();
        }
    }

    /// `leaveGroupRoom`: queues `PEERREQUEST_LEAVEGROUPROOM`, leaves the current room, forgets the players.
    pub fn leave_group_room<H: PeerDefsHost>(&mut self, host: &mut H) {
        let mut req = PeerRequest::default();
        req.peer_request_type = PeerRequestType::LeaveGroupRoom;
        host.peer_queue().add_request(req);
        if !plant(16370) {
            self.set_current_group_room(0);
        }
        self.player_info_map.clear();
    }

    /// `joinBestGroupRoom`: see the module doc; with no room to join shows the `GUI:GSGroupRoomJoinFail` error.
    pub fn join_best_group_room<H: PeerDefsHost>(&mut self, host: &mut H) {
        if self.current_group_room_id != 0 {
            // Bailing - we were already in a room
            self.current_group_room_id = 0;
            return;
        }

        if !self.group_rooms.is_empty() {
            let mut min_id: i32 = -1;
            let mut min_players: i32 = 1000;
            let qm = host.config_qm_channel();
            for (_, room) in self.group_rooms.iter() {
                if qm != room.group_id && (plant(16365) || min_players > 25) && room.num_waiting < min_players {
                    min_id = room.group_id;
                    min_players = room.num_waiting;
                }
            }

            if min_id > 0 {
                let mut req = PeerRequest::default();
                req.peer_request_type = PeerRequestType::JoinGroupRoom;
                req.group_room_id = min_id;
                host.peer_queue().add_request(req);
                self.player_info_map.clear();
            } else {
                host.gs_message_box_error("GUI:GSGroupRoomJoinFail");
            }
        } else {
            host.gs_message_box_error("GUI:GSGroupRoomJoinFail");
        }
    }

    /// `updatePlayerInfo(pi, oldNick)`: a rename first removes the old nick; stores the info under its name and
    /// notes a preorder player.
    pub fn update_player_info(&mut self, pi: PlayerInfo, old_nick: &AsciiString) {
        if !old_nick.is_empty() {
            self.player_left_group_room(old_nick);
        }
        let name = pi.name.clone();
        let preorder = pi.preorder;
        let pid = pi.profile_id;
        self.player_info_map.insert(name, pi);
        if if plant(16367) { preorder > 0 } else { preorder != 0 } {
            self.mark_player_as_preorder(pid);
        }
    }

    /// `playerLeftGroupRoom`.
    pub fn player_left_group_room(&mut self, nick: &AsciiString) {
        self.player_info_map.erase(nick);
    }

    /// `clearStagingRoomList`: not "saw the full list", not dirty, no rooms. (The commented-out dirty flag only
    /// applies "if we actually removed some games".)
    pub fn clear_staging_room_list(&mut self) {
        self.saw_full_game_list = false;
        self.staging_rooms_dirty = false;
        self.staging_rooms.clear();
    }

    /// `addStagingRoom`: removes any room with the id, stores a copy and marks the list dirty if the full list
    /// was already seen.
    pub fn add_staging_room(&mut self, room: GameSpyStagingRoom) {
        self.remove_staging_room_id(room.get_id());
        let id = room.get_id();
        let mut new_room = room;
        new_room.clean_up_slot_pointers();
        self.staging_rooms.insert(id, new_room);
        self.staging_rooms_dirty = if plant(16368) { true } else { self.saw_full_game_list };
    }

    /// `updateStagingRoom`: `addStagingRoom`.
    pub fn update_staging_room(&mut self, room: GameSpyStagingRoom) {
        self.add_staging_room(room);
    }

    fn remove_staging_room_id(&mut self, id: i32) {
        if self.staging_rooms.remove(&id).is_some() {
            self.staging_rooms_dirty = self.saw_full_game_list;
        }
    }

    /// `removeStagingRoom(room)`: by the room's id.
    pub fn remove_staging_room(&mut self, room: &GameSpyStagingRoom) {
        self.remove_staging_room_id(room.get_id());
    }

    /// `hasStagingRoomListChanged`: the dirty flag, cleared by the read.
    pub fn has_staging_room_list_changed(&mut self) -> bool {
        let val = self.staging_rooms_dirty;
        self.staging_rooms_dirty = false;
        val
    }

    /// `findStagingRoomByID`.
    pub fn find_staging_room_by_id(&mut self, id: i32) -> Option<&mut GameSpyStagingRoom> {
        self.staging_rooms.get_mut(&id)
    }

    /// `leaveStagingRoom`: queues `PEERREQUEST_LEAVESTAGINGROOM`, forgets the players, not joined, not hosting.
    pub fn leave_staging_room<H: PeerDefsHost>(&mut self, host: &mut H) {
        self.local_staging_room_id = 0;
        let mut req = PeerRequest::default();
        req.peer_request_type = PeerRequestType::LeaveStagingRoom;
        host.peer_queue().add_request(req);
        self.player_info_map.clear();
        self.joined_staging_room = 0;
        self.is_hosting = false;
    }

    /// `markAsStagingRoomHost`: becomes the host; resets the local room (keeping the options carried over from the
    /// create dialog: stats and old-factions-only), seeds it with `GetTickCount()`, puts the local player in slot 0
    /// at the external IP with the default map and closes the slots the map cannot hold.
    pub fn mark_as_staging_room_host<H: PeerDefsHost>(&mut self, host: &mut H) {
        self.local_staging_room_id = 0;
        self.joined_staging_room = 0;
        self.is_hosting = true;

        // There are a few options we don't want to reset when we are hosting (they carry over from the create
        // game dialog).
        let use_stats = self.local_staging_room.info.use_stats;
        let old_factions_only = self.local_staging_room.info.old_factions_only;

        self.local_staging_room.reset(host);
        self.local_staging_room.info.enter_game(host);
        self.local_staging_room.info.seed = host.get_tick_count() as i32;

        self.local_staging_room.info.use_stats = use_stats;
        self.local_staging_room.info.old_factions_only = old_factions_only;

        let mut new_slot = crate::common::game_info::GameSlot::new();
        let mut u_name = Vec::new();
        for &c in self.local_name.str() {
            u_name.push(c as u16);
        }
        new_slot.set_state(slot_state::PLAYER, &u_name, 0, host);

        self.local_staging_room.info.local_ip = self.external_ip;
        new_slot.ip = self.external_ip;

        self.local_staging_room.info.set_slot(0, new_slot);
        self.local_staging_room.set_local_name(self.local_name.clone());

        host.map_cache_update();
        let default_map = host.default_map();
        let _ = self.local_staging_room.info.set_map(&default_map, host);
        let _ = self.local_staging_room.info.adjust_slots_for_map(host); // close slots that the map can't hold. BGC
    }

    /// `markAsStagingRoomJoiner(game)`: becomes a joiner of listed room `game`; copies that room's options into the
    /// local room by formatting and re-parsing them, then takes the room's CRCs, observer and password flags and
    /// name.
    pub fn mark_as_staging_room_joiner<H: PeerDefsHost>(&mut self, host: &mut H, game: i32) {
        self.local_staging_room_id = game;
        self.joined_staging_room = 1; // TRUE
        self.is_hosting = false;
        self.local_staging_room.reset(host);
        self.local_staging_room.info.enter_game(host);
        if let Some(info) = self.staging_rooms.get_mut(&game) {
            info.clean_up_slot_pointers();
            let options = game_info_to_ascii_string(Some(&info.info), host).unwrap_or_default();
            let exe_crc = info.get_exe_crc();
            let ini_crc = info.get_ini_crc();
            let allow_obs = info.get_allow_observers();
            let has_pw = info.get_has_password();
            let name = info.get_game_name();
            let _res = parse_ascii_string_to_game_info(Some(&mut self.local_staging_room.info), &options, host);
            self.local_staging_room.info.in_game = true; // setInGame()
            self.local_staging_room.set_local_name(self.local_name.clone());
            self.local_staging_room.set_exe_crc(exe_crc);
            self.local_staging_room.set_ini_crc(ini_crc);
            self.local_staging_room.set_allow_observers(allow_obs);
            self.local_staging_room.set_has_password(has_pw);
            self.local_staging_room.set_game_name(name);
        }
    }

    // ---- ignore lists ----
    /// `addToIgnoreList`.
    pub fn add_to_ignore_list(&mut self, nick: &AsciiString) {
        self.ignore_list.insert(nick.str().to_vec());
    }
    /// `removeFromIgnoreList`.
    pub fn remove_from_ignore_list(&mut self, nick: &AsciiString) {
        self.ignore_list.remove(nick.str());
    }
    /// `isIgnored`.
    pub fn is_ignored(&self, nick: &AsciiString) -> bool {
        self.ignore_list.contains(nick.str())
    }
    /// `returnIgnoreList`.
    pub fn return_ignore_list(&self) -> BTreeSet<Vec<u8>> {
        self.ignore_list.clone()
    }
    /// `addToSavedIgnoreList`: remembers and writes the ignore preference.
    pub fn add_to_saved_ignore_list<H: PeerDefsHost>(&mut self, host: &mut H, profile_id: i32, nick: AsciiString) {
        self.saved_ignore_map.insert(profile_id, nick.clone());
        host.ignore_prefs_set_and_write(nick.str(), profile_id, true);
    }
    /// `removeFromSavedIgnoreList`: forgets and writes (with an empty nick).
    pub fn remove_from_saved_ignore_list<H: PeerDefsHost>(&mut self, host: &mut H, profile_id: i32) {
        self.saved_ignore_map.remove(&profile_id);
        host.ignore_prefs_set_and_write(b"", profile_id, plant(16369));
    }
    /// `isSavedIgnored`.
    pub fn is_saved_ignored(&self, profile_id: i32) -> bool {
        self.saved_ignore_map.contains_key(&profile_id)
    }
    /// `returnSavedIgnoreList`.
    pub fn return_saved_ignore_list(&self) -> BTreeMap<i32, AsciiString> {
        self.saved_ignore_map.clone()
    }
    /// `loadSavedIgnoreList`: replaces the saved map with the preferences'.
    pub fn load_saved_ignore_list<H: PeerDefsHost>(&mut self, host: &H) {
        self.saved_ignore_map.clear();
        self.saved_ignore_map = host.ignore_prefs_get();
    }
    /// `PlayerInfo::isIgnored`: by profile id when there is one, else by nick.
    pub fn player_is_ignored(&self, pi: &PlayerInfo) -> bool {
        if pi.profile_id != 0 {
            self.is_saved_ignored(pi.profile_id)
        } else {
            self.is_ignored(&pi.name)
        }
    }

    /// `getPingValue(otherPing)`: both ping strings are pairs of hex digits per ping server; the best (lowest) sum of
    /// the two players' values over the servers, scaled to the config's ping timeout (`best * timeout / 510`). A
    /// length mismatch or an odd length gives the timeout itself.
    pub fn get_ping_value<H: PeerDefsHost>(&self, host: &H, other_ping: &AsciiString) -> i32 {
        let timeout = host.config_ping_timeout_ms();
        if self.ping_string.get_length() != other_ping.get_length() {
            return timeout;
        }
        if self.ping_string.get_length() % 2 != 0 {
            return timeout;
        }
        let mut best = if plant(16361) { 255 + 254 } else { 255 + 255 };
        let my = self.ping_string.str();
        let other = other_ping.str();
        let mut i = 0;
        while i + 1 < my.len() && my[i] != 0 {
            let my_val = grab_hex_int(&my[i..]);
            let other_val = grab_hex_int(&other[i..]);
            let val = my_val + other_val;
            best = if val < best { val } else { best };
            i += 2;
        }
        if plant(16362) { best / (255 + 255) * timeout } else { best * timeout / (255 + 255) }
    }

    /// `updateAdditionalGameSpyDisconnections(count)`: in an internet game with stats on, for a human, non-observer,
    /// AI-free local player whose stats are known, adds `count` to the stats' disconnects for the player's
    /// template (unless that would go below zero), queues a `PSREQUEST_UPDATEPLAYERSTATS` (no add flags), tracks
    /// the new stats, posts a `PSRESPONSE_PLAYERSTATS` for the UI and caches the stats in the misc preferences.
    pub fn update_additional_game_spy_disconnections<H: PeerDefsHost>(
        &mut self,
        host: &mut H,
        count: i32,
        facts: &LocalPlayerFacts,
        game_uses_stats: bool,
    ) {
        if facts.in_internet_multiplayer && game_uses_stats {
            let local_id = self.local_profile_id;
            let ps = host.ps_queue();
            let mut stats = ps.find_player_stats_by_id(local_id);

            let pt_idx = facts.template_index;

            // Check for cases where we're not tracking stats.
            if facts.any_ai
                || stats.id == 0
                || facts.template_is_observer
                || facts.player_is_not_human
                || facts.player_is_observer
            {
                return;
            }

            let mut dis_cons = stats.discons.get(&pt_idx).copied().unwrap_or(0) as i32;
            dis_cons += count;
            if dis_cons < 0 {
                return; // something is wrong here
            }
            stats.discons.insert(pt_idx, dis_cons as u32); // add an additional disconnection to their stats.

            // Add an additional disconnection to player stats.
            let mut req = PsRequest::new();
            req.request_type = PsRequestType::UpdatePlayerStats;
            req.email = self.local_email.str().to_vec();
            req.nick = self.local_base_name.str().to_vec();
            req.password = self.local_passwd.str().to_vec();
            req.player = PsPlayerStats::copy_constructed(&stats);
            req.add_desync = false;
            req.add_discon = false;
            req.last_house = pt_idx;

            ps.add_request(req);
            ps.track_player_stats(PsPlayerStats::copy_constructed(&stats));

            // force an update of our shtuff
            let mut new_resp = PsResponse::new();
            new_resp.response_type = PsResponseType::PlayerStats;
            new_resp.player = PsPlayerStats::copy_constructed(&stats);
            ps.add_response(new_resp);

            // cache our stuff for easy reading next time
            let kv = format_player_kv_pairs(&stats, host.player_template_count());
            host.misc_prefs_set_cached_stats_and_write(&kv);
        }
    }
}

/// `grabHexInt(s)`: the two hex digits at `s` via `strtol("0x..")` (a non-hex digit stops the number).
fn grab_hex_int(s: &[u8]) -> i32 {
    let mut v: i32 = 0;
    let mut n = 0;
    for &c in s.iter().take(2) {
        let d = match c {
            b'0'..=b'9' => (c - b'0') as i32,
            b'a'..=b'f' => (c - b'a') as i32 + 10,
            b'A'..=b'F' => (c - b'A') as i32 + 10,
            _ => {
                if plant(16363) {
                    v *= 16;
                    n += 1;
                    continue;
                }
                break;
            }
        };
        v = v * 16 + d;
        n += 1;
    }
    let _ = n;
    v
}

/// The global singletons `SetUpGameSpy` creates (`TheGameSpyBuddyMessageQueue`, `ThePeer...`, `TheGameSpyInfo`,
/// `TheGameSpyConfig`, `TheLadderList`, `ThePinger`, `TheRankPointValues`).
#[derive(Debug, Default)]
pub struct GameSpyGlobals {
    /// `TheGameSpyBuddyMessageQueue`.
    pub buddy_queue: Option<BuddyMessageQueue>,
    /// `TheGameSpyPeerMessageQueue`.
    pub peer_queue: Option<PeerMessageQueue>,
    /// `TheGameSpyPSMessageQueue`.
    pub ps_queue: Option<PsMessageQueue>,
    /// `TheGameSpyInfo`.
    pub info: Option<GameSpyInfo>,
    /// `TheGameSpyConfig`.
    pub config: Option<GameSpyConfig>,
    /// `TheLadderList`.
    pub ladder_list: Option<LadderList>,
    /// `ThePinger`.
    pub pinger: Option<Pinger>,
    /// `TheRankPointValues != NULL`.
    pub rank_point_values: bool,
}

/// What `SetUpGameSpy` / `TearDownGameSpy` do outside the globals.
pub trait SetUpHost: PeerDefsHost + LadderHost {
    /// `TheGlobalData->getPath_UserData()` and the three `CreateDirectory` calls for `GeneralsOnline` and
    /// `GeneralsOnline\Ladders`.
    fn create_user_data_directories(&mut self);
    /// `CustomMatchPreferences pref; pref.getDisallowAsianText()`.
    fn custom_match_disallow_asian_text(&self) -> bool;
    /// `pref.getDisallowNonAsianText()`.
    fn custom_match_disallow_non_asian_text(&self) -> bool;
    /// `GameSpyConfigInterface::create(config)`'s map lookup (see [`GameSpyConfig::new`]).
    fn qm_map_lookup(&self, lowercase_map: &[u8]) -> Option<Vec<u8>>;
    /// `SignalUIInteraction(SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT)`.
    fn signal_generals_online_logout(&mut self);
    /// `deleteNotificationBox()`.
    fn delete_notification_box(&mut self);
    /// `new RankPoints` / `delete TheRankPointValues`.
    fn set_rank_point_values(&mut self, present: bool);
}

impl GameSpyGlobals {
    /// `SetUpGameSpy(motdBuffer, configBuffer)` (PeerDefs.cpp): tears down any earlier set-up, creates the user data
    /// folders, the buddy, peer and stats queues (threads started), the `GameSpyInfo` with MOTD and config text
    /// and the custom-match text preferences, the config, the ladder list, the pinger (threads started) and the
    /// rank point values. Null buffers are empty strings.
    pub fn set_up_game_spy<H: SetUpHost>(&mut self, host: &mut H, motd_buffer: Option<&[u8]>, config_buffer: Option<&[u8]>) {
        let motd = motd_buffer.unwrap_or(b"");
        let config = config_buffer.unwrap_or(b"");
        self.tear_down_game_spy(host);

        host.create_user_data_directories();

        let b = BuddyMessageQueue::new();
        b.start_thread();
        self.buddy_queue = Some(b);

        let p = PeerMessageQueue::new();
        p.start_thread();
        self.peer_queue = Some(p);

        let ps = PsMessageQueue::new();
        ps.start_thread();
        self.ps_queue = Some(ps);

        let mut info = GameSpyInfo::new(host);
        info.set_motd(AsciiString::try_from_bytes(motd).unwrap_or_default());
        info.set_config(AsciiString::try_from_bytes(config).unwrap_or_default());

        info.set_disallow_asian_text(host.custom_match_disallow_asian_text());
        info.set_disallow_non_asian_text(host.custom_match_disallow_non_asian_text());
        self.info = Some(info);

        let cfg = GameSpyConfig::new(config, &|m: &[u8]| host.qm_map_lookup(m));
        let leftover = cfg.get_leftover_config();
        self.config = Some(cfg);

        self.ladder_list = Some(LadderList::new(host, &leftover));

        let pinger = Pinger::new();
        pinger.start_threads();
        self.pinger = Some(pinger);

        self.rank_point_values = true;
        host.set_rank_point_values(true);
    }

    /// `TearDownGameSpy` (PeerDefs.cpp): saves the cached local stats to the misc preferences, ends the threads
    /// before destroying what they reference, tells the shell if we had logged in (internal IP set), and destroys
    /// the globals in upstream's order.
    pub fn tear_down_game_spy<H: SetUpHost>(&mut self, host: &mut H) {
        // save off cached stats
        if let (Some(info), Some(ps)) = (self.info.as_ref(), self.ps_queue.as_ref()) {
            if info.get_local_profile_id() != 0 {
                let local = ps.find_player_stats_by_id(info.get_local_profile_id());
                if local.id != 0 {
                    let kv = format_player_kv_pairs(&local, host.player_template_count());
                    host.misc_prefs_set_cached_stats_and_write(&kv);
                }
            }
        }

        // End our threads before we kill off the singletons they reference.  No crashy-crash for you!
        if let Some(ps) = self.ps_queue.as_ref() {
            ps.end_thread();
        }
        if let Some(b) = self.buddy_queue.as_ref() {
            b.end_thread();
        }
        if let Some(p) = self.peer_queue.as_ref() {
            p.end_thread();
        }
        if let Some(pg) = self.pinger.as_ref() {
            pg.end_threads();
        }

        if self.rank_point_values {
            self.rank_point_values = false;
            host.set_rank_point_values(false);
        }
        self.ps_queue = None;
        self.buddy_queue = None;
        self.peer_queue = None;

        if let Some(info) = self.info.as_ref() {
            if info.get_internal_ip() != 0 {
                // we've logged in before.  mark us as logging out.
                host.signal_generals_online_logout();
            }
        }
        self.info = None;

        self.pinger = None;
        self.ladder_list = None;
        self.config = None;

        // make sure the notification box doesn't exist
        host.delete_notification_box();
    }
}
