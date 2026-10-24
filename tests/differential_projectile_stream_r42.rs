#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProjectileStreamUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ProjectileStreamUpdate` (`logic::modules::projectile_stream_update`) against the unchanged C++, `p1w13-sonnet`, batch R42.
//!
//! # Method
//!
//! The C++ side is the file, unchanged, in `wrap_module.cpp`'s unit (`differential/l9_projectile_stream_r42.inc`, `ref_l9r42_run`), on the
//! module build's stand-ins: eight objects (the stream is object 1; each maybe a `VEHICLE`, one geometry for all, a position; an object the
//! test removes is no object to `findObjectByID`), `destroyObject` (a line), `Thing::setPosition`, the REAL `GeometryInfo`, `UpdateModule`,
//! `INI` and `Xfer`, and the game logic. The member array the upstream constructor leaves uninitialised (it fills a local that shadows it)
//! is set to the port's `INVALID_ID` after construction.
//!
//! The Rust side is the real thing: a `World` with the same eight objects (the module from `modules::new_module`), the real
//! `World::set_position`, `destroy_object` and `process_destroy_list` (an object removed is gone), `GeometryInfo`, and
//! `cuts.body.stub_object_calls` (the stream's own destruction is a record).
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 120 ops: an `update()`, a change to the frame or the scene,
//! `addProjectile` (a source, a projectile, a victim object or none, a victim position or none, from a table with exact repeats and a
//! `-0.0`), `getAllPoints`, `setPosition`, the removal of an object, or an xfer. Compared, bit for bit: the read's throw; the constructor's
//! effects and wake calls; per op the throw status, the result, the effects (the destructions), the wake calls, the xfer status, bytes and
//! CRC, the module's state (the 20 ids, the indices, the owner, the target and its position), every point `getAllPoints` wrote, and the
//! stream's position.
//!
//! # Results (2026-10-24, `L9R42_CASES=9000`)
//!
//! 9,000 cases (7,238 compared; 1,762 data blocks threw, as the C++ did; none excluded), 439,175 ops (109,832 updates, of which 16,597
//! culled the front of the list; 135,690 shots, 97,358 of which broke the stream with a hole; 16,979 removals; 48,613 xfers) and 61,458
//! `getAllPoints` (465,256 points: 278,675 holes or projectiles gone, 14,334 raised to a vehicle owner's roof); 4,237 self-destructions;
//! 892,826 outputs: **0 mismatches**. The first run passed.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2121 | no hole when the victim object changes | 420,191 |
//! | 2122 | a position shot keeps the target object | 201,840 |
//! | 2123 | dying without waiting for the list to empty | 28,933 |
//! | 2124 | the roof skim within 1 major radius, not 1.5 | 1,048 |
//! | 2125 | the version-2 target fields not saved | 69,967 |
//! | 2126 | the cull stopping after one projectile | 60,530 |
//! | 2127 | the roof height always taken, not the larger | 1,737 |
//! | 2128 | the new target position not kept | 208,217 |
//! | 2129 | dying with no owner ever set | 6,599 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::update_module::UpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r42.bin");
const XFER_FILE: &str = "l9_r42.bin";

/// The radii (and heights), by index (the driver's).
const RADII: [f32; 4] = [5.0, 20.0, 1.0, 0.0];
/// The positions a shot or `setPosition` names, by index (the driver's `g_r42Spots`).
const SPOTS: [[f32; 3]; 8] = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.0, 0.5], [-5.0, 7.5, 2.0], [30.0, -2.0, 0.0], [0.0, 0.0, -0.0], [2.5, 2.5, 12.0], [-40.0, 1.0, 3.0]];

fn spot(i: u32) -> Coord3D {
    let s = SPOTS[(i & 7) as usize];
    Coord3D { x: s[0], y: s[1], z: s[2] }
}

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
    *out = Some(ModuleDataKind::Update(Arc::new(UpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor {log:?} wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..27).map(|_| u32_(&mut p)).collect();
        let np = u32_(&mut p);
        let points: Vec<u32> = (0..np * 3).map(|_| u32_(&mut p)).collect();
        let pos: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} points {np} {points:x?} position {pos:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut s = String::from("D\n");
    if !clean {
        s.push_str(pick(g, &["NoSuchField = 1\n", "Enabled = Yes\n"]));
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// What the compared cases reached (a case excluded as undefined upstream counts for nothing).
#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    adds: u64,
    holes: u64,
    culls: u64,
    point_calls: u64,
    points: u64,
    origins: u64,
    skims: u64,
    destroys: u64,
    removals: u64,
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

fn template_of(vehicle: bool) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R42Thing".to_vec(), ..Default::default() };
    if vehicle {
        values.kind_of.push(KINDOF_VEHICLE);
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn geom_of(g: u32) -> GeometryInfo {
    let ty = match g & 3 {
        0 => GeometryType::Sphere,
        1 => GeometryType::Cylinder,
        _ => GeometryType::Box,
    };
    GeometryInfo::new(ty, false, RADII[(g >> 6 & 3) as usize], RADII[(g >> 2 & 3) as usize], RADII[(g >> 4 & 3) as usize])
}

/// The scene of the configuration (the driver's `scene`), on the objects not removed.
fn set_scene(w: &mut World, cfg: &[u32; 32]) {
    for k in 0..8usize {
        let id = k as ObjectId + 1;
        let Some(o) = w.objects.find_mut(id) else { continue };
        o.thing.friend_set_template(template_of(cfg[2] >> k & 1 != 0));
        o.geometry_info = geom_of(cfg[3]);
        let p = Coord3D { x: f32::from_bits(cfg[4 + 3 * k]), y: f32::from_bits(cfg[5 + 3 * k]), z: f32::from_bits(cfg[6 + 3 * k]) };
        w.set_position(id, &p);
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            if let Some(CutCall::Body(BodyCall::DestroyObject { object })) = cut_iter.next() {
                tally.destroys += 1;
                log.push(format!("destroy {object}"));
            }
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 32], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.stub_object_calls = true;
    w.set_frame(cfg[0]);
    let name: &[u8] = b"ProjectileStreamUpdate";
    for k in 0..8usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, b"ModuleTag_Me", &data).unwrap().unwrap());
        }
        let id = w.new_object(NewObject { template: template_of(false), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
    }
    set_scene(&mut w, &cfg);
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
        let log = effects(&mut w, tally);
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
            Some(UpdateBody::ProjectileStreamUpdate(m)) => m.state_words(),
            _ => panic!("the stream is in its slot"),
        }
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=27).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
            set_scene(&mut w, &cfg);
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let mut points: Vec<Coord3D> = Vec::new();
        let before = state_of(&w);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
                if state_of(&w)[21] != before[21] {
                    tally.culls += 1;
                }
            }
            3 => {
                let s = o[3] >> 4;
                let victim_pos = (s != 0).then(|| spot(s - 1));
                w.module_call(1, idx, ModuleCall::ProjectileStreamAddProjectile { source: o[1] as ObjectId, new_id: o[2] as ObjectId, victim: (o[3] & 15) as ObjectId, victim_pos });
                tally.adds += 1;
                let after = state_of(&w);
                if after[20].wrapping_sub(before[20]) % 20 == 2 {
                    tally.holes += 1;
                }
            }
            4 => {
                let ob = w.objects.find(1).unwrap();
                let Some(UpdateBody::ProjectileStreamUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the stream") };
                points = m.get_all_points(&w).expect("indices in range");
                tally.point_calls += 1;
                tally.points += points.len() as u64;
                let (mut i, ids) = (before[21] as usize, &before[..20]);
                for p in &points {
                    match w.objects.find(ids[i] as ObjectId) {
                        None => tally.origins += 1,
                        Some(q) if q.get_position().z != p.z => tally.skims += 1,
                        Some(_) => {}
                    }
                    i = (i + 1) % 20;
                }
            }
            5 => {
                let p = spot(o[1]);
                w.module_call(1, idx, ModuleCall::ProjectileStreamSetPosition(p));
                // the scene keeps the stream where it now is
                let q = w.objects.find(1).unwrap().get_position();
                cfg[4] = q.x.to_bits();
                cfg[5] = q.y.to_bits();
                cfg[6] = q.z.to_bits();
            }
            6 if (2..=8).contains(&o[1]) => {
                if w.objects.find(o[1] as ObjectId).is_some() {
                    tally.removals += 1;
                }
                w.destroy_object(o[1] as ObjectId);
                w.process_destroy_list();
            }
            7..=9 => {
                let how = o[0] - 7;
                let r = run_xfer(&mut w, 1, idx as usize, how, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if how == 0 {
                    saved = xbytes.clone();
                }
                if how == 1 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
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
        let trace = w.cuts.trace.clone().unwrap();
        let log = effects(&mut w, tally);
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
        for s in state_of(&w) {
            put_u32(&mut v, s);
        }
        put_u32(&mut v, points.len() as u32);
        for p in &points {
            for x in [p.x, p.y, p.z] {
                put_u32(&mut v, x.to_bits());
            }
        }
        let p = w.objects.find(1).unwrap().get_position();
        for x in [p.x, p.y, p.z] {
            put_u32(&mut v, x.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R42_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn projectile_stream_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0042_57EA);
    let mut read = Check::new("R42 data read (throw, message)");
    let mut run = Check::new("R42 ops (effects, wakes, xfers, state, points)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let coords = [0.0f32, 10.0, -5.0, 30.0, 2.5, -40.0, 1.0, 0.5, 12.0];
    // a source, a projectile and a victim: mostly the scene's objects, sometimes no object or an id no object has
    let gen_id = |g: &mut Gen, common: u32| match g.below(10) {
        0 => 0,
        1 => 9 + g.below(2),
        2 => 1 + g.below(8),
        _ => common,
    };
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R42{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 32];
        cfg[0] = g.below(60);
        cfg[2] = (0..8).fold(0, |acc, k| acc | u32::from(g.below(2) == 0) << k);
        cfg[3] = g.below(256);
        for c in &mut cfg[4..28] {
            *c = real(&mut g, &coords);
        }
        let owner = 2 + g.below(2);
        let n_ops = 1 + g.below(120) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(100) {
                0..=24 => [0, 0, 0, 0],
                25..=27 => {
                    frame += 1 + g.below(20);
                    [1, frame, 0, 0]
                }
                28..=35 => {
                    let a = 2 + g.below(26);
                    let b = if a <= 3 { g.below(256) } else { real(&mut g, &coords) };
                    [2, a, b, 0]
                }
                36..=66 => {
                    let source = gen_id(&mut g, owner);
                    let common = 3 + g.below(6);
                    let projectile = gen_id(&mut g, common);
                    let common = 3 + g.below(6);
                    let victim = if g.below(5) < 2 { 0 } else { gen_id(&mut g, common) };
                    let pos = if g.below(4) == 0 { 0 } else { 1 + g.below(8) };
                    [3, source, projectile, victim | pos << 4]
                }
                67..=80 => [4, 0, 0, 0],
                81..=83 => [5, g.below(8), 0, 0],
                84..=88 => [6, 2 + g.below(7), 0, 0],
                89..=92 => [7, 0, 0, 0],
                93..=96 => [8, 0, 0, 0],
                _ => [9, 0, 0, 0],
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
            cpp::ref_l9r42_run(
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
        "R42: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} culls, \
         {} shots, {} holes, {} removals, {} xfers), {} getAllPoints ({} points, {} holes or gone, {} skimming a vehicle's roof), {} self-destructions",
        tally.updates, tally.culls, tally.adds, tally.holes, tally.removals, tally.xfers, tally.point_calls, tally.points, tally.origins, tally.skims, tally.destroys
    );
    println!("R42 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.holes * 4 > cases as u64 && tally.skims > cases as u64 && tally.origins > cases as u64 && tally.culls * 4 > cases as u64 && tally.destroys * 10 > cases as u64,
        "the generator must reach every path"
    );
}
