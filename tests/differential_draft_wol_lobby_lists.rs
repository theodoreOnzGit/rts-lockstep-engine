// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol7) of the lobby draft's window-bound list functions in `src/drafts/ui/wol_lobby_menu.rs`: `player_tooltip`
//! (`playerTooltip`), `populate_group_room_listbox` (`populateGroupRoomListbox`), `insert_player_in_listbox` (`insertPlayerInListbox`, with `lookup_small_rank_image`) and
//! `populate_lobby_player_listbox` (`PopulateLobbyPlayerListbox`) against upstream `WOLLobbyMenu.cpp`, cut verbatim by build.rs (`drafts_wol7`; eight edits, listed in
//! `differential/wrap_draft_wol7_lobby.cpp`) into the program `WOL7_LOBBY_EXE`, which also holds upstream's real window manager and gadgets (list box, combo box). The GameSpy
//! layer is a stand-in on both sides, over the same case state (`tests/support_wol6/lobby_host.rs` `Gs`; the C++ wrapper's `@ w7` operations).
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 2,000; fixed seed). Each is a scene of windows: 0 to 2 user-window decoys, the player list (a multi-select list box of 2 columns and 4 to 20 rows,
//! at an ordinary place or, one time in six, at x 40,000 so that a mouse x of 16 bits is negative as `i16`), a second list box, 0 to 2 combo boxes. The state: a local name, the
//! current and QM group rooms, the rank table (ascending, `INT_MAX` as the sentinel), the declared images (rank images `<Rank>-<USA|CHA|GLA|N>` and `OfficersClubsmall`, widths
//! 1 to 20), the player templates, 0 to 12 players (names of a small pool that collides case-insensitively, with bytes above 0x7F; flags, profile ids, wins, locale texts that are
//! numbers, negative ones, text and empty, rank points on the table's boundaries, sides -2 to 15, pre-order), buddies, VIPs, ignored ids and names, colours. 8 to 30 operations:
//! the state changed, `populateGroupRoomListbox` on a combo box or NULL, `PopulateLobbyPlayerListbox` (with a selection and a top row set before), `insertPlayerInListbox` with a
//! player of its own, `playerTooltip` at a mouse position over, near and outside the list, rows added directly (a name with a unit of low byte 0 among them), the list's selection,
//! its top row, its reset, `listboxLobbyPlayers` switched. After each operation both programs print the stand-in calls (`find`, `tip`), the window dump (every list row's cells), the
//! combo boxes' and list boxes' item data and `listboxLobbyPlayers`; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour or a deliberate guard of the draft): `listboxLobbyPlayers` NULL at `insertPlayerInListbox`; a single-select player list at
//! `PopulateLobbyPlayerListbox` (`(Int *)(&selectedIndices)` then holds the selected row as an address; the game's WND is not available here, so a multi-select list is the only case
//! that is defined); `playerTooltip` over a name that is not in the player map (`it->second` of `end()`); `rankPoints == INT_MAX` (reads past the table); a name with an embedded NUL;
//! locale texts whose number overflows `int`. `PEER_FLAG_OP` is the draft's value (the GameSpy SDK is not in the tree), a stand-in on both sides.
//!
//! # Prediction (before the first run, 2026-11-22)
//!
//! From reading upstream: three draft defects. (1) `insertPlayerInListbox`: `uStr.translate(info.m_name)` sign-extends a byte above 0x7F (`(WideChar)(char)`), the draft zero-extends.
//! (2) `playerTooltip`: `"WOL:Locale%2.2d"` of a negative number is `-05`, Rust's `{:02}` gives `-5`; and the empty tooltip's width is the default 1.0, the draft passes -1.0.
//! (3) `playerTooltip`: `aName.translate(uName)` drops a unit whose low byte is 0, the draft keeps a NUL (the name then misses the map). Everything else: 0 mismatches expected.
//!
//! # Planted bugs (`L9_PLANT=<id>`), results, see below the DIFF line of the run.

mod support;
mod support_wol6;
mod ui4_common;

use rts_lockstep_engine::client::gadget_combo_box as cbx;
use rts_lockstep_engine::client::gadget_list_box as lbx;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GameFont, WinInstanceData};
use rts_lockstep_engine::drafts::ui::wol_lobby_menu as lob;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use support_wol6::lobby_host::LobbyHost;
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

impl HasLog for LobbyHost {
    fn log(&mut self) -> &mut String {
        &mut self.scene_log
    }
}

const RANKS: [&str; 10] = ["Private", "Corporal", "Sergeant", "Lieutenant", "Captain", "Major", "Colonel", "General", "Brigadier", "Commander"];
const SIDES: [&str; 4] = ["USA", "CHA", "GLA", "N"];
const NAMES: [&[u8]; 10] = [b"Alpha", b"alpha", b"ALPHA", b"Bravo", b"Charlie", b"d\xE9lta", b"D\xC9LTA", b"e", b"Echo", b"echo"];
const LOCALES: [&str; 10] = ["5", "12", "-5", "-12", "007", "abc", "0", "99", "3x", "-"];

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

fn units(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

struct Scene {
    ops: Vec<String>,
    players_list: usize,
    other_list: usize,
    combos: Vec<usize>,
    /// the player list's absolute position and size
    rect: (i32, i32, i32, i32),
}

fn gen_scene(g: &mut Gen) -> Scene {
    let en = win_status::ENABLED;
    let mut ops = Vec::new();
    let decoys = g.below(3) as usize;
    for k in 0..decoys {
        ops.push(format!("@ wu -1 0 {} 0 300 200 {en} {}", k * 5, gws::USER_WINDOW));
    }
    let mut next = decoys;
    let far = g.below(6) == 0;
    let (x, y) = if far { (40_000, 20) } else { (10 + g.below(100) as i32, 10 + g.below(100) as i32) };
    let len = if g.below(8) == 0 { 4 } else { 14 + g.below(7) };
    let w = 100 + g.below(100) as i32;
    let h = 60 + g.below(200) as i32;
    ops.push(format!("W -1 6 {x} {y} {w} {h} {en} {} 0 {len} 2 0 0 0 1 0 0 30 70", gws::SCROLL_LISTBOX));
    let players_list = next;
    next += 1;
    ops.push(format!("W -1 6 300 300 150 150 {en} {} 0 {len} 2 0 0 0 1 0 0 30 70", gws::SCROLL_LISTBOX));
    let other_list = next;
    next += 1;
    let mut combos = Vec::new();
    for _ in 0..g.below(3) {
        ops.push(format!("W -1 4 {} 50 120 20 {en} {} 0 0 5 64 0 0 6 1 0", 400 + 10 * combos.len() as i32, gws::COMBO_BOX));
        combos.push(next);
        // a combo box makes its seven children after itself (the drop-down button, the entry, the list box, its two buttons, slider and thumb)
        next += 8;
    }
    Scene { ops, players_list, other_list, combos, rect: (x, y, w, h) }
}

struct State {
    names: Vec<Vec<u8>>,
    table: [i32; 14],
    images: Vec<String>,
}

fn gen_state(g: &mut Gen, st: &mut State, ops: &mut Vec<String>) {
    ops.push(format!("@ w7 me {}", bytes_hex(pick(g, &NAMES))));
    ops.push(format!("@ w7 cur {}", g.below(5)));
    ops.push(format!("@ w7 qm {}", g.below(5)));
    let mut table = [0i32; 14];
    let mut v = 0i32;
    for slot in table.iter_mut().take(13).skip(1) {
        v += 1 + g.below(40) as i32;
        *slot = v;
    }
    table[13] = i32::MAX;
    st.table = table;
    let t: Vec<String> = table.iter().map(|x| x.to_string()).collect();
    ops.push(format!("@ w7 rk {}", t.join(" ")));
    for rk in RANKS {
        for s in SIDES {
            if g.below(6) != 0 {
                let n = format!("{rk}-{s}");
                ops.push(format!("@ w7 img {} {}", bytes_hex(n.as_bytes()), 1 + g.below(20)));
                st.images.push(n);
            }
        }
    }
    if g.below(3) != 0 {
        ops.push(format!("@ w7 img {} {}", bytes_hex(b"OfficersClubsmall"), 1 + g.below(20)));
    }
    for k in 0..g.below(14) {
        let s: &[u8] = pick(g, &[b"USA".as_slice(), b"China", b"GLA", b"Boss", b"S\xE9de", b"\xFF\xFEx"]);
        ops.push(format!("@ w7 tpl {k} {}", bytes_hex(s)));
    }
    for i in 0..g.below(6) {
        let name: String = format!("Room{i}");
        ops.push(format!("@ w7 room {} {}", 1 + g.below(6), hex_of(&name.encode_utf16().collect::<Vec<_>>())));
    }
    for _ in 0..g.below(13) {
        ops.push(player_op(g, st, "pl"));
    }
    for _ in 0..g.below(4) {
        ops.push(format!("@ w7 bud {}", g.below(8)));
        ops.push(format!("@ w7 vip {}", g.below(8)));
        ops.push(format!("@ w7 pre {}", g.below(8)));
        ops.push(format!("@ w7 ign {}", g.below(8)));
        ops.push(format!("@ w7 ignn {}", bytes_hex(pick(g, &NAMES))));
    }
    for i in [1, 2, 6, 7, 8, 10] {
        ops.push(format!("@ w7 col {i} {}", g.i32()));
    }
}

fn hex_of(t: &[u16]) -> String {
    if t.is_empty() {
        return "-".to_string();
    }
    t.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn player_op(g: &mut Gen, st: &mut State, kind: &str) -> String {
    // a player that `ins` makes has a name of the map (a tooltip over its row looks the name up: upstream dereferences `end()` otherwise)
    let name = if kind == "ins" && !st.names.is_empty() {
        pick(g, &st.names)
    } else if g.below(5) == 0 {
        format!("P{}", g.below(100)).into_bytes()
    } else {
        pick(g, &NAMES).to_vec()
    };
    if kind == "pl" {
        st.names.push(name.clone());
    }
    let rank = match g.below(5) {
        0 => 0,
        1 => -(g.below(20) as i32),
        2 => st.table[1 + g.below(12) as usize],
        3 => st.table[1 + g.below(12) as usize] - 1,
        _ => g.below(600) as i32,
    };
    let locale = pick(g, &LOCALES);
    format!(
        "@ w7 {kind} {} {} {} {} {} {} {} {} {}",
        bytes_hex(&name),
        g.below(8),
        g.below(4),
        g.below(1000),
        g.below(1000),
        bytes_hex(locale.as_bytes()),
        rank,
        g.below(18) as i32 - 2,
        g.below(3)
    )
}

fn gen_op(g: &mut Gen, sc: &Scene, st: &mut State, lbp: &mut Option<usize>, lbp_rows: &mut bool) -> Vec<String> {
    let mut o = Vec::new();
    match g.below(20) {
        0 | 1 => {
            let c = if sc.combos.is_empty() || g.below(6) == 0 { -1 } else { pick(g, &sc.combos) as i64 };
            o.push(format!("@ w7 popgr {c}"));
        }
        2..=5 => {
            o.push("@ w7 poppl".to_string());
            *lbp_rows = lbp.is_some();
        }
        6 | 7 => {
            let l = lbp.unwrap_or(sc.players_list);
            if lbp.is_some() && !st.names.is_empty() {
                let p = player_op(g, st, "ins");
                o.push(p.replacen("@ w7 ins ", &format!("@ w7 ins {} ", g.i32()), 1));
                *lbp_rows = true;
            } else if lbp.is_some() {
            } else {
                let _ = l;
                o.push(format!("@ w7 lbp {}", sc.players_list));
                *lbp = Some(sc.players_list);
            }
        }
        8..=10 if *lbp_rows => {
            let l = lbp.expect("lbp");
            let (rx, ry, rw, rh) = sc.rect;
            let (x, y) = if l == sc.players_list {
                (rx + g.below(rw as u32 + 20) as i32 - 10, ry + g.below(rh as u32 + 20) as i32 - 10)
            } else {
                (300 + g.below(100) as i32 - 10, 300 + g.below(100) as i32 - 10)
            };
            if x >= 0 && y >= 0 {
                o.push(format!("@ w7 tip {l} {}", ((y as u32 & 0xFFFF) << 16) | (x as u32 & 0xFFFF)));
            }
        }
        11 | 12 => {
            let l = if g.below(4) == 0 { sc.other_list } else { lbp.unwrap_or(sc.players_list) };
            // a name that is in the map, with a unit of low byte 0 appended now and then (the C++ translate drops it)
            if st.names.is_empty() {
                return o; // (a row whose name is not in the map would make a tooltip look up `end()`: upstream's undefined behaviour)
            }
            let mut name: Vec<u16> = pick(g, &st.names).iter().map(|&b| b as i8 as i16 as u16).collect();
            if g.below(3) == 0 {
                name.push(0x100 * (1 + g.below(3) as u16));
            }
            o.push(format!("@ w7 la {l} {} {} -1 1", hex_of(&name), g.i32()));
            if Some(l) == *lbp {
                *lbp_rows = true;
            }
        }
        13 | 14 => {
            let l = if g.below(3) == 0 { sc.other_list } else { lbp.unwrap_or(sc.players_list) };
            let n = g.below(4);
            let rows: Vec<String> = (0..n).map(|_| g.below(14).to_string()).collect();
            o.push(format!("@ w7 lsel {l} {n} {}", rows.join(" ")));
        }
        15 => {
            let l = lbp.unwrap_or(sc.players_list);
            o.push(format!("@ w7 ltop {l} {}", g.below(10)));
        }
        16 => {
            let l = lbp.unwrap_or(sc.players_list);
            o.push(format!("@ w7 lres {l}"));
            *lbp_rows = false;
        }
        17 => {
            let v = if g.below(5) == 0 { None } else if g.below(2) == 0 { Some(sc.players_list) } else { Some(sc.other_list) };
            o.push(format!("@ w7 lbp {}", v.map_or(-1, |v| v as i64)));
            *lbp = v;
            *lbp_rows = false;
        }
        18 if g.below(3) == 0 => o.push(format!("@ w7 lone {}", pick(g, &[sc.players_list, sc.other_list, 0]))),
        18 => o.push(player_op(g, st, "pl")),
        19 => {
            let k = pick(g, &["bud", "vip", "pre", "ign"]);
            o.push(format!("@ w7 {k} {}", g.below(8)));
        }
        _ => {}
    }
    o
}

fn state_dump(wm: &mut GameWindowManager, h: &mut LobbyHost) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let _ = writeln!(o, "w7 lbp {}", idx(h.menu.listbox_lobby_players));
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
            let _ = write!(o, " {}", lbx::get_item_data(wm, h, i, r, 0));
        }
        o.push('\n');
    }
    o
}

fn combo_create(wm: &mut GameWindowManager, h: &mut LobbyHost, op: &str) -> bool {
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

fn player_of(h: &mut LobbyHost, it: &mut std::str::Split<'_, char>) -> lob::LobbyPlayerInfo {
    let mut next = || it.next().expect("field");
    let name = unhex(next());
    let profile_id: i32 = next().parse().unwrap();
    let flags: i32 = next().parse().unwrap();
    let wins: i32 = next().parse().unwrap();
    let losses: i32 = next().parse().unwrap();
    let locale = unhex(next());
    let rank_points: i32 = next().parse().unwrap();
    let side: i32 = next().parse().unwrap();
    let preorder: i32 = next().parse().unwrap();
    let _ = h;
    lob::LobbyPlayerInfo { name, profile_id, flags, wins, losses, locale, rank_points, side, preorder: preorder != 0, is_ignored: false }
}

fn menu_op(wm: &mut GameWindowManager, h: &mut LobbyHost, op: &str) {
    let mut it = op.split(' ');
    assert_eq!(it.next(), Some("@"));
    assert_eq!(it.next(), Some("w7"));
    let f = it.next().expect("op");
    let num = |it: &mut std::str::Split<'_, char>| -> i64 { it.next().expect("arg").parse().expect("number") };
    match f {
        "me" => h.gs.local_name = unhex(it.next().unwrap()),
        "cur" => h.gs.current_room = num(&mut it) as i32,
        "qm" => h.gs.qm = num(&mut it) as i32,
        "rk" => {
            for i in 0..14 {
                h.rank_values[i] = num(&mut it) as i32;
            }
        }
        "img" => {
            let n = unhex(it.next().unwrap());
            let w = num(&mut it) as i32;
            h.gs.image_widths.insert(n.clone(), w);
            if !h.images.contains(&n) {
                h.images.push(n);
            }
        }
        "tpl" => {
            let k = num(&mut it) as usize;
            let s = unhex(it.next().unwrap());
            if h.gs.templates.len() <= k {
                h.gs.templates.resize(k + 1, Vec::new());
            }
            h.gs.templates[k] = s;
        }
        "room" => {
            let id = num(&mut it) as i32;
            let t = units(it.next().unwrap());
            h.gs.rooms.insert(id, lob::LobbyGroupRoom { group_id: id, translated_name: t });
        }
        "pl" => {
            let p = player_of(h, &mut it);
            h.gs.players.insert(p.name.to_ascii_lowercase(), p);
        }
        "bud" => {
            h.gs.buddies.insert(num(&mut it) as i32);
        }
        "vip" => {
            h.gs.vip.insert(num(&mut it) as i32);
        }
        "pre" => {
            h.gs.preorder.insert(num(&mut it) as i32);
        }
        "ign" => {
            h.gs.saved_ignored.insert(num(&mut it) as i32);
        }
        "ignn" => {
            h.gs.ignored_names.insert(unhex(it.next().unwrap()));
        }
        "col" => {
            let i = num(&mut it) as usize;
            h.gs.colors[i] = num(&mut it) as i32;
        }
        "lbp" => {
            let a = num(&mut it);
            h.menu.listbox_lobby_players = (a >= 0).then_some(a as usize);
        }
        "popgr" => {
            let a = num(&mut it);
            lob::populate_group_room_listbox(wm, h, (a >= 0).then_some(a as usize));
        }
        "poppl" => lob::populate_lobby_player_listbox(wm, h),
        "ins" => {
            let c = num(&mut it) as i32;
            let p = player_of(h, &mut it);
            let p = h.gs.with_ignored(&p);
            let r = lob::insert_player_in_listbox(wm, h, &p, c);
            h.line(format!("inserted {r}"));
        }
        "tip" => {
            let w = num(&mut it) as usize;
            let m = num(&mut it) as u32;
            lob::player_tooltip(wm, h, w, m);
        }
        "la" => {
            let w = num(&mut it) as usize;
            let t = units(it.next().unwrap());
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
        "ltop" => {
            let w = num(&mut it) as usize;
            let p = num(&mut it) as i32;
            lbx::set_top_visible_entry(wm, h, w, p);
        }
        "lone" => {
            let w = num(&mut it);
            wm.win_set_lone_window(h, (w >= 0).then_some(w as usize));
        }
        "lres" => {
            let w = num(&mut it) as usize;
            lbx::reset(wm, h, w);
        }
        other => panic!("@ w7 {other}"),
    }
}

#[test]
fn lobby_list_functions_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_77_01);
    let mut chk = Check::new("WOLLobbyLists");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(b"W7Sys", b"W7Inp")];
    for case in 0..cases(2_000) as u64 {
        let sc = gen_scene(&mut g);
        let mut ops = sc.ops.clone();
        let mut st = State { names: Vec::new(), table: [0; 14], images: Vec::new() };
        gen_state(&mut g, &mut st, &mut ops);
        let mut lbp: Option<usize> = None;
        let mut rows = false;
        // the lobby's list is set before the first use, as Init does
        ops.push(format!("@ w7 lbp {}", sc.players_list));
        lbp = lbp.or(Some(sc.players_list));
        for _ in 0..8 + g.below(23) {
            let o = gen_op(&mut g, &sc, &mut st, &mut lbp, &mut rows);
            ops.extend(o);
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = LobbyHost::new();
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
                // upstream's undefined behaviour in the verified gadgets: this operation and the rest are not run (the C++ may crash)
                ops.truncate(i);
                break;
            }
            let mut log = std::mem::take(&mut *h.log.borrow_mut());
            log.push_str(&std::mem::take(&mut h.scene_log));
            let st = state_dump(&mut wm, &mut h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        if std::env::var("WOL7_DUMP").ok().and_then(|v| v.parse::<u64>().ok()) == Some(case) {
            let _ = std::fs::write("/tmp/claude-0/-home-user/3cefbd62-dbac-57be-8b5b-f778561c06c9/scratchpad/w7_case.txt", ops.join("\n") + "\n");
        }
        compare_case(env!("WOL7_LOBBY_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
