//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PingThread.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/PingThread.cpp
//!
//! The ping pool: a request queue of (hostname, repetitions, timeout), a response queue of averaged pings, the
//! host-name -> ping map that feeds the hex "ping string" of a staging room, and the worker body that resolves a
//! name and averages ICMP echoes.
//!
//! # Windows-only part
//!
//! `doPing` loads `ICMP.DLL` and calls `IcmpSendEcho`; `Thread_Function` uses winsock `gethostbyname`. Both are the
//! [`PingHost`] (`resolve_host`, `icmp_echo`); the ICMP reply parsing (round trip time at byte 8 of the reply
//! buffer, status at byte 4) is host-side. Ten worker threads (`NumWorkerThreads`) each run
//! [`PingWorker::thread_step`].
//!
//! # Defects of upstream
//!
//! * A failed `gethostbyname` leaves `hostStruct == NULL` and the code then dereferences `hostStruct->h_addr`
//!   (a crash) instead of using the `IP = 0xFFFFFFFF` it had just set. Here a failed lookup keeps
//!   `0xFFFFFFFF`, the evident intent: the request answers with `avgPing = -1`, `repetitions = 0`.
//! * `getPingString` divides by `timeout`; with `timeout == 0` C++ traps. Here a zero timeout gives 0 for every
//!   entry (the choice where C++ is undefined).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, RwLock};

/// `NumWorkerThreads`.
pub const NUM_WORKER_THREADS: usize = 10;

/// `class PingRequest`.
#[derive(Clone, Debug, Default)]
pub struct PingRequest {
    /// `hostname`.
    pub hostname: Vec<u8>,
    /// `repetitions`.
    pub repetitions: i32,
    /// `timeout` in milliseconds.
    pub timeout: i32,
}

/// `class PingResponse`.
#[derive(Clone, Debug, Default)]
pub struct PingResponse {
    /// `hostname`.
    pub hostname: Vec<u8>,
    /// `avgPing`.
    pub avg_ping: i32,
    /// `repetitions`: the number of echoes that came back.
    pub repetitions: i32,
}

#[derive(Debug, Default)]
struct PingerState {
    requests: VecDeque<PingRequest>,
    responses: VecDeque<PingResponse>,
    request_count: i32,
    response_count: i32,
    /// `m_pingMap` (ordered by host name, as `std::map<std::string, Int>`).
    ping_map: BTreeMap<Vec<u8>, i32>,
    threads_running: bool,
}

/// `class Pinger` (PingThread.cpp).
#[derive(Clone, Debug, Default)]
pub struct Pinger {
    inner: Arc<RwLock<PingerState>>,
}

impl Pinger {
    /// `Pinger::Pinger()`: empty queues, counts 0, no threads.
    pub fn new() -> Pinger {
        Pinger::default()
    }
    /// `startThreads` (the worker threads are the owner's: [`NUM_WORKER_THREADS`] of [`PingWorker`]).
    pub fn start_threads(&self) {
        self.end_threads();
        if let Ok(mut s) = self.inner.write() {
            s.threads_running = true;
        }
    }
    /// `endThreads`.
    pub fn end_threads(&self) {
        if let Ok(mut s) = self.inner.write() {
            s.threads_running = false;
        }
    }
    /// `areThreadsRunning`.
    pub fn are_threads_running(&self) -> bool {
        self.inner.read().map(|s| s.threads_running).unwrap_or(false)
    }
    /// `addRequest`: counts the request and queues it.
    pub fn add_request(&self, req: PingRequest) {
        if let Ok(mut s) = self.inner.write() {
            s.request_count += 1;
            s.requests.push_back(req);
        }
    }
    /// `getRequest`.
    pub fn get_request(&self) -> Option<PingRequest> {
        self.inner.write().ok().and_then(|mut s| s.requests.pop_front())
    }
    /// `addResponse`: records the ping under the host name, then counts and queues the response.
    pub fn add_response(&self, resp: PingResponse) {
        if let Ok(mut s) = self.inner.write() {
            s.ping_map.insert(resp.hostname.clone(), resp.avg_ping);
            s.response_count += 1;
            s.responses.push_back(resp);
        }
    }
    /// `getResponse`.
    pub fn get_response(&self) -> Option<PingResponse> {
        self.inner.write().ok().and_then(|mut s| s.responses.pop_front())
    }
    /// `arePingsInProgress`: more requests than responses so far.
    pub fn are_pings_in_progress(&self) -> bool {
        self.inner.read().map(|s| s.request_count != s.response_count).unwrap_or(false)
    }
    /// `getPing`: the last average for the host, -1 if none.
    pub fn get_ping(&self, hostname: &[u8]) -> i32 {
        self.inner.read().ok().and_then(|s| s.ping_map.get(hostname).copied()).unwrap_or(-1)
    }
    /// `clearPingMap`.
    pub fn clear_ping_map(&self) {
        if let Ok(mut s) = self.inner.write() {
            s.ping_map.clear();
        }
    }
    /// `getPingString`: for each map entry in host-name order, the ping (a failed or over-`timeout` ping counts as
    /// `timeout`) scaled to 0..255 as two uppercase hex digits.
    pub fn get_ping_string(&self, timeout: i32) -> Vec<u8> {
        let mut ping_string = Vec::new();
        if let Ok(s) = self.inner.read() {
            for (_, &ping) in s.ping_map.iter() {
                let mut p = ping;
                if p < 0 || p > timeout {
                    p = timeout;
                }
                p = if timeout == 0 { 0 } else { p.wrapping_mul(255) / timeout };
                ping_string.extend_from_slice(format!("{:02X}", p).as_bytes());
            }
        }
        ping_string
    }
}

/// The name resolution and ICMP of the worker (Windows-only upstream).
pub trait PingHost {
    /// `isdigit(hostname[0])` -> `inet_addr`, else `gethostbyname`: the IPv4 address as the socket API gives it
    /// (network byte order in a `u32`); None if the lookup failed.
    fn resolve_host(&mut self, hostname: &[u8]) -> Option<u32>;
    /// `PingThreadClass::doPing(IP, timeout)` with `IcmpSendEcho`: the round trip time in ms (cut to `timeout`),
    /// or -1 if no reply or ICMP is unavailable.
    fn icmp_echo(&mut self, ip: u32, timeout: i32) -> i32;
}

/// `class PingThreadClass` (one of the ten workers).
#[derive(Clone, Debug)]
pub struct PingWorker {
    /// `ThePinger`.
    pub pinger: Pinger,
}

impl PingWorker {
    /// `PingThreadClass::PingThreadClass()`.
    pub fn new(pinger: Pinger) -> PingWorker {
        PingWorker { pinger }
    }

    /// One pass of `Thread_Function`'s `while (running)` body: takes one request, resolves the host, pings
    /// `repetitions` times (stopping early if `running` goes false, yielding between echoes via `yield_fn`),
    /// averages the good replies (-1 if none) and posts the response.
    pub fn thread_step<H: PingHost>(&mut self, host: &mut H, running: &dyn Fn() -> bool, yield_fn: &mut dyn FnMut()) {
        if let Some(req) = self.pinger.get_request() {
            // resolve the hostname
            let mut ip: u32 = 0xFFFF_FFFF;
            if let Some(resolved) = host.resolve_host(&req.hostname) {
                ip = resolved;
            }

            // do ping
            let mut total_ping: i32 = 0;
            let mut good_reps: i32 = 0;
            let mut reps = req.repetitions;
            while {
                let go = reps != 0;
                reps = reps.wrapping_sub(1);
                go
            } && running()
                && ip != 0xFFFF_FFFF
            {
                let ping = host.icmp_echo(ip, req.timeout);
                if ping >= 0 {
                    total_ping += ping;
                    good_reps += 1;
                }
                // end our timeslice
                yield_fn();
            }
            if good_reps == 0 {
                total_ping = -1;
            } else {
                total_ping /= good_reps;
            }

            let resp = PingResponse { hostname: req.hostname, avg_ping: total_ping, repetitions: good_reps };
            self.pinger.add_response(resp);
        }
        // end our timeslice
        yield_fn();
    }
}
