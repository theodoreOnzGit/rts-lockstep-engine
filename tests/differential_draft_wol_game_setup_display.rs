// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol8; #232) of the staging room's display functions in `src/drafts/ui/wol_game_setup_menu.rs`: `wol_display_game_options`
//! (`WOLDisplayGameOptions`, with `handlePlayerTemplateSelection` which it calls), `wol_display_slot_list` (`WOLDisplaySlotList`) and `wol_position_start_spots`
//! (`WOLPositionStartSpots`) against upstream `WOLGameSetupMenu.cpp`, cut VERBATIM by build.rs (`drafts_wol8`: setup_pos.inc, setup_tpl.inc, setup_disp.inc, no edit) into the program
//! `WOL8_SETUP_EXE`, which holds upstream's real window manager and gadgets (check box, combo box, list box, static text). The staging room, the GameSpy layer, the GUI helpers
//! (`UpdateSlotList`, `PopulatePlayerTemplateComboBox`), the map preview (`positionStartSpots`, `updateMapStartSpots`) and the peer queue are stand-ins on both sides over the same case
//! state (`differential/wrap_draft_wol8_setup.cpp`; `tests/support_wol6/game_setup_host.rs`); their calls are logged on both sides.
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 300; fixed seed; one run takes about 40 seconds). Each is a scene: a map name static text, three check boxes, a starting cash combo box, a map list box (rows with the map file
//! names as item data, one with none), eight ping windows (user windows), eight player template, colour and team combo boxes with entries and item data, two decoy user windows; the
//! state: a staging room (map names with backslashes and bytes above 0x7F, a map-cache entry for it or not, the local slot -1 to 7, in game or not, the use-stats, old-factions,
//! observers, superweapon (-1 to 2) and starting cash values, host or not), eight slots (template, start position, has-map, human, ping), the ping string and cut-offs (some equal to a
//! ping), the ping images (some missing), the map-select layout on or off. 8 to 20 operations: `opts`, `slots`, `pos`, the room's fields changed, the check boxes set, the
//! combo selections, the list's selection. After each operation both programs print the stand-in calls and the window dump (every window's state, the check boxes, the tooltips, the
//! user windows' enabled images).
//!
//! Excluded inputs (upstream dereferences NULL, or a deliberate guard of the draft): the check boxes, the starting cash combo, the map static text, the player template combos and the
//! colour and team combos unbound; no staging room at `handlePlayerTemplateSelection`'s combo (its selection -1: upstream reads item data of index -1).
//!
//! # Prediction (before the first run, 2026-11-22)
//!
//! #232 defect 1: a map name with a byte above 0x7F shows a different text (0x00E9 against 0xFFE9). Everything else: 0 mismatches.
//!
//! # Planted bugs (`L9_PLANT=<id>`), results below the DIFF line of the run (plants 25071 to 25082).

mod support;
mod support_wol6;
mod ui4_common;

use rts_lockstep_engine::client::gadget_combo_box as cbx;
use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GadgetData, GameFont, WinInstanceData};
use rts_lockstep_engine::drafts::ui::wol_game_setup_menu as gsm;
use rts_lockstep_engine::drafts::ui::wol_game_setup_menu::{StagingRoomInfo, StagingSlotInfo};
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use support_wol6::game_setup_host::GameSetupHost;
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

impl HasLog for GameSetupHost {
    fn log(&mut self) -> &mut String {
        &mut self.scene_log
    }
}

const MAPS: [&[u8]; 6] = [b"maps\\Alpha\\Alpha.map", b"maps\\d\xE9lta\\d\xE9lta.map", b"plain.map", b"\\", b"x\\", b"maps\\\xFF\\\xFE\xFDz"];

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

fn units_hex(t: &[u16]) -> String {
    if t.is_empty() {
        return "-".to_string();
    }
    t.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn units(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

struct Scene {
    ops: Vec<String>,
    map_list: usize,
    cash: usize,
    tpl: Vec<usize>,
    color: Vec<usize>,
    team: Vec<usize>,
}

fn combo(en: u32, x: i32, y: i32) -> String {
    format!("W -1 4 {x} {y} 120 20 {en} {} 0 0 5 64 0 0 6 1 0", gws::COMBO_BOX)
}

fn gen_scene(g: &mut Gen) -> Scene {
    let en = win_status::ENABLED;
    let mut ops = Vec::new();
    let mut next = 0usize;
    // 0 the map name static text, 1..=3 the check boxes
    ops.push(format!("W -1 11 10 10 100 20 {en} {} 0", gws::STATIC_TEXT));
    next += 1;
    for k in 0..3 {
        ops.push(format!("W -1 2 {} 40 20 20 {en} {} 0", 10 + 30 * k, gws::CHECK_BOX));
        next += 1;
    }
    let map_list = next;
    ops.push(format!("W -1 6 300 10 150 100 {en} {} 0 8 2 0 0 0 0 0 0 30 70", gws::SCROLL_LISTBOX));
    next += 1;
    let cash = next;
    ops.push(combo(en, 10, 100));
    next += 8;
    let mut tpl = Vec::new();
    let mut color = Vec::new();
    let mut team = Vec::new();
    for i in 0..8 {
        ops.push(combo(en, 10, 130 + 20 * i));
        tpl.push(next);
        next += 8;
        ops.push(combo(en, 140, 130 + 20 * i));
        color.push(next);
        next += 8;
        ops.push(combo(en, 270, 130 + 20 * i));
        team.push(next);
        next += 8;
    }
    // the ping windows, then two decoys (the map preview window and the map select window)
    for i in 0..10 {
        ops.push(format!("@ wu -1 0 {} 400 20 10 {en} {}", 10 + 25 * i, gws::USER_WINDOW));
        next += 1;
    }
    let first_user = next - 10;
    let _ = first_user;
    ops.push(format!("@ nm {map_list} WOLMapSelectMenu.wnd:ListboxMap"));
    ops.push(format!("@ nm {} MapSelWin", next - 1));
    Scene { ops, map_list, cash, tpl, color, team }
}

/// Window numbers: 0 text, 1..=3 check boxes, `map_list`, `cash`, template/colour/team combos, then ten user windows (8 ping windows, the map preview, the map select window).
fn user(sc: &Scene, k: usize) -> usize {
    sc.team[7] + 8 + k
}

fn gen_state(g: &mut Gen, sc: &Scene, ops: &mut Vec<String>, cut: &mut (i32, i32)) {
    // images: Ping01..03 declared (one time in four missing)
    for n in ["Ping03", "Ping02", "Ping01"] {
        if g.below(4) != 0 {
            ops.push(format!("@ w7 img {} {}", bytes_hex(n.as_bytes()), 1 + g.below(20)));
        }
    }
    for (k, n) in ["Ping03", "Ping02", "Ping01"].iter().enumerate() {
        ops.push(format!("@ w8 pingimg {k} {}", bytes_hex(n.as_bytes())));
    }
    ops.push(format!("@ w8 bind parent 0 {}", if g.below(10) == 0 { -1 } else { 0 }));
    ops.push("@ w8 bind map 0 0".to_string());
    ops.push("@ w8 bind usestats 0 1".to_string());
    ops.push("@ w8 bind limitarmies 0 2".to_string());
    ops.push("@ w8 bind limitsuper 0 3".to_string());
    ops.push(format!("@ w8 bind cash 0 {}", sc.cash));
    ops.push(format!("@ w8 bind windowmap 0 {}", if g.below(5) == 0 { -1 } else { user(sc, 8) as i64 }));
    ops.push(format!("@ w8 mapselid {}", user(sc, 9)));
    for i in 0..8 {
        ops.push(format!("@ w8 bind ping {i} {}", if g.below(6) == 0 { -1 } else { user(sc, i) as i64 }));
        ops.push(format!("@ w8 bind tpl {i} {}", sc.tpl[i]));
        ops.push(format!("@ w8 bind color {i} {}", sc.color[i]));
        ops.push(format!("@ w8 bind team {i} {}", sc.team[i]));
    }
    // the combos' entries and item data
    let cash_values: Vec<u32> = (0..2 + g.below(4)).map(|_| 1000 * g.below(4)).collect();
    for (k, v) in cash_values.iter().enumerate() {
        ops.push(format!("@ w8 cbadd {} 41 0", sc.cash));
        ops.push(format!("@ w8 cbdata {} {k} {v}", sc.cash));
    }
    for i in 0..8 {
        let n = 2 + g.below(3);
        for k in 0..n {
            ops.push(format!("@ w8 cbadd {} 41 0", sc.tpl[i]));
            ops.push(format!("@ w8 cbdata {} {k} {}", sc.tpl[i], g.below(5) as i32 - 2));
        }
        for c in [sc.color[i], sc.team[i]] {
            for k in 0..2 {
                ops.push(format!("@ w8 cbadd {c} 41 0"));
                ops.push(format!("@ w8 cbdata {c} {k} {k}"));
            }
        }
        ops.push(format!("@ w8 cbsel {} {}", sc.tpl[i], g.below(n)));
    }
    // the map list: rows with a file name as item data, one without
    for r in 0..2 + g.below(3) {
        ops.push(format!("@ w7 la {} {:x} 0 -1 0", sc.map_list, 0x41 + r));
        ops.push(format!("@ w8 maprow {} {r} {}", sc.map_list, if g.below(4) == 0 { "-".to_string() } else { bytes_hex(&pick(g, &MAPS)) }));
    }
    for _ in 0..g.below(3) {
        if g.below(2) == 0 {
            ops.push(format!("@ w7 lsel {} 1 {}", sc.map_list, g.below(3)));
        }
    }
    ops.push(format!("@ w8 mapsel {}", g.below(2)));
    let (good, bad) = (40 + g.below(20) as i32, 100 + g.below(20) as i32);
    ops.push(format!("@ w8 cutoff {good} {bad}"));
    ops.push(format!("@ w8 pingstr {}", ping_near(g, good, bad)));
    gen_room(g, ops, &cash_values);
    for i in 0..8 {
        ops.push(slot_op(g, i, good, bad));
    }
    *cut = (good, bad);
}

/// A ping on, next to or away from the cut-offs (the comparisons `<` against `<=` need the exact tie).
fn ping_near(g: &mut Gen, good: i32, bad: i32) -> i32 {
    let v = [good - 1, good, good, good + 1, bad - 1, bad, bad, bad + 1, 0, 130];
    pick(g, &v)
}

fn gen_room(g: &mut Gen, ops: &mut Vec<String>, cash_values: &[u32]) {
    let map = pick(g, &MAPS);
    if g.below(2) == 0 {
        ops.push(format!("@ w8 mapmd {} {}", bytes_hex(&map), units_hex(&[0x44, 0x1234, 0x45])));
    }
    let cash = if g.below(3) == 0 { 7777 } else { pick(g, cash_values) };
    ops.push(format!(
        "@ w8 room {} {} {} {} {} {} {} {} {} {}",
        u8::from(g.below(12) != 0),
        bytes_hex(&map),
        if g.below(8) == 0 { -1 } else { g.below(8) as i32 },
        u8::from(g.below(5) != 0),
        g.below(2),
        g.below(2),
        g.below(2),
        g.below(4) as i32 - 1,
        cash,
        g.below(2)
    ));
}

fn slot_op(g: &mut Gen, i: usize, good: i32, bad: i32) -> String {
    let human = g.below(4) != 0;
    format!("@ w8 slot {i} {} {} {} {} {} {}", g.below(6) as i32 - 2, g.below(9) as i32 - 1, g.below(2), u8::from(human), ping_near(g, good, bad), units_hex(&[0x41 + i as u16]))
}

fn gen_ops(g: &mut Gen, sc: &Scene, ops: &mut Vec<String>, cut: (i32, i32)) {
    for _ in 0..8 + g.below(13) {
        match g.below(10) {
            0 | 1 => ops.push("@ w8 opts".to_string()),
            2 | 3 => ops.push("@ w8 slots".to_string()),
            4 => ops.push("@ w8 pos".to_string()),
            5 => gen_room(g, ops, &[0, 1000, 2000, 7777]),
            6 => {
                let i = g.below(8) as usize;
                ops.push(slot_op(g, i, cut.0, cut.1));
            }
            7 => ops.push(format!("@ w8 chk {} {}", 1 + g.below(3), g.below(2))),
            8 => {
                let i = g.below(8) as usize;
                ops.push(format!("@ w8 cbsel {} {}", sc.tpl[i], g.below(2)));
            }
            _ => ops.push(format!("@ w7 lsel {} 1 {}", sc.map_list, g.below(3))),
        }
    }
}

fn state_dump(wm: &mut GameWindowManager) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    for i in 0..wm.tree.windows.len() {
        if wm.ext[i].freed {
            continue;
        }
        if wm.tree.windows[i].inst.style & gws::USER_WINDOW != 0 {
            let img = wm.tree.windows[i].inst.enabled_draw_data[0].image.as_ref().map_or("-".to_string(), |n| String::from_utf8_lossy(n).into_owned());
            let _ = writeln!(o, "w8 img {i} {img}");
        }
        if wm.tree.windows[i].inst.style & gws::CHECK_BOX != 0 {
            let _ = writeln!(o, "w8 chk {i} {}", u8::from(rts_lockstep_engine::client::gadget::check_box_is_checked(wm, i)));
        }
        if let Some(t) = &wm.tree.windows[i].inst.tooltip_text {
            let _ = write!(o, "w8 tt {i}");
            text(&mut o, t);
            o.push('\n');
        }
    }
    o
}

fn combo_create(wm: &mut GameWindowManager, h: &mut GameSetupHost, op: &str) -> bool {
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

/// A check box (`gogoGadgetCheckbox`), the one type `scene_op` does not make.
fn check_box_create(wm: &mut GameWindowManager, h: &mut GameSetupHost, op: &str) -> bool {
    let v: Vec<&str> = op.split(' ').collect();
    if v[0] != "W" || v[2] != "2" {
        return false;
    }
    let n: Vec<i64> = v[1..].iter().map(|s| s.parse().unwrap()).collect();
    let (x, y, ww, hh) = (n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32);
    let inst = WinInstanceData { style: n[7] as u32, ..WinInstanceData::new() };
    wm.gogo_gadget_look(h, None, n[6] as u32, x, y, ww, hh, inst, GadgetData::None, n[8] != 0);
    true
}

fn menu_op(wm: &mut GameWindowManager, h: &mut GameSetupHost, op: &str) {
    let mut it = op.split(' ');
    assert_eq!(it.next(), Some("@"));
    let ns = it.next().expect("ns");
    let f = it.next().expect("op");
    let num = |it: &mut std::str::Split<'_, char>| -> i64 { it.next().expect("arg").parse().expect("number") };
    let win = |v: i64| -> Option<usize> { (v >= 0).then_some(v as usize) };
    if ns == "w7" {
        match f {
            "img" => {
                let n = unhex(it.next().unwrap());
                h.w8_images.insert(n);
            }
            "la" => {
                let w = num(&mut it) as usize;
                let t = vec![u16::from_str_radix(it.next().unwrap(), 16).unwrap()];
                let (c, r, k) = (num(&mut it) as i32, num(&mut it) as i32, num(&mut it) as i32);
                let ret = lbx::add_entry_text(wm, h, w, &t, c, r, k, true);
                h.line(format!("added {ret}"));
            }
            "lsel" => {
                let w = num(&mut it) as usize;
                let n = num(&mut it) as usize;
                let rows: Vec<i32> = (0..n).map(|_| num(&mut it) as i32).collect();
                lbx::set_selected(wm, h, w, &rows);
            }
            other => panic!("@ w7 {other}"),
        }
        return;
    }
    assert_eq!(ns, "w8");
    match f {
        "bind" => {
            let role = it.next().unwrap();
            let i = num(&mut it) as usize;
            let w = win(num(&mut it));
            let m = &mut h.menu;
            match role {
                "parent" => m.parent_wol_game_setup = w,
                "map" => m.text_entry_map_display = w,
                "usestats" => m.check_box_use_stats = w,
                "limitarmies" => m.check_box_limit_armies = w,
                "limitsuper" => m.check_box_limit_superweapons = w,
                "cash" => m.combo_box_starting_cash = w,
                "windowmap" => m.window_map = w,
                "ping" => m.generic_ping_window[i] = w,
                "tpl" => m.combo_box_player_template[i] = w,
                "color" => m.combo_box_color[i] = w,
                "team" => m.combo_box_team[i] = w,
                other => panic!("role {other}"),
            }
        }
        "room" => {
            let exists = num(&mut it) != 0;
            let map = unhex(it.next().unwrap());
            let (local, ingame, stats, old, obs, sup, cash, host) = (num(&mut it) as i32, num(&mut it) != 0, num(&mut it) != 0, num(&mut it) != 0, num(&mut it) != 0, num(&mut it) as i32, num(&mut it) as u32, num(&mut it) != 0);
            h.w8.room_exists = exists;
            h.w8.am_i_host = host;
            h.w8.room = StagingRoomInfo { map, am_i_host: host, is_in_game: ingame, local_slot_num: local, map_crc: 0, map_size: 0, use_stats: stats, old_factions_only: old, allow_observers: obs, superweapon_restriction: sup, starting_cash: cash };
        }
        "slot" => {
            let i = num(&mut it) as usize;
            let (t, sp, hm, human, ping) = (num(&mut it) as i32, num(&mut it) as i32, num(&mut it) != 0, num(&mut it) != 0, num(&mut it) as i32);
            let name = units(it.next().unwrap());
            if h.w8.slots.len() <= i {
                h.w8.slots.resize(i + 1, StagingSlotInfo::default());
            }
            h.w8.slots[i] = StagingSlotInfo { start_pos: sp, player_template: t, has_map: hm, is_human: human, ping, name, ..Default::default() };
        }
        "pingstr" => h.w8.ping_string = it.next().unwrap().as_bytes().to_vec(),
        "cutoff" => {
            h.w8.good = num(&mut it) as i32;
            h.w8.bad = num(&mut it) as i32;
        }
        "mapmd" => {
            let n = unhex(it.next().unwrap());
            h.w8.maps.insert(n, (0, units(it.next().unwrap()), false));
        }
        "pingimg" => {
            let k = num(&mut it) as usize;
            let n = unhex(it.next().unwrap());
            h.menu.ping_images[k] = h.w8_images.contains(&n).then_some(n);
        }
        "mapsel" => h.w8.map_select_layout = num(&mut it) != 0,
        "mapselid" => h.menu.window_map_select_map_id = wm.tree.windows[num(&mut it) as usize].id,
        "maprow" => {
            let w = num(&mut it) as usize;
            let r = num(&mut it) as i32;
            let n = it.next().unwrap();
            if n == "-" {
                lbx::set_item_data(wm, h, w, 0, r, 0);
            } else {
                h.w8.map_names.push(unhex(n));
                let k = h.w8.map_names.len() as u32;
                lbx::set_item_data(wm, h, w, k, r, 0);
            }
        }
        "cbadd" => {
            let w = num(&mut it) as usize;
            let text = units(it.next().unwrap());
            let c = num(&mut it) as i32;
            let r = cbx::add_entry(wm, h, w, &text, c);
            h.line(format!("cbadd {r}"));
        }
        "cbdata" => {
            let (w, i, d) = (num(&mut it) as usize, num(&mut it) as i32, num(&mut it) as i32);
            cbx::set_item_data(wm, h, w, i, d as u32);
        }
        "cbsel" => {
            let (w, i) = (num(&mut it) as usize, num(&mut it) as i32);
            cbx::set_selected_pos(wm, h, w, i, true);
        }
        "chk" => {
            let (w, on) = (num(&mut it) as usize, num(&mut it) != 0);
            rts_lockstep_engine::client::gadget::check_box_set_checked(wm, h, w, on);
        }
        "opts" => gsm::wol_display_game_options(wm, h),
        "slots" => gsm::wol_display_slot_list(wm, h),
        "pos" => gsm::wol_position_start_spots(wm, h),
        other => panic!("@ w8 {other}"),
    }
}

#[test]
fn game_setup_display_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_88_02);
    let mut chk = Check::new("WOLGameSetupDisplay");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(b"W8Sys", b"W8Inp")];
    for case in 0..cases(300) as u64 {
        let sc = gen_scene(&mut g);
        let mut ops = sc.ops.clone();
        let mut cut = (0, 0);
        gen_state(&mut g, &sc, &mut ops, &mut cut);
        gen_ops(&mut g, &sc, &mut ops, cut);
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = GameSetupHost::new();
        h.w8.keys_on = true;
        let mut keys = KeyTable::default();
        let mut chunks = Vec::new();
        for (i, op) in ops.clone().iter().enumerate() {
            if op.starts_with("@ w7") || op.starts_with("@ w8") {
                menu_op(&mut wm, &mut h, op);
            } else if op.starts_with("@ nm ") {
                let v: Vec<&str> = op.split(' ').collect();
                let k = rts_lockstep_engine::client::shell::ShellHost::name_to_key(&mut h, v[3].as_bytes());
                wm.tree.windows[v[2].parse::<usize>().unwrap()].id = k as i32;
            } else if !combo_create(&mut wm, &mut h, op) && !check_box_create(&mut wm, &mut h, op) {
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() {
                ops.truncate(i);
                break;
            }
            let mut log = std::mem::take(&mut *h.log.borrow_mut());
            log.push_str(&std::mem::take(&mut h.scene_log));
            let st = state_dump(&mut wm);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        compare_case(env!("WOL8_SETUP_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
