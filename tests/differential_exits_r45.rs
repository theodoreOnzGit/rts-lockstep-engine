#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SupplyCenterProductionExitUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SpawnPointProductionExitUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SupplyCenterProductionExitUpdate` (`logic::modules::supply_center_production_exit_update`, kind 0) and
//! `SpawnPointProductionExitUpdate` (`logic::modules::spawn_point_production_exit_update`, kind 1) against the C++, `p1w13-sonnet`, batch R45.
//!
//! # Method
//!
//! The C++ side is the two files in `wrap_module.cpp`'s unit (`differential/l9_exits_r45.inc`, `ref_l9r45_run`; the spawn point's edited by
//! `build.rs` only for MSVC 6's for-loop scoping), on batch R45's scene (the producer with its position, orientation, layer and transform, three
//! objects to put out, each maybe with an AI that moves on the ground, a budding host; the terrain's heights; the pathfinder's answers), plus:
//! the producer's drawable with the test's spawn bones (positions and transforms) and its `STEALTHED` bit, each object's supply-truck AI
//! (batch R44's truck), stealth module (temporary grant or not) and `CAN_STEALTH`, and removed objects. The constructor's uninitialised
//! `m_rallyPoint` (kind 0) is set to the port's zero after construction.
//!
//! The Rust side is the real thing on a `World` as batch R45's, plus the probes for the supply truck and the stealth module, the bones in
//! `cuts.contain.pristine_bones` and `pristine_bone_transforms`, and `destroy_object` for a removal.
//!
//! Each case reads a block `D` and runs 1 to 40 ops: batch R45's (an exit, a door reservation, a budding, the rally point, the natural rally
//! point, the exit position, an update, the flags, an xfer), plus the removal of an object. Compared, bit for bit: the read's throw and the
//! data; per op the throw status, the result, the effects in order (heights, orientations, layers, the pathfinder's calls, the exit paths, the
//! forced wanting states, the stealth grants, the holds), the xfer status, bytes and CRC, the module's state (kind 1: every place's position,
//! angle and occupier) and the three objects' positions.
//!
//! # Results (2026-10-24, `L9R45_CASES=9000`)
//!
//! 9,000 cases (2,250 supply-centre and 3,604 spawn-point cases compared; 1,837 data blocks threw, as the C++ did; 1,309 excluded, undefined
//! upstream: a supply centre's exit that reads the grant its data never set, or grants stealth to a truck with no stealth module), 114,911
//! ops: 31,211 exits, 15,667 exit paths, 7,745 forced wanting states, 2,036 stealth grants, 9,047 objects held at a spawn point, 4,426
//! doors refused (every spawn point taken, or no art), 7,045 removals, 22,759 xfers; 126,619 outputs: **0 mismatches**.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2151 | the supply centre's natural rally point pushed out two cells | 7,127 |
//! | 2152 | no forced wanting state for a new supply truck | 4,297 |
//! | 2153 | stealth granted only when the truck's own grant is temporary | 306 |
//! | 2154 | the rally point's flag not saved | 3,797 |
//! | 2155 | the spawn points' angles zero | 55,379 |
//! | 2156 | dead occupiers not cleared | 8,180 |
//! | 2157 | a spawned object not held | 9,047 |
//! | 2158 | the first occupier not loaded | 3,113 |
//! | 2159 | a spawned object at the ground's height, not its layer's | 36,510 |

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
use rts_lockstep_engine::logic::modules::spawn_point_production_exit_update::SpawnPointProductionExitUpdateModuleData;
use rts_lockstep_engine::logic::modules::supply_center_production_exit_update::SupplyCenterProductionExitUpdateModuleData;
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_STEALTHED};
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r45.bin");
const XFER_FILE: &str = "l9_r45.bin";
const CLASSES: [&str; 2] = ["SupplyCenterProductionExitUpdate", "SpawnPointProductionExitUpdate"];

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
fn read_center(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::SupplyCenterProductionExitUpdate(Arc::new(SupplyCenterProductionExitUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_spawn(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpawnPointProductionExitUpdate(Arc::new(SpawnPointProductionExitUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Read) {
    let blocks = [BlockParse { token: "D", parse: if kind == 0 { read_center } else { read_spawn } }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_coord(v: &mut Vec<u8>, c: &Coord3D) {
    put_u32(v, c.x.to_bits());
    put_u32(v, c.y.to_bits());
    put_u32(v, c.z.to_bits());
}

/// The data dump of `ref_l9r45_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::SupplyCenterProductionExitUpdate(d) => {
            put_coord(v, &d.unit_create_point);
            put_coord(v, &d.natural_rally_point);
            if let Some(f) = d.grant_temporary_stealth_frames {
                put_u32(v, f);
            }
        }
        ModuleDataKind::SpawnPointProductionExitUpdate(d) => put_str(v, &d.spawn_point_bone_name_data),
        _ => unreachable!("not a production exit"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: u32, has_grant: bool, b: &[u8], n_ops: usize) -> Vec<String> {
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
    if kind == 0 {
        // the grant is dumped only when the block sets it
        let data: Vec<u32> = (0..6 + usize::from(has_grant)).map(|_| u32_(&mut p)).collect();
        out.push(format!("data {data:x?}"));
    } else {
        let name = str_(&mut p);
        out.push(format!("data {name}"));
    }
    let ns = if kind == 0 { 4 } else { 2 + 5 * 10 };
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
    if kind == 0 {
        if g.below(100) < 85 {
            lines.push(format!("UnitCreatePoint = {}", coord(g)));
        }
        if g.below(100) < 85 {
            lines.push(format!("NaturalRallyPoint = {}", coord(g)));
        }
        if g.below(100) < 92 {
            lines.push(format!("GrantTemporaryStealth = {}", pick(g, &["0", "1000", "5000", "33"])));
        }
    } else if g.below(100) < 92 {
        lines.push(format!("SpawnPointBoneName = {}", pick(g, &["SpawnBone", "SpawnBone", "Other"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "UnitCreatePoint = X:a Y:1 Z:0".to_string(),
            1 => "GrantTemporaryStealth = x".to_string(),
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
    removals: u64,
    grants: u64,
    holds: u64,
    forced: u64,
    doors_refused: u64,
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
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg: &[u32; 17], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.contain.stub_object_calls = true;
    w.cuts.body.probe_trace = true;
    w.cuts.probe_effect_trace = true;
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
        let stealth = || {
            let mut m = NewModule::plain(b"R45Stealth", b"ModuleTag_Stealth");
            m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
            m
        };
        let mods = if k < 5 { vec![ai(false), physics(Coord3D::default()), stealth()] } else { Vec::new() };
        let id = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: None, modules: mods }).unwrap();
        assert_eq!(id, k);
    }
    let ais: Vec<Option<u16>> = (1..=5).map(|k| w.objects.find(k).unwrap().ai).collect();
    // the physics behaviours are the probes named so; the scene points the objects at them
    let phys: Vec<Option<u16>> = (1..=5).map(|k| w.find_module(k, b"CtPhysics")).collect();
    let stealths: Vec<Option<u16>> = (1..=5).map(|k| w.find_module(k, b"R45Stealth")).collect();
    // the art's spawn bones (the driver's `r45Bone` and `r45Xform`)
    if let ModuleDataKind::SpawnPointProductionExitUpdate(d) = &data {
        for k in 1..=cfg[16] {
            let mut name = d.spawn_point_bone_name_data.clone();
            name.extend_from_slice(format!("{k:02}").as_bytes());
            let pos = Coord3D { x: 5.0 * k as f32 - 12.0, y: if k & 1 != 0 { -3.0 } else { 3.0 }, z: 1.0 };
            let mut m = Matrix3D::identity();
            m.rotate_z(0.3 * k as f32 - 1.0);
            m.set_translation(&Vector3::new(pos.x, pos.y, pos.z));
            w.cuts.contain.pristine_bones.insert((me, name.clone()), pos);
            w.cuts.contain.pristine_bone_transforms.insert((me, name), m);
        }
    }
    let mut last_pos = [Coord3D::default(); 5];
    let mut gone = [false; 5];
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
            if gone[k] {
                continue;
            }
            let ob = w.objects.find_mut(id).unwrap();
            ob.ai = if flags >> (k - 1) & 1 != 0 { ais[k] } else { None };
            ob.physics = if flags >> (k + 5) & 1 != 0 { phys[k] } else { None };
            if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[ais[k].unwrap() as usize].body.as_mut() {
                p.answers.ground_movement = flags >> (k + 2) & 1 != 0;
            }
        }
        w.objects.find_mut(me).unwrap().physics = if flags & 512 != 0 { phys[0] } else { None };
        for k in 1..4usize {
            let id = k as ObjectId + 1;
            let Some(ob) = w.objects.find_mut(id) else { continue };
            ob.stealth = if flags >> (k + 12) & 1 != 0 { stealths[k] } else { None };
            ob.status = Default::default();
            ob.status.set(OBJECT_STATUS_CAN_STEALTH, (flags >> (k + 18) & 1) as i32);
            if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[ais[k].unwrap() as usize].body.as_mut() {
                p.answers.supply_truck = flags >> (k + 9) & 1 != 0;
            }
            if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[stealths[k].unwrap() as usize].body.as_mut() {
                p.answers.temporary_grant = flags >> (k + 15) & 1 != 0;
            }
        }
        {
            let ob = w.objects.find_mut(me).unwrap();
            ob.status = Default::default();
            ob.status.set(OBJECT_STATUS_STEALTHED, (flags >> 22 & 1) as i32);
            ob.drawable = if flags >> 23 & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        }
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
            1 => {
                let door = w.module_call(me, idx, ModuleCall::ReserveDoorForExit { template: None, specific: 0 }).as_int(99);
                if door != 0 {
                    tally.doors_refused += 1;
                }
                res.push(door as u32);
            }
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
            15 => {
                let id = o[1] as ObjectId;
                if (2..=4).contains(&id) && !gone[id as usize - 1] {
                    gone[id as usize - 1] = true;
                    tally.removals += 1;
                    w.destroy_object(id);
                    w.process_destroy_list();
                }
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
                    CutCall::Contain(ContainCall::StealthReceiveGrant { object, active, frames }) => {
                        tally.grants += 1;
                        log.push(format!("stealthgrant {object} {} {frames}", i32::from(*active)))
                    }
                    CutCall::Contain(ContainCall::SetDisabled { object, disabled }) => {
                        tally.holds += 1;
                        log.push(format!("setdisabled {object} {disabled}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("forcewant ") {
                tally.forced += 1;
                log.push(t.clone());
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
            Some(UpdateBody::SupplyCenterProductionExitUpdate(m)) => m.state_words(),
            Some(UpdateBody::SpawnPointProductionExitUpdate(m)) => m.state_words(),
            _ => panic!("the production exit"),
        };
        for s in state {
            put_u32(&mut v, s);
        }
        for k in 2..=4usize {
            // a removed object keeps the position it had (the C++ object is only hidden from findObjectByID)
            if let Some(o) = w.objects.find(k as ObjectId) {
                last_pos[k - 1] = o.get_position();
            }
            put_coord(&mut v, &last_pos[k - 1]);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R45_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn production_exits_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0045_E417);
    let mut read = Check::new("production exits R45 data read (throw, message)");
    let mut run = Check::new("production exits R45 ops (data, results, effects, xfers, state, positions)");
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
        let name = format!("Data\\INI\\R45{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let gen_flags = |g: &mut Gen| (g.u32() & 0xff_ffff) | 7 | u32::from(g.below(10) != 0) << 23;
        let has_grant = matches!(&data, Some(ModuleDataKind::SupplyCenterProductionExitUpdate(d)) if d.grant_temporary_stealth_frames.is_some());
        let mut cfg = [0u32; 17];
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
        cfg[16] = *pick(&mut g, &[&0u32, &1, &2, &3, &5, &10, &12]);
        let n_ops = 1 + g.below(40) as usize;
        let mut removed = [false; 5];
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(32) {
                0..=6 => {
                    // an exit of an object the test has not removed (upstream reads the new object unchecked)
                    let live: Vec<u32> = (2..=4).filter(|&k| !removed[k as usize]).collect();
                    match live.len() {
                        0 => [1, 0, 0, 0, 0, 0],
                        n => [0, live[g.below(n as u32) as usize], 0, 0, 0, 0],
                    }
                }
                30..=31 => {
                    let k = 2 + g.below(3);
                    removed[k as usize] = true;
                    [15, k, 0, 0, 0, 0]
                }
                7..=8 => [1, 0, 0, 0, 0, 0],
                9 => [2, 0, 0, 0, 0, 0],
                10..=11 => [1, 0, 0, 0, 0, 0],
                12..=13 => [4, pick(&mut g, &fl).to_bits(), pick(&mut g, &fl).to_bits(), 0, 0, 0],
                14 => [5, 0, 0, 0, 0, 0],
                15 => [if kind == 0 { 6 } else { 9 }, 0, 0, 0, 0, 0],
                // (a spawn point's are ExitInterface's: FALSE, the coordinate left uninitialised)
                16 => if kind == 0 { [7, g.below(2), 0, 0, 0, 0] } else { [9, 0, 0, 0, 0, 0] },
                17 => if kind == 0 { [8, 0, 0, 0, 0, 0] } else { [1, 0, 0, 0, 0, 0] },
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
            cpp::ref_l9r45_run(
                kind as i32,
                i32::from(has_grant),
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
        let (dr, dc) = (decode(kind, has_grant, &rd, ops.len()), decode(kind, has_grant, &buf, ops.len()));
        for l in &dr {
            for key in ["layerheight", "ground", "setorient", "setlayer", "pathadd", "aiexitpath", "forcewant", "stealthgrant", "setdisabled"] {
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
        "production exits R45: {cases} cases ({compared:?} compared per class), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} exits, {} exit paths, {} xfers, {} effect lines",
        tally.exits, tally.paths, tally.xfers, tally.effects
    );
    println!(
        "R45: {} removals, {} doors refused, {} stealth grants, {} holds, {} forced wanting states",
        tally.removals, tally.doors_refused, tally.grants, tally.holds, tally.forced
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        tally.paths * 4 > cases as u64 && tally.holds * 4 > cases as u64 && tally.doors_refused * 10 > cases as u64 && tally.grants * 20 > cases as u64 && tally.forced * 10 > cases as u64,
        "the generator must reach every path"
    );
}
