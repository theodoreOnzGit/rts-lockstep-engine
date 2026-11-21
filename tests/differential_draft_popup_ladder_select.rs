// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui5) of the draft `src/drafts/ui/popup_ladder_select.rs` (`populateLadderComboBox`, `populateLadderListBox`, `handleLadderSelection`,
//! `setPasswordMode`, `PopupLadderSelectInit`, `Input`, `System`, `ladderSelectedCallback`, `updateLadderDetails`) against upstream `PopupLadderSelect.cpp`, cut verbatim by build.rs
//! (`drafts_ui5`; four `line.format(...)` calls edited, see differential/wrap_draft_ui5_ladder.cpp) into the program `UI5_LADDER_EXE`, which also holds upstream's real window manager
//! and gadgets (list box, text entry, static text, push button).
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 3,000: one run is under a minute; fixed seed). Each is a scene of named windows (`PopupLadderSelect.wnd:...`): the parent (menu callbacks), 0 to 2 decoys,
//! the two list boxes (the list box of ladders and the details), the ladder name's static text, OK and Cancel, the password parent (a user window with the menu's callbacks) holding its OK and
//! Cancel buttons and the password entry, the bad password parent holding its OK button, each present with probability 5/6 (the list box of ladders always: upstream reads the selection
//! of a NULL one as an uninitialised `Int`), under the parent (70 %), top level or under a decoy, a decoy copy of each name one time in three. The case's state: 0 to 4 ladders (names,
//! players per team, location, URL, description, an encrypted password or none, win limits, random factions / maps, faction and map lists), the map cache's display names, the rows
//! `PopulateCustomLadderListBox` fills (item data: ladder indices, 0 and unknown ones) and the row it selects. 4 to 24 operations: Init, ladderSelectedCallback, the system callback
//! called directly (`GBM_SELECTED` with each control, `GLM_SELECTED`, `GLM_DOUBLE_CLICKED`, `GEM_EDIT_DONE`, `CREATE`, `DESTROY`, others), the input callback (Escape in each state), presses
//! and releases at the buttons, the list's selection set (`GLM_SELECTED` reaches the parent's callback through the real gadget), the password typed (a unit 0x100 among them), hiding,
//! enabling, focusing. After each operation both programs print the stand-in calls (the other menus' functions, the overlay), the messages, the dump of every window (the details list box
//! rows and the ladder name's text among them), the statics and the password mode; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour; the draft reports each through `PopupLadderSelectHost::undefined`): the list box of ladders missing (`GadgetListBoxGetSelected(NULL)`
//! leaves `selIndex` uninitialised); `buttonOk` missing at a double click; a NULL control as `mData1`; `GWM_INPUT_FOCUS` sent directly with `mData1` TRUE.
//! Not covered: `RCGameDetailsMenuInit`, `RCGameDetailsMenuSystem` and `closeRightClickMenu` (layouts made from WND files, the lone window and the staging rooms of the GameSpy layer; the
//! wrapper compiles them but they are not run). The formats of `updateLadderDetails` are not MSVC's `_vsnwprintf`: the wrapper joins the format and the arguments with `|`, so each argument is observed.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! One draft defect from reading upstream: the password's `pass.translate(...)` keeps a unit whose low byte is 0 as a NUL (fixed before the first run). Nothing else.
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!
//! see the draft's `plant(...)` calls (22741..22758).
//!
//! # Results
//!
//! see the issue comment on #215.

mod support;
mod ui4_common;

use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, GameFont, WinId};
use rts_lockstep_engine::drafts::ui::popup_ladder_select::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"PopupLadderSelectSystem";
const INP: &[u8] = b"PopupLadderSelectInput";
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Shell,
    pl: PopupLadderSelect,
    ub: bool,
    ladders: Vec<LadderInfoFull>,
    maps: BTreeMap<Vec<u8>, Vec<u16>>,
    rows: Vec<i32>,
    rows_selected: i32,
    rc: (i32, i32),
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            popup_ladder_select_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], _win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            popup_ladder_select_input(wm, self, msg, d1, d2)
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

fn joined(fmt: &[u16], tail: &str, mid: Option<&[u16]>) -> Vec<u16> {
    let mut v = fmt.to_vec();
    v.push('|' as u16);
    if let Some(m) = mid {
        v.extend_from_slice(m);
    }
    v.extend(tail.encode_utf16());
    v
}

impl PopupLadderSelectHost for Host {
    fn popup_ladder_select(&mut self) -> &mut PopupLadderSelect {
        &mut self.pl
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut t = b"T:".to_vec();
        t.extend_from_slice(label);
        t.iter().map(|&b| u16::from(b)).collect()
    }
    fn ladder_find_by_index(&self, index: i32) -> Option<LadderInfoFull> {
        if index == 0 {
            return None;
        }
        self.ladders.iter().find(|l| l.index == index).cloned()
    }
    fn populate_qm_ladder_combo_box(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("pop qm combo\n");
    }
    fn populate_custom_ladder_combo_box(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("pop custom combo\n");
    }
    fn populate_qm_ladder_list_box(&mut self, _wm: &mut GameWindowManager, win: Option<WinId>) {
        let _ = writeln!(self.log, "pop qm list {}", idx(win));
    }
    fn populate_custom_ladder_list_box(&mut self, wm: &mut GameWindowManager, win: Option<WinId>) {
        let _ = writeln!(self.log, "pop custom list {}", idx(win));
        let Some(w) = win else { return };
        lbx::reset(wm, self, w);
        for (i, id) in self.rows.clone().into_iter().enumerate() {
            let index = lbx::add_entry_text(wm, self, w, &[65 + i as u16], WHITE, -1, 0, false);
            lbx::set_item_data(wm, self, w, id as u32, index, 0);
        }
        let sel = self.rows_selected;
        lbx::set_selected(wm, self, w, &[sel]);
    }
    fn handle_qm_ladder_selection(&mut self, id: i32) {
        let _ = writeln!(self.log, "handle qm {id}");
    }
    fn handle_custom_ladder_selection(&mut self, id: i32) {
        let _ = writeln!(self.log, "handle custom {id}");
    }
    fn custom_match_hide_host_popup(&mut self, _wm: &mut GameWindowManager, hide: bool) {
        let _ = writeln!(self.log, "hide host popup {}", u8::from(hide));
    }
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>) {
        wm.win_set_modal(w);
    }
    fn gamespy_close_overlay_ladder_select(&mut self, _wm: &mut GameWindowManager) {
        self.log.push_str("overlay close 3\n");
    }
    fn encrypt_string(&self, pass: &[u8]) -> Vec<u8> {
        let mut v: Vec<u8> = pass.iter().rev().copied().collect();
        v.push(b'#');
        v
    }
    fn format_ladder_name_and_size(&self, fmt: &[u16], name: &[u16], a: i32, b: i32) -> Vec<u16> {
        joined(fmt, &format!("|{a}|{b}"), Some(name))
    }
    fn format_text(&self, fmt: &[u16], text: &[u16]) -> Vec<u16> {
        joined(fmt, "", Some(text))
    }
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16> {
        joined(fmt, &v.to_string(), None)
    }
    fn map_display_name(&self, map: &[u8]) -> Option<Vec<u16>> {
        self.maps.get(map).cloned()
    }
    fn ladder_for_staging_room(&self, _room_id: i32) -> Option<LadderInfoFull> {
        None
    }
    fn win_user_data_int(&self, _wm: &GameWindowManager, _w: WinId) -> i32 {
        0
    }
    fn win_set_user_data_int(&mut self, _wm: &mut GameWindowManager, _w: WinId, _v: i32) {}
    fn win_set_lone_window(&mut self, _wm: &mut GameWindowManager, _w: WinId) {}
    fn rc_game_details_ids(&mut self) -> &mut (i32, i32) {
        &mut self.rc
    }
    fn undefined(&mut self, _what: &str) {
        self.ub = true;
    }
}

// ---------------------------------------------------------------------------------------------------------------------

fn hx(s: &str) -> String {
    hex(&s.encode_utf16().collect::<Vec<u16>>())
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

struct Scene {
    ops: Vec<String>,
    named: Vec<(usize, String)>,
    /// (index, kind, x, y, w, h): 0 menu window, 1 ladder list, 2 details list, 3 name text, 4 OK, 5 Cancel, 6 password parent, 7 password OK, 8 password Cancel, 9 password entry, 10 bad parent, 11 bad OK
    wins: Vec<(usize, u8, i32, i32, i32, i32)>,
}

const NAMES: [(&str, u8, bool); 11] = [
    ("ListBoxLadderSelect", 1, false),
    ("ListBoxLadderDetails", 2, true),
    ("StaticTextLadderName", 3, true),
    ("ButtonOk", 4, true),
    ("ButtonCancel", 5, true),
    ("PasswordParent", 6, true),
    ("ButtonPasswordOk", 7, true),
    ("ButtonPasswordCancel", 8, true),
    ("PasswordEntry", 9, true),
    ("BadPasswordParent", 10, true),
    ("ButtonBadPasswordOk", 11, true),
];

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
    let mut pw_parent: Option<usize> = None;
    let mut bad_parent: Option<usize> = None;
    for (name, kind, optional) in NAMES {
        for copy in 0..2 {
            let present = if copy == 0 { !optional || g.below(6) != 0 } else { g.below(3) == 0 && decoys > 0 };
            if !present {
                continue;
            }
            let general = |g: &mut Gen| -> Option<usize> {
                match g.below(20) {
                    0..=13 if parent_present => Some(0),
                    0..=13 => None,
                    14..=16 => None,
                    _ if decoys > 0 => Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize]),
                    _ => None,
                }
            };
            let parent: Option<usize> = if copy == 1 {
                Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize])
            } else {
                match kind {
                    7 | 8 | 9 if pw_parent.is_some() && g.below(10) < 7 => pw_parent,
                    11 if bad_parent.is_some() && g.below(10) < 7 => bad_parent,
                    _ => general(g),
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
                6 | 10 => format!("@ wu {ps} 0 {x} {y} {w} {h} {status} {}", gws::USER_WINDOW),
                1 | 2 => format!("W {ps} 6 {x} {y} {w} {h} {status} {} 0 6 1 0 0 0 0 0 0", gws::SCROLL_LISTBOX),
                3 => format!("W {ps} 11 {x} {y} {w} {h} {status} {} 0", gws::STATIC_TEXT),
                9 => format!("W {ps} 9 {x} {y} {w} {h} {status} {} 0 {} 0 0 0 0 _", gws::ENTRY_FIELD, g.below(20) as i32 + 1),
                _ => format!("W {ps} 1 {x} {y} {w} {h} {status} {} 0", gws::PUSH_BUTTON),
            };
            ops.push(line);
            wins.push((next, kind, px + x, py + y, w, h));
            if copy == 0 && kind == 6 {
                pw_parent = Some(next);
            }
            if copy == 0 && kind == 10 {
                bad_parent = Some(next);
            }
            named.push((next, name.to_string()));
            next += 1;
        }
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} PopupLadderSelect.wnd:{name}"));
    }
    if parent_present {
        ops.push("@ nm 0 PopupLadderSelect.wnd:Parent".to_string());
    }
    scene_state(g, &mut ops);
    Scene { ops, named, wins }
}

fn scene_state(g: &mut Gen, ops: &mut Vec<String>) {
    let n = g.below(5);
    for k in 0..n {
        let pw = if g.below(2) == 0 { "-".to_string() } else { ["abc", "x", "Ab9"][g.below(3) as usize].chars().rev().collect::<String>() + "#" };
        let factions = if g.below(3) == 0 { "-".to_string() } else { "USA,China".to_string() };
        let maps = if g.below(3) == 0 { "-".to_string() } else { "m1,m2,m3".to_string() };
        let d = |g: &mut Gen, s: &str| if g.below(3) == 0 { "-".to_string() } else { hx(s) };
        ops.push(format!(
            "@ ll ladder {} {} {} {} {} {} {} {} {} {} {} {} {}",
            k + 1,
            hx(&format!("Lad{k}")),
            g.below(5),
            d(g, "Loc"),
            hx("http://x"),
            d(g, "Desc"),
            pw,
            g.below(3) * 5,
            g.below(3) * 7,
            g.below(2),
            g.below(2),
            factions,
            maps
        ));
    }
    for m in ["m1", "m2"] {
        if g.below(2) == 0 {
            ops.push(format!("@ ll map {m} {}", hx(&format!("Map {m}"))));
        }
    }
    let rows = g.below(5);
    let mut l = format!("@ ll rows {rows}");
    for _ in 0..rows {
        let _ = write!(l, " {}", [0i32, 1, 2, 3, 4, 9][g.below(6) as usize]);
    }
    let _ = write!(l, " {}", g.below(rows.max(1)) as i32 - i32::from(g.below(4) == 0));
    ops.push(l);
}

fn pw_texts() -> Vec<String> {
    let mut v: Vec<String> = ["abc", "cba#", "x", "Ab9", "zzz"].iter().map(|s| hx(s)).collect();
    v.push("-".into());
    v.push("61,100,62,63".into());
    v.push("63,62,100,61,23".into());
    v
}

fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let of = |k: u8| -> Vec<usize> { s.wins.iter().filter(|w| w.1 == k).map(|w| w.0).collect() };
    let lists = of(1);
    let entries = of(9);
    let clickable: Vec<(usize, u8, i32, i32, i32, i32)> = s.wins.iter().filter(|w| matches!(w.1, 4 | 5 | 7 | 8 | 11)).cloned().collect();
    let any = |g: &mut Gen| pick(g, &s.wins).0;
    let menu_win = |g: &mut Gen| pick(g, &s.wins.iter().filter(|w| w.1 == 0 || w.1 == 6 || w.1 == 10).map(|w| w.0).collect::<Vec<_>>());
    let by_name = |n: &str| s.named.iter().find(|(_, k)| k == n).map(|(i, _)| *i);
    if g.below(100) < 22 {
        // composites: the selection and the password set, then the press that reads them
        let win = menu_win(g);
        let (list, ok, pwok, pwe) = (by_name("ListBoxLadderSelect"), by_name("ButtonOk"), by_name("ButtonPasswordOk"), by_name("PasswordEntry"));
        if let (Some(list), Some(ok), Some(pwok), Some(pwe)) = (list, ok, pwok, pwe) {
            let t = pick(g, &pw_texts());
            return match g.below(4) {
                0 => format!("@ ll sel {list} {}\n@ sys 0 {win} {} 1 {ok} 0", g.below(5), ggm::GBM_SELECTED),
                1 => format!("@ ll init\n@ ll sel {list} {}\n@ sys 0 {win} {} 1 {ok} 0\n@ tx {pwe} {t}\n@ sys 0 {win} {} 1 {pwok} 0", g.below(5), ggm::GBM_SELECTED, ggm::GBM_SELECTED),
                2 => format!("@ tx {pwe} {t}\n@ sys 0 {win} {} 1 {pwok} 0", ggm::GBM_SELECTED),
                _ => format!("@ ll sel {list} {}\n@ sys 0 {win} {} 1 {list} {}", g.below(5), lbx::glm::DOUBLE_CLICKED, g.below(5)),
            };
        }
    }
    match g.below(100) {
        0..=13 => "@ ll init".into(),
        14..=17 => "@ ll ldcb".into(),
        18..=22 => format!("@ ll lbdata {} {}", pick(g, &s.wins.iter().filter(|w| w.1 == 1 || w.1 == 2).map(|w| w.0).collect::<Vec<_>>().into_iter().chain(std::iter::once(0)).collect::<Vec<_>>()), g.below(6)),
        23..=40 => {
            let msg = match g.below(10) {
                0..=3 => ggm::GBM_SELECTED,
                4 => lbx::glm::SELECTED,
                5 => lbx::glm::DOUBLE_CLICKED,
                6 => ggm::GEM_EDIT_DONE,
                _ => pick(g, &[1, 2, 3, 4, 21, 23, 24, 25]),
            };
            let win = menu_win(g);
            let ctl = msg == ggm::GBM_SELECTED || msg == lbx::glm::DOUBLE_CLICKED || msg == ggm::GEM_EDIT_DONE;
            let (kind, d1, d2) = if ctl { (1, any(g) as u32, g.below(5)) } else if msg == gwm::INPUT_FOCUS { (0, 2 + g.below(2), 0) } else { (0, g.below(3), g.below(3)) };
            format!("@ sys 0 {win} {msg} {kind} {d1} {d2}")
        }
        41..=50 => {
            let key = pick(g, &[1u32, 1, 1, 2, 28, 0x101, 0x201, 57]);
            let state = pick(g, &[1u32, 2, 3, 0, 0x101, 4]);
            let win = menu_win(g);
            let msg = if g.below(6) == 0 { pick(g, &[1u32, 2, 24, 25, 123]) } else { gwm::CHAR };
            format!("@ inp 0 {win} {msg} 0 {key} {state}")
        }
        51..=62 if !clickable.is_empty() => {
            let (_, _, x, y, ww, hh) = pick(g, &clickable);
            format!("M {} {} {} 0 0", if g.below(2) == 0 { gwm::LEFT_DOWN } else { gwm::LEFT_UP }, x + ww / 2, y + hh / 2)
        }
        63..=70 if !lists.is_empty() => format!("@ ll sel {} {}", lists[0], g.below(6) as i32 - 1),
        71..=76 if !entries.is_empty() => format!("@ tx {} {}", entries[0], pick(g, &pw_texts())),
        77..=79 if !entries.is_empty() => format!("I {} {}", entries[0], pick(g, &[u32::from(b'a'), 13, 0x100, 0xE9])),
        80..=84 => {
            let mut v = Vec::new();
            scene_state(g, &mut v);
            v.swap_remove(g.below(v.len() as u32) as usize)
        }
        85..=89 => format!("H {} {}", any(g), g.below(2)),
        90..=94 => format!("E {} {}", any(g), g.below(2)),
        _ => format!("F {}", any(g) as i32),
    }
}

fn state_dump(wm: &GameWindowManager, h: &Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let p = &h.pl;
    let mode = match p.current_mode {
        PasswordMode::PassNone => 0,
        PasswordMode::PassEntry => 1,
        PasswordMode::PassError => 2,
    };
    let _ = writeln!(
        o,
        "ll ids {} {} {} {} {} {} {} {} {} {} {} {} mode {mode} index {}",
        p.parent_id,
        p.listbox_ladder_select_id,
        p.listbox_ladder_details_id,
        p.static_text_ladder_name_id,
        p.button_ok_id,
        p.button_cancel_id,
        p.password_parent_id,
        p.button_password_ok_id,
        p.button_password_cancel_id,
        p.text_entry_password_id,
        p.bad_password_parent_id,
        p.button_bad_password_ok_id,
        p.ladder_index
    );
    let _ = writeln!(
        o,
        "ll wins {} {} {} {} {} {} {} {} {}",
        idx(p.parent),
        idx(p.listbox_ladder_select),
        idx(p.listbox_ladder_details),
        idx(p.static_text_ladder_name),
        idx(p.button_ok),
        idx(p.button_cancel),
        idx(p.password_parent),
        idx(p.text_entry_password),
        idx(p.bad_password_parent)
    );
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
            let r = if v[1] == "sys" { popup_ladder_select_system(wm, h, win, msg, a, d2) } else { popup_ladder_select_input(wm, h, msg, a, d2) };
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "ll" => match v[2] {
            "init" => popup_ladder_select_init(wm, h, 0),
            "ldcb" => ladder_selected_callback(wm, h),
            "lbdata" => {
                let d = lbx::get_item_data(wm, h, num(3) as usize, num(4) as i32, 0);
                let _ = writeln!(h.log, "lbdata {d}");
            }
            "sel" => lbx::set_selected(wm, h, num(3) as usize, &[num(4) as i32]),
            "ladder" => {
                let list = |t: &str| -> Vec<Vec<u8>> { if t == "-" { Vec::new() } else { t.split(',').map(|x| x.as_bytes().to_vec()).collect() } };
                h.ladders.push(LadderInfoFull {
                    index: num(3) as i32,
                    name: hexs(v[4]),
                    players_per_team: num(5) as i32,
                    location: hexs(v[6]),
                    homepage_url: hexs(v[7]),
                    description: hexs(v[8]),
                    crypted_password: if v[9] == "-" { Vec::new() } else { v[9].as_bytes().to_vec() },
                    min_wins: num(10) as i32,
                    max_wins: num(11) as i32,
                    random_factions: num(12) != 0,
                    random_maps: num(13) != 0,
                    valid_factions: list(v[14]),
                    valid_maps: list(v[15]),
                    ..Default::default()
                });
            }
            "map" => {
                h.maps.insert(v[3].as_bytes().to_vec(), hexs(v[4]));
            }
            "rows" => {
                let n = num(3) as usize;
                h.rows = (0..n).map(|i| num(4 + i) as i32).collect();
                h.rows_selected = num(4 + n) as i32;
            }
            other => panic!("ll {other}"),
        },
        other => panic!("@ {other}"),
    }
}

#[test]
fn popup_ladder_select_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C5_03);
    let mut chk = Check::new("PopupLadderSelect");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    for case in 0..cases(3_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 4 + g.below(21);
        let mut ops = scene.ops.clone();
        for _ in 0..steps {
            let o = gen_op(&mut g, &scene);
            ops.extend(o.split('\n').map(str::to_string));
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Host::default();
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            if op.starts_with("@ ll") || op.starts_with("@ sys") || op.starts_with("@ inp") {
                menu_op(&mut wm, &mut h, op);
            } else {
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
        compare_case(env!("UI5_LADDER_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
