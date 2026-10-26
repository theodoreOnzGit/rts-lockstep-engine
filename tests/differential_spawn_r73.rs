#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SpawnBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SpawnBehavior` (`logic::modules::spawn_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R73.
//!
//! # Method
//!
//! The C++ side is `SpawnBehavior.h` and `SpawnBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit
//! (`differential/l9_spawn_r73.inc`, `ref_l9r73_run`; the real interface renamed for the include, as the module build has a stand-in of
//! it), on the module build's stand-ins: the spawner (a team of three with its player, player 0 the neutral one; maybe an AI with its last
//! command source, an exit interface whose door is the test's and whose four calls are lines, a body whose `setInitialHealth` is a line, a
//! drawable, selected or not; reconstructing, under construction, sold or not; produced by the barracks or not; its veterancy and position),
//! the barracks (a structure or not, maybe an exit interface), the spawn (the thing factory: `SpawnA` a slaved update (its calls lines,
//! self-tasking or not), an AI, a body, a drawable and a stealth update (allowed to stealth or not); `SpawnB` an AI, a body and a drawable;
//! `SpawnC` nothing; each spawn's health, selection, veterancy, death, position, `NO_ATTACK` and the two attack answers the test's), the
//! player's `onUnitCreated`, the AI orders, the status, disabled and veterancy calls, the attack questions, the selection message and the
//! UI's selection, the distances (lines), the REAL `Object::kill`, `DieMuxData`, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the thing factory's three templates (their modules untranslated, so made as probes through
//! `cuts.untranslated_probe` and `cuts.untranslated_probe_by_name`); the spawn made by the real `World::thing_factory_new_object`; the
//! spawner's AI, exit and body probes; the Object-level calls records (`cuts.body.stub_object_calls`); the player's `onUnitCreated` and the
//! attack questions the cut points. Every op an `update()`, `onDie`, `onDamage`, `onSpawnDeath`, `onDelete`, a call of the
//! `SpawnBehaviorInterface`, a change to the scene, a spawn or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 80 ops. Compared, bit for bit: the read's throw; the
//! data and the constructor's state; per op the throw status, the result, the effects in order, the xfer status, bytes and CRC, the module's
//! state (the counters, flags, replacement times, spawn ids and the spawn template's name), the spawner's health box offset and whether it
//! was masked, and the spawn made with each one's producer.
//!
//! Not compared (upstream undefined, and the 64-bit reference build differs from the shipped game, as batch R29 found): a death of
//! `DEATH_NORMAL` and a spawner of `LEVEL_REGULAR`, whose die mux tests shift by -1 (`logic::damage::death_bit` and
//! `common::game_common::veterancy_bit` pin the shipped answer). The spawner's veterancy is 1 to 3 and `onDie`'s death type 1 to 3.
//!
//! One difference of the stand-ins is mapped, by design: the module build's `aiIdle` line has no command source, so the Rust probe's
//! `aiidlefrom <object> <source>` is compared as `aiidle <object>`.
//!
//! # Results (2026-10-26, `L9R73_CASES=9000`)
//!
//! 9,000 cases (1,463 data blocks threw, as the C++ did; 707 excluded as undefined upstream: 289 an empty `SpawnTemplateName` list, 267
//! a spawner or spawn with no body where it is read, 143 a spawn with no controlling player, 7 an aggregate with no drawable, 1 an
//! aggregate health out of an `Int`), 277,376 ops and 575,242 outputs: **0 mismatches**. In the compared cases: 124,832 updates, 19,289
//! xfers and 78,434 effect lines; 4,925 spawn made, 1,739 by budding and 49 by the barracks' door; 17,437 aggregate states with 643
//! selections; 5,510 deaths of the spawner and 378 kills of spawn; 36,143 calls of the interface. The first runs differed in the test's
//! own bookkeeping (the stand-in templates had no names; the spawn listed newest first; a throw's result; the real `isAbleToAttack` wanting a
//! weapon), in the 64-bit shift above, and in one translation defect: the `getCanAnySlaves...` questions asked every spawn before
//! reckoning, where upstream returns at the first possible answer.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught. (2441 first survived: no spawn's producer was compared; the
//! state now lists them.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2440 | `onDelete` destroys nothing | 154 |
//! | 2441 | the spawner's death leaves the spawn's producer | 23,979 |
//! | 2442 | `reclaimOrphanSpawn` searches every name | 97 |
//! | 2443 | the template name iterator always wraps | 941 |
//! | 2444 | the first spawn counted from -1 | 81,530 |
//! | 2445 | a dead spawn replaced at once | 8,250 |
//! | 2446 | a spawn of lower veterancy not raised | 690 |
//! | 2447 | the spawner not selected with its spawn | 328 |
//! | 2448 | the burst flag not saved | 160 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_CAN_ATTACK, KINDOF_STRUCTURE};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{
    OBJECT_STATUS_COUNT, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK, OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DamageInfo, DAMAGE_NUM_TYPES};
use rts_lockstep_engine::logic::module::behavior_module::BehaviorModuleData;
use rts_lockstep_engine::logic::modules::spawn_behavior::SpawnBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r73.bin");
const XFER_FILE: &str = "l9_r73.bin";

const SPAWNS: [&str; 3] = ["SpawnA", "SpawnB", "SpawnC"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpawnBehavior(Arc::new(SpawnBehaviorModuleData::friend_new_module_data(Some(ini))?)));
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
    let ints: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    let bools: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let mut damage = Vec::new();
    loop {
        let x = r.u32();
        if x == 0xFFFF_FFFF {
            break;
        }
        damage.push(x);
    }
    let n = r.u32();
    let names: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data {ints:?} {bools:?} damage {damage:?} names {names:?}"));
    let state = |r: &mut Reader| -> String {
        let head: Vec<u32> = (0..10).map(|_| r.u32()).collect();
        let nt = r.u32();
        let times: Vec<u32> = (0..nt).map(|_| r.u32()).collect();
        let ni = r.u32();
        let ids: Vec<u32> = (0..ni).map(|_| r.u32()).collect();
        let name = r.str();
        let rest: Vec<u32> = (0..5).map(|_| r.u32()).collect();
        let producers: Vec<u32> = (0..rest[4]).map(|_| r.u32()).collect();
        format!("{head:x?} times {times:x?} ids {ids:?} template {name:?} {rest:x?} producers {producers:?}")
    };
    out.push(format!("constructed {}", state(&mut r)));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
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
    field(g, 90, "SpawnNumber", &["1", "2", "3", "3", "4", "0"], false);
    field(g, 60, "SpawnReplaceDelay", &["0", "100", "1000", "3000"], false);
    field(g, 30, "OneShot", &["Yes", "No"], false);
    field(g, 20, "CanReclaimOrphans", &["Yes", "No"], false);
    field(g, 35, "AggregateHealth", &["Yes", "No"], false);
    field(g, 50, "ExitByBudding", &["Yes", "Yes", "No"], false);
    field(g, 40, "SpawnedRequireSpawner", &["Yes", "No"], false);
    field(g, 30, "InitialBurst", &["0", "1", "2", "5"], false);
    field(g, 25, "SlavesHaveFreeWill", &["Yes", "No"], false);
    field(g, 15, "PropagateDamageTypesToSlavesWhenExisting", &["EXPLOSION", "SMALL_ARMS ARMOR_PIERCING", "NONE +FLAME"], true);
    field(g, 15, "DeathTypes", &["ALL -CRUSHED", "NONE +NORMAL"], true);
    field(g, 10, "ExemptStatus", &["SOLD"], true);
    if g.below(100) < 96 {
        let n = 1 + g.below(3);
        for _ in 0..n {
            lines.push(format!("SpawnTemplateName = {}", pick(g, &["SpawnA", "SpawnA", "SpawnA", "SpawnB", "SpawnB", "SpawnC", "NoSuchSpawn"])));
        }
    }
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

fn gen_coord(g: &mut Gen) -> u32 {
    f(*pick(g, &[&0.0f32, &5.0, &-12.5, &20.0, &-40.0, &75.0, &150.0]))
}

/// The spawner's flags (cfg[3]).
fn gen_spawner(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let team = if g.below(20) == 0 { 0 } else if g.below(8) == 0 { 1 } else { 2 + g.below(2) };
    team | p(g, 5) << 2 | p(g, 5) << 3 | p(g, 5) << 4 | p(g, 40) << 5 | p(g, 30) << 6 | (1 + g.below(3)) << 8 | (if g.below(3) == 0 { g.below(4) } else { 2 }) << 12
}

fn gen_door(g: &mut Gen) -> u32 {
    (*pick(g, &[&0i32, &0, &1, &2, &-1, &-2])) as u32
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        3 => gen_spawner(g),
        4 | 6 => gen_door(g),
        7 | 8 => gen_coord(g),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[2] = u32::from(g.below(5) != 0) | u32::from(g.below(10) != 0) << 1 | u32::from(g.below(8) != 0) << 2 | u32::from(g.below(8) != 0) << 3;
    cfg[5] = u32::from(g.below(4) != 0) | u32::from(g.below(3) != 0) << 1;
    for a in 3..=8u32 {
        if a != 5 {
            cfg[a as usize] = gen_cfg_word(g, a);
        }
    }
    let n_ops = 1 + g.below(80) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=44 => [0, 0, 0, 0, 0],
            45..=52 => {
                frame += 1 + g.below(200);
                [1, frame, 0, 0, 0]
            }
            53..=57 => {
                let a = *pick(g, &[&3u32, &3, &4, &6, &7, &8]);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            58..=59 => [3, 1 + g.below(3), *pick(g, &[&0u32, &2, &3]), 0, 0],
            60..=61 => [4, f(*pick(g, &[&5.0f32, &12.5])), *pick(g, &[&0u32, &2]), 0, 0],
            62..=65 => [5, 3 + g.below(6), *pick(g, &[&0u32, &2, &3]), 0, 0],
            66 => [6, 0, 0, 0, 0],
            67..=69 => [7, 0, 0, 0, 0],
            70..=72 => [8, 0, 0, 0, 0],
            73 => [9, 0, 0, 0, 0],
            74..=86 => {
                let field = if g.below(4) == 0 { 2 } else { g.below(12) };
                let v = match field {
                    0 | 1 => f(*pick(g, &[&0.0f32, &10.0, &50.0, &100.0, &250.0])),
                    3 => g.below(4),
                    5 | 6 => gen_coord(g),
                    8 | 9 => g.below(5),
                    _ => g.below(2),
                };
                [10, g.below(6), field, v, 0]
            }
            _ => {
                let which = g.below(16);
                let (b, c, d) = match which {
                    0 => (f(*pick(g, &[&0.0f32, &0.5, &1.0])), 0, 0),
                    1 | 3 => (gen_coord(g), gen_coord(g), g.below(3)),
                    2 => (*pick(g, &[&2u32, &3, &1]), 1 + g.below(3), g.below(3)),
                    4 => (g.below(3), *pick(g, &[&2u32, &3, &1]), g.below(3)),
                    5 => (g.below(3), g.below(4), g.below(3)),
                    7 => (g.below(3), 0, 0),
                    8 => (*pick(g, &[&4u32, &10, &2]), frame + g.below(100), 0),
                    9 => (*pick(g, &[&4u32, &10, &2]), 0, 0),
                    10 => (g.below(2), 0, 0),
                    _ => (0, 0, 0),
                };
                [11, which, b, c, d]
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
    spawned: u64,
    budded: u64,
    barracks_exits: u64,
    aggregates: u64,
    selections: u64,
    deaths: u64,
    kills: u64,
    calls: u64,
    excluded: BTreeMap<String, u64>,
}

fn set_flag(o: &mut rts_lockstep_engine::logic::world::object::Object, bit: u8, on: bool) {
    if on {
        o.private_status |= bit;
    } else {
        o.private_status &= !bit;
    }
}

fn level(v: u32) -> VeterancyLevel {
    match v & 3 {
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        3 => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
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

fn set_scene(w: &mut World, cfg: &[u32], drawable: rts_lockstep_engine::common::game_type::DrawableId) {
    let fl = cfg[3];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = fl & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_RECONSTRUCTING, (fl >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (fl >> 3 & 1) as i32);
        o.status.set(OBJECT_STATUS_SOLD, (fl >> 4 & 1) as i32);
        o.producer_id = if fl >> 5 & 1 != 0 { 2 } else { 0 };
        o.experience.level = level(fl >> 8);
        o.drawable = if cfg[2] >> 3 & 1 != 0 { drawable } else { INVALID_DRAWABLE_ID };
    }
    if fl >> 6 & 1 != 0 {
        w.cuts.selected.insert(1);
    } else {
        w.cuts.selected.remove(&1);
    }
    if let Some(p) = probe_mut(w, 1, b"R73AIUpdate") {
        p.answers.last_command_source = (fl >> 12 & 7) as i32;
    }
    if let Some(p) = probe_mut(w, 1, b"R73Exit") {
        p.answers.exit_door = Some(cfg[4] as i32);
    }
    if let Some(p) = probe_mut(w, 2, b"R73Exit") {
        p.answers.exit_door = Some(cfg[6] as i32);
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[7]), y: f32::from_bits(cfg[8]), z: 0.0 });
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them; and whether the spawner was masked.
fn effects(w: &mut World, tally: &mut Tally) -> (Vec<String>, bool) {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    // (every client event, each with its trace line: a new object's `ObjectCreated` too, which is no object event)
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
    let mut masked = false;
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::PlayerOnUnitCreated { player, factory, unit }) => {
                    tally.spawned += 1;
                    log.push(format!("unitcreated {player} {factory} {unit}"))
                }
                Some(CutCall::ObjectGetAbleToAttackSpecificObject { object, attack_type, target, command_source }) => {
                    log.push(format!("canattack {object} {attack_type} {target} {command_source}"))
                }
                Some(CutCall::ObjectGetAbleToUseWeaponAgainstTarget { object, attack_type, victim, pos, command_source }) => log.push(match pos {
                    Some(p) => format!("canuseweapon {object} {attack_type} {victim} {:08x} {:08x} {:08x} {command_source}", b(p.x), b(p.y), b(p.z)),
                    None => format!("canuseweapon {object} {attack_type} {victim} null {command_source}"),
                }),
                Some(CutCall::PartitionGetDistanceSquaredCenter2D { object, other }) => log.push(format!("distsq {object} {other} 0")),
                Some(CutCall::Body(BodyCall::SetDisabledUntil { object, disabled_type, frame })) => log.push(format!("disableduntil {object} {disabled_type} {frame}")),
                Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::SetStatus { object, mask, set })) => {
                    let bit = (0..OBJECT_STATUS_COUNT).find(|&i| mask.test(i)).unwrap_or(-1);
                    if object == 1 && bit == OBJECT_STATUS_MASKED && set {
                        masked = true;
                    }
                    log.push(format!("setstatus {object} {bit} {}", i32::from(set)))
                }
                Some(CutCall::Body(BodyCall::ScoreTheKill { killer, victim })) => log.push(format!("score {killer} {victim}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => log.push(format!("destroy {object}")),
                Some(CutCall::Body(BodyCall::ExperienceSetVeterancyLevel { object, level })) => log.push(format!("xplevel {object} {level}")),
                // the World's own bookkeeping of a move and of a new object (the stand-ins keep a position only, and make a bare object)
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::PartitionRegisterObject { .. }
                    | CutCall::GameLogicUpdateObjectsChangedTriggerAreas
                    | CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction
                    | CutCall::WeaponSetUpdate { .. }
                    | CutCall::PlayerBecomingTeamMember { .. },
                ) => {}
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            match event_iter.next().flatten() {
                Some(ObjectClientEvent::MessageAppendCreateSelectedGroup) => {
                    tally.selections += 1;
                    log.push("msg create".to_string())
                }
                Some(ObjectClientEvent::MessageAppendBooleanArgument(v)) => log.push(format!("msgbool {}", i32::from(v))),
                Some(ObjectClientEvent::MessageAppendObjectIdArgument(id)) => log.push(format!("msgobj {id}")),
                Some(ObjectClientEvent::UiSelectDrawable { object_id }) => log.push(format!("uiselect {object_id}")),
                Some(ObjectClientEvent::UiSetDisplayedMaxWarning { on }) => log.push(format!("maxwarning {}", i32::from(on))),
                // (a new object's drawable starting its model condition and its team: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { .. }, object_id }) if object_id > 2 => {}
                Some(ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id > 2 => {}
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t);
        } else if let Some(l) = t.strip_prefix("bt ") {
            let word = l.split(' ').next().unwrap_or("");
            match word {
                "exitbud" => {
                    tally.budded += 1;
                    log.push(l.to_string())
                }
                "exitdoor" if !l.starts_with("exitdoor 1 ") => {
                    tally.barracks_exits += 1;
                    log.push(l.to_string())
                }
                "initialhealth" => {
                    tally.aggregates += 1;
                    log.push(l.to_string())
                }
                // (the module build's aiIdle line has no command source)
                "aiidlefrom" => {
                    let id = l.split(' ').nth(1).unwrap_or("");
                    log.push(format!("aiidle {id}"))
                }
                "reservedoor" | "exitdoor" | "unreservedoor" | "enslave" | "slaverdie" | "slaverdamage" | "aiforceattack" | "aiattackpos" | "aiidle"
                | "markdetected" => log.push(l.to_string()),
                _ => {}
            }
        }
    }
    (log, masked)
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::SpawnBehavior(d) = &data else { unreachable!() };
    for x in [d.spawn_number, d.spawn_start_number, d.spawn_replace_delay, d.initial_burst] {
        put_i32(&mut v, x);
    }
    for x in [d.is_one_shot, d.can_reclaim_orphans, d.aggregate_health, d.exit_by_budding, d.spawned_require_spawner, d.slaves_have_free_will] {
        put_u32(&mut v, u32::from(x));
    }
    for i in 0..DAMAGE_NUM_TYPES as i32 {
        if d.damage_types_to_propagate_to_slaves.test(i) {
            put_u32(&mut v, i as u32);
        }
    }
    put_u32(&mut v, 0xFFFF_FFFF);
    put_u32(&mut v, d.spawn_template_names.len() as u32);
    for n in &d.spawn_template_names {
        put_str(&mut v, n);
    }

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    for name in SPAWNS {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        // (KINDOF_CAN_ATTACK: the real isAbleToAttack turns on NO_ATTACK alone, as the stand-in's does)
        factory.friend_find_template_mut(name.as_bytes()).unwrap().data.friend_set_kind_of(&[KINDOF_CAN_ATTACK]);
    }
    let plain = || ModuleInfoData::Logic(ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::default())));
    for (name, slaved) in [("SpawnA", true), ("SpawnB", false)] {
        let info = factory.friend_find_template_mut(name.as_bytes()).unwrap().data.friend_behavior_module_info_mut();
        if slaved {
            info.add_module_info(b"R73SlavedUpdate", b"ModuleTag_Slaved", plain(), 0, false, false);
        }
        info.add_module_info(b"R73AIUpdate", b"ModuleTag_AI", plain(), 0, false, false);
        info.add_module_info(b"R73Body", b"ModuleTag_Body", plain(), MODULEINTERFACE_BODY, false, false);
        if slaved {
            info.add_module_info(b"R73StealthUpdate", b"ModuleTag_Stealth", plain(), 0, false, false);
        }
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.untranslated_probe = Some(ProbeAnswers { no_death: true, ..Default::default() });
    w.cuts.untranslated_probe_by_name.insert(b"R73SlavedUpdate".to_vec(), ProbeAnswers { slaved: Some(false), ..Default::default() });
    w.players.set_player_count(4);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, is_ai: bool| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.is_ai = is_ai;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let template_of = |kinds: Vec<i32>| Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R73Thing".to_vec(), kind_of: kinds, ..Default::default() }));
    let mut modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SpawnBehavior", b"ModuleTag_Spawn", &data).unwrap().unwrap()];
    if cfg[2] & 1 != 0 {
        modules.push(probe(b"R73AIUpdate", ProbeAnswers::default(), 0, true));
    }
    if cfg[2] >> 1 & 1 != 0 {
        modules.push(probe(b"R73Exit", ProbeAnswers { exit_door: Some(0), ..Default::default() }, 0, false));
    }
    if cfg[2] >> 2 & 1 != 0 {
        modules.push(probe(b"R73Body", ProbeAnswers { trace_initial_health: true, no_death: true, ..Default::default() }, MODULEINTERFACE_BODY, false));
    }
    let me = w.new_object(NewObject { template: template_of(Vec::new()), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let mut bmods = Vec::new();
    if cfg[5] >> 1 & 1 != 0 {
        bmods.push(probe(b"R73Exit", ProbeAnswers { exit_door: Some(0), ..Default::default() }, 0, false));
    }
    let kinds = if cfg[5] & 1 != 0 { vec![KINDOF_STRUCTURE] } else { Vec::new() };
    let barracks = w.new_object(NewObject { template: template_of(kinds), status: Default::default(), team: None, modules: bmods }).unwrap();
    assert_eq!(barracks, 2);
    let drawable = w.objects.find(me).unwrap().drawable;
    let idx = w.find_module(me, b"SpawnBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, drawable);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SpawnBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    let template = |w: &World| -> Vec<u8> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SpawnBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.spawn_template().map(<[u8]>::to_vec).unwrap_or_default()
    };
    // (the spawn in the order they were made: `ids_in_order` is the object list's, newest first)
    let spawn_ids = |w: &World| -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 2).collect();
        ids.sort_unstable();
        ids
    };
    let put_state = |v: &mut Vec<u8>, w: &World, masked: bool| {
        for x in state(w) {
            put_u32(v, x);
        }
        put_str(v, &template(w));
        let off = w.objects.find(me).unwrap().health_box_offset;
        for x in [off.x.to_bits(), off.y.to_bits(), off.z.to_bits(), u32::from(masked), spawn_ids(w).len() as u32] {
            put_u32(v, x);
        }
        for id in spawn_ids(w) {
            put_u32(v, w.objects.find(id).unwrap().producer_id as u32);
        }
    };
    put_state(&mut v, &w, false);
    let mut saved: Vec<u8> = Vec::new();
    let mut fresh = std::collections::BTreeSet::new();
    for o in ops {
        if o[0] == 2 && (3..=9).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, drawable);
        // (a new spawn of SpawnC has no drawable, as the stand-in's)
        for id in spawn_ids(&w) {
            if w.objects.find(id).unwrap().template().get_name() == b"SpawnC" {
                w.objects.find_mut(id).unwrap().drawable = INVALID_DRAWABLE_ID;
            }
        }
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as i32;
                info.input.source_id = o[2] as ObjectId;
                w.module_call(me, idx, ModuleCall::OnDie(info));
                t.deaths += 1;
            }
            4 => {
                let mut info = DamageInfo::default();
                info.input.amount = f32::from_bits(o[1]);
                info.input.source_id = o[2] as ObjectId;
                w.module_call(me, idx, ModuleCall::OnDamage(info));
            }
            5 => {
                let mut info = DamageInfo::default();
                info.input.source_id = o[2] as ObjectId;
                w.module_call(me, idx, ModuleCall::OnSpawnDeath { dead: o[1] as ObjectId, info });
            }
            6 => {
                w.module_call(me, idx, ModuleCall::OnDelete);
            }
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                t.xfers += 1;
            }
            10 => {
                let ids = spawn_ids(&w);
                if let Some(&id) = ids.get(o[1] as usize) {
                    match o[2] {
                        0 => {
                            if let Some(p) = probe_mut(&mut w, id, b"R73Body") {
                                p.health = f32::from_bits(o[3]);
                            }
                        }
                        1 => {
                            if let Some(p) = probe_mut(&mut w, id, b"R73Body") {
                                p.max_health = f32::from_bits(o[3]);
                            }
                        }
                        2 => {
                            if o[3] != 0 {
                                w.cuts.selected.insert(id);
                            } else {
                                w.cuts.selected.remove(&id);
                            }
                        }
                        3 => w.objects.find_mut(id).unwrap().experience.level = level(o[3]),
                        4 => set_flag(w.objects.find_mut(id).unwrap(), PRIVATE_EFFECTIVELY_DEAD, o[3] != 0),
                        5 | 6 => {
                            let mut p = w.objects.find(id).unwrap().get_position();
                            if o[2] == 5 {
                                p.x = f32::from_bits(o[3]);
                            } else {
                                p.y = f32::from_bits(o[3]);
                            }
                            w.set_position(id, &p);
                        }
                        7 => w.objects.find_mut(id).unwrap().status.set(OBJECT_STATUS_NO_ATTACK, i32::from(o[3] != 0)),
                        8 => {
                            w.cuts.able_to_attack_results.insert(id, o[3] as i32);
                        }
                        9 => {
                            w.cuts.able_to_use_weapon_results.insert(id, o[3] as i32);
                        }
                        10 => {
                            if let Some(p) = probe_mut(&mut w, id, b"R73SlavedUpdate") {
                                p.answers.slaved = Some(o[3] != 0);
                            }
                        }
                        _ => {
                            if let Some(p) = probe_mut(&mut w, id, b"R73StealthUpdate") {
                                p.answers.stealth_allowed = o[3] != 0;
                            }
                        }
                    }
                }
            }
            11 => {
                t.calls += 1;
                let pos = Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: 0.0 };
                let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
                let r = match o[1] {
                    0 => call(&mut w, ModuleCall::SpawnMaySpawnSelfTaskAi(f32::from_bits(o[2]))),
                    1 => call(&mut w, ModuleCall::GetClosestSlave(pos)),
                    2 => {
                        let target = if w.objects.find(o[2] as ObjectId).is_some() { o[2] as ObjectId } else { 0 };
                        call(&mut w, ModuleCall::SpawnOrderSlavesToAttackTarget { target, max_shots: o[3] as i32, command_source: o[4] as i32 })
                    }
                    3 => call(&mut w, ModuleCall::SpawnOrderSlavesToAttackPosition { pos, max_shots: 5, command_source: o[4] as i32 }),
                    4 => {
                        let target = if w.objects.find(o[3] as ObjectId).is_some() { o[3] as ObjectId } else { 0 };
                        call(&mut w, ModuleCall::SpawnGetCanAnySlavesAttackSpecificTarget { attack_type: o[2] as i32, target, command_source: o[4] as i32 })
                    }
                    5 => {
                        let victim = if o[3] & 1 != 0 { 2 } else { 0 };
                        let pos = (o[3] & 2 != 0).then_some(Coord3D { x: f32::from_bits(o[3]), y: 0.0, z: 0.0 });
                        call(&mut w, ModuleCall::SpawnCanAnySlavesUseWeaponAgainstTarget { attack_type: o[2] as i32, victim, pos, command_source: o[4] as i32 })
                    }
                    6 => call(&mut w, ModuleCall::CanAnySlavesAttack),
                    7 => call(&mut w, ModuleCall::SpawnOrderSlavesToGoIdle { command_source: o[2] as i32 }),
                    8 => call(&mut w, ModuleCall::OrderSlavesDisabledUntil { disabled_type: o[2] as i32, frame: o[3] }),
                    9 => call(&mut w, ModuleCall::OrderSlavesToClearDisabled { disabled_type: o[2] as i32 }),
                    10 => call(&mut w, ModuleCall::SpawnGiveSlavesStealthUpgrade(o[2] != 0)),
                    11 => call(&mut w, ModuleCall::SpawnAreAllSlavesStealthed),
                    12 => call(&mut w, ModuleCall::SpawnRevealSlaves),
                    13 => call(&mut w, ModuleCall::SpawnDoSlavesHaveFreedom),
                    14 => call(&mut w, ModuleCall::SpawnStopSpawning),
                    _ => call(&mut w, ModuleCall::SpawnStartSpawning),
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
        // (`ThingFactory::newObject(NULL)` throws `ERROR_BAD_ARG`, an `ErrorCode`: `CATCH_ALL`'s status 3)
        let st = match threw {
            Some((_, e)) if matches!(e.kind, IniErrorKind::BadArg) => 3,
            Some(_) => 2,
            None => 0,
        };
        // (a throw leaves the C++ result unassigned: 0)
        if st != 0 {
            res = 0;
        }
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        let (log, masked) = effects(&mut w, &mut t);
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
        put_state(&mut v, &w, masked);
        // (a new spawn's health, as the stand-in's: nothing in the op that made it reads it)
        for id in spawn_ids(&w) {
            if fresh.insert(id) {
                if let Some(p) = probe_mut(&mut w, id, b"R73Body") {
                    p.health = 100.0;
                    p.max_health = 100.0;
                }
            }
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
fn spawn_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R73 SpawnBehavior",
        seed: 0x0073_5BA7,
        cases_var: "L9R73_CASES",
        ini_stem: "Sp",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r73_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R73: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} spawn made, {} \
         budded, {} by the barracks, {} aggregate states, {} selections, {} spawner deaths, {} kills, {} interface calls)",
        c.cases,
        c.read_failed,
        c.excluded,
        c.ops_run,
        t.updates,
        t.xfers,
        t.effects,
        t.spawned,
        t.budded,
        t.barracks_exits,
        t.aggregates,
        t.selections,
        t.deaths,
        t.kills,
        t.calls
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.spawned > 0 && t.budded > 0 && t.barracks_exits > 0 && t.aggregates > 0 && t.selections > 0 && t.kills > 0, "the generator must reach every effect");
}
