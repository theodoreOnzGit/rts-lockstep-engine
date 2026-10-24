#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/{SubdualDamageHelper, ObjectDefectionHelper}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! Two layer-9 object helpers (`logic::modules::{subdual_damage_helper, object_defection_helper}`, over `modules::update_behavior`) against the
//! unchanged C++, `p1w11-sonnet`, helper batch G. Every object has them (`Object::Object` makes the helpers before the behaviours); until now the
//! world made stand-ins for these two.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_helpers_g.inc`) on the module build's
//! stand-ins: an `Object` with a body (a heal rate, a heal amount, whether it has subdual damage, and an `attemptDamage` that logs the damage it is
//! given), a defector flag, death and firing, a `Drawable` that logs its flash, an audio manager that logs the events, a game logic whose frame the
//! test sets and whose wake recorder counts `setWakeFrame`'s calls, and the REAL `UpdateModule`, `ObjectHelper` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` whose new object gets the helpers `Object::Object` makes (the probe body of `module_probe.rs` plays the
//! body, answering the subdual questions and logging the subdual damage it is given), `update()` is run as the scheduler runs it (`World::with_body`
//! over `UpdateBody::update`), the interface calls (`notifySubdualDamage`, `startDefectionTimer`) go through `World::module_call`, the xfers through
//! `World::xfer_module_slot`, and the effects are read from the trace and the events.
//!
//! Each case runs 1 to 16 ops of its helper. Compared, bit for bit: the constructor's wake calls; per op the throw status, the sleep time `update()`
//! returned, the effects in order (`attemptdamage`, `flash`, `flashcolor`, `audio`), the wake calls and the frame of the last, the xfer status, bytes and
//! CRC, the module's own state and whether the object is an undetected defector.
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save (after the six versions of the chain here, the helper's own among
//! them) are masked in both dumps, as in `differential_update_modules_e.rs`.
//!
//! # Results
//!
//! (2026-10-21, 9000 cases, `L9HG_CASES` overrides) 95,416 dump lines (77,416 ops: 115,190 `update()` calls, 11,616 interface calls, 16,696 xfers)
//! compared, 0 mismatches; 6,069 effect lines.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000.
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 781 | SubdualDamageHelper | no wait for the countdown: it heals every frame |
//! | 782 | SubdualDamageHelper | the heal amount is not negated |
//! | 783 | SubdualDamageHelper | `notifySubdualDamage` does not wake the module |
//! | 784 | SubdualDamageHelper | it never goes back to sleep |
//! | 791 | ObjectDefectionHelper | the timer ends the moment it starts |
//! | 792 | ObjectDefectionHelper | the period lasts a frame too long |
//! | 793 | ObjectDefectionHelper | the flash phase advances at half the rate |
//! | 794 | ObjectDefectionHelper | no "ding" at the end |
//! | 795 | ObjectDefectionHelper | firing a weapon does not end the period |
//!
//! All 9 are caught.

mod support;
mod up_common;

use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::module::{ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_IS_FIRING_WEAPON};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9helpers_g.bin");
const XFER_FILE: &str = "l9helpers_g.bin";

/// The class names of the kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 2] = ["SubdualDamageHelper", "ObjectDefectionHelper"];

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
    let (n, f) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor wakes {n} last {f}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let sleep = u32_(&mut p) as i32;
        let nlog = u32_(&mut p);
        let log: Vec<String> = (0..nlog).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let mut bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        if bytes.len() >= 10 {
            // the scheduling half's frame-and-phase word (after the six versions) is the sleepy list's: the C++ stand-in has no scheduler
            bytes[6..10].iter_mut().for_each(|b| *b = 0);
        }
        let crc = u32_(&mut p);
        let nstate = u32_(&mut p);
        let state: Vec<u32> = (0..nstate).map(|_| u32_(&mut p)).collect();
        let defector = u32_(&mut p);
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:?} defector {defector}"));
    }
    out
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

/// The effect lines of a trace, in the driver's format, and the wake calls of module `idx` among them (the object's other helpers wake too).
fn effects(trace: Vec<String>, events: Vec<ObjectClientEvent>, idx: u16) -> (Vec<String>, u32, u32) {
    let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
    let mut events = events.into_iter();
    for t in trace {
        if let Some(f) = t.strip_prefix("awaken ") {
            let (i, f) = f.split_once(' ').unwrap();
            if i.parse::<i32>().unwrap() == i32::from(idx) {
                wakes += 1;
                last = f.parse().unwrap();
            }
        } else if let Some(r) = t.strip_prefix("attemptdamage ") {
            log.push(format!("attemptdamage {r}"));
        } else if t.starts_with("evt ") {
            match events.next() {
                Some(ObjectClientEvent::DrawableFlashAsSelected { .. }) => log.push("flash".to_string()),
                Some(ObjectClientEvent::DrawableFlashAsSelectedColor { r, g, b, .. }) => log.push(format!("flashcolor {},{},{}", r.to_bits(), g.to_bits(), b.to_bits())),
                Some(ObjectClientEvent::Sound { sound, object_id, .. }) => {
                    let name = match sound {
                        ObjectSound::DefectorTimerTick => "defectorTick",
                        ObjectSound::DefectorTimerDing => "defectorDing",
                        other => panic!("unexpected sound {other:?}"),
                    };
                    log.push(format!("audio {name} - -1 {}", object_id));
                }
                _ => {}
            }
        }
    }
    (log, wakes, last)
}

fn set_dead(w: &mut World, id: ObjectId, dead: bool) {
    let o = w.objects.find_mut(id).unwrap();
    if dead {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
}

fn set_body_answers(w: &mut World, id: ObjectId, rate: u32, amount: f32, has: bool) {
    let b = w.get_body_module(id).expect("a body") as usize;
    if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[b].body.as_mut() {
        p.answers.subdual_rate = rate;
        p.answers.subdual_amount = amount;
        p.answers.subdual_has = has;
    } else {
        panic!("the body is the probe");
    }
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    starts: u64,
}

/// Runs the ops on a fresh world and returns the dump.
fn rust_run(kind: usize, cfg: &[u32; 7], ops: &[[u32; 6]], tally: &mut Tally) -> Vec<u8> {
    let mut v = Vec::new();
    let mut w = World::new();
    let mut names = NameKeyGenerator::new();
    names.reset();
    w.names = names;
    w.cuts.neutral_default_team = 1;
    w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team::new(1, 0)).unwrap();
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let mut body = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
    body.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE;
    let answers = ProbeAnswers { subdual_rate: cfg[4], subdual_amount: f32::from_bits(cfg[5]), subdual_has: cfg[6] != 0, ..Default::default() };
    body.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 100.0, Default::default())));
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: None, modules: vec![body] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    w.friend_bind_to_drawable(me, if cfg[0] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    w.friend_set_undetected_defector(me, cfg[1] != 0);
    set_dead(&mut w, me, cfg[2] != 0);
    if cfg[3] != 0 {
        w.objects.find_mut(me).unwrap().set_status_bit(OBJECT_STATUS_IS_FIRING_WEAPON);
    }
    let idx = w.find_module(me, CLASSES[kind].as_bytes()).unwrap();
    let handle = ModuleHandle { object: me, index: idx };
    let u = w.update_handle_of_module(handle).expect("an update module");
    // the constructor's wake calls (the trace of the creation, which also holds the other helpers' and the binding's)
    let (_, wakes, last) = effects(w.cuts.trace.take().unwrap_or_default(), Vec::new(), idx);
    put_u32(&mut v, wakes);
    put_u32(&mut v, last);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0i32);
        let mut state_change = false;
        match o[0] {
            0 => {
                for _ in 0..o[1].max(1) {
                    sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0 as i32;
                    tally.updates += 1;
                }
            }
            1 => {
                let call = if kind == 0 { ModuleCall::NotifySubdualDamage { amount: f32::from_bits(o[1]) } } else { ModuleCall::StartDefectionTimer { time: o[1], with_defector_fx: o[2] != 0 } };
                w.module_call(me, idx, call);
                tally.starts += 1;
            }
            2 => w.set_frame(o[1]),
            3 => {
                if kind == 0 {
                    set_body_answers(&mut w, me, o[1], f32::from_bits(o[2]), o[3] != 0);
                } else {
                    w.friend_set_undetected_defector(me, o[1] & 1 != 0);
                    set_dead(&mut w, me, o[1] & 2 != 0);
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.status.set(OBJECT_STATUS_IS_FIRING_WEAPON, i32::from(o[1] & 4 != 0));
                    w.friend_bind_to_drawable(me, if o[1] & 8 != 0 { 7 } else { INVALID_DRAWABLE_ID });
                    state_change = true;
                }
            }
            4..=6 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 4, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, sleep);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut log, wakes, last) = effects(w.cuts.trace.take().unwrap(), events, idx);
        if state_change {
            // binding a drawable makes the client set the object's whole model-condition state (the client's, not the module's)
            log.clear();
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, if wakes > 0 { last } else { 0 });
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let state = w.objects.find(me).unwrap().behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::SubdualDamageHelper(m)) if kind == 0 => Some(m.state_words()),
            Some(UpdateBody::ObjectDefectionHelper(m)) if kind == 1 => Some(m.state_words()),
            _ => None,
        });
        let state = state.expect("the module is back in its slot");
        put_u32(&mut v, state.len() as u32);
        for s in state {
            put_u32(&mut v, s);
        }
        put_u32(&mut v, u32::from(w.objects.find(me).unwrap().is_undetected_defector()));
    }
    v
}

fn cases() -> usize {
    std::env::var("L9HG_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

fn gen_amount(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &0.5, &1.5, &10.0, &25.0, &-3.0, &100.0, &0.25])).to_bits()
}

#[test]
fn helpers_g_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_9996);
    let mut run = Check::new("helper G ops (wakes, effects, xfers, state)");
    let mut ops_run = 0u64;
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 2];
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 2;
        // [the drawable is present, the object is an undetected defector, it is dead, it is firing, the body's rate, its amount, it has subdual damage]
        let cfg: [u32; 7] = [
            u32::from(g.below(4) != 0),
            u32::from(g.below(4) != 0),
            u32::from(g.below(10) == 0),
            u32::from(g.below(10) == 0),
            g.below(4),
            gen_amount(&mut g),
            u32::from(g.below(2) == 0),
        ];
        let n_ops = 1 + g.below(16) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| {
                if kind == 0 {
                    match g.below(30) {
                        0..=9 => [0, g.below(5), 0, 0, 0, 0],
                        10..=15 => [1, gen_amount(&mut g), 0, 0, 0, 0],
                        16..=19 => [2, g.below(40), 0, 0, 0, 0],
                        20..=22 => [3, g.below(4), gen_amount(&mut g), g.below(2), 0, 0],
                        23..=24 => [4, 0, 0, 0, 0, 0],
                        25..=26 => [5, 0, 0, 0, 0, 0],
                        _ => [6, 0, 0, 0, 0, 0],
                    }
                } else {
                    match g.below(30) {
                        0..=11 => [0, g.below(12), 0, 0, 0, 0],
                        12..=14 => [1, *pick(&mut g, &[&0u32, &1, &5, &20, &150, &300, &450]), g.below(2), 0, 0, 0],
                        15..=20 => [2, *pick(&mut g, &[&0u32, &1, &2, &10, &100, &200, &299, &300, &301, &450]), 0, 0, 0, 0],
                        21..=23 => [3, g.below(16), 0, 0, 0, 0],
                        24..=25 => [4, 0, 0, 0, 0, 0],
                        26..=27 => [5, 0, 0, 0, 0, 0],
                        _ => [6, 0, 0, 0, 0, 0],
                    }
                }
            })
            .collect();
        let rd = rust_run(kind, &cfg, &ops, &mut tally);
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut len = 0i32;
        let status = unsafe { cpp::ref_l9hg_run(kind as i32, cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
        assert_eq!(status, 0);
        buf.truncate(len as usize);
        let what = || format!("case {case} ({}): cfg {cfg:x?}, ops {ops:?}", CLASSES[kind]);
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
    }
    println!(
        "helpers G: {cases} cases ({per_kind:?} per kind), {ops_run} ops, {} update() calls, {} interface calls, {} xfers, {} effect lines",
        tally.updates, tally.starts, tally.xfers, tally.effects
    );
    run.finish();
    assert!(ops_run * 5 > cases as u64 * 4 && tally.effects * 4 > cases as u64, "the generator must reach every path");
}
