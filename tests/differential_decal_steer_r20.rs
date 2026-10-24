#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadiusDecalUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AnimationSteeringUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `RadiusDecalUpdate` and `AnimationSteeringUpdate` (`logic::modules::{radius_decal_update, animation_steering_update}`, with the logic half of
//! `RadiusDecal` in `logic::radius_decal`) against the unchanged C++, `p1w13-sonnet`, batch R20.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_decal_steer_r20.inc`,
//! `ref_l9r20_run`) on the module build's stand-ins: the module's object (id 1) with maybe a controlling player, maybe a drawable whose model
//! condition calls are lines, the attacking status, maybe a physics behaviour whose turning the test sets; the dynamic shroud driver's
//! `RadiusDecal` stand-ins, whose calls are lines and which now keep `m_empty` as `RadiusDecal.cpp` does (`xferRadiusDecal` is its body,
//! verbatim); the stand-in game logic, whose wake calls are counted; and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same object (the module from `modules::new_module`, a `ProbeModule` physics), the
//! decal's calls `ObjectClientEvent::GridDecal` (decal 0), the conditions `ObjectClientEvent::DrawableModelCondition`, the wake calls the
//! trace's `awaken` lines; every op an `update()` as the scheduler runs it, a module call or a change to the scene.
//!
//! Each case picks the module, reads a block `D` (`MinTransitionTime` for `AnimationSteeringUpdate`; nothing for `RadiusDecalUpdate`, so any
//! field throws), mostly well formed, and runs 1 to 24 ops. Compared, bit for bit: the read's throw; the data; per op the throw status, the
//! sleep time, the effects in order (`dclear`, `dcreate`, `dupdate`, `mc`, `clearset`, `clearflags`), the wake calls (how many, the last
//! frame), the xfer status, bytes and CRC, and two state words (the decal is empty and the kill flag; the turn animation and the next transition
//! frame).
//!
//! One line is not compared, by design: the `dclear` of a load (`xferRadiusDecal` clears the decal on load; the Rust xfer has no world to tell
//! the client, which rebuilds its drawing after a load anyway). The decal's `m_empty` after the load is compared.
//!
//! # Results (2026-10-23, `L9R20_CASES=9000`)
//!
//! 9,000 cases (3,621 `RadiusDecalUpdate`, 3,529 `AnimationSteeringUpdate` compared, none excluded; 1,850 data files threw, as the C++ did),
//! 108,608 ops (32,491 updates, 19,208 xfers) and 119,287 op outputs: **0 mismatches**; 30,372 effect lines compared, among them 4,669 decals
//! made (5,039 ops with a decal that is not empty) and 6,537 turn conditions (3,246 to a side, 1,905 back to the centre, 1,386 straight again).
//!
//! Harness corrections on the way, none in the translation:
//!
//! - the saved scheduling word differed after `createRadiusDecal` (96 mismatches in a 2,000-case run): the stand-in game logic records a wake
//!   call and drops it, where the Rust world's sleepy list applies it. The driver now applies the op's wake call to the module as
//!   `GameLogic::friend_awakenUpdateModule` does (`GameLogic.cpp:2983-3024`), as it already did for the constructor's.
//! - then 68 mismatches, all after a load: a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"), so the Rust world's next
//!   wake call met upstream's "illegal index" crash branch and was dropped. The test now runs the game logic's post-process
//!   (`World::load_post_process`) after a load, as a real load does.
//! - the generator reached the last steering branch (straight again) only 132 times in 9,000 cases; half the steering cases now start with a
//!   whole turn, and the decal calls, which the steering has none of, give way to turns and frames.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1871 | a decal never made (always empty) | 5,070 |
//! | 1872 | the wake after `createRadiusDecal` inverted | 5,952 |
//! | 1873 | the decal not killed when the object stops attacking | 753 |
//! | 1874 | the kill flag not saved | 3,578 |
//! | 1881 | the steering ignores the transition time | 2,536 |
//! | 1882 | the left turn ends while still turning left | 2,711 |
//! | 1883 | straight again waits a transition time | 8,377 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_IS_ATTACKING,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::animation_steering_update::AnimationSteeringUpdateModuleData;
use rts_lockstep_engine::logic::modules::radius_decal_update::RadiusDecalUpdateModuleData;
use rts_lockstep_engine::logic::radius_decal::RadiusDecalTemplate;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_events::{
    GridDecalOp, ModelConditionOp, ObjectClientEvent,
};
use rts_lockstep_engine::logic::world::object_state::mask_of;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r20.bin");
const XFER_FILE: &str = "l9_r20.bin";
const CLASSES: [&str; 2] = ["RadiusDecalUpdate", "AnimationSteeringUpdate"];
/// The decal textures, by index (the driver's `g_r20Names`).
const NAMES: [&str; 3] = ["", "RingA", "RingB"];
/// The player the object's team belongs to (the driver's `player.m_idx`).
const PLAYER: i32 = 2;

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
fn read_decal(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::RadiusDecalUpdate(Arc::new(
        RadiusDecalUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}
fn read_steer(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::AnimationSteeringUpdate(Arc::new(
        AnimationSteeringUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: if kind == 0 { read_decal } else { read_steer },
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r20_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    if let ModuleDataKind::AnimationSteeringUpdate(d) = data {
        put_u32(v, d.transition_frames);
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
    if kind == 1 {
        if b.is_empty() {
            return out;
        }
        let f = u32_(&mut p);
        out.push(format!("data {f}"));
    }
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
        let (s0, s1) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {s0:#x} {s1}"));
    }
    out
}

// -- generators --

/// The block `D`: `MinTransitionTime` for the steering, with some probability; now and then a bogus value or an unknown field.
fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if kind == 1 && g.below(100) < 80 {
        lines.push(format!(
            "MinTransitionTime = {}",
            pick(g, &["0", "33", "100", "500"])
        ));
    }
    if !clean {
        lines.push(match g.below(2) {
            0 if kind == 1 => "MinTransitionTime = x".to_string(),
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
    creates: u64,
    non_empty: u64,
    conditions: u64,
    /// The turn conditions by kind: set (centre to a side), clear and set (a side back to centre), clear (straight again).
    turns: [u64; 3],
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

/// The decal and model condition calls among `events`, as the driver's lines.
fn event_line(e: &ObjectClientEvent, tally: &mut Tally) -> Option<String> {
    match e {
        ObjectClientEvent::GridDecal { index, op, .. } => Some(match op {
            GridDecalOp::Clear => format!("dclear {index}"),
            GridDecalOp::Update => format!("dupdate {index}"),
            GridDecalOp::Create {
                template: t,
                radius,
                position,
                player,
            } => {
                tally.creates += 1;
                format!(
                    "dcreate {index} {} {} {:08x} {:08x} {} {} {} {:08x} {:08x},{:08x},{:08x} {}",
                    String::from_utf8_lossy(&t.name),
                    t.shadow_type,
                    t.min_opacity.to_bits(),
                    t.max_opacity.to_bits(),
                    t.opacity_throb_time,
                    t.color,
                    i32::from(t.only_visible_to_owning_player),
                    radius.to_bits(),
                    position.x.to_bits(),
                    position.y.to_bits(),
                    position.z.to_bits(),
                    player.unwrap_or(-1)
                )
            }
            GridDecalOp::SetPosition(p) => format!(
                "dpos {index} {:08x} {:08x} {:08x}",
                p.x.to_bits(),
                p.y.to_bits(),
                p.z.to_bits()
            ),
            GridDecalOp::SetOpacity(o) => format!("dopacity {index} {:08x}", o.to_bits()),
        }),
        ObjectClientEvent::DrawableModelCondition { op, .. } => {
            tally.conditions += 1;
            Some(match op {
                ModelConditionOp::Set(f) => {
                    tally.turns[0] += 1;
                    format!("mc {}", f[0])
                }
                ModelConditionOp::ClearAndSet { clear, set } => {
                    tally.turns[1] += 1;
                    format!("clearset {} {}", clear[0], set[0])
                }
                ModelConditionOp::Clear(f) => {
                    tally.turns[2] += 1;
                    let mut f = f.clone();
                    f.sort_unstable();
                    format!(
                        "clearflags {}",
                        f.iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            })
        }
        _ => None,
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    kind: u32,
    data: ModuleDataKind,
    cfg: &[u32; 1],
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
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let class = CLASSES[kind as usize];
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        class.as_bytes(),
        b"ModuleTag_R20",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut phys = NewModule::plain(b"CtPhysics", b"ModuleTag_Physics");
    phys.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        0.0,
        Default::default(),
    )));
    let me = w
        .new_object(NewObject {
            template: plain,
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m, phys],
        })
        .unwrap();
    assert_eq!(me, 1);
    let phys_slot = w.find_module(me, b"CtPhysics");
    w.friend_bind_to_drawable(me, 7);
    let drawable = w.objects.find(me).unwrap().drawable;
    let idx = w.find_module(me, class.as_bytes()).unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let mut flags = cfg[0];
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            1 => flags = o[1],
            2 => w.set_frame(o[1]),
            6 => flags = (flags & !0x30) | (o[1] & 3) << 4,
            _ => {}
        }
        if flags & 1 != 0 {
            w.cuts.team_players.insert(1, PLAYER);
        } else {
            w.cuts.team_players.remove(&1);
        }
        w.objects.find_mut(me).unwrap().drawable = if flags & 2 != 0 {
            drawable
        } else {
            rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID
        };
        w.set_status(me, mask_of(OBJECT_STATUS_IS_ATTACKING), flags & 4 != 0);
        w.objects.find_mut(me).unwrap().physics = if flags & 8 != 0 { phys_slot } else { None };
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules
            [phys_slot.unwrap() as usize]
            .body
            .as_mut()
        {
            p.answers.turning = match (flags >> 4) & 3 {
                1 => -1,
                2 => 1,
                _ => 0,
            };
        }
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
            1 | 2 | 6 => {}
            3 => {
                if kind == 0 {
                    let template = RadiusDecalTemplate {
                        name: NAMES[(o[1] % 3) as usize].as_bytes().to_vec(),
                        color: o[2] as i32,
                        ..Default::default()
                    };
                    let pos = Coord3D {
                        x: f32::from_bits(o[4]),
                        y: f32::from_bits(o[5]),
                        z: 0.0,
                    };
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::RadiusDecalCreate {
                            template,
                            radius: f32::from_bits(o[3]),
                            pos,
                        },
                    );
                }
            }
            4 => {
                if kind == 0 {
                    w.module_call(me, idx, ModuleCall::RadiusDecalKill);
                }
            }
            5 => {
                if kind == 0 {
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::RadiusDecalKillWhenNoLongerAttacking(o[1] & 1 != 0),
                    );
                }
            }
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                if o[0] == 8 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process puts the module back
                    // in the sleepy list, as after a real load (the C++ object is in no list)
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
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let mut ev_iter = events.iter();
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("evt ") {
                if let Some(l) = ev_iter.next().and_then(|e| event_line(e, tally)) {
                    log.push(l);
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::RadiusDecalUpdate(m)) => Some(m.state_words()),
                    Some(UpdateBody::AnimationSteeringUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        let state = state.expect("the module is back in its slot");
        if kind == 0 && state[0] == 0 {
            tally.non_empty += 1;
        }
        put_u32(&mut v, state[0]);
        put_u32(&mut v, state[1]);
    }
    (v, w)
}

/// The C++ dump with the `dclear` line of every load op dropped (see the module docs).
fn drop_load_clears(kind: u32, d: Vec<String>, ops: &[[u32; 6]]) -> Vec<String> {
    let first = usize::from(kind == 1);
    d.into_iter()
        .enumerate()
        .map(|(i, l)| {
            if i >= first && ops.get(i - first).is_some_and(|o| o[0] == 8) {
                l.replace("\"dclear 0\"", "").replace("log [, ", "log [")
            } else {
                l
            }
        })
        .collect()
}

fn cases() -> usize {
    std::env::var("L9R20_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn radius_decal_and_animation_steering_updates_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0020_DEC5);
    let mut read = Check::new("R20 data read (throw, message)");
    let mut run = Check::new("R20 ops (data, effects, wakes, xfers, state)");
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
        let name = format!("Data\\INI\\R20{}.ini", case % 5).into_bytes();
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
            u32::from(g.below(6) != 0)
                | u32::from(g.below(6) != 0) << 1
                | g.below(2) << 2
                | u32::from(g.below(6) != 0) << 3
                | g.below(3) << 4
        };
        let cfg = [gen_flags(&mut g)];
        let n_ops = 1 + g.below(24) as usize;
        let mut frame = 0u32;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(28) {
                24..=27 => [6, g.below(3), 0, 0, 0, 0],
                // the steering has no decal calls: turn and advance the frame instead
                13..=17 if kind == 1 => {
                    if g.below(2) == 0 {
                        [6, g.below(3), 0, 0, 0, 0]
                    } else {
                        frame += 1 + g.below(60);
                        [2, frame, 0, 0, 0, 0]
                    }
                }
                0..=7 => [0, 0, 0, 0, 0, 0],
                8..=10 => [1, gen_flags(&mut g), 0, 0, 0, 0],
                11..=12 => {
                    frame += 1 + g.below(150);
                    [2, frame, 0, 0, 0, 0]
                }
                13..=15 => [
                    3,
                    g.below(3),
                    *pick(&mut g, &[&0u32, &0xff00_ff00]),
                    (*pick(&mut g, &[&0.0f32, &-5.0, &50.0, &120.5])).to_bits(),
                    (*pick(&mut g, &[&10.0f32, &-3.5])).to_bits(),
                    (*pick(&mut g, &[&7.0f32, &400.0])).to_bits(),
                ],
                16 => [4, 0, 0, 0, 0, 0],
                17 => [5, g.below(2), 0, 0, 0, 0],
                18..=19 => [7, 0, 0, 0, 0, 0],
                20..=21 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();
        // half the steering cases start with a whole turn: to a side, then straight again, with time for each transition
        let mut ops = ops;
        if kind == 1 && g.below(2) == 0 {
            let side = 1 + g.below(2);
            let mut script = vec![[6, side, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0]];
            let mut f = 0u32;
            for _ in 0..3 {
                f += 1 + g.below(600);
                script.push([2, f, 0, 0, 0, 0]);
                script.push([6, 0, 0, 0, 0, 0]);
                script.push([0, 0, 0, 0, 0, 0]);
            }
            // the random ops keep their frames after the script's
            for o in &mut ops {
                if o[0] == 2 {
                    o[1] += f;
                }
            }
            script.extend(ops);
            ops = script;
        }
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
            cpp::ref_l9r20_run(
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
        let (dr, dc) = (
            decode(kind, &rd, ops.len()),
            drop_load_clears(kind, decode(kind, &buf, ops.len()), &ops),
        );
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
        "R20: {cases} cases ({} RadiusDecalUpdate, {} AnimationSteeringUpdate compared), {read_failed} data blocks threw, {excluded} excluded \
         (undefined upstream), {ops_run} ops, {} updates, {} xfers, {} effect lines, {} decals created, {} ops with a decal, {} model conditions {:?}",
        compared[0], compared[1], tally.updates, tally.xfers, tally.effects, tally.creates, tally.non_empty, tally.conditions, tally.turns
    );
    read.finish();
    run.finish();
    assert!(
        tally.non_empty * 4 > cases as u64 && tally.turns.iter().all(|t| t * 20 > cases as u64),
        "the generator must reach every path"
    );
}
