#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/FireWeaponCollide.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FireWeaponCollide` (`logic::modules::fire_weapon_collide`) against the unchanged C++, `p1w13-sonnet`, batch R62.
//!
//! # Method
//!
//! The C++ side is `FireWeaponCollide.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_fire_weapon_collide_r62.inc`,
//! `ref_l9r62_run`), on the module build's stand-ins: the object (its status bits the test's) and two others it may touch, the weapon store
//! (two templates, `WEAP_A` and `WEAP_B`, with the facts `Weapon`'s constructor reads), the stand-in `Weapon` running `Weapon.cpp`'s own
//! constructor, `crc`, `xfer` and `loadPostProcess` (cut by `build.rs`), whose `loadAmmoNow` and `fireWeapon` are lines; the REAL
//! `CollideModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects and the weapon store's two templates; the module from
//! `modules::new_module`, its weapon made by the world half of its constructor from the store (`Weapon::new_in_store`); the weapon's two
//! calls are records (`cuts.stub_weapon_calls`: the weapon code itself is verified by `tests/differential_weapon_fire.rs`).
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 40 ops: a collision (with an object or the ground),
//! a change to the status bits or the frame, or an xfer. Compared, bit for bit: the read's throw; the data and whether the weapon was made;
//! per op the throw status, the effects in order (the reload and the shot, with the weapon's template), the xfer status, bytes (the
//! weapon's own included) and CRC, and `m_everFired`.
//!
//! # Results (2026-10-25, `L9R62_CASES=9000`)
//!
//! 9,000 cases (633 data blocks threw, as the C++ did; 683 excluded, each a module whose `CollideWeapon` the store does not have, whose
//! weapon's constructor reads the `NULL` template), 156,673 ops and 172,041 outputs: **0 mismatches**, on the first run. In the compared
//! cases: 7,684 weapons made, 70,637 collisions, 33,122 shots and 55,132 xfers, each saved weapon's bytes upstream's own `Weapon::xfer`'s.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2321 | the weapon reloaded but not fired | 33,122 |
//! | 2322 | the required status ignored | 19,456 |
//! | 2323 | the forbidden status ignored | 5,942 |
//! | 2324 | `FireOnce` working as its name says (upstream never sets `m_everFired`) | 6,290 |
//! | 2325 | the weapon not saved | 23,577 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT, OBJECT_STATUS_NAMES};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::fire_weapon_collide::FireWeaponCollideModuleData;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r62.bin");
const XFER_FILE: &str = "l9_r62.bin";

const WEAPONS: [&str; 2] = ["WEAP_A", "WEAP_B"];
/// The status bits the generator names (the object's bits are drawn from the same ones).
const STATUS_BITS: [i32; 8] = [3, 4, 6, 11, 12, 15, 16, 30];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FireWeaponCollide(Arc::new(FireWeaponCollideModuleData::friend_new_module_data(Some(ini))?)));
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
    let data: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let made = r.u32();
    out.push(format!("data {data:x?} weapon made {made}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        let ever = r.u32();
        out.push(format!("op {i}: status {st} log {log:?} {x} ever fired {ever}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    // (mostly a weapon the store has: `None`, an unknown name and no line make the constructor read a `NULL` template)
    if g.below(100) < 97 {
        let name = if g.below(20) == 0 { pick(g, &["None", "NoSuchWeapon"]).to_string() } else { pick(g, &["WEAP_A", "WEAP_A", "WEAP_B"]).to_string() };
        lines.push(format!("CollideWeapon = {name}"));
    }
    if g.below(100) < 40 {
        lines.push(format!("FireOnce = {}", pick(g, &["Yes", "No"])));
    }
    let mut mask = |g: &mut Gen, name: &str| {
        if g.below(100) < 45 {
            let n = 1 + g.below(2);
            let names: Vec<&str> = (0..n).map(|_| OBJECT_STATUS_NAMES[*pick(g, &STATUS_BITS.iter().collect::<Vec<_>>()) as usize]).collect();
            lines.push(format!("{name} = {}", names.join(" ")));
        }
    };
    mask(g, "RequiredStatus");
    mask(g, "ForbiddenStatus");
    if !clean && g.below(4) == 0 {
        lines.push(pick(g, &["Bogus = 1", "FireOnce = maybe", "RequiredStatus = NOT_A_STATUS"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// The object's status bits, as the two words of the configuration.
fn gen_status(g: &mut Gen) -> [u32; 2] {
    let mut w = [0u32; 2];
    for &b in &STATUS_BITS {
        if g.below(100) < 25 {
            w[b as usize / 32] |= 1 << (b % 32);
        }
    }
    w
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 12];
    cfg[0] = *pick(g, &[&0u32, &1, &30, &1000]);
    let s = gen_status(g);
    cfg[1] = s[0];
    cfg[2] = s[1];
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
            0..=44 => [0, if g.below(8) == 0 { 0 } else { 2 + g.below(2) }, 0, 0, 0],
            45..=49 => [1, gen_frame(g), 0, 0, 0],
            50..=64 => {
                let s = gen_status(g);
                let a = 1 + g.below(2);
                [2, a, s[a as usize - 1], 0, 0]
            }
            65..=79 => [3, 0, 0, 0, 0],
            80..=92 => [4, 0, 0, 0, 0],
            _ => [5, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    collisions: u64,
    shots: u64,
    xfers: u64,
    made: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

fn put_mask(v: &mut Vec<u8>, m: &ObjectStatusMaskType) {
    let mut w = [0u32; 2];
    for i in 0..OBJECT_STATUS_COUNT {
        if m.test(i) {
            w[i as usize / 32] |= 1 << (i % 32);
        }
    }
    put_u32(v, w[0]);
    put_u32(v, w[1]);
}

fn set_scene(w: &mut World, cfg: &[u32]) {
    let o = w.objects.find_mut(1).unwrap();
    o.status = Default::default();
    for i in 0..OBJECT_STATUS_COUNT {
        if cfg[1 + i as usize / 32] >> (i % 32) & 1 != 0 {
            o.status.set(i, 1);
        }
    }
}

fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::WeaponLoadAmmoNow { source, template }) => log.push(format!("loadammo {source} {}", WEAPONS[template])),
                Some(CutCall::WeaponFireAtObject { source, target, template }) => {
                    tally.shots += 1;
                    log.push(format!("fire {source} {target} {}", WEAPONS[template]))
                }
                _ => {}
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
    let ModuleDataKind::FireWeaponCollide(d) = &data else { unreachable!() };
    let code = d.collide_weapon.as_deref().and_then(|n| WEAPONS.iter().position(|w| w.as_bytes().eq_ignore_ascii_case(n))).map_or(0, |k| k as u32 + 1);
    put_u32(&mut v, code);
    put_u32(&mut v, u32::from(d.fire_once));
    put_mask(&mut v, &d.required_status);
    put_mask(&mut v, &d.forbidden_status);

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
    for k in 1..=3 {
        let modules = if k == 1 {
            vec![rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FireWeaponCollide", b"ModuleTag_Collide", &data).unwrap().unwrap()]
        } else {
            Vec::new()
        };
        let id = w.new_object(NewObject { template: template.clone(), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    let idx = w.find_module(me, b"FireWeaponCollide").unwrap();
    let made = match w.objects.find(me).unwrap().modules[idx as usize].body.as_ref() {
        Some(UpdateBody::FireWeaponCollide(m)) => m.collide_weapon().is_some(),
        _ => panic!("the module is in its slot"),
    };
    t.made += u64::from(made);
    put_u32(&mut v, u32::from(made));
    set_scene(&mut w, &cfg);
    let ever = |w: &World| match w.objects.find(me).unwrap().modules[idx as usize].body.as_ref() {
        Some(UpdateBody::FireWeaponCollide(m)) => m.state_words()[0],
        _ => panic!("the module is in its slot"),
    };
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
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        match o[0] {
            0 => {
                let other = if (2..=3).contains(&o[1]) { o[1] as ObjectId } else { 0 };
                w.module_call(me, idx, ModuleCall::OnCollide { other, loc: None, normal: None });
                t.collisions += 1;
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
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
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
        put_u32(&mut v, ever(&w));
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
fn fire_weapon_collide_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R62 FireWeaponCollide",
        seed: 0x0062_F12E,
        cases_var: "L9R62_CASES",
        ini_stem: "Fc",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r62_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R62: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} weapons made, {} collisions, {} shots, {} xfers",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.made, t.collisions, t.shots, t.xfers
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.shots > 0 && t.made > 0, "the generator must reach every effect");
}
