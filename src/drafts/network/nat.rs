//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NAT.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NAT.cpp
//!
//! NAT traversal for GameSpy staging rooms: every pair of human players runs a "connection round" in which one
//! side learns the other's externally visible (mangled) port from EA's mangler servers and the two probe each
//! other until packets get through; the round table [`CONNECTION_PAIRS`] says who pairs with whom in each round.
//!
//! # Not reachable today
//!
//! The mangler servers and the GameSpy peer service this talks to no longer exist, so the traversal cannot
//! succeed against them. The state machine, the pairing table, the port arithmetic and the "PROBE"/"KEEPALIVE"
//! packet handling are translated; every external service is the [`NatHost`]: the staging-room slots
//! (`GameSlot **`), `TheFirewallHelper` (the mangler client and spare sockets; `FirewallHelper.cpp` is not
//! drafted yet), `TheGameSpyPeerMessageQueue` (the `UTM` player messages), `TheEstablishConnectionsMenu` and the
//! player-stats wait.
//!
//! `m_timeTillNextSend` is never initialised upstream unless a round starts with the Netgear-bug rule; it is 0
//! here (a probe is allowed at once).

use super::net_command_msg::{MAX_MESSAGES, MAX_SLOTS, NETWORK_BASE_PORT_NUMBER};
use super::transport::Transport;

const SLOTS: usize = MAX_SLOTS as usize;

/// `FirewallHelperClass::tFirewallBehaviorType` bits.
pub mod firewall_type {
    /// `FIREWALL_TYPE_UNKNOWN`.
    pub const UNKNOWN: u32 = 0;
    /// `FIREWALL_TYPE_SIMPLE`.
    pub const SIMPLE: u32 = 1;
    /// `FIREWALL_TYPE_DUMB_MANGLING`.
    pub const DUMB_MANGLING: u32 = 2;
    /// `FIREWALL_TYPE_SMART_MANGLING`.
    pub const SMART_MANGLING: u32 = 4;
    /// `FIREWALL_TYPE_NETGEAR_BUG`.
    pub const NETGEAR_BUG: u32 = 8;
    /// `FIREWALL_TYPE_SIMPLE_PORT_ALLOCATION`.
    pub const SIMPLE_PORT_ALLOCATION: u32 = 16;
    /// `FIREWALL_TYPE_RELATIVE_PORT_ALLOCATION`.
    pub const RELATIVE_PORT_ALLOCATION: u32 = 32;
    /// `FIREWALL_TYPE_DESTINATION_PORT_DELTA`.
    pub const DESTINATION_PORT_DELTA: u32 = 64;
}

/// `enum NATStateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NatStateType {
    /// `NATSTATE_IDLE`.
    Idle,
    /// `NATSTATE_DOCONNECTIONPATHS`.
    DoConnectionPaths,
    /// `NATSTATE_WAITFORSTATS`.
    WaitForStats,
    /// `NATSTATE_DONE`.
    Done,
    /// `NATSTATE_FAILED`.
    Failed,
}

/// `enum NATConnectionState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NatConnectionState {
    /// `NATCONNECTIONSTATE_NOSTATE`.
    NoState,
    /// `NATCONNECTIONSTATE_WAITINGTOBEGIN`.
    WaitingToBegin,
    /// `NATCONNECTIONSTATE_WAITINGFORMANGLERRESPONSE`.
    WaitingForManglerResponse,
    /// `NATCONNECTIONSTATE_WAITINGFORMANGLEDPORT`.
    WaitingForMangledPort,
    /// `NATCONNECTIONSTATE_WAITINGFORRESPONSE`.
    WaitingForResponse,
    /// `NATCONNECTIONSTATE_DONE`.
    Done,
    /// `NATCONNECTIONSTATE_FAILED`.
    Failed,
}

/// `struct ConnectionNodeType`.
#[derive(Clone, Copy, Debug)]
pub struct ConnectionNodeType {
    /// `m_behavior`: the NAT/firewall behavior of this node.
    pub behavior: u32,
    /// `m_slotIndex`: the player list index of this node (-1 unassigned).
    pub slot_index: i32,
}

/// `NAT::m_connectionPairs[MAX_SLOTS-1][MAX_SLOTS-1][MAX_SLOTS]`: indexed by (node count - 2), round, node: the
/// node it pairs with in that round (-1 for none).
pub const CONNECTION_PAIRS: [[[i32; SLOTS]; SLOTS - 1]; SLOTS - 1] = [
    [
        // 2 nodes
        [1, 0, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
    ],
    [
        // 3 nodes
        [1, 0, -1, -1, -1, -1, -1, -1],
        [2, -1, 0, -1, -1, -1, -1, -1],
        [-1, 2, 1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
    ],
    [
        // 4 nodes
        [1, 0, 3, 2, -1, -1, -1, -1],
        [2, 3, 0, 1, -1, -1, -1, -1],
        [3, 2, 1, 0, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
    ],
    [
        // 5 nodes
        [2, 4, 0, -1, 1, -1, -1, -1],
        [-1, 3, 4, 1, 2, -1, -1, -1],
        [3, 2, 1, 0, -1, -1, -1, -1],
        [4, -1, 3, 2, 0, -1, -1, -1],
        [1, 0, -1, 4, 3, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
    ],
    [
        // 6 nodes
        [3, 5, 4, 0, 2, 1, -1, -1],
        [2, 4, 0, 5, 1, 3, -1, -1],
        [4, 3, 5, 1, 0, 2, -1, -1],
        [1, 0, 3, 2, 5, 4, -1, -1],
        [5, 2, 1, 4, 3, 0, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
        [-1, -1, -1, -1, -1, -1, -1, -1],
    ],
    [
        // 7 nodes
        [-1, 6, 5, 4, 3, 2, 1, -1],
        [2, -1, 0, 6, 5, 4, 3, -1],
        [4, 3, -1, 1, 0, 6, 5, -1],
        [6, 5, 4, -1, 2, 1, 0, -1],
        [1, 0, 6, 5, -1, 3, 2, -1],
        [3, 2, 1, 0, 6, -1, 4, -1],
        [5, 4, 3, 2, 1, 0, -1, -1],
    ],
    [
        // 8 nodes
        [4, 5, 6, 7, 0, 1, 2, 3],
        [5, 4, 7, 6, 1, 0, 3, 2],
        [3, 6, 5, 0, 7, 2, 1, 4],
        [2, 7, 0, 5, 6, 3, 4, 1],
        [6, 3, 4, 1, 2, 7, 0, 5],
        [1, 0, 3, 2, 5, 4, 7, 6],
        [7, 2, 1, 4, 3, 6, 5, 0],
    ],
];

/// `NAT::m_timeBetweenRetries`: half a second between retries.
pub const TIME_BETWEEN_RETRIES: i64 = 500;
/// `NAT::m_manglerRetryTimeInterval`.
pub const MANGLER_RETRY_TIME_INTERVAL: i64 = 300;
/// `NAT::m_maxAllowedManglerRetries`.
pub const MAX_ALLOWED_MANGLER_RETRIES: i32 = 25;
/// `NAT::m_keepaliveInterval`: 15 seconds between keepalive packets.
pub const KEEPALIVE_INTERVAL: i64 = 15000;
/// `NAT::m_timeToWaitForPort`.
pub const TIME_TO_WAIT_FOR_PORT: i64 = 15000;
/// `NAT::m_timeForRoundTimeout`.
pub const TIME_FOR_ROUND_TIMEOUT: i64 = 15000;
/// `MS_TO_WAIT_FOR_STATS`.
pub const MS_TO_WAIT_FOR_STATS: u32 = 5000;

/// What the NAT reaches outside itself (see the module doc).
pub trait NatHost {
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    /// `m_slotList[i] != NULL`.
    fn slot_exists(&self, i: usize) -> bool;
    /// `m_slotList[i]->isHuman()`.
    fn slot_is_human(&self, i: usize) -> bool;
    /// `m_slotList[i]->getIP()`.
    fn slot_ip(&self, i: usize) -> u32;
    /// `m_slotList[i]->setIP(ip)`.
    fn slot_set_ip(&mut self, i: usize, ip: u32);
    /// `m_slotList[i]->getPort()`.
    fn slot_port(&self, i: usize) -> u16;
    /// `m_slotList[i]->setPort(port)`.
    fn slot_set_port(&mut self, i: usize, port: u16);
    /// `m_slotList[i]->getNATBehavior()`.
    fn slot_nat_behavior(&self, i: usize) -> u32;
    /// `m_slotList[i]->getName()`, as an ASCII nickname (`hostName.translate(name)`).
    fn slot_name_ascii(&self, i: usize) -> Vec<u8>;
    /// `TheGameSpyGame->getLocalSlotNum()`.
    fn local_slot_num(&self) -> i32;
    /// `TheWritableGlobalData->m_firewallPortOverride`.
    fn firewall_port_override(&self) -> u16;
    /// `TheGlobalData->m_firewallPortAllocationDelta`.
    fn firewall_port_allocation_delta(&self) -> i16;
    /// `TheFirewallHelper != NULL`.
    fn has_firewall_helper(&self) -> bool;
    /// `TheFirewallHelper = createFirewallHelper()`.
    fn create_firewall_helper(&mut self);
    /// `delete TheFirewallHelper; TheFirewallHelper = NULL`.
    fn delete_firewall_helper(&mut self);
    /// `TheFirewallHelper->flagNeedToRefresh(flag)`.
    fn firewall_flag_need_to_refresh(&mut self, flag: bool);
    /// `TheFirewallHelper->getManglerResponse(packetID)`: the mangled port or 0.
    fn firewall_get_mangler_response(&mut self, packet_id: u16) -> u16;
    /// `TheFirewallHelper->closeSpareSocket(port)`.
    fn firewall_close_spare_socket(&mut self, port: u16);
    /// `TheFirewallHelper->sendToManglerFromPort(address, port, packetID)`.
    fn firewall_send_to_mangler_from_port(&mut self, address: u32, port: u16, packet_id: u16);
    /// `TheFirewallHelper->getNextTemporarySourcePort(0)`.
    fn firewall_next_temporary_source_port(&mut self) -> u16;
    /// `TheFirewallHelper->openSpareSocket(port)`.
    fn firewall_open_spare_socket(&mut self, port: u16);
    /// `FirewallHelperClass::getManglerName(1, name)` then `gethostbyname`: the first mangler's IPv4 address in
    /// host byte order; None if it does not resolve.
    fn resolve_first_mangler(&mut self) -> Option<u32>;
    /// `TheGameSpyPeerMessageQueue->addRequest(PEERREQUEST_UTMPLAYER)`: `id` ("NAT/" or "NAT"), the comma
    /// separated nicks and the options text; always to a staging room.
    fn peer_utm_player(&mut self, id: &str, nick: &[u8], options: &[u8]);
    /// `TheEstablishConnectionsMenu->initMenu()` (creating it if need be).
    fn menu_init(&mut self);
    /// `TheEstablishConnectionsMenu->endMenu()`.
    fn menu_end(&mut self);
    /// `TheEstablishConnectionsMenu->setPlayerName(i, name)`.
    fn menu_set_player_name(&mut self, i: i32, name: &[u8]);
    /// `TheEstablishConnectionsMenu->setPlayerStatus(i, state)`.
    fn menu_set_player_status(&mut self, i: i32, state: NatConnectionState);
    /// Whether the player stats of every human slot of `TheGameSpyGame` have arrived
    /// (`TheGameSpyPSMessageQueue->findPlayerStatsByID(profileID).id != 0`).
    fn all_player_stats_arrived(&mut self) -> bool;
}

/// `class NAT`.
pub struct NAT<H: NatHost> {
    /// The services.
    pub host: H,
    /// `m_transport`.
    pub transport: Option<Transport>,
    /// `m_NATState`.
    nat_state: NatStateType,
    /// `m_localNodeNumber`.
    local_node_number: i32,
    /// `m_targetNodeNumber`.
    target_node_number: i32,
    /// `m_localIP`.
    local_ip: u32,
    /// `m_numNodes`.
    num_nodes: u32,
    /// `m_connectionRound`.
    connection_round: u32,
    /// `m_numRetries`.
    num_retries: i32,
    /// `m_maxNumRetriesAllowed`.
    max_num_retries_allowed: i32,
    /// `m_packetID`.
    packet_id: u16,
    /// `m_spareSocketPort`.
    spare_socket_port: u16,
    /// `m_manglerRetryTime`.
    mangler_retry_time: i64,
    /// `m_manglerRetries`.
    mangler_retries: i32,
    /// `m_previousSourcePort`.
    previous_source_port: u16,
    /// `m_beenProbed`.
    been_probed: bool,
    /// `m_manglerAddress`.
    mangler_address: u32,
    /// `m_timeTillNextSend`.
    time_till_next_send: i64,
    /// `m_connectionStates`.
    connection_states: [NatConnectionState; SLOTS],
    /// `m_connectionNodes`.
    connection_nodes: [ConnectionNodeType; SLOTS],
    /// `m_sourcePorts`.
    source_ports: [u16; SLOTS],
    /// `m_myConnections`.
    my_connections: [bool; SLOTS],
    /// `m_nextKeepaliveTime`.
    next_keepalive_time: i64,
    /// `m_connectionPairIndex`.
    connection_pair_index: i32,
    /// `m_startingPortNumber`.
    starting_port_number: u16,
    /// `m_nextPortSendTime`.
    next_port_send_time: i64,
    /// `m_timeoutTime`.
    timeout_time: i64,
    /// `m_roundTimeout`.
    round_timeout: i64,
    /// `static UnsignedInt s_startStatWaitTime` of `update`.
    start_stat_wait_time: u32,
}

impl<H: NatHost> NAT<H> {
    /// `NAT::NAT()` (NAT.cpp).
    pub fn new(host: H) -> NAT<H> {
        NAT {
            host,
            transport: None,
            nat_state: NatStateType::Idle,
            local_node_number: 0,
            target_node_number: 0,
            local_ip: 0,
            num_nodes: 0,
            connection_round: 0,
            num_retries: 0,
            max_num_retries_allowed: 10,
            packet_id: 0x7f00,
            spare_socket_port: 0,
            mangler_retry_time: 0,
            mangler_retries: 0,
            previous_source_port: 0,
            been_probed: false,
            mangler_address: 0,
            time_till_next_send: 0,
            connection_states: [NatConnectionState::NoState; SLOTS],
            connection_nodes: [ConnectionNodeType { behavior: 0, slot_index: -1 }; SLOTS],
            source_ports: [0; SLOTS],
            my_connections: [false; SLOTS],
            next_keepalive_time: 0,
            connection_pair_index: 0,
            starting_port_number: 0,
            next_port_send_time: 0,
            timeout_time: 0,
            round_timeout: 0,
            start_stat_wait_time: 0,
        }
    }

    fn now(&self) -> i64 {
        self.host.time_get_time() as i64
    }

    fn slot_of_node(&self, node: i32) -> i32 {
        self.connection_nodes[node as usize].slot_index
    }

    /// `NAT::update` (NAT.cpp): a finished NAT goes back to idle; while waiting for stats, done once all arrived
    /// or after `MS_TO_WAIT_FOR_STATS`; while doing connection paths, advances rounds, runs the connection update,
    /// and fails the whole traversal if the local connection fails.
    pub fn update(&mut self) -> NatStateType {
        if self.nat_state == NatStateType::Done {
            self.nat_state = NatStateType::Idle;
        } else if self.nat_state == NatStateType::WaitForStats {
            let got_all_stats = self.host.all_player_stats_arrived();
            let now = self.host.time_get_time();
            let timed_out = now > self.start_stat_wait_time.wrapping_add(MS_TO_WAIT_FOR_STATS);
            if got_all_stats || timed_out {
                self.nat_state = NatStateType::Done;
                self.host.menu_end();
                if self.host.has_firewall_helper() {
                    self.host.delete_firewall_helper();
                }
            }
        } else if self.nat_state == NatStateType::DoConnectionPaths {
            if self.all_connections_done_this_round() {
                self.connection_round += 1;
                self.round_timeout = self.now() + TIME_FOR_ROUND_TIMEOUT;
                if self.all_connections_done() {
                    self.nat_state = NatStateType::WaitForStats;
                    self.host.firewall_flag_need_to_refresh(false);
                    self.start_stat_wait_time = self.host.time_get_time();
                } else {
                    self.do_this_connection_round();
                }
            }
            let state = self.connection_update();
            if self.now() > self.round_timeout {
                let l = self.local_node_number;
                self.set_connection_state(l, NatConnectionState::Failed);
                self.notify_users_of_connection_failed(l);
            }
            if state == NatConnectionState::Failed {
                self.nat_state = NatStateType::Failed;
                self.host.menu_end();
                if self.host.has_firewall_helper() {
                    self.host.firewall_flag_need_to_refresh(true);
                    self.host.delete_firewall_helper();
                }
                self.transport = None;
            }
        }
        self.nat_state
    }

    /// `NAT::connectionUpdate` (NAT.cpp): re-sends my port to the target until probed, sends keepalives to the
    /// nodes I connected to, handles received "PROBE"/"KEEPALIVE" packets, retries probes, and drives the mangler
    /// exchange.
    pub fn connection_update(&mut self) -> NatConnectionState {
        if self.target_node_number < 0 {
            return self.connection_states[self.local_node_number as usize];
        }
        let target_slot = self.slot_of_node(self.target_node_number);
        let tsi = target_slot as usize;
        if !self.been_probed && self.now() >= self.next_port_send_time {
            let sp = self.source_ports[self.target_node_number as usize];
            self.send_mangled_port_number_to_target(sp, tsi);
            self.next_port_send_time = self.now() + TIME_BETWEEN_RETRIES;
        }
        if self.now() >= self.next_keepalive_time {
            for node in 0..self.num_nodes as usize {
                if self.my_connections[node] {
                    let slot_index = self.connection_nodes[node].slot_index;
                    if slot_index >= 0 && self.host.slot_exists(slot_index as usize) {
                        let ip = self.host.slot_ip(slot_index as usize);
                        let port = self.host.slot_port(slot_index as usize);
                        if let Some(t) = self.transport.as_mut() {
                            t.queue_send(ip, port, b"KEEPALIVE\0", 10);
                        }
                    }
                }
            }
            self.next_keepalive_time = self.now() + KEEPALIVE_INTERVAL;
        }
        let now = self.host.time_get_time();
        if let Some(t) = self.transport.as_mut() {
            t.update(now);
        }
        for i in 0..MAX_MESSAGES as usize {
            let (len, addr, port, data) = match self.transport.as_ref() {
                Some(t) => (
                    t.in_buffer[i].length,
                    t.in_buffer[i].addr,
                    t.in_buffer[i].port,
                    t.in_buffer[i].data[..(t.in_buffer[i].length.max(0) as usize).min(64)].to_vec(),
                ),
                None => (0, 0, 0, Vec::new()),
            };
            if len > 0 {
                if data.starts_with(b"PROBE") {
                    let from_node = atoi(&data[5..]);
                    if from_node == self.target_node_number {
                        let t = self.target_node_number;
                        self.set_connection_state(t, NatConnectionState::Done);
                        if addr != self.host.slot_ip(tsi) {
                            self.host.slot_set_ip(tsi, addr);
                        }
                        if port != self.host.slot_port(tsi) {
                            self.host.slot_set_port(tsi, port);
                            self.source_ports[t as usize] = port;
                        }
                        self.notify_users_of_connection_done(t);
                    }
                    if let Some(t) = self.transport.as_mut() {
                        t.in_buffer[i].length = 0;
                    }
                }
                if data.starts_with(b"KEEPALIVE") {
                    if let Some(t) = self.transport.as_mut() {
                        t.in_buffer[i].length = 0;
                    }
                }
            }
        }
        let local = self.local_node_number as usize;
        if self.connection_states[local] == NatConnectionState::WaitingForResponse
            && self.time_till_next_send != -1
            && self.time_till_next_send <= self.now()
        {
            if self.num_retries > self.max_num_retries_allowed {
                let l = self.local_node_number;
                self.set_connection_state(l, NatConnectionState::Failed);
                self.notify_users_of_connection_failed(l);
            } else {
                let (ip, port) = (self.host.slot_ip(tsi), self.host.slot_port(tsi));
                let l = self.local_node_number;
                self.send_a_probe(ip, port, l);
                self.time_till_next_send = self.now() + TIME_BETWEEN_RETRIES;
                self.notify_target_of_probe(tsi);
                self.num_retries += 1;
            }
        }
        if self.connection_states[local] == NatConnectionState::WaitingForManglerResponse {
            let mut mangled_port = 0u16;
            if self.host.has_firewall_helper() {
                mangled_port = self.host.firewall_get_mangler_response(self.packet_id);
            }
            if mangled_port != 0 {
                self.process_mangler_response(mangled_port);
                let sp = self.spare_socket_port;
                self.host.firewall_close_spare_socket(sp);
                self.spare_socket_port = 0;
            } else if self.now() >= self.mangler_retry_time {
                self.mangler_retries += 1;
                if self.mangler_retries > MAX_ALLOWED_MANGLER_RETRIES {
                    let sp = self.get_slot_port(self.connection_nodes[local].slot_index);
                    self.send_mangled_port_number_to_target(sp, tsi);
                    self.source_ports[self.target_node_number as usize] = sp;
                    let l = self.local_node_number;
                    self.set_connection_state(l, NatConnectionState::WaitingForResponse);
                } else {
                    if self.host.has_firewall_helper() {
                        let (a, p, id) = (self.mangler_address, self.spare_socket_port, self.packet_id);
                        self.host.firewall_send_to_mangler_from_port(a, p, id);
                    }
                    self.mangler_retry_time = MANGLER_RETRY_TIME_INTERVAL + self.now();
                }
            }
        }
        if self.connection_states[local] == NatConnectionState::WaitingForMangledPort && self.now() > self.timeout_time {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            self.notify_users_of_connection_failed(l);
        }
        self.connection_states[local]
    }

    /// `NAT::establishConnectionPaths` (NAT.cpp): counts the human nodes, assigns them to node numbers (the two
    /// Netgear-bug players are paired in round 0 of the table), finds my node, lists the other players on the
    /// connection screen and starts round 0.
    pub fn establish_connection_paths(&mut self) {
        self.nat_state = NatStateType::DoConnectionPaths;
        self.host.menu_init();
        if !self.host.has_firewall_helper() {
            self.host.create_firewall_helper();
        }
        self.num_nodes = 0;
        for i in 0..SLOTS {
            if self.host.slot_exists(i) && self.host.slot_is_human(i) {
                self.num_nodes += 1;
            }
        }
        if self.num_nodes < 2 {
            self.nat_state = NatStateType::Done;
            return;
        }
        self.connection_round = 0;
        self.connection_pair_index = self.num_nodes as i32 - 2;
        let mut connection_assigned = [false; SLOTS];
        for i in 0..SLOTS {
            self.connection_nodes[i].slot_index = -1;
            connection_assigned[i] = false;
            self.source_ports[i] = 0;
        }
        self.previous_source_port = 0;

        let mut other_netgear_num: i32 = -1;
        for i in 0..SLOTS {
            if self.host.slot_exists(i) && (self.host.slot_nat_behavior(i) & firewall_type::NETGEAR_BUG) != 0 {
                let pair = &CONNECTION_PAIRS[self.connection_pair_index as usize][0];
                if other_netgear_num == -1 {
                    let mut nodeindex = 0usize;
                    while nodeindex < SLOTS - 1
                        && (pair[nodeindex] == -1 || self.connection_nodes[nodeindex].slot_index != -1)
                    {
                        nodeindex += 1;
                    }
                    self.connection_nodes[nodeindex].slot_index = i as i32;
                    self.connection_nodes[nodeindex].behavior = self.host.slot_nat_behavior(i);
                    connection_assigned[i] = true;
                    other_netgear_num = nodeindex as i32;
                } else {
                    let mut nodeindex = 0usize;
                    while nodeindex < SLOTS - 1 && pair[nodeindex] != other_netgear_num {
                        nodeindex += 1;
                    }
                    self.connection_nodes[nodeindex].slot_index = i as i32;
                    self.connection_nodes[nodeindex].behavior = self.host.slot_nat_behavior(i);
                    connection_assigned[i] = true;
                    other_netgear_num = -1;
                }
            }
        }
        for i in 0..SLOTS {
            if connection_assigned[i] || !self.host.slot_exists(i) || !self.host.slot_is_human(i) {
                continue;
            }
            let mut nodeindex = 0usize;
            while nodeindex < SLOTS && self.connection_nodes[nodeindex].slot_index != -1 {
                nodeindex += 1;
            }
            if nodeindex >= SLOTS {
                break;
            }
            self.connection_nodes[nodeindex].slot_index = i as i32;
            self.connection_nodes[nodeindex].behavior = self.host.slot_nat_behavior(i);
            connection_assigned[i] = true;
        }
        for i in 0..self.num_nodes as usize {
            if self.connection_nodes[i].slot_index == self.host.local_slot_num() {
                self.local_node_number = i as i32;
                break;
            }
        }
        let mut player_num = 0;
        let mut i = 0usize;
        while i < SLOTS {
            while i < SLOTS && self.host.slot_exists(i) && !self.host.slot_is_human(i) {
                i += 1;
            }
            if i >= SLOTS {
                break;
            }
            if i as i32 != self.host.local_slot_num() {
                let name = self.host.slot_name_ascii(i);
                self.host.menu_set_player_name(player_num, &name);
                self.host.menu_set_player_status(player_num, NatConnectionState::WaitingToBegin);
                player_num += 1;
            }
            i += 1;
        }
        self.round_timeout = self.now() + TIME_FOR_ROUND_TIMEOUT;
        self.do_this_connection_round();
    }

    /// `NAT::attachSlotList` (NAT.cpp): makes the transport, picks the game's starting port (a function of the
    /// clock) and gives every slot its port, then binds the local one.
    pub fn attach_slot_list(&mut self, local_slot: i32, local_ip: u32) {
        self.local_ip = local_ip;
        self.starting_port_number =
            (NETWORK_BASE_PORT_NUMBER as u32 + ((self.host.time_get_time() / 1000) % 20000)) as u16;
        self.generate_port_numbers(local_slot);
        let mut t = Transport::new();
        let port = self.get_slot_port(local_slot);
        let mut tick = self.host.time_get_time();
        let _ = t.init(self.local_ip, port as u16, || {
            tick = tick.wrapping_add(1);
            tick
        });
        self.transport = Some(t);
    }

    /// `NAT::getSlotPort` (NAT.cpp): the slot's port, 0 for no slot.
    pub fn get_slot_port(&self, slot: i32) -> i32 {
        if slot >= 0 && (slot as usize) < SLOTS && self.host.slot_exists(slot as usize) {
            return self.host.slot_port(slot as usize) as i32;
        }
        0
    }

    /// `NAT::generatePortNumbers` (NAT.cpp): slot `i` gets `startingPortNumber + i`, the local slot the
    /// firewall port override if there is one.
    pub fn generate_port_numbers(&mut self, local_slot: i32) {
        for i in 0..SLOTS {
            if self.host.slot_exists(i) {
                if i as i32 == local_slot && self.host.firewall_port_override() != 0 {
                    let p = self.host.firewall_port_override();
                    self.host.slot_set_port(i, p);
                } else {
                    let p = (i as u32 + self.starting_port_number as u32) as u16;
                    self.host.slot_set_port(i, p);
                }
            }
        }
    }

    /// `NAT::getTransport`.
    pub fn get_transport(&mut self) -> Option<&mut Transport> {
        self.transport.as_mut()
    }

    /// `NAT::doThisConnectionRound` (NAT.cpp): the round's pairing: my target gets my (mangled) port; the others
    /// wait or are done.
    pub fn do_this_connection_round(&mut self) {
        self.target_node_number = -1;
        for i in 0..SLOTS {
            self.set_connection_state(i as i32, NatConnectionState::NoState);
        }
        self.been_probed = false;
        self.num_retries = 0;
        for i in 0..self.num_nodes as usize {
            let target_node_number =
                CONNECTION_PAIRS[self.connection_pair_index as usize][self.connection_round as usize][i];
            if target_node_number != -1 {
                if i as i32 == self.local_node_number {
                    self.target_node_number = target_node_number;
                    let target_slot_index = self.connection_nodes[target_node_number as usize].slot_index;
                    let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
                    if target_slot_index >= 0
                        && local_slot_index >= 0
                        && (self.host.slot_nat_behavior(target_slot_index as usize) & firewall_type::NETGEAR_BUG) == 0
                        && (self.host.slot_nat_behavior(local_slot_index as usize) & firewall_type::NETGEAR_BUG) != 0
                    {
                        self.time_till_next_send = -1;
                    }
                    self.send_mangled_source_port();
                    self.next_port_send_time = self.now() + TIME_BETWEEN_RETRIES;
                    self.timeout_time = self.now() + TIME_TO_WAIT_FOR_PORT;
                } else {
                    self.set_connection_state(i as i32, NatConnectionState::WaitingForResponse);
                }
            } else {
                self.set_connection_state(i as i32, NatConnectionState::Done);
            }
        }
    }

    /// `NAT::sendAProbe` (NAT.cpp): "PROBE<node>" (with its 0) to the address and port, sent at once.
    pub fn send_a_probe(&mut self, ip: u32, port: u16, from_node: i32) {
        let mut s = format!("PROBE{}", from_node).into_bytes();
        s.push(0);
        let now = self.host.time_get_time();
        if let Some(t) = self.transport.as_mut() {
            let len = s.len() as i32;
            t.queue_send(ip, port, &s, len);
            t.do_send(now);
        }
    }

    /// `NAT::sendMangledSourcePort` (NAT.cpp): tells the target which port to probe. Same machine: the real
    /// port; a simple firewall: the real port; a firewall with no destination-port delta and no smart mangling
    /// reuses the previous port; otherwise ask a mangler server what port this NAT shows to the outside.
    pub fn send_mangled_source_port(&mut self) {
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        let source_port = self.get_slot_port(local_slot_index) as u16;
        let fw_type = self.host.slot_nat_behavior(local_slot_index.max(0) as usize);
        let target_slot_index = self.connection_nodes[self.target_node_number as usize].slot_index;
        if target_slot_index < 0 || !self.host.slot_exists(target_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        if local_slot_index < 0 || !self.host.slot_exists(local_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        let tsi = target_slot_index as usize;
        let t = self.target_node_number as usize;
        let l = self.local_node_number;
        if self.host.slot_ip(tsi) == self.host.slot_ip(local_slot_index as usize) {
            // same machine: no NAT in between
            self.send_mangled_port_number_to_target(source_port, tsi);
            self.source_ports[t] = source_port;
            self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
            return;
        }
        if fw_type == 0 || fw_type == firewall_type::SIMPLE {
            self.send_mangled_port_number_to_target(source_port, tsi);
            self.previous_source_port = source_port;
            self.source_ports[t] = source_port;
            self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
            return;
        }
        if (fw_type & firewall_type::DESTINATION_PORT_DELTA) == 0 && (fw_type & firewall_type::SMART_MANGLING) == 0 {
            if self.previous_source_port != 0 {
                let p = self.previous_source_port;
                self.send_mangled_port_number_to_target(p, tsi);
                self.source_ports[t] = p;
                self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
                return;
            }
        }
        match self.host.resolve_first_mangler() {
            None => {
                self.send_mangled_port_number_to_target(source_port, tsi);
                self.source_ports[t] = source_port;
                self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
                return;
            }
            Some(addr) => self.mangler_address = addr,
        }
        self.mangler_retry_time = MANGLER_RETRY_TIME_INTERVAL + self.now();
        self.mangler_retries = 0;
        if self.host.has_firewall_helper() {
            self.spare_socket_port = self.host.firewall_next_temporary_source_port();
            let sp = self.spare_socket_port;
            self.host.firewall_open_spare_socket(sp);
            let (a, id) = (self.mangler_address, self.packet_id);
            self.host.firewall_send_to_mangler_from_port(a, sp, id);
            self.mangler_retry_time = MANGLER_RETRY_TIME_INTERVAL + self.now();
        }
        self.set_connection_state(l, NatConnectionState::WaitingForManglerResponse);
    }

    /// `NAT::processManglerResponse` (NAT.cpp): from the port the mangler saw and the NAT's allocation delta
    /// works out the port the target must probe. (Port arithmetic in `UnsignedShort`, wrapping as upstream's.)
    pub fn process_mangler_response(&mut self, mangled_port: u16) {
        let target_slot_index = self.connection_nodes[self.target_node_number as usize].slot_index;
        if target_slot_index < 0 || !self.host.slot_exists(target_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        let tsi = target_slot_index as usize;
        let delta: i16 = self.host.firewall_port_allocation_delta();
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        let source_port = self.get_slot_port(local_slot_index) as u16;
        let fw_type = self.host.slot_nat_behavior(local_slot_index.max(0) as usize);
        let mut return_port: u16;
        if (fw_type & firewall_type::SIMPLE_PORT_ALLOCATION) != 0 {
            return_port = mangled_port.wrapping_add(delta as u16);
        } else if delta == 100 {
            return_port = mangled_port.wrapping_sub(self.spare_socket_port);
            return_port = return_port.wrapping_sub(1700);
            return_port = return_port.wrapping_add(delta as u16);
            return_port = return_port.wrapping_add(source_port);
            return_port = return_port.wrapping_add(1700);
        } else if delta == 0 {
            return_port = source_port;
        } else {
            let d = delta as u16;
            return_port = mangled_port / d;
            return_port = return_port.wrapping_mul(d);
            return_port = return_port.wrapping_add(source_port % d);
            return_port = return_port.wrapping_add(d);
        }
        // (upstream's `returnPort > 65535` test cannot hold for an UnsignedShort; there is nothing to subtract)
        if return_port < 1024 {
            return_port = return_port.wrapping_add(1024);
        }
        self.previous_source_port = return_port;
        self.send_mangled_port_number_to_target(return_port, tsi);
        self.source_ports[self.target_node_number as usize] = return_port;
        let l = self.local_node_number;
        if self.host.slot_port(tsi) == 0 {
            self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
        } else {
            self.set_connection_state(l, NatConnectionState::WaitingForResponse);
        }
    }

    /// `NAT::allConnectionsDone` (NAT.cpp): rounds needed by node count (2:1, 3:3, 4:3, 5:5, 6:5, 7:7, 8:7).
    pub fn all_connections_done(&self) -> bool {
        let needed = match self.num_nodes {
            2 => 1,
            3 => 3,
            4 => 3,
            5 => 5,
            6 => 5,
            7 => 7,
            8 => 7,
            _ => return false,
        };
        self.connection_round >= needed
    }

    /// `NAT::allConnectionsDoneThisRound` (NAT.cpp).
    pub fn all_connections_done_this_round(&self) -> bool {
        for i in 0..self.num_nodes as usize {
            if self.connection_states[i] != NatConnectionState::Done && self.connection_states[i] != NatConnectionState::Failed {
                return false;
            }
        }
        true
    }

    /// `NAT::connectionComplete` (NAT.cpp): empty.
    pub fn connection_complete(&mut self, _slot_index: i32) {}
    /// `NAT::connectionFailed` (NAT.cpp): empty.
    pub fn connection_failed(&mut self, _slot_index: i32) {}

    /// `NAT::probed` (NAT.cpp): the target says it probed me. A Netgear-bug local player then probes back.
    pub fn probed(&mut self, _node_number: i32) {
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        if local_slot_index < 0 || !self.host.slot_exists(local_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        if !self.been_probed {
            self.been_probed = true;
            if (self.host.slot_nat_behavior(local_slot_index as usize) & firewall_type::NETGEAR_BUG) != 0 {
                let target_slot_index = self.connection_nodes[self.target_node_number as usize].slot_index;
                if target_slot_index < 0 || !self.host.slot_exists(target_slot_index as usize) {
                    let l = self.local_node_number;
                    self.set_connection_state(l, NatConnectionState::Failed);
                    return;
                }
                let tsi = target_slot_index as usize;
                let l = self.local_node_number;
                if self.host.slot_port(tsi) == 0 {
                    self.set_connection_state(l, NatConnectionState::WaitingForMangledPort);
                } else {
                    let (ip, port) = (self.host.slot_ip(tsi), self.host.slot_port(tsi));
                    self.send_a_probe(ip, port, l);
                    self.notify_target_of_probe(tsi);
                    self.set_connection_state(l, NatConnectionState::WaitingForResponse);
                }
            }
        }
    }

    /// `NAT::gotMangledPort` (NAT.cpp): the target's port arrived; probe it (a Netgear-bug local waits to be
    /// probed first unless the target has the bug too).
    pub fn got_mangled_port(&mut self, node_number: i32, mangled_port: u16) {
        let l = self.local_node_number;
        if self.connection_states[l as usize] == NatConnectionState::Done {
            return;
        }
        let target_slot_index = self.connection_nodes[self.target_node_number as usize].slot_index;
        let local_slot_index = self.connection_nodes[l as usize].slot_index;
        if target_slot_index < 0 || !self.host.slot_exists(target_slot_index as usize) {
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        if local_slot_index < 0 || !self.host.slot_exists(local_slot_index as usize) {
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        if node_number != self.target_node_number {
            return;
        }
        let tsi = target_slot_index as usize;
        self.host.slot_set_port(tsi, mangled_port);
        let local_bug = (self.host.slot_nat_behavior(local_slot_index as usize) & firewall_type::NETGEAR_BUG) != 0;
        let target_bug = (self.host.slot_nat_behavior(tsi) & firewall_type::NETGEAR_BUG) != 0;
        if !local_bug || self.been_probed || (local_bug && target_bug) {
            let (ip, port) = (self.host.slot_ip(tsi), self.host.slot_port(tsi));
            self.send_a_probe(ip, port, l);
            self.notify_target_of_probe(tsi);
            self.set_connection_state(l, NatConnectionState::WaitingForResponse);
        }
    }

    /// `NAT::gotInternalAddress` (NAT.cpp): on the same machine (same external IP) the target's internal
    /// address replaces its slot IP.
    pub fn got_internal_address(&mut self, node_number: i32, address: u32) {
        if node_number < 0 || node_number as usize >= SLOTS {
            return;
        }
        let target_slot_index = self.connection_nodes[node_number as usize].slot_index;
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        if target_slot_index < 0
            || !self.host.slot_exists(target_slot_index as usize)
            || local_slot_index < 0
            || !self.host.slot_exists(local_slot_index as usize)
        {
            return;
        }
        if node_number != self.target_node_number {
            return;
        }
        if self.host.slot_ip(local_slot_index as usize) == self.host.slot_ip(target_slot_index as usize) {
            self.host.slot_set_ip(target_slot_index as usize, address);
        }
    }

    /// `NAT::notifyTargetOfProbe` (NAT.cpp): a "PROBED<node>" message to the target.
    pub fn notify_target_of_probe(&mut self, target_slot: usize) {
        let options = format!("PROBED{}", self.local_node_number);
        let nick = self.host.slot_name_ascii(target_slot);
        self.host.peer_utm_player("NAT/", &nick, options.as_bytes());
    }

    fn other_human_nicks(&self) -> Vec<u8> {
        let mut names: Vec<u8> = Vec::new();
        let own = self.connection_nodes[self.local_node_number as usize].slot_index;
        for i in 0..SLOTS {
            if own == i as i32 {
                continue;
            }
            if !self.host.slot_exists(i) || !self.host.slot_is_human(i) {
                continue;
            }
            if !names.is_empty() {
                names.push(b',');
            }
            names.extend_from_slice(&self.host.slot_name_ascii(i));
        }
        names
    }

    /// `NAT::notifyUsersOfConnectionDone` (NAT.cpp): "CONNDONE<node> <me>" to every other human.
    pub fn notify_users_of_connection_done(&mut self, node_index: i32) {
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        if local_slot_index < 0 || !self.host.slot_exists(local_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        let options = format!("CONNDONE{} {}", node_index, self.local_node_number);
        let names = self.other_human_nicks();
        self.host.peer_utm_player("NAT", &names, options.as_bytes());
    }

    /// `NAT::notifyUsersOfConnectionFailed` (NAT.cpp): "CONNFAILED<node>" to every other human.
    pub fn notify_users_of_connection_failed(&mut self, node_index: i32) {
        let local_slot_index = self.connection_nodes[self.local_node_number as usize].slot_index;
        if local_slot_index < 0 || !self.host.slot_exists(local_slot_index as usize) {
            let l = self.local_node_number;
            self.set_connection_state(l, NatConnectionState::Failed);
            return;
        }
        let options = format!("CONNFAILED{}", node_index);
        let names = self.other_human_nicks();
        self.host.peer_utm_player("NAT", &names, options.as_bytes());
    }

    /// `NAT::sendMangledPortNumberToTarget` (NAT.cpp): "PORT<node> <port> <ip hex>" to the target.
    pub fn send_mangled_port_number_to_target(&mut self, mangled_port: u16, target_slot: usize) {
        let options = format!("PORT{} {} {:08X}", self.local_node_number, mangled_port, self.local_ip);
        let nick = self.host.slot_name_ascii(target_slot);
        self.host.peer_utm_player("NAT/", &nick, options.as_bytes());
    }

    /// `NAT::processGlobalMessage` (NAT.cpp): a NAT message from a staging-room player: "PROBED<node>",
    /// "CONNDONE..." (its handling is inside a comment upstream), "CONNFAILED<node>", "PORT<node> <port> <ip>".
    pub fn process_global_message(&mut self, _slot_num: i32, options: &[u8]) {
        let mut p = 0usize;
        while p < options.len() && options[p].is_ascii_whitespace() {
            p += 1;
        }
        let ptr = &options[p..];
        if ptr.starts_with(b"PROBED") {
            let node = atoi(&ptr[6..]);
            if node == self.target_node_number {
                self.probed(node);
            }
        } else if ptr.starts_with(b"CONNDONE") {
            // (the body of this branch is commented out upstream)
        } else if ptr.starts_with(b"CONNFAILED") {
            let node = atoi(&ptr[10..]);
            if node >= 0 && (node as u32) < self.num_nodes {
                self.set_connection_state(node, NatConnectionState::Failed);
            }
        } else if ptr.starts_with(b"PORT") {
            let rest = &ptr[4..];
            let node = atoi(rest);
            let mut c = 0usize;
            while c < rest.len() && rest[c] != b' ' {
                c += 1;
            }
            while c < rest.len() && rest[c] == b' ' {
                c += 1;
            }
            // sscanf(c, "%d %X", &intport, &addr)
            let tail = &rest[c.min(rest.len())..];
            let mut it = tail.split(|&b| b == b' ').filter(|s| !s.is_empty());
            let intport = it.next().map_or(0, atoi) as u32;
            let addr = it
                .next()
                .and_then(|s| std::str::from_utf8(s).ok())
                .and_then(|s| u32::from_str_radix(s.trim(), 16).ok())
                .unwrap_or(0);
            let mut port = intport as u16;
            if node >= 0 && (node as u32) < self.num_nodes {
                if port < 1024 {
                    // DEBUG_ASSERTCRASH(port >= 1024, "Was passed an invalid port number")
                    port = port.wrapping_add(1024);
                }
                self.got_internal_address(node, addr);
                self.got_mangled_port(node, port);
            }
        }
    }

    /// `NAT::setConnectionState` (NAT.cpp): sets the node's state; for the local node also updates the target's
    /// line on the connection screen.
    pub fn set_connection_state(&mut self, node_number: i32, state: NatConnectionState) {
        if node_number < 0 || node_number as usize >= SLOTS {
            return;
        }
        self.connection_states[node_number as usize] = state;
        if node_number != self.local_node_number {
            return;
        }
        if self.local_node_number == self.target_node_number {
            return;
        }
        if self.target_node_number == -1 {
            return;
        }
        let slot_index = self.connection_nodes[self.target_node_number as usize].slot_index;
        let own = self.connection_nodes[self.local_node_number as usize].slot_index;
        let mut slot = 0;
        let mut i = 0usize;
        while i < SLOTS {
            if self.host.slot_exists(i) && self.host.slot_is_human(i) && i as i32 != own {
                if i as i32 == slot_index {
                    break;
                }
                slot += 1;
            }
            i += 1;
        }
        if i == SLOTS {
            // DEBUG_ASSERTCRASH(i < MAX_SLOTS, "Didn't find the node number in the slot list")
            return;
        }
        self.host.menu_set_player_status(slot, state);
    }
}

/// C `atoi`: optional spaces and sign, then digits (0 when there are none).
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add((s[i] - b'0') as i32);
        i += 1;
    }
    if neg { v.wrapping_neg() } else { v }
}
