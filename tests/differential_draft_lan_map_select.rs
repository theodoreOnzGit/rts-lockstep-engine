// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group lan6) of the draft `src/drafts/ui/lan_map_select_menu.rs` (the file's statics, `showLANGameOptionsUnderlyingGUIElements`,
//! `NullifyControls`, `LanMapSelectMenuInit`, `Shutdown`, `Update`, `Input`, `System`) against upstream `LanMapSelectMenu.cpp`, cut verbatim by build.rs (`drafts_lan6`) into the
//! program `LAN6_LAN_EXE` (differential/wrap_draft_lan6_lan.cpp and lan6_menu.h list the stand-ins), which also holds upstream's real window manager and gadgets (list box, radio
//! button, push button), `ShowUnderlyingGUIElements` (GUIUtil.cpp), `MapCache::findMap` (MapUtil.cpp) and `LANPreferences::usesSystemMapDir` (LanLobbyMenu.cpp), all verbatim.
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 3,000: one run is under a minute; fixed seed). A scene (tests/lan6_common): the menu's parent (5/6), 0 to 2 decoy top-level windows, the options
//! screen underneath (parent, the hidden gadgets, the per-player ones, its Back button), the list box, the preview with its eight start-position buttons, the two radio buttons
//! and Back / OK (each present 11/12, under the parent, top level or under a decoy, a decoy copy of a radio or button one time in three), the case's state (map cache entries,
//! the game's map, the preference, the maps that have a preview, the list's rows with their file names), then 4 to 22 operations: Init, Update, Shutdown, the map select layout
//! set or cleared, state changes, the system callback called directly (`GBM_SELECTED`, `GLM_SELECTED`, `GLM_DOUBLE_CLICKED` with a control, `CREATE`, `DESTROY`, `INPUT_FOCUS`,
//! others), the input callback (Escape in each state), the windows hidden, enabled, focused. After each operation both programs print the stand-in calls (map cache update,
//! populate, preview, start spots, game, preferences, post, shell), the messages, the dump of every window and layout and the statics; the texts must be identical. A case stops
//! after the first operation that destroys windows (Back, OK: the layout is gone).
//!
//! Stand-ins (logged on both sides): the map cache, `populateMapListbox` (verified on its own, tests/differential_map_list.rs), `positionStartSpots`, `getMapPreviewImage`,
//! the LAN game, the preferences' file, `PostToLanGameOptions`, `TheShell->shutdownComplete`. The Rust host's `show_underlying_gui_elements` is the test's own copy of
//! GUIUtil.cpp's function (the verified `client::gui_util` one needs the whole `GuiUtilHost`), so the C++ run checks the draft's argument lists, not that translation.
//!
//! Excluded inputs (upstream's undefined behaviour; the draft reports each with `set_undefined` and the operation is dropped on both sides): a missing start-position button, a NULL
//! control as `mData1`, `winMapPreview` NULL on `GLM_SELECTED`, `mapSelectLayout` NULL (or already deleted) on Back / OK, `Shutdown` on a deleted layout; `GWM_INPUT_FOCUS`
//! sent directly with `mData1` TRUE (it writes through a pointer).
//!
//! # Prediction (before the first run)
//!
//! Draft defects: (1) `asciiMap.translate(text)` keeps a unit whose low byte is 0 (upstream appends nothing); (2) `populate_map_listbox` was handed neither `isMultiplayer` nor
//! `mapToSelect`; (3) `GadgetListBoxGetText(NULL)` / `GetItemData(NULL)` were guarded as undefined. All three were fixed on reading. Expected after that: 0 mismatches, except what
//! my generator or stand-ins get wrong.
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!
//! | id | planted bug |
//! |---|---|
//! | 23861 | Init takes the user maps directory when the map is official (the cache's flag negated) |
//! | 23862 | Init selects the user radio button when the preference says system maps |
//! | 23863 | Init does not call `updateCache` before populating |
//! | 23864 | the system radio button writes `no` |
//! | 23865 | the user radio button populates the system maps |
//! | 23866 | Escape acts on the key-down |
//! | 23867 | OK looks the map up without lower-casing it |
//! | 23868 | OK does not reset the start spots |
//! | 23869 | Back posts SEND_GAME_OPTS |
//! | 23870 | `GLM_SELECTED` does not clear the preview's image status when there is no image |
//! | 23871 | `GLM_SELECTED` with a negative row positions the start spots of the old map |
//! | 23872 | Shutdown does not hide the layout |
//!
//! # Results
//!
//! 2026-11-22, `DRAFT_CASES=3000` (the default, about 22 s a run), max ULP 0: **0 mismatches over 3,000 sequences, 204,086 outputs compared.** The prediction held (the three draft
//! defects were fixed on reading, before the first run); reverting defect (1) alone gives 2 mismatches (cases 598 and 1673: `preview map 61,0,62` against `61,62`), so the
//! test reaches it. Planted bugs (mismatches of 3,000 sequences): 23861 468; 23862 1769; 23863 1439; 23864 619; 23865 670; 23866 174; 23867 8; 23868 26; 23869 154; 23870 524;
//! 23871 27; 23872 592. **12 of 12 caught** (an earlier 23867 missed at 0 until the generator got the composite "Init, pick a row, OK" sequences).

mod lan6_common;
mod support;
mod ui4_common;

use lan6_common::*;
use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{GameFont, WinId};
use rts_lockstep_engine::drafts::ui::lan_map_select_menu::*;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"LanMapSelectMenuSystem";
const INP: &[u8] = b"LanMapSelectMenuInput";

#[derive(Default)]
struct Host {
    m: MapHost,
    shell: Shell,
    ms: LanMapSelect,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.m.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            lan_map_select_menu_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            lan_map_select_menu_input(wm, self, win, msg, d1, d2)
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

impl LanMapSelectHost for Host {
    fn lan_map_select(&mut self) -> &mut LanMapSelect {
        &mut self.ms
    }
    fn lan_prefs_uses_system_map_dir(&mut self) -> bool {
        self.m.uses_system_lan()
    }
    fn lan_prefs_write_use_system_map_dir(&mut self, yes: bool) {
        self.m.ini.insert("UseSystemMapDir".into(), if yes { "yes" } else { "no" }.into());
        self.m.prefs_write();
    }
    fn lan_my_game_map(&self) -> Vec<u8> {
        self.m.game_map.clone()
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
    fn lan_game_set_map(&mut self, map: &[u8]) {
        self.m.game_set_map(map);
    }
    fn lan_game_slot0_set_map_availability(&mut self, available: bool) {
        self.m.slot0_avail(available);
    }
    fn lan_game_set_map_crc_and_size(&mut self, crc: u32, size: u32) {
        self.m.crc_size(crc, size);
    }
    fn lan_game_reset_start_spots(&mut self) {
        self.m.logln("game reset spots");
    }
    fn lan_game_adjust_slots_for_map(&mut self) {
        self.m.logln("game adjust slots");
    }
    fn post_to_lan_game_options(&mut self, t: PostToLanGameOptionsType) {
        let _ = writeln!(self.m.log, "post {}", if t == PostToLanGameOptionsType::MapBack { 0 } else { 1 });
    }
}

impl LanUnderlyingGui for Host {
    fn show_underlying_gui_elements(&mut self, wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]) {
        self.m.show_underlying(wm, show, layout_filename, parent_name, gadgets_to_hide, per_player_gadgets_to_hide);
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
                let r = if v[1] == "sys" { lan_map_select_menu_system(wm, self, win, msg, a, d2) } else { lan_map_select_menu_input(wm, self, win, msg, a, d2) };
                let _ = writeln!(self.m.log, "ret {}", u8::from(r == MsgHandled::Handled));
            }
            "ms" => match v[2] {
                "init" => lan_map_select_menu_init(wm, self, self.m.layout.unwrap_or(0)),
                "update" => lan_map_select_menu_update(wm, self, self.m.layout.unwrap_or(0)),
                "shutdown" => {
                    // (the pending push is this layout's own, as group ui5's program models it)
                    self.shell.pending_push = true;
                    self.shell.pending_push_name = b"pushed".to_vec();
                    let l = self.m.layout.expect("a layout");
                    lan_map_select_menu_shutdown(wm, self, l);
                }
                "msl" => {
                    let l = if num(3) != 0 { self.m.layout } else { None };
                    self.ms.map_select_layout = l;
                }
                other => panic!("ms {other}"),
            },
            other => panic!("@ {other}"),
        }
    }
    fn statics(&self, wm: &GameWindowManager) -> String {
        let m = &self.ms;
        let mut o = String::new();
        let _ = write!(o, "ms ids {} {} {} {} {} {} wins {} {} {} sp", m.button_back, m.button_ok, m.listbox_map, m.win_map_preview_id, m.radio_button_system_maps_id, m.radio_button_user_maps_id, idx(m.parent), idx(m.map_list), idx(m.win_map_preview));
        for b in &m.button_map_start_position {
            let _ = write!(o, " {}", idx(*b));
        }
        o.push_str(" spid");
        for i in &m.button_map_start_position_id {
            let _ = write!(o, " {i}");
        }
        let l = m.map_select_layout.filter(|&l| !wm.layouts[l].deleted).map_or(-1, |l| l as i64);
        let _ = writeln!(o, " msl {l}");
        o
    }
}

#[test]
fn lan_map_select_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let cfg = Cfg { file: "LanMapSelectMenu.wnd", parent_name: "LanMapSelectMenuParent", under_file: "LanGameOptionsMenu.wnd", under_parent: "LanGameOptionsMenuParent", wol: false };
    let mut g = Gen::new(0x215_1A_06);
    let mut chk = Check::new("LanMapSelectMenu");
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
        run_case(env!("LAN6_LAN_EXE"), &mut chk, case, ops, h, &menu_funcs);
    }
    finish(chk);
}
