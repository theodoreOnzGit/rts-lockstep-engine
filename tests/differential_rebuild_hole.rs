#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/RebuildHoleExposeDie.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `RebuildHoleExposeDie` and `RebuildHoleBehavior` (`logic::modules::{rebuild_hole_expose_die, rebuild_hole_behavior}`) against the unchanged
//! C++, `p1w13-sonnet`, batch R15.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_hole_r15.inc`) on the module build's
//! stand-ins: four objects (1: the module's object, template `U2`, with a body, a name, a position, an orientation and a box geometry; 2: an
//! attacker; 3: a building that may be under construction; 4: a mine with a sticky bomb on a target the test names), each maybe with an AI, in
//! the game logic's list newest first; three players (0 the neutral one), each maybe an observer or dead; and a thing factory that knows `U1`
//! and `U2` and makes a bare object (ids 5 up, in front of the list) with, as the test says, a body, a `RebuildHoleBehavior` stand-in and an AI
//! whose `construct` returns the object the test names. Every call a module makes is a line of the effect log.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule`s for the bodies, the
//! AIs, the sticky bomb and the made objects' modules through `cuts.crate_new_object_modules`), `cuts.crate_stub_object_calls` set (the new
//! object, its orientation and `maskObject` are records), and every op a `World::module_call`, an `update()` as the scheduler runs it, or a
//! change to the scene.
//!
//! Each case reads a block `D` (kind 0: the die mux, `HoleName`, `HoleMaxHealth`, `TransferAttackers`; kind 1: `WorkerObjectName`,
//! `WorkerRespawnDelay`, `HoleHealthRegen%PerSecond`), mostly well formed, and runs 1 to 16 ops. Compared, bit for bit: the read's throw; the
//! data; per op the throw status, the effects in order (`newobject`, `setorient`, `transfername`, `pathadd`, `startrebuild`, `transferattack`,
//! `construct`, `resume`, `mask`, `bombtarget`, `heal`, `destroy`), the xfer status, bytes and CRC, the `RebuildHoleBehavior`'s state (worker,
//! reconstruction, spawner, wait, template names), object 1's health and maximum, object 3's producer, and every object made (id, team,
//! position, major radius, body maximum, unselectable bit).
//!
//! # Results (2026-10-23, `L9R15_CASES=9000`)
//!
//! 9,000 cases (3,548 `RebuildHoleExposeDie`, 3,268 `RebuildHoleBehavior` compared; 711 excluded as undefined upstream: an unowned dying object,
//! a hole with no body, a reconstruction the AI did not make, a negative respawn delay; 1,473 data files threw, as the C++ did), 69,247 ops
//! (23,732 updates, 7,327 deaths, 3,841 rebuild starts, 32,979 xfers) and 82,879 op outputs: **0 mismatches**. Effect lines compared: heal
//! 12,556, transferattack 7,072, mask 3,687, destroy 3,260, transfername 2,666, newobject 2,216, pathadd / setorient 1,829 each, startrebuild
//! 1,767, construct 313, bombtarget 134, resume 27.
//!
//! Harness corrections on the way, none in the translation: an object the driver's factory made had a `NULL` behaviour list where a real object
//! has an empty one (a segfault in `getRebuildHoleBehaviorInterfaceFromObject`); object 3's producer and the objects made are read only while
//! they are still in the game (the Rust side cannot read an object it removed).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 11 of 11 caught. 1783 ends the full run in a C++ segfault (a worker made every
//! frame, where the planted Rust no longer reaches an undefined case); it is caught by mismatches first (20 in the first 20 cases).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1781 | the hole not unmasked when the wait starts again | 3,347 |
//! | 1782 | sticky bombs left on the hole | 134 |
//! | 1783 | a worker made every frame of the wait | 20 in 20 cases |
//! | 1784 | the attackers left on the hole | 340 |
//! | 1785 | the hole never heals | 20,268 |
//! | 1786 | a loaded worker template never found | 135 |
//! | 1787 | the worker kept when the hole dies | 417 |
//! | 1791 | a hole for the neutral player | 1,113 |
//! | 1792 | a hole for a building under construction | 834 |
//! | 1793 | the hole's behaviour not told what to rebuild | 1,767 |
//! | 1794 | the attackers left on the dead building | 1,641 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_MINE;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_UNSELECTABLE};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::modules::rebuild_hole_behavior::RebuildHoleBehaviorModuleData;
use rts_lockstep_engine::logic::modules::rebuild_hole_expose_die::RebuildHoleExposeDieModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::veterancy_from_i32;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r15.bin");
const XFER_FILE: &str = "l9_r15.bin";
const CLASSES: [&str; 2] = ["RebuildHoleExposeDie", "RebuildHoleBehavior"];
const NO_PLAYER: u32 = 0xFFFF_FFFF;

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

type Read = Option<ModuleDataKind>;
fn read_die(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::RebuildHoleExposeDie(Arc::new(RebuildHoleExposeDieModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_behavior(ini: &mut Ini<'_>, out: &mut Read) -> IniResult<()> {
    *out = Some(ModuleDataKind::RebuildHoleBehavior(Arc::new(RebuildHoleBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Read) {
    let blocks = [BlockParse { token: "D", parse: if kind == 0 { read_die } else { read_behavior } }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r15_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::RebuildHoleExposeDie(d) => {
            put_die_mux(v, &d.die.die_mux_data);
            put_str(v, &d.hole_name);
            put_u32(v, d.hole_max_health.to_bits());
            put_u32(v, u32::from(d.transfer_attackers));
        }
        ModuleDataKind::RebuildHoleBehavior(d) => {
            put_str(v, &d.worker_template_name);
            put_u32(v, d.worker_respawn_delay.to_bits());
            put_u32(v, d.hole_health_regen_percent_per_second.to_bits());
        }
        _ => unreachable!("not a rebuild hole module"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: u32, b: &[u8], n_ops: usize) -> Vec<String> {
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
    if kind == 0 {
        let mux: Vec<u32> = (0..18).map(|_| u32_(&mut p)).collect();
        let name = str_(&mut p);
        let (h, t) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("data mux {mux:x?} hole {name:?} health {h:#x} transfer {t}"));
    } else {
        let name = str_(&mut p);
        let (d, r) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("data worker {name:?} delay {d:#x} regen {r:#x}"));
    }
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
        let state = if kind == 1 {
            let w: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
            format!("{w:?} {:?} {:?}", str_(&mut p), str_(&mut p))
        } else {
            String::new()
        };
        let obj: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        let n_made = u32_(&mut p);
        let made: Vec<Vec<u32>> = (0..n_made).map(|_| (0..8).map(|_| u32_(&mut p)).collect()).collect();
        out.push(format!("op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state} obj {obj:x?} made {made:x?}"));
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

/// The block `D` of `kind`, each field present with some probability; now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if kind == 0 {
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
        if g.below(100) < 90 {
            lines.push(format!("HoleName = {}", pick(g, &["U1", "U1", "U2", "Missing"])));
        }
        if g.below(100) < 60 {
            lines.push(format!("HoleMaxHealth = {}", pick(g, &["100", "250.5", "0", "1e3"])));
        }
        if g.below(100) < 30 {
            lines.push(format!("TransferAttackers = {}", gen_bool(g, clean)));
        }
    } else {
        if g.below(100) < 85 {
            lines.push(format!("WorkerObjectName = {}", pick(g, &["U1", "U1", "U2", "Missing"])));
        }
        if g.below(100) < 70 {
            lines.push(format!("WorkerRespawnDelay = {}", pick(g, &["0", "33", "100", "100", "150", "150", "1000", "-500"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("HoleHealthRegen%PerSecond = {}", pick(g, &["10%", "25%", "0%", "150%"])));
        }
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => if kind == 0 { "HoleMaxHealth = lots".to_string() } else { "WorkerRespawnDelay = soon".to_string() },
            1 => "HoleName".to_string(),
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
    ops: BTreeMap<u32, u64>,
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            // `SC_INVALID_DATA` (reported as `ListNotEmpty`): the driver's `catch (...)`
            Err(rts_lockstep_engine::common::xfer::XferStatus::ListNotEmpty) => -1,
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

/// The modules of an object the factory makes (`cfg[2]`: bit 0 a body of maximum `max`, 1 a `RebuildHoleBehavior`, 2 an AI whose `construct`
/// returns `construct`).
fn made_modules(bits: u32, max: f32, construct: ObjectId) -> Vec<NewModule> {
    let mut v = Vec::new();
    if bits & 1 != 0 {
        let mut b = NewModule::plain(b"CtBody", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max, Default::default())));
        v.push(b);
    }
    if bits & 2 != 0 {
        let mut h = NewModule::plain(b"RebuildHoleBehavior", b"ModuleTag_Hole");
        h.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { rebuild_hole: true, ..Default::default() }, 0.0, Default::default())));
        v.push(h);
    }
    if bits & 4 != 0 {
        let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
        ai.is_ai = true;
        ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { construct_result: construct, ..Default::default() }, 0.0, Default::default())));
        v.push(ai);
    }
    v
}

/// The probe in slot `idx` of `id`.
fn probe_mut(w: &mut World, id: ObjectId, idx: u16) -> &mut ProbeModule {
    match w.objects.find_mut(id).unwrap().modules[idx as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => p,
        _ => panic!("a probe"),
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut factory = ThingFactory::new();
    let mut template_names: BTreeMap<u16, String> = BTreeMap::new();
    for name in ["U1", "U2"] {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let id = factory.find_template(name.as_bytes()).expect("the new template").data.get_template_id();
        template_names.insert(id, name.to_string());
    }
    let u2 = Arc::new(factory.find_template(b"U2").unwrap().get_final_override().data.clone());
    let tid = |name: &str| *template_names.iter().find(|(_, n)| n.as_str() == name).unwrap().0;
    let (u1_id, u2_id) = (tid("U1"), tid("U2"));
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), thing_factory: factory, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.body.probe_trace = true;
    w.cuts.probe_effect_trace = true;
    for team in [1u32, 2, 3, 4, 11, 12, 13] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    for p in 0..3u32 {
        w.players.get_nth_mut(p as i32).unwrap().default_team = Some(11 + p);
        set_player_of_team(&mut w, 11 + p, Some(p));
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    let mut construct = cfg[4] as ObjectId;
    w.cuts.crate_new_object_modules = made_modules(cfg[2], f32::from_bits(cfg[3]), construct);
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let ai = || {
        let mut a = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
        a.is_ai = true;
        a.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        a
    };
    // object 1: the module, a body, an AI
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASSES[kind as usize].as_bytes(), b"ModuleTag_Hole", &data).unwrap().unwrap();
    let mut body = NewModule::plain(b"CtBody", b"ModuleTag_Body");
    body.interfaces = MODULEINTERFACE_BODY;
    let mut bp = ProbeModule::new(ProbeAnswers::default(), f32::from_bits(cfg[7]), Default::default());
    bp.health = f32::from_bits(cfg[6]);
    body.body = Some(UpdateBody::ModuleProbe(bp));
    let me = w.new_object(NewObject { template: u2, status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, body, ai()] }).unwrap();
    assert_eq!(me, 1);
    for k in 2..=4u32 {
        let mut mods = vec![ai()];
        if k == 4 {
            let mut b = NewModule::plain(b"StickyBombUpdate", b"ModuleTag_Bomb");
            b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { sticky_target: Some(cfg[5] as ObjectId), ..Default::default() }, 0.0, Default::default())));
            mods.push(b);
        }
        let template = if k == 4 {
            let mut values = ObjectTemplateValues::default();
            values.kind_of.push(KINDOF_MINE);
            Arc::new(ThingTemplate::from_object_values(&values))
        } else {
            plain.clone()
        };
        let id = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: Some(k), modules: mods }).unwrap();
        assert_eq!(id, k as ObjectId);
    }
    {
        let o = w.objects.find_mut(me).unwrap();
        o.name = b"Hole1".to_vec();
        o.geometry_info = GeometryInfo::new(GeometryType::Box, false, 10.0, f32::from_bits(cfg[11]), f32::from_bits(cfg[11]));
    }
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[8]), y: f32::from_bits(cfg[9]), z: 0.0 });
    w.contain_scene_set_orientation(me, f32::from_bits(cfg[10]));
    let ais: Vec<Option<u16>> = (1..=4).map(|k| w.objects.find(k).unwrap().ai).collect();
    let body_idx = w.objects.find(me).unwrap().body;
    let idx = w.find_module(me, CLASSES[kind as usize].as_bytes()).unwrap();
    let first_made = w.next_object_id();
    let mut flags = cfg[1];
    let mut gone = [false; 4];
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            2 => flags = o[1],
            4 => {
                let id = o[1] as ObjectId;
                if w.objects.find(id).is_some() {
                    w.sched.erase_object_updates(&mut w.objects, id);
                    w.objects.remove(id);
                }
                if id <= 4 {
                    gone[id as usize - 1] = true;
                }
            }
            5 => probe_mut(&mut w, me, body_idx.unwrap()).health = f32::from_bits(o[1]),
            6 => {
                construct = o[1] as ObjectId;
                w.cuts.crate_new_object_modules = made_modules(cfg[2], f32::from_bits(cfg[3]), construct);
                // every AI the factory made asks the same `construct`
                for id in first_made..w.next_object_id() {
                    if let Some(a) = w.objects.find(id).and_then(|ob| ob.ai) {
                        probe_mut(&mut w, id, a).answers.construct_result = construct;
                    }
                }
            }
            _ => {}
        }
        // the scene the flags describe
        for k in 1..=4u32 {
            if gone[k as usize - 1] {
                continue;
            }
            w.objects.find_mut(k as ObjectId).unwrap().ai = if flags >> (k - 1) & 1 != 0 { ais[k as usize - 1] } else { None };
        }
        w.objects.find_mut(me).unwrap().body = if flags & 16 != 0 { body_idx } else { None };
        if !gone[2] {
            w.objects.find_mut(3).unwrap().status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, i32::from(flags & 32 != 0));
        }
        for p in 0..3u32 {
            let pl = w.players.get_nth_mut(p as i32).unwrap();
            pl.observer = flags >> (6 + p) & 1 != 0;
            pl.is_player_dead = flags >> (9 + p) & 1 != 0;
        }
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        *tally.ops.entry(o[0]).or_default() += 1;
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
            }
            1 => {
                let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
                w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot");
            }
            3 => {
                let template = if o[1] == 1 { u1_id } else { u2_id };
                w.module_call(me, idx, ModuleCall::StartRebuildProcess { template, spawner: o[2] as ObjectId });
            }
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
            }
            _ => {}
        }
        let errs: Vec<_> = w.cuts.module_errors[errors_before..].iter().filter(|(_, e)| !e.is_undefined()).collect();
        let st = if errs.iter().any(|(_, e)| e.kind == IniErrorKind::BadArg) { 3 } else if errs.is_empty() { 0 } else { 2 };
        put_i32(&mut v, st);
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        let tname = |id: &str| -> String {
            match id.parse::<i32>() {
                Ok(-1) => "-".to_string(),
                Ok(t) => template_names[&(t as u16)].clone(),
                Err(_) => id.to_string(),
            }
        };
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::ThingFactoryNewObject { template, team } => log.push(format!("newobject {} {}", template_names[template], team.map_or(-1, |t| t as i32))),
                    CutCall::ObjectSetOrientation { object, angle } => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                    CutCall::ScriptEngineTransferObjectName { name, object } => log.push(format!("transfername {} {object}", String::from_utf8_lossy(name))),
                    CutCall::PathfinderAddObjectToPathfindMap { object } => log.push(format!("pathadd {object}")),
                    CutCall::ObjectMaskObject { object, mask } => log.push(format!("mask {object} {}", i32::from(*mask))),
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                // the probes print a template by id: the driver prints its name
                let mut parts: Vec<String> = r.split(' ').map(str::to_string).collect();
                if parts[0] == "construct" || parts[0] == "startrebuild" {
                    parts[2] = tname(&parts[2]);
                }
                log.push(parts.join(" "));
            } else if t.starts_with("heal ") || t.starts_with("destroy ") {
                log.push(t.clone());
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
        if kind == 1 {
            let ob = w.objects.find(me).unwrap();
            let slot = ob.modules[idx as usize].update_index.unwrap() as usize;
            match ob.behaviors[slot].body.as_ref() {
                Some(UpdateBody::RebuildHoleBehavior(m)) => {
                    for x in m.state_words() {
                        put_u32(&mut v, x);
                    }
                    put_str(&mut v, m.get_worker_template().unwrap_or(b""));
                    put_str(&mut v, m.get_rebuild_template().unwrap_or(b""));
                }
                _ => panic!("the RebuildHoleBehavior"),
            }
        }
        let bp = match w.objects.find(me).unwrap().modules[body_idx.unwrap() as usize].body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => (p.health, p.max_health),
            _ => panic!("the body"),
        };
        put_u32(&mut v, bp.0.to_bits());
        put_u32(&mut v, bp.1.to_bits());
        put_u32(&mut v, if gone[2] { 0 } else { w.objects.find(3).unwrap().producer_id as u32 });
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
            put_u32(&mut v, o.geometry_info.get_major_radius().to_bits());
            let max = o.body.and_then(|b| match o.modules[b as usize].body.as_ref() {
                Some(UpdateBody::ModuleProbe(p)) => Some(p.max_health),
                _ => None,
            });
            put_u32(&mut v, max.map_or(0, f32::to_bits));
            put_u32(&mut v, u32::from(o.test_status(OBJECT_STATUS_UNSELECTABLE)));
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R15_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn rebuild_hole_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0015_401E);
    let mut read = Check::new("rebuild hole R15 data read (throw, message)");
    let mut run = Check::new("rebuild hole R15 ops (data, effects, xfers, state, objects made)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut compared = [0u64; 2];
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let coords = [&0.0f32, &10.0, &-12.5, &25.0, &3.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = (case % 2) as u32;
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R15{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a player-owned object with a body, attackers with AIs, a factory that makes a body, a hole and an AI that builds object 3
        let mut cfg = [0u32; 12];
        cfg[0] = match g.below(10) {
            0 => NO_PLAYER,
            1 => 0,
            _ => 1 + g.below(2),
        };
        let gen_flags = |g: &mut Gen| {
            (g.u32() & 15) | 1 << 4 | u32::from(g.below(3) != 0) << 5 | (if g.below(5) == 0 { g.u32() & 0xFC0 & g.u32() } else { 0 })
        };
        cfg[1] = gen_flags(&mut g);
        cfg[2] = if g.below(6) == 0 { g.below(8) } else { 7 };
        cfg[3] = (*pick(&mut g, &[&100.0f32, &50.0, &0.0])).to_bits();
        cfg[4] = *pick(&mut g, &[&3u32, &3, &3, &2, &0]);
        cfg[5] = *pick(&mut g, &[&1u32, &1, &2, &0]);
        cfg[6] = (*pick(&mut g, &[&40.0f32, &100.0, &0.5])).to_bits();
        cfg[7] = (*pick(&mut g, &[&100.0f32, &100.0, &50.0])).to_bits();
        cfg[8] = pick(&mut g, &coords).to_bits();
        cfg[9] = pick(&mut g, &coords).to_bits();
        cfg[10] = (*pick(&mut g, &[&0.0f32, &1.5707964, &-0.75])).to_bits();
        cfg[11] = (*pick(&mut g, &[&10.0f32, &20.0, &5.5])).to_bits();
        let n_ops = 1 + g.below(if kind == 1 { 24 } else { 16 }) as usize;
        let mut gone = [false; 4];
        let mut ops: Vec<[u32; 6]> = Vec::new();
        for i in 0..n_ops {
            // the behaviour mostly starts its rebuild first
            if kind == 1 && i == 0 && g.below(3) != 0 {
                ops.push([3, 1 + g.below(2), *pick(&mut g, &[&1u32, &2, &9]), 0, 0, 0]);
                continue;
            }
            let op = match g.below(30) {
                0..=4 if kind == 0 || g.below(4) == 0 => {
                    // not DEATH_NORMAL nor LEVEL_REGULAR (see the CreateObjectDie test)
                    let st = if g.below(4) == 0 { g.u32() & g.u32() } else { 0 };
                    [0, 1 + g.below(11), 1 + g.below(3), st, 0, *pick(&mut g, &[&2u32, &0])]
                }
                5..=16 if kind == 1 => [1, 0, 0, 0, 0, 0],
                17 if kind == 1 => [3, 1 + g.below(2), *pick(&mut g, &[&1u32, &2, &9]), 0, 0, 0],
                18 => [2, gen_flags(&mut g), 0, 0, 0, 0],
                19 => {
                    let k = 2 + g.below(4);
                    if k == 5 {
                        // the first object the factory made (the worker, mostly): gone if it is there
                        [4, 5, 0, 0, 0, 0]
                    } else if gone[k as usize - 1] {
                        [1, 0, 0, 0, 0, 0]
                    } else {
                        gone[k as usize - 1] = true;
                        [4, k, 0, 0, 0, 0]
                    }
                }
                20 => [5, (*pick(&mut g, &[&10.0f32, &99.5, &100.0])).to_bits(), 0, 0, 0, 0],
                21 => [6, *pick(&mut g, &[&3u32, &2, &0]), 0, 0, 0, 0],
                22..=24 => [7, 0, 0, 0, 0, 0],
                25..=27 => [8, 0, 0, 0, 0, 0],
                28 => [9, 0, 0, 0, 0, 0],
                _ => [1, 0, 0, 0, 0, 0],
            };
            // an update only for the behaviour; construct never names an object that is gone
            let op = if kind == 0 && op[0] == 1 { [9, 0, 0, 0, 0, 0] } else { op };
            let op = if op[0] == 6 && op[1] != 0 && gone[op[1] as usize - 1] { [6, 0, 0, 0, 0, 0] } else { op };
            ops.push(op);
        }

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, kind, data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r15_run(
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
        let what = || format!("case {case} kind {kind}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["newobject", "setorient", "transfername", "pathadd", "startrebuild", "transferattack", "construct", "resume", "mask", "bombtarget", "heal", "destroy"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key} ")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} kind {kind} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared[kind as usize] += 1;
    }
    println!(
        "rebuild hole R15: {cases} cases ({compared:?} compared per class), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops ({:?} by op), {} effect lines",
        tally.ops, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(counts.get("construct").copied().unwrap_or(0) * 40 > cases as u64, "the generator must reach the reconstruction");
}
