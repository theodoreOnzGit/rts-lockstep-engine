#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDamagedBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FireWeaponWhenDamagedBehavior` (`logic::modules::fire_weapon_when_damaged_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R65.
//!
//! # Method
//!
//! The C++ side is `FireWeaponWhenDamagedBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit
//! (`differential/l9_fire_weapon_damaged_r65.inc`, `ref_l9r65_run`), on the module build's stand-ins: the object (maybe with a body, a REAL
//! `InactiveBody` whose damage state the test gives), the weapon store of batches R62 and R64 (`WEAP_A`, `WEAP_B`), the stand-in `Weapon`
//! running `Weapon.cpp`'s own constructor, `crc`, `xfer`, `loadPostProcess` and `getStatus` and `Weapon.h`'s setters, whose `reloadAmmo`
//! and `forceFireWeapon` are lines; the game logic's wake calls applied as `GameLogic::friend_awakenUpdateModule` applies them; the REAL
//! `UpdateModule`, `UpgradeMux`, `UpgradeCenter`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the object (its body a probe answering the damage state) and the weapon store's two
//! templates; the module from `modules::new_module`, its weapons made by the world half of its constructor and their status the real
//! `Weapon::get_status`; the weapons' calls are records (`cuts.stub_weapon_calls`). As in batch R64 the ops set the weapons' state through
//! upstream's setters.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the eight weapons, the damage types and amount, `StartsActive` and the mux's
//! fields; mostly well formed) and runs 1 to 60 ops: an `update()`, a frame change, an upgrade or its reset, a hit (a damage type, the
//! damage dealt and the amount asked), a damage state, a weapon setter, or an xfer. Compared, bit for bit: the read's throw; the data; the
//! constructor's effects (the reloads), the next call frame it left and the weapons made; per op the throw status, the result,
//! `isAlreadyUpgraded`, the effects in order (the shots with their position and template, the mux's FX and removals), the wake calls, the
//! xfer status, bytes (every weapon's own included; the scheduling word masked) and CRC.
//!
//! # Results (2026-10-25, `L9R65_CASES=9000`)
//!
//! 9,000 cases (222 data blocks threw, as the C++ did; 389 excluded: 147 a hit or an update on an object with no body, which upstream reads
//! through `NULL`; 86 an upgrade that names no upgrade template; 156 a constructor that throws, `StartsActive` removing an upgrade the
//! center does not have, as batch M's test excludes them), 255,736 ops and 314,459 outputs: **0 mismatches**. In the compared cases:
//! 28,116 weapons reloaded by the constructors, 39,734 updates, 42,914 hits, 1,126 shots and 17,896 xfers. The first runs differed only in
//! the test's own set-up: the driver had not run `initDamageTypeFlags()` (which `GameEngine::init` runs before any INI is read, so
//! `DAMAGE_TYPE_FLAGS_ALL` read zero), and a save of eight weapons is longer than the 512 bytes the shared helper loads (the driver's
//! buffer and [`run_xfer_cut`]'s are 1,024 here).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2351 | `BODY_DAMAGED` taken as pristine | 647 |
//! | 2352 | a weapon fired without asking whether it is ready | 11,321 |
//! | 2353 | the damage type not tested | 161 |
//! | 2354 | the amount asked, not the damage dealt, compared | 110 |
//! | 2355 | the upgrade does not wake the module | 1,049 |
//! | 2356 | the weapons not reloaded by the constructor | 8,300 |
//! | 2357 | the continuous rubble weapon not saved | 3,415 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DamageInfo, DAMAGE_NUM_TYPES, DAMAGE_TYPE_NAMES};
use rts_lockstep_engine::logic::modules::fire_weapon_when_damaged_behavior::FireWeaponWhenDamagedBehaviorModuleData;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::weapon_types::WeaponStatus;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r65.bin");
const XFER_FILE: &str = "l9_r65.bin";

const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The damage types the cases use (by index), and their names in `DamageTypes`.
const TYPES: [usize; 5] = [0, 1, 2, 5, 6];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FireWeaponWhenDamagedBehavior(Arc::new(FireWeaponWhenDamagedBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn weapon_code(name: &Option<Vec<u8>>) -> u32 {
    name.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |k| k as u32 + 1)
}

fn put_data(v: &mut Vec<u8>, d: &FireWeaponWhenDamagedBehaviorModuleData, fx: &mut FxIds) {
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_u32(v, u32::from(d.initially_active));
    let mut w = [0u32; 2];
    for i in 0..DAMAGE_NUM_TYPES {
        if d.damage_types.test(i as i32) {
            w[i / 32] |= 1 << (i % 32);
        }
    }
    put_u32(v, w[0]);
    put_u32(v, w[1]);
    put_u32(v, d.damage_amount.to_bits());
    for n in &d.weapons {
        put_u32(v, weapon_code(n));
    }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    r.mux(&mut out);
    let words: Vec<u32> = (0..12).map(|_| r.u32()).collect();
    let n = r.u32();
    let ctor: Vec<String> = (0..n).map(|_| r.str()).collect();
    let (next, made) = (r.u32(), r.u32());
    out.push(format!("data {words:x?} constructor {ctor:?} next {next} made {made:#x}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res, up) = (r.u32() as i32, r.u32(), r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let x = r.xfer();
        out.push(format!("op {i}: status {st} result {res:#x} upgraded {up} log {log:?} wakes {wakes} last {last} {x}"));
    }
    out
}

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    for f in rts_lockstep_engine::logic::modules::fire_weapon_when_damaged_behavior::WEAPON_FIELDS {
        if g.below(100) < 45 {
            let name = if g.below(15) == 0 { pick(g, &["None", "NoSuchWeapon"]).to_string() } else { pick(g, &["WEAP_A", "WEAP_A", "WEAP_B", "weap_b"]).to_string() };
            lines.push(format!("{f} = {name}"));
        }
    }
    if g.below(100) < 60 {
        lines.push(format!("StartsActive = {}", pick(g, &["Yes", "Yes", "No"])));
    }
    if g.below(100) < 50 {
        let t = |g: &mut Gen| DAMAGE_TYPE_NAMES[*pick(g, &TYPES.iter().collect::<Vec<_>>())];
        let v = match g.below(4) {
            0 => "ALL".to_string(),
            1 => format!("NONE +{} +{}", t(g), t(g)),
            2 => format!("ALL -{}", t(g)),
            _ => format!("NONE +{}", t(g)),
        };
        lines.push(format!("DamageTypes = {v}"));
    }
    if g.below(100) < 50 {
        lines.push(format!("DamageAmount = {}", pick(g, &["0", "10", "25.5", "100"])));
    }
    let mut mux_lines = Vec::new();
    gen_mux_lines(g, upgrades, clean, &mut mux_lines);
    lines.extend(mux_lines);
    if !clean && g.below(20) == 0 {
        lines.push(pick(g, &["Bogus = 1", "DamageTypes = NOT_A_TYPE", "DamageAmount = lots"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_small_frame(g: &mut Gen) -> u32 {
    match g.below(10) {
        0..=6 => g.below(200),
        7..=8 => g.below(3000),
        _ => *pick(g, &[&0u32, &0xFFFF_FFFF, &0x8000_0000]),
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 12];
    cfg[0] = *pick(g, &[&0u32, &0, &5, &299]);
    cfg[1] = u32::from(g.below(30) != 0);
    cfg[2] = g.below(4);
    for k in 0..2 {
        cfg[3 + 4 * k] = (*pick(g, &[&-std::f32::consts::PI, &-std::f32::consts::PI, &-0.5f32, &0.0])).to_bits();
        cfg[4 + 4 * k] = (*pick(g, &[&std::f32::consts::PI, &std::f32::consts::PI, &0.5f32, &1.0])).to_bits();
        cfg[5 + 4 * k] = *pick(g, &[&1u32, &1, &2, &5]);
        cfg[6 + 4 * k] = *pick(g, &[&0u32, &0, &15, &100]);
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut ops: Vec<[u32; 5]> = Vec::new();
    let all = |n_up: usize| [if n_up >= 32 { u32::MAX } else { (1u32 << n_up) - 1 }, 0, 0, 0];
    // most cases upgrade early, so that the module runs
    if g.below(10) < 7 {
        let m = if g.below(2) == 0 { gen_mask(g, n_up, 1) } else { all(n_up) };
        ops.push([6, m[0], m[1], m[2], m[3]]);
    }
    let amount = |g: &mut Gen| (*pick(g, &[&0.0f32, &5.0, &10.0, &30.0, &150.0])).to_bits();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=19 => {
                ops.push([2, 0, 0, 0, 0]);
                [0, 0, 0, 0, 0]
            }
            20..=24 => [1, gen_small_frame(g), 0, 0, 0],
            25..=28 => [3, 0, 0, 0, 0],
            29..=31 => [4, 0, 0, 0, 0],
            32..=33 => [5, 0, 0, 0, 0],
            34..=38 => {
                let m = if g.below(2) == 0 { gen_mask(g, n_up, 1) } else { all(n_up) };
                [6, m[0], m[1], m[2], m[3]]
            }
            39..=40 => {
                let m = gen_mask(g, n_up, 1);
                [7, m[0], m[1], m[2], m[3]]
            }
            41..=62 => [8, *pick(g, &TYPES.iter().collect::<Vec<_>>()) as u32, amount(g), amount(g), 0],
            63..=70 => [9, g.below(4), 0, 0, 0],
            71..=80 => {
                // (mostly a weapon made ready: READY_TO_FIRE, which getStatus keeps while the weapon cannot fire again yet)
                let k = g.below(8);
                if g.below(3) != 0 {
                    ops.push([10, k, 0, 0, 0]);
                    [11, k, *pick(g, &[&0x7FFF_FFFFu32, &0x7FFF_FFFF, &5000]), 0, 0]
                } else {
                    [10, k, g.below(5), 0, 0]
                }
            }
            81..=92 => [11, g.below(8), gen_small_frame(g), 0, 0],
            _ => [12, g.below(8), if g.below(2) == 0 { 0 } else { gen_small_frame(g) }, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    hits: u64,
    shots: u64,
    reloads: u64,
    xfers: u64,
    effects: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn status_of(v: u32) -> WeaponStatus {
    match v {
        1 => WeaponStatus::OutOfAmmo,
        2 => WeaponStatus::BetweenFiringShots,
        3 => WeaponStatus::ReloadingClip,
        4 => WeaponStatus::PreAttack,
        _ => WeaponStatus::ReadyToFire,
    }
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    let ob = w.objects.find_mut(1).unwrap();
    if let Some(b) = ob.body {
        if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[b as usize].body.as_mut() {
            p.answers.damage_state = Some(cfg[2] as i32);
        }
    }
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::FireWeaponWhenDamagedBehavior(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);

    let mut w = World::new();
    w.names = ctx.names;
    let mut store = WeaponStore::new();
    for (k, name) in WEAPONS.iter().enumerate() {
        let i = store.new_weapon_template(&mut w.names, name.as_bytes()).unwrap().unwrap();
        let tp = &mut store.templates_mut()[i];
        tp.min_target_pitch = f32::from_bits(cfg[3 + 4 * k]);
        tp.max_target_pitch = f32::from_bits(cfg[4 + 4 * k]);
        tp.shots_per_barrel = cfg[5 + 4 * k] as i32;
        tp.suspend_fx_delay = cfg[6 + 4 * k];
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, weapon_store: store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.stub_weapon_calls = true;
    w.cuts.probe_effect_trace = true;
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let mut modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FireWeaponWhenDamagedBehavior", b"ModuleTag_Fire", &data).unwrap().unwrap()];
    if cfg[1] & 1 != 0 {
        let mut b = NewModule::plain(b"InactiveBody", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { damage_state: Some(cfg[2] as i32), ..Default::default() }, 0.0, Default::default())));
        modules.push(b);
    }
    // made under construction, as batch M's test does: `initObject` runs the upgrade pass of a finished object, which the C++ driver does not
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let me: ObjectId = w.new_object(NewObject { template, status, team: None, modules }).unwrap();
    assert_eq!(me, 1);
    w.objects.find_mut(me).unwrap().status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
    // a constructor that throws (`giveSelfUpgrade`'s removal of an upgrade the center does not have) is not compared: the harness compares a
    // made module (batch M's test excludes such cases the same way)
    if let Some((_, e)) = w.cuts.module_errors.first() {
        let reason = if e.is_undefined() { format!("{e}") } else { format!("a constructor that throws ({e})") };
        let mut t = tally.borrow_mut();
        *t = tally_before;
        *t.excluded.entry(reason).or_insert(0) += 1;
        return RunOut { dump: v, keys: Vec::new(), excluded: true };
    }
    let idx = w.find_module(me, b"FireWeaponWhenDamagedBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    w.set_position(me, &Coord3D { x: 11.0, y: 22.0, z: 5.0 });
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    // the effects of a stretch of the trace, and the module's wake calls in it
    let take = |w: &mut World, events_before: usize, fx: &mut FxIds, t: &mut Tally| -> (Vec<String>, u32, u32) {
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        for line in w.cuts.trace.take().unwrap() {
            if line.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::WeaponReloadAmmo { source, template }) => {
                        t.reloads += 1;
                        log.push(format!("reloadammo {source} {}", WEAPONS[template]))
                    }
                    Some(CutCall::WeaponForceFireAtPosition { source, pos, template }) => {
                        t.shots += 1;
                        log.push(format!("forcefire {source} {:08x} {:08x} {:08x} {}", pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), WEAPONS[template]))
                    }
                    _ => {}
                }
            } else if line.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) = ev_iter.next() {
                    log.push(format!("fx{}", fx.id(&Some(name))));
                }
            } else if let Some(fr) = line.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = fr;
            } else if let Some(n) = line.strip_prefix("remove ") {
                log.push(format!("remove {n}"));
            }
        }
        (log, wakes, last)
    };
    let (ctor, _, _) = take(&mut w, 0, &mut fx, &mut tally.borrow_mut());
    put_u32(&mut v, ctor.len() as u32);
    for l in &ctor {
        put_str(&mut v, l.as_bytes());
    }
    put_u32(&mut v, w.objects.find(me).unwrap().behaviors[u.index as usize].update.friend_get_next_call_frame());
    let made = {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::FireWeaponWhenDamagedBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is in its slot") };
        m.state_words()[1]
    };
    put_u32(&mut v, made);
    set_scene(&mut w, &cfg);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        if o[0] == 2 {
            let fr = w.frame().wrapping_add(1);
            w.set_frame(fr);
        }
        if o[0] == 9 {
            cfg[2] = o[1];
        }
        set_scene(&mut w, &cfg);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let mask = mask_from(&o[1..]);
        let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.borrow_mut().updates += 1;
            }
            1 | 2 | 9 => {}
            3..=5 => {
                // (the driver's load buffer is 1,024 bytes: eight weapons do not fit in 512)
                let r = run_xfer_cut(&mut w, me, idx as usize, o[0] - 3, &saved, XFER_FILE, 1024);
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
            6 => res = u32::from(call(&mut w, ModuleCall::AttemptUpgrade(mask)).as_bool()),
            7 => res = u32::from(call(&mut w, ModuleCall::ResetUpgrade(mask)).as_bool()),
            8 => {
                let mut info = DamageInfo::default();
                info.input.damage_type = o[1] as i32;
                info.out.actual_damage_dealt = f32::from_bits(o[2]);
                info.input.amount = f32::from_bits(o[3]);
                call(&mut w, ModuleCall::OnDamage(info));
                tally.borrow_mut().hits += 1;
            }
            10..=12 => {
                let ob = w.objects.find_mut(me).unwrap();
                let Some(UpdateBody::FireWeaponWhenDamagedBehavior(m)) = ob.behaviors[u.index as usize].body.as_mut() else { panic!("the module is in its slot") };
                if let Some(weapon) = m.weapon_mut(o[1] as usize) {
                    match o[0] {
                        10 => weapon.set_status(status_of(o[2])),
                        11 => weapon.set_possible_next_shot_frame(o[2]),
                        _ => weapon.set_pre_attack_finished_frame(o[2]),
                    }
                }
            }
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        put_u32(&mut v, u32::from(call(&mut w, ModuleCall::IsAlreadyUpgraded).as_bool()));
        let mut t = tally.borrow_mut();
        let (log, wakes, last) = take(&mut w, events_before, &mut fx, &mut t);
        t.effects += log.len() as u64;
        drop(t);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
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
fn fire_weapon_when_damaged_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R65 FireWeaponWhenDamagedBehavior",
        seed: 0x0065_F1DA,
        cases_var: "L9R65_CASES",
        ini_stem: "Fd",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r65_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R65: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} hits, {} shots, {} reloads, {} xfers, {} effect lines",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.hits, t.shots, t.reloads, t.xfers, t.effects
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.shots > 0 && t.reloads > 0 && t.hits > 0, "the generator must reach every effect");
}
