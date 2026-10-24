#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EMPUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `EMPUpdate` and `LeafletDropBehavior` (`logic::modules::emp_update`) against the unchanged C++, `p1w13-sonnet`, batch R33.
//!
//! # Method
//!
//! The C++ side is `EMPUpdate.cpp` (its header's particle system fields parsed by the unit's parser, by `build.rs`), compiled into
//! `wrap_module.cpp`'s unit (`differential/l9_emp_r33.inc`, `ref_l9r33_run`), on the module build's stand-ins: five objects (ids 1-5, each with a
//! body whose `attemptDamage` is a line; kinds, the airborne status bit, maybe a drawable, maybe a team, a geometry, a position); the module's
//! object (1) and its producer (2, maybe with an AI whose victim the test names); three teams with players and the test's relationships; the
//! range scan (the objects the test lists; the call a line); the particle system manager (ids 1, 2, ...; its systems' attach, position, lifetime
//! and delay lines); the drawable's scale, tint and flash (lines; `setTintStatus` the helpers' record, compared apart); `setDisabledUntil` (a
//! line); the REAL `Object::kill`, `GeometryInfo`, `UpdateModule`, `INI`, `RandomValue` and `Xfer`; the game logic, whose wake calls are applied
//! as `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` bodies and an AI
//! probe), three teams with players and `cuts.team_relationships`, `cuts.range_scan`, `cuts.body.stub_object_calls` (the disable is a line);
//! every op an `update()` as the scheduler runs it, a death (`LeafletDropBehavior`'s `onDie`), a change to the scene or the frame, or an xfer.
//!
//! Each case reads a block `D` (either class's fields; mostly well formed) and runs 1 to 80 ops. Compared, bit for bit: the read's throw; the
//! data; the constructor's effects and wake calls; per op the throw status, the result, the effects in order (the scale, tint and flash, the
//! scan, the disables, the kills' damage, the particle systems and their attach, position, lifetime and delay), the tint records, the wake
//! calls, the xfer status, bytes and CRC, the module's state; the constructor's orientation (the stand-in's `setOrientation` is a line).
//!
//! # Results (2026-10-24, `L9R33_CASES=9000`)
//!
//! 9,000 cases (5,830 compared; 1,789 data blocks threw, as the C++ did; 1,381 excluded, undefined upstream: 691 an `EMPUpdate` with no
//! drawable, 690 a scan of radius zero or less, whose iterator upstream reads unset), 229,362 ops (96,702 updates, 3,019 deaths, 44,075 xfers)
//! and 476,214 outputs: **0 mismatches**; 279,901 effect lines compared (the compared cases'): 3,051 flashes (each an attack), 32,534 scans,
//! 11,439 disables, 36,777 damages (the kills), 28,083 particle systems, and 468 tint records.
//!
//! On the way: the module build's `setOrientation` is a line and keeps no angle, so the constructor's random orientation is compared as that
//! line (the Rust side's angle after the constructor), not as a state word.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2031 | the start colour saturated by 3, not 2 | 22,416 |
//! | 2032 | the flash's attack the die frame | 3,031 |
//! | 2033 | a hardened aircraft killed | 76 |
//! | 2034 | `DoesNotAffect = ALLIES` ignored | 78 |
//! | 2035 | the disable lasting no frames | 828 |
//! | 2036 | at least 10 sparks, not 15 | 573 |
//! | 2037 | the intended victim's 40-foot reach dropped | 43 |
//! | 2038 | the leaflets disabling neutrals too | 16,114 |
//! | 2039 | the leaflets' FX fired every frame | 87,848 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::{Coord3D, RGBColor};
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::emp_update::{EmpUpdateModuleData, LeafletDropBehaviorModuleData};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r33.bin");
const XFER_FILE: &str = "l9_r33.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r33Kinds`).
const KINDS: [KindOfType; 8] = [
    KINDOF_VEHICLE,
    KINDOF_STRUCTURE,
    KINDOF_SPAWNS_ARE_THE_WEAPONS,
    KINDOF_AIRCRAFT,
    KINDOF_EMP_HARDENED,
    KINDOF_INFANTRY,
    KINDOF_FS_FACTORY,
    KINDOF_SELECTABLE,
];
const KIND_NAMES: [&str; 8] = ["VEHICLE", "STRUCTURE", "SPAWNS_ARE_THE_WEAPONS", "AIRCRAFT", "EMP_HARDENED", "INFANTRY", "FS_FACTORY", "SELECTABLE"];

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
fn read_emp(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::EmpUpdate(Arc::new(EmpUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_leaflet(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::LeafletDropBehavior(Arc::new(LeafletDropBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let emp = [BlockParse { token: "D", parse: read_emp }];
    let leaflet = [BlockParse { token: "D", parse: read_leaflet }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, if kind == 0 { &emp } else { &leaflet }, &mut out);
    (r, out)
}

fn put_color(v: &mut Vec<u8>, c: &RGBColor) {
    for x in [c.red, c.green, c.blue] {
        put_u32(v, x.to_bits());
    }
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::LeafletDropBehavior(d) => {
            put_u32(v, d.delay_frames);
            put_u32(v, d.disabled_duration);
            put_u32(v, d.radius.to_bits());
            put_str(v, d.leaflet_fx_particle_system.as_deref().unwrap_or(b""));
        }
        ModuleDataKind::EmpUpdate(d) => {
            put_u32(v, d.life_frames);
            put_u32(v, d.start_fade_frame);
            put_u32(v, d.disabled_duration);
            for r in [d.start_scale, d.target_scale_min, d.target_scale_max] {
                put_u32(v, r.to_bits());
            }
            put_color(v, &d.start_color);
            put_color(v, &d.end_color);
            put_str(v, d.disable_fx_particle_system.as_deref().unwrap_or(b""));
            put_u32(v, d.sparks_per_cubic_foot.to_bits());
            put_u32(v, d.effect_radius.to_bits());
            put_u32(v, d.reject_mask);
            for i in 0..KINDOF_COUNT {
                put_u32(v, u32::from(d.victim_kind_of.test(i)) | u32::from(d.victim_kind_of_not.test(i)) << 1);
            }
            put_u32(v, u32::from(d.does_not_affect_my_own_buildings));
        }
        _ => unreachable!(),
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
    if kind == 0 {
        let head: Vec<u32> = (0..12).map(|_| u32_(&mut p)).collect();
        let ps = str_(&mut p);
        let tail: Vec<u32> = (0..3 + KINDOF_COUNT as usize + 1).map(|_| u32_(&mut p)).collect();
        out.push(format!("data {head:x?} {ps:?} {tail:x?}"));
    } else {
        let head: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        let ps = str_(&mut p);
        out.push(format!("data {head:x?} {ps:?}"));
    }
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor {log:?} wakes {w} last {l}"));
    let n_state = if kind == 0 { 5 } else { 2 };
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let nh = u32_(&mut p);
        let tints: Vec<(i32, u32)> = (0..nh).map(|_| (u32_(&mut p) as i32, u32_(&mut p))).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..n_state).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} tints {tints:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let color = |g: &mut Gen| format!("R:{} G:{} B:{}", pick(g, &["0", "128", "255"]), pick(g, &["0", "64", "255"]), pick(g, &["0", "200", "255"]));
    if kind == 0 {
        if g.below(100) < 85 {
            lines.push(format!("Lifetime = {}", pick(g, &["0", "100", "500", "1000", "3000"])));
        }
        if g.below(100) < 85 {
            lines.push(format!("StartFadeTime = {}", pick(g, &["0", "33", "200", "500", "2000"])));
        }
        if g.below(100) < 70 {
            lines.push(format!("DisabledDuration = {}", pick(g, &["0", "500", "3000"])));
        }
        for f in ["StartScale", "TargetScaleMin", "TargetScaleMax"] {
            if g.below(100) < 60 {
                lines.push(format!("{f} = {}", pick(g, &["0.5", "1", "2.5", "-1"])));
            }
        }
        if g.below(100) < 50 {
            let c = color(g);
            lines.push(format!("StartColor = {c}"));
        }
        if g.below(100) < 50 {
            let c = color(g);
            lines.push(format!("EndColor = {c}"));
        }
        if g.below(100) < 80 {
            lines.push(format!("DisableFXParticleSystem = {}", pick(g, &["PS_A", "PS_A", "PS_B", "None"])));
        }
        if g.below(100) < 60 {
            lines.push(format!("SparksPerCubicFoot = {}", pick(g, &["0.001", "0.05", "0.1"])));
        }
        if g.below(100) < 80 {
            lines.push(format!("EffectRadius = {}", pick(g, &["50", "200", "25", "0"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("DoesNotAffect = {}", pick(g, &["ALLIES", "ENEMIES ALLIES", "SELF", "NEUTRALS"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("DoesNotAffectMyOwnBuildings = {}", pick(g, &["Yes", "No"])));
        }
        if g.below(100) < 15 {
            lines.push(format!("VictimRequiredKindOf = {}", pick(g, &KIND_NAMES.map(|s| s))));
        }
        if g.below(100) < 15 {
            lines.push(format!("VictimForbiddenKindOf = {}", pick(g, &KIND_NAMES.map(|s| s))));
        }
        if !clean {
            lines.push(pick(g, &["NoSuchField = 1", "Lifetime = x", "StartColor = R:300 G:0 B:0", "DoesNotAffect = BOGUS", "EffectRadius"]).to_string());
        }
    } else {
        if g.below(100) < 85 {
            lines.push(format!("Delay = {}", pick(g, &["0", "33", "100", "500"])));
        }
        if g.below(100) < 70 {
            lines.push(format!("DisabledDuration = {}", pick(g, &["0", "500", "3000"])));
        }
        if g.below(100) < 80 {
            lines.push(format!("AffectRadius = {}", pick(g, &["60", "25", "200", "0"])));
        }
        if g.below(100) < 60 {
            lines.push(format!("LeafletFXParticleSystem = {}", pick(g, &["PS_A", "PS_B", "None"])));
        }
        if !clean {
            lines.push(pick(g, &["NoSuchField = 1", "Delay = x", "AffectRadius = y", "AffectRadius"]).to_string());
        }
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
    deaths: u64,
    xfers: u64,
    effects: u64,
    flashes: u64,
    scans: u64,
    disables: u64,
    damages: u64,
    systems: u64,
    blackouts: u64,
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

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R33Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 20], ai_slot: u16) {
    for k in 0..5usize {
        let id = k as ObjectId + 1;
        let f = cfg[2 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(template_of(f & 255));
            o.status = Default::default();
            o.status.set(OBJECT_STATUS_AIRBORNE_TARGET, (f >> 8 & 1) as i32);
            o.drawable = if f >> 9 & 1 != 0 { 7 + k as i32 } else { INVALID_DRAWABLE_ID };
            o.team = f >> 10 & 3;
            o.geometry_info = if k == 0 {
                GeometryInfo::new(GeometryType::Sphere, false, 10.0, 10.0, 10.0)
            } else {
                GeometryInfo::new(GeometryType::Box, false, f32::from_bits(cfg[16]), 5.0, 3.0)
            };
        }
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[9 + k]), y: 0.0, z: 0.0 });
    }
    w.objects.find_mut(1).unwrap().producer_id = cfg[7] as ObjectId;
    w.objects.find_mut(2).unwrap().ai = (cfg[8] & 1 != 0).then_some(ai_slot);
    let v = cfg[8] >> 4 & 15;
    {
        let o = w.objects.find_mut(2).unwrap();
        match o.modules[ai_slot as usize].body.as_mut() {
            Some(UpdateBody::ModuleProbe(p)) => p.answers.current_victim = if (1..=5).contains(&v) { v as ObjectId } else { INVALID_ID },
            _ => panic!("the AI probe"),
        }
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[14] >> (2 * (3 * a + b)) & 3));
        }
    }
    let mut scan = Vec::new();
    for k in 0..5 {
        let id = cfg[15] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 5 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them, and the tint records apart.
fn effects(w: &mut World, events_before: usize, names: &mut HashMap<u32, Vec<u8>>, tally: &mut Tally) -> (Vec<String>, Vec<(i32, u32)>) {
    let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let (mut log, mut tints) = (Vec::new(), Vec::new());
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                    tally.systems += 1;
                    names.insert(id, template);
                }
                Some(CutCall::ParticleSystemSetPosition { id, position: p }) => {
                    let n = names.get(&id).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default();
                    log.push(format!("psys {n} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::ParticleSystemSetLifetime { id, frames }) => log.push(format!("pslife {id} {frames}")),
                Some(CutCall::ParticleSystemSetInitialDelay { id, frames }) => log.push(format!("psdelay {id} {frames}")),
                Some(CutCall::Body(BodyCall::ParticleAttachToObject { id, object })) => log.push(format!("psattach {id} {object}")),
                Some(CutCall::Body(BodyCall::SetDisabledUntil { object, disabled_type, frame })) => {
                    tally.disables += 1;
                    log.push(format!("disableduntil {object} {disabled_type} {frame}"))
                }
                Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, .. }) => {
                    tally.scans += 1;
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0", b(p.x), b(p.y), b(p.z), b(range)))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::DrawableSetInstanceScale { object_id, scale }) => log.push(format!("scale {object_id} {:08x}", b(scale))),
                Some(ObjectClientEvent::DrawableColorTint { color: c, .. }) => {
                    log.push(format!("colortint {:08x} {:08x} {:08x}", b(c.red), b(c.green), b(c.blue)))
                }
                Some(ObjectClientEvent::DrawableColorFlash { color: c, decay_frames, attack_frames, sustain_at_peak, .. }) => {
                    tally.flashes += 1;
                    log.push(format!(
                        "colorflash {:08x} {:08x} {:08x} {decay_frames} {attack_frames} {}",
                        b(c.red),
                        b(c.green),
                        b(c.blue),
                        u32::from(sustain_at_peak)
                    ))
                }
                Some(ObjectClientEvent::DrawableTint { status, on, .. }) => {
                    tally.blackouts += 1;
                    tints.push((if on { 7 } else { 8 }, status as u32))
                }
                _ => {}
            }
        } else if t.starts_with("damage ") {
            tally.damages += 1;
            log.push(t.clone());
        }
    }
    (log, tints)
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg_in: &[u32; 20], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let (name, tag): (&[u8], &[u8]) = if kind == 0 { (b"EMPUpdate", b"ModuleTag_EMP") } else { (b"LeafletDropBehavior", b"ModuleTag_Leaflet") };
    let body = || {
        let mut b = NewModule::plain(b"R33Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        // the stand-in body's health and maximum are 0: a kill deals 0
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        b
    };
    // the scene's kinds, team, position and status before the constructor (the driver makes the objects first)
    let tt = template_of(cfg[2] & 255);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, tag, &data).unwrap().unwrap();
    let mut ai = NewModule::plain(b"R33Ai", b"ModuleTag_Ai");
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    ai.is_ai = true;
    assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules: vec![m, body()] }).unwrap(), 1);
    assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules: vec![body(), ai] }).unwrap(), 2);
    for k in 3..=5 {
        assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules: vec![body()] }).unwrap(), k);
    }
    let idx = w.find_module(1, name).unwrap();
    let ai_slot = w.find_module(2, b"R33Ai").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let mut names: HashMap<u32, Vec<u8>> = HashMap::new();
    {
        let trace = w.cuts.trace.clone().unwrap();
        let (mut log, _) = effects(&mut w, 0, &mut names, tally);
        if kind == 0 {
            // the stand-in's `setOrientation` is a line (the constructor's random angle)
            log.push(format!("setorient 1 {:08x}", w.objects.find(1).unwrap().thing.orientation().to_bits()));
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (mut cw, mut cl) = (0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                cw += 1;
                cl = f;
            }
        }
        put_u32(&mut v, cw);
        put_u32(&mut v, cl);
    }
    set_scene(&mut w, &cfg, ai_slot);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 3 && (2..=17).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, ai_slot);
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
                if kind != 0 {
                    w.module_call(1, idx, ModuleCall::OnDie(DamageInfo::default()));
                    tally.deaths += 1;
                }
            }
            4..=6 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 4, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                if o[0] == 5 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            2 | 3 => {}
            _ => unreachable!(),
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
        let trace = w.cuts.trace.clone().unwrap();
        let (log, tints) = effects(&mut w, events_before, &mut names, tally);
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, tints.len() as u32);
        for (k, s) in &tints {
            put_i32(&mut v, *k);
            put_u32(&mut v, *s);
        }
        let (mut wakes, mut last) = (0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            }
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        match ob.behaviors[u.index as usize].body.as_ref() {
            Some(UpdateBody::EmpUpdate(m)) => m.state_words().iter().for_each(|s| put_u32(&mut v, *s)),
            Some(UpdateBody::LeafletDropBehavior(m)) => m.state_words().iter().for_each(|s| put_u32(&mut v, *s)),
            _ => panic!("the module is back in its slot"),
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R33_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn emp_update_and_leaflet_drop_behavior_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0033_E3B0);
    let mut read = Check::new("R33 data read (throw, message)");
    let mut run = Check::new("R33 ops (data, effects, tints, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let gen_object = |g: &mut Gen, k: usize| -> u32 {
        // odds per kind (of `KINDS`): vehicles and aircraft often, EMP hardening rarely
        let odds = [2, 4, 5, 3, 6, 3, 4, 4];
        let kinds = (0..8).fold(0, |acc, b| acc | u32::from(g.below(odds[b]) == 0) << b);
        let drawable = if k == 0 { u32::from(g.below(8) != 0) } else { u32::from(g.below(3) != 0) };
        kinds | u32::from(g.below(2) == 0) << 8 | drawable << 9 | g.below(4) << 10
    };
    let gen_rel = |g: &mut Gen| (0..9).fold(0, |acc, k| acc | g.below(3) << (2 * k));
    let gen_scan = |g: &mut Gen| (0..5).fold(0, |acc, k| acc | (1 + g.below(5)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(5)))) - 1);
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = u32::from(g.below(3) == 0);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R33{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 20];
        cfg[0] = g.below(60);
        cfg[1] = g.u32();
        for k in 0..5 {
            cfg[2 + k] = gen_object(&mut g, k);
        }
        cfg[7] = *pick(&mut g, &[&2u32, &2, &2, &0, &1, &3, &6]);
        cfg[8] = u32::from(g.below(4) != 0) | g.below(7) << 4;
        for c in &mut cfg[9..14] {
            *c = real(&mut g, &[0.0, 10.0, 30.0, 45.0, -20.0]);
        }
        cfg[14] = gen_rel(&mut g);
        cfg[15] = gen_scan(&mut g);
        cfg[16] = real(&mut g, &[0.0, 5.0, 12.5, 40.0]);
        let n_ops = 1 + g.below(80) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(26) {
                0..=10 => [0, 0, 0, 0],
                11..=16 => {
                    frame += if g.below(6) == 0 { 2 + g.below(30) } else { 1 };
                    [2, frame, 0, 0]
                }
                17 => [1, 0, 0, 0],
                18..=20 => {
                    let a = 2 + g.below(15);
                    let b = match a {
                        2..=6 => gen_object(&mut g, a as usize - 2),
                        7 => g.below(7),
                        8 => u32::from(g.below(4) != 0) | g.below(7) << 4,
                        9..=13 => real(&mut g, &[0.0, 10.0, 30.0, 45.0, -20.0]),
                        14 => gen_rel(&mut g),
                        15 => gen_scan(&mut g),
                        _ => real(&mut g, &[0.0, 5.0, 12.5, 40.0]),
                    };
                    [3, a, b, 0]
                }
                21..=22 => [4, 0, 0, 0],
                23..=24 => [5, 0, 0, 0],
                _ => [6, 0, 0, 0],
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
            cpp::ref_l9r33_run(
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
        let what = || format!("case {case}: kind {kind} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
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
        compared += 1;
    }
    println!(
        "R33: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} deaths, {} xfers), \
         {} effect lines ({} flashes, {} scans, {} disables, {} damages, {} particle systems; {} tint records; the compared cases')",
        tally.updates, tally.deaths, tally.xfers, tally.effects, tally.flashes, tally.scans, tally.disables, tally.damages, tally.systems, tally.blackouts
    );
    println!("R33 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.flashes * 10 > cases as u64
            && tally.disables > cases as u64
            && tally.systems > cases as u64
            && tally.blackouts * 20 > cases as u64
            && tally.deaths * 10 > cases as u64,
        "the generator must reach every effect"
    );
}
