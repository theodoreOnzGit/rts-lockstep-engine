#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicShroudClearingRangeUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `DynamicShroudClearingRangeUpdate` (`logic::modules::dynamic_shroud_clearing_range_update`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `DynamicShroudClearingRangeUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dynamic_shroud_o.inc`) on the
//! module build's stand-ins: an `Object` with a position, a shroud clearing range and a controlling player the test sets, a game logic whose frame the test
//! sets, the REAL `RadiusDecal.h` with `RadiusDecalTemplate`'s constructor and field parser verbatim from `RadiusDecal.cpp` (cut by `build.rs`) and
//! `RadiusDecal`'s own members as stand-ins that log (the client's shadow decals), and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with one object (a template with that shroud clearing range, a team with that controlling player) and the module
//! from `modules::new_module`; `update()` is run as the scheduler runs it (`World::with_body`), the decal calls are `ObjectClientEvent::GridDecal`, and the
//! xfers go through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (the seven fields and the decal template; mostly well formed) and, when the read succeeds, makes the module (its thirty decal
//! clears are compared) and runs 1 to 30 ops: a run of 1 to 21 updates, a new position, a new frame, a new shroud clearing range (a script), a new controlling
//! player, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the number of decal calls, the FNV-1a hash of all of them and the
//! first 40 (`dclear`, `dcreate`, `dpos`, `dopacity`), the xfer status, bytes and CRC, the twelve state words, the object's clearing range and its position. A float
//! that cannot be an `Int` in `animateGridDecals`, or an `Int` product that overflows, is upstream's undefined behaviour: counted, never sent to the C++.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). `~DynamicShroudClearingRangeUpdate`
//! (`killGridDecals`) is not sent (no hook for a module's destruction): **open (deviation)**.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9DS_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 1494 data
//! blocks threw (read compared: 8709 comparisons, 0 mismatches), 291 cases excluded (a decal position outside `Int`), 112591 ops, 471520 updates, 20772 xfers,
//! 3012420 effect lines (221490 decal creations, 634230 decal moves); states seen after an op: not started 15363, growing 2139, sustaining 7909, shrinking 11066,
//! done 1876, sleeping 85211; ops: 134236 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run found one defect of the test, none of the module:
//! the stand-in `Object::setShroudClearingRange` assigned without upstream's `newRange != m_shroudClearingRange` guard (Object.cpp 5185), so `-0.0` replaced `0.0`
//! in the C++ only (fixed in `l9_powers_h.inc`; the Rust `World` method has the guard).
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1121 | `m_totalFrames` has no `max(1, ..)` |
//! | 1122 | the grow deadline uses the shrink delay |
//! | 1123 | the sustain deadline is the grow deadline plus the grow time |
//! | 1124 | the failsafe frame ignores the countdown |
//! | 1125 | the decals are made at radius 99 |
//! | 1126 | the opacity is the ratio, not one minus it |
//! | 1127 | the decals snap to a grid of 22, not 23 |
//! | 1128 | growing divides by the grow time plus one |
//! | 1129 | shrinking removes the whole native range per period, not the part above the final vision |
//! | 1130 | the done-forever state never goes to sleep |
//! | 1131 | the growing interval is the change interval |
//! | 1132 | `xfer` leaves out the interval countdown |
//! | 1133 | done-forever sets the native range, not the final vision |
//! | 1134 | the constructor does not capture the native range |
//! | 1135 | the failsafe is `>=` |
//! | 1136 | sustaining does not kill the decals |
//!
//! All 16 are caught (9000 cases each), all as mismatches. **Finding (a plant that cannot be caught):** removing the `max(1.0f, ..)` of the growing and the shrinking
//! division survived 9000 cases, and cannot be caught: `GROWING` is entered only when `m_stateCountDown` is above `m_growStartDeadline - m_growTime` and at most
//! `m_growStartDeadline`, an empty range for a grow time of 0, and `SHRINKING` only when the countdown is positive and at most `m_shrinkStartDeadline`, which is the shrink
//! time, so a divisor below 1 is never reached. The two guards are dead code upstream; those two plants were replaced by 1128 and 1129 as listed.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_events::{GridDecalOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9ds_o.bin");
const XFER_FILE: &str = "l9ds_o.bin";
const TEAM: u32 = 100;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::DynamicShroudClearingRangeUpdate(Arc::new(DynamicShroudClearingRangeUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::DynamicShroudClearingRangeUpdate(d) = data else { unreachable!("not a DynamicShroudClearingRangeUpdate") };
    put_u32(v, d.shrink_delay);
    put_u32(v, d.shrink_time);
    put_u32(v, d.grow_delay);
    put_u32(v, d.grow_time);
    put_u32(v, d.final_vision.to_bits());
    put_u32(v, d.change_interval);
    put_u32(v, d.grow_interval);
    put_u32(v, u32::from(d.do_spy_sat_fx));
    let t = &d.grid_decal_template;
    put_str(v, &t.name);
    put_u32(v, t.shadow_type);
    put_u32(v, t.min_opacity.to_bits());
    put_u32(v, t.max_opacity.to_bits());
    put_u32(v, t.opacity_throb_time);
    put_u32(v, t.color as u32);
    put_u32(v, u32::from(t.only_visible_to_owning_player));
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..8).map(|_| r.u32()).collect();
    let name = r.str();
    let t: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    out.push(format!("data {f:x?} template {name:?} {t:x?}"));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let sleep = r.u32() as i32;
        let n = r.u32();
        let hash = (r.u32(), r.u32());
        let log: Vec<String> = (0..n.min(40)).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..16).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} sleep {sleep} lines {n} hash {hash:x?} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=8 => pick(g, &["33", "67", "100", "200", "300", "500", "700", "1000", "34", "133"]).to_string(),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => format!("{}", g.below(1500)),
    }
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_dur(g, clean);
    field(g, 70, "ShrinkDelay", v);
    let v = gen_dur(g, clean);
    field(g, 80, "ShrinkTime", v);
    let v = gen_dur(g, clean);
    field(g, 70, "GrowDelay", v);
    let v = gen_dur(g, clean);
    field(g, 80, "GrowTime", v);
    let v = match g.below(10) {
        0 => "0".to_string(),
        1..=6 => pick(g, &["10", "50", "150", "75.5", "300"]).to_string(),
        7 if !clean => pick(g, &["x", "", "1e99"]).to_string(),
        _ => gen_real(g, clean),
    };
    field(g, 70, "FinalVision", v);
    let v = match g.below(8) {
        0 => "0".to_string(),
        1..=5 => pick(g, &["33", "67", "100", "200", "34"]).to_string(),
        6 if !clean => pick(g, &["x", "", "-"]).to_string(),
        _ => format!("{}", g.below(600)),
    };
    field(g, 70, "ChangeInterval", v);
    let v = match g.below(8) {
        0 => "0".to_string(),
        1..=5 => pick(g, &["33", "67", "100", "34"]).to_string(),
        6 if !clean => pick(g, &["x", "", "-"]).to_string(),
        _ => format!("{}", g.below(600)),
    };
    field(g, 60, "GrowInterval", v);
    if g.below(100) < 60 {
        let mut t = String::from("GridDecalTemplate\n");
        let mut tf = |g: &mut Gen, p: u32, name: &str, value: String| {
            if g.below(100) < p {
                t.push_str(gen_indent(g));
                t.push_str(&format!("{name}{}{value}\n", gen_sep(g)));
            }
        };
        let v = pick(g, &["Ring", "Grid", "", "ring"]).to_string();
        tf(g, 80, "Texture", v);
        let v = pick(g, &["SHADOW_ALPHA_DECAL", "SHADOW_ADDITIVE_DECAL", "SHADOW_DECAL SHADOW_ALPHA_DECAL"]).to_string();
        tf(g, 40, "Style", v);
        let v = pick(g, &["25%", "100%", "0%", "50%"]).to_string();
        tf(g, 40, "OpacityMin", v);
        let v = pick(g, &["75%", "100%", "10%"]).to_string();
        tf(g, 40, "OpacityMax", v);
        let v = gen_dur(g, clean);
        tf(g, 30, "OpacityThrobTime", v);
        let v = pick(g, &["R:1 G:2 B:3", "R:255 G:0 B:0 A:128", "0"]).to_string();
        tf(g, 30, "Color", v);
        let v = gen_bool(g, clean);
        tf(g, 30, "OnlyVisibleToOwningPlayer", v);
        t.push_str("End");
        lines.push(t);
    }
    if !clean && g.below(25) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(30) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn bits(x: f32) -> u32 {
    x.to_bits()
}

fn gen_xy(g: &mut Gen) -> f32 {
    match g.below(40) {
        0 => 1.0e10,
        1 => -1.0e10,
        _ => *pick(g, &[&-10.0f32, &0.0, &5.5, &100.0, &33.25, &-250.75, &1000.0]),
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let range = *pick(g, &[&0.0f32, &100.0, &200.0, &300.0, &75.5, &200.0]);
    let cfg = vec![g.below(60), if g.below(100) < 85 { 1 + g.below(3) } else { 0 }, bits(range), bits(gen_xy(g)), bits(gen_xy(g)), bits(*pick(g, &[&0.0f32, &5.0, &-3.0]))];
    let n_ops = 1 + g.below(30) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=59 => [0, *pick(g, &[&1u32, &1, &2, &3, &5, &8, &13, &21]), 0, 0, 0],
            60..=66 => [1, bits(gen_xy(g)), bits(gen_xy(g)), bits(*pick(g, &[&0.0f32, &5.0, &-3.0])), 0],
            67..=71 => [2, g.below(200), 0, 0, 0],
            72..=77 => [3, bits(*pick(g, &[&0.0f32, &100.0, &200.0, &50.0, &300.0])), 0, 0, 0],
            78..=81 => [4, g.below(4), 0, 0, 0],
            82..=88 => [5, 0, 0, 0, 0],
            89..=95 => [6, 0, 0, 0, 0],
            _ => [7, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    creates: u64,
    positions: u64,
    ranges: u64,
    states: [u64; 6],
}

const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

fn fnv(mut h: u64, s: &str) -> u64 {
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h ^= 0xff;
    h.wrapping_mul(FNV_PRIME)
}

/// The decal calls among `events`, as the driver's lines.
fn decal_lines(events: &[ClientEvent], tally: &RefCell<Tally>) -> Vec<String> {
    let mut log = Vec::new();
    for e in events {
        let ClientEvent::Object(ObjectClientEvent::GridDecal { index, op, .. }) = e else { continue };
        match op {
            GridDecalOp::Clear => log.push(format!("dclear {index}")),
            GridDecalOp::Create { template: t, radius, position, player } => {
                tally.borrow_mut().creates += 1;
                log.push(format!(
                    "dcreate {index} {} {} {:08x} {:08x} {} {} {} {:08x} {:08x},{:08x},{:08x} {}",
                    String::from_utf8_lossy(&t.name),
                    t.shadow_type,
                    t.min_opacity.to_bits(),
                    t.max_opacity.to_bits(),
                    t.opacity_throb_time,
                    t.color,
                    i32::from(t.only_visible_to_owning_player),
                    radius.to_bits(),
                    position.x.to_bits(),
                    position.y.to_bits(),
                    position.z.to_bits(),
                    player.unwrap_or(-1)
                ));
            }
            GridDecalOp::SetPosition(p) => {
                tally.borrow_mut().positions += 1;
                log.push(format!("dpos {index} {:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))
            }
            GridDecalOp::SetOpacity(o) => log.push(format!("dopacity {index} {:08x}", o.to_bits())),
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.set_frame(cfg[0]);
    w.teams.insert_team(Team::new(TEAM, 0)).unwrap();
    let set_player = |w: &mut World, p: u32| {
        if p == 0 {
            w.cuts.team_players.remove(&TEAM);
        } else {
            w.cuts.team_players.insert(TEAM, p as i32 - 1);
        }
    };
    set_player(&mut w, cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"DynamicShroudClearingRangeUpdate", b"ModuleTag_Dsc", &data).unwrap().unwrap();
    let mut values = ObjectTemplateValues::default();
    values.shroud_clearing_range = f32::from_bits(cfg[2]);
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let events_before = w.events.events().len();
    let me = w.new_object(NewObject { template, status: Default::default(), team: Some(TEAM), modules: vec![m] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let constructor = decal_lines(&w.events.events()[events_before..], tally);
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[3]), y: f32::from_bits(cfg[4]), z: f32::from_bits(cfg[5]) });
    let idx = w.find_module(me, b"DynamicShroudClearingRangeUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = !w.cuts.module_errors.is_empty();
    for phase in 0..=ops.len() {
        let events_before = w.events.events().len();
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    for _ in 0..o[1] {
                        sleep = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                        tally.borrow_mut().updates += 1;
                    }
                }
                1 => w.set_position(me, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
                2 => w.set_frame(o[1]),
                3 => {
                    w.objects.find_mut(me).unwrap().shroud_clearing_range = f32::from_bits(o[1]);
                    tally.borrow_mut().ranges += 1;
                }
                4 => set_player(&mut w, o[1] % 4),
                5..=7 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 5 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        }
        if w.cuts.module_errors.len() > errors_before {
            assert!(w.cuts.module_errors[errors_before..].iter().all(|(_, e)| e.is_undefined()), "{:?}", w.cuts.module_errors);
            excluded = true;
        }
        let log = if phase == 0 { constructor.clone() } else { decal_lines(&w.events.events()[events_before..], tally) };
        tally.borrow_mut().effects += log.len() as u64;
        put_i32(&mut v, 0);
        put_u32(&mut v, sleep);
        let h = log.iter().fold(FNV_OFFSET, |h, l| fnv(h, l));
        put_u32(&mut v, log.len() as u32);
        put_u32(&mut v, (h >> 32) as u32);
        put_u32(&mut v, h as u32);
        for l in log.iter().take(40) {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let state = w.objects.find(me).unwrap().behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::DynamicShroudClearingRangeUpdate(m)) => Some(m.state_words()),
            _ => None,
        });
        let state = state.expect("the module is back in its slot");
        tally.borrow_mut().states[state[0] as usize] += 1;
        for s in state {
            put_u32(&mut v, s);
        }
        let o = w.objects.find(me).unwrap();
        let p = o.get_position();
        put_u32(&mut v, o.shroud_clearing_range.to_bits());
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
    }
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn dynamic_shroud_clearing_range_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module O2",
        seed: 0x00D5_C2A1,
        cases_var: "L9DS_CASES",
        ini_stem: "Ds",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9ds_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module O2: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} decal creations, {} decal moves, states seen (not started, growing, sustaining, shrinking, done, sleeping) {:?}",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.creates, t.positions, t.states
    );
    assert!(
        c.read_failed * 40 > c.cases as u64 && c.ops_run * 5 > c.cases as u64 * 4 && t.creates * 10 > c.cases as u64 * 30 && t.positions * 20 > c.cases as u64 && t.states.iter().all(|&n| n * 50 > c.cases as u64),
        "the generator must reach every path"
    );
}
