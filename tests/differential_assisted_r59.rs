#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AssistedTargetingUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `AssistedTargetingUpdate` (`logic::modules::assisted_targeting_update`) against the unchanged C++, `p1w13-sonnet`, batch R59.
//!
//! # Method
//!
//! The C++ side is `AssistedTargetingUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_assisted_r59.inc`,
//! `ref_l9r59_run`), on the module build's stand-ins: three objects (the assisting unit, the requesting object and the victim; positions);
//! the unit (maybe an AI, whose `aiAttackObject` is a line; `NO_ATTACK` or not, which is all `isAbleToAttack`'s stand-in reads; maybe a
//! current weapon with the test's status; a team, each team its player's default team); the thing factory (`LaserA`, with a `LaserUpdate`
//! client module, and `LaserB`, without; `newObject` makes laser objects 4, 5, ... with drawables); the laser's `initLaser` (a line);
//! `setWeaponLock` and `destroyObject` (lines); the REAL `UpdateModule`, `INI` and `Xfer`. After a load the driver runs `loadPostProcess`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the unit `KINDOF_CAN_ATTACK`, so the real `isAbleToAttack` turns on the
//! status bit alone), the thing factory's two templates (`LaserA` with a `LaserUpdate` client module), three teams and players with their
//! default teams, `cuts.cur_weapon_status`; the lasers are made by the real `World::thing_factory_new_object`. Every op an `update()` as the
//! scheduler runs it, `isFreeToAssist`, `assistAttack`, a change to the scene or the frame, or an xfer (a load followed by the world's
//! post-load pass and the module's world-aware one).
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 40 ops. Compared, bit for bit: the read's throw; the
//! data and the constructor's state; per op the throw status, the result, the effects in order (the lock, the attack, the lasers, the
//! destroyed lasers), the xfer status, bytes (the scheduling word masked) and CRC, the templates found and the number of lasers made.
//!
//! # Results (2026-10-25, `L9R59_CASES=9000`)
//!
//! 9,000 cases (279 data blocks threw, as the C++ did; 1,525 excluded, each an `assistAttack` with a null requesting object or victim that
//! upstream dereferences, `Undefined::NullObject`, where the C++ crashes), 139,049 ops and 153,441 outputs: **0 mismatches**, once the
//! null dereferences were marked (the first run's single mismatch was the C++ crashing on one). In the compared cases: 25,484 updates,
//! 26,728 xfers and 73,708 effect lines: 8,687 free to assist, 26,495 attack orders, 11,830 lasers and 8,888 lasers destroyed (`LaserB`
//! has no `LaserUpdate`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2291 | the to-target laser looked up by its own name (upstream uses the from name for both) | 227 |
//! | 2292 | `isFreeToAssist` ignores `isAbleToAttack` | 1,354 |
//! | 2293 | the attack's max shots 1, not the clip size | 13,864 |
//! | 2294 | the from-assisted laser drawn from the unit to the requester | 5,915 |
//! | 2295 | every laser destroyed | 30 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_CAN_ATTACK;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NO_ATTACK;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ClientModuleData, ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::assisted_targeting_update::AssistedTargetingUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r59.bin");
const XFER_FILE: &str = "l9_r59.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::AssistedTargetingUpdate(Arc::new(AssistedTargetingUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let data: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    let (from, to) = (r.str(), r.str());
    let constructed: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    out.push(format!("data {data:x?} {from:?} {to:?} constructed {constructed:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..3).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 70, "AssistingClipSize", &["1", "3", "0", "10"], false);
    field(g, 60, "AssistingWeaponSlot", &["PRIMARY", "SECONDARY", "TERTIARY"], false);
    field(g, 85, "LaserFromAssisted", &["LaserA", "LaserB", "LaserA", "NoSuchLaser"], true);
    field(g, 70, "LaserToTarget", &["LaserA", "LaserB", "NoSuchLaser"], true);
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

fn gen_unit(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let status = if g.below(8) == 0 { 7 } else { *pick(g, &[&0u32, &0, &0, &1, &2, &3, &4]) };
    p(g, 85) | p(g, 15) << 1 | status << 2 | (if g.below(10) == 0 { 0 } else { 1 + g.below(3) }) << 5
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &5.0, &12.5, &-20.0, &30.0, &60.0])).to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 8];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = gen_unit(g);
    for c in cfg.iter_mut().skip(2) {
        *c = gen_coord(g);
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::new();
    // mostly the update first (it finds the templates)
    if g.below(10) < 8 {
        ops.push([0, 0, 0, 0, 0]);
    }
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=14 => [0, 0, 0, 0, 0],
            15..=19 => [1, gen_frame(g), 0, 0, 0],
            20..=34 => {
                let a = 1 + g.below(7);
                [2, a, if a == 1 { gen_unit(g) } else { gen_coord(g) }, 0, 0]
            }
            35..=54 => [3, 0, 0, 0, 0],
            55..=79 => {
                let obj = |g: &mut Gen, a: u32, b: u32| if g.below(16) == 0 { 0 } else if g.below(4) == 0 { b } else { a };
                [4, obj(g, 2, 3), obj(g, 3, 2), 0, 0]
            }
            80..=87 => [5, 0, 0, 0, 0],
            88..=95 => [6, 0, 0, 0, 0],
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
    free: u64,
    attacks: u64,
    lasers: u64,
    destroyed: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32], ai_slot: u16) {
    let f = cfg[1];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.ai = (f & 1 != 0).then_some(ai_slot);
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_NO_ATTACK, (f >> 1 & 1) as i32);
        o.team = f >> 5 & 3;
    }
    let ws = f >> 2 & 7;
    if ws <= 4 {
        w.cuts.cur_weapon_status.insert(1, ws as i32);
    } else {
        w.cuts.cur_weapon_status.remove(&1);
    }
    for k in 0..3usize {
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[2 + k]), y: f32::from_bits(cfg[5 + k]), z: 0.0 });
    }
}

/// The C++ driver's template code: 0 `NULL`, 1 `LaserA`, 2 `LaserB`.
fn code(id: u32, ids: &[u16; 2]) -> u32 {
    if id == u32::MAX {
        0
    } else if id == u32::from(ids[0]) {
        1
    } else {
        2
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, tally: &mut Tally) -> Vec<String> {
    let events: Vec<ClientEvent> = w.events.events()[events_before..].to_vec();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => log.push(format!("weaponlock {object} {slot} {lock}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    tally.destroyed += 1;
                    log.push(format!("destroy {object}"))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            if let Some(ClientEvent::Object(ObjectClientEvent::LaserInit { laser, parent, target, start: s, end: e, bone })) = ev_iter.next() {
                tally.lasers += 1;
                log.push(format!(
                    "laser {laser} {parent} {target} {},{},{} {},{},{} {}",
                    s.x.to_bits(),
                    s.y.to_bits(),
                    s.z.to_bits(),
                    e.x.to_bits(),
                    e.y.to_bits(),
                    e.z.to_bits(),
                    String::from_utf8_lossy(&bone)
                ));
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("aiattack ") {
                tally.attacks += 1;
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
    let ModuleDataKind::AssistedTargetingUpdate(d) = &data else { unreachable!() };
    put_i32(&mut v, d.clip_size);
    put_u32(&mut v, d.weapon_slot as u32);
    put_str(&mut v, &d.laser_from_assisted_name);
    put_str(&mut v, &d.laser_to_target_name);

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    factory.new_template(b"LaserA", 0).expect("a template");
    factory.new_template(b"LaserB", 0).expect("a template");
    factory.friend_find_template_mut(b"LaserA").unwrap().data.friend_client_update_module_info_mut().add_module_info(
        b"LaserUpdate",
        b"ModuleTag_Laser",
        ModuleInfoData::Client(ClientModuleData::default()),
        0,
        false,
        false,
    );
    let ids = [factory.find_template(b"LaserA").unwrap().data.get_template_id(), factory.find_template(b"LaserB").unwrap().data.get_template_id()];
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let unit = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { kind_of: vec![KINDOF_CAN_ATTACK], ..Default::default() }));
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    for k in 1..=3 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"AssistedTargetingUpdate", b"ModuleTag_Assist", &data).unwrap().unwrap());
            let mut ai = NewModule::plain(b"R59Ai", b"ModuleTag_Ai");
            ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
            modules.push(ai);
        }
        let template = if k == 1 { unit.clone() } else { plain.clone() };
        let id = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"AssistedTargetingUpdate").unwrap();
    let ai_slot = w.find_module(me, b"R59Ai").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, ai_slot);
    let state = |w: &World, lasers: usize| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::AssistedTargetingUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let s = m.state_words();
        vec![code(s[0], &ids), code(s[1], &ids), lasers as u32]
    };
    for x in &state(&w, 0)[..2] {
        put_u32(&mut v, *x);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=7).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, ai_slot);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                res = u32::from(w.module_call(me, idx, ModuleCall::AssistedTargetingIsFreeToAssist).as_bool());
                t.free += u64::from(res);
            }
            4 => {
                let obj = |v: u32| if (1..=3).contains(&v) { v as ObjectId } else { INVALID_ID };
                w.module_call(me, idx, ModuleCall::AssistedTargetingAssistAttack { requesting: obj(o[1]), victim: obj(o[2]) });
            }
            5..=7 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 5 {
                    saved = xbytes.clone();
                }
                if o[0] == 6 {
                    w.load_post_process();
                    w.modules_load_post_process(me);
                }
                t.xfers += 1;
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let log = effects(&mut w, events_before, &mut t);
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
        let lasers = w.objects.ids_in_order().into_iter().filter(|&id| id > 3).count();
        for s in state(&w, lasers) {
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
fn assisted_targeting_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R59 AssistedTargetingUpdate",
        seed: 0x0059_A551,
        cases_var: "L9R59_CASES",
        ini_stem: "At",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r59_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R59: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} free to \
         assist, {} attacks, {} lasers, {} lasers destroyed)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.free, t.attacks, t.lasers, t.destroyed
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.free > 0 && t.attacks > 0 && t.lasers > 0 && t.destroyed > 0, "the generator must reach every effect");
}
