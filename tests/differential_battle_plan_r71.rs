#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BattlePlanUpdate` (`logic::modules::battle_plan_update`) against the unchanged C++, `p1w13-sonnet`, batch R71.
//!
//! # Method
//!
//! The C++ side is `BattlePlanUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_battle_plan_r71.inc`,
//! `ref_l9r71_run`; batch R50's `destroyObject` rename) on the module build's stand-ins: the strategy centre (a team of three with its
//! player, maybe an AI whose turret answers are the test's and whose orders are lines, maybe a drawable, maybe a body whose maximum health is
//! the test's and whose `setMaxHealth` is a line, maybe a REAL special power module of `SP_0`..`SP_3`, maybe a REAL `StealthDetectorUpdate`,
//! its vision and shroud clearing ranges) and four troops (kinds; the player's objects the test lists), the REAL special power store (the
//! four powers below), the player's `changeBattlePlan` and academy record, the radar, the message, the audio, the drawable,
//! `setDisabledUntil`, `setWeaponSetFlag` and `setWeaponLock` (lines), `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real module on a `World` with the five objects: the AI, the body and the special power module probes
//! (`ProbeAnswers::{cur_turret, turret_natural, trace_max_health, power_template}`), the REAL `StealthDetectorUpdate`, `cuts.player_objects`;
//! `changeBattlePlan` and the academy record are [`CutCall`] records, the disabling too (`cuts.body.stub_object_calls`).
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), `onObjectCreated`, then 1 to 60 ops, each an `update()`,
//! a choice of plan (`initiateIntentToDoSpecialPower` with a power and the command options), the queries (`getCommandOption`, `isActive`,
//! `getActiveBattlePlan`), `onDelete`, a change to the scene or the frame, or an xfer. Compared, bit for bit: the read's throw; the data; per
//! op the throw status, the result, the effects in order, the xfer status, bytes (the scheduling word masked) and CRC, the module's whole
//! state (the playing handles too), the centre's vision and shroud clearing ranges, its body's maximum health, and the
//! stealth detector's enabled and next call frame.
//!
//! # Results (2026-10-26, `L9R71_CASES=9000`)
//!
//! 9,000 cases (655 data blocks threw, as the C++ did; 1,497 excluded, each reading what upstream leaves undefined: 1,162 a plan chosen with
//! no special power module found, 156 a door moved with no drawable, 152 a plan chosen with no player, 27 a health change with no body),
//! 200,454 ops and 214,150 outputs: **0 mismatches**. In the compared cases: 70,345 updates, 12,155 plans chosen, 6,261 `changeBattlePlan`
//! calls, 1,042 troops paralysed, 4,736 turret calls, 823 health changes and 32,186 xfers. The first runs differed only in the test's set-up:
//! the C++ data read needs the real special power store bound to the INI's lookup (upgrade batch C's binding), the module's next call frame
//! is the game logic's registration (no longer compared), and the module's `findUpdateModule` key is a function-local static from the first
//! case's name key generator (the driver's hook answers the detector for any key).
//!
//! # Planted bugs
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2411 | no veteran weapon set | 5,583 |
//! | 2412 | option two chooses search and destroy | 295 |
//! | 2413 | no idle time after packing | 2,881 |
//! | 2414 | the pack sound gated by its own name | 7,285 |
//! | 2415 | the invalid kinds paralysed too | 212 |
//! | 2416 | no strategy centre health bonus | 165 |
//! | 2417 | no stealth detection while searching | 3,634 |
//! | 2418 | the ready frame ignored | 26,891 |
//! | 2419 | the turret never marked centring | 6,922 |
//! | 2420 | only the plans, the status and the ready frame saved | 18,576 |
//!
//! 10 of 10 caught.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::battle_plan_update::BattlePlanUpdateModuleData;
use rts_lockstep_engine::logic::modules::stealth_detector_update::StealthDetectorUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r71.bin");
const XFER_FILE: &str = "l9_r71.bin";
const POWERS: &str = "SpecialPower SP_0\nEnd\nSpecialPower SP_1\nEnd\nSpecialPower SP_2\nEnd\nSpecialPower SP_3\nEnd\n";
/// The troops' kinds, by bit (the driver's `g_r71Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_STRUCTURE];
/// The command options (`OPTION_ONE`, `OPTION_TWO`, `OPTION_THREE`) and others the generator picks from.
const OPTIONS: [u32; 7] = [0x2000, 0x4000, 0x8000, 0x6000, 0, 0x40000, 0x2000 | 0x40000];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::BattlePlanUpdate(Arc::new(BattlePlanUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The id of `SP_(k-1)` in the store, `None` for 0 (or a power the store does not hold).
fn power_id(store: &SpecialPowerStore, k: u32) -> Option<u32> {
    if k == 0 || k > 4 {
        return None;
    }
    store.find_special_power_template(format!("SP_{}", k - 1).as_bytes()).map(|t| t.id())
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let head: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let names: Vec<String> = (0..14).map(|_| r.str()).collect();
    let mut kinds = Vec::new();
    for _ in 0..2 {
        let mut k = Vec::new();
        loop {
            let x = r.u32();
            if x == u32::MAX {
                break;
            }
            k.push(x);
        }
        kinds.push(k);
    }
    let tail: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let created_status = r.u32() as i32;
    let n = r.u32();
    let created: Vec<String> = (0..n).map(|_| r.str()).collect();
    out.push(format!("data {head:x?} {names:?} {kinds:?} {tail:x?} created {created_status} {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..25).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, pct: u32, name: &str, vals: &[&str]| {
        if g.below(100) < pct {
            lines.push(format!("{name} = {}", pick(g, vals)));
        }
    };
    field(g, 95, "SpecialPowerTemplate", &["SP_0", "SP_1", "SP_0", "SP_2", "SP_1", "SP_3", "SP_0", "SP_2", "None", "NoSuchPower"]);
    for f in ["BombardmentPlanAnimationTime", "HoldTheLinePlanAnimationTime", "SearchAndDestroyPlanAnimationTime"] {
        field(g, 80, f, &["0", "100", "500", "2000"]);
    }
    field(g, 70, "TransitionIdleTime", &["0", "100", "1000"]);
    for f in ["BombardmentPlanUnpackSoundName", "BombardmentPlanPackSoundName", "BombardmentAnnouncementName", "SearchAndDestroyPlanUnpackSoundName",
        "SearchAndDestroyPlanIdleLoopSoundName", "SearchAndDestroyPlanPackSoundName", "SearchAndDestroyAnnouncementName", "HoldTheLinePlanUnpackSoundName",
        "HoldTheLinePlanPackSoundName", "HoldTheLineAnnouncementName"] {
        field(g, 60, f, &["SndA", "SndB", "SndC"]);
    }
    for f in ["BombardmentMessageLabel", "SearchAndDestroyMessageLabel", "HoldTheLineMessageLabel"] {
        field(g, 60, f, &["GUI:One", "GUI:Two"]);
    }
    field(g, 70, "ValidMemberKindOf", &["INFANTRY", "INFANTRY VEHICLE", "VEHICLE", "STRUCTURE"]);
    field(g, 40, "InvalidMemberKindOf", &["STRUCTURE", "VEHICLE"]);
    field(g, 70, "BattlePlanChangeParalyzeTime", &["0", "100", "3000"]);
    field(g, 60, "HoldTheLinePlanArmorDamageScalar", &["1.0", "0.9", "0.5"]);
    field(g, 60, "SearchAndDestroyPlanSightRangeScalar", &["1.0", "1.2", "2"]);
    field(g, 60, "StrategyCenterSearchAndDestroySightRangeScalar", &["1.0", "1.5", "2"]);
    field(g, 50, "StrategyCenterSearchAndDestroyDetectsStealth", &["Yes", "No"]);
    field(g, 60, "StrategyCenterHoldTheLineMaxHealthScalar", &["1.0", "2.0", "0", "1.5"]);
    field(g, 50, "StrategyCenterHoldTheLineMaxHealthChangeType", &["SAME_CURRENTHEALTH", "PRESERVE_RATIO", "ADD_CURRENT_HEALTH_TOO"]);
    field(g, 30, "VisionObjectName", &["VisionA"]);
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "TransitionIdleTime = soon", "ValidMemberKindOf = NOT_A_KIND"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    match a {
        2 => {
            // (no team, no drawable and no body are rare: each is read unchecked somewhere upstream)
            let team = if g.below(30) == 0 { 0 } else { 1 + g.below(3) };
            let power = if g.below(10) == 0 { 0 } else { 1 + g.below(4) };
            team | p(g, 85) << 2 | p(g, 96) << 3 | p(g, 96) << 4 | power << 8 | p(g, 60) << 11 | p(g, 30) << 12
        }
        3 => g.below(4) | p(g, 50) << 2,
        4..=7 => g.below(8) | p(g, 80) << 3,
        8..=10 => (*pick(g, &[&100.0f32, &250.5, &0.0, &400.0])).to_bits(),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &100]);
    cfg[1] = g.u32();
    for (a, c) in cfg.iter_mut().enumerate().take(11).skip(2) {
        *c = gen_cfg_word(g, a as u32);
    }
    // (the centre's special power module mostly the data's power: otherwise every choice of plan reads through NULL; with no power named,
    // plans are seldom chosen, for the same reason)
    let mut named = None;
    if let Some(ModuleDataKind::BattlePlanUpdate(d)) = data {
        named = d.special_power_template.as_deref().and_then(|n| (0..4u32).find(|k| n == format!("SP_{k}").as_bytes()));
        if let Some(k) = named {
            if g.below(10) != 0 {
                cfg[2] = cfg[2] & !(7 << 8) | (k + 1) << 8;
            }
        }
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    let power = cfg[2] >> 8 & 7;
    while ops.len() < n_ops {
        let o = match g.below(100) {
            // (mostly time passing in large steps, and updates)
            0..=24 => {
                frame = frame.wrapping_add(*pick(g, &[&1u32, &10, &50, &100]));
                [1, frame, 0, 0, 0]
            }
            25..=59 => [0, 0, 0, 0, 0],
            60..=71 if named.is_none() && g.below(8) != 0 => [8, 0, 0, 0, 0],
            60..=71 => [3, if g.below(6) == 0 { g.below(5) } else { power }, OPTIONS[g.below(OPTIONS.len() as u32) as usize], 0, 0],
            72..=77 => {
                let a = 3 + g.below(5);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            78..=81 => [8, 0, 0, 0, 0],
            82..=83 => [7, 0, 0, 0, 0],
            84..=89 => [4, 0, 0, 0, 0],
            90..=95 => [5, 0, 0, 0, 0],
            _ => [6, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    choices: u64,
    plans: u64,
    paralyses: u64,
    turrets: u64,
    health: u64,
    excluded: BTreeMap<String, u64>,
}

/// The slots of the centre's probes.
struct Slots {
    ai: u16,
    body: u16,
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeModule {
    match w.objects.find_mut(id).unwrap().modules[slot as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => p,
        _ => unreachable!("a probe"),
    }
}

fn set_scene(w: &mut World, cfg: &[u32], slots: &Slots) {
    let me: ObjectId = 1;
    {
        let o = w.objects.find_mut(me).unwrap();
        o.ai = if cfg[2] >> 2 & 1 != 0 { Some(slots.ai) } else { None };
        o.drawable = if cfg[2] >> 3 & 1 != 0 { 1001 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
        o.body = if cfg[2] >> 4 & 1 != 0 { Some(slots.body) } else { None };
    }
    {
        let p = probe_mut(w, me, slots.ai);
        let tu = cfg[3] & 3;
        p.answers.cur_turret = match tu {
            1 => 0,
            2 => 1,
            _ => -1,
        };
        p.answers.turret_natural = cfg[3] >> 2 & 1 != 0;
    }
    let mut troops = Vec::new();
    for k in 1..5usize {
        let id = k as ObjectId + 1;
        let f = cfg[3 + k];
        let mut values = ObjectTemplateValues { name: b"R71Thing".to_vec(), ..Default::default() };
        for (b, kind) in KINDS.iter().enumerate() {
            if f >> b & 1 != 0 {
                values.kind_of.push(*kind);
            }
        }
        w.objects.find_mut(id).unwrap().thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
        if f >> 3 & 1 != 0 {
            troops.push(id);
        }
    }
    let t = cfg[2] & 3;
    w.cuts.player_objects.clear();
    if t != 0 {
        w.cuts.player_objects.insert((t - 1) as i32, troops);
    }
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
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::PlayerChangeBattlePlan { player, plan, delta, armor_scalar, bombardment, search_and_destroy, hold_the_line, sight_range_scalar }) => {
                    tally.plans += 1;
                    log.push(format!(
                        "battleplan {player} {plan} {delta} {:08x} {bombardment} {search_and_destroy} {hold_the_line} {:08x}",
                        b(armor_scalar),
                        b(sight_range_scalar)
                    ))
                }
                Some(CutCall::AcademyStatsRecordBattlePlanSelected { player }) => {
                    tally.choices += 1;
                    log.push(format!("academyplan {player}"))
                }
                Some(CutCall::WeaponSetUpdate { .. }) => log.push("wset".to_string()),
                Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => log.push(format!("weaponlock {object} {slot} {lock}")),
                Some(CutCall::Body(BodyCall::SetDisabledUntil { object, disabled_type, frame })) => {
                    tally.paralyses += 1;
                    log.push(format!("disableduntil {object} {disabled_type} {frame}"))
                }
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => log.push(format!("destroy {object}")),
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if t.starts_with("evt ") {
            match event_iter.next().flatten() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                    let pos = position.map_or("-".to_string(), |p| format!("{},{},{}", b(p.x), b(p.y), b(p.z)));
                    log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)))
                }
                Some(ObjectClientEvent::AudioRemoveEvent { handle }) => log.push(format!("audioremove {handle}")),
                Some(ObjectClientEvent::UiMessage { label }) => log.push(format!("uimessage {}", String::from_utf8_lossy(&label))),
                Some(ObjectClientEvent::RadarCreatePlayerEvent { player, position: p, event_type }) => {
                    log.push(format!("radarplayer {} {:08x} {:08x} {:08x} {event_type}", player.unwrap_or(-1), b(p.x), b(p.y), b(p.z)))
                }
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(v), .. }) => log.push(format!("mc {}", v[0])),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(v), .. }) => log.push(format!("mcclear {}", v[0])),
                Some(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id, frames }) => log.push(format!("animloop {object_id} {frames}")),
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if let Some(rest) = l.strip_prefix("aiidlefrom ") {
                log.push(format!("aiidle {}", rest.split(' ').next().unwrap_or("")));
            } else {
                if l.starts_with("turret") {
                    tally.turrets += 1;
                }
                if l.starts_with("setmaxhealth ") {
                    tally.health += 1;
                }
                log.push(l.to_string());
            }
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::BattlePlanUpdate(d) = &data else { unreachable!() };
    let code = d.special_power_template.as_deref().and_then(|n| ctx.store.find_special_power_template(n)).map_or(0, |tp| {
        (1..=4).find(|&k| power_id(&ctx.store, k) == Some(tp.id())).unwrap_or(0)
    });
    put_u32(&mut v, code);
    for f in [d.bombardment_plan_animation_frames, d.hold_the_line_plan_animation_frames, d.search_and_destroy_plan_animation_frames, d.transition_idle_frames, d.battle_plan_paralyze_frames] {
        put_u32(&mut v, f);
    }
    for s in [
        &d.bombardment_unpack_name,
        &d.bombardment_pack_name,
        &d.bombardment_message_label,
        &d.bombardment_announcement_name,
        &d.search_and_destroy_unpack_name,
        &d.search_and_destroy_idle_name,
        &d.search_and_destroy_pack_name,
        &d.search_and_destroy_message_label,
        &d.search_and_destroy_announcement_name,
        &d.hold_the_line_unpack_name,
        &d.hold_the_line_pack_name,
        &d.hold_the_line_message_label,
        &d.hold_the_line_announcement_name,
        &d.vision_object_name,
    ] {
        put_str(&mut v, s);
    }
    for m in [&d.valid_member_kind_of, &d.invalid_member_kind_of] {
        for k in 0..KINDOF_COUNT {
            if m.test(k) {
                put_u32(&mut v, k as u32);
            }
        }
        put_u32(&mut v, u32::MAX);
    }
    for x in [d.hold_the_line_armor_damage_scalar, d.search_and_destroy_sight_range_scalar, d.strategy_center_search_and_destroy_sight_range_scalar, d.strategy_center_hold_the_line_max_health_scalar] {
        put_u32(&mut v, x.to_bits());
    }
    put_u32(&mut v, u32::from(d.strategy_center_search_and_destroy_detects_stealth));
    put_u32(&mut v, d.strategy_center_hold_the_line_max_health_change_type as u32);

    let mut w = World::new();
    w.names = ctx.names;
    let power = power_id(&ctx.store, cfg[2] >> 8 & 7);
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let template = || Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R71Thing".to_vec(), ..Default::default() }));
    let probe = |name: &[u8], answers: ProbeAnswers, max: f32| {
        let mut m = NewModule::plain(name, name);
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        m
    };
    for k in 1..=5 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BattlePlanUpdate", b"ModuleTag_Plan", &data).unwrap().unwrap());
            if let Some(id) = power {
                let mut p = probe(b"R71Power", ProbeAnswers { power_template: id, ..Default::default() }, 0.0);
                p.interfaces = MODULEINTERFACE_SPECIAL_POWER;
                modules.push(p);
            }
            if cfg[2] >> 11 & 1 != 0 {
                let sd = StealthDetectorUpdateModuleData { initially_disabled: cfg[2] >> 12 & 1 != 0, ..StealthDetectorUpdateModuleData::friend_new_module_data(None).unwrap() };
                let sd = ModuleDataKind::StealthDetectorUpdate(Arc::new(sd));
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"StealthDetectorUpdate", b"ModuleTag_SD", &sd).unwrap().unwrap());
            }
            let mut ai = probe(b"R71AIUpdate", ProbeAnswers::default(), 0.0);
            ai.is_ai = true;
            modules.push(ai);
            let mut body = probe(b"R71Body", ProbeAnswers { trace_max_health: true, no_death: true, ..Default::default() }, f32::from_bits(cfg[10]));
            body.interfaces = MODULEINTERFACE_BODY;
            modules.push(body);
        }
        let team = if k == 1 && cfg[2] & 3 != 0 { Some(cfg[2] & 3) } else { None };
        let id = w.new_object(NewObject { template: template(), status: Default::default(), team, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    w.set_vision_range(me, f32::from_bits(cfg[8]));
    w.objects.find_mut(me).unwrap().shroud_clearing_range = f32::from_bits(cfg[9]);
    let slots = Slots { ai: w.find_module(me, b"R71AIUpdate").unwrap(), body: w.find_module(me, b"R71Body").unwrap() };
    let idx = w.find_module(me, b"BattlePlanUpdate").unwrap();
    let sd = w.find_module(me, b"StealthDetectorUpdate");
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let sdu = sd.map(|sd| w.update_handle_of_module(ModuleHandle { object: me, index: sd }).expect("an update module"));
    set_scene(&mut w, &cfg, &slots);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.events.drain();
    let errors_before = w.cuts.module_errors.len();
    w.module_call(me, idx, ModuleCall::OnObjectCreated);
    put_i32(&mut v, if w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined()) { 2 } else { 0 });
    let created = effects(&mut w, &mut t);
    put_u32(&mut v, created.len() as u32);
    for l in &created {
        put_str(&mut v, l.as_bytes());
    }
    let next_frame = |w: &World, u: rts_lockstep_engine::logic::world::sleepy::UpdateHandle| {
        w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame()
    };
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::BattlePlanUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let mut s = m.state_words();
        s.push(ob.vision_range.to_bits());
        s.push(ob.shroud_clearing_range.to_bits());
        let max = match ob.modules[slots.body as usize].body.as_ref() {
            Some(UpdateBody::ModuleProbe(p)) => p.max_health,
            _ => 0.0,
        };
        s.push(max.to_bits());
        match sdu {
            Some(sdu) => {
                let Some(UpdateBody::StealthDetectorUpdate(sdm)) = ob.behaviors[sdu.index as usize].body.as_ref() else { panic!("the detector is in its slot") };
                s.push(u32::from(sdm.is_sd_enabled()));
                s.push(next_frame(w, sdu));
            }
            None => {
                s.push(2);
                s.push(0);
            }
        }
        s
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (3..=7).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        w.events.drain();
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let template = power_id(&w.statics.special_power_store.clone(), o[1]).unwrap_or(0);
                let call = ModuleCall::InitiateIntentToDoSpecialPower { template, target_object: 0, target_position: None, waypoint: 0, command_options: o[2] };
                res = u32::from(w.module_call(me, idx, call).as_bool());
            }
            4..=6 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 4, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                if o[0] == 5 {
                    // the driver runs the module's loadPostProcess (the base's), not GameLogic's: back in the sleepy list at the frame read
                    let loaded = next_frame(&w, u);
                    let sd_frame = sdu.map(|s| next_frame(&w, s));
                    w.load_post_process();
                    w.objects.find_mut(me).unwrap().behaviors[u.index as usize].update.friend_set_next_call_frame(loaded);
                    if let (Some(s), Some(f)) = (sdu, sd_frame) {
                        w.objects.find_mut(me).unwrap().behaviors[s.index as usize].update.friend_set_next_call_frame(f);
                    }
                }
                t.xfers += 1;
            }
            7 => {
                w.module_call(me, idx, ModuleCall::OnDelete);
            }
            8 => {
                let int = |r: ModuleReply| match r {
                    ModuleReply::Int(i) => i as u32,
                    _ => 0,
                };
                let opt = int(w.module_call(me, idx, ModuleCall::SpecialPowerUpdateGetCommandOption));
                let active = w.module_call(me, idx, ModuleCall::SpecialPowerUpdateIsActive).as_bool();
                let plan = int(w.module_call(me, idx, ModuleCall::BattlePlanGetActiveBattlePlan));
                res = opt | if active { 0x100 } else { 0 } | plan << 12;
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t);
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
        for s in state(&w) {
            put_u32(&mut v, s);
        }
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
fn battle_plan_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R71 BattlePlanUpdate",
        seed: 0x0071_BA71,
        cases_var: "L9R71_CASES",
        ini_stem: "Bp",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r71_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R71: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} plans chosen, {} changeBattlePlan calls, \
         {} paralyses, {} turret calls, {} health changes, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.choices, t.plans, t.paralyses, t.turrets, t.health, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.choices > 0 && t.plans > 0 && t.paralyses > 0 && t.turrets > 0 && t.health > 0, "the generator must reach every effect");
}
