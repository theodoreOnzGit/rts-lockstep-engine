//! PARTLY DIFFERENTIAL-TESTED (#215): `queue_send`, `do_send`, `do_recv`, `update`, `is_generals_packet`, the six statistics getters,
//! `reset` and the tail of `init_with_socket` (buffers and statistics cleared after the bind) match the upstream C++ on 6,000 operation
//! sequences (test: tests/differential_draft_conn.rs), `encrypt_buf`/`decrypt_buf` through tests/differential_draft_netpkt.rs;
//! not yet wired into the game. The socket is a stand-in on both sides (the C++ one is differential/shim_network2/GameNetwork/udp.h),
//! so `udp.rs` itself is not tested here. NOT covered: `init`/`init_host` (the bind loop on a real socket, `ResolveIP`),
//! `allow_broadcasts`, `set_latency`/`set_packet_loss` (stored flags nothing reads).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/Transport.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/Transport.cpp
//!
//! The wire image of a packet is `crc (u32 LE), magic (u16 LE), data[length]` (`#pragma pack(1)`), encrypted word
//! by word: each little-endian 32-bit word is XORed with a mask that starts at `0x0000Fade` and grows by
//! `0x321` per word, then byte-swapped (`htonl` on a little-endian host); a trailing 1-3 bytes are left alone.
//! The CRC covers the magic and the data (everything after the CRC field).
//!
//! # Not translated
//!
//! The `_DEBUG`/`_INTERNAL`-only latency insertion (`m_delayedInBuffer`, `TheGlobalData->m_latencyAverage` and
//! friends) and packet-loss simulation (`m_usePacketLoss`) are compiled out of a release build and are not
//! translated; `setLatency`/`setPacketLoss` store the flags and nothing reads them. Winsock start-up
//! (`WSAStartup`/`WSACleanup`) has no counterpart: the socket is the [`UDP`] backend.

use crate::logic::modules::plant;
use std::sync::{Arc, RwLock};

use crate::common::crc::Crc;

use super::net_command_msg::{
    TransportMessage, GENERALS_MAGIC_NUMBER, MAX_MESSAGES, MAX_MESSAGE_LEN, MAX_PACKET_SIZE,
    MAX_TRANSPORT_STATISTICS_SECONDS,
};
use super::network_util::resolve_ip;
use super::udp::{SockStat, UDP};

/// `sizeof(TransportMessageHeader)`.
const HEADER_LEN: usize = 6;

/// A transport shared between the connections that send through it (`Connection::m_transport` is a pointer to
/// the one `Transport`).
pub type TransportHandle = Arc<RwLock<Transport>>;

/// `encryptBuf` (Transport.cpp): XOR with the running mask, then `htonl`, on each whole 32-bit word.
pub fn encrypt_buf(buf: &mut [u8], len: usize) {
    let mut mask: u32 = 0x0000_Fade;
    let words = len / 4;
    for i in 0..words {
        let o = i * 4;
        let mut w = u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        w ^= mask;
        if !plant(10906) {
            w = w.swap_bytes(); // htonl
        }
        buf[o..o + 4].copy_from_slice(&w.to_le_bytes());
        mask = mask.wrapping_add(if plant(10907) { 0x0000_0123 } else { 0x0000_0321 }); // just for fun
    }
}

/// `decryptBuf` (Transport.cpp): `ntohl` (a byte swap), then XOR with the running mask, on each whole word.
pub fn decrypt_buf(buf: &mut [u8], len: usize) {
    let mut mask: u32 = 0x0000_Fade;
    let words = len / 4;
    for i in 0..words {
        let o = i * 4;
        let mut w = u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        w = w.swap_bytes(); // htonl
        w ^= if plant(10908) { mask >> 1 } else { mask };
        buf[o..o + 4].copy_from_slice(&w.to_le_bytes());
        mask = mask.wrapping_add(0x0000_0321);
    }
}

/// The packed memory image of a message: header then data (`length` bytes).
fn message_image(m: &TransportMessage) -> Vec<u8> {
    let mut v = Vec::with_capacity(HEADER_LEN + m.length.max(0) as usize);
    v.extend_from_slice(&m.header.crc.to_le_bytes());
    v.extend_from_slice(&m.header.magic.to_le_bytes());
    let n = (m.length.max(0) as usize).min(MAX_MESSAGE_LEN);
    v.extend_from_slice(&m.data[..n]);
    v
}

/// The reverse of [`message_image`] for `image.len()` bytes (`memcpy(&msg, image, len)`).
fn store_image(m: &mut TransportMessage, image: &[u8]) {
    let mut hdr = [0u8; HEADER_LEN];
    let h = image.len().min(HEADER_LEN);
    hdr[..h].copy_from_slice(&image[..h]);
    m.header.crc = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    m.header.magic = u16::from_le_bytes([hdr[4], hdr[5]]);
    if image.len() > HEADER_LEN {
        let d = &image[HEADER_LEN..];
        let n = d.len().min(MAX_MESSAGE_LEN);
        m.data[..n].copy_from_slice(&d[..n]);
    }
}

/// `class Transport`: a thin layer around a UDP socket, with queues.
#[derive(Debug)]
pub struct Transport {
    /// `m_outBuffer[MAX_MESSAGES]`.
    pub out_buffer: Vec<TransportMessage>,
    /// `m_inBuffer[MAX_MESSAGES]`.
    pub in_buffer: Vec<TransportMessage>,
    /// `m_port`.
    pub port: u16,
    /// `m_udpsock` (null before `init`).
    udpsock: Option<UDP>,
    /// `m_useLatency` (debug builds only upstream).
    use_latency: bool,
    /// `m_usePacketLoss` (debug builds only upstream).
    use_packet_loss: bool,
    incoming_bytes: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    unknown_bytes: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    outgoing_bytes: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    incoming_packets: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    unknown_packets: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    outgoing_packets: [u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
    /// `m_statisticsSlot`.
    statistics_slot: i32,
    /// `m_lastSecond`.
    last_second: u32,
}

impl Default for Transport {
    fn default() -> Self {
        Transport::new()
    }
}

impl Transport {
    /// `Transport::Transport()` (Transport.cpp): no socket. (The message buffers are not initialised until
    /// `init`; zero-length here, which is what `init` sets.)
    pub fn new() -> Transport {
        let n = MAX_MESSAGES as usize;
        Transport {
            out_buffer: vec![TransportMessage::default(); n],
            in_buffer: vec![TransportMessage::default(); n],
            port: 0,
            udpsock: None,
            use_latency: false,
            use_packet_loss: false,
            incoming_bytes: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            unknown_bytes: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            outgoing_bytes: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            incoming_packets: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            unknown_packets: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            outgoing_packets: [0; MAX_TRANSPORT_STATISTICS_SECONDS as usize],
            statistics_slot: 0,
            last_second: 0,
        }
    }

    /// `Transport::init(AsciiString ip, UnsignedShort port)` (Transport.cpp): `init(ResolveIP(ip), port)`.
    pub fn init_host<F: FnMut() -> u32>(&mut self, ip: &[u8], port: u16, time_get_time: F) -> bool {
        self.init(resolve_ip(ip), port, time_get_time)
    }

    /// `Transport::init(UnsignedInt ip, UnsignedShort port)` (Transport.cpp): binds a new socket, retrying for up
    /// to a second (`time_get_time` is `timeGetTime()`), then clears the buffers and statistics.
    pub fn init<F: FnMut() -> u32>(&mut self, ip: u32, port: u16, time_get_time: F) -> bool {
        let sock = UDP::new();
        self.init_with_socket(sock, ip, port, time_get_time)
    }

    /// `init` with a caller-made socket (the browser build passes a [`UDP::new_queued`] one).
    pub fn init_with_socket<F: FnMut() -> u32>(
        &mut self,
        sock: UDP,
        ip: u32,
        port: u16,
        mut time_get_time: F,
    ) -> bool {
        let mut sock = sock;
        let mut retval: i32 = -1;
        let now = time_get_time();
        while retval != 0 && time_get_time().wrapping_sub(now) < 1000 {
            retval = sock.bind(ip, port);
        }
        if retval != 0 {
            // DEBUG_CRASH("Could not bind to 0x%8.8X:%d")
            self.udpsock = None;
            return false;
        }
        self.udpsock = Some(sock);
        for i in 0..MAX_MESSAGES as usize {
            self.out_buffer[i].length = 0;
            self.in_buffer[i].length = 0;
        }
        for i in 0..MAX_TRANSPORT_STATISTICS_SECONDS as usize {
            self.incoming_bytes[i] = 0;
            self.outgoing_bytes[i] = 0;
            self.unknown_bytes[i] = 0;
            self.incoming_packets[i] = 0;
            self.outgoing_packets[i] = 0;
            self.unknown_packets[i] = 0;
        }
        self.statistics_slot = 0;
        self.last_second = if plant(15090) { 0 } else { time_get_time() };
        self.port = port;
        true
    }

    /// `Transport::reset` (Transport.cpp): drops the socket.
    pub fn reset(&mut self) {
        self.udpsock = None;
    }

    /// The socket (for the host of a queued backend).
    pub fn socket_mut(&mut self) -> Option<&mut UDP> {
        self.udpsock.as_mut()
    }

    /// `Transport::update` (Transport.cpp): `doRecv` then `doSend`; false when either failed with
    /// `ADDRNOTAVAIL`.
    pub fn update(&mut self, now: u32) -> bool {
        let mut retval = true;
        if plant(15091) {
            self.do_send(now); // the other order: the send rolls the statistics slot before the receive counts into it
        }
        if !self.do_recv() && (plant(15088) || self.udpsock.as_ref().map_or(false, |s| s.get_status() == SockStat::AddrNotAvail)) {
            retval = false;
        }
        if !(if plant(15091) { true } else { self.do_send(now) }) && self.udpsock.as_ref().map_or(false, |s| s.get_status() == SockStat::AddrNotAvail) {
            retval = false;
        }
        retval
    }

    /// `Transport::doSend` (Transport.cpp): rolls the statistics slot each second, then writes every queued
    /// message; a message that was written leaves the queue.
    pub fn do_send(&mut self, now: u32) -> bool {
        if self.udpsock.is_none() {
            // DEBUG_LOG("Transport::doSend() - m_udpSock is NULL!")
            return false;
        }
        let mut retval = true;
        if if plant(15078) { self.last_second.wrapping_add(1000) <= now } else { self.last_second.wrapping_add(1000) < now } {
            self.last_second = now;
            self.statistics_slot = (self.statistics_slot + 1) % if plant(15079) { MAX_TRANSPORT_STATISTICS_SECONDS - 1 } else { MAX_TRANSPORT_STATISTICS_SECONDS };
            let s = self.statistics_slot as usize;
            self.outgoing_packets[s] = 0;
            self.outgoing_bytes[s] = 0;
            self.incoming_packets[s] = 0;
            self.incoming_bytes[s] = 0;
            self.unknown_packets[s] = 0;
            self.unknown_bytes[s] = 0;
        }
        for i in 0..MAX_MESSAGES as usize {
            if self.out_buffer[i].length != 0 {
                let image = message_image(&self.out_buffer[i]);
                let (addr, port) = (self.out_buffer[i].addr, self.out_buffer[i].port);
                let bytes_sent = match self.udpsock.as_mut() {
                    Some(s) => s.write(&image, addr, port),
                    None => -1,
                };
                if bytes_sent > 0 {
                    let s = self.statistics_slot as usize;
                    self.outgoing_packets[s] += 1;
                    self.outgoing_bytes[s] += (self.out_buffer[i].length as usize + if plant(15080) { 0 } else { HEADER_LEN }) as u32;
                    self.out_buffer[i].length = 0; // Remove from queue
                } else {
                    retval = false;
                    if plant(15087) {
                        self.out_buffer[i].length = 0;
                    }
                }
            }
        }
        // (debug-only: delivery of the delayed incoming messages, see the module doc)
        retval
    }

    /// `Transport::doRecv` (Transport.cpp): reads every waiting datagram, decrypts it, drops what is not a
    /// Generals packet (counted as unknown), and queues the rest in the first free `m_inBuffer` slot (dropped
    /// when the buffer is full). False if the socket reported an error (-1).
    pub fn do_recv(&mut self) -> bool {
        if self.udpsock.is_none() {
            // DEBUG_LOG("Transport::doRecv() - m_udpSock is NULL!")
            return false;
        }
        let mut retval = true;
        let mut buf = vec![0u8; MAX_MESSAGE_LEN];
        let mut len: i32;
        loop {
            let (n, from_addr, from_port) = match self.udpsock.as_mut() {
                Some(s) => s.read(&mut buf),
                None => (-1, 0, 0),
            };
            len = n;
            if len <= 0 {
                break;
            }
            let ulen = len as usize;
            // (debug-only packet loss here)
            decrypt_buf(&mut buf, ulen);
            let mut incoming = TransportMessage::default();
            incoming.length = len - HEADER_LEN as i32;
            store_image(&mut incoming, &buf[..ulen]);
            if (if plant(15081) { ulen < HEADER_LEN } else { ulen <= HEADER_LEN }) || !Transport::is_generals_packet(&incoming) {
                let s = self.statistics_slot as usize;
                self.unknown_packets[s] += 1;
                self.unknown_bytes[s] += (if plant(15082) { ulen - 1 } else { ulen }) as u32;
                continue;
            }
            let s = self.statistics_slot as usize;
            self.incoming_packets[s] += 1;
            self.incoming_bytes[s] += ulen as u32;
            for i in 0..MAX_MESSAGES as usize {
                if self.in_buffer[i].length == 0 {
                    self.in_buffer[i].length = incoming.length;
                    self.in_buffer[i].addr = if plant(15083) { from_port as u32 } else { from_addr };
                    self.in_buffer[i].port = from_port;
                    store_image(&mut self.in_buffer[i], &buf[..ulen]);
                    break;
                }
            }
            if plant(15089) {
                break;
            }
        }
        if len == -1 {
            retval = false;
        }
        retval
    }

    /// `Transport::queueSend` (Transport.cpp): puts the packet in the first free out slot with the magic number,
    /// the CRC of magic+data, and encrypted. False for a length outside `1..=MAX_PACKET_SIZE` or no free slot.
    pub fn queue_send(&mut self, addr: u32, port: u16, buf: &[u8], len: i32) -> bool {
        if (if plant(15076) { len < 0 } else { len < 1 }) || (if plant(15075) { len >= MAX_PACKET_SIZE } else { len > MAX_PACKET_SIZE }) {
            return false;
        }
        let n = len as usize;
        for i in 0..MAX_MESSAGES as usize {
            if self.out_buffer[i].length == 0 {
                let m = &mut self.out_buffer[i];
                m.length = len;
                m.data[..n].copy_from_slice(&buf[..n]);
                m.addr = addr;
                m.port = port;
                m.header.magic = GENERALS_MAGIC_NUMBER;
                let mut image = message_image(m);
                let mut crc = Crc::new();
                crc.compute_crc(&image[if plant(15077) { 6 } else { 4 }..]);
                m.header.crc = crc.get();
                image[0..4].copy_from_slice(&m.header.crc.to_le_bytes());
                let total = n + HEADER_LEN;
                encrypt_buf(&mut image, total);
                store_image(m, &image);
                return true;
            }
        }
        false
    }

    /// `Transport::isGeneralsPacket` (Transport.cpp): length in range, CRC of magic+data matches, magic is
    /// `GENERALS_MAGIC_NUMBER`.
    pub fn is_generals_packet(msg: &TransportMessage) -> bool {
        if msg.length < 0 || (if plant(15084) { msg.length >= MAX_MESSAGE_LEN as i32 } else { msg.length > MAX_MESSAGE_LEN as i32 }) {
            return false;
        }
        let image = message_image(msg);
        let mut crc = Crc::new();
        crc.compute_crc(&image[4..]);
        if crc.get() != msg.header.crc {
            return false;
        }
        if msg.header.magic != GENERALS_MAGIC_NUMBER {
            return false;
        }
        true
    }

    /// `allowBroadcasts`: false without a socket.
    pub fn allow_broadcasts(&mut self, val: bool) -> bool {
        match self.udpsock.as_mut() {
            None => false,
            Some(s) => s.allow_broadcasts(val) != 0,
        }
    }

    /// Differential-test accessor (#215): the statistics arrays (incoming bytes, unknown bytes, outgoing bytes, incoming packets, unknown
    /// packets, outgoing packets), the slot and `m_lastSecond`.
    #[doc(hidden)]
    pub fn debug_stats(&self) -> ([[u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize]; 6], i32, u32) {
        (
            [self.incoming_bytes, self.unknown_bytes, self.outgoing_bytes, self.incoming_packets, self.unknown_packets, self.outgoing_packets],
            self.statistics_slot,
            self.last_second,
        )
    }

    /// `setLatency` (see the module doc).
    pub fn set_latency(&mut self, val: bool) {
        self.use_latency = val;
    }

    /// `setPacketLoss` (see the module doc).
    pub fn set_packet_loss(&mut self, val: bool) {
        self.use_packet_loss = val;
    }

    fn per_second(&self, stats: &[u32; MAX_TRANSPORT_STATISTICS_SECONDS as usize]) -> f32 {
        let mut val = 0.0f32;
        for i in 0..MAX_TRANSPORT_STATISTICS_SECONDS as usize {
            if i as i32 != self.statistics_slot || plant(15086) {
                val += stats[i] as f32;
            }
        }
        val / (if plant(15085) { MAX_TRANSPORT_STATISTICS_SECONDS } else { MAX_TRANSPORT_STATISTICS_SECONDS - 1 }) as f32
    }

    /// `getIncomingBytesPerSecond`: the mean over the other 29 slots.
    pub fn get_incoming_bytes_per_second(&self) -> f32 {
        self.per_second(&self.incoming_bytes)
    }
    /// `getIncomingPacketsPerSecond`.
    pub fn get_incoming_packets_per_second(&self) -> f32 {
        self.per_second(&self.incoming_packets)
    }
    /// `getOutgoingBytesPerSecond`.
    pub fn get_outgoing_bytes_per_second(&self) -> f32 {
        self.per_second(&self.outgoing_bytes)
    }
    /// `getOutgoingPacketsPerSecond`.
    pub fn get_outgoing_packets_per_second(&self) -> f32 {
        self.per_second(&self.outgoing_packets)
    }
    /// `getUnknownBytesPerSecond`.
    pub fn get_unknown_bytes_per_second(&self) -> f32 {
        self.per_second(&self.unknown_bytes)
    }
    /// `getUnknownPacketsPerSecond`.
    pub fn get_unknown_packets_per_second(&self) -> f32 {
        self.per_second(&self.unknown_packets)
    }
}
