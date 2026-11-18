//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/GameResultsThread.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/GameResultsThread.cpp
//!
//! The game-results sender: a request queue of (host, port, results text) and a worker that resolves the host
//! and sends the text over one TCP connection. Name resolution and the socket are the [`GameResultsHost`].
//!
//! # Defects of upstream kept
//!
//! * `Thread_Function` builds a `GameResultsResponse` but never calls `addResponse`; the response queue stays empty
//!   and `m_responseCount` stays 0. `areGameResultsBeingSent` is `m_requestCount > 0`, which therefore never goes
//!   false again once a request was added. Both kept.
//! * As in `PingThread.cpp`, a failed `gethostbyname` dereferences NULL; here the lookup failure leaves
//!   `IP = 0xFFFFFFFF` and the send is still attempted to that address (the evident intent was to answer anyway).
//! * `sendGameResults` returns -1 without closing the socket when `connect` fails with `WSAEWOULDBLOCK`,
//!   `WSAEINVAL` or `WSAEALREADY` (a socket leak); the host call here owns its socket, so the leak is not
//!   reproduced.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// `NumWorkerThreads` of GameResultsThread.cpp.
pub const NUM_WORKER_THREADS: usize = 1;

/// `class GameResultsRequest`.
#[derive(Clone, Debug, Default)]
pub struct GameResultsRequest {
    /// `hostname`.
    pub hostname: Vec<u8>,
    /// `port`.
    pub port: u16,
    /// `results`.
    pub results: Vec<u8>,
}

/// `class GameResultsResponse`.
#[derive(Clone, Debug, Default)]
pub struct GameResultsResponse {
    /// `hostname`.
    pub hostname: Vec<u8>,
    /// `port`.
    pub port: u16,
    /// `sentOk`.
    pub sent_ok: bool,
}

#[derive(Debug, Default)]
struct GameResultsState {
    requests: VecDeque<GameResultsRequest>,
    responses: VecDeque<GameResultsResponse>,
    request_count: i32,
    response_count: i32,
    threads_running: bool,
}

/// `class GameResultsQueue` (GameResultsThread.cpp).
#[derive(Clone, Debug, Default)]
pub struct GameResultsQueue {
    inner: Arc<RwLock<GameResultsState>>,
}

impl GameResultsQueue {
    /// `GameResultsQueue::GameResultsQueue()`: counts 0 and `startThreads()`.
    pub fn new() -> GameResultsQueue {
        let q = GameResultsQueue::default();
        q.start_threads();
        q
    }
    /// `startThreads`.
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
    /// `addRequest`: counts and queues.
    pub fn add_request(&self, req: GameResultsRequest) {
        if let Ok(mut s) = self.inner.write() {
            s.request_count += 1;
            s.requests.push_back(req);
        }
    }
    /// `getRequest`.
    pub fn get_request(&self) -> Option<GameResultsRequest> {
        self.inner.write().ok().and_then(|mut s| s.requests.pop_front())
    }
    /// `addResponse`: counts and queues.
    pub fn add_response(&self, resp: GameResultsResponse) {
        if let Ok(mut s) = self.inner.write() {
            s.response_count += 1;
            s.responses.push_back(resp);
        }
    }
    /// `getResponse`.
    pub fn get_response(&self) -> Option<GameResultsResponse> {
        self.inner.write().ok().and_then(|mut s| s.responses.pop_front())
    }
    /// `areGameResultsBeingSent`: `m_requestCount > 0` (true if the lock is busy).
    pub fn are_game_results_being_sent(&self) -> bool {
        self.inner.read().map(|s| s.request_count > 0).unwrap_or(true)
    }
}

/// The resolution and the TCP send of the worker.
pub trait GameResultsHost {
    /// `inet_addr` (host name starts with a digit) or `gethostbyname`: the IPv4 address in network byte order;
    /// None if the lookup failed.
    fn resolve_host(&mut self, hostname: &[u8]) -> Option<u32>;
    /// `GameResultsThreadClass::sendGameResults(IP, port, results)`: socket, connect, send, close. Returns the
    /// number of bytes sent, or a negative / error value.
    fn send_game_results(&mut self, ip: u32, port: u16, results: &[u8]) -> i64;
}

/// `class GameResultsThreadClass`.
#[derive(Clone, Debug)]
pub struct GameResultsWorker {
    /// `TheGameResultsQueue`.
    pub queue: GameResultsQueue,
}

impl GameResultsWorker {
    /// `GameResultsThreadClass::GameResultsThreadClass()`.
    pub fn new(queue: GameResultsQueue) -> GameResultsWorker {
        GameResultsWorker { queue }
    }

    /// One pass of `Thread_Function`'s `while (running)` body: sends one request's results. Returns the response
    /// the thread built, which upstream drops without queuing it (see the module doc); the owner may inspect it.
    pub fn thread_step<H: GameResultsHost>(&mut self, host: &mut H, yield_fn: &mut dyn FnMut()) -> Option<GameResultsResponse> {
        let mut built = None;
        if let Some(req) = self.queue.get_request() {
            let ip = host.resolve_host(&req.hostname).unwrap_or(0xFFFF_FFFF);
            let result = host.send_game_results(ip, req.port, &req.results);
            built = Some(GameResultsResponse {
                hostname: req.hostname.clone(),
                port: req.port,
                sent_ok: result == req.results.len() as i64,
            });
            // upstream never calls TheGameResultsQueue->addResponse(resp)
        }
        // end our timeslice
        yield_fn();
        built
    }
}
