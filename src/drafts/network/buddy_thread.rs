//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/BuddyThread.h
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/BuddyDefs.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/BuddyThread.cpp
//!
//! The GameSpy "GP" (buddy list) thread: a request queue the game fills, a response queue the thread fills, and the
//! thread body that turns requests into GP SDK calls and GP callbacks into responses.
//!
//! # How the SDK and the thread are abstracted
//!
//! The GameSpy GP SDK (`gpConnect`, `gpProcess`, ...) is gone and was never part of the repository (the vendored
//! `Libraries/Source/GameSpy` folder is empty), so every SDK call is the [`BuddySdkHost`]. The GP callbacks are
//! methods of [`BuddyThread`] that the host calls when the SDK would fire them. The thread's `while (running)` loop
//! body is [`BuddyThread::thread_step`], run by whoever owns the thread (native thread or a browser worker);
//! `Switch_Thread()` is the caller's yield. Message queues are `Arc<RwLock<..>>` shared between the game and the
//! thread, as AGENTS.md prescribes, in place of upstream's `MutexClass`.
//!
//! # Defects of upstream kept
//!
//! * `BUDDYREQUEST_DENYADD` has no `break` and falls through into `BUDDYREQUEST_SETSTATUS`. In C++ the request's
//!   `arg` is a union, so `arg.status.status` then aliases `arg.profile.id` (both at offset 0) and the strings are
//!   whatever the request's bytes hold. Here `arg` is flattened, so the fall-through sets the status to the profile
//!   id with empty strings (the choice made where C++ is undefined); the "Loading" guard applies as upstream.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use super::peer_thread::{DisconnectReason, PeerRequest, PeerRequestType, PeerResponse, PeerResponseType, PeerMessageQueue};

/// `MAX_BUDDY_CHAT_LEN`.
pub const MAX_BUDDY_CHAT_LEN: usize = 128;

/// `GPProfile`.
pub type GpProfile = i32;

/// `GPEnum` values the buddy code uses (the SDK header is not in the repository, so only names are kept).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GpStatus {
    /// `GP_OFFLINE`.
    #[default]
    Offline,
    /// `GP_ONLINE`.
    Online,
    /// `GP_PLAYING`.
    Playing,
    /// `GP_STAGING`.
    Staging,
    /// `GP_CHATTING`.
    Chatting,
}

/// `GPResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GpResult {
    /// `GP_NO_ERROR`.
    #[default]
    NoError,
    /// `GP_MEMORY_ERROR`.
    MemoryError,
    /// `GP_PARAMETER_ERROR`.
    ParameterError,
    /// `GP_NETWORK_ERROR`.
    NetworkError,
    /// `GP_SERVER_ERROR`.
    ServerError,
}

/// `GPErrorCode` (the ones `errorCallback` names and `connectCallback` switches on, plus the rest of the list that
/// `errorCallback` prints).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GpErrorCode {
    /// No error recorded (`m_lastErrorCode` = 0).
    #[default]
    None,
    General,
    Parse,
    NotLoggedIn,
    BadSesskey,
    Database,
    Network,
    ForcedDisconnect,
    ConnectionClosed,
    Login,
    LoginTimeout,
    LoginBadNick,
    LoginBadEmail,
    LoginBadPassword,
    LoginBadProfile,
    LoginProfileDeleted,
    LoginConnectionFailed,
    LoginServerAuthFailed,
    NewUser,
    NewUserBadNick,
    NewUserBadPassword,
    UpdateUi,
    UpdateUiBadEmail,
    NewProfile,
    NewProfileBadNick,
    NewProfileBadOldNick,
    UpdatePro,
    UpdateProBadNick,
    AddBuddy,
    AddBuddyBadFrom,
    AddBuddyBadNew,
    AddBuddyAlreadyBuddy,
    AuthAdd,
    AuthAddBadFrom,
    AuthAddBadSig,
    Status,
    Bm,
    BmNotBuddy,
    GetProfile,
    GetProfileBadProfile,
    DelBuddy,
    DelBuddyNotBuddy,
    DelProfile,
    DelProfileLastProfile,
    Search,
    SearchConnectionFailed,
}

/// `BuddyRequest::buddyRequestType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BuddyRequestType {
    /// `BUDDYREQUEST_LOGIN`.
    #[default]
    Login,
    /// `BUDDYREQUEST_RELOGIN`.
    Relogin,
    /// `BUDDYREQUEST_LOGOUT`.
    Logout,
    /// `BUDDYREQUEST_MESSAGE`.
    Message,
    /// `BUDDYREQUEST_LOGINNEW`.
    LoginNew,
    /// `BUDDYREQUEST_ADDBUDDY`.
    AddBuddy,
    /// `BUDDYREQUEST_DELBUDDY`.
    DelBuddy,
    /// `BUDDYREQUEST_OKADD`.
    OkAdd,
    /// `BUDDYREQUEST_DENYADD`.
    DenyAdd,
    /// `BUDDYREQUEST_SETSTATUS`.
    SetStatus,
    /// `BUDDYREQUEST_DELETEACCT`.
    DeleteAcct,
}

/// `BuddyRequest`: the union `arg` is flattened into one struct (see the module doc for the one place that matters).
#[derive(Clone, Debug, Default)]
pub struct BuddyRequest {
    /// `buddyRequestType`.
    pub buddy_request_type: BuddyRequestType,
    /// `arg.message.recipient` / `arg.addbuddy.id` / `arg.profile.id`.
    pub profile: GpProfile,
    /// `arg.message.text` / `arg.addbuddy.text` (at most `MAX_BUDDY_CHAT_LEN` wide chars).
    pub text: Vec<u16>,
    /// `arg.login.nick`.
    pub nick: Vec<u8>,
    /// `arg.login.email`.
    pub email: Vec<u8>,
    /// `arg.login.password`.
    pub password: Vec<u8>,
    /// `arg.login.hasFirewall`.
    pub has_firewall: bool,
    /// `arg.status.status`.
    pub status: GpStatus,
    /// `arg.status.statusString`.
    pub status_string: Vec<u8>,
    /// `arg.status.locationString`.
    pub location_string: Vec<u8>,
}

/// `BuddyResponse::buddyResponseType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BuddyResponseType {
    /// `BUDDYRESPONSE_LOGIN`.
    #[default]
    Login,
    /// `BUDDYRESPONSE_DISCONNECT`.
    Disconnect,
    /// `BUDDYRESPONSE_MESSAGE`.
    Message,
    /// `BUDDYRESPONSE_REQUEST`.
    Request,
    /// `BUDDYRESPONSE_STATUS`.
    Status,
}

/// `BuddyResponse` (the union `arg` flattened).
#[derive(Clone, Debug, Default)]
pub struct BuddyResponse {
    /// `buddyResponseType`.
    pub buddy_response_type: BuddyResponseType,
    /// `profile`.
    pub profile: GpProfile,
    /// `result`.
    pub result: GpResult,
    /// `arg.message.date`.
    pub date: u32,
    /// `arg.message.nick` / `arg.request.nick` / `arg.status.nick`.
    pub nick: Vec<u8>,
    /// `arg.message.text` / `arg.request.text`.
    pub text: Vec<u16>,
    /// `arg.request.email` / `arg.status.email`.
    pub email: Vec<u8>,
    /// `arg.request.countrycode` / `arg.status.countrycode`.
    pub country_code: Vec<u8>,
    /// `arg.error.errorCode`.
    pub error_code: GpErrorCode,
    /// `arg.error.errorString`.
    pub error_string: Vec<u8>,
    /// `arg.error.fatal`.
    pub fatal: bool,
    /// `arg.status.location`.
    pub location: Vec<u8>,
    /// `arg.status.status`.
    pub status: GpStatus,
    /// `arg.status.statusString`.
    pub status_string: Vec<u8>,
}

/// The request/response queues of `GameSpyBuddyMessageQueue`.
#[derive(Debug, Default)]
struct BuddyQueues {
    /// `m_requests`.
    requests: VecDeque<BuddyRequest>,
    /// `m_responses`.
    responses: VecDeque<BuddyResponse>,
}

/// `class GameSpyBuddyMessageQueue` (BuddyThread.cpp): the queues plus the thread's connection flags that
/// `isConnected`/`isConnecting`/`getLocalProfileID` read.
#[derive(Clone, Debug, Default)]
pub struct BuddyMessageQueue {
    queues: Arc<RwLock<BuddyQueues>>,
    /// The thread's state, `m_thread` (None before `startThread`).
    thread: Arc<RwLock<Option<BuddyThreadState>>>,
}

/// What `isConnected`, `isConnecting` and `getLocalProfileID` read from the thread.
#[derive(Clone, Copy, Debug, Default)]
pub struct BuddyThreadState {
    /// `m_isConnecting`.
    pub is_connecting: bool,
    /// `m_isConnected`.
    pub is_connected: bool,
    /// `m_profileID`.
    pub profile_id: GpProfile,
}

impl BuddyMessageQueue {
    /// `GameSpyBuddyMessageQueueInterface::createNewMessageQueue` / `GameSpyBuddyMessageQueue()`: no thread.
    pub fn new() -> BuddyMessageQueue {
        BuddyMessageQueue::default()
    }
    /// `GameSpyBuddyMessageQueue::startThread` (BuddyThread.cpp): creates the thread state if there is none. (Starting
    /// the OS thread is the owner's job.)
    pub fn start_thread(&self) {
        if let Ok(mut t) = self.thread.write() {
            if t.is_none() {
                *t = Some(BuddyThreadState::default());
            }
        }
    }
    /// `GameSpyBuddyMessageQueue::endThread` (BuddyThread.cpp): drops the thread.
    pub fn end_thread(&self) {
        if let Ok(mut t) = self.thread.write() {
            *t = None;
        }
    }
    /// `isThreadRunning` (`m_thread->Is_Running()`; here, whether the thread exists).
    pub fn is_thread_running(&self) -> bool {
        self.thread.read().map(|t| t.is_some()).unwrap_or(false)
    }
    /// `isConnected`.
    pub fn is_connected(&self) -> bool {
        self.thread.read().ok().and_then(|t| t.map(|s| s.is_connected)).unwrap_or(false)
    }
    /// `isConnecting`.
    pub fn is_connecting(&self) -> bool {
        self.thread.read().ok().and_then(|t| t.map(|s| s.is_connecting)).unwrap_or(false)
    }
    /// `getLocalProfileID`: 0 without a thread.
    pub fn get_local_profile_id(&self) -> GpProfile {
        self.thread.read().ok().and_then(|t| t.map(|s| s.profile_id)).unwrap_or(0)
    }
    /// `addRequest`: pushes, or drops the request if the lock cannot be taken (`m.Failed()`).
    pub fn add_request(&self, req: BuddyRequest) {
        if let Ok(mut q) = self.queues.write() {
            q.requests.push_back(req);
        }
    }
    /// `getRequest`: pops the front, None if empty or the lock is unavailable.
    pub fn get_request(&self) -> Option<BuddyRequest> {
        self.queues.write().ok().and_then(|mut q| q.requests.pop_front())
    }
    /// `addResponse`.
    pub fn add_response(&self, resp: BuddyResponse) {
        if let Ok(mut q) = self.queues.write() {
            q.responses.push_back(resp);
        }
    }
    /// `getResponse`.
    pub fn get_response(&self) -> Option<BuddyResponse> {
        self.queues.write().ok().and_then(|mut q| q.responses.pop_front())
    }
    fn set_state(&self, f: impl FnOnce(&mut BuddyThreadState)) {
        if let Ok(mut t) = self.thread.write() {
            if let Some(s) = t.as_mut() {
                f(s);
            }
        }
    }
}

/// `GPBuddyStatus` as `statusCallback` reads it from `gpGetBuddyStatus`.
#[derive(Clone, Debug, Default)]
pub struct GpBuddyStatus {
    /// `profile`.
    pub profile: GpProfile,
    /// `status`.
    pub status: GpStatus,
    /// `statusString`.
    pub status_string: Vec<u8>,
    /// `locationString`.
    pub location_string: Vec<u8>,
}

/// `GPGetInfoResponseArg` (the fields the three `getInfoResponseFor*` callbacks read).
#[derive(Clone, Debug, Default)]
pub struct GpGetInfoResponse {
    /// `profile`.
    pub profile: GpProfile,
    /// `nick`.
    pub nick: Vec<u8>,
    /// `email`.
    pub email: Vec<u8>,
    /// `countrycode`.
    pub country_code: Vec<u8>,
}

/// The GP SDK calls the buddy thread makes.
pub trait BuddySdkHost {
    /// `gpInitialize(con, 0)`.
    fn gp_initialize(&mut self);
    /// `gpDestroy(con)`.
    fn gp_destroy(&mut self);
    /// `gpConnect(...) == GP_NO_ERROR` with `GP_BLOCKING`; `has_firewall` picks `GP_FIREWALL`/`GP_NO_FIREWALL`.
    /// The SDK fires the connect callback ([`BuddyThread::connect_callback`]) during the call.
    fn gp_connect(&mut self, nick: &[u8], email: &[u8], password: &[u8], has_firewall: bool) -> bool;
    /// `gpConnectNewUser(...) == GP_NO_ERROR`.
    fn gp_connect_new_user(&mut self, nick: &[u8], email: &[u8], password: &[u8], has_firewall: bool) -> bool;
    /// `gpSetInfoMask(con, GP_MASK_NONE)`.
    fn gp_set_info_mask_none(&mut self);
    /// `gpDeleteProfile(con)`.
    fn gp_delete_profile(&mut self);
    /// `gpDisconnect(con)`.
    fn gp_disconnect(&mut self);
    /// `gpSendBuddyMessage(con, recipient, text)`.
    fn gp_send_buddy_message(&mut self, recipient: GpProfile, text: &[u8]);
    /// `gpSendBuddyRequest(con, id, text)`.
    fn gp_send_buddy_request(&mut self, id: GpProfile, text: &[u8]);
    /// `gpDeleteBuddy(con, id)`.
    fn gp_delete_buddy(&mut self, id: GpProfile);
    /// `gpAuthBuddyRequest(con, id)`.
    fn gp_auth_buddy_request(&mut self, id: GpProfile);
    /// `gpDenyBuddyRequest(con, id)`.
    fn gp_deny_buddy_request(&mut self, id: GpProfile);
    /// `gpSetStatus(con, status, statusString, locationString)`.
    fn gp_set_status(&mut self, status: GpStatus, status_string: &[u8], location_string: &[u8]);
    /// `gpIsConnected(con, &isConnected)`: (result == GP_NO_ERROR, isConnected == GP_CONNECTED).
    fn gp_is_connected(&mut self) -> (bool, bool);
    /// `gpProcess(con)`.
    fn gp_process(&mut self);
    /// `gpGetInfo(con, profile, GP_CHECK_CACHE, GP_BLOCKING, ...)`: the profile's info (the callback's argument).
    fn gp_get_info(&mut self, profile: GpProfile) -> GpGetInfoResponse;
    /// `gpGetBuddyStatus(con, index, &status)`.
    fn gp_get_buddy_status(&mut self, index: i32) -> GpBuddyStatus;
    /// `WideCharStringToMultiByte`.
    fn wide_to_multi_byte(&self, s: &[u16]) -> Vec<u8>;
    /// `MultiByteToWideCharSingleLine`.
    fn multi_byte_to_wide_single_line(&self, s: &[u8]) -> Vec<u16>;
    /// `TheGameSpyPeerMessageQueue` (the buddy thread starts a chat login and reads the chat state).
    fn peer_queue(&self) -> PeerMessageQueue;
}

/// `class BuddyThreadClass` (BuddyThread.cpp).
#[derive(Debug)]
pub struct BuddyThread {
    /// `TheGameSpyBuddyMessageQueue`.
    pub queue: BuddyMessageQueue,
    /// `m_isNewAccount`.
    is_new_account: bool,
    /// `m_isConnecting`.
    is_connecting: bool,
    /// `m_isConnected`.
    is_connected: bool,
    /// `m_profileID`.
    profile_id: GpProfile,
    /// `m_lastErrorCode`.
    last_error_code: GpErrorCode,
    /// `m_isdeleting`.
    is_deleting: bool,
    /// `m_nick`.
    nick: Vec<u8>,
    /// `m_email`.
    email: Vec<u8>,
    /// `m_pass`.
    pass: Vec<u8>,
    /// The thread function's `lastStatus`.
    last_status: GpStatus,
    /// The thread function's `lastStatusString`.
    last_status_string: Vec<u8>,
}

impl BuddyThread {
    /// `BuddyThreadClass::BuddyThreadClass()`: all flags false, profile 0, no error. Call
    /// [`BuddyThread::thread_start`] before stepping (the head of `Thread_Function`).
    pub fn new(queue: BuddyMessageQueue) -> BuddyThread {
        BuddyThread {
            queue,
            is_new_account: false,
            is_connecting: false,
            is_connected: false,
            profile_id: 0,
            last_error_code: GpErrorCode::None,
            is_deleting: false,
            nick: Vec::new(),
            email: Vec::new(),
            pass: Vec::new(),
            last_status: GpStatus::Offline,
            last_status_string: Vec::new(),
        }
    }

    /// `isConnecting`.
    pub fn is_connecting(&self) -> bool {
        self.is_connecting
    }
    /// `isConnected`.
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }
    /// `getLocalProfileID`.
    pub fn get_local_profile_id(&self) -> GpProfile {
        self.profile_id
    }

    fn publish(&self) {
        let (c, cn, p) = (self.is_connecting, self.is_connected, self.profile_id);
        self.queue.set_state(|s| {
            s.is_connecting = c;
            s.is_connected = cn;
            s.profile_id = p;
        });
    }

    /// The head of `BuddyThreadClass::Thread_Function`: `gpInitialize(con, 0)` and the connection flags cleared.
    /// (The `gpSetCallback` calls are the host routing SDK callbacks to this struct's methods.)
    pub fn thread_start<H: BuddySdkHost>(&mut self, host: &mut H) {
        host.gp_initialize();
        self.is_connected = false;
        self.is_connecting = false;
        self.last_status = GpStatus::Offline;
        self.last_status_string.clear();
        self.publish();
    }

    /// The tail of `Thread_Function` (`gpDestroy(con)`), after `running` went false.
    pub fn thread_end<H: BuddySdkHost>(&mut self, host: &mut H) {
        host.gp_destroy();
    }

    /// One pass of `Thread_Function`'s `while (running)` body: deal with one request, then update the network.
    pub fn thread_step<H: BuddySdkHost>(&mut self, host: &mut H) {
        if let Some(incoming) = self.queue.get_request() {
            self.handle_request(host, incoming);
        }

        // update the network
        let (res_ok, connected) = host.gp_is_connected();
        if connected && res_ok {
            host.gp_process();
        }
    }

    /// The request `switch` of `Thread_Function` (BuddyThread.cpp).
    pub fn handle_request<H: BuddySdkHost>(&mut self, host: &mut H, req: BuddyRequest) {
        match req.buddy_request_type {
            BuddyRequestType::Login => {
                self.is_connecting = true;
                self.nick = req.nick.clone();
                self.email = req.email.clone();
                self.pass = req.password.clone();
                self.publish();
                self.is_connected = host.gp_connect(&req.nick, &req.email, &req.password, req.has_firewall);
                self.is_connecting = false;
                self.publish();
            }
            BuddyRequestType::Relogin => {
                self.is_connecting = true;
                self.publish();
                let (n, e, p) = (self.nick.clone(), self.email.clone(), self.pass.clone());
                self.is_connected = host.gp_connect(&n, &e, &p, true); // GP_FIREWALL
                self.is_connecting = false;
                self.publish();
            }
            BuddyRequestType::DeleteAcct => {
                self.is_deleting = true;
                host.gp_delete_profile();
            }
            BuddyRequestType::Logout => {
                self.is_connecting = false;
                self.is_connected = false;
                self.publish();
                host.gp_disconnect();
            }
            BuddyRequestType::Message => {
                let s = host.wide_to_multi_byte(&req.text);
                host.gp_send_buddy_message(req.profile, &s);
            }
            BuddyRequestType::LoginNew => {
                self.is_connecting = true;
                self.nick = req.nick.clone();
                self.email = req.email.clone();
                self.pass = req.password.clone();
                self.is_new_account = true;
                self.publish();
                self.is_connected = host.gp_connect_new_user(&req.nick, &req.email, &req.password, req.has_firewall);
                if self.is_new_account {
                    // if we didn't re-login
                    host.gp_set_info_mask_none(); // don't share info
                }
                self.is_connecting = false;
                self.publish();
            }
            BuddyRequestType::AddBuddy => {
                let s = host.wide_to_multi_byte(&req.text);
                host.gp_send_buddy_request(req.profile, &s);
            }
            BuddyRequestType::DelBuddy => host.gp_delete_buddy(req.profile),
            BuddyRequestType::OkAdd => host.gp_auth_buddy_request(req.profile),
            BuddyRequestType::DenyAdd => {
                host.gp_deny_buddy_request(req.profile);
                // upstream: no `break` — falls through into BUDDYREQUEST_SETSTATUS (see the module doc)
                self.set_status(host, GpStatus::Offline, Vec::new(), Vec::new());
            }
            BuddyRequestType::SetStatus => {
                self.set_status(host, req.status, req.status_string, req.location_string);
            }
        }
    }

    /// `BUDDYREQUEST_SETSTATUS` body. The DENYADD fall-through calls it with status `Offline` and empty strings: the
    /// union would alias `arg.status.status` onto the profile id, whose GP enum meaning is not known here (the
    /// choice where C++ is undefined).
    fn set_status<H: BuddySdkHost>(
        &mut self,
        host: &mut H,
        status: GpStatus,
        status_string: Vec<u8>,
        location_string: Vec<u8>,
    ) {
        // don't blast our 'Loading' status with 'Online'.
        if self.last_status == GpStatus::Playing && self.last_status_string == b"Loading" && status == GpStatus::Online {
            return;
        }
        host.gp_set_status(status, &status_string, &location_string);
        self.last_status = status;
        self.last_status_string = status_string;
    }

    /// `BuddyThreadClass::errorCallback` (BuddyThread.cpp): remembers the error code; a fatal error posts a
    /// `BUDDYRESPONSE_DISCONNECT` (error string cut to `MAX_BUDDY_CHAT_LEN-1`), clears the connection flags and,
    /// if an account deletion was in progress, asks the peer thread to log out. The result/code name logging is
    /// dropped.
    pub fn error_callback<H: BuddySdkHost>(
        &mut self,
        host: &mut H,
        result: GpResult,
        error_code: GpErrorCode,
        error_string: &[u8],
        fatal: bool,
    ) {
        self.last_error_code = error_code;
        if fatal {
            let mut resp = BuddyResponse::default();
            resp.buddy_response_type = BuddyResponseType::Disconnect;
            resp.result = result;
            resp.error_code = error_code;
            resp.fatal = fatal;
            let mut es = error_string.to_vec();
            es.truncate(MAX_BUDDY_CHAT_LEN - 1);
            resp.error_string = es;
            self.is_connecting = false;
            self.is_connected = false;
            self.publish();
            self.queue.add_response(resp);
            if self.is_deleting {
                let mut req = PeerRequest::default();
                req.peer_request_type = PeerRequestType::Logout;
                host.peer_queue().add_request(req);
                self.is_deleting = false;
            }
        }
    }

    /// `BuddyThreadClass::messageCallback` (BuddyThread.cpp): a buddy message becomes a `BUDDYRESPONSE_MESSAGE`
    /// with the sender's nick (from `gpGetInfo`), the text as one wide line cut to `MAX_BUDDY_CHAT_LEN-1`, and
    /// the date.
    pub fn message_callback<H: BuddySdkHost>(&mut self, host: &mut H, profile: GpProfile, message: &[u8], date: u32) {
        let mut r = BuddyResponse::default();
        r.buddy_response_type = BuddyResponseType::Message;
        r.profile = profile;
        r.nick = host.gp_get_info(profile).nick; // getNickForMessage
        let mut s = host.multi_byte_to_wide_single_line(message);
        s.truncate(MAX_BUDDY_CHAT_LEN - 1);
        r.text = s;
        r.date = date;
        self.queue.add_response(r);
    }

    /// `BuddyThreadClass::connectCallback` (BuddyThread.cpp): on success posts `BUDDYRESPONSE_LOGIN` and, if the
    /// chat connection is neither up nor starting, asks the peer thread to log in; on failure (and only if the chat
    /// is not up or starting) either converts a "create account" onto an existing nick into a plain login, or posts a
    /// `PEERRESPONSE_DISCONNECT` whose reason maps from the last GP error code.
    pub fn connect_callback<H: BuddySdkHost>(&mut self, host: &mut H, result: GpResult, profile: GpProfile) {
        let peer = host.peer_queue();
        if result == GpResult::NoError {
            let mut login = BuddyResponse::default();
            login.buddy_response_type = BuddyResponseType::Login;
            login.result = result;
            login.profile = profile;
            self.queue.add_response(login);
            self.profile_id = profile;
            self.publish();

            if !peer.is_connected() && !peer.is_connecting() {
                let mut req = PeerRequest::default();
                req.peer_request_type = PeerRequestType::Login;
                req.nick = self.nick.clone();
                req.password = self.pass.clone();
                req.email = self.email.clone();
                req.login_profile_id = profile;
                peer.add_request(req);
            }
        } else if !peer.is_connected() && !peer.is_connecting() {
            if self.last_error_code == GpErrorCode::NewUserBadNick {
                self.is_new_account = false;
                // they just hit 'create account' instead of 'log in'.  Fix them.
                let mut req = BuddyRequest::default();
                req.buddy_request_type = BuddyRequestType::Login;
                req.nick = self.nick.clone();
                req.email = self.email.clone();
                req.password = self.pass.clone();
                req.has_firewall = true;
                self.queue.add_request(req);
                return;
            }
            let mut resp = PeerResponse::default();
            resp.peer_response_type = PeerResponseType::Disconnect;
            resp.discon_reason = DisconnectReason::CouldNotConnect;
            resp.discon_reason = match self.last_error_code {
                GpErrorCode::LoginTimeout => DisconnectReason::GpLoginTimeout,
                GpErrorCode::LoginBadNick => DisconnectReason::GpLoginBadNick,
                GpErrorCode::LoginBadEmail => DisconnectReason::GpLoginBadEmail,
                GpErrorCode::LoginBadPassword => DisconnectReason::GpLoginBadPassword,
                GpErrorCode::LoginBadProfile => DisconnectReason::GpLoginBadProfile,
                GpErrorCode::LoginProfileDeleted => DisconnectReason::GpLoginProfileDeleted,
                GpErrorCode::LoginConnectionFailed => DisconnectReason::GpLoginConnectionFailed,
                GpErrorCode::LoginServerAuthFailed => DisconnectReason::GpLoginServerAuthFailed,
                GpErrorCode::NewUserBadNick => DisconnectReason::GpNewUserBadNick,
                GpErrorCode::NewUserBadPassword => DisconnectReason::GpNewUserBadPassword,
                GpErrorCode::NewProfileBadNick => DisconnectReason::GpNewProfileBadNick,
                GpErrorCode::NewProfileBadOldNick => DisconnectReason::GpNewProfileBadOldNick,
                _ => resp.discon_reason,
            };
            peer.add_response(resp);
        }
    }

    /// `BuddyThreadClass::requestCallback` (BuddyThread.cpp): a buddy request becomes a `BUDDYRESPONSE_REQUEST`
    /// with the asker's nick/email/country (from `gpGetInfo`) and the reason cut to `GP_REASON_LEN-1`
    /// (`reason_len` = `GP_REASON_LEN`).
    pub fn request_callback<H: BuddySdkHost>(&mut self, host: &mut H, profile: GpProfile, reason: &[u8], reason_len: usize) {
        let mut r = BuddyResponse::default();
        r.buddy_response_type = BuddyResponseType::Request;
        r.profile = profile;
        let info = host.gp_get_info(profile); // getInfoResponseForRequest
        r.profile = info.profile;
        r.nick = info.nick;
        r.email = info.email;
        r.country_code = info.country_code;
        let mut s = host.multi_byte_to_wide_single_line(reason);
        s.truncate(reason_len.saturating_sub(1));
        r.text = s;
        self.queue.add_response(r);
    }

    /// `BuddyThreadClass::statusCallback` (BuddyThread.cpp): a buddy's status becomes a `BUDDYRESPONSE_STATUS` with
    /// the nick/email/country from `gpGetInfo` and location/status from `gpGetBuddyStatus`.
    pub fn status_callback<H: BuddySdkHost>(&mut self, host: &mut H, profile: GpProfile, index: i32) {
        let mut r = BuddyResponse::default();
        r.buddy_response_type = BuddyResponseType::Status;
        let info = host.gp_get_info(profile); // getInfoResponseForStatus
        r.profile = info.profile;
        r.nick = info.nick;
        r.email = info.email;
        r.country_code = info.country_code;
        let status = host.gp_get_buddy_status(index);
        r.location = status.location_string;
        r.status_string = status.status_string;
        r.status = status.status;
        self.queue.add_response(r);
    }
}
