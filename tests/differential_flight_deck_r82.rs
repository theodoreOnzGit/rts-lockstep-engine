#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FlightDeckBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FlightDeckBehavior` (`logic::modules::flight_deck_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R82.
//!
//! # Method
//!
//! The C++ side is `FlightDeckBehavior.cpp` with the mechanical edits `build.rs` lists (the class's AI base a stand-in update module, MSVC
//! 6's vector iterators made portable), compiled into `wrap_module.cpp`'s unit (`differential/l9_flight_deck_r82.inc`, `ref_l9r82_run`), on
//! the module build's stand-ins: the carrier (under construction, sold or not; a team; maybe a production update whose count and unit id are
//! the test's and whose queued unit is a line; maybe a drawable, whose ramps are lines; the art's bones, a table of nineteen with transforms
//! turned about z, a missing one the object's position and transform as upstream; a position and an orientation), six planes (a team;
//! effectively dead, airborne, above the terrain or not; maybe an AI, idle or not, whose `JetAIUpdate` answers (taxiing to parking, the
//! pending command, reloading) are the test's and whose orders and purged command are lines; maybe a body, whose damage and healing are
//! lines; a `JetAIUpdate`, its parking offset and takeoff the test's), the payload's jets (`ThingFactory::newObject`, no modules), the
//! catapult's particle systems (their transform and position lines), `defect` and `setStatus` (lines), `setProducer`, the pathfinder (a
//! line), the partition's real distance, the REAL `Object::kill`, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the production, AIs, bodies and jets probes; the bones
//! `cuts.bone_transforms`; the Object-level calls records, `cuts.body.stub_object_calls`). Every op an `update()`, `onDie`, a call of the
//! parking place, exit or AI interface (`aiDoCommand`), `calcBestParkingAssignment`, the taxi or creation locations, a change to the scene
//! or the frame, an object destroyed, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw and
//! the data; the constructor's state; per op the throw status, the result, the `PPInfo` written, the effects in order, the xfer status,
//! bytes (the scheduling word masked) and CRC, the module's state (the spaces with their bones, the runways with their bones, start
//! orientation and transform, taxi and creation points and reservations, the healees, the frames, the orders, the ramps, the byte `xfer`
//! saves as `m_rampUp[MAX_RUNWAYS]`) and each object's producer, team, position and orientation. The wake calls are not compared: the
//! module never sleeps.
//!
//! The class's AI base is not in the comparison: upstream's is `AIUpdateInterface` (its data `AIUpdateModuleData`), which the class
//! replaces for `update` and `aiDoCommand`; the C++ side's base is an update module and the Rust module's `UpdateModule` (its save
//! `UpdateModule`'s members only, see the module's deviations), and the test's data names none of the AI's fields.
//!
//! The byte `xfer` saves twice as `m_rampUp[MAX_RUNWAYS]` is the class's padding, never initialised upstream: the driver zeroes it after
//! the constructor, as the Rust module starts it. A runway's start orientation and transform are set only by a creation bone (one
//! `RunwayInfo` is reused for every runway): where none was ever set both sides write a marker, and a use of them is undefined.
//!
//! # Results (2026-10-30, `L9R82_CASES=9000`)
//!
//! 9,000 cases (738 data blocks threw, as the C++ did; 3,151 excluded as undefined upstream: 1,274 a space's bone name past the list
//! (`buildInfo`'s walk), 1,202 an index past an array (a third runway, a front space past the spaces, a runway past the runways), 456 a
//! plane with no body killed or healed, 129 a runway's never-set start orientation or transform used, 71 a carrier with no player
//! making its jets, 19 a ramp with no drawable), 146,299 ops and 307,931 outputs: **0 mismatches**. In the compared cases: 32,257
//! updates, 7,319 xfers and 56,922 effect lines; 6,036 jets made; 9,474 space and 6,379 runway reservations, 1,274 exits, 800 heals,
//! 889 kills, 682 defections, 1,210 jets moved forward, 13 best assignments moved, 1,052 orders passed on, 81 launches, 3,579
//! catapult systems, 141 ramp changes, 507 replacements queued, 1,231 pending commands purged; 1,759 `reserve`s of a negative count
//! threw.
//!
//! The first runs differed in one place, upstream's: `xfer`'s `Int commandType` is never initialised, and a save assigns it to the
//! command as a load does, so upstream's save writes and keeps an undefined word. `build.rs` gives the local the command (see the
//! module's docs).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught. (3825 first survived, unreached: the generator now asks
//! the best assignment mostly of the jets behind the front row, with their position.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3821 | every space on runway 0 | 1,222 |
//! | 3822 | a dead plane's landing reservation not let go | 188 |
//! | 3823 | the approach half the runway's length beyond its end, not three quarters | 3,136 |
//! | 3824 | a plane waiting to take off gives up its space | 195 |
//! | 3825 | the position not reset when a plane blocks the best space | 45 |
//! | 3826 | an idle jet never moved forward | 819 |
//! | 3827 | an order to idle not passed on | 153 |
//! | 3828 | the next allowed production frame not saved | 3,010 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::{DrawableId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{
    OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_COUNT, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::flight_deck_behavior::FlightDeckBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply, PPInfo};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r82.bin");
const XFER_FILE: &str = "l9_r82.bin";

/// The art's bones (the driver's `kR82Bones`).
const BONES: [&str; 19] = [
    "Sp11", "Sp21", "Sp12", "Sp22", "Sp13", "TakeA1", "TakeB1", "TakeA2", "TakeB2", "LandA1", "LandB1", "LandA2", "LandB2", "Taxi1a", "Taxi1b", "Taxi2a",
    "Make1a", "Make1b", "Make2a",
];

/// The pending commands a plane's word picks (the driver's `kR82Pending`).
const PENDING: [i32; 8] = [-1, 5, 11, 12, 14, 23, 29, 0];
/// The commands `aiDoCommand` is given (the driver's `kCmds`).
const COMMANDS: [i32; 8] = [29, 14, 12, 11, 15, 5, 0, 23];

/// Bone `k`'s place and turn (the driver's `r82BonePose`).
fn bone_pose(k: usize) -> (Coord3D, f32) {
    let k = k as f32;
    (Coord3D { x: 40.0 + 10.0 * k, y: -25.0 + 7.0 * k, z: 0.5 * k + 0.5 }, 0.3 * k - 1.0)
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FlightDeckBehavior(Arc::new(FlightDeckBehaviorModuleData::friend_new_module_data(Some(ini))?)));
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
    let nums: Vec<u32> = (0..13).map(|_| r.u32()).collect();
    let payload = r.str();
    let mut runways = Vec::new();
    for _ in 0..2 {
        let mut names = Vec::new();
        for _ in 0..3 {
            let n = r.u32();
            names.push((0..n).map(|_| r.str()).collect::<Vec<_>>());
        }
        let strips: Vec<String> = (0..5).map(|_| r.str()).collect();
        runways.push(format!("{names:?} {strips:?}"));
    }
    out.push(format!("data {nums:x?} {payload:?} {runways:?}"));
    let state = |r: &mut Reader| -> String {
        let head: Vec<u32> = (0..22).map(|_| r.u32()).collect();
        let ns = r.u32();
        let spaces: Vec<u32> = (0..ns * 6).map(|_| r.u32()).collect();
        let nr = r.u32();
        let mut runways = Vec::new();
        for _ in 0..nr {
            let mut w: Vec<u32> = (0..13).map(|_| r.u32()).collect();
            let t = r.u32();
            w.push(t);
            if t != 0xffff_ffff || w[12] != 0xffff_ffff {
                w.extend((0..11).map(|_| r.u32()));
            }
            for _ in 0..2 {
                let n = r.u32();
                w.push(n);
                w.extend((0..n * 3).map(|_| r.u32()));
            }
            w.extend((0..2).map(|_| r.u32()));
            runways.push(w);
        }
        let nh = r.u32();
        let heals: Vec<u32> = (0..nh * 2).map(|_| r.u32()).collect();
        let no = r.u32();
        let mut objects = Vec::new();
        for _ in 0..no {
            let id = r.u32();
            let first = r.u32();
            let mut w = vec![id, first];
            if first != 0xffff_ffff {
                w.extend((0..5).map(|_| r.u32()));
            }
            objects.push(w);
        }
        format!("{head:x?} spaces {spaces:x?} runways {runways:x?} heals {heals:x?} objects {objects:x?}")
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
    field(g, 95, "NumRunways", &["1", "2", "2", "2", "2"], false);
    field(g, 95, "NumSpacesPerRunway", &["1", "2", "2", "2", "2", "3"], false);
    field(g, 95, "Runway1Spaces", &["Sp11 Sp12 Sp13", "Sp11 Sp12 Sp13", "Sp11 Sp12", "Sp11 Missing Sp13"], true);
    field(g, 90, "Runway2Spaces", &["Sp21 Sp22 Sp13", "Sp21 Sp22 Sp13", "Sp21 Sp22"], true);
    field(g, 90, "Runway1Takeoff", &["TakeA1 TakeB1", "TakeA1 TakeB1", "TakeA1"], true);
    field(g, 85, "Runway2Takeoff", &["TakeA2 TakeB2"], true);
    field(g, 90, "Runway1Landing", &["LandA1 LandB1", "LandA1 LandB1", "LandA1"], true);
    field(g, 85, "Runway2Landing", &["LandA2 LandB2"], true);
    field(g, 70, "Runway1Taxi", &["Taxi1a Taxi1b", "Taxi1a"], true);
    field(g, 60, "Runway2Taxi", &["Taxi2a"], true);
    field(g, 95, "Runway1Creation", &["Make1a Make1b", "Make1a Make1b", "Make1b"], true);
    field(g, 85, "Runway2Creation", &["Make2a", "Make2a Make1a"], true);
    field(g, 80, "Runway1CatapultSystem", &["Steam1", "Steam1", "None"], true);
    field(g, 70, "Runway2CatapultSystem", &["Steam2", "None"], true);
    field(g, 50, "ApproachHeight", &["0", "50", "12.5"], false);
    field(g, 40, "LandingDeckHeightOffset", &["0", "10", "4.5"], false);
    field(g, 50, "HealAmountPerSecond", &["0", "5", "12.5"], false);
    field(g, 80, "ParkingCleanupPeriod", &["0", "100", "500"], false);
    field(g, 60, "HumanFollowPeriod", &["0", "200", "1000"], false);
    field(g, 90, "PayloadTemplate", &["R82Jet", "NoSuchJet"], true);
    field(g, 60, "ReplacementDelay", &["0", "300", "2000"], false);
    field(g, 50, "DockAnimationDelay", &["0", "100"], false);
    field(g, 70, "LaunchWaveDelay", &["0", "200", "700"], false);
    field(g, 70, "LaunchRampDelay", &["0", "100", "400"], false);
    field(g, 70, "LowerRampDelay", &["0", "300", "900"], false);
    field(g, 70, "CatapultFireDelay", &["0", "100", "300"], false);
    if !clean && g.below(20) == 0 {
        lines.push("Bogus = 1".to_string());
    }
    if !clean && g.below(25) == 0 {
        lines.push(format!("{} = {}", pick(g, &["NumRunways", "NumSpacesPerRunway"]), pick(g, &["-1", "3"])));
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
    let team = if g.below(20) == 0 { 0 } else { 1 + g.below(3) };
    let offset = (*pick(g, &[&0i32, &0, &3, &-4])) as u32 & 255;
    p(g, 15)
        | team << 1
        | p(g, dead) << 3
        | p(g, 25) << 4
        | p(g, 15) << 5
        | p(g, 97) << 6
        | p(g, 92) << 7
        | p(g, 55) << 8
        | p(g, 20) << 9
        | p(g, 25) << 10
        | g.below(8) << 11
        | p(g, 40) << 16
        | offset << 24
}

fn gen_cfg_word(g: &mut Gen, a: u32, dead: u32) -> u32 {
    match a {
        2 => (if g.below(40) == 0 { 0 } else { 1 + g.below(3) }) | u32::from(g.below(30) == 0) << 2 | u32::from(g.below(30) == 0) << 3,
        3 => {
            // mostly every bone; now and then some missing
            if g.below(5) == 0 {
                g.u32() & 0x7ffff
            } else {
                0x7ffff
            }
        }
        4 | 5 => f(*pick(g, &[&0.0f32, &100.0, &-35.5])),
        6 => f(*pick(g, &[&0.0f32, &0.5, &-2.0])),
        7..=12 => gen_plane(g, dead),
        13 => (if g.below(3) == 0 { 1 } else { 0 }) | (g.below(40) << 8),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = u32::from(g.below(4) != 0) | u32::from(g.below(4) != 0) << 1;
    for a in 2..=13u32 {
        cfg[a as usize] = gen_cfg_word(g, a, 8);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut cur = cfg.clone();
    let mut frame = cfg[0];
    // mostly the first two planes and the first jets made, so that they meet on a runway
    let plane = |g: &mut Gen| match g.below(10) {
        0..=5 => 2 + g.below(2),
        6..=7 => 8 + g.below(4),
        _ => 2 + g.below(6),
    };
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=21 => [0, 0, 0, 0, 0],
            22..=29 => {
                frame += 1 + g.below(30);
                [1, frame, 0, 0, 0]
            }
            30..=37 => {
                // a third of them kill one of the first two planes (the rest of its word kept), a third change one of their words
                let a = if g.below(3) != 0 { 7 + g.below(2) } else { 2 + g.below(12) };
                let v = if a <= 8 && g.below(2) == 0 { cur[a as usize] | 8 } else { gen_cfg_word(g, a, 25) };
                cur[a as usize] = v;
                [2, a, v, 0, 0]
            }
            38 => [3, 0, 0, 0, 0],
            39..=40 => [4, 0, 0, 0, 0],
            41..=42 => [5, 0, 0, 0, 0],
            43 => [6, 0, 0, 0, 0],
            44 => [8, if g.below(2) == 0 { plane(g) } else { 8 + g.below(2) }, 0, 0, 0],
            45..=52 => [9, 2 + g.below(2), g.below(4), 0, 0],
            _ => {
                let which = *pick(
                    g,
                    &[
                        &0u32, &1, &2, &3, &4, &4, &4, &4, &4, &4, &5, &6, &6, &6, &6, &7, &8, &9, &10, &11, &12, &13, &13, &14, &15, &16, &17, &17, &18, &19,
                        &19, &19, &20, &20, &20, &20, &20, &20, &20, &20, &21, &22, &23,
                    ],
                );
                let (b, c, d) = match which {
                    4 => (plane(g), f(*pick(g, &[&0.0f32, &0.0, &2.5])), g.below(2)),
                    6 => (plane(g), u32::from(g.below(5) == 0), 0),
                    10 => (if g.below(8) == 0 { 2 } else { g.below(2) }, g.below(3), 0),
                    13 => (plane(g), u32::from(g.below(4) != 0), 0),
                    15 => (g.below(3), frame + g.below(50), 0),
                    17 => (plane(g), (*pick(g, &[&1i32, &1, &1, &0, &-1])) as u32, 0),
                    18 => (1, 0, 0),
                    // mostly a jet made behind the front row, its position asked (a plane in its way resets it)
                    19 => (if g.below(4) == 0 { plane(g) } else { 8 + g.below(6) }, u32::from(g.below(5) != 0), 0),
                    20 => (g.below(8), *pick(g, &[&0u32, &0, &0, &2, &1]), if g.below(3) == 0 { 0 } else { plane(g) }),
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
    jets_made: u64,
    spaces: u64,
    runways: u64,
    exits: u64,
    heals: u64,
    kills: u64,
    defects: u64,
    moved_forward: u64,
    best_assignments_moved: u64,
    orders: u64,
    launches: u64,
    catapults: u64,
    ramps: u64,
    productions: u64,
    purges: u64,
    teleports: u64,
    length_errors: u64,
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
    ai_slots: [Option<u16>; 6],
    body_slots: [Option<u16>; 6],
    drawable: bool,
}

fn set_scene(w: &mut World, cfg: &[u32], sc: &Scene) {
    let fl = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = fl & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (fl >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_SOLD, (fl >> 3 & 1) as i32);
        o.drawable = if sc.drawable { 1000 as DrawableId } else { INVALID_DRAWABLE_ID };
    }
    w.cuts.bone_transforms.clear();
    for (k, name) in BONES.iter().enumerate() {
        if cfg[3] >> k & 1 != 0 {
            w.cuts.bone_transforms.insert((1, name.as_bytes().to_vec()), bone_pose(k));
        }
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[4]), y: f32::from_bits(cfg[5]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[6]));
    if let Some(p) = probe_mut(w, 1, b"R82Production") {
        p.answers.production = Some(((cfg[13] & 255) as i32, (cfg[13] >> 8 & 255) as i32));
    }
    for k in 0..6usize {
        let id = k as ObjectId + 2;
        let b = cfg[7 + k];
        let Some(o) = w.objects.find_mut(id) else { continue };
        o.status.set(OBJECT_STATUS_AIRBORNE_TARGET, (b & 1) as i32);
        o.team = b >> 1 & 3;
        set_flag(o, PRIVATE_EFFECTIVELY_DEAD, b >> 3 & 1 != 0);
        o.body = if b >> 6 & 1 != 0 { sc.body_slots[k] } else { None };
        o.ai = if b >> 7 & 1 != 0 { sc.ai_slots[k] } else { None };
        o.producer_id = if b >> 16 & 1 != 0 { 1 } else { INVALID_ID };
        let mut p = o.get_position();
        p.z = if b >> 4 & 1 != 0 { 30.0 } else { 0.0 };
        w.set_position(id, &p);
        if let Some(j) = probe_mut(w, id, b"JetAIUpdate") {
            j.answers.jet = Some(((b >> 24 & 255) as u8 as i8 as f32, b >> 5 & 1 != 0));
        }
        if let Some(a) = probe_mut(w, id, b"R82Ai") {
            a.answers.idle = b >> 8 & 1 != 0;
            a.answers.jet_deck = Some((b >> 9 & 1 != 0, PENDING[(b >> 11 & 7) as usize], b >> 10 & 1 != 0));
        }
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally, jet_template: u16, names: &mut BTreeMap<u32, Vec<u8>>) -> Vec<String> {
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
                Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                    names.insert(id, template);
                }
                Some(CutCall::ParticleSystemSetPosition { id, position: p }) => {
                    tally.catapults += 1;
                    let name = String::from_utf8_lossy(names.get(&id).map_or(&[][..], |n| &n[..])).to_string();
                    log.push(format!("psys {name} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::ParticleSystemSetLocalTransform { id, matrix }) => {
                    let m: Vec<String> = matrix.iter().map(|v| format!("{:08x}", b(*v))).collect();
                    log.push(format!("pstransform {id} {}", m.join(" ")))
                }
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::GameLogicUpdateObjectsChangedTriggerAreas
                    | CutCall::WeaponSetUpdate { .. }
                    // (a jet made: the world's registrations, which the C++ side's newObject stand-in does not make)
                    | CutCall::PlayerBecomingTeamMember { .. }
                    | CutCall::PartitionRegisterObject { .. }
                    | CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction,
                ) => {}
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            match event_iter.next().flatten() {
                // (a jet made binds its drawable's conditions: the C++ side's jets have none)
                Some(ObjectClientEvent::DrawableModelCondition { object_id, .. } | ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id != 1 => {}
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. }) => {
                    tally.ramps += 1;
                    log.push(format!("clearset {} {}", clear[0], set[0]))
                }
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t);
        } else if t.starts_with("heal ") {
            tally.heals += 1;
            log.push(t);
        } else if let Some(l) = t.strip_prefix("bt ") {
            let word = l.split(' ').next().unwrap_or("");
            match word {
                "aiexitpath" => {
                    tally.moved_forward += 1;
                    log.push(l.to_string())
                }
                "aienter" | "aiforceattack" | "aiattackpos" | "aiguardpos" | "aiattackmove" => {
                    tally.orders += 1;
                    log.push(l.to_string())
                }
                "jetpurge" => {
                    tally.purges += 1;
                    log.push(l.to_string())
                }
                "produniqueid" => log.push(l.to_string()),
                "prodqueue" => {
                    tally.productions += 1;
                    let mut parts = l.split(' ').skip(1);
                    let template: i32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(-1);
                    let id = parts.next().unwrap_or("");
                    log.push(format!("prodqueue {} {id}", i32::from(template == i32::from(jet_template))))
                }
                _ => {}
            }
        }
    }
    log
}

fn module_of(w: &World, me: ObjectId, slot: usize) -> &rts_lockstep_engine::logic::modules::flight_deck_behavior::FlightDeckBehavior {
    let ob = w.objects.find(me).unwrap();
    let Some(UpdateBody::FlightDeckBehavior(m)) = ob.behaviors[slot].body.as_ref() else { panic!("the module is back in its slot") };
    m
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

fn coords_hash(v: Option<Vec<Coord3D>>) -> u32 {
    let Some(v) = v else { return 0xffff_ffff };
    let mut h = v.len() as u32;
    for c in &v {
        h = h.wrapping_mul(31) ^ c.x.to_bits() ^ c.y.to_bits().wrapping_mul(7) ^ c.z.to_bits().wrapping_mul(13);
    }
    h
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::FlightDeckBehavior(d) = &data else { unreachable!() };
    for x in [d.heal_amount, d.approach_height, d.landing_deck_height_offset] {
        put_u32(&mut v, x.to_bits());
    }
    put_i32(&mut v, d.num_rows);
    put_i32(&mut v, d.num_cols);
    for x in [d.cleanup_frames, d.human_follow_frames, d.replacement_frames, d.dock_animation_frames, d.launch_wave_frames, d.launch_ramp_frames, d.lower_ramp_frames, d.catapult_fire_frames] {
        put_u32(&mut v, x);
    }
    put_str(&mut v, &d.thing_template_name);
    for r in &d.runway_info {
        for names in [&r.spaces_bone_names, &r.taxi_bone_names, &r.creation_bone_names] {
            put_u32(&mut v, names.len() as u32);
            for n in names {
                put_str(&mut v, n);
            }
        }
        for n in r.takeoff_bone_names.iter().chain(r.landing_bone_names.iter()) {
            put_str(&mut v, n);
        }
        put_str(&mut v, r.catapult_particle_system.as_deref().unwrap_or(b"(null)"));
    }

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    factory.new_template(b"R82Jet", 0).expect("a template");
    let jet_template = Arc::new(factory.find_template(b"R82Jet").unwrap().data.clone());
    let jet_template_id = jet_template.get_template_id();
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
    let template_of = || Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R82Carrier".to_vec(), ..Default::default() }));
    let mut modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FlightDeckBehavior", b"ModuleTag_Deck", &data).unwrap().unwrap()];
    if cfg[1] & 1 != 0 {
        modules.push(probe(b"R82Production", ProbeAnswers { has: 1 << 1, production: Some((0, 0)), ..Default::default() }, 0, false, 0.0));
    }
    let me = w.new_object(NewObject { template: template_of(), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let mut ai_slots = [None; 6];
    let mut body_slots = [None; 6];
    for k in 0..6usize {
        let modules = vec![
            probe(b"JetAIUpdate", ProbeAnswers { jet: Some((0.0, false)), ..Default::default() }, 0, false, 0.0),
            probe(b"R82Ai", ProbeAnswers::default(), 0, true, 0.0),
            probe(b"R82Body", ProbeAnswers { no_death: true, ..Default::default() }, MODULEINTERFACE_BODY, false, 100.0),
        ];
        let id = w.new_object(NewObject { template: jet_template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 2);
        ai_slots[k] = w.find_module(id, b"R82Ai");
        body_slots[k] = w.find_module(id, b"R82Body");
    }
    let sc = Scene { ai_slots, body_slots, drawable: cfg[1] >> 1 & 1 != 0 };
    let idx = w.find_module(me, b"FlightDeckBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let slot = u.index as usize;
    set_scene(&mut w, &cfg, &sc);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let put_state = |v: &mut Vec<u8>, w: &World| {
        let mut words = module_of(w, me, slot).state_words();
        words[0] = match words[0] {
            0xffff_ffff => 0xffff_ffff,
            id if id == u32::from(jet_template_id) => 1,
            _ => 2,
        };
        for x in words {
            put_u32(v, x);
        }
        let made = w.next_object_id().saturating_sub(8);
        put_u32(v, (6 + made) as u32);
        for id in 2..8 + made {
            put_u32(v, id as u32);
            let Some(o) = w.objects.find(id) else {
                put_u32(v, 0xffff_ffff);
                continue;
            };
            let p = o.get_position();
            for x in [o.producer_id as u32, o.team, p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), o.thing.orientation().to_bits()] {
                put_u32(v, x);
            }
        }
    };
    put_state(&mut v, &w);
    let mut saved: Vec<u8> = Vec::new();
    let mut names = BTreeMap::new();
    for o in ops {
        if o[0] == 2 && (2..=13).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 8 && o[1] != 1 && w.objects.find(o[1] as ObjectId).is_some() {
            let id = o[1] as ObjectId;
            let World { sched, objects, .. } = &mut w;
            sched.erase_object_updates(objects, id);
            w.objects.remove(id);
        }
        set_scene(&mut w, &cfg, &sc);
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        let made_before = w.next_object_id();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
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
                    0 => call(&mut w, ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template: jet_template_id }),
                    1 => call(&mut w, ModuleCall::ParkingPlaceHasAvailableSpaceFor { template: jet_template_id }),
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
                    16 => call(&mut w, ModuleCall::ReserveDoorForExit { template: Some(jet_template_id), specific: 0 }),
                    17 => {
                        if w.objects.find(id).is_some() && id != 0 {
                            call(&mut w, ModuleCall::ExitObjectViaDoor { obj: id, door: o[3] as i32 })
                        } else {
                            ModuleReply::Done
                        }
                    }
                    18 => call(&mut w, ModuleCall::UnreserveDoorForExit(o[2] as i32)),
                    19 => {
                        let want_pos = o[3] & 1 != 0;
                        let r = w
                            .with_body(u, |b, w| match b {
                                UpdateBody::FlightDeckBehavior(m) => m.calc_best_parking_assignment(w, id, want_pos),
                                _ => unreachable!(),
                            })
                            .expect("the module is in its slot");
                        match r {
                            Ok((moved, pos, old_index, new_index)) => {
                                info = Some(PPInfo {
                                    parking_space: pos.unwrap_or(Coord3D { x: 1.5, y: -2.5, z: 3.5 }),
                                    parking_orientation: f32::from_bits(old_index as u32),
                                    runway_takeoff_dist: f32::from_bits(new_index.unwrap_or(-7) as u32),
                                    ..Default::default()
                                });
                                if moved {
                                    t.best_assignments_moved += 1;
                                }
                                ModuleReply::Bool(moved)
                            }
                            Err(e) => {
                                w.cuts.module_errors.push((ModuleHandle { object: me, index: idx }, e));
                                ModuleReply::Done
                            }
                        }
                    }
                    20 => {
                        let obj = if o[4] != 0 && w.objects.find(o[4] as ObjectId).is_some() { o[4] as ObjectId } else { INVALID_ID };
                        let pos = Coord3D { x: 12.5, y: -7.0, z: 0.0 };
                        call(&mut w, ModuleCall::AiDoCommand { cmd: COMMANDS[(o[2] & 7) as usize], command_source: o[3] as i32, obj, pos })
                    }
                    21 => ModuleReply::Int(coords_hash(module_of(&w, me, slot).get_taxi_locations(id)) as i32),
                    22 => ModuleReply::Int(coords_hash(module_of(&w, me, slot).get_creation_locations(id)) as i32),
                    23 => {
                        let busy = call(&mut w, ModuleCall::IsExitBusy).as_bool();
                        let exit = call(&mut w, ModuleCall::GetExitPosition).as_coord_opt().is_some();
                        let natural = call(&mut w, ModuleCall::GetNaturalRallyPoint).as_coord_opt().is_some();
                        let rally = call(&mut w, ModuleCall::GetRallyPoint).as_coord_opt().is_some();
                        call(&mut w, ModuleCall::SetRallyPoint(Coord3D::default()));
                        ModuleReply::Int(i32::from(busy) | i32::from(exit) << 1 | i32::from(natural) << 2 | i32::from(rally) << 3)
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
            9 => {
                // the plane onto space b's place (none: not moved)
                let at = module_of(&w, me, slot).space_prep(o[2] as usize);
                if let (Some(at), Some(_)) = (at, w.objects.find(o[1] as ObjectId)) {
                    w.set_position(o[1] as ObjectId, &at);
                    t.teleports += 1;
                }
            }
            1 | 2 | 8 => {}
            _ => unreachable!(),
        }
        t.jets_made += (w.next_object_id() - made_before) as u64;
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
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        put_u32(&mut v, u32::from(info.is_some()));
        if let Some(i) = &info {
            put_info(&mut v, i);
        }
        let log = effects(&mut w, &mut t, jet_template_id, &mut names);
        if log.iter().any(|l| l.starts_with("aiguardpos") || l.starts_with("aiattackpos") || l.starts_with("aiattackmove") || l.starts_with("aiforceattack"))
            && o[0] == 0
        {
            t.launches += 1;
        }
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
fn flight_deck_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R82 FlightDeckBehavior",
        seed: 0x0082_F1D8,
        cases_var: "L9R82_CASES",
        ini_stem: "Fd",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r82_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R82: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines, {} jets made ({} space \
         reservations, {} runway reservations, {} exits, {} heals, {} kills, {} defections, {} moved forward, {} best assignments moved, {} orders, \
         {} launch updates, {} catapult systems, {} ramp changes, {} productions queued, {} pending commands purged); {} reserves of a negative count threw",
        c.cases,
        c.read_failed,
        c.excluded,
        c.ops_run,
        t.updates,
        t.xfers,
        t.effects,
        t.jets_made,
        t.spaces,
        t.runways,
        t.exits,
        t.heals,
        t.kills,
        t.defects,
        t.moved_forward,
        t.best_assignments_moved,
        t.orders,
        t.launches,
        t.catapults,
        t.ramps,
        t.productions,
        t.purges,
        t.length_errors
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(
        t.jets_made > 0 && t.spaces > 0 && t.runways > 0 && t.exits > 0 && t.heals > 0 && t.kills > 0 && t.defects > 0 && t.moved_forward > 0 && t.orders > 0
            && t.launches > 0 && t.catapults > 0 && t.ramps > 0 && t.productions > 0 && t.purges > 0,
        "the generator must reach every effect"
    );
}
