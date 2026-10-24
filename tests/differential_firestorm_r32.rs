#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FirestormDynamicGeometryInfoUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FirestormDynamicGeometryInfoUpdate` (`logic::modules::firestorm_dynamic_geometry_info_update`) against the unchanged C++, `p1w13-sonnet`,
//! batch R32.
//!
//! # Method
//!
//! The C++ side is `FirestormDynamicGeometryInfoUpdate.cpp` (its particle system parser routed to the unit's, by `build.rs`; its base
//! `DynamicGeometryInfoUpdate.cpp` is batch R23's), compiled into `wrap_module.cpp`'s unit (`differential/l9_firestorm_r32.inc`,
//! `ref_l9r32_run`), on the module build's stand-ins: four objects (ids 1-4, each with a body whose `attemptDamage` is a line, and a height); the
//! firestorm (id 1: a geometry, maybe a drawable, maybe a player); the terrain's ground height; the particle system manager (its systems found
//! unless their id is dead; a sphere's or a cylinder's radius a line); the range scan (the objects the test lists); the FX lists; the game
//! client's scorch; the REAL `UpdateModule`, `INI` and `Xfer`; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` bodies), a player for
//! the firestorm's team, `cuts.terrain_ground`, `cuts.body.dead_particle_modulus`, `cuts.range_scan`; every op an `update()` as the scheduler
//! runs it, a change to the scene, or an xfer.
//!
//! Each case reads a block `D` (the base's growth and this class's fields; mostly well formed) and runs 1 to 80 ops. Compared, bit for bit: the
//! read's throw; the data; the constructor's wake calls; per op the throw status, the result, the effects in order (the geometry's changes, the
//! particle systems made and their radii, the FX list, the scorch, the scan and the damage done), the wake calls, the xfer status, bytes and
//! CRC, the base's and this class's state, object 1's geometry and the player's firestorm count.
//!
//! # Results (2026-10-24, `L9R32_CASES=9000`)
//!
//! 9,000 cases (6,481 compared; 1,788 data blocks threw, as the C++ did; 731 excluded, undefined upstream: a firestorm with no controlling
//! player), 261,818 ops (152,449 updates, 54,614 xfers) and 536,598 outputs: **0 mismatches**; 372,477 effect lines compared (the compared
//! cases'): 35,713 geometry changes, 13,884 particle systems made and 140,156 radii given them, 2,150 scorches, 61,613 scans and 109,439
//! damages done.
//!
//! On the way: the first runs differed in the scan radius of a box whose growth had made it NaN (a `TransitionTime` of 0): `GeometryInfo`'s
//! square roots went through `petir::Real::sqrt`, which answers a canonical NaN, where the reference build's `sqrt` returns the NaN it is given
//! (negative here). `common::geometry` now passes a NaN through as `sqrtsd` does.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2021 | the FX list not played | 2,356 |
//! | 2022 | the particle offset ignored | 2,058 |
//! | 2023 | a dead particle system kept | 116,219 |
//! | 2024 | a scorch every frame going back | 32,519 |
//! | 2025 | the damage interval exclusive | 42,435 |
//! | 2026 | no height limit on the damage | 16,869 |
//! | 2027 | the damage's death type normal | 43,016 |
//! | 2028 | the scorch flag not saved | 24,828 |
//! | 2029 | the systems take the minor radius | 16,220 |
//!
//! A first 2021 (the effects a frame early) made the C++ run crash: the Rust side fired while the scene had a player, and the C++ a frame
//! later without one; it was replaced by the one above, which changes no path into the undefined case.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::firestorm_dynamic_geometry_info_update::{FirestormDynamicGeometryInfoUpdateModuleData, MAX_FIRESTORM_SYSTEMS};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r32.bin");
const XFER_FILE: &str = "l9_r32.bin";

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
    *out = Some(ModuleDataKind::FirestormDynamicGeometryInfoUpdate(Arc::new(FirestormDynamicGeometryInfoUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// `fxId`: the FX lists numbered in the order the dump first names them (0 for `NULL`).
#[derive(Default)]
struct FxIds(Vec<Vec<u8>>);
impl FxIds {
    fn id(&mut self, fx: &Option<Vec<u8>>) -> i32 {
        let Some(n) = fx else { return 0 };
        match self.0.iter().position(|x| x == n) {
            Some(i) => i as i32 + 1,
            None => {
                self.0.push(n.clone());
                self.0.len() as i32
            }
        }
    }
}

fn put_data(v: &mut Vec<u8>, d: &FirestormDynamicGeometryInfoUpdateModuleData, fx: &mut FxIds) {
    let g = &d.geom;
    put_u32(v, g.initial_delay);
    for r in [g.initial_height, g.initial_major_radius, g.initial_minor_radius, g.final_height, g.final_major_radius, g.final_minor_radius] {
        put_u32(v, r.to_bits());
    }
    put_u32(v, g.transition_time);
    put_u32(v, u32::from(g.reverse_at_transition_time));
    for p in &d.particle_systems {
        put_str(v, p.as_deref().unwrap_or(b""));
    }
    put_i32(v, fx.id(&d.fx_list));
    for r in [d.particle_offset_z, d.scorch_size, d.delay_between_damage_frames, d.damage_amount, d.max_height_for_damage] {
        put_u32(v, r.to_bits());
    }
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
    let head: Vec<u32> = (0..9).map(|_| u32_(&mut p)).collect();
    let systems: Vec<String> = (0..MAX_FIRESTORM_SYSTEMS).map(|_| str_(&mut p)).collect();
    let tail: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} {systems:?} {tail:x?} constructor wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..13 + 19 + 5 + 1).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let real = |g: &mut Gen| pick(g, &["0", "5", "12.5", "30", "-4"]).to_string();
    if g.below(100) < 70 {
        lines.push(format!("InitialDelay = {}", pick(g, &["0", "33", "34", "100", "300"])));
    }
    for f in ["InitialHeight", "InitialMajorRadius", "InitialMinorRadius", "FinalHeight", "FinalMajorRadius", "FinalMinorRadius"] {
        if g.below(100) < 75 {
            let r = real(g);
            lines.push(format!("{f} = {r}"));
        }
    }
    if g.below(100) < 80 {
        lines.push(format!("TransitionTime = {}", pick(g, &["0", "33", "100", "200", "500"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("ReverseAtTransitionTime = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 70 {
        lines.push(format!("DelayBetweenDamageFrames = {}", pick(g, &["0", "100", "250", "500"])));
    }
    if g.below(100) < 70 {
        lines.push(format!("DamageAmount = {}", pick(g, &["0", "10", "35.5"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("MaxHeightForDamage = {}", pick(g, &["0", "5", "20", "-1"])));
    }
    for k in 1..=MAX_FIRESTORM_SYSTEMS {
        if g.below(100) < 18 {
            lines.push(format!("ParticleSystem{k} = {}", pick(g, &["PS_A", "PS_B", "None"])));
        }
    }
    if g.below(100) < 50 {
        lines.push(format!("FXList = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("ParticleOffsetZ = {}", pick(g, &["0", "2", "-1.5"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("ScorchSize = {}", pick(g, &["0", "20", "45.5"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "DamageAmount = x", "ParticleSystem17 = PS_A", "TransitionTime = x", "ScorchSize"]).to_string());
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
    xfers: u64,
    effects: u64,
    systems: u64,
    radii: u64,
    scorches: u64,
    scans: u64,
    damages: u64,
    reacts: u64,
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

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 16]) {
    let f = cfg[1];
    let o = w.objects.find_mut(1).unwrap();
    o.drawable = if f & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
    o.team = if f >> 1 & 1 != 0 { 1 } else { 0 };
    w.cuts.range_scan = scan_list(cfg[6]);
    for k in 0..4u32 {
        let p = Coord3D { x: 10.0 * k as f32, y: -2.5, z: f32::from_bits(cfg[7 + k as usize]) };
        w.set_position(k as ObjectId + 1, &p);
    }
    w.cuts.terrain_ground = [f32::from_bits(cfg[11]); 8];
    w.cuts.body.dead_particle_modulus = (cfg[12] & 255, cfg[12] >> 8 & 255);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 16], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::FirestormDynamicGeometryInfoUpdate(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.players.set_player_count(1);
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    w.cuts.team_players.insert(1, 0);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FirestormDynamicGeometryInfoUpdate", b"ModuleTag_Firestorm", &data).unwrap().unwrap();
    let body = || {
        let mut b = NewModule::plain(b"R32Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 1000.0, Default::default())));
        b
    };
    let tt = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules: vec![m, body()] }).unwrap(), 1);
    for k in 2..=4 {
        assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules: vec![body()] }).unwrap(), k);
    }
    let idx = w.find_module(1, b"FirestormDynamicGeometryInfoUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let (mut cw, mut cl) = (0u32, 0u32);
    for t in w.cuts.trace.take().unwrap() {
        if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
            cw += 1;
            cl = f;
        }
    }
    put_u32(&mut v, cw);
    put_u32(&mut v, cl);
    let geom_type = match cfg[2] & 3 {
        1 => GeometryType::Cylinder,
        2 => GeometryType::Box,
        _ => GeometryType::Sphere,
    };
    w.objects.find_mut(1).unwrap().geometry_info = GeometryInfo::new(geom_type, cfg[2] >> 4 & 1 != 0, f32::from_bits(cfg[3]), f32::from_bits(cfg[4]), f32::from_bits(cfg[5]));
    set_scene(&mut w, &cfg);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 && (1..=12).contains(&o[1]) && !(2..=5).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg);
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
            1 | 2 => {}
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
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let b = |x: f32| x.to_bits();
        let radius_logged = cfg[1] >> 2 & 3 < 2;
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::ParticleSystemCreate { template, position: p, id }) => {
                        tally.systems += 1;
                        log.push(format!("psys {} {id} {:08x},{:08x},{:08x}", String::from_utf8_lossy(&template), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, .. }) => {
                        tally.scans += 1;
                        log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0", b(p.x), b(p.y), b(p.z), b(range)))
                    }
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableReactToGeometryChange { .. }) => {
                        tally.reacts += 1;
                        log.push("react".to_string())
                    }
                    Some(ObjectClientEvent::ParticleSystemSetEmissionRadius { id, radius }) if radius_logged => {
                        tally.radii += 1;
                        log.push(format!("psradius {id} {:08x}", b(radius)))
                    }
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => log.push(format!("fx {}", fx.id(&Some(name)))),
                    Some(ObjectClientEvent::GameClientAddScorch { position: p, radius, scorch_type }) => {
                        tally.scorches += 1;
                        log.push(format!("scorch {:08x} {:08x} {:08x} {:08x} {scorch_type}", b(p.x), b(p.y), b(p.z), b(radius)))
                    }
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if t.starts_with("damage ") {
                tally.damages += 1;
                log.push(t.clone());
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
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        let slot = &ob.behaviors[u.index as usize];
        let Some(UpdateBody::FirestormDynamicGeometryInfoUpdate(m)) = slot.body.as_ref() else { panic!("the module is back in its slot") };
        for s in m.geometry_update().state_words() {
            put_u32(&mut v, s);
        }
        for s in m.state_words() {
            put_u32(&mut v, s);
        }
        let gi = &ob.geometry_info;
        put_u32(&mut v, gi.get_geom_type() as u32);
        put_u32(&mut v, u32::from(gi.get_is_small()));
        put_u32(&mut v, gi.get_max_height_above_position().to_bits());
        put_u32(&mut v, gi.get_major_radius().to_bits());
        put_u32(&mut v, gi.get_minor_radius().to_bits());
        put_u32(&mut v, w.players.get_nth(0).map_or(0, |p| p.academy_stats.firestorms_created()));
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R32_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn firestorm_dynamic_geometry_info_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0032_F1E5);
    let mut read = Check::new("R32 data read (throw, message)");
    let mut run = Check::new("R32 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let gen_scan = |g: &mut Gen| (0..4).fold(0, |acc, k| acc | g.below(5) << (4 * k));
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R32{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 16];
        cfg[0] = g.below(200);
        cfg[1] = u32::from(g.below(6) != 0) | u32::from(g.below(10) != 0) << 1 | g.below(3) << 2;
        cfg[2] = g.below(3) | g.below(2) << 4;
        cfg[3] = real(&mut g, &[0.0, 5.0, 12.5]);
        cfg[4] = real(&mut g, &[0.0, 8.0, 30.0]);
        cfg[5] = real(&mut g, &[0.0, 8.0, 30.0]);
        cfg[6] = gen_scan(&mut g);
        for c in &mut cfg[7..11] {
            *c = real(&mut g, &[0.0, 2.0, 25.0, -3.0]);
        }
        cfg[11] = real(&mut g, &[0.0, 1.5, -2.0]);
        cfg[12] = if g.below(3) == 0 { (2 + g.below(3)) | g.below(2) << 8 } else { 0 };
        let n_ops = 1 + g.below(80) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(24) {
                0..=13 => [0, 0, 0, 0],
                14..=16 => {
                    frame += 1 + g.below(20);
                    [2, frame, 0, 0]
                }
                17..=18 => {
                    let a = *pick(&mut g, &[&1u32, &6, &7, &8, &11, &12]);
                    let b = match a {
                        1 => u32::from(g.below(6) != 0) | u32::from(g.below(10) != 0) << 1 | g.below(3) << 2,
                        6 => gen_scan(&mut g),
                        11 => real(&mut g, &[0.0, 1.5, -2.0]),
                        12 => (2 + g.below(3)) | g.below(2) << 8,
                        _ => real(&mut g, &[0.0, 2.0, 25.0, -3.0]),
                    };
                    [1, a, b, 0]
                }
                19..=20 => [3, 0, 0, 0],
                21..=22 => [4, 0, 0, 0],
                _ => [5, 0, 0, 0],
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
            cpp::ref_l9r32_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R32: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} xfers), \
         {} effect lines ({} geometry changes, {} particle systems, {} radii, {} scorches, {} scans, {} damages; the compared cases')",
        tally.updates, tally.xfers, tally.effects, tally.reacts, tally.systems, tally.radii, tally.scorches, tally.scans, tally.damages
    );
    println!("R32 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.systems * 2 > cases as u64 && tally.radii > cases as u64 && tally.scorches * 10 > cases as u64 && tally.damages > cases as u64, "the generator must reach every effect");
}
