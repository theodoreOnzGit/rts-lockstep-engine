//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/FirewallHelper.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/FirewallHelper.cpp
//!
//! `FirewallHelperClass`: the mangler client. It asks EA's "mangler" servers which source port they see for a UDP
//! packet from a spare local port, and from several such probes works out how the player's NAT rewrites ports
//! (none, dumb, smart, per-destination-port, absolute/relative allocation delta). [`super::nat::NAT`] uses its
//! spare sockets, mangler queries and temporary source ports through `NatHost`.
//!
//! # Not reachable today
//!
//! The mangler servers (`TheGameSpyConfig->getManglerLocation`) were GameSpy-era EA servers and no longer exist; the
//! detection state machine, the 20-byte `ManglerData` wire format with its CRC, the port-allocation analysis and the
//! spare-socket table are translated. The few environment reads (time, DNS, the `OptionPreferences` file, the
//! `TheGlobalData` fields) are the [`FirewallHost`].
//!
//! # Upstream quirks kept
//!
//! * `ManglerData`'s header comment says "size = 16 bytes" but the packed struct is 20 bytes (CRC 4, magic 2,
//!   PacketID 2, MyMangledPortNumber 2, OriginalPortNumber 2, MyMangledAddress 4, NetCommandType 1, BlitzMe 1,
//!   Padding 2); the CRC covers the last `sizeof - 4` = 16 bytes. A `ManglerMessage` is 30 bytes.
//! * `sendToManglerFromPort` fills the data with 0x44 bytes first, so the unset `MyMangledPortNumber`, address and
//!   padding go out as 0x4444 / 0x44 bytes.
//! * `byteAdjust` is `htons`/`htonl` on the native value, applied once before sending and again after receiving.
//!   On a little-endian host that is a swap in both places; here it is `to_be()` on the field, as C++ does.
//! * `detectionBeginUpdate` runs `ntohl((UnsignedInt)mangler_addresses[m])` and throws the result away; the
//!   addresses reach `m_manglers` as big-endian-ordered bytes shifted into a `u32`.
//! * In `detectionTest3WaitForResponsesUpdate` the two close loops run `j < i` with `i` left at `NUM_TEST_PORTS`
//!   by the earlier `for` (VC6 scoping), over the spare-socket table's slots (not `m_sparePorts`).
//! * `getNextTemporarySourcePort` returns the last port it tried even when none could be opened, and
//!   `m_sourcePortPool` wraps to 2048 after the port was already truncated to 16 bits.
//! * The "could not open" debug message prints `%d` without an argument.

use super::net_command_msg::GENERALS_MAGIC_NUMBER;
use super::nat::firewall_type;
use super::udp::UDP;
use crate::common::crc::Crc;

/// `NUM_TEST_PORTS`.
pub const NUM_TEST_PORTS: usize = 4;
/// `MAX_SPARE_SOCKETS`.
pub const MAX_SPARE_SOCKETS: usize = 8;
/// `MAX_NUM_MANGLERS`.
pub const MAX_NUM_MANGLERS: usize = 4;
/// `MANGLER_PORT`.
pub const MANGLER_PORT: u16 = 4321;
/// `sizeof(ManglerData)` (packed).
pub const MANGLER_DATA_SIZE: usize = 20;
/// The initial `m_sourcePortPool`.
const INITIAL_SOURCE_PORT_POOL: i32 = 4096;

/// `enum FirewallDetectionState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirewallDetectionState {
    /// `DETECTIONSTATE_IDLE`.
    Idle,
    /// `DETECTIONSTATE_BEGIN`.
    Begin,
    /// `DETECTIONSTATE_TEST1`.
    Test1,
    /// `DETECTIONSTATE_TEST2`.
    Test2,
    /// `DETECTIONSTATE_TEST3`.
    Test3,
    /// `DETECTIONSTATE_TEST3_WAITFORRESPONSES`.
    Test3WaitForResponses,
    /// `DETECTIONSTATE_TEST4_1`.
    Test4_1,
    /// `DETECTIONSTATE_TEST4_2`.
    Test4_2,
    /// `DETECTIONSTATE_TEST5`.
    Test5,
    /// `DETECTIONSTATE_DONE`.
    Done,
}

/// `struct ManglerData` (packed, 20 bytes). Fields hold native-endian values; [`ManglerData::to_bytes`] writes them
/// in native byte order exactly as the C++ struct sits in memory.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManglerData {
    /// `CRC`.
    pub crc: u32,
    /// `magic`.
    pub magic: u16,
    /// `PacketID`.
    pub packet_id: u16,
    /// `MyMangledPortNumber`.
    pub my_mangled_port_number: u16,
    /// `OriginalPortNumber`.
    pub original_port_number: u16,
    /// `MyMangledAddress`.
    pub my_mangled_address: [u8; 4],
    /// `NetCommandType`.
    pub net_command_type: u8,
    /// `BlitzMe`.
    pub blitz_me: u8,
    /// `Padding`.
    pub padding: u16,
}

impl ManglerData {
    /// The struct's memory image.
    pub fn to_bytes(&self) -> [u8; MANGLER_DATA_SIZE] {
        let mut b = [0u8; MANGLER_DATA_SIZE];
        b[0..4].copy_from_slice(&self.crc.to_ne_bytes());
        b[4..6].copy_from_slice(&self.magic.to_ne_bytes());
        b[6..8].copy_from_slice(&self.packet_id.to_ne_bytes());
        b[8..10].copy_from_slice(&self.my_mangled_port_number.to_ne_bytes());
        b[10..12].copy_from_slice(&self.original_port_number.to_ne_bytes());
        b[12..16].copy_from_slice(&self.my_mangled_address);
        b[16] = self.net_command_type;
        b[17] = self.blitz_me;
        b[18..20].copy_from_slice(&self.padding.to_ne_bytes());
        b
    }

    /// The struct read back from a memory image.
    pub fn from_bytes(b: &[u8; MANGLER_DATA_SIZE]) -> ManglerData {
        ManglerData {
            crc: u32::from_ne_bytes([b[0], b[1], b[2], b[3]]),
            magic: u16::from_ne_bytes([b[4], b[5]]),
            packet_id: u16::from_ne_bytes([b[6], b[7]]),
            my_mangled_port_number: u16::from_ne_bytes([b[8], b[9]]),
            original_port_number: u16::from_ne_bytes([b[10], b[11]]),
            my_mangled_address: [b[12], b[13], b[14], b[15]],
            net_command_type: b[16],
            blitz_me: b[17],
            padding: u16::from_ne_bytes([b[18], b[19]]),
        }
    }
}

/// `struct ManglerMessage`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManglerMessage {
    /// `data`.
    pub data: ManglerData,
    /// `length` (0 = an empty slot).
    pub length: i32,
    /// `ip`.
    pub ip: u32,
    /// `port`.
    pub port: u16,
}

/// `struct SpareSocketStruct`.
#[derive(Debug, Default)]
pub struct SpareSocketStruct {
    /// `udp`.
    pub udp: Option<UDP>,
    /// `port` (0 = free).
    pub port: u16,
}

/// The environment of the helper.
pub trait FirewallHost {
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    /// `OptionPreferences["FirewallNeedToRefresh"]` compared with "TRUE" ignoring case.
    fn pref_firewall_need_to_refresh(&self) -> bool;
    /// `OptionPreferences["FirewallNeedToRefresh"] = "TRUE"/"FALSE"; pref.write()`.
    fn pref_write_firewall_need_to_refresh(&mut self, flag: bool);
    /// `OptionPreferences["FirewallBehavior"]` and `["FirewallPortAllocationDelta"] = ...; pref.write()`.
    fn pref_write_firewall_behavior(&mut self, behavior: u32, port_allocation_delta: i32);
    /// `TheWritableGlobalData->m_firewallBehavior`.
    fn global_firewall_behavior(&self) -> u32;
    /// `TheWritableGlobalData->m_firewallBehavior = value`.
    fn set_global_firewall_behavior(&mut self, value: u32);
    /// `TheGlobalData->m_firewallPortOverride`.
    fn global_firewall_port_override(&self) -> u16;
    /// `TheGlobalData->m_firewallSendDelay`.
    fn global_firewall_send_delay(&self) -> bool;
    /// `TheGlobalData->m_firewallPortAllocationDelta`.
    fn global_firewall_port_allocation_delta(&self) -> i32;
    /// `TheWritableGlobalData->m_firewallPortAllocationDelta = value`.
    fn set_global_firewall_port_allocation_delta(&mut self, value: i32);
    /// `TheGameSpyConfig->getManglerLocation(index, host, port)`: the host name (empty if none).
    fn mangler_location(&self, index: i32) -> Vec<u8>;
    /// `gethostbyname`: the first IPv4 address as its 4 network-order bytes; None if the lookup failed.
    fn gethostbyname(&mut self, name: &[u8]) -> Option<[u8; 4]>;
}

/// `class FirewallHelperClass`.
#[derive(Debug)]
pub struct FirewallHelperClass {
    /// `m_behavior`.
    m_behavior: u32,
    /// `m_lastBehavior`.
    m_last_behavior: u32,
    /// `m_sourcePortAllocationDelta`.
    m_source_port_allocation_delta: i32,
    /// `m_lastSourcePortAllocationDelta`.
    m_last_source_port_allocation_delta: i32,
    /// `m_sourcePortPool` (static upstream; one per helper here).
    m_source_port_pool: i32,
    /// `m_spareSockets`.
    m_spare_sockets: [SpareSocketStruct; MAX_SPARE_SOCKETS],
    /// `m_manglers`.
    m_manglers: [u32; MAX_NUM_MANGLERS],
    /// `m_numManglers`.
    m_num_manglers: i32,
    /// `m_sparePorts`.
    m_spare_ports: [u16; MAX_SPARE_SOCKETS],
    /// `m_mangledPorts`.
    m_mangled_ports: [u16; MAX_SPARE_SOCKETS],
    /// `m_packetID`.
    m_packet_id: u16,
    /// `m_messages`.
    m_messages: [ManglerMessage; MAX_SPARE_SOCKETS],
    /// `m_currentState`.
    m_current_state: FirewallDetectionState,
    /// `m_timeoutStart`.
    m_timeout_start: u32,
    /// `m_timeoutLength`.
    m_timeout_length: u32,
    /// `m_currentTry`.
    m_current_try: i32,
    /// `m_numResponses`.
    m_num_responses: i32,
}

impl FirewallHelperClass {
    /// `FirewallHelperClass::FirewallHelperClass()` (FirewallHelper.cpp): unknown behaviour, empty tables, state
    /// idle, and the source-port pool at `4096 + ((timeGetTime()/1000) % 1000)` so a new run does not reuse a port
    /// before the old connection timed out.
    pub fn new<H: FirewallHost>(host: &H) -> FirewallHelperClass {
        FirewallHelperClass {
            m_behavior: firewall_type::UNKNOWN,
            m_last_behavior: firewall_type::UNKNOWN,
            m_source_port_allocation_delta: 0,
            m_last_source_port_allocation_delta: 0,
            m_source_port_pool: INITIAL_SOURCE_PORT_POOL + ((host.time_get_time() / 1000) % 1000) as i32,
            m_spare_sockets: Default::default(),
            m_manglers: [0; MAX_NUM_MANGLERS],
            m_num_manglers: 0,
            m_spare_ports: [0; MAX_SPARE_SOCKETS],
            m_mangled_ports: [0; MAX_SPARE_SOCKETS],
            m_packet_id: 0,
            m_messages: [ManglerMessage::default(); MAX_SPARE_SOCKETS],
            m_current_state: FirewallDetectionState::Idle,
            m_timeout_start: 0,
            m_timeout_length: 0,
            m_current_try: 0,
            m_num_responses: 0,
        }
    }

    /// `FirewallHelperClass::reset`: closes every spare socket, state idle, clears the stored messages.
    pub fn reset(&mut self) {
        self.close_all_spare_sockets();
        self.m_current_state = FirewallDetectionState::Idle;
        for i in 0..MAX_SPARE_SOCKETS {
            self.m_messages[i].length = 0;
        }
    }

    /// `getRawFirewallBehavior`.
    pub fn get_raw_firewall_behavior(&self) -> u16 {
        self.m_behavior as u16
    }
    /// `setSourcePortPoolStart`.
    pub fn set_source_port_pool_start(&mut self, port: i32) {
        self.m_source_port_pool = port;
    }
    /// `getSourcePortPool`.
    pub fn get_source_port_pool(&self) -> i32 {
        self.m_source_port_pool
    }
    /// `getSourcePortAllocationDelta` (truncated to `Short`).
    pub fn get_source_port_allocation_delta(&self) -> i16 {
        self.m_source_port_allocation_delta as i16
    }
    /// `getFirewallBehavior`: sets the state back to idle and returns the behaviour.
    pub fn get_firewall_behavior(&mut self) -> u32 {
        self.m_current_state = FirewallDetectionState::Idle;
        self.m_behavior
    }
    /// `readFirewallBehavior`: its body is `#if (0)` upstream.
    pub fn read_firewall_behavior(&mut self) {}

    /// `isNAT()`.
    pub fn is_nat(&self) -> bool {
        Self::is_nat_behavior(self.m_behavior)
    }
    /// `isNAT(behavior)`: not unknown and not simple.
    pub fn is_nat_behavior(behavior: u32) -> bool {
        !(behavior == firewall_type::UNKNOWN || (behavior & firewall_type::SIMPLE) != 0)
    }
    /// `isNetgear(behavior)`.
    pub fn is_netgear_behavior(behavior: u32) -> bool {
        (behavior & firewall_type::NETGEAR_BUG) != 0
    }
    /// `isNetgear()`.
    pub fn is_netgear(&self) -> bool {
        Self::is_netgear_behavior(self.m_behavior)
    }

    /// `FirewallHelperClass::detectFirewall`: if the saved options say the firewall needs refreshing, forgets the
    /// stored behaviour; with an unknown stored behaviour starts a detection (false), otherwise detection is
    /// skipped (true).
    pub fn detect_firewall<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        if host.pref_firewall_need_to_refresh() {
            host.set_global_firewall_behavior(firewall_type::UNKNOWN);
        }
        if host.global_firewall_behavior() == firewall_type::UNKNOWN {
            self.detect_firewall_behavior();
            false
        } else {
            true
        }
    }

    /// `detectFirewallBehavior`: behaviour starts as SIMPLE; state BEGIN.
    fn detect_firewall_behavior(&mut self) {
        self.m_behavior = firewall_type::SIMPLE;
        self.m_current_state = FirewallDetectionState::Begin;
    }

    /// `behaviorDetectionUpdate`: dispatches on the state; false while idle, true when done.
    pub fn behavior_detection_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        use FirewallDetectionState as S;
        match self.m_current_state {
            S::Idle => false,
            S::Done => true,
            S::Begin => self.detection_begin_update(host),
            S::Test1 => self.detection_test1_update(host),
            S::Test2 => self.detection_test2_update(host),
            S::Test3 => self.detection_test3_update(host),
            S::Test3WaitForResponses => self.detection_test3_wait_for_responses_update(host),
            S::Test4_1 => self.detection_test4_stage1_update(host),
            S::Test4_2 => self.detection_test4_stage2_update(host),
            S::Test5 => self.detection_test5_update(),
        }
    }

    /// `writeFirewallBehavior`: stores `FirewallBehavior` and the port allocation delta in the options file (and the
    /// delta in the global data first).
    pub fn write_firewall_behavior<H: FirewallHost>(&mut self, host: &mut H) {
        let delta = self.get_source_port_allocation_delta() as i32;
        host.set_global_firewall_port_allocation_delta(delta);
        host.pref_write_firewall_behavior(host.global_firewall_behavior(), host.global_firewall_port_allocation_delta());
    }

    /// `flagNeedToRefresh(flag)`: stores `FirewallNeedToRefresh` = TRUE/FALSE in the options file.
    pub fn flag_need_to_refresh<H: FirewallHost>(&mut self, host: &mut H, flag: bool) {
        host.pref_write_firewall_need_to_refresh(flag);
    }

    /// `getManglerName(manglerIndex, nameBuf)`: the configured host name of mangler `index` (empty if none).
    pub fn get_mangler_name<H: FirewallHost>(host: &H, mangler_index: i32) -> Vec<u8> {
        host.mangler_location(mangler_index)
    }

    // ---- spare sockets ----

    /// `findSpareSocketByPort`: the table slot bound to `port`.
    fn find_spare_socket_by_port(&self, port: u16) -> Option<usize> {
        (0..MAX_SPARE_SOCKETS).find(|&i| self.m_spare_sockets[i].port == port)
    }

    /// `findEmptyMessage`: the first stored-message slot with length 0.
    fn find_empty_message(&self) -> Option<usize> {
        (0..MAX_SPARE_SOCKETS).find(|&i| self.m_messages[i].length == 0)
    }

    /// `openSpareSocket(port)`: binds a new UDP socket to `0.0.0.0:port` in the first free table slot; false if the
    /// table is full or the bind fails (in which case the slot keeps its object but port 0, as upstream).
    pub fn open_spare_socket(&mut self, port: u16) -> bool {
        let mut slot = MAX_SPARE_SOCKETS;
        for i in 0..MAX_SPARE_SOCKETS {
            if self.m_spare_sockets[i].port == 0 {
                slot = i;
                break;
            }
        }
        // don't have room for any more spare sockets.  Fail.
        if slot == MAX_SPARE_SOCKETS {
            return false;
        }
        let mut udp = UDP::new();
        if udp.bind(0, port) != 0 {
            // DEBUG_CRASH("Failed to init spare socket")
            self.m_spare_sockets[slot].udp = Some(udp);
            return false;
        }
        self.m_spare_sockets[slot].udp = Some(udp);
        self.m_spare_sockets[slot].port = port;
        true
    }

    /// `closeSpareSocket(port)`: drops the socket bound to `port` and frees its slot.
    pub fn close_spare_socket(&mut self, port: u16) {
        for i in 0..MAX_SPARE_SOCKETS {
            if self.m_spare_sockets[i].port == port {
                self.m_spare_sockets[i].udp = None;
                self.m_spare_sockets[i].port = 0;
                break;
            }
        }
    }

    /// `closeAllSpareSockets`.
    pub fn close_all_spare_sockets(&mut self) {
        for i in 0..MAX_SPARE_SOCKETS {
            self.m_spare_sockets[i].port = 0;
            self.m_spare_sockets[i].udp = None;
        }
    }

    /// `getNextTemporarySourcePort(skip)`: steps `m_sourcePortPool` by `skip` (1 if 0) up to 256 times until a
    /// port can be bound (and then closes it again), returning it; the last port tried if none could.
    pub fn get_next_temporary_source_port(&mut self, skip: i32) -> u16 {
        let mut return_port = self.m_source_port_pool as u16;
        let mut tries = 256;
        let skip = if skip == 0 { 1 } else { skip };
        while tries > 0 {
            tries -= 1;
            self.m_source_port_pool += skip;
            return_port = self.m_source_port_pool as u16;
            if self.m_source_port_pool > 65535 {
                self.m_source_port_pool = 2048;
            }
            // Validate the port by trying to bind it to a socket.
            if self.open_spare_socket(return_port) {
                self.close_spare_socket(return_port);
                return return_port;
            }
        }
        return_port
    }

    // ---- mangler wire format ----

    /// `byteAdjust`: `htonl`/`htons` of the CRC, magic, mangled port, original port and packet id.
    fn byte_adjust(data: &mut ManglerData) {
        data.crc = data.crc.to_be();
        data.magic = data.magic.to_be();
        data.my_mangled_port_number = data.my_mangled_port_number.to_be();
        data.original_port_number = data.original_port_number.to_be();
        data.packet_id = data.packet_id.to_be();
    }

    /// `sendToManglerFromPort(address, port, packetID, blitzme)`: builds the request (0x44 fill, type 12, packet id,
    /// blitz flag, magic, original port), byte-adjusts it, sets the CRC of its last 16 bytes (`htonl`) and sends it
    /// to the mangler's port 4321 from the spare socket bound to `port`; false if there is no such socket.
    pub fn send_to_mangler_from_port(&mut self, address: u32, port: u16, packet_id: u16, blitzme: bool) -> bool {
        let mut data = ManglerData {
            crc: 0x4444_4444,
            magic: 0x4444,
            packet_id: 0x4444,
            my_mangled_port_number: 0x4444,
            original_port_number: 0x4444,
            my_mangled_address: [0x44; 4],
            net_command_type: 0x44,
            blitz_me: 0x44,
            padding: 0x4444,
        };
        data.net_command_type = 12; // mangler request.
        data.packet_id = packet_id;
        data.blitz_me = if blitzme { 1 } else { 0 };
        data.magic = GENERALS_MAGIC_NUMBER;
        data.original_port_number = port;

        Self::byte_adjust(&mut data);

        let mut crc = Crc::new();
        let bytes = data.to_bytes();
        crc.compute_crc(&bytes[4..MANGLER_DATA_SIZE]);
        data.crc = crc.get().to_be();

        let slot = match self.find_spare_socket_by_port(port) {
            Some(s) => s,
            None => return false, // Could not find spare socket for send.
        };
        let bytes = data.to_bytes();
        if let Some(udp) = self.m_spare_sockets[slot].udp.as_mut() {
            udp.write(&bytes, address, MANGLER_PORT);
        }
        true
    }

    /// `getManglerResponse(packetID, time)`: reads one datagram from every open spare socket (while a stored-message
    /// slot is free), drops those with a bad CRC, byte-adjusts the rest and takes the one whose packet id matches
    /// (directly or after `ntohs`); failing that, a previously stored message with the id. Returns the port the
    /// mangler saw (`MyMangledPortNumber`), 0 if none.
    pub fn get_mangler_response(&mut self, packet_id: u16, _time: i32) -> u16 {
        let mut msg: Option<usize> = None;

        for i in 0..MAX_SPARE_SOCKETS {
            if self.m_spare_sockets[i].udp.is_some() {
                let message_idx = match self.find_empty_message() {
                    Some(m) => m,
                    None => break,
                };
                let mut buf = [0u8; MANGLER_DATA_SIZE];
                let (retval, _ip, _port) = self.m_spare_sockets[i].udp.as_mut().unwrap().read(&mut buf);
                // the message's bytes are whatever the read stored (a short datagram leaves the rest as it was)
                let mut data = ManglerData::from_bytes(&buf);
                if retval > 0 {
                    let mut crc = Crc::new();
                    crc.compute_crc(&buf[4..MANGLER_DATA_SIZE]);
                    if crc.get() != data.crc.to_be() {
                        continue; // CRC mismatch
                    }
                    Self::byte_adjust(&mut data);
                    let message = &mut self.m_messages[message_idx];
                    message.data = data;
                    message.length = retval;
                    if message.data.packet_id == packet_id {
                        msg = Some(message_idx);
                        message.length = 0;
                    }
                    if u16::from_be(message.data.packet_id) == packet_id {
                        msg = Some(message_idx);
                        message.length = 0;
                    }
                }
            }
        }

        // See if we have already received it and saved it.
        if msg.is_none() {
            for i in 0..MAX_SPARE_SOCKETS {
                if self.m_messages[i].length != 0 && self.m_messages[i].data.packet_id == packet_id {
                    msg = Some(i);
                    self.m_messages[i].length = 0;
                }
            }
        }

        match msg {
            None => 0,
            Some(m) => self.m_messages[m].data.my_mangled_port_number,
        }
    }

    // ---- detection state machine ----

    /// `detectionBeginUpdate`: with a user-specified source port, acts as if there is no firewall (SIMPLE, plus the
    /// Netgear bug if a send delay is configured) and is done; otherwise resolves up to 4 distinct mangler
    /// addresses within 5 s (needs at least 3, else done), adds the Netgear bit if configured, opens a spare socket
    /// and sends the first probe (state TEST1).
    pub fn detection_begin_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        self.m_packet_id = 0x7f00;
        let mut mangler_addresses = [[0u8; 4]; MAX_NUM_MANGLERS];

        // If the user specified a particular port to use then we act as if there is no firewall.
        if host.global_firewall_port_override() != 0 {
            self.m_behavior = firewall_type::SIMPLE;
            if host.global_firewall_send_delay() {
                self.m_behavior |= firewall_type::NETGEAR_BUG;
            }
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        self.m_timeout_start = host.time_get_time();
        self.m_timeout_length = 5000;
        let mut namenum = 0;

        loop {
            let name = host.mangler_location(namenum);
            namenum += 1;

            if name.is_empty() {
                break;
            }

            // Do the lookup.
            let addr = match host.gethostbyname(&name) {
                Some(a) => a,
                None => break, // gethostbyname failed
            };

            // See if we already have that address in the list.
            let mut found = false;
            for i in 0..self.m_num_manglers as usize {
                if mangler_addresses[i] == addr {
                    found = true;
                    break;
                }
            }
            // Add the address in if we didn't find it.
            if !found {
                let m = self.m_num_manglers as usize;
                self.m_num_manglers += 1;
                mangler_addresses[m] = addr;
            }

            if !((self.m_num_manglers as usize) < MAX_NUM_MANGLERS
                && host.time_get_time().wrapping_sub(self.m_timeout_start) < self.m_timeout_length)
            {
                break;
            }
        }

        // DEBUG_ASSERTCRASH(m_numManglers > 2, ("not enough mangler addresses found."))
        if self.m_num_manglers < 3 {
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        for i in 0..self.m_num_manglers as usize {
            let a = mangler_addresses[i];
            self.m_manglers[i] = (a[3] as u32) + ((a[2] as u32) << 8) + ((a[1] as u32) << 16) + ((a[0] as u32) << 24);
        }

        // See if the user specified a netgear firewall - that will save us the trouble.
        if host.global_firewall_send_delay() {
            self.m_behavior |= firewall_type::NETGEAR_BUG;
        }

        // First test, see if there is any port mangling at all.
        self.m_spare_ports[0] = self.get_next_temporary_source_port(0);
        if !self.open_spare_socket(self.m_spare_ports[0]) {
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        // Send to the mangler from this port until we get a response.
        self.m_timeout_start = host.time_get_time();
        self.m_timeout_length = 6000;

        self.send_to_mangler_from_port(self.m_manglers[0], self.m_spare_ports[0], self.m_packet_id, false);
        self.m_current_state = FirewallDetectionState::Test1;
        false
    }

    /// `detectionTest1Update`: waits (up to 6 s) for the mangler's answer to the first probe; no answer, or an
    /// unmangled one (also sets the allocation delta to 0), ends detection; a mangled answer starts test 2 by
    /// probing the second mangler from the same port.
    pub fn detection_test1_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        self.m_mangled_ports[0] = self.get_mangler_response(self.m_packet_id, 0);

        // See if we got no response or a non-mangled response.
        if self.m_mangled_ports[0] == 0 || self.m_mangled_ports[0] == self.m_spare_ports[0] {
            if self.m_mangled_ports[0] == self.m_spare_ports[0] {
                self.m_source_port_allocation_delta = 0;
            }
            if self.m_mangled_ports[0] == 0 && host.time_get_time().wrapping_sub(self.m_timeout_start) < self.m_timeout_length {
                // still waiting for a response and haven't timed out yet.
                return false;
            }
            // either we have received a non-mangled response or we timed out waiting for a response.
            self.close_spare_socket(self.m_spare_ports[0]);
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        // Second test. See if the ports are mangled differently for different destination IPs.
        self.m_timeout_start = host.time_get_time();
        self.m_timeout_length = 6000;
        self.m_mangled_ports[1] = 0;
        self.send_to_mangler_from_port(self.m_manglers[1], self.m_spare_ports[0], self.m_packet_id.wrapping_add(1), false);

        self.m_current_state = FirewallDetectionState::Test2;
        false
    }

    /// `detectionTest2Update`: waits (inclusive 6 s) for the second answer; an unmangled answer adds SIMPLE and ends;
    /// equal mangled ports add DUMB_MANGLING, different ones SMART_MANGLING, then moves on to test 3.
    pub fn detection_test2_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        self.m_mangled_ports[1] = self.get_mangler_response(self.m_packet_id.wrapping_add(1), 0);

        if self.m_mangled_ports[1] == 0 {
            if host.time_get_time().wrapping_sub(self.m_timeout_start) <= self.m_timeout_length {
                return false;
            }
            // timed out waiting for mangler response
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        // We are done with this socket/port
        self.close_spare_socket(self.m_spare_ports[0]);

        // See if we got no response or a non-mangled response.
        if self.m_mangled_ports[1] == 0 || self.m_mangled_ports[1] == self.m_spare_ports[0] {
            self.m_current_state = FirewallDetectionState::Done;
            self.m_behavior |= firewall_type::SIMPLE;
            return true;
        }

        if self.m_mangled_ports[0] == self.m_mangled_ports[1] {
            self.m_behavior |= firewall_type::DUMB_MANGLING;
        } else {
            self.m_behavior |= firewall_type::SMART_MANGLING;
        }

        // Third test: a pattern in the ports allocated by the NAT, from several source ports.
        self.m_current_try = 0;
        self.m_packet_id = self.m_packet_id.wrapping_add(10);

        self.m_current_state = FirewallDetectionState::Test3;
        false
    }

    /// `detectionTest3Update`: up to 3 tries: opens `NUM_TEST_PORTS` spare sockets on non-linear source ports
    /// (closing those it opened if one fails, and ending), sends a probe from each (12 s timeout) and waits for the
    /// responses; after 3 tries ends.
    pub fn detection_test3_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        if self.m_current_try < 3 {
            self.m_spare_ports = [0; MAX_SPARE_SOCKETS];
            self.m_mangled_ports = [0; MAX_SPARE_SOCKETS];

            // Open a socket for each source port.
            for i in 0..NUM_TEST_PORTS {
                self.m_spare_ports[i] = self.get_next_temporary_source_port(i as i32);
                if !self.open_spare_socket(self.m_spare_ports[i]) {
                    // Close any spare ports we allocated already before we bail.
                    for j in 0..i {
                        if self.m_spare_ports[j] != 0 {
                            self.close_spare_socket(self.m_spare_ports[j]);
                        }
                    }
                    self.m_current_state = FirewallDetectionState::Done;
                    return true;
                }
            }

            self.m_timeout_start = host.time_get_time();
            self.m_timeout_length = 12000;

            for i in 0..NUM_TEST_PORTS {
                if self.m_mangled_ports[i] == 0 {
                    self.send_to_mangler_from_port(
                        self.m_manglers[0],
                        self.m_spare_ports[i],
                        self.m_packet_id.wrapping_add(i as u16),
                        false,
                    );
                }
            }

            self.m_num_responses = 0;
            self.m_current_state = FirewallDetectionState::Test3WaitForResponses;
            return false;
        }

        // Failed to complete test, quitting
        self.m_current_state = FirewallDetectionState::Done;
        true
    }

    /// `detectionTest3WaitForResponsesUpdate`: collects the four answers (12 s timeout, closing sockets and ending
    /// on timeout), analyses the port allocation, adds SIMPLE_/RELATIVE_PORT_ALLOCATION and the delta (or retries
    /// test 3 keeping the previous run's delta if it looked like a NAT), then for SMART_MANGLING with simple
    /// allocation starts test 4, with relative allocation adds DESTINATION_PORT_DELTA, else goes to test 5.
    pub fn detection_test3_wait_for_responses_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        // upstream's `i` is left at NUM_TEST_PORTS by the for loop below (VC6 scoping)
        let i_after_loop = NUM_TEST_PORTS;
        for i in 0..NUM_TEST_PORTS {
            if self.m_mangled_ports[i] == 0 {
                self.m_mangled_ports[i] = self.get_mangler_response(self.m_packet_id.wrapping_add(i as u16), 0);
                if self.m_mangled_ports[i] != 0 {
                    self.m_num_responses += 1;
                }
            }
        }

        if self.m_num_responses < NUM_TEST_PORTS as i32 {
            if host.time_get_time().wrapping_sub(self.m_timeout_start) > self.m_timeout_length {
                // Close down those sockets - we are finished with them.
                for j in 0..i_after_loop {
                    if self.m_spare_sockets[j].port != 0 {
                        let p = self.m_spare_sockets[j].port;
                        self.close_spare_socket(p);
                    }
                }
                self.m_current_state = FirewallDetectionState::Done;
                return true;
            }
            return false;
        }

        // Close down those sockets - we are finished with them.
        for j in 0..i_after_loop {
            if self.m_spare_sockets[j].port != 0 {
                let p = self.m_spare_sockets[j].port;
                self.close_spare_socket(p);
            }
        }

        // We need at least 4 responses to be sure of the port allocation scheme.
        if self.m_num_responses < 4 {
            if self.m_last_source_port_allocation_delta != 0 && (self.m_last_behavior as i32) > (firewall_type::SIMPLE as i32) {
                // If the delta we got last time we played looks good then use that.
                self.m_source_port_allocation_delta = self.m_last_source_port_allocation_delta;
            }
            self.m_current_state = FirewallDetectionState::Done;
            return true;
        }

        let mut relative_delta = false;
        let mut looks_good = false;
        let mut spare_ports = self.m_spare_ports;
        let mut mangled_ports = self.m_mangled_ports;
        let delta = Self::get_nat_port_allocation_scheme(
            self.m_num_responses,
            &mut spare_ports,
            &mut mangled_ports,
            &mut relative_delta,
            &mut looks_good,
        );
        // the analysis sorts and subtracts in place
        self.m_spare_ports = spare_ports;
        self.m_mangled_ports = mangled_ports;

        if delta != 0 {
            // Hey, we got it!
            if relative_delta {
                self.m_behavior |= firewall_type::RELATIVE_PORT_ALLOCATION;
            } else {
                self.m_behavior |= firewall_type::SIMPLE_PORT_ALLOCATION;
            }
            self.m_source_port_allocation_delta = delta;
        } else {
            if self.m_last_source_port_allocation_delta != 0 && (self.m_last_behavior as i32) > (firewall_type::SIMPLE as i32) {
                // If the delta we got last time we played looks good then use that.
                self.m_source_port_allocation_delta = self.m_last_source_port_allocation_delta;
            }
            self.m_current_try += 1;
            self.m_current_state = FirewallDetectionState::Test3;
            return false;
        }

        // Fourth test. Test to see if the NAT mangles differently per destination port at the same IP.
        if (self.m_behavior & firewall_type::SMART_MANGLING) != 0 {
            if (self.m_behavior & firewall_type::SIMPLE_PORT_ALLOCATION) != 0 {
                // We need 2 source ports for this.
                self.m_spare_ports[0] = self.get_next_temporary_source_port(0);
                if !self.open_spare_socket(self.m_spare_ports[0]) {
                    self.m_current_state = FirewallDetectionState::Done;
                    return true;
                }
                self.m_spare_ports[1] = self.get_next_temporary_source_port(0);
                if !self.open_spare_socket(self.m_spare_ports[1]) {
                    self.close_spare_socket(self.m_spare_ports[0]);
                    self.m_current_state = FirewallDetectionState::Done;
                    return true;
                }

                // Get a reference port.
                self.m_timeout_start = host.time_get_time();
                self.m_timeout_length = 4000;
                self.m_mangled_ports[0] = 0;
                self.m_packet_id = self.m_packet_id.wrapping_add(10);

                // Wait for a response.
                self.send_to_mangler_from_port(self.m_manglers[0], self.m_spare_ports[0], self.m_packet_id, false);

                self.m_current_state = FirewallDetectionState::Test4_1;
                return false;
            } else {
                // NAT32 uses different mangled source ports for different destination ports.
                self.m_behavior |= firewall_type::DESTINATION_PORT_DELTA;
            }
        }
        // else: We don't have smart mangling, skipping test 4, entering test 5

        self.m_current_state = FirewallDetectionState::Test5;
        false
    }

    /// `detectionTest4Stage1Update`: waits (4 s) for the reference answer, then sends three packets from
    /// `spare0 + 1` (no answer expected) and one from the second spare port, and moves to stage 2.
    pub fn detection_test4_stage1_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        self.m_mangled_ports[0] = self.get_mangler_response(self.m_packet_id, 0);

        if self.m_mangled_ports[0] == 0 {
            if host.time_get_time().wrapping_sub(self.m_timeout_start) > self.m_timeout_length {
                self.close_spare_socket(self.m_spare_ports[0]);
                self.close_spare_socket(self.m_spare_ports[1]);
                self.m_current_state = FirewallDetectionState::Done;
                return true;
            }
            return false;
        }

        // Send out to a different port at that IP. We won't get a response for this.
        let addr = self.m_manglers[0];
        let port1 = self.m_spare_ports[0].wrapping_add(1);
        self.send_to_mangler_from_port(addr, port1, self.m_packet_id, false);
        self.send_to_mangler_from_port(addr, port1, self.m_packet_id, false);
        self.send_to_mangler_from_port(addr, port1, self.m_packet_id, false);

        // We can't get a response from a different destination port so the only way to detect this behavior is to
        // check the next mangled port allocation to see if it's double what we would normally expect.
        self.m_packet_id = self.m_packet_id.wrapping_add(1);
        self.m_timeout_start = host.time_get_time();
        self.m_timeout_length = 4000;

        self.send_to_mangler_from_port(self.m_manglers[0], self.m_spare_ports[1], self.m_packet_id, false);

        self.m_current_state = FirewallDetectionState::Test4_2;
        false
    }

    /// `detectionTest4Stage2Update`: waits (4 s) for the second answer; if it is not the first plus the allocation
    /// delta, the NAT uses different source ports per destination port (adds DESTINATION_PORT_DELTA); then test 5.
    pub fn detection_test4_stage2_update<H: FirewallHost>(&mut self, host: &mut H) -> bool {
        self.m_mangled_ports[1] = self.get_mangler_response(self.m_packet_id, 0);

        if self.m_mangled_ports[1] == 0 {
            if host.time_get_time().wrapping_sub(self.m_timeout_start) > self.m_timeout_length {
                self.close_spare_socket(self.m_spare_ports[0]);
                self.close_spare_socket(self.m_spare_ports[1]);
                self.m_current_state = FirewallDetectionState::Done;
                return true;
            }
            return false;
        }

        // `m_mangledPorts[0] + m_sourcePortAllocationDelta` is an `int`
        if self.m_mangled_ports[1] as i32 != self.m_mangled_ports[0] as i32 + self.m_source_port_allocation_delta {
            self.m_behavior |= firewall_type::DESTINATION_PORT_DELTA;
        }
        // else the NAT uses the same source port for different destination ports

        self.m_current_state = FirewallDetectionState::Test5;
        self.detection_test5_update()
    }

    /// `detectionTest5Update`: the netgear test moved before test 1 (`#if (0)`); only reports the behaviour and
    /// finishes.
    pub fn detection_test5_update(&mut self) -> bool {
        self.m_current_state = FirewallDetectionState::Done;
        true
    }

    /// `getNATPortAllocationScheme(numPorts, originalPorts, mangledPorts, relativeDelta, looksGood)`: sorts the
    /// mangled ports (bubble sort, keeping the original ports paired), then looks for absolute sequential (+1),
    /// semi sequential (+2) and constant-`n` allocation (a three-way match is "looks good", a two-way match a
    /// "probable"), then the same on the ports relative to their originals. Returns the delta, 0 if no pattern.
    /// Only the first four entries are examined for the patterns, as upstream.
    pub fn get_nat_port_allocation_scheme(
        num_ports: i32,
        original_ports: &mut [u16],
        mangled_ports: &mut [u16],
        relative_delta: &mut bool,
        looks_good: &mut bool,
    ) -> i32 {
        // Sort the mangled ports into order - should be easier to detect patterns.
        for _x in 0..num_ports {
            for y in 0..(num_ports - 1) as usize {
                if mangled_ports[y] > mangled_ports[y + 1] {
                    let temp = mangled_ports[y];
                    mangled_ports[y] = mangled_ports[y + 1];
                    mangled_ports[y + 1] = temp;
                    let temp = original_ports[y];
                    original_ports[y] = original_ports[y + 1];
                    original_ports[y + 1] = temp;
                }
            }
        }

        let m = |a: &[u16], i: usize| a[i] as i32;

        // 1. Check for absolute sequential allocation.
        if m(mangled_ports, 1) - m(mangled_ports, 0) == 1 {
            if m(mangled_ports, 2) - m(mangled_ports, 1) == 1 {
                if m(mangled_ports, 3) - m(mangled_ports, 2) == 1 {
                    *relative_delta = false;
                    *looks_good = true;
                    return 1;
                }
            }
        }

        // 2. Check for semi sequential.
        if m(mangled_ports, 1) - m(mangled_ports, 0) == 2 {
            if m(mangled_ports, 2) - m(mangled_ports, 1) == 2 {
                if m(mangled_ports, 3) - m(mangled_ports, 2) == 2 {
                    *relative_delta = false;
                    *looks_good = true;
                    return 2;
                }
            }
        }

        let mut diff1 = m(mangled_ports, 1) - m(mangled_ports, 0);
        let mut diff2 = m(mangled_ports, 2) - m(mangled_ports, 1);
        let mut diff3 = m(mangled_ports, 3) - m(mangled_ports, 2);

        // 3. Check for absolute scheme skipping 'n' ports.
        if diff1 == diff2 && diff2 == diff3 {
            *relative_delta = false;
            *looks_good = true;
            return diff1;
        }
        if diff1 == diff2 {
            *relative_delta = false;
            *looks_good = false;
            return diff1;
        }
        if diff2 == diff3 {
            *relative_delta = false;
            *looks_good = false;
            return diff2;
        }

        // 4. Check for relative scheme skipping 'n' ports. NAT32 behaves this way, it skips 100 ports each time.
        for i in 0..num_ports as usize {
            mangled_ports[i] = mangled_ports[i].wrapping_sub(original_ports[i]);
        }

        diff1 = m(mangled_ports, 1) - m(mangled_ports, 0);
        diff2 = m(mangled_ports, 2) - m(mangled_ports, 1);
        diff3 = m(mangled_ports, 3) - m(mangled_ports, 2);

        // Look for a linear pattern.
        if diff1 == diff2 && diff2 == diff3 {
            *relative_delta = true;
            *looks_good = true;
            return diff1;
        }

        // Look for a broken pattern. Maybe the NAT skipped a whole range.
        if diff1 == diff2 || diff1 == diff3 {
            *relative_delta = true;
            *looks_good = false;
            return diff1;
        }
        if diff2 == diff3 {
            *relative_delta = true;
            *looks_good = false;
            return diff2;
        }

        // Aw hell, I don't know what it is.
        *looks_good = false;
        *relative_delta = false;
        0
    }

    /// `getFirewallHardness(behavior)`: SIMPLE +1, DUMB +2, SMART +3, NETGEAR +10, SIMPLE_PORT_ALLOCATION +1,
    /// RELATIVE_PORT_ALLOCATION +2.
    pub fn get_firewall_hardness(behavior: u32) -> i32 {
        let mut hardness = 0;
        if (firewall_type::SIMPLE & behavior) != 0 {
            hardness += 1;
        }
        if (firewall_type::DUMB_MANGLING & behavior) != 0 {
            hardness += 2;
        }
        if (firewall_type::SMART_MANGLING & behavior) != 0 {
            hardness += 3;
        }
        if (firewall_type::NETGEAR_BUG & behavior) != 0 {
            hardness += 10;
        }
        if (firewall_type::SIMPLE_PORT_ALLOCATION & behavior) != 0 {
            hardness += 1;
        }
        if (firewall_type::RELATIVE_PORT_ALLOCATION & behavior) != 0 {
            hardness += 2;
        }
        hardness
    }

    /// `getFirewallRetries(behavior)`: base 2; SIMPLE +1, DUMB +1, SMART +1, RELATIVE_PORT_ALLOCATION +5 (the
    /// Netgear and simple-allocation increments are commented out upstream).
    pub fn get_firewall_retries(behavior: u32) -> i32 {
        let mut retries = 2;
        if (firewall_type::SIMPLE & behavior) != 0 {
            retries += 1;
        }
        if (firewall_type::DUMB_MANGLING & behavior) != 0 {
            retries += 1;
        }
        if (firewall_type::SMART_MANGLING & behavior) != 0 {
            retries += 1;
        }
        if (firewall_type::RELATIVE_PORT_ALLOCATION & behavior) != 0 {
            retries += 5;
        }
        retries
    }
}
