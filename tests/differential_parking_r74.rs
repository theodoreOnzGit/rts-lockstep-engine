#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ParkingPlaceBehavior` (`logic::modules::parking_place_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R74.
//!
//! # Method
//!
//! The C++ side is `ParkingPlaceBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_parking_r74.inc`,
//! `ref_l9r74_run`), on the module build's stand-ins: the airfield (under construction, sold or not; a team; maybe a production update whose
//! `setHoldDoorOpen` is a line; the art's bones, a table of seventeen with transforms turned about z, a missing one the object's position and
//! transform as upstream; a position and an orientation), six planes (the jet or the helipad's template; a team; effectively dead or not; in
//! the air or not; maybe an AI whose orders are lines, maybe a body whose damage and healing are lines; a `JetAIUpdate`, the module build's
//! stand-in, its parking offset and takeoff the test's), `defect` and `setStatus` (lines), `setProducer`, the pathfinder (a line), the REAL
//! `Object::kill`, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the production, AIs, bodies and jets probes; the bones
//! `cuts.bone_transforms`; the Object-level calls records, `cuts.body.stub_object_calls`). Every op an `update()`, `onDie`, a call of the
//! exit or parking place interface, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data and the constructor's state; per op the throw status, the result, the `PPInfo` written, the effects in order, the xfer status, bytes
//! (the scheduling word masked) and CRC, the module's state (the spaces with their bones, runways, healees, rally point, heal frame) and each
//! plane's producer, position and orientation. The wake calls are not compared: the module never sleeps, and the constructor's wake is the
//! game logic's registration.
//!
//! A negative `NumRows` or `NumCols` makes `buildInfo`'s `reserve` throw `std::length_error` (status 5; in a load, the xfer harness's
//! `catch (...)`, xfer status -1): the Rust side reports `IniErrorKind::LengthError`, the spaces' before anything is built, the runways'
//! after the spaces are. Upstream's load builds after the version and `UpdateModule::xfer`; the world stages the build before the xfer
//! (it reads the bones) and the module applies it there.
//!
//! # Results (2026-10-26, `L9R74_CASES=9000`)
//!
//! 9,000 cases (589 data blocks threw, as the C++ did; 2,973 excluded as undefined upstream: 1,894 a runway index beyond the runways
//! (`calcPPInfo`, `reserveRunway`, `getRunwayReservation`), 489 an exit whose space was not reserved (an uninitialised hangar position),
//! 409 a door reserved for a `NULL` template, 181 a healee with no body), 141,585 ops and 299,484 outputs: **0 mismatches**. In the
//! compared cases: 28,457 updates, 10,178 xfers and 22,392 effect lines; 11,656 space reservations, 2,444 `PPInfo` calculations, 14,316
//! runway reservations, 683 exits, 929 heals, 1,010 kills and 661 defections; 1,637 `reserve`s of a negative count threw; 6 dead planes
//! let go of their place in line and 54 producers were cleared by a defection. The first runs differed in two places, both the
//! translation's: the load built the spaces before the version was read (a load that failed there still built them), and a negative
//! count did not throw.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught. (2451 and 2454 first survived, unreached: no plane was the
//! airfield's product, and a plane next in line rarely died. The scene now sets a plane's producer, the generator kills one of the first
//! two planes, and the test asserts both are reached.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2450 | every space on runway 0 | 48,035 |
//! | 2451 | a dead plane next in line not let go | 82 |
//! | 2452 | the approach half the runway's length beyond its end, not three quarters | 2,955 |
//! | 2453 | no plane put next in line | 274 |
//! | 2454 | a defecting airfield's plane keeps its producer | 54 |
//! | 2455 | the space whose door is reserved for an exit not marked reserved | 1,638 |
//! | 2456 | the next heal frame not saved | 18,124 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_PRODUCED_AT_HELIPAD;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_COUNT, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::parking_place_behavior::ParkingPlaceBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply, PPInfo};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r74.bin");
const XFER_FILE: &str = "l9_r74.bin";

/// The art's bones (the driver's `kR74Bones`).
const BONES: [&str; 17] = [
    "Runway1Park1Han",
    "Runway1Parking1",
    "Runway1Prep1",
    "Runway2Park1Han",
    "Runway2Parking1",
    "Runway2Prep1",
    "Runway1Park2Han",
    "Runway1Parking2",
    "Runway1Prep2",
    "Runway2Park2Han",
    "Runway2Parking2",
    "Runway2Prep2",
    "RunwayStart1",
    "RunwayStart2",
    "RunwayEnd1",
    "RunwayEnd2",
    "HeliPark01",
];

/// Bone `k`'s place and turn (the driver's `r74BonePose`).
fn bone_pose(k: usize) -> (Coord3D, f32) {
    let k = k as f32;
    (Coord3D { x: 40.0 + 10.0 * k, y: -25.0 + 7.0 * k, z: 0.5 * k }, 0.3 * k - 1.0)
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ParkingPlaceBehavior(Arc::new(ParkingPlaceBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    out.push(format!("data {data:x?}"));
    let state = |r: &mut Reader| -> String {
        let head: Vec<u32> = (0..6).map(|_| r.u32()).collect();
        let ns = r.u32();
        let spaces: Vec<u32> = (0..ns * 15).map(|_| r.u32()).collect();
        let nr = r.u32();
        let runways: Vec<u32> = (0..nr * 9).map(|_| r.u32()).collect();
        let nh = r.u32();
        let heals: Vec<u32> = (0..nh * 2).map(|_| r.u32()).collect();
        let planes: Vec<u32> = (0..6 * 5).map(|_| r.u32()).collect();
        format!("{head:x?} spaces {spaces:x?} runways {runways:x?} heals {heals:x?} planes {planes:x?}")
    };
    out.push(format!("constructed {}", state(&mut r)));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let has_info = r.u32();
        let info: Vec<u32> = if has_info != 0 { (0..30).map(|_| r.u32()).collect() } else { Vec::new() };
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        out.push(format!("op {i}: status {st} result {res:#x} info {info:x?} log {log:?} {x}"));
        out.push(format!("op {i} state {}", state(&mut r)));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 90, "NumRows", &["1", "2", "2", "2", "0"], false);
    field(g, 90, "NumCols", &["1", "1", "2", "3"], false);
    field(g, 50, "ApproachHeight", &["0", "50", "12.5"], false);
    field(g, 40, "LandingDeckHeightOffset", &["0", "10", "4.5"], false);
    field(g, 97, "HasRunways", &["Yes", "Yes", "Yes", "Yes", "Yes", "Yes", "Yes", "No"], false);
    field(g, 40, "ParkInHangars", &["Yes", "No"], false);
    field(g, 50, "HealAmountPerSecond", &["0", "5", "12.5"], false);
    if !clean && g.below(20) == 0 {
        lines.push("Bogus = 1".to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn f(x: f32) -> u32 {
    x.to_bits()
}

/// A plane's word (`dead` the per cent chance it is effectively dead).
fn gen_plane(g: &mut Gen, dead: u32) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let team = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
    let offset = (*pick(g, &[&0i32, &0, &3, &-4])) as u32 & 255;
    p(g, 20) | team << 1 | p(g, dead) << 3 | p(g, 30) << 4 | p(g, 20) << 5 | p(g, 90) << 6 | p(g, 85) << 7 | offset << 8 | p(g, 40) << 16
}

fn gen_cfg_word(g: &mut Gen, a: u32, dead: u32) -> u32 {
    match a {
        2 => (if g.below(10) == 0 { 0 } else { 1 + g.below(3) }) | u32::from(g.below(12) == 0) << 2 | u32::from(g.below(12) == 0) << 3,
        3 => {
            // mostly every bone; now and then some missing
            if g.below(4) == 0 {
                g.u32() & 0x1ffff
            } else {
                0x1ffff
            }
        }
        4 | 5 => f(*pick(g, &[&0.0f32, &100.0, &-35.5])),
        6 => f(*pick(g, &[&0.0f32, &0.5, &-2.0])),
        7..=12 => gen_plane(g, dead),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = u32::from(g.below(4) != 0);
    for a in 2..=12u32 {
        cfg[a as usize] = gen_cfg_word(g, a, 8);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut cur = cfg.clone();
    let mut frame = cfg[0];
    // mostly the first two planes, so that they meet on a runway
    let plane = |g: &mut Gen| 2 + if g.below(10) < 7 { g.below(2) } else { g.below(6) };
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=19 => [0, 0, 0, 0, 0],
            20..=25 => {
                frame += 1 + g.below(20);
                [1, frame, 0, 0, 0]
            }
            26..=33 => {
                // a third of them kill one of the first two planes (the rest of its word kept), a third change one of their words
                let a = if g.below(3) != 0 { 7 + g.below(2) } else { 2 + g.below(11) };
                let v = if a <= 8 && g.below(2) == 0 { cur[a as usize] | 8 } else { gen_cfg_word(g, a, 25) };
                cur[a as usize] = v;
                [2, a, v, 0, 0]
            }
            34 => [3, 0, 0, 0, 0],
            35..=37 => [4, 0, 0, 0, 0],
            38..=40 => [5, 0, 0, 0, 0],
            41 => [6, 0, 0, 0, 0],
            _ => {
                let which = *pick(g, &[&0u32, &1, &2, &3, &4, &4, &4, &4, &4, &5, &6, &6, &6, &6, &6, &6, &7, &8, &9, &10, &11, &12, &13, &13, &14, &15, &16, &16, &17, &17, &18, &19, &20, &21, &22]);
                let (b, c, d) = match which {
                    0 | 1 => (g.below(3), 0, 0),
                    4 => (plane(g), f(*pick(g, &[&0.0f32, &0.0, &2.5])), g.below(2)),
                    6 => (plane(g), u32::from(g.below(5) == 0), 0),
                    10 => (if g.below(8) == 0 { 2 } else { g.below(2) }, g.below(2), 0),
                    13 => (plane(g), u32::from(g.below(4) != 0), 0),
                    15 => (g.below(3), frame + g.below(50), 0),
                    16 => (if g.below(15) == 0 { 0 } else { 1 + g.below(2) }, 0, 0),
                    17 => (plane(g), (*pick(g, &[&0i32, &1, &2, &3, &-2])) as u32, 0),
                    18 => ((*pick(g, &[&0i32, &1, &2, &-2])) as u32, 0, 0),
                    19 => (f(12.5), f(-7.0), 0),
                    _ => (plane(g), 0, 0),
                };
                [7, which, b, c, d]
            }
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    effects: u64,
    spaces: u64,
    infos: u64,
    runways: u64,
    exits: u64,
    heals: u64,
    kills: u64,
    defects: u64,
    length_errors: u64,
    dead_next_purged: u64,
    producers_cleared: u64,
    excluded: BTreeMap<String, u64>,
}

fn set_flag(o: &mut rts_lockstep_engine::logic::world::object::Object, bit: u8, on: bool) {
    if on {
        o.private_status |= bit;
    } else {
        o.private_status &= !bit;
    }
}

/// The probe of `name` on `id`.
fn probe_mut<'a>(w: &'a mut World, id: ObjectId, name: &[u8]) -> Option<&'a mut ProbeModule> {
    let idx = w.find_module(id, name)?;
    let o = w.objects.find_mut(id)?;
    match o.modules[idx as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => Some(p),
        _ => None,
    }
}

struct Scene {
    templates: [Arc<ThingTemplate>; 2],
    ai_slots: [Option<u16>; 6],
    body_slots: [Option<u16>; 6],
}

fn set_scene(w: &mut World, cfg: &[u32], sc: &Scene) {
    let fl = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = fl & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (fl >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_SOLD, (fl >> 3 & 1) as i32);
    }
    w.cuts.bone_transforms.clear();
    for (k, name) in BONES.iter().enumerate() {
        if cfg[3] >> k & 1 != 0 {
            w.cuts.bone_transforms.insert((1, name.as_bytes().to_vec()), bone_pose(k));
        }
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[4]), y: f32::from_bits(cfg[5]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[6]));
    for k in 0..6usize {
        let id = k as ObjectId + 2;
        let b = cfg[7 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(sc.templates[(b & 1) as usize].clone());
            o.team = b >> 1 & 3;
            set_flag(o, PRIVATE_EFFECTIVELY_DEAD, b >> 3 & 1 != 0);
            o.body = if b >> 6 & 1 != 0 { sc.body_slots[k] } else { None };
            o.ai = if b >> 7 & 1 != 0 { sc.ai_slots[k] } else { None };
            o.producer_id = if b >> 16 & 1 != 0 { 1 } else { INVALID_ID };
        }
        let mut p = w.objects.find(id).unwrap().get_position();
        p.z = if b >> 4 & 1 != 0 { 30.0 } else { 0.0 };
        w.set_position(id, &p);
        if let Some(j) = probe_mut(w, id, b"JetAIUpdate") {
            j.answers.jet = Some(((b >> 8 & 255) as u8 as i8 as f32, b >> 5 & 1 != 0));
        }
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let events: Vec<Option<ObjectClientEvent>> = w
        .events
        .drain()
        .into_iter()
        .map(|e| match e {
            ClientEvent::Object(e) => Some(e),
            _ => None,
        })
        .collect();
    let mut event_iter = events.into_iter();
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Body(BodyCall::SetStatus { object, mask, set })) => {
                    let bit = (0..OBJECT_STATUS_COUNT).find(|&i| mask.test(i)).unwrap_or(-1);
                    log.push(format!("setstatus {object} {bit} {}", i32::from(set)))
                }
                Some(CutCall::Body(BodyCall::Defect { object, team, detection_time })) => {
                    tally.defects += 1;
                    log.push(format!("defect {object} {team} {detection_time}"))
                }
                Some(CutCall::PathfinderAddObjectToPathfindMap { object }) => {
                    tally.exits += 1;
                    log.push(format!("pathadd {object}"))
                }
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::GameLogicUpdateObjectsChangedTriggerAreas
                    | CutCall::WeaponSetUpdate { .. },
                ) => {}
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            if let Some(e) = event_iter.next().flatten() {
                log.push(format!("unexpected {e:?}"));
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t);
        } else if t.starts_with("heal ") {
            tally.heals += 1;
            log.push(t);
        } else if let Some(l) = t.strip_prefix("bt ") {
            let word = l.split(' ').next().unwrap_or("");
            if matches!(word, "holddoor" | "aimove" | "aiexitpath") {
                log.push(l.to_string());
            }
        }
    }
    log
}

/// Each runway's next in line for takeoff, from the module's state words.
fn runway_nexts(w: &World, me: ObjectId, slot: usize) -> Vec<u32> {
    let ob = w.objects.find(me).unwrap();
    let Some(UpdateBody::ParkingPlaceBehavior(m)) = ob.behaviors[slot].body.as_ref() else { return Vec::new() };
    let s = m.state_words();
    let ns = s[6] as usize;
    let at = 7 + ns * 15;
    let nr = s[at] as usize;
    (0..nr).map(|r| s[at + 1 + r * 9 + 7]).collect()
}

fn producers(w: &World) -> Vec<ObjectId> {
    (2..8).map(|id| w.objects.find(id).unwrap().producer_id).collect()
}

fn put_info(v: &mut Vec<u8>, i: &PPInfo) {
    for c in [i.parking_space, i.runway_prep, i.runway_start, i.runway_end, i.runway_exit, i.runway_landing_start, i.runway_landing_end, i.runway_approach, i.hangar_internal] {
        for x in [c.x, c.y, c.z] {
            put_u32(v, x.to_bits());
        }
    }
    for x in [i.parking_orientation, i.runway_takeoff_dist, i.hangar_internal_orient] {
        put_u32(v, x.to_bits());
    }
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::ParkingPlaceBehavior(d) = &data else { unreachable!() };
    put_u32(&mut v, d.heal_amount.to_bits());
    put_i32(&mut v, d.num_rows);
    put_i32(&mut v, d.num_cols);
    put_u32(&mut v, d.approach_height.to_bits());
    put_u32(&mut v, d.landing_deck_height_offset.to_bits());
    put_u32(&mut v, u32::from(d.has_runways));
    put_u32(&mut v, u32::from(d.park_in_hangars));

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    factory.new_template(b"R74Jet", 0).expect("a template");
    factory.new_template(b"R74Heli", 0).expect("a template");
    factory.friend_find_template_mut(b"R74Heli").unwrap().data.friend_set_kind_of(&[KINDOF_PRODUCED_AT_HELIPAD]);
    let templates = [Arc::new(factory.find_template(b"R74Jet").unwrap().data.clone()), Arc::new(factory.find_template(b"R74Heli").unwrap().data.clone())];
    let template_ids = [templates[0].get_template_id(), templates[1].get_template_id()];
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.probe_effect_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, is_ai: bool, health: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.is_ai = is_ai;
        let mut p = ProbeModule::new(answers, health, Default::default());
        p.health = health;
        m.body = Some(UpdateBody::ModuleProbe(p));
        m
    };
    let template_of = || Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R74Field".to_vec(), ..Default::default() }));
    let mut modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ParkingPlaceBehavior", b"ModuleTag_Park", &data).unwrap().unwrap()];
    if cfg[1] & 1 != 0 {
        modules.push(probe(b"R74Production", ProbeAnswers { has: 1 << 1, ..Default::default() }, 0, false, 0.0));
    }
    let me = w.new_object(NewObject { template: template_of(), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let mut ai_slots = [None; 6];
    let mut body_slots = [None; 6];
    for k in 0..6usize {
        let modules = vec![
            probe(b"JetAIUpdate", ProbeAnswers { jet: Some((0.0, false)), ..Default::default() }, 0, false, 0.0),
            probe(b"R74Ai", ProbeAnswers::default(), 0, true, 0.0),
            probe(b"R74Body", ProbeAnswers { no_death: true, ..Default::default() }, MODULEINTERFACE_BODY, false, 100.0),
        ];
        let id = w.new_object(NewObject { template: templates[0].clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 2);
        ai_slots[k] = w.find_module(id, b"R74Ai");
        body_slots[k] = w.find_module(id, b"R74Body");
    }
    let sc = Scene { templates, ai_slots, body_slots };
    let idx = w.find_module(me, b"ParkingPlaceBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, &sc);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let put_state = |v: &mut Vec<u8>, w: &World| {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::ParkingPlaceBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        for x in m.state_words() {
            put_u32(v, x);
        }
        for k in 0..6 {
            let o = w.objects.find(k + 2).unwrap();
            let p = o.get_position();
            for x in [o.producer_id as u32, p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), o.thing.orientation().to_bits()] {
                put_u32(v, x);
            }
        }
    };
    put_state(&mut v, &w);
    let tid = |code: u32| match code {
        1 => Some(template_ids[0]),
        2 => Some(template_ids[1]),
        _ => None,
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=12).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &sc);
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (nexts_before, producers_before) = (runway_nexts(&w, me, u.index as usize), producers(&w));
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let mut info: Option<PPInfo> = None;
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                w.module_call(me, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            4..=6 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 4, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                t.xfers += 1;
            }
            7 => {
                let id = o[2] as ObjectId;
                let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
                let r = match o[1] {
                    0 => call(&mut w, ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template: tid(if o[2] != 0 { o[2] } else { 1 }).unwrap() }),
                    1 => call(&mut w, ModuleCall::ParkingPlaceHasAvailableSpaceFor { template: tid(if o[2] != 0 { o[2] } else { 1 }).unwrap() }),
                    2 => call(&mut w, ModuleCall::ParkingPlaceHasReservedSpace { intended: id }),
                    3 => call(&mut w, ModuleCall::ParkingPlaceGetSpaceIndex { id }),
                    4 => {
                        let want_info = o[4] & 1 != 0;
                        let r = call(&mut w, ModuleCall::ParkingPlaceReserveSpace { id, parking_offset: f32::from_bits(o[3]), want_info });
                        t.spaces += 1;
                        if want_info {
                            info = Some(match &r {
                                ModuleReply::PPInfo { info: Some(i), .. } => *i,
                                _ => PPInfo::default(),
                            });
                        }
                        match r {
                            ModuleReply::PPInfo { ok, .. } => ModuleReply::Bool(ok),
                            other => other,
                        }
                    }
                    5 => call(&mut w, ModuleCall::ParkingPlaceReleaseSpace { id }),
                    6 => {
                        t.runways += 1;
                        call(&mut w, ModuleCall::ParkingPlaceReserveRunway { id, for_landing: o[3] != 0 })
                    }
                    7 => {
                        let r = call(&mut w, ModuleCall::ParkingPlaceCalcPPInfo { id, info: PPInfo::default() });
                        t.infos += 1;
                        info = Some(match r {
                            ModuleReply::PPInfo { info: Some(i), .. } => i,
                            _ => PPInfo::default(),
                        });
                        ModuleReply::Done
                    }
                    8 => call(&mut w, ModuleCall::ParkingPlaceReleaseRunway { id }),
                    9 => call(&mut w, ModuleCall::ParkingPlaceGetRunwayCount),
                    10 => call(&mut w, ModuleCall::ParkingPlaceGetRunwayReservation { runway: o[2] as i32, kind: o[3] as i32 }),
                    11 => call(&mut w, ModuleCall::ParkingPlaceTransferRunwayReservation { id }),
                    12 => {
                        let a = call(&mut w, ModuleCall::ParkingPlaceGetApproachHeight).as_real(0.0).to_bits();
                        let b = call(&mut w, ModuleCall::GetLandingDeckHeightOffset).as_real(0.0).to_bits();
                        ModuleReply::Int((a ^ b) as i32)
                    }
                    13 => {
                        if w.objects.find(id).is_some() && id != 0 {
                            call(&mut w, ModuleCall::ParkingPlaceSetHealee { id, add: o[3] != 0 })
                        } else {
                            ModuleReply::Done
                        }
                    }
                    14 => call(&mut w, ModuleCall::ParkingPlaceKillAllParkedUnits),
                    15 => call(&mut w, ModuleCall::DefectAllParkedUnits { new_team: o[2] % 3 + 1, detection_time: o[3] }),
                    16 => call(&mut w, ModuleCall::ReserveDoorForExit { template: tid(o[2]), specific: 0 }),
                    17 => {
                        if w.objects.find(id).is_some() && id != 0 {
                            t.exits += 0;
                            call(&mut w, ModuleCall::ExitObjectViaDoor { obj: id, door: o[3] as i32 })
                        } else {
                            ModuleReply::Done
                        }
                    }
                    18 => call(&mut w, ModuleCall::UnreserveDoorForExit(o[2] as i32)),
                    19 => call(&mut w, ModuleCall::SetRallyPoint(Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: 0.0 })),
                    20 => match call(&mut w, ModuleCall::GetRallyPoint) {
                        ModuleReply::Coord(p) => ModuleReply::Int((p.x.to_bits() ^ p.y.to_bits() ^ 1) as i32),
                        _ => ModuleReply::Int(0),
                    },
                    21 | 22 => {
                        let c = if o[1] == 21 { ModuleCall::GetExitPosition } else { ModuleCall::GetNaturalRallyPoint };
                        match call(&mut w, c) {
                            ModuleReply::Coord(p) => ModuleReply::Int((1 ^ p.x.to_bits() ^ p.y.to_bits() ^ p.z.to_bits()) as i32),
                            _ => ModuleReply::Int(0),
                        }
                    }
                    _ => unreachable!(),
                };
                res = match r {
                    ModuleReply::Bool(b) => u32::from(b),
                    ModuleReply::Int(i) => i as u32,
                    ModuleReply::Id(id) => id as u32,
                    _ => 0,
                };
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().find(|(_, e)| !e.is_undefined());
        // a load's `std::length_error` (its `buildInfo`) is the xfer harness's `catch (...)`: xfer status -1
        let threw = match threw {
            Some((_, e)) if (4..=6).contains(&o[0]) && matches!(e.kind, IniErrorKind::LengthError) => {
                xst = -1;
                None
            }
            t => t,
        };
        let st = match threw {
            Some((_, e)) if matches!(e.kind, IniErrorKind::BadArg) => 3,
            Some((_, e)) if matches!(e.kind, IniErrorKind::LengthError) => 5,
            Some(_) => 2,
            None => 0,
        };
        if st != 0 {
            res = 0;
        }
        if st == 5 || xst == -1 {
            t.length_errors += 1;
        }
        // the reach of two planted bugs: a dead plane's place in line let go, a defecting airfield's plane's producer cleared
        let nexts_after = runway_nexts(&w, me, u.index as usize);
        for (r, &n) in nexts_before.iter().enumerate() {
            let dead = w.objects.find(n as ObjectId).is_some_and(|p| p.private_status & PRIVATE_EFFECTIVELY_DEAD != 0);
            if n != 0 && dead && nexts_after.get(r) == Some(&0) {
                t.dead_next_purged += 1;
            }
        }
        if o[0] == 7 && o[1] == 15 {
            t.producers_cleared += producers_before.iter().zip(producers(&w)).filter(|(b, a)| **b == me && *a == 0).count() as u64;
        }
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        put_u32(&mut v, u32::from(info.is_some()));
        if let Some(i) = &info {
            put_info(&mut v, i);
        }
        let log = effects(&mut w, &mut t);
        t.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        put_state(&mut v, &w);
    }
    drop(t);
    let excluded = match w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
        Some((_, e)) => {
            let reason = format!("{e}");
            let mut t = tally.borrow_mut();
            *t = tally_before;
            *t.excluded.entry(reason).or_insert(0) += 1;
            true
        }
        None => false,
    };
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn parking_place_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R74 ParkingPlaceBehavior",
        seed: 0x0074_9A4C,
        cases_var: "L9R74_CASES",
        ini_stem: "Pp",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r74_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R74: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} space \
         reservations, {} PPInfo calcs, {} runway reservations, {} exits, {} heals, {} kills, {} defections); {} reserves of a negative count threw, {} dead planes let go of their place in line, {} producers cleared by a defection",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.spaces, t.infos, t.runways, t.exits, t.heals, t.kills, t.defects, t.length_errors, t.dead_next_purged, t.producers_cleared
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.spaces > 0 && t.runways > 0 && t.exits > 0 && t.heals > 0 && t.kills > 0 && t.defects > 0 && t.length_errors > 0 && t.dead_next_purged > 0 && t.producers_cleared > 0, "the generator must reach every effect");
}
