#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/NAT.cpp and Include/GameNetwork/NAT.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/nat.rs` against the C++ (GitHub #215, group network3).
//!
//! # Method
//!
//! `differential/wrap_draft_nat.cpp` compiles upstream's `NAT.cpp` from the connection-pair table to the end of the file (cut verbatim by build.rs, two
//! for-scope edits) on the real class declaration of `NAT.h` and the real `FirewallHelper.h`. The staging-room slots, the firewall helper, the peer queue, the
//! connection menu, the player-stats wait and the transport are stand-ins listed in the wrapper; this file's `Host` answers and logs identically, and the datagrams
//! the draft queues in its transport are read back (decrypted) and logged as the stand-in transport logs `queueSend`. A program is a configuration (clock, local slot,
//! port override and allocation delta, the eight slots, the mangler address and the script of mangler answers) and 20 to 70 operations on one NAT (opcodes at
//! `ref_nat_run`: attach, establish, update, the global messages, the individual members, datagrams put in the transport's in-buffer, state changes); after every
//! operation all the NAT's fields and the slots' addresses are written out and the two logs are compared whole. Also the connection-pair table and the timing constants.
//!
//! Every slot exists (in the game `GameInfo::getSlot` never returns NULL; `establishConnectionPaths` dereferences a NULL slot when it lists the players, so the draft's guards for
//! missing slots are not reachable and not compared).
//!
//! Inputs not run, because upstream indexes outside an array (undefined): a call that reads `m_connectionNodes[m_targetNodeNumber]` while the target is -1
//! (`processManglerResponse`, `sendMangledSourcePort`, `probed`, `gotMangledPort`, and the `PORT` and `PROBED` global messages); both sides log a marker instead.
//! Every program has at least two human slots (with fewer, `establishConnectionPaths` returns before it fills the node table, and the draft's -1 node slots differ
//! from the allocator's zeros), at most two slots with the Netgear bit when the paths are established (a third overruns the pair table), and attaches with the
//! local address 192.0.2.1 (TEST-NET-1) so that the draft's transport cannot bind a socket.
//!
//! The pair table has rounds 0 to 6: `CONNDONE` and `doThisConnectionRound` with the round at 7 or more read past it upstream, and are not run.
//!
//! Not compared: `transport.update`/`doSend` (socket calls: not observable without a socket), whether `attachSlotList` binds.

use rts_lockstep_engine::drafts::network::nat::*;
use rts_lockstep_engine::drafts::network::transport::decrypt_buf;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_nat_pairs(out: *mut c_int) -> c_int;
    fn ref_nat_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn fnv(h: u32, c: u8) -> u32 {
    (h ^ u32::from(c)).wrapping_mul(16777619)
}
fn hash_bytes(s: &[u8]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(h, c))
}
fn hash_str(s: &[u8]) -> u32 {
    let e = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    hash_bytes(&s[..e])
}
fn hash_units(s: &[u16]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(fnv(h, (c & 255) as u8), (c >> 8) as u8))
}

#[test]
fn connection_pairs_and_constants_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 7 * 7 * 8 + 16];
    let n = unsafe { ref_nat_pairs(out.as_mut_ptr()) } as usize;
    let mut chk = Check::new("nat connection pairs and constants");
    let mut k = 0;
    for a in 0..7 {
        for b in 0..7 {
            for c in 0..8 {
                chk.eq(CONNECTION_PAIRS[a][b][c], out[k], || format!("pairs[{a}][{b}][{c}]"));
                k += 1;
            }
        }
    }
    let consts = [
        TIME_BETWEEN_RETRIES as i32,
        MANGLER_RETRY_TIME_INTERVAL as i32,
        MAX_ALLOWED_MANGLER_RETRIES,
        KEEPALIVE_INTERVAL as i32,
        TIME_TO_WAIT_FOR_PORT as i32,
        TIME_FOR_ROUND_TIMEOUT as i32,
        MS_TO_WAIT_FOR_STATS as i32,
    ];
    for (i, c) in consts.iter().enumerate() {
        chk.eq(*c, out[k + i], || format!("constant {i}"));
    }
    assert_eq!(n, k + consts.len());
    chk.finish();
}

// ---------------------------------------------------------------------------------------------------------------
// the host
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct SlotCfg {
    exists: bool,
    human: bool,
    ip: u32,
    port: u16,
    nat: u32,
    name: Vec<u8>,
}

struct Host {
    now: u32,
    slots: Vec<SlotCfg>,
    local_slot: i32,
    port_override: u16,
    delta: i16,
    helper: bool,
    stats_mask: u32,
    mangler: u32,
    next_temp_port: u32,
    responses: Vec<u32>,
    response_at: usize,
    log: RefCell<Vec<u32>>,
}

impl Host {
    fn push(&self, v: u32) {
        self.log.borrow_mut().push(v);
    }
}

impl NatHost for Host {
    fn time_get_time(&self) -> u32 {
        self.now
    }
    fn slot_exists(&self, i: usize) -> bool {
        self.slots[i].exists
    }
    fn slot_is_human(&self, i: usize) -> bool {
        self.slots[i].human
    }
    fn slot_ip(&self, i: usize) -> u32 {
        self.slots[i].ip
    }
    fn slot_set_ip(&mut self, i: usize, ip: u32) {
        self.slots[i].ip = ip;
    }
    fn slot_port(&self, i: usize) -> u16 {
        self.slots[i].port
    }
    fn slot_set_port(&mut self, i: usize, port: u16) {
        self.slots[i].port = port;
    }
    fn slot_nat_behavior(&self, i: usize) -> u32 {
        self.slots[i].nat
    }
    fn slot_name_ascii(&self, i: usize) -> Vec<u8> {
        self.slots[i].name.clone()
    }
    fn local_slot_num(&self) -> i32 {
        self.local_slot
    }
    fn firewall_port_override(&self) -> u16 {
        self.port_override
    }
    fn firewall_port_allocation_delta(&self) -> i16 {
        self.delta
    }
    fn has_firewall_helper(&self) -> bool {
        self.helper
    }
    fn create_firewall_helper(&mut self) {
        self.push(76);
        self.helper = true;
    }
    fn delete_firewall_helper(&mut self) {
        self.push(77);
        self.helper = false;
    }
    fn firewall_flag_need_to_refresh(&mut self, flag: bool) {
        self.push(83);
        self.push(u32::from(flag));
    }
    fn firewall_get_mangler_response(&mut self, packet_id: u16) -> u16 {
        self.push(81);
        self.push(u32::from(packet_id));
        let v = self.responses.get(self.response_at).copied().unwrap_or(0);
        if self.response_at < self.responses.len() {
            self.response_at += 1;
        }
        v as u16
    }
    fn firewall_close_spare_socket(&mut self, port: u16) {
        self.push(82);
        self.push(u32::from(port));
    }
    fn firewall_send_to_mangler_from_port(&mut self, address: u32, port: u16, packet_id: u16) {
        self.push(80);
        self.push(address);
        self.push(u32::from(port));
        self.push(u32::from(packet_id));
    }
    fn firewall_next_temporary_source_port(&mut self) -> u16 {
        self.push(78);
        self.push(0);
        let v = self.next_temp_port;
        self.next_temp_port = self.next_temp_port.wrapping_add(1);
        v as u16
    }
    fn firewall_open_spare_socket(&mut self, port: u16) {
        self.push(79);
        self.push(u32::from(port));
    }
    fn resolve_first_mangler(&mut self) -> Option<u32> {
        if self.mangler == 0 {
            None
        } else {
            Some(self.mangler)
        }
    }
    fn peer_utm_player(&mut self, id: &str, nick: &[u8], options: &[u8]) {
        self.push(71);
        self.push(5);
        self.push(1);
        self.push(hash_str(id.as_bytes()));
        self.push(hash_str(nick));
        self.push(hash_str(options));
    }
    fn menu_init(&mut self) {
        self.push(72);
    }
    fn menu_end(&mut self) {
        self.push(73);
    }
    fn menu_set_player_name(&mut self, i: i32, name: &[u8]) {
        self.push(74);
        self.push(i as u32);
        let units: Vec<u16> = name.iter().map(|&c| u16::from(c)).collect();
        self.push(hash_units(&units));
    }
    fn menu_set_player_status(&mut self, i: i32, state: NatConnectionState) {
        self.push(75);
        self.push(i as u32);
        self.push(state as u32);
    }
    fn all_player_stats_arrived(&mut self) -> bool {
        for i in 0..8 {
            if self.slots[i].exists && self.slots[i].human && (self.stats_mask >> i) & 1 == 0 {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the program
// ---------------------------------------------------------------------------------------------------------------

const NAMES: &[&str] = &["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "X", "A B"];
const SHAPES: &[u32] = &[0, 1, 2, 4, 3, 6, 5, 16, 32, 64, 33, 34, 18, 0x1f, 0x7f];

fn push_bytes(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut w = 0u32;
        for (j, &x) in c.iter().enumerate() {
            w |= u32::from(x) << (8 * j);
        }
        v.push(w);
    }
}

fn ip(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => 0,
        1 => 0x7f000001,
        2 => 0xc0a80001 + g.below(4),
        3 => 0x0a000001 + g.below(4),
        4 => g.u32(),
        _ => 0xc0a80001 + g.below(2),
    }
}

fn nat_value(g: &mut Gen, allow_netgear: bool) -> u32 {
    let mut v = SHAPES[g.below(SHAPES.len() as u32) as usize];
    if !allow_netgear {
        v &= !8;
    }
    v
}

fn program(g: &mut Gen) -> Vec<u32> {
    let mut v = vec![];
    v.push(g.below(3_000_000)); // clock
    let local = g.below(8);
    v.push(local);
    v.push(if g.below(4) == 0 {
        if g.below(3) == 0 {
            1024
        } else {
            1024 + g.below(60_000)
        }
    } else {
        0
    }); // port override (1024 exactly is a boundary of processManglerResponse)
    v.push(match g.below(7) {
        0 => 0,
        1 => 100,
        2 => (-100i32) as u32,
        3 => 1 + g.below(300),
        4 => 1 + g.below(5),
        5 => (g.u32() & 0xffff) as i16 as i32 as u32,
        _ => 2,
    }); // delta
    v.push(g.below(2)); // helper present
    v.push(g.u32() & 0xff); // stats mask
    v.push(if g.below(4) == 0 { 0 } else { 0x0a000001 + g.below(1000) }); // mangler
    v.push(4096 + g.below(2000));
    let nr = g.below(5);
    v.push(nr);
    for _ in 0..nr {
        v.push(match g.below(4) {
            0 => 0,
            1 => 1000 + g.below(60_000),
            2 => 0x10000 + g.below(1000),
            _ => 1 + g.below(1500),
        });
    }
    // slots: at least two humans; at most two with the Netgear bit
    let mut human = [false; 8];
    let mut exists = [false; 8];
    let mut nhuman = 0;
    while nhuman < 2 {
        nhuman = 0;
        // every slot of the staging room exists (GameInfo::getSlot never returns NULL; establishConnectionPaths dereferences a NULL one when it lists the players), some are not human
        for i in 0..8 {
            exists[i] = true;
            let m = if g.below(3) == 0 { 6 } else { 2 };
            human[i] = g.below(m) != 0;
            if human[i] {
                nhuman += 1;
            }
        }
    }
    let mut netgear = 0;
    for i in 0..8 {
        v.push(u32::from(exists[i]));
        v.push(u32::from(human[i]));
        v.push(ip(g));
        v.push(if g.below(3) == 0 { 0 } else { 1024 + g.below(60_000) });
        let mut n = nat_value(g, true);
        if n & 8 != 0 {
            netgear += 1;
            // the Netgear pairing of establishConnectionPaths assigns every slot that has the bit (not only the humans) the first free node of round 0: a third one,
            // or one that is not a human, overruns the node table
            if netgear > 2 || !human[i] {
                n &= !8;
                netgear -= 1;
            }
        }
        v.push(n);
        push_bytes(&mut v, NAMES[i].as_bytes());
    }
    // the operations: attach, establish, then a random mix
    v.push(1);
    v.push(local);
    v.push(0xC000_0201); // 192.0.2.1 (TEST-NET-1): not a local address, so the draft's transport cannot bind a socket and send what it queues
    v.push(2);
    let existing: Vec<u32> = (0..8u32).filter(|&i| exists[i as usize]).collect();
    let pick_existing = |g: &mut Gen| existing[g.below(existing.len() as u32) as usize];
    let nodes = nhuman as u32;
    let nops = 20 + g.below(51);
    for _ in 0..nops {
        let op = match g.below(8) {
            0..=2 => 3,
            _ => 3 + g.below(32),
        };
        v.push(op);
        match op {
            4 => {
                v.push(g.below(8));
                let node = g.below(nodes.min(8));
                let text: String = match g.below(11) {
                    0 => format!("PROBED{node}"),
                    1 => format!("  PROBED{node}"),
                    2 => format!("CONNDONE{node} {}", g.below(8)),
                    3 => format!("CONNFAILED{node}"),
                    4 => format!("PORT{node} {} {:08X}", g.below(70_000), g.u32()),
                    5 => format!("PORT{node} {} {:X}", g.below(2000), 0xc0a80000 + g.below(10)),
                    6 => "junk".to_string(),
                    7 => format!("PORT{node}  {}  {:08x}", 1024 + g.below(60_000), g.u32()),
                    9 => format!("CONNDONE{node} -1"),
                    10 if nodes < 8 => format!("CONNDONE{nodes} -1"),
                    10 => "junk".to_string(),
                    _ => format!("CONNDONE{node} {}", node ^ 1),
                };
                push_bytes(&mut v, text.as_bytes());
            }
            5 => v.push(match g.below(5) {
                0 => g.below(100),
                1 => {
                    if g.below(2) == 0 {
                        500 // exactly the retry interval: the exact tie of `>=` in connectionUpdate
                    } else {
                        500 + g.below(1000)
                    }
                }
                2 => 5000 + g.below(20_000),
                _ => g.below(3000),
            }),
            6 => {
                v.push(g.below(128));
                v.push(match g.below(4) {
                    0 => 0,
                    1 => 10,
                    _ => g.below(60),
                });
                v.push(ip(g));
                v.push(1024 + g.below(60_000));
                let text: Vec<u8> = match g.below(4) {
                    0 => format!("PROBE{}", g.below(nodes.min(8))).into_bytes(),
                    1 => b"KEEPALIVE".to_vec(),
                    2 => format!("PROBE{}x", g.below(nodes.min(8))).into_bytes(),
                    _ => b"OTHER".to_vec(),
                };
                push_bytes(&mut v, &text);
            }
            34 => v.push(8 + g.below(5)),
            7 => {
                v.push(g.below(8));
                v.push(ip(g));
            }
            8 => {
                v.push(g.below(8));
                v.push(1024 + g.below(60_000));
            }
            9 => {
                v.push(g.below(8));
                v.push(nat_value(g, false) | if g.below(4) == 0 { 8 } else { 0 });
            }
            10 | 12 => v.push(g.below(8)),
            11 => v.push(g.below(8)),
            13 => v.push(if g.below(3) == 0 { 0 } else { 1024 + g.below(60_000) }),
            14 => v.push(match g.below(6) {
                0 => 0,
                1 => 100,
                2 => (-100i32) as u32,
                3 => 1 + g.below(300),
                4 => (g.u32() & 0xffff) as i16 as i32 as u32,
                _ => 2,
            }),
            15 => v.push(g.u32() & 0xff),
            16 => v.push(match g.below(4) {
                0 => 0,
                1 => 1000 + g.below(60_000),
                2 => 0x10000 + g.below(1000),
                _ => 1 + g.below(1500),
            }),
            17 => v.push(pick_existing(g)),
            18 => {
                v.push(1024 + g.below(60_000));
                v.push(pick_existing(g));
            }
            19 | 20 => v.push(g.below(nodes.min(8))),
            21 => {
                v.push(g.below(8));
                v.push(g.below(7));
            }
            22 => v.push(match g.below(4) {
                0 => g.below(70_000),
                1 => 1024 + g.below(60_000),
                2 => 65_000 + g.below(1000),
                _ => g.below(3000),
            }),
            26 => v.push(g.below(nodes.min(8))),
            27 => {
                v.push(g.below(nodes.min(8)));
                v.push(1024 + g.below(60_000));
            }
            28 => {
                v.push(g.below(nodes.min(8)));
                v.push(ip(g));
            }
            30 => v.push(g.below(7)),
            31 => {
                v.push(g.below(8));
                v.push(g.below(2));
            }
            32 => v.push(g.below(60_000)),
            33 => {
                v.push(ip(g));
                v.push(1024 + g.below(60_000));
                v.push(g.below(8));
            }
            _ => {}
        }
    }
    v
}

struct Rd<'a> {
    w: &'a [u32],
    i: usize,
}
impl Rd<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.next() as usize;
        let mut b = vec![0u8; n];
        let mut i = 0;
        while i < n {
            let v = self.next();
            for j in 0..4 {
                if i + j < n {
                    b[i + j] = (v >> (8 * j)) as u8;
                }
            }
            i += 4;
        }
        b
    }
}

fn dump(o: &mut Vec<u32>, a: &NAT<Host>) {
    o.push(0xD0D0D0D0);
    o.extend(a.debug_words());
    o.push(u32::from(a.transport.is_some()));
    o.push(u32::from(a.host.helper));
    for s in &a.host.slots {
        o.push(u32::from(s.exists));
        if s.exists {
            o.push(s.ip);
            o.push(u32::from(s.port));
        }
    }
}

fn drain_sent(a: &mut NAT<Host>, o: &mut Vec<u32>) {
    if let Some(t) = a.transport.as_mut() {
        for m in t.out_buffer.iter_mut() {
            if m.length != 0 {
                let n = m.length as usize;
                let mut image = Vec::with_capacity(6 + n);
                image.extend_from_slice(&m.header.crc.to_le_bytes());
                image.extend_from_slice(&m.header.magic.to_le_bytes());
                image.extend_from_slice(&m.data[..n]);
                let total = image.len();
                decrypt_buf(&mut image, total);
                o.extend([90, m.addr, u32::from(m.port), m.length as u32, hash_bytes(&image[6..])]);
                m.length = 0;
            }
        }
    }
}

fn rust_run(w: &[u32]) -> Vec<u32> {
    let mut rd = Rd { w, i: 0 };
    let now = rd.next();
    let local_slot = rd.next() as i32;
    let port_override = rd.next() as u16;
    let delta = rd.next() as i32 as i16;
    let helper = rd.next() != 0;
    let stats_mask = rd.next();
    let mangler = rd.next();
    let next_temp_port = rd.next();
    let nr = rd.next();
    let responses: Vec<u32> = (0..nr).map(|_| rd.next()).collect();
    let mut slots = vec![];
    for _ in 0..8 {
        let exists = rd.next() != 0;
        let human = rd.next() != 0;
        let ipv = rd.next();
        let port = rd.next() as u16;
        let nat = rd.next();
        let name = rd.bytes();
        slots.push(SlotCfg { exists, human, ip: ipv, port, nat, name });
    }
    let host = Host { now, slots, local_slot, port_override, delta, helper, stats_mask, mangler, next_temp_port, responses, response_at: 0, log: RefCell::new(Vec::new()) };
    let mut a = NAT::new(host);
    let mut o: Vec<u32> = Vec::new();
    dump(&mut o, &a);
    while rd.i < w.len() {
        let op = rd.next();
        o.push(0xA5A5A5A5);
        o.push(op);
        let mut res: Vec<u32> = Vec::new();
        match op {
            1 => {
                let ls = rd.next() as i32;
                let lip = rd.next();
                a.attach_slot_list(ls, lip);
            }
            2 => a.establish_connection_paths(),
            3 => {
                // update() at round 6 with all states settled advances to round 7; unless that finishes the negotiation it calls doThisConnectionRound at round 7 (past the table, undefined)
                let wd = a.debug_words();
                let next_round = a.debug_connection_round().wrapping_add(1);
                let needed = match wd[4] {
                    2 => 1,
                    3 | 4 => 3,
                    5 | 6 => 5,
                    7 | 8 => 7,
                    _ => u32::MAX,
                };
                if wd[0] == 1 && a.all_connections_done_this_round() && next_round >= 7 && needed == u32::MAX {
                    res.push(0xBAD0BAD2);
                } else {
                    res.push(a.update() as u32)
                }
            }
            4 => {
                let sn = rd.next() as i32;
                let text = rd.bytes();
                let t: &[u8] = &text;
                let mut s = 0;
                while s < t.len() && (t[s] == b' ' || (9..=13).contains(&t[s])) {
                    s += 1;
                }
                let p = &t[s..];
                if a.debug_target_node() < 0 && (p.starts_with(b"PORT") || p.starts_with(b"PROBED")) {
                    res.push(0xBAD0BAD0);
                } else if a.transport.is_none() && (p.starts_with(b"PORT") || p.starts_with(b"PROBED")) {
                    res.push(0xBAD0BAD1);
                } else if a.debug_connection_round() >= 7 && p.starts_with(b"CONNDONE") {
                    res.push(0xBAD0BAD2);
                } else {
                    a.process_global_message(sn, &text);
                }
            }
            5 => {
                let dt = rd.next();
                a.host.now = a.host.now.wrapping_add(dt);
            }
            6 => {
                let i = (rd.next() % 128) as usize;
                let len = rd.next() as i32;
                let addr = rd.next();
                let port = rd.next() as u16;
                let d = rd.bytes();
                if let Some(t) = a.transport.as_mut() {
                    let m = &mut t.in_buffer[i];
                    *m = Default::default();
                    m.length = len;
                    m.addr = addr;
                    m.port = port;
                    let n = d.len().min(m.data.len());
                    m.data[..n].copy_from_slice(&d[..n]);
                }
            }
            7 => {
                let i = (rd.next() % 8) as usize;
                a.host.slots[i].ip = rd.next();
            }
            8 => {
                let i = (rd.next() % 8) as usize;
                a.host.slots[i].port = rd.next() as u16;
            }
            9 => {
                let i = (rd.next() % 8) as usize;
                a.host.slots[i].nat = rd.next();
            }
            10 => res.push(a.get_slot_port(rd.next() as i32) as u32),
            11 => a.generate_port_numbers(rd.next() as i32),
            12 => a.host.local_slot = rd.next() as i32,
            13 => a.host.port_override = rd.next() as u16,
            14 => a.host.delta = rd.next() as i32 as i16,
            15 => a.host.stats_mask = rd.next(),
            16 => {
                let v = rd.next();
                a.host.responses.push(v);
            }
            17 => {
                let i = (rd.next() % 8) as usize;
                a.notify_target_of_probe(i);
            }
            18 => {
                let p = rd.next() as u16;
                let i = (rd.next() % 8) as usize;
                a.send_mangled_port_number_to_target(p, i);
            }
            19 => a.notify_users_of_connection_done(rd.next() as i32),
            20 => a.notify_users_of_connection_failed(rd.next() as i32),
            21 => {
                let node = rd.next() as i32;
                let st = match rd.next() {
                    0 => NatConnectionState::NoState,
                    1 => NatConnectionState::WaitingToBegin,
                    2 => NatConnectionState::WaitingForManglerResponse,
                    3 => NatConnectionState::WaitingForMangledPort,
                    4 => NatConnectionState::WaitingForResponse,
                    5 => NatConnectionState::Done,
                    _ => NatConnectionState::Failed,
                };
                a.set_connection_state(node, st);
            }
            22 => {
                let p = rd.next() as u16;
                if a.debug_target_node() < 0 {
                    res.push(0xBAD0BAD0);
                } else {
                    a.process_mangler_response(p);
                }
            }
            23 => {
                if a.debug_target_node() < 0 {
                    res.push(0xBAD0BAD0);
                } else {
                    a.send_mangled_source_port();
                }
            }
            24 => {
                if a.debug_connection_round() >= 7 {
                    res.push(0xBAD0BAD2);
                } else {
                    a.do_this_connection_round()
                }
            }
            25 => {
                if a.transport.is_none() {
                    res.push(0xBAD0BAD1);
                } else {
                    res.push(a.connection_update() as u32)
                }
            }
            26 => {
                let node = rd.next() as i32;
                if a.debug_target_node() < 0 {
                    res.push(0xBAD0BAD0);
                } else if a.transport.is_none() {
                    res.push(0xBAD0BAD1);
                } else {
                    a.probed(node);
                }
            }
            27 => {
                let node = rd.next() as i32;
                let p = rd.next() as u16;
                if a.debug_target_node() < 0 {
                    res.push(0xBAD0BAD0);
                } else if a.transport.is_none() {
                    res.push(0xBAD0BAD1);
                } else {
                    a.got_mangled_port(node, p);
                }
            }
            28 => {
                let node = rd.next() as i32;
                let ipv = rd.next();
                a.got_internal_address(node, ipv);
            }
            29 => {
                res.push(u32::from(a.all_connections_done()));
                res.push(u32::from(a.all_connections_done_this_round()));
            }
            30 => a.debug_set_connection_round(rd.next()),
            31 => {
                let i = (rd.next() % 8) as usize;
                let v = rd.next() != 0;
                a.debug_set_my_connection(i, v);
            }
            32 => a.debug_set_previous_source_port(rd.next() as u16),
            33 => {
                let ipv = rd.next();
                let port = rd.next() as u16;
                let node = rd.next() as i32;
                if a.transport.is_none() {
                    res.push(0xBAD0BAD1);
                } else {
                    a.send_a_probe(ipv, port, node);
                }
            }
            34 => a.debug_set_num_retries(rd.next() as i32),
            _ => {}
        }
        // the datagrams of the operation come after its host events and its result (the C++ stand-in transport collects them and appends them the same way)
        let ev: Vec<u32> = std::mem::take(&mut *a.host.log.borrow_mut());
        o.extend(ev);
        o.extend(res);
        drain_sent(&mut a, &mut o);
        dump(&mut o, &a);
    }
    o
}

fn cpp_run(w: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4_000_000];
    let n = unsafe { ref_nat_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

#[test]
fn nat_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("NAT_CASES", 10_000);
    let mut g = Gen::new(0x4a70_0001);
    let mut chk = Check::new("nat programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = cpp_run(&w);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(14);
            let opn = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!("case {i}: operation {opn}, first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k0..(k + 14).min(r.len())], &c[k0..(k + 14).min(c.len())]);
            if std::env::var("NAT_DEBUG").is_ok() {
                println!("program {:?}", w);
            }
        }
        chk.eq(r == c, true, || format!("nat case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
