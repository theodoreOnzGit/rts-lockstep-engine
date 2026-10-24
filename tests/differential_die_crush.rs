#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CrushDie.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CrushDie` (`logic::modules::crush_die`) against the unchanged C++, `p1w13-sonnet`, die batch R14b.
//!
//! # Method
//!
//! The C++ side is `CrushDie.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dies_r14.inc`, `ref_l9r14b_run`) on the module
//! build's stand-ins: the vehicle (id 1: maybe a body keeping its front and back crushed flags and a drawable whose `clearAndSetModelConditionFlags` is a
//! line, a position, an orientation (`getUnitDirectionVector2D` is `Thing.cpp`'s without the cache), a box geometry, a veterancy level and status
//! bits), the crusher (id 2: a position), the logic random, and an audio manager that logs.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` body keeping the two
//! flags), and every op a `World::module_call` or a change to the scene.
//!
//! Each case reads a `D` block (the four die mux fields, the three sounds and their percentages; mostly well formed) and runs 1 to 12 ops. Compared,
//! bit for bit: the read's throw; the data; per op the throw status, the effects in order (`audio`, `clearsetflags`), the xfer status, bytes and CRC,
//! and the body's two flags.
//!
//! Death type `DEATH_NORMAL` and level `LEVEL_REGULAR` are never generated: upstream's `getDeathTypeFlag` and `getVeterancyLevelFlag` shift by -1 for
//! them, which the shipped 32-bit build and this 64-bit C++ build answer differently (the Rust follows the shipped game: `logic::damage::death_bit`,
//! `common::game_common::veterancy_bit`).
//!
//! # Results (2026-10-23, `L9R14B_CASES=9000`)
//!
//! 9,000 cases (6,803 compared, 393 excluded as undefined upstream: a crush of a vehicle with no body or no drawable; 1,804 data blocks threw, as the
//! C++ did), 43,574 ops (23,285 `onDie` calls, 14,087 xfers) and 57,180 op outputs: **0 mismatches**. Effect lines compared: clearsetflags 9,841,
//! audio 2,251.
//!
//! Harness correction on the way: the first generator dropped the body or the drawable too often (792 of 3,000 cases excluded); they are now rare.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 6 of 6 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1761 | the crush points a whole major radius out | 269 |
//! | 1762 | the back crush point never tried | 4,336 |
//! | 1763 | any damage type crushes | 2,343 |
//! | 1764 | no sound | 2,251 |
//! | 1765 | the back crushed flag inverted | 29,652 |
//! | 1766 | the die mux ignored | 3,974 |
//!

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
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
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::logic::modules::crush_die::CrushDieModuleData;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::veterancy_from_i32;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r14b.bin");
const XFER_FILE: &str = "l9_r14b.bin";
const CLASS: &str = "CrushDie";

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
    *out = Some(ModuleDataKind::CrushDie(Arc::new(CrushDieModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r14b_run`: the die mux, the three sounds and the three percentages.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::CrushDie(d) = data else { unreachable!("not a CrushDie") };
    put_die_mux(v, &d.die.die_mux_data);
    for k in 0..3 {
        put_str(v, &d.crush_sounds[k].event_name);
    }
    for k in 0..3 {
        put_i32(v, d.crush_sound_percent[k]);
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
    let sounds: Vec<String> = (0..3).map(|_| str_(&mut p)).collect();
    let percents: Vec<i32> = (0..3).map(|_| u32_(&mut p) as i32).collect();
    out.push(format!("data mux {mux:x?} sounds {sounds:?} percents {percents:?}"));
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
        let (front, back) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} crushed {front} {back}"));
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

/// The block `D`: the die mux, the sounds and their percentages (each present with some probability), now and then a bogus token or an unknown field.
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
    for k in ["Total", "BackEnd", "FrontEnd"] {
        if g.below(100) < 60 {
            lines.push(format!("{k}CrushSound = {}", pick(g, &["CrushA", "CrushB", "NoSound", "nosound"])));
        }
        if g.below(100) < 40 {
            lines.push(format!("{k}CrushSoundPercent = {}", pick(g, &["0", "50", "100", "33", "-5", "150"])));
        }
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "TotalCrushSoundPercent = lots".to_string(),
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

/// The flags (bit 0 the vehicle has a body, 1 a drawable), applied to the scene.
fn apply_flags(w: &mut World, flags: u32, body: u16) {
    w.objects.find_mut(1).unwrap().body = (flags & 1 != 0).then_some(body);
    w.friend_bind_to_drawable(1, if flags & 2 != 0 { 7 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID });
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.random.init_game_logic_random(cfg[7]);
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let flags0 = cfg[0];
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASS.as_bytes(), b"ModuleTag_Die", &data).unwrap().unwrap();
    let mut b = NewModule::plain(b"CtBody", b"ModuleTag_Body");
    b.interfaces = MODULEINTERFACE_BODY;
    let answers = ProbeAnswers { front_crushed: Some(flags0 & 4 != 0), back_crushed: Some(flags0 & 8 != 0), ..Default::default() };
    b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
    let me = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, b] }).unwrap();
    assert_eq!(me, 1);
    let crusher = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: Some(1), modules: Vec::new() }).unwrap();
    assert_eq!(crusher, 2);
    let body = w.objects.find(me).unwrap().modules.iter().position(|m| m.name == b"CtBody").unwrap() as u16;
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[1]), y: f32::from_bits(cfg[2]), z: 0.0 });
    w.contain_scene_set_orientation(me, f32::from_bits(cfg[3]));
    w.objects.find_mut(me).unwrap().geometry_info = GeometryInfo::new(GeometryType::Box, false, 10.0, f32::from_bits(cfg[4]), f32::from_bits(cfg[4]));
    w.set_position(crusher, &Coord3D { x: f32::from_bits(cfg[5]), y: f32::from_bits(cfg[6]), z: 0.0 });
    let mut flags = flags0;
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            flags = o[1];
        }
        if o[0] == 3 {
            w.set_position(crusher, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: 0.0 });
        }
        apply_flags(&mut w, flags, body);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        match o[0] {
            0 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as _;
                info.input.source_id = (o[5] & 255) as _;
                info.input.damage_type = if o[5] & 256 != 0 { (o[5] >> 9) as _ } else { rts_lockstep_engine::logic::damage::DAMAGE_CRUSH };
                {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.experience.level = veterancy_from_i32((o[2] % 4) as i32);
                    ob.status = status_from(&o[3..5]);
                }
                w.module_call(me, idx, ModuleCall::OnDie(info));
                tally.died += 1;
            }
            1 | 3 => {}
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
        let mut log: Vec<String> = Vec::new();
        for e in &w.events.events()[events_before..] {
            if let rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) = e {
                match e {
                    ObjectClientEvent::AudioAddEvent { name, object_id, .. } => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(name))),
                    ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. } => {
                        let list = |l: &[i32]| l.iter().map(|i| format!(" {i}")).collect::<String>();
                        log.push(format!("clearsetflags{} |{}", list(clear), list(set)));
                    }
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
        let (front, back) = match w.objects.find(me).unwrap().modules[body as usize].body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => (p.answers.front_crushed.unwrap_or(false), p.answers.back_crushed.unwrap_or(false)),
            _ => panic!("the vehicle's body"),
        };
        put_u32(&mut v, u32::from(front));
        put_u32(&mut v, u32::from(back));
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R14B_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn crush_die_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0014_B0C5);
    let mut read = Check::new("CrushDie R14b data read (throw, message)");
    let mut run = Check::new("CrushDie R14b ops (data, effects, xfers, crushed flags)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R14b{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a body and a drawable (a crush without either is undefined upstream, and the case is not compared), the crusher near a crush point
        let mut cfg = [0u32; 12];
        cfg[0] = u32::from(g.below(40) != 0) | u32::from(g.below(40) != 0) << 1 | u32::from(g.below(3) == 0) << 2 | u32::from(g.below(3) == 0) << 3;
        let coords = [&0.0f32, &10.0, &-12.5, &25.0, &3.0];
        cfg[1] = pick(&mut g, &coords).to_bits();
        cfg[2] = pick(&mut g, &coords).to_bits();
        cfg[3] = (*pick(&mut g, &[&0.0f32, &1.5707964, &-0.75, &3.0, &2.25])).to_bits();
        cfg[4] = (*pick(&mut g, &[&10.0f32, &20.0, &5.5, &0.0])).to_bits();
        cfg[5] = pick(&mut g, &coords).to_bits();
        cfg[6] = pick(&mut g, &coords).to_bits();
        cfg[7] = g.u32();
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=9 => {
                    // not DEATH_NORMAL nor LEVEL_REGULAR (see the module docs)
                    let death = 1 + g.below(11);
                    let level = 1 + g.below(3);
                    let st = if g.below(4) == 0 { g.u32() & g.u32() } else { 0 };
                    // mostly the crusher as the source, now and then nobody or another damage type
                    let src = *pick(&mut g, &[&2u32, &2, &2, &0, &9]);
                    let ty = if g.below(8) == 0 { 256 | (g.below(12) << 9) } else { 0 };
                    [0, death, level, st, 0, src | ty]
                }
                10..=11 => [1, if g.below(10) == 0 { g.below(4) } else { 3 }, 0, 0, 0, 0],
                12..=13 => [3, pick(&mut g, &coords).to_bits(), pick(&mut g, &coords).to_bits(), 0, 0, 0],
                14..=15 => [7, 0, 0, 0, 0, 0],
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
            cpp::ref_l9r14b_run(
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
            for key in ["audio", "clearsetflags"] {
                *counts.entry(key.trim().to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
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
        "CrushDie R14b: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onDie calls, {} xfers, {} effect lines",
        tally.died, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(counts.get("clearsetflags").copied().unwrap_or(0) * 10 > cases as u64, "the generator must reach the crush");
}
