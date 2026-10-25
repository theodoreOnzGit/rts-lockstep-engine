#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireWeaponUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FireWeaponUpdate` (`logic::modules::fire_weapon_update`) against the unchanged C++, `p1w13-sonnet`, batch R64.
//!
//! # Method
//!
//! The C++ side is `FireWeaponUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_fire_weapon_update_r64.inc`,
//! `ref_l9r64_run`), on the module build's stand-ins: the object (under construction or not, its last shot frame the test's), the weapon
//! store of batch R62 (`WEAP_A`, `WEAP_B`), the stand-in `Weapon` running `Weapon.cpp`'s own constructor, `crc`, `xfer`,
//! `loadPostProcess` and `getStatus` (cut by `build.rs`) and `Weapon.h`'s setters, whose `loadAmmoNow` and `forceFireWeapon` are lines; the
//! REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the object and the weapon store's two templates; the module from `modules::new_module`,
//! its weapon made by the world half of its constructor from the store (`Weapon::new_in_store`) and its status the real
//! `Weapon::get_status`; the weapon's two calls are records (`cuts.stub_weapon_calls`: the weapon code itself is verified by
//! `tests/differential_weapon_fire.rs`). With the reload a record the weapon is never loaded, so the ops set its state through the setters
//! upstream has (`setStatus`, `setPossibleNextShotFrame`, `setPreAttackFinishedFrame`): a status that `getStatus` keeps while the weapon
//! cannot fire again yet.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 40 ops: an `update()`, a change to the object or the
//! frame, a weapon setter, or an xfer. Compared, bit for bit: the read's throw; the data, the constructor's effects, whether the weapon was
//! made and `m_initialDelayFrame`; per op the throw status, the result, the effects in order (the shot, with its position and template),
//! the xfer status, bytes (the weapon's own included; the scheduling word masked) and CRC, and `m_initialDelayFrame`.
//!
//! # Results (2026-10-25, `L9R64_CASES=9000`)
//!
//! 9,000 cases (661 data blocks threw, as the C++ did; none excluded), 172,161 ops and 188,839 outputs: **0 mismatches**, on the first run.
//! In the compared cases: 7,258 weapons made (each loaded by the constructor), 60,197 updates, 2,098 shots and 32,668 xfers (a module with
//! no weapon throws `XFER_INVALID_PARAMETERS` on a save or a load, as upstream's `xferSnapshot(NULL)`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 6 of 6 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2341 | the weapon's status not asked | 24,058 |
//! | 2342 | an object under construction fires | 299 |
//! | 2343 | the exclusive delay not added to the last shot frame | 151 |
//! | 2344 | the weapon not loaded by the constructor | 7,258 |
//! | 2345 | the initial delay ignored | 1,486 |
//! | 2346 | `m_initialDelayFrame` not saved (version 2) | 11,880 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::fire_weapon_update::FireWeaponUpdateModuleData;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::weapon_types::WeaponStatus;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::sleepy::UpdateHandle;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r64.bin");
const XFER_FILE: &str = "l9_r64.bin";

const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FireWeaponUpdate(Arc::new(FireWeaponUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let data: Vec<u32> = (0..3).map(|_| r.u32()).collect();
    let n = r.u32();
    let ctor: Vec<String> = (0..n).map(|_| r.str()).collect();
    let (made, delay) = (r.u32(), r.u32());
    out.push(format!("data {data:?} constructor {ctor:?} weapon made {made} initial delay frame {delay}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let delay = r.u32();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} initial delay frame {delay}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    // (mostly a weapon the store has: `None`, an unknown name and no line leave the module without one)
    if g.below(100) < 93 {
        let name = if g.below(15) == 0 { pick(g, &["None", "NoSuchWeapon"]).to_string() } else { pick(g, &["WEAP_A", "WEAP_A", "WEAP_B", "weap_b"]).to_string() };
        lines.push(format!("Weapon = {name}"));
    }
    if g.below(100) < 50 {
        lines.push(format!("InitialDelay = {}", pick(g, &["0", "33", "100", "1000", "5000"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("ExclusiveWeaponDelay = {}", pick(g, &["0", "34", "500", "2000"])));
    }
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "InitialDelay = soon", "ExclusiveWeaponDelay = -"]).to_string());
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

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 12];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &100]);
    cfg[1] = u32::from(g.below(8) == 0);
    cfg[2] = if g.below(2) == 0 { 0 } else { gen_small_frame(g) };
    for k in 0..2 {
        cfg[3 + 4 * k] = (*pick(g, &[&-std::f32::consts::PI, &-std::f32::consts::PI, &-0.5f32, &0.0])).to_bits();
        cfg[4 + 4 * k] = (*pick(g, &[&std::f32::consts::PI, &std::f32::consts::PI, &0.5f32, &1.0])).to_bits();
        cfg[5 + 4 * k] = *pick(g, &[&1u32, &1, &2, &5]);
        cfg[6 + 4 * k] = *pick(g, &[&0u32, &0, &15, &100]);
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=34 => [0, 0, 0, 0, 0],
            35..=44 => [1, gen_small_frame(g), 0, 0, 0],
            45..=54 => {
                let a = 1 + g.below(2);
                [2, a, if a == 1 { u32::from(g.below(6) == 0) } else { gen_small_frame(g) }, 0, 0]
            }
            55..=62 => [3, 0, 0, 0, 0],
            63..=69 => [4, 0, 0, 0, 0],
            70..=73 => [5, 0, 0, 0, 0],
            74..=83 => [6, if g.below(2) == 0 { 0 } else { g.below(5) }, 0, 0, 0],
            84..=93 => [7, gen_small_frame(g), 0, 0, 0],
            _ => [8, if g.below(2) == 0 { 0 } else { gen_small_frame(g) }, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    shots: u64,
    xfers: u64,
    made: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    let o = w.objects.find_mut(1).unwrap();
    o.status = Default::default();
    if cfg[1] & 1 != 0 {
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    }
    w.cuts.contain.last_shot_fired_frame.insert(1, cfg[2]);
}

fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::WeaponLoadAmmoNow { source, template }) => log.push(format!("loadammo {source} {}", WEAPONS[template])),
                Some(CutCall::WeaponForceFireAtPosition { source, pos, template }) => {
                    tally.shots += 1;
                    log.push(format!("forcefire {source} {:08x} {:08x} {:08x} {}", pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), WEAPONS[template]))
                }
                _ => {}
            }
        }
    }
    log
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

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::FireWeaponUpdate(d) = &data else { unreachable!() };
    let code = d.weapon.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |k| k as u32 + 1);
    put_u32(&mut v, code);
    put_u32(&mut v, d.initial_delay_frames);
    put_u32(&mut v, d.exclusive_weapon_delay);

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
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, weapon_store: store, ..Default::default() });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.stub_weapon_calls = true;
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let modules = vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FireWeaponUpdate", b"ModuleTag_Fire", &data).unwrap().unwrap()];
    let me: ObjectId = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let ctor = effects(&mut w, &mut t);
    w.set_position(me, &Coord3D { x: 11.0, y: 22.0, z: 5.0 });
    let idx = w.find_module(me, b"FireWeaponUpdate").unwrap();
    let u: UpdateHandle = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    put_u32(&mut v, ctor.len() as u32);
    for l in &ctor {
        put_str(&mut v, l.as_bytes());
    }
    let state = |w: &World| -> (bool, u32) {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::FireWeaponUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        (m.weapon().is_some(), m.state_words()[0])
    };
    let (made, delay) = state(&w);
    t.made += u64::from(made);
    put_u32(&mut v, u32::from(made));
    put_u32(&mut v, delay);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=2).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
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
                t.xfers += 1;
            }
            6..=8 => {
                let ob = w.objects.find_mut(me).unwrap();
                let Some(UpdateBody::FireWeaponUpdate(m)) = ob.behaviors[u.index as usize].body.as_mut() else { panic!("the module is in its slot") };
                if let Some(weapon) = m.weapon_mut() {
                    match o[0] {
                        6 => weapon.set_status(status_of(o[1])),
                        7 => weapon.set_possible_next_shot_frame(o[1]),
                        _ => weapon.set_pre_attack_finished_frame(o[1]),
                    }
                }
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
        put_u32(&mut v, state(&w).1);
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
fn fire_weapon_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R64 FireWeaponUpdate",
        seed: 0x0064_F1EE,
        cases_var: "L9R64_CASES",
        ini_stem: "Fu",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r64_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R64: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} weapons made, {} updates, {} shots, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.made, t.updates, t.shots, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.shots > 0 && t.made > 0, "the generator must reach every effect");
}
