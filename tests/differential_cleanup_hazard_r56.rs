#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CleanupHazardUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CleanupHazardUpdate` (`logic::modules::cleanup_hazard_update`) against the unchanged C++, `p1w13-sonnet`, batch R56.
//!
//! # Method
//!
//! The C++ side is `CleanupHazardUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_cleanup_hazard_r56.inc`,
//! `ref_l9r56_run`), on the module build's stand-ins: five objects (ids 1-5, x and y the test's); the unit (object 1: maybe an AI whose
//! `isIdle`, `isBusy` and last command source are the test's and whose `aiBusy`, `aiMoveToPosition` and `aiAttackObject` are lines; maybe a
//! weapon in each slot, whose template's attack range is the test's); the closest hazard (the test's; the call a line); the distances;
//! `setWeaponLock` and `setWeaponSetFlag` (lines); the REAL logic random values (seeded alike on both sides), `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` AI), the weapon
//! store's two templates with the test's attack ranges, `cuts.weapon_slot_template` for the unit's weapons, `cuts.closest_object_at`, and the
//! world's logic random values; every op an `update()` as the scheduler runs it, a `setCleanupAreaParameters`
//! (`ModuleCall::SetCleanupAreaParameters`), `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data; `onObjectCreated`'s effects and the state; per op the throw status, the result, the effects in order (the weapon set flag, the scans,
//! the distances to a position, the lock, the AI's busy, move and attack orders), the xfer status, bytes (the scheduling word masked) and CRC,
//! and the state (the target, in range, the two countdowns, whether the weapon template was found, the area's centre and move range).
//!
//! # Results (2026-10-25, `L9R56_CASES=9000`)
//!
//! 9,000 cases (415 data blocks threw, as the C++ did; 1,277 excluded, undefined upstream: a unit with no weapon in its `WeaponSlot` that
//! tracks a target, reading the attack range of a `NULL` template), 222,016 ops and 236,632 outputs: **0 mismatches**. In the compared cases:
//! 122,006 updates, 26,646 xfers and 202,292 effect lines: 70,167 scans, 45,864 attack orders, 16,397 moves and 14,455 busy orders.
//!
//! On the way: the first run differed in 5,240 outputs, all from the test's scene, not the port: the C++ stand-in's slot weapon carried its
//! template inside it, so the template the module had cached changed its range when the scene gave the slot another weapon, where the Rust
//! side's cached template (a store position) kept its own. The driver now has one weapon per slot and template, as the game has one template
//! per weapon kind.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught, none by a crash of the C++.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2261 | a new command from a player does not abandon the cleanup area | 700 |
//! | 2262 | the area given up within 2.5, not 25 | 281 |
//! | 2263 | a target that left the range kept | 10,846 |
//! | 2264 | the attack ordered whatever the AI is doing | 21,432 |
//! | 2265 | the area scanned without its move range | 15,047 |
//! | 2266 | the area's centre not saved | 25,981 |
//! | 2267 | the re-scan delay drawn from 0 to 1, not 0 to 3 | 6,526 |
//! | 2268 | an idle AI not kept busy | 14,455 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::cleanup_hazard_update::CleanupHazardUpdateModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r56.bin");
const XFER_FILE: &str = "l9_r56.bin";
/// `FROM_CENTER_2D`.
const FROM_CENTER_2D: i32 = 0;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::CleanupHazardUpdate(Arc::new(CleanupHazardUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..3).map(|_| r.u32()).collect();
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    let state: Vec<u32> = (0..9).map(|_| r.u32()).collect();
    out.push(format!("data {data:x?} created {created:?} state {state:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..9).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = if !clean && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 70, "WeaponSlot", &["PRIMARY", "SECONDARY", "TERTIARY", "PRIMARY"]);
    field(g, 80, "ScanRate", &["0", "33", "100", "330"]);
    field(g, 85, "ScanRange", &["50", "100", "200", "20.5"]);
    if !clean && g.below(20) == 0 {
        lines.push("Bogus = 1".to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_ai(g: &mut Gen) -> u32 {
    u32::from(g.below(100) < 85) | u32::from(g.below(2) == 0) << 1 | u32::from(g.below(3) == 0) << 2 | *pick(g, &[&2u32, &2, &0, &1]) << 4
}

fn gen_weapons(g: &mut Gen) -> u32 {
    (0..3).fold(0, |acc, s| acc | (if g.below(5) == 0 { 0 } else { 1 + g.below(2) }) << (4 * s))
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &10.0, &-15.5, &24.0, &30.0, &60.0, &120.0, &-80.0])).to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 16];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = g.below(1 << 30);
    cfg[2] = gen_ai(g);
    cfg[3] = gen_weapons(g);
    cfg[4] = (*pick(g, &[&20.0f32, &45.0, &80.0])).to_bits();
    cfg[5] = (*pick(g, &[&10.0f32, &35.0, &150.0])).to_bits();
    cfg[6] = g.below(7);
    for c in cfg.iter_mut().take(15).skip(7) {
        *c = gen_coord(g);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=54 => [0, 0, 0, 0, 0],
            55..=58 => [1, gen_frame(g), 0, 0, 0],
            59..=79 => {
                let a = 2 + g.below(13);
                let b = match a {
                    2 => gen_ai(g),
                    3 => gen_weapons(g),
                    4 | 5 => (*pick(g, &[&20.0f32, &45.0, &80.0, &10.0])).to_bits(),
                    6 => g.below(7),
                    _ => gen_coord(g),
                };
                [2, a, b, 0, 0]
            }
            80..=85 => [3, gen_coord(g), gen_coord(g), (*pick(g, &[&0.0f32, &50.0, &100.0, &-5.0])).to_bits(), 0],
            86..=90 => [4, 0, 0, 0, 0],
            91..=95 => [5, 0, 0, 0, 0],
            96..=97 => [6, 0, 0, 0, 0],
            _ => [7, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    effects: u64,
    scans: u64,
    attacks: u64,
    moves: u64,
    busies: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32], ai_slot: u16) {
    let a = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.ai = (a & 1 != 0).then_some(ai_slot);
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[ai_slot as usize].body.as_mut() {
            p.answers.idle = a >> 1 & 1 != 0;
            p.answers.busy = a >> 2 & 1 != 0;
            p.answers.last_command_source = (a >> 4 & 15) as i32;
        }
    }
    for s in 0..3 {
        let v = cfg[3] >> (4 * s) & 15;
        if v != 0 {
            w.cuts.weapon_slot_template.insert((1, s), ((v - 1) % 2) as usize);
        } else {
            w.cuts.weapon_slot_template.remove(&(1, s));
        }
    }
    w.cuts.closest_object_at = if (1..=5).contains(&cfg[6]) { cfg[6] as ObjectId } else { 0 };
    for k in 0..5usize {
        let y = if k < 3 { f32::from_bits(cfg[12 + k]) } else { 0.0 };
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[7 + k]), y, z: 0.0 });
    }
}

/// The weapon store's two templates, their attack ranges the configuration's.
fn set_ranges(w: &mut World, cfg: &[u32]) {
    let statics = Arc::make_mut(&mut w.statics);
    for k in 0..2 {
        statics.weapon_store.templates_mut()[k].attack_range = f32::from_bits(cfg[4 + k]);
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::WeaponSetUpdate { .. }) => log.push("wset".to_string()),
                Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => log.push(format!("weaponlock {object} {slot} {lock}")),
                Some(CutCall::PartitionGetClosestCleanupHazardAt { position: p, range, .. }) => {
                    tally.scans += 1;
                    log.push(format!("closestat {:08x} {:08x} {:08x} {:08x} {FROM_CENTER_2D}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::Contain(ContainCall::PartitionGetDistanceSquared { object, position: p })) => {
                    log.push(format!("cdist {object} {:08x} {:08x} {:08x}", b(p.x), b(p.y), b(p.z)))
                }
                _ => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("aiattack ") {
                tally.attacks += 1;
                log.push(l.to_string());
            } else if l.starts_with("aimove ") {
                tally.moves += 1;
                log.push(l.to_string());
            } else if l.starts_with("aibusy ") {
                tally.busies += 1;
                log.push(l.to_string());
            }
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::CleanupHazardUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.weapon_slot as u32);
    put_u32(&mut v, d.scan_frames);
    put_u32(&mut v, d.scan_range.to_bits());

    let mut w = World::new();
    w.names = ctx.names;
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for n in ["WEAP_A", "WEAP_B"] {
        weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap().unwrap();
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store, ..Default::default() });
    set_ranges(&mut w, &cfg);
    w.random.init_game_logic_random(cfg[1]);
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    for k in 1..=5 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"CleanupHazardUpdate", b"ModuleTag_Cleanup", &data).unwrap().unwrap());
            let mut ai = NewModule::plain(b"R56Ai", b"ModuleTag_Ai");
            ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
            modules.push(ai);
        }
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"CleanupHazardUpdate").unwrap();
    let ai_slot = w.find_module(me, b"R56Ai").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    // the object's creation ran onObjectCreated before the scene was set: the C++ driver runs it after, so it is run (and logged) here
    set_scene(&mut w, &cfg, ai_slot);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    let created = effects(&mut w, &mut t);
    put_u32(&mut v, created.len() as u32);
    for l in &created {
        put_str(&mut v, l.as_bytes());
    }
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::CleanupHazardUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    for s in state(&w) {
        put_u32(&mut v, s);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=14).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_ranges(&mut w, &cfg);
        set_scene(&mut w, &cfg, ai_slot);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let loc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: 0.0 };
                w.module_call(me, idx, ModuleCall::SetCleanupAreaParameters { loc, range: f32::from_bits(o[3]) });
            }
            4..=6 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 4, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                if o[0] == 5 {
                    w.load_post_process();
                }
                t.xfers += 1;
            }
            7 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t);
        t.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        for s in state(&w) {
            put_u32(&mut v, s);
        }
    }
    drop(t);
    let excluded = match w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
        Some((_, e)) => {
            let reason = format!("{e}");
            let mut t = tally.borrow_mut();
            *t = tally_before;
            *t.excluded.entry(reason).or_insert(0) += 1;
            true
        }
        None => false,
    };
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn cleanup_hazard_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R56 CleanupHazardUpdate",
        seed: 0x0056_C1EA,
        cases_var: "L9R56_CASES",
        ini_stem: "Ch",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r56_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R56: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} scans, \
         {} attacks, {} moves, {} busies)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.scans, t.attacks, t.moves, t.busies
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.scans > 0 && t.attacks > 0 && t.moves > 0 && t.busies > 0, "the generator must reach every effect");
}
