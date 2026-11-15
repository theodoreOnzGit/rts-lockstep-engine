#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/Radar.cpp (RadarObject::xfer, xferRadarObjectList, Radar::xfer)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #57: the radar's save, `CHUNK_Radar`, against the upstream text compiled verbatim
//! (`differential/wrap_radar_xfer.cpp`, which lists the cuts, the stand-ins and the words), on upstream's real
//! `XferSave` and `XferLoad`. The Rust is [`radar_xfer`] (`client::save_game`), which the game's save reaches through
//! `SaveHost` with the World as its [`RadarXferObjects`]; here a stand-in host that mirrors the C++ one.
//!
//! # Methodology
//!
//! - **Saves:** 20,000 random radars: 0 to 7 objects (distinct ids of 1 to 60, a few of up to 2^31 - 1; one in four
//!   destroyed), the two flags, two object lists of 0 to 6 entries (one list in twenty of up to 40), each entry an
//!   object of the case or none (`m_object == NULL`, one in thirty; the Rust's entry keeps the id of an object the game no longer has), with a random colour; the 64 events with random
//!   types, flags, frames, colours, world positions (`Gen::mixed`, NaNs and infinities included, compared as bits) and
//!   radar positions; random indices. Compared: the status and the file's bytes.
//! - **Loads:** each saved file is loaded again into another random radar (its lists, which the load's "should be
//!   empty" check reads, empty two times in three, otherwise of objects of the case), the bytes first left whole
//!   (three in eight), truncated (two in eight), given a bad version byte (one in eight), a random byte changed that is
//!   not a `Bool` (one in eight, a third of them in the event count; a `Bool` other than 0 or 1 is undefined in C++), or an entry's object id changed to
//!   one of 1 to 60 (one in eight); and the case's objects lose one of theirs one time in five. Compared: the status
//!   and the whole radar after the load (both lists with each entry's object id, 0 for none, and colour; the events;
//!   the indices) and the order of the `friend_setRadarData` calls.
//!
//! Status words: 0, an `XferStatus` + 1, or 100 + a `SaveCode` (`SC_INVALID_DATA` is 106).
//!
//! # Results
//!
//! See `tests/differential_radar_xfer.rs`'s entry in `docs/phase2-client.md` (GitHub #57, 2026-11-14).
//!
//! # Planted defects (each must be caught; `L9_PLANT=<n>`)
//!
//! | plant | defect |
//! |---|---|
//! | 10296 | the forced flag is xfered before the hidden flag |
//! | 10297 | a wrong event count is accepted |
//! | 10298 | an event's `soundPlayed` is not xfered |
//! | 10299 | a load into a list holding a live object goes on |
//! | 10300 | an entry whose object is gone saves its old id, not `INVALID_ID` |
//! | 10301 | a load of an unknown object id goes on |
//! | 10302 | the last event index is not xfered |

mod support;

use rts_lockstep_engine::client::radar::{Radar, RadarObject, MAX_RADAR_EVENTS};
use rts_lockstep_engine::client::save_game::{radar_xfer, RadarXferObjects};
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, RGBAColorInt};
use rts_lockstep_engine::common::game_state::SaveError;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

fn cases() -> u64 {
    std::env::var("RADAR_XFER_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

/// One radar and the objects of its game.
#[derive(Clone, Debug)]
struct Case {
    /// (id, destroyed).
    objs: Vec<(ObjectId, bool)>,
    hidden: bool,
    forced: bool,
    /// (object index or -1, colour).
    lists: [Vec<(i32, i32)>; 2],
    events: Vec<[i32; 19]>,
    next: i32,
    last: i32,
}

impl Case {
    fn random(g: &mut Gen, objs: Vec<(ObjectId, bool)>, null_entries: bool, empty_lists: bool) -> Case {
        let n = objs.len() as u32;
        let list = |g: &mut Gen| -> Vec<(i32, i32)> {
            if empty_lists || n == 0 && !null_entries {
                return Vec::new();
            }
            let len = if g.below(20) == 0 { g.below(41) } else { g.below(7) };
            (0..len)
                .map(|_| {
                    let k = if n == 0 || null_entries && g.below(30) == 0 { -1 } else { g.below(n) as i32 };
                    (k, g.i32())
                })
                .collect()
        };
        let lists = [list(g), list(g)];
        let events = (0..MAX_RADAR_EVENTS)
            .map(|_| {
                let mut w = [0i32; 19];
                w[0] = if g.below(4) == 0 { g.i32() } else { g.below(9) as i32 };
                w[1] = g.below(2) as i32;
                for v in w.iter_mut().take(13).skip(2) {
                    *v = if g.below(2) == 0 { g.below(3000) as i32 } else { g.i32() };
                }
                for v in w.iter_mut().take(16).skip(13) {
                    *v = g.mixed().to_bits() as i32;
                }
                w[16] = g.i32() % 200;
                w[17] = g.i32();
                w[18] = g.below(2) as i32;
                w
            })
            .collect();
        Case { objs, hidden: g.below(2) == 0, forced: g.below(2) == 0, lists, events, next: g.i32() % 70, last: g.i32() % 70 }
    }

    fn words(&self) -> Vec<i32> {
        let mut w = vec![self.objs.len() as i32];
        for &(id, d) in &self.objs {
            w.push(id);
            w.push(d as i32);
        }
        w.push(self.hidden as i32);
        w.push(self.forced as i32);
        for l in &self.lists {
            w.push(l.len() as i32);
            for &(k, c) in l {
                w.push(k);
                w.push(c);
            }
        }
        for e in &self.events {
            w.extend_from_slice(e);
        }
        w.push(self.next);
        w.push(self.last);
        w
    }

    fn radar(&self) -> Radar {
        let mut r = Radar::new();
        r.radar_hidden = self.hidden;
        r.radar_force_on = self.forced;
        // (an entry with no object, `m_object == NULL` in the C++, keeps the id of an object the game no longer has)
        let entry = |&(k, c): &(i32, i32)| RadarObject { object: if k < 0 { 61 + c.rem_euclid(1000) } else { self.objs[k as usize].0 }, color: c };
        r.local_object_list = self.lists[0].iter().map(entry).collect();
        r.object_list = self.lists[1].iter().map(entry).collect();
        for (e, w) in r.event.iter_mut().zip(&self.events) {
            e.event_type = w[0];
            e.active = w[1] != 0;
            e.create_frame = w[2] as u32;
            e.die_frame = w[3] as u32;
            e.fade_frame = w[4] as u32;
            e.color1 = RGBAColorInt { red: w[5] as u32, green: w[6] as u32, blue: w[7] as u32, alpha: w[8] as u32 };
            e.color2 = RGBAColorInt { red: w[9] as u32, green: w[10] as u32, blue: w[11] as u32, alpha: w[12] as u32 };
            e.world_loc = Coord3D { x: f32::from_bits(w[13] as u32), y: f32::from_bits(w[14] as u32), z: f32::from_bits(w[15] as u32) };
            e.radar_loc = ICoord2D { x: w[16], y: w[17] };
            e.sound_played = w[18] != 0;
        }
        r.next_free_radar_event = self.next;
        r.last_radar_event = self.last;
        r
    }
}

/// The C++ stand-ins' objects: `findObjectByID` over the case's, `friend_setRadarData` recorded.
struct Objs {
    objs: Vec<(ObjectId, bool)>,
    set: Vec<i32>,
}

impl RadarXferObjects for Objs {
    fn radar_object_exists(&self, id: ObjectId) -> bool {
        self.objs.iter().any(|o| o.0 == id)
    }
    fn radar_object_is_live(&self, id: ObjectId) -> bool {
        self.objs.iter().any(|o| o.0 == id && !o.1)
    }
    fn radar_object_find_and_set_radar_data(&mut self, id: ObjectId) -> bool {
        if self.radar_object_exists(id) {
            self.set.push(id);
            true
        } else {
            false
        }
    }
}

fn status(r: Result<(), SaveError>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(SaveError::Xfer(s)) => s as i32 + 1,
        Err(SaveError::InvalidData) => 106,
        Err(SaveError::Undefined(_)) => 999,
    }
}

/// The wrapper's words of a radar after an xfer.
fn out_words(st: i32, r: &Radar, objs: &Objs) -> Vec<i32> {
    let mut o = vec![st, r.radar_hidden as i32, r.radar_force_on as i32];
    for l in [&r.local_object_list, &r.object_list] {
        o.push(l.len() as i32);
        for e in l {
            o.push(if objs.radar_object_exists(e.object) { e.object } else { 0 });
            o.push(e.color);
        }
    }
    for e in &r.event {
        o.extend_from_slice(&[
            e.event_type,
            e.active as i32,
            e.create_frame as i32,
            e.die_frame as i32,
            e.fade_frame as i32,
            e.color1.red as i32,
            e.color1.green as i32,
            e.color1.blue as i32,
            e.color1.alpha as i32,
            e.color2.red as i32,
            e.color2.green as i32,
            e.color2.blue as i32,
            e.color2.alpha as i32,
            e.world_loc.x.to_bits() as i32,
            e.world_loc.y.to_bits() as i32,
            e.world_loc.z.to_bits() as i32,
            e.radar_loc.x,
            e.radar_loc.y,
            e.sound_played as i32,
        ]);
    }
    o.push(r.next_free_radar_event);
    o.push(r.last_radar_event);
    o.push(objs.set.len() as i32);
    o.extend_from_slice(&objs.set);
    o
}

fn xfer_path(kind: &str) -> String {
    format!("{}/radar_xfer_{kind}_{}.bin", std::env::temp_dir().display(), std::process::id())
}

fn run_cpp(mode: i32, c: &Case, path: &str) -> Vec<i32> {
    let cpath = std::ffi::CString::new(path).expect("path");
    let words = c.words();
    let mut out = vec![0i32; 8192];
    let n = unsafe { cpp::ref_radar_xfer(mode, words.as_ptr(), words.len() as i32, cpath.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 1 && (n as usize) <= out.len());
    out.truncate(n as usize);
    out
}

/// The save of a case: (status, bytes) by the Rust, then by the C++.
fn save_both(c: &Case) -> ((i32, Vec<u8>), (i32, Vec<u8>)) {
    let mut r = c.radar();
    let mut objs = Objs { objs: c.objs.clone(), set: Vec::new() };
    let mut x = rts_lockstep_engine::common::xfer_save::XferSave::new();
    let st = match x.open(b"radar") {
        Ok(()) => status(radar_xfer(&mut x, &mut r, &mut objs)),
        Err(e) => e as i32 + 1,
    };
    let rust = (st, x.bytes().to_vec());
    let path = xfer_path("save");
    let out = run_cpp(0, c, &path);
    let bytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    (rust, (out[0], bytes))
}

/// The load of `bytes` into the radar of `c`: the wrapper's words by the Rust, then by the C++.
fn load_both(c: &Case, bytes: &[u8]) -> (Vec<i32>, Vec<i32>) {
    let mut r = c.radar();
    let mut objs = Objs { objs: c.objs.clone(), set: Vec::new() };
    let mut x = rts_lockstep_engine::common::xfer_load::XferLoad::new(bytes.to_vec());
    let st = match x.open(b"radar") {
        Ok(()) => status(radar_xfer(&mut x, &mut r, &mut objs)),
        Err(e) => e as i32 + 1,
    };
    let rust = out_words(st, &r, &objs);
    let path = xfer_path("load");
    std::fs::write(&path, bytes).expect("write the save");
    let cc = run_cpp(1, c, &path);
    let _ = std::fs::remove_file(&path);
    (rust, cc)
}

/// The byte offsets of the `Bool`s in a save of `c` (a load of another byte into a C++ `bool` is undefined).
fn bools(c: &Case) -> Vec<usize> {
    let mut b = vec![1, 2];
    let e0 = 3 + (3 + 9 * c.lists[0].len()) + (3 + 9 * c.lists[1].len()) + 2;
    for i in 0..MAX_RADAR_EVENTS {
        b.push(e0 + 70 * i + 4);
        b.push(e0 + 70 * i + 69);
    }
    b
}

/// The byte offsets of the object ids of the entries in a save of `c`.
fn ids(c: &Case) -> Vec<usize> {
    let mut v = Vec::new();
    let l0 = 3 + 3;
    for i in 0..c.lists[0].len() {
        v.push(l0 + 9 * i + 1);
    }
    let l1 = 3 + 3 + 9 * c.lists[0].len() + 3;
    for i in 0..c.lists[1].len() {
        v.push(l1 + 9 * i + 1);
    }
    v
}

fn random_objs(g: &mut Gen) -> Vec<(ObjectId, bool)> {
    let n = g.below(8);
    let mut v: Vec<(ObjectId, bool)> = Vec::new();
    while v.len() < n as usize {
        let id = if g.below(20) == 0 { (g.u32() & 0x7fff_ffff).max(1) as i32 } else { 1 + g.below(60) as i32 };
        if !v.iter().any(|o| o.0 == id) {
            v.push((id, g.below(4) == 0));
        }
    }
    v
}

#[test]
fn the_radar_save_matches_upstream() {
    let mut c = Check::new("Radar::xfer (CHUNK_Radar)");
    let mut g = Gen::new(0x57_0001);
    let (mut saved, mut refused, mut loads_ok, mut loads_failed) = (0u64, 0u64, 0u64, 0u64);
    let mut failures = std::collections::BTreeMap::new();
    let mut round_trips = 0u64;
    for k in 0..cases() {
        let objs = random_objs(&mut g);
        let case = Case::random(&mut g, objs.clone(), true, false);
        let (rust, cc) = save_both(&case);
        if rust.0 == 0 {
            saved += 1;
        } else {
            refused += 1;
        }
        c.eq(rust.clone(), cc, || format!("save {k}: {case:?}"));
        if rust.0 != 0 {
            continue;
        }
        // (the Rust's own round trip: a save loaded into an empty radar and saved again gives the same bytes, when the
        // save names no missing object)
        if case.lists.iter().flatten().all(|e| e.0 >= 0) {
            let mut r = Case { lists: [Vec::new(), Vec::new()], ..case.clone() }.radar();
            let mut o = Objs { objs: case.objs.clone(), set: Vec::new() };
            let mut x = rts_lockstep_engine::common::xfer_load::XferLoad::new(rust.1.clone());
            x.open(b"radar").expect("open");
            assert_eq!(status(radar_xfer(&mut x, &mut r, &mut o)), 0, "round trip {k}: the load");
            let mut s = rts_lockstep_engine::common::xfer_save::XferSave::new();
            s.open(b"radar").expect("open");
            assert_eq!(status(radar_xfer(&mut s, &mut r, &mut o)), 0, "round trip {k}: the save");
            assert_eq!(s.bytes(), &rust.1[..], "round trip {k}: the bytes");
            round_trips += 1;
        }
        let mut bytes = rust.1;
        match g.below(8) {
            0..=2 => {}
            3 | 4 => {
                let n = g.below(bytes.len() as u32) as usize;
                bytes.truncate(n);
            }
            5 => {
                let at = [0usize, 3, 3 + 3 + 9 * case.lists[0].len()][g.below(3) as usize];
                bytes[at] = if g.below(2) == 0 { 0 } else { 2 };
            }
            6 => {
                let b = bools(&case);
                let e0 = 3 + (3 + 9 * case.lists[0].len()) + (3 + 9 * case.lists[1].len());
                let at = if g.below(3) == 0 { e0 + g.below(2) as usize } else { g.below(bytes.len() as u32) as usize };
                if !b.contains(&at) {
                    bytes[at] = g.below(256) as u8;
                }
            }
            _ => {
                let v = ids(&case);
                if !v.is_empty() {
                    let at = v[g.below(v.len() as u32) as usize];
                    bytes[at..at + 4].copy_from_slice(&(1 + g.below(60) as i32).to_le_bytes());
                }
            }
        }
        let mut lobjs = objs;
        if !lobjs.is_empty() && g.below(5) == 0 {
            let i = g.below(lobjs.len() as u32) as usize;
            lobjs.remove(i);
        }
        let empty = g.below(3) != 0;
        let start = Case::random(&mut g, lobjs, false, empty);
        let (rl, cl) = load_both(&start, &bytes);
        if rl[0] == 0 {
            loads_ok += 1;
        } else {
            loads_failed += 1;
            *failures.entry(cl[0]).or_insert(0u64) += 1;
        }
        let first = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
        let around = |v: &Vec<i32>| v[first.saturating_sub(2)..(first + 6).min(v.len())].to_vec();
        let what = format!("load {k}: first difference at word {first} of {}/{}: rust {:?} c++ {:?}; lists {:?}, objects {:?}", rl.len(), cl.len(), around(&rl), around(&cl), start.lists, start.objs);
        c.eq(rl, cl, || what);
    }
    println!(
        "Radar::xfer: {} radars, {saved} saved ({refused} refused), {round_trips} Rust round trips; loads {loads_ok} whole, {loads_failed} failing (C++ status: count {failures:?})",
        cases()
    );
    finish(c);
}
