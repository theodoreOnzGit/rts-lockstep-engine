//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on slot numbers -3 to 12 (test: tests/differential_draft_glsmall.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp (LANGameInfo::getConstLANSlot)

use crate::common::game_info::GameSlot;
use crate::drafts::network::lan_game_info::{LANGameInfo, LANSlotExtra};
use crate::drafts::network::net_command_msg::MAX_SLOTS;
use crate::logic::modules::plant;

impl LANGameInfo {
    /// `LANGameInfo::getConstLANSlot(slotNum)` (LANGameInfo.cpp:126): the slot and its extras, `NULL` (here `None`) for
    /// a slot number outside `0..MAX_SLOTS` (upstream also `DEBUG_ASSERTCRASH`es). The first-pass `get_lan_slot` is
    /// the mutable one.
    pub fn get_const_lan_slot(&self, slot_num: i32) -> Option<(&GameSlot, &LANSlotExtra)> {
        if (if plant(15889) { slot_num <= 0 } else { slot_num < 0 }) || slot_num >= MAX_SLOTS {
            return None;
        }
        let i = if plant(15890) && slot_num > 1 { slot_num as usize - 1 } else { slot_num as usize };
        Some((&self.info.slots[i], &self.extras[i]))
    }
}
