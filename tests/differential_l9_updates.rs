// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of four layer-9 update modules of p1l9-updates against upstream's own text (commit 0a05454d),
//! each compiled unchanged: `LifetimeUpdate.cpp` and `DeletionUpdate.cpp` (differential/wrap_l9updates.cpp), the same
//! file's `BaseRenerateUpdate.cpp` (BaseRegenerateUpdate) and `SmartBombTargetHomingUpdate.cpp`. See build.rs, the
//! `p1l9-updates` block. Not translated here: `FireWeaponUpdate` (its `Weapon` INI field needs the weapon store, not yet
//! wired) and `NeutronBlastBehavior` (needs the AI, the client drawable and the neutral player; see the batch report).
//!
//! # What is compared against what
//!
//! The C++ module is built from the **INI text of the run** (the `D` block, read by the real INI through the class's
//! `friend_newModuleData`), then driven by a program of ops. The Rust module is built from the same text by the crate's
//! INI reader, with a host ([`UpdateHost`] and, for the regen and homing modules, [`BaseRegenHost`] or
//! [`SmartBombHost`]) that answers the calls upstream makes on `TheGameLogic`, its object, the global data and the body.
//! The C++ side answers them with the wrapper's stand-ins; the wake calls go to the recorder of the module harness's
//! `GameLogic` (wrap_module.cpp). Both sides draw from their own game-logic generator, seeded alike.
//!
//! # Method
//!
//! A **program** is an INI text (fields with random values in milliseconds, some missing, some refused on purpose),
//! a seed of the game-logic generator, a frame, and 1 to 40 ops of six words. Ops per module: `update()`,
//! `setLifetimeRange` (frames), `onDamage` with a damage type, set the frame, set the status bits, set the body's
//! health, `SetTargetPosition`, set the object's position and terrain answer, an xfer save, an xfer load of the last
//! save, and an XferCRC. **After every op** both sides write the same state: the module's die frame or wake calls,
//! the kill and destroy counts, the last sleep time, the random generator's six words, and the module's own words
//! (heal count and amounts, health bits, target and position bits). A program whose INI fails to parse must fail on
//! both sides (compared as a status). Every word of every op of every program is compared bit for bit.
//!
//! Inputs upstream leaves undefined are never sent to the C++: a module built without module data (the data is
//! dereferenced); the Rust side checks that case alone in `null_module_data_is_an_error`.
//!
//! # Results
//!
//! Run 2026-10-20 (`cargo test --release --features differential --test differential_l9_updates -- --nocapture`, default
//! `L9U_CASES` = 1,500 programs per module). Zero mismatches in every module:
//!
//! | module | programs | ops | refused INI | words compared | differ |
//! |---|---|---|---|---|---|
//! | LifetimeUpdate | 1,500 | 30,918 | 11 | 491,278 | 0 |
//! | DeletionUpdate | 1,500 | 31,458 | 12 | 500,729 | 0 |
//! | BaseRegenerateUpdate | 1,500 | 29,064 | 78 | 431,034 | 0 |
//! | SmartBombTargetHomingUpdate | 1,500 | 27,991 | 136 | 526,232 | 0 |
//!
//! # Planted bugs
//!
//! Each row is one deliberate edit of the Rust (one at a time, reverted after; the files were restored and re-checked),
//! and the number of **mismatching words** the comparison reports for it (every plant is caught):
//!
//! | plant | module | mismatching words |
//! |---|---|---|
//! | L1 delay floor removed (`if delay < 1` never clamps) | LifetimeUpdate | 914 |
//! | L2 hulk override ignored | LifetimeUpdate | 3,014 |
//! | L3 `setLifetimeRange` uses the maximum as the minimum | LifetimeUpdate | 57,348 |
//! | L4 `update()` returns `UPDATE_SLEEP_NONE` instead of forever | LifetimeUpdate | 27,716 |
//! | L5 die frame one frame late | LifetimeUpdate | 34,025 |
//! | L6 `MaxLifetime` parsed one frame longer | LifetimeUpdate | 6,572 |
//! | D1 delay floor removed | DeletionUpdate | 1,148 |
//! | D2 `update()` kills instead of destroys | DeletionUpdate | 56,524 |
//! | D3 die frame one frame early | DeletionUpdate | 34,659 |
//! | D4 `MaxLifetime` parsed one frame longer | DeletionUpdate | 8,423 |
//! | R1 heal amount doubled | BaseRegenerateUpdate | 14,592 |
//! | R2 constructor's wake forever and none swapped | BaseRegenerateUpdate | 4,920 |
//! | R3 healing counted as damage | BaseRegenerateUpdate | 19,426 |
//! | R4 a sold object sleeps none, not forever | BaseRegenerateUpdate | 3,965 |
//! | R5 heal period one frame longer | BaseRegenerateUpdate | 6,383 |
//! | S1 course-correction scalar not clamped below zero | SmartBombTargetHomingUpdate | 1,443 |
//! | S2 height taken from the target | SmartBombTargetHomingUpdate | 4,946 |
//! | S3 terrain test dropped | SmartBombTargetHomingUpdate | 11,062 |
//! | S4 refused target (zero or NaN) accepted | SmartBombTargetHomingUpdate | 22,590 |
//! | S5 blend swapped in `x` | SmartBombTargetHomingUpdate | 4,143 |
//!
//! A first plant, `if delay < 2` for `if delay < 1` (both modules), **survived**: it is an equivalent mutant (both
//! clamp a draw of 0 to 1). It was replaced by the removal of the clamp, which is caught (L1, D1 above).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::module::ObjectId;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::base_regenerate_update::{BaseRegenHost, BaseRegenerateUpdate, BaseRegenerateUpdateModuleData};
use rts_lockstep_engine::logic::modules::deletion_update::{DeletionUpdate, DeletionUpdateModuleData};
use rts_lockstep_engine::logic::modules::smart_bomb_target_homing_update::{
    SmartBombHost, SmartBombTargetHomingUpdate, SmartBombTargetHomingUpdateModuleData,
};
use rts_lockstep_engine::logic::modules::lifetime_update::{LifetimeUpdate, LifetimeUpdateModuleData};
use rts_lockstep_engine::logic::modules::UpdateHost;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;
use support::{Check, Gen};

/// The object's side of the world, as the C++ stand-ins answer it, and the counters the programs read back.
#[derive(Default, Debug)]
struct Host {
    frame: u32,
    hulk: bool,
    over: i32,
    awakes: u32,
    last_wake: u32,
    kills: u32,
    destroys: u32,
}

impl UpdateHost for Host {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn is_kind_of_hulk(&self) -> bool {
        self.hulk
    }
    fn hulk_max_lifetime_override(&self) -> i32 {
        self.over
    }
    fn awaken_update_module(&mut self, frame: u32) {
        self.awakes += 1;
        self.last_wake = frame;
    }
    fn kill_object(&mut self) {
        self.kills += 1;
    }
    fn destroy_object(&mut self) {
        self.destroys += 1;
    }
}

/// The block parse of the file: the class of `kind` reads the `D` block (0 Lifetime, 1 Deletion).
#[derive(Default)]
struct Slot {
    kind: u32,
    life: Option<LifetimeUpdateModuleData>,
    del: Option<DeletionUpdateModuleData>,
    regen: Option<BaseRegenerateUpdateModuleData>,
    smart: Option<SmartBombTargetHomingUpdateModuleData>,
}

fn read_block(ini: &mut Ini<'_>, s: &mut Slot) -> IniResult<()> {
    match s.kind {
        0 => s.life = Some(LifetimeUpdateModuleData::friend_new_module_data(Some(ini))?),
        1 => s.del = Some(DeletionUpdateModuleData::friend_new_module_data(Some(ini))?),
        2 => s.regen = Some(BaseRegenerateUpdateModuleData::friend_new_module_data(Some(ini))?),
        _ => s.smart = Some(SmartBombTargetHomingUpdateModuleData::friend_new_module_data(Some(ini))?),
    }
    Ok(())
}

/// The file the xfer ops of both sides write, in cargo's per-target temporary directory (the C++ side is given the same
/// path, so the file is the one thing both sides name).
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9updates.bin");
const XFER_FILE: &str = "l9updates.bin";

/// The host of BaseRegenerateUpdate: the global data, the status bits, the body (a health capped at its maximum by a
/// heal, as ActiveBody caps it) and the heals asked for.
#[derive(Default, Debug)]
struct RegenHost {
    frame: u32,
    awakes: u32,
    last_wake: u32,
    pct: f32,
    delay: u32,
    under: bool,
    sold: bool,
    max: f32,
    health: f32,
    heals: Vec<f32>,
}

impl UpdateHost for RegenHost {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn is_kind_of_hulk(&self) -> bool {
        false
    }
    fn hulk_max_lifetime_override(&self) -> i32 {
        -1
    }
    fn awaken_update_module(&mut self, frame: u32) {
        self.awakes += 1;
        self.last_wake = frame;
    }
    fn kill_object(&mut self) {}
    fn destroy_object(&mut self) {}
}

impl BaseRegenHost for RegenHost {
    fn base_regen_health_percent_per_second(&self) -> f32 {
        self.pct
    }
    fn base_regen_delay(&self) -> u32 {
        self.delay
    }
    fn is_under_construction(&self) -> bool {
        self.under
    }
    fn is_sold(&self) -> bool {
        self.sold
    }
    fn max_health(&mut self) -> f32 {
        self.max
    }
    fn health(&mut self) -> f32 {
        self.health
    }
    fn attempt_healing(&mut self, amount: f32) {
        self.heals.push(amount);
        self.health += amount;
        if self.health > self.max {
            self.health = self.max;
        }
    }
}

/// The Rust side of a BaseRegenerateUpdate program: exactly the words `wrap_l9updates.cpp` writes.
fn rust_regen_run(text: &[u8], pct: f32, delay: u32, seed: u32, frame: u32, ops: &[[u32; 6]]) -> Run {
    let mut slot = Slot { kind: 2, ..Default::default() };
    let blocks = [BlockParse { token: "D", parse: read_block }];
    if Ini::load(b"l9updates.ini", text, IniLoadType::Overwrite, None, &blocks, &mut slot).is_err() {
        return Run { failed: true, words: Vec::new() };
    }
    let mut host = RegenHost { frame, pct, delay, ..Default::default() };
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(seed);
    let mut regen = BaseRegenerateUpdate::new(1, slot.regen.map(Arc::new), &mut host).expect("data present");
    let mut words = Vec::new();
    let mut last_sleep: u32 = 1;
    let mut saved: Vec<u8> = Vec::new();
    for op in ops {
        match op[0] {
            0 => last_sleep = regen.update(&mut host).0,
            1 => regen.on_damage(&mut host, op[1] as i32),
            2 => host.frame = op[1],
            3 => {
                host.under = op[1] & 1 != 0;
                host.sold = op[1] & 2 != 0;
            }
            4 => {
                host.max = op[1] as f32;
                host.health = op[2] as f32;
            }
            _ => {}
        }
        let woke = u32::from(host.awakes > 0);
        let last_heal = host.heals.last().map_or(0, |h| h.to_bits());
        words.extend_from_slice(&[host.awakes, host.last_wake, woke, woke, last_sleep, host.heals.len() as u32, last_heal, host.health.to_bits()]);
        words.extend_from_slice(&rng.logic);
        match op[0] {
            5 => {
                let mut x = XferSave::new();
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(&mut regen) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                saved = x.bytes().to_vec();
                words.extend_from_slice(&[st, saved.len() as u32]);
                words.extend(saved.iter().map(|&b| u32::from(b)));
            }
            6 => {
                let mut x = XferLoad::new(saved.clone());
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(&mut regen) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.push(st);
            }
            7 => {
                let mut x = XferCrc::new_without_nan_guard();
                let st = match x.open(b"crc") {
                    Ok(()) => match x.xfer_snapshot(&mut regen) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.extend_from_slice(&[st, x.get_crc()]);
            }
            _ => {}
        }
    }
    Run { failed: false, words }
}

/// The C++ side of a BaseRegenerateUpdate program.
fn cpp_regen_run(text: &[u8], pct: f32, delay: u32, seed: u32, frame: u32, ops: &[[u32; 6]]) -> Run {
    let flat: Vec<u32> = ops.iter().flat_map(|o| o.iter().copied()).collect();
    let mut out = vec![0u32; 1 << 16];
    let mut out_len: c_int = 0;
    let mut status: c_int = 0;
    let mut code: u32 = 0;
    let mut msg = vec![0 as c_char; 1024];
    let xfer_file = CString::new(XFER_FILE_PATH).expect("no NUL in the path");
    let _guard = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        cpp::ref_l9updates_regen_run(
            text.as_ptr() as *const c_char,
            text.len() as c_int,
            pct,
            delay,
            seed,
            frame,
            ops.len() as c_int,
            flat.as_ptr(),
            out.as_mut_ptr(),
            out.len() as c_int,
            &mut out_len,
            &mut status,
            &mut code,
            msg.as_mut_ptr(),
            msg.len() as c_int,
            xfer_file.as_ptr(),
        );
    }
    if status != 0 {
        return Run { failed: true, words: Vec::new() };
    }
    assert!((out_len as usize) <= out.len(), "the wrapper's output overflowed its buffer");
    Run { failed: false, words: out[..out_len as usize].to_vec() }
}

/// One random BaseRegenerateUpdate program: the INI text (empty, or with a field the class does not have), the global
/// rate and delay, the seed and frame, and 1 to 40 ops.
fn gen_regen_program(g: &mut Gen) -> (Vec<u8>, f32, u32, u32, u32, Vec<[u32; 6]>) {
    let mut text = String::from("D\n");
    if g.below(20) == 0 {
        text.push_str("MinLifetime = 3\n");
    }
    text.push_str("End\n");
    let pct = match g.below(8) {
        0 => 0.0,
        1 => 0.0005,
        2 => 0.02,
        3 => 0.1,
        4 => 0.333,
        5 => 1.0,
        _ => g.uniform(0.0, 0.2),
    };
    let delay = g.below(300);
    let frame = if g.below(20) == 0 { 0xFFFF_FFF0u32.wrapping_add(g.below(16)) } else { g.below(1_000_000) };
    let n = 1 + g.below(40) as usize;
    let mut ops = Vec::with_capacity(n);
    for _ in 0..n {
        let op = match g.below(100) {
            0..=29 => [0, 0, 0, 0, 0, 0],
            30..=49 => [1, g.below(12), 0, 0, 0, 0],
            50..=59 => [2, g.below(2_000_000), 0, 0, 0, 0],
            60..=69 => [3, g.below(4), 0, 0, 0, 0],
            70..=84 => [4, g.below(2_000), g.below(2_100), 0, 0, 0],
            85..=89 => [5, 0, 0, 0, 0, 0],
            90..=94 => [6, 0, 0, 0, 0, 0],
            _ => [7, 0, 0, 0, 0, 0],
        };
        ops.push(op);
    }
    (text.into_bytes(), pct, delay, g.u32(), frame, ops)
}

/// One program: the module, its data text, the environment, and the ops.
#[derive(Clone, Debug)]
struct Program {
    kind: u32,
    text: Vec<u8>,
    seed: u32,
    frame: u32,
    hulk: bool,
    over: i32,
    ops: Vec<[u32; 6]>,
}

/// The words of one run, and whether the data read failed.
#[derive(Debug, PartialEq)]
struct Run {
    failed: bool,
    words: Vec<u32>,
}

/// The Rust side of a program: exactly the words `wrap_l9updates.cpp` writes for it.
fn rust_run(p: &Program) -> Run {
    let mut slot = Slot { kind: p.kind, ..Default::default() };
    let blocks = [BlockParse { token: "D", parse: read_block }];
    if Ini::load(b"l9updates.ini", &p.text, IniLoadType::Overwrite, None, &blocks, &mut slot).is_err() {
        return Run { failed: true, words: Vec::new() };
    }
    let mut host = Host { frame: p.frame, hulk: p.hulk, over: p.over, ..Default::default() };
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(p.seed);
    let obj: ObjectId = 1;
    let mut life: Option<LifetimeUpdate> = None;
    let mut del: Option<DeletionUpdate> = None;
    if p.kind == 0 {
        life = Some(LifetimeUpdate::new(obj, slot.life.map(Arc::new), &mut host, &mut rng).expect("data present"));
    } else {
        del = Some(DeletionUpdate::new(obj, slot.del.map(Arc::new), &mut host, &mut rng).expect("data present"));
    }
    let mut words = Vec::new();
    let mut last_sleep: u32 = 1;
    let mut saved: Vec<u8> = Vec::new();
    for op in &p.ops {
        match op[0] {
            0 => {
                last_sleep = if let Some(l) = life.as_mut() { l.update(&mut host).0 } else { del.as_mut().unwrap().update(&mut host).0 };
            }
            1 => {
                if let Some(l) = life.as_mut() {
                    l.set_lifetime_range(&mut host, &mut rng, op[1], op[2]);
                } else {
                    del.as_mut().unwrap().set_lifetime_range(&mut host, &mut rng, op[1], op[2]);
                }
            }
            2 => host.frame = op[1],
            _ => {}
        }
        let die = if let Some(l) = life.as_ref() { l.get_die_frame() } else { del.as_ref().unwrap().get_die_frame() };
        let woke = u32::from(host.awakes > 0);
        words.extend_from_slice(&[die, host.kills, host.destroys, host.awakes, host.last_wake, woke, woke, last_sleep]);
        words.extend_from_slice(&rng.logic);
        let snap: &mut dyn Snapshot = if let Some(l) = life.as_mut() { l } else { del.as_mut().unwrap() };
        match op[0] {
            3 => {
                let mut x = XferSave::new();
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(snap) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                saved = x.bytes().to_vec();
                words.extend_from_slice(&[st, saved.len() as u32]);
                words.extend(saved.iter().map(|&b| u32::from(b)));
            }
            4 => {
                let mut x = XferLoad::new(saved.clone());
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(snap) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.push(st);
            }
            5 => {
                let mut x = XferCrc::new_without_nan_guard();
                let st = match x.open(b"crc") {
                    Ok(()) => match x.xfer_snapshot(snap) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.extend_from_slice(&[st, x.get_crc()]);
            }
            _ => {}
        }
    }
    Run { failed: false, words }
}

/// The C++ side of a program: the same words, from `ref_l9updates_run`.
fn cpp_run(p: &Program) -> Run {
    let flat: Vec<u32> = p.ops.iter().flat_map(|o| o.iter().copied()).collect();
    let mut out = vec![0u32; 1 << 16];
    let mut out_len: c_int = 0;
    let mut status: c_int = 0;
    let mut code: u32 = 0;
    let mut msg = vec![0 as c_char; 1024];
    let xfer_file = CString::new(XFER_FILE_PATH).expect("no NUL in the path");
    let _guard = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        cpp::ref_l9updates_run(
            p.kind as c_int,
            p.text.as_ptr() as *const c_char,
            p.text.len() as c_int,
            p.seed,
            p.frame,
            c_int::from(p.hulk),
            p.over,
            p.ops.len() as c_int,
            flat.as_ptr(),
            out.as_mut_ptr(),
            out.len() as c_int,
            &mut out_len,
            &mut status,
            &mut code,
            msg.as_mut_ptr(),
            msg.len() as c_int,
            xfer_file.as_ptr(),
        );
    }
    if status != 0 {
        return Run { failed: true, words: Vec::new() };
    }
    assert!((out_len as usize) <= out.len(), "the wrapper's output overflowed its buffer");
    Run { failed: false, words: out[..out_len as usize].to_vec() }
}

/// One random program of `kind`.
fn gen_program(g: &mut Gen, kind: u32) -> Program {
    let mut text = String::from("D\n");
    let bad = g.below(40) == 0;
    if bad {
        // a value that does not parse as a duration: both sides must refuse the block
        let junk = ["abc", "1.5", "-5x"][g.below(3) as usize];
        text.push_str(&format!("MinLifetime = {junk}\n"));
    } else {
        let min = g.below(90_001);
        let max = min + g.below(30_001);
        match g.below(8) {
            0 => {}
            1 => text.push_str(&format!("MaxLifetime = {max}\n")),
            _ => {
                text.push_str(&format!("MinLifetime = {min}\n"));
                text.push_str(&format!("MaxLifetime = {max}\n"));
            }
        }
    }
    text.push_str("End\n");
    let frame = if g.below(20) == 0 { 0xFFFF_FFF0u32.wrapping_add(g.below(16)) } else { g.below(1_000_000) };
    let over = match g.below(3) {
        0 => -1,
        1 => g.below(5_000) as i32,
        _ => -1 - g.below(2) as i32,
    };
    let n = 1 + g.below(40) as usize;
    let mut ops = Vec::with_capacity(n);
    for _ in 0..n {
        let a = g.below(3_000);
        let b = a + g.below(3_000);
        let op = match g.below(100) {
            0..=29 => [0, 0, 0, 0, 0, 0],
            30..=54 => [1, a, b, 0, 0, 0],
            55..=69 => [2, if g.below(4) == 0 { 0xFFFF_FFFF } else { g.below(2_000_000) }, 0, 0, 0, 0],
            70..=79 => [3, 0, 0, 0, 0, 0],
            80..=89 => [4, 0, 0, 0, 0, 0],
            _ => [5, 0, 0, 0, 0, 0],
        };
        ops.push(op);
    }
    Program { kind, text: text.into_bytes(), seed: g.u32(), frame, hulk: g.below(2) == 1, over, ops }
}

fn cases() -> u32 {
    std::env::var("L9U_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(1500)
}

fn compare_module(name: &str, kind: u32, seed: u64) {
    let mut g = Gen::new(seed);
    let mut check = Check::new(name);
    let (mut data_failures, mut ops_run) = (0u32, 0u64);
    let mut programs_run = 0u32;
    for case in 0..cases() {
        let p = gen_program(&mut g, kind);
        let r = rust_run(&p);
        let c = cpp_run(&p);
        programs_run += 1;
        check.eq(r.failed, c.failed, || format!("{name} program {case}: data status of {:?}", String::from_utf8_lossy(&p.text)));
        if c.failed {
            data_failures += 1;
            continue;
        }
        ops_run += p.ops.len() as u64;
        check.eq(r.words.len(), c.words.len(), || format!("{name} program {case}: word count, {p:?}"));
        for (i, (rw, cw)) in r.words.iter().zip(c.words.iter()).enumerate() {
            check.eq(*rw, *cw, || format!("{name} program {case}: word {i}, {p:?}"));
        }
    }
    println!("L9U {name}: {programs_run} programs, {ops_run} ops, {data_failures} with a refused INI block");
    check.finish();
}

#[test]
fn lifetime_update_matches_upstream() {
    compare_module("LifetimeUpdate", 0, 0x9_1001);
}

#[test]
fn deletion_update_matches_upstream() {
    compare_module("DeletionUpdate", 1, 0x9_1002);
}

#[test]
fn null_module_data_is_an_error() {
    // upstream reads `getLifetimeUpdateModuleData()` of null data (undefined): only the Rust side is run.
    let mut host = Host::default();
    let mut rng = RandomValues::default();
    assert!(LifetimeUpdate::new(1, None, &mut host, &mut rng).is_err());
    assert!(DeletionUpdate::new(1, None, &mut host, &mut rng).is_err());
    assert!(BaseRegenerateUpdate::new(1, None, &mut RegenHost::default()).is_err());
    assert!(SmartBombTargetHomingUpdate::new(1, None, &mut Host::default()).is_err());
    assert_eq!(host.awakes, 0, "no wake is requested for null data");
}

#[test]
fn accessors_report_the_module_and_its_data() {
    // The accessors are not reached by the differential programs (they are for the world's wiring): the module's
    // scheduling base, its phase, its data, and the constructor's wake frame (the die frame plus nothing).
    let mut host = Host { frame: 10, hulk: false, over: -1, ..Default::default() };
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(7);
    let data = Arc::new(LifetimeUpdateModuleData { min_frames: 3, max_frames: 3, ..Default::default() });
    let lu = LifetimeUpdate::new(4, Some(data.clone()), &mut host, &mut rng).expect("data present");
    assert_eq!(lu.module_data().min_frames, 3);
    assert_eq!(lu.module_data().max_frames, 3);
    assert_eq!(lu.update_module().get_update_phase(), rts_lockstep_engine::logic::module::update_module::SleepyUpdatePhase::Normal);
    assert_eq!(lu.get_die_frame(), 13);
    assert_eq!((host.awakes, host.last_wake), (1, 13), "the constructor wakes the module at its die frame");
    let ddata = Arc::new(DeletionUpdateModuleData { min_frames: 5, max_frames: 5, ..Default::default() });
    let mut host = Host { frame: 20, ..Default::default() };
    let du = DeletionUpdate::new(4, Some(ddata), &mut host, &mut rng).expect("data present");
    assert_eq!(du.module_data().min_frames, 5);
    assert_eq!(du.get_die_frame(), 25);
    assert_eq!(du.update_module().get_update_phase(), rts_lockstep_engine::logic::module::update_module::SleepyUpdatePhase::Normal);
}

fn compare_regen(seed: u64) {
    let name = "BaseRegenerateUpdate";
    let mut g = Gen::new(seed);
    let mut check = Check::new(name);
    let (mut data_failures, mut ops_run, mut programs_run) = (0u32, 0u64, 0u32);
    for case in 0..cases() {
        let (text, pct, delay, pseed, frame, ops) = gen_regen_program(&mut g);
        let r = rust_regen_run(&text, pct, delay, pseed, frame, &ops);
        let c = cpp_regen_run(&text, pct, delay, pseed, frame, &ops);
        programs_run += 1;
        check.eq(r.failed, c.failed, || format!("{name} program {case}: data status of {:?}", String::from_utf8_lossy(&text)));
        if c.failed {
            data_failures += 1;
            continue;
        }
        ops_run += ops.len() as u64;
        let input = || format!("{name} program {case}: pct {pct:?} delay {delay} seed {pseed} frame {frame} ops {ops:?}");
        check.eq(r.words.len(), c.words.len(), || format!("{}: word count", input()));
        for (i, (rw, cw)) in r.words.iter().zip(c.words.iter()).enumerate() {
            check.eq(*rw, *cw, || format!("{}: word {i}", input()));
        }
    }
    println!("L9U {name}: {programs_run} programs, {ops_run} ops, {data_failures} with a refused INI block");
    check.finish();
}

#[test]
fn base_regenerate_matches_upstream() {
    compare_regen(0x9_1003);
}

/// The host of SmartBombTargetHomingUpdate: the frame, the terrain answer and the position of the object.
#[derive(Default, Debug)]
struct SmartHost {
    frame: u32,
    awakes: u32,
    last_wake: u32,
    above: bool,
    pos: Coord3D,
}

impl UpdateHost for SmartHost {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn is_kind_of_hulk(&self) -> bool {
        false
    }
    fn hulk_max_lifetime_override(&self) -> i32 {
        -1
    }
    fn awaken_update_module(&mut self, frame: u32) {
        self.awakes += 1;
        self.last_wake = frame;
    }
    fn kill_object(&mut self) {}
    fn destroy_object(&mut self) {}
}

impl SmartBombHost for SmartHost {
    fn is_significantly_above_terrain(&mut self) -> bool {
        self.above
    }
    fn position(&self) -> Coord3D {
        self.pos
    }
    fn set_position(&mut self, pos: Coord3D) {
        self.pos = pos;
    }
}

fn bits3(c: Coord3D) -> [u32; 3] {
    [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]
}

/// The Rust side of a SmartBombTargetHomingUpdate program.
fn rust_smart_run(text: &[u8], seed: u32, frame: u32, ops: &[[u32; 6]]) -> Run {
    let mut slot = Slot { kind: 3, ..Default::default() };
    let blocks = [BlockParse { token: "D", parse: read_block }];
    if Ini::load(b"l9updates.ini", text, IniLoadType::Overwrite, None, &blocks, &mut slot).is_err() {
        return Run { failed: true, words: Vec::new() };
    }
    let mut host = SmartHost { frame, ..Default::default() };
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(seed);
    let mut sb = SmartBombTargetHomingUpdate::new(1, slot.smart.map(Arc::new), &mut host).expect("data present");
    let mut words = Vec::new();
    let mut last_sleep: u32 = 1;
    let mut saved: Vec<u8> = Vec::new();
    for op in ops {
        match op[0] {
            0 => last_sleep = sb.update(&mut host).0,
            1 => sb.set_target_position(Coord3D { x: f32::from_bits(op[1]), y: f32::from_bits(op[2]), z: f32::from_bits(op[3]) }),
            2 => host.frame = op[1],
            3 => host.above = op[1] & 1 != 0,
            4 => host.pos = Coord3D { x: f32::from_bits(op[1]), y: f32::from_bits(op[2]), z: f32::from_bits(op[3]) },
            _ => {}
        }
        let woke = u32::from(host.awakes > 0);
        words.extend_from_slice(&[host.awakes, host.last_wake, woke, woke, last_sleep, u32::from(sb.target_received())]);
        words.extend_from_slice(&bits3(sb.target()));
        words.extend_from_slice(&bits3(host.pos));
        words.extend_from_slice(&rng.logic);
        match op[0] {
            5 => {
                let mut x = XferSave::new();
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(&mut sb) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                saved = x.bytes().to_vec();
                words.extend_from_slice(&[st, saved.len() as u32]);
                words.extend(saved.iter().map(|&b| u32::from(b)));
            }
            6 => {
                let mut x = XferLoad::new(saved.clone());
                let st = match x.open(XFER_FILE.as_bytes()) {
                    Ok(()) => match x.xfer_snapshot(&mut sb) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.push(st);
            }
            7 => {
                let mut x = XferCrc::new_without_nan_guard();
                let st = match x.open(b"crc") {
                    Ok(()) => match x.xfer_snapshot(&mut sb) {
                        Ok(()) => 0,
                        Err(e) => e as u32,
                    },
                    Err(e) => e as u32,
                };
                let _ = x.close();
                words.extend_from_slice(&[st, x.get_crc()]);
            }
            _ => {}
        }
    }
    Run { failed: false, words }
}

/// The C++ side of a SmartBombTargetHomingUpdate program.
fn cpp_smart_run(text: &[u8], seed: u32, frame: u32, ops: &[[u32; 6]]) -> Run {
    let flat: Vec<u32> = ops.iter().flat_map(|o| o.iter().copied()).collect();
    let mut out = vec![0u32; 1 << 16];
    let mut out_len: c_int = 0;
    let mut status: c_int = 0;
    let mut code: u32 = 0;
    let mut msg = vec![0 as c_char; 1024];
    let xfer_file = CString::new(XFER_FILE_PATH).expect("no NUL in the path");
    let _guard = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        cpp::ref_l9updates_smart_run(
            text.as_ptr() as *const c_char,
            text.len() as c_int,
            seed,
            frame,
            ops.len() as c_int,
            flat.as_ptr(),
            out.as_mut_ptr(),
            out.len() as c_int,
            &mut out_len,
            &mut status,
            &mut code,
            msg.as_mut_ptr(),
            msg.len() as c_int,
            xfer_file.as_ptr(),
        );
    }
    if status != 0 {
        return Run { failed: true, words: Vec::new() };
    }
    assert!((out_len as usize) <= out.len(), "the wrapper's output overflowed its buffer");
    Run { failed: false, words: out[..out_len as usize].to_vec() }
}

/// One random SmartBombTargetHomingUpdate program: the INI text (the scalar, or a refused field), the seed and frame, and
/// 1 to 40 ops. Floats are sent as their bits: ordinary values, zero, the values a target of length zero or NaN takes, and
/// random bit patterns.
fn gen_smart_program(g: &mut Gen) -> (Vec<u8>, u32, u32, Vec<[u32; 6]>) {
    let mut text = String::from("D\n");
    match g.below(12) {
        0 => text.push_str("CourseCorrectionScalar = bogus\n"),
        1 => {}
        _ => text.push_str(&format!("CourseCorrectionScalar = {}\n", [0.0f32, 0.5, 0.99, 1.0, 1.5, -0.25][g.below(6) as usize] + g.uniform(-0.001, 0.001) * (g.below(2) as f32))),
    }
    text.push_str("End\n");
    let float = |g: &mut Gen| -> u32 {
        match g.below(10) {
            0 => 0.0f32.to_bits(),
            1 => f32::NAN.to_bits(),
            2 => g.u32(),
            _ => g.uniform(-1000.0, 1000.0).to_bits(),
        }
    };
    let n = 1 + g.below(40) as usize;
    let mut ops = Vec::with_capacity(n);
    for _ in 0..n {
        let op = match g.below(100) {
            0..=29 => [0, 0, 0, 0, 0, 0],
            30..=49 => [1, float(g), float(g), float(g), 0, 0],
            50..=59 => [2, g.below(2_000_000), 0, 0, 0, 0],
            60..=69 => [3, g.below(4), 0, 0, 0, 0],
            70..=84 => [4, float(g), float(g), float(g), 0, 0],
            85..=89 => [5, 0, 0, 0, 0, 0],
            90..=94 => [6, 0, 0, 0, 0, 0],
            _ => [7, 0, 0, 0, 0, 0],
        };
        ops.push(op);
    }
    let frame = g.below(1_000_000);
    (text.into_bytes(), g.u32(), frame, ops)
}

fn compare_smart(seed: u64) {
    let name = "SmartBombTargetHomingUpdate";
    let mut g = Gen::new(seed);
    let mut check = Check::new(name);
    let (mut data_failures, mut ops_run, mut programs_run) = (0u32, 0u64, 0u32);
    for case in 0..cases() {
        let (text, pseed, frame, ops) = gen_smart_program(&mut g);
        let r = rust_smart_run(&text, pseed, frame, &ops);
        let c = cpp_smart_run(&text, pseed, frame, &ops);
        programs_run += 1;
        check.eq(r.failed, c.failed, || format!("{name} program {case}: data status of {:?}", String::from_utf8_lossy(&text)));
        if c.failed {
            data_failures += 1;
            continue;
        }
        ops_run += ops.len() as u64;
        let input = || format!("{name} program {case}: seed {pseed} frame {frame} ops {ops:?} text {:?}", String::from_utf8_lossy(&text));
        check.eq(r.words.len(), c.words.len(), || format!("{}: word count", input()));
        for (i, (rw, cw)) in r.words.iter().zip(c.words.iter()).enumerate() {
            check.eq(*rw, *cw, || format!("{}: word {i}", input()));
        }
    }
    println!("L9U {name}: {programs_run} programs, {ops_run} ops, {data_failures} with a refused INI block");
    check.finish();
}

#[test]
fn smart_bomb_target_homing_matches_upstream() {
    compare_smart(0x9_1004);
}

#[test]
fn accessors_of_the_regen_and_smart_bomb_modules() {
    // The accessors not reached by the programs: the data, the base and the target of the homing module's constructor.
    let mut host = RegenHost { frame: 5, pct: 0.01, delay: 4, ..Default::default() };
    let rdata = Arc::new(BaseRegenerateUpdateModuleData::new());
    let regen = BaseRegenerateUpdate::new(1, Some(rdata), &mut host).expect("data present");
    assert_eq!(host.awakes, 1);
    assert_eq!(host.last_wake, 6, "a rate above zero: due the next frame");
    assert_eq!(regen.update_module().get_update_phase(), rts_lockstep_engine::logic::module::update_module::SleepyUpdatePhase::Normal);
    assert_eq!(regen.module_data().update.behavior, Default::default(), "the data is the one given");
    let mut zero = RegenHost { frame: 5, pct: 0.0, delay: 4, ..Default::default() };
    let _ = BaseRegenerateUpdate::new(1, Some(Arc::new(BaseRegenerateUpdateModuleData::new())), &mut zero).expect("data present");
    assert_eq!(zero.last_wake, 5 + 0x3fff_ffff, "a rate of zero: asleep for ever");
    let mut sb_host = SmartHost { frame: 3, ..Default::default() };
    let sdata = Arc::new(SmartBombTargetHomingUpdateModuleData::new());
    let mut sb = SmartBombTargetHomingUpdate::new(1, Some(sdata.clone()), &mut sb_host).expect("data present");
    assert!((sb.module_data().course_correction_scalar - 0.99).abs() < 1e-6, "the default course correction");
    assert!(!sb.target_received());
    sb.set_target_position(Coord3D { x: 1.0, y: 2.0, z: 3.0 });
    assert!(sb.target_received());
    assert_eq!(sb.target(), Coord3D { x: 1.0, y: 2.0, z: 3.0 });
    assert_eq!(sb.update_module().get_update_phase(), rts_lockstep_engine::logic::module::update_module::SleepyUpdatePhase::Normal);
}
