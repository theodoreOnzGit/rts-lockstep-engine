// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui4) of the draft `src/drafts/ui/popup_join_game.rs` (`PopupJoinGameInit`, `PopupJoinGameInput`,
//! `PopupJoinGameSystem`, the private `joinGame`, the file's statics) against upstream `PopupJoinGame.cpp`, cut verbatim by build.rs (`drafts_ui4`) into the
//! program `UI4_JOIN_EXE` (differential/wrap_draft_ui4_join.cpp lists the stand-ins and the one edit), which also holds upstream's real window manager and
//! gadgets (text entry, static text, push button: #70's program).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). Each is a scene of named windows (`PopupJoinGame.wnd:...`): the popup's parent (menu callbacks;
//! missing one time in six), 0 to 2 decoy top-level windows, and the three controls (the password text entry, the game name static text, *Cancel*), each
//! present with probability 5/6, under the parent (70 %), top-level (15 %) or under a decoy (15 %), and a decoy copy of each name one time in three (the
//! lookup starts at the parent, so a copy elsewhere must not be found). 3 to 20 operations: `PopupJoinGameInit`, the staging room set (exists or not, an id,
//! a name), the system callback called directly (`GBM_SELECTED` / `GEM_EDIT_DONE` with a control, `CREATE`, `DESTROY`, other messages), the input callback
//! (`GWM_CHAR` with Escape and other keys in the up, down and other states, other messages), presses and releases at the controls (*Cancel*'s
//! `GBM_SELECTED` reaches the parent's callback through the real gadget), typed characters into the entry (Enter ends the edit; letters, spaces, 0xE9, 0x4E2D
//! and 0x100, whose low byte is 0), the entry's text set (with white space), hiding, enabling, focusing. After each operation both programs print the stand-in
//! calls (the overlay closed, the lobby flag, the peer request with the room's id and name and the password's bytes), the messages, the dump of every window,
//! the modal stack and the focus, and the statics of the file; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour or a deliberate deviation of the draft): a NULL control as `GBM_SELECTED` / `GEM_EDIT_DONE`'s `mData1`
//! (upstream dereferences it); `GWM_INPUT_FOCUS` sent directly (`*(Bool *)mData2` needs a pointer; it is exercised through `winSetFocus`, the draft's
//! `focus_reply`); windows destroyed while the statics name them; Unicode spaces beyond ASCII in `trim` (MSVC's `iswspace` for them is not modelled).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! One mismatch kind in the draft, found by reading `AsciiString::translate` while writing the wrapper: a password unit whose low byte is 0 (0x100) is kept as
//! a NUL byte by the draft (`c as u8`) and dropped by upstream's `concat((char)c)`. Nothing else.
//!
//! # Planted bugs (`L9_PLANT=<id>`, the draft's `plant(id)`; mismatching cases of 3,000)
//!
//! | id | planted bug | cases |
//! |---|---|---|
//! | 21481 | the entry is looked up from the whole tree, not from the parent | 778 |
//! | 21482 | `Init` does not clear the entry | 828 |
//! | 21483 | `Init` does not show the game's name | 461 |
//! | 21484 | `Init` does not make the popup modal | 1,990 |
//! | 21485 | Escape acts on the press, not the release | 1,502 |
//! | 21486 | the key is tested as the whole word, not its low byte | 719 |
//! | 21487 | closing the overlay leaves the popup's window named | 575 |
//! | 21488 | any control cancels | 885 |
//! | 21489 | the password is not trimmed | 36 |
//! | 21490 | the peer request's room id is off by one | 53 |
//! | 21491 | `joinGame` does not close the overlay | 53 |
//! | 21492 | `GWM_CREATE` is answered ignored | 328 |
//!
//! **12 of 12 caught.** An earlier 21492 (`GWM_INPUT_FOCUS` answered for any non-zero `mData1`) was an equivalent mutant: the answer is the window manager's
//! `focus_reply`, reset by every `winSetFocus`, so no operation can observe it; replaced.
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences, 413,281 operations compared** (the `DIFF` line is printed by the run), max
//! ULP 0. The first run (300 sequences) had **2 mismatches, both in the draft**, as predicted: the password `61,0` (rust) against `61` (C++), a unit 0x100 kept as a
//! NUL byte. **Fixed** in `popup_join_game.rs` (the units' low bytes, the zeros dropped); then 0. The shim's `AsciiString::translate` has the same NUL slip as the
//! draft had (it appends `(char)c` whatever it is), which is why the wrapper edits that one line of the cut for `ui4_translate`.
//!
//! Not covered: `GWM_INPUT_FOCUS` sent directly with `mData1` TRUE (see the excluded inputs); `GameSpyInfo`, the peer message queue and the overlay are stand-ins (network).

mod support;
mod ui4_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, GameFont, WinId};
use rts_lockstep_engine::drafts::ui::popup_join_game::*;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"PopupJoinGameSystem";
const INP: &[u8] = b"PopupJoinGameInput";

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Option<Shell>,
    pj: PopupJoinGame,
    room: Option<StagingRoomInfo>,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            popup_join_game_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], _win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            popup_join_game_input(wm, self, msg, d1, d2)
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

impl PopupJoinGameHost for Host {
    fn popup_join_game(&mut self) -> &mut PopupJoinGame {
        &mut self.pj
    }
    fn current_staging_room(&self) -> Option<StagingRoomInfo> {
        self.room.clone()
    }
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>) {
        wm.win_set_modal(w);
    }
    fn gamespy_close_overlay_game_password(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("overlay close 1\n");
    }
    fn set_lobby_attempt_host_join_false(&mut self) {
        self.log.push_str("lobby attemptjoin 0\n");
    }
    fn peer_request_join_staging_room(&mut self, game_name: &[u16], room_id: i32, password: &[u8]) {
        let _ = write!(self.log, "peer join {room_id}");
        text(&mut self.log, game_name);
        let pw: Vec<String> = password.iter().map(|b| format!("{b:x}")).collect();
        let _ = writeln!(self.log, " pw [{}]", pw.join(","));
    }
}

const TEXTS: [&str; 9] = ["-", "41", "41,42,43", "20,41,20", "9,41,42,a", "20,20,20", "e9,4e2d", "100,41", "61,20,62,20,63"];

struct Scene {
    ops: Vec<String>,
    /// (index, kind, x, y, w, h): 0 top-level, 1 text entry, 2 static text, 3 button
    wins: Vec<(usize, u8, i32, i32, i32, i32)>,
}

fn gen_scene(g: &mut Gen) -> Scene {
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
    // (a scene always has a window: with no parent, at least one decoy)
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
    for (name, kind) in [("TextEntryGamePassword", 1u8), ("StaticTextGameName", 2), ("ButtonCancel", 3)] {
        // the real one, and a decoy copy
        for copy in 0..2 {
            let present = if copy == 0 { g.below(6) != 0 } else { g.below(3) == 0 && decoys > 0 };
            if !present {
                continue;
            }
            // the real one: under the parent 70 %, top-level 15 %, under a decoy 15 %; a copy: under a decoy
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
            let (px, py) = parent.map_or((0, 0), |p| (wins[wins.iter().position(|q| q.0 == p).expect("parent")].2, wins[wins.iter().position(|q| q.0 == p).expect("parent")].3));
            let ps = parent.map_or(-1, |p| p as i32);
            let line = match kind {
                1 => format!("W {ps} 9 {x} {y} {w} {h} {status} {} 0 {} {} {} {} {} _", gws::ENTRY_FIELD, g.below(14) as i32 + 1, u8::from(g.below(5) == 0), u8::from(g.below(6) == 0), u8::from(g.below(6) == 0), u8::from(g.below(5) == 0)),
                2 => format!("W {ps} 11 {x} {y} {w} {h} {status} {} 0", gws::STATIC_TEXT),
                _ => format!("W {ps} 1 {x} {y} {w} {h} {status} {} 0", gws::PUSH_BUTTON),
            };
            ops.push(line);
            wins.push((next, kind, px + x, py + y, w, h));
            named.push((next, name.to_string()));
            next += 1;
        }
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} PopupJoinGame.wnd:{name}"));
    }
    // the parent's own name (the decoys have none)
    if parent_present {
        ops.push("@ nm 0 PopupJoinGame.wnd:ParentJoinPopUp".to_string());
    }
    Scene { ops, wins }
}

fn pick<T: Copy>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize]
}

fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let text = TEXTS[g.below(TEXTS.len() as u32) as usize];
    let ctl = |g: &mut Gen| pick(g, &s.wins).0;
    let entries: Vec<usize> = s.wins.iter().filter(|w| w.1 == 1).map(|w| w.0).collect();
    match g.below(100) {
        0..=11 => "@ jn init".into(),
        12..=21 => format!("@ jn room {} {} {}", g.below(4).min(1), g.below(4), if g.below(4) == 0 { "-" } else { text }),
        22..=33 => {
            let msg = match g.below(6) {
                0..=1 => ggm::GBM_SELECTED,
                2..=3 => ggm::GEM_EDIT_DONE,
                _ => pick(g, &[1, 2, 3, 4, 21, 23, 24, 25]),
            };
            // (`GWM_INPUT_FOCUS` directly only with a `mData1` other than TRUE: upstream writes `*(Bool *)mData2` for TRUE)
            let (kind, d1, d2) = if msg == ggm::GBM_SELECTED || msg == ggm::GEM_EDIT_DONE { (1, ctl(g) as u32, g.below(3)) } else if msg == gwm::INPUT_FOCUS { (0, 2 + g.below(2), 0) } else { (0, g.below(3), g.below(3)) };
            format!("@ sys 0 0 {msg} {kind} {d1} {d2}")
        }
        34..=45 => {
            // GWM_CHAR: Escape (DIK_ESCAPE = 1) or another key; the state's bits (KEY_STATE_UP = 1, DOWN = 2)
            let key = pick(g, &[1, 1, 1, 2, 28, 0x101, 0x201, 57]);
            let state = pick(g, &[1, 2, 3, 0, 0x101, 4]);
            if g.below(6) == 0 {
                format!("@ inp 0 0 {} 0 {key} {state}", pick(g, &[1, 2, 24, 25, 23 + 100]))
            } else {
                format!("@ inp 0 0 {} 0 {key} {state}", gwm::CHAR)
            }
        }
        46..=57 => {
            let w = ctl(g);
            let (_, _, x, y, ww, hh) = s.wins[s.wins.iter().position(|q| q.0 == w).expect("window")];
            format!("M {} {} {} 0 0", if g.below(2) == 0 { gwm::LEFT_DOWN } else { gwm::LEFT_UP }, x + ww / 2, y + hh / 2)
        }
        58..=73 if !entries.is_empty() => {
            let e = pick(g, &entries);
            let chars = [u32::from(b'a'), u32::from(b'Z'), u32::from(b'5'), 32, 13, 13, 13, 0xE9, 0x4E2D, 0x100, 9];
            format!("I {e} {}", chars[g.below(11) as usize])
        }
        74..=81 if !entries.is_empty() => format!("@ tx {} {text}", pick(g, &entries)),
        82..=87 => format!("H {} {}", ctl(g), g.below(2)),
        88..=91 => format!("E {} {}", ctl(g), g.below(2)),
        92..=97 => format!("F {}", ctl(g) as i32),
        _ => "@ jn init".into(),
    }
}

fn state_dump(wm: &GameWindowManager, h: &mut Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let p = &h.pj;
    let _ = writeln!(o, "jn parent {} te {} ids {}{}{}", idx(p.parent_popup), idx(p.text_entry_game_password), u8::from(p.parent_popup_id != 0), u8::from(p.text_entry_game_password_id != 0), u8::from(p.button_cancel_id != 0));
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
            let r = if v[1] == "sys" { popup_join_game_system(wm, h, win, msg, a, d2) } else { popup_join_game_input(wm, h, msg, a, d2) };
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "jn" => match v[2] {
            "init" => popup_join_game_init(wm, h, 0),
            "room" => h.room = (num(3) != 0).then(|| StagingRoomInfo { id: num(4) as i32, game_name: hexs(v[5]) }),
            other => panic!("jn {other}"),
        },
        other => panic!("@ {other}"),
    }
}

#[test]
fn popup_join_game_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C4_02);
    let mut chk = Check::new("PopupJoinGame");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    for case in 0..cases(20_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 3 + g.below(18);
        let mut ops = scene.ops.clone();
        for _ in 0..steps {
            ops.push(gen_op(&mut g, &scene));
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Host::default();
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            if op.starts_with("@ jn") || op.starts_with("@ sys") || op.starts_with("@ inp") {
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
            let st = state_dump(&wm, &mut h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        ops.truncate(run);
        compare_case(env!("UI4_JOIN_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
