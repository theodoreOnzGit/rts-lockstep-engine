#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `NeutronMissileUpdate` (`logic::modules::neutron_missile_update`) against the unchanged C++, `p1w13-sonnet`, batch R67.
//!
//! # Method
//!
//! The C++ side is `NeutronMissileUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_neutron_missile_r67.inc`,
//! `ref_l9r67_run`; the launcher's physics is the REAL `PhysicsBehavior` under batch R50's names), on the module build's stand-ins: the
//! missile (a position, an orientation, a bounding sphere, maybe a drawable, maybe a controlling player, an experience tracker, a body whose
//! damage is a line), the launcher (maybe; maybe a drawable with the test's launch bone, maybe a physics behaviour), the victim (maybe), a
//! fourth object; the REAL partition distance procedures, the decal stand-ins of batch O, the particle system manager, `destroyObject` (a
//! line), the REAL `Object::kill`, `Object::onCollide`, `UpdateModule`, `INI`, `Xfer`, logic random values and WW maths.
//!
//! The Rust side is the real thing: a `World` with the four objects (the launcher's physics the real `PhysicsBehavior`, its launch bone
//! `cuts.drawable_launch_offset`, the missile's body a probe), the logic random values; the module from `modules::new_module`; the destroy
//! a record (`cuts.body.stub_object_calls`). Every op an `update()`, a launch (`projectileLaunchAtObjectOrPosition`: at the victim or a
//! position, a slot and barrel, an exhaust or none), a collision, `onDie`, a push of the launcher's physics, a move or turn of the missile,
//! the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw;
//! the data; per op the throw status, the result, the effects in order (shown, hidden, the instance matrix, the experience sink, the FX,
//! the exhaust, the decal's create, update and clear, the damage of the kill, the destroy), the xfer status, bytes (the scheduling word
//! masked) and CRC, the module's whole state, the missile's position and transform, and its no-collisions status.
//!
//! Two lines are not compared, by design: the `dclear` of a load (as batch R20: the Rust load has no world to tell the client), and the
//! module build's record of `FXList::doFXObj(NULL, ..)`, which upstream's inline wrapper (`FXList.h` 151-157) skips.
//!
//! # Results (2026-10-26, `L9R67_CASES=9000`)
//!
//! 9,000 cases (673 data blocks threw, as the C++ did; 307 excluded, each a launch or a special-speed climb of a missile with no drawable,
//! which upstream reads through `NULL`), 246,917 ops and 509,874 outputs: **0 mismatches**, the missile's transform included bit for bit
//! (WW maths on both sides). In the compared cases: 10,970 launches, 93,304 updates, 3,305 detonations, 6,859 destroys of a missile whose
//! launcher is gone and 16,717 xfers. The first runs differed only in the test's own set-up (the stand-in object keeps its orientation only
//! with batch R28's flag; the two lines above).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2371 | the launcher's velocity not inherited | 9,751 |
//! | 2372 | `TargetFromDirectlyAbove` never aims above the target | 58,434 |
//! | 2373 | no limit on the turn rate | 73,214 |
//! | 2374 | no jitter during the special climb | 1,275 |
//! | 2375 | the missile detonates on its own launcher | 32,330 |
//! | 2376 | the no-turn distance never spent | 58,817 |
//! | 2377 | the no-turn distance not saved | 10,790 |

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
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::neutron_missile_update::NeutronMissileUpdateModuleData;
use rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehaviorModuleData;
use rts_lockstep_engine::logic::weapon_host::LaunchOffset;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{GridDecalOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r67.bin");
const XFER_FILE: &str = "l9_r67.bin";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::NeutronMissileUpdate(Arc::new(NeutronMissileUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let data: Vec<u32> = (0..11).map(|_| r.u32()).collect();
    let decal = r.str();
    let rest: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    out.push(format!("data {data:x?} decal {decal:?} {rest:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..24).map(|_| r.u32()).collect();
        let me: Vec<u32> = (0..16).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        out.push(format!("op {i} state {state:x?} missile {me:x?}"));
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
    field(g, 60, "DistanceToTravelBeforeTurning", &["0", "20", "100"]);
    field(g, 70, "MaxTurnRate", &["30", "180", "720", "5"]);
    field(g, 50, "ForwardDamping", &["0", "0.05", "0.2"]);
    field(g, 70, "RelativeSpeed", &["0.5", "1", "3"]);
    field(g, 50, "TargetFromDirectlyAbove", &["0", "50", "200"]);
    field(g, 40, "LaunchFX", &["FX_Launch", "None"]);
    field(g, 40, "IgnitionFX", &["FX_Ignite", "FX_Launch", "None"]);
    field(g, 40, "SpecialSpeedTime", &["0", "100", "500"]);
    field(g, 40, "SpecialSpeedHeight", &["0", "80", "300"]);
    field(g, 40, "SpecialAccelFactor", &["1", "0.001", "2.5"]);
    field(g, 40, "SpecialJitterDistance", &["0", "3", "10"]);
    field(g, 40, "DeliveryDecalRadius", &["0", "50", "120.5"]);
    if g.below(100) < 40 {
        lines.push(format!("DeliveryDecal\n Texture = {}\n Style = SHADOW_ALPHA_DECAL\n OpacityMin = 25%\nEnd", pick(g, &["Ring", "Nuke"])));
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

fn gen_coord(g: &mut Gen, z: bool) -> [u32; 3] {
    let c = |g: &mut Gen| f(*pick(g, &[&0.0f32, &10.0, &-35.0, &120.0, &400.0]));
    [c(g), c(g), if z { f(*pick(g, &[&0.0f32, &5.0, &30.0, &150.0])) } else { 0 }]
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &500]);
    cfg[1] = g.u32();
    cfg[2] = p(g, 95) | p(g, 85) << 1;
    cfg[3..6].copy_from_slice(&gen_coord(g, true));
    cfg[6] = f(*pick(g, &[&0.0f32, &0.7, &-2.0, &3.0]));
    cfg[7] = f(*pick(g, &[&1.0f32, &5.0, &20.0]));
    cfg[8] = p(g, 92) | p(g, 85) << 1 | p(g, 80) << 2 | p(g, 50) << 3;
    cfg[9..12].copy_from_slice(&gen_coord(g, true));
    cfg[12] = f(*pick(g, &[&0.0f32, &1.0, &-0.5]));
    cfg[13..16].copy_from_slice(&gen_coord(g, true));
    cfg[16] = f(*pick(g, &[&0.0f32, &0.4, &-1.3]));
    cfg[17] = p(g, 70);
    cfg[18..21].copy_from_slice(&gen_coord(g, false));
    cfg[21..24].copy_from_slice(&gen_coord(g, true));
    let n_ops = 1 + g.below(60) as usize;
    let mut ops: Vec<[u32; 5]> = Vec::new();
    let launch = |g: &mut Gen| {
        let a = p(g, 60) | g.below(3) << 1 | g.below(3) << 3 | p(g, 50) << 5;
        let c = gen_coord(g, true);
        [6, a, c[0], c[1], c[2]]
    };
    // most cases launch early, so that the missile flies
    if g.below(10) < 8 {
        ops.push(launch(g));
    }
    let force = |g: &mut Gen| f(*pick(g, &[&0.0f32, &1.0, &-2.5, &4.0]));
    while ops.len() < n_ops {
        let o = match g.below(100) {
            // mostly a frame and an update
            0..=54 => {
                ops.push([1, 0xFFFF_FFFF, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            55..=59 => [0, 0, 0, 0, 0],
            60..=62 => launch(g),
            63..=67 => [7, *pick(g, &[&0u32, &2, &3, &4, &2]), 0, 0, 0],
            68..=69 => [8, 0, 0, 0, 0],
            70..=73 => [9, force(g), force(g), force(g), 0],
            74..=77 => {
                let c = gen_coord(g, true);
                [10, c[0], c[1], c[2], 0]
            }
            78..=80 => [11, f(*pick(g, &[&0.0f32, &0.7, &-2.0])), 0, 0, 0],
            81..=84 => [3, 0, 0, 0, 0],
            85..=88 => [4, 0, 0, 0, 0],
            89..=91 => [5, 0, 0, 0, 0],
            _ => [1, gen_frame(g), 0, 0, 0],
        };
        ops.push(o);
    }
    // ([1, 0xFFFFFFFF] is "the next frame": the frame op takes an absolute frame, so it is replaced here by the running count)
    let mut frame = cfg[0];
    for o in &mut ops {
        if o[0] == 1 {
            frame = if o[1] == 0xFFFF_FFFF { frame.wrapping_add(1) } else { o[1] };
            o[1] = frame;
        }
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
    destroys: u64,
    excluded: std::collections::BTreeMap<String, u64>,
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
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    t.destroys += 1;
                    log.push(format!("destroy {object}"))
                }
                Some(CutCall::Contain(ContainCall::ExperienceSetSink { object, sink })) => log.push(format!("xpsink {object} {sink}")),
                _ => {}
            }
        } else if line.starts_with("evt ObjectCreated") || line.starts_with("evt ObjectDestroyed") {
        } else if line.starts_with("evt ") {
            let Some(e) = ev_iter.next() else { continue };
            match e {
                ObjectClientEvent::FxListDoFxObj { fx: name, .. } => log.push(format!("fx{}", fx.id(&Some(name)))),
                ObjectClientEvent::DrawableSetHidden { object_id, hidden } => log.push(format!("hidden {object_id} {}", i32::from(hidden))),
                ObjectClientEvent::DrawableSetInstanceMatrix { object_id, matrix } => match matrix {
                    None => log.push(format!("instance {object_id} null")),
                    Some(m) => {
                        let s: String = m.to_array().iter().map(|x| format!(" {:08x}", x.to_bits())).collect();
                        log.push(format!("instance {object_id}{s}"))
                    }
                },
                ObjectClientEvent::ParticleSystemCreateAttached { template, object_id } => log.push(format!("psattach {} {object_id}", String::from_utf8_lossy(&template))),
                ObjectClientEvent::GridDecal { index, op, .. } => log.push(match op {
                    GridDecalOp::Clear => format!("dclear {index}"),
                    GridDecalOp::Update => format!("dupdate {index}"),
                    GridDecalOp::Create { template: t, radius, position, player } => format!(
                        "dcreate {index} {} {} {:08x} {:08x} {} {} {} {:08x} {:08x},{:08x},{:08x} {}",
                        String::from_utf8_lossy(&t.name),
                        t.shadow_type,
                        b(t.min_opacity),
                        b(t.max_opacity),
                        t.opacity_throb_time,
                        t.color,
                        i32::from(t.only_visible_to_owning_player),
                        b(radius),
                        b(position.x),
                        b(position.y),
                        b(position.z),
                        player.unwrap_or(-1)
                    ),
                    GridDecalOp::SetPosition(p) => format!("dpos {index} {:08x} {:08x} {:08x}", b(p.x), b(p.y), b(p.z)),
                    GridDecalOp::SetOpacity(o) => format!("dopacity {index} {:08x}", b(o)),
                }),
                _ => {}
            }
        } else if line.starts_with("damage ") {
            t.detonations += 1;
            log.push(line);
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::NeutronMissileUpdate(d) = &data else { unreachable!() };
    for x in [d.initial_dist, d.max_turn_rate, d.forward_damping, d.relative_speed, d.target_from_directly_above, d.special_accel_factor] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, d.special_speed_time);
    put_u32(&mut v, d.special_speed_height.to_bits());
    put_u32(&mut v, d.special_jitter_distance.to_bits());
    put_u32(&mut v, fx.id(&d.launch_fx) as u32);
    put_u32(&mut v, fx.id(&d.ignition_fx) as u32);
    let t = &d.delivery_decal_template;
    put_str(&mut v, &t.name);
    put_u32(&mut v, t.shadow_type);
    put_u32(&mut v, t.min_opacity.to_bits());
    put_u32(&mut v, t.max_opacity.to_bits());
    put_u32(&mut v, t.opacity_throb_time);
    put_u32(&mut v, t.color as u32);
    put_u32(&mut v, u32::from(t.only_visible_to_owning_player));
    put_u32(&mut v, d.delivery_decal_radius.to_bits());

    let mut w = World::new();
    w.names = ctx.names;
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.players.set_player_count(1);
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    w.cuts.team_players.insert(1, 0);
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let c = |i: usize| Coord3D { x: f32::from_bits(cfg[i]), y: f32::from_bits(cfg[i + 1]), z: f32::from_bits(cfg[i + 2]) };
    for k in 1..=4 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"NeutronMissileUpdate", b"ModuleTag_Missile", &data).unwrap().unwrap());
            let mut b = NewModule::plain(b"R67Body", b"ModuleTag_Body");
            b.interfaces = MODULEINTERFACE_BODY;
            b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
            modules.push(b);
        }
        if k == 2 && cfg[8] >> 3 & 1 != 0 {
            let pd = ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()));
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &pd).unwrap().unwrap());
        }
        let team = (k == 1 && cfg[2] >> 1 & 1 != 0).then_some(1);
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    {
        let o = w.objects.find_mut(me).unwrap();
        let r = f32::from_bits(cfg[7]);
        o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, 1.0, r, r);
    }
    w.set_position(me, &c(3));
    w.set_orientation(me, f32::from_bits(cfg[6]));
    w.set_position(2, &c(9));
    w.set_orientation(2, f32::from_bits(cfg[12]));
    w.set_position(3, &Coord3D { x: f32::from_bits(cfg[18]), y: f32::from_bits(cfg[19]), z: 0.0 });
    w.set_position(4, &c(21));
    let no_drawable = rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
    w.friend_bind_to_drawable(me, if cfg[2] & 1 != 0 { 11 } else { no_drawable });
    w.friend_bind_to_drawable(2, if cfg[8] >> 1 & 1 != 0 { 12 } else { no_drawable });
    if cfg[8] >> 2 & 1 != 0 {
        let mut m = Matrix3D::IDENTITY;
        m.rotate_z(f32::from_bits(cfg[16]));
        m.set_translation(&Vector3::new(f32::from_bits(cfg[13]), f32::from_bits(cfg[14]), f32::from_bits(cfg[15])));
        for wslot in 0..3 {
            for barrel in 0..3 {
                w.cuts.drawable_launch_offset.insert((2, wslot, barrel, -1), LaunchOffset { attach_transform: m, turret_rot_pos: Coord3D::default(), turret_pitch_pos: Coord3D::default() });
            }
        }
    }
    for (k, flag) in [(2, cfg[8] & 1), (3, cfg[17] & 1)] {
        if flag == 0 {
            w.destroy_object(k);
            w.process_destroy_list();
        }
    }
    let idx = w.find_module(me, b"NeutronMissileUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let phys = w.objects.find(2).and_then(|o| o.physics);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let fc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 => {}
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
                    template: 0,
                    exhaust: (o[1] >> 5 & 1 != 0).then(|| b"EXH_A".to_vec()),
                };
                w.module_call(me, idx, call);
                tally.borrow_mut().launches += 1;
            }
            7 => {
                let other = if o[1] != 0 && w.objects.find(o[1] as ObjectId).is_some() { o[1] as ObjectId } else { INVALID_ID };
                res = u32::from(w.module_call(me, idx, ModuleCall::ProjectileHandleCollision { other }).as_bool());
            }
            8 => {
                w.module_call(me, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            9 => {
                if let Some(p) = phys {
                    w.module_call(2, p, ModuleCall::PhysicsAddVelocityTo(fc));
                }
            }
            10 => w.set_position(me, &fc),
            11 => w.set_orientation(me, fc.x),
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
        let Some(UpdateBody::NeutronMissileUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
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
fn neutron_missile_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R67 NeutronMissileUpdate",
        seed: 0x0067_0A11,
        cases_var: "L9R67_CASES",
        ini_stem: "Nm",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r67_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R67: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} launches, {} detonations, {} destroys, {} xfers, {} effect lines",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.launches, t.detonations, t.destroys, t.xfers, t.effects
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.launches > 0 && t.detonations > 0 && t.destroys > 0, "the generator must reach every effect");
}
