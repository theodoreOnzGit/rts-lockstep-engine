#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CommandButtonHuntUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CommandButtonHuntUpdate` (`logic::modules::command_button_hunt_update`) against the unchanged C++, `p1w13-sonnet`, batch R69.
//!
//! # Method
//!
//! The C++ side is `CommandButtonHuntUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_command_button_hunt_r69.inc`,
//! `ref_l9r69_run`) on the module build's stand-ins: six objects (the hunter and five others; a team of three, each with its player,
//! disabled or not), the teams' relationships, the hunter's command set (the override name the test gives), two command sets of up to 18
//! of four buttons (`ControlBar.h`'s `GUICommandType` verbatim; a name, a type, a weapon slot, a special power template), the hunter's AI
//! (idle, the last command's source, an attack priority info whose table is the test's, `getPriority` ScriptEngine.cpp's on it), its
//! special ability update (a power type, active), maybe a REAL special power module of `SP_0`..`SP_3` (the REAL special power store, the
//! four powers below), the scan (the objects the test lists; the call a line with its filters' count and the relationship filter's flags),
//! the squared distances (the test's), `TheActionManager`'s four tests (lines answered by the test's bits), `doCommandButtonAtObject` and
//! `setWeaponLock` (lines), the AI data's distance modifier, the REAL `Object::getRelationship`, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real module on a `World` with the six objects: the AI, the special ability update and the special power module
//! probes (`ProbeAnswers::{idle, last_command_source, attack_priority, special_power_type, power_template}`), `cuts.command_buttons`,
//! `cuts.command_sets`, `cuts.action_allowed`, `cuts.range_scan`, `cuts.body.distance_squared` and `cuts.attack_priority_distance_modifier`.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 60 ops, each an `update()`, a `setCommandButton`
//! (one of the buttons' names, the empty name or a name no button has), a change to the scene or the frame, or an xfer. Compared, bit for
//! bit: the read's throw; the data; the constructor's next call frame; per op the throw status, the result (the sleep), the effects in
//! order (the AI's idle and hunt, the weapon lock, the scan, the action manager's tests, the distances, the button's use), the xfer
//! status, bytes (the scheduling word masked) and CRC, the button's name, the button and the module's next call frame.
//!
//! # Results (2026-10-26, `L9R69_CASES=9000`)
//!
//! 9,000 cases (703 data blocks threw, as the C++ did; 3 excluded, each a score conversion to `Int` out of range, which upstream leaves
//! undefined), 251,186 ops and 267,774 outputs: **0 mismatches**. In the compared cases: 122,233 updates, 35,958 button sets, 16,692 scans,
//! 10,230 uses of a button at a target, 10,894 hunts, 16,067 weapon locks and 34,169 xfers. The first full runs differed only in the test's
//! own set-up: after a load, the Rust side ran `World::load_post_process` (GameLogic's, which moves a next call frame of 0 to now) where the
//! driver runs only the module's; and without it the load leaves the module out of the sleepy list, so a later wake was refused. The test
//! now puts the module back at the frame the load read.
//!
//! # Planted bugs
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2390 | the button's name not saved | 29,612 |
//! | 2391 | `setCommandButton` takes a button with an empty name | 4,021 |
//! | 2392 | a command not from the AI does not end the hunt | 20,106 |
//! | 2393 | the weapon slot not locked | 16,067 |
//! | 2394 | an active special ability does not hold the scan | 170 |
//! | 2395 | the car bomb scans for enemies, not neutrals | 5,596 |
//! | 2396 | the building capture keeps the relationship filter | 200 |
//! | 2397 | the vehicle hack targets disabled vehicles | 23 |
//! | 2398 | a target of priority 0 is scored | 147 |
//! | 2399 | a tie is not broken by the higher priority | 58 |
//!
//! 10 of 10 caught.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::disabled_types::DISABLED_HELD;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::special_power_type::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::command_button_hunt_update::CommandButtonHuntUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::{CommandButtonFacts, CutCall};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r69.bin");
const XFER_FILE: &str = "l9_r69.bin";
/// The four special powers (`SP_0`..`SP_3`): the vehicle hack, the building capture, the timed charges and another.
const POWERS: &str = "SpecialPower SP_0\n Enum = SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK\nEnd\nSpecialPower SP_1\n Enum = SPECIAL_INFANTRY_CAPTURE_BUILDING\n \
ViewObjectRange = 150\nEnd\nSpecialPower SP_2\n Enum = SPECIAL_TIMED_CHARGES\nEnd\nSpecialPower SP_3\n Enum = SPECIAL_HACKER_DISABLE_BUILDING\nEnd\n";
/// The command set names, by index (the driver's `g_r69SetNames`).
const SET_NAMES: [&str; 4] = ["", "SetA", "SetB", "SetC"];
/// The button names, by index (the driver's `g_r69ButtonNames`).
const BUTTON_NAMES: [&str; 5] = ["", "BtnA", "BtnB", "BtnC", "BtnX"];
/// The command types, by index (the driver's `g_r69Types`: none, fire weapon, special power, switch weapon, hijack, car bomb, sabotage,
/// guard).
const TYPES: [i32; 8] = [0, 20, 21, 26, 27, 28, 29, 9];
/// The special ability update's power types, by index (0: none).
const ABILITY_TYPES: [i32; 6] = [-1, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK, SPECIAL_INFANTRY_CAPTURE_BUILDING, SPECIAL_TIMED_CHARGES, SPECIAL_TANKHUNTER_TNT_ATTACK, SPECIAL_HACKER_DISABLE_BUILDING];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::CommandButtonHuntUpdate(Arc::new(CommandButtonHuntUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
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
    let data: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    let next = r.u32();
    out.push(format!("data {data:x?} next {next}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let name = r.str();
        let (button, next) = (r.u32(), r.u32());
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} name {name:?} button {button} next {next}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 80 {
        lines.push(format!("ScanRate = {}", pick(g, &["0", "33", "100", "1000"])));
    }
    if g.below(100) < 80 {
        let r = if g.below(40) == 0 { "3e9" } else { pick(g, &["9999", "100", "500.5", "0", "250"]) };
        lines.push(format!("ScanRange = {r}"));
    }
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "ScanRate = soon", "ScanRange = far"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_object(g: &mut Gen, me: bool) -> u32 {
    let team = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
    let disabled = u32::from(g.below(100) < if me { 10 } else { 30 });
    let set = if me { if g.below(8) == 0 { *pick(g, &[&0u32, &3]) } else { 1 + g.below(2) } } else { 0 };
    team | disabled << 2 | set << 4
}

fn gen_scan(g: &mut Gen) -> u32 {
    let n = g.below(6);
    let mut w = 0u32;
    for k in 0..n {
        w |= (1 + g.below(6)) << (4 * k);
    }
    w
}

fn gen_slots(g: &mut Gen, n: u32) -> u32 {
    (0..n).fold(0u32, |w, j| w | (if g.below(100) < 30 { 1 + g.below(4) } else { 0 }) << (3 * j))
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        2..=7 => gen_object(g, a == 2),
        // (mostly every team the hunter's enemy)
        8 => (0..9).fold(0u32, |m, i| m | (if g.below(3) == 0 { g.below(3) } else { 0 }) << (2 * i)),
        9 => gen_scan(g),
        10 => {
            let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
            let source = if g.below(100) < 80 { 2 } else { *pick(g, &[&0u32, &1, &3]) };
            let default = *pick(g, &[&0i32, &1, &5, &-3, &100]) as u8 as u32;
            p(g, 90) | p(g, 70) << 1 | source << 2 | p(g, 40) << 4 | default << 8
        }
        11 => (0..6).fold(0u32, |m, k| m | (if g.below(2) == 0 { 15 } else { g.below(15) }) << (4 * k)),
        12 => g.below(6) | u32::from(g.below(100) < 30) << 3,
        13 => g.below(5),
        14 => g.u32() & 0x00ff_ffff,
        15 => (*pick(g, &[&1.0f32, &50.0, &0.5, &200.0, &1.0])).to_bits() | if g.below(60) == 0 { 0xffff_ffff } else { 0 },
        16..=21 => (*pick(g, &[&0.0f32, &100.0, &2500.0, &10_000.0, &1.0e6, &1.0e8, &30.25])).to_bits(),
        23 | 25 => gen_slots(g, 10),
        24 | 26 => gen_slots(g, 8),
        27..=30 => {
            let name = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
            let ty = if g.below(8) == 0 { *pick(g, &[&0u32, &7]) } else { 1 + g.below(6) };
            let power = if g.below(5) == 0 { 0 } else { 1 + g.below(4) };
            name | ty << 2 | g.below(3) << 5 | power << 7
        }
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 32];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &100]);
    cfg[1] = g.u32();
    for (a, c) in cfg.iter_mut().enumerate().take(31).skip(2) {
        *c = gen_cfg_word(g, a as u32);
    }
    // (a zero distance modifier: the bits above give a NaN rarely; make a plain zero just as rare)
    if cfg[15] == 0xffff_ffff {
        cfg[15] = 0;
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    // most cases set a button early
    if g.below(10) < 8 {
        ops.push([3, 1 + g.below(3), 0, 0, 0]);
    }
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=49 => [0, 0, 0, 0, 0],
            50..=53 => [1, gen_frame(g), 0, 0, 0],
            54..=73 => {
                let mut a = 2 + g.below(29);
                if a == 13 {
                    a = 12;
                }
                let mut v = gen_cfg_word(g, a);
                if a == 15 && v == 0xffff_ffff {
                    v = 0;
                }
                [2, a, v, 0, 0]
            }
            74..=85 => [3, if g.below(6) == 0 { *pick(g, &[&0u32, &4]) } else { 1 + g.below(3) }, 0, 0, 0],
            86..=91 => [4, 0, 0, 0, 0],
            92..=97 => [5, 0, 0, 0, 0],
            _ => [6, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    ops.truncate(n_ops.max(1));
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    sets: u64,
    xfers: u64,
    scans: u64,
    uses: u64,
    hunts: u64,
    locks: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

/// The slots of the module's object: the AI, the special ability update and the special power module probes.
struct Slots {
    ai: u16,
    ability: u16,
    power: Option<u16>,
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeModule {
    match w.objects.find_mut(id).unwrap().modules[slot as usize].body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => p,
        _ => unreachable!("a probe"),
    }
}

fn set_scene(w: &mut World, cfg: &[u32], slots: &Slots) {
    let me: ObjectId = 1;
    for k in 0..6usize {
        let id = k as ObjectId + 1;
        let f = cfg[2 + k];
        let o = w.objects.find_mut(id).unwrap();
        o.team = f & 3;
        o.disabled.set(DISABLED_HELD, i32::from(f >> 2 & 1 != 0));
        w.cuts.body.distance_squared.insert((me, id), f32::from_bits(cfg[16 + k]));
    }
    w.objects.find_mut(me).unwrap().command_set_string_override = SET_NAMES[(cfg[2] >> 4 & 3) as usize].as_bytes().to_vec();
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[8] >> (2 * (3 * a + b)) & 3));
        }
    }
    let mut scan = Vec::new();
    for k in 0..6 {
        let id = cfg[9] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 6 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
    let a = cfg[10];
    w.objects.find_mut(me).unwrap().ai = if a & 1 != 0 { Some(slots.ai) } else { None };
    {
        let p = probe_mut(w, me, slots.ai);
        p.answers.idle = a >> 1 & 1 != 0;
        p.answers.last_command_source = (a >> 2 & 3) as i32;
        p.answers.attack_priority = if a >> 4 & 1 != 0 {
            let list = (0..6).filter_map(|k| {
                let pr = cfg[11] >> (4 * k) & 15;
                (pr != 15).then_some((k as ObjectId + 1, pr as i32))
            });
            Some(((a >> 8 & 0xff) as u8 as i8 as i32, list.collect()))
        } else {
            None
        };
    }
    {
        let s = (cfg[12] & 7) as usize;
        let p = probe_mut(w, me, slots.ability);
        p.answers.has = if (1..=5).contains(&s) { 1 << 9 } else { 0 };
        p.answers.special_power_type = if (1..=5).contains(&s) { ABILITY_TYPES[s] } else { 0 };
        p.answers.yes = cfg[12] >> 3 & 1 != 0;
    }
    w.cuts.action_allowed = (0..4).flat_map(|kind| (0..6).map(move |k| (kind, k))).filter(|(kind, k)| cfg[14] >> (6 * kind + k) & 1 != 0).map(|(kind, k)| (kind, k as ObjectId + 1)).collect();
    w.cuts.attack_priority_distance_modifier = f32::from_bits(cfg[15]);
    let store = w.statics.special_power_store.clone();
    w.cuts.command_buttons = (0..4)
        .map(|i| {
            let f = cfg[27 + i];
            CommandButtonFacts {
                name: BUTTON_NAMES[(f & 3) as usize].as_bytes().to_vec(),
                command_type: TYPES[(f >> 2 & 7) as usize],
                weapon_slot: (f >> 5 & 3) as i32,
                special_power: power_id(&store, f >> 7 & 7),
            }
        })
        .collect();
    w.cuts.command_sets = (0..2)
        .map(|i| {
            let slots: Vec<Option<usize>> = (0..18)
                .map(|j| {
                    let word = cfg[23 + 2 * i + usize::from(j >= 10)];
                    let b = word >> (3 * (j % 10)) & 7;
                    (1..=4).contains(&b).then(|| b as usize - 1)
                })
                .collect();
            (SET_NAMES[i + 1].as_bytes().to_vec(), slots)
        })
        .collect();
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::PartitionIterateHuntTargets { object, range, relationship, team_filter }) => {
                    tally.scans += 1;
                    let p = w.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
                    let (n, r) = if team_filter { (4, relationship) } else { (3, -1) };
                    log.push(format!("huntscan {:08x} {:08x} {:08x} {:08x} {n} {r} 0", b(p.x), b(p.y), b(p.z), b(range)));
                }
                Some(CutCall::Body(BodyCall::GetDistanceSquared { object, other })) => log.push(format!("distsq {object} {other} 2")),
                Some(CutCall::ActionManagerCan { kind, object, other, source, template }) => log.push(format!("can {kind} {object} {other} {source} {template}")),
                Some(CutCall::ObjectDoCommandButtonAtObject { object, button, victim, source }) => {
                    tally.uses += 1;
                    log.push(format!("cmdbutton {object} {} {victim} {source}", String::from_utf8_lossy(&button)))
                }
                Some(CutCall::ObjectSetWeaponLock { object, slot, lock }) => {
                    tally.locks += 1;
                    log.push(format!("weaponlock {object} {slot} {lock}"))
                }
                Some(c) => log.push(format!("unexpected {c:?}")),
                None => {}
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if let Some(rest) = l.strip_prefix("aiidlefrom ") {
                log.push(format!("aiidle {}", rest.split(' ').next().unwrap_or("")));
            } else {
                if l.starts_with("aihunt ") {
                    tally.hunts += 1;
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
    let ModuleDataKind::CommandButtonHuntUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.scan_frames);
    put_u32(&mut v, d.scan_range.to_bits());

    let mut w = World::new();
    w.names = ctx.names;
    let power = power_id(&ctx.store, cfg[13]);
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let template = || Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R69Thing".to_vec(), ..Default::default() }));
    let probe = |name: &[u8]| {
        let mut m = NewModule::plain(name, name);
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        m
    };
    let mut slots = Slots { ai: 0, ability: 0, power: None };
    for k in 1..=6 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"CommandButtonHuntUpdate", b"ModuleTag_Hunt", &data).unwrap().unwrap());
            let mut ai = probe(b"R69AIUpdate");
            ai.is_ai = true;
            modules.push(ai);
            modules.push(probe(b"SpecialAbilityUpdate"));
            if let Some(id) = power {
                let mut p = probe(b"R69Power");
                p.interfaces = MODULEINTERFACE_SPECIAL_POWER;
                if let Some(UpdateBody::ModuleProbe(pm)) = p.body.as_mut() {
                    pm.answers.power_template = id;
                }
                modules.push(p);
            }
        }
        let id = w.new_object(NewObject { template: template(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"CommandButtonHuntUpdate").unwrap();
    slots.ai = w.find_module(me, b"R69AIUpdate").unwrap();
    slots.ability = w.find_module(me, b"SpecialAbilityUpdate").unwrap();
    slots.power = power.and_then(|_| w.find_module(me, b"R69Power"));
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let next_frame = |w: &World| w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame();
    put_u32(&mut v, next_frame(&w));
    let state = |w: &World| -> (Vec<u8>, u32) {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::CommandButtonHuntUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state()
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=30).contains(&o[1]) && o[1] != 13 {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let name = BUTTON_NAMES[(o[1] % 5) as usize].as_bytes().to_vec();
                w.module_call(me, idx, ModuleCall::CommandButtonHuntSetCommandButton(name));
                t.sets += 1;
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
                    // the driver runs the module's loadPostProcess (the base's: nothing here), not GameLogic's: the load leaves the
                    // module out of the sleepy list (`m_indexInLogic = -1`), so it goes back in, at the next call frame the load read
                    // (`World::load_post_process` would move a frame 0 to now)
                    let loaded = next_frame(&w);
                    w.load_post_process();
                    w.objects.find_mut(me).unwrap().behaviors[u.index as usize].update.friend_set_next_call_frame(loaded);
                }
                t.xfers += 1;
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
        let (name, button) = state(&w);
        put_str(&mut v, &name);
        put_u32(&mut v, button);
        put_u32(&mut v, next_frame(&w));
    }
    let _ = slots.power;
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
fn command_button_hunt_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R69 CommandButtonHuntUpdate",
        seed: 0x0069_C0B4,
        cases_var: "L9R69_CASES",
        ini_stem: "Ch",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r69_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R69: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} button sets, {} scans, {} button uses, \
         {} hunts, {} weapon locks, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.sets, t.scans, t.uses, t.hunts, t.locks, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.uses > 0 && t.scans > 0 && t.locks > 0 && t.hunts > 0, "the generator must reach every effect");
}
