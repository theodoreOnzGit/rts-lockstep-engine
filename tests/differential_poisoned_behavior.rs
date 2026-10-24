#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PoisonedBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PoisonedBehavior` (`logic::modules::poisoned_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R16.
//!
//! # Method
//!
//! The C++ side is `PoisonedBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_poisoned_r16.inc`) on the flammable
//! driver's stand-ins: an Object with a drawable (its tint calls recorded) and maybe a body (it logs the damage it is given and keeps the damage FX
//! override of the last), the game logic whose frame the test sets and whose wake recorder counts the calls `setWakeFrame` makes, and the REAL
//! `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the object (the module from `modules::new_module`, a `ProbeModule` body), the damage through the
//! verified `World::attempt_damage`, every op an `update()` as the scheduler runs it, a `World::module_call` or a change to the scene.
//!
//! Each case reads a block `D` (`PoisonDamageInterval`, `PoisonDuration`), mostly well formed, and runs up to 40 ops. Compared, bit for bit: the
//! read's throw; the data; per op the throw status, the sleep time, the damage lines, the tint calls, the wake calls and the frame of the last, the
//! xfer status, bytes and CRC, the module's four state words, the body's health and the damage FX override of the last damage.
//!
//! # Results (2026-10-23, `L9R16P_CASES=9000`)
//!
//! 9,000 cases (none excluded; 397 data files threw, as the C++ did), 175,172 ops (62,616 updates, 27,726 xfers) and 200,981 op outputs:
//! **0 mismatches**; 10,286 poison damages and 34,873 tint calls compared.
//!
//! **Translation corrected by the test:** the first run had 34,873 mismatches, every tint call: `TINT_STATUS_POISONED` was written as `0x2`
//! (the irradiated bit); `Drawable.h` line 245 has `0x4`. No harness correction was needed.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1811 | a second dose resets the pending damage frame | 5,159 |
//! | 1812 | the tint not cleared at the cure | 16,207 |
//! | 1813 | the repeated damage without the poison's damage FX | 85,337 |
//! | 1814 | the death type not saved | 16,475 |
//! | 1815 | healing does not cure | 22,078 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::poisoned_behavior::PoisonedBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r16p.bin");
const XFER_FILE: &str = "l9_r16p.bin";
/// A million points of health, as float bits.
const BIG: u32 = 0x4974_2400;
/// `DAMAGE_POISON`.
const POISON: u32 = 9;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::PoisonedBehavior(Arc::new(PoisonedBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::PoisonedBehavior(d) = data else { unreachable!("not a PoisonedBehavior") };
    put_u32(v, d.poison_damage_interval_data);
    put_u32(v, d.poison_duration_data);
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    out.push(format!("data {f:?}"));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut res) = (0, 0);
        if i > 0 {
            st = r.u32() as i32;
            res = r.u32();
        }
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let nt = r.u32();
        let tints: Vec<(u32, u32)> = (0..nt).map(|_| (r.u32(), r.u32())).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let mut x = String::new();
        if i > 0 {
            x = r.xfer();
        }
        let state: Vec<u32> = (0..6).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} tints {tints:?} wakes {wakes} last {last} {x} state {state:?}"));
    }
    out
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=7 => pick(g, &["100", "300", "500", "1000", "3000", "33", "34", "67", "200"]).to_string(),
        8 => format!("{}", g.below(20_000)),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => "1000".to_string(),
    }
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_dur(g, clean);
    field(g, 80, "PoisonDamageInterval", v);
    let v = gen_dur(g, clean);
    field(g, 80, "PoisonDuration", v);
    if !clean && g.below(25) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(30) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the health, the maximum, the body is present]
    let cfg = vec![gen_frame(g), BIG, BIG, u32::from(g.below(10) != 0)];
    let n_ops = 1 + g.below(30) as usize;
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // most cases start poisoned, then run a few frames
    if g.below(100) < 60 {
        ops.push([1, POISON, 12.5f32.to_bits(), 5, 0]);
        for _ in 0..1 + g.below(6) {
            frame = frame.wrapping_add(1 + g.below(40));
            ops.push([4, frame, 0, 0, 0]);
            ops.push([0, 0, 0, 0, 0]);
        }
    }
    ops.extend((0..n_ops).map(|_| match g.below(100) {
        0..=29 => [0, 0, 0, 0, 0],
        30..=44 => {
            let ty = if g.below(3) != 0 { POISON } else { g.below(30) };
            let amount = pick(g, &[&0.0f32, &5.0, &10.0, &25.0, &-4.0]).to_owned();
            [1, ty, amount.to_bits(), 1 + g.below(10), 0]
        }
        45..=50 => [2, 0, 0, 0, 0],
        51..=70 => {
            frame = if g.below(8) == 0 { gen_frame(g) } else { frame.wrapping_add(1 + g.below(60)) };
            [4, frame, 0, 0, 0]
        }
        71..=75 => [5, u32::from(g.below(3) == 0), 0, 0, 0],
        76..=83 => [6, 0, 0, 0, 0],
        84..=91 => [7, 0, 0, 0, 0],
        92..=96 => [8, 0, 0, 0, 0],
        _ => [0, 0, 0, 0, 0],
    }));
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    damages: u64,
    tints: u64,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(0);
    let mut modules = Vec::new();
    if cfg[3] != 0 {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), f32::from_bits(cfg[2]), Default::default())));
        modules.push(m);
    }
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PoisonedBehavior", b"ModuleTag_Poison", &data).unwrap().unwrap());
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let idx = w.find_module(me, b"PoisonedBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let constructor_trace = w.cuts.trace.take().unwrap_or_default();
    // a new object has the drawable the client made for it: the test binds one, as batch E does
    w.friend_bind_to_drawable(me, 7);
    let mut saved: Vec<u8> = Vec::new();
    let mut first_trace = Some(constructor_trace);
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let errors_before = w.cuts.module_errors.len();
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    let mut info = DamageInfo::default();
                    info.input.damage_type = o[1] as i32;
                    info.out.actual_damage_dealt = f32::from_bits(o[2]);
                    info.input.death_type = o[3] as i32;
                    w.module_call(me, idx, ModuleCall::OnDamage(info));
                }
                2 => {
                    w.module_call(me, idx, ModuleCall::OnHealing(DamageInfo::default()));
                }
                4 => w.set_frame(o[1]),
                5 => {
                    let ob = w.objects.find_mut(me).unwrap();
                    if o[1] != 0 {
                        ob.private_status |= PRIVATE_EFFECTIVELY_DEAD;
                    } else {
                        ob.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
                    }
                }
                6..=8 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 6, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 6 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        } else {
            events_before = w.events.events().len();
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let tints: Vec<(u32, u32)> = events
            .iter()
            .filter_map(|e| match e {
                ObjectClientEvent::DrawableTint { status, on, .. } => Some((*status as u32, u32::from(*on))),
                _ => None,
            })
            .collect();
        tally.borrow_mut().tints += tints.len() as u64;
        let trace = if phase == 0 { first_trace.take().unwrap_or_default() } else { w.cuts.trace.take().unwrap() };
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                // the wake calls of the module under test count, not the helpers'
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if phase > 0 && t.starts_with("damage ") {
                tally.borrow_mut().damages += 1;
                log.push(t);
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        if phase > 0 {
            put_i32(&mut v, if threw { 2 } else { 0 });
            put_u32(&mut v, if threw { 0 } else { res });
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let tints = if phase == 0 { Vec::new() } else { tints };
        put_u32(&mut v, tints.len() as u32);
        for (s, on) in &tints {
            put_u32(&mut v, *s);
            put_u32(&mut v, *on);
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, if wakes > 0 { last } else { 0 });
        if phase > 0 {
            put_i32(&mut v, xst);
            put_u32(&mut v, xbytes.len() as u32);
            for b in &xbytes {
                put_u32(&mut v, u32::from(*b));
            }
            put_u32(&mut v, crc);
        }
        let state = w.objects.find(me).unwrap().behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::PoisonedBehavior(m)) => Some(m.state_words()),
            _ => None,
        });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        let health = w.objects.find_mut(me).unwrap().modules.iter_mut().find_map(|b| match b.body.as_mut() {
            Some(UpdateBody::ModuleProbe(p)) if b.name == b"ProbeBody" => Some(p.health),
            _ => None,
        });
        put_u32(&mut v, health.map_or(0, f32::to_bits));
        // the body's record: only damage that reached a body
        let last_fx = if cfg[3] != 0 { w.objects.find(me).unwrap().damage_received.last().map_or(-1, |d| d.input.damage_fx_override) } else { -1 };
        put_i32(&mut v, last_fx);
    }
    let excluded = w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined());
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn poisoned_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "PoisonedBehavior R16",
        seed: 0x0016_9015,
        cases_var: "L9R16P_CASES",
        ini_stem: "R16p",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r16p_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "PoisonedBehavior R16: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} damages, {} tints",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.damages, t.tints
    );
    assert!(t.damages * 4 > c.cases as u64 && t.tints * 4 > c.cases as u64, "the generator must reach every path");
}
