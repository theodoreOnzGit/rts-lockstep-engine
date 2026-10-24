#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicGeometryInfoUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GrantStealthBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `DynamicGeometryInfoUpdate` and `GrantStealthBehavior` (`logic::modules::{dynamic_geometry_info_update, grant_stealth_behavior}`) against the
//! unchanged C++, `p1w13-sonnet`, batch R23.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_geom_stealth_r23.inc`,
//! `ref_l9r23_run`; `GrantStealthBehavior.h`'s particle system field names the auto heal driver's parser, `build.rs`) on the module build's
//! stand-ins: the module's object (id 1, a geometry, maybe a drawable, maybe effectively dead) and three more (kinds, maybe a stealth update, maybe a
//! drawable); the auto heal driver's particle system manager and range scan; the game logic, whose `destroyObject` logs and whose wake calls are
//! counted; and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` stealth updates),
//! `cuts.contain.stub_object_calls` set (the grant and the destruction are records), the scan `cuts.range_scan`; every op an `update()` as the
//! scheduler runs it or a change to the scene, a load followed by the game logic's post-process.
//!
//! Each case picks the module, reads a block `D`, mostly well formed, and runs 1 to 30 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's effects and wake calls; per op the throw status, the sleep time, the effects in order (`psys`, `react`, `scan`, `stealthgrant`,
//! `flash`, `destroy`), the wake calls, the xfer status, bytes and CRC, the module's state and object 1's geometry.
//!
//! `m_direction` (`DynamicGeometryInfoUpdate`) is never set upstream and saved: indeterminate bytes. The driver gives it the translation's 0 after
//! the constructor (see the module docs); every other byte is compared as upstream writes it.
//!
//! # Results (2026-10-23, `L9R23_CASES=9000`)
//!
//! 9,000 cases (3,654 `DynamicGeometryInfoUpdate`, 3,570 `GrantStealthBehavior` compared, none excluded; 1,776 data files threw, as the C++
//! did), 111,577 ops (60,898 updates, 28,200 xfers) and 133,249 outputs: **0 mismatches**; 69,165 effect lines compared, among them 6,531
//! geometry changes, 17,411 stealth grants and 8,273 self-destructions.
//!
//! No translation or harness correction was needed, save one for coverage: plant 1933 (the object grants itself) was first an equivalent
//! mutant, the object having neither kinds nor a stealth update; flag bit 7 and bits 21-23 now give it both.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1921 | the initial delay not raised to 1 | 25,668 |
//! | 1922 | the ratio one frame ahead | 40,172 |
//! | 1923 | no reversal at the transition time | 6,419 |
//! | 1924 | `m_finished` not saved | 8,130 |
//! | 1931 | the scan radius never grows | 33,926 |
//! | 1932 | the final scan does not destroy the object | 8,273 |
//! | 1933 | the object grants itself | 4,191 |
//! | 1934 | the `KindOf` test skipped | 4,359 |
//! | 1935 | no flash | 5,898 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::dynamic_geometry_info_update::DynamicGeometryInfoUpdateModuleData;
use rts_lockstep_engine::logic::modules::grant_stealth_behavior::GrantStealthBehaviorModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r23.bin");
const XFER_FILE: &str = "l9_r23.bin";
const CLASSES: [&str; 2] = ["DynamicGeometryInfoUpdate", "GrantStealthBehavior"];
/// The kinds the objects are given, by bit (the driver's `g_r23Kinds`), with their INI names.
const KINDS: [(KindOfType, &str); 3] = [
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_STRUCTURE, "STRUCTURE"),
];

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
fn read_geom(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::DynamicGeometryInfoUpdate(Arc::new(
        DynamicGeometryInfoUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}
fn read_stealth(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::GrantStealthBehavior(Arc::new(
        GrantStealthBehaviorModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: if kind == 0 { read_geom } else { read_stealth },
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r23_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::GrantStealthBehavior(d) => {
            put_u32(v, d.start_radius.to_bits());
            put_u32(v, d.final_radius.to_bits());
            put_u32(v, d.radius_grow_rate.to_bits());
            for i in 0..KINDOF_COUNT {
                put_u32(v, u32::from(d.kind_of.test(i)));
            }
            put_str(v, d.radius_particle_system.as_deref().unwrap_or(b""));
        }
        ModuleDataKind::DynamicGeometryInfoUpdate(d) => {
            put_u32(v, d.initial_delay);
            for f in [
                d.initial_height,
                d.initial_major_radius,
                d.initial_minor_radius,
                d.final_height,
                d.final_major_radius,
                d.final_minor_radius,
            ] {
                put_u32(v, f.to_bits());
            }
            put_u32(v, d.transition_time);
            put_u32(v, u32::from(d.reverse_at_transition_time));
        }
        _ => unreachable!("not an R23 module"),
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
    if kind == 1 {
        let f: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        let kinds: Vec<u32> = (0..KINDOF_COUNT).map(|_| u32_(&mut p)).collect();
        let set: Vec<usize> = kinds
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(i, _)| i)
            .collect();
        let ps = str_(&mut p);
        out.push(format!("data {f:x?} kinds {set:?} psys {ps:?}"));
    } else {
        let f: Vec<u32> = (0..9).map(|_| u32_(&mut p)).collect();
        out.push(format!("data {f:x?}"));
    }
    let n = u32_(&mut p);
    let ctor: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor {ctor:?} wakes {w} last {l}"));
    let n_state = if kind == 1 { 2 } else { 13 };
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
        let state: Vec<u32> = (0..n_state).map(|_| u32_(&mut p)).collect();
        let geom: Vec<u32> = (0..5).map(|_| u32_(&mut p)).collect();
        out.push(format!(
            "op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:x?} geom {geom:x?}"
        ));
    }
    out
}

// -- generators --

fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let real = |g: &mut Gen| pick(g, &["0", "5", "12.5", "30", "100", "-4"]).to_string();
    if kind == 0 {
        if g.below(100) < 70 {
            lines.push(format!(
                "InitialDelay = {}",
                pick(g, &["0", "33", "34", "100", "500"])
            ));
        }
        for f in [
            "InitialHeight",
            "InitialMajorRadius",
            "InitialMinorRadius",
            "FinalHeight",
            "FinalMajorRadius",
            "FinalMinorRadius",
        ] {
            if g.below(100) < 75 {
                let r = real(g);
                lines.push(format!("{f} = {r}"));
            }
        }
        if g.below(100) < 80 {
            lines.push(format!(
                "TransitionTime = {}",
                pick(g, &["0", "33", "100", "200", "1000"])
            ));
        }
        if g.below(100) < 50 {
            lines.push(format!(
                "ReverseAtTransitionTime = {}",
                pick(g, &["Yes", "No"])
            ));
        }
    } else {
        if g.below(100) < 70 {
            lines.push(format!("StartRadius = {}", pick(g, &["0", "10", "50"])));
        }
        if g.below(100) < 70 {
            lines.push(format!(
                "FinalRadius = {}",
                pick(g, &["0", "40", "100", "200"])
            ));
        }
        if g.below(100) < 70 {
            lines.push(format!(
                "RadiusGrowRate = {}",
                pick(g, &["0", "10", "25", "-5", "7.5"])
            ));
        }
        if g.below(100) < 50 {
            let mut t = Vec::new();
            for _ in 0..1 + g.below(2) {
                t.push(KINDS[g.below(3) as usize].1);
            }
            lines.push(format!("KindOf = {}", t.join(" ")));
        }
        if g.below(100) < 50 {
            lines.push(format!(
                "RadiusParticleSystemName = {}",
                pick(g, &["PS_Ring", "PS_Glow", "None"])
            ));
        }
    }
    if !clean {
        lines.push(match g.below(3) {
            0 if kind == 0 => "TransitionTime = x".to_string(),
            1 if kind == 1 => "KindOf = BOGUS".to_string(),
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
    updates: u64,
    xfers: u64,
    grants: u64,
    destroys: u64,
    reacts: u64,
}

fn run_xfer(
    w: &mut World,
    me: ObjectId,
    idx: usize,
    how: u32,
    saved: &[u8],
) -> (i32, Vec<u8>, u32) {
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

/// The kinds of object `id` (bits of [`KINDS`]).
fn set_kinds(w: &mut World, id: ObjectId, bits: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects
        .find_mut(id)
        .unwrap()
        .thing
        .friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The scan: up to four nibbles of ids, 0 ends.
fn scan_list(scan: u32) -> Vec<ObjectId> {
    let mut v = Vec::new();
    for k in 0..4 {
        let id = scan >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 4 {
            v.push(id as ObjectId);
        }
    }
    v
}

/// The effect lines of a stretch of the trace, in the driver's format, and the wake calls of the module at slot `idx`.
fn effects(
    w: &mut World,
    idx: u16,
    events_before: usize,
    tally: &mut Tally,
) -> (Vec<String>, u32, u32) {
    let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let cuts = w.cuts.log.take().unwrap();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), cuts.into_iter());
    let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next().expect("a cut line has a cut call") {
                CutCall::ParticleSystemCreate {
                    template,
                    position: p,
                    id,
                } => log.push(format!(
                    "psys {} {id} {:08x},{:08x},{:08x}",
                    String::from_utf8_lossy(&template),
                    p.x.to_bits(),
                    p.y.to_bits(),
                    p.z.to_bits()
                )),
                CutCall::PartitionIterateAlliesInRange { object, range } => {
                    let p = w.objects.find(object).unwrap().get_position();
                    // the three filters: the relationship, alive, the same map status
                    log.push(format!(
                        "scan {:08x} {:08x} {:08x} {:08x} 3",
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        range.to_bits()
                    ));
                }
                CutCall::Contain(ContainCall::StealthReceiveGrant {
                    object,
                    active,
                    frames,
                }) => {
                    tally.grants += 1;
                    log.push(format!(
                        "stealthgrant {object} {} {frames}",
                        i32::from(active)
                    ));
                }
                CutCall::Contain(ContainCall::DestroyObject { object }) => {
                    tally.destroys += 1;
                    log.push(format!("destroy {object}"));
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::DrawableReactToGeometryChange { .. }) => {
                    tally.reacts += 1;
                    log.push("react".to_string());
                }
                Some(ObjectClientEvent::DrawableFlashAsSelected { .. }) => {
                    log.push("flash".to_string())
                }
                _ => {}
            }
        } else if let Some(f) = t.strip_prefix("awaken ") {
            let (i, f) = f.split_once(' ').unwrap();
            if i.parse::<i32>().unwrap() == i32::from(idx) {
                wakes += 1;
                last = f.parse().unwrap();
            }
        }
    }
    tally.effects += log.len() as u64;
    (log, wakes, last)
}

fn put_log(v: &mut Vec<u8>, log: &[String], wakes: u32, last: u32) {
    put_u32(v, log.len() as u32);
    for l in log {
        put_str(v, l.as_bytes());
    }
    put_u32(v, wakes);
    put_u32(v, last);
}

/// Applies the flags of the scene: drawables, stealth updates, kinds, effective death.
fn set_scene(w: &mut World, flags: u32, drawables: &[i32], stealth: &[Option<u16>]) {
    for k in 0..4usize {
        let id = k as ObjectId + 1;
        w.objects.find_mut(id).unwrap().drawable = if flags >> k & 1 != 0 {
            drawables[k]
        } else {
            rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID
        };
        w.objects.find_mut(id).unwrap().stealth =
            if flags >> (if k > 0 { 3 + k } else { 7 }) & 1 != 0 {
                stealth[k]
            } else {
                None
            };
        set_kinds(
            w,
            id,
            if k > 0 {
                flags >> (12 + 3 * (k - 1)) & 7
            } else {
                flags >> 21 & 7
            },
        );
    }
    w.set_effectively_dead(1, flags >> 8 & 1 != 0);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    kind: u32,
    data: ModuleDataKind,
    cfg: &[u32; 6],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.contain.stub_object_calls = true;
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let class = CLASSES[kind as usize];
    // the other objects first in the scene's terms (ids 2 to 4 follow the module's object)
    let stealth_module = || {
        let mut m = NewModule::plain(b"R23Stealth", b"ModuleTag_Stealth");
        m.is_stealth = true;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers::default(),
            0.0,
            Default::default(),
        )));
        m
    };
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        class.as_bytes(),
        b"ModuleTag_R23",
        &data,
    )
    .unwrap()
    .unwrap();
    // the module's object: its geometry and flags as the driver has them before the constructor
    let events_before = w.events.events().len();
    let me = w
        .new_object(NewObject {
            template: plain.clone(),
            status: ObjectStatusMaskType::default(),
            team: None,
            modules: vec![m, stealth_module()],
        })
        .unwrap();
    assert_eq!(me, 1);
    for k in 2..=4 {
        let id = w
            .new_object(NewObject {
                template: plain.clone(),
                status: ObjectStatusMaskType::default(),
                team: None,
                modules: vec![stealth_module()],
            })
            .unwrap();
        assert_eq!(id, k);
    }
    let idx = w.find_module(me, class.as_bytes()).unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    // the constructor's effects (made at the origin, with no drawable effects: the module's constructor makes none)
    let (ctor, cw, cl) = effects(&mut w, idx, events_before, tally);
    put_log(&mut v, &ctor, cw, cl);
    let drawables: Vec<i32> = (1..=4)
        .map(|k| {
            w.friend_bind_to_drawable(k, 7 + k);
            w.objects.find(k).unwrap().drawable
        })
        .collect();
    let stealth: Vec<Option<u16>> = (1..=4)
        .map(|k| w.objects.find(k).unwrap().stealth)
        .collect();
    let geom_type = match cfg[1] & 3 {
        1 => GeometryType::Cylinder,
        2 => GeometryType::Box,
        _ => GeometryType::Sphere,
    };
    w.objects.find_mut(me).unwrap().geometry_info = GeometryInfo::new(
        geom_type,
        cfg[1] >> 4 & 1 != 0,
        f32::from_bits(cfg[2]),
        f32::from_bits(cfg[3]),
        f32::from_bits(cfg[4]),
    );
    w.set_position(
        me,
        &Coord3D {
            x: 10.0,
            y: -2.5,
            z: 1.0,
        },
    );
    let (mut flags, mut scan) = (cfg[0], cfg[5]);
    let mut saved: Vec<u8> = Vec::new();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    for o in ops {
        match o[0] {
            1 => flags = o[1],
            2 => scan = o[1],
            3 => w.set_frame(o[1]),
            _ => {}
        }
        set_scene(&mut w, flags, &drawables, &stealth);
        w.cuts.range_scan = scan_list(scan);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w
                    .with_body(u, |b, w| b.update(w, u))
                    .expect("the module is in its slot")
                    .0;
                tally.updates += 1;
            }
            1..=3 => {}
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                if o[0] == 8 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let (log, wakes, last) = effects(&mut w, idx, events_before, tally);
        put_log(&mut v, &log, wakes, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let state: Vec<u32> = w
            .objects
            .find(me)
            .unwrap()
            .behaviors
            .iter()
            .find_map(|b| match b.body.as_ref() {
                Some(UpdateBody::DynamicGeometryInfoUpdate(m)) => Some(m.state_words().to_vec()),
                Some(UpdateBody::GrantStealthBehavior(m)) => Some(m.state_words().to_vec()),
                _ => None,
            })
            .expect("the module is back in its slot");
        for s in state {
            put_u32(&mut v, s);
        }
        let gi = w.objects.find(me).unwrap().geometry_info.clone();
        put_u32(&mut v, gi.get_geom_type() as u32);
        put_u32(&mut v, u32::from(gi.get_is_small()));
        put_u32(&mut v, gi.get_max_height_above_position().to_bits());
        put_u32(&mut v, gi.get_major_radius().to_bits());
        put_u32(&mut v, gi.get_minor_radius().to_bits());
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R23_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn dynamic_geometry_and_grant_stealth_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0023_6E05);
    let mut read = Check::new("R23 data read (throw, message)");
    let mut run = Check::new("R23 ops (data, effects, wakes, xfers, state, geometry)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut compared = [0u64; 2];
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = g.below(2);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R23{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let gen_flags = |g: &mut Gen| {
            (g.u32() & 0xF)
                | (g.u32() & 0xF0)
                | u32::from(g.below(10) == 0) << 8
                | g.below(1 << 12) << 12
        };
        let gen_scan = |g: &mut Gen| (0..4).fold(0, |acc, k| acc | g.below(5) << (4 * k));
        let cfg = [
            gen_flags(&mut g),
            g.below(3) | g.below(2) << 4,
            (*pick(&mut g, &[&1.0f32, &10.0, &25.5])).to_bits(),
            (*pick(&mut g, &[&1.0f32, &8.0, &40.0])).to_bits(),
            (*pick(&mut g, &[&1.0f32, &4.0, &12.0])).to_bits(),
            gen_scan(&mut g),
        ];
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = 0u32;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=10 => [0, 0, 0, 0, 0, 0],
                11 => [1, gen_flags(&mut g), 0, 0, 0, 0],
                12..=13 => [2, gen_scan(&mut g), 0, 0, 0, 0],
                14 => {
                    frame += 1 + g.below(100);
                    [3, frame, 0, 0, 0, 0]
                }
                15..=16 => [7, 0, 0, 0, 0, 0],
                17..=18 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
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
            cpp::ref_l9r23_run(
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
        let what = || {
            format!(
                "case {case}: kind {kind} cfg {cfg:x?}, file {:?}, ops {ops:?}",
                show(&text)
            )
        };
        read.eq(
            &outcome,
            &Outcome {
                status,
                detail,
                crc: 0,
            },
            what,
        );
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
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
        "R23: {cases} cases ({} DynamicGeometryInfoUpdate, {} GrantStealthBehavior compared), {read_failed} data blocks threw, {excluded} excluded \
         (undefined upstream), {ops_run} ops, {} updates, {} xfers, {} effect lines, {} geometry changes, {} grants, {} destructions",
        compared[0], compared[1], tally.updates, tally.xfers, tally.effects, tally.reacts, tally.grants, tally.destroys
    );
    read.finish();
    run.finish();
    assert!(
        tally.reacts * 2 > cases as u64
            && tally.grants * 2 > cases as u64
            && tally.destroys * 10 > cases as u64,
        "the generator must reach every path"
    );
}
