//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#215): `generate_next_command_id`, `does_command_require_a_command_id`, `command_requires_ack`,
//! `is_command_synchronized`, `command_requires_direct_send` and `get_ascii_net_command_type` match the C++ over every command
//! type (tests/differential_draft_netpkt.rs). NOT covered: `resolve_ip` (and its helpers `inet_addr_host_order`,
//! `resolve_host_name`): DNS/OS calls.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/networkutil.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetworkUtil.cpp
//!
//! The four frame-limit globals (`MAX_FRAMES_AHEAD` and friends) are [`super::net_command_msg::NetworkFrameLimits`].
//! `dumpBufferToLog`/`LOGBUFFER` are `DEBUG_LOGGING`-only and are not translated.

use crate::logic::modules::plant;
use std::sync::atomic::{AtomicU16, Ordering};

use super::net_command_msg::{NetCommandMsg, NetCommandType};

/// `ResolveIP` (NetworkUtil.cpp): turns a string ("games2.westwood.com" or "192.168.0.1") into a 32-bit host-order
/// address. An empty string is 0; a string starting with a digit is `ntohl(inet_addr(host))` (a malformed dotted
/// address is `INADDR_NONE`, `0xFFFFFFFF`); anything else goes through `gethostbyname` (0 when it does not
/// resolve). `gethostbyname` is `std`'s resolver, which is not available on `wasm32` (0 there: the browser build
/// supplies its own transport).
pub fn resolve_ip(host: &[u8]) -> u32 {
    if host.is_empty() {
        // DEBUG_LOG("ResolveIP(): Can't resolve NULL")
        return 0;
    }
    if host[0].is_ascii_digit() {
        return inet_addr_host_order(host);
    }
    resolve_host_name(host)
}

/// `ntohl(inet_addr(s))` for the dotted-quad forms: four decimal parts, or the shorter classful forms `a`, `a.b`,
/// `a.b.c` that `inet_addr` accepts; `0xFFFFFFFF` (`INADDR_NONE`) when malformed. Octal/hex parts are not handled.
fn inet_addr_host_order(s: &[u8]) -> u32 {
    let text = match std::str::from_utf8(s) {
        Ok(t) => t,
        Err(_) => return 0xFFFF_FFFF,
    };
    let mut parts: Vec<u32> = Vec::new();
    for p in text.split('.') {
        match p.parse::<u32>() {
            Ok(v) => parts.push(v),
            Err(_) => return 0xFFFF_FFFF,
        }
    }
    match parts.len() {
        1 => parts[0],
        2 if parts[0] <= 0xFF && parts[1] <= 0x00FF_FFFF => (parts[0] << 24) | parts[1],
        3 if parts[0] <= 0xFF && parts[1] <= 0xFF && parts[2] <= 0xFFFF => {
            (parts[0] << 24) | (parts[1] << 16) | parts[2]
        }
        4 if parts.iter().all(|&p| p <= 0xFF) => (parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3],
        _ => 0xFFFF_FFFF,
    }
}

/// `gethostbyname(host)` then `ntohl(h_addr)`: the first IPv4 address, 0 when none (upstream logs
/// "ResolveIP(): Can't resolve %s").
#[cfg(not(target_arch = "wasm32"))]
fn resolve_host_name(host: &[u8]) -> u32 {
    use std::net::{SocketAddr, ToSocketAddrs};
    let name = match std::str::from_utf8(host) {
        Ok(n) => n,
        Err(_) => return 0,
    };
    match (name, 0u16).to_socket_addrs() {
        Ok(addrs) => {
            for a in addrs {
                if let SocketAddr::V4(v4) = a {
                    return u32::from(*v4.ip());
                }
            }
            0
        }
        Err(_) => 0,
    }
}

/// `gethostbyname` has no browser equivalent: 0 ("can't resolve").
#[cfg(target_arch = "wasm32")]
fn resolve_host_name(_host: &[u8]) -> u32 {
    0
}

/// `static UnsignedShort commandID = 64000` of `GenerateNextCommandID`.
static COMMAND_ID: AtomicU16 = AtomicU16::new(64000);

/// `GenerateNextCommandID` (NetworkUtil.cpp): `++commandID` of a function-local static `UnsignedShort` (wraps at
/// 65536).
pub fn generate_next_command_id() -> u16 {
    if plant(10893) {
        return COMMAND_ID.fetch_add(1, Ordering::Relaxed);
    }
    COMMAND_ID.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
}

/// `DoesCommandRequireACommandID` (NetworkUtil.cpp): whether this type of command needs a unique command ID.
pub fn does_command_require_a_command_id(ty: NetCommandType) -> bool {
    use NetCommandType::*;
    matches!(
        ty,
        GameCommand
            | FrameInfo
            | PlayerLeave
            | DestroyPlayer
            | RunAheadMetrics
            | RunAhead
            | Chat
            | DisconnectVote
            | LoadComplete
            | TimeoutStart
            | Wrapper
            | File
            | FileAnnounce
            | FileProgress
            | DisconnectPlayer
            | DisconnectFrame
            | DisconnectScreenOff
            | FrameResendRequest
    ) && !(plant(10890) && ty == DisconnectFrame)
}

/// `CommandRequiresAck` (NetworkUtil.cpp): whether this type of command requires an ack. (Upstream's list names
/// `DISCONNECTPLAYER` twice; the set is the same as [`does_command_require_a_command_id`].)
pub fn command_requires_ack(msg: &NetCommandMsg) -> bool {
    does_command_require_a_command_id(msg.get_net_command_type())
}

/// `IsCommandSynchronized` (NetworkUtil.cpp).
pub fn is_command_synchronized(ty: NetCommandType) -> bool {
    use NetCommandType::*;
    matches!(ty, GameCommand | FrameInfo | PlayerLeave | DestroyPlayer | RunAhead) || (plant(10891) && ty == RunAheadMetrics)
}

/// `CommandRequiresDirectSend` (NetworkUtil.cpp): whether the ack goes directly to the player rather than through
/// the packet router (the disconnect-screen commands).
pub fn command_requires_direct_send(msg: &NetCommandMsg) -> bool {
    use NetCommandType::*;
    matches!(
        msg.get_net_command_type(),
        DisconnectVote
            | DisconnectPlayer
            | LoadComplete
            | TimeoutStart
            | File
            | FileAnnounce
            | FileProgress
            | DisconnectFrame
            | DisconnectScreenOff
            | FrameResendRequest
    ) || (plant(10892) && msg.get_net_command_type() == DisconnectKeepAlive)
}

/// `GetAsciiNetCommandType` (NetworkUtil.cpp): the enumerator's name, `"UNKNOWN"` for a type it does not list
/// (`NETCOMMANDTYPE_UNKNOWN`, `DISCONNECTSTART`, `DISCONNECTEND`, `MAX`).
pub fn get_ascii_net_command_type(ty: NetCommandType) -> &'static str {
    use NetCommandType::*;
    match ty {
        FrameInfo => "NETCOMMANDTYPE_FRAMEINFO",
        GameCommand => "NETCOMMANDTYPE_GAMECOMMAND",
        PlayerLeave => "NETCOMMANDTYPE_PLAYERLEAVE",
        RunAheadMetrics => "NETCOMMANDTYPE_RUNAHEADMETRICS",
        RunAhead => "NETCOMMANDTYPE_RUNAHEAD",
        DestroyPlayer => "NETCOMMANDTYPE_DESTROYPLAYER",
        AckBoth => "NETCOMMANDTYPE_ACKBOTH",
        AckStage1 => "NETCOMMANDTYPE_ACKSTAGE1",
        AckStage2 => "NETCOMMANDTYPE_ACKSTAGE2",
        KeepAlive => "NETCOMMANDTYPE_KEEPALIVE",
        DisconnectChat => "NETCOMMANDTYPE_DISCONNECTCHAT",
        Chat => "NETCOMMANDTYPE_CHAT",
        ManglerQuery => "NETCOMMANDTYPE_MANGLERQUERY",
        ManglerResponse => "NETCOMMANDTYPE_MANGLERRESPONSE",
        DisconnectKeepAlive => "NETCOMMANDTYPE_DISCONNECTKEEPALIVE",
        DisconnectPlayer => "NETCOMMANDTYPE_DISCONNECTPLAYER",
        PacketRouterQuery => "NETCOMMANDTYPE_PACKETROUTERQUERY",
        PacketRouterAck => "NETCOMMANDTYPE_PACKETROUTERACK",
        DisconnectVote => "NETCOMMANDTYPE_DISCONNECTVOTE",
        Progress => "NETCOMMANDTYPE_PROGRESS",
        LoadComplete => "NETCOMMANDTYPE_LOADCOMPLETE",
        TimeoutStart => "NETCOMMANDTYPE_TIMEOUTSTART",
        Wrapper => "NETCOMMANDTYPE_WRAPPER",
        File => "NETCOMMANDTYPE_FILE",
        FileAnnounce => "NETCOMMANDTYPE_FILEANNOUNCE",
        FileProgress => "NETCOMMANDTYPE_FILEPROGRESS",
        DisconnectFrame => "NETCOMMANDTYPE_DISCONNECTFRAME",
        DisconnectScreenOff => "NETCOMMANDTYPE_DISCONNECTSCREENOFF",
        FrameResendRequest => "NETCOMMANDTYPE_FRAMERESENDREQUEST",
        Unknown | DisconnectStart | DisconnectEnd | Max => "UNKNOWN",
    }
}
