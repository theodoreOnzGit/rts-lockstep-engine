// SPDX-License-Identifier: GPL-3.0-only
//! Shared pieces of the lan6 differential tests (#215): the LAN and WOL map-select menus' scene (the menu's windows, the options screen underneath, the list, the preview and
//! its start buttons, the radio buttons), the case's state (the map cache, the game's map, the preferences, the previews, the list's rows), the operations both programs read
//! (differential/lan6_menu.h lists them), the stand-ins' log lines and the dump of the statics, in the format the C++ programs write.
#![allow(dead_code)]

use super::support::{Check, Gen};
use super::ui4_common::*;
use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::gadget_combo_box as _cbx;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, GadgetData, RadioButtonData, WinId, WinInstanceData};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// A byte string as the C++ logs it (`l6_hex`): comma-separated hex, `-` for empty.
pub fn hexb(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

pub const MAX_SLOTS: usize = 8;

/// The state the stand-ins hold (differential/lan6_menu.h).
#[derive(Default)]
pub struct MapHost {
    pub log: String,
    pub keys: KeyTable,
    pub ub: bool,
    pub game_map: Vec<u8>,
    pub game_crc: u32,
    pub game_size: u32,
    pub avail: i32,
    /// (official, crc, size) by the cache's (lower case) key
    pub cache: BTreeMap<Vec<u8>, (bool, u32, u32)>,
    pub ini: BTreeMap<String, String>,
    pub previews: BTreeSet<Vec<u8>>,
    pub items: Vec<Vec<u8>>,
    pub images: BTreeMap<usize, Vec<u8>>,
    pub user: BTreeMap<usize, Vec<u8>>,
    pub layout: Option<usize>,
    pub stats: bool,
}

impl MapHost {
    pub fn new() -> Self {
        MapHost { avail: -1, ..Default::default() }
    }
    pub fn logln(&mut self, s: &str) {
        self.log.push_str(s);
        self.log.push('\n');
    }
    pub fn find_map(&self, name: &[u8]) -> Option<&(bool, u32, u32)> {
        self.cache.get(&name.to_ascii_lowercase())
    }
    pub fn uses_system_lan(&self) -> bool {
        // LANPreferences::usesSystemMapDir (LanLobbyMenu.cpp): absent is TRUE, "yes" (any case) TRUE
        match self.ini.get("UseSystemMapDir") {
            None => true,
            Some(v) => v.eq_ignore_ascii_case("yes"),
        }
    }
    pub fn uses_system_custom(&self) -> bool {
        // CustomMatchPreferences::usesSystemMapDir (UserPreferences.cpp): absent is TRUE, "1" TRUE
        match self.ini.get("UseSystemMapDir") {
            None => true,
            Some(v) => v.eq_ignore_ascii_case("1"),
        }
    }
    pub fn prefs_write(&mut self) {
        self.logln("prefs write");
    }
    pub fn populate(&mut self, list: Option<WinId>, system: bool, mp: bool, map: &[u8]) {
        let _ = writeln!(self.log, "populate {} sys {} mp {} map {}", idx(list), u8::from(system), u8::from(mp), hexb(map));
    }
    pub fn spots(&mut self, map: &[u8], buttons: &[Option<WinId>; MAX_SLOTS], win: Option<WinId>) {
        let _ = write!(self.log, "spots map {} btns", hexb(map));
        for b in buttons {
            let _ = write!(self.log, " {}", idx(*b));
        }
        let _ = writeln!(self.log, " win {}", idx(win));
    }
    pub fn preview_image(&mut self, map: &[u8]) -> Option<Vec<u8>> {
        let _ = writeln!(self.log, "preview map {}", hexb(map));
        self.previews.contains(map).then(|| [b"P_".as_slice(), map].concat())
    }
    pub fn set_user(&mut self, w: WinId, map: &[u8]) {
        if self.find_map(map).is_some() {
            self.user.insert(w, map.to_ascii_lowercase());
        } else {
            self.user.remove(&w);
        }
    }
    pub fn item_name(&self, data: u32) -> Option<Vec<u8>> {
        (data > 0).then(|| self.items[data as usize - 1].clone())
    }
    pub fn game_set_map(&mut self, m: &[u8]) {
        self.game_map = m.to_vec();
        let _ = writeln!(self.log, "game setmap {}", hexb(m));
    }
    pub fn slot0_avail(&mut self, b: bool) {
        self.avail = i32::from(b);
        let _ = writeln!(self.log, "game slot0 avail {}", i32::from(b));
    }
    pub fn crc_size(&mut self, crc: u32, size: u32) {
        self.game_crc = crc;
        self.game_size = size;
        let _ = writeln!(self.log, "game crc {crc}\ngame size {size}");
    }
    /// `ShowUnderlyingGUIElements` (GUIUtil.cpp), in the order of its `nameToKey` calls.
    pub fn show_underlying(&mut self, wm: &mut GameWindowManager, show: bool, file: &str, parent: &str, gadgets: &[&str], per_player: &[&str]) {
        let pid = self.keys.key(format!("{file}:{parent}").as_bytes()) as i32;
        let Some(p) = wm.win_get_window_from_id(None, pid) else { return };
        for g in gadgets {
            let id = self.keys.key(format!("{file}:{g}").as_bytes()) as i32;
            if let Some(w) = wm.win_get_window_from_id(Some(p), id) {
                wm.win_hide(w, !show);
            }
        }
        for g in per_player {
            for player in 0..MAX_SLOTS {
                let id = self.keys.key(format!("{file}:{g}{player}").as_bytes()) as i32;
                if let Some(w) = wm.win_get_window_from_id(Some(p), id) {
                    wm.win_hide(w, !show);
                }
            }
        }
    }
    /// The state both programs dump (`l6_dump_common`).
    pub fn dump_common(&self, wm: &GameWindowManager) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "ms game map {} crc {} size {} avail {}", hexb(&self.game_map), self.game_crc, self.game_size, self.avail);
        o.push_str("ms ini");
        for (k, v) in &self.ini {
            let _ = write!(o, " {k}=");
            for b in v.bytes() {
                let _ = write!(o, "{b:02x}");
            }
        }
        o.push_str("\nms img");
        for (w, n) in &self.images {
            if !wm.ext[*w].freed {
                let _ = write!(o, " {w}:{}", String::from_utf8_lossy(n));
            }
        }
        o.push_str("\nms user");
        for (w, n) in &self.user {
            if !wm.ext[*w].freed {
                let _ = write!(o, " {w}:{}", String::from_utf8_lossy(n));
            }
        }
        let l = self.layout.filter(|&l| !wm.layouts[l].deleted).map_or(-1, |l| l as i64);
        let _ = writeln!(o, "\nms layout {l}");
        o
    }
}

/// What a test's host gives the shared runner.
pub trait MapMenu: GuiHost + HasLog {
    fn mh(&mut self) -> &mut MapHost;
    /// `@ ms init | shutdown | update | msl <b>` and `@ sys` / `@ inp` (the menu's own).
    fn menu_op(&mut self, wm: &mut GameWindowManager, op: &str);
    /// The statics' line(s), in the format of the program's `u4_menu_dump`.
    fn statics(&self, wm: &GameWindowManager) -> String;
}

/// Where a scene's names live.
pub struct Cfg {
    pub file: &'static str,
    pub parent_name: &'static str,
    pub under_file: &'static str,
    pub under_parent: &'static str,
    pub wol: bool,
}

const POOL: [&str; 5] = ["a", "b", "maps/c/c.map", "d.map", "e"];
const GAME_MAPS: [&str; 7] = ["a", "b", "A", "MAPS/C/C.MAP", "d.map", "zz", "-"];
const FILES: [&str; 8] = ["a", "b", "A", "maps/c/c.map", "d.map", "E", "zz", "-"];
const TEXTS: [&str; 6] = ["Alpha", "Beta", "A", "Maps", "Z", "-"];

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

fn hx(s: &str) -> String {
    hex(&s.encode_utf16().collect::<Vec<u16>>())
}

pub struct Scene {
    pub ops: Vec<String>,
    /// (index, kind): 0 menu callbacks window, 1 list, 2 preview, 3 radio, 4 push button (back/ok), 5 other
    pub wins: Vec<(usize, u8)>,
    pub list: Option<usize>,
}

pub fn gen_scene(g: &mut Gen, cfg: &Cfg) -> Scene {
    let mut ops = Vec::new();
    let mut wins: Vec<(usize, u8)> = Vec::new();
    let en = win_status::ENABLED;
    let mut next = 0usize;
    let mut tops = Vec::new();
    let mut names: Vec<(usize, String)> = Vec::new();
    let parent_present = g.below(6) != 0;
    if parent_present {
        ops.push(format!("@ wu -1 0 0 0 640 480 {en} {}", gws::USER_WINDOW));
        wins.push((0, 0));
        names.push((0, format!("{}:{}", cfg.file, cfg.parent_name)));
        tops.push(0usize);
        next += 1;
    }
    let decoys = g.below(3) as usize;
    for _ in 0..decoys {
        let (x, y) = (g.below(400) as i32, g.below(300) as i32);
        ops.push(format!("@ wu -1 0 {x} {y} 300 200 {en} {}", gws::USER_WINDOW));
        wins.push((next, 0));
        tops.push(next);
        next += 1;
    }
    // the options screen underneath
    let under = g.below(6) != 0;
    let mut under_idx = None;
    if under {
        ops.push(format!("@ wu -1 0 10 10 600 400 {en} {}", gws::USER_WINDOW));
        wins.push((next, 5));
        names.push((next, format!("{}:{}", cfg.under_file, cfg.under_parent)));
        under_idx = Some(next);
        next += 1;
        let hide = ["MapWindow", "StaticTextTeam", "StaticTextFaction", "StaticTextColor", "TextEntryMapDisplay", "ButtonSelectMap", "ButtonStart", "StaticTextMapPreview"];
        for n in hide {
            if g.below(3) != 0 {
                let st = if g.below(3) == 0 { en | win_status::HIDDEN } else { en };
                ops.push(format!("W {} 1 5 5 20 10 {st} {} 0", under_idx.unwrap(), gws::PUSH_BUTTON));
                wins.push((next, 5));
                names.push((next, format!("{}:{n}", cfg.under_file)));
                next += 1;
            }
        }
        for n in ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"] {
            for p in 0..MAX_SLOTS {
                if g.below(4) == 0 {
                    let st = if g.below(3) == 0 { en | win_status::HIDDEN } else { en };
                    ops.push(format!("W {} 1 5 5 20 10 {st} {} 0", under_idx.unwrap(), gws::PUSH_BUTTON));
                    wins.push((next, 5));
                    names.push((next, format!("{}:{n}{p}", cfg.under_file)));
                    next += 1;
                }
            }
        }
    }
    if g.below(4) != 0 {
        let par = if g.below(3) == 0 { under_idx } else { None };
        let ps = par.map_or(-1, |p| p as i32);
        ops.push(format!("W {ps} 1 20 20 30 10 {en} {} 0", gws::PUSH_BUTTON));
        wins.push((next, 5));
        names.push((next, format!("{}:ButtonBack", cfg.under_file)));
        next += 1;
    }
    let place = |g: &mut Gen, tops: &[usize]| -> i32 {
        match g.below(20) {
            0..=13 if parent_present => 0,
            14..=16 => -1,
            0..=13 => -1,
            _ if decoys > 0 => tops[tops.len() - 1 - g.below(decoys as u32) as usize] as i32,
            _ => -1,
        }
    };
    let fname = |n: &str| format!("{}:{n}", cfg.file);
    // the list
    let mut list = None;
    if g.below(12) != 0 {
        let ps = place(g, &tops);
        ops.push(format!("W {ps} 6 20 20 100 80 {en} {} 0 8 1 0 0 0 0 0 0", gws::SCROLL_LISTBOX));
        wins.push((next, 1));
        names.push((next, fname("ListboxMap")));
        list = Some(next);
        next += 1;
    }
    // the preview and its start buttons
    if g.below(16) != 0 {
        let ps = place(g, &tops);
        ops.push(format!("@ wu {ps} 0 130 20 100 80 {en} {}", gws::USER_WINDOW));
        wins.push((next, 2));
        names.push((next, fname("WinMapPreview")));
        let pv = next;
        next += 1;
        for i in 0..MAX_SLOTS {
            if g.below(32) != 0 {
                ops.push(format!("W {pv} 1 {} 5 8 8 {en} {} 0", 5 + i * 10, gws::PUSH_BUTTON));
                wins.push((next, 5));
                names.push((next, fname(&format!("ButtonMapStartPosition{i}"))));
                next += 1;
            }
        }
    }
    let mut interesting: Vec<usize> = Vec::new();
    for (n, kind) in [("RadioButtonSystemMaps", 3u8), ("RadioButtonUserMaps", 3), ("ButtonBack", 4), ("ButtonOK", 4)] {
        for copy in 0..2 {
            let present = if copy == 0 { g.below(12) != 0 } else { g.below(3) == 0 && decoys > 0 };
            if !present {
                continue;
            }
            let ps = if copy == 1 { tops[tops.len() - 1 - g.below(decoys as u32) as usize] as i32 } else { place(g, &tops) };
            let mut status = en;
            if g.below(10) == 0 {
                status |= win_status::HIDDEN;
            }
            if kind == 3 {
                ops.push(format!("W {ps} 3 240 {} 30 10 {status} {} 0 1", 5 + next * 3, gws::RADIO_BUTTON));
            } else {
                ops.push(format!("W {ps} 1 240 {} 30 10 {status} {} 0", 5 + next * 3, gws::PUSH_BUTTON));
            }
            wins.push((next, kind));
            interesting.push(next);
            names.push((next, fname(n)));
            next += 1;
        }
    }
    for (i, n) in &names {
        ops.push(format!("@ nm {i} {n}"));
    }
    // the layout: the menu's parent (its children go with it)
    if parent_present {
        ops.push("@ ms layout 1 0".to_string());
    } else {
        ops.push("@ ms layout 0".to_string());
    }
    ops.push(format!("@ ms msl {}", u8::from(g.below(6) != 0)));
    // the list's rows
    if let Some(l) = list {
        for _ in 0..g.below(6) {
            let t = match g.below(7) {
                0 => "41,100,42".to_string(),
                1 => "e9,4e2d".to_string(),
                _ => hx(TEXTS[g.below(5) as usize]),
            };
            ops.push(format!("@ ms row {l} {t} {}", pick(g, &FILES)));
        }
    }
    state_ops(g, &mut ops, true);
    let _ = interesting;
    Scene { ops, wins, list }
}

/// The cache, the game's map, the preferences, the previews (`all`: every kind).
pub fn state_ops(g: &mut Gen, ops: &mut Vec<String>, all: bool) {
    if all {
        for _ in 0..g.below(5) {
            ops.push(format!("@ ms mapcache {} {} {} {}", pick(g, &POOL), g.below(2), g.below(3) * 1000 + 7, g.below(4) * 100));
        }
        for p in POOL {
            if g.below(3) == 0 {
                ops.push(format!("@ ms image {p}"));
            }
        }
        ops.push(format!("@ ms gamemap {}", pick(g, &GAME_MAPS)));
        ops.push(format!("@ ms ini {}", pick(g, &["-", "yes", "no", "YES", "1", "0", "maybe"])));
        ops.push(format!("@ ms stats {}", g.below(2)));
    } else {
        match g.below(5) {
            0 => ops.push(format!("@ ms mapcache {} {} {} {}", pick(g, &POOL), g.below(2), g.below(3) * 1000 + 7, g.below(4) * 100)),
            1 => ops.push(format!("@ ms image {}", pick(g, &POOL))),
            2 => ops.push(format!("@ ms gamemap {}", pick(g, &GAME_MAPS))),
            3 => ops.push(format!("@ ms ini {}", pick(g, &["-", "yes", "no", "YES", "1", "0", "maybe"]))),
            _ => ops.push(format!("@ ms stats {}", g.below(2))),
        }
    }
}

pub fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let menu_wins: Vec<usize> = s.wins.iter().filter(|w| w.1 == 0 || w.1 == 2).map(|w| w.0).collect();
    let controls: Vec<usize> = s.wins.iter().filter(|w| matches!(w.1, 1 | 3 | 4)).map(|w| w.0).collect();
    let any = |g: &mut Gen| pick(g, &s.wins).0;
    let menu_win = |g: &mut Gen| if menu_wins.is_empty() { any(g) } else { pick(g, &menu_wins) };
    let control = |g: &mut Gen| if !controls.is_empty() && g.below(10) < 8 { pick(g, &controls) } else { any(g) };
    use rts_lockstep_engine::client::gadget_list_box::glm;
    if let (Some(l), true) = (s.list, g.below(100) < 14) {
        // composites: the menu starts, a row is picked, then it is double-clicked (OK) or Back is pressed
        let row = g.below(6) as i32;
        let w = menu_win(g);
        let tail = if g.below(4) == 0 { format!("@ sys 0 {w} {} 1 {} 0", ggm::GBM_SELECTED, control(g)) } else { format!("@ sys 0 {w} {} 1 {l} {row}", glm::DOUBLE_CLICKED) };
        return format!("@ ms init\n@ sys 0 {w} {} 1 {l} {row}\n{tail}", ggm::GLM_SELECTED);
    }
    match g.below(100) {
        0..=13 => "@ ms init".into(),
        14..=17 => "@ ms update".into(),
        18..=22 => "@ ms shutdown".into(),
        23..=24 => format!("@ ms msl {}", g.below(2)),
        25..=33 => {
            let mut v = Vec::new();
            state_ops(g, &mut v, false);
            v.remove(0)
        }
        34..=36 if s.list.is_some() => format!("@ ms row {} {} {}", s.list.unwrap(), hx(TEXTS[g.below(5) as usize]), pick(g, &FILES)),
        37..=52 => format!("@ sys 0 {} {} 1 {} {}", menu_win(g), ggm::GBM_SELECTED, control(g), g.below(3)),
        53..=62 => {
            let row = g.below(7) as i32 - 1;
            let ctl = if let (Some(l), true) = (s.list, g.below(10) < 8) { l } else { control(g) };
            format!("@ sys 0 {} {} 1 {ctl} {row}", menu_win(g), if g.below(3) == 0 { glm::DOUBLE_CLICKED } else { ggm::GLM_SELECTED })
        }
        63..=67 => {
            let msg = pick(g, &[1u32, 2, 3, 4, 21, 23, 24, 25]);
            let (d1, d2) = if msg == gwm::INPUT_FOCUS { (2 + g.below(2), 0) } else { (g.below(3), g.below(3)) };
            format!("@ sys 0 {} {msg} 0 {d1} {d2}", menu_win(g))
        }
        68..=77 => {
            let key = pick(g, &[1u32, 1, 1, 2, 28, 0x101, 0x201, 57]);
            let state = pick(g, &[1u32, 2, 3, 0, 0x101, 4]);
            let msg = if g.below(6) == 0 { pick(g, &[1u32, 2, 24, 25, 123]) } else { gwm::CHAR };
            format!("@ inp 0 {} {msg} 0 {key} {state}", menu_win(g))
        }
        87..=89 => format!("H {} {}", any(g), g.below(2)),
        90..=92 => format!("E {} {}", any(g), g.below(2)),
        _ => format!("F {}", any(g) as i32),
    }
}

/// `W` of a radio button (type 3), which `ui4_common::scene_op` does not carry; false when `op` is not one.
fn extra_op<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, op: &str) -> bool {
    let v: Vec<&str> = op.split(' ').collect();
    if v[0] == "W" && v[2] == "3" {
        let n: Vec<i64> = v[1..].iter().map(|s| s.parse().unwrap()).collect();
        let p = (n[0] >= 0).then_some(n[0] as usize);
        let inst = WinInstanceData { style: n[7] as u32, ..WinInstanceData::new() };
        wm.gogo_gadget_look(h, p, n[6] as u32, n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32, inst, GadgetData::Radio(RadioButtonData { screen: 1, group: n[9] as i32 }), n[8] != 0);
        return true;
    }
    false
}

fn hexs(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

/// The shared `@ ms` operations; false when `f` is none of them.
fn common_ms<H: MapMenu>(wm: &mut GameWindowManager, h: &mut H, v: &[&str]) -> bool {
    let num = |i: usize| -> i64 { v[i].parse().expect("number") };
    match v[2] {
        "mapcache" => {
            h.mh().cache.insert(v[3].as_bytes().to_vec(), (num(4) != 0, num(5) as u32, num(6) as u32));
        }
        "gamemap" => h.mh().game_map = if v[3] == "-" { Vec::new() } else { v[3].as_bytes().to_vec() },
        "ini" => {
            if v[3] == "-" {
                h.mh().ini.remove("UseSystemMapDir");
            } else {
                h.mh().ini.insert("UseSystemMapDir".into(), v[3].to_string());
            }
        }
        "image" => {
            h.mh().previews.insert(v[3].as_bytes().to_vec());
        }
        "stats" => h.mh().stats = num(3) != 0,
        "row" => {
            let l = num(3) as usize;
            let row = lb::add_entry_text(wm, h, l, &hexs(v[4]), -1, -1, 0, false);
            if row >= 0 && v[5] != "-" {
                h.mh().items.push(v[5].as_bytes().to_vec());
                let id = h.mh().items.len() as u32;
                lb::set_item_data(wm, h, l, id, row, 0);
            }
        }
        "layout" => {
            let n = num(3) as usize;
            let members: Vec<usize> = (0..n).map(|i| num(4 + i) as usize).collect();
            let l = make_layout(wm, &members);
            h.mh().logln("layout MapSelect.wnd");
            h.mh().layout = Some(l);
        }
        _ => return false,
    }
    true
}

/// Runs a case's operations on the Rust side and compares each operation's output with the program `exe`'s.
pub fn run_case<H: MapMenu>(exe: &str, chk: &mut Check, case: u64, ops: Vec<String>, mut h: H, menu_funcs: &[(&[u8], &[u8])]) {
    let mut wm = GameWindowManager::new((800, 600));
    let mut chunks = Vec::new();
    let mut run = ops.len();
    for (i, op) in ops.iter().enumerate() {
        let v: Vec<&str> = op.split(' ').collect();
        let freed_before = wm.ext.iter().filter(|e| e.freed).count();
        if v[0] == "@" && v[1] == "ms" && common_ms(&mut wm, &mut h, &v) {
        } else if v[0] == "@" && matches!(v[1], "ms" | "sys" | "inp") {
            h.menu_op(&mut wm, op);
        } else if !extra_op(&mut wm, &mut h, op) {
            let mut keys = std::mem::take(&mut h.mh().keys);
            let ok = scene_op(&mut wm, &mut h, &mut keys, menu_funcs, op);
            h.mh().keys = keys;
            assert!(ok, "unknown op {op}");
        }
        if wm.undefined.is_some() || h.mh().ub {
            run = i;
            break;
        }
        let log = std::mem::take(&mut h.mh().log);
        let mut st = dump(&wm);
        st.push_str(&dump_layouts(&wm));
        st.push_str(&h.statics(&wm));
        st.push_str(&h.mh().dump_common(&wm));
        chunks.push(format!("#{i}\n{log}{st}"));
        // (a destroyed layout frees windows the later operations would name)
        if wm.ext.iter().filter(|e| e.freed).count() != freed_before {
            run = i + 1;
            break;
        }
    }
    let ops: Vec<String> = ops.into_iter().take(run).collect();
    compare_case(exe, chk, case, &ops, &chunks);
}
