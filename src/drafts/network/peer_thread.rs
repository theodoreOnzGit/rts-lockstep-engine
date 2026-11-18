//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PeerThread.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/PeerThread.cpp
//!
//! The GameSpy "Peer" (chat/lobby/staging-room/quick-match) thread: the request and response types, the queue pair,
//! the staging-room server map, the broadcast-key stats tables, the quick-match state machine and every Peer SDK
//! callback turned into a response.
//!
//! # SDK abstraction
//!
//! The Peer, SB (server browser) and QR2 (query/report) SDKs are gone and were never in the repository (the vendored
//! `Libraries/Source/GameSpy` folder is empty). Every SDK call is the [`PeerSdkHost`]; every SDK callback is a
//! [`PeerThread`] method that the host calls when the SDK would fire it. An `SBServer` is the opaque [`SbServer`]
//! handle. `peerEnumPlayers` with a callback is a host call that returns the players. The thread's `while
//! (running)` body is [`PeerThread::thread_step`] and the quick-match loop is [`PeerThread::do_quick_match_step`]
//! (one iteration), driven by the owner. SDK constants whose numeric values are not in the repository
//! (`PEER_FLAG_OP`, `PEER_ADD`...) are modelled as enums or host queries.
//!
//! # Defects of upstream kept
//!
//! * `QRPlayerKeyCallback`: `FACTION__KEY` and `COLOR__KEY` both answer `getPlayerLosses(index)` (not the faction and
//!   colour), so the game list shows the loss count for them.
//! * `listGroupRoomsCallback`: `resp.groupRoomName.empty()` (a no-op query, not `clear()`) is kept as nothing.
//! * `listingGamesCallback`: the `SBServerHasBasicKeys(server)` branch logs "does not have basic keys" when it
//!   *has* them and returns; kept.
//! * `PEERREQUEST_MESSAGEROOM` with no group room sends to the staging room; a zero `m_groupRoomID` is "in staging".
//! * The `UpdateBuddyStatus` locationString is cut to `GP_LOCATION_STRING_LEN-1` by `sprintf` overruns in C++;
//!   here it is not cut (the SDK length is not in the repository).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, RwLock};

use super::buddy_thread::{BuddyMessageQueue, BuddyRequest, BuddyRequestType, GpStatus};
use super::net_command_msg::MAX_SLOTS;
use super::persistent_storage_thread::{PsMessageQueue, PsRequest, PsRequestType};

const SLOTS: usize = MAX_SLOTS as usize;

/// `enum SerialAuthResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SerialAuthResult {
    /// `SERIAL_NONEXISTENT`.
    NonExistent,
    /// `SERIAL_AUTHFAILED`.
    AuthFailed,
    /// `SERIAL_BANNED`.
    Banned,
    /// `SERIAL_OK`.
    #[default]
    Ok,
}

/// `RoomType` of the Peer SDK.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RoomType {
    /// `TitleRoom`.
    TitleRoom,
    /// `GroupRoom`.
    #[default]
    GroupRoom,
    /// `StagingRoom`.
    StagingRoom,
}

/// `PEERJoinResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PeerJoinResult {
    /// `PEERJoinSuccess`.
    Success,
    /// `PEERFullRoom`.
    FullRoom,
    /// `PEERInviteOnlyRoom`.
    InviteOnlyRoom,
    /// `PEERBannedFromRoom`.
    BannedFromRoom,
    /// `PEERBadPassword`.
    BadPassword,
    /// `PEERAlreadyInRoom`.
    AlreadyInRoom,
    /// `PEERNoTitleSet`.
    NoTitleSet,
    /// `PEERNoConnection`.
    NoConnection,
    /// `PEERAutomatchFailed`.
    AutomatchFailed,
    /// `PEERJoinFailed`.
    #[default]
    JoinFailed,
}

/// The `PEER_ADD`/`PEER_UPDATE`/`PEER_REMOVE`/`PEER_CLEAR`/`PEER_COMPLETE` actions of the game listing callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PeerListAction {
    /// `PEER_CLEAR`.
    #[default]
    Clear,
    /// `PEER_ADD`.
    Add,
    /// `PEER_UPDATE`.
    Update,
    /// `PEER_REMOVE`.
    Remove,
    /// `PEER_COMPLETE`.
    Complete,
}

/// `MessageType` of the Peer SDK (`NormalMessage`, `ActionMessage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerMessageKind {
    /// `NormalMessage`.
    Normal,
    /// `ActionMessage`.
    Action,
}

/// The nick error types of `nickErrorCallback` (`PEER_IN_USE` or any other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerNickError {
    /// `PEER_IN_USE`.
    InUse,
    /// Any other type.
    Other,
}

/// `SBServer`: an opaque handle owned by the server browser SDK.
pub type SbServer = u32;

/// `PeerRequest::peerRequestType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PeerRequestType {
    /// `PEERREQUEST_LOGIN`.
    #[default]
    Login,
    /// `PEERREQUEST_LOGOUT`.
    Logout,
    /// `PEERREQUEST_MESSAGEPLAYER`.
    MessagePlayer,
    /// `PEERREQUEST_MESSAGEROOM`.
    MessageRoom,
    /// `PEERREQUEST_JOINGROUPROOM`.
    JoinGroupRoom,
    /// `PEERREQUEST_LEAVEGROUPROOM`.
    LeaveGroupRoom,
    /// `PEERREQUEST_STARTGAMELIST`.
    StartGameList,
    /// `PEERREQUEST_STOPGAMELIST`.
    StopGameList,
    /// `PEERREQUEST_CREATESTAGINGROOM`.
    CreateStagingRoom,
    /// `PEERREQUEST_SETGAMEOPTIONS`.
    SetGameOptions,
    /// `PEERREQUEST_JOINSTAGINGROOM`.
    JoinStagingRoom,
    /// `PEERREQUEST_LEAVESTAGINGROOM`.
    LeaveStagingRoom,
    /// `PEERREQUEST_UTMPLAYER`.
    UtmPlayer,
    /// `PEERREQUEST_UTMROOM`.
    UtmRoom,
    /// `PEERREQUEST_STARTGAME`.
    StartGame,
    /// `PEERREQUEST_STARTQUICKMATCH`.
    StartQuickMatch,
    /// `PEERREQUEST_WIDENQUICKMATCHSEARCH`.
    WidenQuickMatchSearch,
    /// `PEERREQUEST_STOPQUICKMATCH`.
    StopQuickMatch,
    /// `PEERREQUEST_PUSHSTATS`.
    PushStats,
    /// `PEERREQUEST_GETEXTENDEDSTAGINGROOMINFO`.
    GetExtendedStagingRoomInfo,
}

/// `PeerRequest` (the anonymous union flattened: members of different union arms that share storage in C++ have
/// separate fields here; only the arm selected by the request type is meaningful).
#[derive(Clone, Debug, Default)]
pub struct PeerRequest {
    /// `peerRequestType`.
    pub peer_request_type: PeerRequestType,
    /// `nick`.
    pub nick: Vec<u8>,
    /// `text`.
    pub text: Vec<u16>,
    /// `password`.
    pub password: Vec<u8>,
    /// `email`.
    pub email: Vec<u8>,
    /// `id`.
    pub id: Vec<u8>,
    /// `options`.
    pub options: Vec<u8>,
    /// `ladderIP`.
    pub ladder_ip: Vec<u8>,
    /// `hostPingStr`.
    pub host_ping_str: Vec<u8>,
    /// `gameOptsMapName`.
    pub game_opts_map_name: Vec<u8>,
    /// `gameOptsPlayerNames`.
    pub game_opts_player_names: [Vec<u8>; SLOTS],
    /// `qmMaps`.
    pub qm_maps: Vec<bool>,
    /// `login.profileID`.
    pub login_profile_id: i32,
    /// `groupRoom.id`.
    pub group_room_id: i32,
    /// `gameList.restrictGameList`.
    pub game_list_restrict: bool,
    /// `message.isAction`.
    pub message_is_action: bool,
    /// `stagingRoom.id`.
    pub staging_room_id: i32,
    /// `stagingRoomCreation.*`.
    pub staging_room_creation: StagingRoomCreation,
    /// `gameOptions.*`.
    pub game_options: RequestGameOptions,
    /// `UTM.isStagingRoom`.
    pub utm_is_staging_room: bool,
    /// `QM.*`.
    pub qm: QuickMatchInfo,
    /// `statsToPush.*`.
    pub stats_to_push: StatsToPush,
}

/// `PeerRequest::stagingRoomCreation`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StagingRoomCreation {
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub game_version: u32,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub lad_port: u16,
    pub lad_pass_crc: u32,
    pub restrict_game_list: bool,
}

/// `PeerRequest::gameOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RequestGameOptions {
    pub wins: [i32; SLOTS],
    pub losses: [i32; SLOTS],
    pub profile_id: [i32; SLOTS],
    pub faction: [i32; SLOTS],
    pub color: [i32; SLOTS],
    pub num_players: i32,
    pub max_players: i32,
    pub num_observers: i32,
}

/// `PeerRequest::QM`.
#[derive(Clone, Debug, Default)]
pub struct QuickMatchInfo {
    pub min_point_percentage: i32,
    pub max_point_percentage: i32,
    pub points: i32,
    pub widen_time: i32,
    pub ladder_id: i32,
    pub ladder_pass_crc: u32,
    pub max_ping: i32,
    pub max_discons: i32,
    pub discons: i32,
    /// `pings[17]` (8 servers 0-ff, NUL).
    pub pings: Vec<u8>,
    pub num_players: i32,
    pub bot_id: i32,
    pub room_id: i32,
    pub side: i32,
    pub color: i32,
    pub nat: i32,
    pub exe_crc: u32,
    pub ini_crc: u32,
}

/// `PeerRequest::statsToPush`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StatsToPush {
    pub locale: i32,
    pub wins: i32,
    pub losses: i32,
    pub rank_points: i32,
    pub side: i32,
    pub preorder: bool,
}

/// `enum DisconnectReason`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisconnectReason {
    /// `DISCONNECT_NICKTAKEN` = 1.
    #[default]
    NickTaken = 1,
    BadNick,
    LostCon,
    CouldNotConnect,
    GpLoginTimeout,
    GpLoginBadNick,
    GpLoginBadEmail,
    GpLoginBadPassword,
    GpLoginBadProfile,
    GpLoginProfileDeleted,
    GpLoginConnectionFailed,
    GpLoginServerAuthFailed,
    SerialInvalid,
    SerialNotPresent,
    SerialBanned,
    GpNewUserBadNick,
    GpNewUserBadPassword,
    GpNewProfileBadNick,
    GpNewProfileBadOldNick,
}

/// `enum QMStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QmStatus {
    /// `QM_IDLE`.
    #[default]
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

/// `PeerResponse::peerResponseType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PeerResponseType {
    /// `PEERRESPONSE_LOGIN`.
    #[default]
    Login,
    Disconnect,
    Message,
    GroupRoom,
    StagingRoom,
    StagingRoomListComplete,
    StagingRoomPlayerInfo,
    JoinGroupRoom,
    CreateStagingRoom,
    JoinStagingRoom,
    PlayerJoin,
    PlayerLeft,
    PlayerChangedNick,
    PlayerInfo,
    PlayerChangedFlags,
    RoomUtm,
    PlayerUtm,
    QuickMatchStatus,
    GameStart,
    FailedToHost,
}

/// `PeerResponse` (union flattened, see [`PeerRequest`]).
#[derive(Clone, Debug, Default)]
pub struct PeerResponse {
    pub peer_response_type: PeerResponseType,
    /// `groupRoomName`.
    pub group_room_name: Vec<u8>,
    pub nick: Vec<u8>,
    pub old_nick: Vec<u8>,
    pub text: Vec<u16>,
    pub locale: Vec<u8>,
    pub staging_server_game_options: Vec<u8>,
    pub staging_server_name: Vec<u16>,
    pub staging_server_ping_string: Vec<u8>,
    pub staging_server_ladder_ip: Vec<u8>,
    pub staging_room_map_name: Vec<u8>,
    pub staging_room_player_names: [Vec<u8>; SLOTS],
    pub command: Vec<u8>,
    pub command_options: Vec<u8>,
    /// `discon.reason`.
    pub discon_reason: DisconnectReason,
    /// `groupRoom.*`.
    pub group_room: ResponseGroupRoom,
    /// `joinGroupRoom.*`.
    pub join_group_room: ResponseJoinGroupRoom,
    /// `createStagingRoom.result`.
    pub create_staging_room_result: PeerJoinResult,
    /// `joinStagingRoom.*`.
    pub join_staging_room: ResponseJoinStagingRoom,
    /// `message.*`.
    pub message: ResponseMessage,
    /// `player.*`.
    pub player: ResponsePlayer,
    /// `stagingRoom.*`.
    pub staging_room: ResponseStagingRoom,
    /// `qmStatus.*`.
    pub qm_status: ResponseQmStatus,
}

/// `PeerResponse::groupRoom`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseGroupRoom {
    pub id: i32,
    pub num_waiting: i32,
    pub max_waiting: i32,
    pub num_games: i32,
    pub num_playing: i32,
}

/// `PeerResponse::joinGroupRoom`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseJoinGroupRoom {
    pub id: i32,
    pub ok: bool,
}

/// `PeerResponse::joinStagingRoom`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseJoinStagingRoom {
    pub id: i32,
    pub ok: bool,
    pub is_host_present: bool,
    pub result: PeerJoinResult,
}

/// `PeerResponse::message`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseMessage {
    pub is_private: bool,
    pub is_action: bool,
    pub profile_id: i32,
}

/// `PeerResponse::player`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponsePlayer {
    pub profile_id: i32,
    pub wins: i32,
    pub losses: i32,
    pub room_type: RoomType,
    pub flags: i32,
    pub ip: u32,
    pub rank_points: i32,
    pub side: i32,
    pub preorder: i32,
    pub internal_ip: u32,
    pub external_ip: u32,
}

/// `PeerResponse::stagingRoom`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseStagingRoom {
    pub id: i32,
    pub action: PeerListAction,
    pub is_staging: bool,
    pub requires_password: bool,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub version: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub ladder_port: u16,
    pub wins: [i32; SLOTS],
    pub losses: [i32; SLOTS],
    pub profile_id: [i32; SLOTS],
    pub faction: [i32; SLOTS],
    pub color: [i32; SLOTS],
    pub num_players: i32,
    pub num_observers: i32,
    pub max_players: i32,
    pub percent_complete: i32,
}

/// `PeerResponse::qmStatus`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseQmStatus {
    pub status: QmStatus,
    pub pool_size: i32,
    pub map_idx: i32,
    pub seed: i32,
    pub ip: [u32; SLOTS],
    pub side: [i32; SLOTS],
    pub color: [i32; SLOTS],
    pub nat: [i32; SLOTS],
}

#[derive(Debug, Default)]
struct PeerQueues {
    requests: VecDeque<PeerRequest>,
    responses: VecDeque<PeerResponse>,
    serial_auth: SerialAuthResult,
    /// The thread's published state (`m_thread != NULL`, `isConnected`, `isConnecting`).
    thread_exists: bool,
    is_connected: bool,
    is_connecting: bool,
}

/// `class GameSpyPeerMessageQueue` (PeerThread.cpp).
#[derive(Clone, Debug, Default)]
pub struct PeerMessageQueue {
    inner: Arc<RwLock<PeerQueues>>,
}

impl PeerMessageQueue {
    /// `GameSpyPeerMessageQueue()`: no thread, `m_serialAuth = SERIAL_OK`.
    pub fn new() -> PeerMessageQueue {
        PeerMessageQueue::default()
    }
    /// `startThread`.
    pub fn start_thread(&self) {
        if let Ok(mut q) = self.inner.write() {
            q.thread_exists = true;
        }
    }
    /// `endThread`.
    pub fn end_thread(&self) {
        if let Ok(mut q) = self.inner.write() {
            q.thread_exists = false;
        }
    }
    /// `isThreadRunning`.
    pub fn is_thread_running(&self) -> bool {
        self.inner.read().map(|q| q.thread_exists).unwrap_or(false)
    }
    /// `isConnected`.
    pub fn is_connected(&self) -> bool {
        self.inner.read().map(|q| q.thread_exists && q.is_connected).unwrap_or(false)
    }
    /// `isConnecting`.
    pub fn is_connecting(&self) -> bool {
        self.inner.read().map(|q| q.thread_exists && q.is_connecting).unwrap_or(false)
    }
    /// `addRequest`.
    pub fn add_request(&self, req: PeerRequest) {
        if let Ok(mut q) = self.inner.write() {
            q.requests.push_back(req);
        }
    }
    /// `getRequest`.
    pub fn get_request(&self) -> Option<PeerRequest> {
        self.inner.write().ok().and_then(|mut q| q.requests.pop_front())
    }
    /// `addResponse` (PeerThread.cpp): a response whose nick is `"(END)"` is dropped.
    pub fn add_response(&self, resp: PeerResponse) {
        if resp.nick == b"(END)" {
            return;
        }
        if let Ok(mut q) = self.inner.write() {
            q.responses.push_back(resp);
        }
    }
    /// `getResponse`.
    pub fn get_response(&self) -> Option<PeerResponse> {
        self.inner.write().ok().and_then(|mut q| q.responses.pop_front())
    }
    /// `getSerialAuthResult`.
    pub fn get_serial_auth_result(&self) -> SerialAuthResult {
        self.inner.read().map(|q| q.serial_auth).unwrap_or_default()
    }
    /// `setSerialAuthResult`.
    pub fn set_serial_auth_result(&self, r: SerialAuthResult) {
        if let Ok(mut q) = self.inner.write() {
            q.serial_auth = r;
        }
    }
    fn publish(&self, connected: bool, connecting: bool) {
        if let Ok(mut q) = self.inner.write() {
            q.is_connected = connected;
            q.is_connecting = connecting;
        }
    }
}

/// What the Peer thread needs of `GetStringFromRegistry`, preferences, the buddy/stats queues, sockets and time.
pub trait PeerSdkHost {
    // ---- time, strings, registry, preferences ----
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    /// `WideCharStringToMultiByte`.
    fn wide_to_multi_byte(&self, s: &[u16]) -> Vec<u8>;
    /// `MultiByteToWideCharSingleLine`.
    fn multi_byte_to_wide_single_line(&self, s: &[u8]) -> Vec<u16>;
    /// `GetStringFromRegistry("\\ergc", "", s)`: the CD key if present and non-empty.
    fn registry_cd_key(&self) -> Option<Vec<u8>>;
    /// `GetRegistryVersion()`.
    fn registry_version(&self) -> u32;
    /// `pref.getOnlineIPAddress()`.
    fn pref_online_ip_address(&self) -> u32;
    /// `pref["GameSpyQRPort"]`.
    fn pref_qr_port(&self) -> Vec<u8>;
    /// `IPEnumeration::getAddresses()` as the list of local IPs.
    fn local_ip_list(&self) -> Vec<u32>;
    /// `GetLocalChatConnectionAddress("peerchat.gamespy.com", 6667, localIP)` (Windows SNMP, not translated; the
    /// host may answer with its own idea of the local address).
    fn local_chat_connection_address(&mut self) -> Option<u32>;
    // ---- other queues ----
    /// `TheGameSpyBuddyMessageQueue` (None if it does not exist).
    fn buddy_queue(&self) -> Option<BuddyMessageQueue>;
    /// `TheGameSpyPSMessageQueue`.
    fn ps_queue(&self) -> PsMessageQueue;
    // ---- Peer SDK ----
    /// `peerInitialize(&callbacks)`: false if NULL.
    fn peer_initialize(&mut self) -> bool;
    /// `peerShutdown(peer)`.
    fn peer_shutdown(&mut self);
    /// `qr2_register_key(key, name)`.
    fn qr2_register_key(&mut self, key: i32, name: &str);
    /// `peerSetRoomWatchKeys(peer, room, 1, "username", PEERTrue)`.
    fn peer_set_room_watch_username(&mut self, room: RoomType);
    /// `peerSetTitle(...)` with the `ccgenzh` title/secret, version, 30, no pings: false on failure.
    fn peer_set_title(&mut self, version: u32) -> bool;
    /// `chatSetLocalIP(ip)`.
    fn chat_set_local_ip(&mut self, ip: u32);
    /// `peerConnect(peer, nick, profileID, nickErrorCallbackWrapper, connectCallbackWrapper, ...)` blocking. The
    /// SDK fires [`PeerThread::nick_error_callback`] and [`PeerThread::connect_callback`] during the call.
    fn peer_connect(&mut self, nick: &[u8], profile_id: i32);
    /// `peerRetryWithNick(peer, nick)`; None cancels.
    fn peer_retry_with_nick(&mut self, nick: Option<&[u8]>);
    /// `peerDisconnect(peer)`.
    fn peer_disconnect(&mut self);
    /// `peerIsConnected(peer)`.
    fn peer_is_connected(&mut self) -> bool;
    /// `peerThink(peer)`; fires the Peer callbacks.
    fn peer_think(&mut self);
    /// `peerGetLocalIP(peer)` (network order as the SDK returns it).
    fn peer_get_local_ip(&mut self) -> u32;
    /// `peerAuthenticateCDKey(peer, key, callback, &result, PEERTrue)`: the SDK result `>= 1` is OK.
    fn peer_authenticate_cd_key(&mut self, key: &[u8]) -> i32;
    /// `peerStopGame(peer)`.
    fn peer_stop_game(&mut self);
    /// `peerLeaveRoom(peer, room, NULL)`.
    fn peer_leave_room(&mut self, room: RoomType);
    /// `peerJoinGroupRoom(peer, id, joinRoomCallback, this, PEERTrue)`; the SDK fires
    /// [`PeerThread::join_room_callback`].
    fn peer_join_group_room(&mut self, id: i32);
    /// `peerJoinStagingRoom(peer, server, password, joinRoomCallback, this, PEERTrue)`.
    fn peer_join_staging_room(&mut self, server: SbServer, password: &[u8]);
    /// `peerCreateStagingRoomWithSocket(...)`: returns the `createRoomCallback` result.
    fn peer_create_staging_room_with_socket(
        &mut self,
        name: &[u8],
        max_players: i32,
        password: &[u8],
        qr2_port: u16,
    ) -> PeerJoinResult;
    /// `peerMessagePlayer(peer, nick, message, type)`.
    fn peer_message_player(&mut self, nick: &[u8], message: &[u8], kind: PeerMessageKind);
    /// `peerMessageRoom(peer, room, message, type)`.
    fn peer_message_room(&mut self, room: RoomType, message: &[u8], kind: PeerMessageKind);
    /// `peerUTMPlayer(peer, nick, id, options, PEERFalse)`.
    fn peer_utm_player(&mut self, nick: &[u8], id: &[u8], options: &[u8]);
    /// `peerUTMRoom(peer, room, id, options, PEERFalse)`.
    fn peer_utm_room(&mut self, room: RoomType, id: &[u8], options: &[u8]);
    /// `peerSetRoomKeys(peer, room, nick, 6, keys, values)`.
    fn peer_set_room_keys(&mut self, room: RoomType, nick: &[u8], keys: &[&str], values: &[String]);
    /// `peerGetRoomKeys(peer, GroupRoom, "*", 6, keys, getRoomKeysCallback, this, PEERFalse)`.
    fn peer_get_room_keys_all(&mut self, room: RoomType, keys: &[&str]);
    /// `peerStateChanged(peer)`.
    fn peer_state_changed(&mut self);
    /// `peerStartListingGames(peer, keys, NumKeys, filter, listingGamesCallback, this)`; `restrict` picks `"~"`.
    fn peer_start_listing_games(&mut self, keys: &[u8], restrict: bool);
    /// `peerStopListingGames(peer)`.
    fn peer_stop_listing_games(&mut self);
    /// `peerStartGame(peer, NULL, PEER_STOP_REPORTING)`.
    fn peer_start_game_stop_reporting(&mut self);
    /// `peerUpdateGame(peer, server, PEERTrue)`.
    fn peer_update_game(&mut self, server: SbServer);
    /// `peerListGroupRooms(peer, NULL, listGroupRoomsCallback, this, PEERTrue)`.
    fn peer_list_group_rooms(&mut self);
    /// `peerGetPlayerInfoNoWait(peer, nick, &IP, &profileID)`: (ip, profile id) or None if unknown.
    fn peer_get_player_info_no_wait(&mut self, nick: &[u8]) -> Option<(u32, i32)>;
    /// `peerGetPlayerFlags(peer, nick, room, &flags)`.
    fn peer_get_player_flags(&mut self, nick: &[u8], room: RoomType) -> i32;
    /// `peerGetPlayerProfileID(peer, nick, callback, &id, PEERTrue)`.
    fn peer_get_player_profile_id(&mut self, nick: &[u8]) -> i32;
    /// `peerEnumPlayers(peer, StagingRoom, stagingRoomPlayerEnum, &resp)` as the list of (index, nick, flags).
    fn peer_enum_players_staging(&mut self) -> Vec<(i32, Vec<u8>, i32)>;
    /// `peerEnumPlayers(peer, GroupRoom, quickmatchEnumPlayersCallback, this)`: the SDK calls
    /// [`PeerThread::quickmatch_enum_players_callback`] for each player and once more with None.
    fn peer_enum_players_group_room(&mut self);
    /// `flags & PEER_FLAG_OP`.
    fn flags_have_op(&self, flags: i32) -> bool;
    // ---- QR2 / sockets ----
    /// Binds a UDP socket for QR2 on `local_ip:port`; true on success (`bind(...) == 0`). Replaces any open one.
    fn qr2_socket_open(&mut self, local_ip: u32, port: u16) -> bool;
    /// `closesocket(qr2Sock)` if open.
    fn qr2_socket_close(&mut self);
    /// Whether the QR2 socket is open (`qr2Sock != INVALID_SOCKET`).
    fn qr2_socket_is_open(&self) -> bool;
    /// `checkQR2Queries(peer, qr2Sock)`: reads every waiting datagram (255 bytes at most each) into
    /// `peerParseQuery`.
    fn qr2_check_queries(&mut self);
    // ---- server browser (SBServer) ----
    /// `server->keyvals != NULL`.
    fn sb_server_has_keyvals(&self, server: SbServer) -> bool;
    /// `SBServerGetStringValue(server, key, default)`.
    fn sb_server_get_string(&self, server: SbServer, key: &str, default: &str) -> Vec<u8>;
    /// `SBServerGetIntValue(server, key, default)`.
    fn sb_server_get_int(&self, server: SbServer, key: &str, default: i32) -> i32;
    /// `SBServerGetPlayerStringValue(server, i, key, default)`.
    fn sb_server_get_player_string(&self, server: SbServer, i: usize, key: &str, default: &str) -> Vec<u8>;
    /// `SBServerGetPlayerIntValue(server, i, key, default)`.
    fn sb_server_get_player_int(&self, server: SbServer, i: usize, key: &str, default: i32) -> i32;
    /// `SBServerGetPrivateInetAddress`.
    fn sb_server_private_ip(&self, server: SbServer) -> u32;
    /// `SBServerGetPrivateQueryPort`.
    fn sb_server_private_query_port(&self, server: SbServer) -> u16;
    /// `SBServerGetPublicInetAddress`.
    fn sb_server_public_ip(&self, server: SbServer) -> u32;
    /// `SBServerHasBasicKeys`.
    fn sb_server_has_basic_keys(&self, server: SbServer) -> bool;
}

// ---- the registered QR2 keys ----

/// `EXECRC_KEY` .. `LOSSES__KEY` follow `NUM_RESERVED_KEYS + 1` in upstream's anonymous enum. The reserved-key
/// count and `HOSTNAME_KEY`, `GAMEVER_KEY`, `GAMENAME_KEY`, `MAPNAME_KEY`, `PID__KEY` come from the QR2 SDK
/// (`qr2.h`), which is not in the repository, so the keys are an enum and the host maps them to numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Qr2Key {
    HostName,
    GameVer,
    GameName,
    MapName,
    Pid,
    ExeCrc,
    IniCrc,
    Pw,
    Obs,
    UseStats,
    LadIp,
    LadPort,
    PingStr,
    NumPlayer,
    MaxPlayer,
    NumObs,
    Name,
    Faction,
    Color,
    Wins,
    Losses,
}

/// `EXECRC_STR` etc.
pub const EXECRC_STR: &str = "exeCRC";
pub const INICRC_STR: &str = "iniCRC";
pub const PW_STR: &str = "pw";
pub const OBS_STR: &str = "obs";
pub const USE_STATS_STR: &str = "stat";
pub const LADIP_STR: &str = "ladIP";
pub const LADPORT_STR: &str = "ladPort";
pub const PINGSTR_STR: &str = "pings";
pub const NUMPLAYER_STR: &str = "numRealPlayers";
pub const MAXPLAYER_STR: &str = "maxRealPlayers";
pub const NUMOBS_STR: &str = "numObservers";
pub const NAME_STR: &str = "name";
pub const FACTION_STR: &str = "faction";
pub const COLOR_STR: &str = "color";
pub const WINS_STR: &str = "wins";
pub const LOSSES_STR: &str = "losses";

/// What a QR2 key callback answers: an int or a string (`qr2_buffer_add_int` / `qr2_buffer_add`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Qr2Value {
    Int(i32),
    Str(Vec<u8>),
}

/// `PeerThreadClass::s_keys`: the six broadcast keys.
pub const BROADCAST_KEYS: [&str; 6] = ["b_locale", "b_wins", "b_losses", "b_points", "b_side", "b_pre"];

/// `s_heartbeatInterval`.
pub const HEARTBEAT_INTERVAL: u32 = 10000;

/// The `static`/file-level state of PeerThread.cpp (`isThreadHosting`, `s_lastStateChangedHeartbeat`,
/// `s_wantStateChangedHeartbeat`, `localIP`, `matchbotProfileID`, `preferredQRPort`).
#[derive(Clone, Copy, Debug, Default)]
struct PeerGlobals {
    is_thread_hosting: i32,
    last_state_changed_heartbeat: u32,
    want_state_changed_heartbeat: bool,
    local_ip: u32,
    matchbot_profile_id: i32,
    preferred_qr_port: u32,
}

/// `class PeerThreadClass` (PeerThread.cpp).
#[derive(Debug)]
pub struct PeerThread {
    /// `TheGameSpyPeerMessageQueue`.
    pub queue: PeerMessageQueue,
    g: PeerGlobals,
    is_connecting: bool,
    is_connected: bool,
    login_name: Vec<u8>,
    original_name: Vec<u8>,
    password: Vec<u8>,
    email: Vec<u8>,
    profile_id: i32,
    group_room_id: i32,
    saw_complete_game_list: bool,
    /// `m_groupRoomStats`.
    group_room_stats: BTreeMap<Vec<u8>, i32>,
    /// `m_stagingRoomStats`.
    staging_room_stats: BTreeMap<Vec<u8>, i32>,
    /// `s_valueBuffers`.
    value_buffers: [String; 6],
    is_hosting: bool,
    has_password: bool,
    map_name: Vec<u8>,
    player_names: [Vec<u8>; SLOTS],
    exe_crc: u32,
    ini_crc: u32,
    game_version: u32,
    allow_observers: bool,
    use_stats: bool,
    ping_str: Vec<u8>,
    ladder_ip: Vec<u8>,
    ladder_port: u16,
    player_wins: [i32; SLOTS],
    player_losses: [i32; SLOTS],
    player_profile_id: [i32; SLOTS],
    player_colors: [i32; SLOTS],
    player_factions: [i32; SLOTS],
    num_players: i32,
    max_players: i32,
    num_observers: i32,
    next_staging_server: i32,
    staging_servers: BTreeMap<i32, SbServer>,
    local_staging_server_name: Vec<u16>,
    local_room_id: i32,
    qm_status: QmStatus,
    qm_info: PeerRequest,
    room_joined: bool,
    qm_group_room: i32,
    saw_end_of_enum_players: bool,
    saw_matchbot: bool,
    matchbot_name: Vec<u8>,
    /// Whether `doQuickMatch`'s loop is running.
    qm_loop_active: bool,
}

/// C `strtok_r(str, " ", &last)`: the next space-separated token (runs of spaces skipped), None at the end.
pub fn strtok_space<'a>(s: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    while *pos < s.len() && s[*pos] == b' ' {
        *pos += 1;
    }
    if *pos >= s.len() {
        return None;
    }
    let start = *pos;
    while *pos < s.len() && s[*pos] != b' ' {
        *pos += 1;
    }
    let tok = &s[start..*pos];
    if *pos < s.len() {
        *pos += 1; // the delimiter is overwritten with NUL
    }
    Some(tok)
}

/// `updateBuddyStatus` (PeerThread.cpp): posts a `BUDDYREQUEST_SETSTATUS` for the status. Does nothing without a
/// buddy queue.
pub fn update_buddy_status(buddy: Option<&BuddyMessageQueue>, status: BuddyStatus, group_room: i32, game_name: &[u8]) {
    let q = match buddy {
        Some(q) => q,
        None => return,
    };
    let mut req = BuddyRequest::default();
    req.buddy_request_type = BuddyRequestType::SetStatus;
    match status {
        BuddyStatus::Offline => {
            req.status = GpStatus::Offline;
            req.status_string = b"Offline".to_vec();
            req.location_string = Vec::new();
        }
        BuddyStatus::Online => {
            req.status = GpStatus::Online;
            req.status_string = b"Online".to_vec();
            req.location_string = Vec::new();
        }
        BuddyStatus::Lobby => {
            req.status = GpStatus::Chatting;
            req.status_string = b"Chatting".to_vec();
            req.location_string = format!("{}", group_room).into_bytes();
        }
        BuddyStatus::Staging => {
            req.status = GpStatus::Staging;
            req.status_string = b"Staging".to_vec();
            req.location_string = game_name.to_vec();
        }
        BuddyStatus::Loading => {
            req.status = GpStatus::Playing;
            req.status_string = b"Loading".to_vec();
            req.location_string = game_name.to_vec();
        }
        BuddyStatus::Playing => {
            req.status = GpStatus::Playing;
            req.status_string = b"Playing".to_vec();
            req.location_string = game_name.to_vec();
        }
        BuddyStatus::Matching => {
            req.status = GpStatus::Online;
            req.status_string = b"Matching".to_vec();
            req.location_string = Vec::new();
        }
    }
    q.add_request(req);
}

/// `enum GameSpyBuddyStatus` (PeerDefs.h), minus `BUDDY_MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuddyStatus {
    Offline,
    Online,
    Lobby,
    Staging,
    Loading,
    Playing,
    Matching,
}

impl PeerThread {
    /// `PeerThreadClass::PeerThreadClass()` (PeerThread.cpp): everything cleared; `m_useStats = TRUE`; the next
    /// staging-server id is 1. (`m_numPlayers` and `m_qmStatus` are uninitialised in C++; 0 / `QM_IDLE` here.)
    pub fn new(queue: PeerMessageQueue) -> PeerThread {
        PeerThread {
            queue,
            g: PeerGlobals::default(),
            is_connecting: false,
            is_connected: false,
            login_name: Vec::new(),
            original_name: Vec::new(),
            password: Vec::new(),
            email: Vec::new(),
            profile_id: 0,
            group_room_id: 0,
            saw_complete_game_list: false,
            group_room_stats: BTreeMap::new(),
            staging_room_stats: BTreeMap::new(),
            value_buffers: Default::default(),
            is_hosting: false,
            has_password: false,
            map_name: Vec::new(),
            player_names: Default::default(),
            exe_crc: 0,
            ini_crc: 0,
            game_version: 0,
            allow_observers: false,
            use_stats: true,
            ping_str: Vec::new(),
            ladder_ip: Vec::new(),
            ladder_port: 0,
            player_wins: [0; SLOTS],
            player_losses: [0; SLOTS],
            player_profile_id: [0; SLOTS],
            player_colors: [0; SLOTS],
            player_factions: [0; SLOTS],
            num_players: 0,
            max_players: 0,
            num_observers: 0,
            next_staging_server: 1,
            staging_servers: BTreeMap::new(),
            local_staging_server_name: Vec::new(),
            local_room_id: 0,
            qm_status: QmStatus::Idle,
            qm_info: PeerRequest::default(),
            room_joined: false,
            qm_group_room: 0,
            saw_end_of_enum_players: false,
            saw_matchbot: false,
            matchbot_name: Vec::new(),
            qm_loop_active: false,
        }
    }

    fn publish(&self) {
        self.queue.publish(self.is_connected, self.is_connecting);
    }

    // ---- accessors (PeerThreadClass) ----
    /// `markAsDisconnected`.
    pub fn mark_as_disconnected(&mut self) {
        self.is_connecting = false;
        self.is_connected = false;
        self.publish();
    }
    /// `isConnecting`.
    pub fn is_connecting(&self) -> bool {
        self.is_connecting
    }
    /// `isConnected`.
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }
    /// `isHosting`.
    pub fn is_hosting(&self) -> bool {
        self.is_hosting
    }
    /// `hasPassword`.
    pub fn has_password(&self) -> bool {
        self.has_password
    }
    /// `allowObservers`.
    pub fn allow_observers(&self) -> bool {
        self.allow_observers
    }
    /// `useStats`.
    pub fn use_stats(&self) -> bool {
        self.use_stats
    }
    /// `getMapName`.
    pub fn get_map_name(&self) -> Vec<u8> {
        self.map_name.clone()
    }
    /// `exeCRC`.
    pub fn exe_crc(&self) -> u32 {
        self.exe_crc
    }
    /// `iniCRC`.
    pub fn ini_crc(&self) -> u32 {
        self.ini_crc
    }
    /// `gameVersion`.
    pub fn game_version(&self) -> u32 {
        self.game_version
    }
    /// `getLocalStagingServerName`.
    pub fn get_local_staging_server_name(&self) -> Vec<u16> {
        self.local_staging_server_name.clone()
    }
    /// `getLocalRoomID`.
    pub fn get_local_room_id(&self) -> i32 {
        self.local_room_id
    }
    /// `ladderIP`.
    pub fn ladder_ip(&self) -> Vec<u8> {
        self.ladder_ip.clone()
    }
    /// `ladderPort`.
    pub fn ladder_port(&self) -> u16 {
        self.ladder_port
    }
    /// `pingStr`.
    pub fn ping_str(&self) -> Vec<u8> {
        self.ping_str.clone()
    }
    /// `getPlayerName(idx)`.
    pub fn get_player_name(&self, idx: usize) -> Vec<u8> {
        self.player_names.get(idx).cloned().unwrap_or_default()
    }
    /// `getPlayerWins(idx)`.
    pub fn get_player_wins(&self, idx: usize) -> i32 {
        self.player_wins.get(idx).copied().unwrap_or(0)
    }
    /// `getPlayerLosses(idx)`.
    pub fn get_player_losses(&self, idx: usize) -> i32 {
        self.player_losses.get(idx).copied().unwrap_or(0)
    }
    /// `getPlayerProfileID(idx)`.
    pub fn get_player_profile_id(&self, idx: usize) -> i32 {
        self.player_profile_id.get(idx).copied().unwrap_or(0)
    }
    /// `getPlayerFaction(idx)`.
    pub fn get_player_faction(&self, idx: usize) -> i32 {
        self.player_factions.get(idx).copied().unwrap_or(0)
    }
    /// `getPlayerColor(idx)`.
    pub fn get_player_color(&self, idx: usize) -> i32 {
        self.player_colors.get(idx).copied().unwrap_or(0)
    }
    /// `getNumPlayers`.
    pub fn get_num_players(&self) -> i32 {
        self.num_players
    }
    /// `getMaxPlayers`.
    pub fn get_max_players(&self) -> i32 {
        self.max_players
    }
    /// `getNumObservers`.
    pub fn get_num_observers(&self) -> i32 {
        self.num_observers
    }
    /// `roomJoined`.
    pub fn room_joined(&mut self, val: bool) {
        self.room_joined = val;
    }
    /// `setQMGroupRoom`.
    pub fn set_qm_group_room(&mut self, group_id: i32) {
        self.qm_group_room = group_id;
    }
    /// `sawEndOfEnumPlayers`.
    pub fn saw_end_of_enum_players(&mut self) {
        self.saw_end_of_enum_players = true;
    }
    /// `sawMatchbot`.
    pub fn saw_matchbot(&mut self, bot: &[u8]) {
        self.saw_matchbot = true;
        self.matchbot_name = bot.to_vec();
    }
    /// `getQMStatus`.
    pub fn get_qm_status(&self) -> QmStatus {
        self.qm_status
    }
    /// `getQMBotName`.
    pub fn get_qm_bot_name(&self) -> Vec<u8> {
        self.matchbot_name.clone()
    }
    /// `getQMGroupRoom`.
    pub fn get_qm_group_room(&self) -> i32 {
        self.qm_group_room
    }
    /// `getQMLadder`.
    pub fn get_qm_ladder(&self) -> i32 {
        self.qm_info.qm.ladder_id
    }
    /// `getCurrentGroupRoom`.
    pub fn get_current_group_room(&self) -> i32 {
        self.group_room_id
    }
    /// `setSawCompleteGameList`.
    pub fn set_saw_complete_game_list(&mut self, val: bool) {
        self.saw_complete_game_list = val;
    }
    /// `getSawCompleteGameList`.
    pub fn get_saw_complete_game_list(&self) -> bool {
        self.saw_complete_game_list
    }

    // ---- broadcast-key stats (USE_BROADCAST_KEYS) ----

    fn pack_stat_key(nick: &[u8], key: &[u8]) -> Vec<u8> {
        let mut s = nick.to_vec();
        s.extend_from_slice(key);
        s
    }

    /// `trackStatsForPlayer` (PeerThread.cpp): `atoi(val)` stored under nick+key in the room's table.
    pub fn track_stats_for_player(&mut self, room_type: RoomType, nick: &[u8], key: &[u8], val: &[u8]) {
        let k = Self::pack_stat_key(nick, key);
        let v = super::persistent_storage_thread::atoi(val);
        match room_type {
            RoomType::GroupRoom => {
                self.group_room_stats.insert(k, v);
            }
            RoomType::StagingRoom => {
                self.staging_room_stats.insert(k, v);
            }
            RoomType::TitleRoom => {}
        }
    }

    /// `lookupStatForPlayer`: the stored value, 0 if none (or for the title room).
    pub fn lookup_stat_for_player(&self, room_type: RoomType, nick: &[u8], key: &[u8]) -> i32 {
        let k = Self::pack_stat_key(nick, key);
        match room_type {
            RoomType::GroupRoom => self.group_room_stats.get(&k).copied().unwrap_or(0),
            RoomType::StagingRoom => self.staging_room_stats.get(&k).copied().unwrap_or(0),
            RoomType::TitleRoom => 0,
        }
    }

    /// `clearPlayerStats`.
    pub fn clear_player_stats(&mut self, room_type: RoomType) {
        match room_type {
            RoomType::GroupRoom => self.group_room_stats.clear(),
            RoomType::StagingRoom => self.staging_room_stats.clear(),
            RoomType::TitleRoom => {}
        }
    }

    /// `pushStatsToRoom`: sets the six broadcast keys in both the group and the staging room.
    pub fn push_stats_to_room<H: PeerSdkHost>(&mut self, host: &mut H) {
        let name = self.login_name.clone();
        let values: Vec<String> = self.value_buffers.to_vec();
        host.peer_set_room_keys(RoomType::GroupRoom, &name, &BROADCAST_KEYS, &values);
        host.peer_set_room_keys(RoomType::StagingRoom, &name, &BROADCAST_KEYS, &values);
    }

    /// `getStatsFromRoom`: always asks the *group* room (`roomType` is ignored, as upstream).
    pub fn get_stats_from_room<H: PeerSdkHost>(&mut self, host: &mut H, _room_type: RoomType) {
        host.peer_get_room_keys_all(RoomType::GroupRoom, &BROADCAST_KEYS);
    }

    // ---- staging-room server map ----

    /// `addServerToMap`: the next id (starting at 1).
    pub fn add_server_to_map(&mut self, server: SbServer) -> i32 {
        let val = self.next_staging_server;
        self.next_staging_server += 1;
        self.staging_servers.insert(val, server);
        val
    }

    /// `removeServerFromMap`: the removed id, 0 if the server is not in the map.
    pub fn remove_server_from_map(&mut self, server: SbServer) -> i32 {
        let found = self.staging_servers.iter().find(|(_, s)| **s == server).map(|(k, _)| *k);
        match found {
            Some(k) => {
                self.staging_servers.remove(&k);
                k
            }
            None => 0,
        }
    }

    /// `clearServers`.
    pub fn clear_servers(&mut self) {
        self.staging_servers.clear();
    }

    /// `findServerByID`: None if unknown or if the server has lost its keyvals.
    pub fn find_server_by_id<H: PeerSdkHost>(&self, host: &H, id: i32) -> Option<SbServer> {
        let server = *self.staging_servers.get(&id)?;
        if !host.sb_server_has_keyvals(server) {
            return None; // DEBUG_CRASH("Referencing a missing server!")
        }
        Some(server)
    }

    /// `findServer` (PeerThread.cpp): the id of `server` if it is in the map; otherwise, if another entry has the
    /// same game name, private IP and port and public IP (the game migrated to a new port), a
    /// `PEERRESPONSE_STAGINGROOM` `PEER_REMOVE` for that old entry is posted and the old one dropped; then
    /// `server` is added and its new id returned.
    pub fn find_server<H: PeerSdkHost>(&mut self, host: &H, server: SbServer) -> i32 {
        let new_name = host.sb_server_get_string(server, "gamename", "");
        let new_private_ip = host.sb_server_private_ip(server);
        let new_private_port = host.sb_server_private_query_port(server);
        let new_public_ip = host.sb_server_public_ip(server);

        let mut server_to_remove: Option<SbServer> = None;
        for (id, s) in self.staging_servers.iter() {
            if *s == server {
                return *id;
            }
            let old_name = host.sb_server_get_string(*s, "gamename", "");
            if old_name == new_name
                && host.sb_server_private_ip(*s) == new_private_ip
                && host.sb_server_public_ip(*s) == new_public_ip
                && host.sb_server_private_query_port(*s) == new_private_port
            {
                server_to_remove = Some(*s);
            }
        }

        if let Some(old) = server_to_remove {
            // this is the same as another game - it has just migrated to another port.  Remove the old and replace it.
            let mut resp = PeerResponse::default();
            resp.peer_response_type = PeerResponseType::StagingRoom;
            resp.staging_room.id = self.remove_server_from_map(old);
            resp.staging_room.action = PeerListAction::Remove;
            resp.staging_room.is_staging = true;
            resp.staging_room.percent_complete = -1;
            self.queue.add_response(resp);
        }

        self.add_server_to_map(server)
    }

    // ---- hosting ----

    /// `PeerThreadClass::stopHostingAlready`: clears the hosting debug flag and heartbeat state, stops the game
    /// report and closes the QR2 socket.
    pub fn stop_hosting_already<H: PeerSdkHost>(&mut self, host: &mut H) {
        self.g.is_thread_hosting = 0;
        self.g.last_state_changed_heartbeat = 0;
        self.g.want_state_changed_heartbeat = false;
        host.peer_stop_game();
        if host.qr2_socket_is_open() {
            host.qr2_socket_close();
        }
    }

    /// `QRServerKeyCallback` (PeerThread.cpp): the value for server key `key`; stops hosting first if the thread is
    /// not hosting. `GAMENAME_KEY` is "<host name> <game name>".
    pub fn qr_server_key_callback<H: PeerSdkHost>(&mut self, host: &mut H, key: Qr2Key) -> Qr2Value {
        if !self.is_hosting() {
            self.stop_hosting_already(host);
        }
        match key {
            Qr2Key::HostName => Qr2Value::Str(self.get_player_name(0)),
            Qr2Key::GameVer => Qr2Value::Int(self.game_version as i32),
            Qr2Key::ExeCrc => Qr2Value::Int(self.exe_crc as i32),
            Qr2Key::IniCrc => Qr2Value::Int(self.ini_crc as i32),
            Qr2Key::GameName => {
                let mut tmp = self.get_player_name(0);
                tmp.push(b' ');
                tmp.extend_from_slice(&host.wide_to_multi_byte(&self.local_staging_server_name));
                Qr2Value::Str(tmp)
            }
            Qr2Key::MapName => Qr2Value::Str(self.get_map_name()),
            Qr2Key::Pw => Qr2Value::Int(self.has_password as i32),
            Qr2Key::Obs => Qr2Value::Int(self.allow_observers as i32),
            Qr2Key::UseStats => Qr2Value::Int(self.use_stats as i32),
            Qr2Key::LadIp => Qr2Value::Str(self.ladder_ip()),
            Qr2Key::LadPort => Qr2Value::Int(self.ladder_port as i32),
            Qr2Key::PingStr => Qr2Value::Str(self.ping_str()),
            Qr2Key::NumPlayer => Qr2Value::Int(self.num_players),
            Qr2Key::MaxPlayer => Qr2Value::Int(self.max_players),
            Qr2Key::NumObs => Qr2Value::Int(self.num_observers),
            _ => Qr2Value::Str(Vec::new()),
        }
    }

    /// `QRPlayerKeyCallback`: the value for player key `key` of player `index`. Faction and colour answer the loss
    /// count (see the module doc).
    pub fn qr_player_key_callback<H: PeerSdkHost>(&mut self, host: &mut H, key: Qr2Key, index: usize) -> Qr2Value {
        if !self.is_hosting() {
            self.stop_hosting_already(host);
        }
        match key {
            Qr2Key::Name => Qr2Value::Str(self.get_player_name(index)),
            Qr2Key::Wins => Qr2Value::Int(self.get_player_wins(index)),
            Qr2Key::Losses => Qr2Value::Int(self.get_player_losses(index)),
            Qr2Key::Pid => Qr2Value::Int(self.get_player_profile_id(index)),
            Qr2Key::Faction => Qr2Value::Int(self.get_player_losses(index)),
            Qr2Key::Color => Qr2Value::Int(self.get_player_losses(index)),
            _ => Qr2Value::Str(Vec::new()),
        }
    }

    /// `QRTeamKeyCallback`: stops hosting if not hosting; answers the empty string (teams are not reported).
    pub fn qr_team_key_callback<H: PeerSdkHost>(&mut self, host: &mut H) -> Qr2Value {
        if !self.is_hosting() {
            self.stop_hosting_already(host);
        }
        Qr2Value::Str(Vec::new())
    }

    /// `QRKeyListCallback`: the keys registered for the server list (`key_server`) and for players (`key_player`);
    /// none for teams. (`GAMENAME_KEY` is commented out of the server list upstream.)
    pub fn qr_key_list(is_server: bool, is_player: bool) -> Vec<Qr2Key> {
        if is_server {
            vec![
                Qr2Key::HostName,
                Qr2Key::GameVer,
                Qr2Key::MapName,
                Qr2Key::ExeCrc,
                Qr2Key::IniCrc,
                Qr2Key::Pw,
                Qr2Key::Obs,
                Qr2Key::UseStats,
                Qr2Key::LadIp,
                Qr2Key::LadPort,
                Qr2Key::PingStr,
                Qr2Key::NumPlayer,
                Qr2Key::MaxPlayer,
                Qr2Key::NumObs,
            ]
        } else if is_player {
            vec![Qr2Key::Name, Qr2Key::Wins, Qr2Key::Losses, Qr2Key::Pid, Qr2Key::Faction, Qr2Key::Color]
        } else {
            Vec::new()
        }
    }

    /// `QRCountCallback`: players + observers for `key_player`, else 0 (stopping hosting first if not hosting).
    pub fn qr_count_callback<H: PeerSdkHost>(&mut self, host: &mut H, is_player: bool) -> i32 {
        if !self.is_hosting() {
            self.stop_hosting_already(host);
        }
        if is_player {
            self.num_players + self.num_observers
        } else {
            0
        }
    }

    /// `QRAddErrorCallback`: posts `PEERRESPONSE_FAILEDTOHOST`.
    pub fn qr_add_error_callback(&mut self) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::FailedToHost;
        self.queue.add_response(resp);
    }

    // ---- CD key ----

    /// `AuthenticateCDKeyCallback` result mapping: `result >= 1` is `SERIAL_OK`, else `SERIAL_AUTHFAILED`.
    pub fn cd_key_result(result: i32) -> SerialAuthResult {
        if result >= 1 {
            SerialAuthResult::Ok
        } else {
            SerialAuthResult::AuthFailed
        }
    }

    /// `doCDKeyAuthentication` (PeerThread.cpp): `SERIAL_NONEXISTENT` without a registry key; otherwise the SDK
    /// result mapped by [`PeerThread::cd_key_result`]; on `SERIAL_OK` asks the stats thread to read the CD key's
    /// stats.
    pub fn do_cd_key_authentication<H: PeerSdkHost>(&mut self, host: &mut H) -> SerialAuthResult {
        let mut retval = SerialAuthResult::NonExistent;
        let key = match host.registry_cd_key() {
            Some(k) if !k.is_empty() => k,
            _ => return retval,
        };
        let r = host.peer_authenticate_cd_key(&key);
        retval = Self::cd_key_result(r);
        if retval == SerialAuthResult::Ok {
            let mut req = PsRequest::new();
            req.request_type = PsRequestType::ReadCdKeyStats;
            req.cdkey = key;
            host.ps_queue().add_request(req);
        }
        retval
    }

    // ---- thread function ----

    /// The head of `PeerThreadClass::Thread_Function`: `peerInitialize`, the key registrations, the room watch keys,
    /// `peerSetTitle`, the preferred local IP (the one in the local list equal to the selected one, else
    /// `INADDR_ANY` = 0) and the preferred QR port (`GameSpyQRPort` preference as an int, else 0). Returns false if
    /// `peerSetTitle` failed (upstream shuts the peer down and returns).
    pub fn thread_start<H: PeerSdkHost>(&mut self, host: &mut H) -> bool {
        self.qm_group_room = 0;
        let _ok = host.peer_initialize(); // DEBUG_ASSERTCRASH(peer != NULL)
        self.is_connected = false;
        self.is_connecting = false;
        self.publish();

        host.qr2_register_key(Qr2Key::ExeCrc as i32, EXECRC_STR);
        host.qr2_register_key(Qr2Key::IniCrc as i32, INICRC_STR);
        host.qr2_register_key(Qr2Key::Pw as i32, PW_STR);
        host.qr2_register_key(Qr2Key::Obs as i32, OBS_STR);
        host.qr2_register_key(Qr2Key::UseStats as i32, USE_STATS_STR);
        host.qr2_register_key(Qr2Key::LadIp as i32, LADIP_STR);
        host.qr2_register_key(Qr2Key::LadPort as i32, LADPORT_STR);
        host.qr2_register_key(Qr2Key::PingStr as i32, PINGSTR_STR);
        host.qr2_register_key(Qr2Key::NumObs as i32, NUMOBS_STR);
        host.qr2_register_key(Qr2Key::NumPlayer as i32, NUMPLAYER_STR);
        host.qr2_register_key(Qr2Key::MaxPlayer as i32, MAXPLAYER_STR);
        host.qr2_register_key(Qr2Key::Name as i32, "name_");
        host.qr2_register_key(Qr2Key::Wins as i32, "wins_");
        host.qr2_register_key(Qr2Key::Losses as i32, "losses_");
        host.qr2_register_key(Qr2Key::Faction as i32, "faction_");
        host.qr2_register_key(Qr2Key::Color as i32, "color_");

        host.peer_set_room_watch_username(RoomType::StagingRoom);
        host.peer_set_room_watch_username(RoomType::GroupRoom);

        self.local_room_id = 0;
        self.local_staging_server_name = Vec::new();
        self.qm_status = QmStatus::Idle;

        if !host.peer_set_title(host.registry_version()) {
            // DEBUG_CRASH("Error setting title")
            host.peer_shutdown();
            return false;
        }

        let selected_ip = host.pref_online_ip_address();
        let mut preferred_ip: u32 = 0; // INADDR_ANY
        for ip in host.local_ip_list() {
            if selected_ip == ip {
                preferred_ip = ip;
                break;
            }
        }
        host.chat_set_local_ip(preferred_ip);

        let sel = host.pref_qr_port();
        self.g.preferred_qr_port = 0;
        if !sel.is_empty() {
            self.g.preferred_qr_port = super::persistent_storage_thread::atoi(&sel) as u32;
        }
        true
    }

    /// The tail of `Thread_Function` after `running` went false: `peerShutdown(peer)`. (The `catch (...)` that posts
    /// `DISCONNECT_LOSTCON` is [`PeerThread::on_thread_exception`].)
    pub fn thread_end<H: PeerSdkHost>(&mut self, host: &mut H) {
        host.peer_shutdown();
    }

    /// The `catch ( ... )` of `Thread_Function`: posts `PEERRESPONSE_DISCONNECT` with `DISCONNECT_LOSTCON`.
    pub fn on_thread_exception(&mut self) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::Disconnect;
        resp.discon_reason = DisconnectReason::LostCon;
        self.queue.add_response(resp);
    }

    /// One pass of the `while (running)` body of `Thread_Function`: one request, the state-changed heartbeat, then
    /// the network update (`checkQR2Queries` while hosting, `peerThink`). The quick-match request runs the whole
    /// quick match first: the owner then calls [`PeerThread::do_quick_match_step`] each pass while
    /// [`PeerThread::quick_match_active`] is true, instead of `step`.
    pub fn thread_step<H: PeerSdkHost>(&mut self, host: &mut H) {
        if let Some(req) = self.queue.get_request() {
            self.handle_request(host, req);
        }

        if self.g.is_thread_hosting != 0 && self.g.want_state_changed_heartbeat {
            let now = host.time_get_time();
            if now > self.g.last_state_changed_heartbeat.wrapping_add(HEARTBEAT_INTERVAL) {
                self.g.last_state_changed_heartbeat = now;
                self.g.want_state_changed_heartbeat = false;
                host.peer_state_changed();
            }
        }

        // update the network
        if host.peer_is_connected() {
            if host.qr2_socket_is_open() {
                // check hosting activity
                host.qr2_check_queries();
            }
            host.peer_think();
        }
    }

    /// The request `switch` of `Thread_Function`.
    pub fn handle_request<H: PeerSdkHost>(&mut self, host: &mut H, req: PeerRequest) {
        let buddy = host.buddy_queue();
        match req.peer_request_type {
            PeerRequestType::Login => {
                self.is_connecting = true;
                self.original_name = req.nick.clone();
                self.login_name = req.nick.clone();
                self.profile_id = req.login_profile_id;
                self.password = req.password.clone();
                self.email = req.email.clone();
                self.publish();
                host.peer_connect(&req.nick, req.login_profile_id);
                if self.is_connected {
                    let ret = self.do_cd_key_authentication(host);
                    if ret != SerialAuthResult::Ok {
                        self.is_connecting = false;
                        self.is_connected = false;
                        self.queue.set_serial_auth_result(ret);
                        self.publish();
                        host.peer_disconnect();
                    }
                }
                self.is_connecting = false;
                self.publish();
            }
            PeerRequestType::Logout => {
                self.is_connecting = false;
                self.is_connected = false;
                self.publish();
                host.peer_disconnect();
            }
            PeerRequestType::JoinGroupRoom => {
                self.group_room_id = req.group_room_id;
                self.g.is_thread_hosting = 0;
                self.g.last_state_changed_heartbeat = 0;
                self.g.want_state_changed_heartbeat = false;
                host.peer_stop_game();
                host.peer_leave_room(RoomType::GroupRoom);
                host.peer_leave_room(RoomType::StagingRoom);
                if host.qr2_socket_is_open() {
                    host.qr2_socket_close();
                }
                self.is_hosting = false;
                self.local_room_id = self.group_room_id;
                host.peer_join_group_room(req.group_room_id);
            }
            PeerRequestType::LeaveGroupRoom => {
                self.group_room_id = 0;
                update_buddy_status(buddy.as_ref(), BuddyStatus::Online, 0, b"");
                host.peer_leave_room(RoomType::GroupRoom);
                host.peer_leave_room(RoomType::StagingRoom);
                self.is_hosting = false;
            }
            PeerRequestType::JoinStagingRoom => {
                self.group_room_id = 0;
                update_buddy_status(buddy.as_ref(), BuddyStatus::Online, 0, b"");
                host.peer_leave_room(RoomType::GroupRoom);
                host.peer_leave_room(RoomType::StagingRoom);
                self.is_hosting = false;
                let server = self.find_server_by_id(host, req.staging_room_id);
                self.local_staging_server_name = req.text.clone();
                self.local_room_id = req.staging_room_id;
                if let Some(server) = server {
                    host.peer_join_staging_room(server, &req.password);
                } else {
                    let mut resp = PeerResponse::default();
                    resp.peer_response_type = PeerResponseType::JoinStagingRoom;
                    resp.join_staging_room.id = req.staging_room_id;
                    resp.join_staging_room.ok = false;
                    resp.join_staging_room.result = PeerJoinResult::JoinFailed;
                    self.queue.add_response(resp);
                }
            }
            PeerRequestType::LeaveStagingRoom => {
                self.group_room_id = 0;
                update_buddy_status(buddy.as_ref(), BuddyStatus::Online, 0, b"");
                host.peer_leave_room(RoomType::GroupRoom);
                host.peer_leave_room(RoomType::StagingRoom);
                self.g.is_thread_hosting = 0;
                self.g.last_state_changed_heartbeat = 0;
                self.g.want_state_changed_heartbeat = false;
                if self.is_hosting {
                    host.peer_stop_game();
                    if host.qr2_socket_is_open() {
                        host.qr2_socket_close();
                    }
                    self.is_hosting = false;
                }
            }
            PeerRequestType::MessagePlayer => {
                let s = host.wide_to_multi_byte(&req.text);
                let kind = if req.message_is_action { PeerMessageKind::Action } else { PeerMessageKind::Normal };
                host.peer_message_player(&req.nick, &s, kind);
            }
            PeerRequestType::MessageRoom => {
                let s = host.wide_to_multi_byte(&req.text);
                let kind = if req.message_is_action { PeerMessageKind::Action } else { PeerMessageKind::Normal };
                let room = if self.group_room_id != 0 { RoomType::GroupRoom } else { RoomType::StagingRoom };
                host.peer_message_room(room, &s, kind);
            }
            PeerRequestType::PushStats => {
                let p = req.stats_to_push;
                self.value_buffers[0] = format!("{}", p.locale);
                self.value_buffers[1] = format!("{}", p.wins);
                self.value_buffers[2] = format!("{}", p.losses);
                self.value_buffers[3] = format!("{}", p.rank_points);
                self.value_buffers[4] = format!("{}", p.side);
                self.value_buffers[5] = format!("{}", p.preorder as i32);
                self.push_stats_to_room(host);
            }
            PeerRequestType::SetGameOptions => {
                self.map_name = req.game_opts_map_name.clone();
                self.num_players = req.game_options.num_players;
                self.num_observers = req.game_options.num_observers;
                self.max_players = req.game_options.max_players;
                for i in 0..SLOTS {
                    self.player_names[i] = req.game_opts_player_names[i].clone();
                    self.player_wins[i] = req.game_options.wins[i];
                    self.player_losses[i] = req.game_options.losses[i];
                    self.player_profile_id[i] = req.game_options.profile_id[i];
                    self.player_factions[i] = req.game_options.faction[i];
                    self.player_colors[i] = req.game_options.color[i];
                }
                self.g.want_state_changed_heartbeat = true;
                // send the full string to people in the room
                host.peer_utm_room(RoomType::StagingRoom, b"SL/", &req.options);
            }
            PeerRequestType::GetExtendedStagingRoomInfo => {
                if let Some(server) = self.find_server_by_id(host, req.staging_room_id) {
                    host.peer_update_game(server);
                }
                // else DEBUG_LOG("Tried to update non-existent server!")
            }
            PeerRequestType::CreateStagingRoom => self.create_staging_room(host, req, buddy.as_ref()),
            PeerRequestType::StartGameList => {
                self.saw_complete_game_list = false;
                let mut resp = PeerResponse::default();
                resp.peer_response_type = PeerResponseType::StagingRoom;
                resp.staging_room.action = PeerListAction::Clear;
                resp.staging_room.is_staging = true;
                resp.staging_room.percent_complete = 0;
                self.clear_servers();
                self.queue.add_response(resp);
                // allKeysArray: MAPNAME, GAMEVER, GAMENAME, EXECRC, INICRC, PW, OBS, USE_STATS, LADIP, LADPORT,
                // PINGSTR, NUMOBS, NUMPLAYER, MAXPLAYER, HOSTNAME (15 keys; numbers come from the SDK)
                host.peer_start_listing_games(&[], req.game_list_restrict);
            }
            PeerRequestType::StopGameList => host.peer_stop_listing_games(),
            PeerRequestType::StartGame => host.peer_start_game_stop_reporting(),
            PeerRequestType::UtmPlayer => {
                if !req.nick.is_empty() {
                    host.peer_utm_player(&req.nick, &req.id, &req.options);
                }
            }
            PeerRequestType::UtmRoom => {
                let room = if req.utm_is_staging_room { RoomType::StagingRoom } else { RoomType::GroupRoom };
                host.peer_utm_room(room, &req.id, &req.options);
            }
            PeerRequestType::StartQuickMatch => {
                self.qm_info = req;
                self.do_quick_match_begin(host);
            }
            // upstream's switch has no case for these; they are ignored outside a quick match
            PeerRequestType::WidenQuickMatchSearch | PeerRequestType::StopQuickMatch => {}
        }
    }

    /// `PEERREQUEST_CREATESTAGINGROOM` (Thread_Function): leaves the lobby (unless the game list is restricted),
    /// opens the QR2 socket (port 6500 + the low byte of the local IP if the preferred port is below 1024; falls
    /// back to the SDK default port when the bind fails), creates the room named "<login> <game name>", posts
    /// `PEERRESPONSE_CREATESTAGINGROOM`, and on success resets the hosted-game fields and pushes stats; on
    /// failure rejoins the old group room.
    fn create_staging_room<H: PeerSdkHost>(&mut self, host: &mut H, req: PeerRequest, buddy: Option<&BuddyMessageQueue>) {
        let old_group_id = self.group_room_id;
        self.group_room_id = 0;
        update_buddy_status(buddy, BuddyStatus::Online, 0, b"");
        if !req.staging_room_creation.restrict_game_list {
            host.peer_leave_room(RoomType::GroupRoom);
            host.peer_leave_room(RoomType::StagingRoom);
        }
        self.is_hosting = true;

        if !host.qr2_socket_is_open() {
            // allocate a port
            if self.g.preferred_qr_port < 1024 {
                self.g.preferred_qr_port = 6500 + (u32::from_be(self.g.local_ip) & 0xff);
            }
        } else {
            host.qr2_socket_close();
        }
        if !host.qr2_socket_open(self.g.local_ip, self.g.preferred_qr_port as u16) {
            // Could not bind!  Falling back to GameSpy's default port
            host.qr2_socket_close();
            self.g.preferred_qr_port = 0;
        }
        let mut composite_game = self.login_name.clone();
        composite_game.push(b' ');
        composite_game.extend_from_slice(&host.wide_to_multi_byte(&req.text));
        self.local_staging_server_name = req.text.clone();
        self.player_names[0] = self.login_name.clone();
        let res = host.peer_create_staging_room_with_socket(
            &composite_game,
            MAX_SLOTS,
            &req.password,
            self.g.preferred_qr_port as u16,
        );

        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::CreateStagingRoom;
        resp.create_staging_room_result = res;
        self.queue.add_response(resp);

        if res != PeerJoinResult::Success && res != PeerJoinResult::AlreadyInRoom {
            self.local_room_id = old_group_id;
            if req.staging_room_creation.restrict_game_list {
                host.peer_leave_room(RoomType::StagingRoom);
            } else {
                host.peer_join_group_room(old_group_id);
            }
            self.is_hosting = false;
            self.local_staging_server_name = Vec::new();
            self.player_names[0] = Vec::new();
        } else {
            if req.staging_room_creation.restrict_game_list {
                host.peer_leave_room(RoomType::GroupRoom);
            }
            self.g.is_thread_hosting = 1;
            self.g.last_state_changed_heartbeat = host.time_get_time(); // wait the full interval before updating state
            self.g.want_state_changed_heartbeat = false;
            self.is_hosting = true;
            self.allow_observers = req.staging_room_creation.allow_observers;
            self.use_stats = req.staging_room_creation.use_stats;
            self.map_name = Vec::new();
            for i in 0..SLOTS {
                self.player_names[i] = Vec::new();
                self.player_wins[i] = 0;
                self.player_losses[i] = 0;
                self.player_profile_id[i] = 0;
                self.player_factions[i] = 0;
                self.player_colors[i] = 0;
            }
            self.has_password = !req.password.is_empty();
            self.player_names[0] = self.login_name.clone();
            self.exe_crc = req.staging_room_creation.exe_crc;
            self.ini_crc = req.staging_room_creation.ini_crc;
            self.game_version = req.staging_room_creation.game_version;
            self.local_staging_server_name = req.text.clone();
            self.ladder_ip = req.ladder_ip.clone();
            self.ping_str = req.host_ping_str.clone();
            self.ladder_port = req.staging_room_creation.lad_port;

            self.push_stats_to_room(host);

            let name = host.wide_to_multi_byte(&self.local_staging_server_name);
            update_buddy_status(buddy, BuddyStatus::Staging, 0, &name);
        }
    }

    // ---- quick match ----

    /// Whether `doQuickMatch`'s loop is still running (the owner keeps calling
    /// [`PeerThread::do_quick_match_step`] instead of [`PeerThread::thread_step`] while it is).
    pub fn quick_match_active(&self) -> bool {
        self.qm_loop_active
    }

    /// The head of `PeerThreadClass::doQuickMatch`: `QM_JOININGQMCHANNEL`, remembers the matchbot's profile id and
    /// the QM group room, forgets any matchbot, sets the buddy status to "Matching".
    fn do_quick_match_begin<H: PeerSdkHost>(&mut self, host: &mut H) {
        self.qm_status = QmStatus::JoiningQmChannel;
        self.g.matchbot_profile_id = self.qm_info.qm.bot_id;
        let room = self.qm_info.qm.room_id;
        self.set_qm_group_room(room);
        self.saw_matchbot = false;
        update_buddy_status(host.buddy_queue().as_ref(), BuddyStatus::Matching, 0, b"");
        self.qm_loop_active = true;
    }

    /// `quickmatchEnumPlayersCallback` (PeerThread.cpp): `success`/`nick` None ends the enumeration; otherwise
    /// remembers the matchbot when the player's profile id is the QM bot's and posts a `PEERRESPONSE_PLAYERJOIN`.
    /// `id` is the player's profile id (`peerGetPlayerProfileID`).
    pub fn quickmatch_enum_players_callback<H: PeerSdkHost>(&mut self, host: &mut H, success: bool, room_type: RoomType, nick: Option<&[u8]>) {
        let nick = match (success, nick) {
            (true, Some(n)) if !n.is_empty() => n,
            _ => {
                self.saw_end_of_enum_players();
                return;
            }
        };
        let id = host.peer_get_player_profile_id(nick);
        if id == self.g.matchbot_profile_id {
            self.saw_matchbot(nick);
        }
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerJoin;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        resp.player.ip = 0;
        self.queue.add_response(resp);
    }

    /// `PeerThreadClass::handleQMMatch`: only while `QM_WORKING`: becomes `QM_MATCHED`, leaves the group room,
    /// tells every other player "We're matched!" and posts the `PEERRESPONSE_QUICKMATCHSTATUS` with the players'
    /// IPs, sides, colours and NATs (empty slots zeroed).
    pub fn handle_qm_match<H: PeerSdkHost>(
        &mut self,
        host: &mut H,
        map_index: i32,
        seed: i32,
        player_name: &[Option<Vec<u8>>; SLOTS],
        player_ip: &[Option<Vec<u8>>; SLOTS],
        player_side: &[Option<Vec<u8>>; SLOTS],
        player_color: &[Option<Vec<u8>>; SLOTS],
        player_nat: &[Option<Vec<u8>>; SLOTS],
    ) {
        if self.qm_status != QmStatus::Working {
            return;
        }
        self.qm_status = QmStatus::Matched;
        host.peer_leave_room(RoomType::GroupRoom);

        for i in 0..SLOTS {
            if let Some(name) = &player_name[i] {
                if !name.eq_ignore_ascii_case(&self.login_name) {
                    host.peer_message_player(name, b"We're matched!", PeerMessageKind::Normal);
                }
            }
        }

        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::QuickMatchStatus;
        resp.qm_status.status = QmStatus::Matched;
        let a = super::persistent_storage_thread::atoi;
        for i in 0..SLOTS {
            if let Some(name) = &player_name[i] {
                resp.staging_room_player_names[i] = name.clone();
                resp.qm_status.ip[i] = a(player_ip[i].as_deref().unwrap_or(b"")) as u32;
                resp.qm_status.side[i] = a(player_side[i].as_deref().unwrap_or(b""));
                resp.qm_status.color[i] = a(player_color[i].as_deref().unwrap_or(b""));
                resp.qm_status.nat[i] = a(player_nat[i].as_deref().unwrap_or(b""));
            } else {
                resp.staging_room_player_names[i] = Vec::new();
                resp.qm_status.ip[i] = 0;
                resp.qm_status.side[i] = 0;
                resp.qm_status.color[i] = 0;
                resp.qm_status.nat[i] = 0;
            }
        }
        resp.qm_status.seed = seed;
        resp.qm_status.map_idx = map_index;
        self.queue.add_response(resp);
    }

    /// One iteration of `doQuickMatch`'s `while (!done && running)` loop. Returns true while the loop continues;
    /// false when it is done (the tail posts the buddy status "Online").
    pub fn do_quick_match_step<H: PeerSdkHost>(&mut self, host: &mut H, running: bool) -> bool {
        let mut done = false;
        if !running {
            done = true;
        } else if !host.peer_is_connected() {
            done = true;
        } else {
            // update the network
            host.peer_think();

            if let Some(incoming) = self.queue.get_request() {
                match incoming.peer_request_type {
                    PeerRequestType::WidenQuickMatchSearch => {
                        if self.qm_status != QmStatus::Idle && self.qm_status != QmStatus::Stopped && self.saw_matchbot {
                            host.peer_message_player(&self.matchbot_name.clone(), b"\\WIDEN", PeerMessageKind::Normal);
                        }
                    }
                    PeerRequestType::StopQuickMatch
                    | PeerRequestType::Logout
                    | PeerRequestType::LeaveGroupRoom => {
                        self.qm_status = QmStatus::Stopped;
                        host.peer_leave_room(RoomType::GroupRoom);
                        done = true;
                    }
                    PeerRequestType::UtmPlayer => {
                        host.peer_utm_player(&incoming.nick, &incoming.id, &incoming.options);
                    }
                    _ => {
                        // DEBUG_CRASH("Unanticipated request %d to peer thread!")
                    }
                }
            }

            if !done {
                // do the next bit of QM
                match self.qm_status {
                    QmStatus::JoiningQmChannel => {
                        let mut resp = PeerResponse::default();
                        resp.peer_response_type = PeerResponseType::QuickMatchStatus;
                        resp.qm_status.status = QmStatus::JoiningQmChannel;
                        self.queue.add_response(resp.clone());

                        self.group_room_id = self.qm_group_room;
                        host.peer_leave_room(RoomType::GroupRoom);
                        host.peer_leave_room(RoomType::StagingRoom);
                        self.is_hosting = false;
                        self.local_room_id = self.group_room_id;
                        self.room_joined = false;
                        host.peer_join_group_room(self.local_room_id);
                        if self.room_joined {
                            resp.peer_response_type = PeerResponseType::QuickMatchStatus;
                            resp.qm_status.status = QmStatus::LookingForBot;
                            self.queue.add_response(resp);

                            self.qm_status = QmStatus::LookingForBot;
                            self.saw_matchbot = false;
                            self.saw_end_of_enum_players = false;
                            host.peer_enum_players_group_room();
                        } else {
                            resp.peer_response_type = PeerResponseType::QuickMatchStatus;
                            resp.qm_status.status = QmStatus::CouldNotFindBot;
                            self.queue.add_response(resp);
                            done = true;
                            self.qm_status = QmStatus::Stopped;
                        }
                    }
                    QmStatus::LookingForBot => {
                        if self.saw_end_of_enum_players {
                            if self.saw_matchbot {
                                let msg = self.build_cinfo(host);
                                host.peer_message_player(&self.matchbot_name.clone(), &msg, PeerMessageKind::Normal);
                                self.qm_status = QmStatus::Working;
                                let mut resp = PeerResponse::default();
                                resp.peer_response_type = PeerResponseType::QuickMatchStatus;
                                resp.qm_status.status = QmStatus::SentInfo;
                                self.queue.add_response(resp);
                            } else {
                                // no QM bot.  Bail.
                                let mut resp = PeerResponse::default();
                                resp.peer_response_type = PeerResponseType::QuickMatchStatus;
                                resp.qm_status.status = QmStatus::CouldNotFindBot;
                                self.queue.add_response(resp);

                                self.qm_status = QmStatus::Stopped;
                                host.peer_leave_room(RoomType::GroupRoom);
                                done = true;
                            }
                        }
                    }
                    QmStatus::Matched => {
                        // leave QM channel, and clean up.  Our work here is done.
                        host.peer_leave_room(RoomType::GroupRoom);
                        host.peer_leave_room(RoomType::StagingRoom);
                        self.is_hosting = false;

                        self.qm_status = QmStatus::Stopped;
                        host.peer_leave_room(RoomType::GroupRoom);
                        done = true;
                    }
                    _ => {}
                }
            }
        }
        if done {
            self.qm_loop_active = false;
            update_buddy_status(host.buddy_queue().as_ref(), BuddyStatus::Online, 0, b"");
            return false;
        }
        true
    }

    /// The `\CINFO...` message of `QM_LOOKINGFORBOT`: widen time, ladder id and pass CRC, point range, points,
    /// discons, players, pings, the (host-order) local IP, side, colour, NAT, exe/ini CRCs and the map mask.
    fn build_cinfo<H: PeerSdkHost>(&self, host: &mut H) -> Vec<u8> {
        let q = &self.qm_info.qm;
        let mut msg = String::from("\\CINFO");
        msg.push_str(&format!("\\Widen\\{}", q.widen_time));
        msg.push_str(&format!("\\LadID\\{}", q.ladder_id));
        msg.push_str(&format!("\\LadPass\\{}", q.ladder_pass_crc));
        msg.push_str(&format!("\\PointsMin\\{}", q.min_point_percentage));
        msg.push_str(&format!("\\PointsMax\\{}", q.max_point_percentage));
        msg.push_str(&format!("\\Points\\{}", q.points));
        msg.push_str(&format!("\\Discons\\{}", q.discons));
        msg.push_str(&format!("\\DisconMax\\{}", q.max_discons));
        msg.push_str(&format!("\\NumPlayers\\{}", q.num_players));
        let mut out = msg.into_bytes();
        out.extend_from_slice(b"\\Pings\\");
        out.extend_from_slice(&q.pings);
        // not ntohl(localIP), as we need EXTERNAL address for proper NAT negotiation!
        let ext = u32::from_be(host.peer_get_local_ip());
        let mut tail = String::new();
        tail.push_str(&format!("\\IP\\{}", ext));
        tail.push_str(&format!("\\Side\\{}", q.side));
        tail.push_str(&format!("\\Color\\{}", q.color));
        tail.push_str(&format!("\\NAT\\{}", q.nat));
        tail.push_str(&format!("\\EXE\\{}", q.exe_crc));
        tail.push_str(&format!("\\INI\\{}", q.ini_crc));
        tail.push_str("\\Maps\\");
        out.extend_from_slice(tail.as_bytes());
        for m in &self.qm_info.qm_maps {
            out.push(if *m { b'1' } else { b'0' });
        }
        out
    }

    // ---- callbacks ----

    /// `getPlayerInfo` (PeerThread.cpp): fills a response's player fields from the SDK and the broadcast-key stats
    /// (`USE_BROADCAST_KEYS`): ip/profile id, locale index as text, wins, losses, rank points, side, preorder and
    /// the player's flags in `room_type`.
    fn get_player_info<H: PeerSdkHost>(&self, host: &mut H, nick: &[u8], room_type: RoomType, resp: &mut PeerResponse) {
        if let Some((ip, id)) = host.peer_get_player_info_no_wait(nick) {
            resp.player.ip = ip;
            resp.player.profile_id = id;
        }
        let locale_index = self.lookup_stat_for_player(room_type, nick, b"b_locale");
        resp.locale = format!("{}", locale_index).into_bytes();
        resp.player.wins = self.lookup_stat_for_player(room_type, nick, b"b_wins");
        resp.player.losses = self.lookup_stat_for_player(room_type, nick, b"b_losses");
        resp.player.rank_points = self.lookup_stat_for_player(room_type, nick, b"b_points");
        resp.player.side = self.lookup_stat_for_player(room_type, nick, b"b_side");
        resp.player.preorder = self.lookup_stat_for_player(room_type, nick, b"b_pre");
        resp.player.flags = host.peer_get_player_flags(nick, room_type);
    }

    /// `PeerThreadClass::connectCallback`: on failure posts `DISCONNECT_COULDNOTCONNECT`; on success sets the buddy
    /// status Online, marks connected, posts `PEERRESPONSE_LOGIN` with the internal and external IPs, asks the stats
    /// thread for the player's stats and lists the group rooms.
    pub fn connect_callback<H: PeerSdkHost>(&mut self, host: &mut H, success: bool) {
        let mut resp = PeerResponse::default();
        if !success {
            resp.peer_response_type = PeerResponseType::Disconnect;
            resp.discon_reason = DisconnectReason::CouldNotConnect;
            self.queue.add_response(resp);
            return;
        }

        update_buddy_status(host.buddy_queue().as_ref(), BuddyStatus::Online, 0, b"");

        self.is_connected = true;
        self.publish();
        resp.peer_response_type = PeerResponseType::Login;
        resp.player.profile_id = self.profile_id;
        resp.nick = self.login_name.clone();
        if let Some(ip) = host.local_chat_connection_address() {
            self.g.local_ip = ip;
        }
        host.chat_set_local_ip(self.g.local_ip);
        resp.player.internal_ip = u32::from_be(self.g.local_ip);
        resp.player.external_ip = u32::from_be(host.peer_get_local_ip());
        self.queue.add_response(resp);

        let mut ps_req = PsRequest::new();
        ps_req.request_type = PsRequestType::ReadPlayerStats;
        ps_req.player.id = self.profile_id;
        ps_req.nick = self.original_name.clone();
        ps_req.email = self.email.clone();
        ps_req.password = self.password.clone();
        host.ps_queue().add_request(ps_req);

        host.peer_list_group_rooms();
    }

    /// `PeerThreadClass::nickErrorCallback`: for a taken nick, retries with a `{N}` suffix (an existing `{d}` suffix
    /// is incremented) up to `{9}`, then gives up with `DISCONNECT_NICKTAKEN`; any other error is
    /// `DISCONNECT_BADNICK`. Giving up cancels the connect.
    pub fn nick_error_callback<H: PeerSdkHost>(&mut self, host: &mut H, error: PeerNickError, nick: &[u8]) {
        if error == PeerNickError::InUse {
            let len = nick.len();
            let mut nick_str = nick.to_vec();
            let mut new_val: i32 = 0;
            if len >= 3 && nick[len - 1] == b'}' && nick[len - 3] == b'{' && nick[len - 2].is_ascii_digit() {
                new_val = (nick[len - 2] - b'0') as i32 + 1;
                nick_str.truncate(len - 3);
            }
            if new_val < 10 {
                nick_str.push(b'{');
                nick_str.push(b'0' + new_val as u8);
                nick_str.push(b'}');
                // Retry the connect with a similar nick.
                self.login_name = nick_str.clone();
                host.peer_retry_with_nick(Some(&nick_str));
            } else {
                let mut resp = PeerResponse::default();
                resp.peer_response_type = PeerResponseType::Disconnect;
                resp.discon_reason = DisconnectReason::NickTaken;
                self.queue.add_response(resp);
                // Cancel the connect.
                host.peer_retry_with_nick(None);
            }
        } else {
            let mut resp = PeerResponse::default();
            resp.peer_response_type = PeerResponseType::Disconnect;
            resp.discon_reason = DisconnectReason::BadNick;
            self.queue.add_response(resp);
            // Cancel the connect.
            host.peer_retry_with_nick(None);
        }
    }

    /// `disconnectedCallback`: marks the thread disconnected and posts `PEERRESPONSE_DISCONNECT`, whose reason is
    /// `DISCONNECT_LOSTCON` unless the serial authentication had failed.
    pub fn disconnected_callback(&mut self) {
        self.mark_as_disconnected();
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::Disconnect;
        resp.discon_reason = DisconnectReason::LostCon;
        match self.queue.get_serial_auth_result() {
            SerialAuthResult::NonExistent => resp.discon_reason = DisconnectReason::SerialNotPresent,
            SerialAuthResult::AuthFailed => resp.discon_reason = DisconnectReason::SerialInvalid,
            SerialAuthResult::Banned => resp.discon_reason = DisconnectReason::SerialBanned,
            SerialAuthResult::Ok => {}
        }
        self.queue.add_response(resp);
    }

    /// `roomMessageCallback`: posts the room message (text as one wide line); in a running quick match, a
    /// `MBOT:POOLSIZE` message from the matchbot answers a `QM_POOLSIZE` status with the pool size of the thread's
    /// ladder (the "<pool> <size>" pairs after the command).
    pub fn room_message_callback<H: PeerSdkHost>(&mut self, host: &mut H, nick: &[u8], message: &[u8], kind: PeerMessageKind) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::Message;
        resp.nick = nick.to_vec();
        resp.text = host.multi_byte_to_wide_single_line(message);
        resp.message.is_private = false;
        resp.message.is_action = kind == PeerMessageKind::Action;
        self.queue.add_response(resp.clone());

        // The profile id is read after the response was queued, exactly as upstream (so the queued copy has 0).
        if let Some((_ip, id)) = host.peer_get_player_info_no_wait(nick) {
            resp.message.profile_id = id;
        }

        if self.qm_status != QmStatus::Idle && self.qm_status != QmStatus::Stopped {
            if resp.message.profile_id == self.g.matchbot_profile_id {
                let mut pos = 0;
                let cmd = strtok_space(message, &mut pos);
                if cmd == Some(&b"MBOT:POOLSIZE"[..]) {
                    let mut pool_size = 0;
                    loop {
                        let pool_str = strtok_space(message, &mut pos);
                        let size_str = strtok_space(message, &mut pos);
                        if let (Some(p), Some(s)) = (pool_str, size_str) {
                            let pool = super::persistent_storage_thread::atoi(p);
                            let size = super::persistent_storage_thread::atoi(s);
                            if pool == self.get_qm_ladder() {
                                pool_size = size;
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    let mut r = PeerResponse::default();
                    r.peer_response_type = PeerResponseType::QuickMatchStatus;
                    r.qm_status.status = QmStatus::PoolSize;
                    r.qm_status.pool_size = pool_size;
                    self.queue.add_response(r);
                }
            }
        }
    }

    /// `gameStartedCallback`: posts `PEERRESPONSE_GAMESTART`.
    pub fn game_started_callback(&mut self) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::GameStart;
        self.queue.add_response(resp);
    }

    /// `playerMessageCallback`: posts the private message (profile id from the SDK); in a running quick match a
    /// private matchbot `MBOT:MATCHED <map> <seed> (<nick> <ip> <side> <color> <nat>)x8` becomes
    /// [`PeerThread::handle_qm_match`] when at least two players are listed, `MBOT:WORKING [<pool>]` a `QM_WORKING`
    /// status and `MBOT:WIDENINGSEARCH` a `QM_WIDENINGSEARCH` status.
    pub fn player_message_callback<H: PeerSdkHost>(&mut self, host: &mut H, nick: &[u8], message: &[u8], kind: PeerMessageKind) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::Message;
        resp.nick = nick.to_vec();
        resp.text = host.multi_byte_to_wide_single_line(message);
        resp.message.is_private = true;
        resp.message.is_action = kind == PeerMessageKind::Action;
        if let Some((_ip, id)) = host.peer_get_player_info_no_wait(nick) {
            resp.message.profile_id = id;
        }
        self.queue.add_response(resp.clone());

        if self.qm_status != QmStatus::Idle && self.qm_status != QmStatus::Stopped {
            if resp.message.is_private && resp.message.profile_id == self.g.matchbot_profile_id {
                let mut pos = 0;
                let cmd = strtok_space(message, &mut pos);
                if cmd == Some(&b"MBOT:MATCHED"[..]) {
                    let map_num = strtok_space(message, &mut pos).map(|s| s.to_vec());
                    let seed = strtok_space(message, &mut pos).map(|s| s.to_vec());
                    let mut name: [Option<Vec<u8>>; SLOTS] = Default::default();
                    let mut ip: [Option<Vec<u8>>; SLOTS] = Default::default();
                    let mut side: [Option<Vec<u8>>; SLOTS] = Default::default();
                    let mut color: [Option<Vec<u8>>; SLOTS] = Default::default();
                    let mut nat: [Option<Vec<u8>>; SLOTS] = Default::default();
                    let mut num_players = 0;
                    for i in 0..SLOTS {
                        name[i] = strtok_space(message, &mut pos).map(|s| s.to_vec());
                        ip[i] = strtok_space(message, &mut pos).map(|s| s.to_vec());
                        side[i] = strtok_space(message, &mut pos).map(|s| s.to_vec());
                        color[i] = strtok_space(message, &mut pos).map(|s| s.to_vec());
                        nat[i] = strtok_space(message, &mut pos).map(|s| s.to_vec());
                        if nat[i].is_some() {
                            num_players += 1;
                        } else {
                            name[i] = None;
                            ip[i] = None;
                            side[i] = None;
                            color[i] = None;
                            nat[i] = None;
                        }
                    }
                    if num_players > 1 {
                        // woohoo!  got everything needed for a match!
                        // (atoi(NULL) is undefined upstream when the map or seed is missing; 0 here)
                        let a = super::persistent_storage_thread::atoi;
                        let m = a(map_num.as_deref().unwrap_or(b""));
                        let s = a(seed.as_deref().unwrap_or(b""));
                        self.handle_qm_match(host, m, s, &name, &ip, &side, &color, &nat);
                    }
                } else if cmd == Some(&b"MBOT:WORKING"[..]) {
                    let pool_size = strtok_space(message, &mut pos)
                        .map(super::persistent_storage_thread::atoi)
                        .unwrap_or(0);
                    let mut r = PeerResponse::default();
                    r.peer_response_type = PeerResponseType::QuickMatchStatus;
                    r.qm_status.status = QmStatus::Working;
                    r.qm_status.pool_size = pool_size;
                    self.queue.add_response(r);
                } else if cmd == Some(&b"MBOT:WIDENINGSEARCH"[..]) {
                    let mut r = PeerResponse::default();
                    r.peer_response_type = PeerResponseType::QuickMatchStatus;
                    r.qm_status.status = QmStatus::WideningSearch;
                    self.queue.add_response(r);
                }
            }
        }
    }

    /// `roomUTMCallback`: staging-room UTMs only; posts `PEERRESPONSE_ROOMUTM`.
    pub fn room_utm_callback(&mut self, room_type: RoomType, nick: &[u8], command: &[u8], parameters: &[u8]) {
        if room_type != RoomType::StagingRoom {
            return;
        }
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::RoomUtm;
        resp.nick = nick.to_vec();
        resp.command = command.to_vec();
        resp.command_options = parameters.to_vec();
        self.queue.add_response(resp);
    }

    /// `playerUTMCallback`: posts `PEERRESPONSE_PLAYERUTM`.
    pub fn player_utm_callback(&mut self, nick: &[u8], command: &[u8], parameters: &[u8]) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerUtm;
        resp.nick = nick.to_vec();
        resp.command = command.to_vec();
        resp.command_options = parameters.to_vec();
        self.queue.add_response(resp);
    }

    /// `roomKeyChangedCallback` (USE_BROADCAST_KEYS): records the changed key and posts `PEERRESPONSE_PLAYERINFO`.
    /// Ignores a call without nick/key/value (the SDK's "(END)" sentinel).
    pub fn room_key_changed_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, nick: Option<&[u8]>, key: Option<&[u8]>, val: Option<&[u8]>) {
        let (nick, key, val) = match (nick, key, val) {
            (Some(n), Some(k), Some(v)) => (n, k, v),
            _ => return,
        };
        self.track_stats_for_player(room_type, nick, key, val);
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerInfo;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `getRoomKeysCallback`: records every returned key and posts one `PEERRESPONSE_PLAYERINFO`.
    pub fn get_room_keys_callback<H: PeerSdkHost>(
        &mut self,
        host: &mut H,
        success: bool,
        room_type: RoomType,
        nick: Option<&[u8]>,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
    ) {
        let nick = match nick {
            Some(n) if success && !keys.is_empty() => n,
            _ => return,
        };
        for i in 0..keys.len().min(values.len()) {
            self.track_stats_for_player(room_type, nick, &keys[i], &values[i]);
        }
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerInfo;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `globalKeyChangedCallback`: posts `PEERRESPONSE_PLAYERINFO` for the nick, in the group room if the thread is
    /// in one, else the staging room.
    pub fn global_key_changed_callback<H: PeerSdkHost>(&mut self, host: &mut H, nick: Option<&[u8]>) {
        let nick = match nick {
            Some(n) => n,
            None => return,
        };
        let room_type = if self.get_current_group_room() != 0 { RoomType::GroupRoom } else { RoomType::StagingRoom };
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerInfo;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `playerJoinedCallback`: posts `PEERRESPONSE_PLAYERJOIN` with the player's info.
    pub fn player_joined_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, nick: Option<&[u8]>) {
        let nick = match nick {
            Some(n) => n,
            None => return,
        };
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerJoin;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `playerLeftCallback`: posts `PEERRESPONSE_PLAYERLEFT` with the player's info; if the departed player is the
    /// matchbot of a running quick match, also posts `QM_COULDNOTFINDBOT` and queues `PEERREQUEST_STOPQUICKMATCH`.
    pub fn player_left_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, nick: &[u8]) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerLeft;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        resp.player.profile_id = 0;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);

        if self.qm_status != QmStatus::Idle && self.qm_status != QmStatus::Stopped {
            if self.matchbot_name.eq_ignore_ascii_case(nick) {
                // matchbot left - bail
                let mut r = PeerResponse::default();
                r.peer_response_type = PeerResponseType::QuickMatchStatus;
                r.qm_status.status = QmStatus::CouldNotFindBot;
                self.queue.add_response(r);

                let mut req = PeerRequest::default();
                req.peer_request_type = PeerRequestType::StopQuickMatch;
                self.queue.add_request(req);
            }
        }
    }

    /// `playerChangedNickCallback`: posts `PEERRESPONSE_PLAYERCHANGEDNICK` (new nick, old nick, info of the new).
    pub fn player_changed_nick_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, old_nick: &[u8], new_nick: &[u8]) {
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerChangedNick;
        resp.nick = new_nick.to_vec();
        resp.old_nick = old_nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, new_nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `playerInfoCallback`: posts `PEERRESPONSE_PLAYERINFO`.
    pub fn player_info_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, nick: Option<&[u8]>) {
        let nick = match nick {
            Some(n) => n,
            None => return,
        };
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerInfo;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `playerFlagsChangedCallback`: posts `PEERRESPONSE_PLAYERCHANGEDFLAGS`.
    pub fn player_flags_changed_callback<H: PeerSdkHost>(&mut self, host: &mut H, room_type: RoomType, nick: Option<&[u8]>) {
        let nick = match nick {
            Some(n) => n,
            None => return,
        };
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::PlayerChangedFlags;
        resp.nick = nick.to_vec();
        resp.player.room_type = room_type;
        self.get_player_info(host, nick, room_type, &mut resp);
        self.queue.add_response(resp);
    }

    /// `joinRoomCallback` (PeerThread.cpp): on success pushes our stats to the room and asks for the group room's;
    /// for a group room clears its stats, posts `PEERRESPONSE_JOINGROUPROOM`, notes it was joined and (unless it
    /// is the quick-match room) sets the buddy status to the lobby; for a staging room clears its stats, sets the
    /// buddy status to staging on success, enumerates the room's players (finding the host by the op flag and
    /// asking the stats thread for each player's stats) and posts `PEERRESPONSE_JOINSTAGINGROOM`.
    pub fn join_room_callback<H: PeerSdkHost>(&mut self, host: &mut H, success: bool, result: PeerJoinResult, room_type: RoomType) {
        if success {
            self.push_stats_to_room(host);
            self.get_stats_from_room(host, room_type);
        }
        let buddy = host.buddy_queue();
        match room_type {
            RoomType::GroupRoom => {
                self.clear_player_stats(RoomType::GroupRoom);
                let mut resp = PeerResponse::default();
                resp.peer_response_type = PeerResponseType::JoinGroupRoom;
                resp.join_group_room.id = self.get_local_room_id();
                resp.join_group_room.ok = success;
                self.queue.add_response(resp);
                self.room_joined(success);
                if self.get_qm_group_room() == 0 || self.get_qm_group_room() != self.get_local_room_id() {
                    update_buddy_status(buddy.as_ref(), BuddyStatus::Lobby, self.get_local_room_id(), b"");
                }
            }
            RoomType::StagingRoom => {
                self.clear_player_stats(RoomType::StagingRoom);
                let mut resp = PeerResponse::default();
                resp.peer_response_type = PeerResponseType::JoinStagingRoom;
                resp.join_staging_room.id = self.get_local_room_id();
                resp.join_staging_room.ok = success;
                resp.join_staging_room.result = result;
                if success {
                    let name = host.wide_to_multi_byte(&self.get_local_staging_server_name());
                    update_buddy_status(buddy.as_ref(), BuddyStatus::Staging, 0, &name);
                }

                resp.join_staging_room.is_host_present = false;
                let players = host.peer_enum_players_staging();
                for (index, nick, flags) in players {
                    self.staging_room_player_enum(host, &mut resp, index, &nick, flags);
                }
                self.queue.add_response(resp);
            }
            RoomType::TitleRoom => {}
        }
    }

    /// `stagingRoomPlayerEnum` (PeerThread.cpp): per player of the room just joined: finds the host (op flag),
    /// records the nick by slot and asks the stats thread for the player's stats if the profile id is known.
    pub fn staging_room_player_enum<H: PeerSdkHost>(&mut self, host: &mut H, resp: &mut PeerResponse, index: i32, nick: &[u8], flags: i32) {
        if nick.is_empty() {
            return;
        }
        let id = host.peer_get_player_profile_id(nick);
        if host.flags_have_op(flags) {
            resp.join_staging_room.is_host_present = true;
        }
        if index >= 0 && (index as usize) < SLOTS {
            resp.staging_room_player_names[index as usize] = nick.to_vec();
        }
        if id != 0 {
            let mut req = PsRequest::new();
            req.request_type = PsRequestType::ReadPlayerStats;
            req.player.id = id;
            host.ps_queue().add_request(req);
        }
    }

    /// `listGroupRoomsCallback`: posts `PEERRESPONSE_GROUPROOM` with the counts and (if present) the name; the
    /// final call (`groupID == 0`, no name) is posted too. A failed call posts nothing.
    pub fn list_group_rooms_callback(
        &mut self,
        success: bool,
        group_id: i32,
        name: Option<&[u8]>,
        num_waiting: i32,
        max_waiting: i32,
        num_games: i32,
        num_playing: i32,
    ) {
        if !success {
            return;
        }
        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::GroupRoom;
        resp.group_room.id = group_id;
        resp.group_room.num_waiting = num_waiting;
        resp.group_room.max_waiting = max_waiting;
        resp.group_room.num_games = num_games;
        resp.group_room.num_playing = num_playing;
        if let Some(n) = name {
            resp.group_room_name = n.to_vec();
        }
        self.queue.add_response(resp);
    }

    /// `listingGamesCallback` (PeerThread.cpp): turns a server-browser event into a `PEERRESPONSE_STAGINGROOM`.
    /// A server that lost its keyvals is treated as removed; the first `percentListed == 100` posts
    /// `PEERRESPONSE_STAGINGROOMLISTCOMPLETE`; the name is split at its first space into host and game; an add or
    /// update reads every key from the server (password, observers, stats, version, CRCs, ladder, pings, player
    /// slots, map) and is dropped (with a full-info request on update) while the CRCs are 0; `PEER_CLEAR` empties
    /// the server map.
    pub fn listing_games_callback<H: PeerSdkHost>(
        &mut self,
        host: &mut H,
        success: bool,
        name: Option<&[u8]>,
        server: Option<SbServer>,
        staging: bool,
        mut msg: PeerListAction,
        percent_listed: i32,
    ) {
        if !success {
            return;
        }
        if name.is_none() && (msg == PeerListAction::Add || msg == PeerListAction::Update) {
            return;
        }
        let mut name: Vec<u8> = name.map(|n| n.to_vec()).unwrap_or_else(|| b"bogus".to_vec());

        if let Some(s) = server {
            if (msg == PeerListAction::Add || msg == PeerListAction::Update) && !host.sb_server_has_keyvals(s) {
                msg = PeerListAction::Remove;
            }
        }

        if let Some(s) = server {
            if msg == PeerListAction::Add || msg == PeerListAction::Update {
                let newname = host.sb_server_get_string(s, "gamename", std::str::from_utf8(&name).unwrap_or(""));
                if newname != b"ccgenzh" {
                    name = newname;
                }
            }
        }

        if percent_listed == 100 && !self.get_saw_complete_game_list() {
            self.set_saw_complete_game_list(true);
            let mut complete = PeerResponse::default();
            complete.peer_response_type = PeerResponseType::StagingRoomListComplete;
            self.queue.add_response(complete);
        }

        // AsciiString::nextToken(&hostName, " "): the first token (leading spaces skipped); find(' '): first space.
        let mut p = 0;
        let host_name: Vec<u8> = strtok_space(&name, &mut p).map(|t| t.to_vec()).unwrap_or_default();
        let mut game_name = name.clone();
        if let Some(first_space) = name.iter().position(|&c| c == b' ') {
            game_name = name[first_space + 1..].to_vec();
        }

        let mut resp = PeerResponse::default();
        resp.peer_response_type = PeerResponseType::StagingRoom;
        resp.staging_room.action = msg;
        resp.staging_room.is_staging = staging;
        resp.staging_room.percent_complete = percent_listed;

        if let Some(s) = server {
            if msg == PeerListAction::Add || msg == PeerListAction::Update {
                let has_password = host.sb_server_get_int(s, PW_STR, 0) != 0;
                let allow_observers = host.sb_server_get_int(s, OBS_STR, 0) != 0;
                let uses_stats = host.sb_server_get_int(s, USE_STATS_STR, 1) != 0;
                let ver = host.sb_server_get_string(s, "gamever", "000000");
                let exe = host.sb_server_get_string(s, EXECRC_STR, "000000");
                let ini = host.sb_server_get_string(s, INICRC_STR, "000000");
                let lad_ip = host.sb_server_get_string(s, LADIP_STR, "000000");
                let ping = host.sb_server_get_string(s, PINGSTR_STR, "FFFFFFFFFFFFFFFF");
                let lad_port = host.sb_server_get_int(s, LADPORT_STR, 0) as u16;
                resp.staging_room.requires_password = has_password;
                resp.staging_room.allow_observers = allow_observers;
                resp.staging_room.use_stats = uses_stats;
                resp.staging_room.version = strtoul10(&ver);
                resp.staging_room.exe_crc = strtoul10(&exe);
                resp.staging_room.ini_crc = strtoul10(&ini);
                resp.staging_server_ladder_ip = lad_ip;
                resp.staging_server_ping_string = ping;
                resp.staging_room.ladder_port = lad_port;
                resp.staging_room.num_players = host.sb_server_get_int(s, NUMPLAYER_STR, 0);
                resp.staging_room.num_observers = host.sb_server_get_int(s, NUMOBS_STR, 0);
                resp.staging_room.max_players = host.sb_server_get_int(s, MAXPLAYER_STR, 8);
                resp.staging_room_map_name = host.sb_server_get_string(s, "mapname", "");
                for i in 0..SLOTS {
                    resp.staging_room_player_names[i] = host.sb_server_get_player_string(s, i, NAME_STR, "");
                    resp.staging_room.wins[i] = host.sb_server_get_player_int(s, i, WINS_STR, 0);
                    resp.staging_room.losses[i] = host.sb_server_get_player_int(s, i, LOSSES_STR, 0);
                    resp.staging_room.profile_id[i] = host.sb_server_get_player_int(s, i, "pid", 0);
                    resp.staging_room.color[i] = host.sb_server_get_player_int(s, i, COLOR_STR, 0);
                    resp.staging_room.faction[i] = host.sb_server_get_player_int(s, i, FACTION_STR, 0);
                }
                if resp.staging_room_player_names[0].is_empty() {
                    resp.staging_room_player_names[0] = host_name.clone();
                }
            }
        }

        if msg == PeerListAction::Add || msg == PeerListAction::Update {
            if resp.staging_room.exe_crc == 0 || resp.staging_room.ini_crc == 0 {
                // upstream logs "does not have basic keys" when it HAS them, and returns (see the module doc)
                let s = server.unwrap_or(0);
                if host.sb_server_has_basic_keys(s) {
                    return;
                }
                if msg == PeerListAction::Update {
                    let mut req = PeerRequest::default();
                    req.peer_request_type = PeerRequestType::GetExtendedStagingRoomInfo;
                    req.staging_room_id = self.find_server(host, s);
                    self.queue.add_request(req);
                }
                return; // don't actually try to list it.
            }
        }

        match msg {
            PeerListAction::Clear => self.clear_servers(),
            PeerListAction::Add | PeerListAction::Update => {
                resp.staging_room.id = self.find_server(host, server.unwrap_or(0));
                resp.staging_server_name = host.multi_byte_to_wide_single_line(&game_name);
            }
            PeerListAction::Remove => {
                resp.staging_room.id = self.remove_server_from_map(server.unwrap_or(0));
            }
            PeerListAction::Complete => {}
        }

        self.queue.add_response(resp);
    }
}

/// C `strtoul(s, NULL, 10)`: leading whitespace, optional sign, digits; 0 if none; a minus negates (wraps).
pub fn strtoul10(s: &[u8]) -> u32 {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: u32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add((s[i] - b'0') as u32);
        i += 1;
    }
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}
