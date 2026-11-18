//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/udp.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/udp.cpp
//!
//! # The socket is abstracted
//!
//! Upstream's `UDP` is a Winsock/BSD datagram socket. Here the socket is the [`UdpBackend`] enum: `Native` is a
//! non-blocking `std::net::UdpSocket` (not on `wasm32`), and `Queued` is a pair of datagram queues that a host
//! outside this crate fills and drains. The browser build supplies the transport that way (WebRTC data channels or
//! a WebSocket relay): it pops [`QueuedSocket::outbound`] datagrams and pushes received ones to
//! [`QueuedSocket::inbound`]. Nothing above `UDP` (`Transport`, the packet format) knows which backend it has.
//!
//! `GetStatus` mapped Winsock/errno codes to `sockStat`; here the backend sets the `sockStat` itself
//! (`m_lastError` holds a `sockStat` value). `GetWSAErrorString` is `DEBUG_LOGGING`-only and is not translated.
//! `SetInputBuffer`/`SetOutputBuffer`/`GetInputBuffer`/`GetOutputBuffer` are socket options with no use outside
//! this file in the game; they do nothing on the queued backend and are not forwarded on the native one (std has no
//! API for them).

use std::collections::VecDeque;

/// `enum sockStat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SockStat {
    /// `OK`.
    Ok = 0,
    /// `UNKNOWN`.
    Unknown = -1,
    /// `ISCONN`.
    IsConn = -2,
    /// `INPROGRESS`.
    InProgress = -3,
    /// `ALREADY`.
    Already = -4,
    /// `AGAIN`.
    Again = -5,
    /// `ADDRINUSE`.
    AddrInUse = -6,
    /// `ADDRNOTAVAIL`.
    AddrNotAvail = -7,
    /// `BADF`.
    BadF = -8,
    /// `CONNREFUSED`.
    ConnRefused = -9,
    /// `INTR`.
    Intr = -10,
    /// `NOTSOCK`.
    NotSock = -11,
    /// `PIPE`.
    Pipe = -12,
    /// `WOULDBLOCK`.
    WouldBlock = -13,
    /// `INVAL`.
    Inval = -14,
    /// `TIMEDOUT`.
    TimedOut = -15,
}

/// One datagram: the bytes and the peer's IPv4 address (host byte order) and port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    /// The payload.
    pub bytes: Vec<u8>,
    /// The peer's address, host byte order (`ntohl(sin_addr)`).
    pub addr: u32,
    /// The peer's port, host byte order.
    pub port: u16,
}

/// The queues of the host-supplied transport (browser: WebRTC / WebSocket).
#[derive(Debug, Default)]
pub struct QueuedSocket {
    /// Datagrams the host received for the game to `Read`, oldest first.
    pub inbound: VecDeque<Datagram>,
    /// Datagrams the game `Write`s for the host to send, oldest first.
    pub outbound: VecDeque<Datagram>,
    /// The address `getLocalAddr` reports (set by the host; 0 until then).
    pub local_ip: u32,
    /// The port the socket was bound to.
    pub local_port: u16,
    /// `AllowBroadcasts` was set.
    pub broadcasts: bool,
}

/// What the socket is.
#[derive(Debug)]
pub enum UdpBackend {
    /// `fd = 0`: no socket yet.
    None,
    /// A native non-blocking UDP socket.
    #[cfg(not(target_arch = "wasm32"))]
    Native(std::net::UdpSocket),
    /// Host-supplied datagram queues.
    Queued(QueuedSocket),
}

/// `class UDP`.
#[derive(Debug)]
pub struct UDP {
    /// `fd` and its socket.
    backend: UdpBackend,
    /// `myIP`.
    my_ip: u32,
    /// `myPort`.
    my_port: u16,
    /// `m_lastError` (a `sockStat` value).
    last_error: i32,
    /// Which backend `Bind` creates.
    want_queued: bool,
}

impl Default for UDP {
    fn default() -> Self {
        UDP::new()
    }
}

impl UDP {
    /// `UDP::UDP()` (udp.cpp): `fd = 0`. `Bind` creates a native socket (a queued one on `wasm32`).
    pub fn new() -> UDP {
        UDP {
            backend: UdpBackend::None,
            my_ip: 0,
            my_port: 0,
            last_error: 0,
            want_queued: cfg!(target_arch = "wasm32"),
        }
    }

    /// A socket whose `Bind` creates the host-supplied queued backend.
    pub fn new_queued() -> UDP {
        let mut u = UDP::new();
        u.want_queued = true;
        u
    }

    /// The queued backend's queues (the host's side), when this socket is queued and bound.
    pub fn queued_mut(&mut self) -> Option<&mut QueuedSocket> {
        match &mut self.backend {
            UdpBackend::Queued(q) => Some(q),
            _ => None,
        }
    }

    /// `UDP::Bind(const char *Host, UnsignedShort port)` (udp.cpp): a digit-first host is a dotted address, else
    /// it is resolved; an unresolvable host returns 0 (success, with no bind), as upstream does.
    pub fn bind_host(&mut self, host: &[u8], port: u16) -> i32 {
        if host.first().map_or(false, |c| c.is_ascii_digit()) {
            return self.bind(super::network_util::resolve_ip(host), port);
        }
        let ip = super::network_util::resolve_ip(host);
        if ip == 0 {
            return 0;
        }
        self.bind(ip, port)
    }

    /// `UDP::Bind(UnsignedInt IP, UnsignedShort Port)` (udp.cpp): creates and binds the socket, makes it
    /// non-blocking, records the local address. Returns `OK` (0) or the failing `sockStat`.
    pub fn bind(&mut self, ip: u32, port: u16) -> i32 {
        if self.want_queued {
            self.backend = UdpBackend::Queued(QueuedSocket { local_port: port, local_ip: ip, ..Default::default() });
            self.my_ip = ip;
            self.my_port = port;
            return SockStat::Ok as i32;
        }
        self.bind_native(ip, port)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn bind_native(&mut self, ip: u32, port: u16) -> i32 {
        let addr = std::net::SocketAddrV4::new(std::net::Ipv4Addr::from(ip), port);
        match std::net::UdpSocket::bind(addr) {
            Ok(sock) => {
                if let Ok(std::net::SocketAddr::V4(local)) = sock.local_addr() {
                    self.my_ip = u32::from(*local.ip());
                    self.my_port = local.port();
                }
                if sock.set_nonblocking(true).is_err() {
                    // fprintf(stderr, "Couldn't set nonblocking mode!\n")
                }
                self.backend = UdpBackend::Native(sock);
                SockStat::Ok as i32
            }
            Err(e) => {
                let st = match e.kind() {
                    std::io::ErrorKind::AddrInUse => SockStat::AddrInUse,
                    std::io::ErrorKind::AddrNotAvailable => SockStat::AddrNotAvail,
                    std::io::ErrorKind::Interrupted => SockStat::Intr,
                    std::io::ErrorKind::InvalidInput => SockStat::Inval,
                    std::io::ErrorKind::TimedOut => SockStat::TimedOut,
                    _ => SockStat::Unknown,
                };
                self.last_error = st as i32;
                st as i32
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn bind_native(&mut self, _ip: u32, _port: u16) -> i32 {
        SockStat::Unknown as i32
    }

    /// `UDP::getLocalAddr` (udp.cpp): `(myIP, myPort)`.
    pub fn get_local_addr(&self) -> (u32, u16) {
        (self.my_ip, self.my_port)
    }

    /// `UDP::Write` (udp.cpp): sends `msg` to `ip:port` (host byte order). `ADDRNOTAVAIL` (-7) when either is 0;
    /// otherwise the byte count sent, or -1 on failure.
    pub fn write(&mut self, msg: &[u8], ip: u32, port: u16) -> i32 {
        if ip == 0 || port == 0 {
            return SockStat::AddrNotAvail as i32;
        }
        self.clear_status();
        match &mut self.backend {
            UdpBackend::None => {
                self.last_error = SockStat::BadF as i32;
                -1
            }
            #[cfg(not(target_arch = "wasm32"))]
            UdpBackend::Native(sock) => {
                let to = std::net::SocketAddrV4::new(std::net::Ipv4Addr::from(ip), port);
                match sock.send_to(msg, to) {
                    Ok(n) => n as i32,
                    Err(_) => {
                        self.last_error = SockStat::Unknown as i32;
                        -1
                    }
                }
            }
            UdpBackend::Queued(q) => {
                q.outbound.push_back(Datagram { bytes: msg.to_vec(), addr: ip, port });
                msg.len() as i32
            }
        }
    }

    /// `UDP::Read` (udp.cpp): one datagram into `msg` (at most `msg.len()` bytes). Returns the byte count (0
    /// when nothing is waiting, i.e. `WSAEWOULDBLOCK`; -1 on a real error) and the sender's address and port.
    pub fn read(&mut self, msg: &mut [u8]) -> (i32, u32, u16) {
        match &mut self.backend {
            UdpBackend::None => (-1, 0, 0),
            #[cfg(not(target_arch = "wasm32"))]
            UdpBackend::Native(sock) => match sock.recv_from(msg) {
                Ok((n, from)) => {
                    let (a, p) = match from {
                        std::net::SocketAddr::V4(v4) => (u32::from(*v4.ip()), v4.port()),
                        std::net::SocketAddr::V6(v6) => (0, v6.port()),
                    };
                    (n as i32, a, p)
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        (0, 0, 0)
                    } else {
                        self.last_error = SockStat::Unknown as i32;
                        (-1, 0, 0)
                    }
                }
            },
            UdpBackend::Queued(q) => match q.inbound.pop_front() {
                Some(d) => {
                    let n = std::cmp::min(d.bytes.len(), msg.len());
                    msg[..n].copy_from_slice(&d.bytes[..n]);
                    (n as i32, d.addr, d.port)
                }
                None => (0, 0, 0),
            },
        }
    }

    /// `UDP::GetStatus` (udp.cpp).
    pub fn get_status(&self) -> SockStat {
        match self.last_error {
            0 => SockStat::Ok,
            -2 => SockStat::IsConn,
            -3 => SockStat::InProgress,
            -4 => SockStat::Already,
            -5 => SockStat::Again,
            -6 => SockStat::AddrInUse,
            -7 => SockStat::AddrNotAvail,
            -8 => SockStat::BadF,
            -9 => SockStat::ConnRefused,
            -10 => SockStat::Intr,
            -11 => SockStat::NotSock,
            -12 => SockStat::Pipe,
            -13 => SockStat::WouldBlock,
            -14 => SockStat::Inval,
            -15 => SockStat::TimedOut,
            _ => SockStat::Unknown,
        }
    }

    /// `UDP::ClearStatus` (udp.cpp).
    pub fn clear_status(&mut self) {
        self.last_error = 0;
    }

    /// `UDP::getFD`: no file descriptor here; 0 when unbound, 1 when bound.
    pub fn get_fd(&self) -> i32 {
        match self.backend {
            UdpBackend::None => 0,
            _ => 1,
        }
    }

    /// `UDP::SetInputBuffer` (see the module doc): reports success.
    pub fn set_input_buffer(&mut self, _bytes: u32) -> i32 {
        1
    }
    /// `UDP::SetOutputBuffer`.
    pub fn set_output_buffer(&mut self, _bytes: u32) -> i32 {
        1
    }
    /// `UDP::GetInputBuffer`.
    pub fn get_input_buffer(&self) -> i32 {
        0
    }
    /// `UDP::GetOutputBuffer`.
    pub fn get_output_buffer(&self) -> i32 {
        0
    }

    /// `UDP::AllowBroadcasts` (udp.cpp): `SO_BROADCAST`; TRUE (1) on success.
    pub fn allow_broadcasts(&mut self, status: bool) -> i32 {
        match &mut self.backend {
            UdpBackend::None => 0,
            #[cfg(not(target_arch = "wasm32"))]
            UdpBackend::Native(sock) => i32::from(sock.set_broadcast(status).is_ok()),
            UdpBackend::Queued(q) => {
                q.broadcasts = status;
                1
            }
        }
    }
}
