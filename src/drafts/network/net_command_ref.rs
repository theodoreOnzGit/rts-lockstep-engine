//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetCommandRef.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandRef.cpp
//!
//! A ref is a node of an intrusive doubly linked list owned by one `NetCommandList`; `m_next`/`m_prev` are
//! [`NetCommandRefId`] slot indices into that list's arena (see `net_command_list`).

use super::net_command_msg::NetCommandMsgPtr;

/// A [`NetCommandRef`]'s place in its [`NetCommandList`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetCommandRefId(pub usize);

/// `class NetCommandRef`. (Not `Clone`: a copy would skip the `attach` the constructor does.)
#[derive(Debug)]
pub struct NetCommandRef {
    /// `m_msg`.
    pub msg: NetCommandMsgPtr,
    /// `m_next`.
    pub next: Option<NetCommandRefId>,
    /// `m_prev`.
    pub prev: Option<NetCommandRefId>,
    /// `m_relay`: the send relay of this reference.
    pub relay: u8,
    /// `m_timeLastSent` (`time_t`, -1 until sent).
    pub time_last_sent: i64,
}

impl NetCommandRef {
    /// `NetCommandRef::NetCommandRef(NetCommandMsg *msg)` (NetCommandRef.cpp): attaches to the message
    /// (`m_msg->attach()`), unlinked, `m_timeLastSent` -1. `m_relay` is uninitialised upstream; 0 here.
    pub fn new(msg: NetCommandMsgPtr) -> NetCommandRef {
        msg.write().attach();
        NetCommandRef { msg, next: None, prev: None, relay: 0, time_last_sent: -1 }
    }

    /// `getCommand`.
    pub fn get_command(&self) -> &NetCommandMsgPtr {
        &self.msg
    }
    /// `getNext`.
    pub fn get_next(&self) -> Option<NetCommandRefId> {
        self.next
    }
    /// `getPrev`.
    pub fn get_prev(&self) -> Option<NetCommandRefId> {
        self.prev
    }
    /// `setNext`.
    pub fn set_next(&mut self, next: Option<NetCommandRefId>) {
        self.next = next;
    }
    /// `setPrev`.
    pub fn set_prev(&mut self, prev: Option<NetCommandRefId>) {
        self.prev = prev;
    }
    /// `setRelay`.
    pub fn set_relay(&mut self, relay: u8) {
        self.relay = relay;
    }
    /// `getRelay`.
    pub fn get_relay(&self) -> u8 {
        self.relay
    }
    /// `getTimeLastSent`.
    pub fn get_time_last_sent(&self) -> i64 {
        self.time_last_sent
    }
    /// `setTimeLastSent`.
    pub fn set_time_last_sent(&mut self, t: i64) {
        self.time_last_sent = t;
    }
}

impl Drop for NetCommandRef {
    /// `NetCommandRef::~NetCommandRef`: `m_msg->detach()`.
    fn drop(&mut self) {
        let _ = self.msg.write().detach();
    }
}
