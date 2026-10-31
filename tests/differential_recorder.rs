#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Recorder.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/Recorder.h
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `RecorderClass` and `GameMessageParser` (`common::recorder`) against the C++, layer 12 batch R1.
//!
//! # Method
//!
//! The C++ side is `Recorder.cpp` (build.rs's copy, with readUnicodeString's `= L""` initialiser written `= {0}`), its
//! header and `GameMessageParser.cpp`, compiled by `differential/wrap_recorder.cpp` on the stand-ins of
//! `differential/shim_rec` and linked to the message stream (batch M1) and GameInfo (batch G1) builds, with Windows'
//! 32-bit `time_t`, `SYSTEMTIME` and two-byte wide stream functions stood in (`fgetwc` returning `WEOF`, 0xFFFF). The
//! replay files are real files in a directory of the process; `time()` and `GetLocalTime()` count. It runs a program:
//! the local player, the frame and the game mode; commands of every argument kind put on the command list (among them
//! `MSG_NEW_GAME` of each mode, which starts a recording, and `MSG_CLEAR_GAME_DATA`, which ends it); the recorder's
//! `update` (recording or playing back), `stopRecording`, `playbackFile` and `testVersionPlayback` of replay files (one
//! the recording made, and generated ones: well-formed headers and command streams, then damaged by cutting, replacing
//! or inserting bytes), `handleCRCMessage` from the replay and from the game, the disconnect and CRC-mismatch flags,
//! `reset`, `stopPlayback`, `isMultiplayer`, a skirmish game or none, the stream propagated to the command list, the
//! players' names; and dumps of the recorder, its game, and the two message lists. At the end every replay file is
//! compared byte for byte (the open file closed first; files upstream leaves open unflushed). The Rust side runs the
//! same program on [`Recorder`] with an in-memory file system.
//!
//! An operation the Rust side reports as undefined ends the Rust run there, and the C++ runs only the operations
//! before it.
//!
//! # Results (2026-10-31, `REC_CASES=20000`, the default)
//!
//! 20,000 programs, 542,118 operations, 1,175,613 log lines (24,320 recordings written, 22,069 playbacks started),
//! every replay file compared byte for byte at the end: **0 mismatches**. 7,840 programs cut at an undefined operation:
//! 4,132 a header or command read short (uninitialised locals: a file damaged, cut, or still being written), 2,968
//! `fread` on a `NULL` stream (an update, a CRC or a playback step after the playback's file was closed, or with no
//! playback), 660 a wide or narrow string running past 1,024 units (`fgetwc`'s `WEOF` is never `EOF`), 62 a `Bool`
//! argument byte other than 0 or 1, and a few `tolower` of a byte of 0x80 or more or a number out of range in the game
//! options.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 9 of 9 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3961 | `MSG_LOGIC_CRC` not recorded | 42 programs |
//! | 3962 | the next argument type run never started | 3 programs, then the C++ crashes on the diverged state |
//! | 3963 | a file without `GENREP` read on | 3 programs, then the C++ crashes on the diverged state |
//! | 3964 | the end of the file does not stop the playback | 2,317 programs |
//! | 3965 | `cullBadCommands` deletes `MSG_LOGIC_CRC` too | 1,082 programs |
//! | 3966 | `readCRC` takes the newest CRC, not the oldest | 226 programs |
//! | 3967 | a single-player recording's local slot written as 0 | 242 programs |
//! | 3968 | a `Bool` argument written as four bytes | 24 programs |
//! | 3969 | the playback's `MSG_NEW_GAME` without its frame-rate limit | 4,983 programs |

use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::{IniError, Undefined};
use rts_lockstep_engine::common::message_stream::*;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::recorder::*;
use rts_lockstep_engine::differential as cpp;
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

struct Host {
    gcfg: [u32; 9],
    cfg: [u32; 8],
    tick: u32,
    time: i32,
    local_time: u16,
    seed: u32,
    frame: u32,
    mode: i32,
    local: i32,
    player_count: i32,
    player_names: [i32; 8],
    pending: Vec<u8>,
    dir: Vec<u8>,
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    msgs: Messages,
    skirmish: GameInfo,
    skirmish_on: bool,
    log: String,
}

fn swap_prefix(p: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let s = p.to_ascii_lowercase();
    if s.starts_with(from) {
        let mut r = to.to_vec();
        r.extend_from_slice(&s[from.len()..]);
        r
    } else {
        s
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
        if self.gcfg[0] & 32 != 0 {
            m.deposit(10000, false, &mut Vec::new());
        }
        m
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        self.gcfg[0] & 1 != 0
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<MapMeta> {
        match lower {
            b"data\\alpha\\alpha.map" => Some(MapMeta { num_players: self.gcfg[4] as i32, crc: self.gcfg[5] }),
            b"data\\beta\\beta.map" => Some(MapMeta { num_players: self.gcfg[6] as i32, crc: self.gcfg[7] }),
            _ => None,
        }
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        let mut h = self.gcfg[8];
        for &b in path {
            h = h.wrapping_mul(31).wrapping_add(u32::from(b));
        }
        (h >> 7).is_multiple_of(3)
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        swap_prefix(path, b"data\\", b"maps\\")
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        swap_prefix(path, b"maps\\", b"data\\")
    }
    fn has_multiplayer_settings(&self) -> bool {
        self.gcfg[0] & 2 != 0
    }
    fn num_colors(&self) -> i32 {
        self.gcfg[1] as i32
    }
    fn show_random_player_template(&self) -> bool {
        self.gcfg[0] & 4 != 0
    }
    fn show_random_color(&self) -> bool {
        self.gcfg[0] & 8 != 0
    }
    fn show_random_start_pos(&self) -> bool {
        self.gcfg[0] & 16 != 0
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0x123456)
    }
    fn player_template_count(&self) -> i32 {
        self.gcfg[2] as i32
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, IniError> {
        if n < 0 || n >= self.gcfg[2] as i32 {
            return Err(IniError::undefined(Undefined::NullObject));
        }
        Ok(format!("PT{n}").bytes().map(u16::from).collect())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

fn wide16(s: &str) -> Vec<u16> {
    s.bytes().map(u16::from).collect()
}

impl RecorderHost for Host {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn is_in_game(&self) -> bool {
        self.mode != game_mode::NONE
    }
    fn game_mode(&self) -> i32 {
        self.mode
    }
    fn clear_game_data(&mut self) {
        self.log.push_str("cleargamedata\n");
        self.mode = game_mode::NONE;
    }
    fn messages(&mut self) -> &mut Messages {
        &mut self.msgs
    }
    fn local_player_index(&self) -> i32 {
        self.local
    }
    fn player_name_key_is(&mut self, i: i32, name: &[u8]) -> Option<bool> {
        if i < 0 || i >= self.player_count {
            return None;
        }
        Some(format!("player{}", self.player_names[i as usize]).as_bytes() == name)
    }
    fn pending_file(&self) -> Vec<u8> {
        self.pending.clone()
    }
    fn set_pending_file(&mut self, name: &[u8]) {
        self.pending = name.to_vec();
    }
    fn map_name(&self) -> Vec<u8> {
        b"data\\beta\\beta.map".to_vec()
    }
    fn user_data_path(&self) -> Vec<u8> {
        self.dir.clone()
    }
    fn exe_crc(&self) -> u32 {
        self.cfg[3]
    }
    fn ini_crc(&self) -> u32 {
        self.cfg[4]
    }
    fn version_string(&self) -> Vec<u16> {
        wide16("1.04")
    }
    fn version_time_string(&self) -> Vec<u16> {
        wide16(if self.cfg[2] & 2 != 0 { "Oct 31 2026" } else { "Jan 1 2003" })
    }
    fn version_number(&self) -> u32 {
        self.cfg[5]
    }
    fn local_time(&mut self) -> [u8; 16] {
        let mut t = [0u8; 16];
        for i in 0..8 {
            t[2 * i..2 * i + 2].copy_from_slice(&self.local_time.wrapping_add(i as u16).to_le_bytes());
        }
        self.local_time = self.local_time.wrapping_add(8);
        t
    }
    fn time(&mut self) -> u32 {
        let v = self.time;
        self.time = self.time.wrapping_add(100);
        v as u32
    }
    fn game_logic_random_seed(&self) -> u32 {
        self.seed
    }
    fn init_random(&mut self, seed: u32) {
        self.seed = seed;
    }
    fn create_directory(&mut self, path: &[u8]) {
        let _ = writeln!(self.log, "mkdir {}", String::from_utf8_lossy(path));
    }
    fn read_file(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }
    fn create_file(&mut self, path: &[u8]) -> bool {
        self.files.insert(path.to_vec(), Vec::new());
        true
    }
    fn write_file(&mut self, path: &[u8], bytes: &[u8]) {
        self.files.insert(path.to_vec(), bytes.to_vec());
    }
    fn has_network(&self) -> bool {
        false
    }
    fn network_slot_list(&mut self) -> (Vec<u8>, i32) {
        unreachable!("no network in the tests")
    }
    fn skirmish_game_info(&mut self) -> Option<&mut GameInfo> {
        if self.skirmish_on {
            Some(&mut self.skirmish)
        } else {
            None
        }
    }
}

fn wide(u: &[u16]) -> String {
    let mut r = String::new();
    for &c in u {
        if (0x20..0x7f).contains(&c) && c != u16::from(b'\\') {
            r.push(c as u8 as char);
        } else {
            r.push_str(&format!("\\u{c:04x}"));
        }
    }
    r
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn dump_recorder(r: &Recorder, h: &mut Host) {
    let pos = r.file.as_ref().map_or(-1i64, |f| f.pos as i64);
    let _ = writeln!(
        h.log,
        "rec mode {} orig {} next {} file {} name [{}] pos {} desync {} interval {} current [{}] seed {}",
        r.mode as i32,
        r.original_game_mode,
        r.next_frame,
        i32::from(r.file.is_some()),
        lossy(&r.file_name),
        pos,
        i32::from(r.was_desync),
        r.replay_crc_interval,
        lossy(&r.current_replay_filename()),
        h.seed
    );
    if let Some(c) = &r.crc_info {
        let d: String = c.data.iter().map(|v| format!(" {v:X}")).collect();
        let _ = writeln!(h.log, " crc local {} saw {} data{d}", c.local_player, i32::from(c.saw_crc_mismatch));
    }
    let g = &r.game_info;
    let _ = writeln!(
        h.log,
        " game in {} prog {} ip {:X} map [{}] seed {} crc {} mc {:X} ms {} mm {}",
        i32::from(g.in_game),
        i32::from(g.in_progress),
        g.local_ip,
        lossy(&g.map_name),
        g.seed,
        g.crc_interval,
        g.map_crc,
        g.map_size,
        g.map_mask
    );
    for (i, s) in g.slots.iter().enumerate() {
        let _ = writeln!(
            h.log,
            "  slot {i} st {} name [{}] ip {:X} col {} pt {} sp {} team {} acc {}",
            s.state,
            wide(&s.name),
            s.ip,
            s.color,
            s.player_template,
            s.start_pos,
            s.team_number,
            i32::from(s.is_accepted)
        );
    }
}

fn dump_list(h: &mut Host, what: &str, list: ListId) -> Result<(), IniError> {
    let _ = writeln!(h.log, "{what}:");
    for id in h.msgs.list_messages(list)? {
        let m = h.msgs.message(id)?.clone();
        let args: String = m
            .arguments()
            .iter()
            .map(|a| format!(" {}:{:08x}.{:08x}.{:08x}.{:08x}", a.ty as i32, a.data.0[0], a.data.0[1], a.data.0[2], a.data.0[3]))
            .collect();
        let _ = writeln!(h.log, " msg type {} player {} count {}{args}", m.get_type().0, m.get_player_index(), m.get_argument_count());
    }
    Ok(())
}

struct Cut(String);

fn e<T>(r: Result<T, IniError>) -> Result<T, Cut> {
    r.map_err(|e| Cut(format!("{e:?}")))
}

fn op(r: &mut Recorder, h: &mut Host, names: &[Vec<u8>], w: &Op) -> Result<(), Cut> {
    let (a, b, c, d, ev) = (w[1], w[2], w[3], w[4], w[5]);
    let name = |i: u32| if names.is_empty() { b"none".to_vec() } else { names[i as usize % names.len()].clone() };
    match w[0] {
        0 => {
            h.local = a as i32;
            let _ = writeln!(h.log, "local {}", a as i32);
        }
        1 => {
            h.frame = a;
            let _ = writeln!(h.log, "frame {a}");
        }
        2 => {
            h.mode = a as i32;
            let _ = writeln!(h.log, "mode {}", a as i32);
        }
        3 => {
            let local = h.local;
            let id = h.msgs.new_message(MessageType(a as i32), local);
            let m = e(h.msgs.message_mut(id))?;
            for i in 0..b {
                let k = (c >> (3 * (i % 10))) % 11;
                let x = d.wrapping_add(i.wrapping_mul(0x0100_0193));
                let y = ev ^ i.wrapping_mul(31);
                let (rx, ry, rz) = (f32::from_bits(x), f32::from_bits(y), f32::from_bits(d));
                use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, IRegion2D};
                match k {
                    0 => m.append_integer_argument(x as i32),
                    1 => m.append_real_argument(rx),
                    2 => m.append_boolean_argument(x & 1 != 0),
                    3 => m.append_object_id_argument(x),
                    4 => m.append_drawable_id_argument(x),
                    5 => m.append_team_id_argument(x),
                    6 => m.append_location_argument(Coord3D { x: rx, y: ry, z: rz }),
                    7 => m.append_pixel_argument(ICoord2D { x: x as i32, y: y as i32 }),
                    8 => m.append_pixel_region_argument(IRegion2D { lo: ICoord2D { x: x as i32, y: y as i32 }, hi: ICoord2D { x: d as i32, y: ev as i32 } }),
                    9 => m.append_timestamp_argument(x),
                    _ => m.append_wide_char_argument(x as u16),
                }
            }
            e(h.msgs.append_message(Messages::COMMAND_LIST, id))?;
            let _ = writeln!(h.log, "command {} args {b}", a as i32);
        }
        4 => {
            e(h.msgs.destroy_all_messages(Messages::COMMAND_LIST))?;
            h.log.push_str("command reset\n");
        }
        5 => {
            e(r.update(h))?;
            h.log.push_str("update\n");
        }
        6 => {
            r.stop_recording(h);
            h.log.push_str("stoprecording\n");
        }
        7 => {
            let n = name(a);
            let v = e(r.playback_file(&n, h))?;
            let _ = writeln!(h.log, "playback {}: {}", lossy(&n), i32::from(v));
        }
        8 => {
            let n = name(a);
            let v = e(r.test_version_playback(&n, h))?;
            let _ = writeln!(h.log, "testversion {}: {}", lossy(&n), i32::from(v));
        }
        9 => {
            e(r.handle_crc_message(a, b as i32, c & 1 != 0, h))?;
            let _ = writeln!(h.log, "crc {a:X} {} {}", b as i32, c & 1);
        }
        10 => {
            r.log_player_disconnect(a as i32, h);
            let _ = writeln!(h.log, "discon {}", a as i32);
        }
        11 => {
            r.log_crc_mismatch(h);
            h.log.push_str("crcmismatch\n");
        }
        12 => dump_recorder(r, h),
        13 => {
            e(dump_list(h, "commands", Messages::COMMAND_LIST))?;
            e(dump_list(h, "stream", Messages::STREAM))?;
        }
        14 => {
            r.reset(h);
            h.log.push_str("reset\n");
        }
        15 => {
            e(r.stop_playback(h))?;
            h.log.push_str("stopplayback\n");
        }
        16 => {
            let v = r.is_multiplayer(h);
            let _ = writeln!(h.log, "multiplayer {}", i32::from(v));
        }
        17 => {
            h.skirmish_on = a & 1 != 0;
            let _ = writeln!(h.log, "skirmish {}", a & 1);
        }
        18 => {
            e(Translators::<()>::new().propagate_messages(&mut h.msgs, |_, _, _| Ok(Disposition::Keep)))?;
            h.log.push_str("propagate\n");
        }
        19 => {
            h.player_count = (a % 9) as i32;
            for i in 0..8 {
                h.player_names[i] = ((b >> (4 * i)) & 0xf) as i32 - 1;
            }
            let _ = writeln!(h.log, "players {}", h.player_count);
        }
        20 => {
            e(r.update_playback(h))?;
            h.log.push_str("updateplayback\n");
        }
        _ => h.log.push_str("op?\n"),
    }
    Ok(())
}

fn dir() -> Vec<u8> {
    format!("/tmp/ref_rec_{}/", std::process::id()).into_bytes()
}

fn rust_run(cfg: &[u32; 8], gcfg: &[u32; 9], ops: &[Op], files: &[(Vec<u8>, Vec<u8>)]) -> (String, Option<(usize, String)>) {
    let mut fs = BTreeMap::new();
    let d = dir();
    for (n, c) in files {
        let mut p = d.clone();
        p.extend_from_slice(b"Replays\\");
        p.extend_from_slice(n);
        fs.insert(p, c.clone());
    }
    let mut h = Host {
        gcfg: *gcfg,
        cfg: *cfg,
        tick: gcfg[3],
        time: cfg[0] as i32,
        local_time: cfg[1] as u16,
        seed: cfg[6],
        frame: 0,
        mode: game_mode::NONE,
        local: 0,
        player_count: 0,
        player_names: [0; 8],
        pending: if cfg[2] & 1 != 0 { b"data\\alpha\\alpha.map".to_vec() } else { Vec::new() },
        dir: d.clone(),
        files: fs,
        msgs: Messages::new(),
        skirmish: GameInfo { ..dummy_game() },
        skirmish_on: false,
        log: String::new(),
    };
    h.skirmish = GameInfo::new(&mut h);
    let mut r = Recorder::new(&mut h);
    h.log.push_str("made\n");
    dump_recorder(&r, &mut h);
    let names: Vec<Vec<u8>> = files.iter().map(|f| f.0.clone()).collect();
    let mut cut = None;
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(Cut(why)) = op(&mut r, &mut h, &names, w) {
            h.log.truncate(before);
            cut = Some((i, why));
            break;
        }
    }
    if cut.is_none() {
        // the end: the open file closed (a written one reaches the disk), every file listed
        if let Some(f) = r.file.take() {
            if is_writing(&f) {
                h.files.insert(f.path.clone(), f.bytes.clone());
            }
        }
        let mut all = names.clone();
        all.push(b"00000000.rep".to_vec());
        for n in all {
            let mut p = d.clone();
            p.extend_from_slice(b"Replays\\");
            p.extend_from_slice(&n);
            match h.files.get(&p) {
                None => {
                    let _ = writeln!(h.log, "file {}: none", lossy(&n));
                }
                Some(b) => {
                    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
                    let _ = writeln!(h.log, "file {}: {hex}", lossy(&n));
                }
            }
        }
    }
    (h.log, cut)
}

fn is_writing(f: &ReplayFile) -> bool {
    f.is_writing()
}

fn dummy_game() -> GameInfo {
    struct Null;
    impl GameInfoHost for Null {
        fn game_text(&mut self, _: &str) -> Vec<u16> {
            Vec::new()
        }
        fn tick_count(&mut self) -> u32 {
            0
        }
        fn default_starting_cash(&self) -> Money {
            Money::new()
        }
        fn net_crc_interval(&self) -> i32 {
            100
        }
        fn has_map_cache(&self) -> bool {
            false
        }
        fn map_cache_find(&self, _: &[u8]) -> Option<MapMeta> {
            None
        }
        fn file_exists(&mut self, _: &[u8]) -> bool {
            false
        }
        fn real_map_path_to_portable(&self, p: &[u8]) -> Vec<u8> {
            p.to_vec()
        }
        fn portable_map_path_to_real(&self, p: &[u8]) -> Vec<u8> {
            p.to_vec()
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
            Ok(0)
        }
        fn player_template_count(&self) -> i32 {
            0
        }
        fn player_template_display_name(&self, _: i32) -> Result<Vec<u16>, IniError> {
            Ok(Vec::new())
        }
        fn record_income(&mut self, _: i32) {}
    }
    GameInfo::new(&mut Null)
}

fn cpp_run(cfg: &[u32; 8], gcfg: &[u32; 9], ops: &[Op], files: &[(Vec<u8>, Vec<u8>)]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut packed = Vec::new();
    for (n, c) in files {
        packed.extend_from_slice(&(n.len() as u16).to_le_bytes());
        packed.extend_from_slice(n);
        packed.extend_from_slice(&(c.len() as u32).to_le_bytes()[..3]);
        packed.extend_from_slice(c);
    }
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe {
            cpp::ref_rec_run(
                cfg.as_ptr(),
                gcfg.as_ptr(),
                flat.as_ptr(),
                ops.len() as i32,
                packed.as_ptr().cast(),
                files.len() as i32,
                out.as_mut_ptr().cast(),
                cap as i32,
            )
        } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

fn utf16z(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

/// A replay file: a header like `startRecording`'s, then commands; then perhaps damaged.
fn gen_replay(g: &mut Gen, cfg: &[u32; 8]) -> Vec<u8> {
    let mut f = b"GENREP".to_vec();
    for _ in 0..3 {
        f.extend_from_slice(&g.next().to_le_bytes());
    }
    for _ in 0..10 {
        f.push(if g.chance(4) { 1 } else { 0 });
    }
    f.extend_from_slice(&utf16z("T:GUI:LastReplay"));
    for _ in 0..16 {
        f.push(g.below(256) as u8);
    }
    f.extend_from_slice(&utf16z("1.04"));
    f.extend_from_slice(&utf16z(if g.chance(2) { "Oct 31 2026" } else { "Jan 1 2003" }));
    f.extend_from_slice(&(if g.chance(2) { cfg[5] } else { g.next() }).to_le_bytes());
    f.extend_from_slice(&(if g.chance(2) { cfg[3] } else { g.next() }).to_le_bytes());
    f.extend_from_slice(&(if g.chance(2) { cfg[4] } else { g.next() }).to_le_bytes());
    let slots = match g.below(4) {
        0 => "X:X:X:X:X:X:X:X:",
        1 => "HBob,0,8088,TT,0,1,2,0,1:CE,3,2,-1,1:O:X:X:X:X:X:",
        2 => "HAl,C0A80001,0,TT,1,0,0,1,1:HBo,C0A80002,0,TF,2,1,1,0,1:X:X:X:X:X:X:",
        _ => "CH,1,1,3,0:X:O:X:X:X:X:X:",
    };
    let opts = format!(
        "US=1;M={:02x}maps/{};MC={:X};MS={};SD={};C={};SR=0;SC=10000;O=N;S={slots};",
        g.below(128),
        ["alpha", "beta", "gamma"][g.below(3) as usize],
        g.below(3),
        g.below(1000),
        g.next() as i32 / 4,
        [30, 100, 200][g.below(3) as usize]
    );
    f.extend_from_slice(opts.as_bytes());
    f.push(0);
    f.extend_from_slice(["0", "-1", "1", "7", "8", "x"][g.below(6) as usize].as_bytes());
    f.push(0);
    for _ in 0..4 {
        f.extend_from_slice(&(g.below(70) as i32 - 2).to_le_bytes());
    }
    // the commands
    let mut frame = g.below(3);
    for _ in 0..g.below(8) {
        f.extend_from_slice(&frame.to_le_bytes());
        let t = [1001, 1050, 1095, 1000, 27, 1999, 2000, g.below(2100)][g.below(8) as usize];
        f.extend_from_slice(&t.to_le_bytes());
        f.extend_from_slice(&(g.below(4) as i32 - 1).to_le_bytes());
        let runs = g.below(4);
        f.push(runs as u8);
        let mut kinds = Vec::new();
        for _ in 0..runs {
            let k = if g.chance(10) { 11 + g.below(3) } else { g.below(11) };
            let n = if g.chance(10) { 0 } else { 1 + g.below(3) };
            f.push(k as u8);
            f.push(n as u8);
            kinds.push((k, n));
        }
        for (k, n) in kinds {
            let size = match k {
                0 | 1 | 3 | 4 | 5 | 9 => 4,
                2 => 1,
                6 => 12,
                7 => 8,
                8 => 16,
                10 => 2,
                _ => 0,
            };
            for _ in 0..n.max(1) {
                for _ in 0..size {
                    f.push(if k == 2 { g.below(2) as u8 } else { g.below(256) as u8 });
                }
            }
        }
        frame += g.below(3);
    }
    // damage
    if g.chance(3) {
        match g.below(4) {
            0 => {
                let n = f.len();
                f.truncate(g.below(n as u32) as usize);
            }
            1 => {
                let n = f.len();
                let at = g.below(n as u32) as usize;
                f[at] = g.below(256) as u8;
            }
            2 => {
                let n = f.len();
                let at = g.below(n as u32) as usize;
                f.insert(at, g.below(256) as u8);
            }
            _ => {
                let n = f.len();
                f.truncate(n.saturating_sub(1 + g.below(8) as usize));
            }
        }
    }
    f
}

type Files = Vec<(Vec<u8>, Vec<u8>)>;

fn gen_program(g: &mut Gen) -> ([u32; 8], [u32; 9], Vec<Op>, Files) {
    let cfg = [g.below(1_000_000), g.below(60000), g.below(4), g.next(), g.next(), g.next(), g.next(), 0];
    let gcfg = [1 | 2 | (g.below(64) & !3), 8, 12, g.next(), g.below(9), g.below(3), g.below(9), g.below(3), g.next()];
    let mut files = vec![(b"00000000.rep".to_vec(), Vec::new())];
    files.clear();
    for i in 0..3 {
        files.push((format!("r{i}.rep").into_bytes(), gen_replay(g, &cfg)));
    }
    // the file a recording writes (index 3), empty to begin with
    files.push((b"00000000.rep".to_vec(), Vec::new()));
    let n = 4 + g.below(40);
    let mut ops = Vec::new();
    let mut frame = 0u32;
    let mut played = false;
    let mut closed = false;
    let cmd = |g: &mut Gen| -> Op {
        let t = if g.chance(6) { 1095 } else { 1001 + g.below(90) };
        [3, t, g.below(5), g.next(), g.next(), g.next()]
    };
    // a recording: a new game of a recorded mode, commands over frames, perhaps its end
    if g.chance(2) {
        ops.push([2, [1, 2, 3, 5][g.below(4) as usize], 0, 0, 0, 0]);
        ops.push([3, 2, 1 + g.below(4), 0, [1, 2, 3, 5][g.below(4) as usize], g.next()]);
        ops.push([5, 0, 0, 0, 0, 0]);
        ops.push([4, 0, 0, 0, 0, 0]);
        for _ in 0..g.below(6) {
            frame += 1 + g.below(3);
            ops.push([1, frame, 0, 0, 0, 0]);
            for _ in 0..g.below(3) {
                ops.push(cmd(g));
            }
            ops.push([5, 0, 0, 0, 0, 0]);
            ops.push([4, 0, 0, 0, 0, 0]);
        }
        if !g.chance(3) {
            closed = true;
            ops.push([3, 27, 0, 0, 0, 0]);
            ops.push([5, 0, 0, 0, 0, 0]);
            ops.push([4, 0, 0, 0, 0, 0]);
        }
    }
    // a playback, stepped frame by frame (sometimes of the file just recorded)
    if g.chance(2) {
        frame = 0;
        ops.push([1, 0, 0, 0, 0, 0]);
        ops.push([7, if closed && !g.chance(3) { 3 } else { g.below(3) }, 0, 0, 0, 0]);
        played = true;
        for _ in 0..g.below(10) {
            ops.push([20, 0, 0, 0, 0, 0]);
            if g.chance(3) {
                ops.push([9, g.next(), g.below(3), 1, 0, 0]);
            }
            frame += 1;
            ops.push([1, frame, 0, 0, 0, 0]);
        }
    }
    for _ in 0..n {
        let k = match g.below(100) {
            0..=4 => 0,
            5..=14 => 1,
            15..=17 => 2,
            18..=34 => 3,
            35..=37 => 4,
            38..=52 => 5,
            53..=55 => 6,
            56..=62 => 7,
            63..=64 => 8,
            65..=69 => 9,
            70 => 10,
            71 => 11,
            72..=77 => 12,
            78..=81 => 13,
            82 => 14,
            83 => 15,
            84 => 16,
            85..=86 => 17,
            87..=90 => 18,
            91..=92 => 19,
            _ => 20,
        };
        let o: Op = match k {
            0 => [k, g.below(4), 0, 0, 0, 0],
            1 => {
                frame += g.below(3);
                [k, frame, 0, 0, 0, 0]
            }
            2 => [k, g.below(7), 0, 0, 0, 0],
            3 => {
                let t = match g.below(10) {
                    0..=2 => 2,  // MSG_NEW_GAME
                    3 => 27,     // MSG_CLEAR_GAME_DATA
                    4 => 1000,
                    5 => 1095,   // MSG_LOGIC_CRC
                    6 => 1999,
                    _ => 1001 + g.below(90),
                };
                let nargs = if t == 2 { g.below(5) } else if g.chance(15) { 250 + g.below(12) } else { g.below(5) };
                let d = if t == 2 { [0, 1, 2, 3, 4, 5, 6][g.below(7) as usize] } else { g.next() };
                [k, t, nargs, if t == 2 { 0 } else { g.next() }, d, g.next()]
            }
            7 | 8 => {
                played |= k == 7;
                [k, g.below(if closed { 4 } else { 3 }), 0, 0, 0, 0]
            }
            9 | 20 if !played => [12, 0, 0, 0, 0, 0],
            9 => [k, if g.chance(2) { g.below(4) } else { g.next() }, g.below(6).wrapping_sub(1), g.below(2), 0, 0],
            10 => [k, g.below(10).wrapping_sub(1), 0, 0, 0, 0],
            17 => [k, g.below(2), 0, 0, 0, 0],
            19 => [k, g.below(9), g.next(), 0, 0, 0],
            _ => [k, 0, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, gcfg, ops, files)
}

#[test]
fn recorder_matches_upstream() {
    let cases: usize = std::env::var("REC_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x5245_4331_0bad_f00d);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut by_op = [0usize; 21];
    let mut cut_why = BTreeMap::<String, usize>::new();
    let (mut playbacks_ok, mut recorded) = (0usize, 0usize);
    for case in 0..cases {
        let (cfg, gcfg, ops, files) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &gcfg, &ops, &files);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            *cut_why.entry(format!("{why} (op {})", ops[undefined_at.as_ref().map_or(0, |u| u.0)][0])).or_insert(0) += 1;
        }
        if let Ok(d) = std::env::var("REC_TRACE") {
            let _ = std::fs::write(&d, format!("case {case}: cfg {cfg:?} gcfg {gcfg:?}\nops {:?}\n", &ops[..n]));
        }
        let c = cpp_run(&cfg, &gcfg, &ops[..n], &files);
        // a run cut short has no final file list on the Rust side: compare up to it
        let c = if undefined_at.is_some() { c.split("\nfile ").next().unwrap_or("").to_string() + if c.contains("\nfile ") { "\n" } else { "" } } else { c };
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if let Ok(d) = std::env::var("REC_DUMP_DIR") {
                if mismatches == 1 {
                    let _ = std::fs::write(format!("{d}/r.txt"), &r);
                    let _ = std::fs::write(format!("{d}/c.txt"), &c);
                }
            }
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
            playbacks_ok += r.lines().filter(|l| l.starts_with("playback ") && l.ends_with(": 1")).count();
            recorded += r.lines().filter(|l| l.starts_with("file 00000000.rep: ") && !l.ends_with("none")).count();
        }
    }
    println!("R1: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines ({playbacks_ok} playbacks started, {recorded} recordings), {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
