// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui4) of the draft `src/drafts/ui/game_info_window.rs` (`CreateLANGameInfoWindow`, `DestroyGameInfoWindow`,
//! `RefreshGameInfoWindow`, `HideGameInfoWindow`, `GameInfoWindowInit`, `GameInfoWindowSystem`, the file's statics) against upstream `GameInfoWindow.cpp`, cut
//! verbatim by build.rs (`drafts_ui4`) into the program `UI4_GAMEINFO_EXE` (differential/wrap_draft_ui4_gameinfo.cpp lists the stand-ins and the edits), which
//! also holds upstream's real window manager, window layout and gadgets (list box with two columns and image cells, static text, user windows: #70's program).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). Each is a scene of named windows (`GameInfoWindow.wnd:...`): the parent (menu callbacks; missing one
//! time in six), a decoy top-level window, and under the parent the two static texts, the player list (two columns, `ImgSide` / `Gameinfo` icons in the first,
//! 1 to 12 rows) and the three option windows, each present with probability 5/6 and hidden one time in six; a layout of the parent (or of the decoy). 4 to 24
//! operations: `CreateLANGameInfoWindow` (of a window or of NULL; it runs the layout's init, `GameInfoWindowInit`), `GameInfoWindowInit`,
//! `DestroyGameInfoWindow` (once, last), `HideGameInfoWindow`, `RefreshGameInfoWindow` (with or without a game info), the game's data set (the map: upper case, a
//! path with `\`, bytes above 0x7F, empty; the first player's name; eight slots, occupied or not, human or easy, medium or hard AI, colours -2 to 6 against 0 to 5
//! defined, player templates -3 to 5 against 0 to 5 in the store, the observer's -2; the map cache's entries, hit by the lower-cased map or not), the system callback
//! (`GWM_CREATE`, `GWM_DESTROY`, `GWM_INPUT_FOCUS` with a `mData1` other than TRUE, other messages), hiding, enabling and focusing windows. After each operation both
//! programs print the stand-in calls (the layout made, its init), the messages, the dump of every window (status, position, size, text, the list box's rows and cells,
//! with the image names), the static texts, the layouts and the file's statics; the texts must be identical.
//!
//! Excluded inputs (upstream's undefined behaviour or a deliberate deviation of the draft): `RefreshGameInfoWindow` with no parent window (upstream's
//! `parent->winHide` on NULL), and with a game slot that is occupied but neither human nor easy / medium / hard AI (upstream's `addedRow` is uninitialised; the
//! game's `isOccupied` is `isHuman || isAI`, so the slot cannot exist); `CreateLANGameInfoWindow` after `DestroyGameInfoWindow` (the destroyed windows and the layout
//! are not rebuilt in the test); `GWM_INPUT_FOCUS` sent directly with `mData1` TRUE. Cases where the Rust window manager reports upstream's undefined behaviour
//! stop before that operation.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! One mismatch kind in the draft, from reading `UnicodeString::translate`: the leaf name of a map that is not in the cache is widened with `c as u16` (a byte
//! 0xE9 becomes 0x00E9) where upstream's `(WideChar)getCharAt(i)` sign-extends the `char` (0xFFE9). Nothing else.
//!
//! # Planted bugs (`L9_PLANT=<id>`, the draft's `plant(id)`; mismatching cases of 3,000)
//!
//! | id | planted bug | cases |
//! |---|---|---|
//! | 21493 | `CreateLANGameInfoWindow` does not run the layout's init | 1,898 |
//! | 21494 | it does not bring the layout forward | 1,454 |
//! | 21495 | it leaves the layout shown | 1,898 |
//! | 21496 | the popup takes the sizing window's own, not its screen, position | 1,081 |
//! | 21497 | the popup's width and height are swapped | 795 |
//! | 21498 | the map is not lower-cased for the cache | 56 |
//! | 21499 | the leaf name starts after the first `\`, not the last | 168 |
//! | 21500 | colour 0 is taken as no colour | 32 |
//! | 21501 | the observer is template -1, not -2 | 166 |
//! | 21502 | a template equal to the store's count gets its icon | 102 |
//! | 21503 | the medium AI is labelled easy | 131 |
//! | 21504 | the list is not reset | 235 |
//! | 21505 | `HideGameInfoWindow` hides when told to show | 984 |
//! | 21506 | `GameInfoWindowInit` does not blank the map name | 225 |
//! | 21507 | `GWM_INPUT_FOCUS` is answered ignored | 348 |
//! | 21508 | `DestroyGameInfoWindow` keeps the layout | 67 |
//!
//! **16 of 16 caught.**
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences, 518,909 operations compared** (the `DIFF` line is printed by the run), max ULP 0.
//! The run before the fixes had **two kinds of mismatch in the draft** (13 in the first 3,000 sequences besides harness ones):
//! 1. the image cells of the player list were not added (the cell `{0 0 0 0}` against `{2 -1 25 22 img GameinfoOBSRVR}`): the draft passed `overwrite` FALSE to
//!    `add_entry_text` and `add_entry_image`, where GadgetListBox.h's default is **TRUE** (`Bool overwrite = TRUE`) and the image goes into the row just added;
//!    with FALSE and the list full (`insertPos == listLength`) `GLM_ADD_ENTRY` refuses. **Fixed** (TRUE, as upstream's calls, also in `disconnect_menu.rs` and
//!    `disconnect_window.rs`, where the row is -1 and it makes no difference);
//! 2. the leaf name's non-ASCII bytes (the prediction): `c as u16` against the sign extension. **Fixed** (`c as i8 as i16 as u16`).
//! Harness slips on the way (none in the draft): the window manager's image collection found only `Img*` names (the generated base now also finds `Gameinfo*`, one
//! asserted edit in build.rs) and the Rust host's `has_image` said no; a deleted layout (`deleteInstance`) was read after the free (now its dump line is kept at the
//! deletion); `getMap()` returned a copy whose `reverseFind` pointer dangled (upstream's AsciiString shares its buffer: the stand-in returns a reference).
//!
//! Not covered: nothing in the file; `TheMapCache`, `ThePlayerTemplateStore`, `TheMultiplayerSettings` and the game info are stand-ins (data), the layout's reader is
//! not run.

mod support;
mod ui4_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, GameFont, WinId};
use rts_lockstep_engine::drafts::ui::game_info_window::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"GameInfoWindowSystem";

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Option<Shell>,
    gi: GameInfoWindow,
    members: Vec<usize>,
    map: Vec<u8>,
    name0: Vec<u16>,
    slots: [Option<InfoSlot>; 8],
    cache: BTreeMap<Vec<u8>, Vec<u16>>,
    colors: Vec<i32>,
    templates: i32,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            game_info_window_system(wm, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            game_info_window_system(wm, win, msg, d1, d2)
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
    /// `TheMappedImageCollection->findImageByName(name) != NULL` (differential/wrap_wm.cpp's: `Img*` names, and here `Gameinfo*`).
    fn has_image(&self, image: &[u8]) -> bool {
        let lower = image.to_ascii_lowercase();
        lower.starts_with(b"img") || lower.starts_with(b"gameinfo")
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        self.shell.get_or_insert_with(Shell::new)
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        let _ = writeln!(self.log, "layout {}", String::from_utf8_lossy(filename));
        Some(make_layout(wm, &self.members))
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

impl GameInfoWindowHost for Host {
    fn game_info_window(&mut self) -> &mut GameInfoWindow {
        &mut self.gi
    }
    fn layout_run_init(&mut self, wm: &mut GameWindowManager, layout: usize) {
        self.log.push_str("layout init\n");
        game_info_window_init(wm, self, layout);
    }
    fn game_info_player_name_0(&self) -> Vec<u16> {
        self.name0.clone()
    }
    fn game_info_map(&self) -> Vec<u8> {
        self.map.clone()
    }
    fn map_cache_display_name(&self, lower_map: &[u8]) -> Option<Vec<u16>> {
        self.cache.get(lower_map).cloned()
    }
    fn game_info_occupied_slot(&self, i: usize) -> Option<InfoSlot> {
        self.slots.get(i).cloned().flatten()
    }
    fn multiplayer_num_colors(&self) -> i32 {
        self.colors.len() as i32
    }
    fn multiplayer_color(&self, c: i32) -> i32 {
        self.colors[c as usize]
    }
    fn player_template_count(&self) -> i32 {
        self.templates
    }
    fn player_template_side_icon(&self, n: i32) -> Option<Vec<u8>> {
        (n % 5 != 4).then(|| format!("ImgSide{n}").into_bytes())
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        format!("T:{}", String::from_utf8_lossy(label)).encode_utf16().collect()
    }
}

const TEXTS: [&str; 7] = ["-", "41", "41,42,43", "20,41,20", "e9,4e2d", "61,20,62", "4e2d,4e2d,4e2d,4e2d"];
const MAPS: [&[u8]; 9] = [b"", b"Desert", b"maps\\Tournament\\Desert.map", b"MAPS\\Big\\Alpine Assault\\a.map", b"maps\\caf\xe9\\M.map", b"\\", b"x\\", b"maps\\d\\D\xe9sert\xff.map", b"\xe9\xe9"];

struct Scene {
    ops: Vec<String>,
    members: Vec<usize>,
    wins: Vec<usize>,
    parent_present: bool,
}

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn gen_scene(g: &mut Gen) -> Scene {
    let mut ops = Vec::new();
    let mut wins = Vec::new();
    let en = win_status::ENABLED;
    let mut next = 0usize;
    let parent_present = g.below(6) != 0;
    let mut tops = Vec::new();
    if parent_present {
        ops.push(format!("@ wu -1 0 20 30 400 300 {en} {}", gws::USER_WINDOW));
        tops.push(0usize);
        wins.push(0);
        next += 1;
    }
    ops.push(format!("@ wu -1 0 300 200 100 100 {en} {}", gws::USER_WINDOW));
    tops.push(next);
    wins.push(next);
    next += 1;
    let mut named = Vec::new();
    if parent_present {
        for (k, (name, kind)) in [("StaticTextGameName", 0), ("StaticTextMapName", 0), ("ListBoxPlayers", 1), ("WinCrates", 2), ("WinSuperWeapons", 2), ("WinFreeForAll", 2)].into_iter().enumerate() {
            if g.below(6) == 0 {
                continue;
            }
            let (x, y) = (10 + (k as i32 % 3) * 120, 10 + (k as i32 / 3) * 100);
            let mut status = en;
            if g.below(6) == 0 {
                status |= win_status::HIDDEN;
            }
            let line = match kind {
                0 => format!("W 0 11 {x} {y} 100 20 {status} {} 0", gws::STATIC_TEXT),
                1 => format!("W 0 6 {x} {y} 100 80 {status} {} 0 {} 2 {} 0 0 0 0 0 20 80", gws::SCROLL_LISTBOX, 1 + g.below(12), u8::from(g.below(2) == 0)),
                _ => format!("@ wu 0 0 {x} {y} 50 50 {status} {}", gws::USER_WINDOW),
            };
            ops.push(line);
            named.push((next, name));
            wins.push(next);
            next += 1;
        }
    }
    if parent_present {
        ops.push("@ nm 0 GameInfoWindow.wnd:ParentGameInfo".to_string());
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} GameInfoWindow.wnd:{name}"));
    }
    let members = if parent_present && g.below(8) != 0 { vec![0] } else { vec![*tops.last().expect("a top")] };
    ops.push(format!("@ lay {} {}", members.len(), members.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")));
    Scene { ops, members, wins, parent_present }
}

fn pick<T: Copy>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize]
}

fn gen_op(g: &mut Gen, s: &Scene, map: &mut Vec<u8>, destroyed: &mut bool) -> String {
    let text = TEXTS[g.below(TEXTS.len() as u32) as usize];
    let win = |g: &mut Gen| pick(g, &s.wins);
    match g.below(100) {
        0..=7 if !*destroyed => format!("@ gi create {}", if g.below(4) == 0 { -1 } else { win(g) as i32 }),
        8..=11 if !*destroyed => "@ gi init".into(),
        12 if !*destroyed && g.below(3) == 0 => {
            *destroyed = true;
            "@ gi destroy".into()
        }
        13..=19 => format!("@ gi hide {}", g.below(2)),
        20..=39 if s.parent_present => format!("@ gi refresh {} {text}", u8::from(g.below(6) != 0)),
        40..=45 => {
            *map = MAPS[g.below(MAPS.len() as u32) as usize].to_vec();
            format!("@ gi map {}", hex(map))
        }
        46..=48 => format!("@ gi name0 {text}"),
        49..=62 => {
            let i = g.below(8);
            if g.below(3) == 0 {
                format!("@ gi slot {i} 0 0 0 0 0 0 -")
            } else {
                let human = g.below(2) == 0;
                let st = if human { 3 } else { g.below(3) };
                let (color, tmpl) = (g.below(9) as i32 - 2, g.below(9) as i32 - 3);
                format!("@ gi slot {i} 1 {} {} {st} {color} {tmpl} {}", u8::from(!human), u8::from(human), if human { text } else { "-" })
            }
        }
        63..=70 => {
            let key: Vec<u8> = if g.below(2) == 0 { map.to_ascii_lowercase() } else { pick(g, &MAPS).to_ascii_lowercase() };
            format!("@ gi cache {} {text}", hex(&key))
        }
        71..=74 => {
            let n = g.below(6);
            let cs: Vec<String> = (0..n).map(|_| (g.below(0xFFFFFF) as i32 | 0xFF00_0000u32 as i32).to_string()).collect();
            format!("@ gi colors {n} {}", cs.join(" ")).trim_end().to_string()
        }
        75..=77 => format!("@ gi templates {}", g.below(6)),
        78..=85 => {
            let msg = pick(g, &[1, 2, 3, 4, 21, 23, 24, 25, 16392]);
            let d1 = if msg == gwm::INPUT_FOCUS { 2 + g.below(2) } else { g.below(3) };
            format!("@ sys 0 0 {msg} 0 {d1} 0")
        }
        86..=90 => format!("H {} {}", win(g), g.below(2)),
        91..=94 => format!("E {} {}", win(g), g.below(2)),
        95..=97 => format!("F {}", win(g)),
        _ => "@ gi init".to_string(),
    }
}

fn state_dump(wm: &GameWindowManager, h: &mut Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let g = &h.gi;
    let _ = writeln!(
        o,
        "gi parent {} gn {} mn {} list {} crates {} super {} ffa {} layout {} ids {}{}{}{}{}{}{}",
        idx(g.parent),
        idx(g.static_text_game_name),
        idx(g.static_text_map_name),
        idx(g.list_box_players),
        idx(g.win_crates),
        idx(g.win_super_weapons),
        idx(g.win_free_for_all),
        g.game_info_window_layout.map_or(-1, |l| l as i64),
        u8::from(g.parent_id != 0),
        u8::from(g.static_text_game_name_id != 0),
        u8::from(g.static_text_map_name_id != 0),
        u8::from(g.list_box_players_id != 0),
        u8::from(g.win_crates_id != 0),
        u8::from(g.win_super_weapons_id != 0),
        u8::from(g.win_free_for_all_id != 0)
    );
    o
}

fn hexs(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}
fn hexb(t: &str) -> Vec<u8> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u8::from_str_radix(x, 16).expect("hex")).collect() }
}

fn menu_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) {
    let v: Vec<&str> = op.split(' ').collect();
    let num = |i: usize| -> i64 { v[i].parse().expect("number") };
    match v[1] {
        "sys" | "inp" => {
            let (win, msg, kind, d1, d2) = (num(3) as usize, num(4) as u32, num(5), num(6) as u32, num(7) as u32);
            let a = if kind == 1 { win_data(Some(d1 as usize)) } else { d1 };
            let r = game_info_window_system(wm, win, msg, a, d2);
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "gi" => match v[2] {
            "create" => {
                let w = num(3);
                create_lan_game_info_window(wm, h, (w >= 0).then_some(w as usize));
            }
            "destroy" => destroy_game_info_window(wm, h),
            "init" => game_info_window_init(wm, h, 0),
            "hide" => hide_game_info_window(wm, h, num(3) != 0),
            "refresh" => refresh_game_info_window(wm, h, num(3) != 0, &hexs(v[4])),
            "map" => h.map = hexb(v[3]),
            "name0" => h.name0 = hexs(v[3]),
            "slot" => {
                let i = num(3) as usize;
                h.slots[i] = (num(4) != 0).then(|| InfoSlot {
                    is_ai: num(5) != 0,
                    is_human: num(6) != 0,
                    state: match num(7) {
                        0 => SlotState::EasyAi,
                        1 => SlotState::MedAi,
                        2 => SlotState::BrutalAi,
                        _ => SlotState::Other,
                    },
                    color: num(8) as i32,
                    name: hexs(v[10]),
                    player_template: num(9) as i32,
                });
            }
            "cache" => {
                h.cache.insert(hexb(v[3]), hexs(v[4]));
            }
            "colors" => h.colors = (0..num(3) as usize).map(|i| num(4 + i) as i32).collect(),
            "templates" => h.templates = num(3) as i32,
            other => panic!("gi {other}"),
        },
        other => panic!("@ {other}"),
    }
}

#[test]
fn game_info_window_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C4_03);
    let mut chk = Check::new("GameInfoWindow");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, SYS)];
    for case in 0..cases(20_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 4 + g.below(21);
        let mut ops = scene.ops.clone();
        let mut map = Vec::new();
        let mut destroyed = false;
        for _ in 0..steps {
            let op = gen_op(&mut g, &scene, &mut map, &mut destroyed);
            ops.push(op);
        }
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Host { members: scene.members.clone(), ..Default::default() };
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            if op.starts_with("@ gi") || op.starts_with("@ sys") {
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
        compare_case(env!("UI4_GAMEINFO_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
