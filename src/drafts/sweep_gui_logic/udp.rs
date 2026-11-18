//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/udp.cpp (UDP::SetBlocking)
//!
//! The first-pass `crate::drafts::network::udp::UDP` binds its socket non-blocking and keeps the backend private, so
//! this is drafted over a native socket. The queued (browser) backend has no blocking mode.

use crate::drafts::network::udp::SockStat;

/// `UDP::SetBlocking(block)` (udp.cpp:210): `ioctlsocket(fd, FIONBIO, ..)` on Windows, `fcntl` `O_NONBLOCK` elsewhere;
/// the std call for both is `set_nonblocking(!block)`. `OK`, or `UNKNOWN` when the call fails.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_blocking(socket: &std::net::UdpSocket, block: bool) -> SockStat {
    match socket.set_nonblocking(!block) {
        Ok(()) => SockStat::Ok,
        Err(_) => SockStat::Unknown,
    }
}
