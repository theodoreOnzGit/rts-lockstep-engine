#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/CountermeasuresBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CountermeasuresBehavior` (`logic::modules::countermeasures_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R52.
//!
//! # Method
//!
//! The C++ side is `CountermeasuresBehavior.cpp` (`build.rs` makes its `if( it )`, the end iterator tested as the pointer the shipped
//! `std::vector`'s is, the test of the storage it stands for), compiled into `wrap_module.cpp`'s unit after batch R50's driver, whose REAL
//! `PhysicsBehavior` it uses (`differential/l9_countermeasures_r52.inc`, `ref_l9r52_run`), on the module build's stand-ins: the aircraft
//! (id 1: an angle, a position, maybe a physics behaviour, maybe a controlling player with or without a default team, the airborne-target
//! status, maybe effectively dead); two missiles (ids 2, 3), each maybe with a projectile update whose diversion call is a line; the thing
//! factory (`FLARE_A` makes objects with a physics behaviour, `FLARE_B` objects with none); `findObjectByID` over the objects the test has not
//! removed; the partition manager's plane distance (a line); `Thing`'s transform; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them; the REAL `UpdateModule`, `UpgradeMux`, `UpgradeCenter`, `INI`, `Xfer` and random values.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module, a real `PhysicsBehavior`, `ProbeModule` projectile updates),
//! a thing factory whose `FLARE_A` template has a `PhysicsBehavior`, the loaded upgrade center; every op an `update()` as the scheduler runs
//! it, a call of the module's interface, a change to the scene, or an xfer. A removed object is destroyed and the destroy list processed.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the module's 12 fields and the mux's; mostly well formed) and runs 1 to 60 ops.
//! Compared, bit for bit: the read's throw; the data; the constructor's wake; per op the throw status, the result, `isAlreadyUpgraded`, the
//! effects in order (the diversion calls, the distance look-ups, the mux's FX and removals), the wake calls, the xfer status, bytes and CRC, the
//! module's state (the counts and frames, the flare list) and every flare (its id, whether it is found, its position and angle, its physics'
//! velocity, acceleration, speed and motive expiry).
//!
//! # Results (2026-10-25, `L9R52_CASES=9000`; re-measured the same day after the zero-fill correction below)
//!
//! 9,000 cases (914 data blocks threw, as the C++ did; 130 excluded, undefined upstream: 63 a `NULL` upgrade template, 57 a launch with no
//! physics on the aircraft or on a `FLARE_B` flare, 9 no controlling player, 1 the iterator stepped before the first flare), 255,182 ops
//! and 566,056 outputs: **0 mismatches**. In the compared cases: 87,717 updates, 9,551 xfers, 14,146 missiles reported (2,300 decoyed), at
//! least 105 volleys (up to 8 flares in a case), 9 reloads, 58 diversions found, 3,494 effect lines.
//!
//! **CORRECTED 2026-10-25.** ~~404 excluded (280 an uninitialised member read: `m_reloadFrame` before the first countdown or reload, the
//! data's `m_countermeasureReactionFrames` when the INI does not give it), 244,366 ops and 542,506 outputs: 0 mismatches.~~ Both members are
//! the zero the allocator fills new memory with, in the shipped game and in the test shim (`newInstance`, `MSGNEW`), and are now held as
//! zero and compared (the driver makes its modules with `newInstance`, the data dump includes the reaction latency, the state the reload
//! frame).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2221 | a decoy sets no reaction frame | 1,357 |
//! | 2222 | the volley arc ignored | 534 |
//! | 2223 | a slow aircraft's flares not thrown back | 893 |
//! | 2224 | no volley at the next volley frame | 37 |
//! | 2225 | the reload is due at once | 6,368 |
//! | 2226 | the diverted count not saved | 4,534 |
//! | 2227 | the divert search steps back past a flare it found | 38 |
//! | 2228 | a reported missile not counted | 146,017 |
//! | 2229 | a flare gone does not lower the active count | 104 |
//!
//! (Re-measured after the correction; the first run's counts were 1,357, 372, 747, 37, 164, 4,210, 34, 138,732 and 104.) A first 2227 (the closest flare taken whatever its distance) survived: the search reads the same id again once it finds one, so the
//! distance test never decides; it was an equivalent mutant and was replaced.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::countermeasures_behavior::CountermeasuresBehaviorModuleData;
use rts_lockstep_engine::logic::modules::physics_behavior::{PhysicsBehavior, PhysicsBehaviorModuleData};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r52.bin");
const XFER_FILE: &str = "l9_r52.bin";
/// The aircraft's team, and the player's default team.
const OBJECT_TEAM: u32 = 100;
const DEFAULT_TEAM: u32 = 101;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::CountermeasuresBehavior(Arc::new(CountermeasuresBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, d: &CountermeasuresBehaviorModuleData, fx: &mut FxIds) {
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_str(v, &d.flare_template_name);
    put_str(v, &d.flare_bone_base_name);
    put_u32(v, d.evasion_rate.to_bits());
    put_u32(v, d.volley_size);
    put_u32(v, d.volley_arc_angle.to_bits());
    put_u32(v, d.volley_velocity_factor.to_bits());
    put_u32(v, d.frames_between_volleys);
    put_u32(v, d.number_of_volleys);
    put_u32(v, d.reload_frames);
    put_u32(v, d.missile_decoy_frames);
    put_u32(v, d.countermeasure_reaction_frames);
    put_u32(v, u32::from(d.must_reload_at_airfield));
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    r.mux(&mut out);
    let (flare, bone) = (r.str(), r.str());
    let words: Vec<u32> = (0..10).map(|_| r.u32()).collect();
    let (w, f) = (r.u32(), r.u32());
    out.push(format!("data {flare:?} {bone:?} {words:x?} constructor wakes {w} next {f}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res, up) = (r.u32() as i32, r.u32(), r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let xst = r.u32() as i32;
        let blen = r.u32();
        let bytes: Vec<u32> = (0..blen).map(|_| r.u32()).collect();
        let crc = r.u32();
        let mut state: Vec<u32> = (0..8).map(|_| r.u32()).collect();
        let nf = r.u32();
        state.push(nf);
        state.extend((0..nf).map(|_| r.u32()));
        let flares = r.u32();
        let mut fl = Vec::new();
        for _ in 0..flares {
            let mut w: Vec<u32> = (0..7).map(|_| r.u32()).collect();
            if w[6] != 0 {
                w.extend((0..8).map(|_| r.u32()));
            }
            fl.push(format!("{w:x?}"));
        }
        out.push(format!("op {i}: status {st} result {res:#x} upgraded {up} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} flares {fl:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = if !clean && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 85, "FlareTemplateName", &["FLARE_A", "FLARE_A", "FLARE_B", "FLARE_C"]);
    field(g, 30, "FlareBoneBaseName", &["FLARE", "Bone01"]);
    field(g, 95, "VolleySize", &["1", "2", "3", "2", "4", "0"]);
    field(g, 60, "VolleyArcAngle", &["0", "15", "45", "90"]);
    field(g, 50, "VolleyVelocityFactor", &["1", "0.5", "2.5", "-1"]);
    field(g, 70, "DelayBetweenVolleys", &["0", "33", "100", "66", "200"]);
    field(g, 95, "NumberOfVolleys", &["1", "2", "3", "2", "0"]);
    field(g, 60, "ReloadTime", &["0", "100", "200", "1000"]);
    field(g, 80, "EvasionRate", &["0%", "50%", "100%", "100%", "75%"]);
    field(g, 30, "MustReloadAtAirfield", &["Yes", "No"]);
    field(g, 70, "MissileDecoyDelay", &["0", "33", "200"]);
    field(g, 85, "ReactionLaunchLatency", &["0", "33", "66", "100"]);
    let mut mux_lines = Vec::new();
    gen_mux_lines(g, upgrades, clean, &mut mux_lines);
    lines.extend(mux_lines);
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

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 8];
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    cfg[0] = *pick(g, &[&0u32, &0, &5, &299]);
    cfg[1] = g.u32();
    cfg[2] = p(g, 92) | p(g, 92) << 1 | p(g, 85) << 2 | p(g, 5) << 3 | p(g, 80) << 4 | p(g, 50) << 5 | p(g, 85) << 6;
    cfg[3] = f(*pick(g, &[&0.0f32, &0.5, &1.570_796_4, &3.0, &-2.0]));
    cfg[4] = f(*pick(g, &[&0.0f32, &10.0, &-5.5]));
    cfg[5] = f(*pick(g, &[&0.0f32, &3.0]));
    cfg[6] = f(*pick(g, &[&0.0f32, &50.0, &120.0]));
    let n_ops = 1 + g.below(60) as usize;
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // most cases upgrade early, so that the module runs
    if g.below(10) < 8 {
        // (half the time every upgrade: any TriggeredBy list is met unless a conflict blocks it)
        let m = if g.below(2) == 0 { gen_mask(g, n_up, 1) } else { [if n_up >= 32 { u32::MAX } else { (1u32 << n_up) - 1 }, 0, 0, 0] };
        ops.push([6, m[0], m[1], m[2], m[3]]);
    }
    // (a reload first in most cases: the counts start full however the data multiplies out)
    if g.below(10) < 8 {
        ops.push([10, 0, 0, 0, 0]);
    }
    if g.below(10) < 6 {
        ops.push([13, f(*pick(g, &[&3.0f32, &-2.0, &0.5])), f(*pick(g, &[&1.0f32, &0.0])), 0, 0]);
        ops.push([14, 0, 0, 0, 0]);
    }
    let force = |g: &mut Gen| f(*pick(g, &[&0.0f32, &1.0, &-2.5, &4.0]));
    while ops.len() < n_ops {
        let o = match g.below(100) {
            // most ops step a frame and run the update, so that the volley and reaction frames are met
            0..=34 => {
                ops.push([2, 0, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            35..=44 => [0, 0, 0, 0, 0],
            45..=46 => [1, gen_frame(g), 0, 0, 0],
            47..=49 => [3, 0, 0, 0, 0],
            50..=52 => [4, 0, 0, 0, 0],
            53..=54 => [5, 0, 0, 0, 0],
            55..=59 => {
                let m = if g.below(2) == 0 { gen_mask(g, n_up, 1) } else { [if n_up >= 32 { u32::MAX } else { (1u32 << n_up) - 1 }, 0, 0, 0] };
                [6, m[0], m[1], m[2], m[3]]
            }
            60..=61 => {
                let m = gen_mask(g, n_up, 1);
                [7, m[0], m[1], m[2], m[3]]
            }
            62..=73 => {
                // a report, then (mostly) the frames up to the reaction latency, each updated
                ops.push([8, *pick(g, &[&2u32, &3, &2, &0]), 0, 0, 0]);
                if g.below(10) < 7 {
                    ops.push([0, 0, 0, 0, 0]);
                    for _ in 0..1 + g.below(4) {
                        ops.push([2, 0, 0, 0, 0]);
                        ops.push([0, 0, 0, 0, 0]);
                    }
                }
                [2, 0, 0, 0, 0]
            }
            74..=79 => [9, 0, 0, 0, 0],
            80..=81 => [10, 0, 0, 0, 0],
            82 => [11, 0, 0, 0, 0],
            83..=86 => [12, p(g, 92) << 1 | p(g, 80) << 2 | p(g, 4) << 3 | p(g, 85) << 6, 0, 0, 0],
            87..=90 => [13, force(g), force(g), force(g), 0],
            91..=92 => [14, 0, 0, 0, 0],
            93..=96 => [15, 4 + g.below(8), 0, 0, 0],
            _ => [16, f(*pick(g, &[&0.0f32, &0.7, &-1.2, &2.5])), 0, 0, 0],
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
    reports: u64,
    decoys: u64,
    volleys: u64,
    flares: u64,
    reloads: u64,
    diverts: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    let fl = cfg[2];
    if fl >> 1 & 1 != 0 {
        w.cuts.team_players.insert(OBJECT_TEAM, 0);
    } else {
        w.cuts.team_players.remove(&OBJECT_TEAM);
    }
    w.players.get_nth_mut(0).unwrap().default_team = (fl >> 6 & 1 != 0).then_some(DEFAULT_TEAM);
    let o = w.objects.find_mut(1).unwrap();
    o.status.set(OBJECT_STATUS_AIRBORNE_TARGET, (fl >> 2 & 1) as i32);
    o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    if fl >> 3 & 1 != 0 {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    }
}

fn flare_words(w: &World, id: ObjectId) -> Vec<u32> {
    let o = w.objects.find(id).unwrap();
    let p = o.get_position();
    let mut v = vec![id as u32, 1, p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), o.thing.orientation().to_bits()];
    let phys = o.behaviors.iter().find_map(|b| match b.body.as_ref() {
        Some(UpdateBody::PhysicsBehavior(m)) => Some(m.state_words()),
        _ => None,
    });
    match phys {
        Some(s) => {
            v.push(1);
            // the velocity, the acceleration, the speed and the motive expiry
            v.extend([s[9], s[10], s[11], s[3], s[4], s[5], s[22], s[19]]);
        }
        None => v.push(0),
    }
    v
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::CountermeasuresBehavior(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    factory.new_template(b"FLARE_A", 0).expect("a template");
    factory.new_template(b"FLARE_B", 0).expect("a template");
    factory.friend_find_template_mut(b"FLARE_A").unwrap().data.friend_behavior_module_info_mut().add_module_info(
        b"PhysicsBehavior",
        b"ModuleTag_FlarePhysics",
        ModuleInfoData::Logic(ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()))),
        PhysicsBehavior::get_interface_mask(),
        false,
        false,
    );
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    for team in [OBJECT_TEAM, DEFAULT_TEAM] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    w.players.set_player_count(1);
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let mut modules = Vec::new();
    if cfg[2] & 1 != 0 {
        modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()))).unwrap().unwrap());
    }
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"CountermeasuresBehavior", b"ModuleTag_Flares", &data).unwrap().unwrap());
    // made under construction, as batch M's test does: `initObject` runs the upgrade pass of a finished object, which the C++ driver does not
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let me = w.new_object(NewObject { template: template.clone(), status, team: Some(OBJECT_TEAM), modules }).unwrap();
    assert_eq!(me, 1);
    w.objects.find_mut(me).unwrap().status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
    for k in 0..2 {
        let mut modules = Vec::new();
        if cfg[2] >> (4 + k) & 1 != 0 {
            let mut pm = NewModule::plain(b"R52Projectile", b"ModuleTag_Projectile");
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { has: 1 << 4, ..Default::default() }, 0.0, Default::default())));
            modules.push(pm);
        }
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k + 2);
    }
    let idx = w.find_module(me, b"CountermeasuresBehavior").unwrap();
    let phys_idx = w.find_module(me, b"PhysicsBehavior");
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg);
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[4]), y: f32::from_bits(cfg[5]), z: f32::from_bits(cfg[6]) });
    w.set_orientation(me, f32::from_bits(cfg[3]));
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    put_u32(&mut v, 1);
    put_u32(&mut v, w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame());
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let mut saved: Vec<u8> = Vec::new();
    // the flares' last dump, kept when they are removed (the C++ object stays, not found)
    let mut gone: std::collections::BTreeMap<ObjectId, Vec<u32>> = Default::default();
    for o in ops {
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 2 {
            let fr = w.frame().wrapping_add(1);
            w.set_frame(fr);
        }
        if o[0] == 12 {
            cfg[2] = (cfg[2] & !0x4E) | (o[1] & 0x4E);
        }
        set_scene(&mut w, &cfg);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let mask = mask_from(&o[1..]);
        let fc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
        let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
        let available_before = w.objects.find(me).unwrap().behaviors[u.index as usize].body.as_ref().map(|b| match b {
            UpdateBody::CountermeasuresBehavior(m) => m.state_words()[0],
            _ => 0,
        });
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 | 2 | 12 => {}
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
            6 => res = u32::from(call(&mut w, ModuleCall::AttemptUpgrade(mask)).as_bool()),
            7 => res = u32::from(call(&mut w, ModuleCall::ResetUpgrade(mask)).as_bool()),
            8 => {
                let missile = if o[1] == 2 || o[1] == 3 { o[1] as ObjectId } else { INVALID_ID };
                call(&mut w, ModuleCall::ReportMissileForCountermeasures { missile });
                tally.borrow_mut().reports += 1;
            }
            9 => {
                res = call(&mut w, ModuleCall::CalculateCountermeasureToDivertTo { victim: me }).as_id() as u32;
                if res != 0 {
                    tally.borrow_mut().diverts += 1;
                }
            }
            10 => {
                call(&mut w, ModuleCall::ReloadCountermeasures);
            }
            11 => res = u32::from(call(&mut w, ModuleCall::CountermeasuresIsActive).as_bool()),
            13 => {
                if let Some(p) = phys_idx {
                    w.module_call(me, p, ModuleCall::PhysicsAddVelocityTo(fc));
                }
            }
            14 => {
                if let Some(p) = phys_idx {
                    let vel = w.module_call(me, p, ModuleCall::PhysicsGetVelocity).as_coord();
                    w.module_call(me, p, ModuleCall::PhysicsTransferVelocityFrom(vel));
                }
            }
            15 => {
                let id = o[1] as ObjectId;
                if id >= 4 && w.objects.find(id).is_some() {
                    let mut words = flare_words(&w, id);
                    words[1] = 0;
                    gone.insert(id, words);
                    w.destroy_object(id);
                    w.process_destroy_list();
                }
            }
            16 => w.set_orientation(me, fc.x),
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        put_u32(&mut v, u32::from(call(&mut w, ModuleCall::IsAlreadyUpgraded).as_bool()));
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let mut t = tally.borrow_mut();
        for line in w.cuts.trace.take().unwrap() {
            if line.starts_with("cut ") {
                if let Some(CutCall::PartitionGetDistanceSquaredCenter2D { object, other }) = cut_iter.next() {
                    log.push(format!("distsq {object} {other}"));
                }
            } else if line.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) = ev_iter.next() {
                    log.push(format!("fx{}", fx.id(&Some(name))));
                }
            } else if let Some(fr) = line.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = fr;
            } else if let Some(l) = line.strip_prefix("bt projdecoy ") {
                t.decoys += 1;
                log.push(format!("projdecoy {l}"));
            } else if let Some(n) = line.strip_prefix("remove ") {
                log.push(format!("remove {n}"));
            }
        }
        t.effects += log.len() as u64;
        drop(t);
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
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::CountermeasuresBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let s = m.state_words();
        for x in &s {
            put_u32(&mut v, *x);
        }
        if o[0] == 0 {
            if let Some(before) = available_before {
                let mut t = tally.borrow_mut();
                if s[0] < before || (before == 0 && s[0] > 0x8000_0000) {
                    t.volleys += 1;
                }
                if s[0] > before && before == 0 && s[0] < 0x8000_0000 {
                    t.reloads += 1;
                }
            }
        }
        let first_flare: ObjectId = 4;
        // (the objects made, removed ones included: the world's next id goes back down when the newest is removed)
        let n_flares = (w.next_object_id() - first_flare).max(gone.keys().next_back().map_or(0, |&g| g - first_flare + 1)).max(0);
        put_u32(&mut v, n_flares as u32);
        for id in first_flare..first_flare + n_flares {
            let words = match gone.get(&id) {
                Some(g) => g.clone(),
                None => flare_words(&w, id),
            };
            for x in words {
                put_u32(&mut v, x);
            }
        }
        let most = tally.borrow().flares.max(n_flares as u64);
        tally.borrow_mut().flares = most;
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
fn countermeasures_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R52 CountermeasuresBehavior",
        seed: 0x0052_F1A5,
        cases_var: "L9R52_CASES",
        ini_stem: "Cm",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r52_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R52: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines, {} missiles reported, \
         {} decoyed, {} volleys, {} reloads, {} diversions found, at most {} flares",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.reports, t.decoys, t.volleys, t.reloads, t.diverts, t.flares
    );
    println!("excluded by: {:?}", t.excluded);
}
