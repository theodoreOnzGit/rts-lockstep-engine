//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPICallbacks.cpp (LANAPI::OnSlotList, LANAPI::OnInActive)
//!
//! Sweep 2 finding: the other callbacks of that file are the [`LanApiHost`] methods of the first-pass draft
//! (`crate::drafts::network::lan_api`); these two were not.

use crate::drafts::network::lan_api::{LanApiHost, ReturnType, LANAPI};
use crate::drafts::network::net_command_msg::MAX_SLOTS;

impl<H: LanApiHost> LANAPI<H> {
    /// `LANAPI::OnSlotList(ret, theGame)` (LANAPICallbacks.cpp:472): for the game the player is in only. If none of
    /// the game's slots has the local IP the player was removed: `OnPlayerLeave(m_name)`; else the slot list is redrawn
    /// (`lanUpdateSlotList()`). `the_game` is the game's place in `games`.
    pub fn on_slot_list(&mut self, _ret: ReturnType, the_game: Option<usize>) {
        let Some(g) = the_game else { return };
        if self.current_game != Some(g) {
            return;
        }
        let mut found_me = false;
        for player in 0..(MAX_SLOTS as usize) {
            if self.games[g].get_ip(player) == self.local_ip {
                found_me = true;
                break;
            }
        }
        if !found_me {
            let name = self.name.clone();
            self.host.on_player_leave(&name);
            return;
        }
        self.host.lan_update_slot_list();
    }

    /// `LANAPI::OnInActive(IP)` (LANAPICallbacks.cpp:669): the body is empty upstream.
    pub fn on_in_active(&mut self, _ip: u32) {}
}
