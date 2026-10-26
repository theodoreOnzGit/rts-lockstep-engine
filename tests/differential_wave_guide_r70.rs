#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WaveGuideUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `WaveGuideUpdate` (`logic::modules::wave_guide_update`) against the unchanged C++, `p1w13-sonnet`, batch R70.
//!
//! # Method
//!
//! The C++ side is `WaveGuideUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_wave_guide_r70.inc`,
//! `ref_l9r70_run`; batch R50's `destroyObject` rename) on the module build's stand-ins: the wave guide (maybe an AI whose two path orders
//! are lines, disabled or not, its transform Thing's: the pose of batch R28), five other objects (kinds, wet, maybe a drawable, maybe a body
//! whose damage is a line, a position), the waypoint path (`TerrainLogic.h`'s getters on the test's nodes), the bridges (the test's, by the
//! cell of the position), the ground (batch O's cells), the scan (the objects the test lists), the particle systems (the driver keeps their
//! positions; the test can end one), the water, the radar and the audio (lines), the thing factory (`WaterWaveBridge`, when the test has
//! one), the REAL `Object::topple` (no object has a `ToppleUpdate`, on either side: batch R28 verified it), `UpdateModule`, `INI`, `Xfer` and
//! logic random values.
//!
//! The Rust side is the real module on a `World` with the six objects (the AI and the bodies probes), the REAL `TerrainLogic` waypoints
//! (built by `friend_push_waypoint` and `friend_add_waypoint_link`), the REAL thing factory with a bare `WaterWaveBridge`, the terrain cut
//! points' ground, `cuts.bridge_at`, `cuts.range_scan` and `cuts.particle_systems`; the destroys and the disabling are records
//! (`cuts.body.stub_object_calls`).
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops, each an `update()`, a change to the scene
//! or the frame, the client ending a particle system, the wave guide moved (as its AI would), or an xfer. Compared, bit for bit: the read's
//! throw; the data; per op the throw status, the result, the effects in order (the disabling, the sounds, the path orders, the ground
//! queries, the particle systems and their positions and transforms, the water, the scans, the damage, the drawables, the bridges, the
//! destroy, the radar), the xfer status, bytes (the scheduling word masked) and CRC, the module's whole state (the points and effects to
//! the point count), the wave guide's position and orientation, the objects' wet bits and every new object's id, position and orientation.
//!
//! # Results (2026-10-26, `L9R70_CASES=9000`)
//!
//! 9,000 cases (710 data blocks threw, as the C++ did; none excluded), 252,962 ops and 269,542 outputs: **0 mismatches**. In the compared
//! cases: 88,284 updates, 3,889 starts along a path, 13,193 damage lines, 1,247 bridges replaced, 17,991 destroys of the wave (at its
//! destination, or for a path it cannot follow), 7,636 op rows with a shore splash and 25,537 xfers. Not reached: the conversions to `Int`
//! out of range (no case gives a size or spacing that large) and the toppling (no object has a `ToppleUpdate`). The first runs differed only
//! in the test's set-up: the C++ field table's particle system parser (now this unit's, by build.rs), the stand-in's `findParticleSystem`
//! (now the driver's, which knows each system's template), the save buffer (the module saves 2,337 bytes), a waypoint link to the node
//! itself (a map cannot make one: `addWaypointLink` refuses it), the destroy hook, the wave guide's drawable, the Rust side's bookkeeping of
//! a move and of a new object, and the pairing of the trace with the client events (a new object's `ObjectCreated` is no object event).
//!
//! # Planted bugs
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2400 | the destination never reached | 13,377 |
//! | 2401 | a branching path followed | 6,596 |
//! | 2402 | no third spray on every fifth point | 90,056 |
//! | 2403 | the front never bent | 41,121 |
//! | 2404 | the shore splashes every frame | 20,687 |
//! | 2405 | objects behind the front hit | 5,966 |
//! | 2406 | wet objects hit again | 20,646 |
//! | 2407 | no damage | 8,173 |
//! | 2408 | the new bridge not turned along the old | 1,890 |
//! | 2409 | the wave moves while disabled | 34,403 |
//! | 2410 | the point count, splash frame and destination not saved | 23,521 |
//!
//! 11 of 11 caught.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_WET;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::wave_guide_update::{WaveGuideUpdateModuleData, MAX_SHAPE_EFFECTS, MAX_WAVEGUIDE_SHAPE_POINTS};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r70.bin");
const XFER_FILE: &str = "l9_r70.bin";
/// The positions, by index (the driver's `g_r70Pal`).
const PAL: [f32; 8] = [-200.0, -60.0, -15.0, 0.0, 15.0, 60.0, 200.0, 500.0];
/// The bridges' directions, by index 1-7 (the driver's `g_r70Dir`).
const DIRS: [[f32; 2]; 8] = [[0.0, 0.0], [100.0, 0.0], [0.0, 100.0], [-50.0, 50.0], [70.0, -30.0], [1.0, 1.0], [-100.0, -5.0], [30.0, 90.0]];
/// The kinds, by bit (the driver's `g_r70Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_WAVEGUIDE, KINDOF_BRIDGE_TOWER, KINDOF_BRIDGE];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::WaveGuideUpdate(Arc::new(WaveGuideUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn pos_of(w: u32) -> Coord3D {
    Coord3D { x: PAL[(w & 7) as usize], y: PAL[(w >> 3 & 7) as usize], z: PAL[(w >> 6 & 7) as usize] }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..12).map(|_| r.u32()).collect();
    let names: Vec<String> = (0..3).map(|_| r.str()).collect();
    out.push(format!("data {data:x?} {names:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let head: Vec<u32> = (0..8).map(|_| r.u32()).collect();
        let count = (head[3] as i32).clamp(0, MAX_WAVEGUIDE_SHAPE_POINTS as i32) as usize;
        let arrays: Vec<u32> = (0..count * (6 + MAX_SHAPE_EFFECTS)).map(|_| r.u32()).collect();
        let tail: Vec<u32> = (0..5).map(|_| r.u32()).collect();
        let n_new = r.u32();
        let new: Vec<u32> = (0..n_new * 5).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {head:x?} {arrays:x?} me {tail:x?} new {new:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, pct: u32, name: &str, vals: &[&str]| {
        if g.below(100) < pct {
            lines.push(format!("{name} = {}", pick(g, vals)));
        }
    };
    field(g, 70, "WaveDelay", &["0", "33", "100", "1000"]);
    field(g, 90, "YSize", &["100", "200", "60", "0", "9", "500"]);
    field(g, 90, "LinearWaveSpacing", &["20", "30", "10.5", "50", "0.5"]);
    field(g, 60, "WaveBendMagnitude", &["0", "100", "250.5", "-80"]);
    field(g, 70, "WaterVelocity", &["30", "60", "0"]);
    field(g, 90, "PreferredHeight", &["10", "0", "-5", "25"]);
    field(g, 70, "ShorelineEffectDistance", &["20", "0", "35.5"]);
    field(g, 90, "DamageRadius", &["50", "20", "100"]);
    field(g, 80, "DamageAmount", &["100", "1000", "0.5"]);
    field(g, 70, "ToppleForce", &["2", "0.5", "10"]);
    field(g, 60, "RandomSplashSound", &["SplashA", "SplashB", "NoSound"]);
    field(g, 60, "RandomSplashSoundFrequency", &["0", "50", "90", "100"]);
    field(g, 60, "BridgeParticle", &["BridgeSpray", "None", "OtherSpray"]);
    field(g, 50, "BridgeParticleAngleFudge", &["0", "45", "90", "-30"]);
    field(g, 60, "LoopingSound", &["WaveLoop", "NoSound"]);
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "YSize = wide", "WaveDelay = soon"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_pos(g: &mut Gen) -> u32 {
    // (mostly low: under the water)
    let z = if g.below(3) == 0 { g.below(8) } else { *pick(g, &[&2u32, &3, &4]) };
    g.below(8) | g.below(8) << 3 | z << 6
}

fn gen_scan(g: &mut Gen) -> u32 {
    let n = g.below(6);
    let mut w = 0u32;
    for k in 0..n {
        w |= (1 + g.below(6)) << (4 * k);
    }
    w
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    match a {
        2 => p(g, 85) | p(g, 15) << 1 | gen_pos(g) << 8,
        3..=7 => {
            let kinds = if g.below(3) == 0 { g.below(8) } else { 0 };
            kinds | p(g, 15) << 3 | p(g, 60) << 4 | p(g, 80) << 5 | gen_pos(g) << 8
        }
        8 => gen_scan(g),
        9..=16 => (*pick(g, &[&0.0f32, &5.0, &-10.0, &20.0, &50.0, &-2.5])).to_bits(),
        17 => g.u32() & 0x00ff_ffff,
        18 => {
            let count = if g.below(10) == 0 { g.below(2) } else { 2 + g.below(3) };
            let topo = if g.below(5) == 0 { 1 + g.below(2) } else { 0 };
            count | p(g, 92) << 3 | topo << 4 | g.below(4) << 6
        }
        19 => (0..8).fold(0u32, |m, c| m | (if g.below(2) == 0 { 1 + g.below(7) } else { 0 }) << (3 * c)),
        20 => p(g, 85),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &100]);
    cfg[1] = g.u32();
    for (a, c) in cfg.iter_mut().enumerate().take(21).skip(2) {
        *c = gen_cfg_word(g, a as u32);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    while ops.len() < n_ops {
        let o = match g.below(100) {
            // (mostly the next frame and an update)
            0..=29 => {
                frame = frame.wrapping_add(1);
                [1, frame, 0, 0, 0]
            }
            30..=64 => [0, 0, 0, 0, 0],
            65..=69 => {
                let f = gen_frame(g);
                frame = f;
                [1, f, 0, 0, 0]
            }
            70..=81 => {
                let a = 2 + g.below(15);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            82..=84 => [6, g.below(30), 0, 0, 0],
            85..=89 => [7, gen_pos(g), 0, 0, 0],
            90..=93 => [3, 0, 0, 0, 0],
            94..=97 => [4, 0, 0, 0, 0],
            _ => [5, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    starts: u64,
    damages: u64,
    bridges: u64,
    ends: u64,
    shores: u64,
    excluded: BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32], ai: u16, bodies: &[Option<u16>; 6]) {
    let me: ObjectId = 1;
    {
        let o = w.objects.find_mut(me).unwrap();
        o.ai = if cfg[2] & 1 != 0 { Some(ai) } else { None };
        // (the driver's wave guide has no drawable)
        o.drawable = rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
        o.disabled.set(rts_lockstep_engine::common::disabled_types::DISABLED_HELD, i32::from(cfg[2] >> 1 & 1 != 0));
    }
    for k in 1..6usize {
        let id = k as ObjectId + 1;
        let f = cfg[2 + k];
        let mut values = ObjectTemplateValues { name: b"R70Thing".to_vec(), ..Default::default() };
        for (b, kind) in KINDS.iter().enumerate() {
            if f >> b & 1 != 0 {
                values.kind_of.push(*kind);
            }
        }
        let o = w.objects.find_mut(id).unwrap();
        o.thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
        o.status.set(OBJECT_STATUS_WET, i32::from(f >> 3 & 1 != 0));
        o.drawable = if f >> 4 & 1 != 0 { 1000 + id as rts_lockstep_engine::common::game_type::DrawableId } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        o.body = if f >> 5 & 1 != 0 { bodies[k] } else { None };
        w.set_position(id, &pos_of(f >> 8));
    }
    for c in 0..8 {
        w.cuts.terrain_ground[c] = f32::from_bits(cfg[9 + c]);
    }
    let mut scan = Vec::new();
    for k in 0..6 {
        let id = cfg[8] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 6 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally, names: &mut BTreeMap<u32, Vec<u8>>) -> Vec<String> {
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
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                    names.insert(id, template);
                }
                Some(CutCall::ParticleSystemSetPosition { id, position: p }) => {
                    let name = String::from_utf8_lossy(names.get(&id).map_or(&[][..], |n| &n[..])).to_string();
                    log.push(format!("psys {name} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::ParticleSystemSetLocalTransform { id, matrix }) => {
                    let m: Vec<String> = matrix.iter().map(|v| format!("{:08x}", b(*v))).collect();
                    log.push(format!("pstransform {id} {}", m.join(" ")))
                }
                Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, dc }) => {
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0 {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::TerrainFindBridgeAt { position: p }) => log.push(format!("bridgeat {:08x} {:08x} {:08x}", b(p.x), b(p.y), b(p.z))),
                Some(CutCall::TerrainDeleteBridge { bridge }) => {
                    tally.bridges += 1;
                    log.push(match bridge {
                        Some((f, t)) => format!("deletebridge {:08x} {:08x} {:08x} {:08x}", b(f.x), b(f.y), b(t.x), b(t.y)),
                        None => "deletebridge null".to_string(),
                    })
                }
                Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => log.push(format!("setdisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    tally.ends += 1;
                    log.push(format!("destroy {object}"))
                }
                Some(CutCall::Body(BodyCall::ParticleAttachToObject { id, object })) => log.push(format!("psattach {id} {object}")),
                // the World's own bookkeeping of a move and of a new object (the stand-ins keep a position only, and make a bare object)
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::PartitionRegisterObject { .. }
                    | CutCall::GameLogicUpdateObjectsChangedTriggerAreas
                    | CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction
                    | CutCall::WeaponSetUpdate { .. },
                ) => {}
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            match event_iter.next().flatten() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name))),
                Some(ObjectClientEvent::TerrainVisualAddWaterVelocity { x, y, velocity, preferred_height }) => {
                    log.push(format!("water {:08x} {:08x} {:08x} {:08x}", b(x), b(y), b(velocity), b(preferred_height)))
                }
                Some(ObjectClientEvent::RadarRefreshTerrain) => log.push("radarrefresh".to_string()),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(v), .. }) => log.push(format!("mc {}", v[0])),
                Some(ObjectClientEvent::DrawableSetShadowsEnabled { enabled, .. }) => log.push(format!("shadows {}", i32::from(enabled))),
                // (a new object's drawable starting its model condition: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::ClearAndSet { .. } }) if object_id > 6 => {}
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if t.starts_with("damage ") {
            tally.damages += 1;
            log.push(t);
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("aifollowpath ") {
                tally.starts += 1;
            }
            log.push(l.to_string());
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::WaveGuideUpdate(d) = &data else { unreachable!() };
    for x in [d.wave_delay, d.y_size, d.linear_wave_spacing, d.wave_bend_magnitude, d.water_velocity, d.preferred_height, d.shoreline_effect_distance, d.damage_radius, d.damage_amount, d.topple_force] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, d.random_splash_sound_frequency as u32);
    put_u32(&mut v, d.bridge_particle_angle_fudge.to_bits());
    put_str(&mut v, &d.random_splash_sound.event_name);
    put_str(&mut v, &d.looping_sound.event_name);
    put_str(&mut v, d.bridge_particle.as_deref().unwrap_or(b""));

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    if cfg[20] & 1 != 0 {
        factory.new_template(b"WaterWaveBridge", 0).expect("a template");
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.body.stub_object_calls = true;
    // the path (read once)
    let count = (cfg[18] & 7).min(4) as i32;
    let named = cfg[18] >> 3 & 1 != 0;
    let topo = cfg[18] >> 4 & 3;
    for i in 0..count {
        let loc = Coord3D { x: PAL[(cfg[17] >> (6 * i) & 7) as usize], y: PAL[(cfg[17] >> (6 * i + 3) & 7) as usize], z: 0.0 };
        let name: &[u8] = if i == 0 && named { b"WaveGuide1" } else { b"" };
        w.terrain.friend_push_waypoint(100 + i, name, loc);
    }
    let mut links = vec![0i32; count as usize];
    for i in 0..count - 1 {
        if topo == 2 && i == 0 {
            continue;
        }
        w.terrain.friend_add_waypoint_link(100 + i, 101 + i);
        links[i as usize] += 1;
    }
    if topo == 1 && count > 0 {
        // a second link, as the driver's: node k to the first node, or the first node to the last
        let k = (cfg[18] >> 6 & 3) as i32 % count;
        let to = if k != 0 { 0 } else { count - 1 };
        if links[k as usize] == 1 && to != k && to != k + 1 {
            w.terrain.friend_add_waypoint_link(100 + k, 100 + to);
        }
    }
    for c in 0..8 {
        let bi = (cfg[19] >> (3 * c) & 7) as usize;
        w.cuts.bridge_at[c] = (bi != 0).then(|| (Coord3D::default(), Coord3D { x: DIRS[bi][0], y: DIRS[bi][1], z: 0.0 }));
    }
    let probe = |name: &[u8], answers: ProbeAnswers| {
        let mut m = NewModule::plain(name, name);
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let template = || Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R70Thing".to_vec(), ..Default::default() }));
    for k in 1..=6 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"WaveGuideUpdate", b"ModuleTag_Wave", &data).unwrap().unwrap());
            let mut ai = probe(b"R70AIUpdate", ProbeAnswers::default());
            ai.is_ai = true;
            modules.push(ai);
        } else {
            let mut body = probe(b"NmBody", ProbeAnswers { no_death: true, ..Default::default() });
            body.interfaces = MODULEINTERFACE_BODY;
            modules.push(body);
        }
        let id = w.new_object(NewObject { template: template(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let ai = w.find_module(me, b"R70AIUpdate").unwrap();
    let mut bodies = [None; 6];
    for (k, b) in bodies.iter_mut().enumerate().skip(1) {
        *b = w.find_module(k as ObjectId + 1, b"NmBody");
    }
    let idx = w.find_module(me, b"WaveGuideUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let next_frame = |w: &World| w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame();
    w.set_position(me, &pos_of(cfg[2] >> 8));
    set_scene(&mut w, &cfg, ai, &bodies);
    let mut names: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::WaveGuideUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=16).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 7 {
            w.set_position(me, &pos_of(o[1]));
        }
        if o[0] == 6 {
            let id = if o[1] != 0 { o[1] } else { w.cuts.last_particle_system_id };
            w.cuts.particle_systems.remove(&id);
        }
        set_scene(&mut w, &cfg, ai, &bodies);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        w.events.drain();
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3..=5 => {
                let r = run_xfer_cut(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE, 4096);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    // the driver runs the module's loadPostProcess (the base's), not GameLogic's: the module goes back in the sleepy list at
                    // the next call frame the load read
                    let loaded = next_frame(&w);
                    w.load_post_process();
                    w.objects.find_mut(me).unwrap().behaviors[u.index as usize].update.friend_set_next_call_frame(loaded);
                }
                t.xfers += 1;
            }
            1 | 2 | 6 | 7 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().find(|(_, e)| !e.is_undefined());
        let st = match threw {
            Some((_, e)) if matches!(e.kind, rts_lockstep_engine::common::ini::IniErrorKind::BadArg) => 3,
            Some(_) => 2,
            None => 0,
        };
        put_i32(&mut v, st);
        // (a throw leaves the C++ result unassigned: 0)
        put_u32(&mut v, if st != 0 { 0 } else { res });
        let log = effects(&mut w, &mut t, &mut names);
        if log.iter().any(|l| l.starts_with("psys WaveSplashLeft01") || l.starts_with("psys WaveSplashRight01")) {
            t.shores += 1;
        }
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
        let s = state(&w);
        let count = (s[3] as i32).clamp(0, MAX_WAVEGUIDE_SHAPE_POINTS as i32) as usize;
        for x in &s[..8] {
            put_u32(&mut v, *x);
        }
        let points = 8;
        let transformed = points + 3 * MAX_WAVEGUIDE_SHAPE_POINTS;
        let effects_at = transformed + 3 * MAX_WAVEGUIDE_SHAPE_POINTS;
        for x in s[points..points + 3 * count].iter().chain(&s[transformed..transformed + 3 * count]).chain(&s[effects_at..effects_at + MAX_SHAPE_EFFECTS * count]) {
            put_u32(&mut v, *x);
        }
        let ob = w.objects.find(me).unwrap();
        let p = ob.get_position();
        for x in [p.x, p.y, p.z, ob.thing.orientation()] {
            put_u32(&mut v, x.to_bits());
        }
        let wet = (1..=6).fold(0u32, |m, k| m | u32::from(w.objects.find(k).unwrap().test_status(OBJECT_STATUS_WET)) << (k - 1));
        put_u32(&mut v, wet);
        let mut new: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 6).collect();
        new.sort_unstable();
        put_u32(&mut v, new.len() as u32);
        for id in new {
            let o = w.objects.find(id).unwrap();
            let p = o.get_position();
            put_u32(&mut v, id as u32);
            for x in [p.x, p.y, p.z, o.thing.orientation()] {
                put_u32(&mut v, x.to_bits());
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
fn wave_guide_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R70 WaveGuideUpdate",
        seed: 0x0070_3A7E,
        cases_var: "L9R70_CASES",
        ini_stem: "Wg",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r70_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R70: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} starts, {} damages, {} bridges, {} ends, \
         {} op rows with a shore splash, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.starts, t.damages, t.bridges, t.ends, t.shores, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.starts > 0 && t.damages > 0 && t.bridges > 0 && t.ends > 0 && t.shores > 0, "the generator must reach every effect");
}
