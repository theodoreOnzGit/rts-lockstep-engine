#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `TechBuildingBehavior` (`logic::modules::tech_building_behavior`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `TechBuildingBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_tech_building_o.inc`; its `#include "PreRTS.H"`
//! is found through a one-line forwarding header, the file system upstream was built on being case-insensitive) on the module build's stand-ins: an `Object` with a
//! team and a controlling player and a drawable that logs the model-condition calls, three players (the first is the neutral player) each with a default team and
//! a playable flag, `FXList::doFXObj`, an `Object::setTeam` reduced to what a module sees (a changed team changes the controlling player and tells the modules
//! `onCapture`, Object.cpp `setOrRestoreTeam`), a game logic whose frame the test sets and whose wake recorder counts the calls `setWakeFrame` makes, and the REAL
//! `UpdateModule`, `INI`, FXList parse and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with one object on a team of one of three players, the module from `modules::new_module`, the REAL `World::set_team`
//! (whose `onCapture` reaches the module), `update()` run as the scheduler runs it (`World::with_body`), `onDie` and `onCapture` as `World::module_call`, and the
//! xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (`PulseFX`, `PulseFXRate`; mostly well formed) and, when the read succeeds, makes the module (its wake calls are compared) and runs 1 to
//! 40 ops: an update, `onDie`, `onCapture`, a new controlling player, a new frame, new playable flags, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in order (`fx<id>`, `mc`, `mcclear`, `setteam`), the number
//! of wake calls and the frame of the last, the xfer status, bytes and CRC, and the object's team.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). `Object::setTeam` is a stand-in on the C++
//! side (the Rust side runs the verified one); the stand-in is what the module can observe of it.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9TB_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 560 data blocks threw (read compared: 9000 comparisons, 0 mismatches), 0 cases excluded, 172414 ops, 77640 updates, 17120 deaths, 25983 xfers, 124405 effect lines (21947 pulses, 7698 team changes); ops: 197734 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run found two defects of the test and one of the module: the Rust test did not ask for the wake trace (`probe_effect_trace`), and `onDie` did not lend the module to its slot, so the `onCapture` the team change sends to the module itself was skipped as re-entrant (fixed with `lend_clone`, as the crates do).
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1191 | the captured condition is not cleared when the building is not playable |
//! | 1192 | `onDie` leaves the team alone |
//! | 1193 | `onCapture` does not wake the module |
//! | 1194 | a pulse rate of 0 still plays the FX list |
//! | 1195 | `onDie` gives the building to the second player's team |
//!
//! All 5 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::tech_building_behavior::TechBuildingBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::team::Team;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9tb_o.bin");
const XFER_FILE: &str = "l9tb_o.bin";
const TEAM0: u32 = 100;

#[derive(Default)]
struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    fn id(&mut self, name: &Option<Vec<u8>>) -> i32 {
        match name {
            None => 0,
            Some(n) => {
                let next = self.0.len() as i32 + 1;
                *self.0.entry(n.clone()).or_insert(next)
            }
        }
    }
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::TechBuildingBehavior(Arc::new(
        TechBuildingBehaviorModuleData::friend_new_module_data(Some(ini))?,
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

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let (fx, rate) = (r.u32() as i32, r.u32());
    out.push(format!("data fx {fx} rate {rate}"));
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
        let team = r.u32() as i32;
        out.push(format!("op {i}: status {st} sleep {sleep} log {log:?} wakes {wakes} last {last} {x} team {team}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 80 {
        let v = pick(g, &["FxA", "FxB", "None", "fxa"]).to_string();
        lines.push(format!("PulseFX{}{v}", gen_sep(g)));
    }
    if g.below(100) < 85 {
        let v = match g.below(10) {
            0 => "0".to_string(),
            1..=6 => pick(g, &["34", "67", "100", "33", "200", "1000"]).to_string(),
            7 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
            _ => format!("{}", g.below(1500)),
        };
        lines.push(format!("PulseFXRate{}{v}", gen_sep(g)));
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

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the controlling player + 1, the playable flags]
    let cfg = vec![
        g.below(300),
        if g.below(100) < 90 { 1 + g.below(3) } else { 0 },
        if g.below(100) < 80 { 7 } else { g.below(8) },
    ];
    let n_ops = 1 + g.below(40) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=44 => [0, 0, 0, 0, 0],
            45..=54 => [1, 0, 0, 0, 0],
            55..=62 => [2, g.below(3), g.below(3), 0, 0],
            63..=72 => [3, g.below(4), 0, 0, 0],
            73..=78 => [4, g.below(400), 0, 0, 0],
            79..=84 => [5, g.below(8), 0, 0, 0],
            85..=90 => [6, 0, 0, 0, 0],
            91..=95 => [7, 0, 0, 0, 0],
            _ => [8, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    pulses: u64,
    deaths: u64,
    team_changes: u64,
}

fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32],
    ops: &[[u32; 5]],
    tally: &RefCell<Tally>,
) -> RunOut {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    {
        let ModuleDataKind::TechBuildingBehavior(d) = &data else {
            unreachable!("not a TechBuildingBehavior")
        };
        put_i32(&mut v, fx.id(&d.pulse_fx));
        put_u32(&mut v, d.pulse_fx_rate);
    }

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        ..Default::default()
    });
    w.names = ctx.names;
    w.set_frame(cfg[0]);
    w.cuts.probe_effect_trace = true;
    for k in 0..3u32 {
        w.teams.insert_team(Team::new(TEAM0 + k, 0)).unwrap();
        w.cuts.team_players.insert(TEAM0 + k, k as i32);
        w.cuts.player_default_teams.insert(k as i32, TEAM0 + k);
        if cfg[2] >> k & 1 == 0 {
            w.cuts.unplayable_players.insert(k as i32);
        }
    }
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"TechBuildingBehavior",
        b"ModuleTag_Tech",
        &data,
    )
    .unwrap()
    .unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let team = if cfg[1] != 0 {
        Some(TEAM0 + cfg[1] - 1)
    } else {
        None
    };
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team,
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1);
    w.friend_bind_to_drawable(me, 7);
    let idx = w.find_module(me, b"TechBuildingBehavior").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let first_trace = w.cuts.trace.take().unwrap_or_default();
    let mut first_trace = Some(first_trace);
    let mut saved: Vec<u8> = Vec::new();
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let team_before = w.objects.find(me).unwrap().team;
        let (mut xst, mut xbytes, mut crc, mut sleep) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    sleep = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0;
                    tally.borrow_mut().updates += 1;
                }
                1 => {
                    w.module_call(me, idx, ModuleCall::OnDie(DamageInfo::default()));
                    tally.borrow_mut().deaths += 1;
                }
                2 => {
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::OnCapture {
                            old_owner: o[1] as i32 % 3,
                            new_owner: o[2] as i32 % 3,
                        },
                    );
                }
                3 => {
                    w.objects.find_mut(me).unwrap().team =
                        if o[1] != 0 { TEAM0 + o[1] - 1 } else { 0 }
                }
                4 => w.set_frame(o[1]),
                5 => {
                    for k in 0..3i32 {
                        if o[1] >> k & 1 != 0 {
                            w.cuts.unplayable_players.remove(&k);
                        } else {
                            w.cuts.unplayable_players.insert(k);
                        }
                    }
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
            // an op that is the test's own (a new controlling player) is not an effect of the module
            if o[0] == 3 {
                events_before = w.events.events().len();
            }
        } else {
            events_before = 0;
        }
        let team_after = w.objects.find(me).unwrap().team;
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
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if t.starts_with("evt ") && phase > 0 {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Set(f),
                        ..
                    }) => log.push(format!("mc {}", f[0])),
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Clear(f),
                        ..
                    }) => log.push(format!("mcclear {}", f[0])),
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => {
                        tally.borrow_mut().pulses += 1;
                        log.push(format!("fx{}", fx.id(&Some(name))));
                    }
                    _ => {}
                }
            }
        }
        if phase > 0 && ops[phase - 1][0] != 3 && team_after != team_before {
            tally.borrow_mut().team_changes += 1;
            log.push(format!("setteam {}", team_after as i32 - TEAM0 as i32));
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
        put_i32(
            &mut v,
            if team_after == 0 {
                -1
            } else {
                team_after as i32 - TEAM0 as i32
            },
        );
    }
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded: false,
    }
}

#[test]
fn tech_building_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "behavior module O7",
        seed: 0x007E_C4B1,
        cases_var: "L9TB_CASES",
        ini_stem: "Tb",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9tb_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "behavior module O7: {} cases, {} data blocks threw, {} excluded, {} ops, {} updates, {} deaths, {} xfers, {} effect lines, {} pulses, {} team changes",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.deaths, t.xfers, t.effects, t.pulses, t.team_changes
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.pulses * 10 > c.cases as u64
            && t.team_changes * 10 > c.cases as u64,
        "the generator must reach every path"
    );
}
