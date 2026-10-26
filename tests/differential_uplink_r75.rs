#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ParticleUplinkCannonUpdate` (`logic::modules::particle_uplink_cannon_update`) against the unchanged C++, `p1w13-sonnet`, batch R75.
//!
//! # Method
//!
//! The C++ side is `ParticleUplinkCannonUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_uplink_r75.inc`,
//! `ref_l9r75_run`; its header less the forward declaration `enum ParticleSystemID;`, which the module build has as a typedef), on the
//! module build's stand-ins: the cannon (a team; maybe a drawable; maybe a REAL special power module whose readiness and ready frame are the
//! test's; sold, under construction, effectively dead, disabled four ways; its shroud status for the local player; a position and an
//! orientation), the art's bones (six outer nodes, the connector and the fire bone, each turned about z), four other objects (effectively
//! dead, maybe a body, a position), the scan (the test's list, the filters applied for real), four waypoints, the thing factory (a laser
//! template whose drawables have a `LaserUpdate`, one without, a remnant), the lasers' calls (lines; their radius the test's), the particle
//! systems, FX lists, scorches, shroud reveals and audio (lines), the REAL special power store, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the special power and bodies probes; the bones `cuts.bone_transforms`;
//! the waypoints the terrain's own; the client drawables the world's ids). Every op an `update()`, `initiateIntentToDoSpecialPower` (by a
//! human, a script with or without a waypoint, or a computer), `setSpecialPowerOverridableDestination`, the queries, `onObjectCreated`, a
//! change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data; the disabled types processed; per op the throw status, the result, the effects in order, the xfer status, bytes (the scheduling
//! word masked) and CRC, the module's state (the statuses, the ids, the outer nodes' positions and orientations, the five positions, the
//! frames and counters, the flags and the sounds' handles) and the objects made with the last one's position.
//!
//! # Results (2026-10-26, `L9R75_CASES=9000`)
//!
//! 9,000 cases (998 data blocks threw, as the C++ did; 1,589 excluded as undefined upstream: 648 a next scorch or pulse frame that is no
//! `UnsignedInt` (a zero `TotalScorchMarks` or `TotalDamagePulses`), 544 the special power module never found, 329 a `NULL` target
//! position, 63 no controlling player when it scorches, 5 a waypoint with no next link), 186,642 ops and 392,523 outputs: **0 mismatches**.
//! In the compared cases: 75,030 updates, 28,565 xfers and 234,956 effect lines; 19,906 intents, 17,900 taken; 3,296 firings and 4,163 beams
//! born, 16,473 laser inits, 25,992 particle systems, 2,961 scorches, 2,911 damage pulses with 4,673 damage lines, and 1,650 remnants. The
//! first runs differed only in the test's own stand-ins: the C++ waypoints linked a node to itself, which `addWaypointLink` refuses, and the
//! module build's `PartitionFilterAlive` allowed everything (it now has upstream's `allow`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 10 of 11 caught; 2467 is an equivalent mutant. (`createGroundToOrbitLaser` is
//! only called right after `removeAllEffects`, which has destroyed the ground beam, so its own "destroy the old beam" branch, the one that
//! clears the other beam's id, never runs.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2460 | `killEverything` leaves the orbit-to-target beam | 11 |
//! | 2461 | a click does not remember the one before | 56,141 |
//! | 2462 | a disabled cannon does not stop firing | 7,768 |
//! | 2463 | the damage radius scaled by the scorch scalar | 892 |
//! | 2464 | a sabotaged ready cannon does not pack up | 3,591 |
//! | 2465 | the S-curve's angle not negated | 1,638 |
//! | 2466 | the damage always `DEATH_LASERED` | 740 |
//! | 2467 | the ground beam's id cleared (not the other's) when replaced | equivalent |
//! | 2468 | the laser base's flare not destroyed | 21,157 |
//! | 2469 | the fire bone tested by its own name | 9,908 |
//! | 2470 | version 3's fields not saved | 28,192 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::disabled_types::{DISABLED_COUNT, DISABLED_EMP, DISABLED_HACKED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED};
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModuleData;
use rts_lockstep_engine::logic::modules::spectre_gunship_deployment_update::COMMAND_FIRED_BY_SCRIPT;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r75.bin");
const XFER_FILE: &str = "l9_r75.bin";
const POWERS: &str = "SpecialPower SP_0\nEnd\nSpecialPower SP_1\nEnd\nSpecialPower SP_2\nEnd\nSpecialPower SP_3\nEnd\n";
/// The particle system templates the client's manager has (any other name, and `None`, is `NULL`).
const SYSTEMS: [&str; 3] = ["PS_A", "PS_B", "PS_C"];
/// The art's bones (the driver's `kR75Bones`).
const BONES: [&str; 8] = ["Node01", "Node02", "Node03", "Node04", "Node05", "Node06", "Connector", "FireBone"];
/// The words of the module's state.
const STATE_WORDS: usize = 3 + 16 + 16 + 4 + 48 + 192 + 15 + 10 + 6 + 4;

/// Bone `k`'s place and turn (the driver's `r75BonePose`).
fn bone_pose(k: usize) -> (Coord3D, f32) {
    let k = k as f32;
    (Coord3D { x: 30.0 + 9.0 * k, y: -20.0 + 6.0 * k, z: 12.0 + 0.5 * k }, 0.4 * k - 1.2)
}

/// Waypoint `k`'s place (the driver's).
fn waypoint_pos(k: usize) -> Coord3D {
    Coord3D { x: -50.0 + 40.0 * k as f32, y: 25.0 * (k & 1) as f32 - 10.0, z: 0.0 }
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ParticleUplinkCannonUpdate(Arc::new(ParticleUplinkCannonUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn template_id(store: &SpecialPowerStore, name: &[u8]) -> u32 {
    store.find_special_power_template(name).map_or(0, |t| t.id())
}

/// The id of `SP_(k-1)` in the store, 0 for 0 (or a power the store does not hold).
fn power_id(store: &SpecialPowerStore, k: u32) -> u32 {
    if k == 0 || k > 4 {
        return 0;
    }
    template_id(store, format!("SP_{}", k - 1).as_bytes())
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let sp = r.u32();
    let nums: Vec<u32> = (0..21).map(|_| r.u32()).collect();
    let names: Vec<String> = (0..21).map(|_| r.str()).collect();
    out.push(format!("data sp {sp} {nums:x?} {names:?}"));
    let mask = r.u32();
    let n = r.u32();
    let log: Vec<String> = (0..n).map(|_| r.str()).collect();
    let state: Vec<u32> = (0..STATE_WORDS).map(|_| r.u32()).collect();
    out.push(format!("constructed mask {mask:#x} log {log:?} state {state:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..STATE_WORDS).map(|_| r.u32()).collect();
        let made: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        out.push(format!("op {i} state {state:x?} made {made:x?}"));
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
    field(g, 97, "SpecialPowerTemplate", &["SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_1"], true);
    field(g, 80, "BeginChargeTime", &["1000", "300", "0"], false);
    field(g, 80, "RaiseAntennaTime", &["1000", "500", "0"], false);
    field(g, 80, "ReadyDelayTime", &["500", "1000", "0"], false);
    field(g, 80, "WidthGrowTime", &["300", "100", "0"], false);
    field(g, 80, "BeamTravelTime", &["200", "33", "0"], false);
    field(g, 85, "TotalFiringTime", &["3000", "1000", "500", "0"], false);
    field(g, 50, "RevealRange", &["100", "0", "35.5"], false);
    field(g, 90, "OuterEffectBoneName", &["Node", "Node", "Node", "Nope"], true);
    // (strict: more than MAX_OUTER_NODES bones overruns the arrays, the destructor's loop too)
    field(g, 90, "OuterEffectNumBones", &["6", "4", "2", "0", "6"], true);
    field(g, 60, "OuterNodesLightFlareParticleSystem", &["PS_A", "PS_B", "None"], true);
    field(g, 60, "OuterNodesMediumFlareParticleSystem", &["PS_A", "PS_B", "PS_C"], true);
    field(g, 60, "OuterNodesIntenseFlareParticleSystem", &["PS_C", "PS_B", "None"], true);
    field(g, 80, "ConnectorBoneName", &["Connector", "Connector", "Nope"], true);
    field(g, 60, "ConnectorMediumLaserName", &["LaserA", "LaserA", "LaserB", "Nope"], true);
    field(g, 60, "ConnectorIntenseLaserName", &["LaserA", "LaserB"], true);
    field(g, 60, "ConnectorMediumFlare", &["PS_A", "PS_C", "None"], true);
    field(g, 60, "ConnectorIntenseFlare", &["PS_B", "PS_A"], true);
    field(g, 80, "FireBoneName", &["FireBone", "FireBone", "Connector", "Nope"], true);
    field(g, 50, "LaserBaseLightFlareParticleSystemName", &["PS_A", "None"], true);
    field(g, 50, "LaserBaseMediumFlareParticleSystemName", &["PS_B", "PS_C"], true);
    field(g, 50, "LaserBaseIntenseFlareParticleSystemName", &["PS_C", "PS_A"], true);
    field(g, 85, "ParticleBeamLaserName", &["LaserA", "LaserA", "LaserA", "LaserB", "Nope"], true);
    field(g, 70, "SwathOfDeathDistance", &["200", "500", "0", "-75.5"], false);
    field(g, 70, "SwathOfDeathAmplitude", &["100", "50", "0", "12.5"], false);
    field(g, 90, "TotalScorchMarks", &["10", "5", "1", "20", "10", "5", "3", "0"], false);
    field(g, 50, "ScorchMarkScalar", &["1.5", "0.5", "1"], false);
    field(g, 60, "BeamLaunchFX", &["FX_A", "FX_B", "None"], true);
    field(g, 50, "DelayBetweenLaunchFX", &["100", "33", "0"], false);
    field(g, 60, "GroundHitFX", &["FX_B", "FX_A", "None"], true);
    field(g, 70, "DamagePerSecond", &["100", "250.5", "0"], false);
    field(g, 90, "TotalDamagePulses", &["10", "3", "1", "20", "10", "5", "3", "0"], false);
    field(g, 40, "DamageType", &["LASER", "EXPLOSION", "FLAME"], true);
    field(g, 40, "DeathType", &["LASERED", "BURNED", "NORMAL"], true);
    field(g, 50, "DamageRadiusScalar", &["0.5", "2", "1"], false);
    field(g, 60, "PoweringUpSoundLoop", &["PowerUp", "PowerUp2"], true);
    field(g, 50, "UnpackToIdleSoundLoop", &["Unpack"], true);
    field(g, 50, "FiringToPackSoundLoop", &["FireToPack"], true);
    field(g, 60, "GroundAnnihilationSoundLoop", &["Annihilate"], true);
    field(g, 70, "DamagePulseRemnantObjectName", &["Remnant", "Remnant", "Remnant", "Nope"], true);
    field(g, 70, "ManualDrivingSpeed", &["30", "90.5", "0"], false);
    field(g, 70, "ManualFastDrivingSpeed", &["60", "300", "0"], false);
    field(g, 50, "DoubleClickToFastDriveDelay", &["500", "100", "0"], false);
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

fn gen_near(g: &mut Gen) -> u32 {
    f(*pick(g, &[&0.0f32, &5.0, &-12.5, &20.0, &-40.0, &75.0, &150.0, &-220.0]))
}

/// The cannon's flags (cfg[5]).
fn gen_flags(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let team = if g.below(25) == 0 { 0 } else { 1 + g.below(3) };
    let disabled = if g.below(8) == 0 { 1 << g.below(4) } else { 0 };
    let shroud = if g.below(6) == 0 { *pick(g, &[&0u32, &2, &3, &4]) } else { 1 };
    let local = if g.below(8) == 0 { 3 } else { g.below(3) };
    team | p(g, 3) << 2 | p(g, 3) << 3 | p(g, 3) << 4 | disabled << 5 | shroud << 9 | local << 12 | p(g, 40) << 14
}

/// Objects 2-5 (cfg[11]): mostly alive and with a body.
fn gen_others(g: &mut Gen) -> u32 {
    let mut v = 0;
    for k in 0..4 {
        let b = u32::from(g.below(10) == 0) | u32::from(g.below(8) != 0) << 1;
        v |= b << (8 * k);
    }
    v
}

fn gen_scan(g: &mut Gen) -> u32 {
    let n = g.below(5);
    let mut v = 0;
    for k in 0..n {
        v |= (2 + g.below(4)) << (4 * k);
    }
    v
}

fn gen_cfg_word(g: &mut Gen, a: u32, frame: u32) -> u32 {
    match a {
        5 => gen_flags(g),
        6 => frame.wrapping_add(*pick(g, &[&0u32, &10, &40, &100, &200])).wrapping_sub(*pick(g, &[&0u32, &0, &30])),
        7 | 8 => gen_near(g),
        9 => f(*pick(g, &[&0.0f32, &0.5, &1.5707964, &-2.5, &3.0])),
        10 => {
            // mostly every bone; now and then some missing
            if g.below(5) == 0 {
                g.u32() & 0xff
            } else {
                0xff
            }
        }
        11 => gen_others(g),
        12..=19 => gen_near(g),
        20 => gen_scan(g),
        21 => f(*pick(g, &[&10.0f32, &25.5, &0.0, &60.0])),
        22 => g.u32(),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // the data's power (SP_k: k + 1), which the cannon's power module mostly has
    let own = match data {
        Some(ModuleDataKind::ParticleUplinkCannonUpdate(d)) => match d.special_power_template.as_deref() {
            Some(b"SP_1") => 2,
            _ => 1,
        },
        _ => 1,
    };
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &99, &100]);
    cfg[1] = g.u32();
    cfg[2] = g.u32();
    let power = if g.below(60) == 0 { 0 } else if g.below(30) == 0 { 1 + g.below(4) } else { own };
    cfg[3] = u32::from(g.below(12) != 0) | power << 1;
    // the waypoints: mostly a chain 0 -> 1 -> 2 -> 3 -> 0, now and then other links or none
    cfg[4] = if g.below(4) == 0 { g.u32() & 0xffff } else { 0x1842 };
    for a in 5..=22u32 {
        cfg[a as usize] = gen_cfg_word(g, a, cfg[0]);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=39 => [0, 0, 0, 0, 0],
            40..=49 => {
                frame += 1 + g.below(30);
                [1, frame, 0, 0, 0]
            }
            50..=61 => {
                let a = *pick(g, &[&5u32, &5, &5, &6, &6, &7, &8, &10, &11, &12, &13, &20, &20, &21, &22]);
                [2, a, gen_cfg_word(g, a, frame), 0, 0]
            }
            62..=72 => {
                let k = if g.below(8) == 0 { *pick(g, &[&0u32, &1, &2, &3, &7]) } else { own };
                // a human's (a position), a script's (maybe a waypoint) or a computer's (a position or an object)
                let (script, node, obj, pos) = match g.below(3) {
                    0 => (0, 0, 0, u32::from(g.below(40) != 0)),
                    1 => {
                        let node = if g.below(4) == 0 { 0 } else { 1 + g.below(4) };
                        (1, node, 0, u32::from(node == 0 || g.below(2) == 0))
                    }
                    _ => {
                        let obj = if g.below(3) == 0 { 2 + g.below(4) } else { 0 };
                        (1, 0, obj, u32::from(obj == 0 && g.below(30) != 0))
                    }
                };
                [3, k << 8 | obj << 5 | node << 2 | script << 1 | pos, gen_near(g), gen_near(g), f(*pick(g, &[&0.0f32, &3.0]))]
            }
            73..=80 => [4, gen_near(g), gen_near(g), f(*pick(g, &[&0.0f32, &7.0])), 0],
            81..=86 => [5, 0, 0, 0, 0],
            87..=93 => [6, 0, 0, 0, 0],
            94..=95 => [7, 0, 0, 0, 0],
            96..=98 => [8, 0, 0, 0, 0],
            _ => [9, 0, 0, 0, 0],
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
    intents: u64,
    taken: u64,
    firings: u64,
    beams: u64,
    scorches: u64,
    pulses: u64,
    damage: u64,
    remnants: u64,
    systems: u64,
    lasers: u64,
    excluded: BTreeMap<String, u64>,
}

fn shroud(v: u32) -> ObjectShroudStatus {
    match v {
        0 => ObjectShroudStatus::Invalid,
        1 => ObjectShroudStatus::Clear,
        2 => ObjectShroudStatus::PartialClear,
        3 => ObjectShroudStatus::Fogged,
        4 => ObjectShroudStatus::Shrouded,
        _ => ObjectShroudStatus::InvalidButPreviousValid,
    }
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

fn set_scene(w: &mut World, cfg: &[u32], drawable: rts_lockstep_engine::common::game_type::DrawableId, bodies: &[Option<u16>; 5]) {
    let fl = cfg[5];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = fl & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_SOLD, (fl >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (fl >> 3 & 1) as i32);
        set_flag(o, PRIVATE_EFFECTIVELY_DEAD, fl >> 4 & 1 != 0);
        o.disabled = Default::default();
        for (i, t) in [DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_HACKED].into_iter().enumerate() {
            if fl >> (5 + i) & 1 != 0 {
                o.disabled.set(t, 1);
            }
        }
        o.drawable = if cfg[3] & 1 != 0 { drawable } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        o.in_partition = true;
    }
    let local = fl >> 12 & 3;
    w.players.friend_set_local_index((local < 3).then_some(local as i32));
    w.cuts.shrouded_status.clear();
    if local < 3 {
        w.cuts.shrouded_status.insert((1, local as i32), shroud(fl >> 9 & 7));
    }
    if let Some(p) = probe_mut(w, 1, b"R75Power") {
        p.answers.yes = fl >> 14 & 1 != 0;
        p.answers.ready_frame = cfg[6];
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[7]), y: f32::from_bits(cfg[8]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[9]));
    w.cuts.bone_transforms.clear();
    for (k, name) in BONES.iter().enumerate() {
        if cfg[10] >> k & 1 != 0 {
            w.cuts.bone_transforms.insert((1, name.as_bytes().to_vec()), bone_pose(k));
        }
    }
    for k in 1..5u32 {
        let b = cfg[11] >> (8 * (k - 1)) & 255;
        let id = k as ObjectId + 1;
        {
            let o = w.objects.find_mut(id).unwrap();
            set_flag(o, PRIVATE_EFFECTIVELY_DEAD, b & 1 != 0);
            o.body = if b >> 1 & 1 != 0 { bodies[k as usize] } else { None };
        }
        let i = (12 + 2 * (k - 1)) as usize;
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[i]), y: f32::from_bits(cfg[i + 1]), z: 0.0 });
    }
    w.cuts.range_scan = (0..4).map(|k| cfg[20] >> (4 * k) & 15).take_while(|&id| (2..=5).contains(&id)).map(|id| id as ObjectId).collect();
    w.cuts.laser_radius = f32::from_bits(cfg[21]);
    for (k, h) in w.cuts.terrain_ground.iter_mut().enumerate() {
        *h = (cfg[22] >> (4 * k) & 15) as f32 * 0.5;
    }
}

fn pos3(p: &Coord3D) -> String {
    format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally, names: &mut BTreeMap<u32, Vec<u8>>, fx: &mut FxIds) -> Vec<String> {
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
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                    tally.systems += 1;
                    names.insert(id, template);
                }
                Some(CutCall::ParticleSystemSetPosition { id, position: p }) => {
                    let name = String::from_utf8_lossy(names.get(&id).map_or(&[][..], |n| &n[..])).to_string();
                    log.push(format!("psys {name} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::ParticleSystemSetLocalTransform { id, matrix }) => {
                    let mut s = format!("pstransform {id}");
                    for x in matrix {
                        s.push_str(&format!(" {:08x}", b(x)));
                    }
                    log.push(s)
                }
                Some(CutCall::ParticleSystemDestroyById { id }) => log.push(format!("psdestroyid {id}")),
                Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, dc }) => {
                    tally.pulses += 1;
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 1 {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::PartitionDoShroudReveal { x, y, range, mask }) => log.push(format!("reveal {:08x} {:08x} {:08x} {mask}", b(x), b(y), b(range))),
                Some(CutCall::PartitionUndoShroudReveal { x, y, range, mask }) => log.push(format!("unreveal {:08x} {:08x} {:08x} {mask}", b(x), b(y), b(range))),
                Some(CutCall::Contain(ContainCall::EffectNew { drawable })) => log.push(format!("effnew {drawable}")),
                Some(CutCall::Contain(ContainCall::EffectDestroy { drawable })) => log.push(format!("effdestroy {drawable}")),
                Some(CutCall::Body(BodyCall::ParticleFindTemplate { .. })) => {}
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
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name))),
                Some(ObjectClientEvent::AudioAddEventAtDrawable { name, drawable }) => {
                    if drawable != 0 {
                        log.push(format!("audio {} - -1 0 draw {drawable}", String::from_utf8_lossy(&name)))
                    } else {
                        log.push(format!("audio {} - -1 0", String::from_utf8_lossy(&name)))
                    }
                }
                Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::ClearAndSet { clear, set } }) if object_id <= 5 => {
                    log.push(format!("mc {object_id} clear {clear:?} set {set:?}"))
                }
                Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::Clear(v) }) if object_id <= 5 => {
                    let mut v = v.clone();
                    v.sort_unstable();
                    log.push(format!("mc {object_id} clear {v:?}"))
                }
                // (a new object's drawable starting its model condition: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { object_id, .. }) if object_id > 5 => {}
                Some(ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id > 5 => {}
                Some(ObjectClientEvent::GameClientAddScorch { position: p, radius, scorch_type }) => {
                    tally.scorches += 1;
                    log.push(format!("scorch {:08x} {:08x} {:08x} {:08x} {scorch_type}", b(p.x), b(p.y), b(p.z), b(radius)))
                }
                Some(ObjectClientEvent::FxListDoFxPos { fx: name, position: p }) => log.push(format!("fxpos {} {}", fx.id(&Some(name)), pos3(&p))),
                Some(ObjectClientEvent::LaserDrawInit { drawable, start, end, growth_frames }) => {
                    tally.lasers += 1;
                    log.push(format!("laserdraw {drawable} {} {} {growth_frames}", pos3(&start), pos3(&end)))
                }
                Some(ObjectClientEvent::LaserSetDecayFrames { drawable, frames }) => log.push(format!("laserdecay {drawable} {frames}")),
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            log.push(l.to_string());
        } else if t.starts_with("damage ") {
            tally.damage += 1;
            log.push(t);
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::ParticleUplinkCannonUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.special_power_template.as_deref().map_or(0, |n| template_id(&ctx.store, n)));
    for x in [
        d.begin_charge_frames,
        d.raise_antenna_frames,
        d.ready_delay_frames,
        d.width_grow_frames,
        d.beam_travel_frames,
        d.total_firing_frames,
        d.frames_between_launch_fx_refresh,
        d.outer_effect_num_bones,
        d.total_scorch_marks,
        d.total_damage_pulses,
        d.double_click_to_fast_drive_delay,
    ] {
        put_u32(&mut v, x);
    }
    for x in [
        d.swath_of_death_distance,
        d.swath_of_death_amplitude,
        d.scorch_mark_scalar,
        d.damage_per_second,
        d.damage_radius_scalar,
        d.reveal_range,
        d.manual_driving_speed,
        d.manual_fast_driving_speed,
    ] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, d.damage_type as u32);
    put_u32(&mut v, d.death_type as u32);
    for s in [
        &d.outer_effect_base_bone_name,
        &d.outer_nodes_flares[0],
        &d.outer_nodes_flares[1],
        &d.outer_nodes_flares[2],
        &d.connector_bone_name,
        &d.connector_lasers[0],
        &d.connector_lasers[1],
        &d.connector_flares[0],
        &d.connector_flares[1],
        &d.laser_base_flares[0],
        &d.laser_base_flares[1],
        &d.laser_base_flares[2],
        &d.fire_bone_name,
        &d.particle_beam_laser_name,
        &d.powerup_sound_name,
        &d.unpack_to_ready_sound_name,
        &d.firing_to_idle_sound_name,
        &d.annihilation_sound_name,
        &d.damage_pulse_remnant_object_name,
    ] {
        put_str(&mut v, s);
    }
    put_str(&mut v, d.ground_hit_fx.as_deref().unwrap_or(b""));
    put_str(&mut v, d.beam_launch_fx.as_deref().unwrap_or(b""));

    let store = ctx.store.clone();
    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    for name in ["LaserA", "LaserB", "Remnant"] {
        factory.new_template(name.as_bytes(), 0).expect("a template");
    }
    let laser_a = factory.find_template(b"LaserA").unwrap().data.get_template_id();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, thing_factory: factory, ..Default::default() });
    w.cuts.laser_templates.insert(laser_a);
    for s in SYSTEMS {
        w.cuts.body.particle_templates.insert(s.as_bytes().to_vec());
    }
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_power_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.probe_effect_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    // the waypoints, ids 11-14, linked as cfg[4] says (node k's links in order)
    for k in 0..4usize {
        w.terrain.friend_push_waypoint(11 + k as i32, format!("W{k}").as_bytes(), waypoint_pos(k));
    }
    for k in 0..4 {
        for j in 0..4 {
            if cfg[4] >> (4 * k + j) & 1 != 0 {
                w.terrain.friend_add_waypoint_link(11 + k, 11 + j);
            }
        }
    }
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R75Cannon".to_vec(), ..Default::default() }));
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, health: f32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        let mut p = ProbeModule::new(answers, health, Default::default());
        p.health = health;
        m.body = Some(UpdateBody::ModuleProbe(p));
        m
    };
    let mut modules = Vec::new();
    let power = cfg[3] >> 1 & 7;
    if power_id(&store, power) != 0 {
        modules.push(probe(b"R75Power", ProbeAnswers { power_template: power_id(&store, power), ..Default::default() }, MODULEINTERFACE_SPECIAL_POWER, 0.0));
    }
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ParticleUplinkCannonUpdate", b"ModuleTag_Cannon", &data).unwrap().unwrap());
    let me = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let drawable = w.objects.find(me).unwrap().drawable;
    let mut bodies = [None; 5];
    for k in 2..=5 {
        let modules = vec![probe(b"R75Body", ProbeAnswers { no_death: true, ..Default::default() }, MODULEINTERFACE_BODY, 100.0)];
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
        bodies[(k - 1) as usize] = w.find_module(id, b"R75Body");
    }
    let idx = w.find_module(me, b"ParticleUplinkCannonUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, drawable, &bodies);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::ParticleUplinkCannonUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::ParticleUplinkCannonUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is in its slot") };
        let mask = m.update_module().get_disabled_types_to_process();
        put_u32(&mut v, (0..DISABLED_COUNT).filter(|&k| mask.test(k)).fold(0, |a, k| a | 1 << k));
    }
    let mut names = BTreeMap::new();
    let mut fx = FxIds::default();
    // (the module was made before the scene placed the cannon: onObjectCreated now, where the C++'s is)
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    let log = effects(&mut w, &mut t, &mut names, &mut fx);
    put_u32(&mut v, log.len() as u32);
    for l in &log {
        put_str(&mut v, l.as_bytes());
    }
    for x in state(&w) {
        put_u32(&mut v, x);
    }
    let mut saved: Vec<u8> = Vec::new();
    let made = |w: &World| -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 5).collect();
        ids.sort_unstable();
        ids
    };
    for o in ops {
        if o[0] == 2 && (5..=22).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, drawable, &bodies);
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let status_before = state(&w)[0];
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let template = power_id(&store, o[1] >> 8);
                let target_position = (o[1] & 1 != 0).then(|| Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: f32::from_bits(o[4]) });
                let (obj, node) = (o[1] >> 5 & 7, o[1] >> 2 & 7);
                let target_object = if (2..=5).contains(&obj) { obj as ObjectId } else { 0 };
                let waypoint = if (1..=4).contains(&node) { 10 + node } else { 0 };
                let command_options = if o[1] >> 1 & 1 != 0 { COMMAND_FIRED_BY_SCRIPT } else { 0 };
                let call = ModuleCall::InitiateIntentToDoSpecialPower { template, target_object, target_position, waypoint, command_options };
                res = u32::from(w.module_call(me, idx, call).as_bool());
                t.intents += 1;
                t.taken += u64::from(res);
            }
            4 => {
                let loc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
                w.module_call(me, idx, ModuleCall::SetSpecialPowerOverridableDestination(loc));
            }
            5..=7 => {
                let r = run_xfer_cut(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE, 2048);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 5 {
                    saved = xbytes.clone();
                }
                t.xfers += 1;
            }
            8 => {
                let q = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
                let b = |r: ModuleReply| u32::from(r.as_bool());
                res = b(q(&mut w, ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive))
                    | b(q(&mut w, ModuleCall::SpecialPowerUpdateIsActive)) << 1
                    | b(q(&mut w, ModuleCall::IsPowerCurrentlyInUse)) << 2
                    | b(q(&mut w, ModuleCall::DoesSpecialPowerHaveOverridableDestination)) << 3;
                if let ModuleReply::Int(c) = q(&mut w, ModuleCall::SpecialPowerUpdateGetCommandOption) {
                    res |= (c as u32) << 4;
                }
            }
            9 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
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
        if st != 0 {
            res = 0;
        }
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t, &mut names, &mut fx);
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
        let s = state(&w);
        if s[0] == 6 && status_before != 6 {
            t.firings += 1;
        }
        if s[1] == 1 && log.iter().any(|l| l.starts_with("laserdraw")) {
            t.beams += 1;
        }
        for x in s {
            put_u32(&mut v, x);
        }
        let ids = made(&w);
        put_u32(&mut v, ids.len() as u32);
        let last = ids.last().map(|&id| w.objects.find(id).unwrap().get_position()).unwrap_or_default();
        for x in [last.x, last.y, last.z] {
            put_u32(&mut v, x.to_bits());
        }
    }
    t.remnants += made(&w).len() as u64;
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
fn particle_uplink_cannon_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R75 ParticleUplinkCannonUpdate",
        seed: 0x0075_C4A9,
        cases_var: "L9R75_CASES",
        ini_stem: "Pu",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r75_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R75: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} intents, {} \
         taken, {} firings, {} beams born, {} laser inits, {} particle systems, {} scorches, {} damage pulses, {} damage lines, {} remnants)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.intents, t.taken, t.firings, t.beams, t.lasers, t.systems, t.scorches, t.pulses,
        t.damage, t.remnants
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.taken > 0 && t.firings > 0 && t.scorches > 0 && t.pulses > 0 && t.damage > 0 && t.remnants > 0, "the generator must reach every effect");
}
