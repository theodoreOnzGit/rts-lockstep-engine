#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/Connection.cpp, Transport.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/Connection.h, Transport.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The network drafts' connection and transport (GitHub #215) against the C++: `src/drafts/network/connection.rs` (every member
//! function but the debug printer) and `transport.rs` (`queue_send`, `do_send`, `do_recv`, `update`, `is_generals_packet`, the six
//! statistics getters, `reset`, and `init_with_socket`'s tail).
//!
//! # Method
//!
//! `differential/wrap_draft_conn.cpp` compiles upstream's `Connection.cpp` whole and unchanged on the real `Transport.h`, with
//! `Transport.cpp` cut verbatim by build.rs (everything after `init`'s bind loop; one `S_un.S_addr` edit). Stand-ins: the UDP socket
//! (in-memory queues, `shim_network2/GameNetwork/udp.h`), `timeGetTime()` (a settable clock), the Transport constructor/destructor.
//! A program is a list of operations in words; both sides run it and write a log that is compared whole.
//!
//! - **connection** (`cn_run`): `init`, `reset`, `setUser`, `attachTransport`, `setFrameGrouping`, `sendNetCommandMsg` (commands that fit
//!   and commands that are wrapped), `clearCommandsExceptFrom`, `doSend` (against a transport whose slots can be filled so that
//!   `queueSend` fails), `processAck` (both overloads and the type dispatch), `setQuitting`, and the clock. After every operation the
//!   log holds the queue (every command with its relay and time last sent), the latency average and history, the retry counters, the
//!   times, and every packet in the transport's out buffer (length, address, port, CRC, magic, data hash).
//! - **transport** (`tr_run`): the same against the socket stand-in: `queueSend` of every length, datagrams injected (valid ones, bad CRC,
//!   bad magic, too short, empty), `doRecv`, `doSend`, `update`, with the socket failing, with full buffers, with no socket, `reset`,
//!   `isGeneralsPacket` on arbitrary messages, and the statistics getters.
//!
//! # Inputs left out (upstream undefined behaviour or Windows-only)
//!
//! - `doSend` with no user or no transport attached (upstream dereferences null); `processAck`, `clearCommandsExceptFrom`,
//!   `isQueueEmpty` and `doSend` before `init` (null command list). The drafts guard these; upstream crashes.
//! - Acks as commands to *send* (`FillBufferWithAckCommand` reads garbage, see tests/differential_draft_netpkt.rs); acks are only given to
//!   `processAck`.
//! - A clock that goes backwards: `processAck`'s `timeGetTime() - getTimeLastSent()` is a 32-bit subtraction on the shipped Windows build
//!   (the draft wraps in `u32`) and a 64-bit one on this Linux build of the C++, so the clock only moves forward and stays below 2^31.
//! - Datagrams over `MAX_MESSAGE_LEN` (a real socket reports an error there).

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::drafts::network::connection::Connection;
use rts_lockstep_engine::drafts::network::net_command_msg::{
    NetCommandMsg, NetCommandMsgPtr, NetCommandType, TransportMessage, MAX_MESSAGES, MAX_MESSAGE_LEN,
    MAX_TRANSPORT_STATISTICS_SECONDS,
};
use rts_lockstep_engine::drafts::network::net_command_ref::NetCommandRef;
use rts_lockstep_engine::drafts::network::transport::{encrypt_buf, Transport, TransportHandle};
use rts_lockstep_engine::drafts::network::udp::UDP;
use rts_lockstep_engine::drafts::network::user::User;
use rts_lockstep_engine::common::crc::Crc;
use std::sync::{Arc, RwLock};
use support::{Check, Gen};

extern "C" {
    fn cn_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn tr_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_run(which: u32, input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4_000_000];
    let n = unsafe {
        if which == 0 {
            cn_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32)
        } else {
            tr_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32)
        }
    };
    assert!(n as usize <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn show(what: &str, i: u64, r: &[u32], c: &[u32]) {
    let k = r.iter().zip(c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
    let k0 = k.saturating_sub(10);
    println!("{what} {i}: operation {} (counting from 0), word {} of that operation's log", r[..k].iter().filter(|&&w| w == 0xA5A5A5A5).count(), k - r[..k].iter().rposition(|&w| w == 0xA5A5A5A5).map_or(0, |x| x + 1));
    println!(
        "{what} {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}",
        r.len(),
        c.len(),
        &r[k0..(k + 10).min(r.len())],
        &c[k0..(k + 10).min(c.len())]
    );
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

fn put_bytes(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    let mut i = 0;
    while i < b.len() {
        let mut w = 0u32;
        for j in 0..4 {
            if i + j < b.len() {
                w |= (b[i + j] as u32) << (8 * j);
            }
        }
        v.push(w);
        i += 4;
    }
}

fn fnv(b: &[u8]) -> u32 {
    let mut h = 2166136261u32;
    for &c in b {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    h
}

fn push64(o: &mut Vec<u32>, v: i64) {
    o.push(v as u32);
    o.push(((v as u64) >> 32) as u32);
}

fn ascii(b: &[u8]) -> AsciiString {
    let mut a = AsciiString::default();
    a.set(b).expect("ascii");
    a
}

// ---------------------------------------------------------------------------------------------------------------
// the specs
// ---------------------------------------------------------------------------------------------------------------

/// `makeMsg` of the wrapper.
fn make_msg(r: &mut Rd) -> NetCommandMsg {
    let kind = r.next();
    let (player, frame, id) = (r.next(), r.next(), r.next());
    let ty = NetCommandType::from_i32(kind as i32).expect("kind");
    use NetCommandType as T;
    let mut m = match ty {
        T::AckBoth | T::AckStage1 | T::AckStage2 => {
            let mut m = match ty {
                T::AckBoth => NetCommandMsg::new_ack_both(),
                T::AckStage1 => NetCommandMsg::new_ack_stage1(),
                _ => NetCommandMsg::new_ack_stage2(),
            };
            m.set_command_id(r.next() as u16);
            m.set_original_player_id(r.next() as u8);
            m
        }
        T::FrameInfo => {
            let mut m = NetCommandMsg::new_frame();
            m.set_command_count(r.next() as u16);
            m
        }
        T::PlayerLeave => {
            let mut m = NetCommandMsg::new_player_leave();
            m.set_leaving_player_id(r.next() as u8);
            m
        }
        T::KeepAlive => NetCommandMsg::new_keep_alive(),
        T::Progress => {
            let mut m = NetCommandMsg::new_progress();
            m.set_percentage(r.next() as u8);
            m
        }
        T::File => {
            let mut m = NetCommandMsg::new_file();
            let nm = r.bytes();
            let d = r.bytes();
            m.set_portable_filename(ascii(&nm));
            m.set_file_data(&d);
            m
        }
        T::FrameResendRequest => {
            let mut m = NetCommandMsg::new_frame_resend_request();
            m.set_frame_to_resend(r.next());
            m
        }
        _ => NetCommandMsg::new(),
    };
    m.set_net_command_type(ty);
    m.set_player_id(player);
    m.set_execution_frame(frame);
    m.set_id(id as u16);
    m
}

fn describe(o: &mut Vec<u32>, m: &NetCommandMsg) {
    use NetCommandType as T;
    o.push(m.get_net_command_type() as i32 as u32);
    o.push(m.get_player_id());
    o.push(m.get_execution_frame());
    o.push(m.get_id() as u32);
    match m.get_net_command_type() {
        T::Wrapper => {
            o.push(m.get_wrapped_command_id() as u32);
            o.push(m.get_chunk_number());
            o.push(m.get_num_chunks());
            o.push(m.get_data_offset());
            o.push(m.get_total_data_length());
            o.push(m.get_wrapper_data_length());
            o.push(fnv(m.get_wrapper_data().unwrap_or(&[])));
        }
        T::File => {
            o.push(fnv(m.get_portable_filename().str()));
            o.push(m.get_file_length());
            o.push(fnv(m.get_file_data().unwrap_or(&[])));
        }
        T::FrameInfo => o.push(m.get_command_count() as u32),
        T::PlayerLeave => o.push(m.get_leaving_player_id() as u32),
        T::Progress => o.push(m.get_percentage() as u32),
        T::FrameResendRequest => o.push(m.get_frame_to_resend()),
        _ => {}
    }
}

fn describe_ref(o: &mut Vec<u32>, r: &NetCommandRef) {
    describe(o, &r.get_command().read());
    o.push(r.get_relay() as u32);
    push64(o, r.get_time_last_sent());
}

// ---------------------------------------------------------------------------------------------------------------
// connection
// ---------------------------------------------------------------------------------------------------------------

fn dump_out(o: &mut Vec<u32>, t: &Transport) {
    let n = t.out_buffer.iter().filter(|m| m.length != 0).count();
    o.push(n as u32);
    for (i, m) in t.out_buffer.iter().enumerate() {
        if m.length == 0 {
            continue;
        }
        o.push(i as u32);
        o.push(m.length as u32);
        o.push(m.addr);
        o.push(m.port as u32);
        o.push(m.header.crc);
        o.push(m.header.magic as u32);
        o.push(fnv(&m.data[..m.length as usize]));
    }
}

fn dump_connection(o: &mut Vec<u32>, c: &Connection) {
    let d = c.debug_state();
    o.push(c.is_quitting() as u32);
    o.push(d.quit_time);
    match c.get_user() {
        Some(u) => {
            o.push(1);
            o.push(u.get_ip_addr());
            o.push(u.get_port() as u32);
        }
        None => o.push(0),
    }
    match c.net_command_list() {
        Some(l) => {
            o.push(1 + l.length() as u32);
            let mut id = l.get_first_message();
            while let Some(i) = id {
                let r = l.get(i).expect("ref");
                describe_ref(o, r);
                id = r.get_next();
            }
        }
        None => o.push(0),
    }
    o.push(d.average_latency.to_bits());
    let mut h = 2166136261u32;
    for l in d.latencies.iter() {
        for k in 0..4 {
            h = (h ^ ((l.to_bits() >> (8 * k)) & 0xff)).wrapping_mul(16777619);
        }
    }
    o.push(h);
    o.push(d.num_retries as u32);
    push64(o, d.last_time_sent);
    push64(o, d.retry_metrics_time);
    push64(o, d.retry_time);
    push64(o, d.frame_grouping);
}

fn rust_conn(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let transport: TransportHandle = Arc::new(RwLock::new(Transport::new()));
    let mut c = Connection::new();
    let mut now = 0u32;
    let nops = r.next();
    for _ in 0..nops {
        let op = r.next();
        match op {
            0 => c.init(),
            1 => c.reset(),
            2 => {
                let mut u = User::new();
                u.set_ip_addr(r.next());
                u.set_port(r.next() as u16);
                c.set_user(Some(u));
            }
            3 => c.attach_transport(transport.clone()),
            4 => c.set_frame_grouping(r.next() as i32 as i64),
            5 => {
                let relay = r.next();
                let m = NetCommandMsgPtr::new(make_msg(&mut r));
                c.send_net_command_msg(&m, relay as u8);
                if std::env::var("CONN_DEBUG2").is_ok() {
                    let mut p = rts_lockstep_engine::drafts::network::net_packet::NetPacket::new();
                    p.reset();
                    let tr = NetCommandRef::new(m.clone());
                    println!("send relay {relay}: fits {} list {:?} quitting {}", p.add_command(&tr), c.net_command_list().map(|l| l.length()), c.is_quitting());
                }
            }
            6 => c.clear_commands_except_from(r.next() as i32),
            7 => o.push(c.do_send(now)),
            8 => {
                let (id, pl) = (r.next(), r.next());
                match c.process_ack(id as u16, pl as u8, now) {
                    Some(x) => {
                        o.push(1);
                        describe_ref(&mut o, &x);
                    }
                    None => o.push(0),
                }
            }
            9 => {
                let m = make_msg(&mut r);
                match c.process_ack_command(&m, now) {
                    Some(x) => {
                        o.push(1);
                        describe_ref(&mut o, &x);
                    }
                    None => o.push(0),
                }
            }
            10 => c.set_quitting(now),
            11 => now = r.next(),
            12 => {
                let n = r.next() as usize;
                let mut t = transport.write().unwrap();
                for i in 0..n.min(MAX_MESSAGES as usize) {
                    t.out_buffer[i].length = 1;
                }
            }
            13 => {
                let mut t = transport.write().unwrap();
                for m in t.out_buffer.iter_mut() {
                    m.length = 0;
                }
            }
            14 => {
                o.push(c.is_queue_empty() as u32);
                o.push(c.is_quitting() as u32);
                o.push(c.get_user().is_some() as u32);
            }
            _ => {}
        }
        o.push(0xA5A5A5A5);
        dump_connection(&mut o, &c);
        dump_out(&mut o, &transport.read().unwrap());
    }
    o
}

/// Command ids of one program are `ID_BASE + 0..5`: base 0, or near `CONNECTION_LATENCY_HISTORY_LENGTH` (200) so that the
/// latency history's `id % 200` wraps.
static ID_BASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn put_spec(v: &mut Vec<u32>, g: &mut Gen, kind: u32, big: bool) {
    let ids = |g: &mut Gen| ID_BASE.load(std::sync::atomic::Ordering::Relaxed) + g.below(5);
    v.push(kind);
    v.push(g.below(4)); // player
    v.push(g.below(6)); // frame
    v.push(ids(g)); // id
    match kind {
        0 | 1 | 2 => {
            v.push(ids(g));
            v.push(g.below(4));
        }
        3 => v.push(g.below(9)),
        5 => v.push(g.below(4)),
        14 => v.push(g.below(101)),
        18 => {
            let nm: Vec<u8> = (0..g.below(10)).map(|_| b'a' + g.below(26) as u8).collect();
            put_bytes(v, &nm);
            let n = if big { 400 + g.below(1800) } else { g.below(30) } as usize;
            let d: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
            put_bytes(v, &d);
        }
        21 => v.push(g.below(20)),
        _ => {}
    }
}

fn gen_conn(g: &mut Gen) -> Vec<u32> {
    let mut ops: Vec<u32> = Vec::new();
    let mut n = 0u32;
    let (mut inited, mut user, mut attached) = (false, false, false);
    let mut now = 0u32;
    let nops = 8 + g.below(60);
    ID_BASE.store(if g.below(3) == 0 { 195 + g.below(10) } else { 0 }, std::sync::atomic::Ordering::Relaxed);
    fn step(ops: &mut Vec<u32>, now: &mut u32, n: &mut u32, g: &mut Gen) {
        const T: [u32; 14] = [0, 1, 1, 2, 3, 1999, 2000, 2001, 9999, 10000, 10001, 29999, 30000, 30001];
        if g.below(10) < 8 {
            *now += if g.below(3) == 0 { g.below(40000) } else { T[g.below(14) as usize] };
            ops.push(11);
            ops.push(*now);
            *n += 1;
        }
    }
    let _unused = |g: &mut Gen| -> u32 {
        const T: [u32; 14] = [0, 1, 1, 2, 3, 1999, 2000, 2001, 9999, 10000, 10001, 29999, 30000, 30001];
        if g.below(3) == 0 {
            g.below(40000)
        } else {
            T[g.below(14) as usize]
        }
    };
    while n < nops {
        // before init: only the operations upstream defines on a connection with no command list
        let c = g.below(100);
        if !inited {
            match c {
                0..=29 => {
                    ops.push(0);
                    inited = true;
                    user = false;
                    attached = false;
                }
                30..=44 => {
                    ops.push(2);
                    ops.push(g.u32());
                    ops.push(g.u32());
                    user = true;
                }
                45..=54 => {
                    ops.push(3);
                    attached = true;
                }
                55..=64 => {
                    ops.push(4);
                    ops.push(g.below(6).wrapping_sub(1));
                }
                65..=79 => {
                    step(&mut ops, &mut now, &mut n, g);
                    ops.push(10);
                }
                80..=94 => {
                    ops.push(5);
                    ops.push(g.below(256));
                    let k = [3u32, 5, 9, 14, 21][g.below(5) as usize];
                    put_spec(&mut ops, g, k, false);
                }
                _ => {
                    ops.push(1);
                    inited = true;
                    user = false;
                    attached = false;
                }
            }
            n += 1;
            continue;
        }
        match c {
            0..=27 => {
                ops.push(5);
                ops.push(g.below(256));
                let kinds = [3u32, 5, 9, 14, 18, 21, 26, 3, 5, 18];
                let k = kinds[g.below(10) as usize];
                let big = k == 18 && g.below(4) == 0;
                put_spec(&mut ops, g, k, big);
            }
            28..=47 => {
                if user && attached {
                    step(&mut ops, &mut now, &mut n, g);
                    ops.push(7);
                } else {
                    ops.push(if user { 3 } else { 2 });
                    if user {
                        attached = true;
                    } else {
                        ops.push(g.u32());
                        ops.push(g.u32());
                        user = true;
                    }
                }
            }
            48..=57 => {
                step(&mut ops, &mut now, &mut n, g);
                ops.push(8);
                ops.push(ID_BASE.load(std::sync::atomic::Ordering::Relaxed) + g.below(5));
                ops.push(g.below(4));
            }
            58..=67 => {
                step(&mut ops, &mut now, &mut n, g);
                ops.push(9);
                let k = [0u32, 1, 2, 3, 0, 1][g.below(6) as usize];
                put_spec(&mut ops, g, k, false);
            }
            68..=71 => {
                ops.push(6);
                ops.push(g.below(5));
            }
            72..=76 => {
                step(&mut ops, &mut now, &mut n, g);
                ops.push(10);
            }
            77..=80 => {
                ops.push(4);
                ops.push(g.below(6).wrapping_sub(1));
            }
            81..=84 => {
                ops.push(12);
                ops.push(if g.below(2) == 0 { 128 } else { g.below(5) });
            }
            85..=89 => ops.push(13),
            90..=92 => ops.push(14),
            93..=95 => {
                ops.push(if g.below(2) == 0 { 0 } else { 1 });
                user = false;
                attached = false;
            }
            _ => {
                ops.push(2);
                ops.push(g.u32());
                ops.push(g.u32());
                user = true;
            }
        }
        n += 1;
    }
    let mut v = vec![0, n];
    v.extend(ops);
    v
}

#[test]
fn the_connection_matches_upstream() {
    let n = cases("CONN_CASES", 6000);
    let mut g = Gen::new(0x215_2001);
    let mut chk = Check::new("Connection operations");
    for i in 0..n {
        let v = gen_conn(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(0, &v);
        let r = rust_conn(&v);
        drop(_one);
        if r != c {
            show("connection case", i, &r, &c);
            if std::env::var("CONN_DEBUG").is_ok() {
                for (nm, w) in [("rust", &r), ("c++", &c)] {
                    let segs: Vec<&[u32]> = w.split(|&x| x == 0xA5A5A5A5).collect();
                    for (j, sg) in segs.iter().enumerate().take(18).skip(12) {
                        println!("  {nm} op {j}: {:?}", &sg[..sg.len().min(60)]);
                    }
                }
                println!("  program {:?}", &v[..v.len().min(120)]);
            }
        }
        chk.eq(r, c, || format!("connection case {i}"));
    }
    println!("{n} connection programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// transport
// ---------------------------------------------------------------------------------------------------------------

fn dump_transport(o: &mut Vec<u32>, t: &mut Transport, sent_seen: &mut usize) {
    dump_out(o, t);
    let n = t.in_buffer.iter().filter(|m| m.length != 0).count();
    o.push(n as u32);
    for (i, m) in t.in_buffer.iter().enumerate() {
        if m.length == 0 {
            continue;
        }
        o.push(i as u32);
        o.push(m.length as u32);
        o.push(m.addr);
        o.push(m.port as u32);
        o.push(m.header.crc);
        o.push(m.header.magic as u32);
        o.push(fnv(&m.data[..m.length as usize]));
    }
    let (arrs, slot, last_second) = t.debug_stats();
    let mut h = 2166136261u32;
    for a in arrs.iter() {
        for v in a.iter() {
            for k in 0..4 {
                h = (h ^ ((v >> (8 * k)) & 0xff)).wrapping_mul(16777619);
            }
        }
    }
    o.push(h);
    o.push(slot as u32);
    o.push(last_second);
    o.push(t.port as u32);
    match t.socket_mut() {
        Some(s) => {
            o.push(1);
            o.push(s.get_status() as i32 as u32);
            let q = s.queued_mut().expect("queued");
            o.push((q.outbound.len() - *sent_seen) as u32);
            for d in q.outbound.iter().skip(*sent_seen) {
                o.push(d.addr);
                o.push(d.port as u32);
                o.push(d.bytes.len() as u32);
                o.push(fnv(&d.bytes));
            }
            *sent_seen = q.outbound.len();
            o.push(q.inbound.len() as u32);
        }
        None => o.push(0),
    }
}

fn rust_transport(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let mut t = Transport::new();
    let mut sent_seen = 0usize;
    let mut now = 0u32;
    let nops = r.next();
    for _ in 0..nops {
        let op = r.next();
        match op {
            0 => {
                let port = r.next();
                now = r.next();
                sent_seen = 0;
                let ok = t.init_with_socket(UDP::new_queued(), 0x7f00_0001, port as u16, || now);
                assert!(ok);
            }
            1 => {
                let (a, p) = (r.next(), r.next());
                let b = r.bytes();
                o.push(t.queue_send(a, p as u16, &b, b.len() as i32) as u32);
            }
            2 => {
                let (a, p) = (r.next(), r.next());
                let b = r.bytes();
                if let Some(q) = t.socket_mut().and_then(|s| s.queued_mut()) {
                    q.inbound.push_back(rts_lockstep_engine::drafts::network::udp::Datagram { bytes: b, addr: a, port: p as u16 });
                }
            }
            3 => o.push(t.do_recv() as u32),
            4 => {
                now = r.next();
                o.push(t.do_send(now) as u32);
            }
            5 => {
                now = r.next();
                o.push(t.update(now) as u32);
            }
            6 => {
                let mut m = TransportMessage::default();
                m.length = r.next() as i32;
                m.header.crc = r.next();
                m.header.magic = r.next() as u16;
                let b = r.bytes();
                for (i, v) in b.iter().enumerate().take(MAX_MESSAGE_LEN) {
                    m.data[i] = *v;
                }
                o.push(Transport::is_generals_packet(&m) as u32);
            }
            7 => {
                o.push(t.get_incoming_bytes_per_second().to_bits());
                o.push(t.get_incoming_packets_per_second().to_bits());
                o.push(t.get_outgoing_bytes_per_second().to_bits());
                o.push(t.get_outgoing_packets_per_second().to_bits());
                o.push(t.get_unknown_bytes_per_second().to_bits());
                o.push(t.get_unknown_packets_per_second().to_bits());
            }
            8 => t.reset(),
            9 => {
                let s = r.next();
                if let Some(sock) = t.socket_mut() {
                    sock.set_failure_for_test(if s == 0 { None } else { Some(-(s as i32)) });
                }
            }
            10 => {
                let n = r.next() as usize;
                for i in 0..n.min(MAX_MESSAGES as usize) {
                    t.out_buffer[i].length = 1;
                }
            }
            11 => {
                let n = r.next() as usize;
                for i in 0..n.min(MAX_MESSAGES as usize) {
                    t.in_buffer[i].length = 1;
                }
            }
            12 => {
                for m in t.out_buffer.iter_mut() {
                    m.length = 0;
                }
                for m in t.in_buffer.iter_mut() {
                    m.length = 0;
                }
            }
            _ => {}
        }
        o.push(0xA5A5A5A5);
        dump_transport(&mut o, &mut t, &mut sent_seen);
    }
    o
}

/// A datagram as the wire has it: header (crc, magic) and data, the CRC over magic and data, then encrypted.
fn wire_packet(data: &[u8], good_crc: bool, good_magic: bool, g: &mut Gen) -> Vec<u8> {
    let mut img = Vec::new();
    img.extend_from_slice(&0u32.to_le_bytes());
    let magic: u16 = if good_magic { 0xF00D } else { g.below(65536) as u16 };
    img.extend_from_slice(&magic.to_le_bytes());
    img.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.compute_crc(&img[4..]);
    let c = if good_crc { crc.get() } else { crc.get() ^ (1 << g.below(32)) };
    img[0..4].copy_from_slice(&c.to_le_bytes());
    let n = img.len();
    encrypt_buf(&mut img, n);
    img
}

fn gen_transport(g: &mut Gen) -> Vec<u32> {
    let mut ops: Vec<u32> = Vec::new();
    let mut n = 0u32;
    let mut now = g.below(1000);
    let long = g.below(6) == 0;
    let nops = if long { 90 + g.below(60) } else { 6 + g.below(50) };
    let mut bytes = |g: &mut Gen, len: usize| -> Vec<u8> { (0..len).map(|_| g.below(256) as u8).collect() };
    let delta = |g: &mut Gen| -> u32 {
        const T: [u32; 8] = [0, 1, 999, 1000, 1001, 1002, 2500, 31000];
        if g.below(4) == 0 {
            g.below(5000)
        } else {
            T[g.below(8) as usize]
        }
    };
    if g.below(8) != 0 {
        ops.extend([0, 1 + g.below(65535), now]);
        n += 1;
    }
    while n < nops {
        if long && g.below(100) < 50 {
            now += 1001 + g.below(1500);
            ops.extend([4 + g.below(2), now]);
            n += 1;
            continue;
        }
        let c = g.below(100);
        match c {
            0..=21 => {
                let len = match g.below(8) {
                    0 => 0,
                    1 => 1,
                    2 => 476,
                    3 => 477,
                    4 => 475,
                    5 => 4 + 4 * g.below(10) as usize,
                    _ => g.below(480) as usize,
                };
                let a = if g.below(10) == 0 { 0 } else { g.u32() };
                let p = if g.below(10) == 0 { 0 } else { g.below(65536) };
                ops.extend([1, a, p]);
                let b = bytes(g, len);
                put_bytes(&mut ops, &b);
            }
            22..=44 => {
                // a datagram in
                let (a, p) = (g.u32(), g.below(65536));
                let len = match g.below(6) {
                    0 => 0,
                    1 => g.below(8) as usize,
                    _ => g.below(120) as usize,
                };
                let data = bytes(g, len);
                let img = match g.below(10) {
                    0 => wire_packet(&data, false, true, g),
                    1 => wire_packet(&data, true, false, g),
                    2 => {
                        let l = g.below(30) as usize;
                        bytes(g, l)
                    }
                    3 => wire_packet(&[], true, true, g),
                    4 => {
                        let l = MAX_MESSAGE_LEN - 6 - g.below(3) as usize;
                        let big = bytes(g, l);
                        wire_packet(&big, true, true, g)
                    }
                    _ => wire_packet(&data, true, true, g),
                };
                ops.extend([2, a, p]);
                put_bytes(&mut ops, &img);
            }
            45..=56 => ops.push(3),
            57..=66 => {
                now += delta(g);
                ops.extend([4, now]);
            }
            67..=72 => {
                now += delta(g);
                ops.extend([5, now]);
            }
            73..=79 => {
                let len = g.below(40);
                let data = bytes(g, len as usize);
                let (crc_ok, magic_ok) = (g.below(2) == 0, g.below(2) == 0);
                let img = wire_packet(&data, crc_ok, magic_ok, g);
                let length = match g.below(8) {
                    0 => g.below(2000) as i32 - 500,
                    1 => MAX_MESSAGE_LEN as i32 + 1,
                    2 => MAX_MESSAGE_LEN as i32,
                    _ => len as i32,
                };
                // decrypted header fields are what isGeneralsPacket reads: take them from the clear image
                let mut clear = img.clone();
                rts_lockstep_engine::drafts::network::transport::decrypt_buf(&mut clear, img.len());
                let crc = u32::from_le_bytes([clear[0], clear[1], clear[2], clear[3]]);
                let magic = u16::from_le_bytes([clear[4], clear[5]]) as u32;
                ops.extend([6, length as u32, crc, magic]);
                put_bytes(&mut ops, &data);
            }
            80..=83 => ops.push(7),
            84..=86 => ops.push(8),
            87..=90 => {
                ops.push(9);
                ops.push(if g.below(2) == 0 { 0 } else { [1u32, 7, 13][g.below(3) as usize] });
            }
            91..=93 => {
                ops.push(10);
                ops.push(if g.below(2) == 0 { 128 } else { g.below(4) });
            }
            94..=96 => {
                ops.push(11);
                ops.push(if g.below(2) == 0 { 128 } else { g.below(4) });
            }
            97..=98 => ops.push(12),
            _ => {
                ops.extend([0, 1 + g.below(65535), now]);
            }
        }
        n += 1;
    }
    let mut v = vec![1, n];
    v.extend(ops);
    let _ = MAX_TRANSPORT_STATISTICS_SECONDS;
    v
}

#[test]
fn the_transport_matches_upstream() {
    let n = cases("TRANSPORT_CASES", 6000);
    let mut g = Gen::new(0x215_2002);
    let mut chk = Check::new("Transport operations");
    for i in 0..n {
        let v = gen_transport(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(1, &v);
        let r = rust_transport(&v);
        drop(_one);
        if r != c {
            show("transport case", i, &r, &c);
        }
        chk.eq(r, c, || format!("transport case {i}"));
    }
    println!("{n} transport programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
