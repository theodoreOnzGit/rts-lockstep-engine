#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DemoTrapUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `DemoTrapUpdate` (`logic::modules::demo_trap_update`) against the unchanged C++, `p1w13-sonnet`, batch R55.
//!
//! # Method
//!
//! The C++ side is `DemoTrapUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_demo_trap_r55.inc`,
//! `ref_l9r55_run`), on the module build's stand-ins: five objects (ids 1-5; kinds, the status bits, effectively dead, a team of three, maybe a
//! current weapon (its slot and damage type), x the test's (y 0, z 0 or 5 over flat terrain), a body whose `attemptDamage` is a line); the
//! teams' relationships; the scan (the objects the test lists; the call a line); the distance in the plane; `setWeaponLock` and
//! `setWeaponSetFlag` (lines); the weapon store (`WEAP_A`, `WEAP_B`; `createAndFireTempWeapon` a line); the REAL `Object::kill`,
//! `Object::getRelationship`, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` bodies), three
//! teams and players, `cuts.team_relationships`, `cuts.range_scan`, `cuts.cur_weapon` (the slot) and `cuts.contain.cur_weapon_damage_type`
//! (the damage type) for the current weapons, and the weapon store; every op an `update()` as the scheduler runs it, a `detonate()`
//! (`ModuleCall::Detonate`), `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data (the ignored kinds, the three slots, the range, the weapon, the scan rate, the three flags); `onObjectCreated`'s effects; per op the
//! throw status, the result, the effects in order (the weapon set flag, the lock, the scan, the detonation weapon, the kill's damage), the
//! xfer status, bytes (the scheduling word masked) and CRC, and the state (the scan countdown, the detonated flag).
//!
//! # Results (2026-10-25, `L9R55_CASES=9000`)
//!
//! 9,000 cases (928 data blocks threw, as the C++ did; 417 excluded, undefined upstream: a trap with no current weapon, whose slot `update`
//! reads through `NULL`), 234,454 ops and 249,764 outputs: **0 mismatches**, on the first run. In the compared cases: 128,492 updates,
//! 35,411 xfers and 40,116 effect lines: 14,720 weapon locks, 15,211 scans, 2,876 detonation weapons fired and 7,899 kills.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught, none by a crash of the C++.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2251 | the manual slot locked by default, the proximity slot otherwise | 13,019 |
//! | 2252 | the manual mode scans | 855 |
//! | 2253 | the scan countdown not reset | 36,785 |
//! | 2254 | the ignored kinds not ignored | 1,368 |
//! | 2255 | a dozer disarming the trap not spared | 154 |
//! | 2256 | a friend in range does not stop the scan | 401 |
//! | 2257 | an airborne enemy detonates the trap | 199 |
//! | 2258 | the distance compared with the range, not its square | 1,858 |
//! | 2259 | the trap not killed when it detonates | 7,899 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::{DAMAGE_DISARM, DAMAGE_EXPLOSION};
use rts_lockstep_engine::logic::modules::demo_trap_update::DemoTrapUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::{CutCall, WeaponFacts};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r55.bin");
const XFER_FILE: &str = "l9_r55.bin";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The kinds the test gives an object, by bit (the driver's `g_r55Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_DOZER, KINDOF_INFANTRY, KINDOF_VEHICLE];
/// The status bits, by bit (the driver's `g_r55Status`).
const STATUS: [ObjectStatusTypes; 3] = [OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD, OBJECT_STATUS_IS_ATTACKING];
/// `FROM_CENTER_2D`.
const FROM_CENTER_2D: i32 = 0;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::DemoTrapUpdate(Arc::new(DemoTrapUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R55Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let mut kinds = Vec::new();
    loop {
        let k = r.u32();
        if k == u32::MAX {
            break;
        }
        kinds.push(k);
    }
    let data: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data ignore {kinds:?} {data:x?} created {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..2).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 60, "DefaultProximityMode", &["Yes", "No"], false);
    // mostly three distinct slots, as the data must have them
    let slots = *pick(g, &[&["TERTIARY", "PRIMARY", "SECONDARY"], &["PRIMARY", "SECONDARY", "TERTIARY"], &["SECONDARY", "TERTIARY", "PRIMARY"], &["PRIMARY", "PRIMARY", "SECONDARY"]]);
    field(g, 85, "DetonationWeaponSlot", &[slots[0]], false);
    field(g, 85, "ProximityModeWeaponSlot", &[slots[1]], false);
    field(g, 85, "ManualModeWeaponSlot", &[slots[2]], false);
    field(g, 85, "TriggerDetonationRange", &["10", "25.5", "40", "0"], false);
    field(g, 50, "IgnoreTargetTypes", &["INFANTRY", "VEHICLE", "INFANTRY VEHICLE", "DOZER"], false);
    field(g, 75, "ScanRate", &["0", "33", "100", "330"], false);
    field(g, 50, "AutoDetonationWithFriendsInvolved", &["Yes", "No"], false);
    field(g, 75, "DetonationWeapon", &["WEAP_A", "WEAP_B", "None", "NoSuchWeapon"], true);
    field(g, 50, "DetonateWhenKilled", &["Yes", "No"], false);
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

/// An object's word: its kinds, status bits (rarely built or sold), effectively dead (rarely), its team, its current weapon (mostly one), the
/// weapon's damage type, above the terrain (sometimes).
fn gen_object(g: &mut Gen, k: usize) -> u32 {
    let kinds = (0..3).fold(0, |acc, b| acc | u32::from(g.below(4) == 0) << b);
    let status = u32::from(g.below(25) == 0) << 3 | u32::from(g.below(25) == 0) << 4 | u32::from(g.below(3) == 0) << 5;
    let dead = u32::from(g.below(if k == 0 { 20 } else { 8 }) == 0) << 6;
    let team = g.below(4) << 7;
    let weapon = if g.below(20) == 0 { 0 } else { 1 + g.below(3) } << 9;
    let disarm = u32::from(g.below(3) == 0) << 11;
    let above = u32::from(g.below(6) == 0) << 12;
    kinds | status | dead | team | weapon | disarm | above
}

fn gen_rels(g: &mut Gen) -> u32 {
    (0..9).fold(0, |acc, i| acc | g.below(3) << (2 * i))
}

fn gen_scan(g: &mut Gen) -> u32 {
    (0..5).fold(0, |acc, k| acc | (1 + g.below(5)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(5)))) - 1)
}

fn gen_x(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &5.0, &12.5, &-20.0, &30.0, &39.75, &60.0])).to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 16];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    for k in 0..5 {
        cfg[1 + k] = gen_object(g, k);
        cfg[8 + k] = gen_x(g);
    }
    cfg[6] = gen_rels(g);
    cfg[7] = gen_scan(g);
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=54 => [0, 0, 0, 0, 0],
            55..=58 => [1, gen_frame(g), 0, 0, 0],
            59..=79 => {
                let a = 1 + g.below(12);
                let b = match a {
                    1..=5 => gen_object(g, a as usize - 1),
                    6 => gen_rels(g),
                    7 => gen_scan(g),
                    _ => gen_x(g),
                };
                [2, a, b, 0, 0]
            }
            80..=81 => [3, 0, 0, 0, 0],
            82..=87 => [4, 0, 0, 0, 0],
            88..=93 => [5, 0, 0, 0, 0],
            94..=96 => [6, 0, 0, 0, 0],
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
    detonations: u64,
    kills: u64,
    locks: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    for k in 0..5usize {
        let id = k as ObjectId + 1;
        let f = cfg[1 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(template_of(f & 7));
            o.status = Default::default();
            for (b, s) in STATUS.iter().enumerate() {
                if f >> (3 + b) & 1 != 0 {
                    o.status.set(*s, 1);
                }
            }
            if f >> 6 & 1 != 0 {
                o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
            } else {
                o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
            }
            o.team = f >> 7 & 3;
        }
        let weapon = f >> 9 & 3;
        if weapon != 0 {
            w.cuts.cur_weapon.insert(id, WeaponFacts { slot: weapon as i32 - 1, last_shot_frame: 0, possible_next_shot_frame: 0 });
            w.cuts.contain.cur_weapon_damage_type.insert(id, if f >> 11 & 1 != 0 { DAMAGE_DISARM } else { DAMAGE_EXPLOSION });
        } else {
            w.cuts.cur_weapon.remove(&id);
            w.cuts.contain.cur_weapon_damage_type.remove(&id);
        }
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[8 + k]), y: 0.0, z: if f >> 12 & 1 != 0 { 5.0 } else { 0.0 } });
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[6] >> (2 * (3 * a + b)) & 3));
        }
    }
    let mut scan = Vec::new();
    for k in 0..5 {
        let id = cfg[7] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 5 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
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
                Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => {
                    tally.locks += 1;
                    log.push(format!("weaponlock {object} {slot} {lock}"))
                }
                Some(CutCall::PartitionIterateObjectsInRange { object, range }) => {
                    tally.scans += 1;
                    let p = w.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0 {FROM_CENTER_2D}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) => {
                    tally.detonations += 1;
                    log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), b(p.x), b(p.y), b(p.z)))
                }
                _ => {}
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t.clone());
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::DemoTrapUpdate(d) = &data else { unreachable!() };
    for k in 0..KINDOF_COUNT {
        if d.ignore_kind_of.test(k) {
            put_u32(&mut v, k as u32);
        }
    }
    put_u32(&mut v, u32::MAX);
    for x in [d.manual_mode_weapon_slot, d.detonation_weapon_slot, d.proximity_mode_weapon_slot] {
        put_u32(&mut v, x as u32);
    }
    put_u32(&mut v, d.trigger_detonation_range.to_bits());
    let weapon_code = d.detonation_weapon_template.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |i| i as i32 + 1);
    put_i32(&mut v, weapon_code);
    put_u32(&mut v, d.scan_frames);
    put_u32(&mut v, u32::from(d.defaults_to_proximity_mode) | u32::from(d.friendly_detonation) << 1 | u32::from(d.detonate_when_killed) << 2);

    let mut w = World::new();
    w.names = ctx.names;
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for n in WEAPONS {
        weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap().unwrap();
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let body = || {
        let mut m = NewModule::plain(b"R55Body", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        m
    };
    for k in 1..=5 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"DemoTrapUpdate", b"ModuleTag_Trap", &data).unwrap().unwrap());
        }
        modules.push(body());
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"DemoTrapUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    // the object's creation ran onObjectCreated before the scene was set: the C++ driver runs it after, so it is run (and logged) here
    set_scene(&mut w, &cfg);
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
        let Some(UpdateBody::DemoTrapUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=12).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg);
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
                w.module_call(me, idx, ModuleCall::Detonate);
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
fn demo_trap_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R55 DemoTrapUpdate",
        seed: 0x0055_DE70,
        cases_var: "L9R55_CASES",
        ini_stem: "Dt",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r55_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R55: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} locks, \
         {} scans, {} detonation weapons, {} kills)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.locks, t.scans, t.detonations, t.kills
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.scans > 0 && t.detonations > 0 && t.kills > 0 && t.locks > 0, "the generator must reach every effect");
}
