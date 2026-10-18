// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the object-lifetime part of `GameLogic`
//! (`logic::world`) against upstream's own text (commit 0a05454d).
//!
//! # What is compared against what
//!
//! `build.rs` cuts these pieces **verbatim** out of `GameLogic.cpp`,
//! `GameLogic.h` and `Object.cpp` and compiles them (see
//! `differential/wrap_game_logic.cpp`): the sleepy-update heap
//! (`validateSleepyUpdate` to `friend_awakenUpdateModule`: erase, rebalance up
//! and down, remake, push, peek, pop, awaken), `processDestroyList`,
//! `allocateObjectID`, the id lookup table, `registerObject`,
//! `findObjectByID`, the module loop of `update()`, `destroyAllObjectsImmediate`,
//! `loadPostProcess`, and `Object::isInList`/`prependToList`/`removeFromList`.
//! The classes they touch (Object, UpdateModule, the GameLogic members) are
//! minimal stand-ins; `destroyObject` and the frame wrapper around the module
//! loop are hand-written stand-ins (the real ones need the AI, the pathfinder,
//! the script engine and the partition manager; the file comment of the wrapper
//! says exactly which statements). The Rust side is `World` with `ProbeUpdate`
//! modules running the same scripts.
//!
//! # Method
//!
//! A **case** is a set of update scripts (steps of: sleep time, and actions
//! destroy / wake another module / create an object), a set of object groups
//! (modules with a phase, an initial wake frame and a script), and a list of
//! operations: create an object, run 1-4 logic frames, `destroyObject`,
//! `friend_awakenUpdateModule`, `processDestroyList`, disable or enable an
//! object, `loadPostProcess`, `destroyAllObjectsImmediate` (+ id reset), hold
//! the frame at 0 (`m_startNewGame`), set the frame (a save load). The same
//! case runs on both sides. Compared, word for word:
//!
//! 1. **update calls**: every module update made, in order, with the frame,
//!    the module and its key (`friend_getPriority()`) at the call. This is the
//!    update order of the simulation, tie order included.
//! 2. **deletions**: every `friend_deleteInstance`, in order.
//! 3. **snapshots**, after every operation and after every frame: the frame,
//!    `m_nextObjID`, the lookup-table size, the object list newest first, the
//!    destroy list, and the whole heap array with each entry's key and its
//!    `m_indexInLogic`.
//!
//! Ties in the key (same frame, same phase) are the point: scripts use short
//! sleeps and the groups share phases and wake frames, so many modules are
//! due together, and the heap array decides who goes first.
//!
//! **Planted bug.** A second check runs pure-schedule cases (no actions)
//! against a deliberately wrong Rust scheduler, a stable sort by key that
//! breaks ties by registration order, and must fail: it shows that the inputs
//! can tell upstream's heap order from a plausible cleaner one.
//!
//! # Results
//!
//! 2026-10-18, upstream 0a05454d, `--test differential_world`, seed 0x5701,
//! 3000 cases: **72,519 update calls** (34,840 of them with the same frame and
//! key as the previous call, so decided by heap order alone), **3,974
//! deletions**, **80,462 snapshots** (each the full heap array, object list,
//! destroy list, id counter and table size); **0 mismatches** in each category
//! and the same number of records in every case. Planted bug: the
//! stable-sort scheduler differs from upstream in 29,792 of 45,446 compared
//! words (400 pure-schedule cases), so the inputs separate the two.
//!
//! Not covered here (Rust unit tests instead): `destroyObject`'s hook order,
//! the error returns (`RELEASE_CRASH` paths cannot be reached through a
//! coherent program), `popSleepyUpdate` (upstream never calls it), and an id of
//! 0 (see the wrapper).

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::disabled_types::DISABLED_HELD;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::module::update_module::SleepyUpdatePhase;
use rts_lockstep_engine::logic::world::sleepy::UpdateHandle;
use rts_lockstep_engine::logic::world::update_body::{probe_object, ProbeAction, ProbeModuleSpec, ProbeStep};
use rts_lockstep_engine::logic::world::{World, WorldError};
use support::{Check, Gen};

/// Cases per run.
const CASES: usize = 3000;

#[derive(Clone, Debug)]
enum Act {
    Destroy(i32),
    Awaken { obj: i32, idx: i32, delay: u32 },
    Spawn(i32),
}

#[derive(Clone, Debug)]
struct Step {
    sleep: u32,
    actions: Vec<Act>,
}

#[derive(Clone, Debug)]
struct Mod {
    phase: i32,
    initial: u32,
    script: i32,
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Spawn(i32),
    Frames(i32),
    Destroy(i32),
    Awaken { obj: i32, idx: i32, when: u32 },
    ProcessDestroyList,
    Disable { obj: i32, on: bool },
    LoadPostProcess,
    DestroyAll,
    StartNewGame(bool),
    SetFrame(u32),
}

#[derive(Clone, Debug)]
struct Case {
    scripts: Vec<Vec<Step>>,
    groups: Vec<Vec<Mod>>,
    ops: Vec<Op>,
}

fn gen_sleep(g: &mut Gen) -> u32 {
    match g.below(14) {
        0..=5 => 1,
        6 | 7 => 2,
        8 => 3,
        9 => 5,
        10 => 0,
        11 => 40,
        12 => [u32::MAX, 0x3fff_ffff, 0x7fff_ffff, 0x8000_0000][g.below(4) as usize],
        _ => 1 + g.below(9),
    }
}

/// The delay of a `setWakeFrame` made from inside an update: at least 1 (see `ProbeAction::Awaken`).
fn gen_delay(g: &mut Gen) -> u32 {
    match g.below(8) {
        0..=5 => 1 + g.below(9),
        6 => 0x3fff_ffff,
        _ => 0x4000_0000,
    }
}

fn gen_when(g: &mut Gen) -> u32 {
    match g.below(8) {
        0..=4 => g.below(14),
        5 => 0x3fff_ffff,
        6 => u32::MAX,
        _ => 0x4000_0000,
    }
}

fn gen_case(g: &mut Gen, pure: bool) -> Case {
    let n_groups = 1 + g.below(5) as i32;
    let n_scripts = 1 + g.below(6) as i32;
    // The groups first. A `ProbeAction::Spawn` holds its group's specs by value, so the spawn
    // graph must be acyclic: script `s` may only spawn a group whose scripts all have an index above `s`.
    let mut groups = Vec::new();
    for _ in 0..n_groups {
        let mut mods = Vec::new();
        for _ in 0..g.below(5) {
            let initial = match g.below(8) {
                0..=3 => 0,
                4 => 1,
                5 => 2,
                6 => 5 + g.below(10),
                _ => [0x3fff_ffff, u32::MAX][g.below(2) as usize],
            };
            mods.push(Mod { phase: g.below(4) as i32, initial, script: g.below(n_scripts as u32) as i32 });
        }
        groups.push(mods);
    }
    let mut scripts = Vec::new();
    for s in 0..n_scripts {
        let spawnable: Vec<i32> = (0..n_groups).filter(|&j| groups[j as usize].iter().all(|m| m.script > s)).collect();
        let mut steps = Vec::new();
        for _ in 0..1 + g.below(4) {
            let mut sleep = gen_sleep(g);
            let mut actions = Vec::new();
            if !pure {
                for _ in 0..g.below(3) {
                    match g.below(20) {
                        0..=5 => actions.push(Act::Destroy(g.below(12) as i32)),
                        6..=15 => actions.push(Act::Awaken { obj: 1 + g.below(10) as i32, idx: g.below(4) as i32, delay: gen_delay(g) }),
                        16 | 17 if !spawnable.is_empty() => {
                            // spawning is rare and followed by a long sleep, so the population stays small
                            actions.push(Act::Spawn(spawnable[g.below(spawnable.len() as u32) as usize]));
                            sleep = sleep.clamp(6, 60);
                        }
                        _ => actions.push(Act::Destroy(1 + g.below(10) as i32)),
                    }
                }
            }
            steps.push(Step { sleep, actions });
        }
        scripts.push(steps);
    }
    let mut ops = Vec::new();
    let n_ops = 6 + g.below(26);
    for _ in 0..n_ops {
        let op = if pure {
            match g.below(10) {
                0..=4 => Op::Spawn(g.below(n_groups as u32) as i32),
                _ => Op::Frames(1 + g.below(4) as i32),
            }
        } else {
            match g.below(100) {
                0..=29 => Op::Spawn(g.below(n_groups as u32) as i32),
                30..=59 => Op::Frames(1 + g.below(4) as i32),
                60..=69 => Op::Destroy(g.below(14) as i32),
                70..=79 => Op::Awaken { obj: g.below(12) as i32, idx: g.below(5) as i32, when: gen_when(g) },
                80..=84 => Op::ProcessDestroyList,
                85..=89 => Op::Disable { obj: 1 + g.below(10) as i32, on: g.below(2) == 0 },
                90..=92 => Op::LoadPostProcess,
                93 => Op::DestroyAll,
                94 | 95 => Op::StartNewGame(g.below(2) == 0),
                _ => Op::SetFrame(g.below(20)),
            }
        };
        ops.push(op);
    }
    Case { scripts, groups, ops }
}

/// The program `ref_world_run` reads.
fn encode(c: &Case) -> Vec<i32> {
    let mut p = vec![c.scripts.len() as i32];
    for s in &c.scripts {
        p.push(s.len() as i32);
        for st in s {
            p.push(st.sleep as i32);
            p.push(st.actions.len() as i32);
            for a in &st.actions {
                match *a {
                    Act::Destroy(id) => p.extend([0, id, 0, 0]),
                    Act::Awaken { obj, idx, delay } => p.extend([1, obj, idx, delay as i32]),
                    Act::Spawn(gr) => p.extend([2, gr, 0, 0]),
                }
            }
        }
    }
    p.push(c.groups.len() as i32);
    for gr in &c.groups {
        p.push(gr.len() as i32);
        for m in gr {
            p.extend([m.phase, m.initial as i32, m.script]);
        }
    }
    p.push(c.ops.len() as i32);
    for op in &c.ops {
        let (code, a, b, cc) = match *op {
            Op::Spawn(gr) => (0, gr, 0, 0),
            Op::Frames(n) => (1, n, 0, 0),
            Op::Destroy(id) => (2, id, 0, 0),
            Op::Awaken { obj, idx, when } => (3, obj, idx, when as i32),
            Op::ProcessDestroyList => (4, 0, 0, 0),
            Op::Disable { obj, on } => (5, obj, on as i32, 0),
            Op::LoadPostProcess => (6, 0, 0, 0),
            Op::DestroyAll => (7, 0, 0, 0),
            Op::StartNewGame(on) => (8, on as i32, 0, 0),
            Op::SetFrame(f) => (9, f as i32, 0, 0),
        };
        p.extend([code, a, b, cc]);
    }
    p
}

fn phase_of(i: i32) -> SleepyUpdatePhase {
    SleepyUpdatePhase::from_low_bits(i as u32)
}

fn module_specs(c: &Case, group: i32) -> Vec<ProbeModuleSpec> {
    c.groups[group as usize]
        .iter()
        .map(|m| ProbeModuleSpec { phase: phase_of(m.phase), initial_call_frame: m.initial, script: probe_script(c, m.script) })
        .collect()
}

fn probe_script(c: &Case, script: i32) -> Vec<ProbeStep> {
    c.scripts[script as usize]
        .iter()
        .map(|st| ProbeStep {
            sleep: st.sleep,
            actions: st
                .actions
                .iter()
                .map(|a| match *a {
                    Act::Destroy(id) => ProbeAction::Destroy(id),
                    Act::Awaken { obj, idx, delay } => ProbeAction::Awaken { target: UpdateHandle { object: obj, index: idx as u16 }, delay },
                    Act::Spawn(gr) => ProbeAction::Spawn(module_specs(c, gr)),
                })
                .collect(),
        })
        .collect()
}

/// One record of the word stream: (kind, words).
type Record = (u32, Vec<u32>);

/// Splits a stream into records: kind 1 has 5 words, kind 3 has 2, kind 2 is
/// variable (see the wrapper's comment).
fn parse(words: &[u32]) -> Vec<Record> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let kind = words[i];
        let len = match kind {
            1 => 5,
            3 => 2,
            2 => {
                // 2, op, frame, nextId, tableSize, nList, ids.., nPending, ids.., nHeap, 4*nHeap.., crash
                let n_list = words[i + 5] as usize;
                let p = i + 6 + n_list;
                let n_pend = words[p] as usize;
                let h = p + 1 + n_pend;
                let n_heap = words[h] as usize;
                h + 1 + 4 * n_heap + 1 - i
            }
            k => panic!("bad record kind {k} at {i}"),
        };
        out.push((kind, words[i..i + len].to_vec()));
        i += len;
    }
    out
}

fn snapshot(w: &World, op: usize, crash: u32, out: &mut Vec<u32>) {
    out.extend([2, op as u32, w.frame(), w.next_object_id() as u32, w.objects.lookup_len() as u32]);
    let ids = w.objects.ids_in_order();
    out.push(ids.len() as u32);
    out.extend(ids.iter().map(|&i| i as u32));
    out.push(w.objects_to_destroy().len() as u32);
    out.extend(w.objects_to_destroy().iter().map(|&i| i as u32));
    out.push(w.sched.len() as u32);
    for u in w.sched.heap() {
        let m = w.objects.update_module(*u).expect("heap entry");
        out.extend([u.object as u32, u32::from(u.index), m.friend_get_priority(), m.friend_get_index_in_logic() as u32]);
    }
    out.push(crash);
}

fn collect(w: &mut World, out: &mut Vec<u32>) {
    for e in std::mem::take(w.sched.trace.as_mut().expect("tracing on")) {
        out.extend([1, e.frame, e.handle.object as u32, u32::from(e.handle.index), e.priority]);
    }
    for ev in w.take_client_events() {
        if let ClientEvent::ObjectDestroyed { object_id } = ev {
            out.extend([3, object_id as u32]);
        }
    }
}

fn run_rust(c: &Case) -> Vec<u32> {
    let mut w = World::new();
    w.sched.trace = Some(Vec::new());
    let mut out = Vec::new();
    let mut crash = 0u32;
    for (i, op) in c.ops.iter().enumerate() {
        match *op {
            Op::Spawn(gr) => {
                let id = w.allocate_object_id();
                // id 0 (after a load of an empty world) is refused here; the wrapper does not make that object
                let r = w.register_object(probe_object(id, &module_specs(c, gr)));
                assert!(r.is_ok() || id == 0, "{r:?}");
            }
            Op::Frames(n) => {
                for _ in 0..n {
                    assert!(w.run_logic_frame());
                    collect(&mut w, &mut out);
                    snapshot(&w, i, crash, &mut out);
                }
                continue;
            }
            Op::Destroy(id) => w.destroy_object(id),
            Op::Awaken { obj, idx, when } => {
                if let Some(o) = w.find_object(obj) {
                    if idx >= 0 && (idx as usize) < o.behaviors.len() {
                        let u = UpdateHandle { object: obj, index: idx as u16 };
                        match w.awaken_update_module(obj, u, when) {
                            Ok(()) => {}
                            Err(WorldError::SleepyIndexIllegal(_)) => crash = 1,
                            Err(_) => crash = 2,
                        }
                    }
                }
            }
            Op::ProcessDestroyList => w.process_destroy_list(),
            Op::Disable { obj, on } => {
                if let Some(o) = w.find_object_mut(obj) {
                    if on {
                        o.disabled.set(DISABLED_HELD, 1);
                        // p1w6-object: `checkDisabledStatus` (end of frame, now translated) clears a type whose
                        // frame has come; `setDisabled` sets FOREVER, and so does this stand-in for it
                        o.disabled_till_frame[DISABLED_HELD as usize] = rts_lockstep_engine::common::game_common::FOREVER;
                    } else {
                        o.disabled.clear();
                    }
                }
            }
            Op::LoadPostProcess => w.load_post_process(),
            Op::DestroyAll => w.reset(),
            Op::StartNewGame(on) => w.set_start_new_game(on),
            Op::SetFrame(f) => w.set_frame(f),
        }
        collect(&mut w, &mut out);
        snapshot(&w, i, crash, &mut out);
    }
    out
}

/// The C++ wrapper keeps its program and `TheGameLogic` in globals, as upstream does.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run_cpp(c: &Case) -> Vec<u32> {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prog = encode(c);
    let mut cap = 1 << 16;
    loop {
        let mut out = vec![0u32; cap];
        let n = unsafe { cpp::ref_world_run(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), cap as i32) };
        if n >= 0 {
            out.truncate(n as usize);
            return out;
        }
        cap *= 4;
        assert!(cap < 1 << 28, "output too large");
    }
}

#[test]
fn world_cases_match_cpp() {
    let mut g = Gen::new(0x5701);
    let mut updates = Check::new("GameLogic update calls (order, frame, key)");
    let mut deletions = Check::new("GameLogic object deletions (order)");
    let mut snaps = Check::new("GameLogic snapshots (list, queue, heap, ids)");
    let mut lengths = Check::new("GameLogic records per case");
    let (mut n_updates, mut n_del, mut n_snap, mut ties) = (0u64, 0u64, 0u64, 0u64);
    let cases = std::env::var("DIFF_WORLD_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(CASES);
    for i in 0..cases {
        let c = gen_case(&mut g, false);
        if std::env::var("DIFF_WORLD_DUMP").ok().and_then(|v| v.parse::<usize>().ok()) == Some(i) {
            eprintln!("{c:#?}");
        }
        let rs = parse(&run_rust(&c));
        let cs = parse(&run_cpp(&c));
        lengths.eq(rs.len(), cs.len(), || format!("case {i}: {c:#?}"));
        if rs.len() != cs.len() {
            continue;
        }
        let mut last_key: Option<(u32, u32)> = None;
        for (k, (r, cp)) in rs.iter().zip(cs.iter()).enumerate() {
            let check = match r.0 {
                1 => {
                    n_updates += 1;
                    // two updates in one frame with the same key: decided by heap order
                    if last_key == Some((r.1[1], r.1[4])) {
                        ties += 1;
                    }
                    last_key = Some((r.1[1], r.1[4]));
                    &mut updates
                }
                3 => {
                    n_del += 1;
                    &mut deletions
                }
                _ => {
                    n_snap += 1;
                    last_key = None;
                    &mut snaps
                }
            };
            check.eq(&r.1, &cp.1, || format!("case {i} record {k}: {c:#?}"));
        }
    }
    println!("DIFF world cases {CASES}: update calls {n_updates} (of which {ties} tie with the previous call), deletions {n_del}, snapshots {n_snap}");
    assert!(ties > 1000, "the cases must exercise equal keys");
    lengths.finish();
    updates.finish();
    deletions.finish();
    snaps.finish();
}

/// The wrong scheduler of the planted-bug check: modules in registration
/// order, each frame a stable pick of the smallest key. Pure-schedule cases
/// only (no actions; ops are spawn and frames).
fn naive_updates(c: &Case) -> Vec<Record> {
    struct M {
        obj: u32,
        idx: u32,
        phase: u32,
        key: u32,
        script: i32,
        calls: u32,
    }
    let mut mods: Vec<M> = Vec::new();
    let (mut frame, mut next_id) = (0u32, 1u32);
    let mut out = Vec::new();
    for op in &c.ops {
        match *op {
            Op::Spawn(gr) => {
                let obj = next_id;
                next_id += 1;
                for (idx, m) in c.groups[gr as usize].iter().enumerate() {
                    let now = frame.max(1);
                    let when = if m.initial == 0 { now } else { m.initial };
                    mods.push(M { obj, idx: idx as u32, phase: m.phase as u32, key: (when.min(0x3fff_ffff) << 2) | m.phase as u32, script: m.script, calls: 0 });
                }
            }
            Op::Frames(n) => {
                for _ in 0..n {
                    loop {
                        let pick = (0..mods.len()).filter(|&k| mods[k].key >> 2 <= frame).min_by_key(|&k| (mods[k].key, k));
                        let Some(k) = pick else { break };
                        let m = &mut mods[k];
                        out.push((1, vec![1, frame, m.obj, m.idx, m.key]));
                        let steps = &c.scripts[m.script as usize];
                        let st = &steps[m.calls as usize % steps.len()];
                        m.calls += 1;
                        let sleep = if (st.sleep as i32) < 1 { 1 } else { st.sleep };
                        m.key = (frame.wrapping_add(sleep).min(0x3fff_ffff) << 2) | m.phase;
                    }
                    frame += 1;
                }
            }
            _ => unreachable!("pure cases only"),
        }
    }
    out
}

#[test]
fn a_stable_sort_scheduler_is_caught() {
    let mut g = Gen::new(0x5702);
    let mut planted = Check::new("planted: stable-sort scheduler vs upstream's heap");
    for i in 0..400 {
        let c = gen_case(&mut g, true);
        let upstream: Vec<Record> = parse(&run_cpp(&c)).into_iter().filter(|r| r.0 == 1).collect();
        let wrong = naive_updates(&c);
        planted.eq(wrong.len(), upstream.len(), || format!("case {i}: number of calls"));
        if wrong.len() == upstream.len() {
            for (k, (a, b)) in wrong.iter().zip(upstream.iter()).enumerate() {
                planted.eq(&a.1, &b.1, || format!("case {i} call {k}"));
            }
        }
    }
    planted.finish_planted();
}
