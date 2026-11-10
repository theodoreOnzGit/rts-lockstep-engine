#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ReplayMenu.cpp (GetReplayFilenameFromListbox,
//   PopulateReplayFileListbox)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The replay list (`client::replay_menu::populate_replay_file_listbox`, `get_replay_filename_from_listbox`) against
//! the C++, #98.
//!
//! # Method
//!
//! The C++ side is the two functions build.rs cuts verbatim from `ReplayMenu.cpp`, compiled by
//! `differential/wrap_replay_list.cpp` on stand-ins for the recorder (the case's headers, by file name, for the replay
//! folder's files; a field the case leaves unread is flagged when the code reads it), `ParseAsciiStringToGameInfo`
//! (the case's answer and map), the file system's listing into the **real** `FilenameList` ordering (the real
//! STLTypedefs.h `less_than_nocase<AsciiString>`), the map cache, the version, the global data's CRCs, the game text's
//! `GUI:LastReplay`, `getUnicodeTimeBuffer` (the host's en-US `h:mm tt`) and the list box (a table of rows of four
//! cells). The Rust side runs the translated functions on a real list box (four columns) with a host whose replay
//! folder is the client's `MemoryReplayFolder` (its listing and its header lookup: the code the shell runs) over the
//! same files. Both dump each row's four texts and colours, the selection, whether an undefined step was met, and
//! `GetReplayFilenameFromListbox` of every row and of row -1.
//!
//! A case: up to 12 files from a pool of 18 paths (the last replay, two names differing only in case, sub-folder
//! files whose name is or is not also a top file, a file outside the folder, `.repx` and `.txt` files, an empty name,
//! a name with byte 0xE9, the folder spelt in lower case); a header for most names (some unreadable, some whose
//! options do not parse; the version, the exe and INI CRCs equal to this build's or not, now and then unread; the
//! time valid, out of range or unread; local player -1 or a slot); maps from a pool of 6, 3 in the cache; the last
//! replay's name `00000000` or a pool name; `GUI:LastReplay`'s text, sometimes a pool name (the row then reads as the
//! last replay); rarely no map cache.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 8961 | the last replay's file matched with case | caught: the pool spells `00000000.REP` and the last name `mine` / `Mine.REP` |
//! | 8962 | one character fewer cut from the name | caught in nearly every case with a row |
//! | 8963 | the colours of a good and a bad version swapped | caught in nearly every case with a row |
//! | 8964 | the second row selected | caught in every case with two rows |
//! | 8965 | `GUI:LastReplay`'s text not recognised in `GetReplayFilenameFromListbox` | caught in every case with the last replay listed |
//!
//! # Results (2026-11-10, `REPLAY_LIST_CASES=20000`, the default)
//!
//! **0 mismatches on the first run.** That run had an unread or out-of-range field in one header of 12 and 11,323 of
//! the 20,000 cases met an undefined step (compared on that flag only); the generator was then made to leave fields
//! unread one time in 40 (not a protocol change for the comparison: the same code on both sides), and gave: 20,000
//! cases, 98,837 files, 32,718 rows matched (3,809 reading as the last replay), 5,074 cases undefined (compared on that
//! only), **0 mismatches**.
//!
//! Planted bugs: 5 of 5 caught: 8961 in 1,679 cases, 8962 in 11,145, 8963 in 11,588 (every case with a row), 8964 in
//! 11,588 (**missed the prediction's reach**: every case with a row, not only those with two: on a one-row list the
//! second row does not exist and nothing is selected), 8965 in 7,155.

use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::main_menu::{MainMenu, MenuHost};
use rts_lockstep_engine::client::map_util::MapUtilHost;
use rts_lockstep_engine::client::replay_menu::{get_replay_filename_from_listbox, populate_replay_file_listbox, MemoryReplayFolder, ReplayListHost, ReplayListing, ReplayMenu, ReplayVersion};
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{WinId, WindowScript, WndHost};
use rts_lockstep_engine::common::map_cache::{MapCache, MapMetaData};
use rts_lockstep_engine::common::recorder::ReplayHeader;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::big::FilenameList;
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

const FILES: [&[u8]; 18] = [
    b"Replays\\00000000.rep",
    b"Replays\\00000000.REP",
    b"Replays\\Mine.REP",
    b"Replays\\mine.rep",
    b"Replays\\alpha.rep",
    b"Replays\\Beta.rep",
    b"Replays\\a.rep",
    b"Replays\\.rep",
    b"Replays\\x.repx",
    b"Replays\\notes.txt",
    b"Replays\\sub\\Gamma.rep",
    b"Replays\\sub\\alpha.rep",
    b"Other\\delta.rep",
    b"Replays\\\xE9t\xE9.rep",
    b"Replays\\LONGNAME_123.Rep",
    b"Replays\\Last.rep",
    b"replays\\zeta.rep",
    b"Replays\\Gamma.rep",
];
const MAPS: [&str; 6] = ["Maps\\Alpine\\Alpine.map", "maps\\desert\\desert.map", "Maps\\Gone\\Gone.map", "", "MAPS\\ALPINE\\ALPINE.MAP", "Maps\\x.map"];
const CACHED: [(&str, &str); 3] = [("maps\\alpine\\alpine.map", "Alpine Assault"), ("maps\\desert\\desert.map", "Desert Fury"), ("maps\\x.map", "")];
const LAST_NAMES: [&str; 4] = ["00000000", "00000000", "mine", "Last"];
const LAST_TEXTS: [&str; 4] = ["Last Replay", "Last Replay", "alpha", ""];

/// A header of a case: (readable, options parse, map, version number / exe / INI CRC (None: unread), local player,
/// time (hour, minute, second, ms; None: unread), version string).
#[derive(Clone, Debug)]
struct Hdr {
    ok: bool,
    parsed: bool,
    map: String,
    nums: [Option<u32>; 3],
    local: i32,
    time: Option<[u16; 4]>,
    version: String,
}

#[derive(Clone, Debug)]
struct Case {
    has_cache: bool,
    this: [u32; 3],
    this_version: String,
    last: String,
    last_text: String,
    files: Vec<Vec<u8>>,
    /// By the lower-cased leaf name.
    headers: BTreeMap<Vec<u8>, Hdr>,
}

fn leaf(p: &[u8]) -> Vec<u8> {
    match p.iter().rposition(|&c| c == b'\\') {
        Some(i) => p[i + 1..].to_vec(),
        None => p.to_vec(),
    }
}

fn gen_case(r: &mut Rng) -> Case {
    let this = [r.below(3) as u32, r.below(3) as u32, r.below(3) as u32];
    let this_version = if r.below(2) == 0 { "1.04".to_string() } else { String::new() };
    let mut files = Vec::new();
    for _ in 0..r.below(13) {
        let f = FILES[r.below(FILES.len() as u64) as usize].to_vec();
        if !files.contains(&f) {
            files.push(f);
        }
    }
    let mut headers = BTreeMap::new();
    for f in FILES {
        let name = leaf(f).to_ascii_lowercase();
        if headers.contains_key(&name) || r.below(8) == 0 {
            continue;
        }
        let num = |r: &mut Rng, this: u32| match r.below(40) {
            0 => None,
            1..=6 => Some(this + 1),
            _ => Some(this),
        };
        let nums = [num(r, this[0]), num(r, this[1]), num(r, this[2])];
        let time = match r.below(40) {
            0 => None,
            1 => Some([24, 0, 0, 0]),
            2 => Some([3, 60, 0, 0]),
            _ => Some([r.below(24) as u16, r.below(60) as u16, r.below(60) as u16, r.below(1000) as u16]),
        };
        let version = if r.below(5) == 0 { "9.99".to_string() } else { this_version.clone() };
        headers.insert(
            name,
            Hdr { ok: r.below(10) != 0, parsed: r.below(10) != 0, map: MAPS[r.below(MAPS.len() as u64) as usize].to_string(), nums, local: r.below(3) as i32 - 1, time, version },
        );
    }
    Case {
        has_cache: r.below(40) != 0,
        this,
        this_version,
        last: LAST_NAMES[r.below(4) as usize].to_string(),
        last_text: LAST_TEXTS[r.below(4) as usize].to_string(),
        files,
        headers,
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// The text as the C++ dump writes it: ASCII, else `\uXXXX`.
fn dump(u: &[u16]) -> String {
    u.iter().map(|&c| if c < 0x80 { (c as u8 as char).to_string() } else { format!("\\u{c:04x}") }).collect()
}

struct Host {
    shell: Shell,
    main_menu: MainMenu,
    cache: Option<MapCache>,
    replays: MemoryReplayFolder,
    menu: ReplayMenu,
    version: ReplayVersion,
    last: Vec<u8>,
    last_text: Vec<u16>,
}

impl GuiHost for Host {}

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
        if label == b"GUI:LastReplay" {
            return self.last_text.clone();
        }
        [b"?".as_slice(), label].concat().iter().map(|&b| u16::from(b)).collect()
    }
}

impl MapUtilHost for Host {
    fn map_cache(&self) -> Option<&MapCache> {
        self.cache.as_ref()
    }
    fn update_map_cache(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        Vec::new()
    }
    fn find_mapped_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn map_list_item(&mut self, _name: &[u8]) -> u32 {
        1
    }
    fn map_list_name(&self, _data: u32) -> Option<Vec<u8>> {
        None
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
}

impl ReplayListHost for Host {
    fn replay_dir(&self) -> Vec<u8> {
        b"Replays\\".to_vec()
    }
    fn last_replay_file_name(&self) -> Vec<u8> {
        self.last.clone()
    }
    fn replay_file_list(&mut self, dir: &[u8], search: &[u8]) -> FilenameList {
        self.replays.list(dir, search)
    }
    fn read_replay_header(&mut self, filename: &[u8]) -> Option<ReplayListing> {
        self.replays.header(b"Replays\\", filename)
    }
    fn replay_version(&self) -> ReplayVersion {
        self.version.clone()
    }
    fn replay_file_exists(&mut self, path: &[u8]) -> bool {
        self.replays.find(path).is_some()
    }
    fn delete_replay_file(&mut self, _path: &[u8]) -> Result<(), Vec<u16>> {
        Ok(())
    }
    fn copy_replay_file(&mut self, _from: &[u8], _to: &[u8]) -> Result<(), Vec<u16>> {
        Ok(())
    }
    fn desktop_path(&self) -> Vec<u8> {
        Vec::new()
    }
    fn replay_menu(&mut self) -> &mut ReplayMenu {
        &mut self.menu
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

/// A synthetic list box `T.wnd:List` of four columns at (10, 10)-(610, 400), under a plain parent.
fn list_wnd() -> Vec<u8> {
    "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\nWINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  CHILD\n\
  WINDOW\n    WINDOWTYPE = SCROLLLISTBOX;\n    SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 610 400, CREATIONRESOLUTION: 800 600;\n    NAME = \"T.wnd:List\";\n    STATUS = ENABLED;\n    STYLE = SCROLLLISTBOX+MOUSETRACK;\n\
    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 4, COLUMNSWIDTH%: 40, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, FORCESELECT: 0;\n  END\n  ENDALLCHILDREN\nEND\n"
        .as_bytes()
        .to_vec()
}

fn listing(h: &Hdr) -> Option<ReplayListing> {
    if !h.ok {
        return None;
    }
    let time_val = h.time.map(|t| {
        let mut b = [0u8; 16];
        for (i, v) in t.iter().enumerate() {
            b[8 + 2 * i..10 + 2 * i].copy_from_slice(&v.to_le_bytes());
        }
        b
    });
    let header = ReplayHeader {
        replay_name: wide("(name)"),
        time_val,
        version_string: wide(&h.version),
        version_number: h.nums[0],
        exe_crc: h.nums[1],
        ini_crc: h.nums[2],
        local_player_index: h.local,
        ..Default::default()
    };
    Some(ReplayListing { header, parsed: h.parsed, map: h.map.as_bytes().to_vec() })
}

fn rust_run(c: &Case) -> String {
    let cache = c.has_cache.then(|| {
        let mut m = MapCache::default();
        for (k, d) in CACHED {
            m.maps.insert(k.as_bytes().to_vec(), MapMetaData { display_name: UnicodeString::try_from_chars(&wide(d)).expect("name"), ..Default::default() });
        }
        m
    });
    // (each file's listing: the header of its name, given to the folder's top file of that name, as the replay folder
    // reads the file it opens by name)
    let files = c
        .files
        .iter()
        .map(|p| {
            let top = p.to_ascii_lowercase().starts_with(b"replays\\") && !leaf(p).is_empty() && p.len() == b"Replays\\".len() + leaf(p).len();
            let l = if top { c.headers.get(&leaf(p).to_ascii_lowercase()).and_then(listing) } else { None };
            (p.clone(), Vec::new(), l)
        })
        .collect();
    let version = ReplayVersion { version_string: wide(&c.this_version), version_number: c.this[0], exe_crc: c.this[1], ini_crc: c.this[2], ..Default::default() };
    let mut h = Host {
        shell: Shell::default(),
        main_menu: MainMenu::default(),
        cache,
        replays: MemoryReplayFolder { files, desktop: Vec::new() },
        menu: ReplayMenu::default(),
        version,
        last: c.last.as_bytes().to_vec(),
        last_text: wide(&c.last_text),
    };
    let mut wm = GameWindowManager::new((800, 600));
    let mut wnd = Wnd(BTreeMap::new());
    let made = wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", &list_wnd(), &mut wnd, &mut h).expect("the synthetic file reads");
    let l = wm.tree.find_by_name(b"T.wnd:List").unwrap_or_else(|| panic!("the list box: {made:?}"));
    populate_replay_file_listbox(&mut wm, &mut h, Some(l));
    let ub = wm.undefined.is_some();
    let mut o = String::new();
    let n = lb::get_num_entries(&wm, l);
    for r in 0..n {
        o += &format!("row {r}:");
        for col in 0..4 {
            let (t, color) = lb::get_text_and_color(&mut wm, &mut h, l, r, col);
            o += &format!(" [{}]#{:08x}", dump(&t), color as u32);
        }
        o += "\n";
    }
    let sel = lb::get_selected(&mut wm, &mut h, l).first().copied().unwrap_or(-1);
    o += &format!("selected {sel} ub {}\n", u8::from(ub));
    for r in -1..n {
        let f = get_replay_filename_from_listbox(&mut wm, &mut h, Some(l), r);
        o += &format!("get {r}: {}\n", dump(&f));
    }
    o
}

fn cpp_run(c: &Case) -> String {
    let mut text: Vec<u8> = format!("cfg {} {} {} {}\nlast {}\ntext {}\nversion {}\n", u8::from(c.has_cache), c.this[0], c.this[1], c.this[2], c.last, c.last_text, c.this_version).into_bytes();
    for f in &c.files {
        text.extend_from_slice(b"file ");
        text.extend_from_slice(f);
        text.push(b'\n');
    }
    for (k, d) in CACHED {
        text.extend_from_slice(format!("map {k}|{d}\n").as_bytes());
    }
    for (name, h) in &c.headers {
        let num = |n: Option<u32>| n.map_or("-".to_string(), |v| v.to_string());
        let time = h.time.map_or("-".to_string(), |t| format!("{} {} {} {}", t[0], t[1], t[2], t[3]));
        text.extend_from_slice(b"hdr ");
        text.extend_from_slice(name);
        text.extend_from_slice(
            format!("|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", u8::from(h.ok), u8::from(h.parsed), h.map, num(h.nums[0]), num(h.nums[1]), num(h.nums[2]), h.local, time, h.version).as_bytes(),
        );
    }
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 14;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_replay_list_run(input.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

#[test]
fn the_replay_list_matches_upstream() {
    let cases: usize = std::env::var("REPLAY_LIST_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x5265_706c_6179_4c73);
    let (mut mismatches, mut rows, mut files, mut undefined, mut last_rows) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for case in 0..cases {
        let c = gen_case(&mut r);
        files += c.files.len();
        let a = rust_run(&c);
        let b = cpp_run(&c);
        // (after an undefined step the two sides' later state is not comparable: only that both met it is)
        let ub = |s: &str| s.lines().any(|l| l.starts_with("selected ") && l.ends_with(" ub 1"));
        if ub(&a) && ub(&b) {
            undefined += 1;
            continue;
        }
        if a != b {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {case}: {c:?}\n  rust:\n{a}  c++:\n{b}");
            }
        } else {
            rows += a.lines().filter(|l| l.starts_with("row ")).count();
            last_rows += a.lines().filter(|l| l.starts_with("row ") && l.contains(&format!("[{}]", c.last_text))).count();
        }
    }
    println!("#98 replay list: {cases} cases, {files} files, {rows} rows matched ({last_rows} reading as the last replay), {undefined} undefined (compared on that only), {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
