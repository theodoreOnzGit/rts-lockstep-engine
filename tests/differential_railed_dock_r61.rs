#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RailedTransportDockUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `RailedTransportDockUpdate` (`logic::modules::railed_transport_dock_update`) against the unchanged C++, `p1w13-sonnet`, batch R61.
//!
//! # Method
//!
//! The C++ side is `RailedTransportDockUpdate.cpp` in `wrap_module.cpp`'s unit (`differential/l9_railed_dock_r61.inc`, `ref_l9r61_run`; its
//! header without the interface class the contain driver copied, `build.rs`), on batch R43's stand-ins (the dock, object 1, and eleven
//! dockers, objects 2-12; `findPositionAround`; the model condition changes) and: the dock's contain module (maybe: an `OpenContain`, as
//! upstream casts to one, whose `isValidContainerFor`, `addToContain`, `removeFromContain` and `iterateContained` are the test's, over a list
//! of the dockers; lines), the dockers' AIs (`aiIdle` and `aiMoveToPosition` lines), `deselectObject`, `setStatus`, `setDisabled`,
//! `clearDisabled` and `setOrientation` (lines), `Object`'s single-flag model condition changes (lines), the terrain's ground height (a
//! line and the test's table), the partition manager's real distance procedures, and the art's bones (case-blind, as the art's are).
//!
//! The Rust side is the real thing: a `World` with the same twelve objects (the module from `modules::new_module`; the contain module a probe
//! that answers `validfor` and keeps the list, `ModuleProbe::trace_contain`; the dockers' AIs probes), batch R43's bones and answers, the
//! terrain's table in `cuts.terrain_ground`; the object calls are records (`cuts.body.stub_object_calls`, `cuts.contain.stub_object_calls`);
//! the real `World::set_position`, distances and bone transform.
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 200 ops: an `update()`, a change to the frame or the scene, batch R43's dock
//! calls (dockers are never dozers or harvesters here: the base's one-flag clear of a worker's `MOVING` would read as this module's own),
//! the action, `isLoadingOrUnloading`, `unloadAll`, `unloadSingleObject`, or an xfer. Compared, bit for bit: the read's throw and the data;
//! the constructor's effects and wake calls; per op the throw status, the result, the position and index the call wrote, the effects in
//! order, the wake calls, the xfer status, bytes and CRC, the module's state (the dock's, then this class's five fields), every docker's
//! position and the contain module's list.
//!
//! # Results (2026-10-25, `L9R61_CASES=9000`)
//!
//! 9,000 cases (5,479 compared; 1,770 data blocks threw, as the C++ did; 1,751 excluded, undefined upstream: 828 riders walked off a dock
//! whose art has no `DockWaiting07` bone, 527 unloads with no contain module, 396 approach indices outside the list), 519,637 ops (129,864
//! updates, 61,847 reserves, 29,265 xfers); 46,880 dockers taken in, 17,250 pulled all the way in and contained, 14,741 unloaded, 7,468
//! walked off to the bone, 22,722 ground heights read; 1,050,232 outputs: **0 mismatches**.
//!
//! On the way (test harness only): a zero pull or push duration makes an infinite step, and the not-a-number positions differed in their sign
//! bit; upstream's `Object::reactToTransformChange` destroys such an object, which the module build's `setPosition` does not model, so the
//! generator always gives both durations, never 0. The C++ stand-in's airborne flag held without an AI, where upstream asks the AI.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2311 | the tolerance doubled | 197,584 |
//! | 2312 | a full container lets the docker in | 169 |
//! | 2313 | the pull stops within 3, not 6 | 102,265 |
//! | 2314 | an unloaded rider stays held | 11,232 |
//! | 2315 | a single unload not counted down | 172,290 |
//! | 2316 | `unloadAll` restarts an unload under way | 197,243 |
//! | 2317 | the unload count not saved | 33,100 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODELCONDITION_DOCKING_BEGINNING};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::railed_transport_dock_update::RailedTransportDockUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_COUNT;
use rts_lockstep_engine::common::model_state::MODELCONDITION_MOVING;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r61.bin");
const XFER_FILE: &str = "l9_r61.bin";

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
fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::RailedTransportDockUpdate(Arc::new(RailedTransportDockUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
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
    let head: Vec<u32> = (0..5).map(|_| u32_(&mut p)).collect();
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
        let positions: Vec<u32> = (0..33).map(|_| u32_(&mut p)).collect();
        let nc = u32_(&mut p);
        let inside: Vec<u32> = if nc == u32::MAX { Vec::new() } else { (0..nc).map(|_| u32_(&mut p)).collect() };
        out.push(format!("op {i}: status {st} result {res:#x} written {written:x?} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} positions {positions:x?} contain {nc:#x} {inside:?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 90 {
        lines.push(format!("NumberApproachPositions = {}", pick(g, &["-1", "-1", "-1", "0", "1", "2", "3", "3", "5", "10"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("AllowsPassthrough = {}", pick(g, &["Yes", "No"])));
    }
    // (always, and never 0: a zero duration makes an infinite step, whose not-a-number position Object::reactToTransformChange answers by
    // destroying the object, which the module build's setPosition does not model)
    lines.push(format!("PullInsideDuration = {}", pick(g, &["1000", "500", "2000", "100"])));
    lines.push(format!("PushOutsideDuration = {}", pick(g, &["1000", "500", "3000", "100"])));
    if g.below(100) < 50 {
        lines.push(format!("ToleranceDistance = {}", pick(g, &["50", "10", "100", "25.5"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "NumberApproachPositions = x", "AllowsPassthrough = maybe", "ToleranceDistance"]).to_string());
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
    model: u64,
    actions_true: u64,
    xfers: u64,
    taken: u64,
    contained: u64,
    unloaded: u64,
    walks: u64,
    grounds: u64,
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
    let values = ObjectTemplateValues { name: b"R43Thing".to_vec(), kind_of: kinds.to_vec(), ..Default::default() };
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The art's bones (the driver's `r43Bone`).
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

/// The scene of the configuration (the driver's `scene`; the dockers' positions are set once, and by op 2).
fn set_scene(w: &mut World, cfg: &[u32; 48], contain: Option<u16>, ai_slots: &[Option<u16>; 12]) {
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
        if let Some(c) = contain {
            let Some(UpdateBody::ModuleProbe(p)) = o.modules[c as usize].body.as_mut() else { unreachable!() };
            p.answers.yes = f & 16 != 0;
        }
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
            o.ai = if g & 64 != 0 { ai_slots[k] } else { None };
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
    }
    w.cuts.contain.find_position = (cfg[7] & 1 != 0).then(|| Coord3D { x: f32::from_bits(cfg[8]), y: f32::from_bits(cfg[9]), z: 0.0 });
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
                Some(CutCall::Contain(ContainCall::SetOrientation { object, angle })) => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                Some(CutCall::GameLogicDeselectObject { object, player_mask, affect_client }) => {
                    tally.taken += 1;
                    log.push(format!("deselect {object} {player_mask} {}", i32::from(affect_client)))
                }
                Some(CutCall::Body(BodyCall::SetStatus { object, mask, set })) => {
                    let bit = (0..OBJECT_STATUS_COUNT).find(|&i| mask.test(i)).unwrap_or(-1);
                    log.push(format!("setstatus {object} {bit} {}", i32::from(set)))
                }
                Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => log.push(format!("setdisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                Some(CutCall::TerrainGetGroundHeight { x, y }) => {
                    tally.grounds += 1;
                    log.push(format!("ground {:08x} {:08x}", b(x), b(y)))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            if let Some(Some(ObjectClientEvent::DrawableModelCondition { object_id, op })) = ev_iter.next() {
                tally.model += 1;
                log.push(match op {
                    ModelConditionOp::ClearAndSet { clear, set } => format!("mc {object_id} clear {clear:?} set {set:?}"),
                    // (Object's one-flag changes: the drawable's own lines, which name no object)
                    ModelConditionOp::Clear(clear) if clear == [MODELCONDITION_MOVING] => format!("mcclear {MODELCONDITION_MOVING}"),
                    ModelConditionOp::Set(set) if set.len() == 1 => format!("mc {}", set[0]),
                    ModelConditionOp::Clear(clear) => format!("mc {object_id} clear {clear:?}"),
                    ModelConditionOp::Set(set) => format!("mc {object_id} set {set:?}"),
                });
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("addtocontain ") {
                tally.contained += 1;
            } else if l.starts_with("removefromcontain ") {
                tally.unloaded += 1;
            } else if l.starts_with("aimove ") {
                tally.walks += 1;
            }
            if ["validfor ", "addtocontain ", "removefromcontain ", "aiidle ", "aimove "].iter().any(|p| l.starts_with(p)) {
                log.push(l.to_string());
            }
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 48], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    if let ModuleDataKind::RailedTransportDockUpdate(d) = &data {
        put_i32(&mut v, d.dock.number_approach_positions_data);
        put_u32(&mut v, u32::from(d.dock.is_allow_passthrough));
        put_u32(&mut v, d.pull_inside_duration_in_frames);
        put_u32(&mut v, d.push_outside_duration_in_frames);
        put_u32(&mut v, d.tolerance_distance.to_bits());
    }
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.contain.stub_object_calls = true;
    for (k, h) in w.cuts.terrain_ground.iter_mut().enumerate() {
        *h = (cfg_in[44] >> (4 * k) & 15) as f32 * 0.5;
    }
    w.set_frame(cfg[0]);
    let name: &[u8] = b"RailedTransportDockUpdate";
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, max: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        m
    };
    let mut body_slot = [None; 12];
    let mut ai_slots = [None; 12];
    let initial_health = |k: usize| (cfg_in[31 + k] & 0xffff) as f32 * 0.5;
    for k in 0..12usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, b"ModuleTag_Me", &data).unwrap().unwrap());
            if cfg_in[1] & 8 != 0 {
                let contained = (0..11).filter(|j| cfg_in[43] >> j & 1 != 0).map(|j| j as ObjectId + 2).collect();
                modules.push(probe(b"R61Contain", ProbeAnswers { trace_contain: true, contained, ..Default::default() }, MODULEINTERFACE_CONTAIN, 0.0));
            }
        } else {
            if cfg_in[9 + k] & 1 != 0 {
                modules.push(probe(b"R43Body", ProbeAnswers::default(), MODULEINTERFACE_BODY, (cfg_in[31 + k] >> 16) as f32 * 0.5));
            }
            let mut ai = probe(b"R43Ai", ProbeAnswers::default(), 0, 0.0);
            ai.is_ai = true;
            modules.push(ai);
        }
        let id = w.new_object(NewObject { template: template_of(&[]), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        body_slot[k] = w.find_module(id, b"R43Body");
        ai_slots[k] = w.find_module(id, b"R43Ai");
        if let Some(s) = body_slot[k] {
            let o = w.objects.find_mut(id).unwrap();
            let Some(UpdateBody::ModuleProbe(p)) = o.modules[s as usize].body.as_mut() else { unreachable!() };
            p.health = initial_health(k);
        }
    }
    let contain = w.find_module(1, b"R61Contain");
    set_scene(&mut w, &cfg, contain, &ai_slots);
    for k in 1..12usize {
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[20 + k]), y: 3.0, z: 0.0 });
    }
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
            Some(UpdateBody::RailedTransportDockUpdate(m)) => m.state_words(),
            _ => panic!("the dock is in its slot"),
        }
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=31).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 2 && (21..=31).contains(&o[1]) {
            w.set_position(o[1] as ObjectId - 19, &Coord3D { x: f32::from_bits(o[2]), y: 3.0, z: 0.0 });
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, contain, &ai_slots);
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
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            25 => res = u32::from(w.get_dock_update_interface(1) == Some(idx)),
            26 => res = as_u(call(&mut w, ModuleCall::RailedTransportDockIsLoadingOrUnloading)),
            27 => {
                call(&mut w, ModuleCall::RailedTransportDockUnloadAll);
            }
            28 => {
                call(&mut w, ModuleCall::RailedTransportDockUnloadSingleObject(docker));
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
        for k in 1..12 {
            let p = w.objects.find(k as ObjectId + 1).unwrap().get_position();
            for x in [p.x, p.y, p.z] {
                put_u32(&mut v, x.to_bits());
            }
        }
        match contain {
            Some(c) => {
                let Some(UpdateBody::ModuleProbe(p)) = w.objects.find(1).unwrap().modules[c as usize].body.as_ref() else { unreachable!() };
                put_u32(&mut v, p.answers.contained.len() as u32);
                for &id in &p.answers.contained {
                    put_u32(&mut v, id as u32);
                }
            }
            None => put_u32(&mut v, u32::MAX),
        }
    }
    (v, w)
}

fn pick_u(g: &mut Gen, xs: &[u32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize]
}

fn cases() -> usize {
    std::env::var("L9R61_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn railed_transport_dock_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0061_DA11);
    let mut read = Check::new("R61 data read (throw, message)");
    let mut run = Check::new("R61 ops (data, effects, wakes, xfers, state, positions, contain)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let coords = [0.0f32, 10.0, -5.0, 30.0, 2.5, -40.0];
    let angles = [0.0f32, 1.5707964, 0.5, -2.0, 3.0];
    // (never a dozer or a harvester: see the method)
    let gen_docker = |g: &mut Gen| -> u32 {
        let odds = [20, 70, 40, 0, 0, 20, 90];
        (0..7).fold(0, |acc, b| acc | u32::from(g.below(100) < odds[b]) << b)
    };
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R61{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let number = match &data {
            Some(ModuleDataKind::RailedTransportDockUpdate(d)) => d.dock.number_approach_positions_data,
            _ => 0,
        };
        let mut cfg = [0u32; 48];
        cfg[0] = g.below(60);
        cfg[1] = u32::from(g.below(100) < 85)
            | u32::from(g.below(100) < 30) << 1
            | u32::from(g.below(100) < 15) << 2
            | u32::from(g.below(100) < 92) << 3
            | u32::from(g.below(100) < 80) << 4;
        cfg[2] = g.below(4);
        cfg[3] = real(&mut g, &coords);
        cfg[4] = real(&mut g, &coords);
        cfg[5] = real(&mut g, &angles);
        // (mostly the seven the unloaded rider's walk needs: `DOCKWAITING07`)
        let waiting = if g.below(10) < 8 { (number.max(7) as u32 + g.below(3)).min(15) } else { g.below(13) };
        cfg[6] = (0..3).fold(0, |acc, b| acc | u32::from(g.below(100) < 70) << b) | u32::from(g.below(10) == 0) << 3 | waiting << 8;
        for k in 1..12 {
            cfg[9 + k] = gen_docker(&mut g) | u32::from(g.below(100) < 98);
            cfg[20 + k] = real(&mut g, &coords);
            let max = pick_u(&mut g, &[200, 400, 100, 0]);
            let health = if max == 0 { 0 } else { g.below(max + 1) };
            cfg[31 + k] = health | max << 16;
        }
        cfg[43] = (0..11).fold(0, |acc, b| acc | u32::from(g.below(100) < 30) << b);
        cfg[44] = g.below(u32::MAX);
        cfg[7] = u32::from(g.below(2) == 0);
        cfg[8] = real(&mut g, &coords);
        cfg[9] = real(&mut g, &coords);
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
                // the railed dock's own: updates (the pull and the push), actions, unloads
                if g.below(100) < 35 {
                    return match g.below(30) {
                        0..=13 => [0, 0, 0, 0],
                        14..=20 => [21, d, 0, 0],
                        21..=22 => [26, 0, 0, 0],
                        23..=26 => [27, 0, 0, 0],
                        _ => [28, d, 0, 0],
                    };
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
                    76 => [16, 0, g.below(3).min(1), 0],
                    77 => [17, 0, 0, 0],
                    78 => [18, 0, u32::from(g.below(3) == 0), 0],
                    79 => [19, 0, 0, 0],
                    80 => [20, 0, 0, 0],
                    81..=88 => [21, if g.below(15) == 0 { 0 } else { d }, 0, if g.below(4) == 0 { 2 + g.below(11) } else { 0 }],
                    89..=91 => [22, 0, 0, 0],
                    92..=94 => [23, 0, 0, 0],
                    95..=97 => [24, 0, 0, 0],
                    _ => [25, 0, 0, 0],
                }
            })
            .collect();
        let rust = if outcome.status == 0 {
            let tally_before = tally.clone();
            let (rd, w) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r61_run(
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
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
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
        "R61: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} admissions, \
         {} reserves ({} granted), {} xfers), {} findPositionAround, {} model condition changes, {} actions continued, {} dockers taken, {} pulled in, \
         {} unloaded, {} walked off, {} ground heights",
        tally.updates, tally.admissions, tally.reserves, tally.reserved, tally.xfers, tally.findpos, tally.model, tally.actions_true, tally.taken, tally.contained,
        tally.unloaded, tally.walks, tally.grounds
    );
    println!("R61 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.taken > 0 && tally.contained > 0 && tally.unloaded > 0 && tally.walks > 0 && tally.grounds > 0, "the generator must reach every path");
}
