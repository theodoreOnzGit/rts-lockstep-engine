#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/DumbProjectileBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `DumbProjectileBehavior` (`logic::modules::dumb_projectile_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R68.
//!
//! # Method
//!
//! The C++ side is `DumbProjectileBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_dumb_projectile_r68.inc`,
//! `ref_l9r68_run`; batch R52's physics renames; `MissileAIUpdate.h`, which it includes and does not use, kept out by its guard; BaseType.h's
//! `max` macro), with the REAL `BezierSegment.cpp` (the Bezier build's library), on the module build's stand-ins: the projectile (a position,
//! maybe a physics behaviour, maybe a drawable, maybe a controlling player, a body whose damage is a line), the launcher (maybe; its weapon
//! bonus flags), the victim (maybe; a cylinder of the test's height), a building whose contain's count, garrisonability, immunity and list are
//! the test's, three garrisoned units (kinds, effectively dead, a body); the weapon store's `WEAP_A` and `WEAP_B` (speeds and ranges; the
//! collision test answers the test's set), the launch positioning and the detonation (lines), the terrain extremes along a line (the test's
//! answer, a line), the terrain's layers (all ground), the REAL partition distance procedures, `Object::kill`, `UpdateModule`, `INI`, `Xfer`
//! and logic random values.
//!
//! The Rust side is the real thing: a `World` with the seven objects (the projectile's physics the real `PhysicsBehavior`, the building's
//! contain and every body a probe), `cuts.terrain_extremes`, `cuts.projectile_collides_with`, the weapon store's two templates; the weapon
//! calls are records (`cuts.stub_weapon_calls`), the destroys, kill scores and academy records too (`cuts.body.stub_object_calls`). Every op
//! an `update()`, a launch (at the victim or a position, a slot and barrel, a template), a collision, a change to the scene or the frame, or an
//! xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data; per op the throw status, the result, the effects in order (the launch positioning, the extremes, the collision tests, the scores, the
//! kills' damage, the FX, the academy, the detonation, the detonation's damage, the destroys, the hiding, the layer queries), the xfer status,
//! bytes (the scheduling word masked) and CRC, the module's whole state with its flight path, the projectile's position and transform, its
//! no-collisions status and its physics' rates.
//!
//! # Results (2026-10-26, `L9R68_CASES=9000`)
//!
//! 9,000 cases (765 data blocks threw, as the C++ did; 922 excluded, each reading what upstream leaves undefined: the garrison loop's end
//! iterator dereferenced, or a Bezier error), 220,713 ops and 456,052 outputs, plus 8,078 data reads: **0 mismatches**. In the compared cases:
//! 80,426 updates, 10,951 launches, 5,287 detonations, 42 garrison kills, 5,061 destroys and 14,646 xfers. The first runs had 1,483
//! mismatches with two causes. One was the port's: the flight path's length was taken at tolerance 0.5, where upstream's
//! `getApproximateLength()` defaults to `USUAL_TOLERANCE` (1.0), so the segment counts differed by one in some cases. The other was the
//! test's: the C++ garrison kill's `destroyObject` resolved to an out-of-line copy without the log hook, fixed with batch R50's rename.
//!
//! # Planted bugs
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2381 | `ScaleWeaponSpeed` ignored | 116,502 |
//! | 2382 | the second control point at the first height | 157,973 |
//! | 2383 | every incidental object collided with | 30,927 |
//! | 2384 | the garrison kill falls through to the normal detonation | 127 |
//! | 2385 | a second detonation not suppressed | 37,975 |
//! | 2386 | the flight path's adjustment not limited per frame | 1,550 |
//! | 2387 | the flight path step never advances | 211,850 |
//! | 2388 | the lifespan not saved | 9,003 |
//!
//! 8 of 8 caught.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::dumb_projectile_behavior::DumbProjectileBehaviorModuleData;
use rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehaviorModuleData;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r68.bin");
const XFER_FILE: &str = "l9_r68.bin";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The kinds the test gives a garrisoned unit, by bit (the driver's `g_r68Kinds`), and their names.
const KINDS: [KindOfType; 3] = [KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_HERO];
const KIND_NAMES: [&str; 3] = ["INFANTRY", "VEHICLE", "HERO"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::DumbProjectileBehavior(Arc::new(DumbProjectileBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R68Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    let mut lists = Vec::new();
    for _ in 0..2 {
        let mut kinds = Vec::new();
        loop {
            let k = r.u32();
            if k == u32::MAX {
                break;
            }
            kinds.push(k);
        }
        lists.push(kinds);
    }
    let rest: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    out.push(format!("data {data:x?} kinds {lists:?} {rest:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let mut state: Vec<u32> = (0..16).map(|_| r.u32()).collect();
        let np = state[15];
        state.extend((0..3 * np).map(|_| r.u32()));
        let me: Vec<u32> = (0..19).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        out.push(format!("op {i} state {state:x?} projectile {me:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = if !clean && g.below(20) == 0 { pick(g, &["x", "-"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 50, "MaxLifespan", &["0", "200", "1000", "10000"]);
    field(g, 30, "TumbleRandomly", &["Yes", "No"]);
    field(g, 30, "DetonateCallsKill", &["Yes", "No"]);
    field(g, 30, "OrientToFlightPath", &["Yes", "No", "Yes"]);
    field(g, 70, "FirstHeight", &["0", "20", "75.5"]);
    field(g, 70, "SecondHeight", &["0", "20", "50"]);
    field(g, 70, "FirstPercentIndent", &["0%", "30%", "50%"]);
    field(g, 70, "SecondPercentIndent", &["70%", "90%", "100%"]);
    field(g, 30, "GarrisonHitKillCount", &["0", "1", "2", "3"]);
    field(g, 30, "GarrisonHitKillRequiredKindOf", &KIND_NAMES);
    field(g, 20, "GarrisonHitKillForbiddenKindOf", &KIND_NAMES);
    field(g, 25, "GarrisonHitKillFX", &["FX_Flash", "None"]);
    field(g, 30, "FlightPathAdjustDistPerSecond", &["0", "30", "300"]);
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

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let c = |g: &mut Gen| f(*pick(g, &[&0.0f32, &10.0, &-35.0, &120.0, &400.0]));
    match a {
        3 | 4 | 9 | 10 => c(g),
        5 | 11 => f(*pick(g, &[&0.0f32, &5.0, &30.0])),
        7 => *pick(g, &[&0u32, &0x10, &0x203]),
        12 => f(*pick(g, &[&0.0f32, &10.0, &25.0])),
        13 => {
            let n = g.below(4);
            let mut list = 0;
            for k in 0..n {
                list |= (5 + g.below(3)) << (4 * k);
            }
            p(g, 70) | p(g, 15) << 1 | list << 4
        }
        14..=16 => g.below(8) | p(g, 15) << 3,
        17 => (1..8).fold(0u32, |m, id| m | p(g, 50) << id),
        18 => p(g, 95),
        19 => f(*pick(g, &[&0.0f32, &20.0, &100.0])),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &500]);
    cfg[1] = g.u32();
    cfg[2] = p(g, 40) | p(g, 90) << 1 | p(g, 85) << 2;
    cfg[6] = p(g, 90);
    cfg[8] = p(g, 70);
    for (a, w) in cfg.iter_mut().enumerate().take(20).skip(3) {
        if a != 6 && a != 8 {
            *w = gen_cfg_word(g, a as u32);
        }
    }
    for k in 0..2 {
        cfg[22 + 5 * k] = f(*pick(g, &[&10.0f32, &25.0, &3.0, &60.0]));
        cfg[23 + 5 * k] = f(*pick(g, &[&5.0f32, &1.0, &20.0]));
        cfg[24 + 5 * k] = p(g, 40);
        cfg[25 + 5 * k] = f(*pick(g, &[&200.0f32, &300.0, &100.0]));
        cfg[26 + 5 * k] = f(*pick(g, &[&0.0f32, &20.0, &50.0]));
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops: Vec<[u32; 5]> = Vec::new();
    let launch = |g: &mut Gen| {
        let a = p(g, 60) | g.below(3) << 1 | g.below(3) << 3 | p(g, 50) << 5;
        let c = |g: &mut Gen| f(*pick(g, &[&0.0f32, &50.0, &-80.0, &250.0]));
        [6, a, c(g), c(g), f(*pick(g, &[&0.0f32, &10.0]))]
    };
    // most cases launch early, so that the shell flies
    if g.below(10) < 9 {
        ops.push(launch(g));
    }
    let mut frame = cfg[0];
    while ops.len() < n_ops {
        let o = match g.below(100) {
            // mostly a frame and an update
            0..=59 => {
                frame = frame.wrapping_add(1);
                ops.push([1, frame, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            60..=62 => launch(g),
            63..=69 => [7, *pick(g, &[&0u32, &3, &4, &4, &5, &2]), 0, 0, 0],
            70..=79 => {
                let a = *pick(g, &[&9u32, &10, &11, &13, &14, &15, &16, &17, &18, &19, &3]);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            80..=83 => [3, 0, 0, 0, 0],
            84..=87 => [4, 0, 0, 0, 0],
            88..=90 => [5, 0, 0, 0, 0],
            _ => {
                frame = gen_frame(g);
                [1, frame, 0, 0, 0]
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
    launches: u64,
    detonations: u64,
    garrison_kills: u64,
    destroys: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    let c = |i: usize| Coord3D { x: f32::from_bits(cfg[i]), y: f32::from_bits(cfg[i + 1]), z: f32::from_bits(cfg[i + 2]) };
    w.set_position(1, &c(3));
    if let Some(o) = w.objects.find_mut(2) {
        o.weapon_bonus_condition = cfg[7];
    }
    if w.objects.find(3).is_some() {
        w.set_position(3, &c(9));
        w.objects.find_mut(3).unwrap().geometry_info = GeometryInfo::new(GeometryType::Cylinder, false, f32::from_bits(cfg[12]), 5.0, 5.0);
    }
    let mut list = Vec::new();
    for k in 0..3 {
        let id = cfg[13] >> (4 + 4 * k) & 15;
        if id == 0 {
            break;
        }
        if (5..=7).contains(&id) {
            list.push(id as ObjectId);
        }
    }
    {
        let ob = w.objects.find_mut(4).unwrap();
        let ci = ob.contain.unwrap();
        if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[ci as usize].body.as_mut() {
            p.answers.garrisonable = cfg[13] & 1 != 0;
            p.answers.immune = cfg[13] >> 1 & 1 != 0;
            p.answers.contained = list;
        }
    }
    for k in 0..3usize {
        let id = 5 + k as ObjectId;
        let fl = cfg[14 + k];
        let o = w.objects.find_mut(id).unwrap();
        o.thing.friend_set_template(template_of(fl & 7));
        if fl >> 3 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
    }
    w.cuts.projectile_collides_with = (1..8).filter(|id| cfg[17] >> id & 1 != 0).collect();
    w.cuts.terrain_extremes = Some((cfg[18] & 1 != 0, f32::from_bits(cfg[19])));
}

fn effects(w: &mut World, events_before: usize, fx: &mut FxIds, t: &mut Tally) -> Vec<String> {
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
    for line in w.cuts.trace.take().unwrap() {
        if line.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::WeaponPositionProjectileForLaunch { projectile, launcher, wslot, barrel }) => log.push(format!("positionlaunch {projectile} {launcher} {wslot} {barrel}")),
                Some(CutCall::WeaponShouldProjectileCollideWith { template, launcher, projectile, other, victim }) => {
                    log.push(format!("shouldcollide {} {launcher} {projectile} {other} {victim}", WEAPONS[template]))
                }
                Some(CutCall::WeaponHandleProjectileDetonation { template, source, pos, extra_bonus_flags }) => {
                    t.detonations += 1;
                    log.push(format!("detonation {} {source} {:08x} {:08x} {:08x} {extra_bonus_flags}", WEAPONS[template], b(pos.x), b(pos.y), b(pos.z)))
                }
                Some(CutCall::PartitionEstimateTerrainExtremes { start: s, end: e }) => {
                    log.push(format!("extremes {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}", b(s.x), b(s.y), b(s.z), b(e.x), b(e.y), b(e.z)))
                }
                Some(CutCall::TerrainGetHighestLayerForDestination { x, y, z }) => log.push(format!("highlayer {:08x} {:08x} {:08x}", b(x), b(y), b(z))),
                Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", b(x), b(y))),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    t.destroys += 1;
                    log.push(format!("destroy {object}"))
                }
                Some(CutCall::Body(BodyCall::ScoreTheKill { killer, victim })) => log.push(format!("score {killer} {victim}")),
                Some(CutCall::Body(BodyCall::AcademyClearedGarrisonedBuilding { player })) => {
                    t.garrison_kills += 1;
                    log.push(format!("garrisoncleared {player}"))
                }
                _ => {}
            }
        } else if line.starts_with("evt ObjectCreated") || line.starts_with("evt ObjectDestroyed") {
        } else if line.starts_with("evt ") {
            let Some(e) = ev_iter.next() else { continue };
            match e {
                ObjectClientEvent::FxListDoFxObj { fx: name, .. } => log.push(format!("fx{}", fx.id(&Some(name)))),
                ObjectClientEvent::DrawableSetHidden { object_id, hidden } => log.push(format!("hidden {object_id} {}", i32::from(hidden))),
                _ => {}
            }
        } else if line.starts_with("damage ") {
            log.push(line);
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::DumbProjectileBehavior(d) = &data else { unreachable!() };
    for x in [d.first_height, d.second_height, d.first_percent_indent, d.second_percent_indent] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, d.max_lifespan);
    put_u32(&mut v, u32::from(d.tumble_randomly) | u32::from(d.orient_to_flight_path) << 1 | u32::from(d.detonate_calls_kill) << 2);
    put_i32(&mut v, d.garrison_hit_kill_count);
    for m in [&d.garrison_hit_kill_kind_of, &d.garrison_hit_kill_kind_of_not] {
        for k in 0..KINDOF_COUNT {
            if m.test(k) {
                put_u32(&mut v, k as u32);
            }
        }
        put_u32(&mut v, u32::MAX);
    }
    put_u32(&mut v, fx.id(&d.garrison_hit_kill_fx) as u32);
    put_u32(&mut v, d.flight_path_adjust_dist_per_frame.to_bits());

    let mut w = World::new();
    w.names = ctx.names;
    let mut store = WeaponStore::new();
    for (k, name) in WEAPONS.iter().enumerate() {
        let i = store.new_weapon_template(&mut w.names, name.as_bytes()).unwrap().unwrap();
        let tp = &mut store.templates_mut()[i];
        tp.weapon_speed = f32::from_bits(cfg[22 + 5 * k]);
        tp.min_weapon_speed = f32::from_bits(cfg[23 + 5 * k]);
        tp.is_scale_weapon_speed = cfg[24 + 5 * k] != 0;
        tp.attack_range = f32::from_bits(cfg[25 + 5 * k]);
        tp.minimum_attack_range = f32::from_bits(cfg[26 + 5 * k]);
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store: store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.stub_weapon_calls = true;
    w.cuts.body.stub_object_calls = true;
    w.players.set_player_count(1);
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    w.cuts.team_players.insert(1, 0);
    let body = || {
        let mut b = NewModule::plain(b"R68Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        b
    };
    for k in 1..=7 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"DumbProjectileBehavior", b"ModuleTag_Shell", &data).unwrap().unwrap());
            if cfg[2] & 1 != 0 {
                let pd = ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()));
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &pd).unwrap().unwrap());
            }
        }
        if k == 1 || k >= 5 {
            modules.push(body());
        }
        if k == 4 {
            let mut c = NewModule::plain(b"R68Contain", b"ModuleTag_Contain");
            c.interfaces = MODULEINTERFACE_CONTAIN;
            c.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
            modules.push(c);
        }
        let team = (k == 1 && cfg[2] >> 2 & 1 != 0).then_some(1);
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let no_drawable = rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
    w.friend_bind_to_drawable(me, if cfg[2] >> 1 & 1 != 0 { 11 } else { no_drawable });
    for (k, flag) in [(2, cfg[6] & 1), (3, cfg[8] & 1)] {
        if flag == 0 {
            w.destroy_object(k);
            w.process_destroy_list();
        }
    }
    set_scene(&mut w, &cfg);
    let idx = w.find_module(me, b"DumbProjectileBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let phys = w.objects.find(me).and_then(|o| o.physics);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (3..=21).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 2 {
            set_scene(&mut w, &cfg);
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 | 2 => {}
            3..=5 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    w.load_post_process();
                }
                tally.borrow_mut().xfers += 1;
            }
            6 => {
                let victim = if o[1] & 1 != 0 && w.objects.find(3).is_some() { 3 } else { INVALID_ID };
                let source = if w.objects.find(2).is_some() { 2 } else { INVALID_ID };
                let call = ModuleCall::ProjectileLaunchAtObjectOrPosition {
                    victim,
                    destination: Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: f32::from_bits(o[4]) },
                    source,
                    wslot: (o[1] >> 1 & 3) as i32,
                    barrel: (o[1] >> 3 & 3) as i32,
                    template: (o[1] >> 5 & 1) as usize,
                    exhaust: None,
                };
                w.module_call(me, idx, call);
                tally.borrow_mut().launches += 1;
            }
            7 => {
                let other = if o[1] != 0 && w.objects.find(o[1] as ObjectId).is_some() { o[1] as ObjectId } else { INVALID_ID };
                res = u32::from(w.module_call(me, idx, ModuleCall::ProjectileHandleCollision { other }).as_bool());
            }
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let mut t = tally.borrow_mut();
        let log = effects(&mut w, events_before, &mut fx, &mut t);
        t.effects += log.len() as u64;
        drop(t);
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
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::DumbProjectileBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        for x in m.state_words() {
            put_u32(&mut v, x);
        }
        let p = ob.get_position();
        for x in [p.x, p.y, p.z] {
            put_u32(&mut v, x.to_bits());
        }
        for x in ob.thing.transform_matrix().to_array() {
            put_u32(&mut v, x.to_bits());
        }
        put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_NO_COLLISIONS)));
        // (an update module's body lives in its behaviour slot)
        let rates = phys.and_then(|pi| w.update_handle_of_module(ModuleHandle { object: me, index: pi })).and_then(|h| match ob.behaviors[h.index as usize].body.as_ref() {
            Some(UpdateBody::PhysicsBehavior(pb)) => Some(pb.state_words()),
            _ => None,
        });
        for k in 0..3 {
            put_u32(&mut v, rates.as_ref().map_or(0, |s| s[k]));
        }
    }
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
fn dumb_projectile_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R68 DumbProjectileBehavior",
        seed: 0x0068_D0B1,
        cases_var: "L9R68_CASES",
        ini_stem: "Dp",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r68_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R68: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} launches, {} detonations, {} garrison kills, \
         {} destroys, {} xfers, {} effect lines",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.launches, t.detonations, t.garrison_kills, t.destroys, t.xfers, t.effects
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.launches > 0 && t.detonations > 0 && t.garrison_kills > 0, "the generator must reach every effect");
}
