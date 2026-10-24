#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FlammableUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FlammableUpdate` (`logic::modules::flammable_update`) against the unchanged C++, `p1w12-sonnet`, update batch N.
//!
//! # Method
//!
//! The C++ side is `FlammableUpdate.cpp` (with one mechanical edit of `build.rs`, asserted to apply once: the `static` of its function-local name key
//! `key_FireSpreadUpdate` is removed, because the test resets the name key generator for every case), compiled into `wrap_module.cpp`'s unit (`differential/l9_flammable_n.inc`) on the module build's
//! stand-ins: an `Object` with status bits, a `Drawable` that logs the model-condition calls, a body (`MBody`, `l9_body_m.inc`) that logs the damage and the
//! `setAflame` it is given and takes damage as the scripted probe body of the Rust side does, an audio manager that hands out handles 1, 2, 3, ... and logs
//! (`removeAudioEvent` is new), the REAL `FireSpreadUpdate` (`differential/l9_fire_spread_o.inc`; p1w13-sonnet replaced the earlier stand-in whose `startFireSpreading` logged), a game logic whose frame the test sets and whose wake recorder
//! counts the calls `setWakeFrame` makes, and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with one object (a drawable, a probe body, and the real `FireSpreadUpdate` module if the case has one; since 2026-10-21 the `spread` log line is the wake call the real module makes, counted with the flammable module's own), the
//! module from `modules::new_module`, `update()` run as the scheduler runs it (`World::with_body`), `onDamage`, `tryToIgnite` and `wouldIgnite` as
//! `World::module_call`s, and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (the seven fields; mostly well formed) and, when the read succeeds, makes the module (compared: its wake calls) and runs 1 to 30
//! ops: an update, flame or other damage (`onDamage`), `tryToIgnite`, `wouldIgnite`, a new frame, the object's aflame and burned bits set or cleared from
//! outside, and the three xfers. The body has a million points of health, so the damage never kills.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the result (the sleep time or the answer), the effects in order (`mc`,
//! `mcclear`, `damage`, `aflame`, `audio`, `audioremove`, `spread`), the wake calls, the xfer status, bytes and CRC, the seven state words, the object's aflame and
//! burned bits and the body's health.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). A case that ignites an object
//! with no body module (upstream dereferences `NULL`) is never sent to the C++: the Rust records [`Undefined::NullBodyModule`](rts_lockstep_engine::common::ini::Undefined)
//! and the case is counted as excluded. `~FlammableUpdate` (`stopBurningSound`) is not sent (no hook for a module's destruction).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9FL_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 816 data
//! blocks threw (read compared: 8185 comparisons, 0 mismatches), 815 cases excluded (an ignition of an object with no body, or a read with undefined
//! behaviour), 130159 ops, 43266 updates, 19703 xfers, 88223 effect lines (8249 ignitions, 5541 damages, 8249 burning sounds); ops: 152266 comparisons,
//! 0 mismatches, 0 NaN-payload-only, max ULP 0. The first runs found three defects of the test, none of the module: the Rust events of earlier ops were
//! replayed into later logs, the C++ kept the function-local `static` name key of `FireSpreadUpdate` across cases (a mechanical edit of `build.rs` now
//! removes it), and the first sound handle was 0.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 981 | the frame of the last flame damage is not recorded |
//! | 982 | a burned object that stops burning is normal again |
//! | 983 | `xfer` leaves out the last flame damage frame |
//! | 984 | the flame damage limit is not reset after the expiration |
//! | 985 | the sleep time ignores the damage timer |
//! | 986 | an ignition does not tell the `FireSpreadUpdate` |
//! | 987 | `tryToIgnite` also ignites an object that is aflame or burned |
//! | 988 | the burning sound is not stopped when the fire goes out |
//! | 989 | a burned delay of 0 means "burned at once" |
//!
//! All 9 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{
    ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE,
};
use rts_lockstep_engine::common::object_status_types::{
    OBJECT_STATUS_AFLAME, OBJECT_STATUS_BURNED,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::flammable_update::FlammableUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9fl_n.bin");
const XFER_FILE: &str = "l9fl_n.bin";
/// A million points of health, as float bits.
const BIG: u32 = 0x4974_2400;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FlammableUpdate(Arc::new(
        FlammableUpdateModuleData::friend_new_module_data(Some(ini))?,
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

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::FlammableUpdate(d) = data else {
        unreachable!("not a FlammableUpdate")
    };
    put_u32(v, d.burned_delay);
    put_u32(v, d.aflame_duration);
    put_u32(v, d.aflame_damage_delay);
    put_i32(v, d.aflame_damage_amount);
    put_str(v, &d.burning_sound_name);
    put_u32(v, d.flame_damage_limit_data.to_bits());
    put_u32(v, d.flame_damage_expiration_delay);
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let f: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    let sound = r.str();
    let (limit, exp) = (r.u32(), r.u32());
    out.push(format!(
        "data {f:?} sound {sound:?} limit {limit:#x} expiration {exp}"
    ));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut res) = (0, 0);
        if i > 0 {
            st = r.u32() as i32;
            res = r.u32();
        }
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let mut x = String::new();
        if i > 0 {
            x = r.xfer();
        }
        let state: Vec<u32> = (0..10).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} wakes {wakes} last {last} {x} state {state:?}"));
    }
    out
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=7 => pick(
            g,
            &["100", "300", "500", "1000", "3000", "33", "34", "67", "200"],
        )
        .to_string(),
        8 => format!("{}", g.below(20_000)),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => "1000".to_string(),
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
    field(g, 65, "BurnedDelay", v);
    let v = gen_dur(g, clean);
    field(g, 80, "AflameDuration", v);
    let v = gen_dur(g, clean);
    field(g, 70, "AflameDamageDelay", v);
    let v = match g.below(8) {
        0 => "0".to_string(),
        1 => pick(g, &["-3", "-50"]).to_string(),
        2 if !clean => pick(g, &["x", "", "99999999999"]).to_string(),
        _ => format!("{}", 1 + g.below(60)),
    };
    field(g, 70, "AflameDamageAmount", v);
    let v = pick(g, &["BurnLoop", "Snd2", "snd2", ""]).to_string();
    field(g, 50, "BurningSoundName", v);
    let v = match g.below(7) {
        0 => "0".to_string(),
        1 => "-1".to_string(),
        2 => "5".to_string(),
        3 => "100.5".to_string(),
        4 if !clean => pick(g, &["x", "", "1e99"]).to_string(),
        _ => "20".to_string(),
    };
    field(g, 55, "FlameDamageLimit", v);
    let v = gen_dur(g, clean);
    field(g, 50, "FlameDamageExpiration", v);
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

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the health, the maximum, the body is present, a FireSpreadUpdate is present]
    let cfg = vec![
        gen_frame(g),
        BIG,
        BIG,
        u32::from(g.below(10) != 0),
        u32::from(g.below(2) == 0),
    ];
    let n_ops = 1 + g.below(30) as usize;
    let mut frame = cfg[0];
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // a third of the cases start with flame damage that is enough to ignite and a few frames of burning
    if g.below(100) < 35 {
        ops.push([1, 6, 40.0f32.to_bits(), 0, 0]);
        for _ in 0..1 + g.below(5) {
            frame = frame.wrapping_add(1 + g.below(80));
            ops.push([4, frame, 0, 0, 0]);
            ops.push([0, 0, 0, 0, 0]);
        }
    }
    ops.extend((0..n_ops).map(|_| match g.below(100) {
        0..=24 => [0, 0, 0, 0, 0],
        25..=44 => {
            let ty = match g.below(4) {
                0 => 6,
                1 => 22,
                2 => g.below(30),
                _ => 6,
            };
            let amount = pick(g, &[&0.0f32, &5.0, &10.0, &25.0, &100.0, &-4.0]).to_owned();
            [1, ty, amount.to_bits(), 0, 0]
        }
        45..=52 => [2, 0, 0, 0, 0],
        53..=56 => [3, 0, 0, 0, 0],
        57..=72 => {
            frame = if g.below(8) == 0 {
                gen_frame(g)
            } else {
                frame.wrapping_add(1 + g.below(120))
            };
            [4, frame, 0, 0, 0]
        }
        73..=80 => [5, 0, g.below(4), 0, 0],
        81..=87 => [6, 0, 0, 0, 0],
        88..=93 => [7, 0, 0, 0, 0],
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
    ignitions: u64,
    damages: u64,
    sounds: u64,
}

fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32],
    ops: &[[u32; 5]],
    tally: &RefCell<Tally>,
) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        ..Default::default()
    });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(0);
    let mut modules = Vec::new();
    if cfg[3] != 0 {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers::default(),
            f32::from_bits(cfg[2]),
            Default::default(),
        )));
        modules.push(m);
    }
    if cfg[4] != 0 {
        let spread_data = ModuleDataKind::FireSpreadUpdate(Arc::new(rts_lockstep_engine::logic::modules::fire_spread_update::FireSpreadUpdateModuleData::default()));
        modules.push(
            rts_lockstep_engine::logic::modules::new_module(
                w.next_object_id(),
                b"FireSpreadUpdate",
                b"ModuleTag_Spread",
                &spread_data,
            )
            .unwrap()
            .unwrap(),
        );
    }
    modules.push(
        rts_lockstep_engine::logic::modules::new_module(
            w.next_object_id(),
            b"FlammableUpdate",
            b"ModuleTag_Flammable",
            &data,
        )
        .unwrap()
        .unwrap(),
    );
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: None,
            modules,
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let idx = w.find_module(me, b"FlammableUpdate").unwrap();
    let spread_idx = w.find_module(me, b"FireSpreadUpdate").map(i32::from);
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let constructor_trace = w.cuts.trace.take().unwrap_or_default();
    // a new object has the drawable the client made for it: the test binds one, as batch E does
    w.friend_bind_to_drawable(me, 7);
    let mut saved: Vec<u8> = Vec::new();
    let mut first_trace = Some(constructor_trace);
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let errors_before = w.cuts.module_errors.len();
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    res = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    let mut info = DamageInfo::default();
                    info.input.damage_type = o[1] as i32;
                    info.out.actual_damage_dealt = f32::from_bits(o[2]);
                    w.module_call(me, idx, ModuleCall::OnDamage(info));
                }
                2 => {
                    w.module_call(me, idx, ModuleCall::TryToIgnite);
                }
                3 => res = u32::from(w.module_call(me, idx, ModuleCall::WouldIgnite).as_bool()),
                4 => w.set_frame(o[1]),
                5 => {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.status
                        .set(OBJECT_STATUS_AFLAME, i32::from(o[2] & 1 != 0));
                    ob.status
                        .set(OBJECT_STATUS_BURNED, i32::from(o[2] & 2 != 0));
                }
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
        } else {
            events_before = w.events.events().len();
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let trace = if phase == 0 {
            first_trace.take().unwrap_or_default()
        } else {
            w.cuts.trace.take().unwrap()
        };
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let mut ev_iter = events.into_iter();
        if phase > 0 {
            for t in trace {
                if let Some(f) = t.strip_prefix("awaken ") {
                    let (i, f) = f.split_once(' ').unwrap();
                    // the wake calls of the two modules under test count (the REAL FireSpreadUpdate's too, in the C++ recorder as here), not the helpers'
                    if i.parse::<i32>().unwrap() == i32::from(idx)
                        || spread_idx == Some(i.parse::<i32>().unwrap())
                    {
                        wakes += 1;
                        last = f.parse().unwrap();
                    }
                } else if t.starts_with("evt ") {
                    match ev_iter.next() {
                        Some(ObjectClientEvent::DrawableModelCondition {
                            op: ModelConditionOp::Set(f),
                            ..
                        }) => log.push(format!("mc {}", f[0])),
                        Some(ObjectClientEvent::DrawableModelCondition {
                            op: ModelConditionOp::Clear(f),
                            ..
                        }) => log.push(format!("mcclear {}", f[0])),
                        Some(ObjectClientEvent::AudioAddEvent {
                            name,
                            object_id,
                            position: None,
                            player_index: None,
                        }) => {
                            log.push(format!(
                                "audio {} - -1 {object_id}",
                                String::from_utf8_lossy(&name)
                            ));
                            tally.borrow_mut().sounds += 1;
                        }
                        Some(ObjectClientEvent::AudioRemoveEvent { handle }) => {
                            log.push(format!("audioremove {handle}"))
                        }
                        _ => {}
                    }
                } else if t.starts_with("damage ") || t.starts_with("aflame ") || t == "spread" {
                    if t.starts_with("damage ") {
                        tally.borrow_mut().damages += 1;
                    }
                    if t == "aflame 1" {
                        tally.borrow_mut().ignitions += 1;
                    }
                    log.push(t);
                }
            }
        } else {
            // the constructor: only its wake calls
            for t in trace {
                if let Some(f) = t.strip_prefix("awaken ") {
                    let (i, f) = f.split_once(' ').unwrap();
                    // the wake calls of the two modules under test count (the REAL FireSpreadUpdate's too, in the C++ recorder as here), not the helpers'
                    if i.parse::<i32>().unwrap() == i32::from(idx)
                        || spread_idx == Some(i.parse::<i32>().unwrap())
                    {
                        wakes += 1;
                        last = f.parse().unwrap();
                    }
                }
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        if phase > 0 {
            put_i32(&mut v, if threw { 2 } else { 0 });
            put_u32(&mut v, if threw { 0 } else { res });
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::FlammableUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
        put_u32(
            &mut v,
            u32::from(
                w.objects
                    .find(me)
                    .unwrap()
                    .test_status(OBJECT_STATUS_AFLAME),
            ),
        );
        put_u32(
            &mut v,
            u32::from(
                w.objects
                    .find(me)
                    .unwrap()
                    .test_status(OBJECT_STATUS_BURNED),
            ),
        );
        let health = w
            .objects
            .find_mut(me)
            .unwrap()
            .modules
            .iter_mut()
            .find_map(|b| match b.body.as_mut() {
                Some(UpdateBody::ModuleProbe(p)) if b.name == b"ProbeBody" => Some(p.health),
                _ => None,
            });
        put_u32(&mut v, health.map_or(0, f32::to_bits));
    }
    let excluded = w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined());
    let _ = INVALID_DRAWABLE_ID;
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded,
    }
}

#[test]
fn flammable_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module N2",
        seed: 0x00F1_A33A,
        cases_var: "L9FL_CASES",
        ini_stem: "Fl",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9fl_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module N2: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} ignitions, {} damages, {} sounds",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.ignitions, t.damages, t.sounds
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.ignitions * 10 > c.cases as u64
            && t.damages * 10 > c.cases as u64,
        "the generator must reach every path"
    );
}
