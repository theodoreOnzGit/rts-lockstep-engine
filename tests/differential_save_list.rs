#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (addGameToAvailableList,
//     populateSaveGameListbox, iterateSaveFiles, clearAvailableGames, SaveDate::isNewerThan)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The save/load screen's list of saves (`client::save_load_menu::populate_save_game_listbox`) against the C++, #78.
//!
//! # Method
//!
//! The C++ side is `GameState.h` and the pieces of `GameState.cpp` build.rs cuts verbatim (SaveGameInfo's constructor,
//! `SaveDate::isNewerThan`, `clearAvailableGames`, `addGameToAvailableList` and `populateSaveGameListbox`,
//! `iterateSaveFiles`), compiled by `differential/wrap_save_list.cpp` on stand-ins for the save directory (Win32's
//! directory search: the case's entries, `.` and `..` first), `getSaveGameInfoFromFile` (the case's information, or a
//! throw), the date formats (en-US), the game text (`T:` + the label; `NOTEXT*` labels missing) and the list box (a
//! table of rows: the gadget is `tests/differential_wm.rs`'s). The Rust side runs `populate_save_game_listbox` on a
//! real list box made from the synthetic `PopupSaveLoad.wnd` (three columns), with a host whose save directory is the
//! case's files (its directories left out, the throwing ones `None`: the host's part, as the client's folder is).
//!
//! A case: the layout type (save-and-load, load-only, save-only), and up to 9 entries: names from a pool (`.sav` in any
//! case, `.sav.bak`, `.txt`, no extension, `.sav` alone, a directory named `.sav`), one in six unreadable, dates drawn
//! from small ranges so that many are equal or differ in one field (the day of the week too, which `isNewerThan` does
//! not compare), descriptions empty or not, map labels with and without text or empty, the three file types. Both sides
//! dump each row's three cells (text and colour, or `-` for a cell never written), the save its item data names, and the
//! selection.
//!
//! # Results (2026-11-10, `SAVE_LIST_CASES=20000`, the default)
//!
//! Predicted before the run: 0 mismatches. **First run: 1,229 mismatches**, all the selection on a list with no rows
//! (load-only, no readable save): the Rust's real list box keeps no selection (`GLM_SET_SELECTION` of a row with no
//! cells leaves `selectPos`, -1 after the reset), the C++ stand-in had recorded the 0 asked for. The stand-in was wrong
//! (it now does what GadgetListBox.cpp's `GLM_SET_SELECTION` does with the index); the translation was not changed.
//! **Then:** 20,000 cases, 73,267 directory entries, 53,965 rows compared, **0 mismatches**.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (each in its own run): 3 of 3 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 7801 | the newest last (`isNewerThan` read backwards) | 12,095 cases |
//! | 7802 | the colours' alternation counted from 1 | 14,199 cases |
//! | 7803 | row 1 selected instead of row 0 | 18,771 cases (all but the 1,229 empty lists) |

use rts_lockstep_engine::client::main_menu::{MainMenu, MenuHost};
use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::save_load_menu::{populate_save_game_listbox, SaveLoadHost, SaveLoadMenu};
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::shell_gui::synthetic::save_load_wnd;
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::{WinId, WindowScript, WndHost};
use rts_lockstep_engine::common::game_state::{SaveDate, SaveGameInfo};
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;

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

/// One directory entry of a case.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    dir: bool,
    throws: bool,
    /// year, month, day of week, day, hour, minute, second, milliseconds (`SYSTEMTIME`'s order)
    date: [u16; 8],
    desc: String,
    label: String,
    file_type: i32,
}

const NAMES: [&str; 12] = ["00000000.sav", "00000001.sav", "00000002.SAV", "Mine.Sav", "a.sav.bak", "notes.txt", "noext", ".sav", "x.y.sav", "00000003.sav", "zz.sav", "dir.sav"];
const DESCS: [&str; 4] = ["", "Alpha", "Bravo two", ""];
const LABELS: [&str; 4] = ["MAP:A", "NOTEXT:B", "", "MAP:C"];

fn gen_case(r: &mut Rng) -> (i32, Vec<Entry>) {
    let layout = 1 + r.below(3) as i32;
    let n = r.below(10) as usize;
    let mut out: Vec<Entry> = Vec::new();
    for _ in 0..n {
        let name = NAMES[r.below(NAMES.len() as u64) as usize].to_string();
        if out.iter().any(|e| e.name == name) {
            continue;
        }
        let dir = name == "dir.sav";
        let date = [2025 + r.below(2) as u16, 1 + r.below(2) as u16, r.below(7) as u16, 1 + r.below(2) as u16, r.below(24) as u16, r.below(2) as u16, r.below(2) as u16, r.below(2) as u16];
        out.push(Entry {
            name,
            dir,
            throws: r.below(6) == 0,
            date,
            desc: DESCS[r.below(4) as usize].to_string(),
            label: LABELS[r.below(4) as usize].to_string(),
            file_type: r.below(3) as i32,
        });
    }
    (layout, out)
}

/// The Rust side's host: the save/load host over the case's directory, without a shell or files to read.
struct Host {
    shell: Shell,
    main_menu: MainMenu,
    save_load: SaveLoadMenu,
    files: Vec<(Vec<u8>, Option<SaveGameInfo>)>,
}

impl GuiHost for Host {
    fn system(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _win: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        MsgHandled::Ignored
    }
    fn input(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _win: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        MsgHandled::Ignored
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
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        0
    }
}

impl MenuHost for Host {
    fn main_menu(&mut self) -> &mut MainMenu {
        &mut self.main_menu
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        [b"T:".as_slice(), label].concat().iter().map(|&b| u16::from(b)).collect()
    }
}

impl SaveLoadHost for Host {
    fn save_load_menu(&mut self) -> &mut SaveLoadMenu {
        &mut self.save_load
    }
    fn save_files(&mut self) -> Vec<(Vec<u8>, Option<SaveGameInfo>)> {
        self.files.clone()
    }
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        (self.fetch_text(label), !label.starts_with(b"NOTEXT"))
    }
    fn delete_save_file(&mut self, _filename: &[u8]) {}
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

fn rust_run(layout: i32, entries: &[Entry]) -> String {
    let files = entries
        .iter()
        .filter(|e| !e.dir)
        .map(|e| {
            let d = e.date;
            let info = SaveGameInfo {
                date: SaveDate { year: d[0], month: d[1], day_of_week: d[2], day: d[3], hour: d[4], minute: d[5], second: d[6], milliseconds: d[7] },
                description: e.desc.encode_utf16().collect(),
                map_label: e.label.as_bytes().to_vec(),
                save_file_type: e.file_type,
                ..Default::default()
            };
            (e.name.as_bytes().to_vec(), (!e.throws).then_some(info))
        })
        .collect();
    let mut h = Host { shell: Shell::new(), main_menu: MainMenu::default(), save_load: SaveLoadMenu::default(), files };
    let mut wm = GameWindowManager::new((800, 600));
    let mut wnd = Wnd(BTreeMap::new());
    wm.win_create_from_script(&mut WindowScript::new(), b"Menus/PopupSaveLoad.wnd", &save_load_wnd(true), &mut wnd, &mut h).expect("the synthetic file reads");
    let l = wm.tree.find_by_name(b"PopupSaveLoad.wnd:ListboxGames").expect("the list box");
    populate_save_game_listbox(&mut wm, &mut h, Some(l), layout);
    assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    let mut o = String::new();
    for r in 0..lb::get_num_entries(&wm, l) {
        o += &format!("row {r}:");
        for c in 0..3 {
            let (t, color) = lb::get_text_and_color(&mut wm, &mut h, l, r, c);
            if t.is_empty() {
                o += " -";
            } else {
                o += &format!(" [{}]#{:08x}", String::from_utf16_lossy(&t), color as u32);
            }
        }
        let d = lb::get_item_data(&mut wm, &mut h, l, r, 0);
        let name = if d == 0 { "NULL".to_string() } else { String::from_utf8_lossy(&h.save_load.available_games[d as usize - 1].filename).into_owned() };
        o += &format!(" data {name}\n");
    }
    let sel = lb::get_selected(&mut wm, &mut h, l);
    o += &format!("selected {}\n", sel.first().copied().unwrap_or(-99));
    o
}

fn cpp_run(layout: i32, entries: &[Entry]) -> String {
    let mut text = format!("{layout}\n");
    for e in entries {
        let d = e.date;
        text += &format!(
            "{}|{} {} {} {} {} {} {} {} {} {} {}|{}|{}\n",
            e.name,
            u8::from(e.dir),
            u8::from(e.throws),
            d[0],
            d[1],
            d[2],
            d[3],
            d[4],
            d[5],
            d[6],
            d[7],
            e.file_type,
            e.desc,
            e.label
        );
    }
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 14;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_save_list_run(input.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

#[test]
fn the_list_of_saves_matches_upstream() {
    let cases: usize = std::env::var("SAVE_LIST_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x7853_4c31_cafe_0078);
    let (mut mismatches, mut rows, mut entries_n) = (0usize, 0usize, 0usize);
    for case in 0..cases {
        let (layout, entries) = gen_case(&mut r);
        entries_n += entries.len();
        let a = rust_run(layout, &entries);
        let b = cpp_run(layout, &entries);
        if a != b {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {case}: layout {layout} entries {entries:?}\n  rust:\n{a}  c++:\n{b}");
            }
        } else {
            rows += a.lines().count() - 1;
        }
    }
    println!("#78 save list: {cases} cases, {entries_n} directory entries, {rows} rows matched, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
