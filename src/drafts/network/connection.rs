//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 6,000 operation sequences (test: tests/differential_draft_conn.rs);
//! not yet wired into the game.
//!
//! The test found one defect, fixed (2026-11-19, #215): `clearCommandsExceptFrom` read `tmp->getNext()` AFTER
//! `removeMessage(tmp)`, which clears the ref's next pointer, so upstream's loop ends after the first command it removes. The draft
//! had captured the next pointer first and removed every command not from the player; it now keeps upstream's (defective) behaviour.
//! Inputs left out of the test (upstream dereferences null): `doSend` with no user or no transport, and `processAck`,
//! `clearCommandsExceptFrom`, `isQueueEmpty`, `doSend` before `init`. Not covered: `debug_print_commands` (not translated).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/Connection.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/Connection.cpp
//!
//! A `Connection` holds the outgoing commands for one player (the incoming ones are sorted by frame in the
//! `FrameDataManager`). `timeGetTime()` is the caller's `now` (milliseconds); `time_t` is `i64`.
//!
//! `processAck`'s latency uses `timeGetTime() - temp->getTimeLastSent()`, an unsigned 32-bit subtraction upstream
//! (a `DWORD` minus a 32-bit `time_t`): reproduced with wrapping `u32` arithmetic, so a command never sent
//! (`m_timeLastSent == -1`) contributes `now + 1`.

use crate::logic::modules::plant;

use super::net_command_list::NetCommandList;
use super::net_command_msg::{NetCommandMsg, NetCommandMsgPtr, NetCommandType};
use super::net_command_ref::{NetCommandRef, NetCommandRefId};
use super::net_packet::NetPacket;
use super::network_util::command_requires_ack;
use super::transport::TransportHandle;
use super::user::User;

/// `CONNECTION_LATENCY_HISTORY_LENGTH`.
pub const CONNECTION_LATENCY_HISTORY_LENGTH: usize = 200;

/// `MaxQuitFlushTime`: wait this many milliseconds at most to retry things before quitting.
pub const MAX_QUIT_FLUSH_TIME: i64 = 30000;

/// The members [`Connection::debug_state`] returns (differential tests only).
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ConnectionDebugState {
    pub quit_time: u32,
    pub retry_time: i64,
    pub average_latency: f32,
    pub latencies: [f32; CONNECTION_LATENCY_HISTORY_LENGTH],
    pub frame_grouping: i64,
    pub last_time_sent: i64,
    pub num_retries: i32,
    pub retry_metrics_time: i64,
}

/// `class Connection`.
#[derive(Debug)]
pub struct Connection {
    /// `m_isQuitting`.
    is_quitting: bool,
    /// `m_quitTime`.
    quit_time: u32,
    /// `m_transport`.
    transport: Option<TransportHandle>,
    /// `m_user`.
    user: Option<User>,
    /// `m_netCommandList` (null until `init`).
    net_command_list: Option<NetCommandList>,
    /// `m_retryTime`: milliseconds between retry packets.
    retry_time: i64,
    /// `m_averageLatency`.
    average_latency: f32,
    /// `m_latencies`.
    latencies: [f32; CONNECTION_LATENCY_HISTORY_LENGTH],
    /// `m_frameGrouping`: the minimum time between packet sends.
    frame_grouping: i64,
    /// `m_lastTimeSent`.
    last_time_sent: i64,
    /// `m_numRetries`.
    num_retries: i32,
    /// `m_retryMetricsTime`.
    retry_metrics_time: i64,
}

impl Default for Connection {
    fn default() -> Self {
        Connection::new()
    }
}

impl Connection {
    /// `Connection::Connection()` (Connection.cpp:32).
    pub fn new() -> Connection {
        Connection {
            transport: None,
            user: None,
            net_command_list: None,
            retry_time: 2000, // set retry time to 2 seconds
            last_time_sent: 0,
            frame_grouping: 1,
            is_quitting: false,
            quit_time: 0,
            average_latency: 0.0,
            latencies: [0.0; CONNECTION_LATENCY_HISTORY_LENGTH],
            num_retries: 0,
            retry_metrics_time: 0,
        }
    }

    /// `Connection::init` (Connection.cpp:62): back to the initial state with an empty command list.
    pub fn init(&mut self) {
        self.transport = None;
        self.user = None;
        if self.net_command_list.is_none() {
            let mut l = NetCommandList::new();
            l.init();
            self.net_command_list = Some(l);
        }
        if let Some(l) = self.net_command_list.as_mut() {
            l.reset();
        }
        self.last_time_sent = 0;
        self.frame_grouping = 1;
        self.num_retries = 0;
        self.retry_metrics_time = 0;
        for l in self.latencies.iter_mut() {
            *l = 0.0;
        }
        self.average_latency = 0.0;
        self.is_quitting = false;
        self.quit_time = 0;
    }

    /// `reset`: `init()`.
    pub fn reset(&mut self) {
        self.init();
    }

    /// `update`: doesn't really do anything.
    pub fn update(&mut self) {}

    /// `attachTransport`.
    pub fn attach_transport(&mut self, transport: TransportHandle) {
        self.transport = Some(transport);
    }

    /// `setUser`: the user we send all our packetized goodies to (the old one is deleted).
    pub fn set_user(&mut self, user: Option<User>) {
        self.user = user;
    }

    /// `getUser`.
    pub fn get_user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// `getUser`, mutable.
    pub fn get_user_mut(&mut self) -> Option<&mut User> {
        self.user.as_mut()
    }

    /// `setFrameGrouping`.
    pub fn set_frame_grouping(&mut self, frame_grouping: i64) {
        self.frame_grouping = frame_grouping;
    }

    /// `isQuitting`.
    pub fn is_quitting(&self) -> bool {
        self.is_quitting
    }

    /// `Connection::setQuitting` (Connection.cpp:157).
    pub fn set_quitting(&mut self, now: u32) {
        self.is_quitting = true;
        if !plant(15070) {
            self.quit_time = now;
        }
    }

    /// `Connection::isQueueEmpty` (Connection.cpp:150).
    pub fn is_queue_empty(&self) -> bool {
        self.net_command_list.as_ref().map_or(true, |l| l.get_first_message().is_none())
    }

    /// The command list (for the connection manager and the debug printer).
    pub fn net_command_list(&self) -> Option<&NetCommandList> {
        self.net_command_list.as_ref()
    }

    /// `Connection::sendNetCommandMsg` (Connection.cpp:96): queues the command for this connection with the
    /// given relay. A command that does not fit in a single packet is wrapped: the wrapper commands are queued
    /// instead. Nothing is queued once the connection is quitting.
    pub fn send_net_command_msg(&mut self, msg: &NetCommandMsgPtr, relay: u8) {
        if self.is_quitting {
            return;
        }
        if self.net_command_list.is_none() {
            return;
        }
        let mut packet = NetPacket::new();
        packet.reset();
        let msg_fits = {
            let tempref = NetCommandRef::new(msg.clone());
            packet.add_command(&tempref)
            // the temporary reference is deleted here
        };
        if !msg_fits {
            let mut origref = NetCommandRef::new(msg.clone());
            origref.set_relay(relay);
            let packet_list = NetPacket::construct_big_command_packet_list(&origref);
            for temp_packet in packet_list.iter() {
                let list = temp_packet.get_command_list();
                let mut ref1 = list.get_first_message();
                while let Some(id1) = ref1 {
                    let cmd = list.get(id1).map(|r| r.get_command().clone());
                    if let Some(cmd) = cmd {
                        if let Some(l) = self.net_command_list.as_mut() {
                            let ref2 = l.add_message(cmd);
                            if let Some(id2) = ref2 {
                                if let Some(r2) = l.get_mut(id2) {
                                    if !plant(15067) {
                                        r2.set_relay(relay);
                                    }
                                }
                            }
                        }
                    }
                    ref1 = list.get(id1).and_then(|r| r.get_next());
                }
            }
            return;
        }
        if let Some(l) = self.net_command_list.as_mut() {
            let r = l.add_message(msg.clone());
            if let Some(id) = r {
                if let Some(rr) = l.get_mut(id) {
                    rr.set_relay(relay);
                }
            }
        }
    }

    /// `Connection::clearCommandsExceptFrom` (Connection.cpp:136): removes every queued command whose player is
    /// not `playerIndex`.
    pub fn clear_commands_except_from(&mut self, player_index: i32) {
        let l = match self.net_command_list.as_mut() {
            Some(l) => l,
            None => return,
        };
        let mut tmp = l.get_first_message();
        while let Some(id) = tmp {
            let (pid, next) = match l.get(id) {
                Some(r) => (r.get_command().read().get_player_id(), r.get_next()),
                None => break,
            };
            if if plant(15066) { pid == player_index as u32 } else { pid != player_index as u32 } {
                // DEBUG_LOG("Connection::clearCommandsExceptFrom(%d) - clearing a command from %d for frame %d")
                // Upstream (defect, kept): `removeMessage(tmp)` clears `tmp`'s next pointer, and `tmp = tmp->getNext()` is read
                // AFTER it, so the loop ends after the first command removed; the commands behind it stay queued.
                let removed = l.remove_message(id);
                tmp = removed.as_ref().and_then(|r| r.get_next());
                drop(removed); // toDelete->deleteInstance()
            } else {
                tmp = next;
            }
        }
    }

    /// `Connection::doSend` (Connection.cpp:180): packetizes the queued commands and puts the packets on the
    /// transport's send queue. A quitting connection that has waited `MaxQuitFlushTime` drops everything; sends
    /// are at least `m_frameGrouping` apart; a command is (re)sent when never sent or `m_retryTime` old. A
    /// command that needs no ack leaves the queue once added; one that does stays until acked. Returns the number
    /// of packets made.
    pub fn do_send(&mut self, now: u32) -> u32 {
        let mut numpackets: u32 = 0;
        let curtime = now as i64;
        let mut could_queue = true;

        if self.is_quitting && (if plant(15064) { curtime >= self.quit_time as i64 + MAX_QUIT_FLUSH_TIME } else { curtime > self.quit_time as i64 + MAX_QUIT_FLUSH_TIME }) {
            // DEBUG_LOG("Timed out a quitting connection.  Deleting all %d messages")
            if let Some(l) = self.net_command_list.as_mut() {
                l.reset();
            }
            return 0;
        }

        if (if plant(15063) { (curtime - self.last_time_sent) <= self.frame_grouping } else { (curtime - self.last_time_sent) < self.frame_grouping }) {
            return 0;
        }

        let mut msg: Option<NetCommandRefId> = match self.net_command_list.as_ref() {
            Some(l) => l.get_first_message(),
            None => return 0,
        };
        while msg.is_some() && could_queue {
            let mut packet = NetPacket::new();
            packet.init();
            if let Some(u) = self.user.as_ref() {
                packet.set_address(u.get_ip_addr() as i32, u.get_port() as i32);
            }
            let mut not_done = true;
            while let (Some(id), true) = (msg, not_done) {
                let (next, time_last_sent) = match self.net_command_list.as_ref().and_then(|l| l.get(id)) {
                    Some(r) => (r.get_next(), r.get_time_last_sent()),
                    None => break,
                };
                if (if plant(15061) { (curtime - time_last_sent) >= self.retry_time } else { (curtime - time_last_sent) > self.retry_time }) || time_last_sent == -1 {
                    not_done = {
                        let l = self.net_command_list.as_ref();
                        match l.and_then(|l| l.get(id)) {
                            Some(r) => packet.add_command(r),
                            None => false,
                        }
                    };
                    if not_done {
                        let needs_ack = self
                            .net_command_list
                            .as_ref()
                            .and_then(|l| l.get(id))
                            .map_or(false, |r| command_requires_ack(&r.get_command().read()));
                        if needs_ack {
                            if time_last_sent != -1 || plant(15062) {
                                self.num_retries += 1;
                            }
                            self.do_retry_metrics(now);
                            if let Some(r) = self.net_command_list.as_mut().and_then(|l| l.get_mut(id)) {
                                r.set_time_last_sent(curtime);
                            }
                        } else if let Some(l) = self.net_command_list.as_mut() {
                            let removed = l.remove_message(id);
                            drop(removed); // msg->deleteInstance()
                        }
                    }
                }
                msg = next;
            }
            // if (msg != NULL) DEBUG_LOG("didn't finish sending all commands in connection")
            numpackets += 1;
            if packet.get_num_commands() > 0 || plant(15074) {
                could_queue = plant(15072) || match self.transport.as_ref() {
                    Some(t) => {
                        let mut tr = t.write().unwrap_or_else(|e| e.into_inner());
                        tr.queue_send(packet.get_addr(), packet.get_port(), packet.get_data(), packet.get_length())
                    }
                    None => false,
                };
                self.last_time_sent = curtime;
            }
            // the packet is deleted here
        }
        numpackets
    }

    /// `Connection::processAck(NetAckStage1CommandMsg *)` and `(NetAckBothCommandMsg *)` (Connection.cpp): the
    /// ack's command id and original player.
    pub fn process_ack_msg(&mut self, msg: &NetCommandMsg, now: u32) -> Option<NetCommandRef> {
        self.process_ack(msg.get_command_id(), msg.get_original_player_id(), now)
    }

    /// `Connection::processAck(NetCommandMsg *)` (Connection.cpp): only stage-1 and both acks are processed.
    pub fn process_ack_command(&mut self, msg: &NetCommandMsg, now: u32) -> Option<NetCommandRef> {
        match msg.get_net_command_type() {
            NetCommandType::AckStage1 | NetCommandType::AckBoth => self.process_ack_msg(msg, now),
            NetCommandType::AckStage2 if plant(15071) => self.process_ack_msg(msg, now),
            _ => None,
        }
    }

    /// `Connection::processAck(UnsignedShort commandID, UnsignedByte originalPlayerID)` (Connection.cpp): the
    /// recipient acked one of our commands: finds it by id and player (of any command type), folds its round
    /// trip into the latency average and takes it off the queue. Returns the removed ref (the caller deletes it).
    pub fn process_ack(&mut self, command_id: u16, original_player_id: u8, now: u32) -> Option<NetCommandRef> {
        let l = self.net_command_list.as_mut()?;
        let mut temp = l.get_first_message();
        while let Some(id) = temp {
            let r = l.get(id)?;
            let m = r.get_command().read();
            if m.get_id() == command_id && (m.get_player_id() == original_player_id as u32 || plant(15069)) {
                break;
            }
            temp = r.get_next();
        }
        let id = temp?;
        let (cid, last_sent) = {
            let r = l.get(id)?;
            (r.get_command().read().get_id(), r.get_time_last_sent())
        };
        let index = cid as usize % if plant(15065) { CONNECTION_LATENCY_HISTORY_LENGTH - 1 } else { CONNECTION_LATENCY_HISTORY_LENGTH };
        let lat = now.wrapping_sub(last_sent as u32) as f32;
        if plant(15073) {
            self.average_latency += lat / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
            self.average_latency -= self.latencies[index] / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
        } else {
            self.average_latency -= self.latencies[index] / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
            self.average_latency += lat / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
        }
        self.latencies[index] = lat;
        l.remove_message(id)
    }

    /// `Connection::doRetryMetrics` (Connection.cpp): every 10 seconds restarts the retry count. (The static
    /// `numSeconds` upstream is only incremented.)
    fn do_retry_metrics(&mut self, now: u32) {
        let cur_time = now as i64;
        if (if plant(15068) { (cur_time - self.retry_metrics_time) >= 10000 } else { (cur_time - self.retry_metrics_time) > 10000 }) {
            self.retry_metrics_time = cur_time;
            self.num_retries = 0;
        }
    }

    /// Differential-test accessor (#215): the private members the C++ test reads through `#define private public`.
    #[doc(hidden)]
    pub fn debug_state(&self) -> ConnectionDebugState {
        ConnectionDebugState {
            quit_time: self.quit_time,
            retry_time: self.retry_time,
            average_latency: self.average_latency,
            latencies: self.latencies,
            frame_grouping: self.frame_grouping,
            last_time_sent: self.last_time_sent,
            num_retries: self.num_retries,
            retry_metrics_time: self.retry_metrics_time,
        }
    }

    /// `Connection::debugPrintCommands` (debug builds): only `DEBUG_LOG` output; not translated.
    pub fn debug_print_commands(&self) {}
}
