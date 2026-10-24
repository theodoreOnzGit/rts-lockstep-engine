#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BridgeTowerBehavior` (`logic::modules::bridge_tower_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R38.
//!
//! # Method
//!
//! The C++ side is the file, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_bridge_tower_r38.inc`, `ref_l9r38_run`), on
//! the module build's stand-ins: six objects (ids 1-6; the kinds `BRIDGE`, `BRIDGE_TOWER` and `STRUCTURE`; maybe the module build's `MBody`,
//! whose maximum is 100, 250, 1000 or 0, at half health, and whose `attemptDamage` and `attemptHealing` are lines; up to three behaviour
//! probes (the active-body driver's `AbProbe`), each maybe a bridge interface with its four tower ids); the tower (object 1);
//! `Object::attemptDamage` (the body's); the REAL `Object::attemptHealing` and `Object::kill`; `BehaviorModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`; `ProbeModule` bodies, whose
//! damage and heal are lines of the trace, and three `ProbeModule` behaviours, whose bridge interface (`has` bit 10, `bridge_towers`) the
//! scene turns on and off; a probe the C++ scene leaves out has none), the real `World::attempt_damage`, `attempt_healing` and `kill`.
//!
//! Each case reads a block `D` (mostly well formed), sets the bridge and the tower type, and runs 1 to 80 ops: a damage or a healing (of an
//! amount, a damage and death type and a source), a body state change, a death, one of the tower interface's calls, the interface lookup on
//! an object, a change to the scene, or an xfer. Compared, bit for bit: the read's throw; per op the throw status, the result, the damage and
//! heal lines in order, the xfer status, bytes and CRC, the module's bridge id and tower type, and every object's health.
//!
//! # Results (2026-10-24, `L9R38_CASES=9000`)
//!
//! 9,000 cases (8,033 compared; 967 excluded, undefined upstream: 774 in which the tower, a tower it passes damage or healing to, or the
//! bridge has no body, and 193 whose block names a field, which reaches the plain `ModuleData`'s NULL field parse table), 319,115 ops
//! (95,881 damages, 63,967 healings, 7,979 deaths, 64,019 xfers; 1,126 interface lookups found the module) and 646,263 outputs: **0
//! mismatches**; 65,114 damage lines and 39,209 heal lines compared.
//!
//! On the way: the first run differed on every damage of type `DAMAGE_SUBDUAL_UNRESISTABLE` (34): the Rust probe body only logs that type
//! (a seam of the subdual helper's test), where `MBody` takes it. The generator no longer makes it.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2081 | a tower damaged by the fraction, not the fraction of its maximum | 113,400 |
//! | 2082 | the damage passed on keeps its source, not this tower | 11,530 |
//! | 2083 | the damage type not copied | 11,772 |
//! | 2084 | the death type not copied | 11,485 |
//! | 2085 | damage from a tower passed on (only the bridge's filtered) | 156,014 |
//! | 2086 | the bridge healed by the raw amount | 46,713 |
//! | 2087 | this tower not skipped among the bridge's towers | 6,803 |
//! | 2088 | the bridge killed with `DEATH_NONE` | 6,397 |
//! | 2089 | the tower type saved before the bridge id | 20,746 |

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::{DamageInfo, DAMAGE_HEALING, DAMAGE_NUM_TYPES, DAMAGE_SUBDUAL_UNRESISTABLE, DEATH_NONE, DEATH_NUM_TYPES};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r38.bin");
const XFER_FILE: &str = "l9_r38.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r38Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_STRUCTURE];
/// The bodies' maximum health, by index (the driver's `g_r38Max`).
const MAX: [f32; 4] = [100.0, 250.0, 1000.0, 0.0];
const PROBES: [&[u8]; 3] = [b"R38P0", b"R38P1", b"R38P2"];

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
    *out = Some(rts_lockstep_engine::logic::modules::bridge_tower_behavior::new_module_data(Some(ini))?);
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
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
        let state: Vec<u32> = (0..2 + 6).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut s = String::from("D\n");
    if !clean {
        s.push_str(pick(g, &["NoSuchField = 1", "Bridge = Yes", "TowerType"]));
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// What the compared cases reached (a case excluded as undefined upstream counts for nothing).
#[derive(Default, Clone)]
struct Tally {
    damages: u64,
    heals: u64,
    deaths: u64,
    xfers: u64,
    damage_lines: u64,
    heal_lines: u64,
    found: u64,
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

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R38Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The slots of each object's body and three probes.
struct Slots {
    body: [u16; 6],
    probes: [[u16; 3]; 6],
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 24], s: &Slots) {
    for k in 0..6usize {
        let f = cfg[3 + k];
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.thing.friend_set_template(template_of(f & 7));
        o.body = (f >> 3 & 1 != 0).then_some(s.body[k]);
        let np = f >> 6 & 3;
        for i in 0..3usize {
            let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.probes[k][i] as usize].body.as_mut() else { unreachable!() };
            let bridge = (i as u32) < np && f >> (8 + i) & 1 != 0;
            p.answers.has = if bridge { 1 << 10 } else { 0 };
            for t in 0..4 {
                p.answers.bridge_towers[t] = (cfg[9 + i] >> (4 * t) & 15) as ObjectId;
            }
        }
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    let probe = |name: &[u8], max: f32, interfaces: i32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        let mut p = ProbeModule::new(ProbeAnswers::default(), max, Default::default());
        p.health = max * 0.5;
        m.body = Some(UpdateBody::ModuleProbe(p));
        m
    };
    let mut slots = Slots { body: [0; 6], probes: [[0; 3]; 6] };
    for k in 0..6usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BridgeTowerBehavior", b"ModuleTag_Me", &data).unwrap().unwrap());
        }
        modules.push(probe(b"R38Body", MAX[(cfg[3 + k] >> 4 & 3) as usize], MODULEINTERFACE_BODY));
        for name in PROBES {
            modules.push(probe(name, 0.0, 0));
        }
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        slots.body[k] = w.find_module(id, b"R38Body").unwrap();
        for (i, name) in PROBES.iter().enumerate() {
            slots.probes[k][i] = w.find_module(id, name).unwrap();
        }
    }
    let idx = w.find_module(1, b"BridgeTowerBehavior").unwrap();
    set_scene(&mut w, &cfg, &slots);
    w.module_call(1, idx, ModuleCall::BridgeTowerSetBridge(cfg[1] as ObjectId));
    w.module_call(1, idx, ModuleCall::BridgeTowerSetTowerType(cfg[2] as i32));
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 7 && (3..=11).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        set_scene(&mut w, &cfg, &slots);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let info = |o: &[u32; 4]| {
            let mut d = DamageInfo::default();
            d.input.amount = f32::from_bits(o[1]);
            d.input.damage_type = if o[0] == 1 { DAMAGE_HEALING } else { (o[2] & 255) as i32 };
            d.input.death_type = if o[0] == 1 { DEATH_NONE } else { (o[2] >> 8) as i32 };
            d.input.source_id = o[3] as ObjectId;
            d
        };
        match o[0] {
            0 => {
                tally.damages += 1;
                w.module_call(1, idx, ModuleCall::OnDamage(info(o)));
            }
            1 => {
                tally.heals += 1;
                w.module_call(1, idx, ModuleCall::OnHealing(info(o)));
            }
            2 => {
                w.module_call(1, idx, ModuleCall::OnBodyDamageStateChange { info: info(o), old_state: o[1] as i32, new_state: o[2] as i32 });
            }
            3 => {
                tally.deaths += 1;
                w.module_call(1, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            4 => {
                w.module_call(1, idx, ModuleCall::BridgeTowerSetBridge(o[1] as ObjectId));
            }
            5 => {
                w.module_call(1, idx, ModuleCall::BridgeTowerSetTowerType(o[1] as i32));
            }
            6 => {
                res = w.module_call(1, idx, ModuleCall::BridgeTowerGetBridgeId).as_id() as u32;
            }
            8..=10 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 8, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 8 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            11 => {
                res = match w.bridge_tower_interface(o[1] as ObjectId) {
                    None => 0,
                    Some(i) if o[1] == 1 && i == idx => {
                        tally.found += 1;
                        1
                    }
                    Some(_) => 2,
                };
            }
            7 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let status = if new_errors.iter().any(|(_, e)| e.kind == IniErrorKind::BadArg) {
            3
        } else if new_errors.iter().any(|(_, e)| !e.is_undefined()) {
            2
        } else {
            0
        };
        put_i32(&mut v, status);
        put_u32(&mut v, res);
        let log: Vec<String> = w.cuts.trace.take().unwrap().into_iter().filter(|t| t.starts_with("damage ") || t.starts_with("heal ")).collect();
        for l in &log {
            if l.starts_with("damage ") {
                tally.damage_lines += 1;
            } else {
                tally.heal_lines += 1;
            }
        }
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
        let ModuleReply::Id(bridge) = w.module_call(1, idx, ModuleCall::BridgeTowerGetBridgeId) else { panic!("the module answers") };
        put_u32(&mut v, bridge as u32);
        let ob = w.objects.find(1).unwrap();
        let Some(UpdateBody::BridgeTowerBehavior(m)) = ob.modules[idx as usize].body.as_ref() else { panic!("the module is back in its slot") };
        put_u32(&mut v, m.tower_type() as u32);
        for k in 0..6usize {
            let o = w.objects.find(k as ObjectId + 1).unwrap();
            let Some(UpdateBody::ModuleProbe(p)) = o.modules[slots.body[k] as usize].body.as_ref() else { unreachable!() };
            put_u32(&mut v, p.health.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R38_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

/// An object id: none (0), one of the six (mostly the bridge and towers 2-5), or no object (7).
fn gen_id(g: &mut Gen) -> u32 {
    match g.below(12) {
        0 => 0,
        1 => 7,
        2 => 1,
        3 => 6,
        _ => 2 + g.below(4),
    }
}

/// An object's scene word (`k` is its index: object 2 is mostly the bridge, objects 3-5 mostly towers).
fn gen_object(g: &mut Gen, k: usize) -> u32 {
    let kinds = match k {
        1 => u32::from(g.below(6) != 0) | u32::from(g.below(8) == 0) << 1,
        0 | 2..=4 => u32::from(g.below(10) == 0) | u32::from(g.below(5) != 0) << 1,
        _ => g.below(8),
    } | u32::from(g.below(3) == 0) << 2;
    let body = u32::from(g.below(40) != 0) << 3;
    let max = if g.below(25) == 0 { 3 } else { g.below(3) } << 4;
    let np = if k == 1 { 1 + g.below(3) } else { g.below(4) } << 6;
    let bridges = (0..3).fold(0, |acc, i| acc | u32::from(g.below(if k == 1 && i == 0 { 5 } else { 3 }) != 0 && (k == 1 || g.below(2) == 0)) << (8 + i));
    kinds | body | max | np | bridges
}

/// A probe's four tower ids.
fn gen_towers(g: &mut Gen) -> u32 {
    (0..4).fold(0, |acc, t| acc | (if g.below(8) == 0 { gen_id(g) } else { 3 + t % 3 }) << (4 * t))
}

#[test]
fn bridge_tower_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0038_B71D);
    let mut read = Check::new("R38 data read (throw, message)");
    let mut run = Check::new("R38 ops (effects, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        // a field in the block reaches the plain ModuleData's NULL field parse table (undefined upstream): rarely
        let clean = g.below(50) != 0;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R38{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 24];
        // the bridge is never the tower itself (the probe body dies at zero health, MBody does not: see the module's tests)
        cfg[1] = loop {
            let b = gen_id(&mut g);
            if b != 1 {
                break b;
            }
        };
        cfg[2] = if g.below(10) == 0 { 4 + g.below(10) } else { g.below(4) };
        for k in 0..6 {
            cfg[3 + k] = gen_object(&mut g, k);
        }
        for i in 0..3 {
            cfg[9 + i] = gen_towers(&mut g);
        }
        let amount = |g: &mut Gen| -> u32 { [1.0f32, 5.0, 10.0, 0.0, 20.0, 2.5, -5.0, 60.0][g.below(8) as usize].to_bits() };
        let n_ops = 1 + g.below(80) as usize;
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=11 => {
                    // not DAMAGE_SUBDUAL_UNRESISTABLE: the probe body only logs that one (the subdual helper's test), MBody takes it
                    let t = loop {
                        let t = g.below(DAMAGE_NUM_TYPES as u32);
                        if t != DAMAGE_SUBDUAL_UNRESISTABLE as u32 {
                            break t;
                        }
                    };
                    [0, amount(&mut g), t | g.below(DEATH_NUM_TYPES as u32) << 8, gen_id(&mut g)]
                }
                12..=19 => [1, amount(&mut g), 0, gen_id(&mut g)],
                20 => [2, g.below(4), g.below(4), gen_id(&mut g)],
                21 => [3, 0, 0, 0],
                22 => {
                    let b = loop {
                        let b = gen_id(&mut g);
                        if b != 1 {
                            break b;
                        }
                    };
                    [4, b, 0, 0]
                }
                23 => [5, g.below(6), 0, 0],
                24 => [6, 0, 0, 0],
                25..=29 => {
                    let a = 3 + g.below(9);
                    let b = if a <= 8 { gen_object(&mut g, a as usize - 3) & !(3 << 4) | cfg[a as usize] & (3 << 4) } else { gen_towers(&mut g) };
                    [7, a, b, 0]
                }
                30..=32 => [8, 0, 0, 0],
                33..=35 => [9, 0, 0, 0],
                36..=37 => [10, 0, 0, 0],
                _ => [11, gen_id(&mut g), 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let tally_before = tally.clone();
            let (rd, w) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if let Some((_, e)) = w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
                *reasons.entry(format!("{e}")).or_insert(0u64) += 1;
                excluded += 1;
                tally = tally_before;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r38_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
    println!(
        "R38: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} damages, \
         {} healings, {} deaths, {} xfers, {} interface lookups found the module), {} damage lines, {} heal lines (the compared cases')",
        tally.damages, tally.heals, tally.deaths, tally.xfers, tally.found, tally.damage_lines, tally.heal_lines
    );
    println!("R38 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.damage_lines > cases as u64 && tally.heal_lines > cases as u64 && tally.found > cases as u64 / 20, "the generator must reach every effect");
}
