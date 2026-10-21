#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CheckpointUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CheckpointUpdate` (`logic::modules::checkpoint_update`) against the unchanged C++, `p1w12-sonnet`, update batch N.
//!
//! # Method
//!
//! The C++ side is `CheckpointUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_checkpoint_n.inc`) on the module build's
//! stand-ins: an `Object` with the REAL `GeometryInfo` (`Geometry.cpp`), a vision range and an optional `Drawable` that logs the calls it is given, an `AI`
//! whose two queries log and answer from flags the test sets, a game logic whose frame the test sets, and the REAL `UpdateModule`, `INI`, `Xfer` and
//! logic random values.
//!
//! The Rust side is the real thing: a `World` with one object made from a template with that geometry and vision range, the module from
//! `modules::new_module`, the AI answers in `cuts.ai_enemy_near` / `cuts.ai_ally_near`, `update()` run as the scheduler runs it (`World::with_body`) and
//! the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`ScanDelayTime`; mostly well formed) and, when the read succeeds, makes the module (its random draw is compared through
//! the first state line) and runs 1 to 30 ops: an update, an enemy or an ally near or gone, a new vision range, the drawable bound or unbound, a new
//! minor radius, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in order (`ai enemy <range> <qualifiers> <minor radius>`,
//! `ai ally`, `react`, `clearset`), the xfer status, bytes and CRC, the four state words and the object's minor radius.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9CP_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 322 data
//! blocks threw (read compared: 8948 comparisons, 0 mismatches), 52 cases excluded (a read with undefined behaviour in the INI numbers), 133827 ops, 64118
//! updates, 25281 xfers, 256342 effect lines (122763 geometry reactions, 5343 door changes); ops: 159705 comparisons, 0 mismatches, 0 NaN-payload-only, max
//! ULP 0. The first run found two defects of the test, none of the module: a new object has the drawable the client made for it (the test binds one or
//! none), and the AI stand-in did not show the geometry at the call (plant 961 survived until the call logged the minor radius it is made at).
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 961 | the minor radius not stretched to its maximum for the scan |
//! | 962 | no closing animation |
//! | 963 | the gate moves by 0.33 a frame, not 0.333 |
//! | 964 | `xfer` leaves out the scan delay |
//! | 965 | the scan's radius is not restored |
//! | 966 | the gate opens for an enemy whatever the allies |
//! | 967 | the gate keeps shrinking at radius 0 |
//! | 968 | a change of the ally flag alone is not a change |
//!
//! All 8 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::checkpoint_update::CheckpointUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9cp_n.bin");
const XFER_FILE: &str = "l9cp_n.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::CheckpointUpdate(Arc::new(CheckpointUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::CheckpointUpdate(d) = data else { unreachable!("not a CheckpointUpdate") };
    put_u32(v, d.enemy_scan_delay_time);
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    out.push(format!("data scan delay {}", r.u32()));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let sleep = r.u32() as i32;
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..5).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} {x} state {state:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 80 {
        let v = match g.below(10) {
            0 => "0".to_string(),
            1..=5 => pick(g, &["30", "100", "500", "1000", "33", "67"]).to_string(),
            6 => format!("{}", g.below(100_000)),
            7 if !clean => pick(g, &["x", "", "-", "12abc", "99999999999"]).to_string(),
            _ => "1000".to_string(),
        };
        let sep = gen_sep(g);
        lines.push(format!("ScanDelayTime{sep}{v}"));
    }
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

fn gen_radius(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0.0f32,
        1 => 1.0,
        2 => 2.5,
        3 => 5.0,
        4 => 10.0,
        5 => g.uniform(0.0, 20.0),
        6 => 0.2,
        _ => 15.0,
    }
    .to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the minor radius, the drawable, the random seed, the vision range, the frame]
    let cfg = vec![gen_radius(g), u32::from(g.below(10) < 7), g.u32(), pick(g, &[&0.0f32, &30.0, &150.0, &75.5]).to_owned().to_bits(), g.below(300)];
    let n_ops = 1 + g.below(30) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=44 => [0, 0, 0, 0, 0],
            45..=52 => [1, g.below(2), 0, 0, 0],
            53..=60 => [2, g.below(2), 0, 0, 0],
            61..=64 => [3, pick(g, &[&0.0f32, &30.0, &150.0, &75.5]).to_owned().to_bits(), 0, 0, 0],
            65..=69 => [4, g.below(2), 0, 0, 0],
            70..=77 => [5, gen_radius(g), 0, 0, 0],
            78..=84 => [6, 0, 0, 0, 0],
            85..=91 => [7, 0, 0, 0, 0],
            92..=96 => [8, 0, 0, 0, 0],
            _ => [0, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    reacts: u64,
    clearsets: u64,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.random.init_game_logic_random(cfg[2]);
    w.set_frame(cfg[4]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"CheckpointUpdate", b"ModuleTag_Checkpoint", &data).unwrap().unwrap();
    let mut values = ObjectTemplateValues::default();
    let mut geom = GeometryInfo::new(GeometryType::Sphere, false, 1.0, 1.0, 1.0);
    geom.set_minor_radius(f32::from_bits(cfg[0]));
    values.geometry = Some(geom);
    values.vision_range = f32::from_bits(cfg[3]);
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let me = w.new_object(NewObject { template, status: Default::default(), team: None, modules: vec![m] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    // a new object has the drawable the client made for it: the test binds one or none, as batch E does
    w.friend_bind_to_drawable(me, if cfg[1] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let idx = w.find_module(me, b"CheckpointUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    if o[1] != 0 {
                        w.cuts.ai_enemy_near.insert(me);
                    } else {
                        w.cuts.ai_enemy_near.remove(&me);
                    }
                }
                2 => {
                    if o[1] != 0 {
                        w.cuts.ai_ally_near.insert(me);
                    } else {
                        w.cuts.ai_ally_near.remove(&me);
                    }
                }
                3 => w.objects.find_mut(me).unwrap().vision_range = f32::from_bits(o[1]),
                4 => {
                    w.friend_bind_to_drawable(me, if o[1] != 0 { 7 } else { INVALID_DRAWABLE_ID });
                    // binding a drawable makes the client set the object's whole model-condition state (the client's, not the module's)
                    events_before = w.events.events().len();
                    w.cuts.trace = Some(Vec::new());
                    w.cuts.log = Some(Vec::new());
                }
                5 => w.objects.find_mut(me).unwrap().geometry_info.set_minor_radius(f32::from_bits(o[1])),
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
            // the constructor's own effects were made before the first op: none are compared (the random draw shows in the state)
            w.cuts.trace = Some(Vec::new());
        }
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let trace = w.cuts.trace.take().unwrap_or_default();
        let cuts = w.cuts.log.take().unwrap_or_default();
        let mut log = Vec::new();
        let (mut cut_iter, mut ev_iter) = (cuts.into_iter(), events.into_iter());
        for t in trace {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::AiFindClosestEnemy { range, qualifiers, minor_radius, .. }) => log.push(format!("ai enemy {:08x} {qualifiers} {:08x}", range.to_bits(), minor_radius.to_bits())),
                    Some(CutCall::AiFindClosestAlly { range, qualifiers, minor_radius, .. }) => log.push(format!("ai ally {:08x} {qualifiers} {:08x}", range.to_bits(), minor_radius.to_bits())),
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableReactToGeometryChange { .. }) => {
                        log.push("react".to_string());
                        tally.borrow_mut().reacts += 1;
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. }) => {
                        log.push(format!("clearset {} {}", clear[0], set[0]));
                        tally.borrow_mut().clearsets += 1;
                    }
                    _ => {}
                }
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
            Some(UpdateBody::CheckpointUpdate(m)) => Some(m.state_words()),
            _ => None,
        });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        put_u32(&mut v, w.objects.find(me).unwrap().geometry_info.get_minor_radius().to_bits());
    }
    RunOut { dump: v, keys: Vec::new(), excluded: false }
}

#[test]
fn checkpoint_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module N1",
        seed: 0x00C4_EC40,
        cases_var: "L9CP_CASES",
        ini_stem: "Cp",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9cp_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module N1: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} geometry reactions, {} door changes",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.reacts, t.clearsets
    );
    assert!(c.read_failed * 40 > c.cases as u64 && c.ops_run * 5 > c.cases as u64 * 4 && t.clearsets * 20 > c.cases as u64 && t.reacts * 4 > c.cases as u64, "the generator must reach every path");
}
