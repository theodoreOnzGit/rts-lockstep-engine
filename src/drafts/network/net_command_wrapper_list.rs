//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/NetCommandWrapperList.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandWrapperList.cpp
//!
//! `NetCommandWrapperListNode`'s `m_next` chain is a `Vec` in the same order (newest node first, as
//! `processWrapper` pushes at the head).

use super::net_command_list::NetCommandList;
use super::net_command_msg::NetCommandMsg;
use super::net_command_ref::NetCommandRef;
use super::net_packet::NetPacket;

/// `class NetCommandWrapperListNode`: reassembly state for one wrapped command.
#[derive(Debug)]
pub struct NetCommandWrapperListNode {
    /// `m_commandID`.
    command_id: u16,
    /// `m_data` (`m_dataLength` bytes).
    data: Vec<u8>,
    /// `m_chunksPresent` (`m_numChunks` entries).
    chunks_present: Vec<bool>,
    /// `m_numChunks`.
    num_chunks: u32,
    /// `m_numChunksPresent`.
    num_chunks_present: u32,
}

impl NetCommandWrapperListNode {
    /// `NetCommandWrapperListNode::NetCommandWrapperListNode(NetWrapperCommandMsg *msg)`.
    pub fn new(msg: &NetCommandMsg) -> NetCommandWrapperListNode {
        let num_chunks = msg.get_num_chunks();
        NetCommandWrapperListNode {
            command_id: msg.get_wrapped_command_id(),
            data: vec![0u8; msg.get_total_data_length() as usize],
            chunks_present: vec![false; num_chunks as usize],
            num_chunks,
            num_chunks_present: 0,
        }
    }

    /// `isComplete`.
    pub fn is_complete(&self) -> bool {
        self.num_chunks_present == self.num_chunks
    }

    /// `getPercentComplete`: 100 when complete, else `min(99, REAL_TO_INT(present/chunks*100))`.
    pub fn get_percent_complete(&self) -> i32 {
        if self.is_complete() {
            100
        } else {
            99.min(((self.num_chunks_present as f32) / (self.num_chunks as f32) * 100.0f32) as i32)
        }
    }

    /// `getCommandID`.
    pub fn get_command_id(&self) -> u16 {
        self.command_id
    }

    /// `getRawDataLength`.
    pub fn get_raw_data_length(&self) -> u32 {
        self.data.len() as u32
    }

    /// `getRawData`.
    pub fn get_raw_data(&self) -> &[u8] {
        &self.data
    }

    /// `copyChunkData` (NetCommandWrapperList.cpp): copies the chunk's data to its offset unless the chunk number is
    /// out of range or the chunk was already received.
    ///
    /// Upstream's `memcpy` does not check `offset + dataLength` against `m_dataLength` (a malformed packet can
    /// write past the buffer); here an out-of-range chunk is skipped rather than copied past the end.
    pub fn copy_chunk_data(&mut self, msg: &NetCommandMsg) {
        if msg.get_chunk_number() >= self.num_chunks {
            // DEBUG_ASSERTCRASH(msg->getChunkNumber() < m_numChunks, ...)
            return;
        }
        let n = msg.get_chunk_number() as usize;
        if self.chunks_present[n] {
            // we already received this chunk, no need to recopy it.
            return;
        }
        let offset = msg.get_data_offset() as usize;
        let src = msg.get_wrapper_data().unwrap_or(&[]);
        let len = (msg.get_wrapper_data_length() as usize).min(src.len());
        if offset.checked_add(len).map_or(true, |end| end > self.data.len()) {
            return;
        }
        self.chunks_present[n] = true;
        self.data[offset..offset + len].copy_from_slice(&src[..len]);
        self.num_chunks_present += 1;
    }
}

/// `class NetCommandWrapperList`.
#[derive(Debug, Default)]
pub struct NetCommandWrapperList {
    /// `m_list` and its `m_next` chain, head first.
    list: Vec<NetCommandWrapperListNode>,
}

impl NetCommandWrapperList {
    /// `NetCommandWrapperList::NetCommandWrapperList()`.
    pub fn new() -> NetCommandWrapperList {
        NetCommandWrapperList::default()
    }

    /// `init`.
    pub fn init(&mut self) {
        self.list.clear();
    }

    /// `reset`.
    pub fn reset(&mut self) {
        self.list.clear();
    }

    /// `getPercentComplete(UnsignedShort wrappedCommandID)`: 0 when there is no such node.
    pub fn get_percent_complete(&self, wrapped_command_id: u16) -> i32 {
        match self.list.iter().find(|n| n.get_command_id() == wrapped_command_id) {
            Some(n) => n.get_percent_complete(),
            None => 0,
        }
    }

    /// `processWrapper` (NetCommandWrapperList.cpp): finds (or creates, at the head) the node for the wrapped command
    /// id and copies the chunk into it.
    pub fn process_wrapper(&mut self, r: &NetCommandRef) {
        let guard = r.get_command().read();
        let msg: &NetCommandMsg = &guard;
        let pos = self.list.iter().position(|n| n.get_command_id() == msg.get_wrapped_command_id());
        let idx = match pos {
            Some(i) => i,
            None => {
                self.list.insert(0, NetCommandWrapperListNode::new(msg));
                0
            }
        };
        self.list[idx].copy_chunk_data(msg);
    }

    /// `getReadyCommands` (NetCommandWrapperList.cpp): a new list holding every command whose chunks are all
    /// present, rebuilt with `NetPacket::ConstructNetCommandMsgFromRawData`; the completed nodes are removed.
    pub fn get_ready_commands(&mut self) -> NetCommandList {
        let mut retlist = NetCommandList::new();
        retlist.init();
        let mut i = 0;
        while i < self.list.len() {
            if self.list[i].is_complete() {
                let msg = NetPacket::construct_net_command_msg_from_raw_data(
                    self.list[i].get_raw_data(),
                    self.list[i].get_raw_data_length(),
                );
                if let Some(msg) = msg {
                    let ret = retlist.add_message(msg.get_command().clone());
                    if let Some(id) = ret {
                        if let Some(r) = retlist.get_mut(id) {
                            r.set_relay(msg.get_relay());
                        }
                    }
                }
                // removeFromList(temp)
                let id = self.list[i].get_command_id();
                self.remove_from_list(id);
                // (removal shifts the next node into slot i)
            } else {
                i += 1;
            }
        }
        retlist
    }

    /// `removeFromList`: removes the first node with the same command id.
    fn remove_from_list(&mut self, command_id: u16) {
        if let Some(p) = self.list.iter().position(|n| n.get_command_id() == command_id) {
            self.list.remove(p);
        }
    }
}
