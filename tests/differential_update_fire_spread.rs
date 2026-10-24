#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireSpreadUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FireSpreadUpdate` (`logic::modules::fire_spread_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `FireSpreadUpdate.cpp` with three mechanical edits made by `build.rs` into `mod_gen` (each asserted to apply the stated number of times): the two
//! function-local `static` name keys lose their `static` (the run resets the name key generator for every case), and its calls `fu->wouldIgnite()` and
//! `fu->tryToIgnite()` on another object's `FlammableUpdate` name the driver's functions `l9fsWouldIgnite` / `l9fsTryToIgnite` (the real `FlammableUpdate` needs a
//! body, a drawable and a sound manager and has its own test; here the test says what the candidates answer). It is compiled into `wrap_module.cpp`'s unit
//! (`differential/l9_fire_spread_o.inc`) on the module build's stand-ins: four `Object`s, the `findUpdateModule` hook, a partition manager whose `getClosestObject` logs
//! and runs the filters (the module's own `PartitionFilterFlammable`) over the candidates the test lists, nearest first, the object creation lists `OCL_A` and `OCL_B`
//! (they log), a game logic whose frame the test sets and whose wake recorder counts the calls `setWakeFrame` makes, and the REAL `UpdateModule`, `INI`, `Xfer` and
//! logic random values.
//!
//! The Rust side is the real thing: a `World` with four objects (the candidates have a probe `FlammableUpdate` that answers `wouldIgnite` and logs `tryToIgnite`, or
//! none), the module from `modules::new_module`, the candidates in `cuts.range_scan`, `update()` run as the scheduler runs it (`World::with_body`),
//! `startFireSpreading` as `ModuleCall::StartFireSpreading` and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`OCLEmbers`, `MinSpreadDelay`, `MaxSpreadDelay`, `SpreadTryRange`; mostly well formed) and, when the read succeeds, makes the module (its
//! wake calls are compared) and runs 1 to 40 ops: an update, `startFireSpreading`, the object aflame or not, a candidate list, a candidate's flammability, a new frame, and
//! the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in order (`ocl`, `closest`, `wouldignite`, `tryignite`), the
//! wake calls (their number and the frame of the last), the xfer status, bytes and CRC.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). `getClosestObject` is a stand-in on both
//! sides (the partition manager is verified by `differential_partition`): the module's filter is what is under test.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9FS_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 836 data blocks threw (read compared: 8999 comparisons, 0 mismatches), 1 case excluded, 201403 ops, 80373 updates, 23471 xfers, 99546 effect lines (30784 ember lists, 38289 closest-object scans, 13566 ignitions); ops: 225892 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run found one defect of the test, none of the module: the wake count included the helpers' constructor wakes (the Rust object has eight helper modules; the count is of the module under test, by its index). The flammable test (batch N) was re-run with the real `FireSpreadUpdate` in place of its stand-in: 9000 cases, 152266 comparisons, 0 mismatches, plants 981-989 caught.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1201 | an object that is not aflame still spreads |
//! | 1202 | `startFireSpreading` ignores whether the object is aflame |
//! | 1203 | the spread delay may be 0 |
//! | 1204 | a candidate that has a `FlammableUpdate` passes the filter whether or not it would ignite |
//! | 1205 | the closest candidate is not told to ignite |
//! | 1206 | the ember list is not created |
//! | 1207 | a negative range is no range |
//!
//! All 7 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_AFLAME;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::fire_spread_update::FireSpreadUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9fs_o.bin");
const XFER_FILE: &str = "l9fs_o.bin";

fn ocl_code(name: &Option<Vec<u8>>) -> u32 {
    match name.as_deref() {
        Some(b"OCL_A") => 1,
        Some(b"OCL_B") => 2,
        _ => 0,
    }
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FireSpreadUpdate(Arc::new(FireSpreadUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    out.push(format!("data {f:x?}"));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut sleep) = (0, 0);
        if i > 0 {
            st = r.u32() as i32;
            sleep = r.u32() as i32;
        }
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let mut x = String::new();
        if i > 0 {
            x = r.xfer();
        }
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} wakes {wakes} last {last} {x}"));
    }
    out
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=8 => pick(g, &["33", "67", "100", "200", "500", "1000", "34"]).to_string(),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => format!("{}", g.below(1500)),
    }
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 75 {
        let v = pick(g, &["OCL_A", "OCL_B", "None", "Other"]).to_string();
        lines.push(format!("OCLEmbers{}{v}", gen_sep(g)));
    }
    if g.below(100) < 80 {
        let v = gen_dur(g, clean);
        lines.push(format!("MinSpreadDelay{}{v}", gen_sep(g)));
    }
    if g.below(100) < 80 {
        let v = gen_dur(g, clean);
        lines.push(format!("MaxSpreadDelay{}{v}", gen_sep(g)));
    }
    if g.below(100) < 80 {
        let v = match g.below(8) {
            0 => "0".to_string(),
            1..=5 => pick(g, &["50", "100", "75.5", "300"]).to_string(),
            6 if !clean => pick(g, &["x", "", "-"]).to_string(),
            _ => gen_real(g, clean),
        };
        lines.push(format!("SpreadTryRange{}{v}", gen_sep(g)));
    }
    if !clean && g.below(12) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the random seed]
    let cfg = vec![g.below(300), g.u32()];
    let n_ops = 1 + g.below(40) as usize;
    // most cases begin with the object aflame and some flammable candidates
    let mut ops: Vec<[u32; 5]> = Vec::new();
    if g.below(100) < 85 {
        ops.push([2, 1, 0, 0, 0]);
    }
    if g.below(100) < 80 {
        ops.push([3, 1 + g.below(7), g.below(2), 0, 0]);
        for k in 0..3 {
            ops.push([4, k, 1 + g.below(3), 0, 0]);
        }
    }
    ops.extend((0..n_ops).map(|_| match g.below(100) {
        0..=44 => [0, 0, 0, 0, 0],
        45..=54 => [1, 0, 0, 0, 0],
        55..=60 => [2, g.below(2), 0, 0, 0],
        61..=68 => [3, g.below(8), g.below(2), 0, 0],
        69..=78 => [4, g.below(3), g.below(4), 0, 0],
        79..=82 => [5, g.below(400), 0, 0, 0],
        83..=88 => [6, 0, 0, 0, 0],
        89..=93 => [7, 0, 0, 0, 0],
        94..=96 => [8, 0, 0, 0, 0],
        _ => [0, 0, 0, 0, 0],
    }));
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    embers: u64,
    ignites: u64,
    scans: u64,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    {
        let ModuleDataKind::FireSpreadUpdate(d) = &data else { unreachable!("not a FireSpreadUpdate") };
        put_u32(&mut v, ocl_code(&d.ocl_embers));
        put_u32(&mut v, d.min_spread_try_delay);
        put_u32(&mut v, d.max_spread_try_delay);
        put_u32(&mut v, d.spread_try_range.to_bits());
    }

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FireSpreadUpdate", b"ModuleTag_Spread", &data).unwrap().unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules: vec![m] }).unwrap();
    assert_eq!(me, 1);
    let idx = w.find_module(me, b"FireSpreadUpdate").unwrap();
    let constructor_trace = w.cuts.trace.take().unwrap_or_default();
    for k in 0..3 {
        // the candidates' flammable module is renamed to be absent
        let mut f = NewModule::plain(b"XFlammableUpdate", b"ModuleTag_Flammable");
        f.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules: vec![f] }).unwrap();
        assert_eq!(id, 2 + k);
    }
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    let mut first_trace = Some(constructor_trace);
    for phase in 0..=ops.len() {
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    w.module_call(me, idx, ModuleCall::StartFireSpreading);
                }
                2 => w.objects.find_mut(me).unwrap().status.set(OBJECT_STATUS_AFLAME, i32::from(o[1] != 0)),
                3 => {
                    let mut ids: Vec<i32> = (0..3).filter(|k| o[1] >> k & 1 != 0).map(|k| k + 2).collect();
                    if o[2] != 0 {
                        ids.reverse();
                    }
                    w.cuts.range_scan = ids;
                }
                4 => {
                    let id = 2 + (o[1] % 3) as i32;
                    let ob = w.objects.find_mut(id).unwrap();
                    for m in ob.modules.iter_mut() {
                        if m.name == b"XFlammableUpdate" || m.name == b"FlammableUpdate" {
                            m.name = if o[2] & 1 != 0 { b"FlammableUpdate".to_vec() } else { b"XFlammableUpdate".to_vec() };
                            if let Some(UpdateBody::ModuleProbe(p)) = m.body.as_mut() {
                                p.answers.yes = o[2] & 2 != 0;
                            }
                        }
                    }
                }
                5 => w.set_frame(o[1]),
                6..=8 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 6, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 6 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        }
        let trace = if phase == 0 { first_trace.take().unwrap_or_default() } else { w.cuts.trace.take().unwrap() };
        let cuts = w.cuts.log.take().unwrap_or_default();
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let mut cut_iter = cuts.into_iter();
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if t.starts_with("cut ") && phase > 0 {
                match cut_iter.next() {
                    Some(CutCall::OclCreate { ocl, .. }) => {
                        tally.borrow_mut().embers += 1;
                        match ocl.as_slice() {
                            b"OCL_A" => log.push("ocl 0".to_string()),
                            b"OCL_B" => log.push("ocl 1".to_string()),
                            _ => {}
                        }
                    }
                    Some(CutCall::PartitionGetClosestObject { object, range }) => {
                        tally.borrow_mut().scans += 1;
                        // FROM_CENTER_3D is 1
                        log.push(format!("closest {object} {:08x} 1", range.to_bits()));
                    }
                    _ => {}
                }
            } else if t.starts_with("wouldignite ") || t.starts_with("tryignite ") {
                if t.starts_with("tryignite ") {
                    tally.borrow_mut().ignites += 1;
                }
                log.push(t);
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        if phase > 0 {
            put_i32(&mut v, 0);
            put_u32(&mut v, sleep);
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, if wakes > 0 { last } else { 0 });
        if phase > 0 {
            put_i32(&mut v, xst);
            put_u32(&mut v, xbytes.len() as u32);
            for b in &xbytes {
                put_u32(&mut v, u32::from(*b));
            }
            put_u32(&mut v, crc);
        }
    }
    RunOut { dump: v, keys: Vec::new(), excluded: false }
}

#[test]
fn fire_spread_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O8",
        seed: 0x00F1_5E80,
        cases_var: "L9FS_CASES",
        ini_stem: "Fs",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9fs_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O8: {} cases, {} data blocks threw, {} excluded, {} ops, {} updates, {} xfers, {} effect lines, {} embers, {} scans, {} ignitions",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.embers, t.scans, t.ignites
    );
    assert!(c.read_failed * 40 > c.cases as u64 && c.ops_run * 5 > c.cases as u64 * 4 && t.embers * 5 > c.cases as u64 && t.ignites * 10 > c.cases as u64, "the generator must reach every path");
}
