#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `RailroadBehavior` (`logic::modules::railroad_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R79.
//!
//! # Method
//!
//! The C++ side is `RailroadGuideAIUpdate.cpp` and its header, compiled into `wrap_module.cpp`'s unit on batch R50's real `PhysicsBehavior`
//! (`differential/l9_railroad_r79.inc`, `ref_l9r79_run`), on the module build's stand-ins: six objects (ids 1-6). Objects 1-4 are train cars
//! (1 always, with the module of the data the case reads; 2-4 maybe, with that data made a carriage), maybe with a drawable and a contain;
//! objects 5-6 are what a train hits (their kinds, maybe a demo trap, a `PhysicsBehavior` with or without a bounce sound, an AI whose enter
//! target may be the train, a body; under construction, effectively dead, contained by object 1); every object a position, an angle, a
//! geometry and a team of three. Up to eight waypoints in the test's list order, at its places, with its names (`...Tunnel`, `...Station`,
//! `...Disembark`, `...PingPong`) and links; a thing factory whose `CAR_A` makes a car (ids 7, 8, ...) and `CAR_B` an object that is none; the
//! partition manager's closest object (the first of the test's candidates the caller's filter allows); the pathfinder's walls, the ground,
//! the audio and the game logic's `destroyObject` (lines); the real `Object::kill`, `Object::getRelationship`, `UpdateModule`, `INI`, `Xfer`
//! and logic random values.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, the victims' real
//! `PhysicsBehavior`, `ProbeModule` bodies, contains, AIs and demo traps), the terrain's waypoints (`TerrainLogic::friend_push_waypoint`), a
//! thing factory whose `CAR_A` has a `RailroadBehavior`, the teams' relationships; every op an `update()` as the scheduler runs it, a call of a
//! car's interface, a change to the scene, or an xfer of object 1's module.
//!
//! Each case reads a block `D` (the module's fields, mostly well formed) and runs 1 to 80 ops. Compared, bit for bit: the read's throw, the
//! data; per op the throw status, the result, the effects in order (the ground, closest-object, audio, wall, damage, destroy, drawable and
//! passenger-exit lines), the xfer status, bytes and CRC; and every object's transform, angle and producer, every car's 57 state words and
//! its track (whether it has one, whether it loops, its length, its points and their hash), and the victims' velocities, rates and physics
//! flags.
//!
//! # Results (2026-10-30, the default 9,000 cases)
//!
//! 9,000 cases (1,246 data blocks threw, as the C++ did; 386 excluded, undefined upstream: 370 waypoint paths that cycle without coming back
//! to the anchor, whose walk never ends; 12 impact sounds for an object no player controls; 4 kills of an object with no body), 299,975 ops
//! and 2,219,861 outputs: **0 mismatches**. In the compared cases: 144,870 updates, 24,047 xfers, 12,610 collisions, 727,359 effect lines;
//! 2,634 cars made by the factory; 10,285 tracks read (5,156 looping, 13,179 station, disembark and ping-pong points); 193 whistles, 17,097
//! clickety-clacks, 23,803 impact sounds, 566 damages and kills, 4,701 walls, 837 passenger exits, 7,486 destroys.
//!
//! On the way: the upstream file keeps its name keys in function-local statics made the first time each function runs, and the test's name key
//! generator starts over in most cases, so the C++ found no car after the first case; while the file is included the driver's `NAMEKEY`
//! gives the two names its stand-ins answer to fixed keys. `MAX(heft.z, getGroundHeight(...) + 2)` asks the terrain twice when it picks the
//! ground (the translation asked once). A victim with physics never gets the default impact sound (`getBounceSound()` is the audio manager's
//! silent event, not `NULL`; the translation fell back to the defaults). The module lent to its slot for a hitch was a copy made before the
//! hitch marked it hitched, so the next car's filter took it for free and the cars hitched one another for ever: the hitch's own state is
//! set before the module is lent.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 3,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3771 | an unpulled car never becomes the lead | 56 |
//! | 3772 | a train never kills outright | 11,496 |
//! | 3773 | a non-looping track's end never reached | 14,718 |
//! | 3774 | a car not turned towards its puller | 157,356 |
//! | 3775 | a victim's physics gives no bounce sound | 9,184 |
//! | 3776 | a disembark point makes nobody get out | 8,452 |
//! | 3777 | a ping-pong point does not turn the train | 5,459 |
//! | 3778 | a car made from the template list gets no producer | 14,678 |
//! | 3779 | the car's own pull info not saved | 6,221 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{
    KINDOF_FS_BASE_DEFENSE, KINDOF_FS_FACTORY, KINDOF_FS_POWER, KINDOF_FS_TECHNOLOGY, KINDOF_HUGE_VEHICLE, KINDOF_INFANTRY, KINDOF_REBUILD_HOLE,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehaviorModuleData;
use rts_lockstep_engine::logic::modules::railroad_behavior::{RailroadBehavior, RailroadBehaviorModuleData};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r79.bin");
const XFER_FILE: &str = "l9_r79.bin";
/// The driver's tables: the radii, the ground heights, the waypoint names' suffixes, the victims' kinds.
const RADII: [f32; 8] = [1.0, 2.5, 5.0, 8.0, 12.0, 0.5, 3.0, 6.0];
const GROUNDS: [f32; 8] = [0.0, 1.0, -2.0, 4.0, -5.0, 0.5, 8.0, -10.0];
const SUFFIX: [&str; 6] = ["", "Tunnel", "Station", "Disembark", "PingPong", "StationX"];
const KINDS: [i32; 9] = [
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_HUGE_VEHICLE,
    KINDOF_STRUCTURE,
    KINDOF_FS_POWER,
    KINDOF_FS_FACTORY,
    KINDOF_FS_BASE_DEFENSE,
    KINDOF_FS_TECHNOLOGY,
    KINDOF_REBUILD_HOLE,
];
/// A car's state words.
const STATE_WORDS: usize = 57;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::RailroadBehavior(Arc::new(RailroadBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
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
    if b.is_empty() {
        return out;
    }
    let n_names = u32_(&mut p);
    let names: Vec<String> = (0..n_names).map(|_| str_(&mut p)).collect();
    let (prefix, fx) = (str_(&mut p), str_(&mut p));
    let data: Vec<u32> = (0..9).map(|_| u32_(&mut p)).collect();
    let sounds: Vec<String> = (0..6).map(|_| str_(&mut p)).collect();
    out.push(format!("data {names:?} {prefix:?} {fx:?} {data:x?} {sounds:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        let objects = u32_(&mut p);
        for k in 0..objects {
            let pose: Vec<u32> = (0..14).map(|_| u32_(&mut p)).collect();
            let mut line = format!("op {i} object {} pose {pose:x?}", k + 1);
            if u32_(&mut p) != 0 {
                let state: Vec<u32> = (0..STATE_WORDS).map(|_| u32_(&mut p)).collect();
                line.push_str(&format!(" car {state:x?}"));
                if u32_(&mut p) != 0 {
                    let track: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
                    line.push_str(&format!(" track {track:x?}"));
                }
            }
            if (k == 4 || k == 5) && u32_(&mut p) != 0 {
                let phys: Vec<u32> = (0..7).map(|_| u32_(&mut p)).collect();
                line.push_str(&format!(" physics {phys:x?}"));
            }
            out.push(line);
        }
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = if !clean && g.below(15) == 0 { pick(g, &["x", "", "1.5.5"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 90, "IsLocomotive", &["Yes", "Yes", "Yes", "Yes", "No"]);
    field(g, 30, "PathPrefixName", &["TrainPath", "Line"]);
    field(g, 15, "CrashFXTemplateName", &["FX_TrainCrash"]);
    field(g, 50, "RunningGarrisonSpeedMax", &["0.5", "1", "2.5", "0"]);
    field(g, 50, "KillSpeedMin", &["0.5", "1", "2", "5", "100"]);
    field(g, 70, "SpeedMax", &["1", "2", "4", "6", "10"]);
    field(g, 60, "Acceleration", &["1.01", "1.05", "1.2", "1.5"]);
    field(g, 60, "Braking", &["0.99", "0.9", "0.8", "0.5"]);
    field(g, 50, "Friction", &["0.97", "0.9", "0.5", "1"]);
    field(g, 60, "WaitAtStationTime", &["100", "300", "1000", "5000"]);
    field(g, 80, "RunningSound", &["SND_RUN", "NoSound"]);
    field(g, 80, "ClicketyClackSound", &["SND_CLACK", "NoSound"]);
    field(g, 70, "WhistleSound", &["SND_WHISTLE", "NoSound"]);
    field(g, 60, "BigMetalBounceSound", &["SND_BIG", "NoSound"]);
    field(g, 60, "SmallMetalBounceSound", &["SND_SMALL"]);
    field(g, 60, "MeatyBounceSound", &["SND_MEAT"]);
    field(g, 15, "Mass", &["5", "50"]);
    field(g, 10, "AllowBouncing", &["Yes", "No"]);
    // the carriage list: lines of one to three names
    let n_lines = *pick(g, &[&0u32, &1, &1, &1, &2, &3]);
    for _ in 0..n_lines {
        let k = 1 + g.below(3);
        let names: Vec<&str> = (0..k).map(|_| pick(g, &["CAR_A", "CAR_A", "CAR_A", "CAR_B", "CAR_X"])).collect();
        lines.push(format!("CarriageTemplateName = {}", names.join(" ")));
    }
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["NoSuchField = 1", "IsLocomotive = maybe", "SpeedMax", "WaitAtStationTime = x"]).to_string());
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

/// A waypoint word: x and y (signed bytes, eighths), z (halves), the suffix, the link (0 none, else the waypoint's number).
fn waypoint_word(x: i32, y: i32, z: u32, suffix: u32, link: u32) -> u32 {
    (x as u8 as u32) | (y as u8 as u32) << 8 | z << 16 | suffix << 20 | link << 23
}

fn waypoint_pos(c: u32) -> Coord3D {
    Coord3D { x: f32::from(c as u8 as i8) * 8.0, y: f32::from((c >> 8) as u8 as i8) * 8.0, z: (c >> 16 & 15) as f32 * 0.5 }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    cfg[0] = g.below(200);
    cfg[1] = g.u32();
    // the waypoints: a polyline of 2-8 points, a line, a loop, a line into a loop (the anchor outside it: never read past it, rarely),
    // or random links
    let n = 2 + g.below(7);
    cfg[2] = n;
    let shape = g.below(10);
    let (mut x, mut y) = (g.below(9) as i32 - 4, g.below(9) as i32 - 4);
    for k in 0..n {
        let suffix = match g.below(20) {
            0..=1 => 1,
            2..=4 => 2,
            5..=6 => 3,
            7..=8 => 4,
            9 => 5,
            _ => 0,
        };
        let link = match shape {
            0..=3 => {
                if k + 1 < n {
                    k + 2
                } else {
                    0
                }
            }
            4..=7 => {
                if k + 1 < n {
                    k + 2
                } else {
                    1
                }
            }
            8 => {
                // a line into a loop of the last points (not through the anchor: upstream's walk never ends)
                if k + 1 < n {
                    k + 2
                } else {
                    (n - 1).max(1)
                }
            }
            _ => {
                let t = g.below(n + 1);
                if t == k + 1 {
                    0
                } else {
                    t
                }
            }
        };
        cfg[3 + k as usize] = waypoint_word(x, y, g.below(4), suffix, link);
        x = (x + g.below(9) as i32 - 2).clamp(-120, 120);
        y = (y + g.below(9) as i32 - 4).clamp(-120, 120);
    }
    // object 1 at or near a waypoint; the others behind it or about
    let at = waypoint_pos(cfg[3 + g.below(n) as usize]);
    let jitter = |g: &mut Gen| *pick(g, &[&0.0f32, &0.0, &0.5, &-3.0, &12.0]);
    let pos = |g: &mut Gen, base: Coord3D| (f(base.x + jitter(g)), f(base.y + jitter(g)));
    let (x1, y1) = pos(g, at);
    cfg[11] = x1;
    cfg[17] = y1;
    for k in 1..6usize {
        let base = if g.below(3) == 0 { waypoint_pos(cfg[3 + g.below(n) as usize]) } else { at };
        let (xk, yk) = pos(g, base);
        cfg[11 + k] = xk;
        cfg[17 + k] = yk;
    }
    cfg[23] = g.u32() & 0x00FF_FFFF;
    let team1 = *pick(g, &[&1u32, &1, &2, &3]);
    let mut cars = 0u32;
    for k in 0..4u32 {
        let rail = if k == 0 { 1 } else { p(g, 50) };
        let team = if g.below(5) == 0 { g.below(4) } else { team1 };
        let b = rail | p(g, 60) << 1 | p(g, 25) << 2 | team << 3 | g.below(8) << 5;
        cars |= b << (8 * k);
    }
    cfg[24] = cars;
    for k in 0..2usize {
        let mut kinds = 0u32;
        match g.below(8) {
            0..=2 => kinds |= 1,
            3..=4 => kinds |= 2,
            5 => kinds |= 4,
            6 => kinds |= 8 | ((1 << (4 + g.below(5))) * p(g, 70)),
            _ => {}
        }
        let team = if g.below(15) == 0 { 0 } else { 1 + g.below(3) };
        let w = kinds | p(g, 8) << 9 | p(g, 10) << 10 | p(g, 8) << 11 | p(g, 60) << 12 | p(g, 30) << 13 | p(g, 30) << 14 | p(g, 8) << 15 | team << 16
            | p(g, 40) << 18 | g.below(8) << 19 | p(g, 95) << 22;
        cfg[25 + k] = w;
    }
    let gen_candidates = |g: &mut Gen| {
        let mut c = 0u32;
        let mut ids = vec![2u32, 3, 4];
        if g.below(4) == 0 {
            ids.push(5 + g.below(2));
        }
        let mut slot = 0;
        while !ids.is_empty() && slot < 7 {
            let i = g.below(ids.len() as u32) as usize;
            let id = ids.remove(i);
            if g.below(6) != 0 {
                c |= id << (4 * slot);
                slot += 1;
            }
        }
        c
    };
    cfg[27] = gen_candidates(g);
    let mut rel = 0u32;
    for a in 0..3u32 {
        for b in 0..3u32 {
            let r = if a == b { if g.below(10) == 0 { g.below(3) } else { 2 } } else { g.below(3) };
            rel |= r << (2 * (3 * a + b));
        }
    }
    cfg[28] = rel;
    let gen_ground = |g: &mut Gen| {
        let mut w = p(g, 30);
        if g.below(3) == 0 {
            w |= (g.u32() & 0x00FF_FFFF) << 1;
        }
        w
    };
    cfg[29] = gen_ground(g);
    let n_ops = 1 + g.below(80) as usize;
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    let car = |g: &mut Gen| *pick(g, &[&1u32, &1, &2, &3, &4, &7, &8, &9, &10]);
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=39 => [0, 1, 0, 0, 0],
            40..=59 => [0, car(g), 0, 0, 0],
            60..=69 => {
                let other = *pick(g, &[&5u32, &5, &6, &6, &2, &3, &4, &7, &0]);
                [6, car(g), other, 0, if g.below(10) == 0 { 1 + g.below(3) } else { 0 }]
            }
            70..=74 => {
                let a = *pick(g, &[&11u32, &12, &13, &14, &15, &16, &17, &18, &19, &20, &21, &22, &23, &25, &26, &27, &29]);
                let b = match a {
                    11..=22 => {
                        let base = waypoint_pos(cfg[3 + g.below(n) as usize]);
                        let v = if a <= 16 { base.x } else { base.y };
                        f(v + jitter(g))
                    }
                    23 => g.u32() & 0x00FF_FFFF,
                    25 | 26 => {
                        let mut w = cfg[a as usize] & !(1 << 10 | 1 << 11 | 1 << 14 | 1 << 15);
                        w |= p(g, 15) << 10 | p(g, 10) << 11 | p(g, 40) << 14 | p(g, 10) << 15;
                        w
                    }
                    27 => gen_candidates(g),
                    _ => gen_ground(g),
                };
                [1, a, b, 0, 0]
            }
            75..=77 => {
                frame += 1 + g.below(30);
                [2, frame, 0, 0, 0]
            }
            78..=80 => [3, 0, 0, 0, 0],
            81..=83 => [4, 0, 0, 0, 0],
            84..=85 => [5, 0, 0, 0, 0],
            86..=88 => [7, car(g), 0, 0, 0],
            89..=91 => [8, car(g), p(g, 50), 0, 0],
            92 => [9, car(g), 0, 0, 0],
            93..=94 => [10, car(g), 0, 0, 0],
            95..=97 => [11, car(g), p(g, 50), 0, 0],
            _ => [12, car(g), 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    collides: u64,
    effects: u64,
    made: u64,
    tracks: u64,
    looping: u64,
    stations: u64,
    whistles: u64,
    clacks: u64,
    impacts: u64,
    damages: u64,
    walls: u64,
    exits: u64,
    destroys: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
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
            b.truncate(1024);
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

/// FNV-1a over words, as the driver's.
fn hash(words: &[u32]) -> u32 {
    let mut h = 2_166_136_261u32;
    for w in words {
        for b in 0..4 {
            h ^= w >> (8 * b) & 255;
            h = h.wrapping_mul(16_777_619);
        }
    }
    h
}

fn angle(cfg: &[u32], k: usize) -> f32 {
    (cfg[23] >> (4 * k) & 15) as f32 * 0.4 - 3.0
}

/// The driver's `apply`: the positions and angles, the victims' changing state, the candidates, the audio and the ground.
fn apply(w: &mut World, cfg: &[u32], a: u32, ais: &[Option<u16>; 2]) {
    match a {
        11..=23 => {
            for k in 0..6usize {
                let id = k as ObjectId + 1;
                w.set_position(id, &Coord3D { x: f32::from_bits(cfg[11 + k]), y: f32::from_bits(cfg[17 + k]), z: 0.0 });
                w.set_orientation(id, angle(cfg, k));
            }
        }
        25 | 26 => {
            let k = a as usize - 21;
            let id = k as ObjectId + 1;
            let fl = cfg[a as usize];
            if let Some(i) = ais[k - 4] {
                if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[i as usize].body.as_mut() {
                    p.answers.enter_target = if fl >> 14 & 1 != 0 { 1 } else { INVALID_ID };
                }
            }
            let o = w.objects.find_mut(id).unwrap();
            o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (fl >> 10 & 1) as i32);
            if fl >> 11 & 1 != 0 {
                o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
            } else {
                o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
            }
            o.contained_by = if fl >> 15 & 1 != 0 { 1 } else { INVALID_ID };
        }
        27 => {
            let mut scan = Vec::new();
            for k in 0..8 {
                let id = cfg[27] >> (4 * k) & 15;
                if id == 0 {
                    break;
                }
                if id <= 6 {
                    scan.push(id as ObjectId);
                }
            }
            w.cuts.range_scan = scan;
        }
        29 => {
            w.cuts.audio_playing = cfg[29] & 1 != 0;
            for c in 0..8 {
                w.cuts.terrain_ground[c] = GROUNDS[(cfg[29] >> (1 + 3 * c) & 7) as usize];
            }
        }
        _ => {}
    }
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::RailroadBehavior(d) = &data else { unreachable!() };
    put_u32(&mut v, d.carriage_template_name_data.len() as u32);
    for n in &d.carriage_template_name_data {
        put_str(&mut v, n);
    }
    put_str(&mut v, &d.path_prefix_name);
    put_str(&mut v, &d.crash_fx_template_name);
    put_u32(&mut v, u32::from(d.is_locomotive));
    for x in [d.running_garrison_speed_max, d.kill_speed_min, d.speed_max, d.acceleration, d.braking, d.friction, d.physics.mass] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, d.wait_at_station_time);
    for s in [&d.running_sound, &d.clickety_clack_sound, &d.big_metal_impact_default_sound, &d.small_metal_impact_default_sound, &d.meaty_impact_default_sound, &d.whistle_sound] {
        put_str(&mut v, s);
    }
    if std::env::var("R79_TRACE").is_ok() {
        eprintln!("rust case cfg {cfg:x?} ops {ops:?} data {d:?}");
    }
    let car_data = ModuleDataKind::RailroadBehavior(Arc::new(RailroadBehaviorModuleData { is_locomotive: false, ..(**d).clone() }));
    let phys_data = ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()));

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    factory.new_template(b"CAR_A", 0).expect("a template");
    factory.new_template(b"CAR_B", 0).expect("a template");
    let plain = || ModuleInfoData::Logic(ModuleDataKind::Behavior(Arc::new(Default::default())));
    {
        let info = factory.friend_find_template_mut(b"CAR_A").unwrap().data.friend_behavior_module_info_mut();
        info.add_module_info(b"RailroadBehavior", b"ModuleTag_Rail", ModuleInfoData::Logic(car_data.clone()), RailroadBehavior::get_interface_mask(), false, false);
        info.add_module_info(b"R79Body", b"ModuleTag_Body", plain(), MODULEINTERFACE_BODY, false, false);
    }
    factory.friend_find_template_mut(b"CAR_B").unwrap().data.friend_behavior_module_info_mut().add_module_info(b"R79Body", b"ModuleTag_Body", plain(), MODULEINTERFACE_BODY, false, false);
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.probe_trace_exit_all = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.untranslated_probe = Some(ProbeAnswers { no_death: true, ..Default::default() });
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let rel = |r: u32| match r {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    };
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[28] >> (2 * (3 * a + b)) & 3));
        }
    }
    // the waypoints: the list's head is the last pushed
    let n_ways = (cfg[2] & 255).min(8);
    for k in (0..n_ways).rev() {
        let c = cfg[3 + k as usize];
        let name = format!("WP{}{}", k + 1, SUFFIX[((c >> 20 & 7) % 6) as usize]);
        w.terrain.friend_push_waypoint(21 + k as i32, name.as_bytes(), waypoint_pos(c));
    }
    for k in 0..n_ways {
        let link = cfg[3 + k as usize] >> 23 & 15;
        if link >= 1 && link <= n_ways && link != k + 1 {
            w.terrain.friend_add_waypoint_link(21 + k as i32, 20 + link as i32);
        }
    }
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.interfaces = interfaces;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        pm
    };
    let mut ais: [Option<u16>; 2] = [None, None];
    for k in 0..6usize {
        let id = k as ObjectId + 1;
        let fl = if k < 4 { cfg[24] >> (8 * k) & 255 } else { cfg[21 + k] };
        let mut modules = Vec::new();
        let mut kinds = Vec::new();
        let mut ai_at: Option<usize> = None;
        if k < 4 {
            if k == 0 || fl & 1 != 0 {
                let dk = if k == 0 { &data } else { &car_data };
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"RailroadBehavior", b"ModuleTag_Rail", dk).unwrap().unwrap());
            }
            modules.push(probe(b"R79Body", ProbeAnswers { no_death: true, ..Default::default() }, MODULEINTERFACE_BODY));
            if fl >> 2 & 1 != 0 {
                modules.push(probe(b"R79Contain", ProbeAnswers::default(), MODULEINTERFACE_CONTAIN));
            }
        } else {
            for (b, kk) in KINDS.iter().enumerate() {
                if fl >> b & 1 != 0 {
                    kinds.push(*kk);
                }
            }
            if fl >> 12 & 1 != 0 {
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &phys_data).unwrap().unwrap());
            }
            if fl >> 22 & 1 != 0 {
                modules.push(probe(b"R79Body", ProbeAnswers { no_death: true, ..Default::default() }, MODULEINTERFACE_BODY));
            }
            if fl >> 13 & 1 != 0 {
                let mut ai = probe(b"R79Ai", ProbeAnswers::default(), 0);
                ai.is_ai = true;
                ai_at = Some(modules.len());
                modules.push(ai);
            }
            if fl >> 9 & 1 != 0 {
                modules.push(probe(b"DemoTrapUpdate", ProbeAnswers::default(), 0));
            }
        }
        let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { kind_of: kinds, ..Default::default() }));
        let made = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(made, id);
        if ai_at.is_some() {
            ais[k - 4] = w.find_module(id, b"R79Ai");
        }
        let team = if k < 4 { fl >> 3 & 3 } else { fl >> 16 & 3 };
        let r = if k < 4 { RADII[(fl >> 5 & 7) as usize] } else { RADII[(fl >> 19 & 7) as usize] };
        let o = w.objects.find_mut(id).unwrap();
        o.team = team;
        o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, r, r, r);
        o.drawable = if k < 4 && fl >> 1 & 1 != 0 { 7 + id } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[11 + k]), y: f32::from_bits(cfg[17 + k]), z: 0.0 });
        w.set_orientation(id, angle(&cfg, k));
        if k >= 4 && fl >> 12 & 1 != 0 && fl >> 18 & 1 != 0 {
            let pi = w.find_module(id, b"PhysicsBehavior").unwrap();
            w.module_call(id, pi, ModuleCall::PhysicsSetBounceSound(Some(b"SND_B".to_vec())));
        }
    }
    for a in 25..=29 {
        apply(&mut w, &cfg, a, &ais);
    }
    let idx1 = w.find_module(1, b"RailroadBehavior").expect("the module");
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 && ((11..=23).contains(&o[1]) || (25..=27).contains(&o[1]) || o[1] == 29) {
            cfg[o[1] as usize] = o[2];
            apply(&mut w, &cfg, o[1], &ais);
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        if std::env::var("R79_TRACE").is_ok() {
            eprintln!("rust op {o:?}");
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let a = o[1] as ObjectId;
        let ma = if w.objects.find(a).is_some() { w.find_module(a, b"RailroadBehavior") } else { None };
        let obj_b = if o[2] >= 1 && w.objects.find(o[2] as ObjectId).is_some() { o[2] as ObjectId } else { INVALID_ID };
        match o[0] {
            0 => {
                if let Some(i) = ma {
                    let u = w.update_handle_of_module(ModuleHandle { object: a, index: i }).expect("an update module");
                    res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
            }
            1 | 2 => {}
            3..=5 => {
                let r = run_xfer(&mut w, 1, idx1 as usize, o[0] - 3, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    w.load_post_process_module_slot(1, idx1 as usize);
                }
                tally.borrow_mut().xfers += 1;
            }
            _ => {
                if let Some(i) = ma {
                    let call = match o[0] {
                        6 => {
                            let loc = (o[4] & 1 == 0).then(|| w.objects.find(obj_b).map(|x| x.get_position()).unwrap_or_default());
                            let normal = (o[4] & 2 == 0).then_some(Coord3D { x: 0.0, y: 0.0, z: 1.0 });
                            tally.borrow_mut().collides += 1;
                            ModuleCall::OnCollide { other: obj_b, loc, normal }
                        }
                        7 => ModuleCall::IsRailroad,
                        8 => ModuleCall::RailroadSetHeld(o[2] != 0),
                        9 => ModuleCall::RailroadDestroyTheWholeTrainNow,
                        10 => ModuleCall::RailroadDisembark,
                        11 => ModuleCall::RailroadMakeAWallOutOfThisTrain(o[2] != 0),
                        _ => ModuleCall::RailroadHasEverBeenHitched,
                    };
                    let r = w.module_call(a, i, call);
                    if o[0] == 7 || o[0] == 12 {
                        res = u32::from(r.as_bool());
                    }
                }
            }
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let status = if new_errors.iter().any(|(_, e)| !e.is_undefined()) { 2 } else { 0 };
        put_i32(&mut v, status);
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let mut ev_texts: Vec<Option<String>> = events.iter().map(|e| Some(format!("evt {e:?}"))).collect();
        let ev_list = events;
        let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
        let mut log = Vec::new();
        let b = |x: f32| x.to_bits();
        let pos_text = |p: Option<Coord3D>| match p {
            Some(p) => format!("{},{},{}", b(p.x), b(p.y), b(p.z)),
            None => "-".to_string(),
        };
        let mut t = tally.borrow_mut();
        let trace = w.cuts.trace.take().unwrap();
        if std::env::var("R79_TRACE").is_ok() {
            for l in &trace {
                eprintln!("  trace {l}");
            }
        }
        for line in trace {
            if line.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::PartitionGetClosestObjectAt { position: q, range, dc }) => {
                        log.push(format!("closestat {:08x} {:08x} {:08x} {:08x} {dc}", b(q.x), b(q.y), b(q.z), b(range)))
                    }
                    Some(CutCall::AudioIsCurrentlyPlaying { handle }) => log.push(format!("isplaying {handle}")),
                    Some(CutCall::PathfinderCreateWallFromFootprint { object }) => {
                        t.walls += 1;
                        log.push(format!("wallcreate {object}"))
                    }
                    Some(CutCall::PathfinderRemoveWallFromFootprint { object }) => log.push(format!("wallremove {object}")),
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        t.destroys += 1;
                        log.push(format!("destroy {object}"))
                    }
                    _ => {}
                }
            } else if line.starts_with("evt ") {
                // (the queue's order is not always the trace's: the first event not yet taken that this line traced)
                let found = ev_texts.iter().position(|t| t.as_deref() == Some(line.as_str()));
                match found.and_then(|i| {
                    ev_texts[i] = None;
                    ev_list.get(i).cloned()
                }) {
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(fl), .. }) => log.push(format!("mc {}", fl[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(fl), .. }) => log.push(format!("mcclear {}", fl[0])),
                    Some(ObjectClientEvent::DrawableSetHidden { object_id, hidden }) => log.push(format!("hidden {object_id} {}", i32::from(hidden))),
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                        if name == b"SND_WHISTLE" {
                            t.whistles += 1;
                        }
                        log.push(format!("audio {} {} {} {object_id}", String::from_utf8_lossy(&name), pos_text(position), player_index.unwrap_or(-1)))
                    }
                    Some(ObjectClientEvent::AudioAddEventVolume { name, object_id, position, player_index, volume }) => {
                        if name == b"SND_CLACK" {
                            t.clacks += 1;
                        } else {
                            t.impacts += 1;
                        }
                        log.push(format!(
                            "audio {} {} {} {object_id} vol {:08x}",
                            String::from_utf8_lossy(&name),
                            pos_text(position),
                            player_index.unwrap_or(-1),
                            b(volume)
                        ))
                    }
                    Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                    _ => {}
                }
            } else if line.starts_with("damage ") {
                t.damages += 1;
                log.push(line.clone());
            } else if line.starts_with("exitall ") {
                t.exits += 1;
                log.push(line.clone());
            }
        }
        t.effects += log.len() as u64;
        drop(t);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let mut ids = w.objects.ids_in_order();
        ids.sort_unstable();
        put_u32(&mut v, ids.len() as u32);
        for (k, id) in ids.iter().enumerate() {
            let ob = w.objects.find(*id).unwrap();
            let mx = *ob.thing.transform_matrix();
            for r in 0..3 {
                for c in [mx[r].x, mx[r].y, mx[r].z, mx[r].w] {
                    put_u32(&mut v, c.to_bits());
                }
            }
            put_u32(&mut v, ob.thing.orientation().to_bits());
            put_u32(&mut v, ob.producer_id as u32);
            let body_of = |i: u16| {
                let u = w.update_handle_of_module(ModuleHandle { object: *id, index: i }).expect("an update module");
                ob.behaviors[u.index as usize].body.as_ref()
            };
            let rail = w.find_module(*id, b"RailroadBehavior");
            put_u32(&mut v, u32::from(rail.is_some()));
            if let Some(i) = rail {
                let Some(UpdateBody::RailroadBehavior(m)) = body_of(i) else { panic!("the module is back in its slot") };
                for s in m.state_words() {
                    put_u32(&mut v, s);
                }
                match m.track() {
                    Some(tr) => {
                        put_u32(&mut v, 1);
                        put_u32(&mut v, u32::from(tr.is_looping));
                        put_u32(&mut v, tr.length.to_bits());
                        let mut words = Vec::new();
                        for p in &tr.points {
                            let flags = u32::from(p.is_first_point)
                                | u32::from(p.is_last_point) << 1
                                | u32::from(p.is_tunnel_or_bridge) << 2
                                | u32::from(p.is_station) << 3
                                | u32::from(p.is_disembark) << 4
                                | u32::from(p.is_ping_pong) << 5;
                            words.extend([b(p.position.x), b(p.position.y), b(p.position.z), b(p.distance_from_prev), b(p.distance_from_first), p.handle as u32, flags]);
                        }
                        put_u32(&mut v, tr.points.len() as u32);
                        put_u32(&mut v, hash(&words));
                    }
                    None => put_u32(&mut v, 0),
                }
            }
            if k == 4 || k == 5 {
                let phys = w.find_module(*id, b"PhysicsBehavior");
                put_u32(&mut v, u32::from(phys.is_some()));
                if let Some(i) = phys {
                    let Some(UpdateBody::PhysicsBehavior(m)) = body_of(i) else { panic!("the physics is back in its slot") };
                    let s = m.state_words();
                    for x in [s[9], s[10], s[11], s[0], s[2], s[1], s[14]] {
                        put_u32(&mut v, x);
                    }
                }
            }
        }
    }
    {
        let mut t = tally.borrow_mut();
        let ids = w.objects.ids_in_order();
        t.made += ids.len().saturating_sub(6) as u64;
        for id in ids {
            if let Some(i) = w.find_module(id, b"RailroadBehavior") {
                let u = w.update_handle_of_module(ModuleHandle { object: id, index: i }).expect("an update module");
                if let Some(UpdateBody::RailroadBehavior(m)) = w.objects.find(id).unwrap().behaviors[u.index as usize].body.as_ref() {
                    if let Some(tr) = m.track() {
                        t.tracks += 1;
                        t.looping += u64::from(tr.is_looping);
                        t.stations += tr.points.iter().filter(|p| p.is_station || p.is_disembark || p.is_ping_pong).count() as u64;
                    }
                }
            }
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
fn railroad_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R79 RailroadBehavior",
        seed: 0x0079_7a11,
        cases_var: "L9R79_CASES",
        ini_stem: "Rr",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r79_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R79: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} collisions, {} effect lines, {} cars \
         made, {} tracks ({} looping, {} special points), {} whistles, {} clickety-clacks, {} impact sounds, {} damages and kills, {} walls, {} \
         passenger exits, {} destroys",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.collides, t.effects, t.made, t.tracks, t.looping, t.stations, t.whistles,
        t.clacks, t.impacts, t.damages, t.walls, t.exits, t.destroys
    );
    println!("excluded by: {:?}", t.excluded);
}
