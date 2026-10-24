#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `InstantDeathBehavior` (`logic::modules::instant_death_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R16.
//!
//! # Method
//!
//! The C++ side is `InstantDeathBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_instant_death_r16.inc`; its three
//! file-static parsers renamed by macros away from `SlowDeathBehavior.cpp`'s) on the slow death driver's stand-ins: the FX list store (`FX_A`,
//! `FX_B`), the object creation list store (`OCL_A`, `OCL_B`), the weapon store (`WEAP_A`, `WEAP_B`, any case), an AI that logs `markAsDead`, a
//! game logic whose `destroyObject` logs, and the REAL `DieModule`, `DieMuxData`, `INI`, `RandomValue` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the object (the module from `modules::new_module`, a `ProbeModule` for the AI) and the same
//! stores in `GameStatics`; every op a `World::module_call` or a change to the scene.
//!
//! Each case reads a block `D` (the die mux, the `FX`, `OCL` and `Weapon` lists), mostly well formed, and runs 1 to 12 ops. Compared, bit for
//! bit: the read's throw; the data; per op the throw status, the effects in order (`markdead`, `fx`, `ocl`, `weapon`, `destroy`), the xfer
//! status, bytes and CRC, and the AI's dead flag.
//!
//! # Results (2026-10-23, `L9R16_CASES=9000`)
//!
//! 9,000 cases (7,249 compared, none excluded; 1,751 data files threw, as the C++ did), 47,452 ops (23,815 `onDie` calls, 17,641 xfers) and
//! 61,950 op outputs: **0 mismatches**. Effect lines compared: destroy 9,344, markdead 5,271, fx 4,199, ocl 4,099, weapon 4,062. No harness
//! correction was needed.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1801 | the die mux ignored | 3,139 |
//! | 1802 | a death another module's AI already handled is run again | 12,187 |
//! | 1803 | no FX list | 4,199 |
//! | 1804 | the weapon draw one entry too wide | 3,447 |
//! | 1805 | the object not destroyed | 9,344 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::modules::instant_death_behavior::InstantDeathBehaviorModuleData;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r16.bin");
const XFER_FILE: &str = "l9_r16.bin";
const CLASS: &str = "InstantDeathBehavior";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];

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
    *out = Some(ModuleDataKind::InstantDeathBehavior(Arc::new(InstantDeathBehaviorModuleData::friend_new_module_data(Some(ini))?)));
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
        Some(b"FX_A") => 1,
        Some(b"FX_B") => 2,
        _ => 0,
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
    WEAPONS.iter().position(|w| n.eq_ignore_ascii_case(w.as_bytes())).map_or(0, |i| i as i32 + 1)
}

/// The data dump of `ref_l9r16_run`: the die mux, then the three lists as codes.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::InstantDeathBehavior(d) = data else { unreachable!("not an InstantDeathBehavior") };
    put_die_mux(v, &d.die.die_mux_data);
    put_u32(v, d.fx.len() as u32);
    for f in &d.fx {
        put_i32(v, fx_code(f));
    }
    put_u32(v, d.ocls.len() as u32);
    for o in &d.ocls {
        put_i32(v, ocl_code(o));
    }
    put_u32(v, d.weapons.len() as u32);
    for w in &d.weapons {
        put_i32(v, weapon_code(w));
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
    let mut lists = Vec::new();
    for _ in 0..3 {
        let n = u32_(&mut p);
        lists.push((0..n).map(|_| u32_(&mut p) as i32).collect::<Vec<i32>>());
    }
    out.push(format!("data mux {mux:x?} lists {lists:?}"));
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
        let dead = u32_(&mut p);
        out.push(format!("op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} dead {dead}"));
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

/// A list line's tokens: 1 to 3 names of the stores' and others.
fn list_tokens(g: &mut Gen, names: &[&str]) -> String {
    (0..1 + g.below(3)).map(|_| names[g.below(names.len() as u32) as usize]).collect::<Vec<_>>().join(" ")
}

/// The block `D`: the die mux and the three lists (each present with some probability), now and then a bogus token or an unknown field.
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
    for _ in 0..g.below(3) {
        lines.push(format!("FX = {}", list_tokens(g, &["FX_A", "FX_B", "None"])));
    }
    for _ in 0..g.below(3) {
        lines.push(format!("OCL = {}", list_tokens(g, &["OCL_A", "OCL_B", "OCL_X"])));
    }
    for _ in 0..g.below(3) {
        lines.push(format!("Weapon = {}", list_tokens(g, &["WEAP_A", "weap_b", "WEAP_X"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "FX".to_string(),
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
    died: u64,
    xfers: u64,
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

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg: &[u32; 6], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    let mut scratch = rts_lockstep_engine::common::name_key_generator::NameKeyGenerator::new();
    scratch.reset();
    for n in WEAPONS {
        weapon_store.new_weapon_template(&mut scratch, n.as_bytes()).unwrap();
    }
    w.names = ctx.names.clone();
    // the object creation lists are keyed in the world's own name key generator
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), object_creation_list_store: ocls, weapon_store, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.random.init_game_logic_random(cfg[4]);
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASS.as_bytes(), b"ModuleTag_Die", &data).unwrap().unwrap();
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    let me: ObjectId = w.new_object(NewObject { template: plain, status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, ai] }).unwrap();
    assert_eq!(me, 1);
    let ai_idx = w.objects.find(me).unwrap().ai;
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[1]), y: f32::from_bits(cfg[2]), z: f32::from_bits(cfg[3]) });
    let set_dead = |w: &mut World, dead: bool| {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[ai_idx.unwrap() as usize].body.as_mut() {
            p.answers.ai_dead = dead;
        }
    };
    let mut flags = cfg[0];
    set_dead(&mut w, flags & 2 != 0);
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            flags = o[1];
            set_dead(&mut w, flags & 2 != 0);
        }
        w.objects.find_mut(me).unwrap().ai = if flags & 1 != 0 { ai_idx } else { None };
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        match o[0] {
            0 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as _;
                {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.experience.level = veterancy_from_i32((o[2] % 4) as i32);
                    ob.status = status_from(&o[3..5]);
                }
                w.module_call(me, idx, ModuleCall::OnDie(info));
                tally.died += 1;
            }
            1 => {}
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
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
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
                    CutCall::OclCreate { ocl, .. } => log.push(format!("ocl {}", ocl_code(ocl) - 1)),
                    CutCall::WeaponCreateAndFireTemp { weapon, source, position } => {
                        let name = WEAPONS[weapon_code(weapon) as usize - 1];
                        log.push(format!("weapon {name} {source} {},{},{}", position.x.to_bits(), position.y.to_bits(), position.z.to_bits()))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx, .. }) = event_iter.next().flatten() {
                    log.push(format!("fx {}", String::from_utf8_lossy(&fx)));
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                log.push(r.to_string());
            } else if t.starts_with("destroy ") {
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
        let dead = match w.objects.find(me).unwrap().modules[ai_idx.unwrap() as usize].body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => p.answers.ai_dead,
            _ => panic!("the AI"),
        };
        put_u32(&mut v, u32::from(dead));
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R16_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn instant_death_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0016_1D0E);
    let mut read = Check::new("InstantDeathBehavior R16 data read (throw, message)");
    let mut run = Check::new("InstantDeathBehavior R16 ops (data, effects, xfers, AI)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let coords = [&0.0f32, &10.0, &-12.5, &25.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R16{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 6];
        cfg[0] = u32::from(g.below(5) != 0) | u32::from(g.below(6) == 0) << 1;
        cfg[1] = pick(&mut g, &coords).to_bits();
        cfg[2] = pick(&mut g, &coords).to_bits();
        cfg[3] = pick(&mut g, &coords).to_bits();
        cfg[4] = g.u32();
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(16) {
                0..=7 => {
                    // not DEATH_NORMAL nor LEVEL_REGULAR (see the CreateObjectDie test)
                    let st = if g.below(4) == 0 { g.u32() & g.u32() } else { 0 };
                    [0, 1 + g.below(11), 1 + g.below(3), st, 0, 0]
                }
                8..=9 => [1, u32::from(g.below(5) != 0) | u32::from(g.below(4) == 0) << 1, 0, 0, 0, 0],
                10..=11 => [7, 0, 0, 0, 0, 0],
                12..=13 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r16_run(
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
        let what = || format!("case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["markdead", "fx", "ocl", "weapon", "destroy"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key} ")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "InstantDeathBehavior R16: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onDie calls, {} xfers, {} effect lines",
        tally.died, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(counts.get("destroy").copied().unwrap_or(0) * 4 > cases as u64, "the generator must reach the death");
}
