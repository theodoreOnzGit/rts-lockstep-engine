#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SlavedUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SlavedUpdate` (`logic::modules::slaved_update`) against the unchanged C++, `p1w13-sonnet`, batch R57.
//!
//! # Method
//!
//! The C++ side is `SlavedUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_slaved_r57.inc`, `ref_l9r57_run`),
//! on the module build's stand-ins: four objects (ids 1-4; teams, positions, geometries); the drone (object 1: maybe an AI with a locomotor,
//! idle or not; maybe a drawable, with the welding bone or not); the masters (objects 2-4: maybe an AI with a current victim and a path,
//! effectively dead, unmanned, stealthed, a layer, maybe a body whose healing is a line); the teams' relationships; the ground heights; the
//! distances (PartitionManager.cpp's four distance procedures, verbatim); the AI's, the locomotor's and the object's calls (lines); the
//! particle systems and the misc audio's repair sparks; the REAL logic random values (seeded alike), `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` AIs and bodies),
//! three teams and players, `cuts.team_relationships`, `cuts.terrain_ground`, `cuts.contain.pristine_bones`, `cuts.body.particle_templates`,
//! the object calls stubbed (`cuts.body` and `cuts.contain`'s `stub_object_calls`), and the world's logic random values; the distances are
//! `World::partition_distance_squared` (`partition::collide::dist_calc`). Every op an `update()` as the scheduler runs it, `onEnslave`,
//! `onSlaverDie`, `onSlaverDamage`, `getSlaverID`, `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 80 ops. Compared, bit for bit: the read's throw; the
//! data; `onObjectCreated`'s effects; per op the throw status, the result, the effects in order, the xfer status, bytes (the scheduling word
//! masked) and CRC, the state (the slaver, the guard offset, the countdown, the repair state, repairing), the drone's `UNSELECTABLE` bit and
//! disabled bits, and the masters' weapon bonus bits and health.
//!
//! # Results (2026-10-25, `L9R57_CASES=9000`)
//!
//! 9,000 cases (1,040 data blocks threw, as the C++ did; 981 excluded, undefined upstream: 722 a master with no team, whose relationship is
//! asked of `NULL`; 236 a scouting drone whose master has no AI, read unchecked; 12 welding sparks for a drone with no drawable; 11 an `Int`
//! overflow, a weld time from a negative duration times 30), 410,384 ops and 424,342 outputs: **0 mismatches**. In the compared cases:
//! 243,126 updates, 52,942 xfers and 124,941 effect lines: 18,058 moves, 9,874 defections, 829 drones left unmanned, 577 heals, 229 welds
//! begun (the extending state) and 61 sparks.
//!
//! On the way: the first run differed in 52 outputs. Two kinds: the port's `LOCOMOTORSET_PANIC` was 3, not 4 (`AIUpdate.h`'s enum has
//! `WANDER` before it: a port defect, fixed); and the Rust scene gave the drone itself a body the C++ drone has not, which an enslavement to
//! itself healed (a test defect, fixed). The repair path was then reached more often (the drone beside a master, wounded masters).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught, none by a crash of the C++.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2271 | the update every frame, not every 7 | 336,463 |
//! | 2272 | a wounded master not repaired when above the threshold | 3,178 |
//! | 2273 | the drone-spotting bonus never granted | 12,746 |
//! | 2274 | the guard wander radius `GuardWanderRange`, not upstream's `GuardMaxRange` | 9,640 |
//! | 2275 | the repair rate divided by 15, not 30 | 1,691 |
//! | 2276 | no new repair spot after a weld | 515 |
//! | 2277 | the weld does not repair | 1,775 |
//! | 2278 | a freed slave stays unselectable | 86,175 |
//! | 2279 | repairing not saved | 20,650 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::disabled_types::DISABLED_UNMANNED;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNSELECTABLE};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::slaved_update::SlavedUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent, ObjectSound};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r57.bin");
const XFER_FILE: &str = "l9_r57.bin";
/// The masters' geometry radii (the driver's `g_r57Radius`).
const RADIUS: [f32; 4] = [2.0, 5.0, 10.0, 0.5];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SlavedUpdate(Arc::new(SlavedUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..17).map(|_| r.u32()).collect();
    let (sys, bone) = (r.str(), r.str());
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data {data:x?} {sys:?} {bone:?} created {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..15).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 80, "GuardMaxRange", &["30", "60", "0", "100"], false);
    field(g, 50, "GuardWanderRange", &["10", "0", "25"], false);
    field(g, 60, "AttackRange", &["50", "100", "0", "200"], false);
    field(g, 50, "AttackWanderRange", &["10", "0", "30"], false);
    field(g, 50, "ScoutRange", &["80", "0", "150"], false);
    field(g, 40, "ScoutWanderRange", &["15", "0"], false);
    field(g, 50, "DistToTargetToGrantRangeBonus", &["40", "100", "0"], false);
    field(g, 50, "RepairRange", &["15", "0", "30"], false);
    field(g, 50, "RepairMinAltitude", &["5", "10.5", "0"], false);
    field(g, 50, "RepairMaxAltitude", &["15", "20", "0"], false);
    field(g, 75, "RepairRatePerSecond", &["10", "30.5", "0", "60"], false);
    field(g, 60, "RepairWhenBelowHealth%", &["50", "0", "90", "100"], false);
    field(g, 50, "RepairMinReadyTime", &["100", "0", "33"], false);
    field(g, 50, "RepairMaxReadyTime", &["300", "0", "66"], false);
    field(g, 50, "RepairMinWeldTime", &["200", "0", "33", "100"], false);
    field(g, 50, "RepairMaxWeldTime", &["500", "1500", "0", "66"], false);
    field(g, 70, "RepairWeldingSys", &["WeldSparks", "None", "OtherSys", "WeldSparks"], false);
    field(g, 50, "RepairWeldingFXBone", &["WeldBone", "Other"], false);
    field(g, 30, "StayOnSameLayerAsMaster", &["Yes", "No"], false);
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

fn gen_drone(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    p(g, 92) | p(g, 92) << 1 | p(g, 50) << 2 | p(g, 85) << 3 | (if g.below(10) == 0 { 0 } else { 1 + g.below(3) }) << 4 | p(g, 60) << 6
}

fn gen_master(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    p(g, 85) | p(g, 4) << 1 | p(g, 4) << 2 | p(g, 15) << 3 | (if g.below(15) == 0 { 0 } else { 1 + g.below(3) }) << 4 | p(g, 85) << 6 | g.below(4) << 7
        | (if g.below(3) == 0 { 0 } else { 1 + g.below(4) }) << 9 | p(g, 50) << 12 | g.below(4) << 13
}

fn gen_healths(g: &mut Gen) -> u32 {
    (0..3).fold(0, |acc, k| acc | *pick(g, &[&200u32, &150, &90, &20, &199, &0, &120]) << (8 * k))
}

fn gen_rels(g: &mut Gen) -> u32 {
    (0..9).fold(0, |acc, i| acc | (if g.below(3) == 0 { g.below(3) } else { 2 }) << (2 * i))
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &5.0, &12.5, &-20.0, &30.0, &60.0, &-100.0, &3.0])).to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 24];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = g.below(1 << 30);
    cfg[2] = gen_drone(g);
    for c in cfg.iter_mut().take(6).skip(3) {
        *c = gen_master(g);
    }
    cfg[6] = gen_healths(g);
    cfg[7] = gen_rels(g);
    for c in cfg.iter_mut().take(16).skip(8) {
        *c = gen_coord(g);
    }
    // the drone often beside a master (the repair is done within 12 of it)
    if g.below(2) == 0 {
        let m = 1 + g.below(3) as usize;
        cfg[8] = cfg[8 + m];
        cfg[12] = cfg[12 + m];
    }
    cfg[16] = (*pick(g, &[&0.0f32, &10.0, &25.0])).to_bits();
    cfg[17] = g.below(u32::MAX);
    cfg[18] = gen_coord(g);
    cfg[19] = gen_coord(g);
    cfg[21] = g.below(1 << 24);
    let n_ops = 1 + g.below(120) as usize;
    let mut ops = Vec::new();
    // mostly enslaved at once (the update does nothing without a slaver)
    if g.below(10) < 8 {
        ops.push([3, 2 + g.below(3), 0, 0, 0]);
    }
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=59 => [0, 0, 0, 0, 0],
            60..=62 => [1, gen_frame(g), 0, 0, 0],
            63..=76 => {
                let a = 2 + g.below(22);
                let b = match a {
                    2 => gen_drone(g),
                    3..=5 => gen_master(g),
                    6 => gen_healths(g),
                    7 => gen_rels(g),
                    17 => g.below(u32::MAX),
                    21 => g.below(1 << 24),
                    16 => (*pick(g, &[&0.0f32, &10.0, &25.0])).to_bits(),
                    _ => gen_coord(g),
                };
                [2, a, b, 0, 0]
            }
            77..=79 => [3, *pick(g, &[&2u32, &3, &4, &0, &1]), 0, 0, 0],
            80 => [4, 0, 0, 0, 0],
            81..=82 => [5, g.below(12), (*pick(g, &[&10.0f32, &0.5, &100.0])).to_bits(), 0, 0],
            83..=87 => [6, 0, 0, 0, 0],
            88..=92 => [7, 0, 0, 0, 0],
            93..=95 => [8, 0, 0, 0, 0],
            96 => [9, 0, 0, 0, 0],
            _ => [10, 0, 0, 0, 0],
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
    moves: u64,
    heals: u64,
    defects: u64,
    sparks: u64,
    unmanned: u64,
    welding_states: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

/// The probe slots: the AIs and the bodies, by object.
struct Slots {
    ai: [u16; 4],
    body: [u16; 4],
}

fn set_scene(w: &mut World, cfg: &[u32], s: &Slots, bone_name: &[u8]) {
    let f = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.ai = (f & 1 != 0).then_some(s.ai[0]);
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[0] as usize].body.as_mut() {
            p.answers.has_locomotor = f >> 1 & 1 != 0;
            p.answers.idle = f >> 2 & 1 != 0;
        }
        o.drawable = if f >> 3 & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        o.team = f >> 4 & 3;
        o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, 2.0, 2.0, 2.0);
        // (the driver's drone has no body: an enslavement to itself finds none)
        o.body = None;
    }
    if f >> 6 & 1 != 0 {
        let b = cfg[21];
        let pos = Coord3D { x: (b & 255) as f32 * 0.5, y: (b >> 8 & 255) as f32 * 0.5, z: (b >> 16 & 255) as f32 * 0.5 };
        w.cuts.contain.pristine_bones.insert((1, bone_name.to_vec()), pos);
    } else {
        w.cuts.contain.pristine_bones.clear();
    }
    for k in 1..4usize {
        let id = k as ObjectId + 1;
        let g = cfg[2 + k];
        let o = w.objects.find_mut(id).unwrap();
        o.ai = (g & 1 != 0).then_some(s.ai[k]);
        if g >> 1 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.disabled = Default::default();
        o.disabled.set(DISABLED_UNMANNED, (g >> 2 & 1) as i32);
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_STEALTHED, (g >> 3 & 1) as i32);
        o.team = g >> 4 & 3;
        o.body = (g >> 6 & 1 != 0).then_some(s.body[k]);
        o.layer = (g >> 7 & 3) as i32;
        let v = g >> 9 & 7;
        let path = (g >> 12 & 1 != 0).then_some(Coord3D { x: f32::from_bits(cfg[18]), y: f32::from_bits(cfg[19]), z: 0.0 });
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[k] as usize].body.as_mut() {
            p.answers.current_victim = if (1..=4).contains(&v) { v as ObjectId } else { INVALID_ID };
            p.answers.path_last_node = path;
        }
        let r = RADIUS[(g >> 13 & 3) as usize];
        o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, r, r, r);
    }
    for k in 0..4usize {
        let z = if k == 0 { f32::from_bits(cfg[16]) } else { 0.0 };
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[8 + k]), y: f32::from_bits(cfg[12 + k]), z });
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[7] >> (2 * (3 * a + b)) & 3));
        }
    }
    for c in 0..8 {
        w.cuts.terrain_ground[c] = (cfg[17] >> (4 * c) & 15) as f32 * 0.5;
    }
}

fn set_healths(w: &mut World, cfg: &[u32], s: &Slots) {
    for k in 1..4usize {
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.body[k] as usize].body.as_mut() {
            p.health = (cfg[6] >> (8 * (k - 1)) & 255) as f32;
        }
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, names: &mut BTreeMap<u32, Vec<u8>>, tally: &mut Tally) -> Vec<String> {
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
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => {
                    tally.unmanned += 1;
                    log.push(format!("setdisabled {object} {disabled_type}"))
                }
                Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::SetStatus { object, mask, set })) => {
                    let bit = (0..rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_COUNT).find(|&i| mask.test(i)).unwrap_or(-1);
                    log.push(format!("setstatus {object} {bit} {}", i32::from(set)))
                }
                Some(CutCall::Contain(ContainCall::SetLayer { object, layer })) => log.push(format!("setlayer {object} {layer}")),
                Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                    names.insert(id, template);
                }
                Some(CutCall::ParticleSystemSetPosition { id, position: p }) => {
                    let n = names.get(&id).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default();
                    log.push(format!("psys {n} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::ParticleSystemSetLifetimeRange { id, min, max }) => log.push(format!("pslife2 {id} {:08x} {:08x}", b(min), b(max))),
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(v), .. }) => {
                    for a in v {
                        if a == rts_lockstep_engine::common::model_state::MODELCONDITION_FIRING_B {
                            tally.welding_states += 1;
                        }
                        log.push(format!("mc {a}"));
                    }
                }
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(v), .. }) => {
                    for a in v {
                        log.push(format!("mcclear {a}"));
                    }
                }
                Some(ObjectClientEvent::Sound { sound: ObjectSound::RepairSparks, object_id, position }) => {
                    tally.sparks += 1;
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio repairSparks {pos} -1 {object_id}"));
                }
                _ => {}
            }
        } else if t.starts_with("defect ") {
            tally.defects += 1;
            log.push(t.clone());
        } else if t.starts_with("heal ") {
            tally.heals += 1;
            log.push(t.clone());
        } else if let Some(l) = t.strip_prefix("bt ") {
            if let Some(r) = l.strip_prefix("aiidlefrom ") {
                log.push(format!("aiidle {}", r.split(' ').next().unwrap()));
            } else if l.starts_with("aimove ") {
                tally.moves += 1;
                log.push(l.to_string());
            } else if l.starts_with("locoset ") || l.starts_with("locoultra ") || l.starts_with("locoprecise ") || l.starts_with("aiprone ") {
                log.push(l.to_string());
            }
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::SlavedUpdate(d) = &data else { unreachable!() };
    for x in [
        d.guard_max_range,
        d.guard_wander_range,
        d.attack_range,
        d.attack_wander_range,
        d.scout_range,
        d.scout_wander_range,
        d.dist_to_target_to_grant_range_bonus,
        d.repair_range,
        d.repair_when_health_below_percentage,
        d.min_ready_frames,
        d.max_ready_frames,
        d.min_weld_frames,
        d.max_weld_frames,
        i32::from(d.stay_on_same_layer_as_master),
    ] {
        put_i32(&mut v, x);
    }
    for x in [d.repair_min_altitude, d.repair_max_altitude, d.repair_rate_per_second] {
        put_u32(&mut v, x.to_bits());
    }
    put_str(&mut v, &d.welding_sys_name);
    put_str(&mut v, &d.welding_fx_bone);
    let bone_name = d.welding_fx_bone.clone();

    let mut w = World::new();
    w.names = ctx.names;
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, ..Default::default() });
    w.random.init_game_logic_random(cfg[1]);
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.contain.stub_object_calls = true;
    if !d.welding_sys_name.eq_ignore_ascii_case(b"None") {
        w.cuts.body.particle_templates.insert(d.welding_sys_name.clone());
    }
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let probe = |name: &[u8], interfaces: i32, max: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max, Default::default())));
        m
    };
    let mut slots = Slots { ai: [0; 4], body: [0; 4] };
    for k in 1..=4 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SlavedUpdate", b"ModuleTag_Slaved", &data).unwrap().unwrap());
        }
        modules.push(probe(b"R57Ai", 0, 0.0));
        modules.push(probe(b"R57Body", MODULEINTERFACE_BODY, 200.0));
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
        slots.ai[k as usize - 1] = w.find_module(id, b"R57Ai").unwrap();
        slots.body[k as usize - 1] = w.find_module(id, b"R57Body").unwrap();
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"SlavedUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, &slots, &bone_name);
    set_healths(&mut w, &cfg, &slots);
    // the object's creation ran onObjectCreated before the scene was set: the C++ driver runs it after, so it is run (and logged) here
    let mut names = BTreeMap::new();
    let events_before = w.events.events().len();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    let created = effects(&mut w, events_before, &mut names, &mut t);
    put_u32(&mut v, created.len() as u32);
    for l in &created {
        put_str(&mut v, l.as_bytes());
    }
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SlavedUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let mut s = m.state_words();
        s.push(u32::from(ob.status.test(OBJECT_STATUS_UNSELECTABLE)));
        s.push((0..rts_lockstep_engine::common::disabled_types::DISABLED_COUNT).fold(0u32, |acc, i| acc | u32::from(ob.disabled.test(i)) << i));
        s
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=23).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
            if o[1] == 6 {
                set_healths(&mut w, &cfg, &slots);
            }
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots, &bone_name);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let slaver = if (1..=4).contains(&o[1]) { o[1] as ObjectId } else { INVALID_ID };
                w.module_call(me, idx, ModuleCall::SlavedOnEnslave { slaver });
            }
            4 => {
                w.module_call(me, idx, ModuleCall::SlavedOnSlaverDie);
            }
            5 => {
                let mut info = DamageInfo::default();
                info.input.damage_type = o[1] as i32;
                info.input.amount = f32::from_bits(o[2]);
                w.module_call(me, idx, ModuleCall::SlavedOnSlaverDamage(info));
            }
            6..=8 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 6, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 6 {
                    saved = xbytes.clone();
                }
                if o[0] == 7 {
                    w.load_post_process();
                }
                t.xfers += 1;
            }
            9 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
            }
            10 => {
                res = w.module_call(me, idx, ModuleCall::GetSlaverId).as_id() as u32;
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let log = effects(&mut w, events_before, &mut names, &mut t);
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
        for s in state(&w) {
            put_u32(&mut v, s);
        }
        for k in 1..4usize {
            let ob = w.objects.find(k as ObjectId + 1).unwrap();
            put_u32(&mut v, ob.weapon_bonus_condition);
            let health = match ob.modules[slots.body[k] as usize].body.as_ref() {
                Some(UpdateBody::ModuleProbe(p)) => p.health,
                _ => 0.0,
            };
            put_u32(&mut v, health.to_bits());
        }
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
fn slaved_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R57 SlavedUpdate",
        seed: 0x0057_51A7,
        cases_var: "L9R57_CASES",
        ini_stem: "Sl",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r57_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R57: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} moves, \
         {} heals, {} defections, {} sparks, {} unmanned, {} extending states)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.moves, t.heals, t.defects, t.sparks, t.unmanned, t.welding_states
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.moves > 0 && t.heals > 0 && t.defects > 0 && t.sparks > 0 && t.unmanned > 0 && t.welding_states > 0, "the generator must reach every effect");
}
