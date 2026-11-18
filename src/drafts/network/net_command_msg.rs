//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetworkDefs.h
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetCommandMsg.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandMsg.cpp
//!
//! # Shape
//!
//! Upstream's `NetCommandMsg` is a base class with ~25 subclasses and an intrusive reference count (`attach`/`detach`).
//! Here the base fields are [`NetCommandMsg`] and the subclass state is the [`NetCommandBody`] enum (enum dispatch,
//! no trait objects). Shared ownership is [`NetCommandMsgPtr`] (`Arc<RwLock<..>>`); `attach`/`detach` keep upstream's
//! `m_referenceCount` bookkeeping, but the memory is freed by the `Arc`.

use std::sync::{Arc, RwLock};

use crate::common::ascii_string::AsciiString;
use crate::common::message_stream::{
    ArgumentData, ArgumentDataType, GameMessageArgument, MessageId, MessageType, Messages,
};
use crate::common::unicode_string::UnicodeString;

// ---------------------------------------------------------------------------------------------------------------
// NetworkDefs.h
// ---------------------------------------------------------------------------------------------------------------

/// `WOL_NAME_LEN`.
pub const WOL_NAME_LEN: i32 = 64;
/// `MAX_COMMANDS`: max number of commands per frame.
pub const MAX_COMMANDS: i32 = 256;
/// `MAX_PLAYER` (`enum ConnectionNumbers`): index of the highest possible player number.
pub const MAX_PLAYER: i32 = 7;
/// `NUM_CONNECTIONS` (`enum ConnectionNumbers`).
pub const NUM_CONNECTIONS: i32 = MAX_PLAYER + 1;
/// `MAX_SLOTS`.
pub const MAX_SLOTS: i32 = MAX_PLAYER + 1;
/// `MAX_PACKET_SIZE`: 512 minus 36 bytes of UDP and IP header.
pub const MAX_PACKET_SIZE: i32 = 476;
/// `MAX_MESSAGE_LEN`.
pub const MAX_MESSAGE_LEN: usize = 1024;
/// `MAX_MESSAGES`.
pub const MAX_MESSAGES: i32 = 128;
/// `MAX_TRANSPORT_STATISTICS_SECONDS`.
pub const MAX_TRANSPORT_STATISTICS_SECONDS: i32 = 30;
/// `GENERALS_MAGIC_NUMBER`.
pub const GENERALS_MAGIC_NUMBER: u16 = 0xF00D;
/// `NETWORK_BASE_PORT_NUMBER`.
pub const NETWORK_BASE_PORT_NUMBER: i32 = 8088;

/// `MAX_FRAMES_AHEAD`, `MIN_RUNAHEAD`, `FRAME_DATA_LENGTH` and `FRAMES_TO_KEEP`: upstream's `extern Int` globals,
/// defined in `NetworkUtil.cpp` (`MAX_FRAMES_AHEAD` 128, `MIN_RUNAHEAD` 10, `FRAME_DATA_LENGTH`
/// `(MAX_FRAMES_AHEAD+1)*2`, `FRAMES_TO_KEEP` `(MAX_FRAMES_AHEAD/2)+1`; the header's comment says `+1` only, the
/// definition has the `*2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkFrameLimits {
    /// `MAX_FRAMES_AHEAD`.
    pub max_frames_ahead: i32,
    /// `MIN_RUNAHEAD`.
    pub min_runahead: i32,
    /// `FRAME_DATA_LENGTH`.
    pub frame_data_length: i32,
    /// `FRAMES_TO_KEEP`.
    pub frames_to_keep: i32,
}

impl NetworkFrameLimits {
    /// The values `Network.cpp` initialises the four globals with.
    pub const fn defaults() -> NetworkFrameLimits {
        NetworkFrameLimits { max_frames_ahead: 128, min_runahead: 10, frame_data_length: 258, frames_to_keep: 65 }
    }
}

/// `enum NetMessageFlag`.
pub const MSG_ACK: u8 = 1;
/// `MSG_NEEDACK`.
pub const MSG_NEEDACK: u8 = 2;
/// `MSG_SEQUENCED`.
pub const MSG_SEQUENCED: u8 = 4;
/// `MSG_SUPERCEDING`.
pub const MSG_SUPERCEDING: u8 = 8;

/// `enum NetCommandType`, in upstream's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum NetCommandType {
    /// `NETCOMMANDTYPE_UNKNOWN` (-1).
    Unknown = -1,
    /// `NETCOMMANDTYPE_ACKBOTH`.
    AckBoth = 0,
    /// `NETCOMMANDTYPE_ACKSTAGE1`.
    AckStage1,
    /// `NETCOMMANDTYPE_ACKSTAGE2`.
    AckStage2,
    /// `NETCOMMANDTYPE_FRAMEINFO`.
    FrameInfo,
    /// `NETCOMMANDTYPE_GAMECOMMAND`.
    GameCommand,
    /// `NETCOMMANDTYPE_PLAYERLEAVE`.
    PlayerLeave,
    /// `NETCOMMANDTYPE_RUNAHEADMETRICS`.
    RunAheadMetrics,
    /// `NETCOMMANDTYPE_RUNAHEAD`.
    RunAhead,
    /// `NETCOMMANDTYPE_DESTROYPLAYER`.
    DestroyPlayer,
    /// `NETCOMMANDTYPE_KEEPALIVE`.
    KeepAlive,
    /// `NETCOMMANDTYPE_DISCONNECTCHAT`.
    DisconnectChat,
    /// `NETCOMMANDTYPE_CHAT`.
    Chat,
    /// `NETCOMMANDTYPE_MANGLERQUERY`.
    ManglerQuery,
    /// `NETCOMMANDTYPE_MANGLERRESPONSE`.
    ManglerResponse,
    /// `NETCOMMANDTYPE_PROGRESS`.
    Progress,
    /// `NETCOMMANDTYPE_LOADCOMPLETE`.
    LoadComplete,
    /// `NETCOMMANDTYPE_TIMEOUTSTART`.
    TimeoutStart,
    /// `NETCOMMANDTYPE_WRAPPER`: holds a command too big for a single packet.
    Wrapper,
    /// `NETCOMMANDTYPE_FILE`.
    File,
    /// `NETCOMMANDTYPE_FILEANNOUNCE`.
    FileAnnounce,
    /// `NETCOMMANDTYPE_FILEPROGRESS`.
    FileProgress,
    /// `NETCOMMANDTYPE_FRAMERESENDREQUEST`.
    FrameResendRequest,
    /// `NETCOMMANDTYPE_DISCONNECTSTART`.
    DisconnectStart,
    /// `NETCOMMANDTYPE_DISCONNECTKEEPALIVE`.
    DisconnectKeepAlive,
    /// `NETCOMMANDTYPE_DISCONNECTPLAYER`.
    DisconnectPlayer,
    /// `NETCOMMANDTYPE_PACKETROUTERQUERY`.
    PacketRouterQuery,
    /// `NETCOMMANDTYPE_PACKETROUTERACK`.
    PacketRouterAck,
    /// `NETCOMMANDTYPE_DISCONNECTVOTE`.
    DisconnectVote,
    /// `NETCOMMANDTYPE_DISCONNECTFRAME`.
    DisconnectFrame,
    /// `NETCOMMANDTYPE_DISCONNECTSCREENOFF`.
    DisconnectScreenOff,
    /// `NETCOMMANDTYPE_DISCONNECTEND`.
    DisconnectEnd,
    /// `NETCOMMANDTYPE_MAX`.
    Max,
}

impl NetCommandType {
    /// The enum value from its `Int` (a value read from the wire); `None` outside `-1..=NETCOMMANDTYPE_MAX`.
    pub fn from_i32(v: i32) -> Option<NetCommandType> {
        use NetCommandType::*;
        const ALL: [NetCommandType; 32] = [
            AckBoth, AckStage1, AckStage2, FrameInfo, GameCommand, PlayerLeave, RunAheadMetrics, RunAhead,
            DestroyPlayer, KeepAlive, DisconnectChat, Chat, ManglerQuery, ManglerResponse, Progress, LoadComplete,
            TimeoutStart, Wrapper, File, FileAnnounce, FileProgress, FrameResendRequest, DisconnectStart,
            DisconnectKeepAlive, DisconnectPlayer, PacketRouterQuery, PacketRouterAck, DisconnectVote,
            DisconnectFrame, DisconnectScreenOff, DisconnectEnd, Max,
        ];
        if v == -1 {
            Some(Unknown)
        } else if (0..=NetCommandType::Max as i32).contains(&v) {
            Some(ALL[v as usize])
        } else {
            None
        }
    }
}

/// `enum NetLocalStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetLocalStatus {
    /// `NETLOCALSTATUS_PREGAME`.
    PreGame = 0,
    /// `NETLOCALSTATUS_INGAME`.
    InGame,
    /// `NETLOCALSTATUS_LEAVING`.
    Leaving,
    /// `NETLOCALSTATUS_LEFT`.
    Left,
    /// `NETLOCALSTATUS_POSTGAME`.
    PostGame,
}

/// `enum PlayerLeaveCode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerLeaveCode {
    /// `PLAYERLEAVECODE_CLIENT`.
    Client = 0,
    /// `PLAYERLEAVECODE_LOCAL`.
    Local,
    /// `PLAYERLEAVECODE_PACKETROUTER`.
    PacketRouter,
    /// `PLAYERLEAVECODE_UNKNOWN`.
    Unknown,
}

/// `struct TransportMessageHeader` (`#pragma pack(1)`: 6 bytes on the wire, `crc` first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransportMessageHeader {
    /// `crc`: packet-level CRC.
    pub crc: u32,
    /// `magic`: [`GENERALS_MAGIC_NUMBER`].
    pub magic: u16,
}

/// `struct TransportMessage`: the transport layer's per-packet record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportMessage {
    /// `header`.
    pub header: TransportMessageHeader,
    /// `data[MAX_MESSAGE_LEN]`.
    pub data: [u8; MAX_MESSAGE_LEN],
    /// `length`.
    pub length: i32,
    /// `addr`: IPv4 address in host byte order, as upstream's `UnsignedInt`.
    pub addr: u32,
    /// `port`.
    pub port: u16,
}

impl Default for TransportMessage {
    fn default() -> Self {
        TransportMessage {
            header: TransportMessageHeader::default(),
            data: [0; MAX_MESSAGE_LEN],
            length: 0,
            addr: 0,
            port: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// NetCommandMsg
// ---------------------------------------------------------------------------------------------------------------

/// `NetGameCommandMsg`'s members after the base: `m_type` and the argument list (`m_argList`..`m_argTail`).
#[derive(Clone, Debug, PartialEq)]
pub struct NetGameCommandBody {
    /// `m_numArgs` (upstream never updates it; kept for fidelity, 0).
    pub num_args: i32,
    /// `m_argSize` (never updated either).
    pub arg_size: i32,
    /// `m_type`.
    pub ty: MessageType,
    /// `m_argList` in order.
    pub args: Vec<GameMessageArgument>,
}

/// `NetAckBothCommandMsg`, `NetAckStage1CommandMsg` and `NetAckStage2CommandMsg` share these members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetAckBody {
    /// `m_commandID`.
    pub command_id: u16,
    /// `m_originalPlayerID`.
    pub original_player_id: u8,
}

/// `NetWrapperCommandMsg`'s members.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetWrapperBody {
    /// `m_data`.
    pub data: Option<Vec<u8>>,
    /// `m_dataLength`.
    pub data_length: u32,
    /// `m_dataOffset`.
    pub data_offset: u32,
    /// `m_totalDataLength`.
    pub total_data_length: u32,
    /// `m_chunkNumber`.
    pub chunk_number: u32,
    /// `m_numChunks`.
    pub num_chunks: u32,
    /// `m_wrappedCommandID`.
    pub wrapped_command_id: u16,
}

/// The subclass state of a [`NetCommandMsg`] (one variant per upstream subclass, in `NetCommandMsg.h` order).
#[derive(Clone, Debug, PartialEq)]
pub enum NetCommandBody {
    /// A plain `NetCommandMsg` (base class only; `m_commandType` is whatever was set).
    Base,
    /// `NetGameCommandMsg`.
    GameCommand(NetGameCommandBody),
    /// `NetAckBothCommandMsg`.
    AckBoth(NetAckBody),
    /// `NetAckStage1CommandMsg`.
    AckStage1(NetAckBody),
    /// `NetAckStage2CommandMsg`.
    AckStage2(NetAckBody),
    /// `NetFrameCommandMsg`: `m_commandCount`.
    Frame { command_count: u16 },
    /// `NetPlayerLeaveCommandMsg`: `m_leavingPlayerID`.
    PlayerLeave { leaving_player_id: u8 },
    /// `NetRunAheadMetricsCommandMsg`.
    RunAheadMetrics { average_latency: f32, average_fps: i32 },
    /// `NetRunAheadCommandMsg`.
    RunAhead { run_ahead: u16, frame_rate: u8 },
    /// `NetDestroyPlayerCommandMsg`: `m_playerIndex`.
    DestroyPlayer { player_index: u32 },
    /// `NetKeepAliveCommandMsg`.
    KeepAlive,
    /// `NetDisconnectKeepAliveCommandMsg`.
    DisconnectKeepAlive,
    /// `NetDisconnectPlayerCommandMsg`. `m_disconnectFrame` is left uninitialised by upstream's constructor; 0 here.
    DisconnectPlayer { disconnect_slot: u8, disconnect_frame: u32 },
    /// `NetPacketRouterQueryCommandMsg`.
    PacketRouterQuery,
    /// `NetPacketRouterAckCommandMsg`.
    PacketRouterAck,
    /// `NetDisconnectChatCommandMsg`: `m_text`.
    DisconnectChat { text: UnicodeString },
    /// `NetChatCommandMsg`.
    Chat { text: UnicodeString, player_mask: i32 },
    /// `NetDisconnectVoteCommandMsg`. `m_voteFrame` is left uninitialised by upstream's constructor; 0 here.
    DisconnectVote { slot: u8, vote_frame: u32 },
    /// `NetProgressCommandMsg`: `m_percent`.
    Progress { percent: u8 },
    /// `NetWrapperCommandMsg`.
    Wrapper(NetWrapperBody),
    /// `NetFileCommandMsg`.
    File { portable_filename: AsciiString, data: Option<Vec<u8>>, data_length: u32 },
    /// `NetFileAnnounceCommandMsg`.
    FileAnnounce { portable_filename: AsciiString, file_id: u16, player_mask: u8 },
    /// `NetFileProgressCommandMsg`.
    FileProgress { file_id: u16, progress: i32 },
    /// `NetDisconnectFrameCommandMsg`: `m_disconnectFrame`.
    DisconnectFrame { disconnect_frame: u32 },
    /// `NetDisconnectScreenOffCommandMsg`: `m_newFrame`.
    DisconnectScreenOff { new_frame: u32 },
    /// `NetFrameResendRequestCommandMsg`: `m_frameToResend`.
    FrameResendRequest { frame_to_resend: u32 },
}

/// `NetCommandMsg`: the base fields plus the subclass state.
#[derive(Clone, Debug, PartialEq)]
pub struct NetCommandMsg {
    /// `m_timestamp`.
    pub timestamp: u32,
    /// `m_executionFrame`.
    pub execution_frame: u32,
    /// `m_playerID`.
    pub player_id: u32,
    /// `m_id`.
    pub id: u16,
    /// `m_commandType`.
    pub command_type: NetCommandType,
    /// `m_referenceCount`.
    pub reference_count: i32,
    /// The subclass.
    pub body: NetCommandBody,
}

/// A shared message: upstream's `NetCommandMsg *` with its intrusive reference count.
#[derive(Clone, Debug)]
pub struct NetCommandMsgPtr(pub Arc<RwLock<NetCommandMsg>>);

impl NetCommandMsgPtr {
    /// Wrap a freshly built message (reference count 1: "an attach is implied by creating a NetCommandMsg").
    pub fn new(msg: NetCommandMsg) -> NetCommandMsgPtr {
        NetCommandMsgPtr(Arc::new(RwLock::new(msg)))
    }

    /// Read access (a poisoned lock is read through; nothing here panics while holding it).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, NetCommandMsg> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Write access.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, NetCommandMsg> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Same message (pointer equality), upstream's `msg1 == msg2` on `NetCommandMsg *`.
    pub fn ptr_eq(&self, other: &NetCommandMsgPtr) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl NetCommandMsg {
    /// `NetCommandMsg::NetCommandMsg()`: everything zero, `m_referenceCount` 1, type unknown.
    pub fn new() -> NetCommandMsg {
        NetCommandMsg {
            timestamp: 0,
            execution_frame: 0,
            player_id: 0,
            id: 0,
            command_type: NetCommandType::Unknown,
            reference_count: 1,
            body: NetCommandBody::Base,
        }
    }

    fn with(command_type: NetCommandType, body: NetCommandBody) -> NetCommandMsg {
        let mut m = NetCommandMsg::new();
        m.command_type = command_type;
        m.body = body;
        m
    }

    /// `GetTimestamp`.
    pub fn get_timestamp(&self) -> u32 {
        self.timestamp
    }
    /// `SetTimestamp`.
    pub fn set_timestamp(&mut self, timestamp: u32) {
        self.timestamp = timestamp;
    }
    /// `setExecutionFrame`.
    pub fn set_execution_frame(&mut self, frame: u32) {
        self.execution_frame = frame;
    }
    /// `setPlayerID`.
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
    }
    /// `setID`.
    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }
    /// `getExecutionFrame`.
    pub fn get_execution_frame(&self) -> u32 {
        self.execution_frame
    }
    /// `getPlayerID`.
    pub fn get_player_id(&self) -> u32 {
        self.player_id
    }
    /// `getID`.
    pub fn get_id(&self) -> u16 {
        self.id
    }
    /// `setNetCommandType`.
    pub fn set_net_command_type(&mut self, ty: NetCommandType) {
        self.command_type = ty;
    }
    /// `getNetCommandType`.
    pub fn get_net_command_type(&self) -> NetCommandType {
        self.command_type
    }

    /// `NetCommandMsg::attach` (NetCommandMsg.cpp): adds one to the reference count.
    pub fn attach(&mut self) {
        self.reference_count += 1;
    }

    /// `NetCommandMsg::detach` (NetCommandMsg.cpp): subtracts one; returns whether upstream would now
    /// `deleteInstance()` (count 0, or negative after the assert). The `Arc` frees the memory.
    pub fn detach(&mut self) -> bool {
        self.reference_count -= 1;
        if self.reference_count == 0 {
            return true;
        }
        // DEBUG_ASSERTCRASH(m_referenceCount > 0, ("Invalid reference count for NetCommandMsg"))
        self.reference_count < 0
    }

    /// `getSortNumber` (virtual): `m_commandID` for the three ack kinds, else `m_id`.
    /// `NetCommandMsg::getSortNumber` (NetCommandMsg.cpp), `NetAck*CommandMsg::getSortNumber`.
    pub fn get_sort_number(&self) -> i32 {
        match &self.body {
            NetCommandBody::AckBoth(a) | NetCommandBody::AckStage1(a) | NetCommandBody::AckStage2(a) => {
                a.command_id as i32
            }
            _ => self.id as i32,
        }
    }

    /// `getContentsAsAsciiString` (virtual): the type name for a game command, the empty string otherwise.
    /// `NetGameCommandMsg::getContentsAsAsciiString` formats `"Type:%s"` of `getCommandTypeAsAsciiString`.
    pub fn get_contents_as_ascii_string(&self) -> Vec<u8> {
        match &self.body {
            NetCommandBody::GameCommand(g) => {
                let mut v = b"Type:".to_vec();
                v.extend_from_slice(g.ty.command_string().as_bytes());
                v
            }
            _ => Vec::new(),
        }
    }

    // ---- constructors, one per subclass -------------------------------------------------------------------

    /// `NetGameCommandMsg::NetGameCommandMsg()`.
    pub fn new_game_command() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::GameCommand,
            NetCommandBody::GameCommand(NetGameCommandBody {
                num_args: 0,
                arg_size: 0,
                ty: MessageType(0),
                args: Vec::new(),
            }),
        )
    }

    /// `NetGameCommandMsg::NetGameCommandMsg(GameMessage *msg)`: copies the type and every argument with its data
    /// type (`addArgument(msg->getArgumentDataType(i), *msg->getArgument(i))`).
    pub fn new_game_command_from_message(
        ty: MessageType,
        arguments: &[GameMessageArgument],
    ) -> NetCommandMsg {
        let mut m = NetCommandMsg::new();
        m.command_type = NetCommandType::GameCommand;
        let mut body = NetGameCommandBody { num_args: 0, arg_size: 0, ty, args: Vec::new() };
        for a in arguments {
            body.args.push(GameMessageArgument { data: a.data, ty: a.ty });
        }
        m.body = NetCommandBody::GameCommand(body);
        m
    }

    /// `NetAckBothCommandMsg(NetCommandMsg *msg)`.
    pub fn new_ack_both_for(msg: &NetCommandMsg) -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::AckBoth,
            NetCommandBody::AckBoth(NetAckBody { command_id: msg.get_id(), original_player_id: msg.get_player_id() as u8 }),
        )
    }
    /// `NetAckBothCommandMsg()`. (`m_commandID`/`m_originalPlayerID` are uninitialised upstream; 0 here.)
    pub fn new_ack_both() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::AckBoth, NetCommandBody::AckBoth(NetAckBody::default()))
    }
    /// `NetAckStage1CommandMsg(NetCommandMsg *msg)`.
    pub fn new_ack_stage1_for(msg: &NetCommandMsg) -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::AckStage1,
            NetCommandBody::AckStage1(NetAckBody { command_id: msg.get_id(), original_player_id: msg.get_player_id() as u8 }),
        )
    }
    /// `NetAckStage1CommandMsg()`.
    pub fn new_ack_stage1() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::AckStage1, NetCommandBody::AckStage1(NetAckBody::default()))
    }
    /// `NetAckStage2CommandMsg(NetCommandMsg *msg)`.
    pub fn new_ack_stage2_for(msg: &NetCommandMsg) -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::AckStage2,
            NetCommandBody::AckStage2(NetAckBody { command_id: msg.get_id(), original_player_id: msg.get_player_id() as u8 }),
        )
    }
    /// `NetAckStage2CommandMsg()`.
    pub fn new_ack_stage2() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::AckStage2, NetCommandBody::AckStage2(NetAckBody::default()))
    }
    /// `NetFrameCommandMsg()`.
    pub fn new_frame() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::FrameInfo, NetCommandBody::Frame { command_count: 0 })
    }
    /// `NetPlayerLeaveCommandMsg()`.
    pub fn new_player_leave() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::PlayerLeave, NetCommandBody::PlayerLeave { leaving_player_id: 0 })
    }
    /// `NetRunAheadMetricsCommandMsg()`.
    pub fn new_run_ahead_metrics() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::RunAheadMetrics,
            NetCommandBody::RunAheadMetrics { average_latency: 0.0, average_fps: 0 },
        )
    }
    /// `NetRunAheadCommandMsg()`: `m_runAhead = min(max(20, MIN_RUNAHEAD), MAX_FRAMES_AHEAD/2)`, `m_frameRate = 30`.
    pub fn new_run_ahead(limits: &NetworkFrameLimits) -> NetCommandMsg {
        let run_ahead = (20.max(limits.min_runahead)).min(limits.max_frames_ahead / 2);
        NetCommandMsg::with(
            NetCommandType::RunAhead,
            NetCommandBody::RunAhead { run_ahead: run_ahead as u16, frame_rate: 30 },
        )
    }
    /// `NetDestroyPlayerCommandMsg()`.
    pub fn new_destroy_player() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::DestroyPlayer, NetCommandBody::DestroyPlayer { player_index: 0 })
    }
    /// `NetKeepAliveCommandMsg()`.
    pub fn new_keep_alive() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::KeepAlive, NetCommandBody::KeepAlive)
    }
    /// `NetDisconnectKeepAliveCommandMsg()`.
    pub fn new_disconnect_keep_alive() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::DisconnectKeepAlive, NetCommandBody::DisconnectKeepAlive)
    }
    /// `NetDisconnectPlayerCommandMsg()`.
    pub fn new_disconnect_player() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::DisconnectPlayer,
            NetCommandBody::DisconnectPlayer { disconnect_slot: 0, disconnect_frame: 0 },
        )
    }
    /// `NetPacketRouterQueryCommandMsg()`.
    pub fn new_packet_router_query() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::PacketRouterQuery, NetCommandBody::PacketRouterQuery)
    }
    /// `NetPacketRouterAckCommandMsg()`.
    pub fn new_packet_router_ack() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::PacketRouterAck, NetCommandBody::PacketRouterAck)
    }
    /// `NetDisconnectChatCommandMsg()`.
    pub fn new_disconnect_chat() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::DisconnectChat,
            NetCommandBody::DisconnectChat { text: UnicodeString::new() },
        )
    }
    /// `NetChatCommandMsg()`.
    pub fn new_chat() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::Chat, NetCommandBody::Chat { text: UnicodeString::new(), player_mask: 0 })
    }
    /// `NetDisconnectVoteCommandMsg()`.
    pub fn new_disconnect_vote() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::DisconnectVote,
            NetCommandBody::DisconnectVote { slot: 0, vote_frame: 0 },
        )
    }
    /// `NetProgressCommandMsg()`.
    pub fn new_progress() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::Progress, NetCommandBody::Progress { percent: 0 })
    }
    /// `NetWrapperCommandMsg()`.
    pub fn new_wrapper() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::Wrapper, NetCommandBody::Wrapper(NetWrapperBody::default()))
    }
    /// `NetFileCommandMsg()`.
    pub fn new_file() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::File,
            NetCommandBody::File { portable_filename: AsciiString::default(), data: None, data_length: 0 },
        )
    }
    /// `NetFileAnnounceCommandMsg()`.
    pub fn new_file_announce() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::FileAnnounce,
            NetCommandBody::FileAnnounce { portable_filename: AsciiString::default(), file_id: 0, player_mask: 0 },
        )
    }
    /// `NetFileProgressCommandMsg()`.
    pub fn new_file_progress() -> NetCommandMsg {
        NetCommandMsg::with(NetCommandType::FileProgress, NetCommandBody::FileProgress { file_id: 0, progress: 0 })
    }
    /// `NetDisconnectFrameCommandMsg()`.
    pub fn new_disconnect_frame() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::DisconnectFrame,
            NetCommandBody::DisconnectFrame { disconnect_frame: 0 },
        )
    }
    /// `NetDisconnectScreenOffCommandMsg()`.
    pub fn new_disconnect_screen_off() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::DisconnectScreenOff,
            NetCommandBody::DisconnectScreenOff { new_frame: 0 },
        )
    }
    /// `NetFrameResendRequestCommandMsg()`.
    pub fn new_frame_resend_request() -> NetCommandMsg {
        NetCommandMsg::with(
            NetCommandType::FrameResendRequest,
            NetCommandBody::FrameResendRequest { frame_to_resend: 0 },
        )
    }

    // ---- NetGameCommandMsg ---------------------------------------------------------------------------------

    /// `NetGameCommandMsg::addArgument`: appends an argument at the tail. No effect on other kinds.
    pub fn add_argument(&mut self, ty: ArgumentDataType, arg: ArgumentData) {
        if let NetCommandBody::GameCommand(g) = &mut self.body {
            g.args.push(GameMessageArgument { data: arg, ty });
        }
    }

    /// `NetGameCommandMsg::setGameMessageType`.
    pub fn set_game_message_type(&mut self, ty: MessageType) {
        if let NetCommandBody::GameCommand(g) = &mut self.body {
            g.ty = ty;
        }
    }

    /// `NetGameCommandMsg::constructGameMessage` (NetCommandMsg.cpp).
    ///
    /// Upstream names the player `"player%d"` of `getPlayerID()`, finds it with
    /// `ThePlayerList->findPlayerWithNameKey(nameToKey(name))` and sets the message's player index to that player's
    /// `getPlayerIndex()` (a missing player is a null dereference upstream). `player_index_for_name` is that lookup
    /// over the name bytes; `None` is the null case and returns `None` here. Arguments are appended by their stored
    /// data type, in order, and an argument of an unknown data type is skipped, as in upstream's `if`/`else if` chain.
    pub fn construct_game_message(
        &self,
        messages: &mut Messages,
        player_index_for_name: impl Fn(&[u8]) -> Option<i32>,
    ) -> Option<MessageId> {
        let g = match &self.body {
            NetCommandBody::GameCommand(g) => g,
            _ => return None,
        };
        let name = format!("player{}", self.get_player_id());
        let player_index = player_index_for_name(name.as_bytes())?;
        // newInstance(GameMessage)(m_type); friend_setPlayerIndex replaces the local-player index
        let id = messages.new_message(g.ty, player_index);
        if let Some(m) = messages.get_mut(id) {
            m.set_player_index(player_index);
            for arg in &g.args {
                match arg.ty {
                    ArgumentDataType::Integer => m.append_integer_argument(arg.data.integer()),
                    ArgumentDataType::Real => m.append_real_argument(arg.data.real()),
                    ArgumentDataType::Boolean => m.append_boolean_argument(arg.data.boolean()),
                    ArgumentDataType::ObjectId => m.append_object_id_argument(arg.data.object_id()),
                    ArgumentDataType::DrawableId => m.append_drawable_id_argument(arg.data.drawable_id()),
                    ArgumentDataType::TeamId => m.append_team_id_argument(arg.data.team_id()),
                    ArgumentDataType::Location => m.append_location_argument(arg.data.location()),
                    ArgumentDataType::Pixel => m.append_pixel_argument(arg.data.pixel()),
                    ArgumentDataType::PixelRegion => m.append_pixel_region_argument(arg.data.pixel_region()),
                    ArgumentDataType::Timestamp => m.append_timestamp_argument(arg.data.timestamp()),
                    ArgumentDataType::WideChar => m.append_wide_char_argument(arg.data.wide_char()),
                    _ => {}
                }
            }
        }
        Some(id)
    }

    // ---- accessors of the subclasses ----------------------------------------------------------------------

    /// `getCommandID` of the three ack kinds (0 on other kinds).
    pub fn get_command_id(&self) -> u16 {
        match &self.body {
            NetCommandBody::AckBoth(a) | NetCommandBody::AckStage1(a) | NetCommandBody::AckStage2(a) => a.command_id,
            _ => 0,
        }
    }
    /// `setCommandID` of the three ack kinds.
    pub fn set_command_id(&mut self, command_id: u16) {
        if let NetCommandBody::AckBoth(a) | NetCommandBody::AckStage1(a) | NetCommandBody::AckStage2(a) = &mut self.body {
            a.command_id = command_id;
        }
    }
    /// `getOriginalPlayerID` of the three ack kinds (0 on other kinds).
    pub fn get_original_player_id(&self) -> u8 {
        match &self.body {
            NetCommandBody::AckBoth(a) | NetCommandBody::AckStage1(a) | NetCommandBody::AckStage2(a) => {
                a.original_player_id
            }
            _ => 0,
        }
    }
    /// `setOriginalPlayerID` of the three ack kinds.
    pub fn set_original_player_id(&mut self, id: u8) {
        if let NetCommandBody::AckBoth(a) | NetCommandBody::AckStage1(a) | NetCommandBody::AckStage2(a) = &mut self.body {
            a.original_player_id = id;
        }
    }

    /// `NetFrameCommandMsg::setCommandCount`.
    pub fn set_command_count(&mut self, command_count: u16) {
        if let NetCommandBody::Frame { command_count: c } = &mut self.body {
            *c = command_count;
        }
    }
    /// `NetFrameCommandMsg::getCommandCount`.
    pub fn get_command_count(&self) -> u16 {
        if let NetCommandBody::Frame { command_count } = &self.body { *command_count } else { 0 }
    }

    /// `NetPlayerLeaveCommandMsg::getLeavingPlayerID`.
    pub fn get_leaving_player_id(&self) -> u8 {
        if let NetCommandBody::PlayerLeave { leaving_player_id } = &self.body { *leaving_player_id } else { 0 }
    }
    /// `NetPlayerLeaveCommandMsg::setLeavingPlayerID`.
    pub fn set_leaving_player_id(&mut self, id: u8) {
        if let NetCommandBody::PlayerLeave { leaving_player_id } = &mut self.body {
            *leaving_player_id = id;
        }
    }

    /// `NetRunAheadMetricsCommandMsg::getAverageLatency`.
    pub fn get_average_latency(&self) -> f32 {
        if let NetCommandBody::RunAheadMetrics { average_latency, .. } = &self.body { *average_latency } else { 0.0 }
    }
    /// `NetRunAheadMetricsCommandMsg::setAverageLatency`.
    pub fn set_average_latency(&mut self, avg_lat: f32) {
        if let NetCommandBody::RunAheadMetrics { average_latency, .. } = &mut self.body {
            *average_latency = avg_lat;
        }
    }
    /// `NetRunAheadMetricsCommandMsg::getAverageFps`.
    pub fn get_average_fps(&self) -> i32 {
        if let NetCommandBody::RunAheadMetrics { average_fps, .. } = &self.body { *average_fps } else { 0 }
    }
    /// `NetRunAheadMetricsCommandMsg::setAverageFps`.
    pub fn set_average_fps(&mut self, fps: i32) {
        if let NetCommandBody::RunAheadMetrics { average_fps, .. } = &mut self.body {
            *average_fps = fps;
        }
    }

    /// `NetRunAheadCommandMsg::getRunAhead`.
    pub fn get_run_ahead(&self) -> u16 {
        if let NetCommandBody::RunAhead { run_ahead, .. } = &self.body { *run_ahead } else { 0 }
    }
    /// `NetRunAheadCommandMsg::setRunAhead`.
    pub fn set_run_ahead(&mut self, v: u16) {
        if let NetCommandBody::RunAhead { run_ahead, .. } = &mut self.body {
            *run_ahead = v;
        }
    }
    /// `NetRunAheadCommandMsg::getFrameRate`.
    pub fn get_frame_rate(&self) -> u8 {
        if let NetCommandBody::RunAhead { frame_rate, .. } = &self.body { *frame_rate } else { 0 }
    }
    /// `NetRunAheadCommandMsg::setFrameRate`.
    pub fn set_frame_rate(&mut self, v: u8) {
        if let NetCommandBody::RunAhead { frame_rate, .. } = &mut self.body {
            *frame_rate = v;
        }
    }

    /// `NetDestroyPlayerCommandMsg::getPlayerIndex`.
    pub fn get_player_index(&self) -> u32 {
        if let NetCommandBody::DestroyPlayer { player_index } = &self.body { *player_index } else { 0 }
    }
    /// `NetDestroyPlayerCommandMsg::setPlayerIndex`.
    pub fn set_player_index(&mut self, v: u32) {
        if let NetCommandBody::DestroyPlayer { player_index } = &mut self.body {
            *player_index = v;
        }
    }

    /// `NetDisconnectPlayerCommandMsg::getDisconnectSlot`.
    pub fn get_disconnect_slot(&self) -> u8 {
        if let NetCommandBody::DisconnectPlayer { disconnect_slot, .. } = &self.body { *disconnect_slot } else { 0 }
    }
    /// `NetDisconnectPlayerCommandMsg::setDisconnectSlot`.
    pub fn set_disconnect_slot(&mut self, slot: u8) {
        if let NetCommandBody::DisconnectPlayer { disconnect_slot, .. } = &mut self.body {
            *disconnect_slot = slot;
        }
    }
    /// `getDisconnectFrame` of `NetDisconnectPlayerCommandMsg` and `NetDisconnectFrameCommandMsg`.
    pub fn get_disconnect_frame(&self) -> u32 {
        match &self.body {
            NetCommandBody::DisconnectPlayer { disconnect_frame, .. } => *disconnect_frame,
            NetCommandBody::DisconnectFrame { disconnect_frame } => *disconnect_frame,
            _ => 0,
        }
    }
    /// `setDisconnectFrame` of `NetDisconnectPlayerCommandMsg` and `NetDisconnectFrameCommandMsg`.
    pub fn set_disconnect_frame(&mut self, frame: u32) {
        match &mut self.body {
            NetCommandBody::DisconnectPlayer { disconnect_frame, .. } => *disconnect_frame = frame,
            NetCommandBody::DisconnectFrame { disconnect_frame } => *disconnect_frame = frame,
            _ => {}
        }
    }

    /// `getText` of `NetDisconnectChatCommandMsg` and `NetChatCommandMsg`.
    pub fn get_text(&self) -> UnicodeString {
        match &self.body {
            NetCommandBody::DisconnectChat { text } | NetCommandBody::Chat { text, .. } => text.clone(),
            _ => UnicodeString::new(),
        }
    }
    /// `setText` of `NetDisconnectChatCommandMsg` and `NetChatCommandMsg`.
    pub fn set_text(&mut self, t: UnicodeString) {
        match &mut self.body {
            NetCommandBody::DisconnectChat { text } | NetCommandBody::Chat { text, .. } => *text = t,
            _ => {}
        }
    }
    /// `NetChatCommandMsg::getPlayerMask`.
    pub fn get_chat_player_mask(&self) -> i32 {
        if let NetCommandBody::Chat { player_mask, .. } = &self.body { *player_mask } else { 0 }
    }
    /// `NetChatCommandMsg::setPlayerMask`.
    pub fn set_chat_player_mask(&mut self, mask: i32) {
        if let NetCommandBody::Chat { player_mask, .. } = &mut self.body {
            *player_mask = mask;
        }
    }

    /// `NetDisconnectVoteCommandMsg::getSlot`.
    pub fn get_vote_slot(&self) -> u8 {
        if let NetCommandBody::DisconnectVote { slot, .. } = &self.body { *slot } else { 0 }
    }
    /// `NetDisconnectVoteCommandMsg::setSlot`.
    pub fn set_vote_slot(&mut self, s: u8) {
        if let NetCommandBody::DisconnectVote { slot, .. } = &mut self.body {
            *slot = s;
        }
    }
    /// `NetDisconnectVoteCommandMsg::getVoteFrame`.
    pub fn get_vote_frame(&self) -> u32 {
        if let NetCommandBody::DisconnectVote { vote_frame, .. } = &self.body { *vote_frame } else { 0 }
    }
    /// `NetDisconnectVoteCommandMsg::setVoteFrame`.
    pub fn set_vote_frame(&mut self, f: u32) {
        if let NetCommandBody::DisconnectVote { vote_frame, .. } = &mut self.body {
            *vote_frame = f;
        }
    }

    /// `NetProgressCommandMsg::getPercentage`.
    pub fn get_percentage(&self) -> u8 {
        if let NetCommandBody::Progress { percent } = &self.body { *percent } else { 0 }
    }
    /// `NetProgressCommandMsg::setPercentage`.
    pub fn set_percentage(&mut self, p: u8) {
        if let NetCommandBody::Progress { percent } = &mut self.body {
            *percent = p;
        }
    }

    /// `NetWrapperCommandMsg`'s state (None on other kinds).
    pub fn wrapper(&self) -> Option<&NetWrapperBody> {
        if let NetCommandBody::Wrapper(w) = &self.body { Some(w) } else { None }
    }
    /// `NetWrapperCommandMsg`'s state, mutable.
    pub fn wrapper_mut(&mut self) -> Option<&mut NetWrapperBody> {
        if let NetCommandBody::Wrapper(w) = &mut self.body { Some(w) } else { None }
    }
    /// `NetWrapperCommandMsg::setData`: replaces the data with a copy of `data` and sets `m_dataLength`.
    pub fn set_wrapper_data(&mut self, data: &[u8]) {
        if let NetCommandBody::Wrapper(w) = &mut self.body {
            w.data = Some(data.to_vec());
            w.data_length = data.len() as u32;
        }
    }
    /// `NetWrapperCommandMsg::getData`.
    pub fn get_wrapper_data(&self) -> Option<&[u8]> {
        self.wrapper().and_then(|w| w.data.as_deref())
    }
    /// `NetWrapperCommandMsg::getDataLength`.
    pub fn get_wrapper_data_length(&self) -> u32 {
        self.wrapper().map_or(0, |w| w.data_length)
    }
    /// `NetWrapperCommandMsg::getDataOffset`.
    pub fn get_data_offset(&self) -> u32 {
        self.wrapper().map_or(0, |w| w.data_offset)
    }
    /// `NetWrapperCommandMsg::setDataOffset`.
    pub fn set_data_offset(&mut self, v: u32) {
        if let Some(w) = self.wrapper_mut() {
            w.data_offset = v;
        }
    }
    /// `NetWrapperCommandMsg::getChunkNumber`.
    pub fn get_chunk_number(&self) -> u32 {
        self.wrapper().map_or(0, |w| w.chunk_number)
    }
    /// `NetWrapperCommandMsg::setChunkNumber`.
    pub fn set_chunk_number(&mut self, v: u32) {
        if let Some(w) = self.wrapper_mut() {
            w.chunk_number = v;
        }
    }
    /// `NetWrapperCommandMsg::getNumChunks`.
    pub fn get_num_chunks(&self) -> u32 {
        self.wrapper().map_or(0, |w| w.num_chunks)
    }
    /// `NetWrapperCommandMsg::setNumChunks`.
    pub fn set_num_chunks(&mut self, v: u32) {
        if let Some(w) = self.wrapper_mut() {
            w.num_chunks = v;
        }
    }
    /// `NetWrapperCommandMsg::getTotalDataLength`.
    pub fn get_total_data_length(&self) -> u32 {
        self.wrapper().map_or(0, |w| w.total_data_length)
    }
    /// `NetWrapperCommandMsg::setTotalDataLength`.
    pub fn set_total_data_length(&mut self, v: u32) {
        if let Some(w) = self.wrapper_mut() {
            w.total_data_length = v;
        }
    }
    /// `NetWrapperCommandMsg::getWrappedCommandID`.
    pub fn get_wrapped_command_id(&self) -> u16 {
        self.wrapper().map_or(0, |w| w.wrapped_command_id)
    }
    /// `NetWrapperCommandMsg::setWrappedCommandID`.
    pub fn set_wrapped_command_id(&mut self, v: u16) {
        if let Some(w) = self.wrapper_mut() {
            w.wrapped_command_id = v;
        }
    }

    /// `getPortableFilename` of `NetFileCommandMsg` and `NetFileAnnounceCommandMsg`.
    pub fn get_portable_filename(&self) -> AsciiString {
        match &self.body {
            NetCommandBody::File { portable_filename, .. } | NetCommandBody::FileAnnounce { portable_filename, .. } => {
                portable_filename.clone()
            }
            _ => AsciiString::default(),
        }
    }
    /// `setPortableFilename` of the two file kinds.
    pub fn set_portable_filename(&mut self, name: AsciiString) {
        match &mut self.body {
            NetCommandBody::File { portable_filename, .. } | NetCommandBody::FileAnnounce { portable_filename, .. } => {
                *portable_filename = name;
            }
            _ => {}
        }
    }
    /// `getRealFilename` of the two file kinds: `TheGameState->portableMapPathToRealMapPath(m_portableFilename)`;
    /// `portable_to_real` is that function.
    pub fn get_real_filename(&self, portable_to_real: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
        portable_to_real(self.get_portable_filename().str())
    }
    /// `setRealFilename` of the two file kinds: `m_portableFilename = TheGameState->realMapPathToPortableMapPath(filename)`.
    pub fn set_real_filename(&mut self, filename: &[u8], real_to_portable: impl Fn(&[u8]) -> Vec<u8>) {
        let portable = real_to_portable(filename);
        let mut a = AsciiString::default();
        let _ = a.set(&portable);
        self.set_portable_filename(a);
    }
    /// `NetFileCommandMsg::getFileLength`.
    pub fn get_file_length(&self) -> u32 {
        if let NetCommandBody::File { data_length, .. } = &self.body { *data_length } else { 0 }
    }
    /// `NetFileCommandMsg::getFileData`.
    pub fn get_file_data(&self) -> Option<&[u8]> {
        if let NetCommandBody::File { data, .. } = &self.body { data.as_deref() } else { None }
    }
    /// `NetFileCommandMsg::setFileData`: sets `m_dataLength` and a copy of the data.
    pub fn set_file_data(&mut self, bytes: &[u8]) {
        if let NetCommandBody::File { data, data_length, .. } = &mut self.body {
            *data_length = bytes.len() as u32;
            *data = Some(bytes.to_vec());
        }
    }

    /// `NetFileAnnounceCommandMsg::getFileID` and `NetFileProgressCommandMsg::getFileID`.
    pub fn get_file_id(&self) -> u16 {
        match &self.body {
            NetCommandBody::FileAnnounce { file_id, .. } | NetCommandBody::FileProgress { file_id, .. } => *file_id,
            _ => 0,
        }
    }
    /// `setFileID` of the same two kinds.
    pub fn set_file_id(&mut self, v: u16) {
        match &mut self.body {
            NetCommandBody::FileAnnounce { file_id, .. } | NetCommandBody::FileProgress { file_id, .. } => *file_id = v,
            _ => {}
        }
    }
    /// `NetFileAnnounceCommandMsg::getPlayerMask`.
    pub fn get_file_player_mask(&self) -> u8 {
        if let NetCommandBody::FileAnnounce { player_mask, .. } = &self.body { *player_mask } else { 0 }
    }
    /// `NetFileAnnounceCommandMsg::setPlayerMask`.
    pub fn set_file_player_mask(&mut self, v: u8) {
        if let NetCommandBody::FileAnnounce { player_mask, .. } = &mut self.body {
            *player_mask = v;
        }
    }
    /// `NetFileProgressCommandMsg::getProgress`.
    pub fn get_progress(&self) -> i32 {
        if let NetCommandBody::FileProgress { progress, .. } = &self.body { *progress } else { 0 }
    }
    /// `NetFileProgressCommandMsg::setProgress`.
    pub fn set_progress(&mut self, v: i32) {
        if let NetCommandBody::FileProgress { progress, .. } = &mut self.body {
            *progress = v;
        }
    }

    /// `NetDisconnectScreenOffCommandMsg::getNewFrame`.
    pub fn get_new_frame(&self) -> u32 {
        if let NetCommandBody::DisconnectScreenOff { new_frame } = &self.body { *new_frame } else { 0 }
    }
    /// `NetDisconnectScreenOffCommandMsg::setNewFrame`.
    pub fn set_new_frame(&mut self, v: u32) {
        if let NetCommandBody::DisconnectScreenOff { new_frame } = &mut self.body {
            *new_frame = v;
        }
    }
    /// `NetFrameResendRequestCommandMsg::getFrameToResend`.
    pub fn get_frame_to_resend(&self) -> u32 {
        if let NetCommandBody::FrameResendRequest { frame_to_resend } = &self.body { *frame_to_resend } else { 0 }
    }
    /// `NetFrameResendRequestCommandMsg::setFrameToResend`.
    pub fn set_frame_to_resend(&mut self, v: u32) {
        if let NetCommandBody::FrameResendRequest { frame_to_resend } = &mut self.body {
            *frame_to_resend = v;
        }
    }
}

impl Default for NetCommandMsg {
    fn default() -> Self {
        NetCommandMsg::new()
    }
}
