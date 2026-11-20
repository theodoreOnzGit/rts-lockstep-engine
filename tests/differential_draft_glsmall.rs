#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp (GameInfo::setSuperweaponRestriction)
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp (LANGameInfo::getConstLANSlot)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp (GameWindowManager::winFormatText, winGetTextSize)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (GameWindowManager::freeStaticStrings)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The four small drafts `game_info`, `lan_game_info` and `game_window_global` against upstream's (#215).
//!
//! # Method
//!
//! The four functions are cut verbatim (build.rs, `drafts_guilogic4`) into `differential/wrap_draft_glsmall.cpp`, with stand-ins for the classes
//! around them (listed in its header). `setSuperweaponRestriction`: every 16-bit value that matters (all 65,536) set on a `GameInfo` whose field held
//! 0x1234, the field read back from the verified `GameInfo`. `getConstLANSlot`: slot numbers -3 to 12, the slot returned (by its position in the
//! array, with the draft's `LANGameInfo` built through a minimal host) or none. `winGetTextSize`: the output pointers given or NULL (the draft returns
//! both outputs; with a NULL one the C++ leaves that sentinel); `winFormatText` has an empty body in both (called, nothing to compare). `freeStaticStrings`:
//! the four strings' lengths afterwards.
//!
//! These functions are one assignment, one range check, two zero stores and four `clear()`s: the test is small because the functions are. The
//! `LANAPI::OnSlotList` of `lan_api_callbacks.rs` is NOT tested: it is inside a `/* ... */` comment in LANAPICallbacks.cpp (lines 471-496; its only
//! caller, LANAPI.cpp:1032, is commented out too), so no compiled C++ exists to compare with (see the banner of that file, corrected in this change).
//! `LANAPI::OnInActive` has an empty body upstream and in the draft (nothing to compare).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: 65,536 + 16 + 4 + 1 comparisons, **0 mismatches** (prediction met). Planted bugs, 5 of 5 caught (mismatches): 15889 1 of 16; 15890 6 of 16;
//! 15891 65,280 of 65,536; 15892 2 of 4; 15893 1 of 1.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15889 `getConstLANSlot` accepts slot `MAX_SLOTS` (an off-by-one: the C++ returns NULL); 15890 `getConstLANSlot` returns the slot
//! before the one asked for; 15891 `setSuperweaponRestriction` keeps the low byte only; 15892 `winGetTextSize` gives 1 for the height;
//! 15893 `freeStaticStrings` leaves the tooltip string.

mod support;

use rts_lockstep_engine::common::game_info::{GameInfo, GameInfoHost, MapMeta};
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::drafts::network::lan_game_info::{LANGameInfo, LanGameInfoHost};
use rts_lockstep_engine::drafts::sweep_gui_logic::{game_info as gi, game_window_global as gwg};
use std::sync::Mutex;
use support::Check;

extern "C" {
    fn ref_gs_super(restriction: i32) -> i32;
    fn ref_gs_lan(slot: i32) -> i32;
    fn ref_gs_text(have_w: i32, have_h: i32) -> i32;
    fn ref_gs_free() -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(checks: Vec<Check>) {
    if planted() {
        for c in checks {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in checks {
        c.finish();
    }
}

/// A host with nothing to say: `GameInfo::new` and `LANGameInfo::new` read little of it.
struct Host;
impl GameInfoHost for Host {
    fn game_text(&mut self, _label: &str) -> Vec<u16> {
        Vec::new()
    }
    fn tick_count(&mut self) -> u32 {
        0
    }
    fn default_starting_cash(&self) -> Money {
        Money::default()
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        false
    }
    fn map_cache_find(&self, _lower_name: &[u8]) -> Option<MapMeta> {
        None
    }
    fn file_exists(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn has_multiplayer_settings(&self) -> bool {
        false
    }
    fn num_colors(&self) -> i32 {
        0
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0)
    }
    fn player_template_count(&self) -> i32 {
        0
    }
    fn player_template_display_name(&self, _n: i32) -> Result<Vec<u16>, IniError> {
        Ok(Vec::new())
    }
    fn record_income(&mut self, _player_index: i32) {}
}
impl LanGameInfoHost for Host {
    fn has_lan(&self) -> bool {
        true
    }
    fn lan_local_ip(&self) -> u32 {
        0
    }
    fn lan_reset_game_start_timer(&mut self) {}
    fn lan_my_game_is_this(&self) -> bool {
        false
    }
    fn lan_am_i_host(&self) -> bool {
        false
    }
    fn lan_enable_start_button(&mut self, _enabled: bool) {}
    fn lan_request_has_map(&mut self) {}
    fn lan_update_slot_list(&mut self) {}
    fn lan_update_game_options(&mut self) {}
    fn time_get_time(&self) -> u32 {
        0
    }
}

#[test]
fn small_drafts() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut host = Host;
    let mut sup = Check::new("GameInfo::setSuperweaponRestriction");
    let mut game = GameInfo::new(&mut host);
    for v in 0..=65535u32 {
        game.superweapon_restriction = 0x1234;
        gi::set_superweapon_restriction(&mut game, v as u16);
        sup.eq(game.superweapon_restriction as i32, unsafe { ref_gs_super(v as i32) }, || format!("restriction {v}"));
    }
    let mut lan = Check::new("LANGameInfo::getConstLANSlot");
    let mut g = LANGameInfo::new(&mut host);
    for (i, s) in g.info.slots.iter_mut().enumerate() {
        s.color = i as i32; // each slot knows its own index
    }
    for slot in -3..13 {
        let r = g.get_const_lan_slot(slot).map_or(-1, |(s, _)| s.color);
        lan.eq(r, unsafe { ref_gs_lan(slot) }, || format!("slot {slot}"));
    }
    let mut text = Check::new("GameWindowManager::winGetTextSize");
    for (hw, hh) in [(1, 1), (1, 0), (0, 1), (0, 0)] {
        let (w, h) = gwg::win_get_text_size(None, &[], 9);
        gwg::win_format_text(None, &[], 1, 2, 3, 4, 5);
        let (rw, rh) = (if hw != 0 { w } else { 0x7777 }, if hh != 0 { h } else { 0x7777 });
        text.eq(rw * 100000 + rh, unsafe { ref_gs_text(hw, hh) }, || format!("outputs {hw} {hh}"));
    }
    let mut free = Check::new("GameWindowManager::freeStaticStrings");
    let mut s = gwg::ScriptStaticStrings { system_string: b"abc".to_vec(), input_string: b"abc".to_vec(), tooltip_string: b"abc".to_vec(), draw_string: b"abc".to_vec() };
    s.free_static_strings();
    let rust = (s.system_string.len() + 10 * s.input_string.len() + 100 * s.tooltip_string.len() + 1000 * s.draw_string.len()) as i32;
    free.eq(rust, unsafe { ref_gs_free() }, || "lengths".to_string());
    finish(vec![sup, lan, text, free]);
}
