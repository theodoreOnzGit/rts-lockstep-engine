#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PhysicsBehavior` (`logic::modules::physics_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R50.
//!
//! # Method
//!
//! The C++ side is `PhysicsUpdate.cpp` and its header, compiled a second time into `wrap_module.cpp`'s unit under R50 names
//! (`differential/l9_physics_r50.inc`, `ref_l9r50_run`; the unit's `PhysicsBehavior` is the module build's stand-in), with
//! `Object::canCrushOrSquish`, `Object::onCollide` and `Object::findModule` verbatim from `Object.cpp`, on the module build's stand-ins: four
//! objects (ids 1-4: a body whose damage is a line and whose crushed ends the test gives, kinds, crusher and crushable levels, a geometry, a
//! position, disabled and status bits, maybe effectively dead or off the map); object 1 the module's (an angle, maybe a drawable, maybe an AI with
//! a locomotor set, an ignored obstacle, an answer to `processCollision` and locomotor surfaces); objects 2-4 maybe with a `PhysicsBehavior` of
//! their own; object 2 maybe with an AI; containment (1 in 4, 2 in 1, 4 in 3); the global data's gravity, stiffnesses and structure rubble height;
//! the terrain's layer heights, cliff cells and water; the object transform (`Thing`'s); the audio manager; the weapon store (`WEAP_A`,
//! `WEAP_B`); the script engine's `transferObjectName`, `defect` and `destroyObject` (lines); the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them; and the REAL `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with the same objects (the modules from `modules::new_module`, `ProbeModule` bodies and AIs), the
//! real `Thing` transform, `cuts.terrain_layer_height`, `cuts.terrain_cliff`, `cuts.terrain_has_water`, a weapon store with the two weapons;
//! every op an `update()` as the scheduler runs it, a call of the module's interface, a change to the scene, or an xfer.
//!
//! Each case reads a block `D` (the module's 18 fields; mostly well formed) and runs 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data; the constructor's wake calls; per op the throw status, the result, the effects in order (the layer, cliff and water look-ups, the model
//! conditions, the disabled calls, the damage, crush and kill, the AI's `processCollision`, the bounce sound, the crash weapons, the destroys, the
//! pilot's capture: `transferObjectName` and `defect`), the wake calls, the xfer status, bytes and CRC, the module's 23 state words, object 1's
//! transform and angle, the other objects' positions and physics velocities, and the captured flags.
//!
//! # Results (2026-10-25, `L9R50_CASES=9000`; re-measured the same day after the zero-fill correction below)
//!
//! 9,000 cases (1,442 data blocks threw, as the C++ did; 4 excluded, undefined upstream: a parachuting object bouncing off a structure
//! through a container with no physics), 229,972 ops and 475,052 outputs: **0 mismatches**. In the compared cases: 66,191 updates, 24,208
//! xfers, 17,705 collisions, 85,347 effect lines (1,533 landings, 645 bounce sounds, 204 falling damages, 996 crushes, 861 kills, 3,763 AI
//! `processCollision` calls, 7 crash weapons, 1,023 destroys, 36 pilot captures).
//!
//! **CORRECTED 2026-10-25.** ~~9,000 cases (2,476 excluded, undefined upstream: 2,473 `m_turning` saved or read before anything set it, or
//! the never-set `m_originalAllowBounce` read when a bounce ends; 3 the parachute case), 139,359 ops and 288,882 outputs: 0 mismatches.~~
//! The first run treated the two members the constructor never sets as undefined. They are not, in the shipped game or here: the memory pool
//! zero-fills every module it allocates (`MemoryPool::allocateBlockImplementation`, as the test shim's `newInstance` does), so `m_turning` is
//! `TURN_NONE` and `m_originalAllowBounce` is `false`. The Rust side now holds those zeros, the driver makes its modules with `newInstance`
//! (it had used a plain `new`, whose memory is not zeroed, and pinned `m_turning` to a sentinel), and the 2,472 cases those members excluded
//! are compared.
//!
//! On the way: `applyRandomRotation`'s modifier is `GameLogicRandomValue(-1.0f, 1.0f)`, the integer draw (-1, 0 or 1), where the first
//! translation drew a real; the driver's `createAndFireTempWeapon` stand-in (batch K's) read a `NULL` template, which upstream's returns on
//! (fixed there); a NaN position destroys the object through `Object::reactToTransformChange`, which the driver's transform hook now marks as
//! the Rust world's `Thing` does; and the shim's inline `GameLogic::destroyObject` lost, where it was not inlined, to other units' out-of-line
//! definitions of the same name (the driver calls it by a name of its own).
//!
//! Not reached: a projectile's `projectileHandleCollision` (no projectile update interface in the scene).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2201 | no gravity | 170,715 |
//! | 2202 | the forward friction the lateral one | 45,374 |
//! | 2203 | the fall damage factor ignored | 39 |
//! | 2204 | `KillWhenRestingOnGround` ignored | 860 |
//! | 2205 | the overlap push not capped at 5 | 2,980 |
//! | 2206 | a crush deals no damage | 92 |
//! | 2207 | a motive force keeps its forward part | 65,530 |
//! | 2208 | `scrubVelocity2D` never slows | 3,434 |
//! | 2209 | `m_velMag` not saved | 10,893 |
//!
//! (The counts are the re-measured ones; the first run's, before the correction, were 127,139, 19,176, 18, 465, 1,566, 58, 36,396, 1,933 and
//! 6,287.)

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::disabled_types::{DISABLED_FREEFALL, DISABLED_HELD, DISABLED_UNMANNED};
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_DRONE, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{
    OBJECT_STATUS_BRAKING, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_PARACHUTING,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{PRIVATE_EFFECTIVELY_DEAD, PRIVATE_OFF_MAP};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r50.bin");
const XFER_FILE: &str = "l9_r50.bin";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The driver's geometry tables: the radii and the heights.
const RADII: [f32; 8] = [1.0, 2.5, 5.0, 10.0, 20.0, 0.5, 3.0, 7.5];
const HEIGHTS: [f32; 4] = [1.0, 3.0, 8.0, 0.0];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The driver's `weaponCode`: the store's weapon a name finds (case blind), 0 for none.
fn weapon_code(n: &Option<Vec<u8>>) -> i32 {
    match n.as_deref() {
        Some(x) if x.eq_ignore_ascii_case(b"WEAP_A") => 1,
        Some(x) if x.eq_ignore_ascii_case(b"WEAP_B") => 2,
        _ => 0,
    }
}

const STATE_WORDS: usize = 23 + 12 + 1 + 3 * 6 + 4 + 1;

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
    let data: Vec<u32> = (0..18).map(|_| u32_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {data:x?} constructor wakes {w} last {l}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..STATE_WORDS).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn real_text(g: &mut Gen, choices: &[&str], clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return pick(g, &["x", "", "1.5.5"]).to_string();
    }
    pick(g, choices).to_string()
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = real_text(g, choices, clean);
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 50, "Mass", &["1", "5", "50", "0.5", "200"]);
    field(g, 20, "ShockResistance", &["0.5", "2", "10"]);
    field(g, 20, "ShockMaxYaw", &["0.05", "0.2", "1"]);
    field(g, 20, "ShockMaxPitch", &["0.025", "0.2", "1"]);
    field(g, 20, "ShockMaxRoll", &["0.025", "0.2", "1"]);
    field(g, 40, "ForwardFriction", &["0", "3", "15", "30", "45"]);
    field(g, 40, "LateralFriction", &["0", "3", "15", "30", "45"]);
    field(g, 40, "ZFriction", &["0", "3", "15", "30", "45"]);
    field(g, 40, "AerodynamicFriction", &["0", "1", "3", "15"]);
    field(g, 25, "CenterOfMassOffset", &["0", "2", "-3"]);
    field(g, 50, "AllowBouncing", &["Yes", "No"]);
    field(g, 30, "AllowCollideForce", &["Yes", "No"]);
    field(g, 30, "KillWhenRestingOnGround", &["Yes", "No"]);
    field(g, 40, "MinFallHeightForDamage", &["0", "5", "40", "100"]);
    field(g, 30, "FallHeightDamageFactor", &["0", "1", "2.5", "10"]);
    field(g, 25, "PitchRollYawFactor", &["0", "1", "2", "4"]);
    field(g, 40, "VehicleCrashesIntoBuildingWeaponTemplate", &["WEAP_A", "WEAP_B", "None", "NoSuchWeapon"]);
    field(g, 40, "VehicleCrashesIntoNonBuildingWeaponTemplate", &["WEAP_A", "WEAP_B", "None", "NoSuchWeapon"]);
    if !clean && g.below(3) == 0 {
        lines.push(pick(g, &["NoSuchField = 1", "Mass = 0", "Mass = -1", "ShockMaxYaw", "AllowBouncing = maybe"]).to_string());
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

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    cfg[0] = g.below(200);
    cfg[1] = g.u32();
    cfg[2] = p(g, 80) | p(g, 60) << 1 | p(g, 55) << 2 | p(g, 40) << 3 | p(g, 40) << 4 | p(g, 30) << 5 | p(g, 12) << 6 | p(g, 8) << 7 | p(g, 25) << 8;
    let me_kinds = *pick(g, &[&0u32, &1, &2, &2, &16, &32, &3, &1]);
    let mut kinds = me_kinds;
    for k in 1..4 {
        kinds |= *pick(g, &[&0u32, &4, &12, &12, &2, &1, &6]) << (8 * k);
    }
    cfg[3] = kinds;
    let mut levels = 0;
    for k in 0..4 {
        levels |= (*pick(g, &[&0u32, &1, &2, &3, &0]) | *pick(g, &[&0u32, &1, &2, &0]) << 4) << (8 * k);
    }
    cfg[4] = levels;
    let gen_dis = |g: &mut Gen| (0..4).fold(0u32, |acc, k| acc | if g.below(6) == 0 { g.below(8) << (8 * k) } else { 0 });
    let gen_status = |g: &mut Gen| {
        (0..4).fold(0u32, |acc, k| {
            let mut s = 0u32;
            for (b, pct) in [(0, 12), (1, 10), (2, 8), (3, 4), (4, 5), (5, 4)] {
                if g.below(100) < pct {
                    s |= 1 << b;
                }
            }
            acc | s << (8 * k)
        })
    };
    let gen_ai = |g: &mut Gen| *pick(g, &[&255u32, &5, &0, &255]) | *pick(g, &[&0u32, &0, &2, &3, &4]) << 8 | p(g, 75) << 16 | g.below(16) << 20;
    let gen_heights = |g: &mut Gen| {
        let r = g.u32();
        *pick(g, &[&0u32, &0, &0x2222_2222, &r])
    };
    let gen_terrain = |g: &mut Gen| if g.below(5) == 0 { g.below(256) | g.below(256) << 8 } else { 0 };
    let gen_water = |g: &mut Gen| f(*pick(g, &[&0.0f32, &2.5, &-1.0]));
    let gen_rubble = |g: &mut Gen| f(*pick(g, &[&0.0f32, &5.0, &20.0]));
    cfg[5] = gen_dis(g);
    cfg[6] = gen_status(g);
    cfg[7] = gen_ai(g);
    cfg[8] = *pick(g, &[&0u32, &0, &1]);
    cfg[9] = gen_heights(g);
    cfg[10] = gen_terrain(g);
    cfg[11] = gen_water(g);
    cfg[12] = gen_rubble(g);
    cfg[13] = if g.below(4) == 0 { g.below(256) } else { 0 };
    cfg[14] = (0..4).fold(0u32, |acc, k| acc | g.below(128) << (8 * k));
    let me_pos = [f(*pick(g, &[&0.0f32, &3.0, &-2.5])), f(*pick(g, &[&0.0f32, &1.0])), f(*pick(g, &[&0.0f32, &0.5, &10.0, &50.0, &120.0, &0.0]))];
    cfg[15..18].copy_from_slice(&me_pos);
    let other_pos = |g: &mut Gen| [f(*pick(g, &[&0.0f32, &2.0, &4.0, &8.0, &30.0])), f(*pick(g, &[&0.0f32, &1.0, &-1.0])), f(*pick(g, &[&0.0f32, &0.0, &5.0, &20.0]))];
    for k in 1..4 {
        let q = other_pos(g);
        cfg[15 + 3 * k..18 + 3 * k].copy_from_slice(&q);
    }
    cfg[27] = f(*pick(g, &[&0.0f32, &0.5, &1.570_796_4, &3.0, &-2.0]));
    let n_ops = 1 + g.below(60) as usize;
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // m_turning is set by resetDynamicPhysics or setTurning: most cases set it early, so that their saves are defined
    if g.below(10) < 8 {
        ops.push(if g.below(2) == 0 { [12, 0, 0, 0, 0] } else { [11, 12, *pick(g, &[&0u32, &1, &u32::MAX]), 0, 0] });
    }
    if g.below(10) < 4 {
        ops.push([11, 19, 1, 0, 0]);
    }
    let force = |g: &mut Gen| f(*pick(g, &[&0.0f32, &0.0, &1.0, &-1.0, &3.5, &-10.0, &25.0, &0.01]));
    let obj = |g: &mut Gen| g.below(5);
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=29 => [0, 0, 0, 0, 0],
            30..=37 => {
                let a = *pick(g, &[&5u32, &6, &7, &8, &9, &10, &11, &12, &18, &19, &20, &21, &22, &23, &24, &25, &26]);
                let b = match a {
                    5 => gen_dis(g),
                    6 => gen_status(g),
                    7 => gen_ai(g),
                    8 => *pick(g, &[&0u32, &1]),
                    9 => gen_heights(g),
                    10 => gen_terrain(g),
                    11 => gen_water(g),
                    12 => gen_rubble(g),
                    _ => {
                        let k = (a - 15) % 3;
                        let q = other_pos(g);
                        q[k as usize]
                    }
                };
                [1, a, b, 0, 0]
            }
            38..=45 => {
                frame += 1 + g.below(30);
                [2, frame, 0, 0, 0]
            }
            46..=49 => [3, 0, 0, 0, 0],
            50..=53 => [4, 0, 0, 0, 0],
            54..=56 => [5, 0, 0, 0, 0],
            57..=64 => [6, force(g), force(g), force(g), 0],
            65..=68 => [7, force(g), force(g), force(g), 0],
            69..=71 => [8, force(g), force(g), force(g), 0],
            72..=73 => [9, 0, 0, 0, 0],
            74..=81 => [10, obj(g), 0, 0, 0],
            82..=91 => {
                let a = g.below(21);
                let b = match a {
                    0..=2 => f(*pick(g, &[&0.0f32, &0.1, &-0.2, &0.5])),
                    3..=10 | 19 => p(g, 50),
                    11 | 20 => obj(g),
                    12 => *pick(g, &[&0u32, &1, &u32::MAX]),
                    13 | 14 => f(*pick(g, &[&0.0f32, &0.5, &2.0, &-1.0])),
                    15 => f(*pick(g, &[&1.0f32, &10.0, &0.0, &-2.0])),
                    _ => f(*pick(g, &[&0.0f32, &0.5, &-0.5, &2.0])),
                };
                [11, a, b, 0, 0]
            }
            92..=93 => [12, 0, 0, 0, 0],
            94 => [13, f(*pick(g, &[&0.0f32, &1.0, &-2.5])), f(*pick(g, &[&0.0f32, &0.3, &3.1])), f(*pick(g, &[&0.0f32, &0.2, &-1.6])), 0],
            95 => [14, 0, 0, 0, 0],
            96..=97 => [15, g.below(15), obj(g), 0, 0],
            98 => [16, 2 + g.below(3), obj(g), 0, 0],
            _ => [17, force(g), force(g), force(g), 0],
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
    landings: u64,
    bounces: u64,
    damages: u64,
    crushes: u64,
    kills: u64,
    collides: u64,
    pushes: u64,
    weapons: u64,
    destroys: u64,
    captures: u64,
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

/// The driver's `scene`: the disabled and status bits, the AIs' answers, the terrain, the rubble height.
fn set_scene(w: &mut World, cfg: &[u32], ais: &[Option<usize>; 2]) {
    for k in 0..4u32 {
        let id = k as ObjectId + 1;
        let dis = cfg[5] >> (8 * k) & 255;
        let st = cfg[6] >> (8 * k) & 255;
        let o = w.objects.find_mut(id).unwrap();
        o.disabled.set(DISABLED_UNMANNED, (dis & 1) as i32);
        o.disabled.set(DISABLED_HELD, (dis >> 1 & 1) as i32);
        o.disabled.set(DISABLED_FREEFALL, (dis >> 2 & 1) as i32);
        for (b, bit) in [OBJECT_STATUS_PARACHUTING, OBJECT_STATUS_BRAKING, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_NO_COLLISIONS].into_iter().enumerate() {
            o.status.set(bit, (st >> b & 1) as i32);
        }
        o.private_status &= !(PRIVATE_EFFECTIVELY_DEAD | PRIVATE_OFF_MAP);
        if st >> 4 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        }
        if st >> 5 & 1 != 0 {
            o.private_status |= PRIVATE_OFF_MAP;
        }
    }
    let a = cfg[7];
    for (n, ai) in ais.iter().enumerate() {
        let Some(i) = *ai else { continue };
        let id = n as ObjectId + 1;
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[i].body.as_mut() {
            if n == 0 {
                p.answers.locomotor_set_type = if a & 255 == 255 { None } else { Some((a & 255) as i32) };
                p.answers.ignored_obstacle = (a >> 8 & 255) as ObjectId;
                p.answers.process_collision = a >> 16 & 1 != 0;
                p.answers.locomotor_surfaces = a >> 20 & 15;
            } else {
                p.answers.ignored_obstacle = (cfg[8] & 255) as ObjectId;
            }
        }
    }
    for c in 0..8 {
        w.cuts.terrain_layer[c] = 0;
        w.cuts.terrain_layer_height[c] = (cfg[9] >> (4 * c) & 15) as f32 * 0.5;
        w.cuts.terrain_cliff[c] = cfg[10] >> c & 1 != 0;
        w.cuts.terrain_has_water[c] = cfg[10] >> (8 + c) & 1 != 0;
        w.cuts.terrain_water[c] = f32::from_bits(cfg[11]);
    }
    w.cuts.body.globals.default_structure_rubble_height = f32::from_bits(cfg[12]);
}

fn position(cfg: &[u32], k: usize) -> Coord3D {
    Coord3D { x: f32::from_bits(cfg[15 + 3 * k]), y: f32::from_bits(cfg[16 + 3 * k]), z: f32::from_bits(cfg[17 + 3 * k]) }
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::PhysicsBehavior(d) = &data else { unreachable!() };
    for x in [
        d.mass,
        d.shock_resistance,
        d.shock_max_yaw,
        d.shock_max_pitch,
        d.shock_max_roll,
        d.forward_friction,
        d.lateral_friction,
        d.z_friction,
        d.aerodynamic_friction,
        d.center_of_mass_offset,
        // `m_minFallSpeedForDamage`: heightToSpeed of the height, with the global data's gravity (-1)
        (-2.0f32 * d.min_fall_height_for_damage).abs().sqrt(),
        d.fall_height_damage_factor,
        d.pitch_roll_yaw_factor,
    ] {
        put_u32(&mut v, x.to_bits());
    }
    for b in [d.kill_when_resting_on_ground, d.allow_bouncing, d.allow_collide_force] {
        put_u32(&mut v, u32::from(b));
    }
    put_i32(&mut v, weapon_code(&d.vehicle_crashes_into_building_weapon));
    put_i32(&mut v, weapon_code(&d.vehicle_crashes_into_non_building_weapon));

    let mut w = World::new();
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    let mut scratch = rts_lockstep_engine::common::name_key_generator::NameKeyGenerator::new();
    scratch.reset();
    for n in WEAPONS {
        weapon_store.new_weapon_template(&mut scratch, n.as_bytes()).unwrap();
    }
    w.names = ctx.names;
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, weapon_store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.crate_stub_object_calls = true;
    let flags = cfg[2];
    let probe = |name: &[u8], answers: ProbeAnswers| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        pm
    };
    let kinds_of = |b: u32| {
        let mut v = Vec::new();
        for (bit, k) in [KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_IMMOBILE, KINDOF_STRUCTURE, KINDOF_DRONE, KINDOF_PROJECTILE].into_iter().enumerate() {
            if b >> bit & 1 != 0 {
                v.push(k);
            }
        }
        v
    };
    let mut constructor_wakes = (0u32, 0u32);
    let mut ais: [Option<usize>; 2] = [None, None];
    let mut phys_idx: [Option<usize>; 4] = [None; 4];
    for k in 0..4u32 {
        let mut modules = Vec::new();
        let has_phys = k == 0 || flags >> (1 + k) & 1 != 0;
        if has_phys {
            phys_idx[k as usize] = Some(modules.len());
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &data).unwrap().unwrap());
        }
        let crushed = cfg[13] >> (2 * k);
        let mut body = probe(b"R50Body", ProbeAnswers { front_crushed: Some(crushed & 1 != 0), back_crushed: Some(crushed >> 1 & 1 != 0), no_death: true, ..Default::default() });
        body.interfaces = MODULEINTERFACE_BODY;
        modules.push(body);
        if (k == 0 && flags >> 1 & 1 != 0) || (k == 1 && flags >> 5 & 1 != 0) {
            let mut ai = probe(b"R50Ai", ProbeAnswers::default());
            ai.is_ai = true;
            ais[k as usize] = Some(modules.len());
            modules.push(ai);
        }
        let levels = cfg[4] >> (8 * k);
        let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues {
            kind_of: kinds_of(cfg[3] >> (8 * k) & 255),
            crusher_level: (levels & 15) as u8,
            crushable_level: (levels >> 4 & 15) as u8,
            ..Default::default()
        }));
        let id = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        if k == 0 {
            // the module's own wake calls (the other objects' modules make theirs under the same module index)
            let i = w.find_module(1, b"PhysicsBehavior").expect("the module") as i32;
            for t in w.cuts.trace.take().unwrap() {
                if let Some((m, fr)) = t.strip_prefix("awaken ").and_then(|l| l.split_once(' ')) {
                    if m.parse::<i32>().unwrap() == i {
                        constructor_wakes.0 += 1;
                        constructor_wakes.1 = fr.parse().unwrap();
                    }
                }
            }
            w.cuts.trace = Some(Vec::new());
        }
    }
    // the helper modules the world makes come first: module indices are relative to each object's first own module
    for k in 0..4u32 {
        let base = w.find_module(k as ObjectId + 1, b"R50Body").expect("the body") as usize - usize::from(phys_idx[k as usize].is_some());
        if let Some(i) = &mut phys_idx[k as usize] {
            *i += base;
        }
        if let Some(i) = ais.get_mut(k as usize).and_then(|a| a.as_mut()) {
            *i += base;
        }
    }
    let idx = phys_idx[0].unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx as u16 }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == idx as i32).then(|| f.parse().unwrap())
    };
    put_u32(&mut v, constructor_wakes.0);
    put_u32(&mut v, constructor_wakes.1);
    let others: Vec<Option<rts_lockstep_engine::logic::world::sleepy::UpdateHandle>> =
        (0..4).map(|k| phys_idx[k].and_then(|i| w.update_handle_of_module(ModuleHandle { object: k as ObjectId + 1, index: i as u16 }))).collect();
    for k in 0..4usize {
        let id = k as ObjectId + 1;
        let g = cfg[14] >> (8 * k) & 255;
        let geom_type = match (g & 3) % 3 {
            1 => GeometryType::Cylinder,
            2 => GeometryType::Box,
            _ => GeometryType::Sphere,
        };
        let r = RADII[(g >> 2 & 7) as usize];
        w.objects.find_mut(id).unwrap().geometry_info = GeometryInfo::new(geom_type, false, HEIGHTS[(g >> 5 & 3) as usize], r, r);
        w.set_position(id, &position(&cfg, k));
        if k == 0 {
            w.set_orientation(id, f32::from_bits(cfg[27]));
        }
        let o = w.objects.find_mut(id).unwrap();
        o.drawable = if k == 0 && flags & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        if k == 0 {
            o.name = b"UnitA".to_vec();
        }
    }
    if flags >> 6 & 1 != 0 {
        w.objects.find_mut(1).unwrap().contained_by = 4;
    }
    if flags >> 7 & 1 != 0 {
        w.objects.find_mut(2).unwrap().contained_by = 1;
    }
    if flags >> 8 & 1 != 0 {
        w.objects.find_mut(4).unwrap().contained_by = 3;
    }
    set_scene(&mut w, &cfg, &ais);
    let mut saved: Vec<u8> = Vec::new();
    let mut prev_z = f32::from_bits(cfg[17]);
    for o in ops {
        if o[0] == 1 && ((5..=12).contains(&o[1]) || (18..=26).contains(&o[1])) {
            cfg[o[1] as usize] = o[2];
            if o[1] >= 18 {
                let k = (o[1] as usize - 15) / 3;
                w.set_position(k as ObjectId + 1, &position(&cfg, k));
            }
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &ais);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let fc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
        let obj_b = if (1..=4).contains(&o[2]) { o[2] as ObjectId } else { INVALID_ID };
        let call = |w: &mut World, c: ModuleCall| w.module_call(1, idx as u16, c);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 | 2 => {}
            3..=5 => {
                let r = run_xfer(&mut w, 1, idx, o[0] - 3, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.borrow_mut().xfers += 1;
            }
            6 => {
                call(&mut w, ModuleCall::PhysicsApplyForce(fc));
            }
            7 => {
                call(&mut w, ModuleCall::PhysicsApplyMotiveForce(fc));
            }
            8 => {
                call(&mut w, ModuleCall::PhysicsApplyShock(fc));
            }
            9 => {
                call(&mut w, ModuleCall::PhysicsApplyRandomRotation);
            }
            10 => {
                let other = if (1..=4).contains(&o[1]) { o[1] as ObjectId } else { INVALID_ID };
                call(&mut w, ModuleCall::OnCollide { other, loc: None, normal: None });
                tally.borrow_mut().collides += 1;
            }
            11 => {
                let fb = f32::from_bits(o[2]);
                let bb = o[2] != 0;
                let c = match o[1] {
                    0 => ModuleCall::PhysicsSetYawRate(fb),
                    1 => ModuleCall::PhysicsSetPitchRate(fb),
                    2 => ModuleCall::PhysicsSetRollRate(fb),
                    3 => ModuleCall::PhysicsSetAllowBouncing(bb),
                    4 => ModuleCall::PhysicsSetAllowCollideForce(bb),
                    5 => ModuleCall::PhysicsSetStickToGround(bb),
                    6 => ModuleCall::PhysicsSetAllowToFall(bb),
                    7 => ModuleCall::PhysicsSetImmuneToFallingDamage(bb),
                    8 => ModuleCall::PhysicsSetAllowAirborneFriction(bb),
                    9 => ModuleCall::PhysicsSetStunned(bb),
                    10 => ModuleCall::PhysicsSetIsInFreeFall(bb),
                    11 => ModuleCall::PhysicsSetIgnoreCollisionsWith(obj_b),
                    12 => ModuleCall::PhysicsSetTurning(o[2] as i32),
                    13 => ModuleCall::PhysicsScrubVelocity2D(fb),
                    14 => ModuleCall::PhysicsScrubVelocityZ(fb),
                    15 => ModuleCall::PhysicsSetMass(fb),
                    16 => ModuleCall::PhysicsSetExtraBounciness(fb),
                    17 => ModuleCall::PhysicsSetExtraFriction(fb),
                    18 => ModuleCall::PhysicsClearAcceleration,
                    19 => ModuleCall::PhysicsSetBounceSound(bb.then(|| b"SND_A".to_vec())),
                    _ => ModuleCall::PhysicsAddOverlap(obj_b),
                };
                if !(o[1] == 20 && obj_b == INVALID_ID) {
                    call(&mut w, c);
                }
            }
            12 => {
                call(&mut w, ModuleCall::PhysicsResetDynamicPhysics);
            }
            13 => {
                call(&mut w, ModuleCall::PhysicsSetAngles { yaw: fc.x, pitch: fc.y, roll: fc.z });
            }
            14 => {
                call(&mut w, ModuleCall::OnObjectCreated);
            }
            15 => {
                let real = |r: ModuleReply| r.as_real(0.0).to_bits();
                let flag = |r: ModuleReply| u32::from(r.as_bool());
                res = match o[1] {
                    0 => real(call(&mut w, ModuleCall::PhysicsGetMass)),
                    1 => real(call(&mut w, ModuleCall::PhysicsGetVelocityMagnitude)),
                    2 => real(call(&mut w, ModuleCall::PhysicsGetForwardSpeed2D)),
                    3 => real(call(&mut w, ModuleCall::PhysicsGetForwardSpeed3D)),
                    4 => flag(call(&mut w, ModuleCall::PhysicsIsMotive)),
                    5 => call(&mut w, ModuleCall::PhysicsGetCurrentOverlap).as_id() as u32,
                    6 => call(&mut w, ModuleCall::PhysicsGetPreviousOverlap).as_id() as u32,
                    7 => call(&mut w, ModuleCall::PhysicsGetLastCollidee).as_id() as u32,
                    8 => flag(call(&mut w, ModuleCall::PhysicsIsIgnoringCollisionsWith(o[2] as ObjectId))),
                    9 => flag(call(&mut w, ModuleCall::PhysicsGetAllowToFall)),
                    10 => flag(call(&mut w, ModuleCall::PhysicsGetIsStunned)),
                    11 => flag(call(&mut w, ModuleCall::PhysicsGetIsInFreeFall)),
                    12 => real(call(&mut w, ModuleCall::PhysicsGetCenterOfMassOffset)),
                    13 => flag(call(&mut w, ModuleCall::WouldLikeToCollideWith { other: obj_b })),
                    14 => call(&mut w, ModuleCall::PhysicsGetTurning).as_int(0) as u32,
                    _ => 0,
                };
            }
            16 => {
                let k = o[1] as usize - 1;
                if let Some(i) = phys_idx.get(k).copied().flatten().filter(|_| k >= 1) {
                    w.module_call(k as ObjectId + 1, i as u16, ModuleCall::PhysicsSetIgnoreCollisionsWith(obj_b));
                }
            }
            17 => {
                call(&mut w, ModuleCall::PhysicsAddVelocityTo(fc));
            }
            _ => unreachable!(),
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
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let b = |x: f32| x.to_bits();
        let mut t = tally.borrow_mut();
        for line in w.cuts.trace.take().unwrap() {
            if line.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", b(x), b(y))),
                    Some(CutCall::TerrainIsCliffCell { x, y }) => log.push(format!("cliff {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::TerrainIsUnderwater { x, y }) => log.push(format!("underwater {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => log.push(format!("setdisabled {object} {disabled_type}")),
                    Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        t.destroys += 1;
                        log.push(format!("destroy {object}"))
                    }
                    Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) => {
                        t.weapons += 1;
                        log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(CutCall::ScriptEngineTransferObjectName { name, object }) => {
                        t.captures += 1;
                        log.push(format!("transfername {} {object}", String::from_utf8_lossy(&name)))
                    }
                    Some(CutCall::ObjectDefect { object, team, detection_time }) => log.push(format!("defect {object} {} {detection_time}", u32::from(team.is_some()))),
                    _ => {}
                }
            } else if line.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(fl), .. }) => log.push(format!("mc {}", fl[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(fl), .. }) => log.push(format!("mcclear {}", fl[0])),
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                        t.bounces += 1;
                        let pos = match position {
                            Some(p) => format!("{},{},{}", b(p.x), b(p.y), b(p.z)),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)))
                    }
                    _ => {}
                }
            } else if let Some(fr) = line.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = fr;
            } else if line.starts_with("damage ") {
                let tok: Vec<&str> = line.split(' ').collect();
                match tok.get(3).copied() {
                    Some(dt) if dt == rts_lockstep_engine::logic::damage::DAMAGE_CRUSH.to_string() => t.crushes += 1,
                    Some(dt) if dt == rts_lockstep_engine::logic::damage::DAMAGE_FALLING.to_string() => t.damages += 1,
                    _ => t.kills += 1,
                }
                log.push(line.clone());
            } else if let Some(l) = line.strip_prefix("bt aiprocess ") {
                t.pushes += 1;
                log.push(format!("aiprocess {l}"));
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
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        let Some(UpdateBody::PhysicsBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        for s in m.state_words() {
            put_u32(&mut v, s);
        }
        let mx = *ob.thing.transform_matrix();
        for r in 0..3 {
            for c in [mx[r].x, mx[r].y, mx[r].z, mx[r].w] {
                put_u32(&mut v, c.to_bits());
            }
        }
        put_u32(&mut v, ob.thing.orientation().to_bits());
        let z = ob.get_position().z;
        if o[0] == 0 && prev_z > 0.0 && z <= 0.0 {
            tally.borrow_mut().landings += 1;
        }
        prev_z = z;
        for (k, other) in others.iter().enumerate().skip(1) {
            let oo = w.objects.find(k as ObjectId + 1).unwrap();
            let p = oo.get_position();
            for c in [p.x, p.y, p.z] {
                put_u32(&mut v, c.to_bits());
            }
            let vel = other.map(|h| match oo.behaviors[h.index as usize].body.as_ref() {
                Some(UpdateBody::PhysicsBehavior(m)) => {
                    let s = m.state_words();
                    [s[9], s[10], s[11]]
                }
                _ => [0, 0, 0],
            });
            for c in vel.unwrap_or([0, 0, 0]) {
                put_u32(&mut v, c);
            }
        }
        for k in 1..=4 {
            put_u32(&mut v, u32::from(w.objects.find(k).unwrap().is_captured()));
        }
        put_u32(&mut v, w.objects.find(1).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame());
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
fn physics_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R50 PhysicsBehavior",
        seed: 0x0050_9475,
        cases_var: "L9R50_CASES",
        ini_stem: "Ph",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r50_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R50: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines, {} landings, {} bounce sounds, \
         {} falling damages, {} crushes, {} kills, {} collisions, {} AI pushes, {} crash weapons, {} destroys, {} captures",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.landings, t.bounces, t.damages, t.crushes, t.kills, t.collides, t.pushes,
        t.weapons, t.destroys, t.captures
    );
    println!("excluded by: {:?}", t.excluded);
}
