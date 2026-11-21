// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui4) of the drafts `src/drafts/ui/disconnect_menu.rs` (every `DisconnectMenu` method: `init`,
//! `attachDisconnectManager`, `showScreen`, `hideScreen`, `setPlayerName`, `setPlayerTimeoutTime`, `showPlayerControls`, `hidePlayerControls`,
//! `showPacketRouterTimeout`, `hidePacketRouterTimeout`, `setPacketRouterTimeoutTime`, `sendChat`, `showChat`, `quitGame`, `removePlayer`,
//! `voteForPlayer`, `updateVotes`) and `src/drafts/ui/disconnect_window.rs` (`ShowDisconnectWindow`, `HideDisconnectWindow`,
//! `DisconnectControlInput`, `DisconnectControlSystem`, the file's statics) against upstream `DisconnectMenu.cpp` and `DisconnectWindow.cpp`, cut
//! verbatim by build.rs (`drafts_ui4`) into the program `UI4_DISCONNECT_EXE` (differential/wrap_draft_ui4_disconnect.cpp lists the stand-ins and the one edit), which also
//! holds upstream's real window manager, window layout and gadgets (list box, text entry, push button, static text: #70's program).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). Each is a scene of named windows (`DisconnectScreen.wnd:...`): the screen's parent window
//! with the menu's callbacks, 0 to 2 decoy top-level windows, and under the parent (rarely a decoy) the controls: a text entry, the list box, the
//! quit button and the seven kick buttons (always, upstream dereferences them in `ShowDisconnectWindow`), and each of the 7 x 3 per-player statics
//! and the two packet-router statics with probability 5/6; each hidden with probability 1/6. A layout is made of a random subset of the top-level
//! windows. 3 to 20 operations, the first `DisconnectMenu::init`: the menu's methods, `ShowDisconnectWindow` / `HideDisconnectWindow`, the system
//! callback called directly (`GBM_SELECTED` with a control, `GEM_EDIT_DONE`, other messages), mouse presses and releases at the controls (the
//! buttons' `GBM_SELECTED` reaches the parent's callback through the real gadget), typed characters into the text entry (Enter ends the edit), the
//! entry's text set (with white space), hiding, enabling and focusing windows. After each operation both programs print the stand-in calls (the layout
//! made, the network's chat, quit and vote, the screens hidden), the messages, and the dump of every window (status, text, list box rows, text
//! entry), of the layouts and of the menu's and window's statics; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour or a deliberate deviation of the draft): a NULL control as `GBM_SELECTED` / `GEM_EDIT_DONE`'s `mData1`
//! (upstream dereferences it; the draft answers handled); a vote button or the quit button missing at `ShowDisconnectWindow` (upstream's `winEnable` on
//! NULL); windows destroyed while the statics name them; `winCreateLayout` failing (upstream's `hide` on NULL); `setPlayerTimeoutTime` and
//! `setPacketRouterTimeoutTime` beyond `Int` (the draft takes `i64`; the game's `time_t` is 32-bit, and `itoa` takes an `int`); `isScreenVisible`
//! before `init` (upstream's state is then 0 = ON through NEW's zero fill, the draft's `new()` is OFF; the game always calls `init`);
//! Unicode spaces beyond ASCII in `trim` (MSVC's `iswspace` for them is not modelled). Cases where the Rust window manager reports upstream's
//! undefined behaviour stop before that operation.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! A few mismatches from the harness first (the scene, the stand-ins), then 0 in the drafts: I had read both files against the draft and found no
//! deviation, and expected no other.
//!
//! # Planted bugs (`L9_PLANT=<id>`, the draft's `plant(id)`; mismatching cases of 1,000, the run of the final build)
//!
//! | id | planted bug | cases |
//! |---|---|---|
//! | 21461 | `hidePlayerControls` does not re-enable the vote button | 2 |
//! | 21462 | `showPlayerControls` hides the vote count | 288 |
//! | 21463 | `setPlayerName` clears the timeout text for an empty name too | 4 |
//! | 21464 | `setPlayerName` shows for an empty name, hides otherwise | 395 |
//! | 21465 | `updateVotes` writes the timeout control | 204 |
//! | 21466 | `removePlayer`'s line is white, not red | 119 |
//! | 21467 | `showScreen` does not hide the quit menu | 212 |
//! | 21468 | `init` leaves the state ON | 1,000 |
//! | 21469 | `setPacketRouterTimeoutTime` writes the label | 139 |
//! | 21470 | `showPacketRouterTimeout` does not blank the text | 10 |
//! | 21471 | `ShowDisconnectWindow` enables six vote buttons, not seven | 3 |
//! | 21472 | `ShowDisconnectWindow` does not bring the layout forward | 218 |
//! | 21473 | the quit button stays enabled after Quit | 6 |
//! | 21474 | the vote's slot is off by one | 53 |
//! | 21475 | the chat text is not trimmed | 14 |
//! | 21476 | the entry is not cleared after the edit | 43 |
//! | 21477 | `HideDisconnectWindow` shows the layout | 1,000 |
//! | 21478 | `InitDisconnectWindow` does not focus the entry | 846 |
//! | 21479 | an empty chat line is sent | 105 |
//!
//! **19 of 19 caught.** The first measurement had 21463, 21465, 21469, 21470 at 0: the window manager's dump does not carry a static text's text (it is
//! the gadget's display string, not the window's), so the static texts were invisible. The dump now adds every static text (`GadgetStaticTextGetText`), and the
//! generator makes empty names and timeouts more likely; those four are then caught (4, 204, 139, 10).
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences, 1,412,614 operations compared** (the `DIFF` line is printed by the run), max ULP
//! 0 (nothing is floating point). Before that: the first run (50 sequences) had 19 mismatches, all the harness: the stand-in `itoa` wrote 34 bytes into
//! `updateVotes`' 16-byte buffer (fortified `snprintf` aborted: fixed to `sprintf` of an `int`); then the host did not log `TheAudio->addAudioEvent` (the
//! buttons' click) and the left release's block, which the real gadgets send. No draft change was needed. The prediction held (a few harness mismatches, then 0).
//!
//! Not covered: nothing in the two files; `DisconnectWindow.cpp`'s layout loading (`winCreateLayout`'s reader) and `TheNetwork` are stand-ins (network and file I/O).
//! Note: `DisconnectMenu::new()` starts the state OFF where upstream's zero-filled `NEW` would give ON; only `init` sets it in the game and the test always calls it.

mod support;
mod ui4_common;

use rts_lockstep_engine::client::gadget_text_entry as tex;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, GameFont, WinId};
use rts_lockstep_engine::drafts::ui::disconnect_menu::{self as dm, DisconnectMenu, DisconnectMenuHost};
use rts_lockstep_engine::drafts::ui::disconnect_window::{self as dw, DisconnectWindow, DisconnectWindowHost};
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"DisconnectControlSystem";
const INP: &[u8] = b"DisconnectControlInput";

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Option<Shell>,
    dm: Option<DisconnectMenu>,
    dw: DisconnectWindow,
    members: Vec<usize>,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            dw::disconnect_control_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, _wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            dw::disconnect_control_input(win, msg, d1, d2)
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
        self.shell.get_or_insert_with(Shell::new)
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        let _ = writeln!(self.log, "layout {}", String::from_utf8_lossy(filename));
        Some(make_layout(wm, &self.members))
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

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl DisconnectMenuHost for Host {
    fn disconnect_menu(&mut self) -> &mut DisconnectMenu {
        self.dm.get_or_insert_with(DisconnectMenu::new)
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        self.keys.key(name) as i32
    }
    fn hide_disconnect_window(&mut self, wm: &mut GameWindowManager) {
        dw::hide_disconnect_window(wm, self);
    }
    fn show_disconnect_window(&mut self, wm: &mut GameWindowManager) {
        dw::show_disconnect_window(wm, self);
    }
    fn hide_diplomacy(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("hide diplomacy\n");
    }
    fn hide_in_game_chat(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("hide ingamechat\n");
    }
    fn hide_quit_menu(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("hide quitmenu\n");
    }
    fn network_send_disconnect_chat(&mut self, text: &[u16]) {
        self.log.push_str("net chat");
        ui4_common::text(&mut self.log, text);
        self.log.push('\n');
    }
    fn network_quit_game(&mut self) {
        self.log.push_str("net quit\n");
    }
    fn network_vote_for_player_disconnect(&mut self, slot: i32) {
        let _ = writeln!(self.log, "net vote {slot}");
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut s = format!("T:{}", String::from_utf8_lossy(label));
        if s == "T:Network:PlayerLeftGame" {
            s.push_str(":%ls:");
        }
        utf16(&s)
    }
    fn format_with_name(&self, fmt: &[u16], name: &[u16]) -> Vec<u16> {
        let pat = utf16("%ls");
        match fmt.windows(3).position(|w| w == pat.as_slice()) {
            Some(i) => [&fmt[..i], name, &fmt[i + 3..]].concat(),
            None => fmt.to_vec(),
        }
    }
}

impl DisconnectWindowHost for Host {
    fn disconnect_window(&mut self) -> &mut DisconnectWindow {
        &mut self.dw
    }
}

const TEXTS: [&str; 9] = ["-", "41", "41,42,43", "20,41,20", "9,41,42,a", "20,20,20", "e9,4e2d", "a0,41", "61,20,62,20,63"];

struct Scene {
    ops: Vec<String>,
    members: Vec<usize>,
    /// (index, kind) of every window: 0 top-level, 1 text entry, 2 list box, 3 button, 4 static
    wins: Vec<(usize, u8, i32, i32, i32, i32)>,
}

fn gen_scene(g: &mut Gen) -> Scene {
    let mut ops = Vec::new();
    let mut wins = Vec::new();
    let en = win_status::ENABLED;
    let mut next = 0usize;
    // the parent (menu 0) and 0 to 2 decoys
    ops.push(format!("@ wu -1 0 0 0 640 480 {en} {}", gws::USER_WINDOW));
    wins.push((0, 0, 0, 0, 640, 480));
    next += 1;
    let decoys = g.below(3) as usize;
    for _ in 0..decoys {
        let (x, y) = (g.below(400) as i32, g.below(300) as i32);
        ops.push(format!("@ wu -1 0 {x} {y} 120 90 {en} {}", gws::USER_WINDOW));
        wins.push((next, 0, x, y, 120, 90));
        next += 1;
    }
    let mut names: Vec<(String, u8, bool)> = vec![("TextEntry".into(), 1, g.below(6) != 0), ("ListboxTextDisplay".into(), 2, g.below(6) != 0), ("ButtonQuitGame".into(), 3, true)];
    for i in 1..=7 {
        names.push((format!("ButtonKickPlayer{i}"), 3, true));
    }
    for i in 1..=7 {
        for s in ["Name", "Timeout", "Votes"] {
            names.push((format!("StaticPlayer{i}{s}"), 4, g.below(6) != 0));
        }
    }
    names.push(("StaticPacketRouterTimeout".into(), 4, g.below(6) != 0));
    names.push(("StaticPacketRouterTimeoutLabel".into(), 4, g.below(6) != 0));
    let mut k = 0;
    let mut named = Vec::new();
    for (name, kind, present) in names {
        if !present {
            continue;
        }
        let parent = if g.below(20) == 0 && decoys > 0 { 1 + g.below(decoys as u32) as usize } else { 0 };
        let (x, y, w, h) = (10 + (k % 8) * 70, 10 + (k / 8) * 30, 60, 20);
        k += 1;
        let mut status = en;
        if g.below(6) == 0 {
            status |= win_status::HIDDEN;
        }
        let (px, py) = (wins[parent].2, wins[parent].3);
        let line = match kind {
            1 => format!("W {parent} 9 {x} {y} {w} {h} {status} {} 0 {} {} {} {} {} _", gws::ENTRY_FIELD, g.below(14) as i32 - 1, u8::from(g.below(5) == 0), u8::from(g.below(6) == 0), u8::from(g.below(6) == 0), u8::from(g.below(5) == 0)),
            2 => format!("W {parent} 6 {x} {y} {w} {h} {status} {} 0 {} 1 {} 0 0 0 0 0", gws::SCROLL_LISTBOX, 1 + g.below(20), u8::from(g.below(2) == 0)),
            3 => format!("W {parent} 1 {x} {y} {w} {h} {status} {} 0", gws::PUSH_BUTTON),
            _ => format!("W {parent} 11 {x} {y} {w} {h} {status} {} 0", gws::STATIC_TEXT),
        };
        ops.push(line);
        wins.push((next, kind, px + x, py + y, w, h));
        named.push((next, name));
        next += 1;
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} DisconnectScreen.wnd:{name}"));
    }
    let mut members: Vec<usize> = (0..=decoys).filter(|&i| i == 0 && g.below(5) != 0 || i > 0 && g.below(2) == 0).collect();
    if members.is_empty() {
        members.push(0);
    }
    ops.push(format!("@ lay {} {}", members.len(), members.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")));
    Scene { ops, members, wins }
}

fn pick<T: Copy>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize]
}

fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let slot = g.below(7);
    let text = TEXTS[g.below(TEXTS.len() as u32) as usize];
    let ctl = |g: &mut Gen| pick(g, &s.wins).0;
    let entries: Vec<usize> = s.wins.iter().filter(|w| w.1 == 1).map(|w| w.0).collect();
    let times = [0i32, 1, 59, -1, 12345, i32::MAX, i32::MIN, 100000];
    match g.below(100) {
        0..=3 => "@ dm attach 1".into(),
        4 => "@ dm attach 0".into(),
        5..=8 => "@ dm show".into(),
        9..=12 => "@ dm hide".into(),
        13..=18 => format!("@ dm name {slot} {}", if g.below(3) == 0 { "-" } else { text }),
        19..=24 => format!("@ dm timeout {slot} {}", times[g.below(8) as usize]),
        25 => format!("@ dm showctl {slot}"),
        26..=28 => format!("@ dm hidectl {slot}"),
        29..=30 => "@ dm routershow".into(),
        31..=32 => "@ dm routerhide".into(),
        33..=34 => format!("@ dm routertime {}", times[g.below(8) as usize]),
        35..=36 => format!("@ dm chat {text}"),
        37..=40 => format!("@ dm showchat {text}"),
        41 => "@ dm quit".into(),
        42..=44 => format!("@ dm remove {slot} {text}"),
        45..=46 => format!("@ dm vote {slot}"),
        47..=49 => format!("@ dm votes {slot} {}", times[g.below(8) as usize]),
        50..=53 => "@ dw show".into(),
        54..=57 => "@ dw hide".into(),
        58..=64 => {
            let msg = match g.below(5) {
                0..=1 => ggm::GBM_SELECTED,
                2..=3 => ggm::GEM_EDIT_DONE,
                _ => g.below(40),
            };
            let (kind, d1) = if msg == ggm::GBM_SELECTED || msg == ggm::GEM_EDIT_DONE { (1, ctl(g) as u32) } else { (0, g.below(3)) };
            format!("@ sys 0 0 {msg} {kind} {d1} {}", g.below(3))
        }
        65..=66 => format!("@ inp 0 0 {} 0 {} {}", g.below(40), g.below(300), g.below(3)),
        67..=76 => {
            let w = ctl(g);
            let (_, _, x, y, ww, hh) = s.wins[s.wins.iter().position(|q| q.0 == w).expect("window")];
            let (cx, cy) = (x + ww / 2, y + hh / 2);
            // a click: press, then release
            format!("M {} {cx} {cy} 0 0", if g.below(2) == 0 { gwm::LEFT_DOWN } else { gwm::LEFT_UP })
        }
        77..=84 if !entries.is_empty() => {
            let e = pick(g, &entries);
            let chars = [u32::from(b'a'), u32::from(b'Z'), u32::from(b'5'), 32, 13, 13, 0xE9, 0x4E2D, 9];
            format!("I {e} {}", chars[g.below(9) as usize])
        }
        85..=88 if !entries.is_empty() => format!("@ tx {} {text}", pick(g, &entries)),
        89..=91 => format!("H {} {}", ctl(g), g.below(2)),
        92..=94 => format!("E {} {}", ctl(g), g.below(2)),
        95..=96 => format!("F {}", ctl(g) as i32),
        _ => "@ dm showchat 41".into(),
    }
}

fn state_dump(wm: &GameWindowManager, h: &mut Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    if let Some(m) = &h.dm {
        let _ = writeln!(o, "dm attached {} visible {}", u8::from(m.disconnect_manager_attached), u8::from(m.menu_state == dm::DisconnectMenuStateType::ScreenOn));
    }
    let d = &h.dw;
    let _ = write!(o, "dw layout {} te {} td {} quit {} votes", d.disconnect_menu_layout.map_or(-1, |l| l as i64), idx(d.text_entry_window), idx(d.text_display_window), idx(d.button_quit_window));
    for w in &d.button_vote_player_window {
        let _ = write!(o, " {}", idx(*w));
    }
    let _ = writeln!(o, " ids {}{}{}", u8::from(d.text_entry_id != 0), u8::from(d.text_display_id != 0), u8::from(d.button_quit_id != 0));
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
            let r = if v[1] == "sys" { dw::disconnect_control_system(wm, h, win, msg, a, d2) } else { dw::disconnect_control_input(win, msg, a, d2) };
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "dw" => match v[2] {
            "show" => dw::show_disconnect_window(wm, h),
            _ => dw::hide_disconnect_window(wm, h),
        },
        "dm" => match v[2] {
            "init" => dm::init(wm, h),
            "attach" => dm::attach_disconnect_manager(h, num(3) != 0),
            "show" => dm::show_screen(wm, h),
            "hide" => dm::hide_screen(wm, h),
            "name" => dm::set_player_name(wm, h, num(3) as usize, &hexs(v[4])),
            "timeout" => dm::set_player_timeout_time(wm, h, num(3) as usize, num(4)),
            "showctl" => dm::show_player_controls(wm, h, num(3) as usize),
            "hidectl" => dm::hide_player_controls(wm, h, num(3) as usize),
            "routershow" => dm::show_packet_router_timeout(wm, h),
            "routerhide" => dm::hide_packet_router_timeout(wm, h),
            "routertime" => dm::set_packet_router_timeout_time(wm, h, num(3)),
            "chat" => dm::send_chat(h, &hexs(v[3])),
            "showchat" => dm::show_chat(wm, h, &hexs(v[3])),
            "quit" => dm::quit_game(h),
            "remove" => dm::remove_player(wm, h, num(3) as usize, &hexs(v[4])),
            "vote" => dm::vote_for_player(h, num(3) as i32),
            "votes" => dm::update_votes(wm, h, num(3) as usize, num(4) as i32),
            other => panic!("dm {other}"),
        },
        other => panic!("@ {other}"),
    }
}

#[test]
fn disconnect_menu_and_window_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C4_01);
    let mut chk = Check::new("DisconnectMenu + DisconnectWindow");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    for case in 0..cases(20_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 3 + g.below(18);
        let mut ops = scene.ops.clone();
        let setup = ops.len();
        ops.push("@ dm init".into());
        for _ in 0..steps {
            ops.push(gen_op(&mut g, &scene));
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Host { members: scene.members.clone(), ..Default::default() };
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            if op.starts_with("@ dm") || op.starts_with("@ dw") || op.starts_with("@ sys") || op.starts_with("@ inp") {
                menu_op(&mut wm, &mut h, op);
            } else {
                let mut keys = std::mem::take(&mut h.keys);
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                h.keys = keys;
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() {
                run = i;
                break;
            }
            let log = std::mem::take(&mut h.log);
            // a scene's setup is not compared as an operation; the dump after it is
            let st = state_dump(&wm, &mut h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        ops.truncate(run);
        let _ = setup;
        compare_case(env!("UI4_DISCONNECT_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
