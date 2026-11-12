#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp (copyFromBigToDir, getMapPreviewImage, findDrawPositions)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp
//     (positionStartSpotControls, positionAdditionalImages, positionStartSpots)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp
//     (W3DDrawMapPreview, drawSkinnyBorder)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The skirmish map preview (`client::skirmish_game_options::position_start_spots`, `client::map_util`'s preview image,
//! `client::w3d_map_preview`) against the C++, #157.
//!
//! # Method
//!
//! The C++ side is the pieces build.rs cuts verbatim (see `differential/wrap_map_preview.cpp`) on stand-ins for the
//! windows, the display, the image collection, the map cache, the file system and `TheGameState`'s portable path (the
//! Rust side's answer, given). The Rust side is the shell's own host ([`ShellGui`]) on a synthetic WND: a map window
//! (with `W3DDrawMapPreview` as its draw callback) holding the eight start-spot buttons, as the game's files have
//! them. A case: the map window's place and size (hidden now and then), the buttons' size, 1 to 5 maps (extents of
//! quarter-pixel values, 0 to 8 players, multiplayer or not, the start waypoints usually all there, 0 to 4 supply docks
//! and tech buildings), which maps' `.tga` the archives hold (in either case), which of the marker and frame images
//! exist, and the map to select (a cache key, in upper case, an unknown one, or none). `positionStartSpots(mapName,
//! ...)` runs, then the window's draw. Both sides dump: the preview's copy (path and bytes), the window's user data, its
//! image status and image, each button's position and hidden flag, the supply and tech markers, the new images
//! (name, file, status, UV, texture size), and every draw call.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 9529 | the frame's adjusted half piece without the even rounding | caught (widths where the remainder is even) |
//! | 9530 | `>` for `>=` choosing the bands | caught rarely (an exactly matching aspect); **missed** by the first generator (no tie in 20,000 cases): one map in eight now has the window's aspect |
//! | 9531 | the bottom band a pixel high | caught in every horizontal-band case |
//! | 9532 | a tie between the right and the bottom nudges right only | caught rarely |
//! | 9533 | the markers pushed to the back | caught with two markers or more |
//! | 9534 | the start spot's y not flipped | caught in every case with a spot |
//! | 9535 | the folders' `\` kept in the preview's name | caught with every `.tga` found |
//! | 9536 | a 64x64 texture | caught with every `.tga` found |
//! | 9539 | `findDrawPositions` rounds the band half up | caught often |
//! | 9540 | the hidden map window still gets markers | caught in the hidden cases with a map |
//!
//! # Results (2026-11-12, `MAP_PREVIEW_CASES=20000`, the default)
//!
//! First run: 20,000 cases, 41,570 start spots shown, 8,836 preview images made, 2,047,991 draw calls, **0 mismatches**,
//! as predicted. Plants: 9 of 10 caught; **9530 missed** (0 cases): the random quarter-pixel extents never tied the
//! window's aspect, so `>` and `>=` never differed. The generator now gives one map in eight the window's aspect
//! (`k` times its size). Rerun: 20,000 cases, 42,461 start spots, 8,797 preview images, 2,063,560 draw calls, **0
//! mismatches**; plants 10 of 10 caught: 9529 3,753 cases, 9530 1,706, 9531 7,564, 9532 **1** (a tie is rare, as
//! predicted), 9533 7,282, 9534 9,219, 9535 8,797, 9536 8,797, 9539 2,939, 9540 1,038.

use rts_lockstep_engine::client::shell::{ShellHost, WndFiles};
use rts_lockstep_engine::client::shell_gui::ShellGui;
use rts_lockstep_engine::client::skirmish_game_options::position_start_spots;
use rts_lockstep_engine::client::window_draw::w3d_game_win_default_draw;
use rts_lockstep_engine::client::window_manager::{DrawItem, GameWindowManager, GuiHost, UserData};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WndHost};
use rts_lockstep_engine::common::base_type::{Coord3D, Region3D};
use rts_lockstep_engine::common::map_cache::MapMetaData;
use rts_lockstep_engine::differential as cpp;
use std::collections::{BTreeMap, BTreeSet};

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
    /// A quarter-pixel value in `0 .. n` (exact in decimal and in `float`).
    fn quarter(&mut self, n: u64) -> f32 {
        self.below(n * 4) as f32 / 4.0
    }
}

const KEYS: [&str; 6] = ["maps\\alpha\\alpha.map", "maps\\beta\\b.map", "user\\maps\\mine\\mine.map", "maps\\gamma\\gamma.map", "maps\\delta\\delta.map", "maps\\eps\\eps.map"];
const IMAGES: [&str; 10] = ["TecBuilding", "Cash", "FrameT", "FrameB", "FrameL", "FrameR", "FrameCornerUL", "FrameCornerUR", "FrameCornerLL", "FrameCornerLR"];

#[derive(Clone, Debug, Default)]
struct Files {
    wnd: Vec<u8>,
    images: BTreeSet<Vec<u8>>,
    keys: BTreeMap<Vec<u8>, u32>,
}

impl WndHost for Files {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.images.contains(name).then(|| name.to_vec())
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| u16::from(b)).collect()
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

fn wnd(x: i32, y: i32, w: i32, h: i32, hidden: bool, bw: i32, bh: i32) -> Vec<u8> {
    let f = "SkirmishGameOptionsMenu.wnd";
    let mut buttons = String::new();
    for i in 0..8 {
        let bx = x + 20 * i;
        buttons += &format!("    WINDOW\n      WINDOWTYPE = PUSHBUTTON;\n      SCREENRECT = UPPERLEFT: {bx} {y}, BOTTOMRIGHT: {} {}, CREATIONRESOLUTION: 800 600;\n      NAME = \"{f}:ButtonMapStartPosition{i}\";\n      STATUS = ENABLED;\n      STYLE = PUSHBUTTON;\n    END\n", bx + bw, y + bh);
    }
    let st = if hidden { "ENABLED+HIDDEN" } else { "ENABLED" };
    format!(
        "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\nWINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"{f}:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  CHILD\n\
  WINDOW\n    WINDOWTYPE = USER;\n    SCREENRECT = UPPERLEFT: {x} {y}, BOTTOMRIGHT: {} {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"{f}:MapWindow\";\n    STATUS = {st};\n    STYLE = USER;\n    DRAWCALLBACK = \"W3DDrawMapPreview\";\n    CHILD\n{buttons}    ENDALLCHILDREN\n  END\n  ENDALLCHILDREN\nEND\n",
        x + w,
        y + h
    )
    .into_bytes()
}

fn c3(x: f32, y: f32) -> Coord3D {
    Coord3D { x, y, z: 0.0 }
}

fn case(r: &mut Rng) -> (String, String) {
    let (x, y) = (r.below(500) as i32, r.below(300) as i32);
    let (w, h) = (20 + r.below(280) as i32, 20 + r.below(280) as i32);
    let hidden = r.below(12) == 0;
    let (bw, bh) = (8 + r.below(17) as i32, 8 + r.below(17) as i32);
    let mut text = format!("window {} {y} {w} {h} {}\nbuttons {bw} {bh}\nimg UnknownMap 1\n", x, u8::from(hidden));
    let mut images: BTreeSet<Vec<u8>> = BTreeSet::new();
    images.insert(b"UnknownMap".to_vec());
    for i in IMAGES {
        let on = r.below(4) != 0;
        text += &format!("img {i} {}\n", u8::from(on));
        if on {
            images.insert(i.as_bytes().to_vec());
        }
    }
    let mut maps: BTreeMap<Vec<u8>, MapMetaData> = BTreeMap::new();
    let mut previews: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
    for _ in 0..1 + r.below(5) {
        let key = KEYS[r.below(KEYS.len() as u64) as usize];
        if maps.contains_key(key.as_bytes()) {
            continue;
        }
        let lo = c3(if r.below(2) == 0 { 0.0 } else { r.quarter(50) }, if r.below(2) == 0 { 0.0 } else { r.quarter(50) });
        // (one map in eight has the window's own aspect, `k` times its size: the band test's tie)
        let hi = if r.below(8) == 0 {
            let k = (1 + r.below(8)) as f32;
            c3(lo.x + w as f32 * k, lo.y + h as f32 * k)
        } else {
            c3(lo.x + 50.0 + r.quarter(2000), lo.y + 50.0 + r.quarter(2000))
        };
        let players = r.below(9) as i32;
        let mp = r.below(5) != 0;
        let mut m = MapMetaData { extent: Region3D { lo, hi }, num_players: players, is_multiplayer: mp, ..Default::default() };
        text += &format!("map {key} {} {} {} {} {players} {}\n", lo.x, lo.y, hi.x, hi.y, u8::from(mp));
        for k in 1..=players {
            if r.below(10) == 0 {
                continue;
            }
            let p = if r.below(4) == 0 && k > 1 { c3(lo.x + r.quarter(30), hi.y - r.quarter(30)) } else { c3(lo.x + r.quarter((hi.x - lo.x) as u64), lo.y + r.quarter((hi.y - lo.y) as u64)) };
            let n = format!("Player_{k}_Start");
            text += &format!("wp {key} {n} {} {}\n", p.x, p.y);
            m.waypoints.map.insert(n.into_bytes(), p);
        }
        for (kind, list) in [("supply", 0), ("tech", 1)] {
            for _ in 0..r.below(5) {
                let p = c3(lo.x + r.quarter((hi.x - lo.x) as u64), lo.y + r.quarter((hi.y - lo.y) as u64));
                text += &format!("{kind} {key} {} {}\n", p.x, p.y);
                if list == 0 {
                    m.supply_positions.push(p);
                } else {
                    m.tech_positions.push(p);
                }
            }
        }
        if r.below(3) != 0 {
            let tga = format!("{}.tga", &key[..key.len() - 4]);
            let tga = if r.below(3) == 0 { tga.to_ascii_uppercase() } else { tga };
            let bytes = format!("TGA{}", r.below(1000));
            text += &format!("file {tga} {bytes}\n");
            previews.insert(tga.to_ascii_lowercase().into_bytes(), bytes.into_bytes());
        }
        maps.insert(key.as_bytes().to_vec(), m);
    }
    let keys: Vec<Vec<u8>> = maps.keys().cloned().collect();
    let select: Vec<u8> = match r.below(6) {
        0 => b"maps\\unknown\\unknown.map".to_vec(),
        1 => Vec::new(),
        2 => keys[r.below(keys.len() as u64) as usize].to_ascii_uppercase(),
        _ => keys[r.below(keys.len() as u64) as usize].clone(),
    };
    // the Rust side: the shell's host on the synthetic file
    let files = Files { wnd: wnd(x, y, w, h, hidden, bw, bh), images, keys: BTreeMap::new() };
    let mut gui = ShellGui::new(files);
    gui.skirmish_data.user_data_path = if r.below(2) == 0 { b"User\\".to_vec() } else { b"C:\\Data\\".to_vec() };
    gui.skirmish_data.map_cache.maps = maps;
    gui.skirmish_data.map_previews = previews;
    let portable = if select.len() >= 4 { gui.skirmish_data.real_map_path_to_portable(&select[..select.len() - 4]) } else { Vec::new() };
    text += &format!("portable {}\n", if portable.is_empty() { "-".to_string() } else { String::from_utf8_lossy(&portable).into_owned() });
    text += &format!("select {}\n", if select.is_empty() { "-".to_string() } else { String::from_utf8_lossy(&select).into_owned() });
    let mut wm = GameWindowManager::new((800, 600));
    ShellHost::create_from_script(&mut gui, &mut wm, b"Menus/PreviewTest.wnd").expect("the synthetic file reads");
    let find = |wm: &GameWindowManager, n: &str| wm.tree.find_by_name(format!("SkirmishGameOptionsMenu.wnd:{n}").as_bytes()).unwrap_or_else(|| panic!("{n}"));
    let mw = find(&wm, "MapWindow");
    let buttons: [Option<WinId>; 8] = std::array::from_fn(|i| Some(find(&wm, &format!("ButtonMapStartPosition{i}"))));
    let before: BTreeSet<Vec<u8>> = gui.user_data.files.keys().cloned().collect();
    position_start_spots(&mut wm, &mut gui, &select, &buttons, Some(mw));
    let mut o = String::new();
    for (p, b) in &gui.user_data.files {
        if !before.contains(p) {
            o += &format!("fs write {} {} {}\n", String::from_utf8_lossy(p), b.len(), String::from_utf8_lossy(b));
        }
    }
    let user = match &wm.ext[mw].user {
        UserData::MapMetaData(Some(k)) => String::from_utf8_lossy(k).into_owned(),
        _ => "NULL".to_string(),
    };
    o += &format!("user {user}\n");
    let img = wm.tree.windows[mw].inst.enabled_draw_data[0].image.clone();
    o += &format!("status {} image {}\n", u8::from(wm.tree.windows[mw].status & win_status::IMAGE != 0), img.map_or("NULL".to_string(), |i| String::from_utf8_lossy(&i).into_owned()));
    for (i, b) in buttons.iter().enumerate() {
        let b = b.expect("button");
        let win = &wm.tree.windows[b];
        o += &format!("button {i} {} {} {}\n", win.region.0, win.region.1, u8::from(win.status & win_status::HIDDEN != 0));
    }
    for p in &gui.skirmish.supply_pos_list {
        o += &format!("supply {} {}\n", p.x, p.y);
    }
    for p in &gui.skirmish.tech_pos_list {
        o += &format!("tech {} {}\n", p.x, p.y);
    }
    for m in &gui.map_preview_images {
        o += &format!("preview {} {} {} {} {} {} {} {} {}\n", String::from_utf8_lossy(&m.name), String::from_utf8_lossy(&m.filename), m.status, m.uv[0], m.uv[1], m.uv[2], m.uv[3], m.texture_size.0, m.texture_size.1);
    }
    let mut items = Vec::new();
    GuiHost::draw(&mut gui, &wm, b"W3DDrawMapPreview", mw, &mut items);
    let mut default = Vec::new();
    w3d_game_win_default_draw(&wm, mw, &mut default);
    let mut skip = 0;
    if !matches!(wm.ext[mw].user, UserData::MapMetaData(Some(_))) {
        o += "draw default\n";
        skip = default.len();
    }
    for d in &items[skip..] {
        match d {
            DrawItem::FillRect { x, y, w, h, color } => o += &format!("draw fill {x} {y} {w} {h} #{:08x}\n", *color as u32),
            DrawItem::Line { x0, y0, x1, y1, color } => o += &format!("draw line {x0} {y0} {x1} {y1} #{:08x}\n", *color as u32),
            DrawItem::Image { image, x0, y0, x1, y1, .. } => o += &format!("draw image {} {x0} {y0} {x1} {y1}\n", String::from_utf8_lossy(image)),
            other => o += &format!("draw other {other:?}\n"),
        }
    }
    if let Some(u) = &wm.undefined {
        o += &format!("undefined {u}\n");
    }
    (text, o)
}

fn cpp_run(text: &str) -> String {
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 15;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_map_preview_run(input.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

#[test]
fn the_map_preview_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("MAP_PREVIEW_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x4d61_7050_7276_9157);
    let (mut mismatches, mut spots, mut draws, mut previews, mut found) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for n in 0..cases {
        let (text, rust) = case(&mut r);
        let c = cpp_run(&text);
        spots += rust.lines().filter(|l| l.starts_with("button") && l.ends_with(" 0")).count();
        draws += rust.lines().filter(|l| l.starts_with("draw")).count();
        previews += rust.lines().filter(|l| l.starts_with("preview")).count();
        found += usize::from(!rust.starts_with("fs write") && !rust.starts_with("user NULL"));
        if c != rust {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {n}:\n{text}\n  rust:\n{rust}\n  c++:\n{c}");
            }
        }
    }
    println!("#157 map preview: {cases} cases, {spots} start spots shown, {previews} preview images made, {found} maps found without a `.tga`, {draws} draw calls; {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
