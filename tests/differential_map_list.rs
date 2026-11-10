#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp (populateMapListboxNoReset, populateMapListbox)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The skirmish map list (`client::map_util::populate_map_listbox`, `populate_map_listbox_no_reset`) against the C++,
//! #89.
//!
//! # Method
//!
//! The C++ side is the two functions build.rs cuts verbatim from `MapUtil.cpp`, compiled by
//! `differential/wrap_map_list.cpp` on stand-ins for `TheMapCache` (the case's maps: key, display name, players,
//! multiplayer flag), `TheGlobalData`'s user data path (`User\`), `TheMappedImageCollection` (the case's star images;
//! the gold one's width), `SkirmishBattleHonors::getEnduranceMedal` (the case's medals), the real STLTypedefs.h's
//! `less_than_nocase<UnicodeString>` and the list box (a table of rows of two cells). The Rust side runs the translated
//! functions on a real list box made from a synthetic WND (one or two columns, 300x120), with a host over the same
//! data. A case is `populateMapListbox` and, half the time, a `populateMapListboxNoReset` after it (the skirmish
//! picker's user maps), each with its own directory and multiplayer flag.
//!
//! The list's bottom visible row is the gadget's geometry (its rows' heights and its display): the C++ stand-in is
//! given the Rust gadget's answer in the same state (the list as the call left it, scrolled back to where the call
//! found it: `getListboxBottomEntry`), and moves its top row as `adjustDisplay` clamps it. Both sides then dump each
//! row's cells (text and colour, or the image and its size), its map (item data 0) and star level (item data 1),
//! the selection, the top row, both calls' results, and whether `getListboxBottomEntry` read `listData[-1]` (an
//! empty list: undefined).
//!
//! A case: up to 14 maps from a pool of 16 files (two of them the patch's filtered Generals maps, some in the user's
//! folder, one in upper case, two that only start like a map folder) and 12 display names (case variants of the same
//! name, an empty one); 0 to 10 players (a negative count is left out: upstream loops some 4 billion times before it
//! lists it); medals often 0, the brutal one often `players - 1`; any of the four star images, the gold one 5 to 40
//! wide; the map to select one of the cache's, in upper case, or none.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 8901 | the names in exact order (case first) | caught: the pool has `Alpha` / `alpha` / `ALPHA` and `Zed` / `zed` |
//! | 8902 | the map to select compared without case | caught by the upper-case selections |
//! | 8903 | the red-yellow star for `players`, not `players - 1` brutal medals | caught when the stars are on |
//! | 8904 | scrolled only below the bottom row (`>` for `>=`) | caught, rarely (a selection exactly on the bottom row) |
//!
//! # Results (2026-11-10, `MAP_LIST_CASES=20000`, the default)
//!
//! **Missed the prediction three times, all in the test's protocol, none in the translation:**
//! 1. 1,384 mismatches, all in cases where the first call met the empty list's undefined `listData[-1]`: the two
//!    sides' later state (the second call's item data) differed. After an undefined step nothing is comparable, so a
//!    case where both sides record it is now counted as such and not compared further.
//! 2. Then 80, all the top row: the stand-in clamped `SetTopVisibleEntry` to the rows, where the gadget does not
//!    scroll a list that fits. The stand-in now takes the gadget's top row after `SetTopVisibleEntry(x)` for every x
//!    (the Rust gadget's answers, as for the bottom row).
//! 3. Still 80: `GLM_SET_SELECTION` itself scrolls a row out of view into it (GadgetListBox.cpp lines 1671-1688),
//!    which the stand-in ignored. The probe now takes the gadget's top row after the call's selection too.
//!
//! **Then:** 20,000 cases, 107,919 maps, 32,514 rows matched, 369 lists scrolled by the populate, 7,491 cases ending
//! in the empty list's undefined step (compared on that flag only; 12,509 compared whole), **0 mismatches**.
//!
//! Planted bugs: 4 of 4 caught, as predicted: 8901 in 847 cases, 8902 in 713, 8903 in 2,417, 8904 in 358.

use rts_lockstep_engine::client::gadget_list_box::{self as lb, CellData, LISTBOX_IMAGE};
use rts_lockstep_engine::client::map_util::{populate_map_listbox, populate_map_listbox_no_reset, MapUtilHost};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{WindowScript, WndHost};
use rts_lockstep_engine::common::game_info::slot_state;
use rts_lockstep_engine::common::map_cache::{MapCache, MapMetaData};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::differential as cpp;
use std::collections::{BTreeMap, BTreeSet};

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
}

const KEYS: [&str; 16] = [
    "maps\\alpha\\alpha.map",
    "maps\\beta\\beta.map",
    "maps\\gamma\\gamma.map",
    "maps\\delta\\delta.map",
    "maps\\armored fury\\armored fury.map",
    "maps\\scorched earth\\scorched earth.map",
    "maps\\epsilon\\e.map",
    "maps\\zeta\\z.map",
    "user\\maps\\mine\\mine.map",
    "user\\maps\\yours\\y.map",
    "maps\\Upper\\Upper.map",
    "mapsx\\odd.map",
    "usermaps\\q.map",
    "maps\\eta\\eta.map",
    "maps\\theta\\t.map",
    "user\\maps\\third\\t.map",
];
const NAMES: [&str; 12] = ["Alpha", "alpha", "ALPHA", "Beta", "Gamma (2)", "gamma (2)", "Zed", "zed", "", "Delta 4", "Eps", "Mine"];
const STARS: [&str; 4] = ["Star-Bronze", "Star-Silver", "Star-Gold", "RedYell_Star"];

/// One map of a case: its key, display name, players, multiplayer flag and endurance medals (easy, medium, brutal).
#[derive(Clone, Debug)]
struct Map {
    key: String,
    display: String,
    players: i32,
    mp: bool,
    medals: [i32; 3],
}

/// A case: the calls' arguments, the list's columns, the images, the maps and the map to select.
#[derive(Clone, Debug)]
struct Case {
    sys: bool,
    mp: bool,
    second: Option<(bool, bool)>,
    columns: i32,
    images: u32,
    gold_width: i32,
    maps: Vec<Map>,
    select: String,
}

fn gen_case(r: &mut Rng) -> Case {
    let mut maps: Vec<Map> = Vec::new();
    for _ in 0..r.below(15) {
        let key = KEYS[r.below(KEYS.len() as u64) as usize].to_string();
        if maps.iter().any(|m| m.key == key) {
            continue;
        }
        let players = r.below(11) as i32;
        let medal = |r: &mut Rng| if r.below(2) == 0 { 0 } else { 1 + r.below(3) as i32 };
        let brutal = match r.below(4) {
            0 => 0,
            1 => players - 1,
            _ => medal(r),
        };
        maps.push(Map { key, display: NAMES[r.below(NAMES.len() as u64) as usize].to_string(), players, mp: r.below(3) != 0, medals: [medal(r), medal(r), brutal] });
    }
    let select = match (r.below(4), maps.is_empty()) {
        (0, _) | (_, true) => String::new(),
        (1, false) => maps[r.below(maps.len() as u64) as usize].key.to_ascii_uppercase(),
        _ => maps[r.below(maps.len() as u64) as usize].key.clone(),
    };
    Case {
        sys: r.below(3) != 0,
        mp: r.below(3) != 0,
        second: (r.below(2) == 0).then(|| (r.below(2) == 0, r.below(2) == 0)),
        columns: 1 + r.below(2) as i32,
        images: r.below(16) as u32,
        gold_width: 5 + r.below(36) as i32,
        maps,
        select,
    }
}

/// The Rust side's host: the case's cache, images and medals, and the list's item data.
struct Host {
    cache: MapCache,
    images: BTreeSet<Vec<u8>>,
    gold_width: i32,
    medals: BTreeMap<(Vec<u8>, i32), i32>,
    names: Vec<Vec<u8>>,
}

impl GuiHost for Host {
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        (image == b"Star-Gold").then_some((self.gold_width, self.gold_width))
    }
}

impl MapUtilHost for Host {
    fn map_cache(&self) -> Option<&MapCache> {
        Some(&self.cache)
    }
    fn update_map_cache(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        b"User\\".to_vec()
    }
    fn find_mapped_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.images.contains(name).then(|| name.to_vec())
    }
    fn endurance_medal(&self, map: &[u8], state: i32) -> i32 {
        self.medals.get(&(map.to_vec(), state)).copied().unwrap_or(0)
    }
    fn map_list_item(&mut self, name: &[u8]) -> u32 {
        self.names.push(name.to_vec());
        self.names.len() as u32
    }
    fn map_list_name(&self, data: u32) -> Option<Vec<u8>> {
        (data as usize).checked_sub(1).and_then(|i| self.names.get(i)).cloned()
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
}

/// The reader's host for the synthetic file.
struct Wnd(BTreeMap<Vec<u8>, u32>);

impl WndHost for Wnd {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.0.len() as u32 + 1;
        *self.0.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| u16::from(b)).collect()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}

/// A synthetic list box `T.wnd:List` of `columns` (10/90 % for two) at (10, 10)-(310, 130), under a plain parent.
fn list_wnd(columns: i32) -> Vec<u8> {
    let widths = if columns == 2 { "COLUMNS: 2, COLUMNSWIDTH%: 10, COLUMNSWIDTH%: 90" } else { "COLUMNS: 1, COLUMNSWIDTH%: 100" };
    format!(
        "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\nWINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  CHILD\n\
  WINDOW\n    WINDOWTYPE = SCROLLLISTBOX;\n    SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 310 130, CREATIONRESOLUTION: 800 600;\n    NAME = \"T.wnd:List\";\n    STATUS = ENABLED;\n    STYLE = SCROLLLISTBOX+MOUSETRACK;\n\
    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, {widths}, FORCESELECT: 0;\n  END\n  ENDALLCHILDREN\nEND\n"
    )
    .into_bytes()
}

/// The Rust side: the dump, and the bottom row each call found (for the C++ stand-in; 0 when the list was empty).
fn rust_run(c: &Case) -> (String, [i32; 2], [Vec<i32>; 2], [i32; 2]) {
    let mut cache = MapCache::default();
    let mut medals = BTreeMap::new();
    for m in &c.maps {
        let display = UnicodeString::try_from_chars(&m.display.encode_utf16().collect::<Vec<u16>>()).expect("name");
        cache.maps.insert(m.key.as_bytes().to_vec(), MapMetaData { display_name: display, num_players: m.players, is_multiplayer: m.mp, ..Default::default() });
        for (s, n) in [slot_state::EASY_AI, slot_state::MED_AI, slot_state::BRUTAL_AI].into_iter().zip(m.medals) {
            medals.insert((m.key.as_bytes().to_vec(), s), n);
        }
    }
    let images = (0..4).filter(|i| c.images & (1 << i) != 0).map(|i| STARS[i].as_bytes().to_vec()).collect();
    let mut h = Host { cache, images, gold_width: c.gold_width, medals, names: Vec::new() };
    let mut wm = GameWindowManager::new((800, 600));
    let mut wnd = Wnd(BTreeMap::new());
    let made = wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", &list_wnd(c.columns), &mut wnd, &mut h).expect("the synthetic file reads");
    let l = wm.tree.find_by_name(b"T.wnd:List").unwrap_or_else(|| panic!("the list box: {made:?}"));
    let mut bottoms = [0; 2];
    let mut tops: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
    // the top row the gadget shows after `SetTopVisibleEntry(x)`, for each x from 0 to the rows, in the list as the
    // call left it
    let top_table = |wm: &mut GameWindowManager, h: &mut Host| -> Vec<i32> {
        if wm.undefined.is_some() {
            return Vec::new();
        }
        let now = lb::get_top_visible_entry(wm, l);
        let n = lb::get_num_entries(wm, l);
        let t = (0..=n)
            .map(|x| {
                lb::set_top_visible_entry(wm, h, l, x);
                lb::get_top_visible_entry(wm, l)
            })
            .collect();
        lb::set_top_visible_entry(wm, h, l, now);
        t
    };
    // what the call saw of the gadget: the list as the call left it, scrolled back to `top` (where the call found
    // it), then its selection `sel` set (`GLM_SET_SELECTION` scrolls a row out of view into it): the top row then,
    // and the bottom row; the list is then put back as the call left it
    let probe = |wm: &mut GameWindowManager, h: &mut Host, top: i32, sel: i32| -> (i32, i32) {
        if wm.undefined.is_some() || lb::get_num_entries(wm, l) == 0 {
            return (0, 0);
        }
        let (now, now_sel) = (lb::get_top_visible_entry(wm, l), lb::get_selected(wm, h, l));
        lb::set_top_visible_entry(wm, h, l, top);
        lb::set_selected(wm, h, l, &[sel]);
        let r = (lb::get_top_visible_entry(wm, l), lb::get_bottom_visible_entry(wm, l));
        lb::set_selected(wm, h, l, &now_sel);
        lb::set_top_visible_entry(wm, h, l, now);
        r
    };
    let mut sel_tops = [0; 2];
    let r1 = populate_map_listbox(&mut wm, &mut h, Some(l), c.sys, c.mp, c.select.as_bytes());
    (sel_tops[0], bottoms[0]) = probe(&mut wm, &mut h, 0, r1);
    tops[0] = top_table(&mut wm, &mut h);
    let mut r2 = -99;
    if let Some((sys2, mp2)) = c.second {
        let top = lb::get_top_visible_entry(&mut wm, l);
        r2 = populate_map_listbox_no_reset(&mut wm, &mut h, Some(l), sys2, mp2, c.select.as_bytes());
        (sel_tops[1], bottoms[1]) = probe(&mut wm, &mut h, top, r2);
        tops[1] = top_table(&mut wm, &mut h);
    }
    let mut o = String::new();
    let rt = lb::listbox_rt(&wm, l).expect("the list's data").clone();
    for r in 0..i32::from(rt.end_pos) {
        o += &format!("row {r}:");
        let row = rt.list_data.as_ref().expect("rows")[r as usize];
        for col in 0..c.columns {
            let cell = row.cell.and_then(|a| rt.arrays[a].as_ref()).map(|cells| cells[col as usize].clone());
            match cell {
                None => o += " -",
                Some(cell) => match &cell.data {
                    CellData::Text(d) => o += &format!(" [{}]#{:08x}", String::from_utf16_lossy(&d.text), cell.color as u32),
                    CellData::Image(i) => o += &format!(" img:{} {}x{}", String::from_utf8_lossy(i), cell.width, cell.height),
                    CellData::None if cell.cell_type == LISTBOX_IMAGE => o += &format!(" img:<none> {}x{}", cell.width, cell.height),
                    CellData::None => o += " -",
                },
            }
        }
        let d = lb::get_item_data(&mut wm, &mut h, l, r, 0);
        o += &format!(" data {}", h.map_list_name(d).map_or("NULL".to_string(), |n| String::from_utf8_lossy(&n).into_owned()));
        if c.columns > 1 {
            o += &format!(" level {}", lb::get_item_data(&mut wm, &mut h, l, r, 1) as i32);
        }
        o += "\n";
    }
    let sel = lb::get_selected(&mut wm, &mut h, l).first().copied().unwrap_or(-99);
    let top = lb::get_top_visible_entry(&mut wm, l);
    o += &format!("selected {sel} top {top} ret {r1} {r2} ub {}\n", u8::from(wm.undefined.is_some()));
    (o, bottoms, tops, sel_tops)
}

fn cpp_run(c: &Case, bottoms: [i32; 2], tops: &[Vec<i32>; 2], sel_tops: [i32; 2]) -> String {
    let (sys2, mp2) = c.second.unwrap_or((false, false));
    let mut text = format!(
        "cfg {} {} {} {} {} {} {} {} {} {} {} User\\\n",
        u8::from(c.sys),
        u8::from(c.mp),
        c.columns,
        // (the gadget's own width of column 0: 10 % of 300)
        if c.columns == 2 { 30 } else { 300 },
        c.images,
        c.gold_width,
        u8::from(c.second.is_some()),
        u8::from(sys2),
        u8::from(mp2),
        bottoms[0],
        bottoms[1]
    );
    for m in &c.maps {
        text += &format!("map {}|{}|{}|{}|{} {} {}\n", m.key, m.display, m.players, u8::from(m.mp), m.medals[0], m.medals[1], m.medals[2]);
    }
    text += &format!("sel {}\n", c.select);
    text += &format!("seltops {} {}\n", sel_tops[0], sel_tops[1]);
    for (k, t) in tops.iter().enumerate() {
        text += &format!("tops{} {}\n", k + 1, t.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "));
    }
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 14;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_map_list_run(input.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

#[test]
fn the_map_list_matches_upstream() {
    let cases: usize = std::env::var("MAP_LIST_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x4d61_704c_6973_7489);
    let (mut mismatches, mut rows, mut maps, mut scrolled, mut undefined) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for case in 0..cases {
        let c = gen_case(&mut r);
        maps += c.maps.len();
        let (a, bottoms, tops, sel_tops) = rust_run(&c);
        let b = cpp_run(&c, bottoms, &tops, sel_tops);
        // (after an undefined step the two sides' later state is not comparable: only that both met it is)
        let both_undefined = a.ends_with("ub 1\n") && b.ends_with("ub 1\n");
        if both_undefined {
            undefined += 1;
            continue;
        }
        if a != b {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {case}: {c:?}\n  rust:\n{a}  c++:\n{b}");
            }
        } else {
            rows += a.lines().count() - 1;
            if !a.contains(" top 0 ") {
                scrolled += 1;
            }
        }
    }
    println!("#89 map list: {cases} cases, {maps} maps, {rows} rows matched, {scrolled} scrolled, {undefined} empty (undefined), {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
