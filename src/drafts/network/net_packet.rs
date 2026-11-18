//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetPacket.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetPacket.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameMessageParser.h (the run-length argument-type list, over the
//!     net message's own arguments; the port's `message_arg_type_runs` in `common/recorder.rs` is the same function
//!     over a `GameMessage`)
//!
//! # Wire format
//!
//! A packet is a byte string of tagged fields: `T` command type (1 byte), `F` execution frame (u32), `R` relay
//! (u8), `P` player id (u8), `C` command id (u16), `D` command data, `Z` repeat the last command (ack and frame
//! commands only). A field is written only when it differs from the last one written (`m_lastCommandType`,
//! `m_lastFrame`, `m_lastRelay`, `m_lastPlayerID`, `m_lastCommandID`), and the command id is implied as
//! `last + 1` when it is not written. All multi-byte values are little-endian (x86 `memcpy` upstream).
//!
//! # Differences from upstream that are not behaviour changes on well-formed packets
//!
//! * `m_packet` is a `Vec<u8>` whose length is `m_packetLen`; upstream's fixed 476-byte array is written past its end
//!   by the commands whose `isRoomFor*` undercounts (see [`NetPacket::is_room_for_load_complete_message`]).
//! * Reads past the end of the data return zero bytes (upstream reads whatever follows); a length field that claims
//!   more bytes than remain is clamped to what remains.
//! * `constructGameMessage` (which needs `ThePlayerList`) is only used by upstream to read back the type and the
//!   arguments; here the net message's own list is used, with the arguments of unknown data type dropped and the
//!   count wrapped to an `UnsignedByte`, exactly what the constructed `GameMessage` would hold.
//!
//! # Defects found in upstream while translating (reproduced, not fixed)
//!
//! * `FillBufferWithAckCommand` casts the `NetCommandRef *` itself to the ack message type and reads its fields.
//!   Acks are never wrapped, so it is unreachable; here the ack's fields are read from the command (the intent).
//! * `ConstructBigCommandPacketList` does `ref->setRelay(ref->getRelay())` (a no-op: the wrapper ref's relay is
//!   never set from the original).
//! * `GetRunAheadMetricsCommandSize` omits the 2-byte average-fps field that the fill function writes.
//! * `isRoomForLoadCompleteMessage` and `isRoomForTimeOutGameStartMessage` do not count the `C` field that the add
//!   functions write.

use std::cmp::min;

use crate::common::base_type::{Coord3D, ICoord2D, IRegion2D};
use crate::common::ascii_string::AsciiString;
use crate::common::message_stream::{ArgumentData, ArgumentDataType, GameMessageArgument, MessageType};
use crate::common::unicode_string::UnicodeString;

use super::net_command_list::NetCommandList;
use super::net_command_msg::{
    NetCommandBody, NetCommandMsg, NetCommandMsgPtr, NetCommandType, TransportMessage, MAX_PACKET_SIZE,
};
use super::net_command_ref::NetCommandRef;
use super::network_util::{does_command_require_a_command_id, generate_next_command_id};

/// `MAX_PACKET_SIZE` as a `usize`.
const MAX_PACKET: usize = MAX_PACKET_SIZE as usize;

/// `typedef std::list<NetPacket *> NetPacketList`.
pub type NetPacketList = Vec<NetPacket>;

// ---------------------------------------------------------------------------------------------------------------
// byte helpers
// ---------------------------------------------------------------------------------------------------------------

fn rd_u8(d: &[u8], i: &mut usize) -> u8 {
    let v = d.get(*i).copied().unwrap_or(0);
    *i += 1;
    v
}

fn rd_u16(d: &[u8], i: &mut usize) -> u16 {
    let lo = rd_u8(d, i) as u16;
    let hi = rd_u8(d, i) as u16;
    lo | (hi << 8)
}

fn rd_u32(d: &[u8], i: &mut usize) -> u32 {
    let a = rd_u16(d, i) as u32;
    let b = rd_u16(d, i) as u32;
    a | (b << 16)
}

fn rd_i32(d: &[u8], i: &mut usize) -> i32 {
    rd_u32(d, i) as i32
}

fn wr_u8(v: &mut Vec<u8>, x: u8) {
    v.push(x);
}
fn wr_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn wr_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// `sizeof` of one argument of this type on the wire (the size of the union member). 0 for an unknown type, which
/// upstream's `if` chains skip.
fn arg_wire_size(ty: ArgumentDataType) -> u16 {
    match ty {
        ArgumentDataType::Integer => 4,
        ArgumentDataType::Real => 4,
        ArgumentDataType::Boolean => 1,
        ArgumentDataType::ObjectId => 4,
        ArgumentDataType::DrawableId => 4,
        ArgumentDataType::TeamId => 4,
        ArgumentDataType::Location => 12,
        ArgumentDataType::Pixel => 8,
        ArgumentDataType::PixelRegion => 16,
        ArgumentDataType::Timestamp => 4,
        ArgumentDataType::WideChar => 2,
        _ => 0,
    }
}

/// The argument list a `constructGameMessage()` of the net message would hold: arguments of unknown data type are
/// not appended, and `getArgumentCount()` is an `UnsignedByte` that wraps (the parser and the fill loops read only
/// that many).
fn game_message_view(msg: &NetCommandMsg) -> (MessageType, Vec<GameMessageArgument>) {
    if let NetCommandBody::GameCommand(g) = &msg.body {
        let mut args: Vec<GameMessageArgument> = Vec::new();
        for a in &g.args {
            if arg_wire_size(a.ty) != 0 {
                args.push(*a);
            }
        }
        let count = (args.len() & 0xff) as usize;
        args.truncate(count);
        (g.ty, args)
    } else {
        (MessageType(0), Vec::new())
    }
}

/// `GameMessageParser::GameMessageParser(GameMessage *msg)` over an argument list: the arguments as runs of one
/// type, `(type, count)`. Same function as the port's `message_arg_type_runs`.
fn parser_runs(args: &[GameMessageArgument]) -> Vec<(ArgumentDataType, i32)> {
    let mut runs: Vec<(ArgumentDataType, i32)> = Vec::new();
    let mut last = ArgumentDataType::Unknown;
    let mut this_count = 0;
    for a in args {
        let t = a.ty;
        if t != last {
            if this_count > 0 {
                runs.push((last, this_count));
            }
            last = t;
            this_count = 0;
        }
        this_count += 1;
    }
    if this_count > 0 {
        runs.push((last, this_count));
    }
    runs
}

/// The size in bytes of the argument data of the parser runs (the `while (arg != NULL)` loop shared by
/// `GetGameCommandSize` and `isRoomForGameMessage`), without the 2 bytes per run declaration.
fn runs_data_size(runs: &[(ArgumentDataType, i32)]) -> i32 {
    let mut n = 0;
    for (ty, count) in runs {
        n += count * arg_wire_size(*ty) as i32;
    }
    n
}

/// `writeGameMessageArgumentToPacket` (NetPacket.cpp:4821): the bytes of one argument.
fn write_game_message_argument(v: &mut Vec<u8>, ty: ArgumentDataType, arg: ArgumentData) {
    match ty {
        ArgumentDataType::Integer => wr_u32(v, arg.0[0]),
        ArgumentDataType::Real => wr_u32(v, arg.0[0]),
        ArgumentDataType::Boolean => wr_u8(v, (arg.0[0] & 0xff) as u8),
        ArgumentDataType::ObjectId => wr_u32(v, arg.0[0]),
        ArgumentDataType::DrawableId => wr_u32(v, arg.0[0]),
        ArgumentDataType::TeamId => wr_u32(v, arg.0[0]),
        ArgumentDataType::Location => {
            wr_u32(v, arg.0[0]);
            wr_u32(v, arg.0[1]);
            wr_u32(v, arg.0[2]);
        }
        ArgumentDataType::Pixel => {
            wr_u32(v, arg.0[0]);
            wr_u32(v, arg.0[1]);
        }
        ArgumentDataType::PixelRegion => {
            wr_u32(v, arg.0[0]);
            wr_u32(v, arg.0[1]);
            wr_u32(v, arg.0[2]);
            wr_u32(v, arg.0[3]);
        }
        ArgumentDataType::Timestamp => wr_u32(v, arg.0[0]),
        ArgumentDataType::WideChar => wr_u16(v, (arg.0[0] & 0xffff) as u16),
        _ => {}
    }
}

/// `readGameMessageArgumentFromPacket` (NetPacket.cpp:5267): reads one argument of the type and adds it to the
/// message.
fn read_game_message_argument_from_packet(
    ty: ArgumentDataType,
    msg: &mut NetCommandMsg,
    data: &[u8],
    i: &mut usize,
) {
    let words: [u32; 4] = match ty {
        ArgumentDataType::Integer
        | ArgumentDataType::Real
        | ArgumentDataType::ObjectId
        | ArgumentDataType::DrawableId
        | ArgumentDataType::TeamId
        | ArgumentDataType::Timestamp => [rd_u32(data, i), 0, 0, 0],
        ArgumentDataType::Boolean => [rd_u8(data, i) as u32, 0, 0, 0],
        ArgumentDataType::Location => {
            let x = rd_u32(data, i);
            let y = rd_u32(data, i);
            let z = rd_u32(data, i);
            [x, y, z, 0]
        }
        ArgumentDataType::Pixel => {
            let x = rd_u32(data, i);
            let y = rd_u32(data, i);
            [x, y, 0, 0]
        }
        ArgumentDataType::PixelRegion => {
            let a = rd_u32(data, i);
            let b = rd_u32(data, i);
            let c = rd_u32(data, i);
            let d = rd_u32(data, i);
            [a, b, c, d]
        }
        ArgumentDataType::WideChar => [rd_u16(data, i) as u32, 0, 0, 0],
        _ => return,
    };
    msg.add_argument(ty, ArgumentData(words));
}

// keep the base-type imports used by the doc references above
#[allow(dead_code)]
const _SIZES: (usize, usize, usize) = (
    std::mem::size_of::<Coord3D>(),
    std::mem::size_of::<ICoord2D>(),
    std::mem::size_of::<IRegion2D>(),
);

// ---------------------------------------------------------------------------------------------------------------
// the packet
// ---------------------------------------------------------------------------------------------------------------

/// Where the `F` field goes relative to `R` in an add function (the order of the writes differs per command).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FramePos {
    /// No `F` field.
    None,
    /// `T F R P C D`.
    BeforeRelay,
    /// `T R F P C D`.
    AfterRelay,
}

/// Which header fields an add function writes, and whether its `isRoomFor*` counts the `C` field.
#[derive(Clone, Copy)]
struct Layout {
    frame: FramePos,
    /// The add function writes `C`/updates `m_lastCommandID`.
    cmd_id: bool,
    /// The `isRoomFor*` function counts the `C` field (false for LoadComplete and TimeOutGameStart).
    room_counts_cmd_id: bool,
}

const TRPD: Layout = Layout { frame: FramePos::None, cmd_id: false, room_counts_cmd_id: false };
const TRPCD: Layout = Layout { frame: FramePos::None, cmd_id: true, room_counts_cmd_id: true };
const TFRPCD: Layout = Layout { frame: FramePos::BeforeRelay, cmd_id: true, room_counts_cmd_id: true };
const TRFPCD: Layout = Layout { frame: FramePos::AfterRelay, cmd_id: true, room_counts_cmd_id: true };
/// LoadComplete and TimeOutGameStart: write `C`, but their `isRoomFor*` does not count it.
const TRPCD_UNCOUNTED: Layout = Layout { frame: FramePos::None, cmd_id: true, room_counts_cmd_id: false };

/// `class NetPacket`.
#[derive(Debug)]
pub struct NetPacket {
    /// `m_packet[0..m_packetLen]`.
    packet: Vec<u8>,
    /// `m_addr`.
    addr: u32,
    /// `m_numCommands`.
    num_commands: i32,
    /// `m_lastCommand`.
    last_command: Option<NetCommandRef>,
    /// `m_lastFrame`.
    last_frame: u32,
    /// `m_port`.
    port: u16,
    /// `m_lastCommandID`.
    last_command_id: u16,
    /// `m_lastPlayerID`.
    last_player_id: u8,
    /// `m_lastCommandType`.
    last_command_type: u8,
    /// `m_lastRelay`.
    last_relay: u8,
}

impl Default for NetPacket {
    fn default() -> Self {
        NetPacket::new()
    }
}

impl NetPacket {
    /// `NetPacket::NetPacket()` (NetPacket.cpp:1905): `init()`.
    pub fn new() -> NetPacket {
        NetPacket {
            packet: Vec::new(),
            addr: 0,
            port: 0,
            num_commands: 0,
            last_command: None,
            last_frame: 0,
            last_command_id: 0,
            last_player_id: 0,
            last_command_type: 0,
            last_relay: 0,
        }
    }

    /// `NetPacket::NetPacket(TransportMessage *msg)`: copies `MAX_PACKET_SIZE` bytes of the transport data, sets
    /// `m_packetLen = msg->length` (clamped here to the 476 bytes that were copied), `m_numCommands = -1`, and the
    /// address and port.
    pub fn from_transport_message(msg: &TransportMessage) -> NetPacket {
        let mut p = NetPacket::new();
        let len = if msg.length < 0 { 0 } else { min(msg.length as usize, MAX_PACKET) };
        p.packet = msg.data[..len].to_vec();
        p.num_commands = -1;
        p.addr = msg.addr;
        p.port = msg.port;
        p
    }

    /// `NetPacket::init` (NetPacket.cpp:1932).
    pub fn init(&mut self) {
        self.addr = 0;
        self.port = 0;
        self.num_commands = 0;
        self.packet.clear();
        self.last_player_id = 0;
        self.last_frame = 0;
        self.last_command_id = 0;
        self.last_command_type = 0;
        self.last_relay = 0;
        self.last_command = None;
    }

    /// `NetPacket::reset` (NetPacket.cpp:1948): deletes `m_lastCommand`, then `init()`.
    pub fn reset(&mut self) {
        self.last_command = None;
        self.init();
    }

    /// `NetPacket::setAddress` (NetPacket.cpp:1959): `m_addr = addr; m_port = port` (the `Int`s truncate).
    pub fn set_address(&mut self, addr: i32, port: i32) {
        self.addr = addr as u32;
        self.port = port as u16;
    }

    /// `getNumCommands`.
    pub fn get_num_commands(&self) -> i32 {
        self.num_commands
    }
    /// `getAddr`.
    pub fn get_addr(&self) -> u32 {
        self.addr
    }
    /// `getPort`.
    pub fn get_port(&self) -> u16 {
        self.port
    }
    /// `getData`: `m_packet[0..m_packetLen]`.
    pub fn get_data(&self) -> &[u8] {
        &self.packet
    }
    /// `getLength`: `m_packetLen`.
    pub fn get_length(&self) -> i32 {
        self.packet.len() as i32
    }

    /// `dumpPacketToLog`: only `DEBUG_LOG` output upstream; not translated.
    pub fn dump_packet_to_log(&self) {}

    // ---- ConstructNetCommandMsgFromRawData / ConstructBigCommandPacketList --------------------------------

    /// `NetPacket::ConstructNetCommandMsgFromRawData` (NetPacket.cpp:43): rebuilds the one command a wrapper's data
    /// holds (`T`/`R`/`P`/`C`/`F` fields, then `D` and the command's data). `None` when the data holds no `D`, or
    /// the command type is not one the reader handles (upstream dereferences a null `msg` there).
    pub fn construct_net_command_msg_from_raw_data(data: &[u8], data_length: u32) -> Option<NetCommandRef> {
        let data_length = min(data_length as usize, data.len());
        let data = &data[..data_length];
        let mut command_type: i32 = NetCommandType::GameCommand as i32;
        let mut command_id: u16 = 0;
        let mut frame: u32 = 0;
        let mut player_id: u8 = 0;
        let mut relay: u8 = 0;
        let mut offset: usize = 0;
        let mut not_done = true;
        let mut result: Option<NetCommandRef> = None;
        while offset < data_length && not_done {
            match data[offset] {
                b'T' => {
                    offset += 1;
                    // memcpy of one byte into the low byte of the enum
                    let b = rd_u8(data, &mut offset) as i32;
                    command_type = (command_type & !0xff) | b;
                }
                b'R' => {
                    offset += 1;
                    relay = rd_u8(data, &mut offset);
                }
                b'P' => {
                    offset += 1;
                    player_id = rd_u8(data, &mut offset);
                }
                b'C' => {
                    offset += 1;
                    command_id = rd_u16(data, &mut offset);
                }
                b'F' => {
                    offset += 1;
                    frame = rd_u32(data, &mut offset);
                }
                b'D' => {
                    offset += 1;
                    let ty = NetCommandType::from_i32(command_type)?;
                    let mut msg = NetPacket::read_message_of_type(ty, data, &mut offset)?;
                    msg.set_execution_frame(frame);
                    msg.set_id(command_id);
                    msg.set_player_id(player_id as u32);
                    msg.set_net_command_type(ty);
                    // `ref = NEW_NETCOMMANDREF(msg)` attaches; `msg->detach()` drops the creation reference.
                    let ptr = NetCommandMsgPtr::new(msg);
                    let mut r = NetCommandRef::new(ptr.clone());
                    r.set_relay(relay);
                    let _ = ptr.write().detach();
                    result = Some(r);
                    not_done = false;
                }
                _ => {
                    // upstream's loop does not advance on an unknown byte (it would spin); stop instead.
                    return None;
                }
            }
        }
        result
    }

    /// The `if/else if` chain on `commandType` shared by `ConstructNetCommandMsgFromRawData` and `getCommandList`
    /// (the latter as a `switch`): calls the type's `read*Message`. `None` for a type without a reader.
    fn read_message_of_type(ty: NetCommandType, data: &[u8], i: &mut usize) -> Option<NetCommandMsg> {
        use NetCommandType::*;
        Some(match ty {
            GameCommand => NetPacket::read_game_message(data, i)?,
            AckBoth => NetPacket::read_ack_both_message(data, i),
            AckStage1 => NetPacket::read_ack_stage1_message(data, i),
            AckStage2 => NetPacket::read_ack_stage2_message(data, i),
            FrameInfo => NetPacket::read_frame_message(data, i),
            PlayerLeave => NetPacket::read_player_leave_message(data, i),
            RunAheadMetrics => NetPacket::read_run_ahead_metrics_message(data, i),
            RunAhead => NetPacket::read_run_ahead_message(data, i),
            DestroyPlayer => NetPacket::read_destroy_player_message(data, i),
            KeepAlive => NetPacket::read_keep_alive_message(data, i),
            DisconnectKeepAlive => NetPacket::read_disconnect_keep_alive_message(data, i),
            DisconnectPlayer => NetPacket::read_disconnect_player_message(data, i),
            PacketRouterQuery => NetPacket::read_packet_router_query_message(data, i),
            PacketRouterAck => NetPacket::read_packet_router_ack_message(data, i),
            DisconnectChat => NetPacket::read_disconnect_chat_message(data, i),
            DisconnectVote => NetPacket::read_disconnect_vote_message(data, i),
            Chat => NetPacket::read_chat_message(data, i),
            Progress => NetPacket::read_progress_message(data, i),
            LoadComplete => NetPacket::read_load_complete_message(data, i),
            TimeoutStart => NetPacket::read_time_out_game_start_message(data, i),
            Wrapper => NetPacket::read_wrapper_message(data, i),
            File => NetPacket::read_file_message(data, i),
            FileAnnounce => NetPacket::read_file_announce_message(data, i),
            FileProgress => NetPacket::read_file_progress_message(data, i),
            DisconnectFrame => NetPacket::read_disconnect_frame_message(data, i),
            DisconnectScreenOff => NetPacket::read_disconnect_screen_off_message(data, i),
            FrameResendRequest => NetPacket::read_frame_resend_request_message(data, i),
            _ => return None,
        })
    }

    /// `NetPacket::ConstructBigCommandPacketList` (NetPacket.cpp:153): splits a command too big for one packet into
    /// wrapper commands, one per packet. Empty (upstream returns `NULL`) for a command type without a command id.
    pub fn construct_big_command_packet_list(r: &NetCommandRef) -> NetPacketList {
        let msg_ptr = r.get_command().clone();
        let (msg_type, msg_player, msg_frame, msg_id) = {
            let m = msg_ptr.read();
            (m.get_net_command_type(), m.get_player_id(), m.get_execution_frame(), m.get_id())
        };
        let mut packet_list: NetPacketList = Vec::new();
        if !does_command_require_a_command_id(msg_type) {
            // DEBUG_CRASH("Trying to wrap a command that doesn't have a unique command ID")
            return packet_list;
        }
        let buffer_size = NetPacket::get_buffer_size_needed_for_command(&msg_ptr.read());
        let mut big_packet_data: Vec<u8> = Vec::with_capacity(buffer_size as usize);
        NetPacket::fill_buffer_with_command(&mut big_packet_data, r);
        // the big packet is zero-padded to the size the Get*Size function promised
        big_packet_data.resize(buffer_size as usize, 0);
        let mut big_packet_current_offset: u32 = 0;

        let wrapper = NetCommandMsgPtr::new(NetCommandMsg::new_wrapper());
        let wrapper_size = NetPacket::get_buffer_size_needed_for_command(&wrapper.read());
        let command_size_per_packet: u32 = MAX_PACKET_SIZE as u32 - wrapper_size;
        let mut num_chunks: u32 = buffer_size / command_size_per_packet;
        if (buffer_size % command_size_per_packet) > 0 {
            num_chunks += 1;
        }
        let mut current_chunk: u32 = 0;
        while current_chunk < num_chunks {
            let mut packet = NetPacket::new();
            let mut data_size_this_packet: u16 = command_size_per_packet as u16;
            if (buffer_size - big_packet_current_offset) < data_size_this_packet as u32 {
                data_size_this_packet = (buffer_size - big_packet_current_offset) as u16;
            }
            {
                let mut w = wrapper.write();
                if does_command_require_a_command_id(w.get_net_command_type()) {
                    w.set_id(generate_next_command_id());
                }
                w.set_player_id(msg_player);
                w.set_execution_frame(msg_frame);
                w.set_chunk_number(current_chunk);
                w.set_num_chunks(num_chunks);
                w.set_data_offset(big_packet_current_offset);
                let start = big_packet_current_offset as usize;
                let end = start + data_size_this_packet as usize;
                w.set_wrapper_data(&big_packet_data[start..end]);
                w.set_total_data_length(buffer_size);
                w.set_wrapped_command_id(msg_id);
            }
            big_packet_current_offset += data_size_this_packet as u32;
            let mut wref = NetCommandRef::new(wrapper.clone());
            // upstream: `ref->setRelay(ref->getRelay())` (a self-assignment; the relay stays unset)
            let rel = wref.get_relay();
            wref.set_relay(rel);
            let _ok = packet.add_command(&wref);
            // if (!_ok) DEBUG_LOG("NetPacket::BeginBigCommandPacketList - failed to add a wrapper command to the packet")
            packet_list.push(packet);
            // ref->deleteInstance()
            drop(wref);
            current_chunk += 1;
        }
        // wrapperMsg->detach()
        let _ = wrapper.write().detach();
        packet_list
    }

    // ---- Get*Size ------------------------------------------------------------------------------------------

    /// `NetPacket::GetBufferSizeNeededForCommand` (NetPacket.cpp:233): the size of the command without compression,
    /// repetition etc. 0 for a type it does not know.
    pub fn get_buffer_size_needed_for_command(msg: &NetCommandMsg) -> u32 {
        use NetCommandType::*;
        match msg.get_net_command_type() {
            GameCommand => NetPacket::get_game_command_size(msg),
            AckStage1 | AckStage2 | AckBoth => NetPacket::get_ack_command_size(msg),
            FrameInfo => NetPacket::get_frame_command_size(msg),
            PlayerLeave => NetPacket::get_player_leave_command_size(msg),
            RunAheadMetrics => NetPacket::get_run_ahead_metrics_command_size(msg),
            RunAhead => NetPacket::get_run_ahead_command_size(msg),
            DestroyPlayer => NetPacket::get_destroy_player_command_size(msg),
            KeepAlive => NetPacket::get_keep_alive_command_size(msg),
            DisconnectKeepAlive => NetPacket::get_disconnect_keep_alive_command_size(msg),
            DisconnectPlayer => NetPacket::get_disconnect_player_command_size(msg),
            PacketRouterQuery => NetPacket::get_packet_router_query_command_size(msg),
            PacketRouterAck => NetPacket::get_packet_router_ack_command_size(msg),
            DisconnectChat => NetPacket::get_disconnect_chat_command_size(msg),
            DisconnectVote => NetPacket::get_disconnect_vote_command_size(msg),
            Chat => NetPacket::get_chat_command_size(msg),
            Progress => NetPacket::get_progress_message_size(msg),
            LoadComplete => NetPacket::get_load_complete_message_size(msg),
            TimeoutStart => NetPacket::get_time_out_game_start_message_size(msg),
            Wrapper => NetPacket::get_wrapper_command_size(msg),
            File => NetPacket::get_file_command_size(msg),
            FileAnnounce => NetPacket::get_file_announce_command_size(msg),
            FileProgress => NetPacket::get_file_progress_command_size(msg),
            DisconnectFrame => NetPacket::get_disconnect_frame_command_size(msg),
            DisconnectScreenOff => NetPacket::get_disconnect_screen_off_command_size(msg),
            FrameResendRequest => NetPacket::get_frame_resend_request_command_size(msg),
            _ => {
                // DEBUG_CRASH("Unknown NETCOMMANDTYPE %d")
                0
            }
        }
    }

    /// `GetGameCommandSize` (NetPacket.cpp:302): `msglen` is an `UnsignedShort` upstream and wraps.
    pub fn get_game_command_size(msg: &NetCommandMsg) -> u32 {
        let mut msglen: u16 = 0;
        msglen = msglen.wrapping_add(4 + 1); // frame number
        msglen = msglen.wrapping_add(1 + 1); // player ID
        msglen = msglen.wrapping_add(1 + 1); // relay
        msglen = msglen.wrapping_add(1 + 1); // command type
        msglen = msglen.wrapping_add(2 + 1); // command ID
        msglen = msglen.wrapping_add(1); // the 'D' for the data section
        let (_ty, args) = game_message_view(msg);
        let runs = parser_runs(&args);
        msglen = msglen.wrapping_add(4); // sizeof(GameMessage::Type)
        msglen = msglen.wrapping_add(1);
        for (ty, count) in &runs {
            msglen = msglen.wrapping_add(2); // the type and number of args of that type declaration
            msglen = msglen.wrapping_add((count * arg_wire_size(*ty) as i32) as u16);
        }
        msglen as u32
    }

    /// `GetAckCommandSize` (NetPacket.cpp:358).
    pub fn get_ack_command_size(_msg: &NetCommandMsg) -> u32 {
        let mut msglen = 0;
        msglen += 1;
        msglen += 1;
        msglen += 1;
        msglen += 1;
        msglen += 1;
        msglen += 2;
        msglen += 1;
        msglen
    }

    /// `GetFrameCommandSize` (NetPacket.cpp:372).
    pub fn get_frame_command_size(_msg: &NetCommandMsg) -> u32 {
        let mut msglen = 0;
        msglen += 1; // 'T'
        msglen += 1;
        msglen += 4 + 1; // 'F'
        msglen += 1 + 1; // 'R'
        msglen += 1; // 'P'
        msglen += 1;
        msglen += 2 + 1; // 'C'
        msglen += 1; // 'D'
        msglen += 2;
        msglen
    }

    /// `GetPlayerLeaveCommandSize` (NetPacket.cpp:388).
    pub fn get_player_leave_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (4 + 1) + 1 + 1 + (2 + 1) + 1 + 1
    }

    /// `GetRunAheadMetricsCommandSize` (NetPacket.cpp:405): omits the 2-byte fps field that the fill function
    /// writes (see the module doc).
    pub fn get_run_ahead_metrics_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + (2 + 1) + 1 + 1 + 4
    }

    /// `GetRunAheadCommandSize` (NetPacket.cpp:422).
    pub fn get_run_ahead_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (4 + 1) + 1 + 1 + (2 + 1) + 1 + 2 + 1
    }

    /// `GetDestroyPlayerCommandSize` (NetPacket.cpp:440).
    pub fn get_destroy_player_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (4 + 1) + 1 + 1 + (2 + 1) + 1 + 4
    }

    /// `GetKeepAliveCommandSize` (NetPacket.cpp:457).
    pub fn get_keep_alive_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetDisconnectKeepAliveCommandSize` (NetPacket.cpp:471).
    pub fn get_disconnect_keep_alive_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetDisconnectPlayerCommandSize` (NetPacket.cpp:485).
    pub fn get_disconnect_player_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + (2 + 1) + 1 + 1 + 4
    }

    /// `GetPacketRouterQueryCommandSize` (NetPacket.cpp:502).
    pub fn get_packet_router_query_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetPacketRouterAckCommandSize` (NetPacket.cpp:516).
    pub fn get_packet_router_ack_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetDisconnectChatCommandSize` (NetPacket.cpp:530): the text length is an `UnsignedByte`.
    pub fn get_disconnect_chat_command_size(msg: &NetCommandMsg) -> u32 {
        let textmsglen = (msg.get_text().get_length() & 0xff) as u32;
        1 + 1 + (1 + 1) + 1 + 1 + 1 + 1 + textmsglen * 2
    }

    /// `GetDisconnectVoteCommandSize` (NetPacket.cpp:548).
    pub fn get_disconnect_vote_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + (2 + 1) + 1 + 1 + 4
    }

    /// `GetChatCommandSize` (NetPacket.cpp:565).
    pub fn get_chat_command_size(msg: &NetCommandMsg) -> u32 {
        let textmsglen = (msg.get_text().get_length() & 0xff) as u32;
        1 + 1 + (4 + 1) + (1 + 1) + 1 + 1 + (2 + 1) + 1 + 1 + textmsglen * 2 + 4
    }

    /// `GetProgressMessageSize` (NetPacket.cpp:586).
    pub fn get_progress_message_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1 + 1
    }

    /// `GetLoadCompleteMessageSize` (NetPacket.cpp:601).
    pub fn get_load_complete_message_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetTimeOutGameStartMessageSize` (NetPacket.cpp:615).
    pub fn get_time_out_game_start_message_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + 1 + 1 + 1
    }

    /// `GetWrapperCommandSize` (NetPacket.cpp:630).
    pub fn get_wrapper_command_size(_msg: &NetCommandMsg) -> u32 {
        let mut msglen: u32 = 0;
        msglen += 1; // 'T'
        msglen += 1; // command type
        msglen += 1 + 1; // 'P' and player ID
        msglen += 1 + 2; // 'C' and command ID
        msglen += 1 + 1; // 'R' and relay
        msglen += 1; // 'D'
        msglen += 2; // m_wrappedCommandID
        msglen += 4; // m_chunkNumber
        msglen += 4; // m_numChunks
        msglen += 4; // m_totalDataLength
        msglen += 4; // m_dataLength
        msglen += 4; // m_dataOffset
        msglen
    }

    /// `GetFileCommandSize` (NetPacket.cpp:650).
    pub fn get_file_command_size(msg: &NetCommandMsg) -> u32 {
        let mut msglen: u32 = 0;
        msglen += 1 + 1; // 'T' and command type
        msglen += 1 + 1; // 'P' and player ID
        msglen += 1 + 2; // 'C' and command ID
        msglen += 1 + 1; // 'R' and relay
        msglen += 1; // 'D'
        msglen += msg.get_portable_filename().get_length() as u32 + 1; // PORTABLE filename and the terminating 0
        msglen += 4; // file data length
        msglen += msg.get_file_length(); // the file data
        msglen
    }

    /// `GetFileAnnounceCommandSize` (NetPacket.cpp:667).
    pub fn get_file_announce_command_size(msg: &NetCommandMsg) -> u32 {
        let mut msglen: u32 = 0;
        msglen += 1 + 1;
        msglen += 1 + 1;
        msglen += 1 + 2;
        msglen += 1 + 1;
        msglen += 1;
        msglen += msg.get_portable_filename().get_length() as u32 + 1;
        msglen += 2; // m_fileID
        msglen += 1; // m_playerMask
        msglen
    }

    /// `GetFileProgressCommandSize` (NetPacket.cpp:684).
    pub fn get_file_progress_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (1 + 2) + (1 + 1) + 1 + 2 + 4
    }

    /// `GetDisconnectFrameCommandSize` (NetPacket.cpp:699).
    pub fn get_disconnect_frame_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (1 + 2) + (1 + 1) + 1 + 4
    }

    /// `GetDisconnectScreenOffCommandSize` (NetPacket.cpp:712).
    pub fn get_disconnect_screen_off_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (1 + 2) + (1 + 1) + 1 + 4
    }

    /// `GetFrameResendRequestCommandSize` (NetPacket.cpp:725).
    pub fn get_frame_resend_request_command_size(_msg: &NetCommandMsg) -> u32 {
        1 + 1 + (1 + 1) + (1 + 2) + (1 + 1) + 1 + 4
    }

    // ---- FillBufferWith* ----------------------------------------------------------------------------------

    /// `NetPacket::FillBufferWithCommand` (NetPacket.cpp:739): appends the command's uncompressed bytes (upstream
    /// writes them at the start of a buffer sized by [`NetPacket::get_buffer_size_needed_for_command`]; wrapper
    /// commands have no fill function and write nothing).
    pub fn fill_buffer_with_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        use NetCommandType::*;
        let ty = r.get_command().read().get_net_command_type();
        match ty {
            GameCommand => NetPacket::fill_buffer_with_game_command(buffer, r),
            AckStage1 | AckStage2 | AckBoth => NetPacket::fill_buffer_with_ack_command(buffer, r),
            FrameInfo => NetPacket::fill_buffer_with_frame_command(buffer, r),
            PlayerLeave => NetPacket::fill_buffer_with_player_leave_command(buffer, r),
            RunAheadMetrics => NetPacket::fill_buffer_with_run_ahead_metrics_command(buffer, r),
            RunAhead => NetPacket::fill_buffer_with_run_ahead_command(buffer, r),
            DestroyPlayer => NetPacket::fill_buffer_with_destroy_player_command(buffer, r),
            KeepAlive => NetPacket::fill_buffer_with_keep_alive_command(buffer, r),
            DisconnectKeepAlive => NetPacket::fill_buffer_with_disconnect_keep_alive_command(buffer, r),
            DisconnectPlayer => NetPacket::fill_buffer_with_disconnect_player_command(buffer, r),
            PacketRouterQuery => NetPacket::fill_buffer_with_packet_router_query_command(buffer, r),
            PacketRouterAck => NetPacket::fill_buffer_with_packet_router_ack_command(buffer, r),
            DisconnectChat => NetPacket::fill_buffer_with_disconnect_chat_command(buffer, r),
            DisconnectVote => NetPacket::fill_buffer_with_disconnect_vote_command(buffer, r),
            Chat => NetPacket::fill_buffer_with_chat_command(buffer, r),
            Progress => NetPacket::fill_buffer_with_progress_message(buffer, r),
            LoadComplete => NetPacket::fill_buffer_with_load_complete_message(buffer, r),
            TimeoutStart => NetPacket::fill_buffer_with_time_out_game_start_message(buffer, r),
            File => NetPacket::fill_buffer_with_file_message(buffer, r),
            FileAnnounce => NetPacket::fill_buffer_with_file_announce_message(buffer, r),
            FileProgress => NetPacket::fill_buffer_with_file_progress_message(buffer, r),
            DisconnectFrame => NetPacket::fill_buffer_with_disconnect_frame_message(buffer, r),
            DisconnectScreenOff => NetPacket::fill_buffer_with_disconnect_screen_off_message(buffer, r),
            FrameResendRequest => NetPacket::fill_buffer_with_frame_resend_request_message(buffer, r),
            _ => {
                // DEBUG_CRASH("Unknown NETCOMMANDTYPE %d")
            }
        }
    }

    /// The common head of the `FillBufferWith*` functions: `T type`, then (per `frame`) `F`, `R relay`, `P player`,
    /// `C id`, `D`. `frame` is where `F` goes (`None` for no `F`); `with_id` writes `C`.
    fn fill_header(buffer: &mut Vec<u8>, r: &NetCommandRef, frame: FramePos, with_id: bool) {
        let m = r.get_command().read();
        wr_u8(buffer, b'T');
        wr_u8(buffer, m.get_net_command_type() as i32 as u8);
        if frame == FramePos::BeforeRelay {
            wr_u8(buffer, b'F');
            wr_u32(buffer, m.get_execution_frame());
        }
        wr_u8(buffer, b'R');
        wr_u8(buffer, r.get_relay());
        if frame == FramePos::AfterRelay {
            wr_u8(buffer, b'F');
            wr_u32(buffer, m.get_execution_frame());
        }
        wr_u8(buffer, b'P');
        wr_u8(buffer, m.get_player_id() as u8);
        if with_id {
            wr_u8(buffer, b'C');
            wr_u16(buffer, m.get_id());
        }
        wr_u8(buffer, b'D');
    }

    /// `FillBufferWithGameCommand` (NetPacket.cpp:824): `T F R P C D`, the message type (4 bytes), the number of
    /// argument-type runs, each run's type and count, then every argument.
    pub fn fill_buffer_with_game_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::BeforeRelay, true);
        let (ty, args) = game_message_view(&r.get_command().read());
        wr_u32(buffer, ty.0 as u32);
        let runs = parser_runs(&args);
        wr_u8(buffer, runs.len() as u8);
        for (t, count) in &runs {
            wr_u8(buffer, *t as i32 as u8);
            wr_u8(buffer, *count as u8);
        }
        for a in &args {
            write_game_message_argument(buffer, a.ty, a.data);
        }
    }

    /// `FillBufferWithAckCommand` (NetPacket.cpp:944): `T type P player D`, command id, original player id. (Upstream
    /// reads the ids through the `NetCommandRef` cast, see the module doc; the ack's own fields are used here.)
    pub fn fill_buffer_with_ack_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        let m = r.get_command().read();
        wr_u8(buffer, b'T');
        wr_u8(buffer, m.get_net_command_type() as i32 as u8);
        wr_u8(buffer, b'P');
        wr_u8(buffer, m.get_player_id() as u8);
        wr_u8(buffer, b'D');
        wr_u16(buffer, m.get_command_id());
        wr_u8(buffer, m.get_original_player_id());
    }

    /// `FillBufferWithFrameCommand` (NetPacket.cpp:988): `T F R P C D`, the command count.
    pub fn fill_buffer_with_frame_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::BeforeRelay, true);
        wr_u16(buffer, r.get_command().read().get_command_count());
    }

    /// `FillBufferWithPlayerLeaveCommand` (NetPacket.cpp:1041): `T R F P C D`, the leaving player id.
    pub fn fill_buffer_with_player_leave_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::AfterRelay, true);
        wr_u8(buffer, r.get_command().read().get_leaving_player_id());
    }

    /// `FillBufferWithRunAheadMetricsCommand` (NetPacket.cpp:1091): `T R P C D`, the average latency (Real), the
    /// average fps (UnsignedShort).
    pub fn fill_buffer_with_run_ahead_metrics_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        wr_u32(buffer, m.get_average_latency().to_bits());
        wr_u16(buffer, m.get_average_fps() as u16);
    }

    /// `FillBufferWithRunAheadCommand` (NetPacket.cpp:1139): `T R F P C D`, run ahead (u16), frame rate (u8).
    pub fn fill_buffer_with_run_ahead_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::AfterRelay, true);
        let m = r.get_command().read();
        wr_u16(buffer, m.get_run_ahead());
        wr_u8(buffer, m.get_frame_rate());
    }

    /// `FillBufferWithDestroyPlayerCommand` (NetPacket.cpp:1195): `T R F P C D`, the player index (u32).
    pub fn fill_buffer_with_destroy_player_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::AfterRelay, true);
        wr_u32(buffer, r.get_command().read().get_player_index());
    }

    /// `FillBufferWithKeepAliveCommand` (NetPacket.cpp:1245): `T R P D`.
    pub fn fill_buffer_with_keep_alive_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
    }

    /// `FillBufferWithDisconnectKeepAliveCommand` (NetPacket.cpp:1271): `T R P D`.
    pub fn fill_buffer_with_disconnect_keep_alive_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
    }

    /// `FillBufferWithDisconnectPlayerCommand` (NetPacket.cpp:1298): `T R P C D`, slot (u8), disconnect frame (u32).
    pub fn fill_buffer_with_disconnect_player_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        wr_u8(buffer, m.get_disconnect_slot());
        wr_u32(buffer, m.get_disconnect_frame());
    }

    /// `FillBufferWithPacketRouterQueryCommand` (NetPacket.cpp:1345): `T R P D`.
    pub fn fill_buffer_with_packet_router_query_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
    }

    /// `FillBufferWithPacketRouterAckCommand` (NetPacket.cpp:1375): `T R P D`.
    pub fn fill_buffer_with_packet_router_ack_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
    }

    /// `FillBufferWithDisconnectChatCommand` (NetPacket.cpp:1406): `T R P D`, length (u8), the text (u16 each).
    pub fn fill_buffer_with_disconnect_chat_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
        let text = r.get_command().read().get_text();
        NetPacket::write_text(buffer, &text);
    }

    /// `FillBufferWithDisconnectVoteCommand` (NetPacket.cpp:1444): `T R P C D`, slot (u8), vote frame (u32).
    pub fn fill_buffer_with_disconnect_vote_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        wr_u8(buffer, m.get_vote_slot());
        wr_u32(buffer, m.get_vote_frame());
    }

    /// `FillBufferWithChatCommand` (NetPacket.cpp:1491): `T F R P C D`, length, text, player mask (i32).
    pub fn fill_buffer_with_chat_command(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::BeforeRelay, true);
        let m = r.get_command().read();
        NetPacket::write_text(buffer, &m.get_text());
        wr_u32(buffer, m.get_chat_player_mask() as u32);
    }

    /// `FillBufferWithProgressMessage` (NetPacket.cpp:1549): `T R P D`, the percentage.
    pub fn fill_buffer_with_progress_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, false);
        wr_u8(buffer, r.get_command().read().get_percentage());
    }

    /// `FillBufferWithLoadCompleteMessage` (NetPacket.cpp:1579): `T R P C D`.
    pub fn fill_buffer_with_load_complete_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
    }

    /// `FillBufferWithTimeOutGameStartMessage` (NetPacket.cpp:1612): `T R P C D`.
    pub fn fill_buffer_with_time_out_game_start_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
    }

    /// `FillBufferWithFileMessage` (NetPacket.cpp:1644): `T R P C D`, the portable filename and its 0, the data
    /// length (u32), the data.
    pub fn fill_buffer_with_file_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        buffer.extend_from_slice(m.get_portable_filename().str());
        wr_u8(buffer, 0);
        wr_u32(buffer, m.get_file_length());
        if let Some(d) = m.get_file_data() {
            buffer.extend_from_slice(d);
        }
    }

    /// `FillBufferWithFileAnnounceMessage` (NetPacket.cpp:1693): `T R P C D`, filename and 0, file id (u16),
    /// player mask (u8).
    pub fn fill_buffer_with_file_announce_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        buffer.extend_from_slice(m.get_portable_filename().str());
        wr_u8(buffer, 0);
        wr_u16(buffer, m.get_file_id());
        wr_u8(buffer, m.get_file_player_mask());
    }

    /// `FillBufferWithFileProgressMessage` (NetPacket.cpp:1743): `T R P C D`, file id (u16), progress (i32).
    pub fn fill_buffer_with_file_progress_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        let m = r.get_command().read();
        wr_u16(buffer, m.get_file_id());
        wr_u32(buffer, m.get_progress() as u32);
    }

    /// `FillBufferWithDisconnectFrameMessage` (NetPacket.cpp:1785): `T R P C D`, disconnect frame (u32).
    pub fn fill_buffer_with_disconnect_frame_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        wr_u32(buffer, r.get_command().read().get_disconnect_frame());
    }

    /// `FillBufferWithDisconnectScreenOffMessage` (NetPacket.cpp:1823): `T R P C D`, new frame (u32).
    pub fn fill_buffer_with_disconnect_screen_off_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        wr_u32(buffer, r.get_command().read().get_new_frame());
    }

    /// `FillBufferWithFrameResendRequestMessage` (NetPacket.cpp:1861): `T R P C D`, frame to resend (u32).
    pub fn fill_buffer_with_frame_resend_request_message(buffer: &mut Vec<u8>, r: &NetCommandRef) {
        NetPacket::fill_header(buffer, r, FramePos::None, true);
        wr_u32(buffer, r.get_command().read().get_frame_to_resend());
    }

    /// The `length` byte and `length * sizeof(UnsignedShort)` text bytes of the chat commands.
    fn write_text(buffer: &mut Vec<u8>, text: &UnicodeString) {
        let length = (text.get_length() & 0xff) as usize;
        wr_u8(buffer, length as u8);
        for k in 0..length {
            wr_u16(buffer, text.str().get(k).copied().unwrap_or(0));
        }
    }

    // ---- addCommand and the add*/isRoomFor* families --------------------------------------------------------

    /// The length of the header fields that would have to be written for `cmd`/`relay` (the `len` that every
    /// `isRoomFor*` builds), including the `D`.
    fn header_len(&self, cmd: &NetCommandMsg, relay: u8, layout: Layout) -> i32 {
        let mut len = 0;
        let mut need_new_command_id = false;
        if self.last_command_type as i32 != cmd.get_net_command_type() as i32 {
            len += 1;
            len += 1;
        }
        if layout.frame != FramePos::None && self.last_frame != cmd.get_execution_frame() {
            len += 4 + 1;
        }
        if self.last_relay != relay {
            len += 1 + 1;
        }
        if self.last_player_id as u32 != cmd.get_player_id() {
            len += 1;
            len += 1;
            need_new_command_id = true;
        }
        if layout.room_counts_cmd_id
            && (((self.last_command_id as i32 + 1) != (cmd.get_id() as i32)) || need_new_command_id)
        {
            len += 2 + 1;
        }
        len += 1; // for 'D'
        len
    }

    /// The shared body of every `isRoomFor*Message`: `(len + m_packetLen) > MAX_PACKET_SIZE` is "no room".
    fn is_room_for(&self, r: &NetCommandRef, layout: Layout, payload_len: i32) -> bool {
        let m = r.get_command().read();
        let len = self.header_len(&m, r.get_relay(), layout) + payload_len;
        if len + self.packet.len() as i32 > MAX_PACKET_SIZE {
            return false;
        }
        true
    }

    /// The shared body of every `add*Command`/`add*Message` that carries a `C` or not: writes the changed header
    /// fields in `layout`'s order, then `D`, `payload`, bumps `m_numCommands` and replaces `m_lastCommand`.
    fn add_with_layout(&mut self, r: &NetCommandRef, layout: Layout, payload: &[u8]) -> bool {
        if !self.is_room_for(r, layout, payload.len() as i32) {
            return false;
        }
        {
            let cmd = r.get_command().read();
            let mut need_new_command_id = false;
            if self.last_command_type as i32 != cmd.get_net_command_type() as i32 {
                self.packet.push(b'T');
                self.packet.push(cmd.get_net_command_type() as i32 as u8);
                self.last_command_type = cmd.get_net_command_type() as i32 as u8;
            }
            if layout.frame == FramePos::BeforeRelay {
                self.write_frame_if_changed(&cmd);
            }
            if self.last_relay != r.get_relay() {
                self.packet.push(b'R');
                self.packet.push(r.get_relay());
                self.last_relay = r.get_relay();
            }
            if layout.frame == FramePos::AfterRelay {
                self.write_frame_if_changed(&cmd);
            }
            if self.last_player_id as u32 != cmd.get_player_id() {
                self.packet.push(b'P');
                self.packet.push(cmd.get_player_id() as u8);
                self.last_player_id = cmd.get_player_id() as u8;
                need_new_command_id = true;
            }
            if layout.cmd_id {
                if ((self.last_command_id as i32 + 1) != (cmd.get_id() as i32)) || need_new_command_id {
                    self.packet.push(b'C');
                    wr_u16(&mut self.packet, cmd.get_id());
                }
                self.last_command_id = cmd.get_id();
            }
            self.packet.push(b'D');
            self.packet.extend_from_slice(payload);
        }
        self.num_commands += 1;
        self.set_last_command(r);
        true
    }

    fn write_frame_if_changed(&mut self, cmd: &NetCommandMsg) {
        if self.last_frame != cmd.get_execution_frame() {
            self.packet.push(b'F');
            wr_u32(&mut self.packet, cmd.get_execution_frame());
            self.last_frame = cmd.get_execution_frame();
        }
    }

    /// `m_lastCommand->deleteInstance(); m_lastCommand = NEW_NETCOMMANDREF(msg->getCommand());
    /// m_lastCommand->setRelay(msg->getRelay());`
    fn set_last_command(&mut self, r: &NetCommandRef) {
        self.last_command = None;
        let mut nr = NetCommandRef::new(r.get_command().clone());
        nr.set_relay(r.get_relay());
        self.last_command = Some(nr);
    }

    /// `NetPacket::addCommand` (NetPacket.cpp:1968): adds the command to the packet; false if there was not enough
    /// room. A command type without an add function is "added" (true) without writing anything.
    pub fn add_command(&mut self, r: &NetCommandRef) -> bool {
        use NetCommandType::*;
        let ty = r.get_command().read().get_net_command_type();
        match ty {
            GameCommand => self.add_game_command(r),
            AckStage1 => self.add_ack_stage1_command(r),
            AckStage2 => self.add_ack_stage2_command(r),
            AckBoth => self.add_ack_both_command(r),
            FrameInfo => self.add_frame_command(r),
            PlayerLeave => self.add_player_leave_command(r),
            RunAheadMetrics => self.add_run_ahead_metrics_command(r),
            RunAhead => self.add_run_ahead_command(r),
            DestroyPlayer => self.add_destroy_player_command(r),
            KeepAlive => self.add_keep_alive_command(r),
            DisconnectKeepAlive => self.add_disconnect_keep_alive_command(r),
            DisconnectPlayer => self.add_disconnect_player_command(r),
            PacketRouterQuery => self.add_packet_router_query_command(r),
            PacketRouterAck => self.add_packet_router_ack_command(r),
            DisconnectChat => self.add_disconnect_chat_command(r),
            DisconnectVote => self.add_disconnect_vote_command(r),
            Chat => self.add_chat_command(r),
            Progress => self.add_progress_message(r),
            LoadComplete => self.add_load_complete_message(r),
            TimeoutStart => self.add_time_out_game_start_message(r),
            Wrapper => self.add_wrapper_command(r),
            File => self.add_file_command(r),
            FileAnnounce => self.add_file_announce_command(r),
            FileProgress => self.add_file_progress_command(r),
            DisconnectFrame => self.add_disconnect_frame_command(r),
            DisconnectScreenOff => self.add_disconnect_screen_off_command(r),
            FrameResendRequest => self.add_frame_resend_request_command(r),
            _ => {
                // DEBUG_CRASH("Unknown NETCOMMANDTYPE %d")
                true
            }
        }
    }

    // payload builders: the bytes after 'D' for each command, in the order the add function writes them

    fn payload_frame_resend_request(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u32(&mut p, m.get_frame_to_resend());
        p
    }
    fn payload_disconnect_screen_off(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u32(&mut p, m.get_new_frame());
        p
    }
    fn payload_disconnect_frame(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u32(&mut p, m.get_disconnect_frame());
        p
    }
    fn payload_file(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(m.get_portable_filename().str());
        wr_u8(&mut p, 0);
        wr_u32(&mut p, m.get_file_length());
        if let Some(d) = m.get_file_data() {
            p.extend_from_slice(d);
        }
        p
    }
    fn payload_file_announce(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(m.get_portable_filename().str());
        wr_u8(&mut p, 0);
        wr_u16(&mut p, m.get_file_id());
        wr_u8(&mut p, m.get_file_player_mask());
        p
    }
    fn payload_file_progress(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u16(&mut p, m.get_file_id());
        wr_u32(&mut p, m.get_progress() as u32);
        p
    }
    fn payload_wrapper(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u16(&mut p, m.get_wrapped_command_id());
        wr_u32(&mut p, m.get_chunk_number());
        wr_u32(&mut p, m.get_num_chunks());
        wr_u32(&mut p, m.get_total_data_length());
        wr_u32(&mut p, m.get_wrapper_data_length());
        wr_u32(&mut p, m.get_data_offset());
        let len = m.get_wrapper_data_length() as usize;
        if let Some(d) = m.get_wrapper_data() {
            p.extend_from_slice(&d[..min(len, d.len())]);
        }
        p
    }
    fn payload_disconnect_vote(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u8(&mut p, m.get_vote_slot());
        wr_u32(&mut p, m.get_vote_frame());
        p
    }
    fn payload_disconnect_player(m: &NetCommandMsg) -> Vec<u8> {
        let mut p = Vec::new();
        wr_u8(&mut p, m.get_disconnect_slot());
        wr_u32(&mut p, m.get_disconnect_frame());
        p
    }
    fn payload_text(text: &UnicodeString) -> Vec<u8> {
        let mut p = Vec::new();
        NetPacket::write_text(&mut p, text);
        p
    }

    /// `addFrameResendRequestCommand` (NetPacket.cpp:2050): `T F R P C D` + frame to resend.
    pub fn add_frame_resend_request_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_frame_resend_request(&r.get_command().read());
        self.add_with_layout(r, TFRPCD, &p)
    }
    /// `isRoomForFrameResendRequestMessage` (NetPacket.cpp:2130).
    pub fn is_room_for_frame_resend_request_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TFRPCD, 4)
    }

    /// `addDisconnectScreenOffCommand` (NetPacket.cpp:2161): `T F R P C D` + new frame.
    pub fn add_disconnect_screen_off_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_disconnect_screen_off(&r.get_command().read());
        self.add_with_layout(r, TFRPCD, &p)
    }
    /// `isRoomForDisconnectScreenOffMessage` (NetPacket.cpp:2241).
    pub fn is_room_for_disconnect_screen_off_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TFRPCD, 4)
    }

    /// `addDisconnectFrameCommand` (NetPacket.cpp:2272): `T F R P C D` + disconnect frame.
    pub fn add_disconnect_frame_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_disconnect_frame(&r.get_command().read());
        self.add_with_layout(r, TFRPCD, &p)
    }
    /// `isRoomForDisconnectFrameMessage` (NetPacket.cpp:2352).
    pub fn is_room_for_disconnect_frame_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TFRPCD, 4)
    }

    /// `addFileCommand` (NetPacket.cpp:2383): `T R P C D` + filename, 0, length, data.
    pub fn add_file_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_file(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForFileMessage` (NetPacket.cpp:2457).
    pub fn is_room_for_file_message(&self, r: &NetCommandRef) -> bool {
        let n = NetPacket::payload_file(&r.get_command().read()).len() as i32;
        self.is_room_for(r, TRPCD, n)
    }

    /// `addFileAnnounceCommand` (NetPacket.cpp:2487): `T R P C D` + filename, 0, file id, player mask.
    pub fn add_file_announce_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_file_announce(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForFileAnnounceMessage` (NetPacket.cpp:2566).
    pub fn is_room_for_file_announce_message(&self, r: &NetCommandRef) -> bool {
        let n = NetPacket::payload_file_announce(&r.get_command().read()).len() as i32;
        self.is_room_for(r, TRPCD, n)
    }

    /// `addFileProgressCommand` (NetPacket.cpp:2596): `T R P C D` + file id, progress.
    pub fn add_file_progress_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_file_progress(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForFileProgressMessage` (NetPacket.cpp:2668).
    pub fn is_room_for_file_progress_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD, 6)
    }

    /// `addWrapperCommand` (NetPacket.cpp:2697): `T R P C D` + wrapped command id, chunk number, number of chunks,
    /// total data length, data length, data offset, the chunk's data.
    pub fn add_wrapper_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_wrapper(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForWrapperMessage` (NetPacket.cpp:2800).
    pub fn is_room_for_wrapper_message(&self, r: &NetCommandRef) -> bool {
        let n = NetPacket::payload_wrapper(&r.get_command().read()).len() as i32;
        self.is_room_for(r, TRPCD, n)
    }

    /// `addTimeOutGameStartMessage` (NetPacket.cpp:2837): `T R P C D`.
    pub fn add_time_out_game_start_message(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPCD_UNCOUNTED, &[])
    }
    /// `isRoomForTimeOutGameStartMessage` (NetPacket.cpp:2904): does not count the `C` field.
    pub fn is_room_for_time_out_game_start_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD_UNCOUNTED, 0)
    }

    /// `addLoadCompleteMessage` (NetPacket.cpp:2931): `T R P C D`.
    pub fn add_load_complete_message(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPCD_UNCOUNTED, &[])
    }
    /// `isRoomForLoadCompleteMessage` (NetPacket.cpp:2998): does not count the `C` field.
    pub fn is_room_for_load_complete_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD_UNCOUNTED, 0)
    }

    /// `addProgressMessage` (NetPacket.cpp:3026): `T R P D` + percentage.
    pub fn add_progress_message(&mut self, r: &NetCommandRef) -> bool {
        let p = [r.get_command().read().get_percentage()];
        self.add_with_layout(r, TRPD, &p)
    }
    /// `isRoomForProgressMessage` (NetPacket.cpp:3084).
    pub fn is_room_for_progress_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPD, 1)
    }

    /// `addDisconnectVoteCommand` (NetPacket.cpp:3109): `T R P C D` + slot, vote frame.
    pub fn add_disconnect_vote_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_disconnect_vote(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForDisconnectVoteMessage` (NetPacket.cpp:3192).
    pub fn is_room_for_disconnect_vote_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD, 5)
    }

    /// `addDisconnectChatCommand` (NetPacket.cpp:3222): `T R P D` + length, text.
    pub fn add_disconnect_chat_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_text(&r.get_command().read().get_text());
        self.add_with_layout(r, TRPD, &p)
    }
    /// `isRoomForDisconnectChatMessage` (NetPacket.cpp:3288).
    pub fn is_room_for_disconnect_chat_message(&self, r: &NetCommandRef) -> bool {
        let n = NetPacket::payload_text(&r.get_command().read().get_text()).len() as i32;
        self.is_room_for(r, TRPD, n)
    }

    /// `addChatCommand` (NetPacket.cpp:3313): `T F R P C D` + length, text, player mask.
    pub fn add_chat_command(&mut self, r: &NetCommandRef) -> bool {
        let mut p;
        {
            let m = r.get_command().read();
            p = NetPacket::payload_text(&m.get_text());
            wr_u32(&mut p, m.get_chat_player_mask() as u32);
        }
        self.add_with_layout(r, TFRPCD, &p)
    }
    /// `isRoomForChatMessage` (NetPacket.cpp:3405).
    pub fn is_room_for_chat_message(&self, r: &NetCommandRef) -> bool {
        let n = NetPacket::payload_text(&r.get_command().read().get_text()).len() as i32 + 4;
        self.is_room_for(r, TFRPCD, n)
    }

    /// `addPacketRouterAckCommand` (NetPacket.cpp:3439): `T R P D`.
    pub fn add_packet_router_ack_command(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPD, &[])
    }
    /// `isRoomForPacketRouterAckMessage` (NetPacket.cpp:3499).
    pub fn is_room_for_packet_router_ack_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPD, 0)
    }

    /// `addPacketRouterQueryCommand` (NetPacket.cpp:3521): `T R P D`.
    pub fn add_packet_router_query_command(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPD, &[])
    }
    /// `isRoomForPacketRouterQueryMessage` (NetPacket.cpp:3581).
    pub fn is_room_for_packet_router_query_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPD, 0)
    }

    /// `addDisconnectPlayerCommand` (NetPacket.cpp:3603): `T R P C D` + slot, disconnect frame.
    pub fn add_disconnect_player_command(&mut self, r: &NetCommandRef) -> bool {
        let p = NetPacket::payload_disconnect_player(&r.get_command().read());
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForDisconnectPlayerMessage` (NetPacket.cpp:3686).
    pub fn is_room_for_disconnect_player_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD, 5)
    }

    /// `addDisconnectKeepAliveCommand` (NetPacket.cpp:3719): `T R P D`.
    pub fn add_disconnect_keep_alive_command(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPD, &[])
    }
    /// `isRoomForDisconnectKeepAliveMessage` (NetPacket.cpp:3774).
    pub fn is_room_for_disconnect_keep_alive_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPD, 0)
    }

    /// `addKeepAliveCommand` (NetPacket.cpp:3799): `T R P D`.
    pub fn add_keep_alive_command(&mut self, r: &NetCommandRef) -> bool {
        self.add_with_layout(r, TRPD, &[])
    }
    /// `isRoomForKeepAliveMessage` (NetPacket.cpp:3854).
    pub fn is_room_for_keep_alive_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPD, 0)
    }

    /// `addRunAheadCommand` (NetPacket.cpp:3879): `T R F P C D` + run ahead (u16), frame rate (u8).
    pub fn add_run_ahead_command(&mut self, r: &NetCommandRef) -> bool {
        let mut p = Vec::new();
        {
            let m = r.get_command().read();
            wr_u16(&mut p, m.get_run_ahead());
            wr_u8(&mut p, m.get_frame_rate());
        }
        self.add_with_layout(r, TRFPCD, &p)
    }
    /// `isRoomForRunAheadMessage` (NetPacket.cpp:3970).
    pub fn is_room_for_run_ahead_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRFPCD, 3)
    }

    /// `addDestroyPlayerCommand` (NetPacket.cpp:4005): `T R F P C D` + player index (u32).
    pub fn add_destroy_player_command(&mut self, r: &NetCommandRef) -> bool {
        let mut p = Vec::new();
        wr_u32(&mut p, r.get_command().read().get_player_index());
        self.add_with_layout(r, TRFPCD, &p)
    }
    /// `isRoomForDestroyPlayerMessage` (NetPacket.cpp:4091).
    pub fn is_room_for_destroy_player_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRFPCD, 4)
    }

    /// `addRunAheadMetricsCommand` (NetPacket.cpp:4125): `T R P C D` + average latency (Real), average fps (u16).
    pub fn add_run_ahead_metrics_command(&mut self, r: &NetCommandRef) -> bool {
        let mut p = Vec::new();
        {
            let m = r.get_command().read();
            wr_u32(&mut p, m.get_average_latency().to_bits());
            wr_u16(&mut p, m.get_average_fps() as u16);
        }
        self.add_with_layout(r, TRPCD, &p)
    }
    /// `isRoomForRunAheadMetricsMessage` (NetPacket.cpp:4205).
    pub fn is_room_for_run_ahead_metrics_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRPCD, 6)
    }

    /// `addPlayerLeaveCommand` (NetPacket.cpp:4238): `T R F P C D` + leaving player id.
    pub fn add_player_leave_command(&mut self, r: &NetCommandRef) -> bool {
        let p = [r.get_command().read().get_leaving_player_id()];
        self.add_with_layout(r, TRFPCD, &p)
    }
    /// `isRoomForPlayerLeaveMessage` (NetPacket.cpp:4324).
    pub fn is_room_for_player_leave_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TRFPCD, 1)
    }

    /// `addFrameCommand` (NetPacket.cpp:4358): a frame command that repeats the last (the next frame, no
    /// commands, the next id, same relay) is a single `Z`; otherwise `T F R P C D` + command count.
    pub fn add_frame_command(&mut self, r: &NetCommandRef) -> bool {
        if self.is_frame_repeat(r) {
            if self.packet.len() >= MAX_PACKET {
                return false;
            }
            self.packet.push(b'Z');
            self.last_command_id = r.get_command().read().get_id();
            self.set_last_command(r);
            self.last_frame = self.last_frame.wrapping_add(1); // actually advancing to the next frame
            self.num_commands += 1;
            return true;
        }
        let mut p = Vec::new();
        wr_u16(&mut p, r.get_command().read().get_command_count());
        self.add_with_layout(r, TFRPCD, &p)
    }
    /// `isRoomForFrameMessage` (NetPacket.cpp:4463).
    pub fn is_room_for_frame_message(&self, r: &NetCommandRef) -> bool {
        self.is_room_for(r, TFRPCD, 2)
    }

    /// `isFrameRepeat` (NetPacket.cpp:4494).
    pub fn is_frame_repeat(&self, r: &NetCommandRef) -> bool {
        let last = match &self.last_command {
            None => return false,
            Some(l) => l,
        };
        if last.get_command().read().get_net_command_type() != NetCommandType::FrameInfo {
            return false;
        }
        let framemsg = r.get_command().read();
        let lastmsg = last.get_command().read();
        if framemsg.get_command_count() != 0 {
            return false;
        }
        if framemsg.get_execution_frame() != lastmsg.get_execution_frame().wrapping_add(1) {
            return false;
        }
        if r.get_relay() != last.get_relay() {
            return false;
        }
        if framemsg.get_id() as i32 != lastmsg.get_id() as i32 + 1 {
            return false;
        }
        true
    }

    /// `addAckBothCommand` (NetPacket.cpp:4521).
    pub fn add_ack_both_command(&mut self, r: &NetCommandRef) -> bool {
        let (cid, pid) = {
            let m = r.get_command().read();
            (m.get_command_id(), m.get_original_player_id())
        };
        self.add_ack_command(r, cid, pid)
    }
    /// `addAckStage1Command` (NetPacket.cpp:4529).
    pub fn add_ack_stage1_command(&mut self, r: &NetCommandRef) -> bool {
        let (cid, pid) = {
            let m = r.get_command().read();
            (m.get_command_id(), m.get_original_player_id())
        };
        self.add_ack_command(r, cid, pid)
    }
    /// `addAckStage2Command` (NetPacket.cpp:4537).
    pub fn add_ack_stage2_command(&mut self, r: &NetCommandRef) -> bool {
        let (cid, pid) = {
            let m = r.get_command().read();
            (m.get_command_id(), m.get_original_player_id())
        };
        self.add_ack_command(r, cid, pid)
    }

    /// `addAckCommand` (NetPacket.cpp:4545): an ack that repeats the last (the next command id, same original
    /// player, same relay) is a single `Z`; otherwise `T P D` + command id, original player id (no `R`, no `C`).
    pub fn add_ack_command(&mut self, r: &NetCommandRef, command_id: u16, original_player_id: u8) -> bool {
        if self.is_ack_repeat(r) {
            if self.packet.len() >= MAX_PACKET {
                return false;
            }
            self.packet.push(b'Z');
            self.num_commands += 1;
            self.set_last_command(r);
            return true;
        }
        if self.is_room_for_ack_message(r) {
            {
                let cmd = r.get_command().read();
                if self.last_command_type as i32 != cmd.get_net_command_type() as i32 {
                    self.packet.push(b'T');
                    self.packet.push(cmd.get_net_command_type() as i32 as u8);
                    self.last_command_type = cmd.get_net_command_type() as i32 as u8;
                }
                if self.last_player_id as u32 != cmd.get_player_id() {
                    self.packet.push(b'P');
                    self.packet.push(cmd.get_player_id() as u8);
                    self.last_player_id = cmd.get_player_id() as u8;
                }
            }
            self.packet.push(b'D');
            wr_u16(&mut self.packet, command_id);
            wr_u8(&mut self.packet, original_player_id);
            self.set_last_command(r);
            self.num_commands += 1;
            return true;
        }
        false
    }

    /// `isRoomForAckMessage` (NetPacket.cpp:4607).
    pub fn is_room_for_ack_message(&self, r: &NetCommandRef) -> bool {
        let cmd = r.get_command().read();
        let mut len = 0;
        if self.last_command_type as i32 != cmd.get_net_command_type() as i32 {
            len += 1;
            len += 1;
        }
        if self.last_player_id as u32 != cmd.get_player_id() {
            len += 1;
            len += 1;
        }
        len += 1; // for 'D'
        len += 2;
        len += 1;
        if len + self.packet.len() as i32 > MAX_PACKET_SIZE {
            return false;
        }
        true
    }

    /// `isAckRepeat` (NetPacket.cpp:4628).
    pub fn is_ack_repeat(&self, r: &NetCommandRef) -> bool {
        let last = match &self.last_command {
            None => return false,
            Some(l) => l,
        };
        let ty = r.get_command().read().get_net_command_type();
        if last.get_command().read().get_net_command_type() != ty {
            return false;
        }
        match ty {
            NetCommandType::AckBoth => self.is_ack_both_repeat(r),
            NetCommandType::AckStage1 => self.is_ack_stage1_repeat(r),
            NetCommandType::AckStage2 => self.is_ack_stage2_repeat(r),
            _ => false,
        }
    }

    fn is_ack_repeat_common(&self, r: &NetCommandRef) -> bool {
        let last = match &self.last_command {
            None => return false,
            Some(l) => l,
        };
        let ack = r.get_command().read();
        let last_ack = last.get_command().read();
        if last_ack.get_command_id() as i32 != ack.get_command_id() as i32 - 1 {
            return false;
        }
        if last_ack.get_original_player_id() != ack.get_original_player_id() {
            return false;
        }
        if r.get_relay() != last.get_relay() {
            return false;
        }
        true
    }

    /// `isAckBothRepeat` (NetPacket.cpp:4647).
    pub fn is_ack_both_repeat(&self, r: &NetCommandRef) -> bool {
        self.is_ack_repeat_common(r)
    }
    /// `isAckStage1Repeat` (NetPacket.cpp:4662): upstream casts both messages to the stage-2 type (same layout).
    pub fn is_ack_stage1_repeat(&self, r: &NetCommandRef) -> bool {
        self.is_ack_repeat_common(r)
    }
    /// `isAckStage2Repeat` (NetPacket.cpp:4677).
    pub fn is_ack_stage2_repeat(&self, r: &NetCommandRef) -> bool {
        self.is_ack_repeat_common(r)
    }

    /// `addGameCommand` (NetPacket.cpp:4695): `T F R P C D`, the message type (4 bytes), the number of run
    /// declarations, each run's type and count, then every argument.
    pub fn add_game_command(&mut self, r: &NetCommandRef) -> bool {
        let (ty, args) = game_message_view(&r.get_command().read());
        let mut p = Vec::new();
        wr_u32(&mut p, ty.0 as u32);
        let runs = parser_runs(&args);
        wr_u8(&mut p, runs.len() as u8);
        for (t, count) in &runs {
            wr_u8(&mut p, *t as i32 as u8);
            wr_u8(&mut p, *count as u8);
        }
        for a in &args {
            self.write_game_message_argument_to_packet_into(&mut p, a.ty, a.data);
        }
        if !self.is_room_for_game_message(r, &args) {
            return false;
        }
        self.add_with_layout(r, TFRPCD, &p)
    }

    fn write_game_message_argument_to_packet_into(&self, p: &mut Vec<u8>, ty: ArgumentDataType, arg: ArgumentData) {
        write_game_message_argument(p, ty, arg);
    }

    /// `writeGameMessageArgumentToPacket` (NetPacket.cpp:4821): appends one argument to the packet.
    pub fn write_game_message_argument_to_packet(&mut self, ty: ArgumentDataType, arg: ArgumentData) {
        write_game_message_argument(&mut self.packet, ty, arg);
    }

    /// `isRoomForGameMessage` (NetPacket.cpp:4861): upstream passes the constructed `GameMessage`; its arguments are
    /// given here. `msglen > MAX_PACKET_SIZE - m_packetLen` is "no room".
    pub fn is_room_for_game_message(&self, r: &NetCommandRef, args: &[GameMessageArgument]) -> bool {
        let cmd = r.get_command().read();
        let mut msglen: i32 = 0;
        let mut need_new_command_id = false;
        if self.last_frame != cmd.get_execution_frame() {
            msglen += 4 + 1;
        }
        if self.last_player_id as u32 != cmd.get_player_id() {
            msglen += 1 + 1;
            need_new_command_id = true;
        }
        if self.last_relay != r.get_relay() {
            msglen += 1 + 1;
        }
        if self.last_command_type as i32 != cmd.get_net_command_type() as i32 {
            msglen += 1 + 1;
        }
        if ((self.last_command_id as i32 + 1) != (cmd.get_id() as i32)) || need_new_command_id {
            msglen += 2 + 1;
        }
        let runs = parser_runs(args);
        msglen += 1; // for 'D'
        msglen += 4; // sizeof(GameMessage::Type)
        msglen += 1;
        msglen += 2 * runs.len() as i32;
        msglen += runs_data_size(&runs);
        if msglen > MAX_PACKET_SIZE - self.packet.len() as i32 {
            return false; // there isn't enough room in this packet for this new message
        }
        true
    }

    // ---- getCommandList --------------------------------------------------------------------------------------

    /// `NetPacket::getCommandList` (NetPacket.cpp:4935): decodes the packet into its commands. `T`/`F`/`P`/`R`/`C`
    /// set the running header, `D` reads a command (stamped with the header; the running command id advances for
    /// types that carry one), `Z` repeats the last ack or frame command.
    pub fn get_command_list(&self) -> NetCommandList {
        let mut retval = NetCommandList::new();
        retval.init();
        let data = &self.packet[..];
        let packet_len = data.len();
        let mut player_id: u8 = 0;
        let mut frame: u32 = 0;
        let mut command_id: u16 = 1;
        let mut command_type: u8 = 0;
        let mut relay: u8 = 0;
        let mut last_command: Option<NetCommandRef> = None;
        let mut i: usize = 0;
        while i < packet_len {
            match data[i] {
                b'T' => {
                    i += 1;
                    command_type = rd_u8(data, &mut i);
                }
                b'F' => {
                    i += 1;
                    frame = rd_u32(data, &mut i);
                }
                b'P' => {
                    i += 1;
                    player_id = rd_u8(data, &mut i);
                }
                b'R' => {
                    i += 1;
                    relay = rd_u8(data, &mut i);
                }
                b'C' => {
                    i += 1;
                    command_id = rd_u16(data, &mut i);
                }
                b'D' => {
                    i += 1;
                    let ty = NetCommandType::from_i32(command_type as i32);
                    let msg = ty.and_then(|t| NetPacket::read_message_of_type(t, data, &mut i));
                    let mut msg = match msg {
                        None => {
                            // DEBUG_CRASH("Didn't read a message from the packet. Things are about to go wrong.")
                            continue;
                        }
                        Some(m) => m,
                    };
                    let ty = ty.unwrap_or(NetCommandType::Unknown);
                    msg.set_execution_frame(frame);
                    msg.set_player_id(player_id as u32);
                    msg.set_net_command_type(ty);
                    msg.set_id(command_id);
                    if does_command_require_a_command_id(ty) {
                        command_id = command_id.wrapping_add(1);
                    }
                    let ptr = NetCommandMsgPtr::new(msg);
                    let rid = retval.add_message(ptr.clone());
                    if let Some(id) = rid {
                        if let Some(rr) = retval.get_mut(id) {
                            rr.set_relay(relay);
                        }
                    }
                    last_command = Some(NetCommandRef::new(ptr.clone()));
                    // msg->detach(): the creation reference
                    let _ = ptr.write().detach();
                }
                b'Z' => {
                    i += 1;
                    let last = match &last_command {
                        // DEBUG_CRASH("Got a repeat command with no command to repeat.") then a null dereference
                        None => continue,
                        Some(l) => l,
                    };
                    let (last_cid, last_orig) = {
                        let lm = last.get_command().read();
                        (lm.get_command_id(), lm.get_original_player_id())
                    };
                    let mut msg = if command_type as i32 == NetCommandType::AckStage1 as i32 {
                        let mut m = NetCommandMsg::new_ack_stage1();
                        m.set_command_id(last_cid.wrapping_add(1));
                        m.set_original_player_id(last_orig);
                        m
                    } else if command_type as i32 == NetCommandType::AckStage2 as i32 {
                        let mut m = NetCommandMsg::new_ack_stage2();
                        m.set_command_id(last_cid.wrapping_add(1));
                        m.set_original_player_id(last_orig);
                        m
                    } else if command_type as i32 == NetCommandType::AckBoth as i32 {
                        let mut m = NetCommandMsg::new_ack_both();
                        m.set_command_id(last_cid.wrapping_add(1));
                        m.set_original_player_id(last_orig);
                        m
                    } else if command_type as i32 == NetCommandType::FrameInfo as i32 {
                        let mut m = NetCommandMsg::new_frame();
                        frame = frame.wrapping_add(1); // this is set below
                        m.set_command_count(0);
                        m
                    } else {
                        // DEBUG_CRASH("Trying to repeat a command that shouldn't be repeated.")
                        continue;
                    };
                    let ty = NetCommandType::from_i32(command_type as i32).unwrap_or(NetCommandType::Unknown);
                    msg.set_execution_frame(frame);
                    msg.set_player_id(player_id as u32);
                    msg.set_net_command_type(ty);
                    msg.set_id(command_id);
                    if does_command_require_a_command_id(ty) {
                        command_id = command_id.wrapping_add(1);
                    }
                    let ptr = NetCommandMsgPtr::new(msg);
                    let rid = retval.add_message(ptr.clone());
                    if let Some(id) = rid {
                        if let Some(rr) = retval.get_mut(id) {
                            rr.set_relay(relay);
                        }
                    }
                    last_command = Some(NetCommandRef::new(ptr.clone()));
                    let _ = ptr.write().detach();
                }
                _ => {
                    // DEBUG_CRASH("Unrecognized packet entry, ignoring."); dumpPacketToLog();
                    i += 1;
                }
            }
        }
        retval
    }

    // ---- read*Message ----------------------------------------------------------------------------------------

    /// `readGameMessage` (NetPacket.cpp:5202): the message type, the run declarations, then the arguments run by
    /// run. `None` where upstream returns `NULL` (more arguments than the declared runs hold).
    pub fn read_game_message(data: &[u8], i: &mut usize) -> Option<NetCommandMsg> {
        let mut msg = NetCommandMsg::new_game_command();
        let new_type = rd_i32(data, i);
        msg.set_game_message_type(MessageType(new_type));
        let num_arg_types = rd_u8(data, i);
        let mut total_arg_count: i32 = 0;
        let mut parser: Vec<(ArgumentDataType, i32)> = Vec::new();
        for _j in 0..num_arg_types {
            let t = rd_u8(data, i);
            let arg_count = rd_u8(data, i);
            let ty = ArgumentDataType::from_i32(t as i32).unwrap_or(ArgumentDataType::Unknown);
            parser.push((ty, arg_count as i32));
            total_arg_count += arg_count as i32;
        }
        let mut idx: usize = 0;
        let mut lasttype = ArgumentDataType::Unknown;
        let mut args_left_for_type: i32 = 0;
        if let Some((t, c)) = parser.first() {
            lasttype = *t;
            args_left_for_type = *c;
        }
        for _j in 0..total_arg_count {
            read_game_message_argument_from_packet(lasttype, &mut msg, data, i);
            args_left_for_type -= 1;
            if args_left_for_type == 0 {
                // DEBUG_ASSERTCRASH(parserArgType != NULL, ...)
                if idx >= parser.len() {
                    return None;
                }
                idx += 1;
                if idx < parser.len() {
                    args_left_for_type = parser[idx].1;
                    lasttype = parser[idx].0;
                }
            }
        }
        Some(msg)
    }

    /// `readGameMessageArgumentFromPacket` (NetPacket.cpp:5267).
    pub fn read_game_message_argument_from_packet(
        ty: ArgumentDataType,
        msg: &mut NetCommandMsg,
        data: &[u8],
        i: &mut usize,
    ) {
        read_game_message_argument_from_packet(ty, msg, data, i);
    }

    fn read_ack(mut msg: NetCommandMsg, data: &[u8], i: &mut usize) -> NetCommandMsg {
        let cmd_id = rd_u16(data, i);
        msg.set_command_id(cmd_id);
        let orig = rd_u8(data, i);
        msg.set_original_player_id(orig);
        msg
    }

    /// `readAckBothMessage` (NetPacket.cpp:5351).
    pub fn read_ack_both_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        NetPacket::read_ack(NetCommandMsg::new_ack_both(), data, i)
    }
    /// `readAckStage1Message` (NetPacket.cpp:5374).
    pub fn read_ack_stage1_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        NetPacket::read_ack(NetCommandMsg::new_ack_stage1(), data, i)
    }
    /// `readAckStage2Message` (NetPacket.cpp:5397).
    pub fn read_ack_stage2_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        NetPacket::read_ack(NetCommandMsg::new_ack_stage2(), data, i)
    }
    /// `readFrameMessage` (NetPacket.cpp:5420).
    pub fn read_frame_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_frame();
        let c = rd_u16(data, i);
        msg.set_command_count(c);
        msg
    }
    /// `readPlayerLeaveMessage` (NetPacket.cpp:5437).
    pub fn read_player_leave_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_player_leave();
        let id = rd_u8(data, i);
        msg.set_leaving_player_id(id);
        msg
    }
    /// `readRunAheadMetricsMessage` (NetPacket.cpp:5452): the average latency (Real) and fps (UnsignedShort).
    pub fn read_run_ahead_metrics_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_run_ahead_metrics();
        let lat = f32::from_bits(rd_u32(data, i));
        msg.set_average_latency(lat);
        let fps = rd_u16(data, i);
        msg.set_average_fps(fps as i32);
        msg
    }
    /// `readRunAheadMessage` (NetPacket.cpp:5471).
    pub fn read_run_ahead_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        // upstream's constructor sets m_runAhead from MIN_RUNAHEAD/MAX_FRAMES_AHEAD; both fields are then overwritten
        let mut msg = NetCommandMsg::new_run_ahead(&super::net_command_msg::NetworkFrameLimits::defaults());
        let ra = rd_u16(data, i);
        msg.set_run_ahead(ra);
        let fr = rd_u8(data, i);
        msg.set_frame_rate(fr);
        msg
    }
    /// `readDestroyPlayerMessage` (NetPacket.cpp:5490).
    pub fn read_destroy_player_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_destroy_player();
        let v = rd_u32(data, i);
        msg.set_player_index(v);
        msg
    }
    /// `readKeepAliveMessage` (NetPacket.cpp:5505).
    pub fn read_keep_alive_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new_keep_alive()
    }
    /// `readDisconnectKeepAliveMessage` (NetPacket.cpp:5514).
    pub fn read_disconnect_keep_alive_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new_disconnect_keep_alive()
    }
    /// `readDisconnectPlayerMessage` (NetPacket.cpp:5523).
    pub fn read_disconnect_player_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_disconnect_player();
        let slot = rd_u8(data, i);
        msg.set_disconnect_slot(slot);
        let f = rd_u32(data, i);
        msg.set_disconnect_frame(f);
        msg
    }
    /// `readPacketRouterQueryMessage` (NetPacket.cpp:5542).
    pub fn read_packet_router_query_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new_packet_router_query()
    }
    /// `readPacketRouterAckMessage` (NetPacket.cpp:5551).
    pub fn read_packet_router_ack_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new_packet_router_ack()
    }

    /// The `UnsignedShort text[256]` read of the two chat messages: a length byte, then that many 16-bit units; the
    /// string is the units up to the first 0 (`UnicodeString::set` stops at the terminator).
    fn read_text(data: &[u8], i: &mut usize) -> UnicodeString {
        let length = rd_u8(data, i) as usize;
        let mut text: Vec<u16> = Vec::with_capacity(length);
        for _ in 0..length {
            text.push(rd_u16(data, i));
        }
        if let Some(z) = text.iter().position(|&c| c == 0) {
            text.truncate(z);
        }
        UnicodeString::try_from_chars(&text).unwrap_or_default()
    }

    /// `readDisconnectChatMessage` (NetPacket.cpp:5560).
    pub fn read_disconnect_chat_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_disconnect_chat();
        let t = NetPacket::read_text(data, i);
        msg.set_text(t);
        msg
    }
    /// `readChatMessage` (NetPacket.cpp:5583): text, then the player mask (Int).
    pub fn read_chat_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_chat();
        let t = NetPacket::read_text(data, i);
        let mask = rd_i32(data, i);
        msg.set_text(t);
        msg.set_chat_player_mask(mask);
        msg
    }
    /// `readDisconnectVoteMessage` (NetPacket.cpp:5611).
    pub fn read_disconnect_vote_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_disconnect_vote();
        let slot = rd_u8(data, i);
        msg.set_vote_slot(slot);
        let f = rd_u32(data, i);
        msg.set_vote_frame(f);
        msg
    }
    /// `readProgressMessage` (NetPacket.cpp:5630).
    pub fn read_progress_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_progress();
        let p = rd_u8(data, i);
        msg.set_percentage(p);
        msg
    }
    /// `readLoadCompleteMessage` (NetPacket.cpp:5641): a plain `NetCommandMsg`.
    pub fn read_load_complete_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new()
    }
    /// `readTimeOutGameStartMessage` (NetPacket.cpp:5646): a plain `NetCommandMsg`.
    pub fn read_time_out_game_start_message(_data: &[u8], _i: &mut usize) -> NetCommandMsg {
        NetCommandMsg::new()
    }
    /// `readWrapperMessage` (NetPacket.cpp:5651): wrapped command id, chunk number, number of chunks, total length,
    /// data length, data offset, the chunk's data (clamped to what remains in the packet).
    pub fn read_wrapper_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_wrapper();
        let wrapped = rd_u16(data, i);
        msg.set_wrapped_command_id(wrapped);
        let chunk = rd_u32(data, i);
        msg.set_chunk_number(chunk);
        let num = rd_u32(data, i);
        msg.set_num_chunks(num);
        let total = rd_u32(data, i);
        msg.set_total_data_length(total);
        let data_length = rd_u32(data, i);
        let offset = rd_u32(data, i);
        msg.set_data_offset(offset);
        let remaining = data.len().saturating_sub(*i);
        let n = min(data_length as usize, remaining);
        let start = min(*i, data.len());
        msg.set_wrapper_data(&data[start..start + n]);
        *i += data_length as usize;
        msg
    }

    /// The NUL-terminated portable filename of the file messages.
    fn read_filename(data: &[u8], i: &mut usize) -> AsciiString {
        let mut name: Vec<u8> = Vec::new();
        while *i < data.len() && data[*i] != 0 {
            name.push(data[*i]);
            *i += 1;
        }
        *i += 1; // the terminating 0
        AsciiString::try_from_bytes(&name).unwrap_or_default()
    }

    /// `readFileMessage` (NetPacket.cpp:5700): portable filename, data length (u32), the file data.
    pub fn read_file_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_file();
        let name = NetPacket::read_filename(data, i);
        msg.set_portable_filename(name); // it's transferred as a portable filename
        let data_length = rd_u32(data, i);
        let remaining = data.len().saturating_sub(*i);
        let n = min(data_length as usize, remaining);
        let start = min(*i, data.len());
        msg.set_file_data(&data[start..start + n]);
        *i += data_length as usize;
        msg
    }
    /// `readFileAnnounceMessage` (NetPacket.cpp:5727).
    pub fn read_file_announce_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_file_announce();
        let name = NetPacket::read_filename(data, i);
        msg.set_portable_filename(name);
        let id = rd_u16(data, i);
        msg.set_file_id(id);
        let mask = rd_u8(data, i);
        msg.set_file_player_mask(mask);
        msg
    }
    /// `readFileProgressMessage` (NetPacket.cpp:5754).
    pub fn read_file_progress_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_file_progress();
        let id = rd_u16(data, i);
        msg.set_file_id(id);
        let p = rd_i32(data, i);
        msg.set_progress(p);
        msg
    }
    /// `readDisconnectFrameMessage` (NetPacket.cpp:5770).
    pub fn read_disconnect_frame_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_disconnect_frame();
        let f = rd_u32(data, i);
        msg.set_disconnect_frame(f);
        msg
    }
    /// `readDisconnectScreenOffMessage` (NetPacket.cpp:5783).
    pub fn read_disconnect_screen_off_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_disconnect_screen_off();
        let f = rd_u32(data, i);
        msg.set_new_frame(f);
        msg
    }
    /// `readFrameResendRequestMessage` (NetPacket.cpp:5794).
    pub fn read_frame_resend_request_message(data: &[u8], i: &mut usize) -> NetCommandMsg {
        let mut msg = NetCommandMsg::new_frame_resend_request();
        let f = rd_u32(data, i);
        msg.set_frame_to_resend(f);
        msg
    }
}
