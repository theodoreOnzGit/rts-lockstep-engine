// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol7) of the quick match draft's combo box populate functions in `src/drafts/ui/wol_quick_match_menu.rs`:
//! `populate_qm_color_combo_box` (`populateQMColorComboBox`) and `populate_qm_side_combo_box` (`populateQMSideComboBox`) against upstream `WOLQuickMatchMenu.cpp`, cut verbatim by
//! build.rs (`drafts_wol7`, no edit) into the program `WOL7_QM_EXE` (`differential/wrap_draft_wol7_qm.cpp` lists the stand-ins), which also holds upstream's real window manager and
//! gadgets (combo box, list box). The multiplayer settings, the player template store, the challenge generals and the ladder are stand-ins over the same case state on both sides.
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 3,000; fixed seed). Each is a scene of 0 to 2 decoys and the two combo boxes (colour, side). The state: the random colour, 0 to 8 colour definitions with
//! holes (NULL), a preferred colour 0 to 10 (it can be past the entries), 0 to 8 player templates (a name of its own, a side of a small pool that repeats and has bytes above 0x7F, a starting
//! building or none, a challenge general that is absent, locked or enabled). 4 to 14 operations: the state changed, the colour populate, the side populate with a favourite side -2 to 9
//! and no ladder or a ladder (random factions or not, a list of valid factions). After each operation both programs print the window dump (the two combo boxes, their entries, texts,
//! selection and enabled state) and every list box's item data; the texts must be identical.
//! Excluded: either static combo box NULL (`GadgetComboBoxReset(NULL)` is not defined here; the draft returns), `getColor(PLAYERTEMPLATE_RANDOM)` NULL.
//!
//! # Prediction (before the first run, 2026-11-22)
//!
//! From reading: one draft defect, `format!("SIDE:{}", String::from_utf8_lossy(side))` for a side with a byte above 0x7F (upstream's `%s` keeps the bytes; fixed before the first run).
//! Everything else: 0 mismatches.
//!
//! # Planted bugs (`L9_PLANT=<id>`): see the table in the commit message and the run record below the DIFF line.

mod support;
mod support_wol7;
mod ui4_common;

use rts_lockstep_engine::client::gadget_combo_box as cbx;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GameFont, WinInstanceData};
use rts_lockstep_engine::drafts::ui::wol_quick_match_menu as qm;
use std::sync::Mutex;
use support::{Check, Gen};
use support_wol7::qm_host::QmHost;
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

impl HasLog for QmHost {
    fn log(&mut self) -> &mut String {
        &mut self.scene_log
    }
}

const SIDES: [&[u8]; 6] = [b"USA", b"China", b"GLA", b"Boss", b"S\xE9de", b"\xFF\xFEx"];

fn bytes_hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".to_string();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(t: &str) -> Vec<u8> {
    if t == "-" {
        return Vec::new();
    }
    (0..t.len() / 2).map(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).expect("hex")).collect()
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

fn gen_scene(g: &mut Gen) -> Vec<String> {
    let en = win_status::ENABLED;
    let mut ops = Vec::new();
    let decoys = g.below(3) as usize;
    for k in 0..decoys {
        ops.push(format!("@ wu -1 0 {} 0 300 200 {en} {}", k * 5, gws::USER_WINDOW));
    }
    // the colour combo box at `decoys` and its seven children (the drop-down button, the entry, the list box with its two buttons, its slider and thumb), the side combo box at `decoys + 8`
    for k in 0..2 {
        ops.push(format!("W -1 4 {} 50 120 20 {en} {} 0 0 5 64 0 0 20 1 0", 400 + 10 * k, gws::COMBO_BOX));
    }
    ops.push(format!("@ w7 qmwin {} {}", decoys, decoys + 8));
    ops
}

fn gen_state(g: &mut Gen) -> Vec<String> {
    let mut o = vec![format!("@ w7 qmrand {}", g.i32())];
    let n = g.below(9);
    o.push(format!("@ w7 qmcols {n}"));
    for i in 0..n {
        if g.below(4) != 0 {
            o.push(format!("@ w7 qmcol {i} {} {}", g.i32(), bytes_hex(format!("Color{i}").as_bytes())));
        }
    }
    o.push(format!("@ w7 qmpref {}", g.below(11)));
    for k in 0..g.below(9) {
        let side = pick(g, &SIDES);
        o.push(format!("@ w7 qmtpl {k} {} {} {} {}", bytes_hex(format!("T{k}").as_bytes()), bytes_hex(side), u8::from(g.below(4) == 0), g.below(3) as i32 - 1));
    }
    o
}

fn gen_op(g: &mut Gen) -> Vec<String> {
    match g.below(10) {
        0..=2 => vec!["@ w7 popc".to_string()],
        3..=6 => {
            let fav = g.below(12) as i32 - 2;
            if g.below(2) == 0 {
                vec![format!("@ w7 pops {fav} -")]
            } else {
                let n = g.below(5);
                let sides: Vec<String> = (0..n).map(|_| bytes_hex(&pick(g, &SIDES))).collect();
                vec![format!("@ w7 pops {fav} {} {n} {}", g.below(2), sides.join(" "))]
            }
        }
        7 => gen_state(g),
        8 => vec![format!("@ w7 qmpref {}", g.below(11))],
        _ => vec![format!("@ w7 qmrand {}", g.i32())],
    }
}

fn state_dump(wm: &mut GameWindowManager, h: &mut QmHost) -> String {
    use std::fmt::Write as _;
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    for i in 0..wm.tree.windows.len() {
        if wm.ext[i].freed {
            continue;
        }
        let end = match &wm.ext[i].user {
            UserData::Listbox(l) => l.end_pos,
            _ => continue,
        };
        let _ = write!(o, "w7 items {i}");
        for r in 0..i32::from(end) {
            let _ = write!(o, " {}", rts_lockstep_engine::client::gadget_list_box::get_item_data(wm, h, i, r, 0));
        }
        o.push('\n');
    }
    o
}

fn combo_create(wm: &mut GameWindowManager, h: &mut QmHost, op: &str) -> bool {
    let v: Vec<&str> = op.split(' ').collect();
    if v[0] != "W" || v[2] != "4" {
        return false;
    }
    let n: Vec<i64> = v[1..].iter().filter(|s| s.parse::<i64>().is_ok()).map(|s| s.parse().unwrap()).collect();
    let (parent, x, y, ww, hh) = (n[0] as i32, n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32);
    let (status, style, visual) = (n[6] as u32, n[7] as u32, n[8] != 0);
    let f: Vec<i32> = n[9..17].iter().map(|&x| x as i32).collect();
    let d = ComboBoxData { is_editable: f[0] != 0, max_display: f[1], max_chars: f[2], ascii_only: f[3] != 0, letters_and_numbers_only: f[4] != 0 };
    let mut t = cbx::combo_template(&d);
    t.listbox.list_length = f[5] as i16;
    t.listbox.scroll_bar = f[6] != 0;
    let mut inst = WinInstanceData { style, ..WinInstanceData::new() };
    inst.font = (f[7] != 0).then(|| GameFont { name: b"F".to_vec(), point_size: f[7], bold: false });
    let p = (parent >= 0).then_some(parent as usize);
    cbx::gogo_gadget_combo_box(wm, h, p, status, x, y, ww, hh, inst, t, visual);
    true
}

fn menu_op(wm: &mut GameWindowManager, h: &mut QmHost, op: &str) {
    let mut it = op.split(' ');
    assert_eq!(it.next(), Some("@"));
    assert_eq!(it.next(), Some("w7"));
    let f = it.next().expect("op");
    let num = |it: &mut std::str::Split<'_, char>| -> i64 { it.next().expect("arg").parse().expect("number") };
    match f {
        "qmwin" => {
            h.menu.combo_box_color = Some(num(&mut it) as usize);
            h.menu.combo_box_side = Some(num(&mut it) as usize);
        }
        "qmrand" => h.random_color = num(&mut it) as i32,
        "qmcols" => h.colors = vec![None; num(&mut it) as usize],
        "qmcol" => {
            let i = num(&mut it) as usize;
            let c = num(&mut it) as i32;
            let t = unhex(it.next().unwrap());
            if i < h.colors.len() {
                h.colors[i] = Some((c, t));
            }
        }
        "qmpref" => h.pref_color = num(&mut it) as i32,
        "qmtpl" => {
            let k = num(&mut it) as usize;
            let name = unhex(it.next().unwrap());
            let side = unhex(it.next().unwrap());
            let empty = num(&mut it) != 0;
            let gen = num(&mut it);
            if h.templates.len() <= k {
                h.templates.resize(k + 1, qm::QmPlayerTemplate::default());
            }
            h.templates[k] = qm::QmPlayerTemplate { name, side, starting_building_empty: empty, general_starting_enabled: (gen >= 0).then_some(gen != 0) };
        }
        "popc" => qm::populate_qm_color_combo_box(wm, h),
        "pops" => {
            let fav = num(&mut it) as i32;
            let first = it.next().unwrap();
            if first == "-" {
                qm::populate_qm_side_combo_box(wm, h, fav, None);
            } else {
                let random_factions = first.parse::<i32>().unwrap() != 0;
                let n = num(&mut it) as usize;
                let valid: Vec<Vec<u8>> = (0..n).map(|_| unhex(it.next().unwrap())).collect();
                let li = qm::QmLadderInfo { random_factions, valid_factions: valid, ..Default::default() };
                qm::populate_qm_side_combo_box(wm, h, fav, Some(&li));
            }
        }
        other => panic!("@ w7 {other}"),
    }
}

#[test]
fn qm_combo_populates_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_77_03);
    let mut chk = Check::new("WOLQmCombos");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(b"W7Sys", b"W7Inp")];
    for case in 0..cases(3_000) as u64 {
        let mut ops = gen_scene(&mut g);
        ops.extend(gen_state(&mut g));
        for _ in 0..4 + g.below(11) {
            ops.extend(gen_op(&mut g));
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = QmHost::default();
        let mut keys = KeyTable::default();
        let mut chunks = Vec::new();
        for (i, op) in ops.clone().iter().enumerate() {
            if op.starts_with("@ w7") {
                menu_op(&mut wm, &mut h, op);
            } else if !combo_create(&mut wm, &mut h, op) {
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() {
                ops.truncate(i);
                break;
            }
            let log = std::mem::take(&mut h.scene_log);
            let st = state_dump(&mut wm, &mut h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        compare_case(env!("WOL7_QM_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
