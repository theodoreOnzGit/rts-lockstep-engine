#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MissileLauncherBuildingUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `MissileLauncherBuildingUpdate` (`logic::modules::missile_launcher_building_update`) against the unchanged C++, `p1w13-sonnet`, batch R53.
//!
//! # Method
//!
//! The C++ side is `MissileLauncherBuildingUpdate.cpp` and its base `SpecialPowerUpdateModule.cpp`, compiled unchanged into
//! `wrap_module.cpp`'s unit (`differential/l9_missile_launcher_r53.inc`, `ref_l9r53_run`), with `Object::getSpecialPowerModule` verbatim, on
//! the module build's stand-ins: the silo (id 1: maybe a drawable whose model condition changes and animation loop are lines, maybe under
//! construction, maybe a special power module whose template (`SP_0`..`SP_3` of the real store, or none), readiness and ready frame the test
//! gives); the FX lists `FX_A`, `FX_B`; the audio manager (the idle sound's add and remove lines, `isCurrentlyPlaying` the test's answer);
//! the game logic's frame; the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the silo (the module from `modules::new_module`, a `ProbeModule` special power module),
//! the store the harness loaded (`harness::EXTRA_DEFS` adds the four templates to every case's definitions, on both sides); every op an
//! `update()` as the scheduler runs it, a call of the module's interface, a change to the scene, or an xfer.
//!
//! Each case reads a block `D` (the module's ten fields; mostly well formed) and runs 1 to 60 ops. Compared, bit for bit: the read's throw;
//! the data; per op the throw status, the result, the effects in order (the model conditions, the FX, the sound's play, stop and
//! `isCurrentlyPlaying` calls, the animation loop), the xfer status, bytes and CRC, and the state (the door state, the timeout state and
//! frame, the idle sound's playing handle, `isActive`).
//!
//! # Results (2026-10-25, `L9R53_CASES=9000`)
//!
//! 9,000 cases (492 data blocks threw, as the C++ did; 3,359 excluded, undefined upstream: an intent before the silo's special power module
//! was found, or for a silo whose module is never found, dereferences the `NULL` cache), 145,488 ops and 155,786 outputs: **0 mismatches**.
//! In the compared cases: 54,719 updates, 9,521 xfers, 3,366 intents taken, 2,075 idle sounds started, 37,054 effect lines; the ops ended
//! with the door closed 46,011 times, opening 12,739, open 63,241, waiting to close 17,297, closing 6,200.
//!
//! The FX fields take only `FX_A`, `FX_B` and `None`: the module build's FX store answers any other name with a list, where upstream's has
//! none of that name. The xfer's scheduling word is masked, as in the other tests on this build (the stand-in has no scheduler).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2231 | the idle sound never stopped | 3,567 |
//! | 2232 | the opening timeout at the ready frame, not one before | 12,990 |
//! | 2233 | waiting to close times out to closed | 26,471 |
//! | 2234 | the closing timeout not capped | 4,841 |
//! | 2235 | no animation loop duration | 5,499 |
//! | 2236 | a ready power does not pop the door open | 35,077 |
//! | 2237 | the timeout frame not saved | 6,369 |
//! | 2238 | an intent for another template taken | 55,998 |
//! | 2239 | the door starts opening at the ready frame | 5,545 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::missile_launcher_building_update::MissileLauncherBuildingUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r53.bin");
const XFER_FILE: &str = "l9_r53.bin";
/// The four special power templates every case defines.
const POWERS: &str = "SpecialPower SP_0\nEnd\nSpecialPower SP_1\nEnd\nSpecialPower SP_2\nEnd\nSpecialPower SP_3\nEnd\n";

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::MissileLauncherBuildingUpdate(Arc::new(MissileLauncherBuildingUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn template_id(store: &SpecialPowerStore, name: &[u8]) -> u32 {
    store.find_special_power_template(name).map_or(0, |t| t.id())
}

fn sp_name(k: u32) -> Option<Vec<u8>> {
    (k <= 3).then(|| format!("SP_{k}").into_bytes())
}

/// The FX list as the module build knows it: `FX_A` and `FX_B` (any other name is `NULL`).
fn known_fx(fx: &Option<Vec<u8>>) -> Option<Vec<u8>> {
    fx.clone().filter(|n| n == b"FX_A" || n == b"FX_B")
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let data: Vec<u32> = (0..9).map(|_| r.u32()).collect();
    let audio = r.str();
    out.push(format!("data {data:x?} audio {audio:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..5).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    // (`strict`: only the choices; the FX fields take only names the module build's FX store knows, its stand-in answering any other
    // name, which upstream's does not)
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 95, "SpecialPowerTemplate", &["SP_0", "SP_1", "SP_0", "SP_2", "SP_1", "None", "SP_9"], false);
    field(g, 80, "DoorOpenTime", &["0", "33", "100", "500", "1000"], false);
    field(g, 80, "DoorWaitOpenTime", &["0", "33", "100", "300"], false);
    field(g, 80, "DoorCloseTime", &["0", "33", "100", "400"], false);
    for f in ["DoorOpeningFX", "DoorOpenFX", "DoorWaitingToCloseFX", "DoorClosingFX", "DoorClosedFX"] {
        field(g, 50, f, &["FX_A", "FX_B", "None"], true);
    }
    field(g, 60, "DoorOpenIdleAudio", &["IdleHum", "SiloAudio"], false);
    if !clean && g.below(20) == 0 {
        lines.push("Bogus = 1".to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 4];
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    cfg[0] = *pick(g, &[&0u32, &5, &100, &299]);
    let gen_flags = |g: &mut Gen| p(g, 80) | p(g, 3) << 1 | p(g, 25) << 3 | p(g, 50) << 4;
    cfg[1] = gen_flags(g) | p(g, 92) << 2;
    // the power module's template: mostly the data's (else the module is never found, and an intent dereferences NULL)
    let data_template = match data {
        Some(ModuleDataKind::MissileLauncherBuildingUpdate(d)) => d.special_power_template.as_deref().and_then(|n| n.strip_prefix(b"SP_")).and_then(|k| std::str::from_utf8(k).ok()?.parse::<u32>().ok()),
        _ => None,
    };
    cfg[2] = match data_template {
        Some(k) if g.below(100) < 85 => k,
        _ => *pick(g, &[&0u32, &1, &2, &5]),
    };
    cfg[3] = cfg[0] + *pick(g, &[&0u32, &10, &30, &60, &200]);
    let n_ops = 1 + g.below(60) as usize;
    // an update first in most cases (the module finds its special power module there; an intent before that dereferences NULL)
    let mut ops: Vec<[u32; 5]> = if g.below(10) < 9 { vec![[2, 0, 0, 0, 0], [0, 0, 0, 0, 0]] } else { Vec::new() };
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=44 => {
                ops.push([2, 0, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            45..=52 => [0, 0, 0, 0, 0],
            53..=56 => [1, gen_frame(g), 0, 0, 0],
            57..=60 => [3, 0, 0, 0, 0],
            61..=64 => [4, 0, 0, 0, 0],
            65..=66 => [5, 0, 0, 0, 0],
            67..=74 => [6, gen_flags(g), 0, 0, 0],
            75..=82 => [7, g.below(400), 0, 0, 0],
            83..=96 => [8, *pick(g, &[&0u32, &0, &1, &2, &5]), 0, 0, 0],
            _ => [9, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    effects: u64,
    states: [u64; 5],
    intents: u64,
    sounds: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::MissileLauncherBuildingUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.special_power_template.as_deref().map_or(0, |n| template_id(&ctx.store, n)));
    for x in [d.door_open_time, d.door_wait_open_time, d.door_closing_time] {
        put_u32(&mut v, x);
    }
    for f in [&d.opening_fx, &d.open_fx, &d.waiting_to_close_fx, &d.closing_fx, &d.closed_fx] {
        put_i32(&mut v, fx.id(&known_fx(f)));
    }
    put_str(&mut v, &d.open_idle_audio);

    let store = ctx.store.clone();
    let mut w = World::new();
    w.names = ctx.names;
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let mut modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"MissileLauncherBuildingUpdate", b"ModuleTag_Silo", &data).unwrap().unwrap()];
    let power_template = sp_name(cfg[2]).map_or(0, |n| template_id(&store, &n));
    let has_power = cfg[1] >> 2 & 1 != 0;
    if has_power {
        let mut pm = NewModule::plain(b"R53Power", b"ModuleTag_Power");
        pm.interfaces = MODULEINTERFACE_SPECIAL_POWER;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { power_template, ..Default::default() }, 0.0, Default::default())));
        modules.push(pm);
    }
    // made under construction, as batch M's test does (no upgrade pass); the scene then sets the status
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let me = w.new_object(NewObject { template, status, team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let idx = w.find_module(me, b"MissileLauncherBuildingUpdate").unwrap();
    let power_idx = w.find_module(me, b"R53Power");
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let scene = |w: &mut World, cfg: &[u32]| {
        let f = cfg[1];
        let o = w.objects.find_mut(me).unwrap();
        o.drawable = if f & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 1 & 1) as i32);
        if let Some(i) = power_idx {
            if let Some(UpdateBody::ModuleProbe(p)) = o.modules[i as usize].body.as_mut() {
                p.answers.yes = f >> 3 & 1 != 0;
                p.answers.ready_frame = cfg[3];
            }
        }
        w.cuts.audio_playing = f >> 4 & 1 != 0;
    };
    scene(&mut w, &cfg);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 2 {
            let fr = w.frame().wrapping_add(1);
            w.set_frame(fr);
        }
        if o[0] == 6 {
            cfg[1] = (o[1] & !4) | (cfg[1] & 4);
        }
        if o[0] == 7 {
            cfg[3] = o[1];
        }
        scene(&mut w, &cfg);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 | 2 | 6 | 7 => {}
            3..=5 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 3 {
                    saved = xbytes.clone();
                }
                if o[0] == 4 {
                    w.load_post_process();
                }
                tally.borrow_mut().xfers += 1;
            }
            8 => {
                let template = sp_name(o[1]).map_or(0, |n| template_id(&store, &n));
                res = u32::from(w.module_call(me, idx, ModuleCall::InitiateIntentToDoSpecialPower { template, target_object: 0, target_position: None, waypoint: 0, command_options: 0 }).as_bool());
                tally.borrow_mut().intents += u64::from(res);
            }
            9 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::MissileLauncherBuildingUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let s = m.state_words();
        if o[0] == 9 {
            res = s[4];
        }
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let mut log = Vec::new();
        let mut t = tally.borrow_mut();
        for line in w.cuts.trace.take().unwrap() {
            if line.starts_with("cut ") {
                if let Some(CutCall::AudioIsCurrentlyPlaying { handle }) = cut_iter.next() {
                    log.push(format!("isplaying {handle}"));
                }
            } else if line.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition { object_id, op: ModelConditionOp::ClearAndSet { clear, set } }) => {
                        log.push(format!("mc {object_id} clear {clear:?} set {set:?}"))
                    }
                    Some(ObjectClientEvent::FxListDoFxPos { fx: name, position: p }) => {
                        if let Some(n) = known_fx(&Some(name)) {
                            log.push(format!("fxpos {} {},{},{}", fx.id(&Some(n)), p.x.to_bits(), p.y.to_bits(), p.z.to_bits()));
                        }
                    }
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                        t.sounds += 1;
                        let pos = match position {
                            Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)))
                    }
                    Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                    Some(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id, frames }) => log.push(format!("animloop {object_id} {frames}")),
                    _ => {}
                }
            }
        }
        t.effects += log.len() as u64;
        t.states[(s[0] as usize).min(4)] += 1;
        drop(t);
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
        for x in &s {
            put_u32(&mut v, *x);
        }
    }
    let excluded = match w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
        Some((_, e)) => {
            let reason = format!("{e}");
            let mut t = tally.borrow_mut();
            *t = tally_before;
            *t.excluded.entry(reason).or_insert(0) += 1;
            true
        }
        None => false,
    };
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn missile_launcher_building_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R53 MissileLauncherBuildingUpdate",
        seed: 0x0053_D00F,
        cases_var: "L9R53_CASES",
        ini_stem: "Ml",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r53_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R53: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines, {} intents taken, \
         {} idle sounds; ops ending in each door state {:?}",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.intents, t.sounds, t.states
    );
    println!("excluded by: {:?}", t.excluded);
}
