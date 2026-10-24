// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::experience_tracker` (`ExperienceTracker`)
//! against upstream `GameLogic/Object/ExperienceTracker.cpp` (commit 0a05454d),
//! compiled unchanged in `differential/wrap_experience_tracker.cpp` (see
//! build.rs). Phase 1 layer 8, p1w6-trackers.
//!
//! # Methodology
//!
//! A **program** is a random sequence of operations on one to four objects,
//! each with its own template (experience values, experience required per
//! level, trainable or not) and a random relationship matrix, run on both
//! sides from fresh trackers. Operations: `addExperiencePoints` (scaled and
//! unscaled), `setExperienceAndLevel`, `setVeterancyLevel`,
//! `setMinVeterancyLevel`, `gainExpForLevel`, `canGainExpForLevel`,
//! `setExperienceSink` (always to a higher id, so the chain cannot loop, and
//! sometimes to an id that is no object), `setExperienceScalar` (including
//! huge and negative values, so the product overflows `Int`),
//! `getExperienceValue` (ally and not), `isTrainable`,
//! `isAcceptingExperiencePoints`, a state dump, `XferSave` (the bytes),
//! `XferLoad` of a blob (the status and the state afterwards) and the CRC
//! bytes. The templates' requirements are monotone most of the time and
//! arbitrary otherwise (the level loop is the same code either way, and the
//! arbitrary case lowers levels as upstream does).
//!
//! Compared, word for word: the log of every program, in which every
//! operation logs its result and then every `onVeterancyLevelChanged` call it
//! caused (the object, old level, new level and feedback flag), in order.
//!
//! The blob a program loads is random bytes, a valid save, that save with one
//! bit flipped, or that save cut at a field boundary (see `random_blob` for
//! why not inside a field), **sanitised** in two fields: a level
//! that is not 0..=3 (upstream stores any integer and indexes its tables with
//! it; the Rust refuses it, see the module docs, and
//! `an_out_of_range_level_is_refused_on_load` pins that) and a sink that is
//! one of the program's objects (a load could then close a sink loop, which
//! recurses without end in upstream).
//!
//! **Planted bug.** A sink that forwards the *unscaled* gain (upstream
//! forwards `experienceGain * m_experienceScalar`) runs on the same programs
//! and must differ.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_experience_tracker -- --nocapture` (the `DIFF`, `PLANTED` and
//! `COVERAGE` lines).

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::experience_tracker::{
    add_experience_points, can_gain_exp_for_level, gain_exp_for_level, set_experience_and_level, ExperienceArena,
    ExperienceRules, ExperienceTracker, LevelChange,
};
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// Programs per run.
const PROGRAMS: usize = 3000;
/// `ALLIES` of `enum Relationship`.
const ALLIES: i32 = 2;

/// The objects of one program, keyed by id (1..=n).
struct Arena {
    objects: BTreeMap<ObjectId, (ExperienceTracker, ExperienceRules)>,
    /// Adds that reached a sink (coverage).
    forwarded: usize,
}

impl ExperienceArena for Arena {
    fn experience_of(&mut self, id: ObjectId) -> Option<(&mut ExperienceTracker, ExperienceRules)> {
        self.objects.get_mut(&id).map(|(t, r)| (t, *r))
    }
}

/// A program: the objects, the relations, the operations and the blob.
struct Program {
    rules: Vec<ExperienceRules>,
    /// `rel[killer][target]`.
    rel: Vec<Vec<i32>>,
    ops: Vec<[i32; 4]>,
    blob: Vec<u8>,
}

fn level_from(i: i32) -> VeterancyLevel {
    match i {
        0 => VeterancyLevel::Regular,
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        _ => VeterancyLevel::Heroic,
    }
}

fn push_events(out: &mut Vec<u32>, changes: &mut Vec<LevelChange>) {
    out.push(changes.len() as u32);
    for c in changes.drain(..) {
        out.push(c.object as u32);
        out.push(c.old as i32 as u32);
        out.push(c.new as i32 as u32);
        out.push(u32::from(c.provide_feedback));
    }
}

fn dump(out: &mut Vec<u32>, t: &ExperienceTracker) {
    out.push(t.veterancy_level() as i32 as u32);
    out.push(t.current_experience() as u32);
    out.push(t.experience_scalar().to_bits());
}

fn save_bytes(t: &ExperienceTracker) -> Vec<u8> {
    let mut save = XferSave::new();
    save.open(b"experience").unwrap();
    t.clone().xfer(&mut save).unwrap();
    save.close().unwrap();
    save.take_bytes()
}

fn crc_bytes(t: &ExperienceTracker) -> Vec<u8> {
    let mut save = XferSave::new();
    save.open(b"experience").unwrap();
    t.clone().crc(&mut save).unwrap();
    save.close().unwrap();
    save.take_bytes()
}

fn push_bytes(out: &mut Vec<u32>, bytes: &[u8]) {
    out.push(bytes.len() as u32);
    out.extend(bytes.iter().map(|&b| u32::from(b)));
}

/// The Rust side of one program: the same log words as `ref_experience_run`.
fn rust_run(p: &Program, planted: bool, forwarded: &mut usize) -> Vec<u32> {
    let mut arena = Arena { objects: BTreeMap::new(), forwarded: 0 };
    for (i, r) in p.rules.iter().enumerate() {
        let id = i as ObjectId + 1;
        arena.objects.insert(id, (ExperienceTracker::new(id), *r));
    }
    let mut out = Vec::new();
    let mut changes = Vec::new();
    for &[code, id, x, y] in &p.ops {
        match code {
            0 => {
                let (sink, scalar) = {
                    let t = &arena.objects[&id].0;
                    (t.experience_sink(), t.experience_scalar())
                };
                let sink_exists = sink != INVALID_ID && arena.objects.contains_key(&sink);
                if sink_exists {
                    arena.forwarded += 1;
                }
                if planted && sink_exists {
                    // the planted bug: the unscaled gain goes to the sink
                    let _ = scalar;
                    add_experience_points(&mut arena, sink, x, y != 0, &mut changes);
                } else {
                    add_experience_points(&mut arena, id, x, y != 0, &mut changes);
                }
            }
            1 => set_experience_and_level(&mut arena, id, x, y != 0, &mut changes),
            2 => {
                let r = arena.objects[&id].1;
                arena.objects.get_mut(&id).unwrap().0.set_veterancy_level(level_from(x), &r, y != 0, &mut changes);
            }
            3 => {
                let r = arena.objects[&id].1;
                arena.objects.get_mut(&id).unwrap().0.set_min_veterancy_level(level_from(x), &r, &mut changes);
            }
            4 => out.push(u32::from(gain_exp_for_level(&mut arena, id, x, y != 0, &mut changes))),
            5 => out.push(u32::from(can_gain_exp_for_level(&arena.objects[&id].0, x))),
            6 => arena.objects.get_mut(&id).unwrap().0.set_experience_sink(x),
            7 => arena.objects.get_mut(&id).unwrap().0.set_experience_scalar(x as f32 / 4.0),
            8 => {
                let (t, r) = &arena.objects[&id];
                let ally = p.rel[(x - 1) as usize][(id - 1) as usize] == ALLIES;
                out.push(t.experience_value(ally, r) as u32);
            }
            9 => {
                let (t, r) = &arena.objects[&id];
                out.push(u32::from(t.is_trainable(r)));
            }
            10 => {
                let (t, r) = &arena.objects[&id];
                out.push(u32::from(t.is_accepting_experience_points(r)));
            }
            11 => dump(&mut out, &arena.objects[&id].0),
            12 => push_bytes(&mut out, &save_bytes(&arena.objects[&id].0)),
            13 => {
                let t = &mut arena.objects.get_mut(&id).unwrap().0;
                let mut load = XferLoad::new(p.blob.clone());
                load.open(b"experience").unwrap();
                let status = match t.xfer(&mut load) {
                    Ok(()) => 0,
                    Err(s) => 1 + s as u32,
                };
                load.close().unwrap();
                out.push(status);
                dump(&mut out, t);
            }
            _ => push_bytes(&mut out, &crc_bytes(&arena.objects[&id].0)),
        }
        push_events(&mut out, &mut changes);
    }
    *forwarded += arena.forwarded;
    out
}

/// Serialises the C++ side (its stand-in objects are process globals).
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The C++ side of one program.
fn cpp_run(p: &Program, path: &std::path::Path) -> Vec<u32> {
    let n = p.rules.len();
    let mut spec: Vec<i32> = Vec::new();
    for r in &p.rules {
        spec.extend(r.experience_values);
        spec.extend(r.experience_required);
        spec.push(i32::from(r.trainable));
    }
    for row in &p.rel {
        spec.extend(row.iter().copied());
    }
    let flat: Vec<i32> = p.ops.iter().flat_map(|o| o.iter().copied()).collect();
    let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let _serial = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0u32; 1 << 16];
    let r = unsafe {
        cpp::ref_experience_run(
            n as i32,
            spec.as_ptr(),
            flat.as_ptr(),
            p.ops.len() as i32,
            p.blob.as_ptr(),
            p.blob.len() as i32,
            cpath.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    assert!(r >= 0, "ref_experience_run returned {r}");
    out.truncate(r as usize);
    out
}

/// A random `Int` that stresses wrapping now and then.
fn random_points(g: &mut Gen) -> i32 {
    match g.below(12) {
        0 => i32::MAX - g.below(50) as i32,
        1 => i32::MIN + g.below(50) as i32,
        2 => (g.below(2_000_000) as i32) - 1_000_000,
        _ => g.below(700) as i32 - 100,
    }
}

fn random_rules(g: &mut Gen) -> ExperienceRules {
    let monotone = g.below(5) != 0;
    let mut required = [0i32; 4];
    if monotone {
        let mut acc = 0;
        for r in required.iter_mut().skip(1) {
            acc += 1 + g.below(300) as i32;
            *r = acc;
        }
    } else {
        for r in &mut required {
            *r = g.below(600) as i32 - 100;
        }
    }
    let mut values = [0i32; 4];
    for v in &mut values {
        *v = g.below(50) as i32 - 5;
    }
    ExperienceRules { experience_values: values, experience_required: required, trainable: g.below(6) != 0 }
}

/// The save of a random tracker, as this crate writes it.
fn random_save(g: &mut Gen) -> Vec<u8> {
    let mut t = ExperienceTracker::new(1);
    let rules = random_rules(g);
    let mut changes = Vec::new();
    t.set_veterancy_level(level_from(g.below(4) as i32), &rules, true, &mut changes);
    t.set_experience_sink(g.below(8) as i32);
    t.set_experience_scalar(g.uniform(-2.0, 6.0));
    save_bytes(&t)
}

/// The sanitising described in the module docs: a level in 0..=3 and a sink
/// of 16 or more, wherever the blob is long enough to hold them. (A blob is
/// only ever cut at a field boundary, see `random_blob`.)
fn sanitise(mut b: Vec<u8>) -> Vec<u8> {
    if b.is_empty() || b[0] > 1 {
        return b; // the version is refused before any field is read
    }
    if b.len() >= 5 {
        b[1] &= 3;
        b[2..5].fill(0);
    }
    if b.len() >= 13 {
        // a low byte of 16 or more is never one of the program's (at most
        // four) objects
        b[9] |= 0x10;
    }
    b
}

/// Field boundaries of a save: after the version, level, points, sink, scalar.
const BOUNDARIES: [usize; 6] = [0, 1, 5, 9, 13, 17];

/// The blob a program loads: random bytes, a valid save, that save with one
/// bit flipped, or that save cut at a field boundary. Never cut inside a
/// field: when a load runs out of bytes part-way through a field, upstream's
/// `XferLoad` has already copied the bytes it got into it, and this crate's
/// does not, so the state after such a failed load differs by construction
/// (a property of the Xfer layer, outside this unit; the status is the same).
fn random_blob(g: &mut Gen) -> Vec<u8> {
    let b = match g.below(4) {
        0 => (0..BOUNDARIES[g.below(6) as usize]).map(|_| g.u32() as u8).collect(),
        1 => random_save(g),
        2 => {
            let mut b = random_save(g);
            let i = g.below(b.len() as u32) as usize;
            b[i] ^= 1 << g.below(8);
            b
        }
        _ => {
            let b = random_save(g);
            b[..BOUNDARIES[g.below(6) as usize]].to_vec()
        }
    };
    sanitise(b)
}

fn random_program(g: &mut Gen) -> Program {
    let n = 1 + g.below(4) as usize;
    let rules: Vec<ExperienceRules> = (0..n).map(|_| random_rules(g)).collect();
    let rel: Vec<Vec<i32>> = (0..n).map(|_| (0..n).map(|_| g.below(3) as i32).collect()).collect();
    let blob = random_blob(g);
    let n_ops = 1 + g.below(60);
    let mut ops = Vec::new();
    for _ in 0..n_ops {
        let id = 1 + g.below(n as u32) as i32;
        let flag = g.below(2) as i32;
        let op = match g.below(40) {
            0..=8 => [0, id, random_points(g), flag],
            9..=10 => [1, id, random_points(g), flag],
            11..=12 => [2, id, g.below(4) as i32, flag],
            13 => [3, id, g.below(4) as i32, 0],
            14..=16 => [4, id, g.below(9) as i32 - 2, flag],
            17 => [5, id, g.below(9) as i32 - 2, 0],
            18..=20 => {
                // a sink is always a higher id (no loops), or no object at all
                let sink = if g.below(5) == 0 {
                    0
                } else if g.below(6) == 0 {
                    9
                } else {
                    id + 1 + g.below(3) as i32
                };
                [6, id, sink, 0]
            }
            21..=23 => {
                let x = match g.below(8) {
                    0 => 1 << 28,
                    1 => -(1 << 29),
                    _ => g.below(40) as i32 - 8,
                };
                [7, id, x, 0]
            }
            24..=25 => [8, id, 1 + g.below(n as u32) as i32, 0],
            26 => [9, id, 0, 0],
            27 => [10, id, 0, 0],
            28..=30 => [11, id, 0, 0],
            31..=33 => [12, id, 0, 0],
            34..=36 => [13, id, 0, 0],
            _ => [14, id, 0, 0],
        };
        ops.push(op);
    }
    Program { rules, rel, ops, blob }
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-experience-{}-{tag}.bin", std::process::id()))
}

#[test]
fn experience_tracker_matches_upstream_word_for_word() {
    let path = temp_path("main");
    let mut g = Gen::new(0x0602);
    let mut check = Check::new("ExperienceTracker (programs, saves, loads)");
    let (mut ops_total, mut loads, mut forwarded, mut changes_total) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..PROGRAMS {
        let p = random_program(&mut g);
        ops_total += p.ops.len();
        loads += p.ops.iter().filter(|o| o[0] == 13).count();
        let rust = rust_run(&p, false, &mut forwarded);
        let cpp = cpp_run(&p, &path);
        changes_total += cpp.len();
        check.eq(rust, cpp, || format!("program ops {:?} rules {:?} rel {:?} blob {:?}", p.ops, p.rules, p.rel, p.blob));
    }
    let _ = std::fs::remove_file(&path);
    println!(
        "COVERAGE ExperienceTracker: {ops_total} operations, {loads} loads, {forwarded} adds that reached a sink, \
         {PROGRAMS} programs, {changes_total} log words"
    );
    assert!(forwarded > 500, "the sink path was barely exercised ({forwarded})");
    check.finish();
}

#[test]
fn planted_unscaled_sink_forward_is_caught() {
    let path = temp_path("planted");
    let mut g = Gen::new(0x0602);
    let mut check = Check::new("ExperienceTracker (planted: sink gets unscaled gain)");
    let mut forwarded = 0usize;
    for _ in 0..PROGRAMS {
        let p = random_program(&mut g);
        let rust = rust_run(&p, true, &mut forwarded);
        let cpp = cpp_run(&p, &path);
        check.eq(rust, cpp, || format!("program ops {:?} rules {:?} rel {:?} blob {:?}", p.ops, p.rules, p.rel, p.blob));
    }
    let _ = std::fs::remove_file(&path);
    check.finish_planted();
}

#[test]
fn a_wrong_version_is_refused_before_the_level_is_read() {
    // a load with a wrong version is refused by both before the level is read
    let b = sanitise(vec![2, 9, 0, 0, 0]);
    assert_eq!(b, vec![2, 9, 0, 0, 0]);
    let mut t = ExperienceTracker::new(1);
    let mut load = XferLoad::new(b);
    load.open(b"x").unwrap();
    assert!(matches!(t.xfer(&mut load), Err(XferStatus::InvalidVersion)));
}
