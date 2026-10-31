#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameStateMap.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/GameStateMap.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameStateMap` (`common::game_state_map`) against the C++, layer 12 batch M2.
//!
//! # Method
//!
//! The C++ side is `GameStateMap.cpp` from its global to the end, verbatim (build.rs cuts it), compiled into
//! `differential/wrap_game_state.cpp` with batch S1's `GameState`, the GameInfo build's `SkirmishGameInfo`, the Xfer
//! build's real `XferSave` and `XferLoad` (on a file; their map-path hooks pointed at that `GameState`), and stand-ins
//! for the file system (`TheFileSystem`'s files, the C library's streams and Win32's directory search on one in-memory
//! disk whose paths are case-blind), the game logic (the game mode, the object id counter, `setLoadingSave`,
//! `startNewGame`) and the client (the drawable id counter). Every stand-in logs its calls, so the log holds the order
//! of the file operations, not only their outcome.
//!
//! A program sets the global map name, puts files on the disk (maps under the maps directory, the save directory and
//! elsewhere), sets the game mode and the id counters, injects failures (a short read of either kind, a failed create,
//! a short write), saves and loads the block (directly, or through `GameState::xferSaveData` as `CHUNK_GameStateMap`;
//! loads of the bytes as saved, with a byte replaced, or cut, with or without post-processing), changes the skirmish
//! game, post-processes, resets, and clears the save directory's maps. After each save or load it dumps the game
//! state (mode, counters, map names, the skirmish game's options and slots), the post-process list and the disk.
//!
//! An operation the Rust side reports as undefined ends the Rust run there; the C++ runs only the operations before
//! it: `tolower` of a byte of 0x80 or more in a map path, a block size of 2^31 or more (`new` fails on the shipped
//! 32-bit build), a damaged byte loaded into a skirmish slot's `Bool` (an invalid `bool`), and the skirmish game deleted while a load's post-process list still holds it (upstream's list then
//! dangles; the dump would compare a freed pointer).
//!
//! # Results (2026-10-31, `GSMAP_CASES=20000`, the default)
//!
//! 20,000 programs, 302,455 operations, 1,256,933 log lines (saves byte for byte, the disk after each save and load,
//! the file operations in order): **0 mismatches**. 2,618 programs cut at an undefined operation: 2,486 at `tolower`
//! of a byte of 0x80 or more in a map path, 17 in the skirmish game's map path, 30 at a block size of 2^31 or more, 28
//! at an invalid `Bool`, 57 at a skirmish game freed while queued. Paths reached (log lines): 11,235 saves and 7,046
//! loads that complete (each load starting the game), 4,636 loads queueing the skirmish game, 8,567 scratch pad maps
//! deleted, 26,979 saves and 3,680 loads throwing `SC_INVALID_DATA` (a missing file, a failed create or short write, a
//! loaded map outside the save directory). Saves throwing `XFER_WRITE_ERROR` are upstream's: an empty map file is
//! embedded with `xferUser` of 0 bytes, which `XferSave` refuses.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 8 of 8 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3991 | a short read of the save directory's map ignored | 298 programs |
//! | 3992 | a loaded map name outside the save directory accepted | 445 programs |
//! | 3993 | a first save embeds the save directory's map, not the pristine one | 13,666 programs |
//! | 3994 | the drawable id counter not xferred | 5,198 programs |
//! | 3995 | a new skirmish game's slots not cleared | 3,085 programs |
//! | 3996 | the skirmish game kept outside a skirmish | from program 398 on, until the C++ post-processes the skirmish game the plant kept the Rust side from freeing (a use after free there: the run dies) |
//! | 3997 | `setLoadingSave` left on after a load | 3,580 programs |
//! | 3998 | the scratch pad maps not deleted | 5,831 programs |

use rts_lockstep_engine::common::game_info::{GameInfo, GameInfoHost, MapMeta};
use rts_lockstep_engine::common::game_state::*;
use rts_lockstep_engine::common::game_state_map::*;
use rts_lockstep_engine::common::game_type::{DrawableId, ObjectId};
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::xfer::{take_invalid_bool_read, Xfer, XferResult, XferStatus, XO_NO_POST_PROCESSING};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
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

/// The test's blocks: the map block, the skirmish game and its starting cash (by generation: a deleted game is not the
/// next one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blk {
    Map,
    Skirmish(u32),
    Cash(u32),
}

struct Host {
    cfg: [u32; 8],
    strs: Vec<Vec<u8>>,
    map_name: Vec<u8>,
    mode: i32,
    object_id: ObjectId,
    drawable_id: DrawableId,
    loading_save: bool,
    tick: u32,
    skirmish: Option<GameInfo>,
    skirmish_gen: u32,
    /// The disk, by lower-case path.
    disk: BTreeMap<Vec<u8>, Vec<u8>>,
    faults: u32,
    cwd: Vec<u8>,
    open_path: Vec<u8>,
    pos: usize,
    found: Vec<FindItem>,
    found_at: usize,
    /// What the last map xfer threw beyond an `XferStatus` (a block can only return one).
    thrown: Option<MapXferError>,
    /// A map path `tolower` could not convert (GameInfo's path hooks cannot return the error).
    hook_undefined: Cell<bool>,
    log: RefCell<String>,
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl Host {
    fn str(&self, i: u32) -> Vec<u8> {
        self.strs[i as usize % self.strs.len()].clone()
    }
    fn line(&self, s: String) {
        let mut l = self.log.borrow_mut();
        l.push_str(&s);
        l.push('\n');
    }
}

impl GameStateHost<Blk> for Host {
    fn xfer_block(&mut self, block: Blk, gs: &mut GameState<Blk>, xfer: &mut dyn Xfer) -> XferResult<()> {
        assert_eq!(block, Blk::Map, "only the map is registered");
        match xfer_game_state_map(gs, xfer, self) {
            Ok(()) => Ok(()),
            Err(MapXferError::Xfer(s)) => Err(s),
            Err(e) => {
                self.thrown = Some(e);
                Err(XferStatus::ErrorUnknown)
            }
        }
    }
    fn load_post_process(&mut self, _block: Blk) {}
    fn partition_update(&mut self) {
        self.line("partition update".into());
    }
    fn user_data_path(&self) -> Vec<u8> {
        if self.cfg[0] & 2 != 0 {
            b"C:\\Users\\Me\\Generals\\".to_vec()
        } else {
            b"data\\".to_vec()
        }
    }
    fn map_name(&self) -> Vec<u8> {
        self.map_name.clone()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    fn file_exists(&self, _path: &[u8]) -> bool {
        false
    }
}

impl GameInfoHost for Host {
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        "T:".bytes().chain(label.bytes()).map(u16::from).collect()
    }
    fn tick_count(&mut self) -> u32 {
        let t = self.tick;
        self.tick = self.tick.wrapping_add(7);
        t
    }
    fn default_starting_cash(&self) -> Money {
        let mut m = Money::new();
        if self.cfg[6] & 1 != 0 {
            m.deposit(10000, false, &mut Vec::new());
        }
        m
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        false
    }
    fn map_cache_find(&self, _lower: &[u8]) -> Option<MapMeta> {
        None
    }
    fn file_exists(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        GameState::<Blk>::new().real_map_path_to_portable(path, self).unwrap_or_else(|_| {
            self.hook_undefined.set(true);
            Vec::new()
        })
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        GameState::<Blk>::new().portable_map_path_to_real(path, self).unwrap_or_else(|_| {
            self.hook_undefined.set(true);
            Vec::new()
        })
    }
    fn has_multiplayer_settings(&self) -> bool {
        false
    }
    fn num_colors(&self) -> i32 {
        0
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0x123456)
    }
    fn player_template_count(&self) -> i32 {
        0
    }
    fn player_template_display_name(&self, _n: i32) -> Result<Vec<u16>, IniError> {
        Ok(Vec::new())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

impl GameStateMapHost<Blk> for Host {
    fn set_loading_save(&mut self, on: bool) {
        self.loading_save = on;
        self.line(format!("loadingsave {}", i32::from(on)));
    }
    fn logic_game_mode(&self) -> i32 {
        self.mode
    }
    fn set_logic_game_mode(&mut self, mode: i32) {
        self.mode = mode;
        self.line(format!("setmode {mode}"));
    }
    fn set_map_name(&mut self, name: &[u8]) {
        self.map_name = name.to_vec();
    }
    fn object_id_counter(&self) -> ObjectId {
        self.object_id
    }
    fn set_object_id_counter(&mut self, id: ObjectId) {
        self.object_id = id;
        self.line(format!("objid {id}"));
    }
    fn drawable_id_counter(&self) -> DrawableId {
        self.drawable_id
    }
    fn set_drawable_id_counter(&mut self, id: DrawableId) {
        self.drawable_id = id;
        self.line(format!("drawid {id}"));
    }
    fn start_new_game(&mut self, loading_save_game: bool) {
        self.line(format!("startnewgame {}", i32::from(loading_save_game)));
    }
    fn skirmish_game_info(&mut self) -> &mut Option<GameInfo> {
        &mut self.skirmish
    }
    fn skirmish_block(&self) -> Blk {
        Blk::Skirmish(self.skirmish_gen)
    }
    fn skirmish_cash_block(&self) -> Blk {
        Blk::Cash(self.skirmish_gen)
    }
    fn fs_open(&mut self, path: &[u8]) -> bool {
        let ok = self.disk.contains_key(&path.to_ascii_lowercase());
        self.line(format!("fsopen [{}] {}", lossy(path), i32::from(ok)));
        if ok {
            self.open_path = path.to_ascii_lowercase();
            self.pos = 0;
        }
        ok
    }
    fn fs_seek_end(&mut self) -> i32 {
        let n = self.disk[&self.open_path].len();
        self.pos = n;
        self.line(format!("fssize {n}"));
        n as i32
    }
    fn fs_seek_start(&mut self) {
        self.pos = 0;
    }
    fn fs_read(&mut self, n: usize) -> Vec<u8> {
        let d = &self.disk[&self.open_path];
        let mut got = n.min(d.len().saturating_sub(self.pos));
        if self.faults & 1 != 0 && got > 0 {
            got -= 1;
        }
        let out = d[self.pos..self.pos + got].to_vec();
        self.pos += got;
        self.line(format!("fsread {n} -> {got}"));
        out
    }
    fn fs_close(&mut self) {
        self.line("fsclose".into());
    }
    fn c_fopen(&mut self, path: &[u8], mode: &str) -> bool {
        let write = mode.contains('w');
        let key = path.to_ascii_lowercase();
        let ok = if write { self.faults & 4 == 0 } else { self.disk.contains_key(&key) };
        self.line(format!("fopen [{}] {mode} {}", lossy(path), i32::from(ok)));
        if ok {
            if write {
                self.disk.insert(key.clone(), Vec::new());
            }
            self.open_path = key;
            self.pos = 0;
        }
        ok
    }
    fn c_size(&mut self) -> i32 {
        let n = self.disk[&self.open_path].len();
        self.line(format!("ftell {n}"));
        self.pos = 0;
        n as i32
    }
    fn c_fread(&mut self, n: usize) -> Vec<u8> {
        let d = &self.disk[&self.open_path];
        let mut got = n.min(d.len().saturating_sub(self.pos));
        if self.faults & 2 != 0 && got > 0 {
            got -= 1;
        }
        let out = d[self.pos..self.pos + got].to_vec();
        self.pos += got;
        self.line(format!("fread {n} -> {got}"));
        out
    }
    fn c_fwrite(&mut self, data: &[u8]) -> usize {
        let mut put = data.len();
        if self.faults & 8 != 0 && put > 0 {
            put -= 1;
        }
        let pos = self.pos;
        let d = self.disk.get_mut(&self.open_path).expect("open");
        for (i, &b) in data[..put].iter().enumerate() {
            if pos + i < d.len() {
                d[pos + i] = b;
            } else {
                d.push(b);
            }
        }
        self.pos += put;
        self.line(format!("fwrite {} -> {put}", data.len()));
        put
    }
    fn c_fclose(&mut self) {
        self.line("fclose".into());
    }
    fn current_directory(&mut self) -> Vec<u8> {
        self.cwd.clone()
    }
    fn set_current_directory(&mut self, dir: &[u8]) {
        self.cwd = dir.to_vec();
        self.line(format!("cd [{}]", lossy(dir)));
    }
    fn find_first_file(&mut self, pattern: &[u8]) -> Option<FindItem> {
        let dir = self.cwd.to_ascii_lowercase();
        let mut names = BTreeMap::<Vec<u8>, bool>::new();
        for k in self.disk.keys() {
            if !k.starts_with(&dir) || k.len() == dir.len() {
                continue;
            }
            let rest = &k[dir.len()..];
            match rest.iter().position(|&b| b == b'\\') {
                None => {
                    names.insert(rest.to_vec(), false);
                }
                Some(p) => {
                    names.insert(rest[..p].to_vec(), true);
                }
            }
        }
        self.found.clear();
        self.found_at = 0;
        if names.is_empty() {
            self.line(format!("findfirst [{}] -> none", lossy(pattern)));
            return None;
        }
        self.found.push(FindItem { name: b".".to_vec(), is_directory: true });
        self.found.push(FindItem { name: b"..".to_vec(), is_directory: true });
        self.found.extend(names.into_iter().map(|(name, is_directory)| FindItem { name, is_directory }));
        let f = self.found[0].clone();
        self.line(format!("findfirst [{}] -> [{}] {}", lossy(pattern), lossy(&f.name), i32::from(f.is_directory)));
        Some(f)
    }
    fn find_next_file(&mut self) -> Option<FindItem> {
        if self.found_at + 1 >= self.found.len() {
            self.line("findnext -> none".into());
            return None;
        }
        self.found_at += 1;
        let f = self.found[self.found_at].clone();
        self.line(format!("findnext -> [{}] {}", lossy(&f.name), i32::from(f.is_directory)));
        Some(f)
    }
    fn find_close(&mut self) {
        self.line("findclose".into());
    }
    fn delete_file(&mut self, name: &[u8]) {
        self.line(format!("delete [{}]", lossy(name)));
        let mut p = self.cwd.clone();
        p.extend_from_slice(name);
        self.disk.remove(&p.to_ascii_lowercase());
    }
}

fn dump_state(h: &Host, gs: &GameState<Blk>) {
    h.line(format!(
        "state mode {} objid {} drawid {} loading {} map [{}] savemap [{}] pristine [{}] cwd [{}]",
        h.mode,
        h.object_id,
        h.drawable_id,
        i32::from(h.loading_save),
        lossy(&h.map_name),
        lossy(&gs.info.save_game_map_name),
        lossy(&gs.info.pristine_map_name),
        lossy(&h.cwd)
    ));
    match &h.skirmish {
        None => h.line("skirmish none".into()),
        Some(s) => {
            let slots: String = s.slots.iter().map(|sl| format!(" {}", sl.state)).collect();
            h.line(format!(
                "skirmish seed {} map [{}] crc {} ip {} gid {} crcint {} cash {} slots{slots}",
                s.seed,
                lossy(&s.map_name),
                s.map_crc,
                s.local_ip,
                s.game_id,
                s.crc_interval,
                s.starting_cash.count_money()
            ));
        }
    }
}

fn dump_post_list(h: &Host, gs: &GameState<Blk>) {
    let pl: String = gs
        .post_process
        .iter()
        .map(|b| match b {
            Blk::Map => " M",
            Blk::Skirmish(_) => " S",
            Blk::Cash(_) => " C",
        })
        .collect();
    h.line(format!("postlist{pl}"));
}

fn dump_disk(h: &Host) {
    let mut d = String::new();
    for (k, v) in &h.disk {
        let mut x: u32 = 0;
        for &b in v {
            x = x.wrapping_mul(31).wrapping_add(u32::from(b));
        }
        let _ = write!(d, " [{}] {}:{x:08x}", lossy(k), v.len());
    }
    h.line(format!("disk{d}"));
}

fn thrown_line(what: &str, r: &Result<(), MapXferError>) -> Result<String, Cut> {
    Ok(match r {
        Ok(()) => what.to_string(),
        Err(MapXferError::Xfer(s)) => format!("{what} threw xfer {}", *s as i32),
        Err(MapXferError::InvalidData) => format!("{what} threw save {}", SC_INVALID_DATA),
        Err(MapXferError::Undefined(e)) => return Err(Cut(format!("{e:?}"))),
    })
}

/// `SC_INVALID_DATA`'s value (GameState.h's `SaveCode`: SC_INVALID = -1, SC_OK, SC_NO_FILE_AVAILABLE, SC_FILE_NOT_FOUND,
/// SC_UNABLE_TO_OPEN_FILE, SC_INVALID_XFER, SC_UNKNOWN_BLOCK, SC_INVALID_DATA).
const SC_INVALID_DATA: i32 = 6;

struct Cut(String);

fn damaged(saved: &[u8], b: u32, c: u32, d: u32, e: u32) -> Vec<u8> {
    let mut bytes = saved.to_vec();
    if b & 1 != 0 && !bytes.is_empty() {
        let n = bytes.len();
        bytes[c as usize % n] = d as u8;
    }
    if b & 2 != 0 && !bytes.is_empty() {
        let n = bytes.len();
        bytes.truncate(n - 1 - (e as usize % n));
    }
    bytes
}

fn op(gs: &mut GameState<Blk>, h: &mut Host, saved: &mut Vec<u8>, w: &Op) -> Result<(), Cut> {
    let (a, b, c, d, e) = (w[1], w[2], w[3], w[4], w[5]);
    match w[0] {
        0 => {
            h.map_name = h.str(a);
            h.line(format!("mapname [{}]", lossy(&h.str(a))));
        }
        1 => {
            let data: Vec<u8> = (0..b % 40).map(|k| c.wrapping_add(k * 7) as u8).collect();
            h.disk.insert(h.str(a).to_ascii_lowercase(), data);
            h.line(format!("put [{}] {}", lossy(&h.str(a)), b % 40));
        }
        2 => {
            h.mode = (a % 7) as i32;
            h.line(format!("mode {}", a % 7));
        }
        3 => {
            h.object_id = a as i32;
            h.drawable_id = b as i32;
            h.line("counters".into());
        }
        4 => {
            h.faults = a & 15;
            h.line(format!("faults {}", h.faults));
        }
        5 | 12 => {
            let mut x = XferSave::new();
            x.open(b"gsmap").expect("open");
            h.thrown = None;
            let r = if w[0] == 5 {
                xfer_game_state_map(gs, &mut x, h)
            } else {
                gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, h).map_err(|s| h.thrown.take().unwrap_or(MapXferError::Xfer(s)))
            };
            h.line(thrown_line("save", &r)?);
            *saved = x.bytes().to_vec();
            let hex: String = saved.iter().map(|b| format!("{b:02x}")).collect();
            h.line(format!("bytes {hex}"));
            dump_state(h, gs);
            dump_disk(h);
        }
        6 | 13 => {
            let mut x = XferLoad::new(damaged(saved, b, c, d, e));
            x.open(b"gsmap").expect("open");
            if b & 4 != 0 {
                x.set_options(XO_NO_POST_PROCESSING);
            }
            h.thrown = None;
            let r = if w[0] == 6 {
                xfer_game_state_map(gs, &mut x, h)
            } else {
                gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, h).map_err(|s| h.thrown.take().unwrap_or(MapXferError::Xfer(s)))
            };
            h.line(thrown_line("load", &r)?);
            dump_state(h, gs);
            dump_post_list(h, gs);
            dump_disk(h);
        }
        7 => {
            clear_scratch_pad_maps(gs, h);
            h.line("cleared".into());
            dump_disk(h);
        }
        8 => {
            dump_state(h, gs);
            dump_disk(h);
        }
        9 => {
            let map = h.str(b);
            let mut sk = h.skirmish.take();
            if let Some(s) = sk.as_mut() {
                s.seed = a as i32;
                s.map_name = map;
                s.map_crc = c;
                s.slots[(d % 8) as usize].set_state((e % 6) as i32, &[], 0, h);
            }
            h.skirmish = sk;
            h.line("skset".into());
        }
        10 => {
            gs.game_state_post_process_load(h);
            h.line("postprocessed".into());
        }
        11 => {
            gs.reset();
            h.line("reset".into());
        }
        _ => h.line("op?".into()),
    }
    Ok(())
}

fn rust_run(cfg: &[u32; 8], ops: &[Op], strs: &[Vec<u8>]) -> (String, Option<(usize, String)>) {
    let mut h = Host {
        cfg: *cfg,
        strs: strs.to_vec(),
        map_name: Vec::new(),
        mode: (cfg[2] % 7) as i32,
        object_id: cfg[3] as i32,
        drawable_id: cfg[4] as i32,
        loading_save: false,
        tick: cfg[7],
        skirmish: None,
        skirmish_gen: 0,
        disk: BTreeMap::new(),
        faults: 0,
        cwd: b"C:\\Game\\".to_vec(),
        open_path: Vec::new(),
        pos: 0,
        found: Vec::new(),
        found_at: 0,
        thrown: None,
        hook_undefined: Cell::new(false),
        log: RefCell::new(String::new()),
    };
    h.map_name = h.str(cfg[1]);
    take_invalid_bool_read();
    let mut gs = GameState::<Blk>::new();
    gs.add_snapshot_block(b"CHUNK_GameStateMap", Blk::Map, SnapshotType::SaveLoad);
    let mut saved = Vec::new();
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.borrow().len();
        // the skirmish game's generation names one object: it moves on when the object is deleted
        let had = h.skirmish.is_some();
        let r = op(&mut gs, &mut h, &mut saved, w);
        if had && h.skirmish.is_none() {
            h.skirmish_gen += 1;
        }
        let cut = match r {
            Err(Cut(why)) => Some(why),
            Ok(()) if h.hook_undefined.get() => Some("TolowerNonAscii (a skirmish map path)".to_string()),
            Ok(()) if take_invalid_bool_read() => Some("InvalidBoolLoaded".to_string()),
            Ok(()) => {
                let live = h.skirmish.is_some().then_some(h.skirmish_gen);
                gs.post_process
                    .iter()
                    .any(|b| matches!(b, Blk::Skirmish(g) | Blk::Cash(g) if Some(*g) != live))
                    .then(|| "skirmish game freed while queued for post-processing".to_string())
            }
        };
        if let Some(why) = cut {
            h.log.borrow_mut().truncate(before);
            return (h.log.into_inner(), Some((i, why)));
        }
    }
    (h.log.into_inner(), None)
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
        let len = unsafe { cpp::ref_gsmap_run(cfg.as_ptr(), flat.as_ptr(), ops.len() as i32, packed.as_ptr().cast(), strs.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

const STRS: [&[u8]; 20] = [
    b"Maps\\alpha\\alpha.map",
    b"maps\\Beta\\beta.map",
    b"data\\Save\\alpha.map",
    b"data\\save\\beta.map",
    b"C:\\Users\\Me\\Generals\\Save\\alpha.map",
    b"C:\\Users\\Me\\Generals\\Maps\\c\\c.map",
    b"data\\Maps\\d\\d.map",
    b"data\\Save\\notes.txt",
    b"data\\Save\\sub\\x.map",
    b"data\\Save\\UPPER.MAP",
    b"C:\\Users\\Me\\Generals\\Save\\other.Map",
    b"noslash.map",
    b"Maps\\\xe9t\xe9\\x.map",
    b"elsewhere\\e.map",
    b"",
    b"data\\Save\\",
    b"Maps\\Somewhere\\Somewhere.map",
    b"save\\alpha.map",
    b"data\\Save\\.map",
    b"zz",
];

fn gen_program(g: &mut Gen) -> ([u32; 8], Vec<Op>, Vec<Vec<u8>>) {
    let strs: Vec<Vec<u8>> = STRS.iter().map(|s| s.to_vec()).collect();
    let ns = strs.len() as u32;
    let small = |g: &mut Gen| if g.chance(3) { g.next() } else { g.below(1000) };
    let cfg = [g.below(4), g.below(ns), if g.chance(2) { 2 } else { g.below(7) }, small(g), small(g), 0, g.below(2), g.below(100_000)];
    let mut ops: Vec<Op> = Vec::new();
    // a few maps on the disk first, mostly
    for _ in 0..g.below(5) {
        ops.push([1, g.below(ns), g.below(40), g.next(), 0, 0]);
    }
    for _ in 0..3 + g.below(20) {
        let k = match g.below(100) {
            0..=7 => 0,
            8..=17 => 1,
            18..=23 => 2,
            24..=27 => 3,
            28..=33 => 4,
            34..=48 => 5,
            49..=63 => 6,
            64..=69 => 7,
            70..=71 => 8,
            72..=76 => 9,
            77..=80 => 10,
            81..=82 => 11,
            83..=90 => 12,
            _ => 13,
        };
        // a map name, often with its file, and a save, often followed by its load
        if k == 0 && g.chance(2) {
            let i = g.below(11);
            ops.push([1, i, 1 + g.below(39), g.next(), 0, 0]);
            ops.push([0, i, 0, 0, 0, 0]);
            continue;
        }
        if (k == 5 || k == 12) && g.chance(2) {
            ops.push([k, 0, 0, 0, 0, 0]);
            let damage = if g.chance(3) { g.below(8) } else { 4 * g.below(2) };
            ops.push([k + 1, 0, damage, g.next(), g.below(256), g.next()]);
            continue;
        }
        let o: Op = match k {
            0 => [k, g.below(ns), 0, 0, 0, 0],
            1 => [k, g.below(ns), g.below(40), g.next(), 0, 0],
            2 => [k, if g.chance(2) { 2 } else { g.below(7) }, 0, 0, 0, 0],
            3 => [k, small(g), small(g), 0, 0, 0],
            4 => [k, if g.chance(2) { 0 } else { g.below(16) }, 0, 0, 0, 0],
            6 | 13 => [k, 0, if g.chance(2) { 0 } else { g.below(8) }, g.next(), g.below(256), g.next()],
            9 => [k, g.next(), g.below(ns), g.next(), g.below(8), g.below(6)],
            _ => [k, 0, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops, strs)
}

/// Log lines whose count shows a path was reached.
const MARKS: [&str; 16] = [
    "fsopen", "fsclose", "fopen", "fwrite", "save threw save", "save threw xfer", "load threw save", "load threw xfer", "=save", "=load",
    "startnewgame", "delete [", "findfirst", "postlist C S", "skirmish seed", "postprocessed",
];

#[test]
fn game_state_map_matches_upstream() {
    let cases: usize = std::env::var("GSMAP_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x4753_4d32_cafe_0001);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_op = [0usize; 14];
    let mut marks = [0usize; MARKS.len()];
    let mut threw = BTreeMap::<String, usize>::new();
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
                if std::env::var("GSMAP_DUMP").is_ok() {
                    println!("--- rust\n{r}--- c++\n{c}---");
                }
            }
        } else {
            lines += r.lines().count();
            for l in r.lines() {
                if l.contains(" threw ") {
                    *threw.entry(l.to_string()).or_insert(0) += 1;
                }
                for (k, m) in MARKS.iter().enumerate() {
                    if m.strip_prefix('=').map_or(l.starts_with(m), |e| l == e) {
                        marks[k] += 1;
                    }
                }
            }
        }
    }
    println!("reached: {:?}", MARKS.iter().zip(marks).collect::<Vec<_>>());
    println!("throws: {threw:?}");
    println!("M2: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
