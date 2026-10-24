#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HeightDieUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `HeightDieUpdate` (`logic::modules::height_die_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `HeightDieUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_height_die_o.inc`) on the module build's
//! stand-ins: four `Object`s with the REAL `GeometryInfo` (object 1 carries the module; 2 to 4 are the candidates of the structure scan), a position and a
//! container the test sets, a body (`MBody`) that logs the damage `Object::kill` (Object.cpp 1954-1968) deals it, a terrain logic whose three queries log and
//! answer from tables the test sets (indexed by a hash of the position, the Rust side's `terrain_cell`), a partition manager whose range scan logs and answers
//! with the list the test gives, a particle system manager whose `destroyAttachedSystems` logs, a game logic whose frame the test sets, and the REAL
//! `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with four objects made from templates with those geometries (object 1 has the module from `modules::new_module`
//! and a scripted probe body, `module_probe.rs`, whose damage is a line of the trace when `probe_effect_trace` asks), the terrain and the range scan in
//! `cuts.terrain_*` / `cuts.range_scan`, `update()` run as the scheduler runs it (`World::with_body`) and the xfers through `World::xfer_module_slot`.
//! `setPosition` is the real `World::set_position`.
//!
//! Each case reads a `D` block (the six fields; mostly well formed) and, when the read succeeds, makes the module and runs 1 to 40 ops: an update, a new
//! position, a terrain table cell, a range scan list, the object contained or free, a new frame, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in order (`ground`, `highlayer`, `layerheight`,
//! `scan`, `damage`, `psdestroy`), the xfer status, bytes and CRC, the six state words, the object's position and its body's health. An object with no body
//! that reaches `kill()` is upstream's null dereference (`Undefined::NullBodyModule`): counted, never sent to the C++.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9HD_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 869 data
//! blocks threw (read compared: 8766 comparisons, 0 mismatches), 234 cases excluded (a kill of an object with no body module), 161654 ops, 66191
//! updates, 23254 xfers, 27902 effect lines (2314 kills, 5229 structure scans, 5229 layer queries, 1497 particle destroys); ops: 185345 comparisons, 0 mismatches,
//! 0 NaN-payload-only, max ULP 0. The first run was clean.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1101 | the initial delay is ignored |
//! | 1102 | the object may die on the frame the delay ends (`>=` for `>`) |
//! | 1103 | a contained object is not skipped |
//! | 1104 | "moving down" is `z > last z`, not `z >= last z` |
//! | 1105 | the layer under the object is not consulted |
//! | 1106 | the layer height replaces the terrain height even when lower |
//! | 1107 | the object does not ignore itself in the structure scan |
//! | 1108 | the tallest structure alone is the target (the terrain height is not added) |
//! | 1109 | no snap below the ground when not snapping |
//! | 1110 | `m_hasDied` is never set |
//! | 1111 | the particle test ignores the direction |
//! | 1112 | the particle test and the last position use the position from before the snap |
//! | 1113 | `xfer` leaves out the earliest death frame |
//! | 1114 | the particle test is `<=` |
//!
//! All 14 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{
    ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::height_die_update::HeightDieUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9hd_o.bin");
const XFER_FILE: &str = "l9hd_o.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::HeightDieUpdate(Arc::new(
        HeightDieUpdateModuleData::friend_new_module_data(Some(ini))?,
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
    let ModuleDataKind::HeightDieUpdate(d) = data else {
        unreachable!("not a HeightDieUpdate")
    };
    put_u32(v, d.target_height_above_terrain.to_bits());
    put_u32(v, u32::from(d.target_height_includes_structures));
    put_u32(v, u32::from(d.only_when_moving_down));
    put_u32(v, d.destroy_attached_particles_at_height.to_bits());
    put_u32(v, u32::from(d.snap_to_ground_on_death));
    put_u32(v, d.initial_delay);
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    out.push(format!("data {f:x?}"));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let sleep = r.u32() as i32;
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..10).map(|_| r.u32()).collect();
        out.push(format!(
            "op {i}: status {st} sleep {sleep} log {log:?} {x} state {state:x?}"
        ));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = match g.below(10) {
        0 => "0".to_string(),
        1..=6 => pick(g, &["1", "2.5", "5", "10", "-1", "0.5", "30"]).to_string(),
        7 if !clean => pick(g, &["x", "", "-", "1e99"]).to_string(),
        _ => gen_real(g, clean),
    };
    field(g, 85, "TargetHeight", v);
    let v = gen_bool(g, clean);
    field(g, 70, "TargetHeightIncludesStructures", v);
    let v = gen_bool(g, clean);
    field(g, 60, "OnlyWhenMovingDown", v);
    let v = match g.below(10) {
        0 => "-1".to_string(),
        1..=6 => pick(g, &["0", "1", "3", "10", "5.5", "2", "-2"]).to_string(),
        7 if !clean => pick(g, &["x", "", "-"]).to_string(),
        _ => gen_real(g, clean),
    };
    field(g, 70, "DestroyAttachedParticlesAtHeight", v);
    let v = gen_bool(g, clean);
    field(g, 60, "SnapToGroundOnDeath", v);
    let v = match g.below(10) {
        0 => "0".to_string(),
        1..=6 => pick(g, &["33", "100", "300", "1000", "34", "67"]).to_string(),
        7 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => format!("{}", g.below(5000)),
    };
    field(g, 60, "InitialDelay", v);
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

fn bits(x: f32) -> u32 {
    x.to_bits()
}

fn gen_xy(g: &mut Gen) -> f32 {
    *pick(g, &[&-10.0f32, &0.0, &5.5, &100.0, &33.25])
}

fn gen_z(g: &mut Gen) -> f32 {
    *pick(
        g,
        &[
            &-5.0f32, &-1.0, &0.0, &0.5, &1.0, &2.0, &5.0, &10.0, &50.0, &3.0,
        ],
    )
}

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    let max = *pick(g, &[&100.0f32, &1000.0]);
    let health = *pick(g, &[&max, &max, &1.0, &0.0]);
    let cfg = vec![
        g.below(60),
        bits(gen_xy(g)),
        bits(gen_xy(g)),
        bits(gen_z(g)),
        u32::from(g.below(100) < 92),
        bits(health),
        bits(max),
        u32::from(g.below(100) < 12),
        bits(*pick(g, &[&0.5f32, &1.0, &5.0, &20.0])),
        bits(*pick(g, &[&0.5f32, &1.0, &5.0])),
        bits(*pick(g, &[&1.0f32, &5.0, &10.0, &30.0])),
        bits(*pick(g, &[&0.0f32, &2.0, &7.0, &15.0, &40.0])),
        bits(*pick(g, &[&0.0f32, &2.0, &7.0, &15.0, &40.0])),
        bits(*pick(g, &[&0.0f32, &2.0, &7.0, &15.0, &40.0])),
    ];
    let n_ops = 1 + g.below(40) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=39 => [0, 0, 0, 0, 0],
            40..=54 => [1, bits(gen_xy(g)), bits(gen_xy(g)), bits(gen_z(g)), 0],
            55..=66 => match g.below(3) {
                0 => [
                    2,
                    0,
                    g.below(8),
                    bits(*pick(g, &[&-2.0f32, &0.0, &1.0, &3.0, &8.0])),
                    0,
                ],
                1 => [2, 1, g.below(8), *pick(g, &[&1u32, &1, &2, &3, &15]), 0],
                _ => [
                    2,
                    2,
                    g.below(8),
                    bits(*pick(g, &[&-1.0f32, &0.0, &2.0, &6.0, &20.0])),
                    0,
                ],
            },
            67..=76 => [3, g.below(16), g.below(2), 0, 0],
            77..=80 => [4, g.below(2), 0, 0, 0],
            81..=85 => [5, g.below(80), 0, 0, 0],
            86..=90 => [6, 0, 0, 0, 0],
            91..=95 => [7, 0, 0, 0, 0],
            _ => [8, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    kills: u64,
    scans: u64,
    layers: u64,
    psdestroys: u64,
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
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.set_frame(cfg[0]);
    let mut modules = Vec::new();
    if cfg[4] != 0 {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
        let mut p = ProbeModule::new(
            ProbeAnswers::default(),
            f32::from_bits(cfg[6]),
            Default::default(),
        );
        p.health = f32::from_bits(cfg[5]);
        m.body = Some(UpdateBody::ModuleProbe(p));
        modules.push(m);
    }
    modules.push(
        rts_lockstep_engine::logic::modules::new_module(
            w.next_object_id(),
            b"HeightDieUpdate",
            b"ModuleTag_HeightDie",
            &data,
        )
        .unwrap()
        .unwrap(),
    );
    let values = ObjectTemplateValues {
        geometry: Some(GeometryInfo::new(
            GeometryType::Box,
            false,
            f32::from_bits(cfg[10]),
            f32::from_bits(cfg[8]),
            f32::from_bits(cfg[9]),
        )),
        ..Default::default()
    };
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: None,
            modules,
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    for k in 0..3 {
        let values = ObjectTemplateValues {
            geometry: Some(GeometryInfo::new(
                GeometryType::Box,
                false,
                f32::from_bits(cfg[11 + k]),
                1.0,
                1.0,
            )),
            ..Default::default()
        };
        let template = Arc::new(ThingTemplate::from_object_values(&values));
        let id = w
            .new_object(NewObject {
                template,
                status: Default::default(),
                team: None,
                modules: Vec::new(),
            })
            .unwrap();
        assert_eq!(id, 2 + k as i32);
    }
    w.set_position(
        me,
        &Coord3D {
            x: f32::from_bits(cfg[1]),
            y: f32::from_bits(cfg[2]),
            z: f32::from_bits(cfg[3]),
        },
    );
    w.objects.find_mut(me).unwrap().contained_by = if cfg[7] != 0 { 2 } else { INVALID_ID };
    let idx = w.find_module(me, b"HeightDieUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    for phase in 0..=ops.len() {
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
                2 => match o[1] {
                    0 => w.cuts.terrain_ground[o[2] as usize & 7] = f32::from_bits(o[3]),
                    1 => w.cuts.terrain_layer[o[2] as usize & 7] = o[3] as i32,
                    _ => w.cuts.terrain_layer_height[o[2] as usize & 7] = f32::from_bits(o[3]),
                },
                3 => {
                    let mut ids: Vec<i32> = (0..4)
                        .filter(|k| o[1] >> k & 1 != 0)
                        .map(|k| k + 1)
                        .collect();
                    if o[2] != 0 {
                        ids.reverse();
                    }
                    w.cuts.range_scan = ids;
                }
                4 => {
                    w.objects.find_mut(me).unwrap().contained_by =
                        if o[1] != 0 { 2 } else { INVALID_ID }
                }
                5 => w.set_frame(o[1]),
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
            // the constructor has no effects
            w.cuts.trace = Some(Vec::new());
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
        let trace = w.cuts.trace.take().unwrap_or_default();
        let cuts = w.cuts.log.take().unwrap_or_default();
        let mut log = Vec::new();
        let mut cut_iter = cuts.into_iter();
        for t in trace {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => {
                        log.push(format!("ground {:08x} {:08x}", x.to_bits(), y.to_bits()))
                    }
                    Some(CutCall::TerrainGetHighestLayerForDestination { x, y, z }) => {
                        tally.borrow_mut().layers += 1;
                        log.push(format!(
                            "highlayer {:08x} {:08x} {:08x}",
                            x.to_bits(),
                            y.to_bits(),
                            z.to_bits()
                        ))
                    }
                    Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!(
                        "layerheight {:08x} {:08x} {layer}",
                        x.to_bits(),
                        y.to_bits()
                    )),
                    Some(CutCall::PartitionIterateStructuresInRange { object, range }) => {
                        tally.borrow_mut().scans += 1;
                        // FROM_BOUNDINGSPHERE_3D is 3
                        log.push(format!("scan {object} {:08x} 3", range.to_bits()))
                    }
                    Some(CutCall::ParticleDestroyAttachedSystems { object }) => {
                        tally.borrow_mut().psdestroys += 1;
                        log.push(format!("psdestroy {object}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("damage ") {
                tally.borrow_mut().kills += 1;
                log.push(t);
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
                    Some(UpdateBody::HeightDieUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        let o = w.objects.find(me).unwrap();
        let p = o.get_position();
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
        let health = o.modules.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => Some(p.health.to_bits()),
            _ => None,
        });
        put_u32(&mut v, health.unwrap_or(0));
    }
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded,
    }
}

#[test]
fn height_die_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O1",
        seed: 0x00BE_16D1,
        cases_var: "L9HD_CASES",
        ini_stem: "Hd",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9hd_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O1: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} kills, {} structure scans, {} layer queries, {} particle destroys",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.kills, t.scans, t.layers, t.psdestroys
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.kills * 10 > c.cases as u64
            && t.scans * 20 > c.cases as u64
            && t.psdestroys * 20 > c.cases as u64,
        "the generator must reach every path"
    );
}
