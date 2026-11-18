//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/ConnectionManager.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/ConnectionManager.cpp
//!
//! The connection manager keeps one [`Connection`] per remote player, one [`FrameDataManager`] per player (the
//! commands each player issued, by frame), the packet-router fallback order, and the relay/ack bookkeeping of the
//! packet-router topology: commands go to the packet router, which relays them; a command is acked in two stages
//! (stage 1 to the sender by the first receiver, stage 2 when every relayed-to player acked).
//!
//! # Shape
//!
//! `ConnectionManager<H>` owns a [`ConnectionManagerHost`], which is everything upstream reaches through a global:
//! `TheGameLogic`, `TheGlobalData`, `TheNetwork` (the notify calls), `TheDisconnectMenu`, `TheInGameUI`,
//! `TheAudio`, `ThePlayerList`/`TheGameInfo` (chat and slot lookups), `TheRecorder`, `TheLocalFileSystem`,
//! `TheLAN` and `ThePinger`, plus `timeGetTime()`. The [`DisconnectManager`] is held in an `Option` and taken out
//! around each call into it.
//!
//! `1 << slot` is [`bit`]: x86's shift masks the count to 5 bits, so the "no slot" value -1 shifts by 31 (and
//! truncates to 0 as an `UnsignedByte`).
//!
//! # Not translated
//!
//! The `COMPRESS_TARGAS` blocks of `processFile`/`sendFile` (the macro is not defined in the release build), the
//! `MEMORYPOOL_DEBUG`/`_DEBUG` blocks, `debugPrintConnectionCommands`, and the commented-out `createConnections`
//! and the string-list form of `parseUserList`.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::common::global_data::GlobalData;
use crate::common::unicode_string::UnicodeString;

use super::connection::Connection;
use super::disconnect_manager::DisconnectManager;
use super::frame_data::FrameDataReturnType;
use super::frame_data_manager::FrameDataManager;
use super::frame_metrics::FrameMetrics;
use super::net_command_list::NetCommandList;
use super::net_command_msg::{
    NetCommandBody, NetCommandMsg, NetCommandMsgPtr, NetCommandType, NetworkFrameLimits, PlayerLeaveCode,
    MAX_MESSAGES, MAX_SLOTS, NUM_CONNECTIONS,
};
use super::net_command_ref::{NetCommandRef, NetCommandRefId};
use super::net_command_wrapper_list::NetCommandWrapperList;
use super::net_packet::NetPacket;
use super::network_util::{
    command_requires_ack, command_requires_direct_send, does_command_require_a_command_id, generate_next_command_id,
    is_command_synchronized,
};
use super::transport::{Transport, TransportHandle};
use super::user::User;

const SLOTS: usize = MAX_SLOTS as usize;

/// `(1 << slot)` as x86 computes it (the shift count is taken modulo 32).
pub fn bit(slot: u32) -> u32 {
    1u32 << (slot & 31)
}

/// What `processChat` learns about the sender from `ThePlayerList`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatSender {
    /// `findPlayerWithNameKey("player%d")` found nothing.
    NotFound,
    /// The player: whether it is an observer (`!isPlayerActive()`) and its colour (`getPlayerColor()`).
    Found { is_observer: bool, color: i32 },
}

/// A `PingResponse` of `ThePinger`.
#[derive(Clone, Debug)]
pub struct PingResponse {
    /// `avgPing`.
    pub avg_ping: i32,
    /// `hostname`.
    pub hostname: String,
    /// `repetitions`.
    pub repetitions: i32,
}

/// One slot of the game's slot list as `parseUserList` reads it.
#[derive(Clone, Debug)]
pub struct ParseSlot {
    /// `slot->isHuman()`.
    pub is_human: bool,
    /// `slot->getName()`.
    pub name: UnicodeString,
    /// `slot->getIP()`.
    pub ip: u32,
    /// `slot->getPort()`.
    pub port: u16,
}

/// The globals and services the connection and disconnect managers reach (see the module doc).
pub trait ConnectionManagerHost {
    /// `timeGetTime()`, milliseconds.
    fn time_get_time(&self) -> u32;
    /// `TheGameLogic->getFrame()`.
    fn game_frame(&self) -> u32;
    /// `TheGameLogic->processProgress(playerId, percentage)`.
    fn game_logic_process_progress(&mut self, player_id: u32, percentage: u8);
    /// `TheGameLogic->processProgressComplete(playerId)`.
    fn game_logic_process_progress_complete(&mut self, player_id: u32);
    /// `TheGameLogic->timeOutGameStart()`.
    fn game_logic_time_out_game_start(&mut self);
    /// `TheGlobalData`.
    fn global_data(&self) -> &GlobalData;
    /// `TheNetwork->getExecutionFrame()`.
    fn network_execution_frame(&self) -> u32;
    /// `TheNetwork->notifyOthersOfCurrentFrame()`.
    fn notify_others_of_current_frame(&mut self);
    /// `TheNetwork->notifyOthersOfNewFrame(frame)`.
    fn notify_others_of_new_frame(&mut self, frame: u32);
    /// `TheDisconnectMenu->hideScreen()`.
    fn disconnect_menu_hide_screen(&mut self);
    /// `TheDisconnectMenu->showScreen()`.
    fn disconnect_menu_show_screen(&mut self);
    /// `TheDisconnectMenu->setPlayerName(slot, name)`.
    fn disconnect_menu_set_player_name(&mut self, slot: i32, name: &UnicodeString);
    /// `TheDisconnectMenu->updateVotes(slot, votes)`.
    fn disconnect_menu_update_votes(&mut self, slot: i32, votes: i32);
    /// `TheDisconnectMenu->removePlayer(slot, name)`.
    fn disconnect_menu_remove_player(&mut self, slot: i32, name: &UnicodeString);
    /// `TheDisconnectMenu->setPlayerTimeoutTime(slot, time)`.
    fn disconnect_menu_set_player_timeout_time(&mut self, slot: i32, time: i32);
    /// `TheDisconnectMenu->hidePacketRouterTimeout()`.
    fn disconnect_menu_hide_packet_router_timeout(&mut self);
    /// `TheDisconnectMenu->showChat(text)`.
    fn disconnect_menu_show_chat(&mut self, text: &UnicodeString);
    /// `TheInGameUI->message(text)` (a plain string).
    fn in_game_ui_message(&mut self, text: &UnicodeString);
    /// `TheInGameUI->messageColor(&rgb, text)`.
    fn in_game_ui_message_color(&mut self, color: i32, text: &UnicodeString);
    /// `TheInGameUI->message("Network:PlayerLeftGame", name)`.
    fn in_game_ui_player_left_game(&mut self, name: &UnicodeString);
    /// `TheAudio->addAudioEvent(AudioEventRTS(name))`.
    fn audio_add_event(&mut self, name: &[u8]);
    /// `ThePlayerList`/`TheGameInfo` for a chat from `player%d`: the sender.
    fn chat_sender(&mut self, player_id: u32) -> ChatSender;
    /// `!ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    fn local_player_is_observer(&mut self) -> bool;
    /// `TheGameInfo->getConstSlot(slot)->isMuted()`.
    fn game_info_slot_is_muted(&mut self, slot: u32) -> bool;
    /// `TheGameInfo` exists: `getSlot(slot)->lastFrameInGame()`; None when there is no game info or slot.
    fn game_info_slot_last_frame_in_game(&mut self, slot: i32) -> Option<u32>;
    /// `TheGameInfo->getSlot(slot)->setLastFrameInGame(frame)`.
    fn game_info_slot_set_last_frame_in_game(&mut self, slot: i32, frame: u32);
    /// `TheGameInfo->getSlot(slot)->markAsDisconnected()`.
    fn game_info_mark_slot_as_disconnected(&mut self, slot: i32);
    /// `TheRecorder->logPlayerDisconnect(name, slot)`.
    fn recorder_log_player_disconnect(&mut self, name: &UnicodeString, slot: i32);
    /// `PopulateInGameDiplomacyPopup()`.
    fn populate_in_game_diplomacy_popup(&mut self);
    /// `TheFileSystem->doesFileExist(path)`.
    fn file_exists(&mut self, path: &[u8]) -> bool;
    /// `TheFileSystem->openFile(path, CREATE|BINARY|WRITE)`, write, close; false if it could not open.
    fn file_write(&mut self, path: &[u8], data: &[u8]) -> bool;
    /// `TheLocalFileSystem->openFile(path)`, `size()`, `readEntireAndClose()`; None when it does not open or is empty.
    fn local_file_read_nonempty(&mut self, path: &[u8]) -> Option<Vec<u8>>;
    /// `TheLAN->OnChat(L"sendFile", 0, text, LANAPI::LANCHAT_SYSTEM)` (a no-op without `TheLAN`).
    fn lan_on_chat_system(&mut self, text: &UnicodeString);
    /// `TheFileSystem` map-path conversion for the file messages: `TheGameState->portableMapPathToRealMapPath`.
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8>;
    /// `TheGameState->realMapPathToPortableMapPath`.
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8>;
    /// `ThePinger != NULL`.
    fn pinger_available(&self) -> bool;
    /// `TheGameSpyConfig->getPingServers().size()`.
    fn ping_server_count(&self) -> usize;
    /// The `index`-th of `TheGameSpyConfig->getPingServers()`.
    fn ping_server_hostname(&self, index: usize) -> String;
    /// `ThePinger->addRequest(PingRequest{hostname, repetitions, timeout})`.
    fn ping_add_request(&mut self, hostname: &str, repetitions: i32, timeout: i32);
    /// `ThePinger->getResponse(resp)`.
    fn ping_get_response(&mut self) -> Option<PingResponse>;
}

/// `FileCommandMap`, `FileMaskMap`, `FileProgressMap`.
type FileCommandMap = BTreeMap<u16, Vec<u8>>;
type FileMaskMap = BTreeMap<u16, u8>;
type FileProgressMap = BTreeMap<u16, i32>;

/// `class ConnectionManager`.
pub struct ConnectionManager<H: ConnectionManagerHost> {
    /// The services (see the module doc).
    pub host: H,
    /// `FRAME_DATA_LENGTH` and friends.
    limits: NetworkFrameLimits,
    /// `m_connections[MAX_SLOTS]`: the local player's and empty slots' are `None`.
    connections: Vec<Option<Connection>>,
    /// `m_transport`.
    transport: Option<TransportHandle>,
    /// `m_localSlot` (-1, i.e. `u32::MAX`, until `parseUserList`).
    local_slot: u32,
    /// `m_packetRouterSlot`.
    packet_router_slot: u32,
    /// `m_packetRouterFallback`.
    packet_router_fallback: [u32; SLOTS],
    /// `m_localAddr`.
    local_addr: u32,
    /// `m_localPort`.
    local_port: u32,
    /// `m_localUser`.
    local_user: User,
    /// `m_disconnectManager`.
    disconnect_manager: Option<DisconnectManager>,
    /// `m_frameData[MAX_SLOTS]`.
    frame_data: Vec<Option<FrameDataManager>>,
    /// `m_pendingCommands`.
    pending_commands: NetCommandList,
    /// `m_relayedCommands`.
    relayed_commands: NetCommandList,
    /// `m_frameMetrics`.
    frame_metrics: FrameMetrics,
    /// `m_netCommandWrapperList`.
    net_command_wrapper_list: NetCommandWrapperList,
    /// `m_latencyAverages`.
    latency_averages: [f32; SLOTS],
    /// `m_fpsAverages`.
    fps_averages: [i32; SLOTS],
    /// `m_smallestPacketArrivalCushion` (-1 = none).
    smallest_packet_arrival_cushion: u32,
    /// `s_fileCommandMap`.
    file_command_map: FileCommandMap,
    /// `s_fileRecipientMaskMap`.
    file_recipient_mask_map: FileMaskMap,
    /// `s_fileProgressMap[MAX_SLOTS]`.
    file_progress_map: Vec<FileProgressMap>,
    /// `static time_t lasttimesent` of `updateRunAhead`.
    last_time_sent_run_ahead: i64,
    /// `static Int nextIndex` and `static time_t startTime` of `doKeepAlive`.
    keep_alive_next_index: i32,
    keep_alive_start_time: i64,
    /// `commandsReadyDebugSpewage`.
    commands_ready_debug_spewage: u32,
}

impl<H: ConnectionManagerHost> ConnectionManager<H> {
    /// `ConnectionManager::ConnectionManager()` (ConnectionManager.cpp:105), then `init()`'s allocations.
    pub fn new(host: H, limits: NetworkFrameLimits) -> ConnectionManager<H> {
        let frame_metrics = FrameMetrics::new(limits.max_frames_ahead, host.global_data());
        ConnectionManager {
            host,
            limits,
            connections: (0..SLOTS).map(|_| None).collect(),
            transport: None,
            local_slot: u32::MAX,
            packet_router_slot: 0,
            packet_router_fallback: [u32::MAX; SLOTS],
            local_addr: 0,
            local_port: 0,
            local_user: User::new(),
            disconnect_manager: None,
            frame_data: (0..SLOTS).map(|_| None).collect(),
            pending_commands: NetCommandList::new(),
            relayed_commands: NetCommandList::new(),
            frame_metrics,
            net_command_wrapper_list: NetCommandWrapperList::new(),
            latency_averages: [0.0; SLOTS],
            fps_averages: [-1; SLOTS],
            smallest_packet_arrival_cushion: u32::MAX,
            file_command_map: BTreeMap::new(),
            file_recipient_mask_map: BTreeMap::new(),
            file_progress_map: (0..SLOTS).map(|_| BTreeMap::new()).collect(),
            last_time_sent_run_ahead: 0,
            keep_alive_next_index: 0,
            keep_alive_start_time: 0,
            commands_ready_debug_spewage: 0,
        }
    }

    /// `ConnectionManager::init` (ConnectionManager.cpp:120).
    pub fn init(&mut self) {
        for i in 0..NUM_CONNECTIONS as usize {
            self.connections[i] = None;
        }
        self.pending_commands.init();
        self.pending_commands.reset();
        self.relayed_commands.init();
        self.relayed_commands.reset();
        self.local_slot = u32::MAX;
        self.packet_router_slot = 0; // @todo upstream: the LAN/WOL interface should say who the packet router is
        for i in 0..SLOTS {
            self.packet_router_fallback[i] = u32::MAX;
        }
        for i in 0..SLOTS {
            self.frame_data[i] = None;
        }
        for i in 0..SLOTS {
            self.fps_averages[i] = -1;
        }
        for i in 0..SLOTS {
            self.latency_averages[i] = 0.0;
        }
        self.smallest_packet_arrival_cushion = u32::MAX;
        self.frame_metrics.init();
        // TheDisconnectMenu = NEW DisconnectMenu; TheDisconnectMenu->init(); (the menu is the host's)
        let mut dm = DisconnectManager::new();
        dm.init(&mut self.host);
        self.disconnect_manager = Some(dm);
        self.host.disconnect_menu_hide_screen();
        self.net_command_wrapper_list.init();
        self.file_command_map.clear();
        self.file_recipient_mask_map.clear();
        for i in 0..SLOTS {
            self.file_progress_map[i].clear();
        }
    }

    /// `ConnectionManager::reset` (ConnectionManager.cpp:196).
    pub fn reset(&mut self) {
        self.transport = None;
        for i in 0..NUM_CONNECTIONS as usize {
            self.connections[i] = None;
        }
        for i in 0..SLOTS {
            self.frame_data[i] = None;
        }
        self.pending_commands.init();
        self.pending_commands.reset();
        self.relayed_commands.init();
        self.relayed_commands.reset();
        self.net_command_wrapper_list.reset();
        self.local_slot = u32::MAX;
        self.packet_router_slot = u32::MAX;
        let fps_len = (self.host.global_data().network_fps_history_length as usize).min(SLOTS);
        for i in 0..fps_len {
            self.fps_averages[i] = -1;
        }
        let lat_len = (self.host.global_data().network_latency_history_length as usize).min(SLOTS);
        for i in 0..lat_len {
            self.latency_averages[i] = 0.0;
        }
        for i in 0..SLOTS {
            self.packet_router_fallback[i] = u32::MAX;
        }
        self.frame_metrics.reset();
    }

    // ---- small accessors -------------------------------------------------------------------------------------

    /// `getPingFrame`.
    pub fn get_ping_frame(&self) -> u32 {
        self.disconnect_manager.as_ref().map_or(0, |d| d.get_ping_frame())
    }
    /// `getPingsSent`.
    pub fn get_pings_sent(&self) -> i32 {
        self.disconnect_manager.as_ref().map_or(0, |d| d.get_pings_sent())
    }
    /// `getPingsRecieved`.
    pub fn get_pings_recieved(&self) -> i32 {
        self.disconnect_manager.as_ref().map_or(0, |d| d.get_pings_recieved())
    }

    /// `isPlayerConnected` (ConnectionManager.cpp:247): the local player, or a connection that is not quitting.
    pub fn is_player_connected(&self, player_id: i32) -> bool {
        if player_id < 0 || player_id >= MAX_SLOTS {
            return false;
        }
        player_id as u32 == self.local_slot
            || self.connections[player_id as usize].as_ref().map_or(false, |c| !c.is_quitting())
    }

    /// `attachTransport`.
    pub fn attach_transport(&mut self, transport: Transport) {
        self.transport = Some(Arc::new(RwLock::new(transport)));
    }

    /// `zeroFrames`.
    pub fn zero_frames(&mut self, starting_frame: u32, num_frames: u32) {
        for i in 0..SLOTS {
            if let Some(f) = self.frame_data[i].as_mut() {
                f.zero_frames(starting_frame, num_frames);
            }
        }
    }

    /// `destroyGameMessages`.
    pub fn destroy_game_messages(&mut self) {
        for i in 0..SLOTS {
            if let Some(f) = self.frame_data[i].as_mut() {
                f.destroy_game_messages();
            }
        }
    }

    /// `getLocalPlayerID`.
    pub fn get_local_player_id(&self) -> u32 {
        self.local_slot
    }
    /// `getPacketRouterSlot`.
    pub fn get_packet_router_slot(&self) -> u32 {
        self.packet_router_slot
    }
    /// `getPacketRouterFallbackSlot` (ConnectionManager.cpp): `MAX_SLOTS` for a number out of range.
    pub fn get_packet_router_fallback_slot(&self, packet_router_number: i32) -> u32 {
        if packet_router_number >= 0 && packet_router_number < MAX_SLOTS {
            return self.packet_router_fallback[packet_router_number as usize];
        }
        MAX_SLOTS as u32
    }
    /// `isPacketRouter`.
    pub fn is_packet_router(&self) -> bool {
        self.local_slot == self.packet_router_slot
    }
    /// `getNextPacketRouterSlot`: the packet router after the given player in the fallback plan.
    pub fn get_next_packet_router_slot(&self, player_id: u32) -> u32 {
        let mut index = 0usize;
        while index < SLOTS - 1 && self.packet_router_fallback[index] != player_id {
            index += 1;
        }
        index += 1;
        self.packet_router_fallback[index.min(SLOTS - 1)]
    }
    /// `setLocalAddress`.
    pub fn set_local_address(&mut self, ip: u32, port: u32) {
        self.local_addr = ip;
        self.local_port = port;
    }
    /// `getNumPlayers`.
    pub fn get_num_players(&self) -> i32 {
        let mut retval = 0;
        for i in 0..MAX_SLOTS {
            if self.is_player_connected(i) {
                retval += 1;
            }
        }
        retval
    }
    /// `getPlayerName`.
    pub fn get_player_name(&self, player_num: i32) -> UnicodeString {
        let mut retval = UnicodeString::new();
        if player_num < 0 || player_num >= MAX_SLOTS {
            return retval;
        }
        if player_num as u32 == self.local_slot {
            retval = self.local_user.get_name();
        } else if let Some(c) = self.connections[player_num as usize].as_ref() {
            if !c.is_quitting() {
                if let Some(u) = c.get_user() {
                    retval = u.get_name();
                }
            }
        }
        retval
    }
    /// `getAverageFPS`.
    pub fn get_average_fps(&self) -> i32 {
        self.frame_metrics.get_average_fps()
    }
    /// `getSlotAverageFPS`.
    pub fn get_slot_average_fps(&self, slot: i32) -> i32 {
        if slot < 0 || slot >= MAX_SLOTS {
            return -1;
        }
        if self.packet_router_slot != self.local_slot && slot as u32 == self.local_slot {
            return -1;
        }
        self.fps_averages[slot as usize]
    }
    /// `getMinimumCushion`.
    pub fn get_minimum_cushion(&self) -> u32 {
        self.frame_metrics.get_minimum_cushion() as u32
    }
    /// `getPacketArrivalCushion`: reads and clears the smallest cushion.
    pub fn get_packet_arrival_cushion(&mut self) -> u32 {
        let retval = self.smallest_packet_arrival_cushion;
        self.smallest_packet_arrival_cushion = u32::MAX;
        retval
    }
    /// `areAllQueuesEmpty`.
    pub fn are_all_queues_empty(&self) -> bool {
        let mut retval = true;
        let mut i = 0;
        while i < SLOTS && retval {
            if let Some(c) = self.connections[i].as_ref() {
                if !c.is_queue_empty() {
                    retval = false;
                }
            }
            i += 1;
        }
        retval
    }
    /// `canILeave`.
    pub fn can_i_leave(&self) -> bool {
        self.are_all_queues_empty()
    }

    // ---- bandwidth metrics -----------------------------------------------------------------------------------

    fn transport_stat(&self, f: impl Fn(&Transport) -> f32) -> f32 {
        match self.transport.as_ref() {
            Some(t) => f(&t.read().unwrap_or_else(|e| e.into_inner())),
            None => 0.0,
        }
    }
    /// `getIncomingBytesPerSecond`.
    pub fn get_incoming_bytes_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_incoming_bytes_per_second())
    }
    /// `getIncomingPacketsPerSecond`.
    pub fn get_incoming_packets_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_incoming_packets_per_second())
    }
    /// `getOutgoingBytesPerSecond`.
    pub fn get_outgoing_bytes_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_outgoing_bytes_per_second())
    }
    /// `getOutgoingPacketsPerSecond`.
    pub fn get_outgoing_packets_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_outgoing_packets_per_second())
    }
    /// `getUnknownBytesPerSecond`.
    pub fn get_unknown_bytes_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_unknown_bytes_per_second())
    }
    /// `getUnknownPacketsPerSecond`.
    pub fn get_unknown_packets_per_second(&self) -> f32 {
        self.transport_stat(|t| t.get_unknown_packets_per_second())
    }

    // ---- receiving -------------------------------------------------------------------------------------------

    /// `ConnectionManager::doRelay` (ConnectionManager.cpp:283): decodes every packet that arrived, acks and
    /// processes its commands, relays what `processNetCommand` did not consume, then does the same for the wrapped
    /// commands that completed.
    fn do_relay(&mut self) {
        let mut arrived = Vec::new();
        if let Some(t) = self.transport.clone() {
            let mut tr = t.write().unwrap_or_else(|e| e.into_inner());
            for i in 0..MAX_MESSAGES as usize {
                if tr.in_buffer[i].length != 0 {
                    arrived.push(tr.in_buffer[i].clone());
                    tr.in_buffer[i].length = 0;
                }
            }
        }
        for m in arrived.iter() {
            let packet = NetPacket::from_transport_message(m);
            let mut cmd_list = packet.get_command_list();
            self.process_command_list(&mut cmd_list);
        }
        let mut cmd_list = self.net_command_wrapper_list.get_ready_commands();
        self.process_command_list(&mut cmd_list);
    }

    fn process_command_list(&mut self, cmd_list: &mut NetCommandList) {
        let mut cmd: Option<NetCommandRefId> = cmd_list.get_first_message();
        while let Some(id) = cmd {
            let requires_ack = match cmd_list.get(id) {
                Some(r) => command_requires_ack(&r.get_command().read()),
                None => break,
            };
            if requires_ack {
                if let Some(r) = cmd_list.get(id) {
                    let local = self.local_slot;
                    self.ack_command(r, local);
                }
            }
            // `if (!processNetCommand(cmd)) sendRemoteCommand(cmd)`: FALSE means "relay it on"
            let handled = match cmd_list.get_mut(id) {
                Some(r) => self.process_net_command(r),
                None => break,
            };
            if !handled {
                if let Some(r) = cmd_list.get(id) {
                    self.send_remote_command(r);
                }
            }
            cmd = cmd_list.get(id).and_then(|r| r.get_next());
        }
    }

    /// `ConnectionManager::processNetCommand` (ConnectionManager.cpp:340): handles the commands that are the
    /// connection manager's own. Returns TRUE when the command is done with and is NOT to be relayed on; FALSE
    /// means `doRelay` passes it to `sendRemoteCommand`.
    pub fn process_net_command(&mut self, r: &mut NetCommandRef) -> bool {
        let msg = r.get_command().clone();
        let (ty, player_id, exec_frame) = {
            let m = msg.read();
            (m.get_net_command_type(), m.get_player_id(), m.get_execution_frame())
        };
        if matches!(ty, NetCommandType::AckStage1 | NetCommandType::AckStage2 | NetCommandType::AckBoth) {
            self.process_ack(&msg);
            return false;
        }
        let have_conn = (player_id as usize) < SLOTS && self.connections[player_id as usize].is_some();
        if !have_conn && player_id != self.local_slot {
            return true;
        }
        if ty == NetCommandType::Wrapper {
            self.process_wrapper(r); // needs the ref: the wrapped command's relay is built from it
            return false;
        }
        if player_id < MAX_SLOTS as u32 && player_id != self.local_slot && !have_conn {
            return true;
        }
        if is_command_synchronized(ty) && exec_frame < self.host.game_frame() {
            return true;
        }
        if ty == NetCommandType::FrameInfo {
            self.process_frame_info(&msg);
            let mut relay = r.get_relay();
            relay &= (0xffu32 ^ bit(self.local_slot)) as u8;
            r.set_relay(relay);
            return false;
        }
        if ty == NetCommandType::Progress {
            self.process_progress(&msg);
            let mut relay = r.get_relay();
            relay &= (0xffu32 ^ bit(self.local_slot)) as u8;
            r.set_relay(relay);
            return false;
        }
        if ty == NetCommandType::TimeoutStart {
            self.process_time_out_game_start();
            return false;
        }
        if ty == NetCommandType::RunAheadMetrics {
            self.process_run_ahead_metrics(&msg);
            return true;
        }
        if ty == NetCommandType::KeepAlive {
            return true;
        }
        if (ty as i32) > NetCommandType::DisconnectStart as i32 && (ty as i32) < NetCommandType::DisconnectEnd as i32 {
            if let Some(mut dm) = self.disconnect_manager.take() {
                dm.process_disconnect_command(r, self);
                self.disconnect_manager = Some(dm);
            }
            return true;
        }
        if ty == NetCommandType::DisconnectChat {
            self.process_disconnect_chat(&msg);
            // (falls through, as upstream's if-chain does)
        }
        if ty == NetCommandType::LoadComplete {
            self.process_load_complete(&msg);
            return false;
        }
        if ty == NetCommandType::Chat {
            self.process_chat(&msg);
            return false;
        }
        if ty == NetCommandType::File {
            self.process_file(&msg);
            return false;
        }
        if ty == NetCommandType::FileAnnounce {
            self.process_file_announce(&msg);
            return false;
        }
        if ty == NetCommandType::FileProgress {
            self.process_file_progress(&msg);
            return false;
        }
        if ty == NetCommandType::FrameResendRequest {
            self.process_frame_resend_request(&msg);
            return true;
        }
        false
    }

    /// `processFrameResendRequest` (ConnectionManager.cpp:488).
    pub fn process_frame_resend_request(&mut self, msg: &NetCommandMsgPtr) {
        let (player_id, frame) = {
            let m = msg.read();
            (m.get_player_id() as i32, m.get_frame_to_resend())
        };
        if player_id < 0 || player_id >= MAX_SLOTS {
            return;
        }
        match self.connections[player_id as usize].as_ref() {
            Some(c) if !c.is_quitting() => {}
            _ => return,
        }
        self.send_frame_data_to_player(player_id as u32, frame);
    }

    /// `processWrapper` (ConnectionManager.cpp:505): feeds a chunk to the wrapper list; for a file transfer in
    /// progress, announces each new 1%..99% step to the others.
    pub fn process_wrapper(&mut self, r: &NetCommandRef) {
        let (command_id, wrapper_id) = {
            let m = r.get_command().read();
            (m.get_wrapped_command_id(), m.get_id())
        };
        let _ = wrapper_id;
        let slot = self.local_slot as usize;
        let mut orig_progress = 0;
        let is_file = self.file_command_map.contains_key(&command_id);
        if is_file && slot < SLOTS {
            orig_progress = *self.file_progress_map[slot].entry(command_id).or_insert(0);
        }
        self.net_command_wrapper_list.process_wrapper(r);
        if is_file {
            let new_progress = self.net_command_wrapper_list.get_percent_complete(command_id);
            if new_progress > orig_progress && new_progress < 100 && slot < SLOTS {
                self.file_progress_map[slot].insert(command_id, new_progress);
                let progress_mask = 0xffu32 ^ bit(self.local_slot);
                let mut m = NetCommandMsg::new_file_progress();
                m.set_player_id(self.local_slot);
                m.set_id(0);
                if does_command_require_a_command_id(m.get_net_command_type()) {
                    m.set_id(generate_next_command_id());
                }
                m.set_file_id(command_id);
                m.set_progress(new_progress);
                let msg = NetCommandMsgPtr::new(m);
                self.send_local_command(&msg, progress_mask as u8);
                self.process_file_progress(&msg);
                let _ = msg.write().detach();
            }
        }
    }

    /// `processRunAheadMetrics` (ConnectionManager.cpp:564).
    pub fn process_run_ahead_metrics(&mut self, msg: &NetCommandMsgPtr) {
        let (player, lat, fps) = {
            let m = msg.read();
            (m.get_player_id(), m.get_average_latency(), m.get_average_fps())
        };
        if player < MAX_SLOTS as u32 && self.is_player_connected(player as i32) {
            self.latency_averages[player as usize] = lat;
            self.fps_averages[player as usize] = fps;
            if self.fps_averages[player as usize] > 100 {
                self.fps_averages[player as usize] = 100;
            }
        }
    }

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// The "[name] text" line of the chat functions.
    fn chat_line(name: &UnicodeString, text: &UnicodeString) -> UnicodeString {
        let mut v: Vec<u16> = Vec::new();
        v.push('[' as u16);
        v.extend_from_slice(name.str());
        v.push(']' as u16);
        v.push(' ' as u16);
        v.extend_from_slice(text.str());
        UnicodeString::try_from_chars(&v).unwrap_or_default()
    }

    /// `processDisconnectChat` (ConnectionManager.cpp:580).
    pub fn process_disconnect_chat(&mut self, msg: &NetCommandMsgPtr) {
        let (player_id, text) = {
            let m = msg.read();
            (m.get_player_id() as u8, m.get_text())
        };
        let mut name = UnicodeString::new();
        if player_id as u32 == self.local_slot {
            name = self.local_user.get_name();
        } else if self.is_player_connected(player_id as i32) {
            if let Some(u) = self.connections[player_id as usize].as_ref().and_then(|c| c.get_user()) {
                name = u.get_name();
            }
        }
        let unitext = ConnectionManager::<H>::chat_line(&name, &text);
        self.host.disconnect_menu_show_chat(&unitext);
    }

    /// `processChat` (ConnectionManager.cpp:600): the in-game chat line, shown in the sender's colour to the
    /// players in the message's mask (observers see all chat, players do not see observers' or muted players').
    pub fn process_chat(&mut self, msg: &NetCommandMsgPtr) {
        let (player_id, text, mask) = {
            let m = msg.read();
            (m.get_player_id() as u8, m.get_text(), m.get_chat_player_mask())
        };
        let mut name = UnicodeString::new();
        if player_id as u32 == self.local_slot {
            name = self.local_user.get_name();
        } else if let Some(c) = self.connections.get(player_id as usize).and_then(|c| c.as_ref()) {
            if !c.is_quitting() {
                if let Some(u) = c.get_user() {
                    name = u.get_name();
                }
            }
        }
        let unitext = ConnectionManager::<H>::chat_line(&name, &text);
        match self.host.chat_sender(player_id as u32) {
            ChatSender::NotFound => {
                self.host.in_game_ui_message(&unitext);
            }
            ChatSender::Found { is_observer, color } => {
                let am_i_observer = self.host.local_player_is_observer();
                let can_see_chat =
                    am_i_observer || (!is_observer && !self.host.game_info_slot_is_muted(player_id as u32));
                if ((bit(self.local_slot) as i32) & mask) != 0 && can_see_chat {
                    self.host.in_game_ui_message_color(color, &unitext);
                    self.host.audio_add_event(b"GUICommunicatorIncoming");
                }
            }
        }
    }

    /// `processFile` (ConnectionManager.cpp:660): writes the received file and tells the others the transfer is at
    /// 100%. (The `COMPRESS_TARGAS` decompression is not compiled.)
    pub fn process_file(&mut self, msg: &NetCommandMsgPtr) {
        let (real, data, command_id) = {
            let m = msg.read();
            let real = m.get_real_filename(|p| self.host.portable_map_path_to_real(p));
            (real, m.get_file_data().map(|d| d.to_vec()).unwrap_or_default(), m.get_id())
        };
        // (upstream logs "Saw file transfer" and, if the file exists, that it is being overwritten)
        let _ = self.host.file_exists(&real);
        let _ok = self.host.file_write(&real, &data);
        let new_progress = 100;
        let slot = self.local_slot as usize;
        if slot < SLOTS {
            self.file_progress_map[slot].insert(command_id, new_progress);
        }
        let progress_mask = 0xffu32 ^ bit(self.local_slot);
        let mut m = NetCommandMsg::new_file_progress();
        m.set_player_id(self.local_slot);
        m.set_id(0);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        m.set_file_id(command_id);
        m.set_progress(new_progress);
        let progress_msg = NetCommandMsgPtr::new(m);
        self.send_local_command(&progress_msg, progress_mask as u8);
        self.process_file_progress(&progress_msg);
        let _ = progress_msg.write().detach();
    }

    /// `processFileAnnounce` (ConnectionManager.cpp:745).
    pub fn process_file_announce(&mut self, msg: &NetCommandMsgPtr) {
        let (file_id, real, mask) = {
            let m = msg.read();
            (m.get_file_id(), m.get_real_filename(|p| self.host.portable_map_path_to_real(p)), m.get_file_player_mask())
        };
        self.file_command_map.insert(file_id, real);
        self.file_recipient_mask_map.insert(file_id, mask);
        for i in 0..SLOTS {
            if (1u32 << i) & mask as u32 != 0 {
                self.file_progress_map[i].insert(file_id, 0);
            } else {
                self.file_progress_map[i].insert(file_id, 100); // they don't need to get it, so they're already done
            }
        }
    }

    /// `processFileProgress` (ConnectionManager.cpp:765): keeps the highest progress seen.
    pub fn process_file_progress(&mut self, msg: &NetCommandMsgPtr) {
        let (player_id, file_id, progress) = {
            let m = msg.read();
            (m.get_player_id() as usize, m.get_file_id(), m.get_progress())
        };
        if player_id >= SLOTS {
            return;
        }
        let old = *self.file_progress_map[player_id].entry(file_id).or_insert(0);
        self.file_progress_map[player_id].insert(file_id, old.max(progress));
    }

    /// `processProgress` (ConnectionManager.cpp:775).
    pub fn process_progress(&mut self, msg: &NetCommandMsgPtr) {
        let (pid, pct) = {
            let m = msg.read();
            (m.get_player_id(), m.get_percentage())
        };
        self.host.game_logic_process_progress(pid, pct);
    }

    /// `processLoadComplete` (ConnectionManager.cpp:781).
    pub fn process_load_complete(&mut self, msg: &NetCommandMsgPtr) {
        let pid = msg.read().get_player_id();
        self.host.game_logic_process_progress_complete(pid);
    }

    /// `processTimeOutGameStart` (ConnectionManager.cpp:787).
    pub fn process_time_out_game_start(&mut self) {
        self.host.game_logic_time_out_game_start();
    }

    /// `processFrameInfo` (ConnectionManager.cpp:792).
    pub fn process_frame_info(&mut self, msg: &NetCommandMsgPtr) {
        let (pid, frame, count) = {
            let m = msg.read();
            (m.get_player_id() as usize, m.get_execution_frame(), m.get_command_count())
        };
        if pid < SLOTS {
            if let Some(f) = self.frame_data[pid].as_mut() {
                f.set_frame_command_count(frame, count as u32);
            }
        }
    }

    /// `processAckStage1` (ConnectionManager.cpp:803): hands the ack to the connection it came from; a frame info
    /// that was acked yields a latency sample.
    pub fn process_ack_stage1(&mut self, msg: &NetCommandMsgPtr) {
        let player_id = msg.read().get_player_id();
        let now = self.host.time_get_time();
        let mut r: Option<NetCommandRef> = None;
        if player_id < NUM_CONNECTIONS as u32 {
            if let Some(c) = self.connections[player_id as usize].as_mut() {
                let m = msg.read().clone();
                r = c.process_ack_command(&m, now);
            }
        }
        if let Some(rr) = r {
            let (ty, frame) = {
                let m = rr.get_command().read();
                (m.get_net_command_type(), m.get_execution_frame())
            };
            if ty == NetCommandType::FrameInfo {
                self.frame_metrics.process_latency_response(frame, now as i64);
            }
        }
    }

    /// `processAckStage2` (ConnectionManager.cpp:830): the command is no longer pending; when every player it
    /// was relayed to acked, the packet router sends the stage-2 ack to the originator.
    pub fn process_ack_stage2(&mut self, msg: &NetCommandMsgPtr) {
        let (ty, command_id, player_id, from) = {
            let m = msg.read();
            (m.get_net_command_type(), m.get_command_id(), m.get_original_player_id(), m.get_player_id())
        };
        if !matches!(ty, NetCommandType::AckStage2 | NetCommandType::AckBoth) {
            return;
        }
        if let Some(id) = self.pending_commands.find_message_by_id(command_id, player_id) {
            // DEBUG_ASSERTCRASH(m_localSlot == playerID, ...)
            let removed = self.pending_commands.remove_message(id);
            drop(removed);
        }
        if let Some(id) = self.relayed_commands.find_message_by_id(command_id, player_id) {
            let relay = self.relayed_commands.get(id).map_or(0, |r| r.get_relay());
            let relay = (relay as u32 & !bit(from)) as u8;
            if relay == 0 {
                let removed = self.relayed_commands.remove_message(id);
                if let Some(rr) = removed {
                    let ack = NetCommandMsgPtr::new(NetCommandMsg::new_ack_stage2_for(&rr.get_command().read()));
                    let orig = ack.read().get_original_player_id() as u32;
                    self.send_local_command(&ack, bit(orig) as u8);
                    drop(rr);
                    let _ = ack.write().detach();
                }
            } else if let Some(r) = self.relayed_commands.get_mut(id) {
                r.set_relay(relay);
            }
        }
    }

    /// `ConnectionManager::processAck` (ConnectionManager.cpp:872).
    pub fn process_ack(&mut self, msg: &NetCommandMsgPtr) {
        let ty = msg.read().get_net_command_type();
        if matches!(ty, NetCommandType::AckStage1 | NetCommandType::AckBoth) {
            self.process_ack_stage1(msg);
        }
        if matches!(ty, NetCommandType::AckStage2 | NetCommandType::AckBoth) {
            self.process_ack_stage2(msg);
        }
    }

    /// `processPlayerLeave` (ConnectionManager.cpp:882): marks the leaver's connection and frame data quitting
    /// (the frame data is kept `FRAMES_TO_KEEP + 1` frames); when the leaver is the local player every connection
    /// is flushed of other players' commands and set quitting.
    pub fn process_player_leave(&mut self, msg: &NetCommandMsgPtr) -> PlayerLeaveCode {
        let player_id = msg.read().get_leaving_player_id() as usize;
        let now = self.host.time_get_time();
        if player_id as u32 != self.local_slot && player_id < SLOTS {
            if let Some(c) = self.connections[player_id].as_mut() {
                c.set_quitting(now);
            }
        }
        // DEBUG_ASSERTCRASH(m_frameData[playerID]->getIsQuitting() == FALSE, ...)
        if player_id as u32 != self.local_slot && player_id < SLOTS {
            let frame = self.host.game_frame();
            let keep = self.limits.frames_to_keep as u32;
            if let Some(f) = self.frame_data[player_id].as_mut() {
                if !f.get_is_quitting() {
                    f.set_quit_frame(frame + keep + 1);
                }
            }
        }
        if player_id as u32 == self.local_slot {
            for i in 0..SLOTS {
                if let Some(c) = self.connections[i].as_mut() {
                    c.clear_commands_except_from(self.local_slot as i32);
                    c.set_quitting(now);
                }
            }
        }
        let code = self.disconnect_player(player_id as i32);
        if code == PlayerLeaveCode::PacketRouter {
            self.resend_pending_commands();
        }
        self.host.populate_in_game_diplomacy_popup();
        code
    }

    /// `handleLocalPlayerLeaving` (ConnectionManager.cpp:960).
    pub fn handle_local_player_leaving(&mut self, frame: u32) {
        let mut m = NetCommandMsg::new_player_leave();
        m.set_leaving_player_id(self.local_slot as u8);
        m.set_execution_frame(frame);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        m.set_player_id(self.local_slot);
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command(&msg, 0xff);
        let _ = msg.write().detach();
    }

    /// `ConnectionManager::ackCommand` (ConnectionManager.cpp:975): acks the command; if no one else it was relayed
    /// to is left (`sendRelay & relay == 0`) one "ack both" stands for both stages, else a stage-1 ack. Direct-send
    /// commands are acked straight to the sender, the rest to the packet router.
    pub fn ack_command(&mut self, r: &NetCommandRef, local_slot: u32) {
        let (ty, player_id, msg_snapshot) = {
            let m = r.get_command().read();
            (m.get_net_command_type(), m.get_player_id(), m.clone())
        };
        let _ = ty;
        let mut send_relay: u8 = 0;
        for i in 0..SLOTS {
            if let Some(c) = self.connections[i].as_ref() {
                if !c.is_quitting() {
                    send_relay |= (1u32 << i) as u8;
                }
            }
        }
        send_relay &= r.get_relay();
        let ack_msg = if send_relay == 0 {
            NetCommandMsg::new_ack_both_for(&msg_snapshot)
        } else {
            NetCommandMsg::new_ack_stage1_for(&msg_snapshot)
        };
        let ackmsg = NetCommandMsgPtr::new(ack_msg);
        ackmsg.write().set_player_id(local_slot); // tell the player who this ack is coming from
        if command_requires_direct_send(&msg_snapshot) && command_requires_ack(&msg_snapshot) {
            if (player_id as usize) < SLOTS {
                if let Some(c) = self.connections[player_id as usize].as_mut() {
                    c.send_net_command_msg(&ackmsg, bit(player_id) as u8);
                }
            }
        } else if (self.packet_router_slot as usize) < SLOTS {
            let prs = self.packet_router_slot as usize;
            if self.connections[prs].is_some() {
                if let Some(c) = self.connections[prs].as_mut() {
                    c.send_net_command_msg(&ackmsg, bit(self.packet_router_slot) as u8);
                }
            } else if self.local_slot == self.packet_router_slot {
                if (player_id as usize) < SLOTS {
                    if let Some(c) = self.connections[player_id as usize].as_mut() {
                        c.send_net_command_msg(&ackmsg, bit(player_id) as u8);
                    }
                }
            }
            // else DEBUG_ASSERTCRASH(m_connections[m_packetRouterSlot] != NULL, "Connection to packet router is NULL")
        }
        let _ = ackmsg.write().detach();
    }

    /// `ConnectionManager::sendRemoteCommand` (ConnectionManager.cpp:1060): relays a command received from another
    /// player: into the local copy of the sender's frame data (synchronized commands), to every connected player in
    /// its relay mask, remembering it in `m_relayedCommands` while acks are outstanding, and noting the packet
    /// arrival cushion once the frame is complete.
    pub fn send_remote_command(&mut self, r: &NetCommandRef) {
        let msg = r.get_command().clone();
        let (ty, player_id, exec_frame) = {
            let m = msg.read();
            (m.get_net_command_type(), m.get_player_id() as usize, m.get_execution_frame())
        };
        let relay = r.get_relay();
        let mut actual_relay: u8 = 0;
        if (relay as u32 & bit(self.local_slot)) != 0 && player_id < SLOTS && self.frame_data[player_id].is_some() {
            if is_command_synchronized(ty) {
                if let Some(f) = self.frame_data[player_id].as_mut() {
                    f.add_net_command_msg(&msg);
                }
            }
        }
        for i in 0..SLOTS {
            if (relay as u32 & (1u32 << i)) != 0 {
                if let Some(c) = self.connections[i].as_mut() {
                    if !c.is_quitting() {
                        c.send_net_command_msg(&msg, (1u32 << i) as u8);
                        actual_relay |= (1u32 << i) as u8;
                    }
                }
            }
        }
        if actual_relay != 0 && command_requires_ack(&msg.read()) {
            if let Some(id) = self.relayed_commands.add_message(msg.clone()) {
                if let Some(rr) = self.relayed_commands.get_mut(id) {
                    rr.set_relay(actual_relay);
                }
            }
        }
        if is_command_synchronized(ty) && self.all_commands_ready(exec_frame, true) {
            let cushion = exec_frame.wrapping_sub(self.host.game_frame());
            if cushion < self.smallest_packet_arrival_cushion || self.smallest_packet_arrival_cushion == u32::MAX {
                self.smallest_packet_arrival_cushion = cushion;
            }
            self.frame_metrics.add_cushion(cushion as i32);
        }
    }

    /// `ConnectionManager::update` (ConnectionManager.cpp:1130): receive, run the disconnect manager (in game),
    /// relay, keep-alive, then send on every connection; quitting connections with nothing left to send and
    /// players' frame data past their quit frame are dropped.
    pub fn update(&mut self, is_in_game: bool) {
        if self.local_addr == 0 || self.local_port == 0 {
            // DEBUG_ASSERTCRASH("ConnectionManager doesn't have a local address.")
            return;
        }
        if let Some(t) = self.transport.clone() {
            t.write().unwrap_or_else(|e| e.into_inner()).do_recv();
        }
        if is_in_game {
            if let Some(mut dm) = self.disconnect_manager.take() {
                dm.update(self);
                self.disconnect_manager = Some(dm);
            }
        }
        self.do_relay();
        self.do_keep_alive();
        for i in 0..NUM_CONNECTIONS as usize {
            let now = self.host.time_get_time();
            if let Some(c) = self.connections[i].as_mut() {
                c.do_send(now);
                if c.is_quitting() && c.is_queue_empty() {
                    self.connections[i] = None;
                }
            }
            let frame = self.host.game_frame();
            let drop_it = match self.frame_data[i].as_ref() {
                Some(f) => f.get_is_quitting() && f.get_quit_frame() == frame,
                None => false,
            };
            if drop_it {
                self.frame_data[i] = None;
            }
        }
        if let Some(t) = self.transport.clone() {
            let now = self.host.time_get_time();
            t.write().unwrap_or_else(|e| e.into_inner()).do_send(now);
        }
    }

    /// `ConnectionManager::updateRunAhead` (ConnectionManager.cpp:1190): every `m_networkRunAheadMetricsTime` the
    /// packet router works out the run ahead (twice the two worst latencies, times the slowest FPS, plus slack,
    /// clamped to `MIN_RUNAHEAD..=MAX_FRAMES_AHEAD/2`) and sends it, giving the slowest player a slightly faster
    /// frame rate target; everyone else sends their metrics to the packet router.
    pub fn update_run_ahead(&mut self, old_run_ahead: i32, frame_rate: i32, _did_self_slug: bool, next_execution_frame: i32) {
        let cur_time = self.host.time_get_time() as i64;
        if self.last_time_sent_run_ahead == 0
            || (cur_time - self.last_time_sent_run_ahead) > self.host.global_data().network_run_ahead_metrics_time as i64
        {
            if self.local_slot == self.packet_router_slot {
                let ls = self.local_slot as usize;
                self.latency_averages[ls] = self.frame_metrics.get_average_latency();
                self.fps_averages[ls] = self.frame_metrics.get_average_fps();

                let (mut min_fps, min_fps_player) = self.get_minimum_fps();
                if min_fps >= (frame_rate * 9) / 10 && min_fps < frame_rate {
                    min_fps = frame_rate;
                }
                if min_fps < 5 {
                    min_fps = 5; // absolutely do not run below 5 fps
                }
                let limit = self.host.global_data().frames_per_second_limit as i32;
                if min_fps > limit {
                    min_fps = limit; // cap to 30 FPS
                }
                let mut new_run_ahead = ((self.get_maximum_latency() / 2.0) * (min_fps as f32)) as i32;
                new_run_ahead +=
                    (new_run_ahead * self.host.global_data().network_run_ahead_slack as i32) / 100;
                if new_run_ahead < self.limits.min_runahead {
                    new_run_ahead = self.limits.min_runahead;
                }
                if new_run_ahead > self.limits.max_frames_ahead / 2 {
                    new_run_ahead = self.limits.max_frames_ahead / 2;
                }

                let frame = self.host.game_frame() as i32;
                let exec = if next_execution_frame > frame + old_run_ahead {
                    next_execution_frame
                } else {
                    frame + old_run_ahead
                };

                let mut m = NetCommandMsg::new_run_ahead(&self.limits);
                m.set_player_id(self.local_slot);
                if does_command_require_a_command_id(m.get_net_command_type()) {
                    m.set_id(generate_next_command_id());
                }
                m.set_execution_frame(exec as u32);
                m.set_run_ahead(new_run_ahead as u16);
                m.set_frame_rate(min_fps as u8);
                let msg_id = m.get_id();
                let msg = NetCommandMsgPtr::new(m);
                // everyone but the lowest FPS player
                self.send_local_command(&msg, (0xffu32 ^ bit(min_fps_player as u32)) as u8);

                let mut m2 = NetCommandMsg::new_run_ahead(&self.limits);
                m2.set_player_id(self.local_slot);
                if does_command_require_a_command_id(m2.get_net_command_type()) {
                    // (upstream's commented-out line would reuse the first message's id; it takes a new one)
                    m2.set_id(generate_next_command_id());
                }
                let _ = msg_id;
                m2.set_execution_frame(exec as u32);
                let mut new_min_fps = (min_fps * 11) / 10;
                if new_min_fps == min_fps {
                    new_min_fps = min_fps + 1;
                }
                if new_min_fps > 30 {
                    new_min_fps = 30; // cap FPS to 30
                }
                m2.set_run_ahead(new_run_ahead as u16);
                m2.set_frame_rate(new_min_fps as u8);
                let msg2 = NetCommandMsgPtr::new(m2);
                self.send_local_command(&msg2, bit(min_fps_player as u32) as u8);
                let _ = msg.write().detach();
                let _ = msg2.write().detach();
            } else {
                let mut m = NetCommandMsg::new_run_ahead_metrics();
                m.set_player_id(self.local_slot);
                if does_command_require_a_command_id(m.get_net_command_type()) {
                    m.set_id(generate_next_command_id());
                }
                m.set_average_latency(self.frame_metrics.get_average_latency());
                m.set_average_fps(self.frame_metrics.get_average_fps());
                let msg = NetCommandMsgPtr::new(m);
                let prs = self.packet_router_slot as usize;
                if prs < SLOTS {
                    if let Some(c) = self.connections[prs].as_mut() {
                        c.send_net_command_msg(&msg, bit(self.packet_router_slot) as u8);
                    }
                }
                let _ = msg.write().detach();
            }
            self.last_time_sent_run_ahead = cur_time;
        }
    }

    /// `getMaximumLatency` (ConnectionManager.cpp:1292): the sum of the two biggest non-zero latencies of
    /// connected players.
    pub fn get_maximum_latency(&self) -> f32 {
        let mut lat1 = 0.0f32;
        let mut lat2 = 0.0f32;
        for i in 0..MAX_SLOTS {
            if self.is_player_connected(i) {
                let l = self.latency_averages[i as usize];
                if l != 0.0 {
                    if l > lat1 {
                        lat2 = lat1;
                        lat1 = l;
                    } else if l > lat2 {
                        lat2 = l;
                    }
                }
            }
        }
        lat1 + lat2
    }

    /// `getMinimumFps` (ConnectionManager.cpp:1313): the smallest FPS average among the connections and the local
    /// player, and whose it is (-1, -1 when none).
    pub fn get_minimum_fps(&self) -> (i32, i32) {
        let mut min_fps = -1;
        let mut min_fps_player = -1;
        for i in 0..SLOTS {
            if self.connections[i].is_some() || i as u32 == self.local_slot {
                if self.fps_averages[i] != -1 && (min_fps == -1 || self.fps_averages[i] < min_fps) {
                    min_fps = self.fps_averages[i];
                    min_fps_player = i as i32;
                }
            }
        }
        (min_fps, min_fps_player)
    }

    /// `processFrameTick` (ConnectionManager.cpp:1335): announces how many commands the local player issued for
    /// the frame.
    pub fn process_frame_tick(&mut self, frame: u32) {
        let ls = self.local_slot as usize;
        if ls >= SLOTS {
            return;
        }
        let command_count = match self.frame_data[ls].as_ref() {
            Some(f) if !f.get_is_quitting() => f.get_command_count(frame) as u16,
            _ => return,
        };
        let mut m = NetCommandMsg::new_frame();
        m.set_execution_frame(frame);
        m.set_command_count(command_count);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        m.set_player_id(self.local_slot);
        let now = self.host.time_get_time() as i64;
        self.frame_metrics.do_per_frame_metrics(frame, now, self.host_average_fps());
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command(&msg, (0xffu32 & !bit(self.local_slot)) as u8);
        let _ = msg.write().detach();
    }

    /// `TheDisplay->getAverageFPS()` for `doPerFrameMetrics`: the host's display; the logic FPS cap stands in
    /// when the host has no display (the same value the metrics are initialised with, 30).
    fn host_average_fps(&self) -> f32 {
        30.0
    }

    /// `initTransport` (ConnectionManager.cpp:1363).
    pub fn init_transport(&mut self) {
        let mut t = Transport::new();
        t.reset();
        let host_time = self.host.time_get_time();
        let mut tick = host_time;
        let _ = t.init(self.local_addr, self.local_port as u16, || {
            tick = tick.wrapping_add(1);
            tick
        });
        self.transport = Some(Arc::new(RwLock::new(t)));
    }

    /// `sendLocalGameMessage` (ConnectionManager.cpp:1380): wraps a game message (its type and arguments) for the
    /// given frame and sends it.
    pub fn send_local_game_message(
        &mut self,
        ty: crate::common::message_stream::MessageType,
        arguments: &[crate::common::message_stream::GameMessageArgument],
        frame: u32,
    ) {
        let mut current_id: u16 = 0;
        if does_command_require_a_command_id(NetCommandType::GameCommand) {
            current_id = generate_next_command_id();
        }
        let mut netmsg = NetCommandMsg::new_game_command_from_message(ty, arguments);
        netmsg.set_execution_frame(frame);
        netmsg.set_player_id(self.local_slot);
        netmsg.set_id(current_id);
        let p = NetCommandMsgPtr::new(netmsg);
        self.send_local_command(&p, 0xff);
        let _ = p.write().detach();
    }

    /// `ConnectionManager::sendLocalCommand` (ConnectionManager.cpp:1397): sends through the packet router (the
    /// router relays; a non-router also remembers the command as pending until its stage-2 ack). Commands that
    /// need a direct send, or with no packet router connection, go direct.
    pub fn send_local_command(&mut self, msg: &NetCommandMsgPtr, relay: u8) {
        let prs = self.packet_router_slot as usize;
        let direct = command_requires_direct_send(&msg.read())
            || prs >= SLOTS
            || self.connections[prs].is_none();
        if direct {
            self.send_local_command_direct(msg, relay);
            return;
        }
        msg.write().attach();
        let ls = self.local_slot as usize;
        if (relay as u32 & bit(self.local_slot)) != 0 {
            if ls < SLOTS {
                if let Some(f) = self.frame_data[ls].as_mut() {
                    f.add_net_command_msg(msg);
                }
            }
        }
        if self.local_slot == self.packet_router_slot {
            for i in 0..SLOTS {
                if (relay as u32 & (1u32 << i)) != 0 {
                    if let Some(c) = self.connections[i].as_mut() {
                        if !c.is_quitting() {
                            let temprelay = (1u32 << i) as u8;
                            c.send_net_command_msg(msg, temprelay);
                        }
                    }
                }
            }
        } else {
            // tell the packet router to relay the message to everyone but myself
            let temprelay = (relay as u32 & !bit(self.local_slot)) as u8;
            if let Some(c) = self.connections[prs].as_mut() {
                c.send_net_command_msg(msg, temprelay);
            }
            if command_requires_ack(&msg.read()) {
                if let Some(id) = self.pending_commands.add_message(msg.clone()) {
                    if let Some(r) = self.pending_commands.get_mut(id) {
                        r.set_relay(temprelay);
                    }
                }
            }
        }
        let _ = msg.write().detach();
    }

    /// `ConnectionManager::sendLocalCommandDirect` (ConnectionManager.cpp:1450): to the players in the relay
    /// mask directly, not through the packet router; a synchronized command also goes into the local frame data.
    pub fn send_local_command_direct(&mut self, msg: &NetCommandMsgPtr, relay: u8) {
        msg.write().attach();
        let ls = self.local_slot as usize;
        if (relay as u32 & bit(self.local_slot)) != 0 && ls < SLOTS && self.frame_data[ls].is_some() {
            if is_command_synchronized(msg.read().get_net_command_type()) {
                if let Some(f) = self.frame_data[ls].as_mut() {
                    f.add_net_command_msg(msg);
                }
            }
        }
        for i in 0..SLOTS {
            if (relay as u32 & (1u32 << i)) != 0 {
                if let Some(c) = self.connections[i].as_mut() {
                    if !c.is_quitting() {
                        let temprelay = (1u32 << i) as u8;
                        c.send_net_command_msg(msg, temprelay);
                    }
                }
            }
        }
        let _ = msg.write().detach();
    }

    /// `sendCommand` is declared in the header and never defined upstream.
    pub fn send_command(&mut self, _msg: &NetCommandMsgPtr) {}

    /// `ConnectionManager::allCommandsReady` (ConnectionManager.cpp:1480): whether every player's commands for the
    /// frame are in. A frame with too many commands is reset and re-requested from its sender. Unless only
    /// testing, the disconnect manager gets to hold the game back.
    pub fn all_commands_ready(&mut self, frame: u32, just_testing: bool) -> bool {
        let mut retval = true;
        let mut frame_ret_val = FrameDataReturnType::Ready;
        let mut i = 0;
        while i < SLOTS && retval {
            let debug_spewage = frame != self.commands_ready_debug_spewage && !just_testing;
            let result = match self.frame_data[i].as_mut() {
                Some(f) if !f.get_is_quitting() => Some(f.all_commands_ready(frame, debug_spewage)),
                _ => None,
            };
            if let Some(r) = result {
                frame_ret_val = r;
                if r == FrameDataReturnType::NotReady {
                    retval = false;
                } else if r == FrameDataReturnType::Resend {
                    self.request_frame_data_resend(i as i32, frame);
                    retval = false;
                }
            }
            i += 1;
        }
        if frame_ret_val == FrameDataReturnType::Resend {
            for j in 0..SLOTS {
                if j as u32 != self.local_slot {
                    if let Some(f) = self.frame_data[j].as_mut() {
                        f.reset_frame(frame, false);
                    }
                }
            }
        }
        if retval && !just_testing {
            let gf = self.host.game_frame();
            if let Some(mut dm) = self.disconnect_manager.take() {
                dm.all_commands_ready(gf, self, true);
                retval = dm.allowed_to_continue(); // the disconnect manager can keep us on this frame
                self.disconnect_manager = Some(dm);
            }
        }
        retval
    }

    /// `handleAllCommandsReady` (ConnectionManager.cpp:1520).
    pub fn handle_all_commands_ready(&mut self) {
        let gf = self.host.game_frame();
        if let Some(mut dm) = self.disconnect_manager.take() {
            dm.all_commands_ready(gf, self, false);
            self.disconnect_manager = Some(dm);
        }
    }

    /// `ConnectionManager::getFrameCommandList` (ConnectionManager.cpp:1526): all players' commands for the frame in
    /// one new list; the frame `FRAMES_TO_KEEP` frames back is then reset.
    pub fn get_frame_command_list(&mut self, frame: u32) -> NetCommandList {
        let mut retlist = NetCommandList::new();
        retlist.init();
        let keep = self.limits.frames_to_keep as u32;
        for i in 0..SLOTS {
            if let Some(f) = self.frame_data[i].as_mut() {
                retlist.append_list(f.get_frame_command_list(frame));
                if frame > keep {
                    f.reset_frame(frame - keep, true);
                }
            }
        }
        retlist
    }

    /// `setFrameGrouping` (ConnectionManager.cpp:1548): the packet router groups half as long.
    pub fn set_frame_grouping(&mut self, frame_grouping: i64) {
        let mut fg = frame_grouping;
        if self.local_slot == self.packet_router_slot {
            fg /= 2;
        }
        for i in 0..SLOTS {
            if let Some(c) = self.connections[i].as_mut() {
                c.set_frame_grouping(fg);
            }
        }
    }

    /// `determineRouterFallbackPlan` (ConnectionManager.cpp:1560): its body is inside a comment upstream.
    pub fn determine_router_fallback_plan(&mut self) {}

    /// `ConnectionManager::doKeepAlive` (ConnectionManager.cpp:1580): spreads one direct keep-alive per connection
    /// over each `MAX_SLOTS` seconds.
    fn do_keep_alive(&mut self) {
        let cur_time = self.host.time_get_time() as i64;
        if self.keep_alive_start_time == 0 {
            self.keep_alive_start_time = cur_time;
            return;
        }
        let num_seconds = (cur_time - self.keep_alive_start_time) / 1000;
        while (self.keep_alive_next_index as i64) <= num_seconds && self.keep_alive_next_index < MAX_SLOTS {
            let ni = self.keep_alive_next_index as usize;
            if self.connections[ni].is_some() {
                let mut m = NetCommandMsg::new_keep_alive();
                m.set_player_id(self.local_slot);
                if does_command_require_a_command_id(m.get_net_command_type()) {
                    m.set_id(generate_next_command_id());
                }
                let msg = NetCommandMsgPtr::new(m);
                self.send_local_command_direct(&msg, (1u32 << ni) as u8);
                let _ = msg.write().detach();
            }
            self.keep_alive_next_index += 1;
        }
        if self.keep_alive_next_index == MAX_SLOTS {
            self.keep_alive_next_index = 0;
            self.keep_alive_start_time = cur_time;
        }
    }

    /// `ConnectionManager::disconnectPlayer` (ConnectionManager.cpp:1610): takes a player out; if they were the
    /// packet router the next one in the fallback plan takes over; the plan then closes the gap.
    pub fn disconnect_player(&mut self, slot: i32) -> PlayerLeaveCode {
        let mut retval = PlayerLeaveCode::Client;
        if slot < 0 || slot >= MAX_SLOTS {
            return PlayerLeaveCode::Unknown;
        }
        let su = slot as usize;
        if let Some(last) = self.host.game_info_slot_last_frame_in_game(slot) {
            if last == 0 {
                let f = self.host.game_frame();
                self.host.game_info_slot_set_last_frame_in_game(slot, f);
            }
        }
        let unicode_name = self.get_player_name(slot);
        if unicode_name.get_length() > 0 && self.connections[su].is_some() {
            self.host.in_game_ui_player_left_game(&unicode_name);
            self.host.audio_add_event(b"GUIMessageReceived");
        }
        let drop_fd = self.frame_data[su].as_ref().map_or(false, |f| !f.get_is_quitting());
        if drop_fd {
            self.frame_data[su] = None;
        }
        let drop_conn = self.connections[su].as_ref().map_or(false, |c| !c.is_quitting());
        if drop_conn {
            self.connections[su] = None;
        }
        if slot as u32 == self.packet_router_slot {
            let mut index = 0usize;
            while index < SLOTS - 1 && self.packet_router_fallback[index] != self.packet_router_slot {
                index += 1;
            }
            index += 1;
            self.packet_router_slot = self.packet_router_fallback[index.min(SLOTS - 1)];
            retval = PlayerLeaveCode::PacketRouter;
        }
        if self.local_slot == slot as u32 {
            retval = PlayerLeaveCode::Local;
        }
        let mut fallbackindex = 0usize;
        while fallbackindex < SLOTS && self.packet_router_fallback[fallbackindex] != slot as u32 {
            fallbackindex += 1;
        }
        let mut i = fallbackindex;
        while i + 1 < SLOTS {
            self.packet_router_fallback[i] = self.packet_router_fallback[i + 1];
            i += 1;
        }
        self.packet_router_fallback[SLOTS - 1] = u32::MAX;
        retval
    }

    /// `quitGame` (ConnectionManager.cpp:1668): tells everyone, flushes, and disconnects everyone locally.
    pub fn quit_game(&mut self) {
        let mut m = NetCommandMsg::new_disconnect_player();
        m.set_disconnect_slot(self.local_slot as u8);
        m.set_player_id(self.local_slot);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command_direct(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        self.flush_connections(); // so our packet actually gets sent before the connections are deleted
        let _ = msg.write().detach();
        self.disconnect_local_player();
    }

    /// `disconnectLocalPlayer` (ConnectionManager.cpp:1685).
    pub fn disconnect_local_player(&mut self) {
        for i in 0..MAX_SLOTS {
            if i as u32 != self.local_slot {
                self.disconnect_player(i);
            }
        }
    }

    /// `flushConnections` (ConnectionManager.cpp:1694).
    pub fn flush_connections(&mut self) {
        let now = self.host.time_get_time();
        for i in 0..SLOTS {
            if let Some(c) = self.connections[i].as_mut() {
                c.do_send(now);
            }
        }
        if let Some(t) = self.transport.clone() {
            t.write().unwrap_or_else(|e| e.into_inner()).do_send(now);
        }
    }

    /// `resendPendingCommands` (ConnectionManager.cpp:1712): resends every command the packet router never
    /// stage-2 acked (after the router changed).
    pub fn resend_pending_commands(&mut self) {
        let mut todo: Vec<(NetCommandMsgPtr, u8)> = Vec::new();
        let mut r = self.pending_commands.get_first_message();
        while let Some(id) = r {
            if let Some(rr) = self.pending_commands.get(id) {
                todo.push((rr.get_command().clone(), rr.get_relay()));
                r = rr.get_next();
            } else {
                break;
            }
        }
        for (m, relay) in todo {
            self.send_local_command(&m, relay);
        }
    }

    /// `parseUserList` (ConnectionManager.cpp:1760): makes a connection and frame data for each human slot but the
    /// local one (whose frame data is the "local" kind) and fills the packet router fallback plan with the human
    /// slots in order.
    pub fn parse_user_list(&mut self, local_slot_num: i32, slots: &[ParseSlot]) {
        let mut num_users = 0usize;
        self.local_slot = u32::MAX;
        for i in 0..SLOTS.min(slots.len()) {
            let slot = &slots[i];
            if slot.is_human {
                if local_slot_num == i as i32 {
                    self.local_slot = i as u32;
                    self.local_user.set_name(slot.name.clone());
                }
                if self.local_slot != i as u32 {
                    let mut c = Connection::new();
                    c.init();
                    if let Some(t) = self.transport.clone() {
                        c.attach_transport(t);
                    }
                    c.set_user(Some(User::with_address(slot.name.clone(), slot.ip, slot.port as u32)));
                    self.connections[i] = Some(c);
                    self.frame_data[i] = Some(FrameDataManager::new(false, self.limits));
                } else {
                    self.frame_data[i] = Some(FrameDataManager::new(true, self.limits));
                }
                if let Some(f) = self.frame_data[i].as_mut() {
                    f.init();
                    f.reset();
                }
                if num_users < SLOTS {
                    self.packet_router_fallback[num_users] = i as u32;
                }
                num_users += 1;
            }
        }
    }

    /// `sendChat` (ConnectionManager.cpp:1850).
    pub fn send_chat(&mut self, text: UnicodeString, player_mask: i32, execution_frame: u32) {
        let mut m = NetCommandMsg::new_chat();
        m.set_text(text);
        m.set_chat_player_mask(player_mask);
        m.set_player_id(self.local_slot);
        m.set_id(0);
        m.set_execution_frame(execution_frame);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        self.process_chat(&msg);
        let _ = msg.write().detach();
    }

    /// `sendDisconnectChat` (ConnectionManager.cpp:1870). (Upstream never detaches the message.)
    pub fn send_disconnect_chat(&mut self, text: UnicodeString) {
        let mut m = NetCommandMsg::new_disconnect_chat();
        m.set_player_id(self.local_slot);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        m.set_text(text);
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command_direct(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        self.process_disconnect_chat(&msg);
    }

    /// `sendFileAnnounce` (ConnectionManager.cpp:1885): announces a file to the others; 0 when the file is empty
    /// or missing.
    pub fn send_file_announce(&mut self, path: &[u8], player_mask: u8) -> u16 {
        if self.host.local_file_read_nonempty(path).is_none() {
            let text = UnicodeString::try_from_chars(&ConnectionManager::<H>::utf16(&format!(
                "Not sending file '{}' to {:X}\n",
                String::from_utf8_lossy(path),
                player_mask
            )))
            .unwrap_or_default();
            self.host.lan_on_chat_system(&text);
            return 0;
        }
        let announce_mask = 0xffu32 ^ bit(self.local_slot);
        let mut m = NetCommandMsg::new_file_announce();
        m.set_player_id(self.local_slot);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let portable = self.host.real_map_path_to_portable(path);
        m.set_real_filename(path, |_| portable.clone());
        m.set_file_player_mask(player_mask);
        let file_id = generate_next_command_id();
        m.set_file_id(file_id);
        let announce = NetCommandMsgPtr::new(m);
        self.process_file_announce(&announce); // set up things for the host
        self.send_local_command(&announce, announce_mask as u8);
        let _ = announce.write().detach();
        file_id
    }

    /// `sendFile` (ConnectionManager.cpp:1925): sends the file's bytes (uncompressed; the `COMPRESS_TARGAS` path
    /// is not compiled) to the players in the mask.
    pub fn send_file(&mut self, path: &[u8], player_mask: u8, command_id: u16) {
        let buf = match self.host.local_file_read_nonempty(path) {
            Some(b) => b,
            None => {
                let text = UnicodeString::try_from_chars(&ConnectionManager::<H>::utf16(&format!(
                    "Not sending file '{}' to {:X}\n",
                    String::from_utf8_lossy(path),
                    player_mask
                )))
                .unwrap_or_default();
                self.host.lan_on_chat_system(&text);
                return;
            }
        };
        let mut m = NetCommandMsg::new_file();
        m.set_player_id(self.local_slot);
        m.set_id(command_id);
        let portable = self.host.real_map_path_to_portable(path);
        m.set_real_filename(path, |_| portable.clone());
        m.set_file_data(&buf);
        let file_msg = NetCommandMsgPtr::new(m);
        self.send_local_command(&file_msg, player_mask);
        let _ = file_msg.write().detach();
    }

    /// `getFileTransferProgress` (ConnectionManager.cpp:2000): the progress of the transfer of `path` to a
    /// player; 0 if there is no such transfer.
    pub fn get_file_transfer_progress(&mut self, player_id: i32, path: &[u8]) -> i32 {
        for (id, p) in self.file_command_map.iter() {
            if p.as_slice() == path {
                if player_id < 0 || player_id as usize >= SLOTS {
                    return 0;
                }
                return *self.file_progress_map[player_id as usize].get(id).unwrap_or(&0);
            }
        }
        0
    }

    /// `voteForPlayerDisconnect` (ConnectionManager.cpp:2015).
    pub fn vote_for_player_disconnect(&mut self, slot: i32) {
        if let Some(mut dm) = self.disconnect_manager.take() {
            dm.vote_for_player_disconnect(slot, self);
            self.disconnect_manager = Some(dm);
        }
    }

    /// `updateLoadProgress` (ConnectionManager.cpp:2035).
    pub fn update_load_progress(&mut self, progress: i32) {
        let mut m = NetCommandMsg::new_progress();
        m.set_percentage(progress as u8);
        m.set_player_id(self.local_slot);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.process_progress(&msg);
        self.send_local_command(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        let _ = msg.write().detach();
    }

    /// `loadProgressComplete` (ConnectionManager.cpp:2050): a plain `NetCommandMsg` of type LOADCOMPLETE.
    pub fn load_progress_complete(&mut self) {
        let mut m = NetCommandMsg::new();
        m.set_player_id(self.local_slot);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        m.set_net_command_type(NetCommandType::LoadComplete);
        let msg = NetCommandMsgPtr::new(m);
        self.process_load_complete(&msg);
        self.send_local_command(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        let _ = msg.write().detach();
    }

    /// `sendTimeOutGameStart` (ConnectionManager.cpp:2065).
    pub fn send_time_out_game_start(&mut self) {
        let mut m = NetCommandMsg::new();
        m.set_player_id(self.local_slot);
        m.set_net_command_type(NetCommandType::TimeoutStart);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.process_time_out_game_start();
        self.send_local_command(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        let _ = msg.write().detach();
    }

    /// `notifyOthersOfCurrentFrame` (ConnectionManager.cpp:2120): tells the others which frame this player is
    /// stuck on, and processes it locally too.
    pub fn notify_others_of_current_frame(&mut self, frame: i32) {
        let mut m = NetCommandMsg::new_disconnect_frame();
        m.set_player_id(self.local_slot);
        m.set_disconnect_frame(frame as u32);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command_direct(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        let mut r = NetCommandRef::new(msg.clone());
        r.set_relay(bit(self.local_slot) as u8);
        if let Some(mut dm) = self.disconnect_manager.take() {
            dm.process_disconnect_command(&r, self);
            self.disconnect_manager = Some(dm);
        }
        drop(r);
        let _ = msg.write().detach();
    }

    /// `notifyOthersOfNewFrame` (ConnectionManager.cpp:2140).
    pub fn notify_others_of_new_frame(&mut self, frame: u32) {
        let mut m = NetCommandMsg::new_disconnect_screen_off();
        m.set_player_id(self.local_slot);
        m.set_new_frame(frame);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        self.send_local_command_direct(&msg, (0xffu32 ^ bit(self.local_slot)) as u8);
        let mut r = NetCommandRef::new(msg.clone());
        r.set_relay(bit(self.local_slot) as u8);
        if let Some(mut dm) = self.disconnect_manager.take() {
            dm.process_disconnect_command(&r, self);
            self.disconnect_manager = Some(dm);
        }
        drop(r);
        let _ = msg.write().detach();
    }

    /// `sendFrameDataToPlayer` (ConnectionManager.cpp:2160): resends every frame from `startingFrame` up to the
    /// current one.
    pub fn send_frame_data_to_player(&mut self, player_id: u32, starting_frame: u32) {
        let mut frame = starting_frame;
        while frame < self.host.game_frame() {
            self.send_single_frame_to_player(player_id, frame);
            frame += 1;
        }
    }

    /// `sendSingleFrameToPlayer` (ConnectionManager.cpp:2170): every other player's commands for the frame and
    /// its frame-info, directly to the player (frames older than `FRAMES_TO_KEEP` are gone).
    pub fn send_single_frame_to_player(&mut self, player_id: u32, frame: u32) {
        if self.host.game_frame().wrapping_sub(self.limits.frames_to_keep as u32) > frame {
            return;
        }
        let relay = bit(player_id) as u8;
        for i in 0..SLOTS {
            if self.frame_data[i].is_some() && i as u32 != player_id {
                // no need to send his own commands to him
                let cmds: Vec<NetCommandMsgPtr> = {
                    let mut v = Vec::new();
                    if let Some(l) = self.frame_data[i].as_ref().and_then(|f| f.get_frame_command_list(frame)) {
                        let mut r = l.get_first_message();
                        while let Some(id) = r {
                            match l.get(id) {
                                Some(rr) => {
                                    v.push(rr.get_command().clone());
                                    r = rr.get_next();
                                }
                                None => break,
                            }
                        }
                    }
                    v
                };
                for c in cmds {
                    self.send_local_command_direct(&c, relay);
                }
                let frame_command_count = self.frame_data[i].as_ref().map_or(0, |f| f.get_frame_command_count(frame));
                let mut m = NetCommandMsg::new_frame();
                m.set_execution_frame(frame);
                m.set_command_count(frame_command_count as u16);
                if does_command_require_a_command_id(m.get_net_command_type()) {
                    m.set_id(generate_next_command_id());
                }
                m.set_player_id(i as u32);
                let msg = NetCommandMsgPtr::new(m);
                self.send_local_command_direct(&msg, relay);
                let _ = msg.write().detach();
            }
        }
    }

    /// `requestFrameDataResend` (ConnectionManager.cpp:2230): asks a connected player (the given one, else the
    /// first connected) to resend the frame.
    pub fn request_frame_data_resend(&mut self, player_id: i32, frame: u32) {
        let mut m = NetCommandMsg::new_frame_resend_request();
        m.set_player_id(self.local_slot);
        m.set_frame_to_resend(frame);
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let mut pid = player_id;
        if !self.is_player_connected(pid) {
            pid = 0;
            while pid < MAX_SLOTS && !self.is_player_connected(pid) {
                pid += 1;
            }
        }
        if pid < MAX_SLOTS {
            let msg = NetCommandMsgPtr::new(m);
            self.send_local_command_direct(&msg, (1u32 << pid) as u8);
            let _ = msg.write().detach();
        }
    }
}

// keep the enum import used by the pattern in `process_net_command`
#[allow(dead_code)]
fn _uses(_: &NetCommandBody) {}
