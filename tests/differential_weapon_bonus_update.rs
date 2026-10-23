#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WeaponBonusUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `WeaponBonusUpdate` (`logic::modules::weapon_bonus_update`) against the unchanged C++, `p1w13-sonnet`, batch R17.
//!
//! # Method
//!
//! The C++ side is `WeaponBonusUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_weapon_bonus_r17.inc`; the module
//! build's `Weapon.h` stand-in gained `TheWeaponBonusNames`, verbatim) on the module build's stand-ins: the module's object (id 1) and four more,
//! each with the kinds the test gives and maybe a contain with passengers among them; the partition manager's range scan (the objects the test
//! lists, the call a line); `doTempWeaponBonus` a line; and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` contains),
//! `cuts.crate_stub_object_calls` set (the bonus is a record), the scan `cuts.range_scan`, every op an `update()` as the scheduler runs it or a
//! change to the scene.
//!
//! Each case reads a block `D` (`RequiredAffectKindOf`, `ForbiddenAffectKindOf`, `BonusDuration`, `BonusDelay`, `BonusRange`,
//! `BonusConditionType`), mostly well formed, and runs 1 to 16 ops. Compared, bit for bit: the read's throw; the data; per op the throw status,
//! the sleep time, the effects in order (`scan`, `tempbonus`), the xfer status, bytes and CRC.
//!
//! # Results (2026-10-23, `L9R17W_CASES=9000`)
//!
//! 9,000 cases (7,171 compared, none excluded; 1,829 data files threw, as the C++ did), 61,504 ops (27,662 updates, 15,535 xfers) and 75,846 op
//! outputs: **0 mismatches**; 70,617 bonuses compared.
//!
//! Harness correction on the way, none in the translation: the saved scheduling word differed (6,118 mismatches, all xfer bytes), because the
//! stand-in game logic records the constructor's `setWakeFrame(UPDATE_SLEEP_NONE)` and drops it, where `GameLogic::friend_awakenUpdateModule`
//! sets the module's next call frame for an object not yet in the list (`GameLogic.cpp:3035-3046`); the driver now does that.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 3 of 3 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1831 | the objects in range never get the bonus | 21,247 |
//! | 1832 | the passengers never get it | 10,689 |
//! | 1833 | the delay one frame long | 27,662 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::weapon_bonus_update::WeaponBonusUpdateModuleData;
use rts_lockstep_engine::logic::weapon_bonus::WEAPON_BONUS_NAMES;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r17w.bin");
const XFER_FILE: &str = "l9_r17w.bin";
const CLASS: &str = "WeaponBonusUpdate";
/// The kinds the objects are given, by bit (the driver's `g_r17wKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 4] = [(KINDOF_INFANTRY, "INFANTRY"), (KINDOF_VEHICLE, "VEHICLE"), (KINDOF_STRUCTURE, "STRUCTURE"), (KINDOF_AIRCRAFT, "AIRCRAFT")];

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
fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::WeaponBonusUpdate(Arc::new(WeaponBonusUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r17w_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::WeaponBonusUpdate(d) = data else { unreachable!("not a WeaponBonusUpdate") };
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(d.required_affect_kind_of.test(i)));
    }
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(d.forbidden_affect_kind_of.test(i)));
    }
    put_u32(v, d.bonus_duration);
    put_u32(v, d.bonus_delay);
    put_u32(v, d.bonus_range.to_bits());
    put_i32(v, d.bonus_condition_type);
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
    let set: Vec<usize> = kinds.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, _)| i).collect();
    let f: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
    out.push(format!("data kinds {set:?} fields {f:x?}"));
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
    }
    out
}

// -- generators --

fn kind_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(2) {
        t.push(KINDS[g.below(4) as usize].1.to_string());
    }
    if g.below(10) == 0 {
        return "NONE".to_string();
    }
    t.join(" ")
}

/// The block `D`, each field present with some probability; now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 40 {
        lines.push(format!("RequiredAffectKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 30 {
        lines.push(format!("ForbiddenAffectKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 80 {
        lines.push(format!("BonusDuration = {}", pick(g, &["0", "100", "1000", "3000"])));
    }
    if g.below(100) < 80 {
        lines.push(format!("BonusDelay = {}", pick(g, &["0", "33", "500", "1000"])));
    }
    if g.below(100) < 80 {
        lines.push(format!("BonusRange = {}", pick(g, &["0", "50", "150.5", "-1"])));
    }
    if g.below(100) < 85 {
        lines.push(format!("BonusConditionType = {}", pick(g, &["ENTHUSIASTIC", "SUBLIMINAL", "FRENZY_ONE", "HORDE"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "BonusConditionType = NOT_A_BONUS".to_string(),
            1 => "RequiredAffectKindOf = BOGUS".to_string(),
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
    updates: u64,
    xfers: u64,
    bonuses: u64,
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

/// The kinds of object `id` (bits of [`KINDS`]).
fn set_kinds(w: &mut World, id: ObjectId, bits: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects.find_mut(id).unwrap().thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The scan or a contain's passengers: the objects of `bits` (`from` onwards), in order, reversed by `rev`.
fn list(bits: u32, first: i32, n: i32, rev: u32) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = (0..n).filter(|k| bits >> k & 1 != 0).map(|k| first + k).collect();
    if bits & rev != 0 {
        v.reverse();
    }
    v
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
    w.cuts.crate_stub_object_calls = true;
    let plain = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let contain = || {
        let mut c = NewModule::plain(b"CtContain", b"ModuleTag_Contain");
        c.interfaces = MODULEINTERFACE_CONTAIN;
        c.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        c
    };
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASS.as_bytes(), b"ModuleTag_Bonus", &data).unwrap().unwrap();
    let me = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: None, modules: vec![m, contain()] }).unwrap();
    assert_eq!(me, 1);
    for k in 2..=5 {
        let id = w.new_object(NewObject { template: plain.clone(), status: ObjectStatusMaskType::default(), team: None, modules: vec![contain()] }).unwrap();
        assert_eq!(id, k);
    }
    let slots: Vec<Option<u16>> = (1..=5).map(|k| w.objects.find(k).unwrap().contain).collect();
    for (k, bits) in cfg[0..5].iter().enumerate() {
        set_kinds(&mut w, k as ObjectId + 1, *bits);
    }
    w.set_position(me, &Coord3D { x: 10.0, y: -2.5, z: 1.0 });
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let (mut scan, mut has_contain) = (cfg[11], cfg[5]);
    let mut pass: [u32; 5] = [cfg[6], cfg[7], cfg[8], cfg[9], cfg[10]];
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            1 => scan = o[1],
            2 => set_kinds(&mut w, ((o[1] - 1) % 5 + 1) as ObjectId, o[2]),
            3 => {
                let k = (o[1] - 1) % 5;
                has_contain = (has_contain & !(1 << k)) | (o[2] & 1) << k;
                pass[k as usize] = o[3];
            }
            _ => {}
        }
        w.cuts.range_scan = list(scan, 1, 5, 32);
        for k in 0..5usize {
            let id = k as ObjectId + 1;
            w.objects.find_mut(id).unwrap().contain = if has_contain >> k & 1 != 0 { slots[k] } else { None };
            if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[slots[k].unwrap() as usize].body.as_mut() {
                p.answers.contained = list(pass[k], 2, 4, 16);
            }
        }
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1..=3 => {}
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
        put_u32(&mut v, res);
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::PartitionIterateAlliesInRange { object, range } => {
                        let p = w.objects.find(*object).unwrap().get_position();
                        // the three filters: the relationship, alive, the same map status
                        log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 3", p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), range.to_bits()));
                    }
                    CutCall::ObjectDoTempWeaponBonus { object, bonus, duration } => {
                        tally.bonuses += 1;
                        log.push(format!("tempbonus {object} {bonus} {duration}"));
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
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R17W_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn weapon_bonus_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0017_B0E5);
    let mut read = Check::new("WeaponBonusUpdate R17 data read (throw, message)");
    let mut run = Check::new("WeaponBonusUpdate R17 ops (data, effects, xfers)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R17w{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 12];
        for word in &mut cfg[0..5] {
            *word = g.below(16);
        }
        cfg[5] = g.u32() & g.u32() & 31;
        for word in &mut cfg[6..11] {
            *word = g.below(32);
        }
        cfg[11] = g.below(64);
        let n_ops = 1 + g.below(16) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=8 => [0, 0, 0, 0, 0, 0],
                9..=10 => [1, g.below(64), 0, 0, 0, 0],
                11..=12 => [2, 1 + g.below(5), g.below(16), 0, 0, 0],
                13..=14 => [3, 1 + g.below(5), g.below(2), g.below(32), 0, 0],
                15..=16 => [7, 0, 0, 0, 0, 0],
                17..=18 => [8, 0, 0, 0, 0, 0],
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
            cpp::ref_l9r17w_run(
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
    let _ = WEAPON_BONUS_NAMES;
    println!(
        "WeaponBonusUpdate R17: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} updates, {} xfers, {} effect lines, {} bonuses",
        tally.updates, tally.xfers, tally.effects, tally.bonuses
    );
    read.finish();
    run.finish();
    assert!(tally.bonuses > cases as u64, "the generator must reach the bonus");
}
