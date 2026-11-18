//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/FrameData.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameData.cpp

use super::net_command_list::NetCommandList;
use super::net_command_msg::NetCommandMsgPtr;

/// `enum FrameDataReturnType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameDataReturnType {
    /// `FRAMEDATA_NOTREADY`.
    NotReady,
    /// `FRAMEDATA_RESEND`.
    Resend,
    /// `FRAMEDATA_READY`.
    Ready,
}

/// `class FrameData`: the commands of one frame.
#[derive(Debug)]
pub struct FrameData {
    /// `m_frame`.
    frame: u32,
    /// `m_frameCommandCount` (-1, i.e. `u32::MAX`, until set).
    frame_command_count: u32,
    /// `m_commandCount`.
    command_count: u32,
    /// `m_commandList` (null until `init`).
    command_list: Option<NetCommandList>,
    /// `m_lastFailedCC`.
    last_failed_cc: u32,
    /// `m_lastFailedFrameCC`.
    last_failed_frame_cc: u32,
}

impl Default for FrameData {
    fn default() -> Self {
        FrameData::new()
    }
}

impl FrameData {
    /// `FrameData::FrameData()` (FrameData.cpp:30).
    pub fn new() -> FrameData {
        FrameData {
            frame: 0,
            command_list: None,
            command_count: 0,
            frame_command_count: u32::MAX,
            last_failed_cc: 0,
            last_failed_frame_cc: 0,
        }
    }

    /// `FrameData::init` (FrameData.cpp:55): creates the list when there is none, then resets it.
    pub fn init(&mut self) {
        self.frame = 0;
        if self.command_list.is_none() {
            let mut l = NetCommandList::new();
            l.init();
            self.command_list = Some(l);
        }
        if let Some(l) = self.command_list.as_mut() {
            l.reset();
        }
        self.frame_command_count = u32::MAX;
        self.command_count = 0;
        self.last_failed_cc = u32::MAX - 1; // -2
        self.last_failed_frame_cc = u32::MAX - 1; // -2
    }

    /// `FrameData::reset` (FrameData.cpp:73): `init()`.
    pub fn reset(&mut self) {
        self.init();
    }

    /// `FrameData::update` (FrameData.cpp:80): does nothing.
    pub fn update(&mut self) {}

    /// `getFrame`.
    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// `setFrame`.
    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }

    /// `FrameData::allCommandsReady` (FrameData.cpp:101): ready when the announced command count equals the
    /// received count; too many received is a resend (the frame is reset); fewer is not ready yet.
    pub fn all_commands_ready(&mut self, debug_spewage: bool) -> FrameDataReturnType {
        if self.frame_command_count == self.command_count {
            self.last_failed_frame_cc = u32::MAX - 1;
            self.last_failed_cc = u32::MAX - 1;
            return FrameDataReturnType::Ready;
        }
        if debug_spewage
            && (self.last_failed_frame_cc != self.frame_command_count || self.last_failed_cc != self.command_count)
        {
            // DEBUG_LOG of the counts
            self.last_failed_frame_cc = self.frame_command_count;
            self.last_failed_cc = self.command_count;
        }
        if self.command_count > self.frame_command_count {
            // (upstream walks the list here to log each message)
            self.reset();
            return FrameDataReturnType::Resend;
        }
        FrameDataReturnType::NotReady
    }

    /// `setFrameCommandCount` (FrameData.cpp:150).
    pub fn set_frame_command_count(&mut self, frame_command_count: u32) {
        self.frame_command_count = frame_command_count;
    }

    /// `getFrameCommandCount`.
    pub fn get_frame_command_count(&self) -> u32 {
        self.frame_command_count
    }

    /// `getCommandCount`.
    pub fn get_command_count(&self) -> u32 {
        self.command_count
    }

    /// `FrameData::addCommand` (FrameData.cpp:171): a command already on the list is ignored.
    pub fn add_command(&mut self, msg: &NetCommandMsgPtr) {
        if self.command_list.is_none() {
            self.init();
        }
        if let Some(l) = self.command_list.as_mut() {
            if l.find_message(msg).is_some() {
                return;
            }
            l.add_message(msg.clone());
        }
        self.command_count = self.command_count.wrapping_add(1);
    }

    /// `getCommandList`.
    pub fn get_command_list(&self) -> Option<&NetCommandList> {
        self.command_list.as_ref()
    }

    /// `getCommandList`, mutable (the caller walks and edits the list through the pointer upstream).
    pub fn get_command_list_mut(&mut self) -> Option<&mut NetCommandList> {
        self.command_list.as_mut()
    }

    /// `FrameData::zeroFrame` (FrameData.cpp:195).
    pub fn zero_frame(&mut self) {
        self.command_count = 0;
        self.frame_command_count = 0;
    }

    /// `FrameData::destroyGameMessages` (FrameData.cpp:203).
    pub fn destroy_game_messages(&mut self) {
        if let Some(l) = self.command_list.as_mut() {
            l.reset();
            self.command_count = 0;
        }
    }
}
