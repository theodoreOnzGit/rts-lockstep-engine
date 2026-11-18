//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp (LANGameInfo::getConstLANSlot)

use crate::common::game_info::GameSlot;
use crate::drafts::network::lan_game_info::{LANGameInfo, LANSlotExtra};
use crate::drafts::network::net_command_msg::MAX_SLOTS;

impl LANGameInfo {
    /// `LANGameInfo::getConstLANSlot(slotNum)` (LANGameInfo.cpp:126): the slot and its extras, `NULL` (here `None`) for
    /// a slot number outside `0..MAX_SLOTS` (upstream also `DEBUG_ASSERTCRASH`es). The first-pass `get_lan_slot` is
    /// the mutable one.
    pub fn get_const_lan_slot(&self, slot_num: i32) -> Option<(&GameSlot, &LANSlotExtra)> {
        if slot_num < 0 || slot_num >= MAX_SLOTS {
            return None;
        }
        let i = slot_num as usize;
        Some((&self.info.slots[i], &self.extras[i]))
    }
}
