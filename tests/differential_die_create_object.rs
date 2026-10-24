#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CreateObjectDie` (`logic::modules::create_object_die`) against the unchanged C++, `p1w13-sonnet`, die batch R14a.
//!
//! # Method
//!
//! The C++ side is `CreateObjectDie.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dies_r14.inc`) on the module build's
//! stand-ins: the dying object (id 1: maybe a body answering its maximum, previous health, subdual damage and last damage source, an AI, a veterancy level
//! and status bits), the damage dealer (2), the object the creation list makes (3: maybe a body; a damage is a line), one more object (4), each maybe
//! with an AI whose `transferAttack` is a line; the game logic's list newest first; the lists `OCL_A` and `OCL_B` (`l9_upgrades_b.inc`), which make
//! the object the test names or nothing.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule`s for the bodies and the AIs),
//! the creation list's result answered by `cuts.ocl_create_result`, and every op a `World::module_call` or a change to the scene.
//!
//! Each case reads a `D` block (the four die mux fields, `CreationList`, `TransferPreviousHealth`; mostly well formed) and runs 1 to 12 ops. Compared,
//! bit for bit: the read's throw; the data; per op the throw status, the effects in order (`ocl`, `oclsec`, `attemptdamage`, `damage`,
//! `transferattack`), the xfer status, bytes and CRC.
//!
//! Death type `DEATH_NORMAL` and level `LEVEL_REGULAR` are never generated: upstream's `getDeathTypeFlag` and `getVeterancyLevelFlag` shift by -1 for
//! them, which the shipped 32-bit build and this 64-bit C++ build answer differently (the Rust follows the shipped game: `logic::damage::death_bit`,
//! `common::game_common::veterancy_bit`).
//!
//! # Results (2026-10-23, `L9R14_CASES=9000`)
//!
//! 9,000 cases (7,221 compared; 1,779 data blocks threw, as the C++ did), 47,519 ops (23,753 `onDie` calls, 16,624 xfers) and 61,961 op outputs:
//! **0 mismatches**. Effect lines compared: ocl 8,465, oclsec 2,778, transferattack 5,552, damage 508, attemptdamage 317.
//!
//! Harness correction on the way: the first runs generated `DEATH_NORMAL` and `LEVEL_REGULAR` (above); the 607 and then 82 mismatches were all those.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 6 of 6 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1751 | the die mux ignored | 1,720 |
//! | 1752 | the subdual damage not passed on | 317 |
//! | 1753 | the whole maximum health taken off | 870 |
//! | 1754 | the damage without its source | 363 |
//! | 1755 | no transfer at all | 2,655 |
//! | 1756 | the attackers not moved | 2,600 |

mod support;
mod up_common;

use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::modules::create_object_die::CreateObjectDieModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r14.bin");
const XFER_FILE: &str = "l9_r14.bin";
const CLASS: &str = "CreateObjectDie";

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
    *out = Some(ModuleDataKind::CreateObjectDie(Arc::new(
        CreateObjectDieModuleData::friend_new_module_data(Some(ini))?,
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

/// The data dump of `ref_l9r14_run`: the die mux, the list (`OCL_A` 0, `OCL_B` 1, -1 for none or no list), the flag.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::CreateObjectDie(d) = data else {
        unreachable!("not a CreateObjectDie")
    };
    put_die_mux(v, &d.die.die_mux_data);
    put_i32(
        v,
        match d.ocl.as_deref() {
            Some(b"OCL_A") => 0,
            Some(b"OCL_B") => 1,
            _ => -1,
        },
    );
    put_u32(v, u32::from(d.transfer_previous_health));
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
    let ocl = u32_(&mut p) as i32;
    let transfer = u32_(&mut p);
    out.push(format!("data mux {mux:x?} ocl {ocl} transfer {transfer}"));
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
        out.push(format!(
            "op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"
        ));
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

/// The block `D`: the die mux, the list and the flag (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 25 {
        lines.push(format!("DeathTypes = {}", death_tokens(g)));
    }
    if g.below(100) < 15 {
        lines.push(format!("VeterancyLevels = {}", level_tokens(g)));
    }
    if g.below(100) < 10 {
        lines.push(format!("ExemptStatus = {}", status_tokens(g)));
    }
    if g.below(100) < 10 {
        lines.push(format!("RequiredStatus = {}", status_tokens(g)));
    }
    if g.below(100) < 85 {
        lines.push(format!(
            "CreationList = {}",
            pick(g, &["OCL_A", "OCL_B", "OCL_A", "ocl_a", "None", "OCL_C"])
        ));
    }
    if g.below(100) < 70 {
        lines.push(format!("TransferPreviousHealth = {}", gen_bool(g, clean)));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "TransferPreviousHealth = Maybe".to_string(),
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

/// The scene's module indices: each object's AI probe, the two bodies.
struct Slots {
    ai: [u16; 4],
    body1: u16,
    body3: u16,
}

/// The flags (bits 0-3 the objects' AIs, 4 object 1's body, 5 object 3's), applied to the scene.
fn apply_flags(w: &mut World, flags: u32, s: &Slots) {
    for k in 0..4 {
        w.objects.find_mut(k as i32 + 1).unwrap().ai = if flags >> k & 1 != 0 {
            Some(s.ai[k])
        } else {
            None
        };
    }
    w.objects.find_mut(1).unwrap().body = (flags & 16 != 0).then_some(s.body1);
    w.objects.find_mut(3).unwrap().body = (flags & 32 != 0).then_some(s.body3);
}

fn set_ocl_results(w: &mut World, a: u32, b: u32) {
    w.cuts.ocl_create_result.clear();
    for (name, id) in [(&b"OCL_A"[..], a), (b"OCL_B", b)] {
        if id != 0 {
            w.cuts
                .ocl_create_result
                .insert(name.to_vec(), id as ObjectId);
        }
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    cfg: &[u32; 12],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        // the fixture's two lists (the case matters)
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        object_creation_list_store: ocls,
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let ai = || {
        let mut a = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
        a.is_ai = true;
        a.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers::default(),
            0.0,
            Default::default(),
        )));
        a
    };
    let body = |answers: ProbeAnswers, max: f32| {
        let mut b = NewModule::plain(b"CtBody", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            answers,
            max,
            Default::default(),
        )));
        b
    };
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        CLASS.as_bytes(),
        b"ModuleTag_Die",
        &data,
    )
    .unwrap()
    .unwrap();
    let answers1 = ProbeAnswers {
        previous_health: Some(f32::from_bits(cfg[2])),
        subdual_damage: Some(f32::from_bits(cfg[3])),
        last_damage_source: Some(cfg[4] as ObjectId),
        ..Default::default()
    };
    let me = w
        .new_object(NewObject {
            template: plain.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m, ai(), body(answers1, f32::from_bits(cfg[1]))],
        })
        .unwrap();
    assert_eq!(me, 1);
    for k in 2..=4 {
        // object 3's body never dies of what it is given (the driver's only logs)
        let mut mods = vec![ai()];
        if k == 3 {
            mods.push(body(ProbeAnswers::default(), 1.0e9));
        }
        let id = w
            .new_object(NewObject {
                template: plain.clone(),
                status: ObjectStatusMaskType::default(),
                team: Some(1),
                modules: mods,
            })
            .unwrap();
        assert_eq!(id, k);
    }
    let slot = |w: &World, id: ObjectId, name: &[u8]| {
        w.objects
            .find(id)
            .unwrap()
            .modules
            .iter()
            .position(|m| m.name == name)
            .unwrap() as u16
    };
    let slots = Slots {
        ai: [
            slot(&w, 1, b"CtProbe"),
            slot(&w, 2, b"CtProbe"),
            slot(&w, 3, b"CtProbe"),
            slot(&w, 4, b"CtProbe"),
        ],
        body1: slot(&w, 1, b"CtBody"),
        body3: slot(&w, 3, b"CtBody"),
    };
    let mut flags = cfg[0];
    set_ocl_results(&mut w, cfg[5], cfg[6]);
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            flags = o[1];
        }
        if o[0] == 2 {
            set_ocl_results(&mut w, o[1], o[2]);
        }
        apply_flags(&mut w, flags, &slots);
        let errors_before = w.cuts.module_errors.len();
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
            1 | 2 => {}
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
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                if let Some(CutCall::OclCreateWithSecondary { secondary, ocl, .. }) =
                    cut_iter.next()
                {
                    log.push(format!(
                        "ocl {}",
                        if ocl.as_slice() == b"OCL_A" { 0 } else { 1 }
                    ));
                    if *secondary != INVALID_ID {
                        log.push(format!("oclsec {secondary}"));
                    }
                }
            } else if let Some(r) = t.strip_prefix("bt ") {
                log.push(r.to_string());
            } else if t.starts_with("damage ") || t.starts_with("attemptdamage ") {
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
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R14_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn create_object_die_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0014_C0D1);
    let mut read = Check::new("CreateObjectDie R14a data read (throw, message)");
    let mut run = Check::new("CreateObjectDie R14a ops (data, effects, xfers)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    let healths = [&0.0f32, &50.0, &100.0, &250.5, &1000.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R14a{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly two bodies, an OCL that makes object 3, AIs on some objects
        let mut cfg = [0u32; 12];
        cfg[0] = (g.u32() & 15) | u32::from(g.below(6) != 0) << 4 | u32::from(g.below(6) != 0) << 5;
        cfg[1] = pick(&mut g, &healths).to_bits();
        cfg[2] = pick(&mut g, &healths).to_bits();
        cfg[3] = (*pick(&mut g, &[&0.0f32, &0.0, &30.0, &-5.0])).to_bits();
        cfg[4] = *pick(&mut g, &[&0u32, &2, &4, &9]);
        cfg[5] = *pick(&mut g, &[&3u32, &3, &3, &0, &4]);
        cfg[6] = *pick(&mut g, &[&3u32, &0, &2]);
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=9 => {
                    // not DEATH_NORMAL (0): `getDeathTypeFlag` shifts by -1 there, which the 64-bit C++ build and the shipped one answer differently
                    // (`logic::damage::death_bit`)
                    let death = 1 + g.below(11);
                    // not LEVEL_REGULAR (0): `getVeterancyLevelFlag` shifts by -1 there, as `getDeathTypeFlag` does (`common::game_common::veterancy_bit`)
                    let level = 1 + g.below(3);
                    let st = if g.below(3) == 0 {
                        g.u32() & g.u32()
                    } else {
                        0
                    };
                    [0, death, level, st, 0, *pick(&mut g, &[&2u32, &0, &9])]
                }
                10..=11 => [
                    1,
                    (g.u32() & 15)
                        | u32::from(g.below(6) != 0) << 4
                        | u32::from(g.below(6) != 0) << 5,
                    0,
                    0,
                    0,
                    0,
                ],
                12 => [
                    2,
                    *pick(&mut g, &[&3u32, &0, &4]),
                    *pick(&mut g, &[&3u32, &0]),
                    0,
                    0,
                    0,
                ],
                13..=15 => [7, 0, 0, 0, 0, 0],
                16..=17 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
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
            cpp::ref_l9r14_run(
                0,
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
                "ocl ",
                "oclsec",
                "attemptdamage",
                "damage",
                "transferattack",
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
        "CreateObjectDie R14a: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onDie calls, {} xfers, {} effect lines",
        tally.died, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        counts.get("transferattack").copied().unwrap_or(0) * 10 > cases as u64,
        "the generator must reach the transfer"
    );
}
