//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetCommandList.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandList.cpp
//!
//! # Shape
//!
//! `NetCommandRef` is a node of an intrusive doubly linked list (`m_next`/`m_prev`) owned by exactly one
//! `NetCommandList`. Here the list owns its nodes in a slot arena and names them with [`NetCommandRefId`];
//! `m_next`/`m_prev` are slot indices and every link is written in upstream's order, so `addMessage`'s
//! insertion logic (including its `m_lastMessageInserted` shortcut and its `||` conditions) is upstream's. A ref
//! that is not on a list (the result of `NetPacket::ConstructNetCommandMsgFromRawData`, or one taken off with
//! [`NetCommandList::remove_message`]) is a plain [`NetCommandRef`] value.

use super::net_command_msg::{NetCommandBody, NetCommandMsg, NetCommandMsgPtr, NetCommandType};
use super::net_command_ref::{NetCommandRef, NetCommandRefId};
use super::network_util::does_command_require_a_command_id;

/// `class NetCommandList`: an ordered linked list of [`NetCommandRef`]s, ordered by command type, then player id,
/// then sort number.
#[derive(Debug, Default)]
pub struct NetCommandList {
    slots: Vec<Option<NetCommandRef>>,
    free: Vec<usize>,
    /// `m_first`.
    first: Option<NetCommandRefId>,
    /// `m_last`.
    last: Option<NetCommandRefId>,
    /// `m_lastMessageInserted`.
    last_message_inserted: Option<NetCommandRefId>,
}

impl NetCommandList {
    /// `NetCommandList::NetCommandList()`.
    pub fn new() -> NetCommandList {
        NetCommandList::default()
    }

    fn r(&self, id: NetCommandRefId) -> &NetCommandRef {
        self.slots[id.0].as_ref().expect("stale NetCommandRefId")
    }
    fn rm(&mut self, id: NetCommandRefId) -> &mut NetCommandRef {
        self.slots[id.0].as_mut().expect("stale NetCommandRefId")
    }

    /// The ref `id` names, `None` when it is not on this list.
    pub fn get(&self, id: NetCommandRefId) -> Option<&NetCommandRef> {
        self.slots.get(id.0).and_then(|s| s.as_ref())
    }
    /// The ref `id` names, mutable.
    pub fn get_mut(&mut self, id: NetCommandRefId) -> Option<&mut NetCommandRef> {
        self.slots.get_mut(id.0).and_then(|s| s.as_mut())
    }

    fn alloc(&mut self, node: NetCommandRef) -> NetCommandRefId {
        if let Some(i) = self.free.pop() {
            self.slots[i] = Some(node);
            NetCommandRefId(i)
        } else {
            self.slots.push(Some(node));
            NetCommandRefId(self.slots.len() - 1)
        }
    }

    // the (type, player, id, sortNumber) of a ref's command
    fn ty(&self, id: NetCommandRefId) -> NetCommandType {
        self.r(id).msg.read().get_net_command_type()
    }
    fn player(&self, id: NetCommandRefId) -> u32 {
        self.r(id).msg.read().get_player_id()
    }
    fn cmd_id(&self, id: NetCommandRefId) -> u16 {
        self.r(id).msg.read().get_id()
    }
    fn sort(&self, id: NetCommandRefId) -> i32 {
        self.r(id).msg.read().get_sort_number()
    }

    /// `NetCommandList::getFirstMessage`.
    pub fn get_first_message(&self) -> Option<NetCommandRefId> {
        self.first
    }

    /// `m_last`.
    pub fn get_last_message(&self) -> Option<NetCommandRefId> {
        self.last
    }

    /// `NetCommandList::init`: `reset()`.
    pub fn init(&mut self) {
        self.reset();
    }

    /// `NetCommandList::reset` (NetCommandList.cpp): deletes every ref.
    pub fn reset(&mut self) {
        let mut temp = self.first;
        while let Some(id) = temp {
            let next = self.r(id).next;
            // m_first->setNext(NULL); setPrev(NULL); deleteInstance()
            self.slots[id.0] = None;
            self.free.push(id.0);
            temp = next;
        }
        self.first = None;
        self.last = None;
        self.last_message_inserted = None;
    }

    /// `NetCommandList::appendList` (NetCommandList.cpp): adds every message of `list` to this list, copying each
    /// relay.
    pub fn append_list(&mut self, list: Option<&NetCommandList>) {
        let list = match list {
            Some(l) => l,
            None => return,
        };
        let mut msg = list.get_first_message();
        while let Some(id) = msg {
            let next = list.r(id).next;
            let temp = self.add_message(list.r(id).msg.clone());
            if let Some(t) = temp {
                let relay = list.r(id).relay;
                self.rm(t).set_relay(relay);
            }
            msg = next;
        }
    }

    /// `NetCommandList::removeMessage` (NetCommandList.cpp): unlinks the ref and hands it back (upstream's caller
    /// then `deleteInstance()`s it or re-adds its command).
    pub fn remove_message(&mut self, id: NetCommandRefId) -> Option<NetCommandRef> {
        self.get(id)?;
        let (prev, next) = {
            let n = self.r(id);
            (n.prev, n.next)
        };
        if self.last_message_inserted == Some(id) {
            self.last_message_inserted = next;
        }
        if let Some(p) = prev {
            self.rm(p).set_next(next);
        }
        if let Some(n) = next {
            self.rm(n).set_prev(prev);
        }
        if self.first == Some(id) {
            self.first = next;
        }
        if self.last == Some(id) {
            self.last = prev;
        }
        let mut node = self.slots[id.0].take();
        self.free.push(id.0);
        if let Some(n) = node.as_mut() {
            n.set_next(None);
            n.set_prev(None);
        }
        node
    }

    /// `NetCommandList::addMessage` (NetCommandList.cpp): insert-sorts the message (by command type, then player id,
    /// then sort number) in a new ref and returns it; `None` when an equal command is already on the list (upstream
    /// deletes the new ref).
    pub fn add_message(&mut self, cmd_msg: NetCommandMsgPtr) -> Option<NetCommandRefId> {
        // `msg = NEW_NETCOMMANDREF(cmdMsg)`: attaches. On the "already in the list" paths the ref is dropped,
        // which detaches again.
        let mut node = NetCommandRef::new(cmd_msg);
        let mt = node.msg.read().get_net_command_type();
        let mp = node.msg.read().get_player_id();
        let mid = node.msg.read().get_id();
        let msort = node.msg.read().get_sort_number();

        let first = match self.first {
            None => {
                // this is the first node, so we don't have to worry about ordering it.
                let id = self.alloc(node);
                self.first = Some(id);
                self.last = Some(id);
                self.last_message_inserted = Some(id);
                return Some(id);
            }
            Some(f) => f,
        };
        let last = self.last.unwrap_or(first);

        if let Some(lmi) = self.last_message_inserted {
            // Messages inserted in order go right after each other: the saved placement of the last message
            // inserted is a huge efficiency boost.
            let the_next = self.r(lmi).next;
            let next_after = match the_next {
                None => true,
                Some(tn) => self.ty(tn) > mt || self.player(tn) > mp || self.cmd_id(tn) > mid,
            };
            if self.ty(lmi) == mt && self.player(lmi) == mp && self.cmd_id(lmi) < mid && next_after {
                if self.is_equal_command_msg_ids(lmi, &node.msg) {
                    return None;
                }
                match the_next {
                    None => {
                        // m_lastMessageInserted == m_last
                        node.set_next(self.r(lmi).next);
                        node.set_prev(Some(lmi));
                        let id = self.alloc(node);
                        self.rm(lmi).set_next(Some(id));
                        self.last_message_inserted = Some(id);
                        self.last = Some(id);
                        return Some(id);
                    }
                    Some(tn) => {
                        node.set_next(self.r(lmi).next);
                        node.set_prev(Some(lmi));
                        let id = self.alloc(node);
                        self.rm(lmi).set_next(Some(id));
                        self.rm(tn).set_prev(Some(id));
                        self.last_message_inserted = Some(id);
                        return Some(id);
                    }
                }
            }
        }

        if mt > self.ty(last) {
            // easy optimization for a command that goes at the end of the list
            if self.is_equal_command_msg_ids(last, &node.msg) {
                return None;
            }
            node.set_prev(Some(last));
            node.set_next(None);
            let id = self.alloc(node);
            self.rm(last).set_next(Some(id));
            self.last = Some(id);
            self.last_message_inserted = Some(id);
            return Some(id);
        }

        if mt < self.ty(first) {
            if self.is_equal_command_msg_ids(first, &node.msg) {
                return None;
            }
            // The command goes at the head of the list.
            node.set_next(Some(first));
            node.set_prev(None);
            let id = self.alloc(node);
            self.rm(first).set_prev(Some(id));
            self.first = Some(id);
            self.last_message_inserted = Some(id);
            return Some(id);
        }

        // Find the start of the command type we're looking for.
        let mut tempmsg = Some(first);
        while let Some(t) = tempmsg {
            if mt > self.ty(t) {
                tempmsg = self.r(t).next;
            } else {
                break;
            }
        }
        if tempmsg.is_none() {
            return self.append_at_end(node, last);
        }

        // Now find the player position.
        while let Some(t) = tempmsg {
            if mt == self.ty(t) && mp > self.player(t) {
                tempmsg = self.r(t).next;
            } else {
                break;
            }
        }
        if tempmsg.is_none() {
            return self.append_at_end(node, last);
        }

        // Find the position within the player's section based on the sort number.
        while let Some(t) = tempmsg {
            if mt == self.ty(t) && mp == self.player(t) && msort > self.sort(t) {
                tempmsg = self.r(t).next;
            } else {
                break;
            }
        }
        let tempmsg = match tempmsg {
            None => return self.append_at_end(node, last),
            Some(t) => t,
        };

        if tempmsg == first {
            if self.is_equal_command_msg_ids(first, &node.msg) {
                return None;
            }
            // This message goes at the head of the list.
            node.set_next(Some(first));
            node.set_prev(None);
            let id = self.alloc(node);
            self.rm(first).set_prev(Some(id));
            self.first = Some(id);
            self.last_message_inserted = Some(id);
            return Some(id);
        }

        if self.is_equal_command_msg_ids(tempmsg, &node.msg) {
            return None;
        }
        // Insert message before tempmsg.
        let tprev = self.r(tempmsg).prev;
        node.set_next(Some(tempmsg));
        node.set_prev(tprev);
        let id = self.alloc(node);
        if let Some(p) = tprev {
            self.rm(p).set_next(Some(id));
        }
        self.rm(tempmsg).set_prev(Some(id));
        self.last_message_inserted = Some(id);
        Some(id)
    }

    /// The three "message goes at the end of the list" tails of `addMessage`, which all test the duplicate against
    /// `m_last` first.
    fn append_at_end(&mut self, mut node: NetCommandRef, last: NetCommandRefId) -> Option<NetCommandRefId> {
        if self.is_equal_command_msg_ids(last, &node.msg) {
            return None;
        }
        node.set_prev(Some(last));
        node.set_next(None);
        let id = self.alloc(node);
        self.rm(last).set_next(Some(id));
        self.last = Some(id);
        self.last_message_inserted = Some(id);
        Some(id)
    }

    fn is_equal_command_msg_ids(&self, existing: NetCommandRefId, msg: &NetCommandMsgPtr) -> bool {
        let a = self.r(existing).msg.read();
        let b = msg.read();
        NetCommandList::is_equal_command_msg_values(&a, &b)
    }

    /// `NetCommandList::length` (NetCommandList.cpp): the number of nodes (a debug tool).
    pub fn length(&self) -> i32 {
        let mut retval = 0;
        let mut temp = self.first;
        while let Some(id) = temp {
            retval += 1;
            temp = self.r(id).next;
        }
        retval
    }

    /// `NetCommandList::findMessage(NetCommandMsg *msg)` (NetCommandList.cpp): the first ref whose command
    /// `isEqualCommandMsg` to `msg`.
    pub fn find_message(&self, msg: &NetCommandMsgPtr) -> Option<NetCommandRefId> {
        let mut retval = self.first;
        while let Some(id) = retval {
            if self.is_equal_command_msg_ids(id, msg) {
                return Some(id);
            }
            retval = self.r(id).next;
        }
        None
    }

    /// `NetCommandList::findMessage(UnsignedShort commandID, UnsignedByte playerID)` (NetCommandList.cpp): only
    /// checks commands of types that require a command id.
    pub fn find_message_by_id(&self, command_id: u16, player_id: u8) -> Option<NetCommandRefId> {
        let mut retval = self.first;
        while let Some(id) = retval {
            if does_command_require_a_command_id(self.ty(id))
                && self.cmd_id(id) == command_id
                && self.player(id) == player_id as u32
            {
                return Some(id);
            }
            retval = self.r(id).next;
        }
        None
    }

    /// `NetCommandList::isEqualCommandMsg` (NetCommandList.cpp).
    pub fn is_equal_command_msg(msg1: &NetCommandMsgPtr, msg2: &NetCommandMsgPtr) -> bool {
        NetCommandList::is_equal_command_msg_values(&msg1.read(), &msg2.read())
    }

    /// The body of `isEqualCommandMsg` over two messages.
    pub fn is_equal_command_msg_values(msg1: &NetCommandMsg, msg2: &NetCommandMsg) -> bool {
        if does_command_require_a_command_id(msg1.get_net_command_type())
            != does_command_require_a_command_id(msg2.get_net_command_type())
        {
            return false;
        }
        // At this point we know that the commands both do or do not require a command id.
        if does_command_require_a_command_id(msg1.get_net_command_type()) {
            // Are the commands from the same player?
            if msg1.get_player_id() != msg2.get_player_id() {
                return false;
            }
            // Do they have the same command ID?
            if msg1.get_id() != msg2.get_id() {
                return false;
            }
            return true;
        }
        // The commands do not require a command id: type-specific equality.
        if msg1.get_net_command_type() != msg2.get_net_command_type() {
            return false;
        }
        if msg1.get_player_id() != msg2.get_player_id() {
            return false;
        }
        match (&msg1.body, &msg2.body) {
            (NetCommandBody::AckStage1(a), NetCommandBody::AckStage1(b))
            | (NetCommandBody::AckStage2(a), NetCommandBody::AckStage2(b))
            | (NetCommandBody::AckBoth(a), NetCommandBody::AckBoth(b)) => {
                a.original_player_id == b.original_player_id && a.command_id == b.command_id
            }
            _ => false,
        }
    }
}
