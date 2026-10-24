#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProneUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ProneUpdate` (`logic::modules::prone_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `ProneUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_prone_o.inc`) on the module build's stand-ins: an
//! `Object` with status bits and an optional `Drawable` that logs the model-condition calls, and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with one object (a drawable bound or not), the module from `modules::new_module`, `update()` run as the
//! scheduler runs it (`World::with_body`), `goProne` as `ModuleCall::ProneGoProne` and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`DamageToFramesRatio`; mostly well formed) and, when the read succeeds, makes the module and runs 1 to 40 ops: an update, a
//! `goProne` with an actual damage dealt, the drawable bound or unbound, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects (`mc`, `mcclear`), the xfer status, bytes and CRC,
//! the prone frames and the `NO_ATTACK` bit. A damage or a frame count outside `Int` (or NaN), and an effect on an object with no drawable, are upstream's
//! undefined behaviour: counted, never sent to the C++.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9PR_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 554 data blocks threw (read compared: 7611 comparisons, 0 mismatches), 1389 cases excluded (a damage or frame count outside `Int`, or an effect on an object with no drawable), 138657 ops, 78217 updates, 43504 goProne calls, 39784 xfers, 9174 effect lines; ops: 159828 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run was clean; the generator was then tightened because 46 % of the cases were excluded.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1161 | the ratio is ignored (1.0) |
//! | 1162 | the effects stop at every frame, not when the count reaches 0 |
//! | 1163 | the effects stop without clearing `NO_ATTACK` |
//! | 1164 | the effects start for every `goProne`, not only the one that makes the unit prone |
//!
//! All 4 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NO_ATTACK;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::prone_update::ProneUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9pr_o.bin");
const XFER_FILE: &str = "l9pr_o.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ProneUpdate(Arc::new(ProneUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::ProneUpdate(d) = data else { unreachable!("not a ProneUpdate") };
    put_u32(v, d.damage_to_frames_ratio.to_bits());
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    out.push(format!("data ratio {:#x}", r.u32()));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let sleep = r.u32() as i32;
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..2).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 85 {
        let v = match g.below(10) {
            0 => "0".to_string(),
            1..=6 => pick(g, &["0.5", "1", "2", "3.5", "10", "0.1", "-1"]).to_string(),
            7 if !clean => pick(g, &["x", "", "-", "1e99"]).to_string(),
            _ => gen_real(g, clean),
        };
        lines.push(format!("DamageToFramesRatio{}{v}", gen_sep(g)));
    }
    if !clean && g.below(12) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn gen_damage(g: &mut Gen) -> u32 {
    match g.below(80) {
        0 => 1.0e10f32,
        1 => f32::NAN,
        2..=5 => -5.0,
        6..=15 => 0.0,
        16..=35 => 1.0,
        36..=50 => 3.5,
        51..=65 => 10.0,
        66..=72 => 100.0,
        _ => 1000.0,
    }
    .to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let cfg = vec![u32::from(g.below(100) < 97)];
    let n_ops = 1 + g.below(40) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=44 => [0, 0, 0, 0, 0],
            45..=69 => [1, gen_damage(g), 0, 0, 0],
            70..=76 => [2, u32::from(g.below(12) != 0), 0, 0, 0],
            77..=84 => [3, 0, 0, 0, 0],
            85..=92 => [4, 0, 0, 0, 0],
            _ => [5, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    prones: u64,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ProneUpdate", b"ModuleTag_Prone", &data).unwrap().unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template, status: Default::default(), team: None, modules: vec![m] }).unwrap();
    assert_eq!(me, 1);
    w.friend_bind_to_drawable(me, if cfg[0] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let idx = w.find_module(me, b"ProneUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    let mut info = DamageInfo::default();
                    info.out.actual_damage_dealt = f32::from_bits(o[1]);
                    w.module_call(me, idx, ModuleCall::ProneGoProne(info));
                    tally.borrow_mut().prones += 1;
                }
                2 => {
                    w.friend_bind_to_drawable(me, if o[1] != 0 { 7 } else { INVALID_DRAWABLE_ID });
                    // binding a drawable makes the client set the object's whole model-condition state (the client's, not the module's)
                    events_before = w.events.events().len();
                }
                3..=5 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 3 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        } else {
            events_before = w.events.events().len();
        }
        if w.cuts.module_errors.len() > errors_before {
            assert!(w.cuts.module_errors[errors_before..].iter().all(|(_, e)| e.is_undefined()), "{:?}", w.cuts.module_errors);
            excluded = true;
        }
        let mut log = Vec::new();
        for e in &w.events.events()[events_before..] {
            match e {
                ClientEvent::Object(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(f), .. }) => log.push(format!("mc {}", f[0])),
                ClientEvent::Object(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(f), .. }) => log.push(format!("mcclear {}", f[0])),
                _ => {}
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        put_i32(&mut v, 0);
        put_u32(&mut v, sleep);
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
        let state = w.objects.find(me).unwrap().behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::ProneUpdate(m)) => Some(m.state_words()),
            _ => None,
        });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        put_u32(&mut v, u32::from(w.objects.find(me).unwrap().status.test(OBJECT_STATUS_NO_ATTACK)));
    }
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn prone_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O4",
        seed: 0x00B0_9E01,
        cases_var: "L9PR_CASES",
        ini_stem: "Pr",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9pr_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O4: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} goProne calls, {} xfers, {} effect lines",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.prones, t.xfers, t.effects
    );
    assert!(c.read_failed * 40 > c.cases as u64 && c.ops_run * 5 > c.cases as u64 * 4 && t.effects * 5 > c.cases as u64, "the generator must reach every path");
}
