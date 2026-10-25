#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BattleBusSlowDeathBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BattleBusSlowDeathBehavior` (`logic::modules::battle_bus_slow_death_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R47.
//!
//! # Method
//!
//! The C++ side is the file in `wrap_module.cpp`'s unit (`differential/l9_battle_bus_r47.inc`, `ref_l9r47_run`) on `SlowDeathBehavior.cpp`
//! and batch Q's scene (`differential/l9_slow_death_q.inc`: an object with a body, a drawable, an AI, a physics behaviour, a slaved update, a
//! tree, the slow death scale, the stores), with module 0 the bus (its data read from the block) and up to two `SlowDeathBehavior`s beside it,
//! plus: a contain module (maybe) with a count, the physics' `applyShock`, `applyRandomRotation` and `scrubVelocity2D` (lines; the scrub says
//! whether `clearAcceleration`, the header's inline, ran since the last one), the body's `attemptDamage` (a line: `Object::kill`'s damage), and
//! the real `Object::kill`. The Rust side is the real thing on a `World` as batch R46's test (`tests/differential_jet_death_r46.rs`), with the
//! bus from `modules::new_module`, a contain probe (`trace_throw`, its `contained` the count), the physics probe's `trace_throw` lines, and the
//! body probe's `damage` line (`probe_effect_trace`; it does not die, as the C++ body does not).
//!
//! Each case reads a block `D` (the base's fields and the bus's) and runs 1 to 60 ops: batch Q's (updates, deaths of every module or one,
//! `beginSlowDeath`, the probability and applicability queries, the scene, the xfers), plus the contain module and its count. Compared, bit for
//! bit: batch Q's outputs (the read, the data, every module's state, the effects), and the bus's data, its two flags and two frames and its
//! effects (the start and hit-ground FX and OCLs, the AI's idling, the physics' stop, throw and rotation, the damage to the passengers, the
//! second-life model condition, `DISABLED_HELD`, the empty-hulk kill).
//!
//! # Results (2026-10-24, `L9R47_CASES=9000`)
//!
//! 9,000 cases (1,779 data blocks threw, as the C++ did; 1,299 excluded, undefined upstream: batch Q's float-to-int conversions out of range
//! (757), no drawable (282), and no body module (260: the base's, and the empty-hulk kill's)), 175,029 ops (the harness's count, excluded cases
//! included); in the compared cases 52,594 updates, 12,165 deaths, 91,897 op rows with a slow death begun, 9,294 flings, 21,547 destructions,
//! 6,996 xfers, 5,335 undeaths (throws), 560 landings, 5,604 scrubs, 489 empty-hulk kills (the tally is restored for an excluded case); 7,701
//! data reads and 192,795 op outputs (200,496): **0 mismatches**.
//!
//! The generator differs from batch R46's in three ways, all to reach the bus's paths: deaths are rarer and `beginSlowDeath` commoner (a death
//! makes the bus's death real, and the undeath is `beginSlowDeath`'s), the frame op steps forward from the current frame three times in four
//! (the ground check and the empty-hulk timer wait on it), and the object starts on the ground more often.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2171 | the first-death flag not saved | 2,393 |
//! | 2172 | no ground-check delay | 104,132 |
//! | 2173 | no random rotation on the throw | 5,335 |
//! | 2174 | not held after landing | 532 |
//! | 2175 | a refilled bus not spared | 113 |
//! | 2176 | the empty-hulk kill one frame early (`>=`) | 458 |
//! | 2177 | the kill's death type `EXTRA_3`, not `EXTRA_4` | 489 |
//! | 2178 | the empty-hulk watch every frame, not every 15 | 4,405 |
//! | 2179 | `onDie` not marking the death real | 111,415 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::disabled_types::DISABLED_HELD;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_HULK, KINDOF_INFANTRY, KINDOF_SHRUBBERY};
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModuleData;
use rts_lockstep_engine::logic::modules::slow_death_behavior::{SlowDeathBehaviorModuleData, SD_PHASE_COUNT};
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r46.bin");
const XFER_FILE: &str = "l9_r46.bin";
/// Where the bus's scene starts in `cfg` (the driver's): the contain module (bit 0), its count.
const CFG_BUS: usize = 61;
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];

/// The words of the scene before the damage infos (the driver's `cfg`).
const CFG_INFOS: usize = 37;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::BattleBusSlowDeathBehavior(Arc::new(BattleBusSlowDeathBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn fx_code(n: &Option<Vec<u8>>) -> i32 {
    match n.as_deref() {
        None => 0,
        Some(b"FX_A") => 1,
        Some(_) => 2,
    }
}

fn ocl_code(n: &[u8]) -> i32 {
    match n {
        b"OCL_A" => 1,
        b"OCL_B" => 2,
        _ => 0,
    }
}

fn weapon_code(n: &[u8]) -> i32 {
    if n.eq_ignore_ascii_case(b"WEAP_A") {
        1
    } else if n.eq_ignore_ascii_case(b"WEAP_B") {
        2
    } else {
        0
    }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let head: Vec<u32> = (0..12).map(|_| r.u32()).collect();
    let mut lists = Vec::new();
    for _ in 0..SD_PHASE_COUNT * 3 {
        let n = r.u32();
        lists.push((0..n).map(|_| r.u32() as i32).collect::<Vec<_>>());
    }
    let mask = r.u32();
    let bus: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    out.push(format!("data {head:08x?} lists {lists:?} mask {mask} bus {bus:08x?}"));
    let n_mods = r.u32();
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut result, mut sleep) = (0, 0, 0);
        if i > 0 {
            st = r.u32() as i32;
            result = r.u32();
            sleep = r.u32() as i32;
        }
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let (wakes, last_mod, last_frame) = (r.u32(), r.u32() as i32, r.u32());
        let mods: Vec<Vec<u32>> = (0..n_mods).map(|_| (0..5).map(|_| r.u32()).collect()).collect();
        let mut x = String::new();
        if i > 0 {
            // the bus's save (longer than a SlowDeathBehavior's 29 bytes) has six versions before its scheduling word, a SlowDeathBehavior's five
            let len = r.peek_xfer_len();
            x = r.xfer_after_versions(if len > 29 { 6 } else { 5 });
        }
        let obj: Vec<u32> = (0..9 + 4).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {result} sleep {sleep} log {log:?} wakes {wakes} last {last_mod} {last_frame} mods {mods:08x?} {x} obj {obj:08x?}"));
    }
    out
}

// -- generators --

const DEATH_NAMES: [&str; 20] = [
    "NONE", "CRUSHED", "BURNED", "EXPLODED", "POISONED", "TOPPLED", "FLOODED", "SUICIDED", "LASERED", "DETONATED", "SPLATTED", "POISONED_BETA",
    "EXTRA_2", "EXTRA_3", "EXTRA_4", "EXTRA_5", "EXTRA_6", "EXTRA_7", "EXTRA_8", "POISONED_GAMMA",
];

fn death_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        t.push(match g.below(10) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=6 => format!("+{}", DEATH_NAMES[1 + g.below(19) as usize]),
            _ => format!("-{}", DEATH_NAMES[1 + g.below(19) as usize]),
        });
    }
    t.join(" ")
}

fn level_tokens(g: &mut Gen) -> String {
    let names = ["VETERAN", "ELITE", "HEROIC"];
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        t.push(match g.below(8) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=4 => format!("+{}", names[g.below(3) as usize]),
            _ => format!("-{}", names[g.below(3) as usize]),
        });
    }
    t.join(" ")
}

/// The first eight status names (the object's status bits are the first eight: the die mux can meet them).
fn status_tokens(g: &mut Gen) -> String {
    let names = rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NAMES;
    let mode = g.below(5);
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = names[1 + g.below(7) as usize];
        t.push(match mode {
            0 => format!("-{n}"),
            1 => format!("+{n}"),
            _ => n.to_string(),
        });
    }
    if mode == 4 && g.below(3) == 0 {
        return "NONE".to_string();
    }
    t.join(" ")
}

fn real_text(g: &mut Gen, choices: &[&str], clean: bool) -> String {
    if !clean && g.below(14) == 0 {
        return pick(g, &["x", "", "-", "1.5abc"]).to_string();
    }
    pick(g, choices).to_string()
}

fn field(lines: &mut Vec<String>, g: &mut Gen, p: u32, name: &str, value: String) {
    if g.below(100) < p {
        let sep = gen_sep(g);
        lines.push(format!("{name}{sep}{value}"));
    }
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let v = real_text(g, &["0", "0.5", "1", "2.5", "10", "-1"], clean);
    field(&mut lines, g, 55, "SinkRate", v);
    // never under 1 (the constructor throws INVALID_DATA, which the unit test of the module covers; the driver cannot report a constructor's throw)
    let v = match g.below(10) {
        0..=2 => "10".to_string(),
        3 => "1".to_string(),
        4 => "100".to_string(),
        _ if !clean && g.below(8) == 0 => pick(g, &["x", "", "1.5"]).to_string(),
        _ => format!("{}", 1 + g.below(30)),
    };
    field(&mut lines, g, 80, "ProbabilityModifier", v);
    let v = real_text(g, &["0%", "50%", "100%", "200%", "-50%", "25%"], clean);
    field(&mut lines, g, 55, "ModifierBonusPerOverkillPercent", v);
    for name in ["SinkDelay", "SinkDelayVariance", "DestructionDelay", "DestructionDelayVariance"] {
        let v = match g.below(8) {
            0 => "0".to_string(),
            1..=4 => format!("{}", g.below(3000)),
            5 => pick(g, &["1", "33", "34", "1000", "100000"]).to_string(),
            _ if !clean => pick(g, &["x", "", "-"]).to_string(),
            _ => "500".to_string(),
        };
        field(&mut lines, g, 65, name, v);
    }
    let v = real_text(g, &["-10", "0", "5.5", "100"], clean);
    field(&mut lines, g, 30, "DestructionAltitude", v);
    for ph in ["INITIAL", "MIDPOINT", "FINAL", "BOGUS"] {
        for (name, names) in [("FX", &["FX_A", "FX_B", "None", "FX_A"][..]), ("OCL", &["OCL_A", "OCL_B", "None", "OCL_C", "ocl_a"][..]), ("Weapon", &["WEAP_A", "WEAP_B", "None", "WEAP_X", "weap_a"][..])] {
            if (ph != "BOGUS" && g.below(100) < 28) || (ph == "BOGUS" && !clean && g.below(100) < 3) {
                let n = 1 + g.below(3);
                let list: Vec<&str> = (0..n).map(|_| pick(g, names)).collect();
                let sep = gen_sep(g);
                lines.push(format!("{name}{sep}{ph} {}", list.join(" ")));
            }
        }
    }
    let v = real_text(g, &["0", "5", "10", "-1", "20.5"], clean);
    field(&mut lines, g, 35, "FlingForce", v);
    let v = real_text(g, &["0", "2", "7"], clean);
    field(&mut lines, g, 25, "FlingForceVariance", v);
    let v = real_text(g, &["0", "30", "45", "90", "-20"], clean);
    field(&mut lines, g, 30, "FlingPitch", v);
    let v = real_text(g, &["0", "10", "30"], clean);
    field(&mut lines, g, 25, "FlingPitchVariance", v);
    let v = death_tokens(g);
    field(&mut lines, g, 20, "DeathTypes", v);
    let v = level_tokens(g);
    field(&mut lines, g, 12, "VeterancyLevels", v);
    let v = status_tokens(g);
    field(&mut lines, g, 10, "ExemptStatus", v);
    let v = status_tokens(g);
    field(&mut lines, g, 10, "RequiredStatus", v);
    // the bus's
    for name in ["FXStartUndeath", "FXHitGround"] {
        let v = pick(g, &["FX_A", "FX_B", "None"]).to_string();
        field(&mut lines, g, 60, name, v);
    }
    for name in ["OCLStartUndeath", "OCLHitGround"] {
        let v = pick(g, &["OCL_A", "OCL_B", "None", "OCL_C"]).to_string();
        field(&mut lines, g, 55, name, v);
    }
    let v = real_text(g, &["1", "0", "5.5", "20", "-2"], clean);
    field(&mut lines, g, 60, "ThrowForce", v);
    let v = real_text(g, &["0%", "50%", "100%", "25%"], clean);
    field(&mut lines, g, 60, "PercentDamageToPassengers", v);
    let v = pick(g, &["0", "100", "1000", "33", "3000", "500"]).to_string();
    field(&mut lines, g, 70, "EmptyHulkDestructionDelay", v);
    if !clean && g.below(14) == 0 {
        lines.push(pick(g, &["NoSuchField = 1", "DeathTypes = +BOGUS", "FX = NOPHASE FX_A", "FX = INITIAL", "OCL", "ThrowForce = x", "FXStartUndeath"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(14) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn f(x: f32) -> u32 {
    x.to_bits()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let n_mods = *pick(g, &[&1u32, &1, &1, &2, &2, &3]);
    let scale = match g.below(20) {
        0..=13 => 1.0,
        14..=15 => 0.0,
        16..=17 => 0.5,
        18 => 2.0,
        _ => 3.0,
    };
    let mut cfg = vec![
        g.below(2000),                                         // 0 the frame
        f(scale),                                              // 1 the slow death scale
        if g.below(5) == 0 { 18 } else { (-1i32) as u32 },     // 2 the hulk override
        n_mods,                                                // 3
        (if g.below(100) < 55 { 1 } else { 0 }) | (if g.below(100) < 30 { 2 } else { 0 }), // 4 the kinds
        (u32::from(g.below(100) < 85))                         // 5 flags
            | (u32::from(g.below(100) < 70) << 1)
            | (u32::from(g.below(100) < 55) << 2)
            | (u32::from(g.below(100) < 30) << 3)
            | (u32::from(g.below(100) < 96) << 4)
            | (u32::from(g.below(100) < 20) << 5)
            | (u32::from(g.below(100) < 15) << 6)
            | (u32::from(g.below(100) < 40) << 7),
        1 + g.below(3),                                        // 6 the veterancy (not REGULAR: see the module docs)
        f(*pick(g, &[&100.0f32, &200.0, &50.0, &1000.0, &100.0, &1.0, &100.0, &200.0, &0.5, &300.0, &0.0])), // 7 the maximum health
        f(*pick(g, &[&0.0f32, &10.0, &-5.5])),                 // 8..10 the position
        f(*pick(g, &[&0.0f32, &20.0, &-3.0])),
        f(*pick(g, &[&0.0f32, &0.0, &5.0, &0.5, &20.0, &-3.0, &1.0, &0.0])), // (on the ground often: the bus's landing)
        *pick(g, &[&0u32, &2, &7, &2]),                        // 11 the last collidee
        g.below(256) & g.below(256),                           // 12 the status bits
    ];
    // the data of modules 1 and 2
    for _ in 0..2 {
        cfg.push(1 + g.below(30));                             // the probability modifier
        cfg.push(f(*pick(g, &[&0.0f32, &0.5, &2.0, &-1.0, &10.0]))); // the overkill bonus
        cfg.push(f(*pick(g, &[&0.0f32, &0.1, &0.5, &1.0])));   // the sink rate
        cfg.push(*pick(g, &[&0u32, &10, &30, &100]));          // the sink delay
        cfg.push(*pick(g, &[&0u32, &5, &20]));
        cfg.push(*pick(g, &[&0u32, &30, &90, &300, &10]));     // the destruction delay
        cfg.push(*pick(g, &[&0u32, &5, &40]));
        cfg.push(f(*pick(g, &[&0.0f32, &0.0, &5.0, &10.0])));  // the fling force
        cfg.push(f(*pick(g, &[&0.0f32, &3.0])));
        cfg.push(f(*pick(g, &[&0.0f32, &0.5, &0.8])));         // the fling pitch (radians)
        cfg.push(f(*pick(g, &[&0.0f32, &0.2])));
    }
    cfg.push(g.u32()); // 35 the logic seed
    let n_infos = 1 + g.below(5);
    cfg.push(n_infos); // 36
    for _ in 0..n_infos {
        cfg.push(*pick(g, &[&1u32, &1, &2, &3, &5, &10, &19]));
        let dealt = *pick(g, &[&0.0f32, &50.0, &200.0, &1000.0, &10.0, &1.0e6]);
        let clipped = *pick(g, &[&0.0f32, &0.0, &50.0, &100.0, &10.0, &300.0]);
        cfg.push(f(dealt));
        cfg.push(f(clipped));
    }
    debug_assert_eq!(cfg.len(), CFG_INFOS + 3 * n_infos as usize);
    cfg.resize(CFG_BUS, 0);
    cfg.push(u32::from(g.below(100) < 75)); // 61 the contain module
    cfg.push(*pick(g, &[&0u32, &0, &1, &3])); // 62 its count
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    // a load goes into the module the last save came from: the bus's save and a SlowDeathBehavior's differ in layout, and reading one as the
    // other reads a version from a frame's byte (the two codes then fail at different words)
    let mut saved_from = None;
    // the frame, so that the frame op can step forward from it (the ground check and the empty-hulk timer wait on it)
    let mut frame = cfg[0];
    for _ in 0..n_ops {
        let a = g.below(3);
        let info = g.below(8);
        let op: [u32; 5] = match g.below(1000) {
            0..=299 => [0, if g.below(3) == 0 { a } else { 0 }, 0, 0, 0],
            // (deaths rarer than batch Q's, beginSlowDeath commoner: a death makes the bus's death real, and the undeath is beginSlowDeath's)
            300..=339 => [1, info, 0, 0, 0],
            340..=369 => [2, a, info, 0, 0],
            370..=549 => [3, a, info, 0, 0],
            550..=599 => [4, a, info, 0, 0],
            600..=639 => [5, a, info, 0, 0],
            640..=729 => {
                frame = match g.below(4) {
                    0 => g.below(2500),
                    1 => frame.wrapping_add(100 + g.below(3000)),
                    _ => frame.wrapping_add(1 + g.below(40)),
                };
                [6, frame, 0, 0, 0]
            }
            730..=769 => [7, f(*pick(g, &[&1.0f32, &0.0, &0.5, &2.0, &3.0])), 0, 0, 0],
            770..=789 => [8, *pick(g, &[&(-1i32 as u32), &18, &5]), 0, 0, 0],
            790..=829 => [9, f(*pick(g, &[&0.0f32, &5.0, &-2.0])), f(*pick(g, &[&0.0f32, &3.0])), f(*pick(g, &[&0.0f32, &0.4, &6.0, &-1.0, &30.0])), 0],
            830..=849 => [10, g.below(4), 0, 0, 0],
            850..=869 => [11, g.below(2), 0, 0, 0],
            870..=889 => [12, *pick(g, &[&0u32, &2, &7]), 0, 0, 0],
            890..=909 => [13, g.below(2), 0, 0, 0],
            910..=929 => [14, g.below(2), 0, 0, 0],
            930..=939 => [15, 1 + g.below(3), 0, 0, 0],
            940..=949 => [16, f(*pick(g, &[&100.0f32, &0.0, &50.0, &1000.0])), 0, 0, 0],
            950..=954 => [17, g.below(256) & g.below(256), 0, 0, 0],
            955..=957 => [21, *pick(g, &[&0u32, &0, &1, &2]), 0, 0, 0],
            958..=959 => [22, u32::from(g.below(100) < 75), 0, 0, 0],
            960..=974 => [18, a, 0, 0, 0],
            975..=989 => [19, saved_from.unwrap_or(a), 0, 0, 0],
            _ => [20, a, 0, 0, 0],
        };
        if op[0] == 18 {
            saved_from = Some(a);
        }
        ops.push(op);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    /// op rows in which a slow death was active
    begun: u64,
    flung: u64,
    destroyed: u64,
    phase_fx: u64,
    phase_ocl: u64,
    phase_weapon: u64,
    undeaths: u64,
    landings: u64,
    scrubs: u64,
    kills: u64,
    deaths: u64,
    /// why cases were excluded (upstream undefined)
    excluded: std::collections::BTreeMap<String, u64>,
}

/// The physics behaviour's state the Rust probe's lines fold into: the fall and bounce flags, the extra bounciness and friction (as bits).
#[derive(Default, Clone, Copy)]
struct Physics {
    fall: bool,
    bounce: bool,
    extra_bounciness: u32,
    extra_friction: u32,
}

fn veterancy(i: u32) -> VeterancyLevel {
    match i {
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        3 => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
}

fn info_of(cfg: &[u32], i: u32) -> DamageInfo {
    let n_infos = cfg[CFG_INFOS - 1] as usize;
    let k = (i as usize % n_infos.max(1)) & 7;
    let k = k.min(n_infos.saturating_sub(1));
    let mut info = DamageInfo::default();
    info.input.death_type = cfg[CFG_INFOS + 3 * k] as i32;
    info.out.actual_damage_dealt = f32::from_bits(cfg[CFG_INFOS + 3 * k + 1]);
    info.out.actual_damage_clipped = f32::from_bits(cfg[CFG_INFOS + 3 * k + 2]);
    info
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    // the tally as it was, restored if the case is excluded (its outputs are not compared: see the porting graph's R31 note)
    let tally_before = tally.borrow().clone();
    let mut v = Vec::new();
    {
        let ModuleDataKind::BattleBusSlowDeathBehavior(jd) = &data else { unreachable!("not a BattleBusSlowDeathBehavior") };
        let d = &jd.slow;
        for x in [
            d.sink_rate.to_bits(),
            d.probability_modifier as u32,
            d.modifier_bonus_per_overkill_percent.to_bits(),
            d.sink_delay,
            d.sink_delay_variance,
            d.destruction_altitude.to_bits(),
            d.destruction_delay,
            d.destruction_delay_variance,
            d.fling_force.to_bits(),
            d.fling_force_variance.to_bits(),
            d.fling_pitch.to_bits(),
            d.fling_pitch_variance.to_bits(),
        ] {
            put_u32(&mut v, x);
        }
        let (mut has_fx, mut has_ocl, mut has_weapon) = (false, false, false);
        for ph in 0..SD_PHASE_COUNT {
            put_u32(&mut v, d.fx[ph].len() as u32);
            for n in &d.fx[ph] {
                put_i32(&mut v, fx_code(n));
                has_fx |= n.is_some();
            }
            put_u32(&mut v, d.ocls[ph].len() as u32);
            for n in &d.ocls[ph] {
                put_i32(&mut v, ocl_code(n));
                has_ocl |= ocl_code(n) != 0;
            }
            put_u32(&mut v, d.weapons[ph].len() as u32);
            for n in &d.weapons[ph] {
                put_i32(&mut v, weapon_code(n));
                has_weapon |= weapon_code(n) != 0;
            }
        }
        put_u32(&mut v, u32::from(has_fx) | u32::from(has_ocl) << 1 | u32::from(has_weapon) << 2);
        put_i32(&mut v, fx_code(&jd.fx_start_undeath));
        put_i32(&mut v, jd.ocl_start_undeath.as_deref().map_or(0, ocl_code));
        put_i32(&mut v, fx_code(&jd.fx_hit_ground));
        put_i32(&mut v, jd.ocl_hit_ground.as_deref().map_or(0, ocl_code));
        put_u32(&mut v, jd.throw_force.to_bits());
        put_u32(&mut v, jd.percent_damage_to_passengers.to_bits());
        put_u32(&mut v, jd.empty_hulk_destruction_delay);
    }
    let n_mods = cfg[3] as usize;
    put_u32(&mut v, n_mods as u32);

    let mut w = World::new();
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    let mut scratch = rts_lockstep_engine::common::name_key_generator::NameKeyGenerator::new();
    scratch.reset();
    for n in WEAPONS {
        weapon_store.new_weapon_template(&mut scratch, n.as_bytes()).unwrap();
    }
    w.names = ctx.names;
    // the object creation lists are keyed in the world's own name key generator
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, object_creation_list_store: ocls, weapon_store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[35]);
    w.cuts.globals.slow_death_scale = f32::from_bits(cfg[1]);
    w.cuts.globals.hulk_max_lifetime_override = cfg[2] as i32;
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.contain.stub_object_calls = true;
    let mut kinds = vec![];
    if cfg[4] & 1 != 0 {
        kinds.push(KINDOF_INFANTRY);
    }
    if cfg[4] & 2 != 0 {
        kinds.push(KINDOF_HULK);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { kind_of: kinds, ..Default::default() }));
    let flags = cfg[5];

    let mut modules = Vec::new();
    for k in 0..n_mods {
        let d = if k == 0 {
            data.clone()
        } else {
            let o = &cfg[13 + 11 * (k - 1)..];
            let mut x = SlowDeathBehaviorModuleData { probability_modifier: o[0] as i32, ..Default::default() };
            x.modifier_bonus_per_overkill_percent = f32::from_bits(o[1]);
            x.sink_rate = f32::from_bits(o[2]);
            x.sink_delay = o[3];
            x.sink_delay_variance = o[4];
            x.destruction_delay = o[5];
            x.destruction_delay_variance = o[6];
            x.fling_force = f32::from_bits(o[7]);
            x.fling_force_variance = f32::from_bits(o[8]);
            x.fling_pitch = f32::from_bits(o[9]);
            x.fling_pitch_variance = f32::from_bits(o[10]);
            ModuleDataKind::SlowDeathBehavior(Arc::new(x))
        };
        let name: &[u8] = if k == 0 { b"BattleBusSlowDeathBehavior" } else { b"SlowDeathBehavior" };
        modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, format!("ModuleTag_SD{k}").as_bytes(), &d).unwrap().unwrap());
    }
    let probe = |name: &[u8], answers: ProbeAnswers, max: f32| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        pm
    };
    let mut body_idx = None;
    if flags >> 4 & 1 != 0 {
        let mut pm = probe(b"SdBody", ProbeAnswers { no_death: true, ..Default::default() }, f32::from_bits(cfg[7]));
        pm.interfaces = MODULEINTERFACE_BODY;
        body_idx = Some(modules.len());
        modules.push(pm);
    }
    let mut ai_idx = None;
    if flags >> 1 & 1 != 0 {
        let mut pm = probe(b"SdAi", ProbeAnswers { ai_dead: flags >> 6 & 1 != 0, ..Default::default() }, 0.0);
        pm.is_ai = true;
        ai_idx = Some(modules.len());
        modules.push(pm);
    }
    let mut phys_idx = None;
    if flags >> 2 & 1 != 0 {
        phys_idx = Some(modules.len());
        modules.push(probe(b"PhysicsBehavior", ProbeAnswers { collidee: cfg[11] as i32, trace_throw: true, ..Default::default() }, 0.0));
    }
    let contained = |n: u32| (0..n as i32).map(|i| 100 + i).collect::<Vec<_>>();
    // (always made; the object's contain module is it or none, as the scene says)
    let contain_idx = modules.len();
    let mut pm = probe(b"BbContain", ProbeAnswers { trace_throw: true, contained: contained(cfg[CFG_BUS + 1]), ..Default::default() }, 0.0);
    pm.interfaces = MODULEINTERFACE_CONTAIN;
    modules.push(pm);
    if flags >> 3 & 1 != 0 {
        modules.push(probe(b"SlavedUpdate", ProbeAnswers::default(), 0.0));
    }
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let me = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    // the helper modules the world makes come first: module indices are relative to the first slow death behaviour
    let base = w.find_module(me, b"BattleBusSlowDeathBehavior").expect("the module") as usize;
    if flags >> 7 & 1 != 0 {
        let tree_template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { kind_of: vec![KINDOF_SHRUBBERY], ..Default::default() }));
        let tree = w.new_object(NewObject { template: tree_template, status: Default::default(), team: None, modules: Vec::new() }).unwrap();
        assert_eq!(tree, 2);
    }
    {
        let o = w.objects.find_mut(me).unwrap();
        o.drawable = if flags & 1 != 0 { 7 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        o.experience.level = veterancy(cfg[6]);
        for b in 0..8 {
            if cfg[12] >> b & 1 != 0 {
                o.status.set(b, 1);
            }
        }
        if flags >> 5 & 1 != 0 {
            o.disabled.set(DISABLED_HELD, 1);
        }
        o.contain = (cfg[CFG_BUS] & 1 != 0).then_some((base + contain_idx) as u16);
    }
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[8]), y: f32::from_bits(cfg[9]), z: f32::from_bits(cfg[10]) });
    let first_trace = w.cuts.trace.take().unwrap_or_default();
    let mut first_trace = Some(first_trace);
    let first_events = w.events.events().len();
    let _ = first_events;
    let mut phys = Physics::default();
    let mut ai_dead = flags >> 6 & 1 != 0;
    let mut saved: Vec<u8> = Vec::new();
    let mut was_first = false;
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        let mut cuts_before = w.cuts.log.as_ref().map_or(0, |l| l.len());
        w.cuts.trace = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut sleep, mut result) = (-2i32, Vec::new(), 0u32, 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            let k = o[1] as usize % n_mods;
            let mod_idx = (base + k) as u16;
            match o[0] {
                0 => {
                    let u = w.update_handle_of_module(ModuleHandle { object: me, index: mod_idx }).expect("an update module");
                    sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    let info = info_of(cfg, o[1]);
                    for j in 0..n_mods {
                        w.module_call(me, (base + j) as u16, ModuleCall::OnDie(info.clone()));
                    }
                    tally.borrow_mut().deaths += 1;
                }
                2 => {
                    w.module_call(me, mod_idx, ModuleCall::OnDie(info_of(cfg, o[2])));
                    tally.borrow_mut().deaths += 1;
                }
                3 => {
                    w.module_call(me, mod_idx, ModuleCall::BeginSlowDeath(info_of(cfg, o[2])));
                }
                4 => {
                    result = match w.module_call(me, mod_idx, ModuleCall::GetProbabilityModifier(info_of(cfg, o[2]))) {
                        ModuleReply::Int(i) => i as u32,
                        _ => 0,
                    };
                }
                5 => result = u32::from(w.module_call(me, mod_idx, ModuleCall::IsDieApplicable(info_of(cfg, o[2]))).as_bool()),
                6 => w.set_frame(o[1]),
                7 => w.cuts.globals.slow_death_scale = f32::from_bits(o[1]),
                8 => w.cuts.globals.hulk_max_lifetime_override = o[1] as i32,
                9 => w.set_position(me, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
                10 => {}
                11 => w.objects.find_mut(me).unwrap().disabled.set(DISABLED_HELD, i32::from(o[1] != 0)),
                12 => {
                    if let Some(i) = phys_idx {
                        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[base + i].body.as_mut() {
                            p.answers.collidee = o[1] as i32;
                        }
                    }
                }
                13 => {
                    ai_dead = o[1] != 0;
                    if let Some(i) = ai_idx {
                        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[base + i].body.as_mut() {
                            p.answers.ai_dead = ai_dead;
                        }
                    }
                }
                14 => w.objects.find_mut(me).unwrap().drawable = if o[1] != 0 { 7 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID },
                15 => w.objects.find_mut(me).unwrap().experience.level = veterancy(o[1]),
                16 => {
                    if let Some(i) = body_idx {
                        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[base + i].body.as_mut() {
                            p.max_health = f32::from_bits(o[1]);
                        }
                    }
                }
                17 => {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.status = Default::default();
                    for b in 0..8 {
                        if o[1] >> b & 1 != 0 {
                            ob.status.set(b, 1);
                        }
                    }
                }
                21 => {
                    if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[base + contain_idx].body.as_mut() {
                        p.answers.contained = contained(o[1]);
                    }
                }
                22 => w.objects.find_mut(me).unwrap().contain = (o[1] != 0).then_some((base + contain_idx) as u16),
                18..=20 => {
                    let r = run_xfer(&mut w, me, base + k, o[0] - 18, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 18 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
            // an op that is the test's own is not an effect of the module
            if (6..=17).contains(&o[0]) || (21..=22).contains(&o[0]) {
                events_before = w.events.events().len();
                cuts_before = w.cuts.log.as_ref().map_or(0, |l| l.len());
            }
        } else {
            events_before = 0;
            cuts_before = 0;
        }
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let cuts: Vec<CutCall> = w.cuts.log.as_ref().map_or(Vec::new(), |l| l[cuts_before..].to_vec());
        let trace = if phase == 0 { first_trace.take().unwrap_or_default() } else { w.cuts.trace.take().unwrap() };
        let (mut log, mut wakes, mut last_mod, mut last_frame) = (Vec::new(), 0u32, -1i32, 0u32);
        let (mut cut_iter, mut ev_iter) = (cuts.into_iter(), events.into_iter());
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                let i = i.parse::<i32>().unwrap() - base as i32;
                // (the helper modules the world makes wake themselves too: not the driver's modules)
                if i >= 0 && (i as usize) < n_mods {
                    wakes += 1;
                    last_mod = i;
                    last_frame = f.parse().unwrap();
                }
            } else if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => log.push(format!("setdisabled {object} {disabled_type}")),
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        tally.borrow_mut().destroyed += 1;
                        log.push(format!("destroy {object}"))
                    }
                    Some(CutCall::GameLogicDeselectObject { object, player_mask, affect_client }) => log.push(format!("deselect {object} {player_mask} {}", i32::from(affect_client))),
                    Some(CutCall::OclCreate { ocl, .. }) => {
                        tally.borrow_mut().phase_ocl += 1;
                        log.push(format!("ocl {}", ocl_code(&ocl) - 1))
                    }
                    Some(CutCall::Contain(ContainCall::TerrainGetLayerForDestination { position: p })) => {
                        log.push(format!("destlayer {:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))
                    }
                    Some(CutCall::Contain(ContainCall::SetLayer { object, layer })) => log.push(format!("setlayer {object} {layer}")),
                    Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", x.to_bits(), y.to_bits())),
                    Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position }) => {
                        tally.borrow_mut().phase_weapon += 1;
                        let name = WEAPONS[weapon_code(&weapon) as usize - 1];
                        log.push(format!("weapon {name} {source} {},{},{}", position.x.to_bits(), position.y.to_bits(), position.z.to_bits()))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") && phase > 0 {
                match ev_iter.next() {
                    Some(ObjectClientEvent::FxListDoFxObj { fx, .. }) => {
                        tally.borrow_mut().phase_fx += 1;
                        log.push(format!("fx {}", String::from_utf8_lossy(&fx)))
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(f), .. }) => log.push(format!("mc {}", f[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(f), .. }) => log.push(format!("mcclear {}", f[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. }) => log.push(format!("clearset {} {}", clear[0], set[0])),
                    Some(ObjectClientEvent::DrawableSetShadowsEnabled { enabled, .. }) => log.push(format!("shadows {}", i32::from(enabled))),
                    Some(ObjectClientEvent::DrawableTerrainDecalFadeTarget { target, rate, .. }) => log.push(format!("decalfade {:08x} {:08x}", target.to_bits(), rate.to_bits())),
                    _ => {}
                }
            } else if t.starts_with("damage ") {
                tally.borrow_mut().kills += 1;
                log.push(t);
            } else if let Some(line) = t.strip_prefix("bt ") {
                let tok: Vec<&str> = line.split(' ').collect();
                match tok[0] {
                    "pfall" => phys.fall = tok[2] == "1",
                    "pallowbounce" => phys.bounce = tok[2] == "1",
                    "pbounce" => phys.extra_bounciness = u32::from_str_radix(tok[2], 16).unwrap(),
                    "pfriction" => phys.extra_friction = u32::from_str_radix(tok[2], 16).unwrap(),
                    "markdead" => {
                        ai_dead = true;
                        log.push(line.to_string())
                    }
                    "slaverdie" | "pforce" | "pangles" | "ppitch" | "aiidle" | "prandrot" | "pdmgcontained" => log.push(line.to_string()),
                    "pshock" => {
                        tally.borrow_mut().undeaths += 1;
                        log.push(line.to_string())
                    }
                    "pscrub" => {
                        tally.borrow_mut().scrubs += 1;
                        log.push(line.to_string())
                    }
                    _ => {}
                }
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        if let Some((_, e)) = w.cuts.module_errors.first() {
            let mut t = tally.borrow_mut();
            let excluded = std::mem::take(&mut t.excluded);
            *t = tally_before;
            t.excluded = excluded;
            *t.excluded.entry(format!("{e:?}").chars().take(60).collect()).or_insert(0) += 1;
            return RunOut { dump: Vec::new(), keys: Vec::new(), excluded: true };
        }
        if phase > 0 {
            put_i32(&mut v, 0);
            put_u32(&mut v, result);
            put_u32(&mut v, sleep);
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_i32(&mut v, if wakes > 0 { last_mod } else { -1 });
        put_u32(&mut v, if wakes > 0 { last_frame } else { 0 });
        for j in 0..n_mods {
            let words = match w.peek_module_body(ModuleHandle { object: me, index: (base + j) as u16 }) {
                Some(UpdateBody::SlowDeathBehavior(m)) => m.state_words(),
                Some(UpdateBody::BattleBusSlowDeathBehavior(m)) => m.state_words()[..5].to_vec(),
                _ => panic!("the module {j} is in its slot (phase {phase}, {n_mods} modules, op {:?})", ops.get(phase.wrapping_sub(1))),
            };
            for x in words {
                put_u32(&mut v, x);
            }
        }
        if phase > 0 {
            put_i32(&mut v, xst);
            put_u32(&mut v, xbytes.len() as u32);
            for b in &xbytes {
                put_u32(&mut v, u32::from(*b));
            }
            put_u32(&mut v, crc);
        }
        let ob = w.objects.find(me).unwrap();
        let pos = ob.get_position();
        put_u32(&mut v, pos.x.to_bits());
        put_u32(&mut v, pos.y.to_bits());
        put_u32(&mut v, pos.z.to_bits());
        put_u32(&mut v, if ob.is_disabled_by_type(DISABLED_HELD) { 1 << DISABLED_HELD } else { 0 });
        put_u32(&mut v, u32::from(ai_dead));
        put_u32(&mut v, u32::from(phys.fall));
        put_u32(&mut v, u32::from(phys.bounce));
        put_u32(&mut v, phys.extra_bounciness);
        put_u32(&mut v, phys.extra_friction);
        let sd_state: Vec<u32> = match w.peek_module_body(ModuleHandle { object: me, index: base as u16 }) {
            Some(UpdateBody::BattleBusSlowDeathBehavior(m)) => m.state_words(),
            _ => Vec::new(),
        };
        for x in &sd_state[5..] {
            put_u32(&mut v, *x);
        }
        // a landing: the first death over, the death not real
        let first = sd_state.get(6) == Some(&1);
        if was_first && !first && sd_state.get(5) == Some(&0) {
            tally.borrow_mut().landings += 1;
        }
        was_first = first;
        if sd_state.get(4).is_some_and(|f| f & 1 != 0) && phase > 0 {
            tally.borrow_mut().begun += 1;
        }
        if sd_state.get(4).is_some_and(|f| f & 4 != 0) {
            tally.borrow_mut().flung += 1;
        }
    }
    RunOut { dump: v, keys: Vec::new(), excluded: false }
}

#[test]
fn battle_bus_slow_death_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R47 BattleBusSlowDeathBehavior",
        seed: 0x0047_BB5D,
        cases_var: "L9R47_CASES",
        ini_stem: "Bb",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r47_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R47: {} cases, {} data blocks threw, {} excluded, {} ops, {} updates, {} deaths, {} begun, {} flung, {} destroys, {} xfers, {} effect lines, {} fx, {} ocl, {} weapon, \
         {} undeaths, {} landings, {} scrubs, {} kills",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.deaths, t.begun, t.flung, t.destroyed, t.xfers, t.effects, t.phase_fx, t.phase_ocl, t.phase_weapon,
        t.undeaths, t.landings, t.scrubs, t.kills
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(
        c.read_failed * 40 > c.cases as u64 && t.begun * 10 > c.cases as u64 && t.undeaths * 10 > c.cases as u64 && t.landings * 20 > c.cases as u64 && t.scrubs * 10 > c.cases as u64 && t.kills * 40 > c.cases as u64,
        "the generator must reach every path"
    );
}
