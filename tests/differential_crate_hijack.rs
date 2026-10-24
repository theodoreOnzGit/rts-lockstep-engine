#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToHijackedVehicleCrateCollide.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HijackerUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ConvertToHijackedVehicleCrateCollide` and `HijackerUpdate` (`logic::modules::{convert_to_hijacked_vehicle_crate_collide,hijacker_update}`) against
//! the unchanged C++, `p1w13-sonnet`, batch R13e.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crates_r13e.inc`) on the module build's
//! stand-ins: the hijacker (id 1: player, level, the name `Pilot`, vision and shroud clearing ranges, maybe an AI with a goal and a drawable, the crate
//! module and maybe a REAL `HijackerUpdate`) and the vehicle (id 2: kinds, player, the hijacked bit, level, position, maybe an AI that may be a dozer,
//! a contain with passengers and an `EjectPilotDie`), three players with default teams 11 to 13, the local player, the teams' relationship, and a radar,
//! an Eva, a script engine, an audio manager, the partition manager and a thing factory that log.
//!
//! The Rust side is the real thing: a `World` with the same objects (the crate module and the `HijackerUpdate` from `modules::new_module`,
//! `ProbeModule`s for the AIs, the contain and the `EjectPilotDie`), `cuts.crate_stub_object_calls` set (the team change and the veterancy are records,
//! as the driver's are lines), the vehicle's going a destruction processed at once, and every op a `World::module_call`, an `update()` as the scheduler
//! runs it, or a change to the scene.
//!
//! Each case reads a file of two blocks, `D` (the `CrateCollide` fields) and `H` (`AttachToTargetBone`, `ParachuteName`), mostly well formed, and runs
//! 1 to 16 ops; after the vehicle has gone the ops leave it alone. Compared, bit for bit: the read's throw; the data; per op the throw status, a result,
//! the effects in order (`infiltrate`, `eva`, `setteam`, `aimove`, `aiidle`, `canceltask`, `audio`, `transfername`, `xplevel`, `leavegroup`,
//! `punregister`, `pregister`, `hidden`, `newobject`, `validfor`, `addtocontain`, `destroy`), the xfer status, bytes and CRC, the hijacker's position and status bits, the
//! vehicle's hijacked bit, both levels, the vehicle's ranges and the `HijackerUpdate`'s state.
//!
//! # Results (2026-10-23, `L9R13E_CASES=9000`)
//!
//! 9,000 cases (6,980 compared, 106 excluded as undefined upstream: an unowned hijacker, a vehicle with no AI, a parachute with no contain; 1,914 data
//! files threw, as the C++ did), 59,463 ops (11,808 `onCollide` calls, 15,405 `HijackerUpdate` updates, 4,138 vehicles gone, 812 hijackers destroyed
//! with a vehicle that cannot eject, 11,019 xfers) and 73,423 op outputs: **0 mismatches**. Effect lines compared: xplevel 2,948, aiidle 1,869,
//! infiltrate / setteam / aimove / audio / transfername 1,139 each, canceltask 843, leavegroup / punregister 733 each, hidden 493, eva 276,
//! pregister 257, newobject 9, validfor 9, addtocontain 6 (the parachute path is rare: a hijacker riding an airborne vehicle that then goes).
//!
//! Harness corrections on the way, none in the translation: the Rust world schedules the `HijackerUpdate` when it makes the object, so the driver sets
//! the bare C++ module's scheduling word to match (as the contain driver does); the base's destroy line moved to `crate_destroy_object`, which the
//! hijack crate's own destroy needed too; a vehicle that has gone is removed with its updates unscheduled (a bare removal left the scheduler naming
//! it); the parachute the factory makes now has a contain on both sides, without which every case reaching that path was excluded.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 18 of 18 caught. 1721, 1726, 1734 and 1741 end the full run in a C++ segfault (a case the
//! planted Rust no longer excludes as undefined); each is caught by mismatches first (11 in the first 20 cases, 10 in the first 200, 34 in the first
//! 20, 15 in the first 20).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1721 | the target ignored (the hijacker never follows it) | 11 in 20 cases |
//! | 1722 | the hijacker's level left out of the shared veterancy | 123 |
//! | 1723 | the hijacker not shown again | 132 |
//! | 1724 | the update left on after the vehicle has gone | 1,281 |
//! | 1725 | the in-vehicle and airborne flags not saved | 1,530 |
//! | 1726 | `loadPostProcess` loses the target | 10 in 200 cases |
//! | 1731 | a vehicle immune to capture hijacked | 94 |
//! | 1732 | a drone hijacked | 167 |
//! | 1733 | a hijacked vehicle hijacked again | 2,012 |
//! | 1734 | the relationship test inverted | 34 in 20 cases |
//! | 1735 | a transport with passengers hijacked | 322 |
//! | 1736 | the AI's goal ignored | 1,138 |
//! | 1737 | no `VehicleStolen` | 276 |
//! | 1738 | the vehicle's AI not sent to where it stands | 1,139 |
//! | 1739 | the vehicle's veterancy not raised | 2,066 |
//! | 1740 | the hijacker kept when the vehicle cannot eject | 2,376 |
//! | 1741 | the `HijackerUpdate` not turned on | 15 in 20 cases |
//! | 1742 | the hijacker left in the partition manager | 733 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::{PlayerType, Relationship};
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_HIJACKED, OBJECT_STATUS_MASKED,
    OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_UNSELECTABLE,
};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::upgrade::UpgradeCenter;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::convert_to_hijacked_vehicle_crate_collide::ConvertToHijackedVehicleCrateCollideModuleData;
use rts_lockstep_engine::logic::modules::hijacker_update::HijackerUpdateModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{veterancy_from_i32, PRIVATE_EFFECTIVELY_DEAD};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{EvaMessage, ObjectClientEvent, RadarCall};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r13e.bin");
const XFER_FILE: &str = "l9_r13e.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
const CLASS: &str = "ConvertToHijackedVehicleCrateCollide";
/// The flags of `cfg[3]` an op may change: all but the `EjectPilotDie` (bit 7) and the `HijackerUpdate` (bit 9), the modules being fixed.
const OP_FLAGS: u32 = 0x3FD7F;

/// The kinds the vehicle is given, by bit (the driver's `g_r13eKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 8] = [
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_AIRCRAFT, "AIRCRAFT"),
    (KINDOF_BOAT, "BOAT"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_IMMUNE_TO_CAPTURE, "IMMUNE_TO_CAPTURE"),
    (KINDOF_DRONE, "DRONE"),
    (KINDOF_TRANSPORT, "TRANSPORT"),
];

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}

#[derive(Default)]
struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    fn id(&mut self, name: &Option<Vec<u8>>) -> i32 {
        match name {
            None => 0,
            Some(n) => {
                let next = self.0.len() as i32 + 1;
                *self.0.entry(n.clone()).or_insert(next)
            }
        }
    }
}

type Read = (Option<ModuleDataKind>, Option<ModuleDataKind>);
fn read_crate(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    out.0 = Some(ModuleDataKind::ConvertToHijackedVehicleCrateCollide(
        Arc::new(
            ConvertToHijackedVehicleCrateCollideModuleData::friend_new_module_data(Some(ini))?,
        ),
    ));
    Ok(())
}
fn read_hijacker(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    out.1 = Some(ModuleDataKind::HijackerUpdate(Arc::new(
        HijackerUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Read) {
    let blocks = [
        BlockParse {
            token: "D",
            parse: read_crate,
        },
        BlockParse {
            token: "H",
            parse: read_hijacker,
        },
    ];
    let mut out = (None, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r13e_run`: the crate base, then the `HijackerUpdate`'s two names.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, hijack: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::ConvertToHijackedVehicleCrateCollide(d) = data else {
        unreachable!("not a hijack crate")
    };
    let ModuleDataKind::HijackerUpdate(h) = hijack else {
        unreachable!("not a HijackerUpdate")
    };
    let c = &d.crate_data;
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of.test(i)));
    }
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of_not.test(i)));
    }
    put_u32(v, u32::from(c.is_forbid_owner_player));
    put_u32(v, u32::from(c.is_building_pickup));
    put_u32(v, u32::from(c.is_human_only_pickup));
    let sci = SCIENCES
        .iter()
        .position(|s| s.as_bytes().eq_ignore_ascii_case(&c.pickup_science))
        .map_or(-1, |i| i as i32);
    put_i32(v, sci);
    put_i32(v, fx.id(&c.execute_fx));
    put_str(v, &c.execution_animation_template);
    put_u32(v, c.execute_animation_display_time_in_seconds.to_bits());
    put_u32(v, c.execute_animation_z_rise_per_second.to_bits());
    put_u32(v, u32::from(c.execute_animation_fades));
    put_str(v, &h.attach_to_bone);
    put_str(v, &h.parachute_name);
}

/// A dump as text, for the comparison.
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
    let kinds: Vec<u32> = (0..2 * KINDOF_COUNT).map(|_| u32_(&mut p)).collect();
    let set: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0)
        .map(|(i, _)| i)
        .collect();
    let flags = [u32_(&mut p), u32_(&mut p), u32_(&mut p)];
    let sci = u32_(&mut p) as i32;
    let fx = u32_(&mut p) as i32;
    let anim = str_(&mut p);
    let (t, z, fades) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let (bone, chute) = (str_(&mut p), str_(&mut p));
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades} bone {bone:?} chute {chute:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p) as i32;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..16).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:x?}"));
    }
    out
}

// -- generators --

fn kind_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = KINDS[g.below(8) as usize].1;
        t.push(match g.below(10) {
            0 => "NONE".to_string(),
            1..=6 => format!("+{n}"),
            _ => format!("-{n}"),
        });
    }
    t.join(" ")
}

/// The two blocks: `D` (the crate fields) and `H` (the `HijackerUpdate`'s), each field present with some probability; now and then a bogus token or
/// an unknown field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut d: Vec<String> = Vec::new();
    if g.below(100) < 8 {
        d.push(format!("RequiredKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 8 {
        d.push(format!("ForbiddenKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 10 {
        d.push(format!("ForbidOwnerPlayer = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 8 {
        d.push(format!("BuildingPickup = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 8 {
        d.push(format!("HumanOnly = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 30 {
        d.push(format!(
            "ExecuteFX = {}",
            pick(g, &["FX_A", "FX_B", "None"])
        ));
    }
    let mut h: Vec<String> = Vec::new();
    if g.below(100) < 30 {
        h.push(format!(
            "AttachToTargetBone = {}",
            pick(g, &["Bone01", "DRIVER"])
        ));
    }
    if g.below(100) < 60 {
        h.push(format!(
            "ParachuteName = {}",
            pick(g, &["Missing", "U1", "U1"])
        ));
    }
    if !clean {
        let bad = match g.below(3) {
            0 => "RequiredKindOf = +BOGUS".to_string(),
            1 => "EffectRange = 5".to_string(),
            _ => "NoSuchField = 1".to_string(),
        };
        if g.below(2) == 0 {
            d.push(bad);
        } else {
            h.push(bad);
        }
    }
    let mut s = String::from("D\n");
    for l in &d {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\nH\n");
    for l in &h {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    collides: u64,
    destroyed: u64,
    xfers: u64,
    updates: u64,
    gone: u64,
}

struct Text;
impl GameTextFetch for Text {
    fn fetch(&self, _label: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
}
struct SciCtx {
    store: ScienceStore,
    names: NameKeyGenerator,
}
fn science_block(ini: &mut Ini<'_>, c: &mut SciCtx) -> IniResult<()> {
    c.store.parse_science_definition(ini, &mut c.names, &Text)
}

fn run_xfer(
    w: &mut World,
    me: ObjectId,
    idx: usize,
    how: u32,
    saved: &[u8],
) -> (i32, Vec<u8>, u32) {
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
            b.truncate(512);
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

fn set_player_of_team(w: &mut World, team: u32, player: Option<u32>) {
    match player {
        Some(p) => {
            w.cuts.team_players.insert(team, p as i32);
        }
        None => {
            w.cuts.team_players.remove(&team);
        }
    }
}

/// The vehicle's template: the test's kinds.
fn set_kinds(w: &mut World, id: ObjectId, bits: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects
        .find_mut(id)
        .unwrap()
        .thing
        .friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The scene's module indices: the hijacker's AI, the vehicle's AI and contain.
struct Slots {
    crate_ai: Option<u16>,
    other_ai: u16,
    contain: u16,
}

/// The flags of `cfg[3]` that ops change, applied to the scene (while the vehicle is there).
fn apply_flags(w: &mut World, flags: u32, s: &Slots) {
    let (me, other) = (1, 2);
    w.objects.find_mut(me).unwrap().ai = if flags & 1 != 0 { s.crate_ai } else { None };
    if let Some(ai) = s.crate_ai {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules
            [ai as usize]
            .body
            .as_mut()
        {
            p.answers.goal_object = if flags & 2 != 0 { other } else { INVALID_ID };
        }
    }
    w.friend_bind_to_drawable(
        me,
        if flags & 256 != 0 {
            7
        } else {
            INVALID_DRAWABLE_ID
        },
    );
    w.objects.find_mut(me).unwrap().experience.level = veterancy_from_i32((flags >> 10 & 3) as i32);
    let rel = match flags >> 14 & 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    };
    w.cuts.team_relationships.insert((1, 2), rel);
    let local = flags >> 16 & 3;
    w.players
        .friend_set_local_index(if local == 3 { None } else { Some(local as i32) });
    if w.objects.find(other).is_none() {
        return;
    }
    w.objects.find_mut(other).unwrap().ai = if flags & 4 != 0 {
        Some(s.other_ai)
    } else {
        None
    };
    if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(other).unwrap().modules
        [s.other_ai as usize]
        .body
        .as_mut()
    {
        p.answers.dozer = flags & 8 != 0;
    }
    let o = w.objects.find_mut(other).unwrap();
    if flags & 16 != 0 {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
    o.status
        .set(OBJECT_STATUS_HIJACKED, i32::from(flags & 32 != 0));
    o.contain = if flags & 64 != 0 {
        Some(s.contain)
    } else {
        None
    };
    o.experience.level = veterancy_from_i32((flags >> 12 & 3) as i32);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    names: NameKeyGenerator,
    center: UpgradeCenter,
    data: ModuleDataKind,
    hijack: ModuleDataKind,
    cfg: &[u32; 12],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &hijack, &mut fx);

    let mut sc = SciCtx {
        store: ScienceStore::new(),
        names,
    };
    let defs: String = SCIENCES
        .iter()
        .map(|s| format!("Science {s}\nEnd\n"))
        .collect();
    static BLOCKS: [BlockParse<SciCtx>; 1] = [BlockParse {
        token: "Science",
        parse: science_block,
    }];
    Ini::load(
        b"Data\\INI\\Science.ini",
        defs.as_bytes(),
        IniLoadType::Overwrite,
        None,
        &BLOCKS,
        &mut sc,
    )
    .expect("sciences load");

    // the thing factory knows U1 and U2 (the parachute)
    let mut factory = ThingFactory::new();
    let mut template_names: BTreeMap<u16, String> = BTreeMap::new();
    for name in ["U1", "U2"] {
        factory
            .new_template(name.as_bytes(), 0)
            .expect("a template");
        let id = factory
            .find_template(name.as_bytes())
            .expect("the new template")
            .data
            .get_template_id();
        template_names.insert(id, name.to_string());
    }

    let mut w = World::new();
    w.names = sc.names;
    w.statics = Arc::new(GameStatics {
        science_store: sc.store,
        upgrade_center: center,
        thing_factory: factory,
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.body.probe_trace = true;
    w.cuts.globals.draw_icon_ui = false;
    w.cuts.globals.anim2d_collection_present = false;
    w.cuts.globals.gravity = -1.0;
    w.cuts.crate_new_object_contain = match cfg[10] {
        0 => None,
        1 => Some(false),
        _ => Some(true),
    };
    for team in [1u32, 2, 11, 12, 13] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    let flags0 = cfg[3];
    for p in 0..3u32 {
        let pl = w.players.get_nth_mut(p as i32).unwrap();
        pl.player_type = if flags0 >> (18 + p) & 1 != 0 {
            PlayerType::Computer
        } else {
            PlayerType::Human
        };
        pl.default_team = Some(11 + p);
        set_player_of_team(&mut w, 11 + p, Some(p));
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));

    // the hijacker: the crate module, his AI, maybe his HijackerUpdate
    let next = w.next_object_id();
    let m = rts_lockstep_engine::logic::modules::new_module(
        next,
        CLASS.as_bytes(),
        b"ModuleTag_Crate",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        0.0,
        Default::default(),
    )));
    let mut mods = vec![m, ai];
    let has_hijacker = flags0 & 512 != 0;
    if has_hijacker {
        mods.push(
            rts_lockstep_engine::logic::modules::new_module(
                next,
                b"HijackerUpdate",
                b"ModuleTag_Hijack",
                &hijack,
            )
            .unwrap()
            .unwrap(),
        );
    }
    let plain_template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template: plain_template.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: mods,
        })
        .unwrap();
    assert_eq!(me, 1);
    let crate_ai = w.objects.find(me).unwrap().ai;
    {
        let o = w.objects.find_mut(me).unwrap();
        o.name = b"Pilot".to_vec();
        o.vision_range = f32::from_bits(cfg[8]);
        o.shroud_clearing_range = f32::from_bits(cfg[9]);
    }
    // the vehicle: an AI, a contain, maybe an EjectPilotDie
    let mut vai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    vai.is_ai = true;
    vai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        0.0,
        Default::default(),
    )));
    let mut vc = NewModule::plain(b"CtContain", b"ModuleTag_Contain");
    vc.interfaces = MODULEINTERFACE_CONTAIN;
    vc.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers {
            contained: (3..3 + cfg[4] as i32).collect(),
            ..Default::default()
        },
        0.0,
        Default::default(),
    )));
    let mut vmods = vec![vai, vc];
    if flags0 & 128 != 0 {
        vmods.push(NewModule::plain(b"EjectPilotDie", b"ModuleTag_Eject"));
    }
    let other = w
        .new_object(NewObject {
            template: plain_template.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(2),
            modules: vmods,
        })
        .unwrap();
    assert_eq!(other, 2);
    let o = w.objects.find(other).unwrap();
    let slots = Slots {
        crate_ai,
        other_ai: o.ai.unwrap(),
        contain: o.contain.unwrap(),
    };
    set_kinds(&mut w, other, cfg[2]);
    w.set_position(
        other,
        &Coord3D {
            x: f32::from_bits(cfg[5]),
            y: f32::from_bits(cfg[6]),
            z: f32::from_bits(cfg[7]),
        },
    );
    let mut flags = flags0;
    apply_flags(&mut w, flags, &slots);
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let hidx = w.find_module(me, b"HijackerUpdate");
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0i32);
        let target = if o[1] != 0 { other } else { INVALID_ID };
        let xidx = if o[2] != 0 {
            hidx.expect("a HijackerUpdate xfer only with one")
        } else {
            idx
        };
        match o[0] {
            0 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::OnCollide {
                        other: target,
                        loc: None,
                        normal: None,
                    },
                );
                tally.collides += 1;
            }
            1 => {
                res = i32::from(
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::WouldLikeToCollideWith { other: target },
                    )
                    .as_bool(),
                )
            }
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => {
                flags = (flags & !OP_FLAGS) | (o[1] & OP_FLAGS);
                apply_flags(&mut w, flags, &slots);
            }
            4 => set_kinds(&mut w, other, o[1]),
            5 => {
                let p = w.objects.find(other).unwrap().get_position();
                w.set_position(
                    other,
                    &Coord3D {
                        z: f32::from_bits(o[1]),
                        ..p
                    },
                );
            }
            6 => {
                if let Some(h) = hidx {
                    let u = w
                        .update_handle_of_module(ModuleHandle {
                            object: me,
                            index: h,
                        })
                        .expect("an update module");
                    res = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0 as i32;
                    tally.updates += 1;
                }
            }
            7..=9 => {
                let r = run_xfer(&mut w, me, xidx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            10 => {
                // gone from the world, its updates unscheduled, as `processDestroyList` does for it (the hijacker a crate destroyed stays: the
                // driver's destroy is a line)
                w.sched.erase_object_updates(&mut w.objects, other);
                w.objects.remove(other);
                tally.gone += 1;
            }
            11 => {
                if let Some(h) = hidx {
                    w.module_call(me, h, ModuleCall::LoadPostProcess);
                }
            }
            _ => unreachable!(),
        }
        if (2..=5).contains(&o[0]) || o[0] == 10 {
            // a change to the scene (the drawable's binding makes client events of its own): the driver's changes log nothing
            w.cuts.trace = Some(Vec::new());
            w.cuts.log = Some(Vec::new());
            events_before = w.events.events().len();
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, res);
        // every client event has its `evt` line, so the lines and the events are paired before choosing
        let events: Vec<Option<ObjectClientEvent>> = w.events.events()[events_before..]
            .iter()
            .map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let mut event_iter = events.into_iter();
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::ObjectSetTeam { object, team } => log.push(format!(
                        "setteam {object} {}",
                        team.map_or(-1, |t| t as i32)
                    )),
                    CutCall::ScriptEngineTransferObjectName { name, object } => log.push(format!(
                        "transfername {} {object}",
                        String::from_utf8_lossy(name)
                    )),
                    CutCall::ExperienceSetVeterancyLevel { object, level, .. } => {
                        log.push(format!("xplevel {object} {level}"))
                    }
                    CutCall::ThingFactoryNewObject { template, team } => log.push(format!(
                        "newobject {} {}",
                        template_names[template],
                        team.map_or(-1, |t| t as i32)
                    )),
                    CutCall::Contain(ContainCall::LeaveGroup { object }) => {
                        log.push(format!("leavegroup {object}"))
                    }
                    CutCall::Contain(ContainCall::PartitionUnregisterObject { object }) => {
                        log.push(format!("punregister {object}"))
                    }
                    CutCall::Contain(ContainCall::PartitionRegisterObject { object }) => {
                        log.push(format!("pregister {object}"))
                    }
                    CutCall::Contain(ContainCall::DrawableSetHidden { object, hidden }) => {
                        log.push(format!("hidden {object} {}", i32::from(*hidden)))
                    }
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                log.push(r.to_string());
            } else if t.starts_with("evt ") {
                match event_iter.next().flatten() {
                    Some(ObjectClientEvent::Radar {
                        call: RadarCall::TryInfiltrationEvent,
                        object_id,
                        ..
                    }) => log.push(format!("infiltrate {object_id}")),
                    Some(ObjectClientEvent::Eva { message }) => log.push(format!(
                        "eva {}",
                        match message {
                            EvaMessage::VehicleStolen => 17,
                            other => panic!("unexpected eva {other:?}"),
                        }
                    )),
                    Some(ObjectClientEvent::AudioAddEvent {
                        name, object_id, ..
                    }) => log.push(format!(
                        "audio {} - -1 {object_id}",
                        String::from_utf8_lossy(&name)
                    )),
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => {
                        log.push(format!("fx{}", fx.id(&Some(name))))
                    }
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("destroy ") {
                tally.destroyed += 1;
                log.push(format!("destroy {r}"));
            }
        }
        tally.effects += log.len() as u64;
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
        let h = w.objects.find(me).unwrap();
        let p = h.get_position();
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
        put_u32(
            &mut v,
            u32::from(h.test_status(OBJECT_STATUS_NO_COLLISIONS))
                | u32::from(h.test_status(OBJECT_STATUS_MASKED)) << 1
                | u32::from(h.test_status(OBJECT_STATUS_UNSELECTABLE)) << 2,
        );
        let crate_level = h.experience.level as u32;
        match w.objects.find(other) {
            Some(ob) => {
                put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_HIJACKED)));
                put_u32(&mut v, crate_level);
                put_u32(&mut v, ob.experience.level as u32);
                put_u32(&mut v, ob.vision_range.to_bits());
                put_u32(&mut v, ob.shroud_clearing_range.to_bits());
            }
            None => {
                put_u32(&mut v, 0);
                put_u32(&mut v, crate_level);
                for _ in 0..3 {
                    put_u32(&mut v, 0);
                }
            }
        }
        let words = match hidx {
            Some(hi) => {
                let ob = w.objects.find(me).unwrap();
                match ob.behaviors[ob.modules[hi as usize].update_index.unwrap() as usize]
                    .body
                    .as_ref()
                {
                    Some(UpdateBody::HijackerUpdate(m)) => m.state_words(),
                    _ => panic!("the HijackerUpdate"),
                }
            }
            None => vec![0; 7],
        };
        for x in words {
            put_u32(&mut v, x);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R13E_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn hijack_crate_and_hijacker_update_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0013_E41C);
    let mut read = Check::new("hijack crate R13e data read (throw, message)");
    let mut run = Check::new("hijack crate R13e ops (data, effects, xfers, hijacker, vehicle)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let floats = [&0.0f32, &10.0, &-35.5, &120.25];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R13e{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly an enemy vehicle with an AI, an EjectPilotDie, a hijacker with an AI after it and a HijackerUpdate
        let player = |g: &mut Gen| match g.below(12) {
            0 => NO_PLAYER,
            1 => 0,
            _ => 1 + g.below(2),
        };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = if g.below(4) != 0 {
            1 | if g.below(4) == 0 { 128 } else { 0 }
        } else {
            g.u32() & 0xff
        };
        cfg[3] = u32::from(g.below(4) != 0)
            | u32::from(g.below(5) != 0) << 1
            | u32::from(g.below(8) != 0) << 2
            | u32::from(g.below(4) == 0) << 3
            | u32::from(g.below(12) == 0) << 4
            | u32::from(g.below(10) == 0) << 5
            | u32::from(g.below(3) == 0) << 6
            | u32::from(g.below(3) != 0) << 7
            | u32::from(g.below(2) == 0) << 8
            | u32::from(g.below(5) != 0) << 9
            | g.below(4) << 10
            | g.below(4) << 12
            | (if g.below(4) != 0 { 0 } else { g.below(3) }) << 14
            | g.below(4) << 16
            | (g.u32() & g.u32() & 7) << 18;
        cfg[4] = *pick(&mut g, &[&0u32, &0, &1, &2]);
        cfg[5] = pick(&mut g, &floats).to_bits();
        cfg[6] = pick(&mut g, &floats).to_bits();
        cfg[7] = if g.below(3) == 0 {
            20.0f32.to_bits()
        } else {
            0
        };
        cfg[8] = (*pick(&mut g, &[&0.0f32, &150.0, &300.5])).to_bits();
        cfg[9] = (*pick(&mut g, &[&0.0f32, &200.0, &450.25])).to_bits();
        cfg[10] = *pick(&mut g, &[&0u32, &1, &2, &2]);
        let has_hijacker = cfg[3] & 512 != 0;
        let n_ops = 1 + g.below(16) as usize;
        let mut gone = false;
        let mut ops: Vec<[u32; 6]> = Vec::new();
        for _ in 0..n_ops {
            let xm = u32::from(has_hijacker && g.below(2) == 0);
            let op = if gone {
                // the vehicle has gone: the hijacker's own ops only
                match g.below(10) {
                    0..=5 => [6, 0, 0, 0, 0, 0],
                    6 => [7, 0, xm, 0, 0, 0],
                    7 => [8, 0, xm, 0, 0, 0],
                    8 => [9, 0, xm, 0, 0, 0],
                    _ => [11, 0, 0, 0, 0, 0],
                }
            } else {
                match g.below(44) {
                    0..=13 => [0, u32::from(g.below(12) != 0), 0, 0, 0, 0],
                    14..=16 => [1, g.below(2), 0, 0, 0, 0],
                    17..=18 => [2, g.below(4), 0, 0, 0, 0],
                    19..=21 => [
                        3,
                        (g.u32() & OP_FLAGS) | 3 | 4 | u32::from(g.below(4) == 0) << 16,
                        0,
                        0,
                        0,
                        0,
                    ],
                    22..=23 => [
                        4,
                        if g.below(2) == 0 { 1 } else { g.u32() & 0xff },
                        0,
                        0,
                        0,
                        0,
                    ],
                    24..=25 => [
                        5,
                        if g.below(2) == 0 {
                            20.0f32.to_bits()
                        } else {
                            0
                        },
                        0,
                        0,
                        0,
                        0,
                    ],
                    26..=31 => [6, 0, 0, 0, 0, 0],
                    32..=33 => [7, 0, xm, 0, 0, 0],
                    34..=35 => [8, 0, xm, 0, 0, 0],
                    36 => [9, 0, xm, 0, 0, 0],
                    37..=38 => [11, 0, 0, 0, 0, 0],
                    _ => [10, 0, 0, 0, 0, 0],
                }
            };
            if op[0] == 10 {
                gone = true;
            }
            ops.push(op);
        }

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(
                ctx.names.clone(),
                ctx.center.clone(),
                data.0.expect("read ok"),
                data.1.expect("read ok"),
                &cfg,
                &ops,
                &mut tally,
            );
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r13e_run(
                cfile.as_ptr(),
                cxfer.as_ptr(),
                cfg.as_ptr(),
                ops.len() as i32,
                flat.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut len,
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut code,
            )
        };
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || {
            format!(
                "case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}",
                show(&text)
            )
        };
        read.eq(
            &outcome,
            &Outcome {
                status,
                detail,
                crc: 0,
            },
            what,
        );
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in [
                "infiltrate",
                "eva",
                "setteam",
                "aimove",
                "aiidle",
                "canceltask",
                "audio",
                "transfername",
                "xplevel",
                "leavegroup",
                "punregister",
                "pregister",
                "hidden",
                "newobject",
                "validfor",
                "addtocontain",
                "destroy",
            ] {
                *counts.entry(key.to_string()).or_default() +=
                    l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
            eprintln!(
                "--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}",
                show(&text),
                dr.get(i),
                dc.get(i)
            );
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "hijack crate R13e: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onCollide calls, {} destroyed, {} updates, {} vehicles gone, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.updates, tally.gone, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        tally.destroyed * 20 > cases as u64,
        "the generator must reach the crate's actions"
    );
}
