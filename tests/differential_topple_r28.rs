#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ToppleUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ToppleUpdate` (`logic::modules::topple_update`) against the unchanged C++, `p1w13-sonnet`, batch R28.
//!
//! # Method
//!
//! The C++ side is `ToppleUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_topple_r28.inc`, `ref_l9r28_run`), on
//! the module build's stand-ins: the toppling object (id 1) with a body whose `attemptDamage` is a line, a drawable whose calls are lines (and no
//! client modules), a name, and the transform `Thing` keeps (`setTransformMatrix` takes the position and the z angle from it, `setOrientation`
//! poses it); a crusher (id 2: a crusher level, maybe a physics behaviour with a velocity); the stumps the thing factory makes of the one template
//! it knows (ids 3, 4, ...); the script engine's topple directions (`ScriptEngine::adjustToppleDirection`, verbatim); the pathfinder and the FX (lines);
//! the REAL `Object::kill`, `Matrix3D`, `UpdateModule`, `INI` and `Xfer`; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` body, the crusher's
//! template and physics probe), the real `Thing` transform, a thing factory that knows `Stump`, `cuts.script_topple_directions`; every op an
//! `update()` as the scheduler runs it, a module call or a change to the scene.
//!
//! Each case reads a block `D` (every field; mostly well formed) and runs 1 to 40 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's wake calls; per op the throw status, the result, the effects in order (the sway, the model condition, the FX, the pathfinder, the
//! damage, the shadows), the wake calls, the xfer status, bytes and CRC, the state, the object's transform, position and orientation, its health
//! and how many stumps were made.
//!
//! # Results (2026-10-24, `L9R28_CASES=9000`)
//!
//! 9,000 cases (6,384 compared; 1,771 data blocks threw, as the C++ did; 845 excluded, undefined upstream: a topple of an object with no drawable,
//! whose `getClientUpdateModules` is unchecked), 250,763 ops (~~217,907~~ **187,954** updates, ~~30,666~~ **26,489** xfers) and 514,294 outputs:
//! **0 mismatches**; ~~103,048~~ **101,399** effect lines compared; ~~137,927~~ **113,490** ops ended falling, ~~14,488~~ **11,777** bouncing back
//! and ~~18,772~~ **15,200** down, ~~30,232~~ **29,766** with a stump made.
//! **CORRECTED 2026-10-24** (the same day): the first record's counts of updates, xfers and effects also counted the cases later excluded as
//! undefined upstream, whose outputs are not compared; the counts below are the compared cases' only (the output and mismatch counts were right).
//!
//! On the way (a driver fix): `applyTopplingForce` has no "already toppling" guard, so every topple makes a stump and a tree toppled twice makes
//! two; the first stand-in factory gave every stump id 3. It gives 3, 4, ... now, as the game logic does, and the dump counts them.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1981 | `KillWhenStartToppling` ignored | 18,875 |
//! | 1982 | the topple direction not normalised | 90,504 |
//! | 1983 | the acceleration from the velocity percent | 127,903 |
//! | 1984 | a fence not taken out of the pathfinder | 1,046 |
//! | 1985 | no floor of 1 on the z-turn steps | 42,617 |
//! | 1986 | `TOPPLE_OPTIONS_NO_BOUNCE` ignored | 19,810 |
//! | 1987 | `KillStumpWhenToppled` ignored | **survived** |
//! | 1988 | the last step not clamped to the limit | 42,985 |
//! | 1989 | a crusher of level 1 topples | 27,224 |
//!
//! 1987 survives because the instrument cannot see it: the stumps here have no body (on either side), so a stump's death changes nothing the dump
//! holds. Catching it needs a stump with a body probe, which the Rust thing factory does not make yet.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::topple_update::ToppleUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r28.bin");
const XFER_FILE: &str = "l9_r28.bin";

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
    *out = Some(ModuleDataKind::ToppleUpdate(Arc::new(ToppleUpdateModuleData::friend_new_module_data(Some(ini))?)));
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

fn put_data(v: &mut Vec<u8>, d: &ToppleUpdateModuleData, fx: &mut FxIds) {
    put_i32(v, fx.id(&d.topple_fx));
    put_i32(v, fx.id(&d.bounce_fx));
    put_str(v, &d.stump_name);
    for r in [d.initial_velocity_percent, d.initial_accel_percent, d.bounce_velocity_percent] {
        put_u32(v, r.to_bits());
    }
    put_u32(
        v,
        u32::from(d.kill_when_toppled)
            | u32::from(d.kill_when_start_toppled) << 1
            | u32::from(d.kill_stump_when_toppled) << 2
            | u32::from(d.topple_left_or_right_only) << 3
            | u32::from(d.reorient_toppled_rubble) << 4,
    );
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
    let (f1, f2) = (u32_(&mut p), u32_(&mut p));
    let stump = str_(&mut p);
    let rest: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data fx {f1} {f2} stump {stump:?} {rest:x?} constructor wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..12).map(|_| u32_(&mut p)).collect();
        let xform: Vec<u32> = (0..12).map(|_| u32_(&mut p)).collect();
        let tail: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} xform {xform:x?} tail {tail:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 40 {
        lines.push(format!("ToppleFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("BounceFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("StumpName = {}", pick(g, &["Stump", "Stump", "NoSuchStump"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("KillWhenStartToppling = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("KillWhenFinishedToppling = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("KillStumpWhenToppled = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("ToppleLeftOrRightOnly = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("ReorientToppledRubble = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("InitialVelocityPercent = {}", pick(g, &["20%", "50%", "5%"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("InitialAccelPercent = {}", pick(g, &["1%", "10%", "0%"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("BounceVelocityPercent = {}", pick(g, &["30%", "60%", "5%"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "BounceFX".to_string(),
            1 => "InitialVelocityPercent = x".to_string(),
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

/// What the compared cases reached (a case excluded as undefined upstream counts for nothing).
#[derive(Default, Clone)]
struct Tally {
    effects: u64,
    updates: u64,
    xfers: u64,
    falls: u64,
    bounces: u64,
    downs: u64,
    stumps: u64,
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

fn template_with_crusher(level: u8) -> Arc<ThingTemplate> {
    let values = ObjectTemplateValues { crusher_level: level, ..Default::default() };
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The scene of the configuration (the driver's `scene`); `pose` also puts the object where the configuration says.
fn set_scene(w: &mut World, cfg: &[u32; 12], pose: bool, physics: u16) {
    let f = cfg[1];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.drawable = if f & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        if f >> 1 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.name = if f >> 2 & 1 != 0 { b"Tree1".to_vec() } else { Vec::new() };
    }
    if pose {
        w.set_position(1, &Coord3D { x: f32::from_bits(cfg[2]), y: f32::from_bits(cfg[3]), z: 0.0 });
        w.set_orientation(1, f32::from_bits(cfg[4]));
        let b = usize::from(w.objects.find(1).unwrap().body.unwrap());
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(1).unwrap().modules[b].body.as_mut() {
            p.health = f32::from_bits(cfg[5]);
            p.max_health = p.health;
        }
    }
    w.objects.find_mut(2).unwrap().thing.friend_set_template(template_with_crusher((cfg[6] & 15) as u8));
    w.objects.find_mut(2).unwrap().physics = (cfg[6] >> 4 & 1 != 0).then_some(physics);
    if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(2).unwrap().modules[physics as usize].body.as_mut() {
        p.answers.velocity = Coord3D { x: f32::from_bits(cfg[9]), y: f32::from_bits(cfg[10]), z: 0.0 };
    }
    w.set_position(2, &Coord3D { x: f32::from_bits(cfg[7]), y: f32::from_bits(cfg[8]), z: 0.0 });
    w.cuts.script_topple_directions.clear();
    if cfg[11] != 0 {
        let a = (cfg[11] % 8) as f32 * (2.0 * rts_lockstep_engine::common::base_type::PI / 8.0);
        w.cuts.script_topple_directions.push((b"Tree1".to_vec(), Coord3D { x: 3.0 * a.cos(), y: 3.0 * a.sin(), z: 0.0 }));
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::ToppleUpdate(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut factory = ThingFactory::new();
    factory.new_template(b"Stump", 0).expect("a template");
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), thing_factory: factory, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ToppleUpdate", b"ModuleTag_Topple", &data).unwrap().unwrap();
    let mut body = NewModule::plain(b"R28Body", b"ModuleTag_Body");
    body.interfaces = MODULEINTERFACE_BODY;
    body.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), f32::from_bits(cfg[5]), Default::default())));
    assert_eq!(w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default())), status: ObjectStatusMaskType::default(), team: None, modules: vec![m, body] }).unwrap(), 1);
    let mut phys = NewModule::plain(b"PhysicsBehavior", b"ModuleTag_Physics");
    phys.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    assert_eq!(w.new_object(NewObject { template: template_with_crusher(0), status: ObjectStatusMaskType::default(), team: None, modules: vec![phys] }).unwrap(), 2);
    let physics = w.find_module(2, b"PhysicsBehavior").unwrap();
    let idx = w.find_module(1, b"ToppleUpdate").unwrap();
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
    set_scene(&mut w, &cfg, true, physics);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let mut pose = false;
        if o[0] == 6 && (1..12).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
            pose = (2..=5).contains(&o[1]);
        }
        if o[0] == 5 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, pose, physics);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let dir = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: 0.0 };
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1 => res = u32::from(w.module_call(1, idx, ModuleCall::IsAbleToBeToppled).as_bool()),
            2 => {
                w.module_call(1, idx, ModuleCall::ApplyTopplingForce { direction: Some(dir), speed: f32::from_bits(o[3]), options: o[4] });
            }
            3 => {
                let other = if o[1] & 1 != 0 { 2 } else { rts_lockstep_engine::common::game_type::INVALID_ID };
                w.module_call(1, idx, ModuleCall::OnCollide { other, loc: None, normal: None });
            }
            4 => w.topple(1, Some(dir), f32::from_bits(o[3]), o[4]),
            7..=9 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 7, &saved);
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
            5 | 6 => {}
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
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                if let Some(CutCall::PathfinderRemoveObjectFromPathfindMap { object }) = cut_iter.next() {
                    log.push(format!("pathremove {object}"));
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableStopSway { object_id }) => log.push(format!("clientmodules {object_id}")),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(a), .. }) => log.push(format!("mc {}", a[0])),
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => log.push(format!("fx {}", fx.id(&Some(name)))),
                    Some(ObjectClientEvent::DrawableSetShadowsEnabled { enabled, .. }) => log.push(format!("shadows {}", i32::from(enabled))),
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("damage ") {
                log.push(format!("damage {l}"));
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
        let ob = w.objects.find(1).unwrap();
        let slot = &ob.behaviors[u.index as usize];
        let Some(UpdateBody::ToppleUpdate(m)) = slot.body.as_ref() else { panic!("the module is back in its slot") };
        let state = m.state_words();
        match state[5] {
            1 => tally.falls += 1,
            2 => tally.downs += 1,
            _ => {}
        }
        if f32::from_bits(state[0]) < 0.0 {
            tally.bounces += 1;
        }
        if state[11] != 0 {
            tally.stumps += 1;
        }
        for s in state {
            put_u32(&mut v, s);
        }
        let mx = *ob.thing.transform_matrix();
        for r in 0..3 {
            for c in [mx[r].x, mx[r].y, mx[r].z, mx[r].w] {
                put_u32(&mut v, c.to_bits());
            }
        }
        let pos = ob.get_position();
        for c in [pos.x, pos.y, pos.z, ob.thing.orientation()] {
            put_u32(&mut v, c.to_bits());
        }
        let health = match ob.body.and_then(|b| ob.modules[b as usize].body.as_ref()) {
            Some(UpdateBody::ModuleProbe(p)) => p.health,
            _ => 0.0,
        };
        put_u32(&mut v, health.to_bits());
        // every topple makes a stump: ids 3, 4, ...
        put_u32(&mut v, (w.next_object_id() - 3) as u32);
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R28_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn topple_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0028_70BB);
    let mut read = Check::new("R28 data read (throw, message)");
    let mut run = Check::new("R28 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R28{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let cfg = [
            g.below(200),
            u32::from(g.below(6) != 0) | u32::from(g.below(15) == 0) << 1 | g.below(2) << 2 | g.below(2) << 3,
            real(&mut g, &[0.0, 10.0, -35.5]),
            real(&mut g, &[0.0, 7.0, 120.25]),
            real(&mut g, &[0.0, 0.5, -1.2, 2.9, 1.5707964]),
            real(&mut g, &[100.0, 5000.0]),
            g.below(4) | u32::from(g.below(3) != 0) << 4,
            real(&mut g, &[0.0, 20.0, -15.0]),
            real(&mut g, &[0.0, 3.0, 40.0]),
            real(&mut g, &[0.0, 1.0, 0.25, -2.0]),
            real(&mut g, &[0.0, 0.5, 3.0]),
            if g.below(4) == 0 { 1 + g.below(8) } else { 0 },
        ];
        let n_ops = 1 + g.below(80) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(24) {
                _ if g.below(2) == 0 => [0, 0, 0, 0, 0, 0],
                0..=11 => [0, 0, 0, 0, 0, 0],
                12 => [1, 0, 0, 0, 0, 0],
                13..=14 => [
                    2 + 2 * g.below(2),
                    real(&mut g, &[1.0, 0.0, -0.7, 3.0]),
                    real(&mut g, &[0.0, 1.0, 0.7, -2.5]),
                    real(&mut g, &[0.1, 0.05, 0.3, 0.0, 1.0]),
                    g.below(4),
                    0,
                ],
                15..=16 => [3, u32::from(g.below(5) != 0), 0, 0, 0, 0],
                17 => {
                    frame += 1 + g.below(30);
                    [5, frame, 0, 0, 0, 0]
                }
                18 => {
                    let a = *pick(&mut g, &[&1u32, &2, &4, &6, &7, &9, &11]);
                    let b = match a {
                        1 => u32::from(g.below(6) != 0) | u32::from(g.below(15) == 0) << 1 | g.below(2) << 2,
                        2 => real(&mut g, &[0.0, 10.0, -35.5]),
                        4 => real(&mut g, &[0.0, 0.5, -1.2, 2.9]),
                        6 => g.below(4) | g.below(2) << 4,
                        7 => real(&mut g, &[0.0, 20.0, -15.0]),
                        9 => real(&mut g, &[0.0, 1.0, 0.25]),
                        _ => g.below(9),
                    };
                    [6, a, b, 0, 0, 0]
                }
                19..=20 => [7, 0, 0, 0, 0, 0],
                21..=22 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
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
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r28_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R28: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} xfers, \
         {} effect lines, {} ops falling, {} bouncing, {} down, {} with a stump",
        tally.updates, tally.xfers, tally.effects, tally.falls, tally.bounces, tally.downs, tally.stumps
    );
    println!("R28 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.falls > cases as u64 && tally.bounces * 4 > cases as u64 && tally.downs > cases as u64 && tally.stumps * 4 > cases as u64, "the generator must reach every path");
}
