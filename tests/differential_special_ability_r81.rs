#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialAbilityUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SpecialAbilityUpdate` (`logic::modules::special_ability_update`) against the unchanged C++, `p1w13-sonnet`, batch R81.
//!
//! # Method
//!
//! The C++ side is `SpecialAbilityUpdate.cpp` and its header, compiled into `wrap_module.cpp`'s unit (as `R81SpecialAbilityUpdate`: the module
//! build's header of that name is batch O's stand-in) on batch R53's real `SpecialPowerUpdateModule` and batch R50's real `PhysicsBehavior`
//! (`differential/l9_special_ability_r81.inc`, `ref_l9r81_run`), on the module build's stand-ins: the unit (object 1) with the module of the data
//! the case reads, maybe a second module of the same data for another special power, maybe a special power module, an AI, a `PhysicsBehavior`, a
//! stealth update, a drawable, the bone `BONE_A`, weapons; two targets (objects 2-3: kinds, a drawable, a contain that may be garrisonable,
//! dead, stealthed, detected, booby trapped, a trap that goes off); maybe a mine (object 4); positions, angles, geometries, three teams and
//! players (money, colours, relationships, the local player); a thing factory whose `SO_LASER`, `SO_PLAIN`, `SO_BOMB` and `SO_PHYS` make the
//! special objects (5, 6, ...: a laser, nothing, a sticky bomb, a `PhysicsBehavior`); the partition manager's distances (PartitionManager.cpp's)
//! and lists (the test's); every other call a line.
//!
//! The Rust side is the real thing: a `World` with the same objects (the modules from `modules::new_module`, the real `PhysicsBehavior`s,
//! `ProbeModule` special power modules, AIs, stealth updates, contains and sticky bombs), the special power store's 13 templates (one per
//! special power type the module knows, and one it does not), a thing factory with the four templates; every op an `update()`, a call of a
//! module's interface, a change to the scene, or an xfer of the first module.
//!
//! Each case reads a block `D` (the module's fields, mostly well formed) and runs 1 to 80 ops. Compared, bit for bit: the read's throw, the
//! data; per op the throw status, the result, the effects in order, the xfer status, bytes and CRC; each module's state; and every object's
//! position, angle, using-ability status, producer, experience sink and physics.
//!
//! # Results (2026-10-30, the default 9,000 cases)
//!
//! 9,000 cases (1,085 data blocks threw, as the C++ did; 552 excluded, undefined upstream: 535 with a `NULL` special power template read for
//! its type, 17 with a cash steal or capture involving an object no player controls), 367,949 ops and 2,143,174 outputs: **0 mismatches**. In
//! the compared cases: 233,668 updates, 15,851 intents taken, 20,860 xfers, 238,949 effect lines; 2,236 preparations, 275 special objects made,
//! 377 trigger sounds, 101 laser inits, 161 sticky bombs, 200 captures, 4 capture flashes, 10 hack effects, 39 cash steals, 405 packs, 163
//! flee mine scans.
//!
//! On the way: the C++ class's destructor runs `onExit(TRUE)`, which reads the template's type, so the driver leaves a module of no template
//! (upstream's would crash; the translation has no destructor). A load leaves the module out of the update queue (`m_indexInLogic` -1) until the
//! game's post-process puts it back, so the next `setWakeFrame` failed on the Rust side; the test puts it back at once, as the C++ stand-in's queue
//! has no indices. The partition manager's closest object for the flee: the module build's two filters (`PartitionFilterAcceptByKindOf`,
//! `PartitionFilterSamePlayer`) have no `allow` of their own (and the second's name is another build's class too, whose virtual table the
//! linker took), so the driver applies PartitionManager.cpp's two `allow`s itself.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 3,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3811 | the preparation never counts down | 3,102 |
//! | 3812 | the capture's flash phase grows by half the fraction, not a third | 118 |
//! | 3813 | the cash steal takes nothing | 19 |
//! | 3814 | the flash phase not saved | 2,603 |
//! | 3815 | the other exclusive special abilities not ended | 1,408 |
//! | 3816 | the flee goes the wrong way | 63 |
//! | 3817 | `NeedToFaceTarget = No` ignored | 10,356 |
//! | 3818 | the laser's removal leaves the primary weapon unlocked | 420 |
//! | 3819 | a special object that is gone still counted | 35 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_MINE, KINDOF_STRUCTURE};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_BOOBY_TRAPPED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_STEALTHED};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ClientModuleData, ModuleInfoData, ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::common::audio_event_name::AudioEventName;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehaviorModuleData;
use rts_lockstep_engine::logic::modules::special_ability_update::{SpecialAbilityUpdate, SpecialAbilityUpdateModuleData};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{EvaMessage, ModelConditionOp, ObjectClientEvent, ObjectSound, RadarCall};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r81.bin");
const XFER_FILE: &str = "l9_r81.bin";
/// The special power templates every case defines: one per type the module knows, and one (`SP_12`) it does not.
const POWERS: &str = "SpecialPower SP_0\nEnum = SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES\nEnd\n\
SpecialPower SP_1\nEnum = SPECIAL_HACKER_DISABLE_BUILDING\nEnd\n\
SpecialPower SP_2\nEnum = SPECIAL_INFANTRY_CAPTURE_BUILDING\nEnd\n\
SpecialPower SP_3\nEnum = SPECIAL_BLACKLOTUS_CAPTURE_BUILDING\nEnd\n\
SpecialPower SP_4\nEnum = SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK\nEnd\n\
SpecialPower SP_5\nEnum = SPECIAL_BLACKLOTUS_STEAL_CASH_HACK\nEnd\n\
SpecialPower SP_6\nEnum = SPECIAL_REMOTE_CHARGES\nEnd\n\
SpecialPower SP_7\nEnum = SPECIAL_TIMED_CHARGES\nEnd\n\
SpecialPower SP_8\nEnum = SPECIAL_TANKHUNTER_TNT_ATTACK\nEnd\n\
SpecialPower SP_9\nEnum = SPECIAL_BOOBY_TRAP\nEnd\n\
SpecialPower SP_10\nEnum = SPECIAL_HELIX_NAPALM_BOMB\nEnd\n\
SpecialPower SP_11\nEnum = SPECIAL_DISGUISE_AS_VEHICLE\nEnd\n\
SpecialPower SP_12\nEnum = SPECIAL_DAISY_CUTTER\nEnd\n";
/// The driver's radii.
const RADII: [f32; 8] = [1.0, 2.5, 5.0, 8.0, 12.0, 0.5, 3.0, 15.0];
const TEMPLATES: [&str; 4] = ["SO_LASER", "SO_PLAIN", "SO_BOMB", "SO_PHYS"];
const VOICES: [&str; 3] = ["VoiceCaptureBuildingComplete", "VoiceDisableVehicleComplete", "VoiceStealCashComplete"];
const COORDS: [f32; 12] = [-30.0, -12.0, -4.0, -1.0, 0.0, 0.5, 1.0, 2.5, 4.0, 9.0, 12.0, 30.0];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpecialAbilityUpdate(Arc::new(SpecialAbilityUpdateModuleData::friend_new_module_data(Some(ini))?)));
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

fn sp_name(k: u32) -> Option<Vec<u8>> {
    (k < 13).then(|| format!("SP_{k}").into_bytes())
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
    let template = u32_(&mut p);
    let (object, bone) = (str_(&mut p), str_(&mut p));
    let words: Vec<u32> = (0..15).map(|_| u32_(&mut p)).collect();
    let ps = str_(&mut p);
    let sounds: Vec<String> = (0..4).map(|_| str_(&mut p)).collect();
    out.push(format!("data {template} {object:?} {bone:?} {words:x?} {ps:?} {sounds:?}"));
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
        for k in 0..2 {
            if u32_(&mut p) != 0 {
                let mut s: Vec<u32> = (0..11).map(|_| u32_(&mut p)).collect();
                let ids: Vec<u32> = (0..s[10]).map(|_| u32_(&mut p)).collect();
                s.extend(ids);
                s.extend((0..3).map(|_| u32_(&mut p)));
                out.push(format!("op {i} module {} {s:x?}", k + 1));
            }
        }
        let objects = u32_(&mut p);
        for _ in 0..objects {
            let mut o: Vec<u32> = (0..9).map(|_| u32_(&mut p)).collect();
            if o[8] != 0 {
                o.extend((0..7).map(|_| u32_(&mut p)));
            }
            out.push(format!("op {i} object {o:x?}"));
        }
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let sps = ["SP_0", "SP_1", "SP_2", "SP_3", "SP_4", "SP_5", "SP_6", "SP_7", "SP_8", "SP_9", "SP_10", "SP_11", "SP_12"];
    if g.below(100) < 97 {
        // (the captures, whose preparation flashes the target, a quarter of the time)
        let v = if g.below(30) == 0 {
            pick(g, &["None", "SP_99"]).to_string()
        } else if g.below(4) == 0 {
            pick(g, &["SP_2", "SP_3"]).to_string()
        } else {
            pick(g, &sps).to_string()
        };
        lines.push(format!("SpecialPowerTemplate = {v}"));
    }
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str]| {
        if g.below(100) < p {
            let v = if !clean && g.below(20) == 0 { pick(g, &["x", "", "1.5.5"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 70, "StartAbilityRange", &["0", "1", "2.4", "5", "15", "30", "200", "200", "60"]);
    field(g, 30, "AbilityAbortRange", &["5", "20", "40", "500"]);
    field(g, 70, "PreparationTime", &["0", "33", "100", "200", "500", "1000", "1000", "2000"]);
    field(g, 20, "PersistentPrepTime", &["0", "33", "100", "300"]);
    field(g, 45, "PackTime", &["0", "33", "100", "300"]);
    field(g, 45, "UnpackTime", &["0", "33", "100", "300"]);
    field(g, 20, "PreTriggerUnstealthTime", &["0", "33", "100"]);
    field(g, 25, "SkipPackingWithNoTarget", &["Yes", "No"]);
    field(g, 30, "PackUnpackVariationFactor", &["0", "0.1", "0.5"]);
    field(g, 85, "SpecialObject", &["SO_LASER", "SO_LASER", "SO_PLAIN", "SO_BOMB", "SO_BOMB", "SO_PHYS", "SO_X"]);
    field(g, 40, "SpecialObjectAttachToBone", &["BONE_A", "BONE_B"]);
    field(g, 50, "MaxSpecialObjects", &["0", "1", "2", "3"]);
    field(g, 35, "SpecialObjectsPersistent", &["Yes", "No"]);
    field(g, 40, "EffectDuration", &["0", "100", "1000"]);
    field(g, 15, "EffectValue", &["1", "5"]);
    field(g, 15, "UniqueSpecialObjectTargets", &["Yes", "No"]);
    field(g, 25, "SpecialObjectsPersistWhenOwnerDies", &["Yes", "No"]);
    field(g, 25, "AlwaysValidateSpecialObjects", &["Yes", "No"]);
    field(g, 20, "FlipOwnerAfterPacking", &["Yes", "No"]);
    field(g, 20, "FlipOwnerAfterUnpacking", &["Yes", "No"]);
    field(g, 40, "FleeRangeAfterCompletion", &["0", "20", "50"]);
    field(g, 50, "DisableFXParticleSystem", &["PS_A", "PS_A", "None"]);
    field(g, 50, "DoCaptureFX", &["Yes", "Yes", "No"]);
    field(g, 50, "PackSound", &["SND_PACK", "NoSound"]);
    field(g, 50, "UnpackSound", &["SND_UNPACK", "NoSound"]);
    field(g, 50, "PrepSoundLoop", &["SND_PREP", "NoSound"]);
    field(g, 50, "TriggerSound", &["SND_TRIGGER", "NoSound"]);
    field(g, 35, "LoseStealthOnTrigger", &["Yes", "No"]);
    field(g, 35, "AwardXPForTriggering", &["0", "5", "-2"]);
    field(g, 30, "SkillPointsForTriggering", &["-1", "0", "3"]);
    field(g, 40, "ApproachRequiresLOS", &["Yes", "No"]);
    field(g, 40, "NeedToFaceTarget", &["Yes", "No"]);
    field(g, 25, "PersistenceRequiresRecharge", &["Yes", "No"]);
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["NoSuchField = 1", "PackTime = x", "SpecialObject", "MaxSpecialObjects = -1"]).to_string());
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

/// The template a module's data names, as a number (13: none).
fn data_template(data: &Option<ModuleDataKind>) -> u32 {
    match data {
        Some(ModuleDataKind::SpecialAbilityUpdate(d)) => d
            .special_power_template
            .as_deref()
            .and_then(|n| n.strip_prefix(b"SP_"))
            .and_then(|k| std::str::from_utf8(k).ok()?.parse::<u32>().ok())
            .filter(|&k| k < 13)
            .unwrap_or(13),
        _ => 13,
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let own = data_template(data);
    cfg[0] = g.below(200);
    cfg[1] = g.u32();
    let power_k = if g.below(5) == 0 { g.below(14) } else { own };
    let second_k = if g.below(3) == 0 { own } else { g.below(14) };
    let team1 = 1 + g.below(3);
    cfg[2] = p(g, 80) | p(g, 90) << 1 | p(g, 40) << 2 | p(g, 40) << 3 | p(g, 85) << 4 | power_k << 5 | p(g, 20) << 9 | second_k << 10 | p(g, 50) << 14
        | p(g, 60) << 15 | (if g.below(12) == 0 { 0 } else { team1 }) << 16 | g.below(8) << 18 | p(g, 50) << 21;
    let source = |g: &mut Gen| if g.below(6) == 0 { g.below(4) } else { 2 };
    cfg[3] = p(g, 4) | p(g, 75) << 1 | p(g, 15) << 2 | source(g) << 3 | p(g, 75) << 5;
    cfg[4] = cfg[0] + g.below(20) - 5;
    for k in 0..2usize {
        let team = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
        cfg[5 + k] = p(g, 50) | p(g, 8) << 1 | p(g, 70) << 2 | p(g, 40) << 3 | p(g, 50) << 4 | team << 5 | g.below(8) << 7 | p(g, 40) << 10;
        cfg[7 + k] = p(g, 8) | p(g, 15) << 1 | p(g, 50) << 2 | p(g, 15) << 3 | p(g, 15) << 4;
    }
    cfg[9] = p(g, 80) | (if g.below(5) == 0 { g.below(4) } else { team1 }) << 1 | g.below(8) << 3;
    let coord = |g: &mut Gen| f(*pick(g, &COORDS.iter().collect::<Vec<_>>()));
    // the unit anywhere; the targets often next to it (most abilities start in range only after an approach the AI stand-ins never make)
    for k in 0..4usize {
        cfg[10 + k] = coord(g);
        cfg[14 + k] = coord(g);
        if k > 0 && g.below(10) < 6 {
            let near = *pick(g, &[&0.0f32, &0.5, &1.0, &-1.0, &2.5, &4.0]);
            cfg[10 + k] = f(f32::from_bits(cfg[10]) + near);
            cfg[14 + k] = f(f32::from_bits(cfg[14]) + *pick(g, &[&0.0f32, &0.5, &-2.5, &1.0]));
        }
    }
    cfg[18] = g.u32() & 0xFFFF;
    // the partition manager's list: mostly the targets (the line of sight scan and the collisions find them), sometimes the mine
    let gen_list = |g: &mut Gen| {
        let mut c = 0u32;
        for slot in 0..(g.below(5)) {
            let id = if g.below(3) == 0 { 1 + g.below(9) } else { *pick(g, &[&2u32, &3, &2, &3, &4]) };
            c |= id << (4 * slot);
        }
        c
    };
    cfg[19] = gen_list(g);
    cfg[20] = (g.below(2048)) | (g.below(2048)) << 11 | (g.below(1024)) << 22;
    let mut rel = 0u32;
    for a in 0..3u32 {
        for b in 0..3u32 {
            let r = if a == b { if g.below(10) == 0 { g.below(3) } else { 2 } } else { g.below(3) };
            rel |= r << (2 * (3 * a + b));
        }
    }
    cfg[21] = rel;
    cfg[22] = g.below(4);
    // a fifth of the cases are long runs of updates (a preparation of 2 s is 60 of them)
    let long = g.below(5) == 0;
    let n_ops = if long { 60 + g.below(60) as usize } else { 1 + g.below(80) as usize };
    if long {
        // (a unit standing still, among enemies: nothing cuts its preparation short)
        cfg[3] &= !(1 << 2);
        cfg[21] = 0x0002_0202;
    }
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    let target = |g: &mut Gen| *pick(g, &[&2u32, &2, &3, &3, &4, &5, &6, &0, &0]);
    // most cases start the ability at once and then mostly run it (the phases take many updates)
    if g.below(10) < 8 {
        ops.push([6, own | 1 << 8, *pick(g, &[&2u32, &2, &3, &3, &0]), p(g, 50), g.u32() & 0xFFFF]);
    }
    while ops.len() < n_ops {
        let o = match if long && g.below(10) < 8 { 0 } else { g.below(100) } {
            0..=49 => [0, 1, 0, 0, 0],
            50..=51 => [0, 2, 0, 0, 0],
            52..=57 => {
                let which = if g.below(8) == 0 { 2 } else { 1 };
                let k = if g.below(6) == 0 { g.below(14) } else if which == 2 { second_k } else { own };
                let pos = g.u32() & 0xFFFF;
                [6, k | which << 8, target(g), p(g, 40), pos]
            }
            58..=67 => {
                let a = *pick(g, &[&3u32, &3, &3, &4, &7, &8, &7, &8, &10, &10, &10, &11, &12, &13, &14, &14, &14, &15, &16, &17, &18, &19, &19]);
                let b = match a {
                    3 => g.below(64),
                    4 => frame + g.below(20) - 5,
                    7 | 8 => g.below(32),
                    // (often the unit arrives: its coordinate a target's, as the approach would leave it)
                    10 | 14 if g.below(2) == 0 => cfg[a as usize + 1 + g.below(2) as usize],
                    10..=17 => coord(g),
                    18 => g.u32() & 0xFFFF,
                    _ => gen_list(g),
                };
                [1, a, b, 0, 0]
            }
            68..=74 => {
                frame += 1 + g.below(30);
                [2, frame, 0, 0, 0]
            }
            75..=77 => [3, 0, 0, 0, 0],
            78..=80 => [4, 0, 0, 0, 0],
            81..=82 => [5, 0, 0, 0, 0],
            83..=85 => [7, 1 + u32::from(g.below(6) == 0), 0, 0, 0],
            86..=89 => [8, 1 + u32::from(g.below(6) == 0), target(g), 0, 0],
            90..=91 => [9, 1 + u32::from(g.below(6) == 0), 0, 0, 0],
            92..=93 => [10, 1 + u32::from(g.below(6) == 0), 0, 0, 0],
            _ => [11, 0, *pick(g, &[&2u32, &3, &4, &5, &5, &6, &7]), 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    intents: u64,
    xfers: u64,
    effects: u64,
    made: u64,
    triggers: u64,
    lasers: u64,
    bombs: u64,
    captures: u64,
    hacks: u64,
    cash: u64,
    packs: u64,
    flees: u64,
    flashes: u64,
    preparations: u64,
    excluded: BTreeMap<String, u64>,
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

/// The driver's `apply`: the words op 1 may set.
fn apply(w: &mut World, cfg: &[u32], a: u32, probes: &Probes) {
    let set_probe = |w: &mut World, id: ObjectId, idx: Option<u16>, f: &dyn Fn(&mut ProbeAnswers)| {
        if let Some(i) = idx {
            if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[i as usize].body.as_mut() {
                f(&mut p.answers);
            }
        }
    };
    match a {
        3 => {
            let fl = cfg[3];
            let o = w.objects.find_mut(1).unwrap();
            if fl & 1 != 0 {
                o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
            } else {
                o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
            }
            set_probe(w, 1, probes.ai, &|a| {
                a.idle = fl >> 1 & 1 != 0;
                a.moving = fl >> 2 & 1 != 0;
                a.last_command_source = (fl >> 3 & 3) as i32;
            });
            set_probe(w, 1, probes.power, &|a| a.yes = fl >> 5 & 1 != 0);
        }
        4 => {
            let frame = cfg[4];
            set_probe(w, 1, probes.power, &|a| a.ready_frame = frame);
        }
        7 | 8 => {
            let id = a as ObjectId - 5;
            let fl = cfg[a as usize];
            if let Some(o) = w.objects.find_mut(id) {
                if fl & 1 != 0 {
                    o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
                } else {
                    o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
                }
                o.status.set(OBJECT_STATUS_STEALTHED, (fl >> 1 & 1) as i32);
                o.status.set(OBJECT_STATUS_DETECTED, (fl >> 2 & 1) as i32);
                o.status.set(OBJECT_STATUS_BOOBY_TRAPPED, (fl >> 3 & 1) as i32);
            }
            let answers = w.cuts.booby_trap_answers.as_mut().unwrap();
            if fl >> 4 & 1 != 0 {
                answers.insert(id);
            } else {
                answers.remove(&id);
            }
        }
        10..=18 => {
            for k in 0..4usize {
                let id = k as ObjectId + 1;
                if w.objects.find(id).is_none() {
                    continue;
                }
                w.set_position(id, &Coord3D { x: f32::from_bits(cfg[10 + k]), y: f32::from_bits(cfg[14 + k]), z: 0.0 });
                w.set_orientation(id, (cfg[18] >> (4 * k) & 15) as f32 * 0.4 - 3.0);
            }
            bone(w, cfg);
        }
        19 => {
            let mut scan = Vec::new();
            for k in 0..8 {
                let id = cfg[19] >> (4 * k) & 15;
                if id == 0 {
                    break;
                }
                if w.objects.find(id as ObjectId).is_some() {
                    scan.push(id as ObjectId);
                }
            }
            w.cuts.range_scan = scan;
        }
        _ => {}
    }
}

/// The unit's bone `BONE_A`, at (1, 2, 3) from its position, when it has one.
fn bone(w: &mut World, cfg: &[u32]) {
    w.cuts.contain.single_bones.clear();
    if cfg[2] >> 21 & 1 != 0 {
        if let Some(o) = w.objects.find(1) {
            let p = o.get_position();
            w.cuts.contain.single_bones.insert((1, b"BONE_A".to_vec()), Coord3D { x: p.x + 1.0, y: p.y + 2.0, z: p.z + 3.0 });
        }
    }
}

#[derive(Default)]
struct Probes {
    ai: Option<u16>,
    power: Option<u16>,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let store = ctx.store.clone();
    let ModuleDataKind::SpecialAbilityUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.special_power_template.as_deref().map_or(0, |n| template_id(&store, n)));
    put_str(&mut v, &d.special_object_name);
    put_str(&mut v, &d.special_object_attach_to_bone_name);
    for x in [d.start_ability_range, d.ability_abort_range, d.pack_unpack_variation_factor, d.flee_range_after_completion] {
        put_u32(&mut v, x.to_bits());
    }
    for x in [d.effect_value, d.award_xp_for_triggering, d.skill_points_for_triggering] {
        put_u32(&mut v, x as u32);
    }
    for x in [d.preparation_frames, d.persistent_prep_frames, d.effect_duration, d.max_special_objects, d.pack_time, d.unpack_time, d.pre_trigger_unstealth_frames] {
        put_u32(&mut v, x);
    }
    let bools = [
        d.skip_packing_with_no_target,
        d.special_objects_persistent,
        d.unique_special_object_targets,
        d.special_objects_persist_when_owner_dies,
        d.flip_object_after_packing,
        d.flip_object_after_unpacking,
        d.always_validate_special_objects,
        d.do_capture_fx,
        d.lose_stealth_on_trigger,
        d.approach_requires_los,
        d.need_to_face_target,
        d.persistence_requires_recharge,
    ];
    put_u32(&mut v, bools.iter().enumerate().fold(0u32, |a, (i, &b)| a | (u32::from(b) << i)));
    put_str(&mut v, d.disable_fx_particle_system.as_deref().unwrap_or(b"-"));
    for s in [&d.pack_sound, &d.unpack_sound, &d.prep_sound_loop, &d.trigger_sound] {
        put_str(&mut v, s);
    }
    if std::env::var("R81_TRACE").is_ok() {
        eprintln!("rust case cfg {cfg:x?} ops {ops:?} data {d:?}");
    }
    let second_name = sp_name(cfg[2] >> 10 & 15);
    let data2 = ModuleDataKind::SpecialAbilityUpdate(Arc::new(SpecialAbilityUpdateModuleData { special_power_template: second_name, ..(**d).clone() }));
    let so_phys = ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData { center_of_mass_offset: 1.5, ..Default::default() }));
    let phys_data = ModuleDataKind::PhysicsBehavior(Arc::new(PhysicsBehaviorModuleData::default()));

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    for t in TEMPLATES {
        factory.new_template(t.as_bytes(), 0).expect("a template");
    }
    factory.friend_find_template_mut(b"SO_LASER").unwrap().data.friend_client_update_module_info_mut().add_module_info(
        b"LaserUpdate",
        b"ModuleTag_Laser",
        ModuleInfoData::Client(ClientModuleData::default()),
        0,
        false,
        false,
    );
    let plain = || ModuleInfoData::Logic(ModuleDataKind::Behavior(Arc::new(Default::default())));
    factory.friend_find_template_mut(b"SO_BOMB").unwrap().data.friend_behavior_module_info_mut().add_module_info(
        b"StickyBombUpdate",
        b"ModuleTag_Bomb",
        plain(),
        0,
        false,
        false,
    );
    factory.friend_find_template_mut(b"SO_PHYS").unwrap().data.friend_behavior_module_info_mut().add_module_info(
        b"PhysicsBehavior",
        b"ModuleTag_Physics",
        ModuleInfoData::Logic(so_phys),
        rts_lockstep_engine::logic::modules::physics_behavior::PhysicsBehavior::get_interface_mask(),
        false,
        false,
    );
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: store.clone(), thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.probe_power_trace = true;
    w.cuts.probe_ability_trace = true;
    w.cuts.crate_stub_object_calls = true;
    w.cuts.booby_trap_answers = Some(BTreeSet::new());
    w.cuts.untranslated_probe = Some(ProbeAnswers::default());
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        let pl = (t - 1) as i32;
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, pl);
        w.cuts.player_default_teams.insert(pl, t);
        w.cuts.player_colors.insert(pl, (0xFF10_2030u32.wrapping_add(0x0020_3040u32.wrapping_mul(t - 1))) as i32);
        let pm = w.players.get_nth_mut(pl).unwrap();
        pm.money.init();
        pm.money.set_player_index(pl);
        let mut events = Vec::new();
        pm.money.deposit(cfg[20] >> (11 * (t - 1)) & 0x7FF, false, &mut events);
    }
    let rel = |r: u32| match r {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    };
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[21] >> (2 * (3 * a + b)) & 3));
        }
    }
    w.players.friend_set_local_index((cfg[22] & 3 < 3).then_some((cfg[22] & 3) as i32));
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.interfaces = interfaces;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        pm
    };
    let mut probes = Probes::default();
    let f2 = cfg[2];
    for k in 0..4usize {
        let id = k as ObjectId + 1;
        let fl = if k == 0 { f2 } else if k < 3 { cfg[4 + k] } else { cfg[9] };
        let mut modules = Vec::new();
        let mut kinds = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SpecialAbilityUpdate", b"ModuleTag_A", &data).unwrap().unwrap());
            if fl >> 9 & 1 != 0 {
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SpecialAbilityUpdate", b"ModuleTag_B", &data2).unwrap().unwrap());
            }
            if fl >> 4 & 1 != 0 {
                let power_template = sp_name(fl >> 5 & 15).map_or(0, |n| template_id(&store, &n));
                modules.push(probe(b"R81Power", ProbeAnswers { power_template, ..Default::default() }, MODULEINTERFACE_SPECIAL_POWER));
            }
            if fl >> 2 & 1 != 0 {
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PhysicsBehavior", b"ModuleTag_Physics", &phys_data).unwrap().unwrap());
            }
            if fl >> 1 & 1 != 0 {
                let mut ai = probe(b"R81Ai", ProbeAnswers::default(), 0);
                ai.is_ai = true;
                modules.push(ai);
            }
            if fl >> 3 & 1 != 0 {
                let mut st = probe(b"R81Stealth", ProbeAnswers::default(), 0);
                st.is_stealth = true;
                modules.push(st);
            }
        } else if k < 3 {
            if fl & 1 != 0 {
                kinds.push(KINDOF_STRUCTURE);
            }
            if fl >> 1 & 1 != 0 {
                kinds.push(KINDOF_MINE);
            }
            if fl >> 3 & 1 != 0 {
                modules.push(probe(b"R81Contain", ProbeAnswers { garrisonable: fl >> 4 & 1 != 0, ..Default::default() }, MODULEINTERFACE_CONTAIN));
            }
        } else if fl & 1 != 0 {
            kinds.push(KINDOF_MINE);
        }
        let mut template = ThingTemplate::from_object_values(&ObjectTemplateValues { kind_of: kinds, ..Default::default() });
        if k == 0 {
            for name in VOICES {
                template.friend_set_per_unit_sound(name.as_bytes(), AudioEventName { event_name: name.as_bytes().to_vec() });
            }
            template.friend_set_audio(ThingTemplateAudioType::VoiceTaskComplete, AudioEventName { event_name: b"VoiceTaskComplete".to_vec() });
        }
        let made = w.new_object(NewObject { template: Arc::new(template), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(made, id);
        let team = if k == 0 { fl >> 16 & 3 } else if k < 3 { fl >> 5 & 3 } else { fl >> 1 & 3 };
        let r = if k == 0 { RADII[(fl >> 18 & 7) as usize] } else if k < 3 { RADII[(fl >> 7 & 7) as usize] } else { RADII[(fl >> 3 & 7) as usize] };
        let has_drawable = if k == 0 { fl & 1 != 0 } else { k < 3 && fl >> 2 & 1 != 0 };
        let o = w.objects.find_mut(id).unwrap();
        o.team = team;
        o.geometry_info = if (k == 1 || k == 2) && fl >> 10 & 1 != 0 {
            GeometryInfo::new(GeometryType::Box, false, 5.0, r, r)
        } else {
            GeometryInfo::new(GeometryType::Sphere, false, r, r, r)
        };
        o.drawable = if has_drawable { 7 + id } else { INVALID_DRAWABLE_ID };
        if k == 0 {
            for slot in [0, 1] {
                if fl >> (14 + slot) & 1 != 0 {
                    w.cuts.weapons.insert((1, slot));
                }
            }
        }
    }
    probes.ai = w.find_module(1, b"R81Ai");
    probes.power = w.find_module(1, b"R81Power");
    for a in 3..=19 {
        apply(&mut w, &cfg, a, &probes);
    }
    let idx1 = w.find_module(1, b"SpecialAbilityUpdate").expect("the module");
    let idx2 = if f2 >> 9 & 1 != 0 { Some(idx1 + 1) } else { None };
    let mut saved: Vec<u8> = Vec::new();
    let mut particle_names: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for o in ops {
        if o[0] == 1 && o[1] < 32 {
            cfg[o[1] as usize] = o[2];
            apply(&mut w, &cfg, o[1], &probes);
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        if std::env::var("R81_TRACE").is_ok() {
            eprintln!("rust op {o:?}");
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let which = if o[0] == 6 { o[1] >> 8 } else { o[1] };
        let ma = match which {
            1 => Some(idx1),
            2 => idx2,
            _ => None,
        };
        let obj_b = if o[2] >= 1 && w.objects.find(o[2] as ObjectId).is_some() { o[2] as ObjectId } else { INVALID_ID };
        match o[0] {
            0 => {
                if let Some(i) = ma {
                    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: i }).expect("an update module");
                    res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
            }
            1 | 2 => {}
            3..=5 => {
                // (a load leaves the module out of the update queue, `m_indexInLogic` -1, until the game's post-process puts it back: as the
                // C++ stand-in's queue has no indices, the test puts it back at once, where it was, at its loaded frame)
                let u1 = w.update_handle_of_module(ModuleHandle { object: 1, index: idx1 }).expect("an update module");
                let index_in_logic = w.objects.update_module(u1).map_or(-1, |m| m.friend_get_index_in_logic());
                let r = run_xfer(&mut w, 1, idx1 as usize, o[0] - 3, &saved);
                if o[0] == 4 {
                    if let Some(m) = w.objects.update_module_mut(u1) {
                        m.friend_set_index_in_logic(index_in_logic);
                    }
                    if index_in_logic >= 0 {
                        let mut objects = std::mem::take(&mut w.objects);
                        w.sched.rebalance(&mut objects, index_in_logic);
                        w.objects = objects;
                    }
                }
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
            11 => {
                if o[2] >= 2 && w.objects.find(o[2] as ObjectId).is_some() {
                    // gone without a trace: off the update queue and out of the table (the C++ stand-in's findObjectByID stops finding it)
                    let mut objects = std::mem::take(&mut w.objects);
                    w.sched.erase_object_updates(&mut objects, o[2] as ObjectId);
                    w.objects = objects;
                    w.objects.remove(o[2] as ObjectId);
                }
            }
            _ => {
                if let Some(i) = ma {
                    match o[0] {
                        6 => {
                            let template = sp_name(o[1] & 255).map_or(0, |n| template_id(&store, &n));
                            let pos = (o[3] != 0).then(|| Coord3D { x: f32::from((o[4] & 255) as u8 as i8) * 4.0, y: f32::from((o[4] >> 8 & 255) as u8 as i8) * 4.0, z: 0.0 });
                            let call = ModuleCall::InitiateIntentToDoSpecialPower { template, target_object: obj_b, target_position: pos, waypoint: 0, command_options: 0 };
                            res = u32::from(w.module_call(1, i, call).as_bool());
                            tally.borrow_mut().intents += u64::from(res);
                        }
                        7 => res = u32::from(w.module_call(1, i, ModuleCall::IsPowerCurrentlyInUse).as_bool()),
                        8 => {
                            let count = w.module_call(1, i, ModuleCall::SpecialAbilityGetSpecialObjectCount).as_int(0) as u32;
                            let max = w.module_call(1, i, ModuleCall::SpecialAbilityGetSpecialObjectMax).as_int(0) as u32;
                            let found = if obj_b != INVALID_ID {
                                let u = w.update_handle_of_module(ModuleHandle { object: 1, index: i }).expect("an update module");
                                let ob = w.objects.find(1).unwrap();
                                match ob.behaviors[u.index as usize].body.as_ref() {
                                    Some(UpdateBody::SpecialAbilityUpdate(m)) => m.special_object_with_producer_id(&w, obj_b).unwrap_or(0),
                                    _ => 0,
                                }
                            } else {
                                0
                            };
                            res = count | max << 8 | (found as u32) << 16;
                        }
                        9 => res = u32::from(w.module_call(1, i, ModuleCall::SpecialPowerUpdateIsActive).as_bool()),
                        _ => res = w.module_call(1, i, ModuleCall::GetSpecialAbilityType).as_int(0) as u32,
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
        let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
        let mut log = Vec::new();
        let b = |x: f32| x.to_bits();
        let mut t = tally.borrow_mut();
        let trace = w.cuts.trace.take().unwrap();
        if std::env::var("R81_TRACE").is_ok() {
            for l in &trace {
                eprintln!("  trace {l}");
            }
        }
        for line in trace {
            if line.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::PartitionIteratePotentialCollisions { position: q, geom_type, major_radius, minor_radius, angle }) => log.push(format!(
                        "potcol {:08x} {:08x} {:08x} {geom_type} {:08x} {:08x} {:08x}",
                        b(q.x),
                        b(q.y),
                        b(q.z),
                        b(major_radius),
                        b(minor_radius),
                        b(angle)
                    )),
                    Some(CutCall::PartitionIterateLineOfSightNearToFar { object, range }) => log.push(format!("scan {object} {:08x} 2", b(range))),
                    Some(CutCall::PartitionGetClosestObjectAt { position: q, range, dc }) => {
                        t.flees += 1;
                        log.push(format!("closestat {:08x} {:08x} {:08x} {:08x} {dc}", b(q.x), b(q.y), b(q.z), b(range)))
                    }
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => log.push(format!("destroy {object}")),
                    Some(CutCall::Body(BodyCall::ParticleAttachToObject { id, object })) => log.push(format!("psattach {id} {object}")),
                    Some(CutCall::ParticleSystemCreateUnplaced { template, id }) => {
                        t.hacks += 1;
                        particle_names.insert(id, template);
                    }
                    Some(CutCall::ParticleSystemSetPosition { id, position: q }) => log.push(format!(
                        "psys {} {id} {:08x},{:08x},{:08x}",
                        String::from_utf8_lossy(particle_names.get(&id).map_or(&b""[..], |n| &n[..])),
                        b(q.x),
                        b(q.y),
                        b(q.z)
                    )),
                    Some(CutCall::ParticleSystemSetLifetime { id, frames }) => log.push(format!("pslife {id} {frames}")),
                    Some(CutCall::ObjectSetDisabledUntil { object, disabled, frame }) => log.push(format!("disableduntil {object} {disabled} {frame}")),
                    Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => log.push(format!("weaponlock {object} {slot} {lock}")),
                    Some(CutCall::ExperienceAddPoints { object, points, can_scale_for_bonus }) => {
                        log.push(format!("xpadd {object} {points} {}", i32::from(can_scale_for_bonus)))
                    }
                    Some(CutCall::ObjectSetExperienceSink { object, sink }) => log.push(format!("xpsink {object} {sink}")),
                    Some(CutCall::PlayerAddSkillPoints { player, points }) => log.push(format!("skill {player} {points}")),
                    Some(CutCall::PlayerAcademyRecordBuildingCapture { player }) => {
                        t.captures += 1;
                        log.push(format!("captured {player}"))
                    }
                    Some(CutCall::ObjectDefect { object, team, detection_time }) => log.push(format!("defect {object} {} {detection_time}", team.unwrap_or(0))),
                    Some(CutCall::PlayerMoneyWithdraw { player, amount }) => {
                        t.cash += 1;
                        log.push(format!("withdraw {player} {amount} 1"))
                    }
                    Some(CutCall::PlayerMoneyDeposit { player, amount }) => log.push(format!("deposit {player} {amount} 1")),
                    Some(CutCall::PlayerScoreAddMoneyEarned { player, amount }) => log.push(format!("earned {player} {amount}")),
                    Some(CutCall::InGameUiAddFloatingTextNamed { text, amount, position: q, color }) => {
                        log.push(format!("float {} {amount} {},{},{} {:08x}", String::from_utf8_lossy(&text), b(q.x), b(q.y), b(q.z), color as u32))
                    }
                    Some(CutCall::ObjectCheckAndDetonateBoobyTrap { object, victim, result }) => log.push(format!("booby {object} {victim} {}", i32::from(result))),
                    _ => {}
                }
            } else if line.starts_with("evt ") {
                // (the queue's order is not always the trace's: the first event not yet taken that this line traced)
                let found = ev_texts.iter().position(|t| t.as_deref() == Some(line.as_str()));
                match found.and_then(|i| {
                    ev_texts[i] = None;
                    events.get(i).cloned()
                }) {
                    // (a special object's drawable gets its first model condition when the factory makes it, which the C++ stand-in's does not)
                    Some(ObjectClientEvent::DrawableModelCondition { object_id, .. }) if object_id >= 5 => {}
                    Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::ClearAndSet { clear, set } }) => {
                        if set.contains(&rts_lockstep_engine::common::model_state::MODELCONDITION_PACKING) {
                            t.packs += 1;
                        }
                        log.push(format!("mc {object_id} clear {clear:?} set {set:?}"))
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::Clear(clear) }) => log.push(format!("mc {object_id} clear {clear:?}")),
                    Some(ObjectClientEvent::DrawableSetAnimationCompletionTime { object_id, frames }) => log.push(format!("animdone {object_id} {frames}")),
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                        if name == b"SND_TRIGGER" {
                            t.triggers += 1;
                        }
                        let pos = match position {
                            Some(q) => format!("{},{},{}", b(q.x), b(q.y), b(q.z)),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)))
                    }
                    Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                    Some(ObjectClientEvent::Sound { sound: ObjectSound::DefectorTimerTick, object_id, position: None }) => log.push(format!("audio defectorTick - -1 {object_id}")),
                    Some(ObjectClientEvent::DrawableFlashAsSelectedColor { r, g, b: bl, .. }) => {
                        t.flashes += 1;
                        log.push(format!("flashcolor {},{},{}", b(r), b(g), b(bl)))
                    }
                    Some(ObjectClientEvent::Eva { message }) => log.push(format!(
                        "eva {}",
                        match message {
                            EvaMessage::CashStolen => 16,
                            EvaMessage::BuildingBeingStolen => 19,
                            EvaMessage::BuildingStolen => 20,
                            other => panic!("unexpected eva {other:?}"),
                        }
                    )),
                    Some(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id, .. }) => log.push(format!("infiltrate {object_id}")),
                    Some(ObjectClientEvent::LaserInit { laser, parent, target, start: s, end: e, bone }) => {
                        t.lasers += 1;
                        log.push(format!(
                            "laser {laser} {parent} {target} {},{},{} {},{},{} {}",
                            b(s.x),
                            b(s.y),
                            b(s.z),
                            b(e.x),
                            b(e.y),
                            b(e.z),
                            String::from_utf8_lossy(&bone)
                        ))
                    }
                    _ => {}
                }
            } else if let Some(rest) = line.strip_prefix("bt ") {
                if rest.starts_with("stickyinit ") {
                    t.bombs += 1;
                }
                if rest.starts_with("triggered ") {
                    t.preparations += 1;
                }
                log.push(rest.to_string());
            } else if line.starts_with("recharge ") {
                // (the power module's place in the C++ list is not its index here)
                log.push(line.rsplit_once(' ').map_or("", |x| x.0).to_string());
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
        for i in [Some(idx1), idx2] {
            match i {
                Some(i) => {
                    put_u32(&mut v, 1);
                    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: i }).expect("an update module");
                    let Some(UpdateBody::SpecialAbilityUpdate(m)) = w.objects.find(1).unwrap().behaviors[u.index as usize].body.as_ref() else {
                        panic!("the module is back in its slot")
                    };
                    put_u32(&mut v, w.objects.update_module(u).map_or(0, |um| um.friend_get_next_call_frame()));
                    for s in m.state_words() {
                        put_u32(&mut v, s);
                    }
                }
                None => put_u32(&mut v, 0),
            }
        }
        let mut ids = w.objects.ids_in_order();
        ids.sort_unstable();
        put_u32(&mut v, ids.len() as u32);
        for id in &ids {
            let ob = w.objects.find(*id).unwrap();
            let pos = ob.get_position();
            put_u32(&mut v, *id as u32);
            for x in [pos.x, pos.y, pos.z, ob.thing.orientation()] {
                put_u32(&mut v, x.to_bits());
            }
            put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_IS_USING_ABILITY)));
            put_u32(&mut v, ob.producer_id as u32);
            put_u32(&mut v, ob.experience.sink as u32);
            match ob.physics {
                Some(pi) => {
                    put_u32(&mut v, 1);
                    let u = w.update_handle_of_module(ModuleHandle { object: *id, index: pi }).expect("an update module");
                    let Some(UpdateBody::PhysicsBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the physics is back in its slot") };
                    let s = m.state_words();
                    for x in [s[9], s[10], s[11], s[0], s[2], s[1], s[14]] {
                        put_u32(&mut v, x);
                    }
                }
                None => put_u32(&mut v, 0),
            }
        }
    }
    tally.borrow_mut().made += w.objects.ids_in_order().len().saturating_sub(4) as u64;
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
fn special_ability_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R81 SpecialAbilityUpdate",
        seed: 0x0081_5AB1,
        cases_var: "L9R81_CASES",
        ini_stem: "Sa",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r81_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R81: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} intents taken, {} xfers, {} effect lines, {} \
         preparations, {} special objects made, {} trigger sounds, {} laser inits, {} sticky bombs, {} captures, {} capture flashes, {} hack effects, \
         {} cash steals, {} packs, {} flee mine scans",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.intents, t.xfers, t.effects, t.preparations, t.made, t.triggers, t.lasers, t.bombs,
        t.captures, t.flashes, t.hacks, t.cash, t.packs, t.flees
    );
    println!("excluded by: {:?}", t.excluded);
}

#[allow(dead_code)]
fn _uses(_: SpecialAbilityUpdate) {}
