// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui5) of the draft `src/drafts/ui/popup_host_game.rs` (the file's statics, `CustomMatchHideHostPopup`, `HandleCustomLadderSelection`,
//! `PopulateCustomLadderListBox`, `PopulateCustomLadderComboBox`, `PopupHostGameInit`, `Update`, `Input`, `System`, `createGame`) against upstream `PopupHostGame.cpp`, cut verbatim by
//! build.rs (`drafts_ui5`) into the program `UI5_HOST_EXE` (differential/wrap_draft_ui5_host.cpp lists the stand-ins), which also holds upstream's real window manager and gadgets
//! (text entry, check box, combo box, list box, push button).
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 3,000: one run is under a minute; fixed seed). Each is a scene of named windows (`PopupHostGame.wnd:...`): the popup's parent (menu
//! callbacks), 0 to 2 decoy top-level windows, the four text entries, the two buttons and the combo box each present with probability 5/6 (the three check boxes always: upstream
//! dereferences them), under the parent (70 %), top level (15 %) or under a decoy (15 %), a decoy copy of each name one time in three, and a list box for `PopulateCustomLadderListBox`.
//! The case's state: 0 to 6 ladders (special, standard, local; indices positive or, for local ones, negative; valid for custom play or not; addresses shared so that a lookup can
//! find a different ladder than the one named), the recent ladders, the saved preferences (last ladder, observers, limited, stats). 4 to 22 operations: Init, Update, hide,
//! `HandleCustomLadderSelection`, the two populate functions, the system callback called directly (`GBM_SELECTED` / `GCM_SELECTED` / `GEM_UPDATE_TEXT` with a control, `CREATE`,
//! `DESTROY`, others), the input callback (Escape in each state), presses and releases at the buttons and check boxes, typed characters, the texts set (white space, a unit 0x100),
//! the combo's selection, the ladder state changed, hiding, enabling, focusing. After each operation both programs print the stand-in calls (preferences, GameSpy, the peer
//! request), the messages, the dump of every window, the statics and the preferences; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour; the draft reports each through `PopupHostGameHost::undefined`): a missing check box (`GadgetCheckBoxSetChecked` /
//! `IsChecked` dereference it); a NULL control as `mData1`; `GWM_INPUT_FOCUS` sent directly with `mData1` TRUE; Unicode spaces beyond ASCII (`iswspace` of MSVC is not modelled).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! Draft defects I expect from reading upstream against the draft: (1) `PopupHostGameInit` sets the game name from the local name BEFORE the text entry lookup order? no: same order;
//! the one I expect is `GEM_UPDATE_TEXT` (the draft strips leading white space only, as upstream, but `trim`'s second use is not there either) and `createGame`'s password through
//! `c as u8` (a unit whose low byte is 0 is kept as a NUL, upstream's `translate` appends nothing). Nothing else.
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!
//! | id | planted bug |
//! |---|---|
//! | 22701 | `HandleCustomLadderSelection` stores the port plus one |
//! | 22702 | the list box selects row 0, not the last ladder's row |
//! | 22703 | the recent ladders take the special colour |
//! | 22704 | the local ladders take the special colour |
//! | 22705 | the combo box has no "choose a ladder" entry |
//! | 22706 | that entry's item data is 0, not -1 |
//! | 22707 | the combo box adds a last ladder that is not valid for custom games |
//! | 22708 | Init enables "limit armies" even when stats are used |
//! | 22709 | Init does not clear "limit armies" when stats are used |
//! | 22710 | Update does not disable "limit armies" when stats are used |
//! | 22711 | Escape acts on the key-down |
//! | 22712 | `GWM_DESTROY` leaves the parent set |
//! | 22713 | the game name's leading tab and line breaks are kept |
//! | 22714 | the "no ladder" entry opens the ladder overlay |
//! | 22715 | Cancel leaves the lobby join flag |
//! | 22716 | an empty game name is not replaced by the local name |
//! | 22717 | the default ladder address is `local` |
//! | 22718 | the staging room's group is 1 |
//! | 22719 | the password keeps a unit whose low byte is 0 as a NUL |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=3000` (the default, about 15 s a run), max ULP 0. The first run had **157 mismatches of 121,325 outputs, all in my generator** (two ladders with the same index,
//! which the draft keys by; a Rust `true` written where the C++ reads `1`); with those fixed, **0 mismatches over 3,000 sequences, 125,345 outputs compared**. The prediction (the password's
//! `translate`) was fixed on reading before the first run; plant 22719 shows the test would have caught it. Planted bugs (mismatches of 3,000 sequences): 22701 53; 22702 12; 22703 10;
//! 22704 58; 22705 298; 22706 11; 22707 21; 22708 369; 22709 177; 22710 4; 22711 353; 22712 16; 22713 19; 22714 17; 22715 149; 22716 26; 22717 372; 22718 395; 22719 12. **19 of 19 caught**
//! (an earlier 22703, the recent ladders' `continue` on the last ladder, is an equivalent mutant in the list variant: the last ladder is already used; replaced).

mod support;
mod ui4_common;

use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::gadget_combo_box as cbx;
use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GadgetData, GameFont, WinId, WinInstanceData};
use rts_lockstep_engine::drafts::ui::popup_host_game::*;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"PopupHostGameSystem";
const INP: &[u8] = b"PopupHostGameInput";
const SELECTED: i32 = 0xFF00_FF00u32 as i32;
const UNSELECTED: i32 = 0xFFC0_C0C0u32 as i32;

#[derive(Default, Clone)]
struct CPref {
    addr: Vec<u8>,
    port: u16,
    ao: bool,
    fl: bool,
    us: bool,
}

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Shell,
    ph: PopupHostGame,
    ub: bool,
    local_name: Vec<u8>,
    ping: Vec<u8>,
    ip: u32,
    exe_crc: u32,
    ini_crc: u32,
    restrict: bool,
    special: Vec<LadderInfoLite>,
    standard: Vec<LadderInfoLite>,
    local: Vec<LadderInfoLite>,
    recent: Vec<(Vec<u8>, u16)>,
    cp: CPref,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            popup_host_game_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            popup_host_game_input(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn block_input_left_up(&mut self) {
        self.log.push_str("block\n");
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
    fn create_layout(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<usize> {
        None
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
        self.keys.key(name)
    }
}

impl Host {
    fn all(&self) -> Vec<&LadderInfoLite> {
        self.special.iter().chain(self.standard.iter()).chain(self.local.iter()).collect()
    }
}

fn ascii_units(b: &[u8]) -> Vec<u16> {
    b.iter().map(|&c| (c as i8) as i16 as u16).collect()
}

impl PopupHostGameHost for Host {
    fn popup_host_game(&mut self) -> &mut PopupHostGame {
        &mut self.ph
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut t = b"T:".to_vec();
        t.extend_from_slice(label);
        t.iter().map(|&b| u16::from(b)).collect()
    }
    fn gamespy_color_map_selected(&self) -> i32 {
        SELECTED
    }
    fn gamespy_color_map_unselected(&self) -> i32 {
        UNSELECTED
    }
    fn custom_pref_set_last_ladder_and_write(&mut self, addr: &[u8], port: u16) {
        self.cp.addr = addr.to_vec();
        self.cp.port = port;
        self.cpref_write();
    }
    fn custom_pref_last_ladder_addr(&mut self) -> Vec<u8> {
        self.cp.addr.clone()
    }
    fn custom_pref_last_ladder_port(&mut self) -> u16 {
        self.cp.port
    }
    fn custom_pref_load_for_local_profile(&mut self) {
        self.log.push_str("cpref load GeneralsOnline\\CustomPref7.ini\n");
    }
    fn custom_pref_allows_observers(&mut self) -> bool {
        self.cp.ao
    }
    fn custom_pref_use_stats(&mut self) -> bool {
        self.cp.us
    }
    fn custom_pref_factions_limited(&mut self) -> bool {
        self.cp.fl
    }
    fn custom_pref_set_and_write(&mut self, allow_observers: bool, factions_limited: bool, use_stats: bool) {
        self.cp.ao = allow_observers;
        self.cp.fl = factions_limited;
        self.cp.us = use_stats;
        self.cpref_write();
    }
    fn ladder_find_by_index(&self, index: i32) -> Option<LadderInfoLite> {
        if index == 0 {
            return None;
        }
        self.all().into_iter().find(|l| l.index == index).cloned()
    }
    fn ladder_find(&self, addr: &[u8], port: u16) -> Option<LadderInfoLite> {
        self.all().into_iter().find(|l| l.address == addr && l.port == port).cloned()
    }
    fn ladder_local(&self) -> Vec<LadderInfoLite> {
        self.local.clone()
    }
    fn ladder_special(&self) -> Vec<LadderInfoLite> {
        self.special.clone()
    }
    fn ladder_standard(&self) -> Vec<LadderInfoLite> {
        self.standard.clone()
    }
    fn ladder_pref_recent_ladders(&mut self) -> Vec<(Vec<u8>, u16)> {
        self.recent.clone()
    }
    fn gamespy_local_name(&self) -> Vec<u16> {
        ascii_units(&self.local_name)
    }
    fn gamespy_set_current_group_room(&mut self, room: i32) {
        let _ = writeln!(self.log, "gsi room {room}");
    }
    fn gamespy_internal_ip(&self) -> u32 {
        self.ip
    }
    fn gamespy_ping_string(&self) -> Vec<u8> {
        self.ping.clone()
    }
    fn gamespy_restrict_games_to_lobby(&self) -> bool {
        self.restrict
    }
    fn global_exe_crc(&self) -> u32 {
        self.exe_crc
    }
    fn global_ini_crc(&self) -> u32 {
        self.ini_crc
    }
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>) {
        wm.win_set_modal(w);
    }
    fn gamespy_open_overlay_ladder_select(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("overlay open 3\n");
    }
    fn gamespy_close_overlay_game_options(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("overlay close 2\n");
    }
    fn set_lobby_attempt_host_join_false(&mut self) {
        self.log.push_str("lobby attemptjoin 0\n");
    }
    fn game_spy_game_set_game_name(&mut self, name: &[u16]) {
        self.log.push_str("game name");
        text(&mut self.log, name);
        self.log.push('\n');
    }
    fn game_spy_game_set_options(&mut self, allow_observers: bool, old_factions_only: bool, use_stats: bool) {
        let _ = writeln!(self.log, "game obs {}\ngame old {}\ngame stats {}", u8::from(allow_observers), u8::from(old_factions_only), u8::from(use_stats));
    }
    fn game_spy_game_set_ladder(&mut self, ip: &[u8], port: u16) {
        let _ = writeln!(self.log, "game ladderip {}\ngame ladderport {}", String::from_utf8_lossy(ip), port);
    }
    fn peer_request_create_staging_room(&mut self, r: &CreateStagingRoomRequest) {
        let h = |b: &[u8]| format!("[{}]", b.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(","));
        self.log.push_str("peer create");
        text(&mut self.log, &r.text);
        let _ = writeln!(
            self.log,
            " pw {} obs {} stats {} exe {} ini {} ver {} restrict {} ladip {} port {} ping {}",
            h(&r.password),
            u8::from(r.allow_observers),
            u8::from(r.use_stats),
            r.exe_crc,
            r.ini_crc,
            r.game_version,
            u8::from(r.restrict_game_list),
            h(&r.ladder_ip),
            r.lad_port,
            h(&r.host_ping_str)
        );
    }
    fn undefined(&mut self, _what: &str) {
        self.ub = true;
    }
}

impl Host {
    fn cpref_write(&mut self) {
        let c = &self.cp;
        let _ = writeln!(self.log, "cpref write {} {} {} {} {}", String::from_utf8_lossy(&c.addr), c.port, u8::from(c.ao), u8::from(c.fl), u8::from(c.us));
    }
}

// ---------------------------------------------------------------------------------------------------------------------

fn hx(s: &str) -> String {
    hex(&s.encode_utf16().collect::<Vec<u16>>())
}

fn name_texts() -> Vec<String> {
    let mut v: Vec<String> = ["Bob", "A", "  Bob", "Bob  ", "  ", "x y", "pass"].iter().map(|s| hx(s)).collect();
    v.push("-".into());
    v.push("20,20,41".into());
    v.push("9,a,42".into());
    v.push("41,100,42".into());
    v.push("e9,4e2d".into());
    v
}

struct Scene {
    ops: Vec<String>,
    /// the first window of each name
    named: Vec<(usize, String)>,
    /// (index, kind, x, y, w, h): 0 top-level, 1 name entry, 2 other entry, 3 create, 4 cancel, 5 check box, 6 combo, 7 list box
    wins: Vec<(usize, u8, i32, i32, i32, i32)>,
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

fn gen_scene(g: &mut Gen) -> Scene {
    NEXT_INDEX.store(1, std::sync::atomic::Ordering::Relaxed);
    let mut ops = Vec::new();
    let mut wins: Vec<(usize, u8, i32, i32, i32, i32)> = Vec::new();
    let en = win_status::ENABLED;
    let mut next = 0usize;
    let mut tops = Vec::new();
    let parent_present = g.below(6) != 0;
    if parent_present {
        ops.push(format!("@ wu -1 0 0 0 640 480 {en} {}", gws::USER_WINDOW));
        wins.push((0, 0, 0, 0, 640, 480));
        tops.push(0usize);
        next += 1;
    }
    let decoys = if parent_present { g.below(3) as usize } else { 1 + g.below(2) as usize };
    for _ in 0..decoys {
        let (x, y) = (g.below(400) as i32, g.below(300) as i32);
        ops.push(format!("@ wu -1 0 {x} {y} 300 200 {en} {}", gws::USER_WINDOW));
        wins.push((next, 0, x, y, 300, 200));
        tops.push(next);
        next += 1;
    }
    let mut named: Vec<(usize, String)> = Vec::new();
    let mut k = 0;
    // a list box (unnamed), then the controls; the combo box last (its children take the indices after it)
    ops.push(format!("W -1 6 600 10 60 20 {en} {} 0 6 1 0 0 0 0 0 0", gws::SCROLL_LISTBOX));
    wins.push((next, 7, 600, 10, 60, 20));
    next += 1;
    let list = [
        ("TextEntryGameName", 1u8, true),
        ("TextEntryGameDescription", 2, true),
        ("TextEntryLadderPassword", 2, true),
        ("TextEntryGamePassword", 2, true),
        ("ButtonCreateGame", 3, true),
        ("ButtonCancel", 4, true),
        ("CheckBoxAllowObservers", 5, false),
        ("CheckBoxUseStats", 5, false),
        ("CheckBoxLimitArmies", 5, false),
        ("ComboBoxLadderName", 6, true),
    ];
    for (name, kind, optional) in list {
        for copy in 0..2 {
            let present = if copy == 0 { !optional || g.below(6) != 0 } else { g.below(3) == 0 && decoys > 0 && kind != 6 };
            if !present {
                continue;
            }
            let parent: Option<usize> = if copy == 1 {
                Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize])
            } else {
                match g.below(20) {
                    0..=13 if parent_present => Some(0),
                    0..=13 => None,
                    14..=16 => None,
                    _ if decoys > 0 => Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize]),
                    _ => None,
                }
            };
            let (x, y, w, h) = (10 + (k % 5) * 70, 10 + (k / 5) * 30, 60, 20);
            k += 1;
            let mut status = en;
            if g.below(8) == 0 {
                status |= win_status::HIDDEN;
            }
            let at = |p: usize| wins.iter().position(|q| q.0 == p).expect("parent");
            let (px, py) = parent.map_or((0, 0), |p| (wins[at(p)].2, wins[at(p)].3));
            let ps = parent.map_or(-1, |p| p as i32);
            let line = match kind {
                1 | 2 => format!("W {ps} 9 {x} {y} {w} {h} {status} {} 0 {} 0 0 0 0 _", gws::ENTRY_FIELD, g.below(20) as i32 + 1),
                3 | 4 => format!("W {ps} 1 {x} {y} {w} {h} {status} {} 0", gws::PUSH_BUTTON),
                5 => format!("W {ps} 2 {x} {y} {w} {h} {status} {} 0", gws::CHECK_BOX),
                _ => format!("W {ps} 4 {x} {y} {w} {h} {status} {} 0 {} 5 64 0 0 6 1 0", gws::COMBO_BOX, u8::from(g.below(5) != 0)),
            };
            ops.push(line);
            wins.push((next, kind, px + x, py + y, w, h));
            named.push((next, name.to_string()));
            next += 1;
        }
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} PopupHostGame.wnd:{name}"));
    }
    if parent_present {
        ops.push("@ nm 0 PopupHostGame.wnd:ParentHostPopUp".to_string());
    }
    scene_state(g, &mut ops);
    let named: Vec<(usize, String)> = named.into_iter().rev().collect::<Vec<_>>().into_iter().rev().collect();
    Scene { ops, named, wins }
}

static NEXT_INDEX: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);

const ADDRS: [&str; 3] = ["a.b", "c.d", "e.f"];

fn scene_state(g: &mut Gen, ops: &mut Vec<String>) {
    for _ in 0..g.below(7) {
        let kind = g.below(3);
        // (the draft keys a ladder by its index, as upstream's pointers: unique within a case)
        let n = NEXT_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let idx = if kind == 2 { -n } else { n };
        ops.push(format!("@ hg ladder {kind} {idx} {} {} {} {}", g.below(3), pick(g, &ADDRS), u8::from(g.below(4) != 0), hx(&format!("L{}", g.below(100)))));
    }
    for _ in 0..g.below(4) {
        ops.push(format!("@ hg recent {} {}", pick(g, &ADDRS), g.below(3)));
    }
    ops.push(format!("@ hg cpref {} {} {} {} {}", if g.below(3) == 0 { "-".to_string() } else { pick(g, &ADDRS).to_string() }, g.below(3), g.below(2), g.below(2), g.below(2)));
    ops.push(format!("@ hg name {}", pick(g, &["Host", "-", "caf\u{e9}", "A"])));
    ops.push(format!("@ hg ip {}", pick(g, &[0u32, 1, 0xC0A8_0001])));
    ops.push(format!("@ hg ping {}", pick(g, &["-", "p1", "xyz"])));
    ops.push(format!("@ hg crc {} {} {}", g.below(3), 7 * g.below(3), g.below(2)));
}

fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let combo: Vec<usize> = s.wins.iter().filter(|w| w.1 == 6).map(|w| w.0).collect();
    let entries: Vec<usize> = s.wins.iter().filter(|w| w.1 == 1 || w.1 == 2).map(|w| w.0).collect();
    let list: Vec<usize> = s.wins.iter().filter(|w| w.1 == 7).map(|w| w.0).collect();
    let clickable: Vec<(usize, u8, i32, i32, i32, i32)> = s.wins.iter().filter(|w| (3..=5).contains(&w.1)).cloned().collect();
    let any = |g: &mut Gen| pick(g, &s.wins).0;
    let menu_win = |g: &mut Gen| pick(g, &s.wins.iter().filter(|w| w.1 == 0).map(|w| w.0).collect::<Vec<_>>());
    let by_name = |n: &str| s.named.iter().find(|(_, k)| k == n).map(|(i, _)| *i);
    if g.below(100) < 14 {
        // composites: a text set, then the message that reads it
        let t = pick(g, &name_texts());
        let t2 = pick(g, &name_texts());
        let win = menu_win(g);
        if let (Some(name), Some(pw), Some(create)) = (by_name("TextEntryGameName"), by_name("TextEntryGamePassword"), by_name("ButtonCreateGame")) {
            return match g.below(3) {
                0 => format!("@ tx {name} {t}\n@ tx {pw} {t2}\n@ sys 0 {win} {} 1 {create} 0", ggm::GBM_SELECTED),
                1 => format!("@ tx {name} {t}\n@ sys 0 {win} {} 1 {name} 0", rts_lockstep_engine::client::gadget_text_entry::gem::UPDATE_TEXT),
                _ => format!("@ tx {name} {t}\n@ hg init\n@ sys 0 {win} {} 1 {create} 0", ggm::GBM_SELECTED),
            };
        }
    }
    match g.below(100) {
        0..=11 => "@ hg init".into(),
        12..=19 => "@ hg update".into(),
        20..=22 => format!("@ hg hide {}", g.below(2)),
        23..=27 => format!("@ hg ladsel {}", g.below(7) as i32 - 3),
        28..=31 => "@ hg popcombo".into(),
        32..=35 if !list.is_empty() => format!("@ hg poplist {}", list[0]),
        36..=37 if !list.is_empty() => format!("@ hg lbdata {} {}", list[0], g.below(6)),
        38..=49 => {
            let msg = match g.below(8) {
                0..=2 => ggm::GBM_SELECTED,
                3 => ggm::GCM_SELECTED,
                4 => rts_lockstep_engine::client::gadget_text_entry::gem::UPDATE_TEXT,
                _ => pick(g, &[1, 2, 3, 4, 21, 23, 24, 25]),
            };
            let win = menu_win(g);
            let ctl = msg == ggm::GBM_SELECTED || msg == ggm::GCM_SELECTED || msg == rts_lockstep_engine::client::gadget_text_entry::gem::UPDATE_TEXT;
            let (kind, d1, d2) = if ctl { (1, any(g) as u32, g.below(3)) } else if msg == gwm::INPUT_FOCUS { (0, 2 + g.below(2), 0) } else { (0, g.below(3), g.below(3)) };
            format!("@ sys 0 {win} {msg} {kind} {d1} {d2}")
        }
        50..=57 => {
            let key = pick(g, &[1u32, 1, 1, 2, 28, 0x101, 0x201, 57]);
            let state = pick(g, &[1u32, 2, 3, 0, 0x101, 4]);
            let win = menu_win(g);
            let msg = if g.below(6) == 0 { pick(g, &[1u32, 2, 24, 25, 123]) } else { gwm::CHAR };
            format!("@ inp 0 {win} {msg} 0 {key} {state}")
        }
        58..=67 if !clickable.is_empty() => {
            let (_, _, x, y, ww, hh) = pick(g, &clickable);
            format!("M {} {} {} 0 0", if g.below(2) == 0 { gwm::LEFT_DOWN } else { gwm::LEFT_UP }, x + ww / 2, y + hh / 2)
        }
        68..=73 if !entries.is_empty() => format!("@ tx {} {}", pick(g, &entries), pick(g, &name_texts())),
        74..=77 if !entries.is_empty() => format!("I {} {}", pick(g, &entries), pick(g, &[u32::from(b'a'), 32, 13, 0xE9, 0x100])),
        78..=81 if !combo.is_empty() => format!("s {} {} {}", combo[0], g.below(5) as i32 - 1, g.below(2)),
        82..=85 => {
            let mut v = Vec::new();
            scene_state(g, &mut v);
            v.swap_remove(g.below(v.len() as u32) as usize)
        }
        86..=89 => format!("H {} {}", any(g), g.below(2)),
        90..=93 => format!("E {} {}", any(g), g.below(2)),
        _ => format!("F {}", any(g) as i32),
    }
}

fn state_dump(wm: &GameWindowManager, h: &Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let p = &h.ph;
    let _ = writeln!(
        o,
        "hg ids {} {} {} {} {} {} {} {} {} {} {} populating {}",
        p.parent_popup_id,
        p.text_entry_game_name_id,
        p.button_create_game_id,
        p.check_box_allow_observers_id,
        p.text_entry_game_description_id,
        p.button_cancel_id,
        p.text_entry_ladder_password_id,
        p.combo_box_ladder_name_id,
        p.text_entry_game_password_id,
        p.check_box_limit_armies_id,
        p.check_box_use_stats_id,
        u8::from(p.is_populating_ladder_box)
    );
    let _ = writeln!(
        o,
        "hg wins {} {} {} {} {} {} {} {} {} {} {}",
        idx(p.parent_popup),
        idx(p.text_entry_game_name),
        idx(p.button_create_game),
        idx(p.check_box_allow_observers),
        idx(p.text_entry_game_description),
        idx(p.button_cancel),
        idx(p.combo_box_ladder_name),
        idx(p.text_entry_ladder_password),
        idx(p.text_entry_game_password),
        idx(p.check_box_limit_armies),
        idx(p.check_box_use_stats)
    );
    let c = &h.cp;
    let hexb = c.addr.iter().map(|b| format!("{b:x}")).collect::<Vec<_>>().join(",");
    let _ = writeln!(o, "hg cpref [{hexb}] {} {} {} {}", c.port, u8::from(c.ao), u8::from(c.fl), u8::from(c.us));
    o
}

fn hexs(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

fn menu_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) {
    let v: Vec<&str> = op.split(' ').collect();
    let num = |i: usize| -> i64 { v[i].parse().expect("number") };
    match v[1] {
        "sys" | "inp" => {
            let (win, msg, kind, d1, d2) = (num(3) as usize, num(4) as u32, num(5), num(6) as u32, num(7) as u32);
            let a = if kind == 1 { win_data(Some(d1 as usize)) } else { d1 };
            let r = if v[1] == "sys" { popup_host_game_system(wm, h, win, msg, a, d2) } else { popup_host_game_input(wm, h, win, msg, a, d2) };
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "hg" => match v[2] {
            "init" => popup_host_game_init(wm, h, 0),
            "update" => popup_host_game_update(wm, h, 0),
            "hide" => custom_match_hide_host_popup(wm, h, num(3) != 0),
            "ladsel" => handle_custom_ladder_selection(h, num(3) as i32),
            "popcombo" => populate_custom_ladder_combo_box(wm, h),
            "poplist" => populate_custom_ladder_list_box(wm, h, Some(num(3) as usize)),
            "lbdata" => {
                let d = lbx::get_item_data(wm, h, num(3) as usize, num(4) as i32, 0);
                let _ = writeln!(h.log, "lbdata {d}");
            }
            "name" => h.local_name = if v[3] == "-" { Vec::new() } else { v[3].as_bytes().to_vec() },
            "ip" => h.ip = num(3) as u32,
            "ping" => h.ping = if v[3] == "-" { Vec::new() } else { v[3].as_bytes().to_vec() },
            "crc" => {
                h.exe_crc = num(3) as u32;
                h.ini_crc = num(4) as u32;
                h.restrict = num(5) != 0;
            }
            "ladder" => {
                let l = LadderInfoLite { index: num(4) as i32, port: num(5) as u16, address: v[6].as_bytes().to_vec(), valid_custom: v[7] != "0" && v[7] != "false", name: hexs(v[8]) };
                match num(3) {
                    0 => h.special.push(l),
                    1 => h.standard.push(l),
                    _ => h.local.push(l),
                }
            }
            "recent" => h.recent.push((v[3].as_bytes().to_vec(), num(4) as u16)),
            "cpref" => h.cp = CPref { addr: if v[3] == "-" { Vec::new() } else { v[3].as_bytes().to_vec() }, port: num(4) as u16, ao: num(5) != 0, fl: num(6) != 0, us: num(7) != 0 },
            other => panic!("hg {other}"),
        },
        other => panic!("@ {other}"),
    }
}

/// `W` of a check box (type 2) or a combo box (type 4), and the combo's `s` operation, which `ui4_common::scene_op` does not carry; false when `op` is none of them.
fn extra_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) -> bool {
    let v: Vec<&str> = op.split(' ').collect();
    match v[0] {
        "W" if v[2] == "2" || v[2] == "4" => {
            let n: Vec<i64> = v[1..].iter().map(|s| s.parse().unwrap()).collect();
            let (parent, ty, x, y, ww, hh) = (n[0] as i32, n[1], n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32);
            let (status, style, visual) = (n[6] as u32, n[7] as u32, n[8] != 0);
            let inst0 = WinInstanceData { style, ..WinInstanceData::new() };
            let p = (parent >= 0).then_some(parent as usize);
            if ty == 2 {
                wm.gogo_gadget_look(h, p, status, x, y, ww, hh, inst0, GadgetData::None, visual);
            } else {
                let f: Vec<i32> = n[9..17].iter().map(|&x| x as i32).collect();
                let d = ComboBoxData { is_editable: f[0] != 0, max_display: f[1], max_chars: f[2], ascii_only: f[3] != 0, letters_and_numbers_only: f[4] != 0 };
                let mut t = cbx::combo_template(&d);
                t.listbox.list_length = f[5] as i16;
                t.listbox.scroll_bar = f[6] != 0;
                let mut inst = inst0;
                inst.font = (f[7] != 0).then(|| GameFont { name: b"F".to_vec(), point_size: f[7], bold: false });
                cbx::gogo_gadget_combo_box(wm, h, p, status, x, y, ww, hh, inst, t, visual);
            }
            true
        }
        "s" => {
            cbx::set_selected_pos(wm, h, v[1].parse().unwrap(), v[2].parse().unwrap(), v[3] != "0");
            true
        }
        _ => false,
    }
}

#[test]
fn popup_host_game_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C5_02);
    let mut chk = Check::new("PopupHostGame");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    let _ = gadget::check_box_is_checked;
    for case in 0..cases(3_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 4 + g.below(19);
        let mut ops = scene.ops.clone();
        for _ in 0..steps {
            let o = gen_op(&mut g, &scene);
            ops.extend(o.split('\n').map(str::to_string));
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Host { cp: CPref { ao: true, us: true, ..Default::default() }, ..Default::default() };
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            if op.starts_with("@ hg") || op.starts_with("@ sys") || op.starts_with("@ inp") {
                menu_op(&mut wm, &mut h, op);
            } else if !extra_op(&mut wm, &mut h, op) {
                let mut keys = std::mem::take(&mut h.keys);
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                h.keys = keys;
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() || h.ub {
                run = i;
                break;
            }
            let log = std::mem::take(&mut h.log);
            let st = state_dump(&wm, &h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        ops.truncate(run);
        compare_case(env!("UI5_HOST_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
