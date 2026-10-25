#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BunkerBusterBehavior` (`logic::modules::bunker_buster_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R54.
//!
//! # Method
//!
//! The C++ side is `BunkerBusterBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_bunker_buster_r54.inc`,
//! `ref_l9r54_run`), on the module build's stand-ins: four objects (ids 1-4, at (10 k, 20 k, k)); the bomb (object 1: maybe a player of
//! three, maybe `MISSILE_KILLING_SELF`, maybe an AI whose current victim is the test's); the targets (objects 2-4: maybe a contain module,
//! bustable or not, whose `killAllContained` and `harmAndForceExitAllContained` are lines); the players' completed upgrades; the REAL upgrade
//! center with the harness's upgrades; the weapon store (`WEAP_A`, `WEAP_B` with the test's damage and death types; `createAndFireTempWeapon`
//! a line); the FX lists (`FX_A`, `FX_B`; `doFXObj` a line naming its object); and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` AI on the bomb and
//! a `ProbeModule` contain on each target), three teams and players, the upgrade center, `cuts.player_upgrades` and the weapon store; every
//! op an `update()` as the scheduler runs it, a death, `onObjectCreated`, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: 1 to 8 upgrades, a block `D` (mostly well formed), 1 to 60 ops. Compared, bit for bit: the
//! read's throw; the data (the upgrade's name, the two FX lists, the frequency, the seismic numbers, the two weapons); `onObjectCreated`'s
//! state; per op the throw status, the result, the effects in order (the FX and their objects, the kills, the harms with their damage info,
//! the shockwave), the xfer status, bytes (the scheduling word masked) and CRC, and the state (the victim, whether the upgrade was found).
//!
//! # Results (2026-10-25, `L9R54_CASES=9000`)
//!
//! 9,000 cases (276 data blocks threw, as the C++ did; 871 excluded, undefined upstream: 611 a smart bomb whose
//! `CrashThroughBunkerFXFrequency` is 0, which `update` divides the frame by; 260 a bomb with no controlling player whose upgrade or occupant
//! damage dereferences it), 237,297 ops and 253,003 outputs: **0 mismatches**, on the first run. In the compared cases: 81,484 updates,
//! 20,212 deaths, 27,123 xfers and 38,197 effect lines: 3,648 crash-through FX, 9,493 detonation FX, 2,961 occupants killed, 2,173
//! harmed and forced out, 6,781 shockwaves.
//!
//! The FX and weapon fields take only names the module build's stand-ins know (`FX_A`, `FX_B`; `WEAP_A`, `WEAP_B`, `None` and unknown
//! names, which both stores answer with `NULL`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught. (The first 2249, which dropped the occupant weapon, also skipped
//! the controlling player's dereference: the C++ crashed on the first case the Rust side no longer excluded, ending the run at 26
//! mismatches; it was replaced by a value change.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2241 | the crash FX on frames where the frequency divides the frame, not one after | 10,221 |
//! | 2242 | the crash FX while not killing itself | 3,646 |
//! | 2243 | the bunker busted only without the upgrade | 3,743 |
//! | 2244 | the victim read on every update, not once | 15,566 |
//! | 2245 | the detonation FX on the bomb, not the victim | 4,960 |
//! | 2246 | a container that is not bustable busted | 2,140 |
//! | 2247 | the occupants harmed for 50, not 100 | 2,173 |
//! | 2248 | the shockwave fired from the bomb, not the victim | 3,547 |
//! | 2249 | the harm's damage and death types swapped | 2,117 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_MISSILE_KILLING_SELF;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::bunker_buster_behavior::BunkerBusterBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r54.bin");
const XFER_FILE: &str = "l9_r54.bin";
const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::BunkerBusterBehavior(Arc::new(BunkerBusterBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The driver's weapon code: 0 `NULL`, 1 `WEAP_A`, 2 `WEAP_B`.
fn weapon_code(name: &Option<Vec<u8>>) -> i32 {
    name.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |i| i as i32 + 1)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let upgrade = r.str();
    let data: Vec<u32> = (0..7).map(|_| r.u32()).collect();
    let created: Vec<u32> = (0..2).map(|_| r.u32()).collect();
    out.push(format!("data {upgrade:?} {data:x?} created {created:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..2).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:?}"));
    }
    out
}

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut ups: Vec<&str> = upgrades.iter().map(|s| s.as_str()).collect();
    ups.extend(["NoSuchUpgrade", ""]);
    // (`strict`: only the choices; the FX fields take only names the module build's FX store knows, its stand-in answering any other name,
    // which upstream's does not)
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 45, "UpgradeRequired", &ups, true);
    field(g, 75, "DetonationFX", &["FX_A", "FX_B", "None"], true);
    field(g, 75, "CrashThroughBunkerFX", &["FX_A", "FX_B", "None"], true);
    field(g, 70, "CrashThroughBunkerFXFrequency", &["33", "66", "100", "133", "300", "33", "100", "0"], false);
    field(g, 30, "SeismicEffectRadius", &["10", "140.5", "0"], false);
    field(g, 30, "SeismicEffectMagnitude", &["2", "6.25"], false);
    field(g, 70, "ShockwaveWeaponTemplate", &["WEAP_A", "WEAP_B", "None", "NoSuchWeapon"], true);
    field(g, 60, "OccupantDamageWeaponTemplate", &["WEAP_A", "WEAP_B", "None", "weap_b"], true);
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

/// The bomb's word: an AI (mostly), killing itself (half the time), its victim (mostly a target).
fn gen_bomb(g: &mut Gen) -> u32 {
    let ai = u32::from(g.below(100) < 80);
    let killing = u32::from(g.below(2) == 0) << 1;
    let victim = *pick(g, &[&2u32, &3, &4, &2, &3, &0, &1, &7]);
    ai | killing | victim << 4
}

/// The targets' nibbles: a contain module (mostly), bustable (mostly).
fn gen_targets(g: &mut Gen) -> u32 {
    (0..3).fold(0, |acc, k| acc | (u32::from(g.below(100) < 75) | u32::from(g.below(100) < 70) << 1) << (4 * k))
}

fn gen_players(g: &mut Gen, n_up: usize) -> u32 {
    let m = (1u32 << n_up) - 1;
    (0..3).fold(0, |acc, p| acc | (if g.below(2) == 0 { m } else { g.below(256) & m }) << (8 * p))
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 8];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100, &299]);
    cfg[1] = if g.below(12) == 0 { 0 } else { 1 + g.below(3) };
    cfg[2] = gen_players(g, n_up);
    cfg[3] = gen_bomb(g);
    cfg[4] = gen_targets(g);
    cfg[5] = (0..4).fold(0, |acc, k| acc | g.below(if k < 2 { 37 } else { 12 }) << (8 * k));
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=39 => {
                ops.push([2, 0, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            40..=47 => [0, 0, 0, 0, 0],
            48..=51 => [1, gen_frame(g), 0, 0, 0],
            52..=63 => [3, 0, 0, 0, 0],
            64..=79 => {
                let a = 1 + g.below(4);
                let b = match a {
                    1 => if g.below(12) == 0 { 0 } else { 1 + g.below(3) },
                    2 => gen_players(g, n_up),
                    3 => gen_bomb(g),
                    _ => gen_targets(g),
                };
                [4, a, b, 0, 0]
            }
            80..=85 => [5, 0, 0, 0, 0],
            86..=91 => [6, 0, 0, 0, 0],
            92..=95 => [7, 0, 0, 0, 0],
            _ => [8, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    deaths: u64,
    xfers: u64,
    effects: u64,
    crash_fx: u64,
    detonations: u64,
    kills: u64,
    harms: u64,
    shockwaves: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

/// The upgrade masks of `Up<k>` for each bit `k` of `bits`.
fn upgrade_mask(w: &mut World, bits: u32) -> UpgradeMaskType {
    let statics = w.statics.clone();
    let mut m = UpgradeMaskType::default();
    for k in 0..8 {
        if bits >> k & 1 != 0 {
            if let Some(u) = statics.upgrade_center.find_upgrade(&mut w.names, format!("Up{k}").as_bytes()).unwrap() {
                m.set_mask(&u.upgrade_mask());
            }
        }
    }
    m
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::BunkerBusterBehavior(d) = &data else { unreachable!() };
    put_str(&mut v, &d.upgrade_required);
    put_i32(&mut v, fx.id(&d.detonation_fx));
    put_i32(&mut v, fx.id(&d.crash_through_bunker_fx));
    put_u32(&mut v, d.crash_through_bunker_fx_frequency);
    put_u32(&mut v, d.seismic_effect_radius.to_bits());
    put_u32(&mut v, d.seismic_effect_magnitude.to_bits());
    put_i32(&mut v, weapon_code(&d.shockwave_weapon_template));
    put_i32(&mut v, weapon_code(&d.occupant_damage_weapon_template));

    let mut w = World::new();
    w.names = ctx.names;
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for (k, n) in WEAPONS.iter().enumerate() {
        let i = weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap().unwrap();
        let t = &mut weapon_store.templates_mut()[i];
        t.damage_type = (cfg[5] >> (8 * k) & 255) as i32;
        t.death_type = (cfg[5] >> (16 + 8 * k) & 255) as i32;
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let probe = |name: &[u8], answers: ProbeAnswers| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    for k in 1..=4u32 {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BunkerBusterBehavior", b"ModuleTag_Buster", &data).unwrap().unwrap());
            modules.push(probe(b"R54Ai", ProbeAnswers::default()));
        } else {
            modules.push(probe(b"R54Contain", ProbeAnswers { trace_kill_contained: true, trace_bust: true, ..Default::default() }));
        }
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId);
        w.set_position(id, &Coord3D { x: 10.0 * k as f32, y: 20.0 * k as f32, z: k as f32 });
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"BunkerBusterBehavior").unwrap();
    let ai_slot = w.find_module(me, b"R54Ai").unwrap();
    let contain_slot = w.find_module(2, b"R54Contain").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let scene = |w: &mut World, cfg: &[u32]| {
        let tp = cfg[1];
        w.objects.find_mut(me).unwrap().team = if (1..=3).contains(&tp) { tp } else { 0 };
        for p in 0..3 {
            let m = upgrade_mask(w, cfg[2] >> (8 * p) & 255);
            w.cuts.player_upgrades.insert(p, m);
        }
        let b = cfg[3];
        let o = w.objects.find_mut(me).unwrap();
        o.ai = (b & 1 != 0).then_some(ai_slot);
        o.status.set(OBJECT_STATUS_MISSILE_KILLING_SELF, (b >> 1 & 1) as i32);
        let victim = b >> 4 & 15;
        if let Some(UpdateBody::ModuleProbe(p)) = o.modules[ai_slot as usize].body.as_mut() {
            p.answers.current_victim = if (1..=4).contains(&victim) { victim as ObjectId } else { INVALID_ID };
        }
        for k in 2..=4 {
            let f = cfg[4] >> (4 * (k - 2)) & 15;
            let o = w.objects.find_mut(k).unwrap();
            o.contain = (f & 1 != 0).then_some(contain_slot);
            if let Some(UpdateBody::ModuleProbe(p)) = o.modules[contain_slot as usize].body.as_mut() {
                p.answers.bustable = f >> 1 & 1 != 0;
            }
        }
    };
    scene(&mut w, &cfg);
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::BunkerBusterBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    for s in state(&w) {
        put_u32(&mut v, s);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 2 {
            let fr = w.frame().wrapping_add(1);
            w.set_frame(fr);
        }
        if o[0] == 4 && (1..=4).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
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
            3 => {
                w.module_call(me, idx, ModuleCall::OnDie(DamageInfo::default()));
                tally.borrow_mut().deaths += 1;
            }
            5..=7 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 5 {
                    saved = xbytes.clone();
                }
                if o[0] == 6 {
                    w.load_post_process();
                }
                tally.borrow_mut().xfers += 1;
            }
            8 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
            }
            1 | 2 | 4 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
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
                if let Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) = cut_iter.next() {
                    t.shockwaves += 1;
                    log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), p.x.to_bits(), p.y.to_bits(), p.z.to_bits()));
                }
            } else if line.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx: name, object_id }) = ev_iter.next() {
                    if o[0] == 0 {
                        t.crash_fx += 1;
                    } else {
                        t.detonations += 1;
                    }
                    log.push(format!("fx {}", fx.id(&Some(name))));
                    log.push(format!("fxobj {object_id}"));
                }
            } else if let Some(l) = line.strip_prefix("bt ") {
                if l.starts_with("killall ") {
                    t.kills += 1;
                    log.push(l.to_string());
                } else if l.starts_with("harmall ") {
                    t.harms += 1;
                    log.push(l.to_string());
                }
            }
        }
        t.effects += log.len() as u64;
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
        for s in state(&w) {
            put_u32(&mut v, s);
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
fn bunker_buster_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R54 BunkerBusterBehavior",
        seed: 0x0054_B0B0,
        cases_var: "L9R54_CASES",
        ini_stem: "Bb",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r54_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R54: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} deaths, {} xfers, {} effect lines \
         ({} crash FX, {} detonation FX, {} kills, {} harms, {} shockwaves)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.deaths, t.xfers, t.effects, t.crash_fx, t.detonations, t.kills, t.harms, t.shockwaves
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.crash_fx > 0 && t.detonations > 0 && t.kills > 0 && t.harms > 0 && t.shockwaves > 0, "the generator must reach every effect");
}
