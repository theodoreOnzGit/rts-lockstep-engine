#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PointDefenseLaserUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PointDefenseLaserUpdate` (`logic::modules::point_defense_laser_update`) against the unchanged C++, `p1w13-sonnet`, batch R66.
//!
//! # Method
//!
//! The C++ side is `PointDefenseLaserUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_point_defense_r66.inc`,
//! `ref_l9r66_run`), on the module build's stand-ins: six objects (the laser and five others: kinds, status bits, effectively dead, a team of
//! three, a position), the teams' relationships, the scan (the objects the test lists; the call a line), the distance in the plane, the
//! weapon store (`WEAP_A`, `WEAP_B`: the attack range, the delays between shots and the anti mask the test gives, read through
//! `Weapon.cpp`'s `getAttackRange` and `getDelayBetweenShots`), the stand-in `Weapon` with `Weapon.cpp`'s own constructor, whose
//! `loadAmmoNow` and `fireWeapon` are lines; the REAL `Object::getRelationship`, `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with the six objects (their kinds through their templates), teams, `cuts.team_relationships`,
//! `cuts.range_scan`, the logic random values and the weapon store's two templates; the temporary weapon is made from the store
//! (`Weapon::new_in_store`) and its two calls are records (`cuts.stub_weapon_calls`). Every op an `update()` as the scheduler runs it,
//! `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw;
//! the data; the constructor's next call frame; per op the throw status, the result, the effects in order (the scans, the reloads and the
//! shots), the xfer status, bytes (the scheduling word masked) and CRC, and the state (the best target, whether it is in range, the frames
//! to the next scan and the next shot).
//!
//! # Results (2026-10-25, `L9R66_CASES=9000`)
//!
//! 9,000 cases (677 data blocks threw, as the C++ did; 690 excluded, each a laser whose `WeaponTemplate` is `NULL`, which the scan reads
//! through), 231,969 ops and 247,235 outputs: **0 mismatches**. In the compared cases: 139,361 updates, 79,097 scans, 1,359 shots and
//! 30,409 xfers. The first run differed only in the test's own set-up (the driver did not apply the constructor's wake call).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2361 | a tracked target shot out of range | 56,262 |
//! | 2362 | no delay after a shot | 6,546 |
//! | 2363 | a dead target does not force a rescan | 762 |
//! | 2364 | the secondary target types ignored | 17,466 |
//! | 2365 | friends targeted | 28,876 |
//! | 2366 | a scan every frame | 119,206 |
//! | 2367 | the frames to the next shot not saved | 12,846 |

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
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::point_defense_laser_update::PointDefenseLaserUpdateModuleData;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r66.bin");
const XFER_FILE: &str = "l9_r66.bin";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The kinds the test gives an object, by bit (the driver's `g_r66Kinds`), and their names.
const KINDS: [KindOfType; 5] = [KINDOF_SMALL_MISSILE, KINDOF_BALLISTIC_MISSILE, KINDOF_AIRCRAFT, KINDOF_IMMOBILE, KINDOF_INFANTRY];
const KIND_NAMES: [&str; 5] = ["SMALL_MISSILE", "BALLISTIC_MISSILE", "AIRCRAFT", "IMMOBILE", "INFANTRY"];
/// The status bits, by bit (the driver's `g_r66Status`).
const STATUS: [ObjectStatusTypes; 4] = [OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::PointDefenseLaserUpdate(Arc::new(PointDefenseLaserUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let mut values = ObjectTemplateValues { name: b"R66Thing".to_vec(), ..Default::default() };
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
    let code = r.u32();
    let mut lists = Vec::new();
    for _ in 0..2 {
        let mut kinds = Vec::new();
        loop {
            let k = r.u32();
            if k == u32::MAX {
                break;
            }
            kinds.push(k);
        }
        lists.push(kinds);
    }
    let data: Vec<u32> = (0..3).map(|_| r.u32()).collect();
    let next = r.u32();
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data weapon {code} kinds {lists:?} {data:x?} next {next} created {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:?}"));
    }
    out
}

fn gen_kinds(g: &mut Gen) -> String {
    let n = 1 + g.below(2);
    (0..n).map(|_| KIND_NAMES[g.below(KIND_NAMES.len() as u32) as usize]).collect::<Vec<_>>().join(" ")
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 95 {
        let name = if g.below(25) == 0 { pick(g, &["None", "NoSuchWeapon"]).to_string() } else { pick(g, &["WEAP_A", "WEAP_A", "WEAP_B", "weap_b"]).to_string() };
        lines.push(format!("WeaponTemplate = {name}"));
    }
    if g.below(100) < 85 {
        let k = gen_kinds(g);
        lines.push(format!("PrimaryTargetTypes = {k}"));
    }
    if g.below(100) < 60 {
        let k = gen_kinds(g);
        lines.push(format!("SecondaryTargetTypes = {k}"));
    }
    if g.below(100) < 80 {
        lines.push(format!("ScanRate = {}", pick(g, &["0", "33", "100", "200"])));
    }
    if g.below(100) < 85 {
        lines.push(format!("ScanRange = {}", pick(g, &["100", "250", "60.5", "0"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("PredictTargetVelocityFactor = {}", pick(g, &["0", "1", "10.5"])));
    }
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "ScanRate = soon", "PrimaryTargetTypes = NOT_A_KIND"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_object(g: &mut Gen, me: bool) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let kinds = if me { 0 } else { (0..5).fold(0u32, |m, b| m | p(g, if b < 2 { 40 } else { 15 }) << b) };
    let status = p(g, if me { 10 } else { 75 }) | p(g, 10) << 1 | p(g, 50) << 2 | p(g, 30) << 3;
    let team = if g.below(15) == 0 { 0 } else { 1 + g.below(3) };
    kinds | status << 5 | p(g, if me { 3 } else { 10 }) << 9 | team << 10
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &10.0, &25.0, &-40.0, &60.0, &90.0, &-150.0, &300.0])).to_bits()
}

fn gen_scan(g: &mut Gen) -> u32 {
    let n = g.below(6);
    let mut w = 0u32;
    for k in 0..n {
        w |= (1 + g.below(6)) << (4 * k);
    }
    w
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        2..=7 => gen_object(g, a == 2),
        // (mostly every team the laser's enemy)
        8 => (0..9).fold(0u32, |m, i| m | (if g.below(3) == 0 { g.below(3) } else { 0 }) << (2 * i)),
        9 => gen_scan(g),
        _ => gen_coord(g),
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &100]);
    cfg[1] = g.u32();
    for (a, c) in cfg.iter_mut().enumerate().take(22).skip(2) {
        *c = gen_cfg_word(g, a as u32);
    }
    for k in 0..2 {
        cfg[22 + 4 * k] = (*pick(g, &[&50.0f32, &100.0, &2.0, &0.0])).to_bits();
        let min = *pick(g, &[&0u32, &3, &10]);
        cfg[23 + 4 * k] = min;
        cfg[24 + 4 * k] = if g.below(2) == 0 { min } else { min + g.below(20) };
        cfg[25 + 4 * k] = *pick(g, &[&0x02u32, &0x04, &0x06, &0x01]);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=59 => [0, 0, 0, 0, 0],
            60..=64 => [1, gen_frame(g), 0, 0, 0],
            65..=84 => {
                let a = 2 + g.below(20);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            85..=89 => [3, 0, 0, 0, 0],
            90..=94 => [4, 0, 0, 0, 0],
            95..=97 => [5, 0, 0, 0, 0],
            _ => [6, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    scans: u64,
    shots: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    for k in 0..6usize {
        let id = k as ObjectId + 1;
        let f = cfg[2 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(template_of(f & 31));
            o.status = Default::default();
            for (b, s) in STATUS.iter().enumerate() {
                if f >> (5 + b) & 1 != 0 {
                    o.status.set(*s, 1);
                }
            }
            if f >> 9 & 1 != 0 {
                o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
            } else {
                o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
            }
            o.team = f >> 10 & 3;
        }
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[10 + k]), y: f32::from_bits(cfg[16 + k]), z: 0.0 });
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[8] >> (2 * (3 * a + b)) & 3));
        }
    }
    let mut scan = Vec::new();
    for k in 0..6 {
        let id = cfg[9] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 6 {
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
                Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, dc }) => {
                    tally.scans += 1;
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0 {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::WeaponLoadAmmoNow { source, template }) => log.push(format!("loadammo {source} {}", WEAPONS[template])),
                Some(CutCall::WeaponFireAtObject { source, target, template }) => {
                    tally.shots += 1;
                    log.push(format!("fire {source} {target} {}", WEAPONS[template]))
                }
                _ => {}
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
    let ModuleDataKind::PointDefenseLaserUpdate(d) = &data else { unreachable!() };
    let code = d.weapon_template.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |i| i as u32 + 1);
    put_u32(&mut v, code);
    for m in [&d.primary_target_kind_of, &d.secondary_target_kind_of] {
        for k in 0..KINDOF_COUNT {
            if m.test(k) {
                put_u32(&mut v, k as u32);
            }
        }
        put_u32(&mut v, u32::MAX);
    }
    put_u32(&mut v, d.scan_frames);
    put_u32(&mut v, d.scan_range.to_bits());
    put_u32(&mut v, d.velocity_factor.to_bits());

    let mut w = World::new();
    w.names = ctx.names;
    let mut store = WeaponStore::new();
    for (k, name) in WEAPONS.iter().enumerate() {
        let i = store.new_weapon_template(&mut w.names, name.as_bytes()).unwrap().unwrap();
        let tp = &mut store.templates_mut()[i];
        tp.attack_range = f32::from_bits(cfg[22 + 4 * k]);
        tp.min_delay_between_shots = cfg[23 + 4 * k] as i32;
        tp.max_delay_between_shots = cfg[24 + 4 * k] as i32;
        tp.anti_mask = cfg[25 + 4 * k];
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store: store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.stub_weapon_calls = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    for k in 1..=6 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PointDefenseLaserUpdate", b"ModuleTag_Laser", &data).unwrap().unwrap());
        }
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"PointDefenseLaserUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    put_u32(&mut v, w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame());
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
        let Some(UpdateBody::PointDefenseLaserUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=21).contains(&o[1]) {
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
            3..=5 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    w.load_post_process();
                }
                t.xfers += 1;
            }
            6 => {
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
fn point_defense_laser_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R66 PointDefenseLaserUpdate",
        seed: 0x0066_9DEF,
        cases_var: "L9R66_CASES",
        ini_stem: "Pd",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r66_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R66: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} scans, {} shots, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.scans, t.shots, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.shots > 0 && t.scans > 0, "the generator must reach every effect");
}
