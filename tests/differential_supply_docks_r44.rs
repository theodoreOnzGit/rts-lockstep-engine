#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyWarehouseDockUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyCenterDockUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SupplyWarehouseDockUpdate` (`logic::modules::supply_warehouse_dock_update`, kind 0) and `SupplyCenterDockUpdate`
//! (`logic::modules::supply_center_dock_update`, kind 1) against the unchanged C++, `p1w13-sonnet`, batch R44.
//!
//! # Method
//!
//! The C++ side is the two files in `wrap_module.cpp`'s unit (`differential/l9_supply_docks_r44.inc`, `ref_l9r44_run`) on batch R44's
//! `DockUpdate` and scene (the dock, its art, eleven dockers with bodies, drawables, kinds and locomotors), plus: the dock's controlling player
//! (of three, with money, score and colour), its `STEALTHED` and `DETECTED` bits, the local player; each docker's AI (maybe, a supply truck's or
//! not: its boxes, whether it takes one, its boost; each call a line), its stealth module (whether the grant is temporary) and `CAN_STEALTH` bit;
//! the docker-to-dock distances, the ground's heights, the box value, the REAL `Object::kill` and `RandomValue`.
//!
//! The Rust side is the real thing on a `World` as batch R44's, plus: the dock's team mapped to its player (`cuts.team_players`), the players'
//! colours, the local player, `cuts.globals.base_value_per_supply_box`, `cuts.body.distance_squared`, `cuts.terrain_ground`, and probes for
//! the AI (with `supply_truck`, `supply_boxes`, `gain_box`, `supply_boost`) and the stealth module (`temporary_grant`).
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 200 ops: batch R44's (the action is this kind's; a load is followed by
//! `loadPostProcess`), and for a warehouse `setCashValue`, `getBoxesStored` and `onObjectCreated`. Compared, bit for bit: batch R44's
//! outputs, the warehouse's boxes, every docker's position (a truck too far is nudged), every truck's boxes, and the effects in order (the
//! distance queries, the box calls, the deposits, scores and floating texts, the ground heights, the stealth grants, the kills, idles and
//! forced states, the destructions, the drawable's supply status).
//!
//! # Results (2026-10-24, `L9R44_CASES=9000`)
//!
//! 9,000 cases, both kinds (5,811 compared; 1,788 data blocks threw, as the C++ did; 1,401 excluded, undefined upstream: 735 approach
//! indices outside the list, 297 approach lists longer than the art's bones, 162 boxes or money for a dock with no controlling player,
//! 140 dockers with no AI, 65 cash values over a box value of 0, 2 kills of a body-less truck), 552,790 ops; 4,720 boxes taken, 6,007
//! trucks nudged closer, 12,968 deposits, 10,985 floating texts, 956 stealth grants, 319 trucks killed and 263 sent idle by a crippled
//! warehouse, 346 empty warehouses destroyed, 12,770 supply statuses; 1,669,992 outputs: **0 mismatches**.
//!
//! On the way (test harness only): the generator named no local player, which the port's `setLocalPlayer(NULL)` makes player 0 (as
//! upstream's does), so a stealthed centre's money showed in Rust; every scene now has a local player.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught. Under a plant the scenes avoid docks with no player, dockers
//! with no AI or body and a box value of 0: a planted box count can lead the C++ down a path that is undefined there but no longer flagged
//! in Rust (it crashed under 2141-2144 before).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2141 | the warehouse's boxes not saved | 52,120 |
//! | 2142 | no distance check before handing a box | 215,687 |
//! | 2143 | a refused box not put back | 92,667 |
//! | 2144 | the truck inside a crippled warehouse not killed | 9,862 |
//! | 2145 | each box worth 1, not the box value | 8,112 |
//! | 2146 | stealth granted only when the truck's own grant is temporary | 385 |
//! | 2147 | a stealthed centre always showing the money | 2,142 |
//! | 2148 | the floating text at the truck's height, not the ground's | 11,264 |
//! | 2149 | the nudge up to 3, not 4 | 6,727 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODELCONDITION_DOCKING_BEGINNING};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::supply_center_dock_update::SupplyCenterDockUpdateModuleData;
use rts_lockstep_engine::logic::modules::supply_warehouse_dock_update::SupplyWarehouseDockUpdateModuleData;
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r44.bin");
const XFER_FILE: &str = "l9_r44.bin";

/// The radii, by index (the driver's).
const RADII: [f32; 4] = [5.0, 20.0, 1.0, 0.0];
/// The position a call starts with (the driver's).
const SENTINEL: Coord3D = Coord3D { x: 77.0, y: 77.0, z: 77.0 };

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
fn read_warehouse(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SupplyWarehouseDockUpdate(Arc::new(SupplyWarehouseDockUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_center(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SupplyCenterDockUpdate(Arc::new(SupplyCenterDockUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: if kind == 0 { read_warehouse } else { read_center } }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
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
    let head: Vec<u32> = (0..if kind == 0 { 4 } else { 3 }).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} constructor {log:?} wakes {w} last {l}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let written: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let ns = u32_(&mut p);
        let state: Vec<u32> = (0..ns).map(|_| u32_(&mut p)).collect();
        let health: Vec<u32> = (0..11).map(|_| u32_(&mut p)).collect();
        let positions: Vec<u32> = (0..22).map(|_| u32_(&mut p)).collect();
        let boxes: Vec<u32> = (0..11).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} written {written:x?} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} health {health:x?}"));
        out.push(format!("op {i} positions {positions:x?} boxes {boxes:?}"));
    }
    out
}

fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 90 {
        lines.push(format!("NumberApproachPositions = {}", pick(g, &["-1", "-1", "-1", "0", "1", "2", "3", "3", "5", "10"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("AllowsPassthrough = {}", pick(g, &["Yes", "No"])));
    }
    if kind == 0 {
        if g.below(100) < 80 {
            lines.push(format!("StartingBoxes = {}", pick(g, &["1", "3", "10", "0", "40"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("DeleteWhenEmpty = {}", pick(g, &["Yes", "No"])));
        }
    } else if g.below(100) < 70 {
        lines.push(format!("GrantTemporaryStealth = {}", pick(g, &["0", "1000", "5000", "33"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "NumberApproachPositions = x", "StartingBoxes = many", "DeleteWhenEmpty = maybe", "GrantTemporaryStealth"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// What the compared cases reached (a case excluded as undefined upstream counts for nothing).
#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    admissions: u64,
    reserves: u64,
    reserved: u64,
    grown: u64,
    advances: u64,
    findpos: u64,
    biased: u64,
    model: u64,
    actions_true: u64,
    xfers: u64,
    boxes_taken: u64,
    nudges: u64,
    deposits: u64,
    floats: u64,
    grants: u64,
    kills: u64,
    idles: u64,
    destroys: u64,
    statuses: u64,
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
            b.truncate(4096);
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

fn template_of(kinds: &[KindOfType]) -> Arc<ThingTemplate> {
    let values = ObjectTemplateValues { name: b"R44Thing".to_vec(), kind_of: kinds.to_vec(), ..Default::default() };
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The art's bones (the driver's `r44Bone`).
fn bones(b: u32) -> Vec<(Vec<u8>, Coord3D)> {
    let c = |x, y, z| Coord3D { x, y, z };
    let mut out = Vec::new();
    if b & 1 != 0 {
        out.push((b"DockStart".to_vec(), if b & 8 != 0 { c(0.0, 0.0, 0.0) } else { c(2.0, 3.0, 0.0) }));
    }
    if b & 2 != 0 {
        out.push((b"DockAction".to_vec(), c(0.0, 5.0, 1.0)));
    }
    if b & 4 != 0 {
        out.push((b"DockEnd".to_vec(), c(-2.0, 3.0, 0.0)));
    }
    for k in 1..=(b >> 8 & 15) {
        out.push((format!("DockWaiting{k:02}").into_bytes(), c(4.0 * k as f32, -6.0, 0.5)));
    }
    out
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 72], slots: &Slots) {
    let f = cfg[1];
    let mut kinds = Vec::new();
    if f & 2 != 0 {
        kinds.push(KINDOF_SUPPLY_SOURCE);
    }
    if f & 4 != 0 {
        kinds.push(KINDOF_IGNORE_DOCKING_BONES);
    }
    {
        let o = w.objects.find_mut(1).unwrap();
        o.thing.friend_set_template(template_of(&kinds));
        o.drawable = if f & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        let r = RADII[(cfg[2] & 3) as usize];
        o.geometry_info = GeometryInfo::new(GeometryType::Cylinder, false, 10.0, r, r);
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[3]), y: f32::from_bits(cfg[4]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[5]));
    w.cuts.contain.pristine_bones.retain(|(id, _), _| *id != 1);
    for (name, p) in bones(cfg[6]) {
        w.cuts.contain.pristine_bones.insert((1, name), p);
    }
    for k in 1..12usize {
        let id = k as ObjectId + 1;
        let g = cfg[9 + k];
        let mut kinds = Vec::new();
        if g & 8 != 0 {
            kinds.push(KINDOF_DOZER);
        }
        if g & 16 != 0 {
            kinds.push(KINDOF_HARVESTER);
        }
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(template_of(&kinds));
            o.drawable = if g & 2 != 0 { 7 + k as i32 } else { INVALID_DRAWABLE_ID };
        }
        let mut flags = ModelConditionFlags::default();
        if g & 4 != 0 {
            flags.set(MODELCONDITION_DOCKING_BEGINNING, 1);
        }
        w.reads.drawable_model_condition.insert(id, flags);
        if g & 32 != 0 {
            w.cuts.ai_airborne.insert(id);
        } else {
            w.cuts.ai_airborne.remove(&id);
        }
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[20 + k]), y: 3.0, z: 0.0 });
    }
    w.cuts.contain.find_position = (cfg[7] & 1 != 0).then(|| Coord3D { x: f32::from_bits(cfg[8]), y: f32::from_bits(cfg[9]), z: 0.0 });
    // batch R44: the dock's player and status, the local player, the box value, the dockers' AI and stealth, the distances, the ground
    let dk = cfg[43];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = dk & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_STEALTHED, (dk >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_DETECTED, (dk >> 3 & 1) as i32);
    }
    w.players.set_local_player((dk >> 4 & 3 != 0).then(|| (dk >> 4 & 3) as i32 - 1));
    w.cuts.globals.base_value_per_supply_box = cfg[44];
    for k in 1..12usize {
        let id = k as ObjectId + 1;
        let a = cfg[44 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.ai = if a & 1 != 0 { slots.ai[k] } else { None };
            o.stealth = if a & 8 != 0 { slots.stealth[k] } else { None };
            o.status = Default::default();
            o.status.set(OBJECT_STATUS_CAN_STEALTH, (a >> 5 & 1) as i32);
            for (slot, f) in [(slots.ai[k], 0), (slots.stealth[k], 1)] {
                let Some(UpdateBody::ModuleProbe(p)) = o.modules[slot.unwrap() as usize].body.as_mut() else { unreachable!() };
                if f == 0 {
                    p.answers.supply_truck = a & 2 != 0;
                    p.answers.gain_box = a & 4 != 0;
                } else {
                    p.answers.temporary_grant = a & 16 != 0;
                }
            }
        }
        w.cuts.body.distance_squared.insert((id, 1), (a >> 8 & 0xff) as f32 * 0.25);
    }
    for c in 0..8 {
        w.cuts.terrain_ground[c] = (cfg[56] >> (4 * c) & 15) as f32 * 0.5;
    }
}

/// The probes' slots on dockers 2-12.
struct Slots {
    ai: [Option<u16>; 12],
    stealth: [Option<u16>; 12],
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, tally: &mut Tally) -> Vec<String> {
    let events: Vec<Option<ObjectClientEvent>> = w.events.events()[events_before..]
        .iter()
        .map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Contain(ContainCall::PartitionFindPositionAroundFor { position: p, min_radius, max_radius, flags, source_to_path_to_dest, ignore_object })) => {
                    tally.findpos += 1;
                    log.push(format!(
                        "findpos {:08x} {:08x} {:08x} {:08x} {:08x} {flags} {source_to_path_to_dest} {ignore_object}",
                        b(p.x),
                        b(p.y),
                        b(p.z),
                        b(min_radius),
                        b(max_radius)
                    ));
                }
                Some(CutCall::Contain(ContainCall::StealthReceiveGrant { object, active, frames })) => {
                    tally.grants += 1;
                    log.push(format!("stealthgrant {object} {} {frames}", i32::from(active)));
                }
                Some(CutCall::Body(BodyCall::GetDistanceSquared { object, other })) => log.push(format!("distsq {object} {other}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    tally.destroys += 1;
                    log.push(format!("destroy {object}"));
                }
                Some(CutCall::AiIdle { object }) => {
                    tally.idles += 1;
                    log.push(format!("aiidle {object}"));
                }
                Some(CutCall::PlayerMoneyDeposit { player, amount }) => {
                    tally.deposits += 1;
                    log.push(format!("deposit {player} {amount} 1"));
                }
                Some(CutCall::PlayerScoreAddMoneyEarned { player, amount }) => log.push(format!("earned {player} {amount}")),
                Some(CutCall::InGameUiAddFloatingText { amount, position, color }) => {
                    tally.floats += 1;
                    log.push(format!("float GUI:AddCash {amount} {},{},{} {:08x}", b(position.x), b(position.y), b(position.z), color as u32));
                }
                Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(Some(ObjectClientEvent::DrawableModelCondition { object_id, op })) => {
                    tally.model += 1;
                    log.push(match op {
                        ModelConditionOp::ClearAndSet { clear, set } => format!("mc {object_id} clear {clear:?} set {set:?}"),
                        ModelConditionOp::Clear(clear) => format!("mc {object_id} clear {clear:?}"),
                        ModelConditionOp::Set(set) => format!("mc {object_id} set {set:?}"),
                    });
                }
                Some(Some(ObjectClientEvent::DrawableUpdateSupplyStatus { object_id, max_boxes, current_boxes })) => {
                    tally.statuses += 1;
                    log.push(format!("supply {object_id} {max_boxes} {current_boxes}"));
                }
                _ => {}
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t.clone());
        } else if t.starts_with("gainbox ") || t.starts_with("losebox ") || t.starts_with("forcewant ") {
            if t.starts_with("gainbox ") && t.ends_with(" 1") {
                tally.boxes_taken += 1;
            }
            log.push(t.clone());
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg_in: &[u32; 72], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    match &data {
        ModuleDataKind::SupplyWarehouseDockUpdate(d) => {
            put_i32(&mut v, d.dock.number_approach_positions_data);
            put_u32(&mut v, u32::from(d.dock.is_allow_passthrough));
            put_i32(&mut v, d.starting_boxes_data);
            put_u32(&mut v, u32::from(d.delete_when_empty));
        }
        ModuleDataKind::SupplyCenterDockUpdate(d) => {
            put_i32(&mut v, d.dock.number_approach_positions_data);
            put_u32(&mut v, u32::from(d.dock.is_allow_passthrough));
            put_u32(&mut v, d.grant_temporary_stealth_frames);
        }
        _ => unreachable!(),
    }
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[0].wrapping_mul(7919).wrapping_add(1));
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_colors.insert((t - 1) as i32, 0x0011_2233 * t as i32);
    }
    let name: &[u8] = if kind == 0 { b"SupplyWarehouseDockUpdate" } else { b"SupplyCenterDockUpdate" };
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, max: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        m
    };
    let mut body_slot = [None; 12];
    let mut slots = Slots { ai: [None; 12], stealth: [None; 12] };
    let initial_health = |k: usize| (cfg_in[31 + k] & 0xffff) as f32 * 0.5;
    for k in 0..12usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, b"ModuleTag_Me", &data).unwrap().unwrap());
        } else {
            if cfg_in[9 + k] & 1 != 0 {
                modules.push(probe(b"R44Body", ProbeAnswers::default(), MODULEINTERFACE_BODY, (cfg_in[31 + k] >> 16) as f32 * 0.5));
            }
            let answers = ProbeAnswers { supply_boxes: (cfg_in[56 + k] & 0xff) as i32, supply_boost: i32::from((cfg_in[56 + k] >> 8 & 0xff) as u8 as i8), ..Default::default() };
            let mut ai = probe(b"R44Ai", answers, 0, 0.0);
            ai.is_ai = true;
            modules.push(ai);
            modules.push(probe(b"R44Stealth", ProbeAnswers::default(), 0, 0.0));
        }
        let id = w.new_object(NewObject { template: template_of(&[]), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        body_slot[k] = w.find_module(id, b"R44Body");
        slots.ai[k] = w.find_module(id, b"R44Ai");
        slots.stealth[k] = w.find_module(id, b"R44Stealth");
        if let Some(o) = w.objects.find_mut(id) {
            o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, 3.0, 3.0, 3.0);
        }
        if let Some(s) = body_slot[k] {
            let o = w.objects.find_mut(id).unwrap();
            let Some(UpdateBody::ModuleProbe(p)) = o.modules[s as usize].body.as_mut() else { unreachable!() };
            p.health = initial_health(k);
        }
    }
    set_scene(&mut w, &cfg, &slots);
    // the objects' own creation (their drawables' weapon-set flags) is not the dock's: the C++ objects are made without it
    let events_made = w.events.events().len();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let idx = w.find_module(1, name).unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let wake_count = |trace: &[String]| {
        let (mut n, mut last) = (0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                n += 1;
                last = f;
            }
        }
        (n, last)
    };
    {
        let trace = w.cuts.trace.clone().unwrap();
        let log = effects(&mut w, events_made, tally);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (cw, cl) = wake_count(&trace);
        put_u32(&mut v, cw);
        put_u32(&mut v, cl);
    }
    let state_of = |w: &World| -> Vec<u32> {
        match w.objects.find(1).unwrap().behaviors[u.index as usize].body.as_ref() {
            Some(UpdateBody::SupplyWarehouseDockUpdate(m)) => m.state_words(),
            Some(UpdateBody::SupplyCenterDockUpdate(m)) => m.state_words(),
            _ => panic!("the dock is in its slot"),
        }
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && ((1..=31).contains(&o[1]) || (43..=56).contains(&o[1])) {
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
        let (mut pos, mut index) = (SENTINEL, o[3] as i32);
        let has_pos = o[2] & 1 != 0;
        let docker = o[1] as ObjectId;
        let before = state_of(&w);
        let call = |w: &mut World, c: ModuleCall| w.module_call(1, idx, c);
        let as_u = |r: ModuleReply| u32::from(r.as_bool());
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
                if state_of(&w)[12] != before[12] {
                    tally.admissions += 1;
                }
            }
            3 => res = as_u(call(&mut w, ModuleCall::IsClearToApproach { docker })),
            4 | 5 => {
                let position = has_pos.then_some(SENTINEL);
                let c = if o[0] == 4 {
                    ModuleCall::DockReserveApproachPosition { docker, position, index }
                } else {
                    ModuleCall::DockAdvanceApproachPosition { docker, position, index }
                };
                if let ModuleReply::Approach { ok, position, index: i } = call(&mut w, c) {
                    res = u32::from(ok);
                    pos = position.unwrap_or(SENTINEL);
                    index = i;
                }
                if o[0] == 4 {
                    tally.reserves += 1;
                    tally.reserved += u64::from(res);
                    if state_of(&w)[16] != before[16] {
                        tally.grown += 1;
                    }
                } else {
                    tally.advances += u64::from(res);
                }
            }
            6 => res = as_u(call(&mut w, ModuleCall::DockIsClearToEnter { docker })),
            7 => res = as_u(call(&mut w, ModuleCall::DockIsClearToAdvance { docker, index: o[3] as i32 })),
            8..=10 => {
                let c = match o[0] {
                    8 => ModuleCall::DockGetEnterPosition { docker, position: has_pos },
                    9 => ModuleCall::DockGetDockPosition { docker, position: has_pos },
                    _ => ModuleCall::DockGetExitPosition { docker, position: has_pos },
                };
                if let ModuleReply::Coord(p) = call(&mut w, c) {
                    pos = p;
                }
            }
            11 => {
                call(&mut w, ModuleCall::DockOnApproachReached { docker });
            }
            12 => {
                call(&mut w, ModuleCall::DockOnEnterReached { docker });
            }
            13 => {
                call(&mut w, ModuleCall::DockOnDockReached { docker });
            }
            14 => {
                call(&mut w, ModuleCall::DockOnExitReached { docker });
            }
            15 => {
                call(&mut w, ModuleCall::DockCancelDock { docker });
            }
            16 => {
                call(&mut w, ModuleCall::DockSetOpen(has_pos));
            }
            17 => res = as_u(call(&mut w, ModuleCall::DockIsOpen)),
            18 => {
                call(&mut w, ModuleCall::DockSetDockCrippled(has_pos));
            }
            19 => res = as_u(call(&mut w, ModuleCall::DockIsAllowPassthroughType)),
            20 => res = as_u(call(&mut w, ModuleCall::DockIsRallyPointAfterDockType)),
            21 => {
                res = as_u(call(&mut w, ModuleCall::DockAction { docker, drone: o[3] as ObjectId }));
                tally.actions_true += u64::from(res);
            }
            22..=24 => {
                let how = o[0] - 22;
                let r = run_xfer(&mut w, 1, idx as usize, how, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if how == 0 {
                    saved = xbytes.clone();
                }
                if how == 1 {
                    // the game logic's post-process, then the modules' own (the warehouse shows its boxes)
                    w.load_post_process();
                    w.modules_load_post_process(1);
                }
                tally.xfers += 1;
            }
            25 => res = u32::from(w.get_dock_update_interface(1) == Some(idx)),
            26 if kind == 0 => {
                call(&mut w, ModuleCall::SupplyWarehouseSetCashValue(o[1] as i32));
            }
            27 if kind == 0 => res = call(&mut w, ModuleCall::SupplyWarehouseGetBoxesStored).as_int(0) as u32,
            28 if kind == 0 => {
                call(&mut w, ModuleCall::OnObjectCreated);
            }
            _ => {}
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let status = if new_errors.iter().any(|(_, e)| e.kind == IniErrorKind::BadArg) {
            3
        } else if new_errors.iter().any(|(_, e)| !e.is_undefined()) {
            2
        } else {
            0
        };
        put_i32(&mut v, status);
        put_u32(&mut v, res);
        for x in [pos.x, pos.y, pos.z] {
            put_u32(&mut v, x.to_bits());
        }
        put_i32(&mut v, index);
        let trace = w.cuts.trace.clone().unwrap();
        let log = effects(&mut w, events_before, tally);
        if log.iter().any(|l| l.starts_with("findpos")) && before[10] == 0 {
            tally.biased += 1;
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (wakes, last) = wake_count(&trace);
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let state = state_of(&w);
        put_u32(&mut v, state.len() as u32);
        for s in state {
            put_u32(&mut v, s);
        }
        for (k, slot) in body_slot.iter().enumerate().skip(1) {
            let h = match *slot {
                Some(s) => {
                    let ob = w.objects.find(k as ObjectId + 1).unwrap();
                    let Some(UpdateBody::ModuleProbe(p)) = ob.modules[s as usize].body.as_ref() else { unreachable!() };
                    p.health
                }
                None => initial_health(k),
            };
            put_u32(&mut v, h.to_bits());
        }
        for k in 1..12 {
            let p = w.objects.find(k as ObjectId + 1).unwrap().get_position();
            if o[0] == 21 && (p.x.to_bits() != cfg[20 + k] || p.y != 3.0) {
                tally.nudges += 1;
            }
            put_u32(&mut v, p.x.to_bits());
            put_u32(&mut v, p.y.to_bits());
        }
        for (k, slot) in slots.ai.iter().enumerate().skip(1) {
            let ob = w.objects.find(k as ObjectId + 1).unwrap();
            let Some(UpdateBody::ModuleProbe(p)) = ob.modules[slot.unwrap() as usize].body.as_ref() else { unreachable!() };
            put_i32(&mut v, p.answers.supply_boxes);
        }
    }
    (v, w)
}

fn pick_u(g: &mut Gen, xs: &[u32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize]
}

fn cases() -> usize {
    std::env::var("L9R44_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn supply_warehouse_and_supply_center_dock_update_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0044_5CDC);
    let mut read = Check::new("R44 data read (throw, message)");
    let mut run = Check::new("R44 ops (data, effects, wakes, xfers, state, health)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let coords = [0.0f32, 10.0, -5.0, 30.0, 2.5, -40.0];
    let angles = [0.0f32, 1.5707964, 0.5, -2.0, 3.0];
    // under a planted bug the planted state can lead the C++ where the Rust no longer flags an undefined path (a box count off by one reaching
    // a docker with no AI): the scenes then avoid the inputs that make those paths undefined
    let safe = std::env::var("L9_PLANT").is_ok();
    let gen_dock = |g: &mut Gen| -> u32 { (if g.below(20) == 0 && !safe { 0 } else { 1 + g.below(3) }) | u32::from(g.below(3) == 0) << 2 | u32::from(g.below(3) == 0) << 3 | pick_u(g, &[1, 2, 3]) << 4 }; // always a local player: `setLocalPlayer(NULL)` is player 0's
    let gen_ai = |g: &mut Gen| -> u32 {
        let bits = u32::from(g.below(100) < 98 || safe) | u32::from(g.below(100) < 85) << 1 | u32::from(g.below(100) < 70) << 2;
        bits | u32::from(g.below(2) == 0) << 3 | u32::from(g.below(2) == 0) << 4 | u32::from(g.below(3) == 0) << 5 | g.below(256) << 8
    };
    let gen_docker = |g: &mut Gen| -> u32 {
        let odds = [20, 70, 40, 50, 50, 20];
        (0..6).fold(0, |acc, b| acc | u32::from(g.below(100) < odds[b]) << b)
    };
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let kind = g.below(2);
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R44{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let number = match &data {
            Some(ModuleDataKind::SupplyWarehouseDockUpdate(d)) => d.dock.number_approach_positions_data,
            Some(ModuleDataKind::SupplyCenterDockUpdate(d)) => d.dock.number_approach_positions_data,
            _ => 0,
        };
        let mut cfg = [0u32; 72];
        cfg[0] = g.below(60);
        cfg[1] = u32::from(g.below(100) < 85) | u32::from(g.below(100) < 30) << 1 | u32::from(g.below(100) < 15) << 2;
        cfg[2] = g.below(4);
        cfg[3] = real(&mut g, &coords);
        cfg[4] = real(&mut g, &coords);
        cfg[5] = real(&mut g, &angles);
        let waiting = if g.below(10) < 7 { (number.max(0) as u32 + g.below(3)).min(15) } else { g.below(13) };
        cfg[6] = (0..3).fold(0, |acc, b| acc | u32::from(g.below(100) < 70) << b) | u32::from(g.below(10) == 0) << 3 | waiting << 8;
        for k in 1..12 {
            cfg[9 + k] = gen_docker(&mut g) | u32::from(g.below(100) < 98 || safe);
            cfg[20 + k] = real(&mut g, &coords);
            let max = pick_u(&mut g, &[200, 400, 100, 0]);
            let health = if max == 0 { 0 } else { g.below(max + 1) };
            cfg[31 + k] = health | max << 16;
        }
        cfg[7] = u32::from(g.below(2) == 0);
        cfg[8] = real(&mut g, &coords);
        cfg[9] = real(&mut g, &coords);
        cfg[43] = gen_dock(&mut g);
        cfg[44] = if g.below(40) == 0 && !safe { 0 } else { pick_u(&mut g, &[100, 100, 75, 1]) };
        for k in 1..12 {
            cfg[44 + k] = gen_ai(&mut g);
            cfg[56 + k] = g.below(6) | pick_u(&mut g, &[0, 0, 0, 20, 0xf6]) << 8;
        }
        cfg[56] = g.below(u32::MAX);
        let n_ops = 1 + g.below(200) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| {
                let d = 2 + g.below(11);
                let size = if number == -1 { 10 } else { number.max(1) as u32 };
                let small = |g: &mut Gen| if number == 0 && g.below(10) != 0 { u32::MAX } else if g.below(40) == 0 { g.below(12) } else { g.below(size.min(4)) };
                let reserve_extra = if number == -1 { 12 } else { 0 };
                if g.below(100) < reserve_extra {
                    return [4, d, 1, g.below(4)];
                }
                match g.below(100) {
                    0..=14 => [0, 0, 0, 0],
                    15 => {
                        frame += 1 + g.below(20);
                        [1, frame, 0, 0]
                    }
                    16..=20 => {
                        let (d1, d2, d3) = (10 + g.below(11), 10 + g.below(11), 21 + g.below(11));
                        let a = pick_u(&mut g, &[1, 1, 2, 3, 5, 6, 6, 7, 8, d1, d2, d3]);
                        let b = match a {
                            1 => g.below(8),
                            2 => g.below(4),
                            3 | 4 | 8 | 9 | 21..=31 => real(&mut g, &coords),
                            5 => real(&mut g, &angles),
                            6 => g.below(16) | g.below(13) << 8,
                            7 => g.below(2),
                            _ => gen_docker(&mut g),
                        };
                        [2, a, b, 0]
                    }
                    21..=23 => [3, d, 0, 0],
                    24..=36 => [4, d, u32::from(g.below(10) != 0), g.below(4)],
                    37..=40 => [5, d, u32::from(g.below(10) != 0), small(&mut g)],
                    41..=44 => [6, d, 0, 0],
                    45..=47 => [7, d, 0, small(&mut g)],
                    48..=51 => [8 + g.below(3), d, u32::from(g.below(8) != 0), 0],
                    52..=60 => [11, d, 0, 0],
                    61..=65 => [12, d, 0, 0],
                    66..=68 => [13, d, 0, 0],
                    69..=72 => [14, d, 0, 0],
                    73..=75 => [15, d, 0, 0],
                    76 => [if g.below(2) == 0 { 18 } else { 16 }, 0, g.below(3).min(1), 0],
                    77 => [17, 0, 0, 0],
                    78 => [18, 0, u32::from(g.below(2) == 0), 0],
                    79 => [19, 0, 0, 0],
                    80 => [20, 0, 0, 0],
                    81..=88 => [21, d, 0, if g.below(4) == 0 { 2 + g.below(11) } else { 0 }],
                    89..=90 => [22, 0, 0, 0],
                    91..=92 => [23, 0, 0, 0],
                    93 => [24, 0, 0, 0],
                    94 => [25, 0, 0, 0],
                    95..=96 => [26, pick_u(&mut g, &[0, 250, 1000, 99, 5]), 0, 0],
                    97 => [27, 0, 0, 0],
                    98 => [28, 0, 0, 0],
                    _ => {
                        let (a, b) = (43 + g.below(14), 0);
                        let b = match a {
                            43 => gen_dock(&mut g),
                            44 => pick_u(&mut g, &[100, 75, 1]),
                            56 => g.below(u32::MAX),
                            _ => gen_ai(&mut g) | b,
                        };
                        [2, a, b, 0]
                    }
                }
            })
            .collect();
        let rust = if outcome.status == 0 {
            let tally_before = tally.clone();
            let (rd, w) = rust_run(&ctx, kind, data.expect("read ok"), &cfg, &ops, &mut tally);
            if let Some((_, e)) = w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
                *reasons.entry(format!("{e}")).or_insert(0u64) += 1;
                excluded += 1;
                tally = tally_before;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r44_run(
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
        let what = || format!("case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R44: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} admissions, \
         {} reserves, {} xfers, {} actions continued), {} boxes taken, {} trucks nudged, {} deposits, {} floating texts, {} stealth grants, {} kills, {} idles, \
         {} destructions, {} supply statuses, {} model condition changes",
        tally.updates, tally.admissions, tally.reserves, tally.xfers, tally.actions_true, tally.boxes_taken, tally.nudges, tally.deposits, tally.floats, tally.grants,
        tally.kills, tally.idles, tally.destroys, tally.statuses, tally.model
    );
    println!("R44 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.boxes_taken * 4 > cases as u64 && tally.nudges * 4 > cases as u64 && tally.deposits * 4 > cases as u64 && tally.floats * 4 > cases as u64
            && tally.grants * 50 > cases as u64 && tally.kills * 50 > cases as u64 && tally.idles * 50 > cases as u64 && tally.destroys * 50 > cases as u64
            && tally.statuses * 4 > cases as u64,
        "the generator must reach every path"
    );
}
