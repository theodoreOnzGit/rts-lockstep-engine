#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EnemyNearUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `EnemyNearUpdate` (`logic::modules::enemy_near_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `EnemyNearUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_enemy_near_o.inc`) on the module build's stand-ins:
//! an `Object` with a vision range and an optional `Drawable` that logs the model-condition calls, an `AI` whose `findClosestEnemy` logs and answers from a
//! flag the test sets, and the REAL `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with one object (a template with that geometry and vision range), the module from `modules::new_module`, the AI
//! answer in `cuts.ai_enemy_near`, `update()` run as the scheduler runs it (`World::with_body`) and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`ScanDelayTime`; mostly well formed) and, when the read succeeds, makes the module (its random draw is compared through the
//! first state line) and runs 1 to 60 ops: an update, an enemy near or gone, a new vision range, the drawable bound or unbound, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in order (`ai enemy <range> <qualifiers> <minor
//! radius>`, `mc`, `mcclear`), the xfer status, bytes and CRC and the two state words.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9EN_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 515 data blocks threw (read compared: 8968 comparisons, 0 mismatches), 32 cases excluded (a read with undefined behaviour in the INI numbers), 258841 ops, 165297 updates, 49408 xfers, 44261 effect lines (40614 scans, 3647 model-condition changes); ops: 284200 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run was clean.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1171 | the scan delay reloads one frame short |
//! | 1172 | the scan has no `CAN_SEE` qualifier |
//! | 1173 | the model condition is not cleared when the enemy goes |
//! | 1174 | `xfer` leaves out the enemy flag |
//!
//! All 4 are caught (9000 cases each), all as mismatches.

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
use rts_lockstep_engine::logic::modules::enemy_near_update::EnemyNearUpdateModuleData;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9en_o.bin");
const XFER_FILE: &str = "l9en_o.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::EnemyNearUpdate(Arc::new(
        EnemyNearUpdateModuleData::friend_new_module_data(Some(ini))?,
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
    let ModuleDataKind::EnemyNearUpdate(d) = data else {
        unreachable!("not an EnemyNearUpdate")
    };
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
        let state: Vec<u32> = (0..2).map(|_| r.u32()).collect();
        out.push(format!(
            "op {i}: status {st} sleep {sleep} log {log:?} {x} state {state:?}"
        ));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 85 {
        let v = match g.below(10) {
            0 => "0".to_string(),
            1..=6 => pick(g, &["34", "67", "100", "33", "134", "200"]).to_string(),
            7 if !clean => pick(g, &["x", "", "-", "12abc", "99999999999"]).to_string(),
            8 => format!("{}", g.below(2000)),
            _ => "1000".to_string(),
        };
        lines.push(format!("ScanDelayTime{}{v}", gen_sep(g)));
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

fn gen_vision(g: &mut Gen) -> u32 {
    pick(g, &[&0.0f32, &30.0, &150.0, &75.5]).to_bits()
}

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the drawable, the random seed, the vision range]
    let cfg = vec![u32::from(g.below(100) < 85), g.u32(), gen_vision(g)];
    let n_ops = 1 + g.below(60) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=59 => [0, 0, 0, 0, 0],
            60..=67 => [1, g.below(2), 0, 0, 0],
            68..=71 => [2, gen_vision(g), 0, 0, 0],
            72..=76 => [3, g.below(2), 0, 0, 0],
            77..=83 => [4, 0, 0, 0, 0],
            84..=90 => [5, 0, 0, 0, 0],
            91..=95 => [6, 0, 0, 0, 0],
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
    scans: u64,
    conditions: u64,
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
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.random.init_game_logic_random(cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"EnemyNearUpdate",
        b"ModuleTag_EnemyNear",
        &data,
    )
    .unwrap()
    .unwrap();
    let values = ObjectTemplateValues {
        geometry: Some(GeometryInfo::new(
            GeometryType::Sphere,
            false,
            1.0,
            1.0,
            1.0,
        )),
        vision_range: f32::from_bits(cfg[2]),
        ..Default::default()
    };
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: None,
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1);
    w.friend_bind_to_drawable(me, if cfg[0] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let idx = w.find_module(me, b"EnemyNearUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
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
                    sleep = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    if o[1] != 0 {
                        w.cuts.ai_enemy_near.insert(me);
                    } else {
                        w.cuts.ai_enemy_near.remove(&me);
                    }
                }
                2 => w.objects.find_mut(me).unwrap().vision_range = f32::from_bits(o[1]),
                3 => {
                    w.friend_bind_to_drawable(me, if o[1] != 0 { 7 } else { INVALID_DRAWABLE_ID });
                    // binding a drawable makes the client set the object's whole model-condition state (the client's, not the module's)
                    events_before = w.events.events().len();
                    w.cuts.trace = Some(Vec::new());
                }
                4..=6 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 4, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 4 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        } else {
            // the constructor's own effects are not compared (the random draw shows in the state)
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
                if let Some(CutCall::AiFindClosestEnemy {
                    range,
                    qualifiers,
                    minor_radius,
                    ..
                }) = cut_iter.next()
                {
                    tally.borrow_mut().scans += 1;
                    log.push(format!(
                        "ai enemy {:08x} {qualifiers} {:08x}",
                        range.to_bits(),
                        minor_radius.to_bits()
                    ));
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Set(f),
                        ..
                    }) => {
                        tally.borrow_mut().conditions += 1;
                        log.push(format!("mc {}", f[0]));
                    }
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Clear(f),
                        ..
                    }) => {
                        tally.borrow_mut().conditions += 1;
                        log.push(format!("mcclear {}", f[0]));
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::EnemyNearUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
    }
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded: false,
    }
}

#[test]
fn enemy_near_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O5",
        seed: 0x00E4_E17A,
        cases_var: "L9EN_CASES",
        ini_stem: "En",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9en_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O5: {} cases, {} data blocks threw, {} excluded, {} ops, {} updates, {} xfers, {} effect lines, {} scans, {} model-condition changes",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.scans, t.conditions
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.scans * 4 > c.cases as u64
            && t.conditions * 8 > c.cases as u64,
        "the generator must reach every path"
    );
}
