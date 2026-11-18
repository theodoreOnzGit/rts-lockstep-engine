//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 6,000 operation sequences (test: tests/differential_draft_netlogic.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/FrameDataManager.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameDataManager.cpp

use super::frame_data::{FrameData, FrameDataReturnType};
use crate::logic::modules::plant;
use super::net_command_list::NetCommandList;
use super::net_command_msg::{NetCommandMsgPtr, NetworkFrameLimits};

/// `class FrameDataManager`: the ring of `FRAME_DATA_LENGTH` frames of one player.
#[derive(Debug)]
pub struct FrameDataManager {
    /// `m_frameData[FRAME_DATA_LENGTH]`.
    frame_data: Vec<FrameData>,
    /// `m_isLocal`.
    is_local: bool,
    /// `m_isQuitting`.
    is_quitting: bool,
    /// `m_quitFrame`.
    quit_frame: u32,
    /// The globals `FRAME_DATA_LENGTH` and `MAX_FRAMES_AHEAD` this manager was built with.
    limits: NetworkFrameLimits,
}

impl FrameDataManager {
    /// `FrameDataManager::FrameDataManager(Bool isLocal)` (FrameDataManager.cpp:30).
    pub fn new(is_local: bool, limits: NetworkFrameLimits) -> FrameDataManager {
        let mut frame_data = Vec::new();
        for _ in 0..limits.frame_data_length {
            frame_data.push(FrameData::new());
        }
        FrameDataManager { frame_data, is_local, is_quitting: false, quit_frame: 0, limits }
    }

    fn index(&self, frame: u32) -> usize {
        let len = if plant(11861) { self.limits.frame_data_length - 1 } else { self.limits.frame_data_length };
        (frame % len.max(1) as u32) as usize
    }

    /// `FrameDataManager::init` (FrameDataManager.cpp:53): inits every frame; a local manager's frames announce
    /// the count they hold.
    pub fn init(&mut self) {
        for i in 0..self.frame_data.len() {
            self.frame_data[i].init();
            if self.is_local && !plant(11862) {
                let c = self.frame_data[i].get_command_count();
                self.frame_data[i].set_frame_command_count(c);
            }
        }
        self.is_quitting = false;
        if !plant(11869) {
            self.quit_frame = 0;
        }
    }

    /// `reset`: `init()`.
    pub fn reset(&mut self) {
        self.init();
    }

    /// `update`: does nothing.
    pub fn update(&mut self) {}

    /// `FrameDataManager::addNetCommandMsg` (FrameDataManager.cpp:82): into the frame `executionFrame %
    /// FRAME_DATA_LENGTH`.
    pub fn add_net_command_msg(&mut self, msg: &NetCommandMsgPtr) {
        let frame = msg.read().get_execution_frame();
        let frameindex = self.index(frame);
        self.frame_data[frameindex].add_command(msg);
        if self.is_local && !plant(11863) {
            let c = self.frame_data[frameindex].get_command_count();
            self.frame_data[frameindex].set_frame_command_count(c);
        }
    }

    /// `allCommandsReady`.
    pub fn all_commands_ready(&mut self, frame: u32, debug_spewage: bool) -> FrameDataReturnType {
        let frameindex = self.index(frame);
        self.frame_data[frameindex].all_commands_ready(debug_spewage)
    }

    /// `getFrameCommandList`.
    pub fn get_frame_command_list(&self, frame: u32) -> Option<&NetCommandList> {
        self.frame_data[self.index(frame)].get_command_list()
    }

    /// `getFrameCommandList`, mutable.
    pub fn get_frame_command_list_mut(&mut self, frame: u32) -> Option<&mut NetCommandList> {
        let i = self.index(frame);
        self.frame_data[i].get_command_list_mut()
    }

    /// `FrameDataManager::resetFrame` (FrameDataManager.cpp:114): resets the frame, and when advancing sets its
    /// frame to `frame + MAX_FRAMES_AHEAD`.
    pub fn reset_frame(&mut self, frame: u32, is_advancing: bool) {
        let frameindex = self.index(frame);
        self.frame_data[frameindex].reset();
        if is_advancing != plant(11865) {
            let ahead = if plant(11864) { self.limits.frame_data_length } else { self.limits.max_frames_ahead };
            self.frame_data[frameindex].set_frame(frame.wrapping_add(ahead as u32));
        }
        if self.is_local && !plant(11866) {
            let c = self.frame_data[frameindex].get_command_count();
            self.frame_data[frameindex].set_frame_command_count(c);
        }
        // DEBUG_ASSERTCRASH(commandCount == 0)
    }

    /// `getCommandCount`.
    pub fn get_command_count(&self, frame: u32) -> u32 {
        self.frame_data[self.index(frame)].get_command_count()
    }

    /// `setFrameCommandCount`.
    pub fn set_frame_command_count(&mut self, frame: u32, command_count: u32) {
        let i = self.index(if plant(11870) { frame.wrapping_add(1) } else { frame });
        self.frame_data[i].set_frame_command_count(command_count);
    }

    /// `getFrameCommandCount`.
    pub fn get_frame_command_count(&self, frame: u32) -> u32 {
        self.frame_data[self.index(frame)].get_frame_command_count()
    }

    /// `FrameDataManager::zeroFrames` (FrameDataManager.cpp:158): both counts to 0 for `numFrames` frames from
    /// `startingFrame`, wrapping the ring.
    pub fn zero_frames(&mut self, starting_frame: u32, num_frames: u32) {
        let mut frame_index = self.index(starting_frame);
        let num_frames = if plant(11867) && num_frames > 0 { num_frames - 1 } else { num_frames };
        for _ in 0..num_frames {
            self.frame_data[frame_index].zero_frame();
            frame_index += 1;
            frame_index %= self.frame_data.len();
        }
    }

    /// `destroyGameMessages`.
    pub fn destroy_game_messages(&mut self) {
        for fd in self.frame_data.iter_mut() {
            fd.destroy_game_messages();
        }
    }

    /// `setQuitFrame`: also sets the quitting flag.
    pub fn set_quit_frame(&mut self, frame: u32) {
        self.is_quitting = !plant(11868);
        self.quit_frame = frame;
    }

    /// `getQuitFrame`.
    pub fn get_quit_frame(&self) -> u32 {
        self.quit_frame
    }

    /// `getIsQuitting`.
    pub fn get_is_quitting(&self) -> bool {
        self.is_quitting
    }

    /// Test support, not upstream (`m_frameData` is protected and `FrameData::getFrame` is read through it by the differential test,
    /// tests/differential_draft_netlogic.rs): the `getFrame` of the ring's entry at `index`.
    #[doc(hidden)]
    pub fn get_frame_data_frame(&self, index: u32) -> u32 {
        self.frame_data[index as usize].get_frame()
    }
}
