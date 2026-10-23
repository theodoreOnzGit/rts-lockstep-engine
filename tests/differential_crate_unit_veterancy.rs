#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/{Unit,Veterancy}CrateCollide.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `UnitCrateCollide` and `VeterancyCrateCollide` (`logic::modules::{unit,veterancy}_crate_collide`) against the unchanged C++, `p1w13-sonnet`, batch R13b.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crates_r13b.inc`) on the module build's
//! stand-ins: the crate (id 1: player, veterancy level, the name `Pilot`, maybe an AI with a goal), the collector (id 2: kinds, player, position,
//! orientation, level, whether it trains, whether its AI's locomotor flies), two more objects (ids 3 and 4) the range scan may list, three players with
//! default teams 11 to 13, and a thing factory that knows `U1` and `U2` and makes a bare object for each (ids 5 up). `findPositionAround` answers as the
//! test says; `setOrientation`, the experience gain, the range scan and the name transfer are lines.
//!
//! The Rust side is the real thing: a `World` with the same objects, the crate module from `modules::new_module`, `cuts.crate_stub_object_calls` set
//! (the new object is a bare one of the template, its orientation and the experience gain are records, as the driver's are lines), and every op a
//! `World::module_call` or a change to the scene.
//!
//! Each case reads a `D` block (the `CrateCollide` fields and the class's own; mostly well formed) and runs 1 to 14 ops. Compared, bit for bit: the
//! read's throw; the data; per op the throw status, a result, the effects in order (`newobject`, `findpos`, `setorient`, `audio`, `gainexp`, `scan`,
//! `transfername`, `fx`, `destroy`), the xfer status, bytes and CRC, and every object made so far (id, team, position).
//!
//! # Results (2026-10-23, `L9R13B_CASES=9000`)
//!
//! 9,000 cases (8,955 compared, 45 excluded as undefined upstream: an unowned collector of a unit crate, a `NULL` player; 1,949 data blocks threw, as the
//! C++ did), 52,227 ops (20,951 `onCollide` calls, 1,174 crates used, 1,122 objects made, 6,618 xfers) and 66,239 op outputs: **0 mismatches**. Effect
//! lines compared: newobject / findpos / setorient 1,079 each, audio 850, gainexp 378, scan 79, transfername 79, destroy 1,154.
//!
//! Harness corrections on the way, none in the translation: the trace's `evt` lines were paired with the object events only, where the new objects'
//! own client events have lines too; and the driver's players had no player type (the stand-in has no constructor), so `HumanOnly` read uninitialised
//! memory: players are now human or computer as `cfg[3]` bits 12-14 say (the sabotage driver had the same gap: see its test).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 14 of 14 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1661 | at most one unit | 871 |
//! | 1662 | the units on no team | 1,464 |
//! | 1663 | `findPositionAround`'s radius 0 | 578 |
//! | 1664 | the units not turned as `other` | 578 |
//! | 1665 | no pickup sound | 850 |
//! | 1671 | `AddsOwnerVeterancy` ignored (always 1 level) | 71 |
//! | 1672 | a collector above the terrain accepted | 123 |
//! | 1673 | an untrainable collector accepted | 103 |
//! | 1674 | a pilot entering another player's vehicle | 41 |
//! | 1675 | a pilot entering an aircraft | 20 |
//! | 1676 | the AI's goal ignored | 94 |
//! | 1677 | the bonus scaling inverted | 300 |
//! | 1678 | the range halved | 79 |
//! | 1679 | the pilot's name not handed on | 79 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
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
use rts_lockstep_engine::logic::modules::unit_crate_collide::UnitCrateCollideModuleData;
use rts_lockstep_engine::logic::modules::veterancy_crate_collide::VeterancyCrateCollideModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{veterancy_from_i32, PRIVATE_EFFECTIVELY_DEAD};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r13b.bin");
const XFER_FILE: &str = "l9_r13b.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
const CLASSES: [&str; 2] = ["UnitCrateCollide", "VeterancyCrateCollide"];

/// The kinds a victim is given, by bit (the driver's `g_sabKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 16] = [
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_PARACHUTE, "PARACHUTE"),
    (KINDOF_COMMANDCENTER, "COMMANDCENTER"),
    (KINDOF_FS_POWER, "FS_POWER"),
    (KINDOF_FS_INTERNET_CENTER, "FS_INTERNET_CENTER"),
    (KINDOF_FS_BARRACKS, "FS_BARRACKS"),
    (KINDOF_FS_WARFACTORY, "FS_WARFACTORY"),
    (KINDOF_FS_AIRFIELD, "FS_AIRFIELD"),
    (KINDOF_AIRCRAFT_CARRIER, "AIRCRAFT_CARRIER"),
    (KINDOF_FS_SUPERWEAPON, "FS_SUPERWEAPON"),
    (KINDOF_FS_STRATEGY_CENTER, "FS_STRATEGY_CENTER"),
    (KINDOF_FS_SUPPLY_CENTER, "FS_SUPPLY_CENTER"),
    (KINDOF_FS_SUPPLY_DROPZONE, "FS_SUPPLY_DROPZONE"),
    (KINDOF_FS_FAKE, "FS_FAKE"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_VEHICLE, "VEHICLE"),
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

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    out.1 = Some(if out.0 == 0 {
        ModuleDataKind::UnitCrateCollide(Arc::new(UnitCrateCollideModuleData::friend_new_module_data(Some(ini))?))
    } else {
        ModuleDataKind::VeterancyCrateCollide(Arc::new(VeterancyCrateCollideModuleData::friend_new_module_data(Some(ini))?))
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9r13b_run`: the crate base, then the class's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let c = match data {
        ModuleDataKind::UnitCrateCollide(d) => &d.crate_data,
        ModuleDataKind::VeterancyCrateCollide(d) => &d.crate_data,
        _ => unreachable!("not an R13b crate"),
    };
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of.test(i)));
    }
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of_not.test(i)));
    }
    put_u32(v, u32::from(c.is_forbid_owner_player));
    put_u32(v, u32::from(c.is_building_pickup));
    put_u32(v, u32::from(c.is_human_only_pickup));
    let sci = SCIENCES.iter().position(|s| s.as_bytes().eq_ignore_ascii_case(&c.pickup_science)).map_or(-1, |i| i as i32);
    put_i32(v, sci);
    put_i32(v, fx.id(&c.execute_fx));
    put_str(v, &c.execution_animation_template);
    put_u32(v, c.execute_animation_display_time_in_seconds.to_bits());
    put_u32(v, c.execute_animation_z_rise_per_second.to_bits());
    put_u32(v, u32::from(c.execute_animation_fades));
    match data {
        ModuleDataKind::UnitCrateCollide(d) => {
            put_u32(v, d.unit_count);
            put_str(v, &d.unit_type);
        }
        ModuleDataKind::VeterancyCrateCollide(d) => {
            put_u32(v, d.range_of_effect);
            put_u32(v, u32::from(d.adds_owner_veterancy));
            put_u32(v, u32::from(d.is_pilot));
        }
        _ => unreachable!(),
    }
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], kind: usize, n_ops: usize) -> Vec<String> {
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
    let set: Vec<usize> = kinds.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, _)| i).collect();
    let flags = [u32_(&mut p), u32_(&mut p), u32_(&mut p)];
    let sci = u32_(&mut p) as i32;
    let fx = u32_(&mut p) as i32;
    let anim = str_(&mut p);
    let (t, z, fades) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let own = if kind == 0 { format!("count {} name {:?}", u32_(&mut p), str_(&mut p)) } else { format!("range {} owner {} pilot {}", u32_(&mut p), u32_(&mut p), u32_(&mut p)) };
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades} {own}"));
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
        let n_made = u32_(&mut p);
        let made: Vec<[u32; 5]> = (0..n_made).map(|_| [u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p)]).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} made {made:x?}"));
    }
    out
}

// -- generators --

fn kind_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = KINDS[g.below(16) as usize].1;
        t.push(match g.below(10) {
            0 => "NONE".to_string(),
            1..=6 => format!("+{n}"),
            _ => format!("-{n}"),
        });
    }
    t.join(" ")
}

/// The block `D`: the class's own fields and the crate fields (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if kind == 0 {
        if g.below(100) < 85 {
            lines.push(format!("UnitCount = {}", pick(g, &["0", "1", "2", "3", "1", "2"])));
        }
        if g.below(100) < 85 {
            lines.push(format!("UnitName = {}", pick(g, &["U1", "U2", "U1", "Missing"])));
        }
    } else {
        if g.below(100) < 60 {
            lines.push(format!("EffectRange = {}", pick(g, &["0", "0", "50", "200"])));
        }
        if g.below(100) < 40 {
            lines.push(format!("AddsOwnerVeterancy = {}", gen_bool(g, clean)));
        }
        if g.below(100) < 50 {
            lines.push(format!("IsPilot = {}", gen_bool(g, clean)));
        }
    }
    if g.below(100) < 8 {
        lines.push(format!("RequiredKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 8 {
        lines.push(format!("ForbiddenKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 10 {
        lines.push(format!("ForbidOwnerPlayer = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 15 {
        lines.push(format!("BuildingPickup = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 8 {
        lines.push(format!("HumanOnly = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 30 {
        lines.push(format!("ExecuteFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if !clean {
        lines.push(match g.below(4) {
            0 => "RequiredKindOf = +BOGUS".to_string(),
            1 => "UnitCount = many".to_string(),
            2 => "IsPilot = Perhaps".to_string(),
            _ => "NoSuchField = 1".to_string(),
        });
    }
    let mut s = String::from("D\n");
    for l in &lines {
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
    made: u64,
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

/// The collector's template: the test's kinds and whether it trains.
fn set_kinds(w: &mut World, id: ObjectId, bits: u32, trainable: bool) {
    let mut values = ObjectTemplateValues { is_trainable: trainable, ..Default::default() };
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects.find_mut(id).unwrap().thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The range scan's list: the objects of the mask (bits 0-3 over objects 1 to 4), reversed with bit 4.
fn set_scan(w: &mut World, a: u32) {
    let mut v: Vec<ObjectId> = (1..=4).filter(|k| a >> (k - 1) & 1 != 0).collect();
    if a & 16 != 0 {
        v.reverse();
    }
    w.cuts.range_scan = v;
}

/// The flags of `cfg[3]` that ops change, applied to the scene.
fn apply_flags(w: &mut World, flags: u32, kinds: u32, crate_ai: Option<u16>) {
    let (me, other) = (1, 2);
    w.objects.find_mut(me).unwrap().ai = if flags & 1 != 0 { crate_ai } else { None };
    if let Some(ai) = crate_ai {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[ai as usize].body.as_mut() {
            p.answers.goal_object = if flags & 2 != 0 { other } else { INVALID_ID };
        }
    }
    // the collector's AI is a module index nothing calls (only its presence is read)
    w.objects.find_mut(other).unwrap().ai = if flags & 16 != 0 { Some(u16::MAX) } else { None };
    let o = w.objects.find_mut(other).unwrap();
    if flags & 32 != 0 {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
    if flags & 128 != 0 {
        w.cuts.ai_airborne.insert(other);
    } else {
        w.cuts.ai_airborne.remove(&other);
    }
    w.objects.find_mut(me).unwrap().experience.level = veterancy_from_i32((flags >> 8 & 3) as i32);
    w.objects.find_mut(other).unwrap().experience.level = veterancy_from_i32((flags >> 10 & 3) as i32);
    set_kinds(w, other, kinds, flags & 64 != 0);
}

fn set_find_position(w: &mut World, has: bool, x: u32, y: u32) {
    w.cuts.contain.find_position = has.then(|| Coord3D { x: f32::from_bits(x), y: f32::from_bits(y), z: 0.0 });
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(kind: usize, names: NameKeyGenerator, center: UpgradeCenter, data: ModuleDataKind, cfg: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut sc = SciCtx { store: ScienceStore::new(), names };
    let defs: String = SCIENCES.iter().map(|s| format!("Science {s}\nEnd\n")).collect();
    static BLOCKS: [BlockParse<SciCtx>; 1] = [BlockParse { token: "Science", parse: science_block }];
    Ini::load(b"Data\\INI\\Science.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut sc).expect("sciences load");

    // the thing factory knows U1 and U2
    let mut factory = ThingFactory::new();
    let mut template_names: BTreeMap<u16, String> = BTreeMap::new();
    for name in ["U1", "U2"] {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let id = factory.find_template(name.as_bytes()).expect("the new template").data.get_template_id();
        template_names.insert(id, name.to_string());
    }

    let mut w = World::new();
    w.names = sc.names;
    w.statics = Arc::new(GameStatics { science_store: sc.store, upgrade_center: center, thing_factory: factory, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.globals.draw_icon_ui = false;
    w.cuts.globals.anim2d_collection_present = false;
    w.cuts.globals.gravity = -1.0;
    for team in [1u32, 2, 3, 4, 11, 12, 13] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    for p in 0..3u32 {
        w.players.get_nth_mut(p as i32).unwrap().default_team = Some(11 + p);
        w.players.get_nth_mut(p as i32).unwrap().player_type = if cfg[3] >> (12 + p) & 1 != 0 { PlayerType::Computer } else { PlayerType::Human };
        set_player_of_team(&mut w, 11 + p, Some(p));
    }
    let flags0 = cfg[3];
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));

    // the crate: the crate module, then its AI
    let class = CLASSES[kind];
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), class.as_bytes(), b"ModuleTag_Crate", &data).unwrap().unwrap();
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    let plain_template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, ai] }).unwrap();
    assert_eq!(me, 1);
    w.objects.find_mut(me).unwrap().name = b"Pilot".to_vec();
    let crate_ai = w.objects.find(me).unwrap().ai;
    for team in 2..=4u32 {
        let id = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(team), modules: Vec::new() }).unwrap();
        assert_eq!(id, team as i32);
    }
    let other = 2;
    w.set_position(other, &Coord3D { x: f32::from_bits(cfg[4]), y: f32::from_bits(cfg[5]), z: f32::from_bits(cfg[6]) });
    w.contain_scene_set_orientation(other, f32::from_bits(cfg[7]));
    set_find_position(&mut w, cfg[8] != 0, cfg[9], cfg[10]);
    set_scan(&mut w, cfg[11]);
    let mut flags = flags0;
    let mut kinds = cfg[2];
    apply_flags(&mut w, flags, kinds, crate_ai);
    let idx = w.find_module(me, class.as_bytes()).unwrap();
    let first_made = w.next_object_id();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0i32);
        let target = if o[1] != 0 { other } else { INVALID_ID };
        match o[0] {
            0 => {
                w.module_call(me, idx, ModuleCall::OnCollide { other: target, loc: None, normal: None });
                tally.collides += 1;
            }
            1 => res = i32::from(w.module_call(me, idx, ModuleCall::WouldLikeToCollideWith { other: target }).as_bool()),
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => {
                flags = (flags & !0xFF3) | (o[1] & 0xFF3);
                apply_flags(&mut w, flags, kinds, crate_ai);
            }
            4 => {
                kinds = o[1];
                set_kinds(&mut w, other, kinds, flags & 64 != 0);
            }
            5 => {
                let p = *w.objects.find(other).unwrap().thing.position();
                w.set_position(other, &Coord3D { z: f32::from_bits(o[1]), ..p });
            }
            6 => set_find_position(&mut w, o[1] & 1 != 0, o[2], o[3]),
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            10 => set_scan(&mut w, o[1]),
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, res);
        // every client event has its `evt` line (the new objects' own events too), so the lines and the events are paired before choosing
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
                    CutCall::ThingFactoryNewObject { template, team } => {
                        log.push(format!("newobject {} {}", template_names[template], team.map_or(-1, |t| t as i32)));
                        tally.made += 1;
                    }
                    CutCall::Contain(ContainCall::PartitionFindPositionAround { position: p, min_radius, max_radius, flags }) => log.push(format!(
                        "findpos {:08x} {:08x} {:08x} {:08x} {:08x} {flags}",
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        min_radius.to_bits(),
                        max_radius.to_bits()
                    )),
                    CutCall::ObjectSetOrientation { object, angle } => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                    CutCall::ExperienceGainForLevel { object, levels, can_scale_for_bonus } => {
                        log.push(format!("gainexp {object} {levels} {}", i32::from(*can_scale_for_bonus)))
                    }
                    // `FROM_CENTER_2D` is 0
                    CutCall::PartitionIterateSamePlayerInRange { object, range } => log.push(format!("scan {object} {:08x} 0", range.to_bits())),
                    CutCall::ScriptEngineTransferObjectName { name, object } => log.push(format!("transfername {} {object}", String::from_utf8_lossy(name))),
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match event_iter.next().flatten() {
                    Some(ObjectClientEvent::Sound { sound, object_id, position }) => {
                        let name = match sound {
                            ObjectSound::CrateFreeUnit => "crateFreeUnit",
                            other => panic!("unexpected sound {other:?}"),
                        };
                        let pos = match position {
                            Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {name} {pos} -1 {object_id}"));
                    }
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => log.push(format!("fx{}", fx.id(&Some(name)))),
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
        let made: Vec<ObjectId> = (first_made..w.next_object_id()).filter(|id| w.objects.find(*id).is_some()).collect();
        put_u32(&mut v, made.len() as u32);
        for id in made {
            let o = w.objects.find(id).unwrap();
            put_u32(&mut v, id as u32);
            put_i32(&mut v, w.get_team(id).map_or(-1, |t| t as i32));
            let p = o.thing.position();
            put_u32(&mut v, p.x.to_bits());
            put_u32(&mut v, p.y.to_bits());
            put_u32(&mut v, p.z.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R13B_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn unit_and_veterancy_crates_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0013_B0C8);
    let mut read = Check::new("unit/veterancy crates R13b data read (throw, message)");
    let mut run = Check::new("unit/veterancy crates R13b ops (data, effects, xfers, objects made)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 2];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let floats = [&0.0f32, &10.0, &-35.5, &120.25];
    for case in 0..cases {
        let kind = case % 2;
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R13b{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a collector of the crate's player, trainable, on the ground, with the crate's AI after it
        let player = |g: &mut Gen| if g.below(12) == 0 { NO_PLAYER } else { g.below(3) };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = if g.below(3) != 0 { cfg[0] } else { player(&mut g) };
        cfg[2] = if g.below(3) == 0 { g.u32() & g.u32() & 0xffff } else { 1 << (14 + g.below(2)) };
        cfg[3] = u32::from(g.below(4) != 0)
            | u32::from(g.below(5) != 0) << 1
            | u32::from(g.below(3) == 0) << 4
            | u32::from(g.below(10) == 0) << 5
            | u32::from(g.below(5) != 0) << 6
            | u32::from(g.below(4) == 0) << 7
            | g.below(4) << 8
            | g.below(4) << 10
            | (g.u32() & g.u32() & 7) << 12;
        cfg[4] = pick(&mut g, &floats).to_bits();
        cfg[5] = pick(&mut g, &floats).to_bits();
        cfg[6] = if g.below(6) == 0 { 20.0f32.to_bits() } else { 0 };
        cfg[7] = pick(&mut g, &[&0.0f32, &1.5, &-0.75]).to_bits();
        cfg[8] = g.below(2);
        cfg[9] = pick(&mut g, &floats).to_bits();
        cfg[10] = pick(&mut g, &floats).to_bits();
        cfg[11] = g.below(32);
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=15 => [0, u32::from(g.below(12) != 0), 0, 0, 0, 0],
                16..=19 => [1, g.below(2), 0, 0, 0, 0],
                20..=21 => [2, g.below(4), 0, 0, 0, 0],
                22..=25 => [3, g.u32() & 0xFF3, 0, 0, 0, 0],
                26..=27 => [4, if g.below(2) == 0 { g.u32() & 0xffff } else { 1 << (14 + g.below(2)) }, 0, 0, 0, 0],
                28..=29 => [5, if g.below(2) == 0 { 20.0f32.to_bits() } else { 0 }, 0, 0, 0, 0],
                30..=31 => [6, g.below(2), pick(&mut g, &floats).to_bits(), pick(&mut g, &floats).to_bits(), 0, 0],
                32..=33 => [7, 0, 0, 0, 0, 0],
                34..=35 => [8, 0, 0, 0, 0, 0],
                36 => [9, 0, 0, 0, 0, 0],
                _ => [10, g.below(32), 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(kind, ctx.names.clone(), ctx.center.clone(), data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r13b_run(
                kind as i32,
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
        let what = || format!("case {case} ({}): cfg {cfg:x?}, file {:?}, ops {ops:?}", CLASSES[kind], show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, kind, ops.len()), decode(&buf, kind, ops.len()));
        for l in &dr {
            for key in ["newobject", "findpos", "setorient", "audio", "gainexp", "scan", "transfername", "destroy"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
    }
    println!(
        "unit/veterancy crates R13b: {cases} cases ({per_kind:?} compared per class), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} onCollide calls, {} destroyed, {} objects made, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.made, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(tally.destroyed * 10 > cases as u64, "the generator must reach the crates' actions");
}
