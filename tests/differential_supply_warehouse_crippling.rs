#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SupplyWarehouseCripplingBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SupplyWarehouseCripplingBehavior` (`logic::modules::supply_warehouse_crippling_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R17.
//!
//! # Method
//!
//! The C++ side is `SupplyWarehouseCripplingBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crippling_r17.inc`) on
//! the flammable driver's stand-ins: an Object with maybe a body (it logs the healing it is given) and maybe a dock (its `setDockCrippled` logs),
//! the REAL `Object::attemptHealing`, the game logic whose frame the test sets and whose wake recorder counts the calls `setWakeFrame` makes, and
//! the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the object (the module from `modules::new_module`, `ProbeModule`s for the body and the dock),
//! the healing through the verified `World::attempt_healing`, every op an `update()` as the scheduler runs it, a `World::module_call` or a change to
//! the scene.
//!
//! Each case reads a block `D` (`SelfHealSupression`, `SelfHealDelay`, `SelfHealAmount`), mostly well formed, and runs up to 40 ops. Compared,
//! bit for bit: the read's throw; the data; per op the throw status, the sleep time, the heal and cripple lines, the wake calls and the frame of
//! the last, the xfer status, bytes and CRC, the module's two state words and the body's health.
//!
//! # Results (2026-10-23, `L9R17_CASES=9000`)
//!
//! 9,000 cases (1,060 excluded as undefined upstream: an object with no body or no dock; 676 data files threw, as the C++ did), 146,289 ops
//! (60,395 updates, 27,304 xfers) and 168,081 op outputs: **0 mismatches**; 55,188 heals and 5,087 cripple calls compared. No harness correction
//! was needed.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 4 of 4 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1821 | the next healing frame not held off by the damage | 21,260 |
//! | 1822 | the module stays awake at full health | 13,766 |
//! | 1823 | the next healing frame not saved | 12,961 |
//! | 1824 | the dock not uncrippled when the damage state improves | 20 |

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
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::supply_warehouse_crippling_behavior::SupplyWarehouseCripplingBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r17.bin");
const XFER_FILE: &str = "l9_r17.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SupplyWarehouseCripplingBehavior(Arc::new(
        SupplyWarehouseCripplingBehaviorModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: read_block,
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::SupplyWarehouseCripplingBehavior(d) = data else {
        unreachable!("not a SupplyWarehouseCripplingBehavior")
    };
    put_u32(v, d.self_heal_supression);
    put_u32(v, d.self_heal_delay);
    put_u32(v, d.self_heal_amount.to_bits());
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..3).map(|_| r.u32()).collect();
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
        let (wakes, last) = (r.u32(), r.u32());
        let mut x = String::new();
        if i > 0 {
            x = r.xfer();
        }
        let state: Vec<u32> = (0..3).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} wakes {wakes} last {last} {x} state {state:?}"));
    }
    out
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=7 => pick(
            g,
            &["100", "300", "500", "1000", "3000", "33", "34", "67", "200"],
        )
        .to_string(),
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
    field(g, 80, "SelfHealSupression", v);
    let v = gen_dur(g, clean);
    field(g, 80, "SelfHealDelay", v);
    let v = match g.below(8) {
        0 => "0".to_string(),
        1 => "-5".to_string(),
        2 if !clean => pick(g, &["x", ""]).to_string(),
        _ => pick(g, &["5", "10", "2.5", "1000"]).to_string(),
    };
    field(g, 80, "SelfHealAmount", v);
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

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the health, the maximum, the body is present, the dock is present]
    let cfg = vec![
        gen_frame(g),
        40.0f32.to_bits(),
        100.0f32.to_bits(),
        u32::from(g.below(12) != 0),
        u32::from(g.below(10) != 0),
    ];
    let n_ops = 1 + g.below(30) as usize;
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // most cases start with a hit, then run a few frames
    if g.below(100) < 60 {
        ops.push([1, g.below(30), 0, 0, 0]);
        for _ in 0..1 + g.below(6) {
            frame = frame.wrapping_add(1 + g.below(40));
            ops.push([4, frame, 0, 0, 0]);
            ops.push([0, 0, 0, 0, 0]);
        }
    }
    ops.extend((0..n_ops).map(|_| match g.below(100) {
        0..=29 => [0, 0, 0, 0, 0],
        30..=39 => [1, g.below(30), 0, 0, 0],
        40..=49 => [2, g.below(4), g.below(4), 0, 0],
        50..=69 => {
            frame = if g.below(8) == 0 {
                gen_frame(g)
            } else {
                frame.wrapping_add(1 + g.below(60))
            };
            [4, frame, 0, 0, 0]
        }
        70..=75 => [
            5,
            (*pick(g, &[&40.0f32, &100.0, &99.5, &0.0])).to_bits(),
            0,
            0,
            0,
        ],
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
    heals: u64,
    cripples: u64,
}

fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32],
    ops: &[[u32; 5]],
    tally: &RefCell<Tally>,
) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        ..Default::default()
    });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(0);
    let mut modules = Vec::new();
    if cfg[3] != 0 {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY;
        let mut p = ProbeModule::new(
            ProbeAnswers::default(),
            f32::from_bits(cfg[2]),
            Default::default(),
        );
        p.health = f32::from_bits(cfg[1]);
        m.body = Some(UpdateBody::ModuleProbe(p));
        modules.push(m);
    }
    if cfg[4] != 0 {
        let mut m = NewModule::plain(b"ProbeDock", b"ModuleTag_Dock");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers {
                has: 1 << 2,
                ..Default::default()
            },
            0.0,
            Default::default(),
        )));
        modules.push(m);
    }
    modules.push(
        rts_lockstep_engine::logic::modules::new_module(
            w.next_object_id(),
            b"SupplyWarehouseCripplingBehavior",
            b"ModuleTag_Cripple",
            &data,
        )
        .unwrap()
        .unwrap(),
    );
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: None,
            modules,
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let idx = w
        .find_module(me, b"SupplyWarehouseCripplingBehavior")
        .unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let constructor_trace = w.cuts.trace.take().unwrap_or_default();
    // a new object has the drawable the client made for it: the test binds one, as batch E does
    w.friend_bind_to_drawable(me, 7);
    let mut saved: Vec<u8> = Vec::new();
    let mut first_trace = Some(constructor_trace);
    for phase in 0..=ops.len() {
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let errors_before = w.cuts.module_errors.len();
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    res = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    let mut info = DamageInfo::default();
                    info.input.damage_type = o[1] as i32;
                    w.module_call(me, idx, ModuleCall::OnDamage(info));
                }
                2 => {
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::OnBodyDamageStateChange {
                            info: DamageInfo::default(),
                            old_state: o[1] as i32,
                            new_state: o[2] as i32,
                        },
                    );
                }
                4 => w.set_frame(o[1]),
                5 => {
                    if let Some(p) =
                        w.objects
                            .find_mut(me)
                            .unwrap()
                            .modules
                            .iter_mut()
                            .find_map(|b| match b.body.as_mut() {
                                Some(UpdateBody::ModuleProbe(p)) if b.name == b"ProbeBody" => {
                                    Some(p)
                                }
                                _ => None,
                            })
                    {
                        p.health = f32::from_bits(o[1]);
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
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        let trace = if phase == 0 {
            first_trace.take().unwrap_or_default()
        } else {
            w.cuts.trace.take().unwrap()
        };
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                // the wake calls of the module under test count, not the helpers'
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if phase > 0 && t.starts_with("heal ") {
                tally.borrow_mut().heals += 1;
                log.push(t);
            } else if let Some(r) = t.strip_prefix("bt crippled ").filter(|_| phase > 0) {
                tally.borrow_mut().cripples += 1;
                log.push(format!("crippled {r}"));
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::SupplyWarehouseCripplingBehavior(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        let health = w
            .objects
            .find_mut(me)
            .unwrap()
            .modules
            .iter_mut()
            .find_map(|b| match b.body.as_mut() {
                Some(UpdateBody::ModuleProbe(p)) if b.name == b"ProbeBody" => Some(p.health),
                _ => None,
            });
        put_u32(&mut v, health.map_or(0, f32::to_bits));
    }
    let excluded = w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined());
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded,
    }
}

#[test]
fn supply_warehouse_crippling_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "SupplyWarehouseCripplingBehavior R17",
        seed: 0x0017_C219,
        cases_var: "L9R17_CASES",
        ini_stem: "R17",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r17_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "SupplyWarehouseCripplingBehavior R17: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} heals, {} cripples",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.heals, t.cripples
    );
    assert!(
        t.heals * 4 > c.cases as u64 && t.cripples * 4 > c.cases as u64,
        "the generator must reach every path"
    );
}
