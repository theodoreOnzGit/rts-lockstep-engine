// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol8; #232) of the lobby draft's right-click menu, `GLM_RIGHT_CLICKED` of `wol_lobby_menu_system`
//! (src/drafts/ui/wol_lobby_menu.rs) against upstream `WOLLobbyMenu.cpp:1719`, cut VERBATIM by build.rs (`drafts_wol8`, `lobby_rc.inc`, no edit) into the program
//! `WOL8_LOBBY_EXE`: group wol7's lobby program (upstream's real window manager and gadgets) extended with the stand-ins listed in
//! `differential/wrap_draft_wol8_rc_post.h`. The GameSpy layer (player map, buddies, local profile, staging rooms, ladders, the layouts `winCreateLayout` makes, `setUnignoreText`)
//! is a stand-in on both sides over the same case state (`tests/support_wol6/lobby_host.rs`, `@ w7` and `@ w8` operations); its calls are logged on both sides.
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 1,500; fixed seed). Each is a scene: a player list and a game list (multi-column list boxes, 14 to 20 rows), a third list box, 24 right-click
//! menu candidates (user windows 40 to 360 wide, some hidden at first; each layout takes fresh ones), and the state: players (names of a small pool that collides case-insensitively, with bytes above 0x7F;
//! profile ids -1 to 7), buddies, a local profile, staging rooms (some NULL), ladders (ip, port), 8 to 24 operations: rows added to the player list (a name with a unit of low byte 0
//! appended one time in three, the #232 case), rows of the game list with item data (0 among them), the selection, `@ lay` (1 or 2 of the menus) and `@ w8 rc <list> <pos> <x> <y>` on
//! the player list, the game list or the third list, at a row from -1 to the last, the mouse at, near and beyond the display's edge, and exactly where `x + width == 800`.
//! After each operation both programs print the stand-in calls (`layout`, `unignore`, `rcdata`, `ud`) and the window dump (positions, hidden state, list selections, the lone window).
//!
//! Excluded inputs: a layout of no window (`rcMenu` NULL: upstream dereferences it); `winCreateLayout` returning NULL (the C++ stand-in always makes one); a row index past the list's last row.
//!
//! # Prediction (before the first run, 2026-11-22)
//!
//! #232 defect 2 only: a player name whose appended unit has low byte 0 misses the map in the draft (profile 0, `RCNoProfileMenu`) and finds it in the C++. Everything else: 0 mismatches.
//!
//! # Planted bugs (`L9_PLANT=<id>`), results below the DIFF line of the run (plants 25061 to 25070).

mod support;
mod support_wol6;
mod ui4_common;

use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status};
use rts_lockstep_engine::drafts::ui::wol_lobby_menu as lob;
use rts_lockstep_engine::drafts::ui::wol_lobby_menu::LobbyRoomToJoin;
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

const NAMES: [&[u8]; 8] = [b"Alpha", b"alpha", b"Bravo", b"Charlie", b"d\xE9lta", b"D\xC9LTA", b"e", b"Echo"];

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
    t.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

struct Scene {
    ops: Vec<String>,
    players_list: usize,
    game_list: usize,
    third_list: usize,
    menus: Vec<usize>,
    /// the menus' sizes (the clamp's tie: x + width == 800)
    sizes: Vec<(i32, i32)>,
    rows: [i32; 3],
}

fn gen_scene(g: &mut Gen) -> Scene {
    let en = win_status::ENABLED;
    let mut ops = Vec::new();
    let mut next = 0;
    let mut lists = [0usize; 3];
    let mut rows = [0i32; 3];
    for (k, l) in lists.iter_mut().enumerate() {
        let len = 14 + g.below(7);
        ops.push(format!("W -1 6 {} {} 150 150 {en} {} 0 {len} 2 0 0 0 1 0 0 30 70", 10 + 160 * k, 10 + g.below(50), gws::SCROLL_LISTBOX));
        *l = next;
        next += 1;
        let _ = &mut rows[k];
    }
    ops.push(format!("@ nm {} PlayersList", lists[0]));
    ops.push(format!("@ nm {} GameList", lists[1]));
    let mut menus = Vec::new();
    let mut sizes = Vec::new();
    for _ in 0..24 {
        let (w, h) = (40 + g.below(321), 40 + g.below(321));
        sizes.push((w as i32, h as i32));
        ops.push(format!("@ wu -1 0 0 0 {w} {h} {en} {}", gws::USER_WINDOW));
        if g.below(3) == 0 {
            ops.push(format!("H {next} 1"));
        }
        menus.push(next);
        next += 1;
    }
    ops.push(format!("@ w8 plid {}", lists[0]));
    ops.push(format!("@ w8 gid {}", lists[1]));
    Scene { ops, players_list: lists[0], game_list: lists[1], third_list: lists[2], menus, sizes, rows }
}

fn gen_state(g: &mut Gen, ops: &mut Vec<String>, names: &mut Vec<Vec<u8>>) {
    ops.push(format!("@ w8 locprof {}", g.below(5)));
    for _ in 0..1 + g.below(8) {
        let name = pick(g, &NAMES).to_vec();
        ops.push(format!("@ w7 pl {} {} 0 1 1 - 0 0 0", bytes_hex(&name), g.below(9) as i32 - 1));
        names.push(name);
    }
    for _ in 0..g.below(4) {
        ops.push(format!("@ w7 bud {}", g.below(8)));
    }
    for id in 0..=4 {
        let ip = bytes_hex(format!("10.0.0.{}", g.below(3)).as_bytes());
        if g.below(6) == 0 {
            ops.push(format!("@ w8 srmnull {id}"));
        } else if g.below(5) != 0 {
            ops.push(format!("@ w8 srm {id} {ip} {}", 100 + g.below(2)));
        }
    }
    for _ in 0..g.below(4) {
        ops.push(format!("@ w8 ladder {} {}", bytes_hex(format!("10.0.0.{}", g.below(3)).as_bytes()), 100 + g.below(2)));
    }
}

fn gen_ops(g: &mut Gen, sc: &mut Scene, names: &[Vec<u8>], ops: &mut Vec<String>) {
    let lists = [sc.players_list, sc.game_list, sc.third_list];
    let mut used = 0usize;
    for _ in 0..8 + g.below(17) {
        match g.below(6) {
            0 | 1 if sc.rows[0] < 12 => {
                // a player's name; one time in three a unit whose low byte is 0 is appended (#232)
                let mut name: Vec<u16> = pick(g, names).iter().map(|&b| b as i8 as i16 as u16).collect();
                if g.below(3) == 0 {
                    name.push(0x100 * (1 + g.below(3) as u16));
                }
                ops.push(format!("@ w7 la {} {} {} -1 1", sc.players_list, units_hex(&name), g.i32()));
                sc.rows[0] += 1;
            }
            2 if sc.rows[1] < 12 => {
                ops.push(format!("@ w7 la {} {:x} 0 -1 0", sc.game_list, 0x41 + g.below(20)));
                ops.push(format!("@ w8 lsd {} {} {}", sc.game_list, sc.rows[1], g.below(6) as i32 - 1));
                sc.rows[1] += 1;
            }
            3 if sc.rows[2] < 12 => {
                ops.push(format!("@ w7 la {} {:x} 0 -1 1", sc.third_list, 0x41 + g.below(20)));
                sc.rows[2] += 1;
            }
            4 => {
                let l = pick(g, &lists);
                ops.push(format!("@ w7 lsel {l} 1 {}", g.below(4)));
            }
            _ => {
                let k = pick(g, &[0usize, 0, 1, 1, 2]);
                let rows = sc.rows[k];
                if rows == 0 {
                    continue;
                }
                // each layout holds windows no earlier layout held (a window in two layouts is upstream's undefined list linking)
                let n = (1 + g.below(2) as usize).min(sc.menus.len() - used);
                if n == 0 {
                    continue;
                }
                let m: Vec<String> = sc.menus[used..used + n].iter().map(|w| w.to_string()).collect();
                used += n;
                ops.push(format!("@ lay {} {}", m.len(), m.join(" ")));
                let pos = g.below(rows as u32 + 1) as i32 - 1;
                let (x, y) = match g.below(5) {
                    4 => (800 - sc.sizes[used - n].0, 600 - sc.sizes[used - n].1),
                    0 => (g.below(900) as i32 - 50, g.below(700) as i32 - 50),
                    1 => (800 - g.below(40) as i32, 600 - g.below(40) as i32),
                    2 => (700 + g.below(20) as i32, 500 + g.below(20) as i32),
                    _ => (g.below(800) as i32, g.below(600) as i32),
                };
                ops.push(format!("@ w8 rc {} {pos} {x} {y}", lists[k]));
            }
        }
    }
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
            let _ = write!(o, " {}", rts_lockstep_engine::client::gadget_list_box::get_item_data(wm, h, i, r, 0));
        }
        o.push('\n');
    }
    o
}

fn menu_op(wm: &mut GameWindowManager, h: &mut LobbyHost, op: &str, menus: &[usize]) {
    let mut it = op.split(' ');
    assert_eq!(it.next(), Some("@"));
    let ns = it.next().expect("ns");
    if ns == "lay" {
        let n: usize = it.next().unwrap().parse().unwrap();
        h.gs.lay_members = (0..n).map(|_| it.next().unwrap().parse().unwrap()).collect();
        let _ = menus;
        return;
    }
    let f = it.next().expect("op");
    let num = |it: &mut std::str::Split<'_, char>| -> i64 { it.next().expect("arg").parse().expect("number") };
    if ns == "w7" {
        match f {
            "pl" => {
                let name = unhex(it.next().unwrap());
                let profile_id = num(&mut it) as i32;
                let flags = num(&mut it) as i32;
                let (wins, losses) = (num(&mut it) as i32, num(&mut it) as i32);
                let _locale = it.next();
                let (rank_points, side, preorder) = (num(&mut it) as i32, num(&mut it) as i32, num(&mut it) as i32);
                let p = lob::LobbyPlayerInfo { name: name.clone(), profile_id, flags, wins, losses, locale: Vec::new(), rank_points, side, preorder: preorder != 0, is_ignored: false };
                h.gs.players.insert(name.to_ascii_lowercase(), p);
            }
            "bud" => {
                h.gs.buddies.insert(num(&mut it) as i32);
            }
            "la" => {
                let w = num(&mut it) as usize;
                let t: Vec<u16> = it.next().unwrap().split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect();
                let (c, r, k) = (num(&mut it) as i32, num(&mut it) as i32, num(&mut it) as i32);
                let ret = rts_lockstep_engine::client::gadget_list_box::add_entry_text(wm, h, w, &t, c, r, k, true);
                h.line(format!("added {ret}"));
            }
            "lsel" => {
                let w = num(&mut it) as usize;
                let n = num(&mut it) as usize;
                let rows: Vec<i32> = (0..n).map(|_| num(&mut it) as i32).collect();
                rts_lockstep_engine::client::gadget_list_box::set_selected(wm, h, w, &rows);
            }
            other => panic!("@ w7 {other}"),
        }
        return;
    }
    assert_eq!(ns, "w8");
    match f {
        "plid" => h.menu.listbox_lobby_players_id = wm.tree.windows[num(&mut it) as usize].id,
        "gid" => h.gs.game_list_id = wm.tree.windows[num(&mut it) as usize].id,
        "locprof" => h.gs.local_profile = num(&mut it) as i32,
        "srm" => {
            let id = num(&mut it) as i32;
            let ip = unhex(it.next().unwrap());
            let port = num(&mut it) as u16;
            h.gs.rooms_to_join.insert(id, Some(LobbyRoomToJoin { ladder_ip: ip, ladder_port: port, ..Default::default() }));
        }
        "srmnull" => {
            h.gs.rooms_to_join.insert(num(&mut it) as i32, None);
        }
        "ladder" => {
            let ip = unhex(it.next().unwrap());
            h.gs.ladders.insert((ip, num(&mut it) as u16));
        }
        "lsd" => {
            let w = num(&mut it) as usize;
            let r = num(&mut it) as i32;
            let d = num(&mut it) as i32;
            rts_lockstep_engine::client::gadget_list_box::set_item_data(wm, h, w, d as u32, r, 0);
        }
        "rc" => {
            let w = num(&mut it) as usize;
            let (pos, x, y) = (num(&mut it) as i32, num(&mut it) as i32, num(&mut it) as i32);
            wm.list_msg.right_click = (pos, x, y);
            lob::wol_lobby_menu_system(wm, h, 0, rts_lockstep_engine::client::gadget_list_box::glm::RIGHT_CLICKED, win_data(Some(w)), 0);
        }
        other => panic!("@ w8 {other}"),
    }
}

#[test]
fn lobby_right_click_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_88_01);
    let mut chk = Check::new("WOLLobbyRightClick");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(b"W7Sys", b"W7Inp")];
    for case in 0..cases(1_500) as u64 {
        let mut sc = gen_scene(&mut g);
        let mut ops = sc.ops.clone();
        let mut names = Vec::new();
        gen_state(&mut g, &mut ops, &mut names);
        let mut body = Vec::new();
        gen_ops(&mut g, &mut sc, &names, &mut body);
        ops.extend(body);
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = LobbyHost::new();
        let mut keys = KeyTable::default();
        let mut chunks = Vec::new();
        for (i, op) in ops.clone().iter().enumerate() {
            if op.starts_with("@ w7") || op.starts_with("@ w8") || op.starts_with("@ lay") {
                menu_op(&mut wm, &mut h, op, &sc.menus);
            } else {
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() {
                ops.truncate(i);
                break;
            }
            let mut log = std::mem::take(&mut *h.log.borrow_mut());
            log.push_str(&std::mem::take(&mut h.scene_log));
            let st = state_dump(&mut wm, &mut h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        compare_case(env!("WOL8_LOBBY_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
