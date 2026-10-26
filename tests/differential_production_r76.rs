#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ProductionUpdate` (`logic::modules::production_update`) against the unchanged C++, `p1w13-sonnet`, batch R76.
//!
//! # Method
//!
//! The C++ side is `ProductionUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_production_r76.inc`,
//! `ref_l9r76_run`), on the module build's stand-ins: the factory (a team; sold, under construction; maybe a drawable, whose
//! `ACTIVELY_CONSTRUCTING` is the test's; selected or not; maybe an exit module whose door is the test's, maybe a parking place behaviour
//! whose answers are the test's, their calls lines; an "upgrade module" that would act on the test's upgrades once), the players (money, the
//! upgrades in production and complete, build permissions; their calls lines), the thing templates (two units, a dozer and a tower: their
//! cost and build time the test's), the build assistant's `canMakeUnit` and the object's `canProduceUpgrade` (the test's), the client's
//! sounds, messages, radar and control bar (lines), the REAL `UpgradeCenter` (four test upgrades, of either type), `UpdateModule`, `INI` and
//! `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the exit, parking place and upgrade module probes; the players' money
//! and masks the world's cut-point tables; the client's flags and selection the world's reads). Every op an `update()`, a queue, a cancel,
//! the death, a held door, the queries, a unique id, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the read's throw; the
//! data; per op the throw status, the result, the effects in order, the xfer status, bytes (the scheduling word masked) and CRC, the module's
//! state (the counters, the doors, the pending flags, every entry), the upgrades (the factory's, the upgrade module's flag, each player's
//! completed and in production) and the units made with the last one's producer and team.
//!
//! # Results (2026-10-26, `L9R76_CASES=9000`)
//!
//! 9,000 cases (2,197 data blocks threw, as the C++ did; 1,305 excluded as undefined upstream: 628 a `NULL` template or upgrade where it is
//! read (a `NULL` unit queued), 408 a queue or cancel by a factory with no controlling player, 269 `cancelUnitCreate` matching an upgrade's
//! entry), 157,184 ops and 330,862 outputs: **0 mismatches**. In the compared cases: 55,376 updates and 19,066 xfers (6,725 loads failed,
//! most into a queue that was not empty: `SC_INVALID_DATA`); 99,477 effect lines; 16,971 units and 4,536 upgrades queued, 3,336 units made,
//! 678 upgrades finished, 7,749 refunds and 10,498 door lines. The first runs differed in the test's own bookkeeping (the drawable's line
//! format, the driver deleting the module after its object let go of its modules) and in one translation defect: the queue's size was
//! compared with `MaxQueueEntries` as signed numbers, where upstream compares an `UnsignedInt` with the `Int` converted (a negative maximum
//! never fills).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2480 | `canQueueUpgrade` never full | 579 |
//! | 2481 | a cancelled player upgrade left in production | 18,082 |
//! | 2482 | every entry finished at once | 37,712 |
//! | 2483 | every unit made says `VoiceCreate` | 2,789 |
//! | 2484 | an object's upgrade not given | 11,945 |
//! | 2485 | `ACTIVELY_CONSTRUCTING` not cleared by an empty queue | 7,383 |
//! | 2486 | `cancelAllUnitsOfType` cancels nothing | 11,947 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::audio_event_name::AudioEventName;
use rts_lockstep_engine::common::disabled_types::DISABLED_COUNT;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_DOZER, KINDOF_STRUCTURE};
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODELCONDITION_ACTIVELY_CONSTRUCTING};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::common::upgrade::{UpgradeCenter, UpgradeMaskType};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::production_update::ProductionUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{EvaMessage, ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r76.bin");
const XFER_FILE: &str = "l9_r76.bin";
/// The test's upgrades: two of the player's, two of the object's, with costs, build times and sounds.
const UPGRADES: &str = "Upgrade R76P1\n Type = PLAYER\n BuildTime = 2\n BuildCost = 100\n DisplayName = UPGRADE:R76P1\n ResearchSound = R76Done\n \
                        UnitSpecificSound = R76Unit\nEnd\nUpgrade R76P2\n Type = PLAYER\n BuildTime = 0.5\n BuildCost = 0\nEnd\nUpgrade R76O1\n \
                        Type = OBJECT\n BuildTime = 1\n BuildCost = 50\n DisplayName = UPGRADE:R76O1\n ResearchSound = R76Other\nEnd\nUpgrade R76O2\n \
                        Type = OBJECT\n BuildTime = 0\n BuildCost = 30\nEnd\n";
const UPGRADE_NAMES: [&str; 4] = ["R76P1", "R76P2", "R76O1", "R76O2"];
const TEMPLATES: [&str; 4] = ["UnitA", "UnitB", "Dozer", "Tower"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ProductionUpdate(Arc::new(ProductionUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let nums: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let nq = r.u32();
    let qm: Vec<(String, u32)> = (0..nq).map(|_| (r.str(), r.u32())).collect();
    let disabled = r.u32();
    out.push(format!("data {nums:x?} {qm:?} {disabled:#x}"));
    let state = |r: &mut Reader| -> String {
        let head: Vec<u32> = (0..4 + 16).map(|_| r.u32()).collect();
        let mut flags = Vec::new();
        for _ in 0..2 {
            let mut f = Vec::new();
            loop {
                let x = r.u32();
                if x == u32::MAX {
                    break;
                }
                f.push(x);
            }
            flags.push(f);
        }
        let n = r.u32();
        let entries: Vec<u32> = (0..n * 9).map(|_| r.u32()).collect();
        let upgrades = (r.u32(), r.u32());
        format!("{head:x?} flags {flags:?} entries {entries:x?} upgrades {upgrades:x?}")
    };
    out.push(format!("constructed {}", state(&mut r)));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let s = state(&mut r);
        let made: Vec<u32> = (0..3).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        out.push(format!("op {i} state {s} made {made:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 70, "MaxQueueEntries", &["9", "3", "2", "5"], false);
    field(g, 60, "NumDoorAnimations", &["1", "2", "0"], false);
    field(g, 60, "DoorOpeningTime", &["100", "300", "0"], false);
    field(g, 60, "DoorWaitOpenTime", &["200", "500", "0"], false);
    field(g, 60, "DoorCloseTime", &["100", "300", "0"], false);
    field(g, 60, "ConstructionCompleteDuration", &["300", "1000", "0"], false);
    for _ in 0..g.below(3) {
        let v = *pick(g, &[&"UnitA 4", &"UnitB", &"UnitB 2", &"Nope 3", &"Dozer 0", &"UnitA -1"]);
        lines.push(format!("QuantityModifier = {v}"));
    }
    if g.below(100) < 20 {
        lines.push(format!("DisabledTypesToProcess = {}", pick(g, &["HELD", "HELD UNDERPOWERED", "EMP"])));
    }
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

/// The factory's word (cfg[2]).
fn gen_factory(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    let team = if g.below(20) == 0 { 0 } else { 1 + g.below(3) };
    let local = if g.below(6) == 0 { 3 } else { g.below(3) };
    let door = if g.below(3) == 0 { g.below(7) } else { g.below(2) };
    team | p(g, 4) << 2 | p(g, 4) << 3 | p(g, 85) << 4 | p(g, 50) << 5 | local << 6 | p(g, 30) << 8 | p(g, 92) << 9 | door << 10 | p(g, 25) << 13
        | p(g, 50) << 14
        | p(g, 60) << 15
}

/// A template's cost (low half) and build time (high half).
fn gen_cost(g: &mut Gen) -> u32 {
    let cost = *pick(g, &[&0i16, &100, &250, &-5]) as u16 as u32;
    let time = *pick(g, &[&0i16, &1, &3, &10, &30]) as u16 as u32;
    cost | time << 16
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        2 => gen_factory(g),
        3 => *pick(g, &[&0u32, &50, &100, &1000, &1000]),
        4 => {
            if g.below(5) == 0 {
                g.u32() & 0x3f
            } else {
                0x3f
            }
        }
        5 => {
            if g.below(4) == 0 {
                (0..4).fold(0, |v, i| v | (if g.below(3) == 0 { 1 + g.below(6) } else { 0 }) << (3 * i))
            } else {
                0
            }
        }
        6 => {
            if g.below(5) == 0 {
                g.u32() & 15
            } else {
                15
            }
        }
        7 => u32::from(g.below(2) == 0),
        8..=11 => gen_cost(g),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    cfg[1] = g.u32() & 15;
    for a in 2..=11u32 {
        cfg[a as usize] = gen_cfg_word(g, a);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    let mut next_id = 1u32;
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=34 => [0, 0, 0, 0, 0],
            35..=40 => {
                frame += 1 + g.below(20);
                [1, frame, 0, 0, 0]
            }
            41..=47 => {
                let a = *pick(g, &[&2u32, &2, &2, &3, &4, &5, &6, &7, &8, &9, &10, &11]);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            48..=61 => {
                let t = if g.below(40) == 0 { 4 } else { *pick(g, &[&0u32, &0, &1, &2, &3]) };
                let id = if g.below(10) == 0 { g.below(3) } else { next_id };
                next_id += 1;
                [3, t, id, 0, 0]
            }
            // (mostly the upgrades quick to build: R76P2 and R76O2)
            62..=69 => [4, if g.below(30) == 0 { 5 } else { *pick(g, &[&0u32, &1, &1, &1, &2, &3, &3, &3, &4]) }, 0, 0, 0],
            70..=73 => [5, if g.below(8) == 0 { 0 } else { 1 + g.below(next_id) }, 0, 0, 0],
            74..=76 => [6, g.below(5), 0, 0, 0],
            77..=78 => [7, g.below(4), 0, 0, 0],
            79 => [8, 0, 0, 0, 0],
            80 => [9, 0, 0, 0, 0],
            81..=83 => [10, (*pick(g, &[&0i32, &1, &2, &3, &-1, &5])) as u32, g.below(2), 0, 0],
            84..=86 => [11, g.below(5), g.below(6), 0, 0],
            87 => [12, 0, 0, 0, 0],
            88..=92 => [13, 0, 0, 0, 0],
            93..=97 => [14, 0, 0, 0, 0],
            _ => [15, 0, 0, 0, 0],
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
    units_queued: u64,
    upgrades_queued: u64,
    units_made: u64,
    upgrades_done: u64,
    refunds: u64,
    door_lines: u64,
    load_failures: u64,
    excluded: BTreeMap<String, u64>,
}

/// The probe of `name` on `id`.
fn probe_mut<'a>(w: &'a mut World, id: ObjectId, name: &[u8]) -> Option<&'a mut ProbeModule> {
    let idx = w.find_module(id, name)?;
    let o = w.objects.find_mut(id)?;
    match o.modules[idx as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => Some(p),
        _ => None,
    }
}

fn set_scene(w: &mut World, cfg: &[u32], drawable: rts_lockstep_engine::common::game_type::DrawableId) {
    let f = cfg[2];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.team = f & 3;
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_SOLD, (f >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 3 & 1) as i32);
        o.drawable = if f >> 4 & 1 != 0 { drawable } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
    }
    let mut flags = ModelConditionFlags::default();
    if f >> 5 & 1 != 0 {
        flags.set(MODELCONDITION_ACTIVELY_CONSTRUCTING, 1);
    }
    w.reads.drawable_model_condition.insert(1, flags);
    let local = f >> 6 & 3;
    w.players.friend_set_local_index((local < 3).then_some(local as i32));
    w.reads.local_selection = if f >> 8 & 1 != 0 && f >> 4 & 1 != 0 { vec![1] } else { Vec::new() };
    let door = [0, 1, 2, 3, -1, -2, 5, 0][(f >> 10 & 7) as usize];
    if let Some(p) = probe_mut(w, 1, b"R76Exit") {
        p.answers.exit_door = (f >> 9 & 1 != 0).then_some(door);
    }
    if let Some(p) = probe_mut(w, 1, b"R76Park") {
        let on = f >> 13 & 1 != 0;
        p.answers.has = if on { 1 << 6 } else { 0 };
        p.answers.parking = on.then_some((f >> 14 & 1 != 0, f >> 15 & 1 != 0));
    }
    for p in 0..3i32 {
        if let Some(pl) = w.players.get_nth_mut(p) {
            pl.money.init();
            let mut events = Vec::new();
            pl.money.deposit(cfg[3], false, &mut events);
        }
        w.cuts.player_build_permissions.insert(p, (cfg[4] >> (2 * p) & 1 != 0, cfg[4] >> (2 * p + 1) & 1 != 0));
    }
    let ids: Vec<u16> = TEMPLATES.iter().map(|n| w.statics.thing_factory.find_template(n.as_bytes()).unwrap().data.get_template_id()).collect();
    w.cuts.can_make_unit.clear();
    w.cuts.template_build_costs.clear();
    for (i, &id) in ids.iter().enumerate() {
        w.cuts.can_make_unit.insert(id, (cfg[5] >> (3 * i) & 7) as i32);
        let c = cfg[8 + i];
        w.cuts.template_build_costs.insert(id, ((c & 0xffff) as u16 as i16 as i32, (c >> 16) as u16 as i16 as i32));
    }
    w.cuts.can_produce_upgrades.clear();
    for (i, n) in UPGRADE_NAMES.iter().enumerate() {
        if cfg[6] >> i & 1 != 0 {
            if let Some(u) = upgrade_key(&w.statics.upgrade_center, n) {
                w.cuts.can_produce_upgrades.insert(u.0);
            }
        }
    }
    w.cuts.valid_audio_events.clear();
    if cfg[7] & 1 != 0 {
        w.cuts.valid_audio_events.insert(b"R76Done".to_vec());
    }
}

fn upgrade_key(center: &UpgradeCenter, name: &str) -> Option<NameKeyType> {
    center.templates().find(|t| t.upgrade_name() == name.as_bytes()).map(|t| t.upgrade_name_key())
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let events: Vec<Option<ObjectClientEvent>> = w
        .events
        .drain()
        .into_iter()
        .map(|e| match e {
            ClientEvent::Object(e) => Some(e),
            _ => None,
        })
        .collect();
    let mut event_iter = events.into_iter();
    let up_name = |w: &World, key: u32| -> String {
        w.statics.upgrade_center.find_upgrade_by_key(NameKeyType(key)).map_or("?".to_string(), |u| String::from_utf8_lossy(u.upgrade_name()).to_string())
    };
    let tmpl_name = |w: &World, id: u16| -> String {
        w.statics.thing_factory.find_by_template_id(id).map_or("(null)".to_string(), |t| String::from_utf8_lossy(t.data.get_name()).to_string())
    };
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::BuildAssistantCanMakeUnit { builder, template }) => log.push(format!("canmake {builder} {}", tmpl_name(w, template))),
                Some(CutCall::PlayerMoneyWithdraw { player, amount }) => log.push(format!("withdraw {player} {amount} 1")),
                Some(CutCall::PlayerMoneyDeposit { player, amount }) => {
                    tally.refunds += 1;
                    log.push(format!("deposit {player} {amount} 1"))
                }
                Some(CutCall::PlayerAddUpgrade { player, upgrade, status }) => log.push(format!("addupgrade {player} {} {status}", up_name(w, upgrade))),
                Some(CutCall::PlayerRemoveUpgrade { player, upgrade }) => log.push(format!("removeupgrade {player} {}", up_name(w, upgrade))),
                Some(CutCall::PlayerScoreAddMoneySpent { player, amount }) => {
                    tally.upgrades_done += 1;
                    log.push(format!("spent {player} {amount}"))
                }
                Some(CutCall::ScriptNotifyOfCompletedUpgrade { player, name, source }) => log.push(format!("scriptupgrade {player} {} {source}", String::from_utf8_lossy(&name))),
                Some(CutCall::PlayerAcademyRecordProduction { player, object, constructor }) => log.push(format!("recordproduction {player} {object} {constructor}")),
                Some(CutCall::PlayerAcademyRecordUpgrade { player, upgrade, granted }) => log.push(format!("recordupgrade {player} {} {}", up_name(w, upgrade), i32::from(granted))),
                Some(CutCall::PlayerOnUnitCreated { player, factory, unit }) => {
                    tally.units_made += 1;
                    log.push(format!("unitcreated {player} {factory} {unit}"))
                }
                // the World's own bookkeeping of a new object (the stand-ins make a bare object)
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::PartitionRegisterObject { .. }
                    | CutCall::GameLogicUpdateObjectsChangedTriggerAreas
                    | CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction
                    | CutCall::WeaponSetUpdate { .. }
                    | CutCall::PlayerBecomingTeamMember { .. },
                ) => {}
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            match event_iter.next().flatten() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name))),
                Some(ObjectClientEvent::DrawableModelCondition { object_id: 1, op: ModelConditionOp::ClearAndSet { clear, set } }) => {
                    // (the drawable's own call: batch R14b's line)
                    let mut l = "clearsetflags".to_string();
                    for f in &clear {
                        l.push_str(&format!(" {f}"));
                    }
                    l.push_str(" |");
                    for f in &set {
                        l.push_str(&format!(" {f}"));
                    }
                    log.push(l)
                }
                // (a new object's drawable starting its model condition, and its team: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { object_id, .. } | ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id > 5 => {}
                Some(ObjectClientEvent::UiMessageFormatted { format, arg }) => {
                    log.push(format!("uimessage {} {}", String::from_utf8_lossy(&format), String::from_utf8_lossy(&arg)))
                }
                Some(ObjectClientEvent::RadarCreateEvent { position: p, event_type }) => {
                    log.push(format!("radarevent {:08x} {:08x} {:08x} {event_type} {:08x}", b(p.x), b(p.y), b(p.z), b(4.0)))
                }
                Some(ObjectClientEvent::Eva { message: EvaMessage::UpgradeComplete }) => log.push("eva 18".to_string()),
                Some(ObjectClientEvent::ControlBarMarkUiDirty) => log.push("uidirty".to_string()),
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            let mut words: Vec<String> = l.split(' ').map(str::to_string).collect();
            if matches!(words[0].as_str(), "ppreserve" | "ppspace") {
                let id: u16 = words[2].parse().unwrap_or(0);
                words[2] = tmpl_name(w, id);
            }
            if matches!(words[0].as_str(), "reservedoor" | "exitdoor" | "unreservedoor") {
                tally.door_lines += 1;
            }
            log.push(words.join(" "));
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::ProductionUpdate(d) = &data else { unreachable!() };
    put_i32(&mut v, d.num_door_animations);
    for x in [d.door_opening_time, d.door_wait_open_time, d.door_closing_time, d.construction_complete_duration] {
        put_u32(&mut v, x);
    }
    put_i32(&mut v, d.max_queue_entries);
    put_u32(&mut v, d.quantity_modifiers.len() as u32);
    for q in &d.quantity_modifiers {
        put_str(&mut v, &q.template_name);
        put_i32(&mut v, q.quantity);
    }
    put_u32(&mut v, (0..DISABLED_COUNT).filter(|&k| d.disabled_types_to_process.test(k)).fold(0, |a, k| a | 1 << k));

    let mut w = World::new();
    w.names = ctx.names;
    let mut factory = ThingFactory::new();
    for (i, name) in TEMPLATES.iter().enumerate() {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let tm = &mut factory.friend_find_template_mut(name.as_bytes()).unwrap().data;
        match i {
            2 => tm.friend_set_kind_of(&[KINDOF_DOZER]),
            3 => tm.friend_set_kind_of(&[KINDOF_STRUCTURE]),
            _ => {}
        }
        tm.friend_set_audio(ThingTemplateAudioType::VoiceCreated, AudioEventName { event_name: format!("R76Voice{i}").into_bytes() });
        tm.friend_set_per_unit_sound(b"VoiceCreate", AudioEventName { event_name: b"R76VoiceCreate".to_vec() });
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let mut template = ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R76Factory".to_vec(), ..Default::default() });
    for (i, n) in UPGRADE_NAMES.iter().enumerate() {
        template.friend_set_upgrade_cameo_name(i + 1, n.as_bytes());
    }
    let mut trigger = UpgradeMaskType::new();
    for (i, n) in UPGRADE_NAMES.iter().enumerate() {
        if cfg[1] >> i & 1 != 0 {
            if let Some(u) = w.statics.upgrade_center.templates().find(|t| t.upgrade_name() == n.as_bytes()) {
                trigger.set_mask(&u.upgrade_mask());
            }
        }
    }
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32, trigger: UpgradeMaskType| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, trigger)));
        m
    };
    let modules = vec![
        rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ProductionUpdate", b"ModuleTag_Production", &data).unwrap().unwrap(),
        probe(b"R76Exit", ProbeAnswers::default(), 0, UpgradeMaskType::new()),
        probe(b"R76Park", ProbeAnswers::default(), 0, UpgradeMaskType::new()),
        probe(b"R76Upgrade", ProbeAnswers::default(), MODULEINTERFACE_UPGRADE, trigger),
    ];
    let me = w.new_object(NewObject { template: Arc::new(template), status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let drawable = w.objects.find(me).unwrap().drawable;
    for k in 2..=5 {
        let id = w.new_object(NewObject { template: w.objects.find(me).unwrap().template().clone(), status: Default::default(), team: None, modules: Vec::new() }).unwrap();
        assert_eq!(id, k);
    }
    let idx = w.find_module(me, b"ProductionUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, drawable);
    w.events.drain();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let ids: Vec<u16> = TEMPLATES.iter().map(|n| w.statics.thing_factory.find_template(n.as_bytes()).unwrap().data.get_template_id()).collect();
    let keys: Vec<Option<NameKeyType>> = UPGRADE_NAMES.iter().map(|n| upgrade_key(&w.statics.upgrade_center, n)).collect();
    let put_state = |v: &mut Vec<u8>, w: &World| {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::ProductionUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let s = m.state_words();
        // (the entries' template and upgrade as the driver's indices)
        let fixed = 4 + 16;
        let mut i = fixed;
        for _ in 0..2 {
            while s[i] != u32::MAX {
                i += 1;
            }
            i += 1;
        }
        for x in &s[..i] {
            put_u32(v, *x);
        }
        let n = s[i] as usize;
        put_u32(v, n as u32);
        for e in 0..n {
            let w9 = &s[i + 1 + 9 * e..i + 1 + 9 * e + 9];
            let t = ids.iter().position(|&id| id as u32 == w9[1]).map_or(u32::MAX, |p| p as u32);
            let up = if w9[2] == u32::MAX { u32::MAX } else { keys.iter().position(|k| k.map(|k| k.0) == Some(w9[2])).map_or(100, |p| p as u32) };
            let (t, up) = if w9[0] == 1 { (t, u32::MAX) } else { (u32::MAX, up) };
            for x in [w9[0], t, up, w9[3], w9[4], w9[5], w9[6], w9[7], w9[8]] {
                put_u32(v, x);
            }
        }
        // the upgrades: the factory's, each player's completed and in production; the upgrade module's flag
        let mut bits = 0u32;
        for (i, k) in keys.iter().enumerate() {
            let Some(k) = k else { continue };
            let mask = w.statics.upgrade_center.find_upgrade_by_key(*k).unwrap().upgrade_mask();
            if w.has_upgrade(me, &mask) {
                bits |= 1 << i;
            }
            for p in 0..3 {
                if w.player_has_upgrade_complete(p, &mask) {
                    bits |= 1 << (4 + 8 * p + i as i32);
                }
                if w.player_has_upgrade_in_production(p, &mask) {
                    bits |= 1 << (8 + 8 * p + i as i32);
                }
            }
        }
        put_u32(v, bits);
        let upgraded = w.find_module(me, b"R76Upgrade").and_then(|i| match w.objects.find(me).unwrap().modules[i as usize].body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => Some(p.upgraded),
            _ => None,
        });
        put_u32(v, u32::from(upgraded.unwrap_or(false)));
    };
    put_state(&mut v, &w);
    let tmpl = |a: u32| (a < 4).then(|| ids[a as usize]);
    let upg = |w: &World, a: u32| -> Option<u32> {
        match a {
            0..=3 => keys[a as usize].map(|k| k.0),
            4 => upgrade_key(&w.statics.upgrade_center, "Up0").map(|k| k.0),
            _ => None,
        }
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=12).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, drawable);
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                res = u32::from(call(&mut w, ModuleCall::ProductionQueueCreateUnit { template: tmpl(o[1]), production_id: o[2] as i32 }).as_bool());
                t.units_queued += u64::from(res);
            }
            4 => {
                let up = upg(&w, o[1]);
                res = u32::from(call(&mut w, ModuleCall::ProductionQueueUpgrade { upgrade: up }).as_bool());
                t.upgrades_queued += u64::from(res);
            }
            5 => {
                call(&mut w, ModuleCall::ProductionCancelUnitCreate { production_id: o[1] as i32 });
            }
            6 => {
                let up = upg(&w, o[1]);
                call(&mut w, ModuleCall::ProductionCancelUpgrade { upgrade: up });
            }
            7 => {
                call(&mut w, ModuleCall::ProductionCancelAllUnitsOfType { template: tmpl(o[1]) });
            }
            8 => {
                call(&mut w, ModuleCall::CancelAndRefundAllProduction);
            }
            9 => {
                call(&mut w, ModuleCall::OnDie(DamageInfo::default()));
            }
            10 => {
                call(&mut w, ModuleCall::SetHoldDoorOpen { door: o[1] as i32, hold: o[2] != 0 });
            }
            11 => {
                let up = upg(&w, o[2]);
                let a = call(&mut w, ModuleCall::ProductionCanQueueCreateUnit { template: ids[(o[1] % 4) as usize] }).as_int(0) as u32;
                let b2 = call(&mut w, ModuleCall::ProductionCanQueueUpgrade { upgrade: up }).as_int(0) as u32;
                let c = u32::from(call(&mut w, ModuleCall::ProductionIsUpgradeInQueue { upgrade: up }).as_bool());
                let n = call(&mut w, ModuleCall::ProductionCountUnitTypeInQueue { template: tmpl(o[1]) }).as_int(0) as u32;
                let count = call(&mut w, ModuleCall::ProductionGetCount).as_int(0) as u32;
                res = a | b2 << 4 | c << 8 | n << 9 | count << 16;
            }
            12 => {
                res = call(&mut w, ModuleCall::ProductionRequestUniqueUnitId).as_int(0) as u32;
            }
            13..=15 => {
                let r = run_xfer_cut(&mut w, me, idx as usize, o[0] - 13, &saved, XFER_FILE, 2048);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 13 {
                    saved = xbytes.clone();
                }
                if o[0] == 14 && xst != 0 {
                    t.load_failures += 1;
                }
                t.xfers += 1;
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().find(|(_, e)| !e.is_undefined());
        let st = match threw {
            Some((_, e)) if matches!(e.kind, IniErrorKind::BadArg) => 3,
            Some(_) => 2,
            None => 0,
        };
        if st != 0 {
            res = 0;
        }
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t);
        t.effects += log.len() as u64;
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
        put_state(&mut v, &w);
        let mut made: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 5).collect();
        made.sort_unstable();
        put_u32(&mut v, made.len() as u32);
        let last = made.last().and_then(|&id| w.objects.find(id));
        put_u32(&mut v, last.map_or(0, |o| o.producer_id as u32));
        put_u32(&mut v, last.map_or(0, |o| o.team));
    }
    drop(t);
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
fn production_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = UPGRADES.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R76 ProductionUpdate",
        seed: 0x0076_9F0D,
        cases_var: "L9R76_CASES",
        ini_stem: "Pr",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r76_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R76: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers ({} loads failed), {} effect lines \
         ({} units queued, {} upgrades queued, {} units made, {} upgrades done, {} refunds, {} door lines)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.load_failures, t.effects, t.units_queued, t.upgrades_queued, t.units_made,
        t.upgrades_done, t.refunds, t.door_lines
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.units_queued > 0 && t.upgrades_queued > 0 && t.units_made > 0 && t.upgrades_done > 0 && t.refunds > 0, "the generator must reach every effect");
}
