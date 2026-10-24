#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/TensileFormationUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `TensileFormationUpdate` (`logic::modules::tensile_formation_update`) against the unchanged C++, `p1w13-sonnet`, batch R34.
//!
//! # Method
//!
//! The C++ side is `TensileFormationUpdate.cpp` (`getTFU`'s name key made non-static by `build.rs`, as the test resets the name keys every
//! case), compiled into `wrap_module.cpp`'s unit (`differential/l9_tensile_r34.inc`, `ref_l9r34_run`), on the module build's stand-ins: five
//! objects (ids 1-5; each maybe a member of the formation, which `findUpdateModule` answers; maybe a drawable; maybe a body whose damage state
//! the test sets and whose `setDamageState` is a line; maybe a shrub; a position; object 1, the module's, a sphere of the test's radius); the
//! scan (the objects the test lists, filtered as the partition manager filters; the call a line); the closest object; the terrain's eight
//! cells (heights and normals); the pathfinder's wall; the audio; the drawable's model condition flags; the REAL `Object::topple`,
//! `UpdateModule`, `INI`, `RandomValue` and `Xfer`; the game logic, whose wake calls are applied as `GameLogic::friend_awakenUpdateModule`
//! applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`; a `ProbeModule` named
//! `TensileFormationUpdate` on each other member, which is what the formation filter looks for; `ProbeModule` bodies with
//! `trace_damage_state`), `cuts.range_scan`, `cuts.closest_object_at`, `cuts.terrain_ground` and `cuts.terrain_normal`, `cuts.audio_playing`;
//! every op an `update()` as the scheduler runs it, a change to the scene or the frame, or an xfer.
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 120 ops (a third of them 330 to 479, for the slide to end). Compared, bit for bit: the read's throw; the data; the
//! constructor's effects and wake calls; per op the throw status, the result, the effects in order (the walls, the scans, the damage states,
//! the sound, the ground reads, the closest-object query, the model condition flags), the wake calls, the xfer status, bytes and CRC, the
//! module's state (the links and their tensors, the inertia, the life, the lowest elevation, the sound's handle), object 1's position and
//! orientation.
//!
//! # Results (2026-10-24, `L9R34_CASES=9000`)
//!
//! 9,000 cases (6,846 compared; 1,783 data blocks threw, as the C++ did; 371 excluded, undefined upstream: 276 an object 1 with no body read
//! by the idle check or the rubble, 95 no object closest to the new position), 1,187,366 ops (791,719 updates, 158,084 xfers) and 2,388,424
//! outputs: **0 mismatches**; 3,269,909 effect lines compared (the compared cases'): 2,707 enablings, 1,821 crack sounds, 19,167 scans,
//! 39,428 damage states set (1,499 rubble); 742,475 updates with links and 381,339 moves.
//!
//! On the way: the first runs differed in one ground read per link: upstream's `MIN(m_lowestSlideElevation, getGroundHeight(..))` is the
//! macro `a < b ? a : b`, which reads the ground a second time when it is the answer; the translation now does.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2041 | a friction of 0.96, not 0.95 | 990,561 |
//! | 2042 | a nearer object overwrites the first link (no push) | 546,179 |
//! | 2043 | an object as near as the nearest linked too | 499,507 |
//! | 2044 | the idle sleep 31 frames, not 30 | 326,533 |
//! | 2045 | the settled formation DAMAGED, not RUBBLE | 1,499 |
//! | 2046 | the dislodgement on the 28th frame of 30 | 24,784 |
//! | 2047 | the wall not removed when enabled | 2,707 |
//! | 2048 | the crack sound replayed while playing | 126,876 |
//! | 2049 | the tensors pull with 0.08, not 0.07 (x) | 933,057 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_SHRUBBERY;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::tensile_formation_update::TensileFormationUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r34.bin");
const XFER_FILE: &str = "l9_r34.bin";

/// The ground heights and normals a cell can have, by nibble (the driver's `g_r34Grounds`, `g_r34Normals`).
const GROUNDS: [f32; 8] = [0.0, 2.0, 5.5, -3.0, 12.0, 1.25, 30.0, -0.5];
const NORMALS: [[f32; 3]; 8] =
    [[0.0, 0.0, 1.0], [0.3, 0.0, 0.954], [-0.2, 0.4, 0.894], [0.0, -0.6, 0.8], [0.5, 0.5, 0.707], [-0.7, -0.1, 0.707], [0.05, 0.02, 0.998], [0.0, 0.0, 0.0]];

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
    *out = Some(ModuleDataKind::TensileFormationUpdate(Arc::new(TensileFormationUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The words of the state the dump holds after the xfer: the module's 25 and object 1's position and orientation.
const STATE_WORDS: usize = 25 + 4;

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
    let enabled = u32_(&mut p);
    let sound = str_(&mut p);
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {enabled} {sound:?} constructor {log:?} wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..STATE_WORDS).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 50 {
        lines.push(format!("Enabled = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("CrackSound = {}", pick(g, &["AvalancheCrack", "IceCrack", "NoSound"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "Enabled = maybe", "CrackSound", "Enabled"]).to_string());
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
    enabled: u64,
    sounds: u64,
    scans: u64,
    damage_states: u64,
    rubble: u64,
    links: u64,
    moved: u64,
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

fn template_of(shrub: bool) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R34Thing".to_vec(), ..Default::default() };
    if shrub {
        values.kind_of.push(KINDOF_SHRUBBERY);
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeAnswers {
    let o = w.objects.find_mut(id).unwrap();
    match o.modules[slot as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => &mut p.answers,
        _ => panic!("a probe in slot {slot}"),
    }
}

/// The word `a` of the configuration applied (the driver's `apply`); `bodies` are the body slots (`None`: no body).
fn apply(w: &mut World, cfg: &[u32; 24], a: usize, bodies: &[Option<u16>; 5]) {
    match a {
        2..=6 => {
            let k = a - 2;
            let id = k as ObjectId + 1;
            let f = cfg[a];
            {
                let o = w.objects.find_mut(id).unwrap();
                o.drawable = if f >> 1 & 1 != 0 { 7 + k as i32 } else { INVALID_DRAWABLE_ID };
                o.thing.friend_set_template(template_of(f >> 2 & 1 != 0));
            }
            if let Some(slot) = bodies[k] {
                probe_mut(w, id, slot).damage_state = Some((f >> 4 & 3) as i32);
            }
        }
        8..=11 => {
            let id = (a - 7) as ObjectId + 1;
            let mut p = w.objects.find(id).unwrap().get_position();
            p.x = f32::from_bits(cfg[a]);
            w.set_position(id, &p);
        }
        13..=16 => {
            let id = (a - 12) as ObjectId + 1;
            let mut p = w.objects.find(id).unwrap().get_position();
            p.y = f32::from_bits(cfg[a]);
            w.set_position(id, &p);
        }
        17 => {
            let mut scan = Vec::new();
            for k in 0..5 {
                let id = cfg[17] >> (4 * k) & 15;
                if id == 0 {
                    break;
                }
                if id <= 5 {
                    scan.push(id as ObjectId);
                }
            }
            w.cuts.range_scan = scan;
        }
        18 => w.cuts.closest_object_at = if (1..=5).contains(&cfg[18]) { cfg[18] as ObjectId } else { 0 },
        19 => w.cuts.audio_playing = cfg[19] != 0,
        20 | 21 => {
            for c in 0..8 {
                w.cuts.terrain_ground[c] = GROUNDS[(cfg[20] >> (4 * c) & 7) as usize];
                let n = NORMALS[(cfg[21] >> (4 * c) & 7) as usize];
                w.cuts.terrain_normal[c] = Coord3D { x: n[0], y: n[1], z: n[2] };
            }
        }
        22 => {
            let r = f32::from_bits(cfg[22]);
            w.objects.find_mut(1).unwrap().geometry_info = GeometryInfo::new(GeometryType::Sphere, false, r, r, r);
        }
        _ => {}
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, tally: &mut Tally) -> Vec<String> {
    let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    let flags = |v: &[i32]| v.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::PathfinderCreateWallFromFootprint { object }) => log.push(format!("wallcreate {object}")),
                Some(CutCall::PathfinderRemoveWallFromFootprint { object }) => {
                    tally.enabled += 1;
                    log.push(format!("wallremove {object}"))
                }
                Some(CutCall::PartitionIterateObjectsInRangeOf { object, range, dc }) => {
                    tally.scans += 1;
                    log.push(format!("scan {object} {:08x} {dc}", b(range)))
                }
                Some(CutCall::PartitionGetClosestObjectAt { position: p, range, dc }) => {
                    log.push(format!("closestat {:08x} {:08x} {:08x} {:08x} {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                Some(CutCall::AudioIsCurrentlyPlaying { handle }) => log.push(format!("isplaying {handle}")),
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => {
                    tally.sounds += 1;
                    log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name)))
                }
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(v), .. }) => log.push(format!("setflags {}", flags(&v))),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(v), .. }) => log.push(format!("clearflags {}", flags(&v))),
                _ => {}
            }
        } else if let Some(l) = t.strip_prefix("bt setdamagestate ") {
            tally.damage_states += 1;
            if l.ends_with(" 3") {
                tally.rubble += 1;
            }
            log.push(format!("setdamagestate {l}"));
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let ModuleDataKind::TensileFormationUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, u32::from(d.enabled));
    put_str(&mut v, &d.crack_sound.event_name);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    let body = || {
        let mut b = NewModule::plain(b"R34Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { trace_damage_state: true, ..Default::default() }, 0.0, Default::default())));
        b
    };
    let member = || {
        let mut m = NewModule::plain(b"TensileFormationUpdate", b"ModuleTag_Member");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        m
    };
    let mut bodies: [Option<u16>; 5] = [None; 5];
    for k in 0..5usize {
        let f = cfg[2 + k];
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"TensileFormationUpdate", b"ModuleTag_TFU", &data).unwrap().unwrap());
        } else if f & 1 != 0 {
            modules.push(member());
        }
        if f >> 3 & 1 != 0 {
            modules.push(body());
        }
        let id = w.new_object(NewObject { template: template_of(false), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        bodies[k] = w.find_module(id, b"R34Body");
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[7 + k]), y: f32::from_bits(cfg[12 + k]), z: 0.0 });
    }
    for a in 2..=22 {
        apply(&mut w, &cfg, a, &bodies);
    }
    let idx = w.find_module(1, b"TensileFormationUpdate").unwrap();
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
        let log = effects(&mut w, 0, tally);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (cw, cl) = wake_count(&trace);
        put_u32(&mut v, cw);
        put_u32(&mut v, cl);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        if o[0] == 2 && (2..=22).contains(&o[1]) && o[1] != 7 && o[1] != 12 {
            cfg[o[1] as usize] = o[2];
            apply(&mut w, &cfg, o[1] as usize, &bodies);
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        // (the scene's own changes are not the module's effects)
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let pos_before = w.objects.find(1).unwrap().get_position();
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
        if w.objects.find(1).unwrap().get_position() != pos_before {
            tally.moved += 1;
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
        let log = effects(&mut w, events_before, tally);
        tally.effects += log.len() as u64;
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
        let ob = w.objects.find(1).unwrap();
        let Some(UpdateBody::TensileFormationUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let words = m.state_words();
        if o[0] == 0 && words[1] == 1 && words[8] != 0 {
            tally.links += 1;
        }
        for s in words {
            put_u32(&mut v, s);
        }
        let p = ob.get_position();
        for x in [p.x, p.y, p.z, ob.thing.orientation()] {
            put_u32(&mut v, x.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R34_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn tensile_formation_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0034_7E45);
    let mut read = Check::new("R34 data read (throw, message)");
    let mut run = Check::new("R34 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let coords = [0.0f32, 15.0, -22.5, 40.0, 8.25, -60.0, 120.0];
    let gen_object = |g: &mut Gen, k: usize| -> u32 {
        let member = u32::from(g.below(4) != 0);
        let drawable = u32::from(if k == 0 { g.below(10) != 0 } else { g.below(2) == 0 });
        let shrub = u32::from(g.below(5) == 0);
        let body = u32::from(if k == 0 { g.below(20) != 0 } else { g.below(4) != 0 });
        let state = if g.below(3) == 0 { g.below(4) } else { 0 };
        member | drawable << 1 | shrub << 2 | body << 3 | state << 4
    };
    let gen_scan = |g: &mut Gen| (0..5).fold(0, |acc, k| acc | (1 + g.below(5)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(5)))) - 1);
    let gen_cells = |g: &mut Gen| (0..8).fold(0, |acc, k| acc | g.below(8) << (4 * k));
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R34{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 24];
        cfg[0] = g.below(60);
        cfg[1] = g.u32();
        for k in 0..5 {
            cfg[2 + k] = gen_object(&mut g, k);
        }
        for c in &mut cfg[7..17] {
            *c = real(&mut g, &coords);
        }
        cfg[17] = gen_scan(&mut g);
        cfg[18] = if g.below(30) == 0 { 0 } else { 1 + g.below(5) };
        cfg[19] = u32::from(g.below(4) == 0);
        cfg[20] = gen_cells(&mut g);
        cfg[21] = gen_cells(&mut g);
        cfg[22] = real(&mut g, &[5.0, 12.5, 30.0]);
        // a third of the cases run long enough for the slide to end (300 frames of life)
        let n_ops = if g.below(3) == 0 { 330 + g.below(150) as usize } else { 1 + g.below(120) as usize };
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(30) {
                0..=19 => [0, 0, 0, 0],
                20..=21 => {
                    frame += 1 + g.below(40);
                    [1, frame, 0, 0]
                }
                22..=25 => {
                    let a = *pick(&mut g, &[&2u32, &3, &4, &5, &6, &8, &9, &13, &14, &17, &18, &19, &20, &21, &22]);
                    let b = match a {
                        2..=6 => gen_object(&mut g, a as usize - 2),
                        8..=16 => real(&mut g, &coords),
                        17 => gen_scan(&mut g),
                        18 => 1 + g.below(5),
                        19 => g.below(2),
                        20 | 21 => gen_cells(&mut g),
                        _ => real(&mut g, &[5.0, 12.5, 30.0]),
                    };
                    [2, a, b, 0]
                }
                26..=27 => [3, 0, 0, 0],
                28 => [4, 0, 0, 0],
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
            cpp::ref_l9r34_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R34: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} xfers), \
         {} effect lines ({} enablings, {} crack sounds, {} scans, {} damage states set, {} rubble; {} updates linked, {} moves; the compared cases')",
        tally.updates, tally.xfers, tally.effects, tally.enabled, tally.sounds, tally.scans, tally.damage_states, tally.rubble, tally.links, tally.moved
    );
    println!("R34 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.enabled * 4 > cases as u64 && tally.sounds * 8 > cases as u64 && tally.rubble * 20 > cases as u64 && tally.moved > cases as u64,
        "the generator must reach every effect"
    );
}
