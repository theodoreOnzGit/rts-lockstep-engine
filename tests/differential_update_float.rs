#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FloatUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FloatUpdate` (`logic::modules::float_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `FloatUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_float_o.inc`) on the module build's stand-ins: an
//! `Object` with a position the test sets and an optional `Drawable` whose instance matrix calls log the frame they are made at (the matrix maths is the
//! client's), a terrain logic whose `isUnderwater` logs and answers from tables the test sets (indexed by a hash of the position, the Rust side's `terrain_cell`;
//! it assigns `waterZ` only where it has water, as upstream does), a game logic whose frame the test sets, and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with one object, the module from `modules::new_module`, the water in `cuts.terrain_has_water` /
//! `cuts.terrain_water`, `update()` run as the scheduler runs it (`World::with_body`), `setEnabled` as `ModuleCall::FloatSetEnabled`, `setPosition` the real
//! `World::set_position`, and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`Enabled`; mostly well formed) and, when the read succeeds, makes the module and runs 1 to 40 ops: an update, a new position, a
//! new frame, water in a terrain cell, `setEnabled`, the drawable bound or unbound, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects (`underwater <x> <y>`, `instance <frame>`), the xfer
//! status, bytes and CRC, the enabled flag and the object's position. An enabled update over a cell with no water reads `waterZ` unassigned: upstream's undefined
//! behaviour, counted, never sent to the C++.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). The bob of the drawable (the yaw and the
//! pitch `sin(frame * 0.0291) * 0.05` and `sin(frame * 0.0515) * 0.05` applied to the drawable's instance matrix, keeping its Z rotation) is the client's
//! maths on client data: the translation sends the frame (`ObjectClientEvent::DrawableFloatTilt`) and the test compares that the call is made, once per update with
//! a drawable, at that frame.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9FLR_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 495 data blocks threw (read compared: 7828 comparisons, 0 mismatches), 1172 cases excluded (an enabled update over a cell with no water: `waterZ` unassigned), 200906 ops, 69895 updates, 24502 xfers, 77599 effect lines (42810 bobs); ops: 222905 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run was clean; the generator was then given water in every cell at the start of most cases because 60 % of the cases were excluded.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1181 | `setEnabled(TRUE)` disables |
//! | 1182 | the object keeps its own z |
//! | 1183 | no bob |
//! | 1184 | `xfer` leaves out the flag |
//!
//! All 4 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::float_update::FloatUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9flr_o.bin");
const XFER_FILE: &str = "l9flr_o.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FloatUpdate(Arc::new(
        FloatUpdateModuleData::friend_new_module_data(Some(ini))?,
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
    let ModuleDataKind::FloatUpdate(d) = data else {
        unreachable!("not a FloatUpdate")
    };
    put_u32(v, u32::from(d.enabled));
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    out.push(format!("data enabled {}", r.u32()));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let sleep = r.u32() as i32;
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        out.push(format!(
            "op {i}: status {st} sleep {sleep} log {log:?} {x} state {state:x?}"
        ));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 80 {
        lines.push(format!("Enabled{}{}", gen_sep(g), gen_bool(g, clean)));
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

fn bits(x: f32) -> u32 {
    x.to_bits()
}

fn gen_xy(g: &mut Gen) -> f32 {
    *pick(g, &[&-10.0f32, &0.0, &5.5, &100.0, &33.25])
}

fn gen_z(g: &mut Gen) -> f32 {
    *pick(g, &[&-5.0f32, &0.0, &1.0, &5.0, &10.0])
}

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, x, y, z, the drawable]
    let cfg = vec![
        g.below(2000),
        bits(gen_xy(g)),
        bits(gen_xy(g)),
        bits(gen_z(g)),
        u32::from(g.below(100) < 80),
    ];
    let n_ops = 1 + g.below(40) as usize;
    // most cases begin with water in every cell, so that an enabled update has a surface to follow
    let prefix: Vec<[u32; 5]> = if g.below(100) < 80 {
        (0..8)
            .map(|i| [3, i, 1, bits(*pick(g, &[&-2.0f32, &0.0, &1.5, &7.0])), 0])
            .collect()
    } else {
        Vec::new()
    };
    let ops: Vec<[u32; 5]> = prefix
        .into_iter()
        .chain((0..n_ops).map(|_| match g.below(100) {
            0..=39 => [0, 0, 0, 0, 0],
            40..=49 => [1, bits(gen_xy(g)), bits(gen_xy(g)), bits(gen_z(g)), 0],
            50..=56 => [2, g.below(5000), 0, 0, 0],
            57..=72 => [
                3,
                g.below(8),
                u32::from(g.below(100) < 93),
                bits(*pick(g, &[&-2.0f32, &0.0, &1.5, &7.0, &-0.0])),
                0,
            ],
            73..=80 => [4, g.below(2), 0, 0, 0],
            81..=85 => [5, g.below(2), 0, 0, 0],
            86..=91 => [6, 0, 0, 0, 0],
            92..=96 => [7, 0, 0, 0, 0],
            _ => [8, 0, 0, 0, 0],
        }))
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    moves: u64,
    bobs: u64,
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
    w.set_frame(cfg[0]);
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"FloatUpdate",
        b"ModuleTag_Float",
        &data,
    )
    .unwrap()
    .unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: None,
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1);
    w.set_position(
        me,
        &Coord3D {
            x: f32::from_bits(cfg[1]),
            y: f32::from_bits(cfg[2]),
            z: f32::from_bits(cfg[3]),
        },
    );
    w.friend_bind_to_drawable(me, if cfg[4] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let idx = w.find_module(me, b"FloatUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
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
                1 => w.set_position(
                    me,
                    &Coord3D {
                        x: f32::from_bits(o[1]),
                        y: f32::from_bits(o[2]),
                        z: f32::from_bits(o[3]),
                    },
                ),
                2 => w.set_frame(o[1]),
                3 => {
                    w.cuts.terrain_has_water[o[1] as usize & 7] = o[2] != 0;
                    w.cuts.terrain_water[o[1] as usize & 7] = f32::from_bits(o[3]);
                }
                4 => {
                    w.module_call(me, idx, ModuleCall::FloatSetEnabled(o[1] != 0));
                }
                5 => {
                    w.friend_bind_to_drawable(me, if o[1] != 0 { 7 } else { INVALID_DRAWABLE_ID });
                    // binding a drawable makes the client set the object's whole model-condition state (the client's, not the module's)
                    events_before = w.events.events().len();
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
        if w.cuts.module_errors.len() > errors_before {
            assert!(
                w.cuts.module_errors[errors_before..]
                    .iter()
                    .all(|(_, e)| e.is_undefined()),
                "{:?}",
                w.cuts.module_errors
            );
            excluded = true;
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
                if let Some(CutCall::TerrainIsUnderwater { x, y }) = cut_iter.next() {
                    log.push(format!(
                        "underwater {:08x} {:08x}",
                        x.to_bits(),
                        y.to_bits()
                    ));
                }
            } else if t.starts_with("evt ") {
                if let Some(ObjectClientEvent::DrawableFloatTilt { frame, .. }) = ev_iter.next() {
                    tally.borrow_mut().bobs += 1;
                    log.push(format!("instance {frame}"));
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
                    Some(UpdateBody::FloatUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        let p = w.objects.find(me).unwrap().get_position();
        if p.z.to_bits() != f32::from_bits(cfg[3]).to_bits() {
            tally.borrow_mut().moves += 1;
        }
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
    }
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded,
    }
}

#[test]
fn float_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O6",
        seed: 0x00F1_0A70,
        cases_var: "L9FLR_CASES",
        ini_stem: "Fl",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9flr_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O6: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} bobs, {} op results with z moved",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.bobs, t.moves
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.bobs * 4 > c.cases as u64
            && t.moves * 10 > c.cases as u64,
        "the generator must reach every path"
    );
}
