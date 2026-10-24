#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `StealthDetectorUpdate` (`logic::modules::stealth_detector_update`) against the unchanged C++, `p1w13-sonnet`, batch R27.
//!
//! # Method
//!
//! The C++ side is `StealthDetectorUpdate.cpp`, compiled into `wrap_module.cpp`'s unit from `build.rs`'s copy, whose field table names the unit's
//! particle system parser (`differential/l9_stealth_detector_r27.inc`, `ref_l9r27_run`), on the module build's stand-ins: the detector (id 1) with
//! its player, drawable, states, position, vision range and container (object 2); the height-die driver's scan of objects 3 to 6, filtered by the
//! module's own filter; their status bits, kinds, players, drawables, positions, stand-in stealth updates (their `markAsDetected` a line, their Eva
//! events the test's) and contain modules with riders 7 and 8; the REAL `Object::getRelationship` on the teams' relationships; the radar's
//! `tryEvent` (a line, and the test's answer) and `createEvent`, the two misc audio sounds, the UI message, the Eva, the particle systems, the second
//! pass; the local player's shroud of the detector; the game logic, whose wake calls are applied as `GameLogic::friend_awakenUpdateModule` applies
//! them; and the REAL `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule`s for the stealth updates
//! and contain modules), the teams' relationships, the scan `cuts.range_scan`, the shroud `cuts.shrouded_status`; the client's two decisions (the
//! radar's answer, the bone of the ping) are the C++ stand-ins' own, applied to the events by the comparison.
//!
//! Each case reads a block `D` (every field; mostly well formed) and runs 1 to 30 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's wake calls; per op the throw status, the result, the effects in order (the scan, the radar, the sounds, the messages, the Eva, the
//! detections, the second pass, the particle systems), the wake calls, the xfer status, bytes and CRC, `m_enabled` and the next call frame.
//!
//! # Results (2026-10-23, `L9R27_CASES=9000`)
//!
//! 9,000 cases (6,467 compared; 1,788 data files threw, as the C++ did; 745 excluded, undefined upstream: 711 a `NULL` local player's shroud, 34
//! a neutralisation's sound for an object with no player), 99,123 ops (50,124 updates, 27,822 xfers) and 112,057 outputs: **0 mismatches**;
//! 221,011 effect lines compared, among them 13,568 discoveries and 6,228 neutralisations, 48,349 objects marked detected, 14,426 IR pings and
//! 14,403 grid particles.
//!
//! On the way: the first C++ run crashed in the scan's filters: `PartitionFilterAcceptByKindOf` was the one partition filter the module prelude did
//! not rename, so its vtable was the partition build's (the filters had never run in this unit before R25); the prelude renames it now.
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1971 | detected for the rate, not one frame more | 19,638 |
//! | 1972 | allies discovered too | 3,016 |
//! | 1973 | no stealth garrison | 1,080 |
//! | 1974 | the grid's `y` not brought down to a multiple of 12 | 5,934 |
//! | 1975 | the bright and plain pings swapped | 10,620 |
//! | 1976 | `m_enabled` not saved | 12,072 |
//! | 1977 | detects while garrisoned regardless | 2,200 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::{ObjectShroudStatus, Relationship};
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::stealth_detector_update::{StealthDetectorUpdateModuleData, IR_DEFAULT_BONE_POSITION, RADAR_EVENT_STEALTH_DISCOVERED, RADAR_EVENT_STEALTH_NEUTRALIZED};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ObjectClientEvent, StealthClientOp, StealthFeedback};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r27.bin");
const XFER_FILE: &str = "l9_r27.bin";

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
    *out = Some(ModuleDataKind::StealthDetectorUpdate(Arc::new(StealthDetectorUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, d: &StealthDetectorUpdateModuleData) {
    put_u32(v, d.update_rate);
    put_u32(v, d.detection_range.to_bits());
    put_u32(v, u32::from(d.initially_disabled));
    put_str(v, &d.ping_sound.event_name);
    put_str(v, &d.loud_ping_sound.event_name);
    for p in [&d.ir_beacon_particle_sys, &d.ir_particle_sys, &d.ir_bright_particle_sys, &d.ir_grid_particle_sys] {
        put_str(v, p.as_deref().unwrap_or(b""));
    }
    put_str(v, &d.ir_particle_sys_bone);
    for m in [&d.extra_detect_kindof, &d.extra_detect_kindof_not] {
        for i in 0..KINDOF_COUNT {
            put_u32(v, u32::from(m.test(i)));
        }
    }
    put_u32(v, u32::from(d.can_detect_while_garrisoned) | u32::from(d.can_detect_while_transported) << 1);
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
    let head: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
    let names: Vec<String> = (0..7).map(|_| str_(&mut p)).collect();
    let kinds: Vec<Vec<usize>> = (0..2).map(|_| (0..KINDOF_COUNT).map(|_| u32_(&mut p)).enumerate().filter(|(_, v)| *v != 0).map(|(i, _)| i).collect()).collect();
    let flags = u32_(&mut p);
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} {names:?} {kinds:?} {flags} constructor wakes {w} last {l}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let (enabled, next) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} enabled {enabled} next {next}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 75 {
        lines.push(format!("DetectionRate = {}", pick(g, &["0", "33", "100", "500"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("DetectionRange = {}", pick(g, &["0", "50", "150.5"])));
    }
    if g.below(100) < 20 {
        lines.push(format!("InitiallyDisabled = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("PingSound = {}", pick(g, &["PingA", "NoSound"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("LoudPingSound = {}", pick(g, &["PingLoud", "NoSound"])));
    }
    for f in ["IRBeaconParticleSysName", "IRParticleSysName", "IRBrightParticleSysName", "IRGridParticleSysName"] {
        if g.below(100) < 45 {
            lines.push(format!("{f} = {}", pick(g, &["PsA", "PsB", "None"])));
        }
    }
    if g.below(100) < 30 {
        lines.push(format!("IRParticleSysBone = {}", pick(g, &["IRBone", "Muzzle01"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("ExtraRequiredKindOf = {}", pick(g, &["MINE", "INFANTRY"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("ExtraForbiddenKindOf = {}", pick(g, &["DEMOTRAP", "VEHICLE"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("CanDetectWhileGarrisoned = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("CanDetectWhileContained = {}", pick(g, &["Yes", "No"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "ExtraRequiredKindOf = NOSUCHKIND".to_string(),
            1 => "DetectionRate = x".to_string(),
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

#[derive(Default)]
struct Tally {
    effects: u64,
    updates: u64,
    xfers: u64,
    discoveries: u64,
    neutralisations: u64,
    marks: u64,
    pings: u64,
    grids: u64,
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

fn template_of(kinds: &[KindOfType]) -> Arc<ThingTemplate> {
    let values = ObjectTemplateValues { kind_of: kinds.to_vec(), ..Default::default() };
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn rel(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn shroud(v: u32) -> ObjectShroudStatus {
    match v {
        0 => ObjectShroudStatus::Invalid,
        1 => ObjectShroudStatus::Clear,
        2 => ObjectShroudStatus::PartialClear,
        3 => ObjectShroudStatus::Fogged,
        4 => ObjectShroudStatus::Shrouded,
        _ => ObjectShroudStatus::InvalidButPreviousValid,
    }
}

struct Slots {
    stealth: [u16; 9],
    contain: [u16; 9],
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeAnswers {
    match w.objects.find_mut(id).unwrap().modules[slot as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => &mut p.answers,
        _ => panic!("a probe in slot {slot}"),
    }
}

fn set_private(w: &mut World, id: ObjectId, bit: u8, on: bool) {
    let o = w.objects.find_mut(id).unwrap();
    if on {
        o.private_status |= bit;
    } else {
        o.private_status &= !bit;
    }
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 14], s: &Slots) {
    let team = |p: u32| if p < 3 { p + 1 } else { 0 };
    let f = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.drawable = if f & 1 != 0 { if o.drawable == INVALID_DRAWABLE_ID { 7 } else { o.drawable } } else { INVALID_DRAWABLE_ID };
        o.team = team(f >> 1 & 3);
        o.status = ObjectStatusMaskType::default();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 4 & 1) as i32);
        o.status.set(OBJECT_STATUS_SOLD, (f >> 5 & 1) as i32);
        o.status.set(OBJECT_STATUS_STEALTHED, (f >> 6 & 1) as i32);
        o.contained_by = if f >> 7 & 1 != 0 { 2 } else { rts_lockstep_engine::common::game_type::INVALID_ID };
        o.vision_range = cfg[3] as f32;
        o.in_partition = true;
    }
    set_private(w, 1, PRIVATE_EFFECTIVELY_DEAD, f >> 3 & 1 != 0);
    w.set_position(1, &Coord3D { x: 0.0, y: 0.0, z: cfg[4] as f32 });
    w.objects.find_mut(2).unwrap().contain = (f >> 9 & 1 != 0).then_some(s.contain[2]);
    probe_mut(w, 2, s.contain[2]).garrisonable = f >> 8 & 1 != 0;
    let local = cfg[5] & 3;
    w.players.friend_set_local_index((local < 3).then_some(local as i32));
    w.cuts.shrouded_status.clear();
    if local < 3 {
        w.cuts.shrouded_status.insert((1, local as i32), shroud(cfg[5] >> 4 & 7));
    }
    for i in 0..3u32 {
        for j in 0..3u32 {
            w.cuts.team_relationships.insert((i + 1, j + 1), rel(cfg[6] >> (2 * (3 * i + j)) & 3));
        }
    }
    w.cuts.range_scan = (0..4).map(|k| cfg[7] >> (4 * k) & 15).take_while(|&id| id != 0).filter(|id| (3..=6).contains(id)).map(|id| id as ObjectId).collect();
    for k in 3..=6usize {
        let g = cfg[8 + (k - 3)];
        let id = k as ObjectId;
        let mut kinds = Vec::new();
        for (b, kind) in [(3, KINDOF_MINE), (4, KINDOF_BOOBY_TRAP), (5, KINDOF_DEMOTRAP)] {
            if g >> b & 1 != 0 {
                kinds.push(kind);
            }
        }
        w.objects.find_mut(id).unwrap().thing.friend_set_template(template_of(&kinds));
        {
            let o = w.objects.find_mut(id).unwrap();
            o.status = ObjectStatusMaskType::default();
            o.status.set(OBJECT_STATUS_STEALTHED, (g & 1) as i32);
            o.status.set(OBJECT_STATUS_DETECTED, (g >> 1 & 1) as i32);
            o.stealth = (g >> 6 & 1 != 0).then_some(s.stealth[k]);
            o.contain = (g >> 7 & 1 != 0).then_some(s.contain[k]);
            o.team = team(g >> 12 & 3);
            o.drawable = if g >> 14 & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        }
        set_private(w, id, PRIVATE_EFFECTIVELY_DEAD, g >> 2 & 1 != 0);
        {
            let c = probe_mut(w, id, s.contain[k]);
            c.garrisonable = g >> 8 & 1 != 0;
            c.stealth_contain = true;
            c.stealth_units = u32::from(g >> 9 & 1 != 0);
            c.contained = Vec::new();
            if g >> 10 & 1 != 0 {
                c.contained.push(7);
            }
            if g >> 11 & 1 != 0 {
                c.contained.push(8);
            }
        }
        let (ee, oe) = (g >> 15 & 3, g >> 17 & 3);
        probe_mut(w, id, s.stealth[k]).eva_events = Some((if ee != 0 { 29 + ee as i32 } else { -1 }, if oe != 0 { 29 + oe as i32 } else { -1 }));
        w.set_position(id, &Coord3D { x: (g >> 20 & 255) as f32, y: 5.0, z: 0.0 });
    }
    for k in 0..2u32 {
        let r = cfg[12] >> (4 * k);
        let id = 7 + k as ObjectId;
        let o = w.objects.find_mut(id).unwrap();
        o.stealth = (r & 1 != 0).then_some(s.stealth[id as usize]);
        o.team = team(r >> 1 & 3);
    }
}

/// Runs the ops on a fresh world and returns the dump and the world, and whether a feedback that plays read a `NULL` player (undefined upstream).
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 14], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World, bool) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let ModuleDataKind::StealthDetectorUpdate(d) = &data else { unreachable!() };
    put_data(&mut v, d);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let probe = |name: &[u8], answers: ProbeAnswers| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let stealth_probe = || {
        let mut m = probe(b"R27Stealth", ProbeAnswers::default());
        m.is_stealth = true;
        m
    };
    let contain_probe = || {
        let mut m = probe(b"R27Contain", ProbeAnswers::default());
        m.interfaces = MODULEINTERFACE_CONTAIN;
        m
    };
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"StealthDetectorUpdate", b"ModuleTag_Detector", &data).unwrap().unwrap();
    assert_eq!(w.new_object(NewObject { template: template_of(&[]), status: ObjectStatusMaskType::default(), team: None, modules: vec![m] }).unwrap(), 1);
    for id in 2..=8 {
        let modules = if (3..=6).contains(&id) { vec![stealth_probe(), contain_probe()] } else if id == 2 { vec![contain_probe()] } else { vec![stealth_probe()] };
        assert_eq!(w.new_object(NewObject { template: template_of(&[]), status: ObjectStatusMaskType::default(), team: None, modules }).unwrap(), id);
    }
    let mut slots = Slots { stealth: [0; 9], contain: [0; 9] };
    for id in 2..=8 {
        if let Some(s) = w.find_module(id, b"R27Stealth") {
            slots.stealth[id as usize] = s;
        }
        if let Some(c) = w.find_module(id, b"R27Contain") {
            slots.contain[id as usize] = c;
        }
    }
    let idx = w.find_module(1, b"StealthDetectorUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    // the constructor's wake calls
    let (mut cw, mut cl) = (0u32, 0u32);
    for t in w.cuts.trace.take().unwrap() {
        if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
            cw += 1;
            cl = f;
        }
    }
    put_u32(&mut v, cw);
    put_u32(&mut v, cl);
    let mut saved: Vec<u8> = Vec::new();
    let mut undefined_feedback = false;
    for o in ops {
        if o[0] == 2 && (2..14).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 6 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1 => {
                w.module_call(1, idx, ModuleCall::StealthDetectorSetEnabled(o[1] & 1 != 0));
            }
            3..=5 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 3, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            2 | 6 => {}
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let bits = |p: &Coord3D| format!("{:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::PartitionIterateObjectsInRangeOf { object, range, dc }) => log.push(format!("scan {object} {:08x} {dc}", range.to_bits())),
                    Some(CutCall::ParticleSystemCreate { template, position, id }) => {
                        tally.grids += 1;
                        log.push(format!("psys {} {id} {:08x},{:08x},{:08x}", String::from_utf8_lossy(&template), position.x.to_bits(), position.y.to_bits(), position.z.to_bits()))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::StealthDetectorFeedback { feedback, try_radar_event, position, player_index, eva }) => {
                        let (kind, bit, sound, label) = match feedback {
                            StealthFeedback::Discovered => {
                                tally.discoveries += 1;
                                (RADAR_EVENT_STEALTH_DISCOVERED, 0, "StealthDiscoveredSnd", "MESSAGE:StealthDiscovered")
                            }
                            StealthFeedback::Neutralized => {
                                tally.neutralisations += 1;
                                (RADAR_EVENT_STEALTH_NEUTRALIZED, 1, "StealthNeutralizedSnd", "MESSAGE:StealthNeutralized")
                            }
                        };
                        // the client's decision: the radar stand-in's answer (tryEvent), or always (createEvent)
                        let play = if try_radar_event {
                            log.push(format!("radartry {kind} {}", bits(&position)));
                            cfg[13] >> bit & 1 != 0
                        } else {
                            log.push(format!("radarevent {} {kind} {:08x}", bits(&position), 4.0f32.to_bits()));
                            true
                        };
                        if play {
                            match player_index {
                                Some(p) => log.push(format!("audio {sound} - {p} 0")),
                                None => undefined_feedback = true,
                            }
                            log.push(format!("message {label}"));
                            if eva != -1 {
                                log.push(format!("eva {eva}"));
                            }
                        }
                    }
                    Some(ObjectClientEvent::ParticleSystemAtBone { template, id, object_id, attach_to_drawable, .. }) => {
                        tally.pings += 1;
                        log.push(if attach_to_drawable { format!("psattachdraw {id} {object_id}") } else { format!("psattach {id} {object_id}") });
                        // (the drawable stand-in has no such bone: the default position)
                        let pos = IR_DEFAULT_BONE_POSITION;
                        log.push(format!("psys {} {id} {:08x},{:08x},{:08x}", String::from_utf8_lossy(&template), pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()));
                    }
                    Some(ObjectClientEvent::Stealth { object_id, op: StealthClientOp::SecondMaterialPassOpacity(x) }) => log.push(format!("secondpass {object_id} {:08x}", x.to_bits())),
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name))),
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("bt ") {
                if l.starts_with("markdetected ") {
                    tally.marks += 1;
                    log.push(l.to_string());
                }
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let o = w.objects.find(1).unwrap();
        let slot = &o.behaviors[u.index as usize];
        let Some(UpdateBody::StealthDetectorUpdate(m)) = slot.body.as_ref() else { panic!("the module is back in its slot") };
        put_u32(&mut v, u32::from(m.is_sd_enabled()));
        put_u32(&mut v, slot.update.friend_get_next_call_frame());
    }
    (v, w, undefined_feedback)
}

fn cases() -> usize {
    std::env::var("L9R27_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn stealth_detector_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0027_5DE7);
    let mut read = Check::new("R27 data read (throw, message)");
    let mut run = Check::new("R27 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R27{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let gen_det = |g: &mut Gen| {
            u32::from(g.below(5) != 0)
                | (if g.below(8) == 0 { 3 } else { g.below(3) }) << 1
                | u32::from(g.below(15) == 0) << 3
                | u32::from(g.below(12) == 0) << 4
                | u32::from(g.below(15) == 0) << 5
                | u32::from(g.below(5) == 0) << 6
                | u32::from(g.below(5) == 0) << 7
                | g.below(2) << 8
                | u32::from(g.below(4) != 0) << 9
        };
        let gen_obj = |g: &mut Gen| {
            u32::from(g.below(4) != 0)
                | u32::from(g.below(3) == 0) << 1
                | u32::from(g.below(12) == 0) << 2
                | u32::from(g.below(6) == 0) << 3
                | u32::from(g.below(10) == 0) << 4
                | u32::from(g.below(10) == 0) << 5
                | u32::from(g.below(4) != 0) << 6
                | u32::from(g.below(3) == 0) << 7
                | g.below(2) << 8
                | g.below(2) << 9
                | g.below(2) << 10
                | g.below(2) << 11
                | (if g.below(8) == 0 { 3 } else { g.below(3) }) << 12
                | g.below(2) << 14
                | g.below(4) << 15
                | g.below(4) << 17
                | g.below(256) << 20
        };
        let gen_scan = |g: &mut Gen| (0..4).fold(0, |acc, k| acc | (if g.below(6) == 0 { 0 } else { 3 + g.below(4) }) << (4 * k));
        let gen_rel = |g: &mut Gen| (0..9).fold(0, |acc, k| acc | g.below(3) << (2 * k));
        let mut cfg = [
            g.below(200),
            g.u32(),
            gen_det(&mut g),
            *pick(&mut g, &[&0u32, &100, &300]),
            g.below(20),
            (if g.below(8) == 0 { 3 } else { g.below(3) }) | (if g.below(4) == 0 { g.below(6) } else { 1 }) << 4,
            gen_rel(&mut g),
            gen_scan(&mut g),
            gen_obj(&mut g),
            gen_obj(&mut g),
            gen_obj(&mut g),
            gen_obj(&mut g),
            g.below(2) | g.below(4) << 1 | g.below(2) << 4 | g.below(4) << 5,
            g.below(4),
        ];
        // half the cases: the local player is the detector's or the detected objects' (the feedbacks)
        if g.below(2) == 0 {
            cfg[5] = cfg[5] & !3 | (cfg[2] >> 1 & 3);
        }
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=8 => [0, 0, 0, 0, 0, 0],
                9 => [1, g.below(2), 0, 0, 0, 0],
                10..=12 => {
                    let a = 2 + g.below(12);
                    let b = match a {
                        2 => gen_det(&mut g),
                        6 => gen_rel(&mut g),
                        7 => gen_scan(&mut g),
                        8..=11 => gen_obj(&mut g),
                        _ => cfg[a as usize] ^ 1 << g.below(6),
                    };
                    [2, a, b, 0, 0, 0]
                }
                13..=14 => {
                    frame += 1 + g.below(60);
                    [6, frame, 0, 0, 0, 0]
                }
                15..=16 => [3, 0, 0, 0, 0, 0],
                17..=18 => [4, 0, 0, 0, 0, 0],
                _ => [5, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w, undefined_feedback) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if let Some((_, e)) = w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
                *reasons.entry(format!("{e}")).or_insert(0u64) += 1;
                excluded += 1;
                continue;
            }
            if undefined_feedback {
                *reasons.entry("a feedback that plays reads a NULL player".to_string()).or_insert(0u64) += 1;
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
            cpp::ref_l9r27_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R27: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} xfers, \
         {} effect lines, {} discoveries, {} neutralisations, {} objects marked, {} pings, {} grid particles",
        tally.updates, tally.xfers, tally.effects, tally.discoveries, tally.neutralisations, tally.marks, tally.pings, tally.grids
    );
    println!("R27 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.discoveries * 4 > cases as u64 && tally.neutralisations * 4 > cases as u64 && tally.marks > cases as u64 && tally.pings > cases as u64 && tally.grids * 4 > cases as u64,
        "the generator must reach every path"
    );
}
