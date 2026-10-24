#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateCrateDie.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CreateCrateDie` (`logic::modules::create_crate_die`) against the unchanged C++, `p1w13-sonnet`, die batch R14c.
//!
//! # Method
//!
//! The C++ side is `CreateCrateDie.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dies_r14.inc`) on the module build's
//! stand-ins: the dying object (id 1, team 1: player, position, layer, veterancy level and status bits), the killer (id 2, team 2: player, kinds,
//! maybe an AI), three players with default teams 11 to 13, sciences and types, the teams' relationship, a crate system (a stand-in holding the
//! test's templates, `shim_module/GameLogic/CrateSystem.h`), a thing factory that knows `U1` and `U2` and makes a bare object (ids 3 up, maybe
//! with a drawable), and the partition manager's `findPositionAround`, the objects' `setOrientation`, `setLayer` and `setTeam`, the drawable's
//! three decal calls and the AI's `notifyCrate`, which are lines of the effect log.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` for the killer's AI),
//! `GameStatics::crate_system` read from INI text of the same templates (the C++ templates are built from what the Rust read, so the two hold
//! the same values), `cuts.crate_stub_object_calls` set (the orientation, the layer and the team are records, as the driver's are lines) and every
//! op a `World::module_call` or a change to the scene.
//!
//! Each case reads a block `D` (the die mux and 0 to 4 `CrateData` lines), mostly well formed, holds 0 to 4 crate templates (`C0` to `C3`: a
//! creation chance, maybe a veterancy level, killer kinds and a killer science, owned or not, 0 to 3 crates of `U1`, `U2` and `Missing`) and runs
//! 1 to 12 ops. Compared, bit for bit: the read's throw; the data; per op the throw status, the effects in order (`findpos`, `newobject`,
//! `setorient`, `setlayer`, `decal`, `decalsize`, `decalfade`, `setteam`, `notifycrate`), the xfer status, bytes and CRC, and every crate made so
//! far (id, team, position).
//!
//! # Results (2026-10-23, `L9R14C_CASES=9000`)
//!
//! 9,000 cases (7,206 compared, 28 excluded as undefined upstream: a crate owned by its maker when the dying object has no controlling player;
//! 1,766 data files threw, as the C++ did), 47,139 ops (21,615 `onDie` calls, 12,019 xfers) and 61,551 op outputs: **0 mismatches**. Effect lines
//! compared: newobject / setorient / setlayer 1,722 each, findpos 1,500, decal / decalsize / decalfade 842 each, setteam 522, notifycrate 367.
//!
//! Harness corrections on the way, none in the translation: under `cuts.crate_stub_object_calls` the Rust world bound every object
//! `crate_new_object` made to a drawable, where the driver's factory makes one only when the case says so (23 mismatches in the first 300 cases,
//! all a decal on a crate without a drawable); `crate_new_object` now binds one or none as `cuts.crate_new_object_drawable` says. The batch R13b to
//! R13e crate tests were re-run after it: 0 mismatches each.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught. 1771 and 1775 end the full run in a C++ segfault (a crate owned by a
//! maker with no player, which the planted Rust no longer reaches and so no longer excludes); each is caught by mismatches first (5 and 2 in the
//! first 20 cases, 72 and 23 in the first 200).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1771 | the weighted pick never matches (no crate is made) | 5 in 20 cases |
//! | 1772 | no wide second search for a position | 248 |
//! | 1773 | a crate for killing an ally | 315 |
//! | 1774 | the veterancy condition ignored | 1,133 |
//! | 1775 | the killer-science condition ignored | 2 in 20 cases |
//! | 1776 | a computer killer's AI not told of the crate | 290 |
//! | 1777 | the wrong terrain decal | 675 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::{PlayerType, Relationship};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT};
use rts_lockstep_engine::common::science::ScienceType;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::crate_system::CrateSystem;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::modules::create_crate_die::CreateCrateDieModuleData;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::veterancy_from_i32;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r14c.bin");
const XFER_FILE: &str = "l9_r14c.bin";
const CLASS: &str = "CreateCrateDie";
const NO_PLAYER: u32 = 0xFFFF_FFFF;

/// The killer's kinds (and the templates' `KilledByType`), by bit (the driver's `g_r14cKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 4] = [
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_AIRCRAFT, "AIRCRAFT"),
    (KINDOF_STRUCTURE, "STRUCTURE"),
];
/// The crates a template may make (the driver's `kCrateNames`).
const CRATES: [&str; 3] = ["U1", "U2", "Missing"];

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
fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
}
fn put_die_mux(v: &mut Vec<u8>, m: &DieMuxData) {
    put_u32(v, m.death_types);
    put_u32(v, m.veterancy_levels);
    put_words(v, m.exempt_status.words());
    put_words(v, m.required_status.words());
}
fn status_from(w: &[u32]) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    for i in 0..OBJECT_STATUS_COUNT {
        if w[(i / 32) as usize] >> (i % 32) & 1 != 0 {
            m.set(i, 1);
        }
    }
    m
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::CreateCrateDie(Arc::new(
        CreateCrateDieModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: read_block,
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r14c_run`: the die mux, then the crate names.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::CreateCrateDie(d) = data else {
        unreachable!("not a CreateCrateDie")
    };
    put_die_mux(v, &d.die.die_mux_data);
    put_u32(v, d.crate_name_list.len() as u32);
    for n in &d.crate_name_list {
        put_str(v, n);
    }
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
    let mux: Vec<u32> = (0..18).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let names: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    out.push(format!("data mux {mux:x?} crates {names:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let n_made = u32_(&mut p);
        let made: Vec<[u32; 5]> = (0..n_made)
            .map(|_| {
                [
                    u32_(&mut p),
                    u32_(&mut p),
                    u32_(&mut p),
                    u32_(&mut p),
                    u32_(&mut p),
                ]
            })
            .collect();
        out.push(format!("op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} made {made:x?}"));
    }
    out
}

// -- generators --

const DEATH_NAMES: [&str; 20] = [
    "NONE",
    "CRUSHED",
    "BURNED",
    "EXPLODED",
    "POISONED",
    "TOPPLED",
    "FLOODED",
    "SUICIDED",
    "LASERED",
    "DETONATED",
    "SPLATTED",
    "POISONED_BETA",
    "EXTRA_2",
    "EXTRA_3",
    "EXTRA_4",
    "EXTRA_5",
    "EXTRA_6",
    "EXTRA_7",
    "EXTRA_8",
    "POISONED_GAMMA",
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

fn status_tokens(g: &mut Gen) -> String {
    // a list is all plain names or all signed ones (a mix is refused by the INI reader; the junk lines of an unclean file cover refusals)
    let names = rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NAMES;
    let mode = g.below(5);
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        let n = names[1 + g.below(44) as usize];
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

/// The block `D`: the die mux and 0 to 4 `CrateData` lines (each field present with some probability), now and then a bogus token or an unknown
/// field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 15 {
        lines.push(format!("DeathTypes = {}", death_tokens(g)));
    }
    if g.below(100) < 10 {
        lines.push(format!("VeterancyLevels = {}", level_tokens(g)));
    }
    if g.below(100) < 8 {
        lines.push(format!("ExemptStatus = {}", status_tokens(g)));
    }
    if g.below(100) < 8 {
        lines.push(format!("RequiredStatus = {}", status_tokens(g)));
    }
    for _ in 0..g.below(5) {
        lines.push(format!(
            "CrateData = {}",
            pick(g, &["C0", "C1", "C2", "C3", "C0", "C1", "Unknown"])
        ));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "CrateData =".to_string(),
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

/// The crate templates: INI text of 0 to 4 `CrateData` blocks (`C0` to `C3`, in some order).
fn gen_crates(g: &mut Gen) -> Vec<u8> {
    let mut s = String::new();
    let mut names = vec![0u32, 1, 2, 3];
    for i in (1..4).rev() {
        names.swap(i, g.below(i as u32 + 1) as usize);
    }
    let count = *pick(g, &[&0usize, &1, &2, &2, &3, &4]);
    for n in names.into_iter().take(count) {
        s.push_str(&format!("CrateData C{n}\n"));
        s.push_str(&format!(
            "  CreationChance = {}\n",
            pick(g, &["1.0", "1.0", "0.5", "0.75", "0.1", "0"])
        ));
        if g.below(4) == 0 {
            s.push_str(&format!(
                "  VeterancyLevel = {}\n",
                pick(g, &["REGULAR", "VETERAN", "ELITE", "HEROIC"])
            ));
        }
        if g.below(4) == 0 {
            let k: Vec<&str> = KINDS
                .iter()
                .filter(|_| g.below(3) == 0)
                .map(|k| k.1)
                .collect();
            if !k.is_empty() {
                s.push_str(&format!("  KilledByType = {}\n", k.join(" ")));
            }
        }
        if g.below(4) == 0 {
            s.push_str(&format!(
                "  KillerScience = {}\n",
                SCIENCES[g.below(5) as usize]
            ));
        }
        if g.below(3) == 0 {
            s.push_str("  OwnedByMaker = Yes\n");
        }
        for _ in 0..g.below(4) {
            s.push_str(&format!(
                "  CrateObject = {} {}\n",
                pick(g, &["U1", "U1", "U2", "Missing"]),
                pick(g, &["1.0", "0.5", "0.25", "0.6"])
            ));
        }
        s.push_str("End\n");
    }
    s.into_bytes()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    died: u64,
    xfers: u64,
    made: u64,
}

struct CrateCtx {
    crates: CrateSystem,
    names: NameKeyGenerator,
}
fn crate_block(ini: &mut Ini<'_>, c: &mut CrateCtx) -> IniResult<()> {
    c.crates.parse_crate_template_definition(ini, &mut c.names)
}

/// The crate system read from `text`, and the driver's 14 words of each template (its values as the Rust read them).
fn read_crates(
    names: NameKeyGenerator,
    sciences: &[rts_lockstep_engine::common::name_key_generator::NameKeyType],
    text: &[u8],
) -> (CrateSystem, NameKeyGenerator, Vec<u32>) {
    static BLOCKS: [BlockParse<CrateCtx>; 1] = [BlockParse {
        token: "CrateData",
        parse: crate_block,
    }];
    let mut c = CrateCtx {
        crates: CrateSystem::new(),
        names,
    };
    Ini::load(
        b"Data\\INI\\Crate.ini",
        text,
        IniLoadType::Overwrite,
        None,
        &BLOCKS,
        &mut c,
    )
    .expect("crates load");
    let mut words = Vec::new();
    for chain in c.crates.chains() {
        let t = &chain.get_final_override().data;
        let name = t.name();
        let mut w = [0u32; 14];
        w[0] = u32::from(name[1] - b'0');
        w[1] = t.creation_chance().to_bits();
        w[2] = t.veterancy_level().map_or(4, |l| l as u32);
        w[3] = KINDS
            .iter()
            .enumerate()
            .filter(|(_, k)| t.killed_by_type_kindof().test(k.0))
            .map(|(i, _)| 1u32 << i)
            .sum();
        w[4] = t.killer_science().map_or(5, |k| {
            sciences
                .iter()
                .position(|s| *s == k)
                .expect("a test science") as u32
        });
        w[5] = u32::from(t.is_owned_by_maker());
        w[6] = t.possible_crates().len() as u32;
        for (e, entry) in t.possible_crates().iter().enumerate() {
            w[7 + 2 * e] = CRATES
                .iter()
                .position(|c| c.as_bytes() == entry.crate_name.as_slice())
                .unwrap() as u32;
            w[8 + 2 * e] = entry.crate_chance.to_bits();
        }
        words.extend_from_slice(&w);
    }
    (c.crates, c.names, words)
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

/// The killer's template: the test's kinds.
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

/// The flags (bit 0 the killer has an AI, 1 the crates made have a drawable, 2-3 the relationship, 4-6 the computers, 8 a position is found),
/// applied to the scene.
fn apply_flags(w: &mut World, flags: u32, ai: Option<u16>, find: Coord3D) {
    w.objects.find_mut(2).unwrap().ai = if flags & 1 != 0 { ai } else { None };
    w.cuts.crate_new_object_drawable = flags & 2 != 0;
    let rel = match flags >> 2 & 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    };
    w.cuts.team_relationships.insert((2, 1), rel);
    for p in 0..3u32 {
        w.players.get_nth_mut(p as i32).unwrap().player_type = if flags >> (4 + p) & 1 != 0 {
            PlayerType::Computer
        } else {
            PlayerType::Human
        };
    }
    w.cuts.contain.find_position = (flags & 256 != 0).then_some(find);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    crate_text: &[u8],
    cfg: &[u32; 12],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let (crates, names, _) = read_crates(ctx.names.clone(), &ctx.sciences, crate_text);
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
    w.names = names;
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        thing_factory: factory,
        crate_system: crates,
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.body.probe_trace = true;
    w.random.init_game_logic_random(cfg[7]);
    for team in [1u32, 2, 11, 12, 13] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    for p in 0..3u32 {
        let pl = w.players.get_nth_mut(p as i32).unwrap();
        pl.default_team = Some(11 + p);
        pl.sciences = (0..5)
            .filter(|s| cfg[4 + p as usize] >> s & 1 != 0)
            .map(|s| ScienceType(ctx.sciences[s].0 as i32))
            .collect();
        set_player_of_team(&mut w, 11 + p, Some(p));
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        CLASS.as_bytes(),
        b"ModuleTag_Die",
        &data,
    )
    .unwrap()
    .unwrap();
    let me = w
        .new_object(NewObject {
            template: plain.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1);
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        0.0,
        Default::default(),
    )));
    let killer = w
        .new_object(NewObject {
            template: plain.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(2),
            modules: vec![ai],
        })
        .unwrap();
    assert_eq!(killer, 2);
    let killer_ai = w.objects.find(killer).unwrap().ai;
    set_kinds(&mut w, killer, cfg[2]);
    w.set_position(
        me,
        &Coord3D {
            x: f32::from_bits(cfg[8]),
            y: f32::from_bits(cfg[9]),
            z: f32::from_bits(cfg[10]),
        },
    );
    w.objects.find_mut(me).unwrap().layer = cfg[11] as i32;
    let mut flags = cfg[3];
    let mut find = Coord3D {
        x: 7.5,
        y: -3.25,
        z: 0.0,
    };
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let first_made = w.next_object_id();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            1 => flags = o[1],
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => set_kinds(&mut w, killer, o[1]),
            4 => {
                flags = (flags & !256) | (o[1] & 1) << 8;
                find = Coord3D {
                    x: f32::from_bits(o[2]),
                    y: f32::from_bits(o[3]),
                    z: 0.0,
                };
            }
            5 => w.objects.find_mut(me).unwrap().layer = o[1] as i32,
            _ => {}
        }
        apply_flags(&mut w, flags, killer_ai, find);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        match o[0] {
            0 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as _;
                info.input.source_id = o[5] as _;
                {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.experience.level = veterancy_from_i32((o[2] % 4) as i32);
                    ob.status = status_from(&o[3..5]);
                }
                w.module_call(me, idx, ModuleCall::OnDie(info));
                tally.died += 1;
            }
            1..=5 => {}
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
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
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
                    CutCall::ThingFactoryNewObject { template, team } => {
                        log.push(format!(
                            "newobject {} {}",
                            template_names[template],
                            team.map_or(-1, |t| t as i32)
                        ));
                        tally.made += 1;
                    }
                    CutCall::Contain(ContainCall::PartitionFindPositionAround {
                        position: p,
                        min_radius,
                        max_radius,
                        flags,
                    }) => log.push(format!(
                        "findpos {:08x} {:08x} {:08x} {:08x} {:08x} {flags}",
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        min_radius.to_bits(),
                        max_radius.to_bits()
                    )),
                    CutCall::ObjectSetOrientation { object, angle } => {
                        log.push(format!("setorient {object} {:08x}", angle.to_bits()))
                    }
                    CutCall::ObjectSetLayer { object, layer } => {
                        log.push(format!("setlayer {object} {layer}"))
                    }
                    CutCall::ObjectSetTeam { object, team } => log.push(format!(
                        "setteam {object} {}",
                        team.map_or(-1, |t| t as i32)
                    )),
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                log.push(r.to_string());
            } else if t.starts_with("evt ") {
                match event_iter.next().flatten() {
                    Some(ObjectClientEvent::DrawableSetTerrainDecal { decal, .. }) => {
                        log.push(format!("decal {decal}"))
                    }
                    Some(ObjectClientEvent::DrawableSetTerrainDecalSize { x, y, .. }) => {
                        log.push(format!("decalsize {:08x} {:08x}", x.to_bits(), y.to_bits()))
                    }
                    Some(ObjectClientEvent::DrawableTerrainDecalFadeTarget {
                        target,
                        rate,
                        ..
                    }) => log.push(format!(
                        "decalfade {:08x} {:08x}",
                        target.to_bits(),
                        rate.to_bits()
                    )),
                    _ => {}
                }
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
        let made: Vec<ObjectId> = (first_made..w.next_object_id())
            .filter(|id| w.objects.find(*id).is_some())
            .collect();
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
    std::env::var("L9R14C_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn create_crate_die_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0014_C0C8);
    let mut read = Check::new("CreateCrateDie R14c data read (throw, message)");
    let mut run = Check::new("CreateCrateDie R14c ops (data, effects, xfers, crates made)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let coords = [&0.0f32, &10.0, &-12.5, &25.0, &3.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R14c{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let crate_text = gen_crates(&mut g);
        let (_, _, crate_words) = read_crates(ctx.names.clone(), &ctx.sciences, &crate_text);
        // mostly an enemy killer of player 1 or 2 with an AI, a dying object owned by player 0 to 2, a position found
        let player = |g: &mut Gen| match g.below(12) {
            0 => NO_PLAYER,
            1 => 0,
            _ => 1 + g.below(2),
        };
        let gen_flags = |g: &mut Gen| {
            u32::from(g.below(5) != 0)
                | u32::from(g.below(2) == 0) << 1
                | (if g.below(4) != 0 { 0 } else { g.below(3) }) << 2
                | (g.u32() & 7) << 4
                | u32::from(g.below(4) != 0) << 8
        };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = g.u32() & 15;
        cfg[3] = gen_flags(&mut g);
        for word in &mut cfg[4..7] {
            *word = g.u32() & g.u32() & 31;
        }
        cfg[7] = g.u32();
        cfg[8] = pick(&mut g, &coords).to_bits();
        cfg[9] = pick(&mut g, &coords).to_bits();
        cfg[10] = if g.below(4) == 0 { 5.0f32.to_bits() } else { 0 };
        cfg[11] = *pick(&mut g, &[&1u32, &1, &1, &0, &2]);
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(24) {
                0..=10 => {
                    // not DEATH_NORMAL nor LEVEL_REGULAR (see the module docs)
                    let death = 1 + g.below(11);
                    let level = 1 + g.below(3);
                    let st = if g.below(4) == 0 {
                        g.u32() & g.u32()
                    } else {
                        0
                    };
                    // mostly the killer as the source, now and then nobody or one not found
                    let src = *pick(&mut g, &[&2u32, &2, &2, &0, &9]);
                    [0, death, level, st, 0, src]
                }
                11..=12 => [1, gen_flags(&mut g), 0, 0, 0, 0],
                13 => [2, g.below(4), 0, 0, 0, 0],
                14 => [3, g.u32() & 15, 0, 0, 0, 0],
                15..=16 => [
                    4,
                    g.below(4).min(1),
                    pick(&mut g, &coords).to_bits(),
                    pick(&mut g, &coords).to_bits(),
                    0,
                    0,
                ],
                17 => [5, *pick(&mut g, &[&1u32, &1, &0, &2]), 0, 0, 0, 0],
                18..=19 => [7, 0, 0, 0, 0, 0],
                20..=21 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(
                &ctx,
                data.expect("read ok"),
                &crate_text,
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
            cpp::ref_l9r14c_run(
                cfile.as_ptr(),
                cxfer.as_ptr(),
                cfg.as_ptr(),
                (crate_words.len() / 14) as i32,
                crate_words.as_ptr(),
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
                "case {case}: cfg {cfg:x?}, file {:?}, crates {:?}, ops {ops:?}",
                show(&text),
                show(&crate_text)
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
                "findpos",
                "newobject",
                "setorient",
                "setlayer",
                "decal ",
                "decalsize",
                "decalfade",
                "setteam",
                "notifycrate",
            ] {
                *counts.entry(key.trim().to_string()).or_default() +=
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
            eprintln!("--- case {case} cfg {cfg:x?} file {:?} crates {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), show(&crate_text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "CreateCrateDie R14c: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onDie calls, {} crates made, {} xfers, {} effect lines",
        tally.died, tally.made, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        tally.made * 10 > cases as u64,
        "the generator must reach the crates"
    );
}
