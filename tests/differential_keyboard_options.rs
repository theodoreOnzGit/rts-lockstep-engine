#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/KeyboardOptionsMenu.cpp (the whole file)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The keyboard options screen (`client::keyboard_options_menu`) against the C++, #160.
//!
//! # Method
//!
//! The C++ side is KeyboardOptionsMenu.cpp cut verbatim (see `differential/wrap_keyboard_options.cpp`) on stand-in
//! windows and gadgets that behave as upstream's do for its calls, with the REAL `KeyNames` and `CategoryListName`
//! (MetaEvent.h) and key codes (KeyDefs.h). The Rust side runs the translation on the REAL Rust window manager and
//! gadgets, the screen made from a synthetic `KeyboardOptionsMenu.wnd` and pushed on a real shell (its `Init` run by
//! the shell). Each case:
//!
//! - the game's text (`KEYBOARD:Alt+`, `Ctrl+`, `Shift+`: the usual, missing (`T:` and the label), two the same, an
//!   empty one, one ending in U+012B, whose low byte is `+`), `TheMetaMap` (0 to 30 records of random categories, keys
//!   (the real `KeyNames`' and others), display names (some empty, some the same) and descriptions), the entry's filters
//!   and mouse tracking, and the file's globals as an earlier visit left them;
//! - `init`, then 1 to 40 operations: a category picked in the combo box (-1 .. 12), a command picked in the list (-1
//!   .. 31), the *Back*, *Reset All* and *Assign* buttons, Escape and other keys to the screen, the entry's keys
//!   (the three modifiers down and up, backspace, delete, the tab and arrow keys, escape and the function keys, any
//!   other), characters (`GWM_IME_CHAR`: letters, digits, `+`, return, 0, U+012B, characters of 0x80 and more), the
//!   mouse messages, the focus question, create / destroy / another message, and the update (not in the lexicon).
//!
//! After each, the callback's answer, the messages the screen was told (`sys`), the pop of the shell, and the state:
//! the combo box's count and selection (not its labels: `populateCategoryBox`'s `"GUI:%s"` of a struct is undefined
//! upstream), the list's rows and selection, the two static texts, the entry's text, secret text, character position,
//! enabled flag, hilite and input function, the globals and the layout's hidden flag. A case whose Rust run meets an
//! undefined step (`GameWindowManager::undefined`: `getCharAt(-1)` of an emptied entry) is cut before it and counted.
//! The focus and the tab moves are the window manager's (tested by `differential_wm`), not compared.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! 0 mismatches over 20,000 cases; under 1 % cut (an emptied entry's last character read). Planted bugs, each caught:
//! 9561 (the category combo box starts on information), 9562 (releasing alt with control leaves `Alt+`), 9563 (`Shift+`
//! before alt or control), 9564 (Escape's release ignored), 9565 (a command does not enable the entry), 9566 (*Reset
//! All* keeps shift), 9567 (a category leaves the entry enabled), 9568 (the same character replaced again), 9569
//! (backspace keeps alt), 9570 (no `GGM_LEFT_DRAG`).
//!
//! # Results (2026-11-12, `KEYBOARD_OPTIONS_CASES=20000`, the default)
//!
//! 20,000 cases, 377,807 operations (3,583 cases ended by a pop: *Back* or Escape), **0 mismatches**; 168 cases (0.8 %)
//! cut at `doKeyUp`'s `getCharAt(-1)` of an emptied entry. As predicted. The miss was the wrapper's build: glibc has no
//! `iswascii`; the stand-in tests `c < 0x80`, MSVC's definition.
//!
//! Planted bugs: **10 of 10 caught**, mismatching cases: 9561 20,000, 9562 59, 9563 640, 9564 1,577, 9565 1,377, 9566
//! 1,317, 9567 9,026, 9568 82, 9569 1,664, 9570 2,750.

use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::gadget_combo_box as combo;
use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::gadget_text_entry::entry_rt;
use rts_lockstep_engine::client::keyboard_options_menu::{self as kom, KeyboardOptionsHost, KeyboardOptionsMenu};
use rts_lockstep_engine::client::meta_event::{MetaMap, MetaMapRec, KEY_NAMES};
use rts_lockstep_engine::client::shell::{self, KeyedFiles, LayoutFiles, NameKeys, Shell, ShellEnv, ShellHost, ShellRequest, WndFiles};
use rts_lockstep_engine::client::window_manager::{ggm, gwm, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData, WIN_STATE_HILITED};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WndHost};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::ffi::CString;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

const F: &str = "KeyboardOptionsMenu.wnd";

/// The synthetic `Menus/KeyboardOptionsMenu.wnd`: the parent with the screen's callbacks and its controls.
fn wnd(track: bool, filters: (bool, bool, bool)) -> Vec<u8> {
    let w = |kind: &str, name: &str, r: (i32, i32, i32, i32), style: &str, extra: &str| {
        format!(
            "  WINDOW\n    WINDOWTYPE = {kind};\n    SCREENRECT = UPPERLEFT: {} {}, BOTTOMRIGHT: {} {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"{F}:{name}\";\n    STATUS = ENABLED;\n    STYLE = {style};\n{extra}  END\n",
            r.0, r.1, r.2, r.3
        )
    };
    let mut c = String::new();
    c += &w("PUSHBUTTON", "ButtonBack", (20, 560, 110, 580), "PUSHBUTTON+MOUSETRACK", "    TEXT = \"GUI:Back\";\n");
    c += &w("COMBOBOX", "ComboBoxCategoryList", (20, 20, 220, 40), "COMBOBOX", "    COMBOBOXDATA = ISEDITABLE: 0, MAXCHARS: 16, MAXDISPLAY: 5, ASCIIONLY: 0, LETTERSANDNUMBERS: 0;\n");
    c += &w("SCROLLLISTBOX", "ListBoxCommandList", (20, 60, 380, 400), "SCROLLLISTBOX+MOUSETRACK", "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, COLUMNSWIDTH%: 100, FORCESELECT: 0;\n");
    c += &w("STATICTEXT", "StaticTextDescription", (400, 60, 780, 100), "STATICTEXT", "    STATICTEXTDATA = CENTERED: 0;\n");
    c += &w("STATICTEXT", "StaticTextCurrentHotkey", (400, 110, 780, 130), "STATICTEXT", "    STATICTEXTDATA = CENTERED: 0;\n");
    c += &w("PUSHBUTTON", "ButtonResetAll", (140, 560, 230, 580), "PUSHBUTTON+MOUSETRACK", "    TEXT = \"GUI:ResetAll\";\n");
    let style = if track { "ENTRYFIELD+MOUSETRACK" } else { "ENTRYFIELD" };
    c += &w(
        "ENTRYFIELD",
        "TextEntryAssignHotkey",
        (400, 140, 780, 160),
        style,
        &format!("    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: {}, ALPHANUMERICALONLY: {}, ASCIIONLY: {};\n", u8::from(filters.0), u8::from(filters.1), u8::from(filters.2)),
    );
    c += &w("PUSHBUTTON", "ButtonAssign", (260, 560, 350, 580), "PUSHBUTTON+MOUSETRACK", "    TEXT = \"GUI:Assign\";\n");
    format!(
        "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = KeyboardOptionsMenuInit;\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = KeyboardOptionsMenuShutdown;\nENDLAYOUTBLOCK\nWINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"{F}:ParentKeyboardOptionsMenu\";\n  STATUS = ENABLED+HIDDEN;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"KeyboardOptionsMenuSystem\";\n  INPUTCALLBACK = \"KeyboardOptionsMenuInput\";\n  CHILD\n{c}  ENDALLCHILDREN\nEND\n"
    )
    .into_bytes()
}

#[derive(Clone, Default)]
struct Files {
    wnd: Vec<u8>,
    text: BTreeMap<Vec<u8>, Vec<u16>>,
}

impl WndHost for Files {
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        unreachable!("the keys are the host's (KeyedFiles)")
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.text.get(label).cloned().unwrap_or_else(|| b"T:".iter().chain(label).map(|&b| u16::from(b)).collect())
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}

impl WndFiles for Files {
    fn open_wnd(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        Some(self.wnd.clone())
    }
}

/// The test's host: the shell, the files, the screen's statics, `TheMetaMap`; each message the screen is told logged.
struct Host {
    shell: Shell,
    files: Option<LayoutFiles<Files>>,
    keys: NameKeys,
    km: KeyboardOptionsMenu,
    meta: MetaMap,
    log: String,
    popped: bool,
}

fn short(wm: &GameWindowManager, w: Option<WinId>) -> String {
    w.map_or("-".to_string(), |w| String::from_utf8_lossy(wm.tree.windows[w].name()).trim_start_matches("KeyboardOptionsMenu.wnd:").to_string())
}

fn msg_name(m: u32) -> Option<&'static str> {
    Some(match m {
        ggm::GBM_SELECTED => "GBM_SELECTED",
        ggm::GLM_SELECTED => "GLM_SELECTED",
        ggm::GCM_SELECTED => "GCM_SELECTED",
        rts_lockstep_engine::client::gadget_text_entry::gem::EDIT_DONE => "GEM_EDIT_DONE",
        rts_lockstep_engine::client::gadget_text_entry::gem::UPDATE_TEXT => "GEM_UPDATE_TEXT",
        ggm::GBM_MOUSE_ENTERING => "GBM_MOUSE_ENTERING",
        ggm::GBM_MOUSE_LEAVING => "GBM_MOUSE_LEAVING",
        ggm::LEFT_DRAG => "GGM_LEFT_DRAG",
        _ => return None,
    })
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func != b"KeyboardOptionsMenuSystem" {
            return MsgHandled::Ignored;
        }
        if let Some(n) = msg_name(msg) {
            self.log += &format!("sys {n} {}\n", short(wm, rts_lockstep_engine::client::window_manager::data_win(d1)));
        }
        kom::keyboard_options_menu_system(wm, self, win, msg, d1, d2)
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        match func {
            b"KeyboardOptionsMenuInput" => kom::keyboard_options_menu_input(wm, self, win, msg, d1, d2),
            b"KeyboardTextEntryInput" => kom::keyboard_text_entry_input(wm, self, win, msg, d1, d2),
            _ => MsgHandled::Ignored,
        }
    }
    fn layout(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize) {
        self.layout_callback(wm, func, layout, None);
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        let mut f = self.files.take()?;
        let mut k = KeyedFiles { files: &mut f.files, keys: self.keys.clone() };
        let r = shell::win_create_layout_with(&mut f.script, &mut k, wm, self, filename);
        self.files = Some(f);
        r
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize, _user: Option<bool>) {
        match func {
            b"KeyboardOptionsMenuInit" => kom::keyboard_options_menu_init(wm, self, layout),
            b"KeyboardOptionsMenuShutdown" => {
                self.log += "pop\n";
                self.popped = true;
                kom::keyboard_options_menu_shutdown(wm, self, layout)
            }
            _ => {}
        }
    }
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.name_to_key(name)
    }
}

impl KeyboardOptionsHost for Host {
    fn keyboard_options(&mut self) -> &mut KeyboardOptionsMenu {
        &mut self.km
    }
    fn meta_map(&self) -> &MetaMap {
        &self.meta
    }
    fn keyboard_text(&self, label: &[u8]) -> Vec<u16> {
        self.files.as_ref().map_or_else(Vec::new, |f| f.files.fetch_text(label))
    }
}

/// Text as the wrapper prints it: printable ASCII as itself, the rest (and `\`) as `\uXXXX`.
fn narrow(t: &[u16]) -> String {
    t.iter().map(|&c| if (0x20..0x7F).contains(&c) && c != u16::from(b'\\') { char::from(c as u8).to_string() } else { format!("\\u{c:04x}") }).collect()
}
fn wide_hex(t: &[u16]) -> String {
    if t.is_empty() {
        return "-".into();
    }
    t.iter().map(|c| format!("{c:04x}")).collect()
}
fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn ustr(t: &[u16]) -> UnicodeString {
    let mut s = UnicodeString::new();
    for &c in t {
        let _ = s.concat_char(c);
    }
    s
}

fn find(wm: &GameWindowManager, n: &str) -> WinId {
    wm.tree.find_by_name(format!("{F}:{n}").as_bytes()).expect("window")
}

fn dump(wm: &mut GameWindowManager, h: &mut Host) -> String {
    let c = find(wm, "ComboBoxCategoryList");
    let l = find(wm, "ListBoxCommandList");
    let mut o = format!("combo {} {}\n", combo::get_length(wm, c), combo::get_selected_pos(wm, h, c, -1));
    let n = lb::get_num_entries(wm, l);
    o += &format!("list {n} {}", lb::get_selected(wm, h, l).first().copied().unwrap_or(-1));
    for r in 0..n {
        o += &format!(" [{}]", narrow(&lb::get_text(wm, h, l, r, 0)));
    }
    o += "\n";
    let d = find(wm, "StaticTextDescription");
    let k = find(wm, "StaticTextCurrentHotkey");
    o += &format!("desc [{}]\nhot [{}]\n", narrow(&gadget::static_text_get_text(wm, d)), narrow(&gadget::static_text_get_text(wm, k)));
    let e = find(wm, "TextEntryAssignHotkey");
    let rt = entry_rt(wm, e).expect("the entry's data");
    let input = matches!(&wm.ext[e].input, rts_lockstep_engine::client::window_manager::InputFunc::Host(n) if n == b"KeyboardTextEntryInput");
    o += &format!(
        "entry [{}] [{}] {} {} {} {}\n",
        narrow(&rt.text.text),
        narrow(&rt.s_text.text),
        rt.char_pos,
        u8::from(wm.tree.windows[e].status & win_status::ENABLED != 0),
        u8::from(wm.tree.windows[e].inst.state & WIN_STATE_HILITED != 0),
        u8::from(input)
    );
    let km = &h.km;
    let layout_hidden = wm.layouts.iter().any(|ly| ly.hidden);
    o += &format!(
        "flags {} {} {} {} [{}] [{}] [{}] {}\n",
        u8::from(km.shift_down),
        u8::from(km.alt_down),
        u8::from(km.ctrl_down),
        u8::from(km.absolute),
        narrow(&km.alt),
        narrow(&km.ctrl),
        narrow(&km.shift),
        u8::from(layout_hidden)
    );
    o
}

const KEY_CODES: [u32; 22] = [0x1D, 0x1D, 0x2A, 0x36, 0x38, 0x38, 0x0E, 0xD3, 0xD3, 0x0F, 0xD0, 0xCD, 0xC8, 0xCB, 0x01, 0x3B, 0x58, 0x3A, 0xC9, 0x1E, 0x9D, 0xB8];

fn case(r: &mut Rng) -> (String, String, Option<&'static str>) {
    // the game's text
    let mut text: BTreeMap<Vec<u8>, Vec<u16>> = BTreeMap::new();
    let mods = ["KEYBOARD:Alt+", "KEYBOARD:Ctrl+", "KEYBOARD:Shift+"];
    let usual = ["Alt+", "Ctrl+", "Shift+"];
    for (i, m) in mods.iter().enumerate() {
        match r.below(14) {
            0 => {}
            1 => {
                text.insert(m.as_bytes().to_vec(), Vec::new());
            }
            2 => {
                text.insert(m.as_bytes().to_vec(), u(usual[(i + 1) % 3]));
            }
            3 => {
                text.insert(m.as_bytes().to_vec(), u("Alt\u{12B}"));
            }
            _ => {
                text.insert(m.as_bytes().to_vec(), u(usual[i]));
            }
        }
    }
    if r.chance(3) {
        text.insert(b"GUI:NULL".to_vec(), u("(none)"));
    }
    // TheMetaMap
    let mut meta = MetaMap::default();
    let names = ["Attack", "Move", "", "Stop", "Guard", "Scatter", "Attack", "Select All", "\u{e9}t\u{e9}"];
    for _ in 0..r.below(31) {
        let mut rec = MetaMapRec::new(Default::default());
        rec.category = r.below(9) as i32;
        rec.key = if r.chance(8) { r.below(300) as i32 } else { KEY_NAMES[r.below(KEY_NAMES.len() as u64) as usize].value };
        let n = names[r.below(names.len() as u64) as usize];
        rec.display_name = ustr(&u(&if r.chance(3) { format!("{n}{}", r.below(5)) } else { n.to_string() }));
        rec.description = ustr(&u(&format!("does {}", r.below(100))));
        meta.recs.push(rec);
    }
    let track = r.below(3) != 0;
    let filters = (r.chance(8), r.chance(8), r.chance(6));
    let mut km = KeyboardOptionsMenu { shift_down: r.chance(4), alt_down: r.chance(4), ctrl_down: r.chance(4), absolute: r.chance(4), ..Default::default() };
    if r.chance(2) {
        km.alt = u("Alt+");
        km.ctrl = u("Ctrl+");
        km.shift = u("Shift+");
    }
    // the header
    let mut head = String::new();
    for (k, v) in &text {
        head += &format!("text {} {}\n", String::from_utf8_lossy(k), wide_hex(v));
    }
    for rec in &meta.recs {
        head += &format!("rec {} {} {} {}\n", rec.category, rec.key, wide_hex(rec.display_name.str()), wide_hex(rec.description.str()));
    }
    head += &format!("entry {} {} {} {}\n", u8::from(filters.0), u8::from(filters.1), u8::from(filters.2), u8::from(track));
    head += &format!(
        "globals {} {} {} {} {} {} {}\n",
        u8::from(km.shift_down),
        u8::from(km.alt_down),
        u8::from(km.ctrl_down),
        u8::from(km.absolute),
        wide_hex(&km.alt),
        wide_hex(&km.ctrl),
        wide_hex(&km.shift)
    );
    // the operations
    let mut ops = vec!["init".to_string()];
    for _ in 0..1 + r.below(40) {
        ops.push(match r.below(30) {
            0 | 1 => format!("cat {}", r.below(14) as i32 - 1),
            2 | 3 => format!("pick {}", r.below(33) as i32 - 1),
            4 => format!("button {}", if r.chance(6) { 0 } else { 1 + r.below(2) }),
            5 => format!("menukey {} {}", if r.chance(2) { 1 } else { KEY_CODES[r.below(22) as usize] }, if r.chance(4) { 1 } else { 2 }),
            6..=17 => {
                let k = KEY_CODES[r.below(22) as usize];
                let st = [1, 2, 2, 3, 0][r.below(5) as usize];
                format!("key {k} {st}")
            }
            18..=25 => {
                let ch: u32 = match r.below(10) {
                    0 => 0x0D,
                    1 => 0,
                    2 => u32::from(b'+'),
                    3 => 0x12B,
                    4 => 0x80 + r.below(0x200) as u32,
                    5 => u32::from(b'0') + r.below(10) as u32,
                    _ => u32::from(b'a') + r.below(26) as u32,
                };
                format!("ime {ch}")
            }
            26 | 27 => format!("mouse {}", r.below(5)),
            28 => format!("focus {}", r.below(2)),
            _ => {
                if r.chance(2) {
                    "update".to_string()
                } else {
                    format!("sys {}", r.below(3))
                }
            }
        });
    }
    // the Rust run, cut at an undefined step
    let files = Files { wnd: wnd(track, filters), text };
    let mut h = Host { shell: Shell::with_display_width(800), files: Some(LayoutFiles::new(files)), keys: NameKeys::default(), km, meta, log: String::new(), popped: false };
    let mut wm = GameWindowManager::new((800, 600));
    let mut o = String::new();
    let mut ran = 0;
    let mut undefined = None;
    for op in &ops {
        let mut w = op.split(' ');
        let cmd = w.next().expect("op");
        let a: i64 = w.next().map_or(0, |x| x.parse().expect("number"));
        let b: i64 = w.next().map_or(0, |x| x.parse().expect("number"));
        let mut line = format!("> {op}\n");
        let mut ret = -1i32;
        let mut want = None;
        let r2 = |m: MsgHandled| if m == MsgHandled::Handled { 1 } else { 0 };
        if cmd == "init" {
            shell::push(&mut wm, &mut h, b"Menus/KeyboardOptionsMenu.wnd", false);
        } else {
            let parent = find(&wm, "ParentKeyboardOptionsMenu");
            let entry = find(&wm, "TextEntryAssignHotkey");
            match cmd {
                "cat" => {
                    let c = find(&wm, "ComboBoxCategoryList");
                    combo::set_selected_pos(&mut wm, &mut h, c, a as i32, false);
                }
                "pick" => {
                    let l = find(&wm, "ListBoxCommandList");
                    lb::set_selected(&mut wm, &mut h, l, &[a as i32]);
                }
                "button" => {
                    let b = find(&wm, ["ButtonBack", "ButtonResetAll", "ButtonAssign"][a as usize]);
                    let id = wm.tree.windows[b].id;
                    ret = r2(wm.win_send_system_msg(&mut h, Some(parent), ggm::GBM_SELECTED, win_data(Some(b)), id as u32));
                }
                "menukey" => ret = r2(wm.win_send_input_msg(&mut h, Some(parent), gwm::CHAR, a as u32, b as u32)),
                "key" => ret = r2(wm.win_send_input_msg(&mut h, Some(entry), gwm::CHAR, a as u32, b as u32)),
                "ime" => ret = r2(wm.win_send_input_msg(&mut h, Some(entry), gwm::IME_CHAR, a as u32, 0)),
                "mouse" => {
                    let m = [gwm::LEFT_DOWN, gwm::MOUSE_ENTERING, gwm::MOUSE_LEAVING, gwm::LEFT_DRAG, gwm::RIGHT_DOWN][a as usize];
                    ret = r2(wm.win_send_input_msg(&mut h, Some(entry), m, 0, 0));
                }
                "focus" => {
                    wm.focus_reply = false;
                    ret = r2(kom::keyboard_options_menu_system(&mut wm, &mut h, parent, gwm::INPUT_FOCUS, a as u32, 0));
                    want = Some(wm.focus_reply);
                }
                "sys" => {
                    let m = [gwm::CREATE, gwm::DESTROY, gwm::RIGHT_DOWN][a as usize];
                    ret = r2(kom::keyboard_options_menu_system(&mut wm, &mut h, parent, m, 0, 0));
                }
                "update" => {
                    let l = wm.layouts.len() - 1;
                    kom::keyboard_options_menu_update(&mut wm, &mut h, l);
                }
                _ => unreachable!(),
            }
        }
        if wm.undefined.is_some() {
            undefined = wm.undefined;
            break;
        }
        line += &std::mem::take(&mut h.log);
        if let Some(wf) = want {
            line += &format!("want {}\n", u8::from(wf));
        }
        line += &format!("= {ret}\n");
        o += &line;
        ran += 1;
        if h.popped {
            o += "popped\n";
            break;
        }
        o += &dump(&mut wm, &mut h);
    }
    let text = head + &ops[..ran].iter().map(|o| format!("{o}\n")).collect::<String>();
    (text, o, undefined)
}

fn cpp_run(text: &str) -> String {
    let c = CString::new(text).expect("no NUL");
    let mut out = vec![0u8; 1 << 22];
    let n = unsafe { cpp::ref_keyboard_options_run(c.as_ptr(), out.as_mut_ptr() as *mut _, out.len() as i32) };
    assert!((n as usize) <= out.len(), "the dump overflowed");
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

#[test]
fn the_keyboard_options_screen_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("KEYBOARD_OPTIONS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x4b65_7942_6f61_7264);
    let (mut mismatches, mut undefined, mut ops, mut popped) = (0usize, 0usize, 0usize, 0usize);
    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    for n in 0..cases {
        let (text, rust, cut) = case(&mut r);
        if let Some(k) = cut {
            undefined += 1;
            *kinds.entry(k).or_insert(0usize) += 1;
        }
        popped += usize::from(rust.contains("popped"));
        ops += rust.matches("\n> ").count() + 1;
        let c = cpp_run(&text);
        if c != rust {
            mismatches += 1;
            if mismatches <= 2 {
                let at = rust.lines().zip(c.lines()).position(|(x, y)| x != y).unwrap_or(0);
                println!("case {n}:\n{text}\n  first difference at line {at}:\n  rust: {:?}\n  c++:  {:?}\n  context: {:?}", rust.lines().nth(at), c.lines().nth(at), rust.lines().take(at).collect::<Vec<_>>().join(" | "));
            }
        }
    }
    println!("#160 keyboard options: {cases} cases, {ops} operations, {popped} popped; {undefined} cut at an undefined step ({kinds:?}); mismatches: {mismatches}");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
