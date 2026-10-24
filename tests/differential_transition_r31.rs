#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/TransitionDamageFX.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `TransitionDamageFX` (`logic::modules::transition_damage_fx`) against the unchanged C++, `p1w13-sonnet`, batch R31.
//!
//! # Method
//!
//! The C++ side is `TransitionDamageFX.cpp` (its particle system parser routed to the unit's, by `build.rs`), compiled into `wrap_module.cpp`'s
//! unit (`differential/l9_transition_r31.inc`, `ref_l9r31_run`), on the module build's stand-ins: the object (id 1) with maybe a body (its last
//! damage none or the test's), maybe a drawable (its art has the bones `B1`, `B2` and `F01`.. as many as the case says), a position and an
//! orientation; the damage's source (id 2) through `findObjectByID`; the FX list store (`FX_A`, `FX_B`), the object creation list store (`OCL_A`,
//! `OCL_B`), the particle system manager (a template for every name but `None`, ids 1, 2, ...); the REAL `DamageModule`, `INI`, random values
//! and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, a `ProbeModule` body), the same
//! logic random seed, `cuts.contain.pristine_bones`, an OCL store with the two lists; every op a body damage state change (a damage from object
//! 0, 1 or 2), a change to the scene, or an xfer.
//!
//! Each case reads a block `D` (the damage type masks and up to twenty entries, at bones, random bones or points; mostly well formed) and runs 1
//! to 40 ops. Compared, bit for bit: the read's throw; the data (masks, every entry's location type, bone, random flag, point and effect); per op
//! the throw status, the effects in order (the FX at their world positions, the OCLs with their source's position, the particle systems made,
//! attached and destroyed), the xfer status, bytes and CRC, and the particle system ids.
//!
//! # Results (2026-10-24, `L9R31_CASES=9000`)
//!
//! 9,000 cases (5,148 compared; 1,824 data blocks threw, as the C++ did; 2,028 excluded, undefined upstream: 1,224 an OCL for a damage with no
//! source object, 804 a worse state with no body), 97,887 ops (54,661 state changes, 30,938 xfers) and 849,495 outputs: **0 mismatches**; 58,178
//! effect lines compared: 14,103 FX lists and 9,177 OCLs, 14,067 particle systems made and 6,764 destroyed (the counts are the compared cases').
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2011 | the same state counts as worse | 29,609 |
//! | 2012 | the old state's systems not destroyed | 25,712 |
//! | 2013 | a random bone is always the first | 1,477 |
//! | 2014 | a missing bone falls back on zero, not `Loc:` | 117 |
//! | 2015 | the last damage's type not tested | 1,483 |
//! | 2016 | the OCL's secondary position is the primary | 6,140 |
//! | 2017 | an FX list at the local position | 9,501 |
//! | 2018 | the ids not saved | 20,057 |
//! | 2019 | the new systems' ids not kept | 35,709 |

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
use rts_lockstep_engine::logic::modules::transition_damage_fx::{
    TransitionDamageFxModuleData, BODYDAMAGETYPE_COUNT, DAMAGE_MODULE_MAX_FX, KIND_FX, KIND_OCL,
};
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r31.bin");
const XFER_FILE: &str = "l9_r31.bin";

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
    *out = Some(ModuleDataKind::TransitionDamageFx(Arc::new(
        TransitionDamageFxModuleData::friend_new_module_data(Some(ini))?,
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

fn put_data(v: &mut Vec<u8>, d: &TransitionDamageFxModuleData, fx: &mut FxIds) {
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
            for i in 0..DAMAGE_MODULE_MAX_FX {
                let e = &d.lists[k][s][i];
                put_i32(v, i32::from(e.loc_type));
                put_str(v, &e.bone_name);
                put_u32(v, u32::from(e.random_bone));
                for c in [e.loc.x, e.loc.y, e.loc.z] {
                    put_u32(v, c.to_bits());
                }
                match k {
                    KIND_FX => put_i32(v, fx.id(&e.effect)),
                    KIND_OCL => put_i32(v, e.effect.as_deref().map_or(0, ocl_code)),
                    _ => put_str(v, e.effect.as_deref().unwrap_or(b"")),
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
            for i in 0..DAMAGE_MODULE_MAX_FX {
                let t = u32_(&mut p);
                let bone = str_(&mut p);
                let random = u32_(&mut p);
                let loc: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
                let effect = if k == 2 {
                    str_(&mut p)
                } else {
                    format!("{}", u32_(&mut p))
                };
                out.push(format!("entry {k} {s} {i}: type {t} {bone:?} random {random} loc {loc:x?} effect {effect}"));
            }
        }
    }
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let ids: Vec<u32> = (0..BODYDAMAGETYPE_COUNT * DAMAGE_MODULE_MAX_FX)
            .map(|_| u32_(&mut p))
            .collect();
        out.push(format!(
            "op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} ids {ids:?}"
        ));
    }
    out
}

const STATES: [&str; 3] = ["Damaged", "ReallyDamaged", "Rubble"];
const DAMAGE_NAMES: [&str; 6] = [
    "EXPLOSION",
    "CRUSH",
    "ARMOR_PIERCING",
    "SMALL_ARMS",
    "FLAME",
    "POISON",
];

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
    for _ in 0..2 + g.below(18) {
        let kind = g.below(3) as usize;
        let state = g.below(3) as usize;
        let range = if g.below(3) == 0 { 12 } else { 3 };
        let index = 1 + g.below(range);
        let (field, label, names): (&str, &str, &[&str]) = match kind {
            0 => ("FXList", "FXList", &["FX_A", "FX_B", "None", "FX_A"]),
            1 => ("OCL", "OCL", &["OCL_A", "OCL_B", "None", "OCL_C", "OCL_A"]),
            _ => ("ParticleSystem", "PSys", &["PS_A", "PS_B", "None", "PS_A"]),
        };
        let loc = match g.below(6) {
            0 => "Bone:B1 RandomBone:No".to_string(),
            1 => "Bone:B2 RandomBone:No".to_string(),
            2 => "Bone:Gone RandomBone:No".to_string(),
            3 => "Bone:F RandomBone:Yes".to_string(),
            4 => "Bone:G RandomBone:Yes".to_string(),
            _ => format!(
                "Loc: X:{} Y:{} Z:{}",
                pick(g, &["0", "2.5", "-3"]),
                pick(g, &["0", "1", "-7.25"]),
                pick(g, &["0", "4"])
            ),
        };
        let effect = pick(g, names);
        lines.push(format!(
            "{}{field}{index} = {loc} {label}:{effect}",
            STATES[state]
        ));
    }
    if !clean {
        lines.push(
            pick(
                g,
                &[
                    "DamagedFXList1 = Bone:B1 Random:No FXList:FX_A",
                    "DamagedOCL2 = Spot:1 OCL:OCL_A",
                    "DamagedFXList3 = Loc: X:1 Y:2 FXList:FX_A",
                    "DamagedFXList13 = Loc: X:1 Y:2 Z:3 FXList:FX_A",
                    "PristineFXList1 = Loc: X:1 Y:2 Z:3 FXList:FX_A",
                    "RubbleOCL1 = Bone:B1 RandomBone:No FX:FX_A",
                    "DamageFXTypes = +BOGUS",
                    "DamagedParticleSystem1 = Bone:B1 RandomBone:Maybe PSys:PS_A",
                ],
            )
            .to_string(),
        );
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
    changes: u64,
    xfers: u64,
    effects: u64,
    fx: u64,
    ocls: u64,
    systems: u64,
    destroys: u64,
}

fn run_xfer(
    w: &mut World,
    me: ObjectId,
    idx: usize,
    how: u32,
    saved: &[u8],
) -> (i32, Vec<u8>, u32) {
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

/// The scene of the configuration (the driver's `scene`): the drawable, the body's last damage, the two positions, the bones.
fn set_scene(w: &mut World, cfg: &[u32; 20], body: Option<u16>) {
    let o = w.objects.find_mut(1).unwrap();
    o.drawable = if cfg[3] & 1 != 0 {
        7
    } else {
        INVALID_DRAWABLE_ID
    };
    if let Some(b) = body {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(1).unwrap().modules[b as usize]
            .body
            .as_mut()
        {
            p.answers.last_damage = (cfg[3] >> 2 & 1 != 0).then_some((0, cfg[4] as i32));
        }
    }
    w.set_position(
        1,
        &Coord3D {
            x: f32::from_bits(cfg[5]),
            y: f32::from_bits(cfg[6]),
            z: 0.0,
        },
    );
    w.set_orientation(1, f32::from_bits(cfg[7]));
    w.set_position(
        2,
        &Coord3D {
            x: f32::from_bits(cfg[8]),
            y: f32::from_bits(cfg[9]),
            z: 0.0,
        },
    );
    w.cuts.contain.pristine_bones.clear();
    for (k, name) in ["B1", "B2", "F01", "F02", "F03"].iter().enumerate() {
        if k >= 2 && k - 2 >= cfg[1] as usize {
            break;
        }
        let bone = Coord3D {
            x: f32::from_bits(cfg[10 + 2 * k]),
            y: f32::from_bits(cfg[11 + 2 * k]),
            z: (k + 1) as f32,
        };
        w.cuts
            .contain
            .pristine_bones
            .insert((1, name.as_bytes().to_vec()), bone);
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    cfg_in: &[u32; 20],
    ops: &[[u32; 4]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::TransitionDamageFx(d) = &data else {
        unreachable!()
    };
    put_data(&mut v, d, &mut fx);
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        object_creation_list_store: ocls,
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(1);
    w.random.init_game_logic_random(cfg[0]);
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"TransitionDamageFX",
        b"ModuleTag_Transition",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut modules = vec![m];
    if cfg[3] >> 1 & 1 != 0 {
        let mut b = NewModule::plain(b"R31Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers::default(),
            100.0,
            Default::default(),
        )));
        modules.push(b);
    }
    let tt = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    assert_eq!(
        w.new_object(NewObject {
            template: tt.clone(),
            status: Default::default(),
            team: None,
            modules
        })
        .unwrap(),
        1
    );
    assert_eq!(
        w.new_object(NewObject {
            template: tt,
            status: Default::default(),
            team: None,
            modules: Vec::new()
        })
        .unwrap(),
        2
    );
    let idx = w.find_module(1, b"TransitionDamageFX").unwrap();
    let body = w.objects.find(1).unwrap().body;
    w.cuts.module_errors.clear();
    set_scene(&mut w, &cfg, body);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 1 && (3..=9).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        set_scene(&mut w, &cfg, body);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        match o[0] {
            0 => {
                let mut info = DamageInfo::default();
                info.input.source_id = o[3] as ObjectId;
                w.module_call(
                    1,
                    idx,
                    ModuleCall::OnBodyDamageStateChange {
                        info,
                        old_state: o[1] as i32,
                        new_state: o[2] as i32,
                    },
                );
                tally.changes += 1;
            }
            2..=4 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 2, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 2 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            1 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let status = if new_errors
            .iter()
            .any(|(_, e)| e.kind == IniErrorKind::BadArg)
        {
            3
        } else if new_errors.iter().any(|(_, e)| !e.is_undefined()) {
            2
        } else {
            0
        };
        put_i32(&mut v, status);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) =
            (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let mut log = Vec::new();
        let b = |x: f32| x.to_bits();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::OclCreateAt {
                        ocl,
                        primary_pos: p,
                        secondary_pos: q,
                        angle,
                        ..
                    }) => {
                        tally.ocls += 1;
                        log.push(format!(
                            "oclat {} {:08x},{:08x},{:08x} {:08x},{:08x},{:08x} {:08x}",
                            ocl_code(&ocl) - 1,
                            b(p.x),
                            b(p.y),
                            b(p.z),
                            b(q.x),
                            b(q.y),
                            b(q.z),
                            b(angle)
                        ))
                    }
                    Some(CutCall::ParticleSystemCreate {
                        template,
                        position: p,
                        id,
                    }) => {
                        tally.systems += 1;
                        log.push(format!(
                            "psys {} {id} {:08x},{:08x},{:08x}",
                            String::from_utf8_lossy(&template),
                            b(p.x),
                            b(p.y),
                            b(p.z)
                        ))
                    }
                    Some(CutCall::Body(BodyCall::ParticleAttachToObject { id, object })) => {
                        log.push(format!("psattach {id} {object}"))
                    }
                    Some(CutCall::ParticleSystemDestroyById { id }) => {
                        tally.destroys += 1;
                        log.push(format!("psdestroyid {id}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxPos {
                    fx: name,
                    position: p,
                }) = ev_iter.next()
                {
                    tally.fx += 1;
                    log.push(format!(
                        "fxpos {} {},{},{}",
                        fx.id(&Some(name)),
                        b(p.x),
                        b(p.y),
                        b(p.z)
                    ));
                }
            } else if let Some(l) = t.strip_prefix("bt ") {
                log.push(l.to_string());
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        let Some(UpdateBody::TransitionDamageFx(m)) = ob.modules[idx as usize].body.as_ref() else {
            panic!("the module is in its slot")
        };
        for s in m.state_words() {
            put_u32(&mut v, s);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R31_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn transition_damage_fx_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0031_7DFC);
    let mut read = Check::new("R31 data read (throw, message)");
    let mut run = Check::new("R31 ops (data, effects, xfers, ids)");
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
        let name = format!("Data\\INI\\R31{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let flags = u32::from(g.below(8) != 0) | u32::from(g.below(8) != 0) << 1 | g.below(2) << 2;
        let mut cfg = [0u32; 20];
        cfg[0] = g.u32();
        cfg[1] = g.below(4);
        cfg[3] = flags;
        cfg[4] = g.below(6);
        cfg[5] = real(&mut g, &[0.0, 10.0, -35.5]);
        cfg[6] = real(&mut g, &[0.0, 7.0, 120.25]);
        cfg[7] = real(&mut g, &[0.0, 0.5, -1.2, 2.9]);
        cfg[8] = real(&mut g, &[0.0, 50.0, -20.5]);
        cfg[9] = real(&mut g, &[0.0, 3.0]);
        for c in &mut cfg[10..20] {
            *c = real(&mut g, &[0.0, 1.5, -4.0, 12.0]);
        }
        let n_ops = 1 + g.below(40) as usize;
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(16) {
                0..=8 => [
                    0,
                    g.below(4),
                    g.below(4),
                    *pick(&mut g, &[&2u32, &2, &2, &2, &1, &1, &2, &0]),
                ],
                9..=10 => {
                    let a = 3 + g.below(7);
                    let b = match a {
                        3 => u32::from(g.below(8) != 0) | (flags & 2) | g.below(2) << 2,
                        4 => g.below(6),
                        5 | 8 => real(&mut g, &[0.0, 10.0, -35.5]),
                        6 | 9 => real(&mut g, &[0.0, 7.0]),
                        _ => real(&mut g, &[0.0, 0.5, -1.2]),
                    };
                    [1, a, b, 0]
                }
                11..=12 => [2, 0, 0, 0],
                13..=14 => [3, 0, 0, 0],
                _ => [4, 0, 0, 0],
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
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r31_run(
                cfile.as_ptr(),
                cxfer.as_ptr(),
                cfg.as_ptr(),
                ops.len() as i32,
                flat.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut len,
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut code,
            )
        };
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || {
            format!(
                "case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}",
                show(&text)
            )
        };
        read.eq(
            &outcome,
            &Outcome {
                status,
                detail,
                crc: 0,
            },
            what,
        );
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
            eprintln!(
                "--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}",
                show(&text),
                dr.get(i),
                dc.get(i)
            );
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R31: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} state changes, {} xfers, \
         {} effect lines ({} FX, {} OCLs, {} particle systems, {} destroyed)",
        tally.changes, tally.xfers, tally.effects, tally.fx, tally.ocls, tally.systems, tally.destroys
    );
    println!("R31 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.fx * 2 > cases as u64
            && tally.ocls * 2 > cases as u64
            && tally.systems * 2 > cases as u64
            && tally.destroys * 2 > cases as u64,
        "the generator must reach every effect"
    );
}
