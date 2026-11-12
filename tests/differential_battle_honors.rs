#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/SkirmishBattleHonors.cpp (the whole file)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupPlayerInfo.cpp (BattleHonorTooltip,
//     ResetBattleHonorInsertion, InsertBattleHonor)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp
//     (populateSkirmishBattleHonors)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp (isSlotLocalAlly,
//     updateSkirmishBattleHonors, updateChallengeMedals, populatePlayerInfo's skirmish block)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The skirmish battle honours (`client::battle_honors`, `client::skirmish_game_options::populate_skirmish_battle_honors`,
//! `client::score_screen::skirmish_stats_update`) against the C++, #157.
//!
//! # Method
//!
//! The C++ side is the pieces build.rs cuts verbatim (see `differential/wrap_battle_honors.cpp`), on the REAL
//! UserPreferences and AsciiString, with `SkirmishStats.ini` a real file read and written by the C library in a
//! folder of this test's. The Rust side runs the translation on the same bytes (a folder in memory whose writes are
//! read back without the CR that MSVC's text mode adds: glibc's text mode adds none). Four kinds of case, in turn:
//!
//! - **stats**: a random `SkirmishStats.ini` (the class's keys and others, numbers, words, map keys) and 1 to 40
//!   random calls of every getter, setter, `clear` and `write`; each answer, each written file and the final map;
//! - **populate**: a random file, 0 to 6 maps (official or not, multiplayer or not, 1 to 8 players), a random subset
//!   of the honours' images, 1 to 6 list columns, the display (800x600, 1024x768, 390x844 or 1600x1200), the
//!   pre-order, and which of the five windows exist; the list's every cell (image, size, colour, item data), the four
//!   numbers' texts and the list's tooltip function. The Rust list is the real gadget made from a synthetic WND;
//! - **tooltip**: 8 random cells (row -1, 0 or 1; column -1 to 3; random bits with or without
//!   `BATTLE_HONOR_NOT_GAINED`; random streaks and wins): the text, delay and width set;
//! - **tail**: a random file and game (sandbox, victory, defeat, the player active, the local slot, eight slots'
//!   states and teams, the map, the frame, the superweapons and units built, the two sides), the score screen's
//!   skirmish block run; the file it leaves. `isSkirmish` and the local slot are the Rust side's (GameInfo.cpp's, tested
//!   elsewhere), given to the C++.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 9511 | the endurance key without its `_` | caught in stats (a set then get) and populate (medals) |
//! | 9512 | `setHonors` replaces | caught in stats and tail |
//! | 9513 | the challenge key's numbers swapped | caught in stats (asymmetric pairs) and populate |
//! | 9514 | the nuke stored as the cannon | caught in stats and tail |
//! | 9515 | no not-gained bit | caught in populate |
//! | 9516 | the row ends a column late | caught in populate |
//! | 9517 | the extra on the honour's own row | caught in populate |
//! | 9518 | the gold streak from 24 | caught in populate, rarely (a best streak of exactly 24) |
//! | 9519 | the silver endurance missing on the medium medal alone | caught in populate |
//! | 9520 | the ultimate with `<=` | caught in populate |
//! | 9521 | the battle tank before the GLA loyalty (not gained) | caught in tooltip |
//! | 9522 | the 25 streak tooltip from 24 | caught in tooltip, rarely |
//! | 9523 | the blitz 5 with `<=` | caught in tail (a game of 5 minutes) |
//! | 9524 | the easy medal without the hard opponents | caught in tail |
//! | 9525 | a slot without a team an ally | caught in tail |
//! | 9526 | ~~no return for an allied computer~~ (replaced, see the results) the medal of one brutal opponent more | caught in tail |
//! | 9527 | a change of general keeps the count | caught in tail |
//! | 9528 | the best streak before the streak | caught in tail |
//! | 9537 | the streak honour from 4 | caught in tail |
//! | 9538 | a loss keeps the streak | caught in tail |
//!
//! # Results (2026-11-12, `BATTLE_HONORS_CASES=20000`, the default)
//!
//! **Missed the prediction once, in the test's own wrapper:** the first run gave 2,047 stats mismatches, every one at
//! a `write`: the wrapper printed `outf("w %d %s", s.write(), readFile(path))`, and C++ evaluates function arguments in
//! no set order (GCC right to left): the file was read before it was written. The wrapper now writes first.
//!
//! **Then:** 20,000 cases (5,000 of each kind; 86,256 list cells; 4,601 tail cases that wrote the file), **0
//! mismatches**.
//!
//! Planted bugs: 20 of 20 caught, but not all as predicted. Mismatching cases (stats / populate / tooltip / tail):
//! 9511 11 / 2,884 / 0 / 25; 9512 549 / 0 / 0 / 1,670; 9513 12 / 657 / 0 / 0; 9514 1,831 / 0 / 0 / 1,303; 9515 populate
//! 3,812; 9516 populate 4,180; 9517 populate 4,603; 9518 populate 26; 9519 populate 269; 9520 populate 1,024; 9521
//! tooltip 1,217; 9522 tooltip 2; 9523 tail 334; 9524 tail 1,191; 9525 tail 1,265; 9526 tail 950; 9527 tail 3,663; 9528
//! tail 1,822; 9537 tail 22; 9538 tail 1,468.
//!
//! **Misses:** (1) the first 9526, *no return for an allied computer* in `updateChallengeMedals`, was **not caught**,
//! and cannot be: `isSkirmish()` (GameInfo.cpp) is already false when a computer is the local player's ally, and with
//! no local slot no opponent is counted, so upstream's `return` never changes the medals. Replaced by an observable
//! plant (the medal of one hard opponent more). (2) 9516 was caught first as a test panic (the Rust gadget recorded an
//! undefined cell write the stand-in list lacks); the test now counts an undefined step as a mismatch. Every other plant was caught where predicted (9518 and 9522
//! rarely, as predicted).

use rts_lockstep_engine::client::battle_honors::{battle_honor_tooltip_label, SkirmishBattleHonors, BATTLE_HONOR_NOT_GAINED};
use rts_lockstep_engine::client::gadget_list_box::{self as lb, CellData, LISTBOX_IMAGE};
use rts_lockstep_engine::client::score_screen::{skirmish_stats_skipped, skirmish_stats_update, SkirmishHonorsInputs, APOCALYPSE_TEMPLATES};
use rts_lockstep_engine::client::shell::{ShellHost, WndFiles};
use rts_lockstep_engine::client::shell_gui::ShellGui;
use rts_lockstep_engine::client::skirmish_game_options::populate_skirmish_battle_honors;
use rts_lockstep_engine::client::user_preferences::{MemoryUserData, UserDataFiles};
use rts_lockstep_engine::client::window_manager::GameWindowManager;
use rts_lockstep_engine::client::window_script::WndHost;
use rts_lockstep_engine::common::game_info::{slot_state, GameInfo, TheGameInfo};
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
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }
}

const MAPS: [&str; 8] = ["maps\\alpha\\alpha.map", "maps\\beta\\beta.map", "maps\\gamma\\g.map", "maps\\delta\\delta.map", "maps\\eps\\eps.map", "user\\maps\\mine\\mine.map", "Maps\\Upper\\Upper.map", "maps\\z\\z.map"];
const SIDES: [&str; 5] = ["America", "China", "GLA", "Observer", "america"];
const IMAGES: [&str; 26] = [
    "ChinaCampaign_G", "ChinaCampaign_S", "ChinaCampaign_B", "GLACampaign_G", "GLACampaign_S", "GLACampaign_B", "USACampaign_G", "USACampaign_S", "USACampaign_B", "Challenge_Gold", "Challenge_Silver",
    "Challenge_Bronz", "HonorAirWing", "HonorBattleTank", "Endurance_G", "Endurance_S", "Endurance_B", "Apocalypse", "HonorBlitz5", "HonorBlitz10", "HonorStreak_1000", "HonorStreak_500", "HonorStreak_100",
    "HonorStreak_G", "Ultimate", "OfficersClub",
];
const WINDOWS: [&str; 5] = ["ListboxInfo", "StaticTextStreakValue", "StaticTextBestStreakValue", "StaticTextWinsValue", "StaticTextLossesValue"];

fn number(r: &mut Rng) -> i64 {
    match r.below(8) {
        0 => 0,
        1 => 1 + r.below(3) as i64,
        2 => 3 + r.below(30) as i64,
        3 => [24, 25, 99, 100, 499, 500, 999, 1000, 9999, 10000][r.below(10) as usize],
        4 => -(r.below(50) as i64),
        5 => r.below(1 << 20) as i64,
        6 => r.below(64) as i64,
        _ => r.below(5000) as i64,
    }
}

/// A `SkirmishStats.ini`: lines of the class's keys (and others) with numbers, words or flags; no CR, no Ctrl-Z, no
/// line without a key and a value (upstream's undefined read), every number in `Int`'s range.
fn stats_file(r: &mut Rng) -> Vec<u8> {
    let mut s = String::new();
    for _ in 0..r.below(14) {
        let key = match r.below(12) {
            0 => r.pick(&["Wins", "Losses", "WinStreak", "BestWinStreak", "Challenge", "LoyalGames", "Honors"]).to_string(),
            1 => r.pick(&["SCUD", "PPC", "Nuke"]).to_string(),
            2 => format!("{}_{}", r.pick(&MAPS), 2 + r.below(3)),
            3 => format!("{}Campaign_{}", r.pick(&["USA", "CHINA", "GLA"]), r.below(3)),
            4 => format!("ChallengeCampaign{}_{}", r.below(10), r.below(3)),
            5 => "LastHouse".to_string(),
            6 => r.pick(&["Junk", "wins", "Honors2"]).to_string(),
            _ => r.pick(&["Wins", "Honors", "BestWinStreak", "WinStreak", "Losses"]).to_string(),
        };
        let val = match r.below(6) {
            0 => r.pick(&["yes", "TRUE", "0", "1", "no", "ok"]).to_string(),
            1 => r.pick(&SIDES).to_string(),
            2 => format!("{}x", number(r)),
            _ => number(r).to_string(),
        };
        let sp = |r: &mut Rng| r.pick(&["", " ", "  ", "\t"]);
        s += &format!("{}{key}{}={}{val}{}\n", sp(r), sp(r), sp(r), sp(r));
    }
    s.into_bytes()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The Rust side's user data folder: in memory, its writes kept as glibc's text mode writes them (no CR).
struct Folder(MemoryUserData);

impl UserDataFiles for Folder {
    fn path_user_data(&self) -> Vec<u8> {
        self.0.path.clone()
    }
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.0.files.get(path).cloned()
    }
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        let mut out = Vec::with_capacity(bytes.len());
        for (i, &c) in bytes.iter().enumerate() {
            if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                continue;
            }
            out.push(c);
        }
        self.0.files.insert(path.to_vec(), out);
        true
    }
}

fn folder(file: &Option<Vec<u8>>) -> Folder {
    let mut f = MemoryUserData { path: b"U/".to_vec(), ..Default::default() };
    if let Some(b) = file {
        f.files.insert(b"U/SkirmishStats.ini".to_vec(), b.clone());
    }
    Folder(f)
}

fn dump_map(o: &mut String, s: &SkirmishBattleHonors) {
    for (k, v) in &s.prefs.map {
        *o += &format!("key {}={}\n", String::from_utf8_lossy(k), String::from_utf8_lossy(v));
    }
}

fn cpp_run(text: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rts_battle_honors_{}/", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    let d = std::ffi::CString::new(dir.to_str().expect("utf-8").to_string()).expect("no NUL");
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 15;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_battle_honors_run(input.as_ptr(), d.as_ptr(), out.as_mut_ptr().cast(), cap as i32) };
        assert!(len >= 0, "the C++ side refused the case:\n{text}");
        let len = len as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

// ---------------------------------------------------------------- stats

fn stats_case(r: &mut Rng) -> (String, String) {
    let file = (r.below(5) != 0).then(|| stats_file(r));
    let mut text = format!("file {}\nmode stats\n", file.as_ref().map_or("-".to_string(), |f| hex(f)));
    let mut fs = folder(&file);
    let mut s = SkirmishBattleHonors::new(&mut fs);
    let mut o = String::new();
    for _ in 0..1 + r.below(40) {
        let k = r.below(34) as i32;
        let a = match r.below(3) {
            0 => r.below(5) as i32,
            1 => number(r) as i32,
            _ => r.below(12) as i32,
        };
        let b = if r.below(2) == 0 { r.below(5) as i32 } else { number(r) as i32 };
        let st = if k == 21 { r.pick(&SIDES) } else { r.pick(&MAPS) };
        text += &format!("op {k} {a} {b} {st}\n");
        let st = st.as_bytes();
        match k {
            0 => o += &format!("r {}\n", s.get_wins()),
            1 => s.set_wins(a),
            2 => o += &format!("r {}\n", s.get_losses()),
            3 => s.set_losses(a),
            4 => o += &format!("r {}\n", s.get_win_streak()),
            5 => s.set_win_streak(a),
            6 => o += &format!("r {}\n", s.get_best_win_streak()),
            7 => s.set_best_win_streak(a),
            8 => o += &format!("r {}\n", s.get_challenge_medals()),
            9 => s.set_challenge_medals(a),
            10 => o += &format!("r {}\n", u8::from(s.built_scud())),
            11 => s.set_built_scud(),
            12 => o += &format!("r {}\n", u8::from(s.built_particle_cannon())),
            13 => s.set_built_particle_cannon(),
            14 => o += &format!("r {}\n", u8::from(s.built_nuke())),
            15 => s.set_built_nuke(),
            16 => o += &format!("r {}\n", s.get_honors()),
            17 => s.set_honors(a),
            18 => o += &format!("r {}\n", s.get_endurance_medal(st, a)),
            19 => s.set_endurance_medal(st, a, b),
            20 => o += &format!("r {}\n", String::from_utf8_lossy(&s.get_last_general())),
            21 => s.set_last_general(st),
            22 => o += &format!("r {}\n", s.get_num_games_loyal()),
            23 => s.set_num_games_loyal(a),
            24 => o += &format!("r {}\n", u8::from(s.get_usa_campaign_complete(a))),
            25 => s.set_usa_campaign_complete(a),
            26 => o += &format!("r {}\n", u8::from(s.get_china_campaign_complete(a))),
            27 => s.set_china_campaign_complete(a),
            28 => o += &format!("r {}\n", u8::from(s.get_gla_campaign_complete(a))),
            29 => s.set_gla_campaign_complete(a),
            30 => o += &format!("r {}\n", u8::from(s.get_challenge_campaign_complete(a, b))),
            31 => s.set_challenge_campaign_complete(a, b),
            32 => s.clear(),
            _ => {
                let ok = s.write(&mut fs);
                o += &format!("w {} {}\n", u8::from(ok), hex(fs.0.files.get(&b"U/SkirmishStats.ini"[..]).map_or(&[][..], |v| v.as_slice())));
            }
        }
    }
    dump_map(&mut o, &s);
    (text, o)
}

// ---------------------------------------------------------------- populate

/// The reader's files for the populate case: one synthetic WND (any path) and the case's images.
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

/// The synthetic WND: a parent holding the case's windows (`ListboxInfo` with `columns` columns).
fn honors_wnd(columns: i32, present: u32) -> Vec<u8> {
    let f = "SkirmishGameOptionsMenu.wnd";
    let mut c = String::new();
    for (i, n) in WINDOWS.iter().enumerate() {
        if present & (1 << i) == 0 {
            continue;
        }
        if i == 0 {
            let widths: String = (0..columns).map(|_| format!(", COLUMNSWIDTH%: {}", 100 / columns)).collect();
            c += &format!("  WINDOW\n    WINDOWTYPE = SCROLLLISTBOX;\n    SCREENRECT = UPPERLEFT: 400 300, BOTTOMRIGHT: 780 560, CREATIONRESOLUTION: 800 600;\n    NAME = \"{f}:{n}\";\n    STATUS = ENABLED;\n    STYLE = SCROLLLISTBOX+MOUSETRACK;\n    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: {columns}{widths}, FORCESELECT: 0;\n  END\n");
        } else {
            let y = 300 + 25 * i;
            c += &format!("  WINDOW\n    WINDOWTYPE = STATICTEXT;\n    SCREENRECT = UPPERLEFT: 20 {y}, BOTTOMRIGHT: 200 {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"{f}:{n}\";\n    STATUS = ENABLED;\n    STYLE = STATICTEXT;\n    STATICTEXTDATA = CENTERED: 0;\n  END\n", y + 20);
        }
    }
    format!("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\nWINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"{f}:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  CHILD\n{c}  ENDALLCHILDREN\nEND\n").into_bytes()
}

fn populate_case(r: &mut Rng) -> (String, String) {
    // the file: random, with the endurance medals of the case's maps often set
    let mut file = stats_file(r);
    let maps: Vec<(&str, bool, bool, i32)> = {
        let mut v: Vec<(&str, bool, bool, i32)> = Vec::new();
        for _ in 0..r.below(7) {
            let m = r.pick(&MAPS);
            if v.iter().any(|x| x.0 == m) {
                continue;
            }
            v.push((m, r.below(4) != 0, r.below(4) != 0, 1 + r.below(8) as i32));
        }
        v
    };
    for (m, _, _, players) in &maps {
        for st in 2..5 {
            if r.below(3) != 0 {
                let n = if r.below(2) == 0 { players - 1 } else { r.below(8) as i32 };
                file.extend_from_slice(format!("{m}_{st} = {n}\n").as_bytes());
            }
        }
    }
    let columns = 1 + r.below(6) as i32;
    let display = [(800, 600), (1024, 768), (390, 844), (1600, 1200)][r.below(4) as usize];
    let preorder = r.below(2) as u32;
    let present = if r.below(6) == 0 { r.below(32) as u32 } else { 0x1f };
    let images: BTreeSet<Vec<u8>> = IMAGES.iter().filter(|_| r.below(5) != 0).map(|i| i.as_bytes().to_vec()).collect();
    let mut text = format!("file {}\nmode populate\ncfg {columns} {} {} {preorder}\n", hex(&file), display.0, display.1);
    for (i, w) in WINDOWS.iter().enumerate() {
        if present & (1 << i) != 0 {
            text += &format!("win SkirmishGameOptionsMenu.wnd:{w}\n");
        }
    }
    for i in &images {
        text += &format!("img {}\n", String::from_utf8_lossy(i));
    }
    for (m, off, mp, p) in &maps {
        text += &format!("map {m} {} {} {p}\n", u8::from(*off), u8::from(*mp));
    }
    // the Rust side
    let files = Files { wnd: honors_wnd(columns, present), images, keys: BTreeMap::new() };
    let mut h = ShellGui::new(files);
    h.preorder = preorder != 0;
    h.user_data = MemoryUserData { path: Vec::new(), files: [(b"SkirmishStats.ini".to_vec(), file)].into_iter().collect() };
    for (m, off, mp, p) in &maps {
        h.skirmish_data.map_cache.maps.insert(m.as_bytes().to_vec(), MapMetaData { is_official: *off, is_multiplayer: *mp, num_players: *p, ..Default::default() });
    }
    let mut wm = GameWindowManager::new(display);
    ShellHost::create_from_script(&mut h, &mut wm, b"Menus/HonorsTest.wnd").expect("the synthetic file reads");
    populate_skirmish_battle_honors(&mut wm, &mut h);
    let mut o = String::new();
    for (i, w) in WINDOWS.iter().enumerate().skip(1) {
        if present & 1 == 0 || present & (1 << i) == 0 {
            continue;
        }
        let win = wm.tree.find_by_name(format!("SkirmishGameOptionsMenu.wnd:{w}").as_bytes()).expect("window");
        o += &format!("text SkirmishGameOptionsMenu.wnd:{w} {}\n", String::from_utf16_lossy(&rts_lockstep_engine::client::gadget::static_text_get_text(&wm, win)));
    }
    if present & 1 != 0 {
        let l = wm.tree.find_by_name(b"SkirmishGameOptionsMenu.wnd:ListboxInfo").expect("list");
        o += &format!("tooltipfunc {}\n", u8::from(wm.ext[l].tooltip.as_deref() == Some(&b"BattleHonorTooltip"[..])));
        let rt = lb::listbox_rt(&wm, l).expect("the list's data").clone();
        for row in 0..i32::from(rt.end_pos) {
            o += &format!("row {row}:");
            let rr = rt.list_data.as_ref().expect("rows")[row as usize];
            for col in 0..columns {
                let cell = rr.cell.and_then(|a| rt.arrays[a].as_ref()).map(|cells| cells[col as usize].clone());
                let data = lb::get_item_data(&mut wm, &mut h, l, row, col) as i32;
                match cell {
                    Some(cell) if cell.cell_type == LISTBOX_IMAGE => {
                        let name = match &cell.data {
                            CellData::Image(i) => String::from_utf8_lossy(i).into_owned(),
                            _ => "<none>".to_string(),
                        };
                        o += &format!(" {name} {}x{} #{:08x}/{data}", cell.width, cell.height, cell.color as u32);
                    }
                    _ => o += &format!(" -/{data}"),
                }
            }
            o += "\n";
        }
    }
    // (an undefined step on the Rust side, which the C++ stand-in's list does not have: a mismatch)
    if let Some(u) = &wm.undefined {
        o += &format!("undefined {u}\n");
    }
    (text, o)
}

// ---------------------------------------------------------------- tooltip

fn honor_bits(r: &mut Rng) -> i32 {
    let mut b = 0;
    for _ in 0..1 + r.below(3) {
        b |= 1 << r.below(26);
    }
    if r.below(2) == 0 {
        b |= BATTLE_HONOR_NOT_GAINED;
    }
    if r.below(10) == 0 {
        b = 0;
    }
    b
}

fn tooltip_case(r: &mut Rng) -> (String, String) {
    let mut text = "file -\nmode tooltip\n".to_string();
    let mut o = String::new();
    for _ in 0..8 {
        let row = r.below(3) as i32 - 1;
        let col = r.below(5) as i32 - 1;
        let data = honor_bits(r);
        let extra = number(r) as i32;
        text += &format!("tip {row} {col} {data} {extra}\n");
        // the C++'s table: row 1 the honour, row 0 the extra (in column `col & 3`)
        let at = |rr: i32, cc: i32| -> i32 {
            if !(0..4).contains(&cc) || cc != (col & 3) {
                return 0;
            }
            match rr {
                1 => data,
                0 => extra,
                _ => 0,
            }
        };
        let pick = if row == -1 || col == -1 { Some((&b"TOOLTIP:BattleHonors"[..], false)) } else { battle_honor_tooltip_label(at(row, col), at(row - 1, col)) };
        if let Some((label, wide)) = pick {
            o += &format!("tooltip {} -1 {}\n", String::from_utf8_lossy(label), if wide { "1.5" } else { "1" });
        }
    }
    (text, o)
}

// ---------------------------------------------------------------- tail

fn tail_case(r: &mut Rng) -> (String, String) {
    let file = (r.below(4) != 0).then(|| stats_file(r));
    let mut gi = GameInfo::new(&mut rts_lockstep_engine::client::gui_util::SkirmishData::default());
    gi.in_game = true;
    gi.local_ip = 7;
    let local = if r.below(12) == 0 { -1 } else { r.below(8) as i32 };
    let mut slots_text = String::new();
    for i in 0..8 {
        let s = &mut gi.slots[i];
        s.state = [slot_state::OPEN, slot_state::CLOSED, slot_state::EASY_AI, slot_state::MED_AI, slot_state::BRUTAL_AI, slot_state::BRUTAL_AI][r.below(6) as usize];
        s.team_number = r.below(5) as i32 - 1;
        s.ip = 0;
        if i as i32 == local {
            s.state = slot_state::PLAYER;
            s.ip = 7;
        }
        slots_text += &format!("slot {i} {} {}\n", s.state, s.team_number);
    }
    gi.map_name = r.pick(&MAPS).as_bytes().to_vec();
    assert_eq!(gi.local_slot_num(), local);
    let is_skirmish = gi.is_skirmish(|i| TheGameInfo::Is(&gi, Some(i))).expect("isSkirmish");
    let sandbox = r.below(5) == 0;
    let (victory, defeat) = match r.below(4) {
        0 => (false, false),
        1 => (false, true),
        _ => (true, false),
    };
    let active = r.below(3) != 0;
    let template_side = r.pick(&SIDES);
    let local_side = if r.below(4) == 0 { r.pick(&SIDES) } else { template_side };
    let frame = match r.below(4) {
        0 => [8999, 9000, 9001, 17999, 18000][r.below(5) as usize],
        _ => r.below(30 * 60 * 15) as u32,
    };
    let objects: Vec<i32> = APOCALYPSE_TEMPLATES.iter().map(|_| if r.below(6) == 0 { 1 + r.below(2) as i32 } else { 0 }).collect();
    let inp = SkirmishHonorsInputs {
        objects_built: objects.iter().map(|&n| Some(n)).collect(),
        vehicles_built: [0, 49, 50, 51, r.below(80) as i32][r.below(5) as usize],
        aircraft_built: [0, 19, 20, r.below(40) as i32][r.below(4) as usize],
        frame,
        local_side: local_side.as_bytes().to_vec(),
    };
    let mut text = format!("file {}\nmode tail\n{slots_text}", file.as_ref().map_or("-".to_string(), |f| hex(f)));
    text += &format!(
        "game {} {} {} {} {local} {} {frame} {} {} {} {template_side} {local_side}\n",
        u8::from(sandbox),
        u8::from(victory),
        u8::from(defeat),
        u8::from(active),
        u8::from(is_skirmish),
        inp.vehicles_built,
        inp.aircraft_built,
        String::from_utf8_lossy(&gi.map_name)
    );
    for (n, c) in APOCALYPSE_TEMPLATES.iter().zip(&objects) {
        text += &format!("built {n} {c}\n");
    }
    let mut fs = folder(&file);
    if !skirmish_stats_skipped(sandbox, victory, defeat, active) {
        skirmish_stats_update(&mut fs, victory, template_side.as_bytes(), &inp, &gi, is_skirmish).expect("defined");
    }
    let o = format!("file {}\n", hex(fs.0.files.get(&b"U/SkirmishStats.ini"[..]).map_or(&[][..], |v| v.as_slice())));
    (text, o)
}

#[test]
fn the_battle_honours_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("BATTLE_HONORS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x4261_7474_6c65_4831);
    let mut mismatches = [0usize; 4];
    let mut counted = [0usize; 4];
    let names = ["stats", "populate", "tooltip", "tail"];
    let mut cells = 0usize;
    let mut writes = 0usize;
    for case in 0..cases {
        let kind = case % 4;
        let (text, rust) = match kind {
            0 => stats_case(&mut r),
            1 => populate_case(&mut r),
            2 => tooltip_case(&mut r),
            _ => tail_case(&mut r),
        };
        let c = cpp_run(&text);
        counted[kind] += 1;
        if kind == 1 {
            cells += rust.matches('/').count();
        }
        if kind == 3 && rust != "file \n" {
            writes += 1;
        }
        if c != rust {
            mismatches[kind] += 1;
            if mismatches.iter().sum::<usize>() <= 3 {
                println!("case {case} ({}):\n{text}\n  rust:\n{rust}\n  c++:\n{c}", names[kind]);
            }
        }
    }
    println!(
        "#157 battle honours: {cases} cases ({} stats, {} populate with {cells} cells, {} tooltip, {} tail with {writes} files written); mismatches: stats {}, populate {}, tooltip {}, tail {}",
        counted[0], counted[1], counted[2], counted[3], mismatches[0], mismatches[1], mismatches[2], mismatches[3]
    );
    assert_eq!(mismatches.iter().sum::<usize>(), 0);
}
