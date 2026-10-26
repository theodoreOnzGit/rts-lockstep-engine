#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `DefaultProductionExitUpdate` and `QueueProductionExitUpdate` (`logic::modules::{default,queue}_production_exit_update`) against the unchanged
//! C++, `p1w13-sonnet`, batch R18.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_exits_r18.inc`) on the module build's
//! stand-ins: the producer (id 1: position, orientation, layer, the transform made from them, maybe physics with a velocity), three objects to put
//! out (ids 2 to 4: each maybe an AI that moves on the ground or not, maybe physics with a mass and a centre of mass offset), a budding host (id
//! 5), the terrain's two height tables, and a pathfinder whose `addObjectToPathfindMap`, `adjustDestination` and `snapPosition` log and answer
//! from the test.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule`s for the AIs and the
//! physics), `cuts.contain.stub_object_calls` set (the moves are records; the positions are kept), the terrain tables and the pathfinder's answers
//! in the cut tables, every op a `World::module_call` or an `update()` as the scheduler runs it.
//!
//! Each case reads a block `D` (kind 0: `UnitCreatePoint`, `NaturalRallyPoint`, `UseSpawnRallyPoint`; kind 1: `UnitCreatePoint`,
//! `NaturalRallyPoint`, `ExitDelay`, `AllowAirborneCreation`, `InitialBurst`), mostly well formed, and runs 1 to 20 ops. Compared, bit for bit:
//! the read's throw; the data; per op the throw status, the result words, the effects in order (`layerheight`, `ground`, `setorient`,
//! `setlayer`, `pathadd`, `snap`, `adjust`, `aiexitpath`, `aimove`, `pmotive`, `ppitch`), the xfer status, bytes and CRC, the module's state and
//! the positions of the objects put out.
//!
//! # Results (2026-10-23, `L9R18_CASES=9000`)
//!
//! 9,000 cases (3,548 `DefaultProductionExitUpdate`, 3,605 `QueueProductionExitUpdate` compared, none excluded; 1,847 data files threw, as the
//! C++ did), 76,068 ops (22,901 exits, 12,585 xfers) and 90,374 op outputs: **0 mismatches**. Effect lines compared: setorient 20,391, aiexitpath
//! and pathadd 17,868 each, ground 12,605, setlayer 10,537, snap 9,015, layerheight 8,853, adjust 2,715, aimove 2,523, pmotive and ppitch 1,191
//! each.
//!
//! Harness correction on the way, none in the translation: the saved scheduling word of `QueueProductionExitUpdate` (whose constructor sets no
//! wake frame) differed in 2,589 outputs; the driver now gives the bare C++ module the next frame, as the Rust world schedules an update module
//! when it makes the object (the batch R13e correction).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1841 | the default exit ignores the rally point | 1,329 |
//! | 1842 | the default exit's rally flag not saved | 2,872 |
//! | 1843 | the queue's burst does not override the delay | 556 |
//! | 1844 | the queue's second natural point dropped without a rally point | 6,146 |
//! | 1845 | the queue's delay counted down two a frame | 3,829 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::default_production_exit_update::DefaultProductionExitUpdateModuleData;
use rts_lockstep_engine::logic::modules::queue_production_exit_update::QueueProductionExitUpdateModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r18.bin");
const XFER_FILE: &str = "l9_r18.bin";
const CLASSES: [&str; 2] = ["DefaultProductionExitUpdate", "QueueProductionExitUpdate"];

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
type Read = Option<ModuleDataKind>;
fn read_default(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::DefaultProductionExitUpdate(Arc::new(DefaultProductionExitUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_queue(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::QueueProductionExitUpdate(Arc::new(QueueProductionExitUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Read) {
    let blocks = [BlockParse { token: "D", parse: if kind == 0 { read_default } else { read_queue } }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_coord(v: &mut Vec<u8>, c: &Coord3D) {
    put_u32(v, c.x.to_bits());
    put_u32(v, c.y.to_bits());
    put_u32(v, c.z.to_bits());
}

/// The data dump of `ref_l9r18_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::DefaultProductionExitUpdate(d) => {
            put_coord(v, &d.unit_create_point);
            put_coord(v, &d.natural_rally_point);
            put_u32(v, u32::from(d.use_spawn_rally_point));
        }
        ModuleDataKind::QueueProductionExitUpdate(d) => {
            put_coord(v, &d.unit_create_point);
            put_coord(v, &d.natural_rally_point);
            put_u32(v, d.exit_delay_data);
            put_u32(v, u32::from(d.allow_airborne_creation_data));
            put_u32(v, d.initial_burst);
        }
        _ => unreachable!("not a production exit"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: u32, b: &[u8], n_ops: usize) -> Vec<String> {
    let mut p = 0usize;
    let mut out = Vec::new();
    let u32_ = |p: &mut usize| -> u32 {
        let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
        *p += 4;
        v
    };
    let str_ = |p: &mut usize| -> String {
        let n = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap()) as usize;
        *p += 4;
        let s = String::from_utf8_lossy(&b[*p..*p + n]).into_owned();
        *p += n;
        s
    };
    if b.is_empty() {
        return out;
    }
    let nd = if kind == 0 { 7 } else { 9 };
    let data: Vec<u32> = (0..nd).map(|_| u32_(&mut p)).collect();
    out.push(format!("data {data:x?}"));
    let ns = if kind == 0 { 4 } else { 7 };
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let nr = u32_(&mut p);
        let res: Vec<u32> = (0..nr).map(|_| u32_(&mut p)).collect();
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..ns).map(|_| u32_(&mut p)).collect();
        let pos: Vec<u32> = (0..9).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:x?} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:x?} pos {pos:x?}"));
    }
    out
}

// -- generators --

fn coord(g: &mut Gen) -> String {
    let c = [&"0", &"10", &"-12.5", &"25", &"3", &"0.5"];
    format!("X:{} Y:{} Z:{}", pick(g, &c), pick(g, &c), pick(g, &[&"0", &"0", &"5", &"-2"]))
}

/// The block `D` of `kind`, each field present with some probability; now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 85 {
        lines.push(format!("UnitCreatePoint = {}", coord(g)));
    }
    if g.below(100) < 85 {
        lines.push(format!("NaturalRallyPoint = {}", coord(g)));
    }
    if kind == 0 {
        if g.below(100) < 40 {
            lines.push(format!("UseSpawnRallyPoint = {}", gen_bool(g, clean)));
        }
    } else {
        if g.below(100) < 70 {
            lines.push(format!("ExitDelay = {}", pick(g, &["0", "33", "100", "500"])));
        }
        if g.below(100) < 40 {
            lines.push(format!("AllowAirborneCreation = {}", gen_bool(g, clean)));
        }
        if g.below(100) < 50 {
            lines.push(format!("InitialBurst = {}", pick(g, &["0", "1", "2", "5"])));
        }
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "UnitCreatePoint = X:a Y:1 Z:0".to_string(),
            1 => "NaturalRallyPoint".to_string(),
            _ => "NoSuchField = 1".to_string(),
        });
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    exits: u64,
    paths: u64,
    xfers: u64,
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(512);
            let mut x = XferLoad::new(b);
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg: &[u32; 16], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.contain.stub_object_calls = true;
    w.cuts.body.probe_trace = true;
    w.cuts.terrain_ground = [f32::from_bits(cfg[14]); 8];
    w.cuts.terrain_layer_height = [f32::from_bits(cfg[15]); 8];
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let physics = |velocity: Coord3D| {
        let mut p = NewModule::plain(b"CtPhysics", b"ModuleTag_Physics");
        let answers = ProbeAnswers { velocity, mass: f32::from_bits(cfg[6]), center_of_mass_offset: f32::from_bits(cfg[7]), ..Default::default() };
        p.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        p
    };
    let ai = |ground: bool| {
        let mut a = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
        a.is_ai = true;
        a.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { ground_movement: ground, ..Default::default() }, 0.0, Default::default())));
        a
    };
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASSES[kind as usize].as_bytes(), b"ModuleTag_Exit", &data).unwrap().unwrap();
    let me = w
        .new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: None, modules: vec![m, physics(Coord3D { x: 2.0, y: -1.0, z: f32::from_bits(cfg[5]) })] })
        .unwrap();
    assert_eq!(me, 1);
    for k in 2..=5 {
        let mods = if k < 5 { vec![ai(false), physics(Coord3D::default())] } else { Vec::new() };
        let id = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: None, modules: mods }).unwrap();
        assert_eq!(id, k);
    }
    let ais: Vec<Option<u16>> = (1..=5).map(|k| w.objects.find(k).unwrap().ai).collect();
    // the physics behaviours are the probes named so; the scene points the objects at them
    let phys: Vec<Option<u16>> = (1..=5).map(|k| w.find_module(k, b"CtPhysics")).collect();
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[0]), y: f32::from_bits(cfg[1]), z: 0.0 });
    w.contain_scene_set_orientation(me, f32::from_bits(cfg[2]));
    w.objects.find_mut(me).unwrap().layer = cfg[3] as i32;
    w.set_position(5, &Coord3D { x: -7.5, y: 12.0, z: 3.0 });
    w.contain_scene_set_orientation(5, 0.5);
    w.objects.find_mut(5).unwrap().layer = 2;
    if cfg[8] & 1 != 0 {
        for k in 2..=4 {
            w.cuts.contain.adjust_destination.insert(k, Coord3D { x: f32::from_bits(cfg[9]), y: f32::from_bits(cfg[10]), z: 0.0 });
        }
    }
    w.cuts.contain.snap_position = (cfg[11] & 1 != 0).then(|| Coord3D { x: f32::from_bits(cfg[12]), y: f32::from_bits(cfg[13]), z: 0.0 });
    let idx = w.find_module(me, CLASSES[kind as usize].as_bytes()).unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let mut flags = cfg[4];
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 11 {
            flags = o[1];
        }
        for k in 1..4usize {
            let id = k as ObjectId + 1;
            let ob = w.objects.find_mut(id).unwrap();
            ob.ai = if flags >> (k - 1) & 1 != 0 { ais[k] } else { None };
            ob.physics = if flags >> (k + 5) & 1 != 0 { phys[k] } else { None };
            if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[ais[k].unwrap() as usize].body.as_mut() {
                p.answers.ground_movement = flags >> (k + 2) & 1 != 0;
            }
        }
        w.objects.find_mut(me).unwrap().physics = if flags & 512 != 0 { phys[0] } else { None };
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        let mut res: Vec<u32> = Vec::new();
        let coord_words = |r: ModuleReply| -> Vec<u32> {
            match r {
                ModuleReply::Coord(c) => vec![1, c.x.to_bits(), c.y.to_bits(), c.z.to_bits()],
                _ => panic!("a coordinate"),
            }
        };
        match o[0] {
            0 => {
                w.module_call(me, idx, ModuleCall::ExitObjectViaDoor { obj: o[1] as ObjectId, door: 0 });
                tally.exits += 1;
            }
            1 => res.push(w.module_call(me, idx, ModuleCall::ReserveDoorForExit { template: None, specific: 0 }).as_int(99) as u32),
            2 => {
                w.module_call(me, idx, ModuleCall::UnreserveDoorForExit(0));
            }
            3 => {
                w.module_call(me, idx, ModuleCall::ExitObjectByBudding { obj: o[1] as ObjectId, bud_host: o[2] as ObjectId });
                tally.exits += 1;
            }
            4 => {
                w.module_call(me, idx, ModuleCall::SetRallyPoint(Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }));
            }
            5 => {
                if let ModuleReply::Coord(c) = w.module_call(me, idx, ModuleCall::GetRallyPoint) {
                    res.extend([c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]);
                }
            }
            6 => res.push(u32::from(w.module_call(me, idx, ModuleCall::UseSpawnRallyPoint).as_bool())),
            7 => res = coord_words(w.module_call(me, idx, ModuleCall::GetNaturalRallyPointWithOffset(o[1] != 0))),
            8 => res = coord_words(w.module_call(me, idx, ModuleCall::GetExitPosition)),
            9 => res.push(w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0),
            10 => res.push(u32::from(w.module_call(me, idx, ModuleCall::IsExitBusy).as_bool())),
            11 => {}
            12..=14 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 12, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 12 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res.len() as u32);
        for r in &res {
            put_u32(&mut v, *r);
        }
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        let bits = |c: &Coord3D| format!("{:08x} {:08x} {:08x}", c.x.to_bits(), c.y.to_bits(), c.z.to_bits());
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::TerrainGetGroundHeight { x, y } => log.push(format!("ground {:08x} {:08x}", x.to_bits(), y.to_bits())),
                    CutCall::TerrainGetLayerHeight { x, y, layer } => log.push(format!("layerheight {:08x} {:08x} {layer}", x.to_bits(), y.to_bits())),
                    CutCall::Contain(ContainCall::SetOrientation { object, angle }) => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                    CutCall::Contain(ContainCall::SetLayer { object, layer }) => log.push(format!("setlayer {object} {layer}")),
                    CutCall::Contain(ContainCall::PathfindAddObject { object }) => log.push(format!("pathadd {object}")),
                    CutCall::Contain(ContainCall::PathfindAdjustDestination { object, before }) => log.push(format!("adjust {object} {}", bits(before))),
                    CutCall::Contain(ContainCall::PathfindSnapPosition { object, before }) => log.push(format!("snap {object} {}", bits(before))),
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                if r.starts_with("aiexitpath ") {
                    tally.paths += 1;
                }
                log.push(r.to_string());
            }
        }
        tally.effects += log.len() as u64;
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
        let ob = w.objects.find(me).unwrap();
        let slot = ob.modules[idx as usize].update_index.unwrap() as usize;
        let state = match ob.behaviors[slot].body.as_ref() {
            Some(UpdateBody::DefaultProductionExitUpdate(m)) => m.state_words(),
            Some(UpdateBody::QueueProductionExitUpdate(m)) => m.state_words(),
            _ => panic!("the production exit"),
        };
        for s in state {
            put_u32(&mut v, s);
        }
        for k in 2..=4 {
            let p = w.objects.find(k).unwrap().get_position();
            put_coord(&mut v, &p);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R18_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn production_exits_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0018_E417);
    let mut read = Check::new("production exits R18 data read (throw, message)");
    let mut run = Check::new("production exits R18 ops (data, results, effects, xfers, state, positions)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut compared = [0u64; 2];
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let fl = [&0.0f32, &10.0, &-12.5, &25.0, &3.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = (case % 2) as u32;
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R18{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let gen_flags = |g: &mut Gen| (g.u32() & 0x3ff) | 7;
        let mut cfg = [0u32; 16];
        cfg[0] = pick(&mut g, &fl).to_bits();
        cfg[1] = pick(&mut g, &fl).to_bits();
        cfg[2] = (*pick(&mut g, &[&0.0f32, &1.5707964, &-0.75, &3.0])).to_bits();
        cfg[3] = *pick(&mut g, &[&1u32, &1, &2, &0]);
        cfg[4] = gen_flags(&mut g);
        cfg[5] = (*pick(&mut g, &[&0.0f32, &4.0])).to_bits();
        cfg[6] = (*pick(&mut g, &[&1.0f32, &2.5])).to_bits();
        cfg[7] = (*pick(&mut g, &[&0.0f32, &3.0])).to_bits();
        cfg[8] = g.below(2);
        cfg[9] = pick(&mut g, &fl).to_bits();
        cfg[10] = pick(&mut g, &fl).to_bits();
        cfg[11] = g.below(2);
        cfg[12] = pick(&mut g, &fl).to_bits();
        cfg[13] = pick(&mut g, &fl).to_bits();
        cfg[14] = (*pick(&mut g, &[&0.0f32, &0.0, &5.0])).to_bits();
        cfg[15] = (*pick(&mut g, &[&0.0f32, &2.0])).to_bits();
        let n_ops = 1 + g.below(20) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(30) {
                0..=6 => [0, 2 + g.below(3), 0, 0, 0, 0],
                7..=8 => [1, 0, 0, 0, 0, 0],
                9 => [2, 0, 0, 0, 0, 0],
                10..=11 => [3, 2 + g.below(3), *pick(&mut g, &[&5u32, &5, &0]), 0, 0, 0],
                12..=13 => [4, pick(&mut g, &fl).to_bits(), pick(&mut g, &fl).to_bits(), 0, 0, 0],
                14 => [5, 0, 0, 0, 0, 0],
                15 => [if kind == 0 { 6 } else { 9 }, 0, 0, 0, 0, 0],
                16 => [7, g.below(2), 0, 0, 0, 0],
                17 => [8, 0, 0, 0, 0, 0],
                18..=21 => [9, 0, 0, 0, 0, 0],
                22 => [10, 0, 0, 0, 0, 0],
                23..=24 => [11, gen_flags(&mut g), 0, 0, 0, 0],
                25..=26 => [12, 0, 0, 0, 0, 0],
                27..=28 => [13, 0, 0, 0, 0, 0],
                _ => [14, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, kind, data.expect("read ok"), &cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r18_run(
                kind as i32,
                cfile.as_ptr(),
                cxfer.as_ptr(),
                cfg.as_ptr(),
                ops.len() as i32,
                flat.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut len,
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut code,
            )
        };
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || format!("case {case} kind {kind}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["layerheight", "ground", "setorient", "setlayer", "pathadd", "snap", "adjust", "aiexitpath", "aimove", "pmotive", "ppitch"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key} ")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} kind {kind} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared[kind as usize] += 1;
    }
    println!(
        "production exits R18: {cases} cases ({compared:?} compared per class), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} exits, {} exit paths, {} xfers, {} effect lines",
        tally.exits, tally.paths, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(tally.paths * 4 > cases as u64, "the generator must reach the exit paths");
}
