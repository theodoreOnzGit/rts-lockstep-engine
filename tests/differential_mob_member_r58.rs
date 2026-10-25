#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MobMemberSlavedUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `MobMemberSlavedUpdate` (`logic::modules::mob_member_slaved_update`) against the unchanged C++, `p1w13-sonnet`, batch R58.
//!
//! # Method
//!
//! The C++ side is `MobMemberSlavedUpdate.cpp`, unchanged (its two file-level constants renamed by macro for the include: `SlavedUpdate.cpp`
//! has the same names in this one unit), compiled into `wrap_module.cpp`'s unit (`differential/l9_mob_member_r58.inc`, `ref_l9r58_run`), on
//! the module build's stand-ins: four objects (ids 1-4; positions; drawables; bodies whose `attemptDamage` is a line); the member (object 1:
//! maybe an AI with a locomotor, moving or not, a victim, a last command source, a next mood target, a goal and the distance to it; its
//! drawable's flags); the nexus and others (objects 2-4: maybe an AI, moving, idle, attacking, a victim, a goal and the distance to it; maybe
//! a spawn behaviour that may let members pick targets); the distances (PartitionManager.cpp's procedures); the AI's calls, the drawable's
//! flag clear, `setStatus` and `clearDisabled` (lines); the REAL `Object::kill`, logic random values (seeded alike), `UpdateModule`, `INI`
//! and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` AIs, bodies and
//! spawn behaviours), `reads.drawable_model_condition` for the member's flags, the object calls stubbed, the world's logic random values
//! (the constructor's draws are its world half); every op an `update()` as the scheduler runs it, `onEnslave`, `onSlaverDie`,
//! `onSlaverDamage`, `getSlaverID`, `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 120 ops. Compared, bit for bit: the read's throw; the
//! data; the constructor's state and `onObjectCreated`'s effects; per op the throw status, the result, the effects in order (the gait, the
//! moves, the attacks, the idle, the kills' damage, the flag clear), the xfer status, bytes (the scheduling word masked) and CRC, and the
//! state (the slaver, the countdown, the mob state, the colour, the primary victim, the squirrelliness, self-tasking, the crisis timer, the
//! member's `UNSELECTABLE` bit).
//!
//! # Results (2026-10-25, `L9R58_CASES=9000`)
//!
//! 9,000 cases (364 data blocks threw, as the C++ did; none undefined upstream), 526,115 ops and 543,387 outputs: **0 mismatches**, on the
//! first run. In the compared cases: 338,372 updates, 56,910 xfers and 381,338 effect lines: 99,614 kills (most of a member with no nexus,
//! killed on every update, as upstream does), 49,717 flag clears, 6,963 gait changes, 6,322 moves, 341 idles and 292 attack orders.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught, none by a crash of the C++.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2281 | the mob logic every 8 frames, not 16 | 246,674 |
//! | 2282 | a member already headed elsewhere not sent to the nexus's goal | 2,310 |
//! | 2283 | the crisis move after a half of the bail time, not a third | 8 |
//! | 2284 | the "normal" gait drawn as wandering | 173 |
//! | 2285 | the crisis timer not reset while the mob stands | 3,850 |
//! | 2286 | the nexus's last victim not attacked | 153 |
//! | 2287 | a freed member stays unselectable | 104,247 |
//! | 2288 | the crisis timer not saved | 43,983 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODELCONDITION_WEAPONSET_PLAYER_UPGRADE};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_COUNT, OBJECT_STATUS_UNSELECTABLE};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::mob_member_slaved_update::MobMemberSlavedUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r58.bin");
const XFER_FILE: &str = "l9_r58.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::MobMemberSlavedUpdate(Arc::new(MobMemberSlavedUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let data: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    let constructed: Vec<u32> = (0..11).map(|_| r.u32()).collect();
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data {data:x?} constructed {constructed:x?} created {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..11).map(|_| r.u32()).collect();
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
    field(g, 70, "MustCatchUpRadius", &["50", "20", "100", "5"]);
    field(g, 40, "NoNeedToCatchUpRadius", &["25", "10"]);
    field(g, 60, "Squirrelliness", &["0.5", "0", "1.5", "-0.25", "0.01"]);
    field(g, 60, "CatchUpCrisisBailTime", &["3", "10", "999999", "0", "1"]);
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

fn obj_choice(g: &mut Gen) -> u32 {
    if g.below(3) == 0 { 0 } else { 1 + g.below(4) }
}

fn gen_member(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    p(g, 93) | p(g, 93) << 1 | p(g, 40) << 2 | p(g, 90) << 3 | p(g, 30) << 4 | obj_choice(g) << 5 | *pick(g, &[&2u32, &2, &0, &1]) << 8 | obj_choice(g) << 12
}

fn gen_other(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    p(g, 90) | p(g, 50) << 1 | p(g, 30) << 2 | p(g, 40) << 3 | p(g, 90) << 4 | p(g, 75) << 5 | p(g, 60) << 6 | obj_choice(g) << 7
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &5.0, &12.5, &-20.0, &30.0, &60.0, &-100.0, &200.0, &0.25])).to_bits()
}

fn gen_dist(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &10.0, &50.0, &100.0])).to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 24];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = g.below(1 << 30);
    cfg[2] = gen_member(g);
    for c in cfg.iter_mut().take(6).skip(3) {
        *c = gen_other(g);
    }
    cfg[6] = gen_dist(g);
    cfg[7] = gen_dist(g);
    for c in cfg.iter_mut().take(20).skip(8) {
        *c = gen_coord(g);
    }
    // the member often beside one of the others (within the catch-up radius: the standing mob's targeting)
    if g.below(2) == 0 {
        let m = 1 + g.below(3) as usize;
        cfg[8] = cfg[8 + m];
        cfg[12] = cfg[12 + m];
    }
    let n_ops = 1 + g.below(120) as usize;
    let mut ops = Vec::new();
    if g.below(10) < 8 {
        ops.push([3, 2 + g.below(3), 0, 0, 0]);
    }
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=64 => [0, 0, 0, 0, 0],
            65..=67 => [1, gen_frame(g), 0, 0, 0],
            68..=80 => {
                let a = 2 + g.below(18);
                let b = match a {
                    2 => gen_member(g),
                    3..=5 => gen_other(g),
                    6 | 7 => gen_dist(g),
                    _ => gen_coord(g),
                };
                [2, a, b, 0, 0]
            }
            81..=82 => [3, *pick(g, &[&2u32, &3, &4, &0, &1]), 0, 0, 0],
            83 => [4, 0, 0, 0, 0],
            84..=85 => [5, g.below(12), (*pick(g, &[&10.0f32, &0.5])).to_bits(), 0, 0],
            86..=89 => [6, 0, 0, 0, 0],
            90..=94 => [7, 0, 0, 0, 0],
            95..=96 => [8, 0, 0, 0, 0],
            97 => [9, 0, 0, 0, 0],
            _ => [10, 0, 0, 0, 0],
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
    moves: u64,
    attacks: u64,
    kills: u64,
    gaits: u64,
    idles: u64,
    clears: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

/// The probe slots, by object: the AI, the body and the spawn behaviour.
struct Slots {
    ai: [u16; 4],
    body: [u16; 4],
}

fn obj(v: u32) -> ObjectId {
    if (1..=4).contains(&v) { v as ObjectId } else { INVALID_ID }
}

fn set_scene(w: &mut World, cfg: &[u32], s: &Slots) {
    let f = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.ai = (f & 1 != 0).then_some(s.ai[0]);
        o.drawable = if f >> 3 & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[0] as usize].body.as_mut() {
            p.answers.has_locomotor = f >> 1 & 1 != 0;
            p.answers.moving = f >> 2 & 1 != 0;
            p.answers.current_victim = obj(f >> 5 & 7);
            p.answers.last_command_source = (f >> 8 & 15) as i32;
            p.answers.mood_target = obj(f >> 12 & 7);
            p.answers.distance_to_goal = f32::from_bits(cfg[6]);
            p.answers.goal_position = Coord3D { x: f32::from_bits(cfg[16]), y: f32::from_bits(cfg[17]), z: 0.0 };
        }
    }
    let mut flags = ModelConditionFlags::default();
    if f >> 4 & 1 != 0 {
        flags.set(MODELCONDITION_WEAPONSET_PLAYER_UPGRADE, 1);
    }
    w.reads.drawable_model_condition.insert(1, flags);
    for k in 1..4usize {
        let g = cfg[2 + k];
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.ai = (g & 1 != 0).then_some(s.ai[k]);
        o.drawable = if g >> 4 & 1 != 0 { 7 + k as i32 } else { INVALID_DRAWABLE_ID };
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[k] as usize].body.as_mut() {
            p.answers.moving = g >> 1 & 1 != 0;
            p.answers.idle = g >> 2 & 1 != 0;
            p.answers.attacking = g >> 3 & 1 != 0;
            // the spawn behaviour is this probe too (the `Has...Interface` bit 3)
            p.answers.has = if g >> 5 & 1 != 0 { 1 << 3 } else { 0 };
            p.answers.may_self_task = g >> 6 & 1 != 0;
            p.answers.current_victim = obj(g >> 7 & 7);
            p.answers.distance_to_goal = f32::from_bits(cfg[7]);
            p.answers.goal_position = Coord3D { x: f32::from_bits(cfg[18]), y: f32::from_bits(cfg[19]), z: 0.0 };
        }
    }
    for k in 0..4usize {
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[8 + k]), y: f32::from_bits(cfg[12 + k]), z: 0.0 });
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, tally: &mut Tally) -> Vec<String> {
    let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::SetStatus { object, mask, set })) => {
                    let bit = (0..OBJECT_STATUS_COUNT).find(|&i| mask.test(i)).unwrap_or(-1);
                    log.push(format!("setstatus {object} {bit} {}", i32::from(set)))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            if let Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::Clear(mut v) }) = ev_iter.next() {
                tally.clears += 1;
                v.sort_unstable();
                log.push(format!("mc {object_id} clear {v:?}"));
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t.clone());
        } else if let Some(l) = t.strip_prefix("bt ") {
            if let Some(r) = l.strip_prefix("aiidlefrom ") {
                tally.idles += 1;
                log.push(format!("aiidle {}", r.split(' ').next().unwrap()));
            } else if l.starts_with("aimove ") {
                tally.moves += 1;
                log.push(l.to_string());
            } else if l.starts_with("aiattack ") {
                tally.attacks += 1;
                log.push(l.to_string());
            } else if l.starts_with("locoset ") {
                tally.gaits += 1;
                log.push(l.to_string());
            } else if l.starts_with("aiprone ") {
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
    let ModuleDataKind::MobMemberSlavedUpdate(d) = &data else { unreachable!() };
    put_i32(&mut v, d.must_catch_up_radius);
    put_i32(&mut v, d.no_need_to_catch_up_radius);
    put_u32(&mut v, d.squirrelliness_ratio.to_bits());
    put_u32(&mut v, d.catch_up_crisis_bail_time);

    let mut w = World::new();
    w.names = ctx.names;
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, ..Default::default() });
    w.random.init_game_logic_random(cfg[1]);
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let probe = |name: &[u8], interfaces: i32, max: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max, Default::default())));
        m
    };
    let mut slots = Slots { ai: [0; 4], body: [0; 4] };
    // (the member first: its constructor's draws are the first of the logic random values, as the driver's are)
    for k in 1..=4 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"MobMemberSlavedUpdate", b"ModuleTag_Mob", &data).unwrap().unwrap());
        }
        modules.push(probe(b"R58Ai", 0, 0.0));
        modules.push(probe(b"R58Body", MODULEINTERFACE_BODY, 200.0));
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
        slots.ai[k as usize - 1] = w.find_module(id, b"R58Ai").unwrap();
        slots.body[k as usize - 1] = w.find_module(id, b"R58Body").unwrap();
        let body = slots.body[k as usize - 1];
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[body as usize].body.as_mut() {
            p.health = 100.0;
        }
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"MobMemberSlavedUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, &slots);
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::MobMemberSlavedUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let mut s = m.state_words();
        s.push(u32::from(ob.status.test(OBJECT_STATUS_UNSELECTABLE)));
        s
    };
    // the state the constructor left: the object's creation also ran onObjectCreated (before the scene), so the squirrelliness is put back
    {
        let mut s = state(&w);
        s[7] = 0;
        for x in s {
            put_u32(&mut v, x);
        }
    }
    let events_before = w.events.events().len();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    let created = effects(&mut w, events_before, &mut t);
    put_u32(&mut v, created.len() as u32);
    for l in &created {
        put_str(&mut v, l.as_bytes());
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=23).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
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
                w.module_call(me, idx, ModuleCall::SlavedOnEnslave { slaver: obj(o[1]) });
            }
            4 => {
                w.module_call(me, idx, ModuleCall::SlavedOnSlaverDie);
            }
            5 => {
                let mut info = DamageInfo::default();
                info.input.damage_type = o[1] as i32;
                info.input.amount = f32::from_bits(o[2]);
                w.module_call(me, idx, ModuleCall::SlavedOnSlaverDamage(info));
            }
            6..=8 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 6, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 6 {
                    saved = xbytes.clone();
                }
                if o[0] == 7 {
                    w.load_post_process();
                }
                t.xfers += 1;
            }
            9 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
            }
            10 => {
                res = w.module_call(me, idx, ModuleCall::GetSlaverId).as_id() as u32;
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
fn mob_member_slaved_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R58 MobMemberSlavedUpdate",
        seed: 0x0058_30B0,
        cases_var: "L9R58_CASES",
        ini_stem: "Mm",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r58_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R58: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} moves, \
         {} attacks, {} kills, {} gait changes, {} idles, {} flag clears)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.moves, t.attacks, t.kills, t.gaits, t.idles, t.clears
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.moves > 0 && t.attacks > 0 && t.kills > 0 && t.gaits > 0 && t.idles > 0 && t.clears > 0, "the generator must reach every effect");
}
