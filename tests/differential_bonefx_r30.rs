#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BoneFXUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/BoneFXDamage.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BoneFXUpdate` and `BoneFXDamage` (`logic::modules::bone_fx_update`) against the unchanged C++, `p1w13-sonnet`, batch R30.
//!
//! # Method
//!
//! The C++ side is `BoneFXUpdate.cpp` (its particle system parser routed to the unit's, by `build.rs`) and `BoneFXDamage.cpp`, compiled into
//! `wrap_module.cpp`'s unit (`differential/l9_bonefx_r30.inc`, `ref_l9r30_run`; the classes under R30 names, as StructureCollapseUpdate's
//! driver has a stand-in `BoneFXUpdate`), on the module build's stand-ins: the object (id 1) with maybe a body (its last damage none or the
//! test's), maybe a drawable (hidden or not; its art has the bones `B1`-`B4` at the test's positions), a position and an orientation; maybe a
//! `BoneFXDamage`; the FX list store (`FX_A`, `FX_B`), the object creation list store (`OCL_A`, `OCL_B`), the particle system manager (a
//! template for every name but `None`, ids 1, 2, ...); the REAL `UpdateModule`, `DamageModule`, `INI`, random values (the logic's and the
//! client's) and `Xfer`; the game logic, whose wake calls are applied as `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same object (both modules from `modules::new_module`, a `ProbeModule` body), the same
//! two random seeds, `cuts.contain.pristine_bones`, an OCL store with the two lists; every op an `update()` as the scheduler runs it, a body
//! damage state change (through the damage module when there is one), `stopAllBoneFX`, `onObjectCreated`, or a change to the scene.
//!
//! Each case reads a block `D` (the damage type masks and up to a dozen entries; mostly well formed) and runs 1 to 80 ops. Compared, bit for
//! bit: the read's throw; the data (masks, every entry's bone, once flag, delay and effect); the constructor's wake calls; per op the throw
//! status, the result, the effects in order (the FX and OCLs at their world positions, the particle systems made, attached, stopped and
//! destroyed), the wake calls, the xfer status, bytes and CRC, and the whole state (the next frames, the bone positions, the state, the
//! resolved flags, the active flag, the particle system ids).
//!
//! # Results (2026-10-24, `L9R30_CASES=9000`)
//!
//! 9,000 cases (6,065 compared; 1,771 data blocks threw, as the C++ did; 1,164 excluded, undefined upstream: 641 an effect's bones with no
//! drawable, 523 its damage test with no body), 237,129 ops (~~194,858~~ **158,285** updates, ~~16,314~~ **13,254** body damage state changes,
//! ~~32,459~~ **26,426** xfers) and 1,548,951 outputs: **0 mismatches**; ~~30,049~~ **29,614** effect lines compared: ~~5,744~~ **5,644** FX lists and
//! ~~5,371~~ **5,303** OCLs at their bones' world positions, ~~5,772~~ **5,688** particle systems made (~~2,738~~ **2,707** stopped, the drawable
//! hidden) and ~~4,652~~ **4,584** destroyed.
//! **CORRECTED 2026-10-24** (the same day): the first record's counts of updates, xfers and effects also counted the cases later excluded as
//! undefined upstream, whose outputs are not compared; the counts below are the compared cases' only (the output and mismatch counts were right).
//!
//! On the way (driver and stand-in fixes): upstream's function-local `static` name keys outlive the test's per-case name key generator, so
//! `build.rs` drops the `static`, as for `FlammableUpdate.cpp`; the stand-in particle system manager made a system of a `NULL` template, where
//! the real one makes none; the classes are compiled under R30 names (a copy of the header without its `#pragma once`), StructureCollapseUpdate's
//! driver having a stand-in `BoneFXUpdate`.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2001 | the last damage's type not tested | 1,354 |
//! | 2002 | `OnlyOnce` ignored | 76,485 |
//! | 2003 | the logic and client delays swapped | 138,357 |
//! | 2004 | an FX list at the bone's art position, not the world's | 3,868 |
//! | 2005 | a state change keeps the particle systems | 32,288 |
//! | 2006 | the bones looked up at every effect | 127,426 |
//! | 2007 | an effect a frame early | 69,297 |
//! | 2008 | the loaded particle system ids dropped | 1,838 |
//! | 2009 | a system made under a hidden drawable not stopped | 1,983 |
//!
//! A first 2008 (the state not loaded) and 2009 (a state change to the old state) made the C++ run crash: they took the Rust side off the
//! cases where upstream is undefined, so the C++ ran into them. They were replaced by the two above, which change no path into those cases.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::damage_module::DamageModuleData;
use rts_lockstep_engine::logic::modules::bone_fx_update::{BoneFxUpdateModuleData, BODYDAMAGETYPE_COUNT, BONE_FX_MAX_BONES, KIND_FX, KIND_OCL};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r30.bin");
const XFER_FILE: &str = "l9_r30.bin";

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::BoneFxUpdate(Arc::new(BoneFxUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// `fxId`: the FX lists numbered in the order the dump first names them (0 for `NULL`).
#[derive(Default)]
struct FxIds(Vec<Vec<u8>>);
impl FxIds {
    fn id(&mut self, fx: &Option<Vec<u8>>) -> i32 {
        let Some(n) = fx else { return 0 };
        match self.0.iter().position(|x| x == n) {
            Some(i) => i as i32 + 1,
            None => {
                self.0.push(n.clone());
                self.0.len() as i32
            }
        }
    }
}

fn ocl_code(n: &[u8]) -> i32 {
    match n {
        b"OCL_A" => 1,
        b"OCL_B" => 2,
        _ => 0,
    }
}

/// `DAMAGE_NUM_TYPES`, the bits of the masks the dump writes.
const DAMAGE_NUM_TYPES: i32 = rts_lockstep_engine::logic::damage::DAMAGE_NUM_TYPES as i32;

fn put_data(v: &mut Vec<u8>, d: &BoneFxUpdateModuleData, fx: &mut FxIds) {
    for k in 0..3 {
        let mut w = [0u32; 2];
        for t in 0..DAMAGE_NUM_TYPES.min(64) {
            if d.damage_types[k].test(t) {
                w[(t / 32) as usize] |= 1 << (t % 32);
            }
        }
        put_u32(v, w[0]);
        put_u32(v, w[1]);
    }
    for k in 0..3 {
        for s in 0..BODYDAMAGETYPE_COUNT {
            for i in 0..BONE_FX_MAX_BONES {
                let b = &d.lists[k][s][i];
                put_str(v, &b.bone_name);
                put_u32(v, u32::from(b.only_once));
                if !b.bone_name.is_empty() {
                    let (lo, hi, t) = if k == 2 {
                        (b.game_client_delay.low, b.game_client_delay.high, b.game_client_delay.distribution as u32)
                    } else {
                        (b.game_logic_delay.low, b.game_logic_delay.high, b.game_logic_delay.distribution as u32)
                    };
                    put_u32(v, lo.to_bits());
                    put_u32(v, hi.to_bits());
                    put_u32(v, t);
                }
                match k {
                    KIND_FX => put_i32(v, fx.id(&b.effect)),
                    KIND_OCL => put_i32(v, b.effect.as_deref().map_or(0, ocl_code)),
                    _ => put_str(v, b.effect.as_deref().unwrap_or(b"")),
                }
            }
        }
    }
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut p = 0usize;
    let mut out = Vec::new();
    let u32_ = |p: &mut usize| -> u32 {
        let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
        *p += 4;
        v
    };
    let str_ = |p: &mut usize| -> String {
        let n = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap()) as usize;
        *p += 4;
        let s = String::from_utf8_lossy(&b[*p..*p + n]).into_owned();
        *p += n;
        s
    };
    if b.is_empty() {
        return out;
    }
    let masks: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    out.push(format!("masks {masks:x?}"));
    for k in 0..3 {
        for s in 0..BODYDAMAGETYPE_COUNT {
            for i in 0..BONE_FX_MAX_BONES {
                let bone = str_(&mut p);
                let once = u32_(&mut p);
                let delay: Vec<u32> = if bone.is_empty() { Vec::new() } else { (0..3).map(|_| u32_(&mut p)).collect() };
                let effect = if k == 2 { str_(&mut p) } else { format!("{}", u32_(&mut p)) };
                out.push(format!("entry {k} {s} {i}: {bone:?} once {once} delay {delay:x?} effect {effect}"));
            }
        }
    }
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor wakes {w} last {l}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let frames: Vec<u32> = (0..96).map(|_| u32_(&mut p)).collect();
        let positions: Vec<u32> = (0..288).map(|_| u32_(&mut p)).collect();
        let tail: Vec<u32> = (0..7).map(|_| u32_(&mut p)).collect();
        if tail[6] > 4096 || p + 4 * tail[6] as usize > b.len() {
            out.push(format!("op {i}: a particle system count of {} (the dump is out of step)", tail[6]));
            break;
        }
        let ids: Vec<u32> = (0..tail[6]).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} frames {frames:x?}"));
        out.push(format!("op {i} positions {positions:x?}"));
        out.push(format!("op {i} state {tail:x?} ids {ids:?}"));
    }
    out
}

const STATES: [&str; 4] = ["Pristine", "Damaged", "ReallyDamaged", "Rubble"];
const DAMAGE_NAMES: [&str; 6] = ["EXPLOSION", "CRUSH", "ARMOR_PIERCING", "SMALL_ARMS", "FLAME", "POISON"];

fn damage_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        t.push(match g.below(8) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=4 => format!("+{}", DAMAGE_NAMES[g.below(6) as usize]),
            _ => format!("-{}", DAMAGE_NAMES[g.below(6) as usize]),
        });
    }
    t.join(" ")
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    for name in ["DamageFXTypes", "DamageOCLTypes", "DamageParticleTypes"] {
        if g.below(100) < 20 {
            lines.push(format!("{name} = {}", damage_tokens(g)));
        }
    }
    // most entries in the pristine state, where the update starts
    for _ in 0..3 + g.below(18) {
        let kind = g.below(3) as usize;
        let state = if g.below(3) == 0 { 1 + g.below(3) as usize } else { 0 };
        let range = if g.below(3) == 0 { 8 } else { 3 };
        let index = 1 + g.below(range);
        let (field, label, names): (&str, &str, &[&str]) = match kind {
            0 => ("FXList", "FXList", &["FX_A", "FX_B", "None", "FX_A"]),
            1 => ("OCL", "OCL", &["OCL_A", "OCL_B", "None", "OCL_C", "OCL_A", "OCL_B"]),
            _ => ("ParticleSystem", "PSys", &["PS_A", "PS_B", "None", "PS_A"]),
        };
        let bone = pick(g, &["B1", "B2", "B3", "B4", "Gone"]);
        let once = pick(g, &["Yes", "No", "No"]);
        let (lo, hi) = (pick(g, &["0", "100", "500"]), pick(g, &["300", "1000", "2000"]));
        let effect = pick(g, names);
        lines.push(format!("{}{field}{index} = Bone:{bone} OnlyOnce:{once} {lo} {hi} {label}:{effect}", STATES[state]));
    }
    if !clean {
        lines.push(pick(g, &["PristineFXList1 = Joint:B1 OnlyOnce:Yes 0 0 FXList:FX_A", "PristineOCL2 = Bone:B1 Once:Yes 0 0 OCL:OCL_A",
            "PristineParticleSystem1 = Bone:B2 OnlyOnce:Yes 0 0 FXList:PS_A", "PristineFXList9 = Bone:B1 OnlyOnce:No 0 0 FXList:FX_A",
            "DamageFXTypes = +BOGUS", "PristineFXList1 = Bone:B1 OnlyOnce:Maybe 0 0 FXList:FX_A"]).to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// What the compared cases reached (a case excluded as undefined upstream counts for nothing).
#[derive(Default, Clone)]
struct Tally {
    effects: u64,
    updates: u64,
    xfers: u64,
    changes: u64,
    fx: u64,
    ocls: u64,
    systems: u64,
    stops: u64,
    destroys: u64,
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(4096);
            let mut x = XferLoad::new(b);
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

/// The scene of the configuration (the driver's `scene`): the drawable, the body's last damage, the position, the bones.
fn set_scene(w: &mut World, cfg: &[u32; 16], body: Option<u16>) {
    let o = w.objects.find_mut(1).unwrap();
    o.drawable = if cfg[3] & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
    if let Some(b) = body {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(1).unwrap().modules[b as usize].body.as_mut() {
            p.answers.last_damage = (cfg[3] >> 3 & 1 != 0).then_some((0, cfg[4] as i32));
        }
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[5]), y: f32::from_bits(cfg[6]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[7]));
    for k in 0..4 {
        let bone = Coord3D { x: f32::from_bits(cfg[8 + 2 * k]), y: f32::from_bits(cfg[9 + 2 * k]), z: (k + 1) as f32 };
        w.cuts.contain.pristine_bones.insert((1, format!("B{}", k + 1).into_bytes()), bone);
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 16], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::BoneFxUpdate(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), object_creation_list_store: ocls, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BoneFXUpdate", b"ModuleTag_BoneFX", &data).unwrap().unwrap();
    let mut modules = vec![m];
    if cfg[3] >> 4 & 1 != 0 {
        let dd = ModuleDataKind::Damage(Arc::new(DamageModuleData::new()));
        modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BoneFXDamage", b"ModuleTag_BoneFXDamage", &dd).unwrap().unwrap());
    }
    if cfg[3] >> 2 & 1 != 0 {
        let mut b = NewModule::plain(b"R30Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 100.0, Default::default())));
        modules.push(b);
    }
    let values = ObjectTemplateValues::default();
    assert_eq!(w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&values)), status: Default::default(), team: None, modules }).unwrap(), 1);
    let idx = w.find_module(1, b"BoneFXUpdate").unwrap();
    let damage = w.find_module(1, b"BoneFXDamage");
    let body = w.objects.find(1).unwrap().body;
    // (making the object ran `onObjectCreated` on both modules: what it recorded is not an op's)
    w.cuts.module_errors.clear();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let (mut cw, mut cl) = (0u32, 0u32);
    for t in w.cuts.trace.take().unwrap() {
        if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
            cw += 1;
            cl = f;
        }
    }
    put_u32(&mut v, cw);
    put_u32(&mut v, cl);
    set_scene(&mut w, &cfg, body);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 5 && (3..=7).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 4 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, body);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1 => {
                let (old_state, new_state) = (o[1] as i32, o[2] as i32);
                match damage {
                    Some(dm) => w.module_call(1, dm, ModuleCall::OnBodyDamageStateChange { info: DamageInfo::default(), old_state, new_state }),
                    None => w.module_call(1, idx, ModuleCall::BoneFxChangeBodyDamageState { old_state, new_state }),
                };
                tally.changes += 1;
            }
            2 => {
                w.module_call(1, idx, ModuleCall::BoneFxStopAll);
            }
            3 => {
                w.module_call(1, idx, ModuleCall::OnObjectCreated);
                if let Some(dm) = damage {
                    w.module_call(1, dm, ModuleCall::OnObjectCreated);
                }
            }
            6..=8 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 6, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 6 {
                    saved = xbytes.clone();
                }
                if o[0] == 7 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            4 | 5 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let status = if new_errors.iter().any(|(_, e)| e.kind == IniErrorKind::BadArg) {
            3
        } else if new_errors.iter().any(|(_, e)| !e.is_undefined()) {
            2
        } else {
            0
        };
        put_i32(&mut v, status);
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let b = |x: f32| x.to_bits();
        let hidden = cfg[3] >> 1 & 1 != 0;
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::OclCreateAtPrimary { ocl, primary_pos: p, angle, .. }) => {
                        tally.ocls += 1;
                        log.push(format!("oclat {} {:08x},{:08x},{:08x} null {:08x}", ocl_code(&ocl) - 1, b(p.x), b(p.y), b(p.z), b(angle)))
                    }
                    Some(CutCall::ParticleSystemCreate { template, position: p, id }) => {
                        tally.systems += 1;
                        log.push(format!("psys {} {id} {:08x},{:08x},{:08x}", String::from_utf8_lossy(&template), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(CutCall::Body(BodyCall::ParticleAttachToObject { id, object })) => log.push(format!("psattach {id} {object}")),
                    Some(CutCall::Body(BodyCall::ParticleDestroy { id, found })) if found => {
                        tally.destroys += 1;
                        log.push(format!("psdestroy {id}"));
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::FxListDoFxPos { fx: name, position: p }) => {
                        tally.fx += 1;
                        log.push(format!("fxpos {} {},{},{}", fx.id(&Some(name)), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(ObjectClientEvent::ParticleSystemStopIfDrawableHidden { id, .. }) if hidden => {
                        tally.stops += 1;
                        log.push(format!("psstop {id}"));
                    }
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("bt ") {
                log.push(l.to_string());
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        let slot = &ob.behaviors[u.index as usize];
        let Some(UpdateBody::BoneFxUpdate(m)) = slot.body.as_ref() else { panic!("the module is back in its slot") };
        for s in m.state_words() {
            put_u32(&mut v, s);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R30_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn bone_fx_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0030_B0FE);
    let mut read = Check::new("R30 data read (throw, message)");
    let mut run = Check::new("R30 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R30{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let flags = u32::from(g.below(8) != 0) | g.below(2) << 1 | u32::from(g.below(8) != 0) << 2 | g.below(2) << 3 | u32::from(g.below(3) != 0) << 4;
        let mut cfg = [0u32; 16];
        cfg[0] = g.below(200);
        cfg[1] = g.u32();
        cfg[2] = g.u32();
        cfg[3] = flags;
        cfg[4] = g.below(6);
        cfg[5] = real(&mut g, &[0.0, 10.0, -35.5]);
        cfg[6] = real(&mut g, &[0.0, 7.0, 120.25]);
        cfg[7] = real(&mut g, &[0.0, 0.5, -1.2, 2.9]);
        for c in &mut cfg[8..16] {
            *c = real(&mut g, &[0.0, 1.5, -4.0, 12.0]);
        }
        let n_ops = 1 + g.below(80) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(24) {
                _ if g.below(3) == 0 => [0, 0, 0, 0],
                0..=11 => [0, 0, 0, 0],
                12..=13 => [1, g.below(4), g.below(4), 0],
                14 => [2, 0, 0, 0],
                15 => [3, 0, 0, 0],
                16..=18 => {
                    frame += 1 + g.below(40);
                    [4, frame, 0, 0]
                }
                19 => {
                    let a = 3 + g.below(5);
                    let b = match a {
                        3 => (flags & 0x14) | u32::from(g.below(8) != 0) | g.below(2) << 1 | g.below(2) << 3,
                        4 => g.below(6),
                        5 => real(&mut g, &[0.0, 10.0, -35.5]),
                        6 => real(&mut g, &[0.0, 7.0]),
                        _ => real(&mut g, &[0.0, 0.5, -1.2]),
                    };
                    [5, a, b, 0]
                }
                20..=21 => [6, 0, 0, 0],
                22 => [7, 0, 0, 0],
                _ => [8, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let tally_before = tally.clone();
            let (rd, w) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if let Some((_, e)) = w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
                *reasons.entry(format!("{e}")).or_insert(0u64) += 1;
                excluded += 1;
                tally = tally_before;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 23];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r30_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
        };
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || format!("case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R30: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} state changes, \
         {} xfers, {} effect lines ({} FX, {} OCLs, {} particle systems, {} stopped, {} destroyed)",
        tally.updates, tally.changes, tally.xfers, tally.effects, tally.fx, tally.ocls, tally.systems, tally.stops, tally.destroys
    );
    println!("R30 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.fx * 2 > cases as u64 && tally.ocls * 2 > cases as u64 && tally.systems * 2 > cases as u64 && tally.destroys * 10 > cases as u64, "the generator must reach every effect");
}
