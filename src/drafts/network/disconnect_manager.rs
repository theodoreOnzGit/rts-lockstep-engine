//! PARTLY DIFFERENTIAL-TESTED (#215): the members that need no network match the upstream C++ on 6,000 operation sequences (test:
//! tests/differential_draft_conn.rs): `new`, `init`, the ping getters, `apply_disconnect_vote`, `next_frame`, `all_commands_ready`,
//! `allowed_to_continue`, `translated_slot_position`, `untranslated_slot_position`, `reset_player_timeouts`, `reset_player_timeout`,
//! `reset_packet_router_timeout`, `has_player_timed_out`, `is_player_voted_out`, `is_player_in_game`, `get_max_disconnect_frame`,
//! `player_has_advanced_a_frame`, `count_votes_for_player`, `reset_players_votes` (the surroundings, the menu, the clock and the game
//! frame, are stand-ins on both sides). Inputs left out: out-of-range slots upstream indexes arrays with (the draft guards them), and
//! `all_commands_ready` with the local slot unset. NOT covered (they send commands or read the connections' state): `update`,
//! `update_disconnect_status`, `update_wait_for_packet_router`, the seven `process_*`, `send_keep_alive`, `populate_disconnect_screen`,
//! `turn_on_screen`, `disconnect_player`, `send_disconnect_command`, `send_vote_command`, `vote_for_player_disconnect`,
//! `recalculate_packet_router_index`, `all_on_same_frame`, `is_local_player_next_packet_router`, `send_player_destruct`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/DisconnectManager.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/DisconnectManager.cpp
//!
//! The disconnect manager runs the "waiting for player" screen: when the game is stuck on a frame it shows the
//! screen, sends keep-alives, counts timeouts and votes, and drops a player once the remaining players agree.
//!
//! # Shape
//!
//! Every function that takes `ConnectionManager *conMgr` takes `&mut ConnectionManager<H>` here; the
//! connection manager owns this object in an `Option` and `take`s it around each call into it (so the call does not
//! alias `self`). `TheGameLogic`, `TheGlobalData`, `TheNetwork`, `TheDisconnectMenu`, `TheRecorder`,
//! `TheGameInfo` and `ThePinger` are the connection manager's [`ConnectionManagerHost`].

use super::connection_manager::{ConnectionManager, ConnectionManagerHost};
use crate::logic::modules::plant;
use super::net_command_msg::{NetCommandMsg, NetCommandMsgPtr, NetCommandType, MAX_SLOTS, PlayerLeaveCode};
use super::net_command_ref::NetCommandRef;
use super::network_util::{does_command_require_a_command_id, generate_next_command_id};

const SLOTS: usize = MAX_SLOTS as usize;

/// `enum DisconnectStateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectStateType {
    /// `DISCONNECTSTATETYPE_SCREENON`.
    ScreenOn,
    /// `DISCONNECTSTATETYPE_SCREENOFF`.
    ScreenOff,
}

/// `struct DisconnectVoteStruct`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DisconnectVoteStruct {
    /// `vote`.
    pub vote: bool,
    /// `frame`.
    pub frame: u32,
}

/// `class DisconnectManager`.
#[derive(Debug)]
pub struct DisconnectManager {
    /// `m_lastFrame`.
    last_frame: u32,
    /// `m_lastFrameTime`.
    last_frame_time: i64,
    /// `m_disconnectState`.
    disconnect_state: DisconnectStateType,
    /// `m_packetRouterFallback` (unused upstream beyond zeroing).
    packet_router_fallback: [u32; SLOTS],
    /// `m_currentPacketRouterIndex`.
    current_packet_router_index: u32,
    /// `m_lastKeepAliveSendTime`.
    last_keep_alive_send_time: i64,
    /// `m_playerTimeouts[MAX_SLOTS - 1]`.
    player_timeouts: [i64; SLOTS - 1],
    /// `m_packetRouterTimeout`.
    packet_router_timeout: i64,
    /// `m_playerVotes[MAX_SLOTS][MAX_SLOTS]`.
    player_votes: [[DisconnectVoteStruct; SLOTS]; SLOTS],
    /// `m_disconnectFrames`.
    disconnect_frames: [u32; SLOTS],
    /// `m_disconnectFramesReceived`.
    disconnect_frames_received: [bool; SLOTS],
    /// `m_haveNotifiedOtherPlayersOfCurrentFrame`.
    have_notified_other_players_of_current_frame: bool,
    /// `m_timeOfDisconnectScreenOn`.
    time_of_disconnect_screen_on: i64,
    /// `m_pingsSent`.
    pings_sent: i32,
    /// `m_pingsRecieved`.
    pings_recieved: i32,
    /// `m_pingFrame`.
    ping_frame: u32,
    /// `static int serverIndex` of `update`.
    ping_server_index: i32,
}

impl Default for DisconnectManager {
    fn default() -> Self {
        DisconnectManager::new()
    }
}

impl DisconnectManager {
    /// Differential-test accessor (#215): the members the C++ test dumps, as words, in its order.
    #[doc(hidden)]
    pub fn debug_words(&self) -> Vec<u32> {
        let mut o = Vec::new();
        let push64 = |o: &mut Vec<u32>, v: i64| {
            o.push(v as u32);
            o.push(((v as u64) >> 32) as u32);
        };
        o.push(self.last_frame);
        push64(&mut o, self.last_frame_time);
        o.push(match self.disconnect_state {
            DisconnectStateType::ScreenOn => 0,
            DisconnectStateType::ScreenOff => 1,
        });
        push64(&mut o, self.packet_router_timeout);
        for t in self.player_timeouts.iter() {
            push64(&mut o, *t);
        }
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                o.push(self.player_votes[i][j].vote as u32);
                o.push(self.player_votes[i][j].frame);
            }
        }
        for i in 0..SLOTS {
            o.push(self.disconnect_frames[i]);
            o.push(self.disconnect_frames_received[i] as u32);
        }
        push64(&mut o, self.time_of_disconnect_screen_on);
        o.push(self.pings_sent as u32);
        o.push(self.pings_recieved as u32);
        o.push(self.ping_frame);
        o.push(self.current_packet_router_index);
        push64(&mut o, self.last_keep_alive_send_time);
        o
    }

    /// Differential-test hook (#215): `m_disconnectState = DISCONNECTSTATETYPE_SCREENON`.
    #[doc(hidden)]
    pub fn debug_force_screen_on(&mut self) {
        self.disconnect_state = DisconnectStateType::ScreenOn;
    }

    /// `DisconnectManager::DisconnectManager()` (DisconnectManager.cpp:40).
    pub fn new() -> DisconnectManager {
        DisconnectManager {
            current_packet_router_index: 0,
            last_frame: 0,
            last_frame_time: 0,
            last_keep_alive_send_time: 0,
            have_notified_other_players_of_current_frame: false,
            time_of_disconnect_screen_on: 0,
            packet_router_fallback: [0; SLOTS],
            packet_router_timeout: 0,
            player_timeouts: [0; SLOTS - 1],
            player_votes: [[DisconnectVoteStruct::default(); SLOTS]; SLOTS],
            disconnect_state: DisconnectStateType::ScreenOff,
            disconnect_frames: [0; SLOTS],
            disconnect_frames_received: [false; SLOTS],
            pings_sent: 0,
            pings_recieved: 0,
            ping_frame: 0,
            ping_server_index: 0,
        }
    }

    /// `DisconnectManager::init` (DisconnectManager.cpp:65): hides the screen and clears the state.
    pub fn init<H: ConnectionManagerHost>(&mut self, host: &mut H) {
        host.disconnect_menu_hide_screen(); // make sure the screen starts out hidden
        self.last_frame = 0;
        self.last_frame_time = if plant(15105) { 0 } else { -1 };
        self.last_keep_alive_send_time = -1;
        self.disconnect_state = DisconnectStateType::ScreenOff;
        self.current_packet_router_index = 0;
        self.time_of_disconnect_screen_on = 0;
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                self.player_votes[i][j].vote = false;
                self.player_votes[i][j].frame = 0;
            }
        }
        for i in 0..SLOTS {
            self.disconnect_frames[i] = 0;
            self.disconnect_frames_received[i] = false;
        }
        self.ping_frame = 0;
        self.pings_sent = 0;
        self.pings_recieved = 0;
    }

    /// `DisconnectManager::update` (DisconnectManager.cpp:89): once the game has been stuck on a frame longer
    /// than `m_networkDisconnectTime` the screen goes on and keep-alives go out; a new frame resets the timeouts.
    /// While the screen is on, a ping to a GameSpy ping server is sent after 10 seconds so the screen can tell a
    /// local network failure from a remote one.
    pub fn update<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        if self.last_frame_time == -1 {
            self.last_frame_time = conn.host.time_get_time() as i64;
        }
        if conn.host.game_frame() == self.last_frame {
            let cur_time = conn.host.time_get_time() as i64;
            if (cur_time - self.last_frame_time) > conn.host.global_data().network_disconnect_time as i64 {
                if self.disconnect_state == DisconnectStateType::ScreenOff {
                    self.turn_on_screen(conn);
                }
                self.send_keep_alive(conn);
            }
        } else {
            let f = conn.host.game_frame();
            self.next_frame(f, conn);
        }

        if self.disconnect_state != DisconnectStateType::ScreenOff {
            self.update_disconnect_status(conn);

            if self.ping_frame < conn.host.game_frame() {
                let cur_time = conn.host.time_get_time() as i64;
                if (cur_time - self.last_frame_time) > 10000 {
                    // @todo: plug in some better measure here
                    self.ping_frame = conn.host.game_frame();
                    self.pings_sent = 0;
                    self.pings_recieved = 0;
                    if conn.host.pinger_available() {
                        self.ping_server_index += 1;
                        let count = conn.host.ping_server_count() as i32;
                        if self.ping_server_index >= count {
                            self.ping_server_index = 0; // wrap back to first ping server
                        }
                        let hostname = conn.host.ping_server_hostname(self.ping_server_index as usize);
                        let repetitions = 5;
                        let timeout = 2000;
                        self.pings_sent = repetitions;
                        conn.host.ping_add_request(&hostname, repetitions, timeout);
                    }
                }
            }

            if conn.host.pinger_available() {
                while let Some(resp) = conn.host.ping_get_response() {
                    if self.ping_frame != conn.host.game_frame() {
                        // a response to an old request: logged only
                    } else if resp.avg_ping < 2000 {
                        self.pings_recieved += resp.repetitions;
                    }
                }
            }
        }
    }

    /// `getPingFrame`.
    pub fn get_ping_frame(&self) -> u32 {
        self.ping_frame
    }
    /// `getPingsSent`.
    pub fn get_pings_sent(&self) -> i32 {
        self.pings_sent
    }
    /// `getPingsRecieved`.
    pub fn get_pings_recieved(&self) -> i32 {
        self.pings_recieved
    }

    /// `DisconnectManager::updateDisconnectStatus` (DisconnectManager.cpp:196): refreshes each player's timeout on
    /// the screen; when a player has timed out (or been voted out) and everyone is on the same frame, the next
    /// packet router disconnects them.
    pub fn update_disconnect_status<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        for i in 0..MAX_SLOTS {
            if conn.is_player_connected(i) {
                let slot = self.translated_slot_position(i, conn.get_local_player_id() as i32);
                if slot != -1 {
                    let cur_time = conn.host.time_get_time() as i64;
                    let timeout = conn.host.global_data().network_player_timeout_time as i64;
                    let mut new_time = timeout - (cur_time - self.player_timeouts[slot as usize]);

                    if !self.have_notified_other_players_of_current_frame {
                        if new_time < timeout / 3 || self.is_player_voted_out(slot, conn) {
                            conn.host.notify_others_of_current_frame();
                            self.have_notified_other_players_of_current_frame = true;
                        }
                        if self.time_of_disconnect_screen_on != 0
                            && (cur_time - self.time_of_disconnect_screen_on)
                                > conn.host.global_data().network_disconnect_screen_notify_time as i64
                        {
                            conn.host.notify_others_of_current_frame();
                            self.have_notified_other_players_of_current_frame = true;
                        }
                    }

                    if new_time < 0 || self.is_player_voted_out(slot, conn) {
                        new_time = 0;
                        if self.all_on_same_frame(conn) && self.is_local_player_next_packet_router(conn) {
                            self.send_disconnect_command(i, conn);
                            self.disconnect_player(i, conn);
                            self.send_player_destruct(i, conn);
                        }
                    }
                    conn.host.disconnect_menu_set_player_timeout_time(slot, new_time as i32);
                }
            }
        }
    }

    /// `DisconnectManager::updateWaitForPacketRouter` (DisconnectManager.cpp:263): its whole body is inside a
    /// comment upstream (the wait-for-packet-router state was removed); nothing to do.
    pub fn update_wait_for_packet_router<H: ConnectionManagerHost>(&mut self, _conn: &mut ConnectionManager<H>) {}

    /// `DisconnectManager::processDisconnectCommand` (DisconnectManager.cpp:277).
    pub fn process_disconnect_command<H: ConnectionManagerHost>(
        &mut self,
        r: &NetCommandRef,
        conn: &mut ConnectionManager<H>,
    ) {
        let msg = r.get_command().clone();
        let ty = msg.read().get_net_command_type();
        match ty {
            NetCommandType::DisconnectKeepAlive => self.process_disconnect_keep_alive(&msg, conn),
            NetCommandType::DisconnectPlayer => self.process_disconnect_player(&msg, conn),
            NetCommandType::PacketRouterQuery => self.process_packet_router_query(&msg, conn),
            NetCommandType::PacketRouterAck => self.process_packet_router_ack(&msg, conn),
            NetCommandType::DisconnectVote => self.process_disconnect_vote(&msg, conn),
            NetCommandType::DisconnectFrame => self.process_disconnect_frame(&msg, conn),
            NetCommandType::DisconnectScreenOff => self.process_disconnect_screen_off(&msg, conn),
            _ => {}
        }
    }

    /// `processDisconnectKeepAlive` (DisconnectManager.cpp:300): the sender's timeout restarts.
    pub fn process_disconnect_keep_alive<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let pid = msg.read().get_player_id() as i32;
        let slot = self.translated_slot_position(pid, conn.get_local_player_id() as i32);
        if slot != -1 {
            self.reset_player_timeout(slot, conn.host.time_get_time());
        }
    }

    /// `processDisconnectPlayer` (DisconnectManager.cpp:311).
    pub fn process_disconnect_player<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        // DEBUG_ASSERTCRASH(TheGameLogic->getFrame() == cmdMsg->getDisconnectFrame(), "disconnecting player on the wrong frame!!!")
        let slot = msg.read().get_disconnect_slot() as i32;
        self.disconnect_player(slot, conn);
    }

    /// `processPacketRouterQuery` (DisconnectManager.cpp:320): the packet router answers with an ack.
    pub fn process_packet_router_query<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let from = msg.read().get_player_id();
        if conn.get_packet_router_slot() == conn.get_local_player_id() {
            let mut ack = NetCommandMsg::new_packet_router_ack();
            ack.set_player_id(conn.get_local_player_id());
            if does_command_require_a_command_id(ack.get_net_command_type()) {
                ack.set_id(generate_next_command_id());
            }
            let ackmsg = NetCommandMsgPtr::new(ack);
            conn.send_local_command_direct(&ackmsg, super::connection_manager::bit(from) as u8);
            let _ = ackmsg.write().detach();
        }
    }

    /// `processPacketRouterAck` (DisconnectManager.cpp:338).
    pub fn process_packet_router_ack<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let from = msg.read().get_player_id();
        if conn.get_packet_router_slot() == from {
            self.reset_packet_router_timeout(conn.host.time_get_time());
            let current_packet_router_slot = conn.get_packet_router_slot();
            let mut current_packet_router_index: i32 = 0;
            while current_packet_router_slot != conn.get_packet_router_fallback_slot(current_packet_router_index)
                && current_packet_router_index < MAX_SLOTS
            {
                current_packet_router_index += 1;
            }
            conn.resend_pending_commands();
            self.current_packet_router_index = current_packet_router_index as u32;
            // set to screen on so that the next call to allCommandsReady sets up the next frame properly
            self.disconnect_state = DisconnectStateType::ScreenOn;
        }
    }

    /// `processDisconnectVote` (DisconnectManager.cpp:366).
    pub fn process_disconnect_vote<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let (pid, slot, frame) = {
            let m = msg.read();
            (m.get_player_id(), m.get_vote_slot(), m.get_vote_frame())
        };
        let trans_slot = self.translated_slot_position(pid as i32, conn.get_local_player_id() as i32);
        if !self.is_player_in_game(trans_slot, conn) {
            return;
        }
        self.apply_disconnect_vote(slot as i32, frame, pid as i32, conn);
    }

    /// `processDisconnectFrame` (DisconnectManager.cpp:380): a player says which frame it is stuck on; frame data
    /// is resent to whoever is behind.
    pub fn process_disconnect_frame<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let (player_id, disconnect_frame) = {
            let m = msg.read();
            (m.get_player_id(), m.get_disconnect_frame())
        };
        let pid = player_id as usize;
        if pid >= SLOTS {
            return;
        }
        if self.disconnect_frames[pid] >= disconnect_frame {
            return;
        }
        self.reset_players_votes(player_id as i32, disconnect_frame.wrapping_sub(1), conn);
        self.disconnect_frames[pid] = disconnect_frame;
        self.disconnect_frames_received[pid] = true;

        let local = conn.get_local_player_id();
        if player_id == local {
            for i in 0..MAX_SLOTS {
                if i as u32 != player_id {
                    let trans_slot = self.translated_slot_position(i, local as i32);
                    if self.is_player_in_game(trans_slot, conn) {
                        let iu = i as usize;
                        if self.disconnect_frames[iu] < self.disconnect_frames[pid]
                            && self.disconnect_frames_received[iu]
                        {
                            conn.send_frame_data_to_player(i as u32, self.disconnect_frames[iu]);
                        }
                    }
                }
            }
        } else if (local as usize) < SLOTS
            && self.disconnect_frames[pid] < self.disconnect_frames[local as usize]
            && self.disconnect_frames_received[pid]
        {
            conn.send_frame_data_to_player(player_id, self.disconnect_frames[pid]);
        }
    }

    /// `processDisconnectScreenOff` (DisconnectManager.cpp:416).
    pub fn process_disconnect_screen_off<H: ConnectionManagerHost>(
        &mut self,
        msg: &NetCommandMsgPtr,
        conn: &mut ConnectionManager<H>,
    ) {
        let (player_id, new_frame) = {
            let m = msg.read();
            (m.get_player_id(), m.get_new_frame())
        };
        if player_id as usize >= SLOTS {
            return;
        }
        if new_frame >= self.disconnect_frames[player_id as usize] {
            self.disconnect_frames_received[player_id as usize] = false;
            // in case the screen-off message gets here before the disconnect frame message
            self.disconnect_frames[player_id as usize] = new_frame;
            self.reset_players_votes(player_id as i32, new_frame, conn);
        }
    }

    /// `applyDisconnectVote` (DisconnectManager.cpp:480).
    pub fn apply_disconnect_vote<H: ConnectionManagerHost>(
        &mut self,
        slot: i32,
        frame: u32,
        from_slot: i32,
        conn: &mut ConnectionManager<H>,
    ) {
        if slot < 0 || slot >= MAX_SLOTS || from_slot < 0 || from_slot >= MAX_SLOTS {
            return;
        }
        let early_votes = self.count_votes_for_player(slot, conn.host.game_frame());
        self.player_votes[slot as usize][from_slot as usize].vote = true;
        if !plant(15100) {
            self.player_votes[slot as usize][from_slot as usize].frame = frame;
        }
        let num_votes = if plant(15108) { early_votes } else { self.count_votes_for_player(slot, conn.host.game_frame()) };
        let trans_slot = self.translated_slot_position(slot, conn.get_local_player_id() as i32);
        if trans_slot != -1 {
            conn.host.disconnect_menu_update_votes(if plant(15107) { slot } else { trans_slot }, num_votes);
        }
    }

    /// `DisconnectManager::nextFrame` (DisconnectManager.cpp:460).
    pub fn next_frame<H: ConnectionManagerHost>(&mut self, frame: u32, conn: &mut ConnectionManager<H>) {
        self.last_frame = frame;
        self.last_frame_time = conn.host.time_get_time() as i64;
        if !plant(15103) {
            self.reset_player_timeouts(conn);
        }
    }

    /// `DisconnectManager::allCommandsReady` (DisconnectManager.cpp:435): all commands arrived, so the screen goes
    /// off (the others are told the new frame and the local votes reset).
    pub fn all_commands_ready<H: ConnectionManagerHost>(
        &mut self,
        frame: u32,
        conn: &mut ConnectionManager<H>,
        _wait_for_packet_router: bool,
    ) {
        if self.disconnect_state != DisconnectStateType::ScreenOff {
            conn.host.disconnect_menu_hide_screen();
            self.disconnect_state = DisconnectStateType::ScreenOff;
            conn.host.notify_others_of_new_frame(frame);

            // reset the votes since we're moving to a new frame
            let local = conn.get_local_player_id() as usize;
            for i in 0..SLOTS {
                if local < SLOTS {
                    if plant(15102) {
                        self.player_votes[local][i].vote = false;
                    } else {
                        self.player_votes[i][local].vote = false;
                    }
                }
            }
            self.time_of_disconnect_screen_on = 0;
        }
    }

    /// `DisconnectManager::allowedToContinue` (DisconnectManager.cpp:480): the next frame may go only while the
    /// screen is off.
    pub fn allowed_to_continue(&self) -> bool {
        (self.disconnect_state == DisconnectStateType::ScreenOff) != plant(15106)
    }

    /// `sendKeepAlive` (DisconnectManager.cpp:490): at most every 500 ms, to everyone but the local player.
    pub fn send_keep_alive<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        let cur_time = conn.host.time_get_time() as i64;
        if (cur_time - self.last_keep_alive_send_time) > 500 || self.last_keep_alive_send_time == -1 {
            let mut m = NetCommandMsg::new_disconnect_keep_alive();
            m.set_player_id(conn.get_local_player_id());
            if does_command_require_a_command_id(m.get_net_command_type()) {
                m.set_id(generate_next_command_id());
            }
            let pid = m.get_player_id();
            let msg = NetCommandMsgPtr::new(m);
            conn.send_local_command_direct(&msg, (0xffu32 ^ super::connection_manager::bit(pid)) as u8);
            let _ = msg.write().detach();
            self.last_keep_alive_send_time = cur_time;
        }
    }

    /// `populateDisconnectScreen` (DisconnectManager.cpp:510).
    pub fn populate_disconnect_screen<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        for i in 0..MAX_SLOTS {
            let name = conn.get_player_name(i);
            let slot = self.translated_slot_position(i, conn.get_local_player_id() as i32);
            if slot != -1 {
                conn.host.disconnect_menu_set_player_name(slot, &name);
                let num_votes = self.count_votes_for_player(i, conn.host.game_frame());
                conn.host.disconnect_menu_update_votes(slot, num_votes);
            }
        }
    }

    /// `translatedSlotPosition` (DisconnectManager.cpp:525): the slot's position with the local player's slot
    /// taken out (-1 for the local player).
    pub fn translated_slot_position(&self, slot: i32, local_slot: i32) -> i32 {
        if if plant(15092) { slot <= local_slot } else { slot < local_slot } {
            return slot;
        }
        if slot == local_slot {
            return -1;
        }
        slot - 1
    }

    /// `untranslatedSlotPosition` (DisconnectManager.cpp:536).
    pub fn untranslated_slot_position(&self, slot: i32, local_slot: i32) -> i32 {
        if slot == -1 {
            return local_slot;
        }
        if if plant(15093) { slot <= local_slot } else { slot < local_slot } {
            return slot;
        }
        slot + 1
    }

    /// `resetPlayerTimeouts` (DisconnectManager.cpp:548).
    pub fn reset_player_timeouts<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        let now = conn.host.time_get_time();
        for i in 0..MAX_SLOTS {
            let slot = self.translated_slot_position(i, conn.get_local_player_id() as i32);
            if slot != -1 && !(plant(15104) && slot == 0) {
                self.reset_player_timeout(slot, now);
            }
        }
    }

    /// `resetPlayerTimeout` (DisconnectManager.cpp:560).
    pub fn reset_player_timeout(&mut self, slot: i32, now: u32) {
        if slot >= 0 && (slot as usize) < SLOTS - 1 {
            self.player_timeouts[slot as usize] = now as i64;
        }
    }

    /// `resetPacketRouterTimeout`.
    pub fn reset_packet_router_timeout(&mut self, now: u32) {
        self.packet_router_timeout = now as i64;
    }

    /// `turnOnScreen` (DisconnectManager.cpp:578).
    pub fn turn_on_screen<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        conn.host.disconnect_menu_show_screen();
        self.disconnect_state = DisconnectStateType::ScreenOn;
        self.last_keep_alive_send_time = -1;
        self.populate_disconnect_screen(conn);
        self.reset_player_timeouts(conn);
        conn.host.disconnect_menu_hide_packet_router_timeout();
        self.have_notified_other_players_of_current_frame = false;
        self.time_of_disconnect_screen_on = conn.host.time_get_time() as i64;
    }

    /// `disconnectPlayer` (DisconnectManager.cpp:592): marks the slot disconnected, logs it in the recorder,
    /// takes the player off the screen and out of the connection manager.
    pub fn disconnect_player<H: ConnectionManagerHost>(&mut self, slot: i32, conn: &mut ConnectionManager<H>) {
        if slot < 0 || slot >= MAX_SLOTS {
            return;
        }
        conn.host.game_info_mark_slot_as_disconnected(slot);

        let trans_slot = self.translated_slot_position(slot, conn.get_local_player_id() as i32);
        if trans_slot != -1 {
            let uname = conn.get_player_name(slot);
            conn.host.recorder_log_player_disconnect(&uname, slot);
            conn.host.disconnect_menu_remove_player(trans_slot, &uname);
            let retcode = conn.disconnect_player(slot);
            // DEBUG_ASSERTCRASH(retcode != PLAYERLEAVECODE_UNKNOWN, "Invalid player leave code")
            if retcode == PlayerLeaveCode::PacketRouter {
                conn.resend_pending_commands();
            }
        }
    }

    /// `sendDisconnectCommand` (DisconnectManager.cpp:622): tells everyone to disconnect the slot at the highest
    /// frame anyone reached.
    pub fn send_disconnect_command<H: ConnectionManagerHost>(&mut self, slot: i32, conn: &mut ConnectionManager<H>) {
        if slot < 0 || slot >= MAX_SLOTS {
            return;
        }
        let disconnect_frame = self.get_max_disconnect_frame();
        let mut m = NetCommandMsg::new_disconnect_player();
        m.set_disconnect_slot(slot as u8);
        m.set_disconnect_frame(disconnect_frame);
        m.set_player_id(conn.get_local_player_id());
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let msg = NetCommandMsgPtr::new(m);
        conn.send_local_command(&msg, 0xff);
        let _ = msg.write().detach();
    }

    /// `sendVoteCommand` (DisconnectManager.cpp:644).
    pub fn send_vote_command<H: ConnectionManagerHost>(&mut self, slot: i32, conn: &mut ConnectionManager<H>) {
        let mut m = NetCommandMsg::new_disconnect_vote();
        m.set_player_id(conn.get_local_player_id());
        m.set_vote_slot(slot as u8);
        m.set_vote_frame(conn.host.game_frame());
        if does_command_require_a_command_id(m.get_net_command_type()) {
            m.set_id(generate_next_command_id());
        }
        let local = conn.get_local_player_id();
        let msg = NetCommandMsgPtr::new(m);
        conn.send_local_command_direct(&msg, (0xffu32 & !super::connection_manager::bit(local)) as u8);
        let _ = msg.write().detach();
    }

    /// `voteForPlayerDisconnect` (DisconnectManager.cpp:660): casts the local vote once per frame.
    pub fn vote_for_player_disconnect<H: ConnectionManagerHost>(
        &mut self,
        slot: i32,
        conn: &mut ConnectionManager<H>,
    ) {
        let trans_slot = self.untranslated_slot_position(slot, conn.get_local_player_id() as i32);
        let local = conn.get_local_player_id() as usize;
        if trans_slot < 0 || trans_slot >= MAX_SLOTS || local >= SLOTS {
            return;
        }
        if !self.player_votes[trans_slot as usize][local].vote {
            self.player_votes[trans_slot as usize][local].vote = true;
            self.send_vote_command(trans_slot, conn);
            let f = conn.host.game_frame();
            self.apply_disconnect_vote(trans_slot, f, local as i32, conn);
        }
    }

    /// `recalculatePacketRouterIndex` (DisconnectManager.cpp:675).
    pub fn recalculate_packet_router_index<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) {
        let current_packet_router_slot = conn.get_packet_router_slot();
        self.current_packet_router_index = 0;
        while current_packet_router_slot != conn.get_packet_router_fallback_slot(self.current_packet_router_index as i32)
            && (self.current_packet_router_index as i32) < MAX_SLOTS
        {
            self.current_packet_router_index += 1;
        }
    }

    /// `allOnSameFrame` (DisconnectManager.cpp:687): every player still in the game has reported a disconnect frame
    /// equal to ours.
    pub fn all_on_same_frame<H: ConnectionManagerHost>(&mut self, conn: &mut ConnectionManager<H>) -> bool {
        let mut retval = true;
        let local = conn.get_local_player_id();
        let mut i = 0;
        while i < MAX_SLOTS && retval {
            let trans_slot = self.translated_slot_position(i, local as i32);
            if trans_slot == -1 {
                i += 1;
                continue;
            }
            if conn.is_player_connected(i) && self.is_player_in_game(trans_slot, conn) {
                if !self.disconnect_frames_received[i as usize] {
                    retval = false;
                }
                if self.disconnect_frames_received[i as usize]
                    && (local as usize) < SLOTS
                    && self.disconnect_frames[local as usize] != self.disconnect_frames[i as usize]
                {
                    retval = false;
                }
            }
            i += 1;
        }
        retval
    }

    /// `isLocalPlayerNextPacketRouter` (DisconnectManager.cpp:715): the local player is the first player still in
    /// the game in the fallback order starting at the current packet router.
    pub fn is_local_player_next_packet_router<H: ConnectionManagerHost>(
        &mut self,
        conn: &mut ConnectionManager<H>,
    ) -> bool {
        let local_slot = conn.get_local_player_id();
        let mut packet_router_slot = conn.get_packet_router_slot();
        let mut trans_slot = self.translated_slot_position(packet_router_slot as i32, local_slot as i32);
        while trans_slot != -1 && !self.is_player_in_game(trans_slot, conn) {
            packet_router_slot = conn.get_next_packet_router_slot(packet_router_slot);
            if packet_router_slot >= MAX_SLOTS as u32 {
                // DEBUG_CRASH("no more players left to be the packet router, this shouldn't happen.")
                return false;
            }
            trans_slot = self.translated_slot_position(packet_router_slot as i32, local_slot as i32);
        }
        packet_router_slot == local_slot
    }

    /// `hasPlayerTimedOut` (DisconnectManager.cpp:745).
    pub fn has_player_timed_out<H: ConnectionManagerHost>(&self, slot: i32, conn: &mut ConnectionManager<H>) -> bool {
        if slot == -1 || slot < 0 || slot as usize >= SLOTS - 1 {
            return false;
        }
        let timeout = conn.host.global_data().network_player_timeout_time as i64;
        let new_time = timeout - (conn.host.time_get_time() as i64 - self.player_timeouts[slot as usize]);
        if plant(15097) { new_time < 0 } else { new_time <= 0 }
    }

    /// `sendPlayerDestruct` (DisconnectManager.cpp:757): the destroy-player command runs one frame after the
    /// network's execution frame.
    pub fn send_player_destruct<H: ConnectionManagerHost>(&mut self, slot: i32, conn: &mut ConnectionManager<H>) {
        let mut current_id: u16 = 0;
        if does_command_require_a_command_id(NetCommandType::DestroyPlayer) {
            current_id = generate_next_command_id();
        }
        let mut m = NetCommandMsg::new_destroy_player();
        m.set_execution_frame(conn.host.network_execution_frame().wrapping_add(1));
        m.set_player_id(conn.get_local_player_id());
        m.set_id(current_id);
        m.set_player_index(slot as u32);
        let msg = NetCommandMsgPtr::new(m);
        conn.send_local_command(&msg, 0xff);
        let _ = msg.write().detach();
    }

    /// `isPlayerVotedOut` (DisconnectManager.cpp:777): votes against the slot reach `numPlayers - 1`.
    pub fn is_player_voted_out<H: ConnectionManagerHost>(&self, slot: i32, conn: &mut ConnectionManager<H>) -> bool {
        if slot == -1 {
            return false;
        }
        let trans_slot = self.untranslated_slot_position(slot, conn.get_local_player_id() as i32);
        let num_votes = self.count_votes_for_player(trans_slot, conn.host.game_frame());
        if plant(15098) { num_votes > conn.get_num_players() - 1 } else { num_votes >= conn.get_num_players() - 1 }
    }

    /// `getMaxDisconnectFrame` (DisconnectManager.cpp:791).
    pub fn get_max_disconnect_frame(&self) -> u32 {
        let mut retval = 0;
        for i in 0..if plant(15096) { SLOTS - 1 } else { SLOTS } {
            if self.disconnect_frames[i] > retval {
                retval = self.disconnect_frames[i];
            }
        }
        retval
    }

    /// `isPlayerInGame` (DisconnectManager.cpp:803): connected, not voted out and not timed out.
    pub fn is_player_in_game<H: ConnectionManagerHost>(&self, slot: i32, conn: &mut ConnectionManager<H>) -> bool {
        let trans_slot = self.untranslated_slot_position(slot, conn.get_local_player_id() as i32);
        if trans_slot < 0 || trans_slot >= MAX_SLOTS || !conn.is_player_connected(trans_slot) {
            return false;
        }
        if !plant(15099) && self.is_player_voted_out(slot, conn) {
            return false;
        }
        if self.has_player_timed_out(slot, conn) {
            return false;
        }
        true
    }

    /// `playerHasAdvancedAFrame` (DisconnectManager.cpp:820).
    pub fn player_has_advanced_a_frame(&mut self, slot: i32, frame: u32) {
        if slot < 0 || slot >= MAX_SLOTS {
            return;
        }
        if frame >= self.disconnect_frames[slot as usize] {
            // in case we get a disconnect frame command after this is called
            if !plant(15095) {
                self.disconnect_frames[slot as usize] = frame;
            }
            self.disconnect_frames_received[slot as usize] = false;
        }
    }

    /// `countVotesForPlayer` (DisconnectManager.cpp:830): votes for the slot cast on the current frame
    /// (`game_frame` is `TheGameLogic->getFrame()`).
    pub fn count_votes_for_player(&self, slot: i32, game_frame: u32) -> i32 {
        if slot < 0 || slot >= MAX_SLOTS {
            return 0;
        }
        let mut retval = 0;
        for i in 0..SLOTS {
            let v = &self.player_votes[slot as usize][i];
            if v.vote && (if plant(15094) { v.frame <= game_frame } else { v.frame == game_frame }) {
                retval += 1;
            }
        }
        retval
    }

    /// `resetPlayersVotes` (DisconnectManager.cpp:845): clears the votes this player cast at or before `frame`.
    pub fn reset_players_votes<H: ConnectionManagerHost>(
        &mut self,
        player_id: i32,
        frame: u32,
        conn: &mut ConnectionManager<H>,
    ) {
        if player_id < 0 || player_id >= MAX_SLOTS {
            return;
        }
        for i in 0..SLOTS {
            if if plant(15101) { self.player_votes[i][player_id as usize].frame < frame } else { self.player_votes[i][player_id as usize].frame <= frame } {
                self.player_votes[i][player_id as usize].vote = false;
            }
        }
        let num_votes = self.count_votes_for_player(player_id, conn.host.game_frame());
        let trans_slot = self.translated_slot_position(player_id, conn.get_local_player_id() as i32);
        if trans_slot != -1 {
            conn.host.disconnect_menu_update_votes(trans_slot, num_votes);
        }
    }
}
