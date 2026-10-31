#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/GameState.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameState` (`common::game_state`) against the C++, layer 12 batch S1.
//!
//! # Method
//!
//! The C++ side is `GameState.h` and the pieces of `GameState.cpp` build.rs cuts verbatim (the constants,
//! `SaveDate::isNewerThan`, `findBlockInfoByToken`, `reset`, `addSnapshotBlock`, `findNextSaveFilename`, the save
//! directory and map path functions, `xferSaveData` and `friend_xferSaveDataForCRC`, the post-process list, `xfer`),
//! compiled by `differential/wrap_game_state.cpp` with the Xfer build's real `XferSave` and `XferLoad` (on a file) and
//! stand-ins for the global data, the world dict, the campaign manager, the partition manager, `GetLocalTime` and
//! `_access`. Its blocks are test snapshots of an `Int` and a string that log their xfer and post-process. A program
//! registers blocks under names (the two the mission save keeps, others, names that differ only in case, `SG_EOF` in
//! either case, empty), sets them, saves them (or for a CRC), loads the bytes (as saved, with a byte replaced, or cut;
//! with or without post-processing), post-processes, saves and loads the save-information block, finds the next save
//! file's name among existing files, converts map paths both ways, and compares dates. The Rust side runs the same on
//! [`GameState`] with the crate's in-memory `XferSave` and `XferLoad`.
//!
//! An operation the Rust side reports as undefined (`tolower` of a byte of 0x80 or more in a map path) ends the Rust run
//! there; the C++ runs only the operations before it.
//!
//! # Results (2026-10-31, `GSTATE_CASES=20000`, the default)
//!
//! 20,000 programs, 345,860 operations, 605,843 log lines (saves byte for byte; loads of damaged and cut files):
//! **0 mismatches**. 1,485 programs cut at an undefined operation (`tolower` of a byte of 0x80 or more in a map path).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 4 of 4 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3981 | `isNewerThan` true for equal dates | 4,913 programs |
//! | 3982 | an empty block name ends a load like `SG_EOF` | 4,225 programs |
//! | 3983 | a loaded block not added to the post-process list | 5,825 programs |
//! | 3984 | a path in the map directory not made portable | 1,410 programs |

use rts_lockstep_engine::common::game_state::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::xfer::{Xfer, XferResult, XO_NO_POST_PROCESSING};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
}

type Op = [u32; 6];

struct Host {
    cfg: [u32; 8],
    strs: Vec<Vec<u8>>,
    blocks: [(i32, Vec<u8>); 6],
    clock: u16,
    files: BTreeSet<Vec<u8>>,
    log: String,
}

impl Host {
    fn str(&self, i: u32) -> Vec<u8> {
        if self.strs.is_empty() {
            Vec::new()
        } else {
            self.strs[i as usize % self.strs.len()].iter().copied().take_while(|&b| b != 0).collect()
        }
    }
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl GameStateHost<u32> for Host {
    fn xfer_block(&mut self, block: u32, xfer: &mut dyn Xfer) -> XferResult<()> {
        let b = &mut self.blocks[block as usize];
        xfer.xfer_int(&mut b.0)?;
        xfer.xfer_ascii_string(&mut b.1)?;
        let mode = xfer.get_xfer_mode() as i32;
        let (v, t) = (b.0, lossy(&b.1));
        let _ = writeln!(self.log, "block {block} xfer mode {mode} value {v} text [{t}]");
        Ok(())
    }
    fn load_post_process(&mut self, block: u32) {
        let _ = writeln!(self.log, "post {block}");
    }
    fn partition_update(&mut self) {
        self.log.push_str("partition update\n");
    }
    fn user_data_path(&self) -> Vec<u8> {
        if self.cfg[0] & 2 != 0 {
            b"C:\\Users\\Me\\Generals\\".to_vec()
        } else {
            b"data\\".to_vec()
        }
    }
    fn map_name(&self) -> Vec<u8> {
        self.str(self.cfg[1])
    }
    fn local_time(&mut self) -> [u16; 8] {
        let mut t = [0u16; 8];
        for (i, v) in t.iter_mut().enumerate() {
            *v = self.clock.wrapping_add(i as u16);
        }
        self.clock = self.clock.wrapping_add(8);
        t
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        (self.cfg[0] & 1 != 0 && self.cfg[0] & 8 != 0).then(|| self.str(self.cfg[2]))
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        (self.cfg[0] & 4 != 0).then(|| (self.str(self.cfg[3]), self.cfg[4] as i32))
    }
    fn file_exists(&self, path: &[u8]) -> bool {
        self.files.contains(path)
    }
}

fn dump_info(log: &mut String, i: &SaveGameInfo) {
    let d: String = i.description.iter().map(|c| format!(" {c:04x}")).collect();
    let _ = writeln!(
        log,
        "info label [{}] date {} {} {} {} {} {} {} {} side [{}] mission {} type {} missionmap [{}] pristine [{}] desc{d}",
        lossy(&i.map_label),
        i.date.year,
        i.date.month,
        i.date.day,
        i.date.day_of_week,
        i.date.hour,
        i.date.minute,
        i.date.second,
        i.date.milliseconds,
        lossy(&i.campaign_side),
        i.mission_number,
        i.save_file_type,
        lossy(&i.mission_map_name),
        lossy(&i.pristine_map_name)
    );
}

fn which(v: u32) -> SnapshotType {
    match v % 3 {
        0 => SnapshotType::SaveLoad,
        1 => SnapshotType::DeepCrcLogicOnly,
        _ => SnapshotType::DeepCrc,
    }
}

struct Cut(String);

fn op(gs: &mut GameState<u32>, h: &mut Host, saved: &mut Vec<u8>, w: &Op) -> Result<(), Cut> {
    let (a, b, c, d, e) = (w[1], w[2], w[3], w[4], w[5]);
    match w[0] {
        0 => {
            let name = h.str(a);
            gs.add_snapshot_block(&name, b % 6, which(c));
            let _ = writeln!(h.log, "add [{}] {} {}", lossy(&name), b % 6, c % 3);
        }
        1 => {
            h.blocks[(a % 6) as usize] = (b as i32, h.str(c));
            let _ = writeln!(h.log, "set {}", a % 6);
        }
        2 => {
            let mut x = XferSave::new();
            x.open(b"gstate").expect("open");
            let r = if b & 1 != 0 { gs.xfer_save_data_for_crc(&mut x, which(a), h) } else { gs.xfer_save_data(&mut x, which(a), h) };
            h.log.push_str(if r.is_ok() { "save\n" } else { "save threw\n" });
            *saved = x.bytes().to_vec();
            let hex: String = saved.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(h.log, "bytes {hex}");
        }
        3 => {
            let mut bytes = saved.clone();
            if b & 1 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes[c as usize % n] = d as u8;
            }
            if b & 2 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes.truncate(n - 1 - (e as usize % n));
            }
            let mut x = XferLoad::new(bytes);
            x.open(b"gstate").expect("open");
            if b & 4 != 0 {
                x.set_options(XO_NO_POST_PROCESSING);
            }
            let r = gs.xfer_save_data(&mut x, which(a), h);
            h.log.push_str(if r.is_ok() { "load\n" } else { "load threw\n" });
            let pl: String = gs.post_process.iter().map(|b| format!(" {b}")).collect();
            let _ = writeln!(h.log, "postlist{pl}");
        }
        4 => {
            gs.game_state_post_process_load(h);
            h.log.push_str("postprocessed\n");
        }
        5 => {
            gs.reset();
            h.log.push_str("reset\n");
        }
        6 => {
            let mut x = XferSave::new();
            x.open(b"gstate").expect("open");
            let r = gs.xfer_info(&mut x, h);
            h.log.push_str(if r.is_ok() { "infosave\n" } else { "infosave threw\n" });
            *saved = x.bytes().to_vec();
            let hex: String = saved.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(h.log, "bytes {hex}");
        }
        7 => {
            let mut bytes = saved.clone();
            if b & 1 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes[c as usize % n] = d as u8;
            }
            let mut x = XferLoad::new(bytes);
            x.open(b"gstate").expect("open");
            let r = gs.xfer_info(&mut x, h);
            h.log.push_str(if r.is_ok() { "infoload\n" } else { "infoload threw\n" });
            let i = gs.info.clone();
            dump_info(&mut h.log, &i);
        }
        8 => {
            gs.info.save_file_type = (a % 3) as i32;
            gs.info.mission_map_name = h.str(b);
            gs.info.pristine_map_name = h.str(c);
            let mut desc = vec![u16::from(b'a') + (d % 26) as u16];
            if e & 0xffff != 0 {
                desc.push((e & 0xffff) as u16);
            }
            gs.info.description = desc;
            h.log.push_str("setinfo\n");
        }
        9 => {
            let i = gs.info.clone();
            dump_info(&mut h.log, &i);
        }
        10 => {
            let n = gs.find_next_save_filename(h);
            let _ = writeln!(h.log, "next [{}]", lossy(&n));
        }
        11 => {
            let s = h.str(a);
            h.files.insert(s.clone());
            let _ = writeln!(h.log, "file [{}]", lossy(&s));
        }
        12 => {
            let s = h.str(a);
            let r = gs.real_map_path_to_portable(&s, h).map_err(|e: IniError| Cut(format!("{e:?}")))?;
            let _ = writeln!(h.log, "portable [{}] -> [{}]", lossy(&s), lossy(&r));
        }
        13 => {
            let s = h.str(a);
            let r = gs.portable_map_path_to_real(&s, h).map_err(|e: IniError| Cut(format!("{e:?}")))?;
            let _ = writeln!(h.log, "real [{}] -> [{}]", lossy(&s), lossy(&r));
        }
        14 => {
            let s = h.str(a);
            let leaf = map_leaf_name(&s);
            let insave = gs.is_in_save_directory(&s, h);
            let _ = writeln!(h.log, "leaf [{}] -> [{}] insave {}", lossy(&s), lossy(&leaf), i32::from(insave));
        }
        15 => {
            let mk = |v: u32| {
                let f = |k: u32| ((v >> (4 * k)) & 3) as u16;
                SaveDate { year: f(0), month: f(1), day: f(2), day_of_week: f(3), hour: f(4), minute: f(5), second: f(6), milliseconds: f(7) }
            };
            let r = mk(a).is_newer_than(&mk(b));
            let _ = writeln!(h.log, "newer {}", i32::from(r));
        }
        _ => h.log.push_str("op?\n"),
    }
    Ok(())
}

fn rust_run(cfg: &[u32; 8], ops: &[Op], strs: &[Vec<u8>]) -> (String, Option<(usize, String)>) {
    let mut h = Host { cfg: *cfg, strs: strs.to_vec(), blocks: Default::default(), clock: cfg[5] as u16, files: BTreeSet::new(), log: String::new() };
    let mut gs = GameState::<u32>::new();
    let mut saved = Vec::new();
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(Cut(why)) = op(&mut gs, &mut h, &mut saved, w) {
            h.log.truncate(before);
            return (h.log, Some((i, why)));
        }
    }
    (h.log, None)
}

fn cpp_run(cfg: &[u32; 8], ops: &[Op], strs: &[Vec<u8>]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut packed = Vec::new();
    for s in strs {
        packed.extend_from_slice(&(s.len() as u16).to_le_bytes());
        packed.extend_from_slice(s);
    }
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_gstate_run(cfg.as_ptr(), flat.as_ptr(), ops.len() as i32, packed.as_ptr().cast(), strs.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

const STRS: [&[u8]; 30] = [
    b"CHUNK_GameState",
    b"CHUNK_Campaign",
    b"chunk_gamestate",
    b"CHUNK_A",
    b"CHUNK_B",
    b"CHUNK_C",
    b"SG_EOF",
    b"sg_eof",
    b"",
    b"hello",
    b"text two",
    b"Maps\\alpha\\alpha.map",
    b"maps\\Beta\\beta.map",
    b"data\\Save\\x.map",
    b"C:\\Users\\Me\\Generals\\Save\\foo.map",
    b"UserData\\Maps\\b\\b.map",
    b"C:\\Users\\Me\\Generals\\Maps\\c\\c.map",
    b"data\\Maps\\d\\d.map",
    b"save\\leaf.map",
    b"Maps\\x",
    b"noslash",
    b"a\\b\\",
    b"\\",
    b"MAPS\\Q\\R\\S.map",
    b"data\\Save\\00000000.sav",
    b"data\\Save\\00000001.sav",
    b"C:\\Users\\Me\\Generals\\Save\\00000000.sav",
    b"Maps\\\xe9t\xe9\\x.map",
    b"Maps\\Somewhere\\Somewhere.map",
    b"zz",
];

fn gen_program(g: &mut Gen) -> ([u32; 8], Vec<Op>, Vec<Vec<u8>>) {
    let strs: Vec<Vec<u8>> = STRS.iter().map(|s| s.to_vec()).collect();
    let ns = strs.len() as u32;
    let cfg = [g.below(16), g.below(ns), g.below(ns), g.below(ns), g.below(9), g.below(60000), 0, 0];
    let mut ops: Vec<Op> = Vec::new();
    // blocks registered first, mostly
    for _ in 0..g.below(7) {
        let name = if g.chance(4) { g.below(ns) } else { g.below(6) };
        ops.push([0, name, g.below(6), if g.chance(3) { g.below(3) } else { 0 }, 0, 0]);
    }
    for _ in 0..3 + g.below(25) {
        let k = match g.below(100) {
            0..=6 => 0,
            7..=14 => 1,
            15..=29 => 2,
            30..=44 => 3,
            45..=49 => 4,
            50..=51 => 5,
            52..=57 => 6,
            58..=63 => 7,
            64..=68 => 8,
            69..=70 => 9,
            71..=74 => 10,
            75..=78 => 11,
            79..=83 => 12,
            84..=88 => 13,
            89..=93 => 14,
            _ => 15,
        };
        let o: Op = match k {
            0 => [k, if g.chance(4) { g.below(ns) } else { g.below(6) }, g.below(6), g.below(3), 0, 0],
            1 => [k, g.below(6), g.next(), 8 + g.below(4), 0, 0],
            2 => [k, if g.chance(4) { g.below(3) } else { 0 }, u32::from(g.chance(5)), 0, 0, 0],
            3 => [k, if g.chance(4) { g.below(3) } else { 0 }, if g.chance(2) { 0 } else { g.below(8) }, g.next(), g.below(256), g.next()],
            7 => [k, 0, u32::from(g.chance(3)), g.next(), g.below(256), 0],
            8 => [k, g.below(3), g.below(ns), g.below(ns), g.below(30), if g.chance(3) { 0 } else { g.next() }],
            11 => [k, 24 + g.below(3), 0, 0, 0, 0],
            12..=14 => [k, 11 + g.below(19), 0, 0, 0, 0],
            15 => {
                let a = g.next();
                [k, a, if g.chance(3) { a } else { g.next() }, 0, 0, 0]
            }
            _ => [k, 0, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops, strs)
}

#[test]
fn game_state_matches_upstream() {
    let cases: usize = std::env::var("GSTATE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x4753_5431_cafe_0001);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_op = [0usize; 16];
    for case in 0..cases {
        let (cfg, ops, strs) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &ops, &strs);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            *cut_why.entry(why.clone()).or_insert(0) += 1;
        }
        let c = cpp_run(&cfg, &ops[..n], &strs);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
        }
    }
    println!("S1: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
