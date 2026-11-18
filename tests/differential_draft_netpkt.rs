#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetPacket.cpp, NetCommandMsg.cpp, NetCommandRef.cpp,
//     NetCommandWrapperList.cpp, NetworkUtil.cpp, Transport.cpp (encryptBuf, decryptBuf)
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetPacket.h, NetCommandMsg.h, NetworkDefs.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The network drafts' packet bytes (GitHub #215) against the C++: `src/drafts/network/net_packet.rs` (every add, isRoomFor,
//! Get*Size, Fill*, read*, getCommandList, ConstructNetCommandMsgFromRawData, ConstructBigCommandPacketList),
//! `net_command_msg.rs`, `net_command_ref.rs`, `net_command_wrapper_list.rs`, `network_util.rs` and `transport.rs`'s
//! `encrypt_buf` and `decrypt_buf`.
//!
//! # Method
//!
//! `differential/wrap_draft_netpkt.cpp` compiles upstream's `NetPacket.cpp` (two for-scope/`NULL` edits in a copy),
//! `NetCommandMsg.cpp`, `NetCommandList.cpp`, `NetCommandWrapperList.cpp`, `NetworkUtil.cpp` unchanged, and Transport.cpp's
//! two crypt functions verbatim. A *spec* is a command in words (the encoding is at `makeMsg` in the wrapper); both sides
//! build the same messages from the same words, and write a log of words that is compared whole:
//!
//! - **pack** (op 1): a random sequence of commands of every type is packed the way `ConnectionManager` does. Per command: the
//!   size `GetBufferSizeNeededForCommand` says, the bytes `FillBufferWithCommand` writes (and so how far it writes), the
//!   `isRoomFor*` of its type, `addCommand`'s result, and what became of a command that did not fit (a new packet; the wrapper
//!   packets of `ConstructBigCommandPacketList`); then every packet's length, command count and bytes (up to the 476-byte
//!   limit).
//! - **parse** (op 2): the packets the pack produced are read back with `getCommandList` (the list in its sorted order,
//!   every command in the spec encoding, its relay, sort number and contents string). Because the pack logs are equal,
//!   the Rust packets are the C++ packets, so this reads each side's bytes with the other side's reader.
//! - **raw** (op 3): `ConstructNetCommandMsgFromRawData` of every `FillBufferWithCommand` output.
//! - **wrap** (op 4): a big command is split, each wrapper read back from its packet, fed in a shuffled order (with repeats) to a
//!   `NetCommandWrapperList`, the percentage asked after each, and `getReadyCommands` compared.
//! - **truncated** packets (op 2 on a prefix of a valid packet).
//! - **crypt** (op 5) and **util** (op 6).
//!
//! Acks are left out of the *fill* comparison: `FillBufferWithAckCommand` casts the `NetCommandRef *` itself to an ack message
//! and reads garbage (upstream defect, the draft reads the ack's fields instead; acks are never wrapped). Their packet bytes
//! (the add functions) are compared. Generated arguments keep a boolean to 0 or 1, a `WideChar` to 16 bits, strings that fit upstream's 260-byte stack buffers.

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::message_stream::{ArgumentData, ArgumentDataType, GameMessageArgument, MessageType};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::net_command_list::NetCommandList;
use rts_lockstep_engine::drafts::network::net_command_msg::{
    NetCommandBody, NetCommandMsg, NetCommandMsgPtr, NetCommandType, NetworkFrameLimits, TransportMessage,
};
use rts_lockstep_engine::drafts::network::net_command_ref::NetCommandRef;
use rts_lockstep_engine::drafts::network::net_command_wrapper_list::NetCommandWrapperList;
use rts_lockstep_engine::drafts::network::net_packet::NetPacket;
use rts_lockstep_engine::drafts::network::network_util;
use rts_lockstep_engine::drafts::network::transport::{decrypt_buf, encrypt_buf};
use support::{Check, Gen};

extern "C" {
    fn ref_np_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The C++ log of this input.
fn cpp_run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 400_000];
    let n = unsafe { ref_np_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

// ---------------------------------------------------------------------------------------------------------------
// the specs
// ---------------------------------------------------------------------------------------------------------------

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
    fn ustr(&mut self) -> UnicodeString {
        let n = self.next();
        let mut s: Vec<u16> = Vec::new();
        for _ in 0..n {
            let c = (self.next() & 0xffff) as u16;
            s.push(if c == 0 { 1 } else { c });
        }
        UnicodeString::try_from_chars(&s).expect("string")
    }
}

fn arg_words(t: u32) -> usize {
    match t {
        6 => 3,
        7 => 2,
        8 => 4,
        _ => 1,
    }
}

fn put_bytes(o: &mut Vec<u32>, b: &[u8]) {
    o.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut v = 0u32;
        for (j, x) in c.iter().enumerate() {
            v |= (*x as u32) << (8 * j);
        }
        o.push(v);
    }
}

fn put_ustr(o: &mut Vec<u32>, t: &UnicodeString) {
    o.push(t.get_length() as u32);
    for c in t.str() {
        o.push(*c as u32);
    }
}

fn ascii(b: &[u8]) -> AsciiString {
    let mut a = AsciiString::default();
    a.set(b).expect("ascii");
    a
}

/// `makeMsg` of the wrapper: the message and its relay.
fn make_msg(r: &mut Rd) -> (NetCommandMsg, u32) {
    let kind = r.next();
    let (player, frame, id) = (r.next(), r.next(), r.next());
    let relay = r.next();
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
        T::GameCommand => {
            let mut m = NetCommandMsg::new_game_command();
            m.set_game_message_type(MessageType(r.next() as i32));
            let n = r.next();
            for _ in 0..n {
                let t = r.next();
                let mut w = [0u32; 4];
                for q in 0..arg_words(t) {
                    w[q] = r.next();
                }
                m.add_argument(ArgumentDataType::from_i32(t as i32).expect("arg type"), ArgumentData(w));
            }
            m
        }
        T::PlayerLeave => {
            let mut m = NetCommandMsg::new_player_leave();
            m.set_leaving_player_id(r.next() as u8);
            m
        }
        T::RunAheadMetrics => {
            let mut m = NetCommandMsg::new_run_ahead_metrics();
            m.set_average_latency(f32::from_bits(r.next()));
            m.set_average_fps(r.next() as i32);
            m
        }
        T::RunAhead => {
            let mut m = NetCommandMsg::new_run_ahead(&NetworkFrameLimits::defaults());
            m.set_run_ahead(r.next() as u16);
            m.set_frame_rate(r.next() as u8);
            m
        }
        T::DestroyPlayer => {
            let mut m = NetCommandMsg::new_destroy_player();
            m.set_player_index(r.next());
            m
        }
        T::KeepAlive => NetCommandMsg::new_keep_alive(),
        T::DisconnectChat => {
            let mut m = NetCommandMsg::new_disconnect_chat();
            m.set_text(r.ustr());
            m
        }
        T::Chat => {
            let mut m = NetCommandMsg::new_chat();
            m.set_chat_player_mask(r.next() as i32);
            m.set_text(r.ustr());
            m
        }
        T::Progress => {
            let mut m = NetCommandMsg::new_progress();
            m.set_percentage(r.next() as u8);
            m
        }
        T::Wrapper => {
            let mut m = NetCommandMsg::new_wrapper();
            m.set_wrapped_command_id(r.next() as u16);
            m.set_chunk_number(r.next());
            m.set_num_chunks(r.next());
            m.set_data_offset(r.next());
            m.set_total_data_length(r.next());
            let b = r.bytes();
            m.set_wrapper_data(&b);
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
        T::FileAnnounce => {
            let mut m = NetCommandMsg::new_file_announce();
            let nm = r.bytes();
            m.set_portable_filename(ascii(&nm));
            m.set_file_id(r.next() as u16);
            m.set_file_player_mask(r.next() as u8);
            m
        }
        T::FileProgress => {
            let mut m = NetCommandMsg::new_file_progress();
            m.set_file_id(r.next() as u16);
            m.set_progress(r.next() as i32);
            m
        }
        T::FrameResendRequest => {
            let mut m = NetCommandMsg::new_frame_resend_request();
            m.set_frame_to_resend(r.next());
            m
        }
        T::DisconnectKeepAlive => NetCommandMsg::new_disconnect_keep_alive(),
        T::DisconnectPlayer => {
            let mut m = NetCommandMsg::new_disconnect_player();
            m.set_disconnect_slot(r.next() as u8);
            m.set_disconnect_frame(r.next());
            m
        }
        T::PacketRouterQuery => NetCommandMsg::new_packet_router_query(),
        T::PacketRouterAck => NetCommandMsg::new_packet_router_ack(),
        T::DisconnectVote => {
            let mut m = NetCommandMsg::new_disconnect_vote();
            m.set_vote_slot(r.next() as u8);
            m.set_vote_frame(r.next());
            m
        }
        T::DisconnectFrame => {
            let mut m = NetCommandMsg::new_disconnect_frame();
            m.set_disconnect_frame(r.next());
            m
        }
        T::DisconnectScreenOff => {
            let mut m = NetCommandMsg::new_disconnect_screen_off();
            m.set_new_frame(r.next());
            m
        }
        _ => NetCommandMsg::new(),
    };
    m.set_net_command_type(ty);
    m.set_player_id(player);
    m.set_execution_frame(frame);
    m.set_id(id as u16);
    (m, relay)
}

fn describe(o: &mut Vec<u32>, m: &NetCommandMsg, relay: u32) {
    o.push(m.get_net_command_type() as i32 as u32);
    o.push(m.get_player_id());
    o.push(m.get_execution_frame());
    o.push(m.get_id() as u32);
    o.push(relay);
    use NetCommandType as T;
    match m.get_net_command_type() {
        T::AckBoth | T::AckStage1 | T::AckStage2 => {
            o.push(m.get_command_id() as u32);
            o.push(m.get_original_player_id() as u32);
        }
        T::FrameInfo => o.push(m.get_command_count() as u32),
        T::GameCommand => {
            if let NetCommandBody::GameCommand(g) = &m.body {
                o.push(g.ty.0 as u32);
                o.push(g.args.len() as u32);
                for a in &g.args {
                    let t = a.ty as i32 as u32;
                    o.push(t);
                    for q in 0..arg_words(t) {
                        let mut v = a.data.0[q];
                        if t == 2 {
                            v &= 0xff;
                        }
                        if t == 10 {
                            v &= 0xffff;
                        }
                        o.push(v);
                    }
                }
            }
        }
        T::PlayerLeave => o.push(m.get_leaving_player_id() as u32),
        T::RunAheadMetrics => {
            o.push(m.get_average_latency().to_bits());
            o.push(m.get_average_fps() as u32);
        }
        T::RunAhead => {
            o.push(m.get_run_ahead() as u32);
            o.push(m.get_frame_rate() as u32);
        }
        T::DestroyPlayer => o.push(m.get_player_index()),
        T::DisconnectChat => put_ustr(o, &m.get_text()),
        T::Chat => {
            o.push(m.get_chat_player_mask() as u32);
            put_ustr(o, &m.get_text());
        }
        T::Progress => o.push(m.get_percentage() as u32),
        T::Wrapper => {
            o.push(m.get_wrapped_command_id() as u32);
            o.push(m.get_chunk_number());
            o.push(m.get_num_chunks());
            o.push(m.get_data_offset());
            o.push(m.get_total_data_length());
            put_bytes(o, m.get_wrapper_data().unwrap_or(&[]));
        }
        T::File => {
            put_bytes(o, m.get_portable_filename().str());
            put_bytes(o, m.get_file_data().unwrap_or(&[]));
        }
        T::FileAnnounce => {
            put_bytes(o, m.get_portable_filename().str());
            o.push(m.get_file_id() as u32);
            o.push(m.get_file_player_mask() as u32);
        }
        T::FileProgress => {
            o.push(m.get_file_id() as u32);
            o.push(m.get_progress() as u32);
        }
        T::FrameResendRequest => o.push(m.get_frame_to_resend()),
        T::DisconnectPlayer => {
            o.push(m.get_disconnect_slot() as u32);
            o.push(m.get_disconnect_frame());
        }
        T::DisconnectVote => {
            o.push(m.get_vote_slot() as u32);
            o.push(m.get_vote_frame());
        }
        T::DisconnectFrame => o.push(m.get_disconnect_frame()),
        T::DisconnectScreenOff => o.push(m.get_new_frame()),
        _ => {}
    }
    o.push(m.get_sort_number() as u32);
}

fn describe_ref(o: &mut Vec<u32>, r: &NetCommandRef) {
    let m = r.get_command().read();
    describe(o, &m, r.get_relay() as u32);
    put_bytes(o, &m.get_contents_as_ascii_string());
}

fn describe_list(o: &mut Vec<u32>, l: &NetCommandList) {
    o.push(l.length() as u32);
    let mut id = l.get_first_message();
    while let Some(i) = id {
        let r = l.get(i).expect("ref");
        describe_ref(o, r);
        id = r.get_next();
    }
}

fn emit_packet(o: &mut Vec<u32>, p: &NetPacket) {
    o.push(p.get_length() as u32);
    o.push(p.get_num_commands() as u32);
    put_bytes(o, p.get_data());
}

/// The arguments `constructGameMessage` would give the `GameMessage` (the wrapper's stand-in for it): the known data types, as
/// many as an `UnsignedByte` counts.
fn game_args(m: &NetCommandMsg) -> Vec<GameMessageArgument> {
    if let NetCommandBody::GameCommand(g) = &m.body {
        let mut v: Vec<GameMessageArgument> = g.args.iter().filter(|a| a.ty != ArgumentDataType::Unknown).copied().collect();
        // GameMessage::getArgumentCount() is an UnsignedByte: the parser and the fill loops read that many (the generator makes
        // up to 259 arguments, so this wraps)
        let count = v.len() & 0xff;
        v.truncate(count);
        v
    } else {
        Vec::new()
    }
}

/// `roomFor` of the wrapper: the `isRoomFor*` of the command's type; 2 for a type without.
fn room_for(p: &NetPacket, r: &NetCommandRef) -> u32 {
    use NetCommandType as T;
    let ty = r.get_command().read().get_net_command_type();
    let b = match ty {
        T::GameCommand => {
            let a = game_args(&r.get_command().read());
            p.is_room_for_game_message(r, &a)
        }
        T::AckStage1 | T::AckStage2 | T::AckBoth => p.is_room_for_ack_message(r),
        T::FrameInfo => p.is_room_for_frame_message(r),
        T::PlayerLeave => p.is_room_for_player_leave_message(r),
        T::RunAheadMetrics => p.is_room_for_run_ahead_metrics_message(r),
        T::RunAhead => p.is_room_for_run_ahead_message(r),
        T::DestroyPlayer => p.is_room_for_destroy_player_message(r),
        T::KeepAlive => p.is_room_for_keep_alive_message(r),
        T::DisconnectKeepAlive => p.is_room_for_disconnect_keep_alive_message(r),
        T::DisconnectPlayer => p.is_room_for_disconnect_player_message(r),
        T::PacketRouterQuery => p.is_room_for_packet_router_query_message(r),
        T::PacketRouterAck => p.is_room_for_packet_router_ack_message(r),
        T::DisconnectChat => p.is_room_for_disconnect_chat_message(r),
        T::DisconnectVote => p.is_room_for_disconnect_vote_message(r),
        T::Chat => p.is_room_for_chat_message(r),
        T::Progress => p.is_room_for_progress_message(r),
        T::LoadComplete => p.is_room_for_load_complete_message(r),
        T::TimeoutStart => p.is_room_for_time_out_game_start_message(r),
        T::Wrapper => p.is_room_for_wrapper_message(r),
        T::File => p.is_room_for_file_message(r),
        T::FileAnnounce => p.is_room_for_file_announce_message(r),
        T::FileProgress => p.is_room_for_file_progress_message(r),
        T::DisconnectFrame => p.is_room_for_disconnect_frame_message(r),
        T::DisconnectScreenOff => p.is_room_for_disconnect_screen_off_message(r),
        T::FrameResendRequest => p.is_room_for_frame_resend_request_message(r),
        _ => return 2,
    };
    b as u32
}

fn is_ack(t: NetCommandType) -> bool {
    matches!(t, NetCommandType::AckBoth | NetCommandType::AckStage1 | NetCommandType::AckStage2)
}

fn make_ref(r: &mut Rd) -> NetCommandRef {
    let (m, relay) = make_msg(r);
    let ptr = NetCommandMsgPtr::new(m);
    let mut rf = NetCommandRef::new(ptr.clone());
    rf.set_relay(relay as u8);
    let _ = ptr.write().detach();
    rf
}

/// What the Rust side leaves for the cross reads: the packets it made and the bytes of each fill.
#[derive(Default)]
struct Made {
    /// Per packet: it holds a command with a length-prefixed field (chat, disconnect chat, wrapper, file): see the truncated reads.
    var: Vec<bool>,
    packets: Vec<Vec<u8>>,
    fills: Vec<Vec<u8>>,
}

fn rust_pack(input: &[u32], made: &mut Made) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let n = r.next();
    let mut cur = NetPacket::new();
    let mut cur_var = false;
    let is_var = |t: NetCommandType| matches!(t, NetCommandType::Chat | NetCommandType::DisconnectChat | NetCommandType::Wrapper | NetCommandType::File);
    for _ in 0..n {
        let rf = make_ref(&mut r);
        let ty = rf.get_command().read().get_net_command_type();
        let size = NetPacket::get_buffer_size_needed_for_command(&rf.get_command().read());
        o.push(size);
        if is_ack(ty) {
            o.push(0xFFFF_FFFF);
        } else {
            let mut buf = Vec::new();
            NetPacket::fill_buffer_with_command(&mut buf, &rf);
            put_bytes(&mut o, &buf);
            made.fills.push(buf);
        }
        o.push(room_for(&cur, &rf));
        let ok = cur.add_command(&rf);
        o.push(ok as u32);
        if ok && is_var(ty) {
            cur_var = true;
        }
        if !ok {
            let wrap_it;
            if cur.get_num_commands() > 0 {
                emit_packet(&mut o, &cur);
                made.packets.push(cur.get_data().to_vec());
                made.var.push(cur_var);
                cur = NetPacket::new();
                cur_var = false;
                let ok2 = cur.add_command(&rf);
                o.push(ok2 as u32);
                if ok2 && is_var(ty) {
                    cur_var = true;
                }
                wrap_it = !ok2;
            } else {
                wrap_it = true;
            }
            if wrap_it {
                if network_util::does_command_require_a_command_id(ty) {
                    let l = NetPacket::construct_big_command_packet_list(&rf);
                    o.push(0xB1C0);
                    o.push(l.len() as u32);
                    for p in &l {
                        emit_packet(&mut o, p);
                        made.packets.push(p.get_data().to_vec());
                        made.var.push(true);
                    }
                } else {
                    o.push(0xDEAD);
                }
            }
        }
    }
    o.push(0xE0D0);
    if cur.get_num_commands() > 0 {
        emit_packet(&mut o, &cur);
        made.packets.push(cur.get_data().to_vec());
        made.var.push(cur_var);
    } else {
        o.push(0xFFFF_FFFF);
    }
    o
}

fn bytes_input(op: u32, b: &[u8]) -> Vec<u32> {
    let mut v = vec![op];
    put_bytes(&mut v, b);
    v
}

fn transport_of(b: &[u8]) -> TransportMessage {
    let mut tm = TransportMessage::default();
    tm.data[..b.len()].copy_from_slice(b);
    tm.length = b.len() as i32;
    tm.addr = 0x7f00_0001;
    tm.port = 8088;
    tm
}

fn rust_parse(b: &[u8]) -> Vec<u32> {
    let p = NetPacket::from_transport_message(&transport_of(b));
    let mut o = vec![p.get_num_commands() as u32, p.get_addr(), p.get_port() as u32];
    describe_list(&mut o, &p.get_command_list());
    o
}

fn rust_raw(b: &[u8]) -> Vec<u32> {
    match NetPacket::construct_net_command_msg_from_raw_data(b, b.len() as u32) {
        None => vec![0],
        Some(r) => {
            let mut o = vec![1];
            describe_ref(&mut o, &r);
            o
        }
    }
}

fn rust_wrap(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let n_order = r.next() as usize;
    let order: Vec<u32> = (0..n_order).map(|_| r.next()).collect();
    let rf = make_ref(&mut r);
    let wrapped_id = rf.get_command().read().get_id() as u16;
    let l = NetPacket::construct_big_command_packet_list(&rf);
    let mut o = vec![l.len() as u32];
    let mut lists = Vec::new();
    for p in &l {
        emit_packet(&mut o, p);
        let q = NetPacket::from_transport_message(&transport_of(p.get_data()));
        lists.push(q.get_command_list());
    }
    let mut wl = NetCommandWrapperList::new();
    o.push(wl.get_percent_complete(wrapped_id) as u32);
    if !l.is_empty() {
        for k in &order {
            let idx = *k as usize % l.len();
            let first = lists[idx].get_first_message();
            let Some(f) = first else {
                o.push(0xFFFF_FFFF);
                continue;
            };
            let wr = lists[idx].get(f).expect("ref");
            let wid = wr.get_command().read().get_wrapped_command_id();
            wl.process_wrapper(wr);
            o.push(wl.get_percent_complete(wid) as u32);
        }
    }
    let rdy = wl.get_ready_commands();
    describe_list(&mut o, &rdy);
    o.push(wl.get_percent_complete(wrapped_id) as u32);
    o
}

fn rust_crypt(which: u32, b: &[u8]) -> Vec<u32> {
    let mut buf = b.to_vec();
    buf.extend_from_slice(&[0; 4]);
    if which == 0 {
        encrypt_buf(&mut buf, b.len());
    } else {
        decrypt_buf(&mut buf, b.len());
    }
    let mut o = Vec::new();
    put_bytes(&mut o, &buf[..b.len()]);
    o
}

fn rust_util(n: u32) -> Vec<u32> {
    let mut o = Vec::new();
    for t in -1..=(NetCommandType::Max as i32 + 1) {
        let ty = NetCommandType::from_i32(t).unwrap_or(NetCommandType::Unknown);
        let mut m = NetCommandMsg::new();
        m.set_net_command_type(ty);
        o.push(network_util::does_command_require_a_command_id(ty) as u32);
        o.push(network_util::command_requires_ack(&m) as u32);
        o.push(network_util::command_requires_direct_send(&m) as u32);
        o.push(network_util::is_command_synchronized(ty) as u32);
        put_bytes(&mut o, network_util::get_ascii_net_command_type(ty).as_bytes());
    }
    for _ in 0..n {
        o.push(network_util::generate_next_command_id() as u32);
    }
    o
}

// ---------------------------------------------------------------------------------------------------------------
// the generator
// ---------------------------------------------------------------------------------------------------------------

/// The header values the next command repeats or changes, so that the packet's field compression (a field is written only
/// when it changed, the command id when it is not last + 1, the repeat of acks and frames) is exercised.
struct Ctx {
    player: u32,
    frame: u32,
    relay: u32,
    id: u32,
    ack_kind: u32,
    ack_orig: u32,
    ack_cmd: u32,
}

fn rand_bytes(g: &mut Gen, n: usize, text: bool) -> Vec<u8> {
    (0..n)
        .map(|_| {
            if text {
                b"abcdefghijklmnopqrstuvwxyz./_-0123456789"[g.below(40) as usize]
            } else {
                g.below(256) as u8
            }
        })
        .collect()
}

fn gen_args(g: &mut Gen, o: &mut Vec<u32>, scale: u32) {
    let n = match g.below(20) {
        0 => 0,
        1..=12 => g.below(6),
        13..=16 => g.below(24),
        17 | 18 => 20 + g.below(60 * scale),
        _ => 100 + g.below(155),
    };
    o.push(g.below(2010 + 40)); // message type: a named one, or past MSG_COUNT
    let count_at = o.len();
    o.push(0);
    let mut written = 0;
    while written < n {
        let t = if g.below(40) == 0 { 11 } else { g.below(11) };
        let run = (1 + g.below(5)).min(n - written);
        for _ in 0..run {
            o.push(t);
            for _ in 0..arg_words(t) {
                let v = match t {
                    2 => g.below(2),
                    10 => g.below(65536),
                    1 | 6 => {
                        if g.below(8) == 0 {
                            g.u32()
                        } else {
                            g.uniform(-3000.0, 3000.0).to_bits()
                        }
                    }
                    _ => {
                        if g.below(3) == 0 {
                            g.below(300)
                        } else {
                            g.u32()
                        }
                    }
                };
                o.push(v);
            }
            written += 1;
        }
    }
    o[count_at] = n;
}

fn gen_string(g: &mut Gen, o: &mut Vec<u32>) {
    let n = match g.below(12) {
        0 => 0,
        1..=8 => g.below(30),
        9 | 10 => g.below(120),
        _ => 200 + g.below(101),
    };
    o.push(n);
    for _ in 0..n {
        o.push(if g.below(4) == 0 { 0x100 + g.below(0xff00) } else { 32 + g.below(95) });
    }
}

fn gen_bytes(g: &mut Gen, o: &mut Vec<u32>, max: u32, text: bool) {
    let n = g.below(max + 1) as usize;
    put_bytes(o, &rand_bytes(g, n, text));
}

/// One command, appended to `o`.
fn gen_cmd(g: &mut Gen, c: &mut Ctx, o: &mut Vec<u32>, big: bool, allow_lc: bool) {
    gen_cmd_kind(g, c, o, big, allow_lc, u32::MAX)
}

/// [`gen_cmd`] with the kind forced (`u32::MAX`: chosen at random).
fn gen_cmd_kind(g: &mut Gen, c: &mut Ctx, o: &mut Vec<u32>, big: bool, allow_lc: bool, force: u32) {
    // kinds with an add function, weighted to the common ones
    const KINDS: [u32; 38] = [
        4, 4, 4, 4, 4, 0, 1, 2, 3, 3, 5, 6, 7, 8, 9, 10, 11, 11, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 28, 29, 12, 13, 22,
        30, 31,
    ];
    let mut kind = if big { [4u32, 4, 11, 18, 10][g.below(5) as usize] } else { KINDS[g.below(KINDS.len() as u32) as usize] };
    if (kind == 15 || kind == 16) && !allow_lc {
        kind = 14; // see gen_pack_case
    }
    if force != u32::MAX {
        kind = force;
    }
    // header: repeat or change
    if g.below(10) < 2 {
        c.player = if g.below(100) == 0 { 256 + g.below(10) } else { g.below(8) };
        c.id = g.below(65536);
    }
    if g.below(10) < 3 {
        c.frame = if g.below(2) == 0 { c.frame + 1 } else { g.below(100_000) };
    }
    if g.below(10) < 2 {
        c.relay = g.below(256);
    }
    if g.below(10) < 7 {
        c.id = (c.id + 1) & 0xffff;
    } else if g.below(2) == 0 {
        c.id = g.below(65536);
    }
    o.push(kind);
    o.push(c.player);
    o.push(c.frame);
    o.push(c.id);
    o.push(c.relay);
    match kind {
        0 | 1 | 2 => {
            if c.ack_kind == kind && g.below(10) < 6 {
                c.ack_cmd = (c.ack_cmd + 1) & 0xffff;
            } else {
                c.ack_kind = kind;
                c.ack_cmd = g.below(65536);
                c.ack_orig = g.below(8);
            }
            o.push(c.ack_cmd);
            o.push(c.ack_orig);
        }
        3 => o.push(if g.below(2) == 0 { 0 } else { g.below(70000) }),
        4 => gen_args(g, o, if big { 4 } else { 1 }),
        5 => o.push(g.below(300)),
        6 => {
            o.push(if g.below(6) == 0 { g.u32() } else { g.uniform(0.0, 500.0).to_bits() });
            o.push(g.u32() % 300);
        }
        7 => {
            o.push(g.below(70000));
            o.push(g.below(300));
        }
        8 => o.push(g.below(10)),
        10 => gen_string(g, o),
        11 => {
            o.push(g.u32());
            gen_string(g, o);
        }
        14 => o.push(g.below(300)),
        17 => {
            o.push(g.below(70000));
            o.push(g.below(10));
            o.push(g.below(10));
            o.push(g.below(1000));
            o.push(g.below(5000));
            let mx = if g.below(5) == 0 { 400 } else { 60 };
            gen_bytes(g, o, mx, false);
        }
        18 => {
            let n = 1 + g.below(40) as usize;
            put_bytes(o, &rand_bytes(g, n, true));
            let mx = if big || g.below(4) == 0 { 900 } else { 120 };
            gen_bytes(g, o, mx, false);
        }
        19 => {
            let n = 1 + g.below(40) as usize;
            put_bytes(o, &rand_bytes(g, n, true));
            o.push(g.below(70000));
            o.push(g.below(300));
        }
        20 => {
            o.push(g.below(70000));
            o.push(g.u32());
        }
        21 | 28 | 29 => o.push(g.u32()),
        24 | 27 => {
            o.push(g.below(300));
            o.push(g.u32());
        }
        _ => {}
    }
}

fn gen_pack_case(g: &mut Gen) -> Vec<u32> {
    let mut c = Ctx { player: g.below(8), frame: g.below(1000), relay: g.below(256), id: g.below(65536), ack_kind: 99, ack_orig: 0, ack_cmd: 0 };
    let n = match g.below(10) {
        0 => 1,
        1..=6 => 2 + g.below(14),
        _ => 10 + g.below(50),
    };
    let mut v = vec![1, n];
    // Load-complete and time-out-start messages (15, 16) overrun upstream's packet when it is within 3 bytes of full (the
    // known defect: the_known_upstream_overruns), which leaves the C++ heap corrupted; they are generated only among the first
    // four commands and only while every command so far is a small one, so the packet is far from full. The probe covers the
    // full-packet case.
    let mut small_so_far = true;
    for k in 0..n {
        let b = g.below(25) == 0;
        let at = v.len();
        gen_cmd(g, &mut c, &mut v, b, k < 4 && small_so_far);
        if matches!(v[at], 4 | 10 | 11 | 17 | 18) {
            small_so_far = false;
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------------------------
// the tests
// ---------------------------------------------------------------------------------------------------------------

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Prints the first difference of two logs.
fn show(what: &str, i: u64, r: &[u32], c: &[u32]) {
    let k = r.iter().zip(c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
    let k0 = k.saturating_sub(8);
    println!(
        "{what} {i}: first difference at word {k} (lengths rust {} c++ {}): rust {:?} c++ {:?}",
        r.len(),
        c.len(),
        &r[k0.min(r.len())..(k + 8).min(r.len())],
        &c[k0.min(c.len())..(k + 8).min(c.len())]
    );
}

#[test]
fn packets_are_packed_and_read_as_upstream() {
    let n = cases("NETPKT_CASES", 6000);
    let mut g = Gen::new(0x215_0001);
    let mut pack = Check::new("pack: sizes, fills, room, add results and packets");
    let mut parse = Check::new("parse: getCommandList of every packet");
    let mut raw = Check::new("raw: ConstructNetCommandMsgFromRawData of every fill");
    let mut trunc = Check::new("parse of truncated packets");
    let (mut cmds, mut packets, mut wrapped, mut full, mut truncs, mut fills, mut overruns) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut kinds = [0u64; 40];
    for i in 0..n {
        let input = gen_pack_case(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&input);
        let mut made = Made::default();
        let r = rust_pack(&input, &mut made);
        drop(_one);
        cmds += input[1] as u64;
        if made.packets.iter().any(|p| p.len() > 476) {
            // a load-complete or time-out-start message was added past the 476 bytes (isRoomFor does not count its C field):
            // upstream's write runs over m_packet into m_packetLen, so the C++ state after it is corrupted memory. See
            // the_known_upstream_overruns.
            overruns += 1;
            continue;
        }
        let same = r == c;
        pack.eq(r.clone(), c.clone(), || format!("pack case {i}"));
        if !same {
            show("pack case", i, &r, &c);
            if std::env::var("NETPKT_DUMP").is_ok() {
                let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(0);
                let (mut idx, mut cmd) = (0usize, 0usize);
                while idx < k {
                    let start = idx;
                    idx += 1;
                    if c[idx] == 0xFFFF_FFFF {
                        idx += 1;
                    } else {
                        idx += 1 + (c[idx] as usize + 3) / 4;
                    }
                    println!("  [cmd {cmd}: words {start}..{}: size {} room rust {} c++ {} ok {}]", idx + 2, c[start], r[idx], c[idx], c[idx + 1]);
                    idx += 2;
                    cmd += 1;
                }
                let mut rd = Rd { w: &input, i: 2 };
                for k in 0..input[1] {
                    let (m, relay) = make_msg(&mut rd);
                    println!("  cmd {k}: {:?} player {} frame {} id {} relay {} {}", m.get_net_command_type(), m.get_player_id(), m.get_execution_frame(), m.get_id(), relay, if let NetCommandBody::Chat { text, .. } = &m.body { format!("chat len {}", text.get_length()) } else { String::new() });
                }
            }
            continue;
        }
        wrapped += c.iter().filter(|&&w| w == 0xB1C0).count() as u64;
        packets += made.packets.len() as u64;
        full += made.packets.iter().filter(|p| p.len() > 440).count() as u64;
        fills += made.fills.len() as u64;
        for (pi, p) in made.packets.iter().enumerate() {
            let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
            let cp = cpp_run(&bytes_input(2, p));
            let rp = rust_parse(p);
            drop(_one);
            if rp != cp {
                show("parse case", i, &rp, &cp);
            }
            parse.eq(rp, cp, || format!("parse of a packet of case {i}"));
            if p.len() > 4 && !made.var[pi] && g.below(2) == 0 {
                let cut = 1 + g.below(p.len() as u32 - 1) as usize;
                truncs += 1;
                let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
                let cp = cpp_run(&bytes_input(2, &p[..cut]));
                let rp = rust_parse(&p[..cut]);
                drop(_one);
                if rp != cp {
                    show("truncated case", i, &rp, &cp);
                }
                trunc.eq(rp, cp, || format!("packet of case {i} cut at {cut}"));
            }
        }
        for f in &made.fills {
            if f.is_empty() {
                continue;
            }
            kinds[f[1.min(f.len() - 1)] as usize % 40] += 1;
            let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
            let cr = cpp_run(&bytes_input(3, f));
            let rr = rust_raw(f);
            drop(_one);
            if rr != cr {
                show("raw case", i, &rr, &cr);
            }
            raw.eq(rr, cr, || format!("raw fill of case {i}"));
        }
    }
    println!(
        "{n} pack cases, {cmds} commands, {fills} fills, {packets} packets ({full} over 440 bytes), {wrapped} wrapped commands, {truncs} truncated reads, {overruns} cases skipped for a packet over 476 bytes"
    );
    println!("fill command-type histogram (by the T byte): {:?}", kinds.iter().enumerate().filter(|(_, c)| **c > 0).collect::<Vec<_>>());
    if planted() {
        let (a, b, c, d) = (pack.report(), parse.report(), raw.report(), trunc.report());
        println!("MEASURED mismatching: pack {a} parse {b} raw {c} truncated {d}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        pack.finish();
        parse.finish();
        raw.finish();
        trunc.finish();
    }
}

#[test]
fn wrapped_commands_are_reassembled_as_upstream() {
    let n = cases("NETPKT_WRAP_CASES", 600);
    let mut g = Gen::new(0x215_0002);
    let mut chk = Check::new("wrap: packets, percentages and the reassembled command");
    let mut chunks = 0u64;
    for i in 0..n {
        let mut c = Ctx { player: g.below(8), frame: g.below(1000), relay: g.below(256), id: g.below(65536), ack_kind: 99, ack_orig: 0, ack_cmd: 0 };
        let mut spec = Vec::new();
        gen_cmd(&mut g, &mut c, &mut spec, true, false);
        // a command type without a command id is not wrapped (empty list upstream): the generator's big kinds all have ids
        let mut order: Vec<u32> = Vec::new();
        let nchunks_guess = 12;
        for k in 0..nchunks_guess {
            order.push(k);
        }
        // shuffle, repeat some, drop some
        for k in (1..order.len()).rev() {
            let j = g.below(k as u32 + 1) as usize;
            order.swap(k, j);
        }
        let keep = if g.below(4) == 0 { g.below(order.len() as u32) as usize } else { order.len() };
        order.truncate(keep);
        for _ in 0..g.below(4) {
            let k = g.below(12);
            order.push(k);
        }
        let mut input = vec![4, order.len() as u32];
        input.extend_from_slice(&order);
        input.extend_from_slice(&spec);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&input);
        let r = rust_wrap(&input);
        drop(_one);
        chunks += c[0] as u64;
        if r != c {
            show("wrap case", i, &r, &c);
        }
        chk.eq(r, c, || format!("wrap case {i}"));
    }
    println!("{n} wrapped commands, {chunks} wrapper packets");
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: wrap {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}

#[test]
fn the_word_wise_encryption_and_the_util_predicates_are_upstream_s() {
    let n = cases("NETPKT_CRYPT_CASES", 2000);
    let mut g = Gen::new(0x215_0003);
    let mut chk = Check::new("encryptBuf, decryptBuf, NetworkUtil");
    for i in 0..n {
        let len = match g.below(4) {
            0 => g.below(8) as usize,
            1 => 4 * g.below(30) as usize,
            _ => g.below(500) as usize,
        };
        let b = rand_bytes(&mut g, len, false);
        for which in 0..2u32 {
            let mut inp = vec![5, which];
            put_bytes(&mut inp, &b);
            let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
            let c = cpp_run(&inp);
            let r = rust_crypt(which, &b);
            drop(_one);
            chk.eq(r, c, || format!("crypt {which} case {i}, len {len}"));
        }
    }
    for k in 0..40 {
        let cnt = g.below(5) + if k == 0 { 70000 } else { 0 }; // the second round wraps the 16-bit counter
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&[6, cnt]);
        let r = rust_util(cnt);
        drop(_one);
        if r != c {
            show("util case", k, &r, &c);
        }
        chk.eq(r, c, || format!("util case {k}"));
    }
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: crypt/util {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// NetCommandList and FrameData
// ---------------------------------------------------------------------------------------------------------------

use rts_lockstep_engine::drafts::network::frame_data::{FrameData, FrameDataReturnType};
use rts_lockstep_engine::drafts::network::net_command_ref::NetCommandRefId;

/// A small command out of a small population (three players, ten ids, four frames), so that equal commands, runs of one type
/// and out-of-order inserts are frequent.
fn gen_small_cmd(g: &mut Gen, o: &mut Vec<u32>) {
    const K: [u32; 17] = [0, 1, 2, 3, 3, 4, 5, 8, 9, 14, 15, 16, 20, 21, 23, 25, 28];
    let kind = K[g.below(K.len() as u32) as usize];
    o.push(kind);
    o.push(g.below(3));
    o.push(g.below(4));
    o.push(g.below(10));
    o.push(g.below(4));
    match kind {
        0 | 1 | 2 => {
            o.push(g.below(6));
            o.push(g.below(2));
        }
        3 | 5 | 8 | 14 | 21 | 28 => o.push(g.below(5)),
        4 => {
            o.push(g.below(40));
            o.push(0);
        }
        20 => {
            o.push(g.below(3));
            o.push(g.below(3));
        }
        _ => {}
    }
}

fn pos_of(l: &NetCommandList, id: Option<NetCommandRefId>) -> u32 {
    let Some(id) = id else { return u32::MAX };
    let mut i = 0u32;
    let mut x = l.get_first_message();
    while let Some(c) = x {
        if c == id {
            return i;
        }
        i += 1;
        x = l.get(c).and_then(|r| r.get_next());
    }
    u32::MAX
}

fn summary(o: &mut Vec<u32>, l: &NetCommandList) {
    o.push(l.length() as u32);
    let mut x = l.get_first_message();
    while let Some(c) = x {
        let r = l.get(c).expect("ref");
        let m = r.get_command().read();
        o.push(m.get_net_command_type() as i32 as u32);
        o.push(m.get_player_id());
        o.push(m.get_execution_frame());
        o.push(m.get_id() as u32);
        o.push(r.get_relay() as u32);
        o.push(m.get_sort_number() as u32);
        x = r.get_next();
    }
    o.push(pos_of(l, l.get_last_message()));
}

fn rust_list(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let mut l = NetCommandList::new();
    l.init();
    let nops = r.next();
    for _ in 0..nops {
        match r.next() {
            0 => {
                let (m, relay) = make_msg(&mut r);
                let id = l.add_message(NetCommandMsgPtr::new(m));
                if let Some(i) = id {
                    l.get_mut(i).expect("ref").set_relay(relay as u8);
                }
                o.push(id.is_some() as u32);
                o.push(pos_of(&l, id));
            }
            1 => {
                let (m, _) = make_msg(&mut r);
                let id = l.find_message(&NetCommandMsgPtr::new(m));
                o.push(pos_of(&l, id));
            }
            2 => {
                let (id, pl) = (r.next(), r.next());
                let f = l.find_message_by_id(id as u16, pl as u8);
                o.push(pos_of(&l, f));
            }
            3 => {
                let a = r.next();
                let len = l.length();
                if len > 0 {
                    let mut x = l.get_first_message().expect("first");
                    for _ in 0..(a % len as u32) {
                        x = l.get(x).and_then(|r| r.get_next()).expect("next");
                    }
                    let _ = l.remove_message(x);
                    o.push(1);
                } else {
                    o.push(0);
                }
            }
            4 => {
                let (a, _) = make_msg(&mut r);
                let (b, _) = make_msg(&mut r);
                o.push(NetCommandList::is_equal_command_msg(&NetCommandMsgPtr::new(a), &NetCommandMsgPtr::new(b)) as u32);
            }
            5 => {
                let mut l2 = NetCommandList::new();
                l2.init();
                let n = r.next();
                for _ in 0..n {
                    let (m, relay) = make_msg(&mut r);
                    if let Some(i) = l2.add_message(NetCommandMsgPtr::new(m)) {
                        l2.get_mut(i).expect("ref").set_relay(relay as u8);
                    }
                }
                l.append_list(Some(&l2));
            }
            6 => l.reset(),
            7 => l.append_list(None),
            _ => {}
        }
        summary(&mut o, &l);
    }
    o
}

fn rust_fd(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let mut f = FrameData::new();
    let nops = r.next();
    for _ in 0..nops {
        match r.next() {
            0 => f.init(),
            1 => f.reset(),
            2 => f.set_frame(r.next()),
            3 => f.set_frame_command_count(r.next()),
            4 => {
                let (m, _) = make_msg(&mut r);
                f.add_command(&NetCommandMsgPtr::new(m));
            }
            5 => {
                let flag = r.next() & 1 != 0;
                o.push(match f.all_commands_ready(flag) {
                    FrameDataReturnType::NotReady => 0,
                    FrameDataReturnType::Resend => 1,
                    FrameDataReturnType::Ready => 2,
                });
            }
            6 => f.zero_frame(),
            7 => f.destroy_game_messages(),
            8 => f.update(),
            _ => {}
        }
        o.push(f.get_frame());
        o.push(f.get_frame_command_count());
        o.push(f.get_command_count());
        match f.get_command_list() {
            Some(l) => {
                o.push(1);
                summary(&mut o, l);
            }
            None => o.push(0),
        }
    }
    o
}

#[test]
fn the_command_list_orders_finds_and_removes_as_upstream() {
    let n = cases("NETPKT_LIST_CASES", 8000);
    let mut g = Gen::new(0x215_0004);
    let mut chk = Check::new("NetCommandList operations");
    let (mut ops, mut dups) = (0u64, 0u64);
    for i in 0..n {
        let nops = 1 + g.below(40);
        let mut v = vec![7, nops];
        for _ in 0..nops {
            match g.below(20) {
                0..=10 => {
                    v.push(0);
                    gen_small_cmd(&mut g, &mut v);
                }
                11 | 12 => {
                    v.push(1);
                    gen_small_cmd(&mut g, &mut v);
                }
                13 => {
                    v.push(2);
                    v.push(g.below(10));
                    v.push(g.below(3));
                }
                14 | 15 => {
                    v.push(3);
                    v.push(g.below(50));
                }
                16 => {
                    v.push(4);
                    gen_small_cmd(&mut g, &mut v);
                    gen_small_cmd(&mut g, &mut v);
                }
                17 => {
                    v.push(5);
                    let m = g.below(6);
                    v.push(m);
                    for _ in 0..m {
                        gen_small_cmd(&mut g, &mut v);
                    }
                }
                18 => v.push(if g.below(4) == 0 { 6 } else { 7 }),
                _ => {
                    v.push(0);
                    gen_small_cmd(&mut g, &mut v);
                }
            }
        }
        ops += nops as u64;
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_list(&v);
        drop(_one);
        dups += c.windows(2).filter(|w| w[0] == 0).count() as u64 * 0;
        if r != c {
            show("list case", i, &r, &c);
        }
        chk.eq(r, c, || format!("list case {i}"));
    }
    println!("{n} list sequences, {ops} operations{}", if dups > 0 { "" } else { "" });
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: list {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}

#[test]
fn the_frame_data_counts_and_resets_as_upstream() {
    let n = cases("NETPKT_FD_CASES", 6000);
    let mut g = Gen::new(0x215_0005);
    let mut chk = Check::new("FrameData operations");
    let mut results = [0u64; 3];
    for i in 0..n {
        let nops = 1 + g.below(30);
        let mut v = vec![8, nops];
        for _ in 0..nops {
            match g.below(16) {
                0 => v.push(0),
                1 => v.push(1),
                2 => {
                    v.push(2);
                    v.push(g.below(100));
                }
                3 | 4 => {
                    v.push(3);
                    v.push(if g.below(8) == 0 { u32::MAX } else { g.below(6) });
                }
                5..=8 => {
                    v.push(4);
                    gen_small_cmd(&mut g, &mut v);
                }
                9..=11 => {
                    v.push(5);
                    v.push(g.below(2));
                }
                12 => v.push(6),
                13 => v.push(7),
                14 => v.push(8),
                _ => {
                    v.push(4);
                    gen_small_cmd(&mut g, &mut v);
                }
            }
        }
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_fd(&v);
        drop(_one);
        if r != c {
            show("frame data case", i, &r, &c);
        }
        let _ = &mut results;
        chk.eq(r, c, || format!("frame data case {i}"));
    }
    println!("{n} frame data sequences");
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: frame data {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}

/// The upstream defects of `isRoomForLoadCompleteMessage` and `isRoomForTimeOutGameStartMessage` (#210): both leave out the `C`
/// field their add function writes (3 bytes), so the message is added to a packet that has room for the message but not for its
/// `C`, and `m_packet[476]` is overrun. This packs `n` filler commands (progress messages for three players, keep-alives, or
/// player-leave messages: different header fields, so different lengths) and then one load-complete or time-out-start, and
/// compares the log up to the message's `addCommand` result (sizes, fill, `isRoomFor`, the result); what follows is the packet,
/// whose C++ state after an overrun is corrupted memory (the write runs into `m_packetLen`), so its Rust and C++ values are
/// printed, not compared.
#[test]
fn the_known_upstream_overruns() {
    let mut found = Vec::new();
    let mut chk = Check::new("overrun probe: the log up to the final message's add result");
    let mut probes = 0u64;
    for kind in [15u32, 16] {
        for filler in [14u32, 9, 5] {
            for n in 0..140u32 {
                let mut v = vec![1, n + 1];
                for k in 0..n {
                    v.extend_from_slice(&[filler, k % 3, 10, 100 + k, 4]);
                    if filler != 9 {
                        v.push(k % 7);
                    }
                }
                v.extend_from_slice(&[kind, 7, 10, 5000, 4]);
                probes += 1;
                let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
                let c = cpp_run(&v);
                let mut made = Made::default();
                let r = rust_pack(&v, &mut made);
                drop(_one);
                // the log ends: ..., room, ok, 0xE0D0, [len, count, bytes] (or the packet before the last was sent: then 0xE0D0 follows
                // the new packet's ok2 and the message's own packet comes after)
                let ec = c.iter().rposition(|&w| w == 0xE0D0).expect("E0D0");
                let er = r.iter().rposition(|&w| w == 0xE0D0).expect("E0D0");
                chk.eq(r[..er].to_vec(), c[..ec].to_vec(), || format!("kind {kind} filler {filler} n {n}"));
                let rl = made.packets.last().map(|p| p.len()).unwrap_or(0);
                if rl > 476 {
                    found.push((kind, filler, n, rl, c[ec + 1]));
                }
            }
        }
    }
    println!("{probes} probes; {} added past 476 bytes (kind, filler, fillers, rust length, C++ m_packetLen afterwards):", found.len());
    for f in found.iter().take(12) {
        println!("  {:?}", f);
    }
    assert!(!found.is_empty(), "the probe must reach the overrun");
    assert!(found.iter().all(|f| f.3 <= 479), "an overrun is by 3 at most");
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: overrun probe {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}

/// Upstream defect (#210 item 1): `GetRunAheadMetricsCommandSize` does not match `FillBufferWithRunAheadMetricsCommand` (and
/// `addRunAheadMetricsCommand`). Measured on the C++: one run-ahead metrics command (player 1, frame 7, id 9, relay 3,
/// latency 0.5, fps 30): the size says 15, the fill writes 16 bytes. **The defect is one byte, not the two the brief
/// assumed:** the size counts a stray `sizeof(UnsignedByte)` after the `D` and leaves out the 2-byte fps (net one short). The
/// Rust draft reproduces both numbers (the pack test compares them on 2,400 such commands).
#[test]
fn the_known_upstream_run_ahead_metrics_size_omits_the_fps() {
    let spec = [1u32, 1, 6, 1, 7, 9, 3, 0.5f32.to_bits(), 30];
    let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let c = cpp_run(&spec);
    let mut made = Made::default();
    let r = rust_pack(&spec, &mut made);
    drop(_one);
    let (size, fill_len) = (c[0], c[1]);
    println!("C++: GetRunAheadMetricsCommandSize = {size}, FillBufferWithCommand wrote {fill_len} bytes; Rust: {} and {}", r[0], r[1]);
    assert_eq!((size, fill_len), (15, 16));
    assert_eq!(r, c);
}

/// The room checks at the edge: `n` filler commands (progress messages for three players, keep-alives or player-leave messages,
/// whose different headers give different packet lengths) bring the packet close to 476 bytes, then one command of each kind
/// with a header that repeats or changes the filler's; the whole log (sizes, fills, `isRoomFor`, the add results, the packets)
/// is compared. Load-complete and time-out-start are the overrun probe's.
#[test]
fn the_room_checks_agree_at_the_edge_of_a_full_packet() {
    let mut g = Gen::new(0x215_0006);
    let mut chk = Check::new("room boundary probe: the whole log");
    let (mut probes, mut refused, mut accepted_tight) = (0u64, 0u64, 0u64);
    for kind in (0..=29u32).filter(|k| ![12, 13, 15, 16, 22].contains(k)) {
        for filler in [14u32, 9, 5] {
            for n in (0..160u32).step_by(1) {
                for variant in 0..2u32 {
                    let mut v = vec![1, n + 1];
                    for k in 0..n {
                        v.extend_from_slice(&[filler, k % 3, 10, 100 + k, 4]);
                        if filler != 9 {
                            v.push(k % 7);
                        }
                    }
                    let mut c = Ctx { player: (n + 1) % 3, frame: 10, relay: 4, id: 100 + n, ack_kind: 99, ack_orig: 0, ack_cmd: 0 };
                    if variant == 1 {
                        c.player = 5;
                        c.frame = 11;
                        c.relay = 9;
                        c.id = 7000;
                    }
                    // a game message with few arguments, a short chat, a small file: the sizes that straddle the edge
                    gen_cmd_kind(&mut g, &mut c, &mut v, false, false, kind);
                    probes += 1;
                    let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
                    let cc = cpp_run(&v);
                    let mut made = Made::default();
                    let r = rust_pack(&v, &mut made);
                    drop(_one);
                    if r != cc {
                        show("room probe", probes, &r, &cc);
                    }
                    refused += cc.windows(2).filter(|w| w[0] == 0 && w[1] == 0).count() as u64 * 0;
                    accepted_tight += made.packets.last().map(|p| (p.len() > 470) as u64).unwrap_or(0);
                    chk.eq(r, cc, || format!("kind {kind} filler {filler} n {n} variant {variant}"));
                }
            }
        }
    }
    println!("{probes} probes, {accepted_tight} ended with a packet over 470 bytes{}", if refused > 0 { "" } else { "" });
    if planted() {
        let a = chk.report();
        println!("MEASURED mismatching: room probe {a}");
        assert!(false, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        chk.finish();
    }
}
