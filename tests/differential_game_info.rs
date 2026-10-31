#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameInfo.h
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp (the map-derived paths)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/strtok_r.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameSlot`, `GameInfo`, the options string and `SkirmishGameInfo::xfer` (`common::game_info`) against the C++, layer 12
//! batch G1.
//!
//! # Method
//!
//! The C++ side is `GameInfo.cpp` and its header (the `.cpp` in build.rs's copy with one for-scope edit), FileTransfer.cpp's
//! map-derived paths cut verbatim, and Westwood's `strtok_r.cpp`, compiled by `differential/wrap_game_info.cpp` on the
//! stand-ins of `differential/shim_ginfo` (a map cache of two maps, a file system that answers by a hash of the path and
//! logs every open, game text that names its label, map paths that swap `data\` for `maps\`, multiplayer settings and
//! player templates from the configuration, a tick count stepping by 7, Windows' UTF-8 converters as the port reads them).
//! Money is the money build's real `Money.cpp`, whose income records are counted. It runs a program on a zero-filled
//! `SkirmishGameInfo` (with a second one as another `TheGameInfo`): parsing option strings (well-formed ones with random
//! fields, then mutated by deleting, inserting or replacing characters, cutting, or doubling a key) into the game or into
//! `NULL`, writing the string, the game's life cycle, slot states and whole slots, the queries, `setMap` and the other
//! setters, accepting, start spots, `adjustSlotsForMap`, closing and clearing slots, the apparent template, colour, start
//! and name with `TheGameInfo` `NULL`, this game or the other, and saving and loading (bytes as saved, or with one byte
//! replaced). Each operation writes a line, and the game is dumped field by field after a load and on request; the Rust
//! side writes the same lines.
//!
//! An operation the Rust side reports as undefined (a `NULL` dereferenced, `tolower` of a byte of 0x80 or more, a number
//! out of range for `atoi` or `sscanf`, an endless loop) ends the Rust run there, and the C++ runs only the operations
//! before it.
//!
//! # Results (2026-10-31, `GINFO_CASES=20000`, the default)
//!
//! 20,000 programs, 305,354 operations, 689,428 log lines (13,920 option strings parsed into a game): **0 mismatches**.
//! 9,515 programs cut at an undefined operation: 6,149 `isSandbox` with no local slot, 2,065 a parse that dereferences a
//! missing `TheMultiplayerSettings` or `TheMapCache`, 561 an apparent value asked with `TheGameInfo` `NULL` (or a template
//! name outside the store), 232 `adjustSlotsForMap` or `isSkirmish` with no map cache or no `TheGameInfo`, 170 `tolower`
//! of a byte of 0x80 or more, 159 `atoi`/`sscanf`/`strtoul` out of range, 111 a save of a string over 255 units
//! (`XFER_STRING_ERROR`, uncaught upstream), 68 a damaged save loading a byte other than 0 or 1 into a `Bool`.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 9 of 9 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3951 | `setPlayerTemplate` of an observer keeps the start position | 174 programs |
//! | 3952 | an AI slot's team may be `MAX_SLOTS/2` | 3 programs, then the C++ crashes on the diverged state |
//! | 3953 | `setMapCRC` stores the CRC with its low bit flipped | 6,013 programs |
//! | 3954 | the name's share of the options length divided by one slot more | 870 programs |
//! | 3955 | a load forgets a slot's accepted flag | 482 programs |
//! | 3956 | `grabHexInt` reads one hex digit, not two | 3,398 programs |
//! | 3957 | `isSlotLocalAlly`'s observer test reads the template, not the original | 3 programs |
//! | 3958 | `reset` takes the tick count but seeds 0 | 20,000 programs |
//! | 3959 | `closeOpenSlots` leaves open slots open | 619 programs |

use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::io::Write as _;

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
    cfg: [u32; 9],
    tick: u32,
    log: Vec<u8>,
    income: i32,
}

fn line(log: &mut Vec<u8>, s: &[u8]) {
    log.extend_from_slice(s);
    log.push(b'\n');
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
        if self.cfg[0] & 32 != 0 {
            m.deposit(10000, false, &mut Vec::new());
        }
        m
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        self.cfg[0] & 1 != 0
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<MapMeta> {
        match lower {
            b"data\\alpha\\alpha.map" => Some(MapMeta { num_players: self.cfg[4] as i32, crc: self.cfg[5] }),
            b"data\\beta\\beta.map" => Some(MapMeta { num_players: self.cfg[6] as i32, crc: self.cfg[7] }),
            _ => None,
        }
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        let mut l = b"open ".to_vec();
        l.extend_from_slice(path);
        line(&mut self.log, &l);
        let mut h = self.cfg[8];
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
        self.cfg[0] & 2 != 0
    }
    fn num_colors(&self) -> i32 {
        self.cfg[1] as i32
    }
    fn show_random_player_template(&self) -> bool {
        self.cfg[0] & 4 != 0
    }
    fn show_random_color(&self) -> bool {
        self.cfg[0] & 8 != 0
    }
    fn show_random_start_pos(&self) -> bool {
        self.cfg[0] & 16 != 0
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0x123456)
    }
    fn player_template_count(&self) -> i32 {
        self.cfg[2] as i32
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, IniError> {
        // ThePlayerTemplateStore->getNthPlayerTemplate(n) is NULL outside the store, and dereferenced
        if n < 0 || n >= self.cfg[2] as i32 {
            return Err(IniError::undefined(rts_lockstep_engine::common::ini::Undefined::NullObject));
        }
        Ok(format!("PT{n}").bytes().map(u16::from).collect())
    }
    fn record_income(&mut self, player_index: i32) {
        if (0..8).contains(&player_index) {
            self.income += 1;
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

fn dump(log: &mut Vec<u8>, what: &str, g: &GameInfo) {
    let mut head = format!(
        "{what}: pre {} crc {} in {} prog {} sur {} id {} ip {:X} map [",
        g.preorder_mask,
        g.crc_interval,
        i32::from(g.in_game),
        i32::from(g.in_progress),
        i32::from(g.surrendered),
        g.game_id,
        g.local_ip
    )
    .into_bytes();
    head.extend_from_slice(&g.map_name);
    head.extend_from_slice(
        format!(
            "] mc {:X} ms {} mm {} seed {} stats {} sr {} cash {} old {}",
            g.map_crc,
            g.map_size,
            g.map_mask,
            g.seed,
            g.use_stats,
            g.superweapon_restriction,
            g.starting_cash.count_money(),
            i32::from(g.old_factions_only)
        )
        .as_bytes(),
    );
    line(log, &head);
    for (i, s) in g.slots.iter().enumerate() {
        let l = format!(
            " slot {i}: st {} acc {} map {} mute {} col {} sp {} pt {} team {} ocol {} osp {} opt {} name [{}] ip {:X} port {} nat {} last {} dis {}",
            s.state,
            i32::from(s.is_accepted),
            i32::from(s.has_map),
            i32::from(s.is_muted),
            s.color,
            s.start_pos,
            s.player_template,
            s.team_number,
            s.orig_color,
            s.orig_start_pos,
            s.orig_player_template,
            wide(&s.name),
            s.ip,
            s.port,
            s.nat_behavior,
            s.last_frame_in_game,
            i32::from(s.disconnected)
        );
        line(log, l.as_bytes());
    }
}

struct Cut(String);

fn e<T>(r: Result<T, IniError>) -> Result<T, Cut> {
    r.map_err(|e| Cut(format!("{e:?}")))
}

/// `AsciiString(std::string(...).c_str())`: up to the first 0.
fn cstr(s: &[u8]) -> Vec<u8> {
    s.iter().copied().take_while(|&b| b != 0).collect()
}

fn translate(s: &[u8]) -> Vec<u16> {
    cstr(s).iter().map(|&b| b as i8 as i16 as u16).collect()
}

struct Sim<'a> {
    h: Host,
    game: GameInfo,
    other: GameInfo,
    the: u32,
    strs: &'a [Vec<u8>],
    saved: Vec<u8>,
}

impl Sim<'_> {
    fn str(&self, i: u32) -> Vec<u8> {
        if self.strs.is_empty() {
            Vec::new()
        } else {
            cstr(&self.strs[i as usize % self.strs.len()])
        }
    }

    fn the_info(&self, slot: usize) -> TheGameInfo<'_> {
        match self.the % 3 {
            0 => TheGameInfo::Null,
            1 => TheGameInfo::Is(&self.game, Some(slot)),
            _ => TheGameInfo::Is(&self.other, None),
        }
    }

    fn op(&mut self, w: &Op) -> Result<(), Cut> {
        let (a, b, c, d, ev) = (w[1], w[2], w[3], w[4], w[5]);
        match w[0] {
            0 => {
                let before = self.h.income;
                let s = self.str(a);
                let target = if b & 1 == 1 { None } else { Some(&mut self.game) };
                let r = e(parse_ascii_string_to_game_info(target, &s, &mut self.h))?;
                let l = format!("parse {} income {}", i32::from(r), self.h.income - before);
                line(&mut self.h.log, l.as_bytes());
            }
            1 => {
                let s = e(game_info_to_ascii_string(if b & 1 == 1 { None } else { Some(&self.game) }, &self.h))?;
                let mut l = b"string [".to_vec();
                l.extend_from_slice(&s);
                l.push(b']');
                line(&mut self.h.log, &l);
            }
            2 => {
                self.game.reset(&mut self.h);
                line(&mut self.h.log, b"reset");
            }
            3 => {
                self.game.enter_game(&mut self.h);
                line(&mut self.h.log, b"enter");
            }
            4 => {
                self.game.leave_game(&mut self.h);
                line(&mut self.h.log, b"leave");
            }
            5 => {
                self.game.start_game(a as i32, &mut self.h);
                line(&mut self.h.log, format!("start {}", a as i32).as_bytes());
            }
            6 => {
                self.game.end_game();
                line(&mut self.h.log, b"end");
            }
            7 => {
                let name = translate(&self.str(c));
                self.game.slots[(a % 8) as usize].set_state(b as i32, &name, d, &mut self.h);
                line(&mut self.h.log, format!("setstate {} {}", a % 8, b as i32).as_bytes());
            }
            8 => {
                let name = translate(&self.str(c));
                let mut s = GameSlot::new();
                s.set_state((b % 7) as i32, &name, d, &mut self.h);
                s.color = (ev & 0xff) as i32 - 2;
                s.set_player_template(((ev >> 8) & 0xff) as i32 - 3);
                s.start_pos = ((ev >> 16) & 0xf) as i32 - 2;
                s.team_number = ((ev >> 20) & 0xf) as i32 - 2;
                if ev & (1 << 24) != 0 {
                    s.un_accept();
                }
                if ev & (1 << 25) != 0 {
                    s.set_map_availability(false);
                }
                self.game.set_slot(a as i32, s);
                line(&mut self.h.log, format!("setslot {}", a as i32).as_bytes());
            }
            9 => {
                let g = &self.game;
                let max = e(g.max_players(&self.h))?;
                let l = format!(
                    "q players {} nonobs {} max {} local {} host {} multi {} pre {}{}{}",
                    g.num_players(),
                    g.num_non_observer_players(),
                    max,
                    g.local_slot_num(),
                    i32::from(g.am_i_host()),
                    i32::from(g.is_multi_player()),
                    i32::from(g.is_player_preorder(a as i32)),
                    i32::from(g.is_player_preorder(b as i32)),
                    i32::from(g.is_player_preorder(-1))
                );
                line(&mut self.h.log, l.as_bytes());
            }
            10 => {
                let n = self.game.slot_num(&self.str(a));
                line(&mut self.h.log, format!("slotnum {n}").as_bytes());
            }
            11 => {
                let the = self.the;
                let (game, other) = (&self.game, &self.other);
                let r = e(game.is_skirmish(|i| match the % 3 {
                    0 => TheGameInfo::Null,
                    1 => TheGameInfo::Is(game, Some(i)),
                    _ => TheGameInfo::Is(other, None),
                }))?;
                line(&mut self.h.log, format!("skirmish {}", i32::from(r)).as_bytes());
            }
            12 => {
                let r = e(self.game.is_sandbox())?;
                line(&mut self.h.log, format!("sandbox {}", i32::from(r)).as_bytes());
            }
            13 => {
                let s = self.str(a);
                e(self.game.set_map(&s, &mut self.h))?;
                line(&mut self.h.log, b"setmap");
            }
            14 => {
                e(self.game.set_map_crc(a, &self.h))?;
                line(&mut self.h.log, b"setmapcrc");
            }
            15 => {
                e(self.game.set_map_size(a, &self.h))?;
                line(&mut self.h.log, b"setmapsize");
            }
            16 => {
                self.game.set_crc_interval(a as i32);
                line(&mut self.h.log, b"setcrc");
            }
            17 => {
                self.game.local_ip = a;
                line(&mut self.h.log, b"setip");
            }
            18 => {
                self.game.mark_player_as_preorder(a as i32);
                line(&mut self.h.log, b"preorder");
            }
            19 => {
                let l = format!(
                    "taken {} {}",
                    i32::from(self.game.is_color_taken(a as i32, b as i32)),
                    i32::from(self.game.is_start_position_taken(c as i32, b as i32))
                );
                line(&mut self.h.log, l.as_bytes());
            }
            20 => {
                self.game.reset_accepted();
                line(&mut self.h.log, b"resetaccepted");
            }
            21 => {
                self.game.reset_start_spots();
                line(&mut self.h.log, b"resetspots");
            }
            22 => {
                e(self.game.adjust_slots_for_map(&mut self.h))?;
                line(&mut self.h.log, b"adjust");
            }
            23 => {
                self.game.close_open_slots(&mut self.h);
                line(&mut self.h.log, b"close");
            }
            24 => {
                self.game.clear_slot_list(&mut self.h);
                line(&mut self.h.log, b"clear");
            }
            25 => {
                let i = (a % 8) as usize;
                let the = self.the_info(i);
                let s = &self.game.slots[i];
                let pt = e(s.apparent_player_template(the, &self.h))?;
                let col = e(s.apparent_color(the, &self.h))?;
                let sp = e(s.apparent_start_pos(the, &self.h))?;
                let s = s.clone();
                let mut h2 = Host { cfg: self.h.cfg, tick: 0, log: Vec::new(), income: 0 };
                let name = e(s.apparent_player_template_display_name(self.the_info(i), &mut h2))?;
                let l = format!("apparent {i}: pt {pt} col {col} sp {sp} name [{}]", wide(&name));
                line(&mut self.h.log, l.as_bytes());
            }
            26 => {
                self.the = a % 3;
                line(&mut self.h.log, format!("the {}", a % 3).as_bytes());
            }
            27 => {
                let mut x = XferSave::new();
                x.open(b"ginfo").expect("open");
                // XferSave throws XFER_STRING_ERROR for a string over 255 units, which nothing catches upstream
                SkirmishXfer { game: &mut self.game, host: &mut self.h }.xfer(&mut x).map_err(|s| Cut(format!("save threw {s:?}")))?;
                self.saved = x.bytes().to_vec();
                let hex: String = self.saved.iter().map(|b| format!("{b:02x}")).collect();
                line(&mut self.h.log, format!("save {hex}").as_bytes());
            }
            28 => {
                let mut bytes = self.saved.clone();
                if b & 1 == 1 && !bytes.is_empty() {
                    let n = bytes.len();
                    bytes[c as usize % n] = d as u8;
                }
                let mut x = XferLoad::new(bytes);
                x.open(b"ginfo").expect("open");
                rts_lockstep_engine::common::xfer::take_invalid_bool_read();
                let r = SkirmishXfer { game: &mut self.game, host: &mut self.h }.xfer(&mut x);
                // a byte other than 0 or 1 loaded into a `Bool` is undefined upstream
                if rts_lockstep_engine::common::xfer::take_invalid_bool_read() {
                    return Err(Cut("InvalidBoolLoaded".into()));
                }
                line(&mut self.h.log, if r.is_ok() { b"load" } else { b"load threw" });
                let g = self.game.clone();
                dump(&mut self.h.log, "loaded", &g);
            }
            29 => {
                let g = self.game.clone();
                dump(&mut self.h.log, "game", &g);
            }
            30 => {
                let s = &mut self.game.slots[(a % 8) as usize];
                match b % 8 {
                    0 => s.color = c as i32,
                    1 => s.start_pos = c as i32,
                    2 => s.set_player_template(c as i32),
                    3 => s.team_number = c as i32,
                    4 => s.save_off_original_info(),
                    5 => s.un_accept(),
                    6 => s.set_map_availability(c & 1 != 0),
                    _ => s.set_accept(),
                }
                line(&mut self.h.log, format!("slotset {} {} {}", a % 8, b % 8, c as i32).as_bytes());
            }
            _ => line(&mut self.h.log, b"op?"),
        }
        Ok(())
    }
}

fn rust_run(cfg: &[u32; 9], ops: &[Op], strs: &[Vec<u8>]) -> (Vec<u8>, Option<(usize, String)>) {
    let mut h = Host { cfg: *cfg, tick: cfg[3], log: Vec::new(), income: 0 };
    let game = GameInfo::new(&mut h);
    let other = GameInfo::new(&mut h);
    let mut s = Sim { h, game, other, the: 1, strs, saved: Vec::new() };
    line(&mut s.h.log, b"made");
    let g = s.game.clone();
    dump(&mut s.h.log, "game", &g);
    for (i, w) in ops.iter().enumerate() {
        let before = s.h.log.len();
        if let Err(Cut(why)) = s.op(w) {
            s.h.log.truncate(before);
            return (s.h.log, Some((i, why)));
        }
    }
    (s.h.log, None)
}

fn cpp_run(cfg: &[u32; 9], ops: &[Op], strs: &[Vec<u8>]) -> Vec<u8> {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut packed = Vec::new();
    for s in strs {
        packed.extend_from_slice(&(s.len() as u16).to_le_bytes());
        packed.extend_from_slice(s);
    }
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe {
            cpp::ref_ginfo_run(cfg.as_ptr(), flat.as_ptr(), ops.len() as i32, packed.as_ptr().cast(), strs.len() as i32, out.as_mut_ptr().cast(), cap as i32)
        } as usize;
        if len <= cap {
            out.truncate(len);
            return out;
        }
        cap = len;
    }
}

const NAMES: [&[u8]; 8] = [b"Bob", b"alice", b"BOB", b"Zed\xc3\xa9", b"x\xff\xfey", b"a\nb\rc", b"", b"LongPlayerNameThatGoesOnAndOn"];

/// A slot that parses: values inside every bound the parser checks.
fn gen_clean_slot(g: &mut Gen) -> Vec<u8> {
    match g.below(6) {
        0 | 1 => format!(
            "H{},{:X},{},{},{},{},{},{},{}",
            ["Bob", "alice", "Zed", "Q", "NameOfSeventyCharactersXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"][g.below(5) as usize],
            if g.chance(2) { g.below(4) } else { g.next() },
            g.below(65536),
            ["TT", "TF", "FT", "FF"][g.below(4) as usize],
            g.below(9) as i32 - 1,
            g.below(14) as i32 - 2,
            g.below(9) as i32 - 1,
            g.below(5) as i32 - 1,
            g.below(129)
        )
        .into_bytes(),
        2 | 3 => format!(
            "C{},{},{},{},{}",
            ["E", "M", "H"][g.below(3) as usize],
            g.below(9) as i32 - 1,
            g.below(14) as i32 - 2,
            g.below(9) as i32 - 1,
            if g.chance(8) { 4 } else { g.below(5) as i32 - 1 }
        )
        .into_bytes(),
        4 => b"O".to_vec(),
        _ => b"X".to_vec(),
    }
}

fn gen_clean_options(g: &mut Gen) -> Vec<u8> {
    let map = ["maps/alpha", "maps/beta", "maps/gamma", "maps/Alpha"][g.below(4) as usize];
    let mut s = format!(
        "US={};M={:02x}{map};MC={:X};MS={};SD={};C={};SR={};SC={};O={};S=",
        g.below(2),
        g.below(128),
        if g.chance(2) { g.next() } else { g.below(3) },
        g.below(100000),
        g.next() as i32 / 4,
        g.below(200),
        g.below(65536),
        [0, 5000, 10000][g.below(3) as usize],
        ["Y", "N"][g.below(2) as usize]
    )
    .into_bytes();
    for _ in 0..8 {
        s.extend_from_slice(&gen_clean_slot(g));
        s.push(b':');
    }
    s.extend_from_slice(b";");
    s
}

fn gen_slot(g: &mut Gen) -> Vec<u8> {
    match g.below(8) {
        0..=2 => {
            let name = NAMES[g.below(8) as usize];
            let mut s = b"H".to_vec();
            s.extend_from_slice(name);
            let fields = [
                format!("{:X}", if g.chance(4) { g.next() } else { g.below(4) }),
                format!("{}", g.below(70000) as i32 - 5),
                [&b"TT"[..], b"TF", b"FT", b"FF", b"T", b"XYZ"][g.below(6) as usize].iter().map(|&b| b as char).collect(),
                format!("{}", g.below(12) as i32 - 2),
                format!("{}", g.below(16) as i32 - 3),
                format!("{}", g.below(11) as i32 - 2),
                format!("{}", g.below(7) as i32 - 2),
                format!("{}", g.below(140) as i32 - 5),
            ];
            let n = if g.chance(10) { g.below(8) as usize } else { 8 };
            for f in &fields[..n] {
                s.push(b',');
                s.extend_from_slice(f.as_bytes());
            }
            s
        }
        3..=4 => {
            let mut s = format!("C{}", ["E", "M", "H", "Q", ""][g.below(5) as usize]).into_bytes();
            let fields = [
                format!("{}", g.below(12) as i32 - 2),
                format!("{}", g.below(16) as i32 - 3),
                format!("{}", g.below(11) as i32 - 2),
                format!("{}", g.below(7) as i32 - 2),
            ];
            let n = if g.chance(10) { g.below(4) as usize } else { 4 };
            for f in &fields[..n] {
                s.push(b',');
                s.extend_from_slice(f.as_bytes());
            }
            s
        }
        5 => b"O".to_vec(),
        6 => b"X".to_vec(),
        _ => [&b""[..], b"Z", b"o"][g.below(3) as usize].to_vec(),
    }
}

fn gen_options(g: &mut Gen) -> Vec<u8> {
    if g.chance(2) {
        return gen_clean_options(g);
    }
    let map = [&b"maps/alpha"[..], b"maps/beta", b"userdata/maps/gamma", b"alpha", b"maps/Alpha", b"maps\\beta"][g.below(6) as usize];
    let mut keys: Vec<Vec<u8>> = vec![
        format!("US={}", g.below(3)).into_bytes(),
        {
            let mut m = format!("M={:02x}", g.below(128)).into_bytes();
            m.extend_from_slice(map);
            m
        },
        format!("MC={:X}", if g.chance(2) { g.next() } else { g.below(3) }).into_bytes(),
        format!("MS={}", g.below(100000)).into_bytes(),
        format!("SD={}", g.next() as i32 / 4).into_bytes(),
        format!("C={}", g.below(200) as i32 - 50).into_bytes(),
        format!("SR={}", g.below(70000)).into_bytes(),
        format!("SC={}", [0, 5000, 10000, g.next()][g.below(4) as usize]).into_bytes(),
        format!("O={}", ["Y", "N", "y", "YES"][g.below(4) as usize]).into_bytes(),
    ];
    let mut slots = b"S=".to_vec();
    for i in 0..8 {
        if i > 0 || g.chance(1) {
            slots.extend_from_slice(&gen_slot(g));
            slots.push(b':');
        }
    }
    keys.push(slots);
    if g.chance(3) {
        let i = g.below(keys.len() as u32) as usize;
        let j = g.below(keys.len() as u32) as usize;
        keys.swap(i, j);
    }
    if g.chance(8) {
        let i = g.below(keys.len() as u32) as usize;
        let k = keys[i].clone();
        keys.push(k);
    }
    if g.chance(10) {
        let i = g.below(keys.len() as u32) as usize;
        keys.remove(i);
    }
    let mut s = Vec::new();
    for k in keys {
        s.extend_from_slice(&k);
        s.push(b';');
    }
    // mutations
    let muts = if g.chance(2) { 0 } else { 1 + g.below(3) };
    for _ in 0..muts {
        if s.is_empty() {
            break;
        }
        let at = g.below(s.len() as u32) as usize;
        match g.below(5) {
            0 => {
                s.remove(at);
            }
            1 => s.insert(at, b";=,:\nHCOX0123456789-xT"[g.below(22) as usize]),
            2 => s[at] = b";=,:\nHCOX0123456789-xTF"[g.below(23) as usize],
            3 => s.truncate(at),
            _ => s.insert(at, b' '),
        }
    }
    s
}

fn gen_program(g: &mut Gen) -> ([u32; 9], Vec<Op>, Vec<Vec<u8>>) {
    let cfg = [
        if g.chance(6) { g.below(64) } else { 1 | 2 | (g.below(64) & !3) },
        if g.chance(5) { g.below(4) } else { 8 },
        if g.chance(5) { g.below(5) } else { 12 },
        g.next(),
        g.below(9),
        if g.chance(2) { 1 } else { g.next() },
        g.below(9),
        g.below(3),
        g.next(),
    ];
    let mut strs: Vec<Vec<u8>> = (0..4).map(|_| gen_options(g)).collect();
    for n in NAMES {
        strs.push(n.to_vec());
    }
    for m in [&b"data\\alpha\\alpha.map"[..], b"data\\beta\\beta.map", b"DATA\\Alpha\\ALPHA.MAP", b"nowhere.map", b"a\\b", b"x"] {
        strs.push(m.to_vec());
    }
    let ns = strs.len() as u32;
    let n = 4 + g.below(40);
    let mut ops = Vec::new();
    for _ in 0..n {
        let k = match g.below(100) {
            0..=19 => 0,
            20..=27 => 1,
            28..=30 => 2,
            31..=34 => 3,
            35 => 4,
            36..=37 => 5,
            38 => 6,
            39..=42 => 7,
            43..=47 => 8,
            48..=52 => 9,
            53..=54 => 10,
            55..=57 => 11,
            58..=59 => 12,
            60..=62 => 13,
            63..=64 => 14,
            65 => 15,
            66 => 16,
            67..=70 => 17,
            71 => 18,
            72 => 19,
            73 => 20,
            74 => 21,
            75..=76 => 22,
            77 => 23,
            78 => 24,
            79..=82 => 25,
            83..=84 => 26,
            85..=88 => 27,
            89..=92 => 28,
            93..=94 => 29,
            _ => 30,
        };
        let ip = if g.chance(2) { g.below(4) } else { g.next() };
        let op: Op = match k {
            0 => [k, g.below(4), u32::from(g.chance(6)), 0, 0, 0],
            1 => [k, 0, u32::from(g.chance(8)), 0, 0, 0],
            5 => [k, g.next() % 100, 0, 0, 0, 0],
            7 => [k, g.below(8), g.below(7), 4 + g.below(8), ip, 0],
            8 => [k, if g.chance(10) { g.next() % 12 } else { g.below(8) }, g.below(7), 4 + g.below(8), ip, g.next()],
            9 => [k, g.below(10).wrapping_sub(1), g.below(10), 0, 0, 0],
            10 => [k, 4 + g.below(8), 0, 0, 0, 0],
            13 => [k, if g.chance(3) { g.below(4) } else { 12 + g.below(6) }, 0, 0, 0, 0],
            14 | 15 => [k, if g.chance(2) { g.below(3) } else { g.next() }, 0, 0, 0, 0],
            16 => [k, g.next() % 300, 0, 0, 0, 0],
            17 => [k, ip, 0, 0, 0, 0],
            18 => [k, g.below(11).wrapping_sub(1), 0, 0, 0, 0],
            19 => [k, g.below(12).wrapping_sub(2), g.below(10).wrapping_sub(1), g.below(12).wrapping_sub(2), 0, 0],
            25 => [k, g.below(8), 0, 0, 0, 0],
            26 => [k, if g.chance(4) { 0 } else { 1 + g.below(2) }, 0, 0, 0, 0],
            28 => [k, 0, u32::from(g.chance(3)), g.next(), g.below(256), 0],
            30 => [k, g.below(8), g.below(8), g.below(14).wrapping_sub(4), 0, 0],
            _ => [k, 0, 0, 0, 0, 0],
        };
        ops.push(op);
    }
    let _ = ns;
    (cfg, ops, strs)
}

#[test]
fn lan_options_length_is_upstreams() {
    assert_eq!(LAN_MAX_OPTIONS_LENGTH, 400);
}

#[test]
fn game_info_matches_upstream() {
    let cases: usize = std::env::var("GINFO_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x4749_0001_dead_beef);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut by_op = [0usize; 31];
    let mut cut_why = std::collections::BTreeMap::<String, usize>::new();
    let mut parses_ok = 0usize;
    for case in 0..cases {
        let (cfg, ops, strs) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &ops, &strs);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            *cut_why.entry(format!("{why} (op {})", ops[undefined_at.as_ref().map_or(0, |u| u.0)][0])).or_insert(0) += 1;
        }
        if let Ok(d) = std::env::var("GINFO_TRACE") {
            let _ = std::fs::write(&d, format!("case {case}: cfg {cfg:?}\nops {:?}\nstrs {:?}\n", &ops[..n], strs.iter().map(|s| s.escape_ascii().to_string()).collect::<Vec<_>>()));
        }
        let c = cpp_run(&cfg, &ops[..n], &strs);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if let Ok(d) = std::env::var("GINFO_DUMP_DIR") {
                if mismatches == 1 {
                    let _ = std::fs::write(format!("{d}/r.txt"), &r);
                    let _ = std::fs::write(format!("{d}/c.txt"), &c);
                }
            }
            if mismatches <= 3 {
                let (rl, cl): (Vec<&[u8]>, Vec<&[u8]>) = (r.split(|&b| b == b'\n').collect(), c.split(|&b| b == b'\n').collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                let mut out = std::io::stdout().lock();
                let _ = writeln!(
                    out,
                    "case {case}: cfg {cfg:?} ops {:?}\n  strs {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}",
                    &ops[..n],
                    strs.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect::<Vec<_>>(),
                    rl.get(at).map(|l| String::from_utf8_lossy(l).into_owned()),
                    cl.get(at).map(|l| String::from_utf8_lossy(l).into_owned())
                );
            }
        } else {
            lines += r.split(|&b| b == b'\n').count();
            parses_ok += r.split(|&b| b == b'\n').filter(|l| l.starts_with(b"parse 1")).count();
        }
    }
    println!("G1: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines ({parses_ok} successful parses), {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
