// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/{PowerPlantUpdate, RadarUpdate}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! Two layer-9 update modules (`logic::modules::{power_plant_update, radar_update}`, over `modules::update_behavior`) against the unchanged C++,
//! `p1w11-sonnet`, batch E. The first update modules that run inside the `World`: the scheduler's `update()` lends the module to its slot, and a
//! wake call goes to the sleepy list.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_updates_e.inc`) on the module build's
//! stand-ins: an `Object` with or without a `Drawable`, a `Drawable` that logs the model-condition calls (a set, a clear of a set of flags in ascending
//! bit order, a clear-and-set), a game logic whose frame the test sets and whose wake recorder counts `setWakeFrame`'s calls, and the REAL
//! `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` holding an object with the module from `modules::new_module`; `update()` is run as the scheduler runs it
//! (`World::with_body` over `UpdateBody::update`), the interface calls (`extendRods`, `extendRadar`) go through `World::module_call`, the xfers through
//! `World::xfer_module_slot` (the slot's scheduling half, then the base, then the module's fields), the drawable is bound or unbound, and the effects are
//! read from the trace (`cuts.probe_effect_trace` makes the host log each wake call).
//!
//! Each case reads a `D` block (`RodsExtendTime` or `RadarExtendTime`, mostly well formed, now and then damaged), and when the read succeeds runs 1 to 14
//! ops: `update()`, the interface call, the frame, the drawable present or not, an xfer save, load of the last save, and CRC.
//!
//! Compared, bit for bit: the read's throw; the data; the constructor's wake calls; per op the throw status, the sleep time `update()` returned, the effects
//! in order, the number of wake calls and the frame of the last, the xfer status, bytes and CRC, and the module's own state.
//!
//! Inputs for which upstream is undefined (a `RadarUpdate` whose frame plus extend time is outside the range of an `UnsignedInt`, converted from a float)
//! are never sent to the C++: the Rust records an `is_undefined()` error and the case is counted as excluded.
//!
//! # Results
//!
//! (2026-10-21, 9000 cases, `L9UE_CASES` overrides)
//!
//! 8,795 data reads and 78,410 dump lines (56,105 ops: 15,069 `update()`, 13,476 interface calls, 15,038 xfers) compared, 0 mismatches; 205 inputs
//! excluded as undefined upstream (a radar whose frame plus extend time leaves the range of `unsigned int`); 1,360 data blocks threw on both sides.
//! Effects: 5,909 `mc` (set), 5,822 `clearset`, 1,684 `clearflags`.
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save (after the five versions) are masked in both dumps: that word is the
//! sleepy list's (`World::awaken_update_module`, covered by its own tests), and the C++ stand-in has no scheduler. Everything else in the bytes, and
//! the CRC, is compared.
//!
//! # Upstream behaviour kept
//!
//! `RadarUpdate::extendRadar` computes `m_extendDoneFrame = TheGameLogic->getFrame() + modData->m_radarExtendTime`: an `UnsignedInt` plus a `Real`,
//! so a single-precision sum (a frame above 2^24 loses its low bits) converted to `UnsignedInt`. The Rust does the same sum; a sum outside the range of
//! `unsigned int` is undefined in C++ and is an `Undefined::FloatToUnsignedOutOfRange` error here.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000.
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 731 | PowerPlantUpdate | `update()` does not swap the model conditions |
//! | 732 | PowerPlantUpdate | `extendRods(TRUE)` acts even when the rods are out |
//! | 736 | PowerPlantUpdate | `extendRods(FALSE)` does not sleep for ever |
//! | 738 | PowerPlantUpdate | the constructor does not sleep for ever |
//! | 733 | RadarUpdate | `extendRadar` adds the time as an integer (truncating it) |
//! | 734 | RadarUpdate | the extension completes on the due frame, not after it |
//! | 735 | RadarUpdate | the radar is not made active |
//! | 737 | RadarUpdate | `extendRadar` does not set the extending condition |

mod support;
mod up_common;

use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::power_plant_update::PowerPlantUpdateModuleData;
use rts_lockstep_engine::logic::modules::radar_update::RadarUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9updates_e.bin");
const XFER_FILE: &str = "l9updates_e.bin";

/// The class names of the kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 2] = ["PowerPlantUpdate", "RadarUpdate"];

// -- the dump, in ref_l9ue_run's format --

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

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    let kind = out.0;
    out.1 = Some(match kind {
        0 => ModuleDataKind::PowerPlantUpdate(Arc::new(PowerPlantUpdateModuleData::friend_new_module_data(Some(ini))?)),
        _ => ModuleDataKind::RadarUpdate(Arc::new(RadarUpdateModuleData::friend_new_module_data(Some(ini))?)),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9ue_run`: the extend time.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::PowerPlantUpdate(d) => put_u32(v, d.rods_extend_time),
        ModuleDataKind::RadarUpdate(d) => put_u32(v, d.radar_extend_time.to_bits()),
        _ => unreachable!("not an update module"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: usize, b: &[u8], n_ops: usize) -> Vec<String> {
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
    out.push(format!("time {:#x}", u32_(&mut p)));
    let (n, f) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor wakes {n} last {f}"));
    let _ = kind;
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
        if bytes.len() >= 9 {
            // the scheduling half's frame-and-phase word (after the five versions) is the sleepy list's: the C++ stand-in has no scheduler
            bytes[5..9].iter_mut().for_each(|b| *b = 0);
        }
        let crc = u32_(&mut p);
        let nstate = u32_(&mut p);
        let state: Vec<u32> = (0..nstate).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:?}"));
    }
    out
}

// -- generators --

/// The block `D`: the extend time (a duration in milliseconds), now and then damaged or with an unknown field.
fn gen_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 92 {
        if kind == 0 {
            lines.push(format!("RodsExtendTime = {}", gen_int(g, clean)));
        } else {
            lines.push(format!("RadarExtendTime = {}", gen_real(g, clean)));
        }
    }
    if !clean && g.below(2) == 0 {
        lines.push("NoSuchField = 1".to_string());
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
    xfers: u64,
    updates: u64,
    extends: u64,
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

/// The effect lines of a trace, in the driver's format, and the wake calls of module `idx` among them (the object's helper modules wake too).
fn effects(trace: Vec<String>, idx: u16) -> (Vec<String>, u32, u32) {
    let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
    for t in trace {
        if let Some(f) = t.strip_prefix("awaken ") {
            let (i, f) = f.split_once(' ').unwrap();
            if i.parse::<i32>().unwrap() == i32::from(idx) {
                wakes += 1;
                last = f.parse().unwrap();
            }
        } else if let Some(rest) = t.strip_prefix("evt DrawableModelCondition") {
            // `{ object_id: N, op: Set([x]) }`, `Clear([a, b])`, `ClearAndSet { clear: [a], set: [b] }`
            let nums = |s: &str| -> Vec<String> { s.split(|c: char| !c.is_ascii_digit() && c != '-').filter(|x| !x.is_empty()).map(str::to_string).collect() };
            if let Some(x) = rest.split("Set([").nth(1).filter(|_| !rest.contains("ClearAndSet")) {
                log.push(format!("mc {}", nums(x.split("])").next().unwrap()).join(",")));
            } else if rest.contains("ClearAndSet") {
                let clr = rest.split("clear: [").nth(1).unwrap().split(']').next().unwrap();
                let set = rest.split("set: [").nth(1).unwrap().split(']').next().unwrap();
                log.push(format!("clearset {} {}", nums(clr).join(","), nums(set).join(",")));
            } else if let Some(x) = rest.split("Clear([").nth(1) {
                log.push(format!("clearflags {}", nums(x.split("])").next().unwrap()).join(",")));
            }
        }
    }
    (log, wakes, last)
}

/// Runs the ops on a fresh world and returns the dump (after the data dump) and the world.
fn rust_run(kind: usize, data: ModuleDataKind, cfg: &[u32; 1], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    let mut names = NameKeyGenerator::new();
    names.reset();
    w.names = names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASSES[kind].as_bytes(), b"ModuleTag_Update", &data).unwrap().unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: None, modules: vec![m] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    w.friend_bind_to_drawable(me, if cfg[0] != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let idx = w.find_module(me, CLASSES[kind].as_bytes()).unwrap();
    let handle = ModuleHandle { object: me, index: idx };
    let u = w.update_handle_of_module(handle).expect("an update module");
    // the constructor's wake calls
    let (_, wakes, last) = effects(w.cuts.trace.take().unwrap_or_default(), idx);
    put_u32(&mut v, wakes);
    put_u32(&mut v, last);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0i32);
        match o[0] {
            0 => {
                sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0 as i32;
                tally.updates += 1;
            }
            1 => {
                let call = if kind == 0 { ModuleCall::ExtendRods { extend: o[1] != 0 } } else { ModuleCall::ExtendRadar };
                w.module_call(me, idx, call);
                tally.extends += 1;
            }
            2 => w.set_frame(o[1]),
            3 => w.friend_bind_to_drawable(me, if o[1] != 0 { 7 } else { INVALID_DRAWABLE_ID }),
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
        let (mut log, wakes, last) = effects(w.cuts.trace.take().unwrap(), idx);
        if o[0] == 3 {
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
            Some(UpdateBody::PowerPlantUpdate(m)) => Some(m.state_words()),
            Some(UpdateBody::RadarUpdate(m)) => Some(m.state_words()),
            _ => None,
        });
        let state = state.expect("the module is back in its slot");
        put_u32(&mut v, state.len() as u32);
        for s in state {
            put_u32(&mut v, s);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9UE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

/// A frame: small, around where a float stops counting by ones, or near the top of the range.
fn gen_frame(g: &mut Gen) -> u32 {
    match g.below(6) {
        0..=2 => g.below(3000),
        3 => 16_777_216 - 4 + g.below(10),
        4 => 100_000 + g.below(1000),
        _ => 4_000_000_000 + g.below(200_000_000),
    }
}

#[test]
fn update_modules_e_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_EEE5);
    let mut read = Check::new("update module E data read (throw, message)");
    let mut run = Check::new("update module E ops (data, wakes, effects, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 2];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 2;
        cpp_reset(true, true);
        let clean = g.below(10) < 7;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\Ue{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the drawable is present]
        let cfg = [u32::from(g.below(5) != 0)];
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(30) {
                0..=7 => [0, 0, 0, 0, 0, 0],
                8..=14 => [1, g.below(3), 0, 0, 0, 0],
                15..=19 => [2, gen_frame(&mut g), 0, 0, 0, 0],
                20..=21 => [3, g.below(2), 0, 0, 0, 0],
                22..=23 => [4, 0, 0, 0, 0, 0],
                24..=25 => [5, 0, 0, 0, 0, 0],
                _ => [6, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(kind, data.expect("read ok"), &cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9ue_run(
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
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["\"mc ", "\"clearset ", "\"clearflags "] {
                *counts.entry(key.trim_matches('"').to_string()).or_default() += l.matches(key).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
    }
    println!(
        "update modules E: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} update() calls, {} interface calls, {} xfers, {} effect lines",
        tally.updates, tally.extends, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.effects * 4 > cases as u64, "the generator must reach every path");
}
