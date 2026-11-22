// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group lan6) of the draft `src/drafts/ui/wol_map_select_menu.rs` (the file's statics, `NullifyControls`, `showGameSpyGameOptionsUnderlyingGUIElements`,
//! `WOLMapSelectMenuInit`, `Shutdown`, `Update`, `Input`, `System`) against upstream `WOLMapSelectMenu.cpp`, cut verbatim by build.rs (`drafts_lan6`) into the program `LAN6_WOL_EXE`
//! (differential/wrap_draft_lan6_wol.cpp and lan6_menu.h list the stand-ins), which also holds upstream's real window manager and gadgets, `ShowUnderlyingGUIElements` (GUIUtil.cpp),
//! `MapCache::findMap` (MapUtil.cpp) and `CustomMatchPreferences::usesSystemMapDir` / `setUsesSystemMapDir` (UserPreferences.cpp), all verbatim.
//!
//! # Method
//!
//! As tests/differential_draft_lan_map_select.rs (same scene generator, tests/lan6_common), with `DRAFT_CASES` 3,000 (default; one run is about 25 s, fixed seed), plus the stand-ins of the
//! GameSpy game: `getUseStats()` (the case's `@ ms stats`, which also disables the user maps' radio button in Init), `setGameOptions`, `resetAccepted`, `WOLDisplaySlotList`,
//! `WOLDisplayGameOptions`, `WOLPositionStartSpots`, `RaiseGSMessageBox` (each logged on both sides). The preference is `CustomMatchPreferences`: absent or "1" is system maps, and
//! the radio buttons write "1" / "0". Same exclusions as the LAN test, plus: stats on and no user-maps radio button (`winEnable` through NULL), and OK with no list
//! (`selected` is read uninitialised).
//!
//! # Prediction (before the first run)
//!
//! The draft defects found on reading were fixed first (the translate of a unit with a zero low byte, `populateMapListbox`'s arguments, the NULL-safe list reads). Expected: 0 mismatches.
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!
//! | id | planted bug |
//! |---|---|
//! | 23873 | Init does not force the system maps when the room records stats |
//! | 23874 | Init does not disable the user maps' radio button when stats are used |
//! | 23875 | Init takes the cache's official flag negated |
//! | 23876 | the system radio button writes "0" |
//! | 23877 | OK does not reset the accepted flags |
//! | 23878 | OK does not send the game options |
//! | 23879 | Back destroys the layout before showing the underlying screen's gadgets, and does not reposition the start spots |
//! | 23880 | Update does not clear `raiseMessageBoxes` |
//! | 23881 | Shutdown does not hide the layout |
//! | 23882 | OK looks the map up without lower-casing it |
//! | 23883 | `GLM_SELECTED` does not clear the preview's image status when there is no image |
//! | 23884 | Escape acts on the key-down |
//!
//! # Results
//!
//! 2026-11-22, `DRAFT_CASES=3000` (the default, about 25 s a run), max ULP 0: **0 mismatches over 3,000 sequences, 202,824 outputs compared.** The prediction held. Misses of mine, all
//! in the test, none in the draft: a copy-paste left the scene's names in LAN spelling (every Init then found nothing: 0 mismatches that meant nothing, and plants 23873-23880 at 0, which
//! is how I saw it); plant 23876 was written inverted (an unplanted 875 mismatches); the case did not stop when `Back` / `OK` deleted the layout (the C++ then crashed on freed
//! windows). Planted bugs (mismatches of 3,000 sequences): 23873 494; 23874 858; 23875 216; 23876 861; 23877 177; 23878 177; 23879 145; 23880 124; 23881 538; 23882 12; 23883 524;
//! 23884 154. **12 of 12 caught.**

mod lan6_common;
mod support;
mod ui4_common;

use lan6_common::*;
use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{GameFont, WinId};
use rts_lockstep_engine::drafts::ui::wol_map_select_menu::*;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"WolMapSelectMenuSystem";
const INP: &[u8] = b"WolMapSelectMenuInput";

#[derive(Default)]
struct Host {
    m: MapHost,
    shell: Shell,
    ms: WolMapSelect,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.m.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            wol_map_select_menu_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            wol_map_select_menu_input(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.m.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn block_input_left_up(&mut self) {
        self.m.log.push_str("block\n");
    }
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        assert_eq!(filename, b"pushed");
        let l = self.m.layout.expect("a layout");
        let _ = writeln!(self.m.log, "shell complete {l}");
        Some(l)
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _layout: usize, _user: Option<bool>) {}
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.m.keys.key(name)
    }
}

impl WolMapSelectHost for Host {
    fn wol_map_select(&mut self) -> &mut WolMapSelect {
        &mut self.ms
    }
    fn custom_prefs_uses_system_map_dir(&mut self) -> bool {
        self.m.uses_system_custom()
    }
    fn custom_prefs_set_uses_system_map_dir_and_write(&mut self, v: bool) {
        self.m.ini.insert("UseSystemMapDir".into(), if v { "1" } else { "0" }.into());
        self.m.prefs_write();
    }
    fn game_spy_game_map(&self) -> Vec<u8> {
        self.m.game_map.clone()
    }
    fn current_staging_room_use_stats(&self) -> bool {
        self.m.stats
    }
    fn map_cache_is_official(&self, map: &[u8]) -> Option<bool> {
        self.m.find_map(map).map(|m| m.0)
    }
    fn map_cache_find(&self, map: &[u8]) -> Option<(u32, u32)> {
        self.m.cache.get(map).map(|m| (m.1, m.2))
    }
    fn map_cache_update(&mut self) {
        self.m.logln("mapcache update");
    }
    fn populate_map_listbox(&mut self, _wm: &mut GameWindowManager, list: Option<WinId>, system: bool, is_multiplayer: bool, map_to_select: &[u8]) {
        self.m.populate(list, system, is_multiplayer, map_to_select);
    }
    fn map_list_name(&self, item_data: u32) -> Option<Vec<u8>> {
        self.m.item_name(item_data)
    }
    fn get_map_preview_image(&mut self, map: &[u8]) -> Option<Vec<u8>> {
        self.m.preview_image(map)
    }
    fn set_preview_user_data_map(&mut self, _wm: &mut GameWindowManager, preview: WinId, map: &[u8]) {
        self.m.set_user(preview, map);
    }
    fn win_set_enabled_image0(&mut self, _wm: &mut GameWindowManager, w: WinId, image: &[u8]) {
        self.m.images.insert(w, image.to_vec());
    }
    fn position_start_spots(&mut self, _wm: &mut GameWindowManager, map: &[u8], buttons: &[Option<WinId>; MAX_SLOTS], map_window: Option<WinId>) {
        self.m.spots(map, buttons, map_window);
    }
    fn show_underlying_gui_elements(&mut self, wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]) {
        self.m.show_underlying(wm, show, layout_filename, parent_name, gadgets_to_hide, per_player_gadgets_to_hide);
    }
    fn game_spy_game_set_map(&mut self, map: &[u8]) {
        self.m.game_set_map(map);
    }
    fn game_spy_game_slot0_set_map_availability(&mut self, v: bool) {
        self.m.slot0_avail(v);
    }
    fn game_spy_game_set_map_crc_and_size(&mut self, crc: u32, size: u32) {
        self.m.crc_size(crc, size);
    }
    fn game_spy_game_adjust_slots_for_map(&mut self) {
        self.m.logln("game adjust slots");
    }
    fn game_spy_game_reset_accepted(&mut self) {
        self.m.logln("game reset accepted");
    }
    fn game_spy_game_reset_start_spots(&mut self) {
        self.m.logln("game reset spots");
    }
    fn game_spy_info_set_game_options(&mut self) {
        self.m.logln("gsi setgameoptions");
    }
    fn wol_display_slot_list(&mut self, _wm: &mut GameWindowManager) {
        self.m.logln("wol slotlist");
    }
    fn wol_display_game_options(&mut self, _wm: &mut GameWindowManager) {
        self.m.logln("wol gameoptions");
    }
    fn wol_position_start_spots(&mut self, _wm: &mut GameWindowManager) {
        self.m.logln("wol positionspots");
    }
    fn raise_gs_message_box(&mut self) {
        self.m.logln("wol raisebox");
    }
}

impl MapMenu for Host {
    fn mh(&mut self) -> &mut MapHost {
        &mut self.m
    }
    fn menu_op(&mut self, wm: &mut GameWindowManager, op: &str) {
        let v: Vec<&str> = op.split(' ').collect();
        let num = |i: usize| -> i64 { v[i].parse().expect("number") };
        match v[1] {
            "sys" | "inp" => {
                let (win, msg, kind, d1, d2) = (num(3) as usize, num(4) as u32, num(5), num(6) as u32, num(7) as u32);
                let a = if kind == 1 { win_data(Some(d1 as usize)) } else { d1 };
                let r = if v[1] == "sys" { wol_map_select_menu_system(wm, self, win, msg, a, d2) } else { wol_map_select_menu_input(wm, self, win, msg, a, d2) };
                let _ = writeln!(self.m.log, "ret {}", u8::from(r == MsgHandled::Handled));
            }
            "ms" => match v[2] {
                "init" => wol_map_select_menu_init(wm, self, self.m.layout.unwrap_or(0)),
                "update" => wol_map_select_menu_update(wm, self, self.m.layout.unwrap_or(0)),
                "shutdown" => {
                    // (the pending push is this layout's own, as group ui5's program models it)
                    self.shell.pending_push = true;
                    self.shell.pending_push_name = b"pushed".to_vec();
                    let l = self.m.layout.expect("a layout");
                    wol_map_select_menu_shutdown(wm, self, l);
                }
                "msl" => {
                    let l = if num(3) != 0 { self.m.layout } else { None };
                    self.ms.wol_map_select_layout = l;
                }
                other => panic!("ms {other}"),
            },
            other => panic!("@ {other}"),
        }
    }
    fn statics(&self, wm: &GameWindowManager) -> String {
        let m = &self.ms;
        let mut o = String::new();
        let _ = write!(o, "ms ids {} {} {} {} {} {} wins {} {} {} {} sp", m.button_back, m.button_ok, m.listbox_map, m.win_map_preview_id, m.radio_button_system_maps_id, m.radio_button_user_maps_id, idx(m.parent), idx(m.map_list), idx(m.win_map_preview), idx(m.win_map_window));
        for b in &m.button_map_start_position {
            let _ = write!(o, " {}", idx(*b));
        }
        o.push_str(" spid");
        for i in &m.button_map_start_position_id {
            let _ = write!(o, " {i}");
        }
        let l = m.wol_map_select_layout.filter(|&l| !wm.layouts[l].deleted).map_or(-1, |l| l as i64);
        let _ = writeln!(o, " raise {} msl {l}", u8::from(m.raise_message_boxes));
        o
    }
}

#[test]
fn wol_map_select_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let cfg = Cfg { file: "WOLMapSelectMenu.wnd", parent_name: "WOLMapSelectMenuParent", under_file: "GameSpyGameOptionsMenu.wnd", under_parent: "GameSpyGameOptionsMenuParent", wol: true };
    let mut g = Gen::new(0x215_1A_07);
    let mut chk = Check::new("WOLMapSelectMenu");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    let _ = lbx::get_num_entries;
    for case in 0..cases(3_000) as u64 {
        let scene = gen_scene(&mut g, &cfg);
        let steps = 4 + g.below(19);
        let mut ops = scene.ops.clone();
        for _ in 0..steps {
            let o = gen_op(&mut g, &scene);
            ops.extend(o.split('\n').map(str::to_string));
        }
        let h = Host { m: MapHost::new(), ..Default::default() };
        run_case(env!("LAN6_WOL_EXE"), &mut chk, case, ops, h, &menu_funcs);
    }
    finish(chk);
}
