#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SpectreGunshipUpdate` (`logic::modules::spectre_gunship_update`) against the unchanged C++, `p1w13-sonnet`, batch R72.
//!
//! # Method
//!
//! The C++ side is `SpectreGunshipUpdate.cpp`, unchanged but for its particle system parser (and the name of its file-static `zero`, which
//! the unit's `SpectreGunshipDeploymentUpdate.cpp` also defines; both edits by `build.rs`), compiled into `wrap_module.cpp`'s unit
//! (`differential/l9_spectre_gunship_r72.inc`, `ref_l9r72_run`), on the module build's stand-ins: the gunship (a team of three with their
//! players, human or computer, maybe the local one; maybe an AI whose orders and locomotor calls are lines and whose current locomotor may be
//! missing; maybe a drawable; maybe a contain module whose `addToContain` is a line; maybe a REAL `SpecialPowerModule` of `SP_0`..`SP_3`
//! whose `setReadyFrame` and `markSpecialPowerTriggered` are lines; disabled, effectively dead, off the map or not; its shroud status for the
//! local player; a position and an orientation), four other objects (a team, effectively dead, off the map or not, a position), the
//! gattlings (`GattlingA` with an AI, `GattlingB` without; firing or not), the teams' relationships, the scan (the objects of the test's list
//! within the range, nearest first; the file's own `PartitionFilterLiveMapEnemies` applied for real), the terrain's extent and ground
//! heights, the weapon store (`WEAP_A`, `WEAP_B`: `createAndFireTempWeapon` a line), the particle systems, the decals, the audio (the
//! playing answer the test's), `setDisabled`, `clearDisabled` and `destroyObject` (lines), the REAL special power store, `UpdateModule`,
//! `INI`, `Xfer` and the logic and client random values.
//!
//! The Rust side is the real thing: a `World` with the special power store, the weapon store and the thing factory's two gattlings (made by
//! the real `World::thing_factory_new_object`, `GattlingA`'s AI a probe); the gunship's AI, contain and power modules probes; the teams,
//! players, relationships, local player and shroud status as the C++'s; the scan the cut point [`World::partition_iterate_spectre_targets`]
//! over `cuts.range_scan`; `setDisabled`, `clearDisabled` and `destroyObject` records (`cuts.body.stub_object_calls`). Every op an `update()`,
//! `initiateIntentToDoSpecialPower` (a template, a target position or none, the script flag and other options),
//! `setSpecialPowerOverridableDestination`, the queries, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data, the disabled types the module processes and the constructor's state; per op the throw status, the result, the effects in order
//! (the AI's orders and locomotor, the doors and afterburner model conditions, the sounds, the gattling's making, paralysis and end, the
//! decals, the scans, the howitzer, the strafing smoke and its ground heights, the power's calls, the destroy), the xfer status, bytes (the
//! scheduling word masked) and CRC, the module's state, and the number of gattlings with the last one's team.
//!
//! The Rust gunship is made before the scene places it, so `onObjectCreated` (which keeps the gunship's position as the satellite's) is
//! called again once it is placed; it does nothing else.
//!
//! # Results (2026-10-26, `L9R72_CASES=9000`)
//!
//! 9,000 cases (558 data blocks threw, as the C++ did; 1,620 excluded as undefined upstream: 840 an intent with the gunship's power module
//! never found, 239 an AI with no current locomotor, 234 a zero `HowitzerFiringRate`, 220 a `NULL` target position, 44 no local player
//! while strafing, 43 no controlling player when picking a target), 199,803 ops and 420,072 outputs: **0 mismatches**. In the compared
//! cases: 80,314 updates, 32,156 xfers and 624,936 effect lines; 25,593 intents, 22,929 taken; 2,332 orbit insertions and 281 departures;
//! 6,112 scans, 2,031 gattling attacks, 269 howitzer shots, 1,514 strafing smoke systems; 14,971 gattlings made, 4,444 destroys and 12,015
//! moves of the reticle. The first runs differed only in the test's own bookkeeping (the Rust world's team bookkeeping of a new gattling;
//! "no local player" set as the neutral player, `setLocalPlayer(NULL)`'s meaning, where the driver has none).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 17 of 17 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2421 | the live-map-enemies filter ignores the map status | 184 |
//! | 2422 | the satellite not placed at the gunship when it is created | 174,247 |
//! | 2423 | the strafing point not set by a player's intent | 143,508 |
//! | 2424 | the new gattling not paralysed | 14,971 |
//! | 2425 | the reticle decal drawn with the attack area's radius | 23,652 |
//! | 2426 | no attack voice when the local player moves the reticle | 19,359 |
//! | 2427 | "a fair distance" half the orbit radius, not three quarters | 540 |
//! | 2428 | `cleanUp` leaves the gattling | 2,132 |
//! | 2429 | the insertion slope's two weights equal | 156,562 |
//! | 2430 | the reticle not kept within the attack area | 28,868 |
//! | 2431 | the gattling stays paralysed in orbit | 3,206 |
//! | 2432 | no wide scan for a computer player | 3,092 |
//! | 2433 | the howitzer's y offset the x's draw | 229 |
//! | 2434 | the strafing point on target does not count towards the howitzer | 3,747 |
//! | 2435 | the departing gunship never leaves | 3,456 |
//! | 2436 | version 2's fields not saved | 36,004 |
//! | 2437 | the overridable destination active while departing | 2,282 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, Region2D};
use rts_lockstep_engine::common::disabled_types::{DISABLED_COUNT, DISABLED_HELD};
use rts_lockstep_engine::common::game_common::{ObjectShroudStatus, PlayerType, Relationship};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_IS_FIRING_WEAPON;
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::audio_event_name::AudioEventName;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::module::behavior_module::BehaviorModuleData;
use rts_lockstep_engine::logic::modules::spectre_gunship_deployment_update::COMMAND_FIRED_BY_SCRIPT;
use rts_lockstep_engine::logic::modules::spectre_gunship_update::SpectreGunshipUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{PRIVATE_EFFECTIVELY_DEAD, PRIVATE_OFF_MAP};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{GridDecalOp, ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r72.bin");
const XFER_FILE: &str = "l9_r72.bin";

const POWERS: &str = "SpecialPower SP_0\nEnd\nSpecialPower SP_1\nEnd\nSpecialPower SP_2\nEnd\nSpecialPower SP_3\nEnd\n";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpectreGunshipUpdate(Arc::new(SpectreGunshipUpdateModuleData::friend_new_module_data(Some(ini))?)));
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

/// The driver's weapon code: 0 `NULL`, 1 `WEAP_A`, 2 `WEAP_B`.
fn weapon_code(name: &Option<Vec<u8>>) -> u32 {
    match name.as_deref() {
        Some(n) if n.eq_ignore_ascii_case(b"WEAP_A") => 1,
        Some(n) if n.eq_ignore_ascii_case(b"WEAP_B") => 2,
        _ => 0,
    }
}

const STATE_WORDS: usize = 1 + 15 + 6;

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let (sp, weapon) = (r.u32(), r.u32());
    let gattling = r.str();
    let nums: Vec<u32> = (0..9).map(|_| r.u32()).collect();
    let ps = r.str();
    let mut decals = Vec::new();
    for _ in 0..2 {
        let name = r.str();
        let rest: Vec<u32> = (0..6).map(|_| r.u32()).collect();
        decals.push(format!("{name:?} {rest:x?}"));
    }
    let mask = r.u32();
    let n = r.u32();
    let log: Vec<String> = (0..n).map(|_| r.str()).collect();
    let state: Vec<u32> = (0..STATE_WORDS).map(|_| r.u32()).collect();
    out.push(format!("data sp {sp} weapon {weapon} gattling {gattling:?} {nums:x?} ps {ps:?} decals {decals:?}"));
    out.push(format!("constructed mask {mask:#x} log {log:?} state {state:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..STATE_WORDS).map(|_| r.u32()).collect();
        let (gattlings, team) = (r.u32(), r.u32());
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        out.push(format!("op {i} state {state:x?} gattlings {gattlings} last team {team}"));
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
    field(g, 97, "SpecialPowerTemplate", &["SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_1"], true);
    field(g, 90, "GattlingTemplateName", &["GattlingA", "GattlingA", "GattlingA", "GattlingA", "GattlingB", "NoSuchGun"], true);
    field(g, 80, "HowitzerFiringRate", &["33", "33", "66", "100", "0"], false);
    field(g, 70, "OrbitTime", &["1000", "3000", "300", "0"], false);
    field(g, 70, "HowitzerFollowLag", &["0", "0", "33", "100"], false);
    field(g, 50, "AttackAreaRadius", &["200", "150", "40", "300.5"], false);
    field(g, 50, "TargetingReticleRadius", &["25", "60", "10.5"], false);
    field(g, 50, "GunshipOrbitRadius", &["250", "100", "50", "400"], false);
    field(g, 50, "StrafingIncrement", &["20", "5", "100", "0.5"], false);
    field(g, 50, "OrbitInsertionSlope", &["0.7", "0.3", "0.9", "0.55"], false);
    field(g, 50, "RandomOffsetForHowitzer", &["20", "7.5", "0", "-4.5"], false);
    field(g, 70, "HowitzerWeaponTemplate", &["WEAP_A", "WEAP_A", "WEAP_B", "None", "weap_b"], true);
    field(g, 75, "GattlingStrafeFXParticleSystem", &["PS_A", "PS_A", "PS_B", "None"], true);
    if g.below(100) < 75 {
        lines.push(format!("AttackAreaDecal\n Texture = {}\n Style = SHADOW_ALPHA_DECAL\n OpacityMin = 25%\nEnd", pick(g, &["Ring", "Area"])));
    }
    if g.below(100) < 60 {
        lines.push("TargetingReticleDecal\n Texture = Reticle\n OnlyVisibleToOwningPlayer = No\nEnd".to_string());
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
    f(*pick(g, &[&0.0f32, &12.5, &-20.0, &60.0, &-130.0, &180.0, &240.0, &-310.0, &450.0, &600.0]))
}

fn gen_near(g: &mut Gen) -> u32 {
    f(*pick(g, &[&0.0f32, &5.0, &-12.5, &20.0, &-40.0, &75.0, &150.0, &-220.0]))
}

/// The gunship's flags (cfg[4]).
fn gen_flags(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let team = if g.below(25) == 0 { 0 } else { 1 + g.below(3) };
    let shroud = if g.below(6) == 0 { *pick(g, &[&0u32, &3, &4, &5]) } else { 1 + g.below(2) };
    team | p(g, 6) << 2 | p(g, 3) << 3 | p(g, 4) << 4 | shroud << 5 | p(g, 70) << 8 | p(g, 50) << 9
}

/// Objects 2-5 (cfg[9]): a team (mostly an enemy's), dead or off the map now and then.
fn gen_others(g: &mut Gen) -> u32 {
    let mut v = 0;
    for k in 0..4 {
        let team = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
        let b = team | u32::from(g.below(12) == 0) << 2 | u32::from(g.below(12) == 0) << 3;
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

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        4 => gen_flags(g),
        5 | 6 => gen_coord(g),
        7 => f(*pick(g, &[&0.0f32, &80.0])),
        8 => f(*pick(g, &[&0.0f32, &0.5, &1.5707964, &-2.5, &3.0])),
        9 => gen_others(g),
        10..=17 => gen_near(g),
        18 => gen_scan(g),
        19 => g.below(8) | (if g.below(8) == 0 { 3 } else { g.below(3) }) << 4,
        20 => {
            // mostly enemies across teams, allies within
            let mut v = 0;
            for a in 0..3 {
                for b in 0..3 {
                    let r = if a == b { 2 } else if g.below(6) == 0 { g.below(3) } else { 0 };
                    v |= r << (2 * (3 * a + b));
                }
            }
            v
        }
        21 | 22 => f(*pick(g, &[&-500.0f32, &-500.0, &-1000.0, &-100.0])),
        23 | 24 => f(*pick(g, &[&500.0f32, &500.0, &1000.0, &100.0])),
        25 => g.u32(),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // the data's power (SP_k: k + 1), which the gunship's power module mostly has
    let own = match data {
        Some(ModuleDataKind::SpectreGunshipUpdate(d)) => match d.special_power_template.as_deref() {
            Some(b"SP_0") => 1,
            Some(b"SP_1") => 2,
            _ => 1,
        },
        _ => 1,
    };
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &99, &100]);
    cfg[1] = g.u32();
    cfg[2] = g.u32();
    // the gunship's parts: mostly an AI with a locomotor, a drawable, a contain and SP_0's power
    let power = if g.below(25) == 0 { 0 } else if g.below(15) == 0 { 1 + g.below(4) } else { own };
    cfg[3] = u32::from(g.below(15) != 0) | u32::from(g.below(25) != 0) << 1 | u32::from(g.below(6) != 0) << 2 | u32::from(g.below(8) != 0) << 3 | power << 4;
    for a in 4..=25u32 {
        cfg[a as usize] = gen_cfg_word(g, a);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=39 => [0, 0, 0, 0, 0],
            40..=46 => [1, cfg[0] + g.below(200), 0, 0, 0],
            47..=61 => {
                let a = *pick(g, &[&4u32, &4, &5, &6, &5, &6, &8, &9, &10, &11, &12, &13, &18, &18, &19, &20, &21, &23, &25]);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            62..=74 => {
                let k = if g.below(8) == 0 { *pick(g, &[&0u32, &1, &2, &3, &7]) } else { own };
                let flags = u32::from(g.below(120) != 0) | u32::from(g.below(5) == 0) << 1 | (if g.below(6) == 0 { 1 + g.below(3) } else { 0 }) << 2;
                [3, k << 8 | flags, gen_near(g), gen_near(g), f(*pick(g, &[&0.0f32, &3.0]))]
            }
            75..=80 => [4, gen_near(g), gen_near(g), f(*pick(g, &[&0.0f32, &7.0])), 0],
            81..=86 => [5, 0, 0, 0, 0],
            87..=93 => [6, 0, 0, 0, 0],
            94..=96 => [7, 0, 0, 0, 0],
            _ => [8, 0, 0, 0, 0],
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
    orbits: u64,
    departures: u64,
    scans: u64,
    howitzers: u64,
    smoke: u64,
    gattlings: u64,
    attacks: u64,
    destroys: u64,
    overrides: u64,
    excluded: BTreeMap<String, u64>,
}

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
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

fn set_scene(w: &mut World, cfg: &[u32], drawable: rts_lockstep_engine::common::game_type::DrawableId) {
    let fl = cfg[4];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = fl & 3;
        o.disabled = Default::default();
        if fl >> 2 & 1 != 0 {
            o.disabled.set(DISABLED_HELD, 1);
        }
        set_flag(o, PRIVATE_EFFECTIVELY_DEAD, fl >> 3 & 1 != 0);
        set_flag(o, PRIVATE_OFF_MAP, fl >> 4 & 1 != 0);
        o.drawable = if cfg[3] >> 2 & 1 != 0 { drawable } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        o.in_partition = true;
    }
    let ids: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 5).collect();
    for id in ids {
        let o = w.objects.find_mut(id).unwrap();
        o.status.set(OBJECT_STATUS_IS_FIRING_WEAPON, (fl >> 8 & 1) as i32);
    }
    w.cuts.audio_playing = fl >> 9 & 1 != 0;
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[5]), y: f32::from_bits(cfg[6]), z: f32::from_bits(cfg[7]) });
    w.set_orientation(1, f32::from_bits(cfg[8]));
    for k in 1..5u32 {
        let b = cfg[9] >> (8 * (k - 1)) & 255;
        {
            let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
            o.team = b & 3;
            set_flag(o, PRIVATE_EFFECTIVELY_DEAD, b >> 2 & 1 != 0);
            set_flag(o, PRIVATE_OFF_MAP, b >> 3 & 1 != 0);
        }
        let i = (10 + 2 * (k - 1)) as usize;
        w.set_position(k as ObjectId + 1, &Coord3D { x: f32::from_bits(cfg[i]), y: f32::from_bits(cfg[i + 1]), z: 0.0 });
    }
    w.cuts.range_scan = (0..4).map(|k| cfg[18] >> (4 * k) & 15).take_while(|&id| (2..=5).contains(&id)).map(|id| id as ObjectId).collect();
    for p in 0..3 {
        w.players.get_nth_mut(p).unwrap().player_type = if cfg[19] >> p & 1 != 0 { PlayerType::Human } else { PlayerType::Computer };
    }
    let local = cfg[19] >> 4 & 3;
    w.players.friend_set_local_index((local < 3).then_some(local as i32));
    w.cuts.shrouded_status.clear();
    if local < 3 {
        w.cuts.shrouded_status.insert((1, local as i32), shroud(fl >> 5 & 7));
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[20] >> (2 * (3 * a + b)) & 3));
        }
    }
    let c = |i: usize| f32::from_bits(cfg[i]);
    w.cuts.extent_including_border = Region2D { lo: Coord2D { x: c(21), y: c(22) }, hi: Coord2D { x: c(23), y: c(24) } };
    for (k, h) in w.cuts.terrain_ground.iter_mut().enumerate() {
        *h = (cfg[25] >> (4 * k) & 15) as f32 * 0.5;
    }
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
                    tally.smoke += 1;
                    let name = String::from_utf8_lossy(names.get(&id).map_or(&[][..], |n| &n[..])).to_string();
                    log.push(format!("psys {name} {id} {:08x},{:08x},{:08x}", b(p.x), b(p.y), b(p.z)))
                }
                Some(CutCall::PartitionIterateSpectreTargets { position: p, range, player_index, .. }) => {
                    tally.scans += 1;
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 4 {player_index} 2", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) => {
                    tally.howitzers += 1;
                    log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))
                }
                Some(CutCall::AudioIsCurrentlyPlaying { handle }) => log.push(format!("isplaying {handle}")),
                Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => log.push(format!("setdisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::ClearDisabled { object, disabled_type })) => log.push(format!("cleardisabled {object} {disabled_type}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    tally.destroys += 1;
                    log.push(format!("destroy {object}"))
                }
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
                Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(v), object_id }) if object_id <= 5 => log.push(format!("mc {}", v[0])),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(v), object_id }) if object_id <= 5 => log.push(format!("mcclear {}", v[0])),
                Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::ClearAndSet { clear, set } }) if object_id <= 5 => {
                    log.push(format!("mc {object_id} clear {clear:?} set {set:?}"))
                }
                // (a new object's drawable starting its model condition: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { .. }) => {}
                Some(ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id > 5 => {}
                Some(ObjectClientEvent::GridDecal { index, op, .. }) => log.push(match op {
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
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("aiattack") {
                tally.attacks += 1;
            }
            if l.starts_with("addtocontain ") {
                tally.gattlings += 1;
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
    let ModuleDataKind::SpectreGunshipUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.special_power_template.as_deref().map_or(0, |n| template_id(&ctx.store, n)));
    put_u32(&mut v, weapon_code(&d.howitzer_weapon_template));
    put_str(&mut v, &d.gattling_template_name);
    for x in [d.orbit_frames, d.howitzer_firing_rate, d.howitzer_follow_lag] {
        put_u32(&mut v, x);
    }
    for x in [d.attack_area_radius, d.targeting_reticle_radius, d.gunship_orbit_radius, d.strafing_increment, d.orbit_insertion_slope, d.random_offset_for_howitzer] {
        put_u32(&mut v, x.to_bits());
    }
    put_str(&mut v, d.gattling_strafe_fx_particle_system.as_deref().unwrap_or(b""));
    for t in [&d.attack_area_decal_template, &d.targeting_reticle_decal_template] {
        put_str(&mut v, &t.name);
        put_u32(&mut v, t.shadow_type);
        put_u32(&mut v, t.min_opacity.to_bits());
        put_u32(&mut v, t.max_opacity.to_bits());
        put_u32(&mut v, t.opacity_throb_time);
        put_u32(&mut v, t.color as u32);
        put_u32(&mut v, u32::from(t.only_visible_to_owning_player));
    }

    let store = ctx.store.clone();
    let mut w = World::new();
    w.names = ctx.names;
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for n in WEAPONS {
        weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap().unwrap();
    }
    let mut factory = ThingFactory::new();
    for name in ["GattlingA", "GattlingB"] {
        factory.new_template(name.as_bytes(), 0).expect("a template");
    }
    let plain = || ModuleInfoData::Logic(ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::default())));
    factory.friend_find_template_mut(b"GattlingA").unwrap().data.friend_behavior_module_info_mut().add_module_info(
        b"R72GattlingAIUpdate",
        b"ModuleTag_AI",
        plain(),
        0,
        false,
        false,
    );
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, weapon_store, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_power_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.untranslated_probe = Some(ProbeAnswers::default());
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let values = ObjectTemplateValues { name: b"R72Gunship".to_vec(), audio: vec![(ThingTemplateAudioType::VoiceAttack, b"R72Voice".to_vec())], ..Default::default() };
    let mut template = ThingTemplate::from_object_values(&values);
    template.friend_set_per_unit_sound(b"Afterburner", AudioEventName { event_name: b"R72Burner".to_vec() });
    template.friend_set_per_unit_sound(b"HowitzerFire", AudioEventName { event_name: b"R72Howitzer".to_vec() });
    let template = Arc::new(template);
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let mut modules = Vec::new();
    let power = cfg[3] >> 4 & 7;
    if power_id(&store, power) != 0 {
        modules.push(probe(b"R72Power", ProbeAnswers { power_template: power_id(&store, power), ..Default::default() }, MODULEINTERFACE_SPECIAL_POWER));
    }
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SpectreGunshipUpdate", b"ModuleTag_Gunship", &data).unwrap().unwrap());
    if cfg[3] & 1 != 0 {
        let mut ai = probe(b"R72Ai", ProbeAnswers { has_locomotor: cfg[3] >> 1 & 1 != 0, ..Default::default() }, 0);
        ai.is_ai = true;
        modules.push(ai);
    }
    if cfg[3] >> 3 & 1 != 0 {
        modules.push(probe(b"R72Contain", ProbeAnswers { trace_contain: true, ..Default::default() }, MODULEINTERFACE_CONTAIN));
    }
    let me = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let drawable = w.objects.find(me).unwrap().drawable;
    for k in 2..=5 {
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules: Vec::new() }).unwrap();
        assert_eq!(id, k);
    }
    let idx = w.find_module(me, b"SpectreGunshipUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, drawable);
    // (the Rust gunship was made before the scene placed it: onObjectCreated again, now that it is where the C++'s was)
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SpectreGunshipUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SpectreGunshipUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is in its slot") };
        let mask = m.update_module().get_disabled_types_to_process();
        put_u32(&mut v, (0..DISABLED_COUNT).filter(|&k| mask.test(k)).fold(0, |a, k| a | 1 << k));
    }
    put_u32(&mut v, 0);
    for x in state(&w) {
        put_u32(&mut v, x);
    }
    let mut names = BTreeMap::new();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (4..=25).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, drawable);
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
                let command_options = (if o[1] >> 1 & 1 != 0 { COMMAND_FIRED_BY_SCRIPT } else { 0 }) | (o[1] & 0xff) >> 2;
                let call = ModuleCall::InitiateIntentToDoSpecialPower { template, target_object: 0, target_position, waypoint: 0, command_options };
                res = u32::from(w.module_call(me, idx, call).as_bool());
                t.intents += 1;
                t.taken += u64::from(res);
            }
            4 => {
                let loc = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
                w.module_call(me, idx, ModuleCall::SetSpecialPowerOverridableDestination(loc));
                t.overrides += 1;
            }
            5..=7 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE);
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
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t, &mut names);
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
        if o[0] == 0 && status_before == 0 && s[0] == 1 {
            t.orbits += 1;
        }
        if o[0] == 0 && status_before == 1 && s[0] == 2 {
            t.departures += 1;
        }
        for x in s {
            put_u32(&mut v, x);
        }
        let gattlings: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 5).collect();
        put_u32(&mut v, gattlings.len() as u32);
        put_u32(&mut v, gattlings.iter().max().map_or(0, |&g| w.objects.find(g).unwrap().team));
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
fn spectre_gunship_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R72 SpectreGunshipUpdate",
        seed: 0x0072_5BEC,
        cases_var: "L9R72_CASES",
        ini_stem: "Sh",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r72_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R72: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} intents, {} \
         taken, {} orbits, {} departures, {} scans, {} gattling attacks, {} howitzer shots, {} smoke, {} gattlings, {} destroys, {} overrides)",
        c.cases,
        c.read_failed,
        c.excluded,
        c.ops_run,
        t.updates,
        t.xfers,
        t.effects,
        t.intents,
        t.taken,
        t.orbits,
        t.departures,
        t.scans,
        t.attacks,
        t.howitzers,
        t.smoke,
        t.gattlings,
        t.destroys,
        t.overrides
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.taken > 0 && t.orbits > 0 && t.departures > 0 && t.scans > 0 && t.howitzers > 0 && t.smoke > 0, "the generator must reach every effect");
}
