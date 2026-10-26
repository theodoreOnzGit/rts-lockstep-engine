#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `BridgeBehavior` (`logic::modules::bridge_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R77.
//!
//! # Method
//!
//! The C++ side is `BridgeBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit under R77 names (`differential/l9_bridge_r77.inc`,
//! `ref_l9r77_run`), on the module build's stand-ins: the bridge (a team; a body whose damage state is the test's; a position), four towers
//! (`BRIDGE_TOWER` or not, a body or not), four objects that may stand on it (their kinds, physics, body, layer and position the test's), the
//! terrain bridge (present or not; its corners, template name and layer), two bridge templates (their transition effects, sounds, effect
//! height and count, and scaffold names the test's), the scaffold and support templates (their geometry the test's; the objects made carry
//! the REAL `BridgeScaffoldBehavior`), the partition scan's candidates, the bridge's bone, the FX and OCL stores, and the audio, radar,
//! pathfinder and terrain calls as lines; the REAL `Object::kill` and `attemptHealing`, `PointInsideArea2D`, `UpdateModule`, `INI`, the
//! logic's random values and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the bodies and physics `ProbeModule`s; the bridge, its templates, the
//! scan and the bone the world's cut-point tables; the scaffold objects the thing factory's, with the real `BridgeScaffoldBehavior`). Every
//! op an `update()`, damage, healing, a damage state change, the death, a tower set or read, the scaffolding made, removed or asked about,
//! `onDelete`, the scaffolds' updates, a change to the scene or the frame, or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 50 ops. Compared, bit for bit: the read's throw; the
//! data; per op the throw status, the result, the effects in order, the xfer status, bytes (the scheduling word masked) and CRC, the module's
//! state (the towers, the flags, the death frame, the scaffold list), the resolved effects, every object's health, layer and fall flag, and
//! every object made (its template, team, position, orientation and scaffold state).
//!
//! # Results (2026-10-26, `L9R77_CASES=9000`)
//!
//! 9,000 cases (547 data blocks threw, as the C++ did; 1,098 excluded as undefined upstream: 522 an object without a body where it is read
//! (damage passed on, a kill, the scaffolding removed), 295 a bridge template not found where it is read (its effects or scaffolding), 253 no
//! bridge where one is read (the scaffolding made, a load), 28 a support template of no height (a column of supports that never ends)),
//! 181,485 ops and 1,886,222 outputs: **0 mismatches**. In the compared cases: 27,261 updates and 16,315 xfers (4,157 loads failed);
//! 181,733 effect lines; 12,775 damages, 16,353 damage state changes and 7,115 deaths; 61,937 scaffold objects made; 41,972 area and timed
//! effects, 259 OCLs made from the parent object. The first runs differed only in the test's own bookkeeping (the configuration words of
//! the objects, the FX ids: the C++ build's INI and the logic's FX stores give the same name distinct pointers) and in one case the C++ build
//! could not finish (a support of no height, which this port reports as `Undefined::EndlessLoop`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2490 | damage from a tower passed on to the towers | 16,916 |
//! | 2491 | a surface spot measured to `toRight` instead of `fromRight` | 11,420 |
//! | 2492 | repaired and damaged swapped | 47,599 |
//! | 2493 | the death effects a frame late | 3,355 |
//! | 2494 | objects below the bridge dropped too | 1,144 |
//! | 2495 | the scaffold's 0.1 nudge left out | 1,145,572 |
//! | 2496 | a rubble bridge opened when the scaffolding goes | 773 |
//! | 2497 | the death frame not saved | 11,316 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use rts_lockstep_engine::logic::module::update_module::UpdateModuleData;
use rts_lockstep_engine::logic::modules::bridge_behavior::BridgeBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::{BridgeDetails, BridgeTemplateDetails, CutCall};
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r77.bin");
const XFER_FILE: &str = "l9_r77.bin";
/// The test's tables, by index (the same lists in the driver).
const MAX: [f32; 4] = [100.0, 250.0, 1000.0, 40.0];
const OCL: [&str; 4] = ["", "OCL_A", "OCL_B", "None"];
const FX: [&str; 4] = ["", "FX_A", "FX_B", "None"];
const SOUND: [&str; 4] = ["", "R77SndA", "R77SndB", "R77SndC"];
const NAMES: [&str; 4] = ["R77Scaffold", "R77Support", "Nope", ""];
const ROAD: [&str; 4] = ["R77Road", "R77Other", "Nope", ""];
const ROAD_NAMES: [&str; 2] = ["R77Road", "R77Other"];
const RADIUS: [f32; 4] = [5.0, 8.0, 12.0, 20.0];
const SCAFFOLD_HEIGHT: [f32; 4] = [3.0, 6.0, 10.0, 0.0];
const SUPPORT_HEIGHT: [f32; 4] = [4.0, 6.0, 10.0, 25.0];
const TEMPLATES: [&str; 2] = ["R77Scaffold", "R77Support"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::BridgeBehavior(Arc::new(BridgeBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The canonical number of an FX list: 0 for none, else 1 + the order the lists were first seen (the driver's `fxId`). A list is its
/// name and its store: the data's lists are the INI's store's (source 0), the bridge template's the logic's (source 1), distinct pointers
/// in the C++ build for the same name.
#[derive(Default)]
struct FxIds(Vec<(u8, Vec<u8>)>);
impl FxIds {
    fn id(&mut self, source: u8, fx: &Option<Vec<u8>>) -> u32 {
        let Some(n) = fx else { return 0 };
        let n = (source, n.clone());
        match self.0.iter().position(|x| *x == n) {
            Some(i) => i as u32 + 1,
            None => {
                self.0.push(n);
                self.0.len() as u32
            }
        }
    }
}

fn ocl_code(n: &[u8]) -> u32 {
    match n {
        b"OCL_A" => 1,
        b"OCL_B" => 2,
        _ => 0,
    }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let speeds = (r.u32(), r.u32());
    let nf = r.u32();
    let fx: Vec<(u32, u32, String)> = (0..nf).map(|_| (r.u32(), r.u32(), r.str())).collect();
    let no = r.u32();
    let ocl: Vec<(u32, u32, String)> = (0..no).map(|_| (r.u32(), r.u32(), r.str())).collect();
    out.push(format!("data {speeds:x?} fx {fx:?} ocl {ocl:?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        let x = r.xfer();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x}"));
        let head: Vec<u32> = (0..8).map(|_| r.u32()).collect();
        let list: Vec<u32> = (0..head[7]).map(|_| r.u32()).collect();
        out.push(format!("op {i} state {head:x?} list {list:?}"));
        let mut eff = Vec::new();
        for _ in 0..8 {
            let codes: Vec<u32> = (0..6).map(|_| r.u32()).collect();
            eff.push(format!("{codes:?} {} {}", r.str(), r.u32()));
        }
        out.push(format!("op {i} effects {eff:?}"));
        let objs: Vec<u32> = (0..27).map(|_| r.u32()).collect();
        out.push(format!("op {i} objects {objs:x?}"));
        let nm = r.u32();
        for k in 0..nm {
            let m: Vec<u32> = (0..21).map(|_| r.u32()).collect();
            out.push(format!("op {i} made {k} {m:x?}"));
        }
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    for name in ["LateralScaffoldSpeed", "VerticalScaffoldSpeed"] {
        if g.below(100) < 60 {
            let v = if !clean && g.below(15) == 0 { "x" } else { pick(g, &["1", "15", "30", "4.5", "-3"]) };
            lines.push(format!("{name} = {v}"));
        }
    }
    for _ in 0..g.below(4) {
        let list = *pick(g, &[&"FX_A", &"FX_B", &"FX_B", &"None"]);
        let delay = *pick(g, &[&"0", &"33", &"100", &"300"]);
        let bone = *pick(g, &[&"", &"", &" Bone:BoneA", &" Bone:BoneB"]);
        lines.push(format!("BridgeDieFX = FX:{list} Delay:{delay}{bone}"));
    }
    for _ in 0..g.below(4) {
        let list = *pick(g, &[&"OCL_A", &"OCL_B", &"OCL_B", &"None", &"Nope"]);
        let delay = *pick(g, &[&"0", &"33", &"100", &"300"]);
        let bone = *pick(g, &[&"", &"", &" Bone:BoneA", &" Bone:ParentObject", &" Bone:BoneB"]);
        lines.push(format!("BridgeDieOCL = OCL:{list} Delay:{delay}{bone}"));
    }
    if !clean && g.below(8) == 0 {
        lines.push(
            pick(
                g,
                &[
                    "BridgeDieFX = OCL:FX_A Delay:3",
                    "BridgeDieFX = FX:FX_A Wait:3",
                    "BridgeDieFX = FX:FX_A Delay:3 Nope:x",
                    "BridgeDieOCL = OCL:OCL_A",
                    "BridgeDieFX = FX:FX_A Delay:x",
                    "Bogus = 1",
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

/// A signed byte as the low bits of a word.
fn sb(v: i32) -> u32 {
    v as i8 as u8 as u32
}

/// Two bits a slot over `n` slots, mostly one of the defined lists.
fn slots(g: &mut Gen, n: u32, empty: u32) -> u32 {
    (0..n).fold(0, |v, i| v | (if g.below(100) < empty { 0 } else { 1 + g.below(3) }) << (2 * i))
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    match a {
        2 => {
            let team = if g.below(10) == 0 { 0 } else { 1 + g.below(3) };
            team | g.below(4) << 2 | g.below(4) << 4 | p(g, 97) << 6
        }
        6..=9 => p(g, 85) | p(g, 96) << 1 | g.below(4) << 2,
        10..=13 => {
            // (mostly inside the bridge's span, on the ground or the bridge's layer)
            let (x, y) = (g.below(60) as i32 - 30, g.below(30) as i32 - 15);
            let z = *pick(g, &[&0i32, &0, &1, &-2, &5]);
            p(g, 10) | p(g, 10) << 1 | p(g, 50) << 2 | p(g, 95) << 3 | (1 + g.below(3)) << 4 | p(g, 50) << 6 | sb(x) << 8 | sb(y) << 16 | sb(z) << 24
        }
        14 => {
            let t = if g.below(20) == 0 { 2 + g.below(2) } else { g.below(2) };
            p(g, 96) | t << 1 | (1 + g.below(3)) << 3
        }
        15..=18 => {
            // fromLeft, fromRight, toLeft, toRight around (0, 0): the bridge runs along x
            let (bx, by) = [(-30, 10), (-30, -10), (30, 10), (30, -10)][(a - 15) as usize];
            let j = |g: &mut Gen| g.below(7) as i32 - 3;
            let z = *pick(g, &[&0i32, &0, &2, &-3, &10]);
            sb(bx + j(g)) | sb(by + j(g)) << 8 | sb(z) << 16
        }
        19 | 25 => slots(g, 12, 60),
        20 | 26 => slots(g, 12, 60),
        21 | 27 => slots(g, 12, 60),
        22 | 28 => slots(g, 12, 60),
        23 | 29 => slots(g, 8, 30),
        24 | 30 => {
            let scaffold = if g.below(10) == 0 { 2 + g.below(2) } else { 0 };
            let support = if g.below(10) == 0 { g.below(4) } else { 1 };
            *pick(g, &[&0u32, &5, &20]) | g.below(4) << 8 | scaffold << 10 | support << 12
        }
        31 => g.below(4) | g.below(4) << 2 | g.below(4) << 4 | g.below(4) << 6,
        32 => {
            let n = g.below(9);
            (0..n).fold(0, |v, i| v | (1 + g.below(9)) << (4 * i))
        }
        33 => p(g, 60),
        _ => 0,
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 40];
    cfg[0] = *pick(g, &[&1u32, &5, &100]);
    cfg[1] = g.u32();
    for a in 2..=33u32 {
        cfg[a as usize] = gen_cfg_word(g, a);
    }
    // the bridge at the middle of its span, a little off, maybe raised
    cfg[3] = (g.below(5) as f32 - 2.0).to_bits();
    cfg[4] = (g.below(5) as f32 - 2.0).to_bits();
    cfg[5] = (*pick(g, &[&0.0f32, &0.0, &3.0])).to_bits();
    let n_ops = 1 + g.below(50) as usize;
    let mut ops = Vec::new();
    let mut frame = cfg[0];
    while ops.len() < n_ops {
        let amount = |g: &mut Gen| (*pick(g, &[&10.0f32, &50.0, &1.0, &200.0, &0.5])).to_bits();
        let o = match g.below(100) {
            0..=14 => [0, 0, 0, 0, 0],
            15..=22 => {
                frame += *pick(g, &[&1u32, &1, &2, &3, &7]);
                [1, frame, 0, 0, 0]
            }
            23..=30 => {
                let a = 2 + g.below(32);
                let a = if (3..=5).contains(&a) { 2 } else { a };
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            31..=37 => [3, amount(g), g.below(12) | g.below(4) << 8, g.below(10), 0],
            38..=41 => [4, amount(g), 0, g.below(10), 0],
            42..=50 => {
                let (a, b) = (g.below(4), g.below(4));
                [5, a, if a == b { (b + 1) % 4 } else { b }, 0, 0]
            }
            51..=54 => [6, 0, 0, 0, 0],
            55..=61 => [7, if g.below(15) == 0 { (*pick(g, &[&-1i32, &4])) as u32 } else { g.below(4) }, g.below(10), 0, 0],
            62..=63 => [8, if g.below(10) == 0 { 4 } else { g.below(4) }, 0, 0, 0],
            64..=70 => [9, 0, 0, 0, 0],
            71..=75 => [10, 0, 0, 0, 0],
            76..=78 => [11, 0, 0, 0, 0],
            79 => [12, 0, 0, 0, 0],
            80..=83 => [13, 0, 0, 0, 0],
            84..=87 => [14, 0, 0, 0, 0],
            88 => [15, 0, 0, 0, 0],
            _ => [16, 0, 0, 0, 0],
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
    damages: u64,
    state_changes: u64,
    deaths: u64,
    scaffolds_made: u64,
    death_effects: u64,
    area_effects: u64,
    falls: u64,
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

fn probe_ref<'a>(w: &'a World, id: ObjectId, name: &[u8]) -> Option<&'a ProbeModule> {
    let idx = w.find_module(id, name)?;
    match w.objects.find(id)?.modules[idx as usize].body.as_ref() {
        Some(UpdateBody::ModuleProbe(p)) => Some(p),
        _ => None,
    }
}

fn template_of(kinds: &[i32]) -> Arc<ThingTemplate> {
    Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: b"R77Bridge".to_vec(), kind_of: kinds.to_vec(), ..Default::default() }))
}

fn byte(w: u32, k: u32) -> f32 {
    (w >> (8 * k) & 255) as u8 as i8 as f32
}

/// A bridge template from six words (the driver's `r77Road`).
fn road(w: &[u32]) -> BridgeTemplateDetails {
    let mut t = BridgeTemplateDetails::default();
    for s in 0..4 {
        for i in 0..3 {
            let k = 2 * (3 * s + i);
            t.damage_to_ocl[s][i] = OCL[(w[0] >> k & 3) as usize].as_bytes().to_vec();
            t.damage_to_fx[s][i] = FX[(w[1] >> k & 3) as usize].as_bytes().to_vec();
            t.repaired_to_ocl[s][i] = OCL[(w[2] >> k & 3) as usize].as_bytes().to_vec();
            t.repaired_to_fx[s][i] = FX[(w[3] >> k & 3) as usize].as_bytes().to_vec();
        }
        t.damage_to_sound[s] = SOUND[(w[4] >> (2 * s) & 3) as usize].as_bytes().to_vec();
        t.repaired_to_sound[s] = SOUND[(w[4] >> (8 + 2 * s) & 3) as usize].as_bytes().to_vec();
    }
    t.transition_effects_height = (w[5] & 255) as f32;
    t.num_fx_per_type = (w[5] >> 8 & 3) as i32;
    t.scaffold_object_name = NAMES[(w[5] >> 10 & 3) as usize].as_bytes().to_vec();
    t.scaffold_support_object_name = NAMES[(w[5] >> 12 & 3) as usize].as_bytes().to_vec();
    t
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32]) {
    for k in 1..=9 as ObjectId {
        let ku = k as usize;
        let mut kinds = Vec::new();
        let mut physics = false;
        let body;
        if k == 1 {
            kinds.push(KINDOF_BRIDGE);
            body = cfg[2] >> 6 & 1 != 0;
            if let Some(p) = probe_mut(w, 1, b"R77Body") {
                p.answers.damage_state = Some((cfg[2] >> 4 & 3) as i32);
            }
        } else if k <= 5 {
            let f = cfg[4 + ku];
            if f & 1 != 0 {
                kinds.push(KINDOF_BRIDGE_TOWER);
            }
            body = f >> 1 & 1 != 0;
        } else {
            let f = cfg[4 + ku];
            if f & 1 != 0 {
                kinds.push(KINDOF_BRIDGE);
            }
            if f >> 1 & 1 != 0 {
                kinds.push(KINDOF_BRIDGE_TOWER);
            }
            physics = f >> 2 & 1 != 0;
            body = f >> 3 & 1 != 0;
            if let Some(p) = probe_mut(w, k, b"R77Physics") {
                p.answers.can_fall = f >> 6 & 1 != 0;
            }
            let pos = Coord3D { x: byte(f, 1), y: byte(f, 2), z: byte(f, 3) };
            w.set_position(k, &pos);
        }
        let body_slot = w.find_module(k, b"R77Body");
        let physics_slot = w.find_module(k, b"R77Physics");
        let o = w.objects.find_mut(k).unwrap();
        o.thing.friend_set_template(template_of(&kinds));
        o.body = if body { body_slot } else { None };
        o.physics = if physics { physics_slot } else { None };
        if k == 1 {
            o.team = cfg[2] & 3;
        }
        if k >= 6 {
            o.layer = (cfg[4 + ku] >> 4 & 3) as i32;
        }
    }
    let b = cfg[14];
    let corner = |i: usize| Coord3D { x: byte(cfg[15 + i], 0), y: byte(cfg[15 + i], 1), z: byte(cfg[15 + i], 2) };
    let details = BridgeDetails {
        from_left: corner(0),
        from_right: corner(1),
        to_left: corner(2),
        to_right: corner(3),
        template_name: ROAD[(b >> 1 & 3) as usize].as_bytes().to_vec(),
        layer: (b >> 3 & 3) as i32,
    };
    for c in 0..8 {
        w.cuts.bridge_details[c] = (b & 1 != 0).then(|| details.clone());
    }
    w.cuts.bridge_templates.clear();
    w.cuts.bridge_templates.insert(ROAD_NAMES[0].as_bytes().to_vec(), road(&cfg[19..25]));
    w.cuts.bridge_templates.insert(ROAD_NAMES[1].as_bytes().to_vec(), road(&cfg[25..31]));
    let g = cfg[31];
    let geoms = [
        GeometryInfo::new(GeometryType::Box, false, SCAFFOLD_HEIGHT[(g >> 2 & 3) as usize], RADIUS[(g & 3) as usize], RADIUS[(g & 3) as usize]),
        GeometryInfo::new(GeometryType::Box, false, SUPPORT_HEIGHT[(g >> 6 & 3) as usize], RADIUS[(g >> 4 & 3) as usize], RADIUS[(g >> 4 & 3) as usize]),
    ];
    let statics = Arc::make_mut(&mut w.statics);
    for (name, geom) in TEMPLATES.iter().zip(geoms) {
        statics.thing_factory.friend_find_template_mut(name.as_bytes()).unwrap().data.friend_set_geometry_info(geom);
    }
    w.cuts.range_scan.clear();
    for i in 0..8 {
        let id = cfg[32] >> (4 * i) & 15;
        if id == 0 {
            break;
        }
        if w.objects.find(id as ObjectId).is_some() {
            w.cuts.range_scan.push(id as ObjectId);
        }
    }
    let bone = (1 as ObjectId, b"BoneA".to_vec());
    if cfg[33] & 1 != 0 {
        w.cuts.bone_transforms.insert(bone, (Coord3D { x: 7.0, y: -3.0, z: 2.0 }, 0.0));
    } else {
        w.cuts.bone_transforms.remove(&bone);
    }
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, fx: &mut FxIds, fx_source: u8, tally: &mut Tally) -> Vec<String> {
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
                Some(CutCall::TerrainFindBridgeAt { position: p }) => log.push(format!("bridgeat {:08x} {:08x} {:08x}", b(p.x), b(p.y), b(p.z))),
                Some(CutCall::BridgeSetBridgeObjectId { id, .. }) => log.push(format!("setbridgeid {id}")),
                Some(CutCall::BridgeSetTowerObjectId { id, tower, .. }) => log.push(format!("settowerid {id} {tower}")),
                Some(CutCall::TerrainUpdateBridgeDamageStates) => log.push("bridgestates".to_string()),
                Some(CutCall::RadarQueueTerrainRefresh) => log.push("radarqueue".to_string()),
                Some(CutCall::PathfinderChangeBridgeState { layer, repaired }) => log.push(format!("bridgestate {layer} {}", i32::from(repaired))),
                Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, dc }) => {
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 0 {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::OclCreate { ocl, .. }) => {
                    tally.death_effects += 1;
                    log.push(format!("ocl {}", ocl_code(&ocl) - 1))
                }
                Some(CutCall::OclCreateAtPrimary { ocl, primary_pos: p, angle, .. }) => {
                    tally.area_effects += 1;
                    log.push(format!("oclat {} {:08x},{:08x},{:08x} null {:08x}", ocl_code(&ocl) - 1, b(p.x), b(p.y), b(p.z), b(angle)))
                }
                // (a sunk scaffold destroys itself: the driver's lines leave those out)
                Some(CutCall::Body(rts_lockstep_engine::logic::world::body_cuts::BodyCall::DestroyObject { .. })) => {}
                // the World's own bookkeeping of a new object, a move and a change of layer (the stand-ins have none)
                Some(
                    CutCall::PartitionMakeDirty { .. }
                    | CutCall::PathfinderUpdatePos { .. }
                    | CutCall::PathfinderRemovePos { .. }
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
                Some(ObjectClientEvent::FxListDoFxPos { fx: name, position: p }) => {
                    tally.area_effects += 1;
                    log.push(format!("fxpos {} {},{},{}", fx.id(fx_source, &Some(name)), b(p.x), b(p.y), b(p.z)))
                }
                // (a new object's drawable and team: World::new_object's)
                Some(ObjectClientEvent::DrawableModelCondition { object_id, .. } | ObjectClientEvent::ObjectChangedTeam { object_id, .. }) if object_id > 9 => {}
                Some(e) => log.push(format!("unexpected {e:?}")),
                None => {}
            }
        } else if t.starts_with("damage ") || t.starts_with("heal ") {
            log.push(t);
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::BridgeBehavior(d) = &data else { unreachable!() };
    put_u32(&mut v, d.lateral_scaffold_speed.to_bits());
    put_u32(&mut v, d.vertical_scaffold_speed.to_bits());
    put_u32(&mut v, d.fx.len() as u32);
    for e in &d.fx {
        put_u32(&mut v, fx.id(0, &e.list));
        put_u32(&mut v, e.time_and_location_info.delay);
        put_str(&mut v, &e.time_and_location_info.bone_name);
    }
    put_u32(&mut v, d.ocl.len() as u32);
    for e in &d.ocl {
        put_u32(&mut v, e.list.as_deref().map_or(0, ocl_code));
        put_u32(&mut v, e.time_and_location_info.delay);
        put_str(&mut v, &e.time_and_location_info.bone_name);
    }

    let mut w = World::new();
    w.names = ctx.names;
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    let mut factory = ThingFactory::new();
    for name in TEMPLATES {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let info = factory.friend_find_template_mut(name.as_bytes()).unwrap().data.friend_behavior_module_info_mut();
        let scaffold = ModuleInfoData::Logic(ModuleDataKind::Update(Arc::new(UpdateModuleData::new())));
        info.add_module_info(
            b"BridgeScaffoldBehavior",
            b"ModuleTag_Scaffold",
            scaffold,
            rts_lockstep_engine::logic::modules::bridge_scaffold_behavior::BridgeScaffoldBehavior::get_interface_mask(),
            false,
            false,
        );
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, object_creation_list_store: ocls, ..Default::default() });
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[1]);
    w.random.init_game_logic_random(cfg[1]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.players.set_player_count(3);
    for tm in 1..=3u32 {
        w.teams.insert_team(Team::new(tm, 0)).unwrap();
        w.cuts.team_players.insert(tm, (tm - 1) as i32);
        w.cuts.player_default_teams.insert((tm - 1) as i32, tm);
    }
    let probe = |name: &[u8], max: f32, interfaces: i32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        let mut p = ProbeModule::new(ProbeAnswers { no_death: true, ..Default::default() }, max, Default::default());
        p.health = max * 0.5;
        m.body = Some(UpdateBody::ModuleProbe(p));
        m
    };
    for k in 1..=9 as ObjectId {
        let mut modules = Vec::new();
        if k == 1 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"BridgeBehavior", b"ModuleTag_Bridge", &data).unwrap().unwrap());
        }
        let max_idx = match k {
            1 => cfg[2] >> 2 & 3,
            2..=5 => cfg[4 + k as usize] >> 2 & 3,
            _ => 0,
        };
        modules.push(probe(b"R77Body", MAX[max_idx as usize], MODULEINTERFACE_BODY));
        if k >= 6 {
            modules.push(probe(b"R77Physics", 0.0, 0));
        }
        let id = w.new_object(NewObject { template: template_of(&[]), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k);
    }
    let me: ObjectId = 1;
    w.set_position(me, &Coord3D { x: f32::from_bits(cfg[3]), y: f32::from_bits(cfg[4]), z: f32::from_bits(cfg[5]) });
    let idx = w.find_module(me, b"BridgeBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    let put_state = |v: &mut Vec<u8>, w: &World, fx: &mut FxIds| {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::BridgeBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        for x in m.state_words() {
            put_u32(v, x);
        }
        for (lists, sound, object) in m.resolved_effects() {
            for l in &lists[..3] {
                put_u32(v, l.as_deref().map_or(0, ocl_code));
            }
            for l in &lists[3..] {
                put_u32(v, fx.id(1, l));
            }
            put_str(v, &sound);
            put_u32(v, object as u32);
        }
        for k in 1..=9 as ObjectId {
            let o = w.objects.find(k).unwrap();
            put_u32(v, probe_ref(w, k, b"R77Body").map_or(0, |p| p.health.to_bits()));
            put_u32(v, o.layer as u32);
            put_u32(v, match o.physics {
                Some(_) => u32::from(probe_ref(w, k, b"R77Physics").is_some_and(|p| p.answers.can_fall)),
                None => 2,
            });
        }
        let made: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 9).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        put_u32(v, made.len() as u32);
        for id in made {
            let o = w.objects.find(id).unwrap();
            let name = o.template().get_name().to_vec();
            put_u32(v, TEMPLATES.iter().position(|t| t.as_bytes() == name.as_slice()).map_or(u32::MAX, |p| p as u32));
            put_u32(v, o.team);
            let p = o.get_position();
            for x in [p.x, p.y, p.z, o.thing.orientation()] {
                put_u32(v, x.to_bits());
            }
            let words = w.find_module(id, b"BridgeScaffoldBehavior").and_then(|i| match o.behaviors[i as usize].body.as_ref() {
                Some(UpdateBody::BridgeScaffoldBehavior(s)) => Some(s.state_words()),
                _ => None,
            });
            for x in words.unwrap_or([0; 15]) {
                put_u32(v, x);
            }
        }
    };
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (2..=33).contains(&o[1]) && !(3..=5).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg);
        w.events.drain();
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let call = |w: &mut World, c: ModuleCall| w.module_call(me, idx, c);
        let info = |o: &[u32; 5]| {
            let mut d = DamageInfo::default();
            d.input.amount = f32::from_bits(o[1]);
            d.input.damage_type = if o[0] == 4 { DAMAGE_HEALING } else { (o[2] & 255) as i32 };
            d.input.death_type = if o[0] == 4 { DEATH_NONE } else { (o[2] >> 8) as i32 };
            d.input.source_id = o[3] as ObjectId;
            d
        };
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                t.damages += 1;
                call(&mut w, ModuleCall::OnDamage(info(o)));
            }
            4 => {
                call(&mut w, ModuleCall::OnHealing(info(o)));
            }
            5 => {
                t.state_changes += 1;
                call(&mut w, ModuleCall::OnBodyDamageStateChange { info: DamageInfo::default(), old_state: o[1] as i32, new_state: o[2] as i32 });
            }
            6 => {
                t.deaths += 1;
                call(&mut w, ModuleCall::OnDie(DamageInfo::default()));
            }
            7 => {
                call(&mut w, ModuleCall::BridgeSetTower { tower: o[1] as i32, object: o[2] as ObjectId });
            }
            8 => {
                res = call(&mut w, ModuleCall::GetBridgeTowerId { tower: o[1] as i32 }).as_id() as u32;
            }
            9 => {
                let before = w.objects.ids_in_order().len();
                call(&mut w, ModuleCall::BridgeCreateScaffolding);
                t.scaffolds_made += (w.objects.ids_in_order().len() - before) as u64;
            }
            10 => {
                call(&mut w, ModuleCall::BridgeRemoveScaffolding);
            }
            11 => {
                let a = u32::from(call(&mut w, ModuleCall::BridgeIsScaffoldInMotion).as_bool());
                let b2 = u32::from(call(&mut w, ModuleCall::BridgeIsScaffoldPresent).as_bool());
                res = a | b2 << 1;
            }
            12 => {
                call(&mut w, ModuleCall::OnDelete);
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
            16 => {
                let mut made: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 9).collect();
                made.sort_unstable();
                for id in made {
                    let Some(i) = w.find_module(id, b"BridgeScaffoldBehavior") else { continue };
                    let Some(su) = w.update_handle_of_module(ModuleHandle { object: id, index: i }) else { continue };
                    w.with_body(su, |b, w| b.update(w, su));
                }
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
        // (an update plays the data's FX lists, a state change the template's)
        let log = effects(&mut w, &mut fx, u8::from(o[0] != 0), &mut t);
        t.falls += log.iter().filter(|l| l.starts_with("damage ")).count() as u64;
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
        put_state(&mut v, &w, &mut fx);
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
fn bridge_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R77 BridgeBehavior",
        seed: 0x0077_B41D,
        cases_var: "L9R77_CASES",
        ini_stem: "Br",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r77_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R77: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers ({} loads failed), {} effect lines \
         ({} damages, {} state changes, {} deaths, {} scaffold objects made, {} death OCLs from the parent, {} area effects, {} damage lines)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.load_failures, t.effects, t.damages, t.state_changes, t.deaths,
        t.scaffolds_made, t.death_effects, t.area_effects, t.falls
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.scaffolds_made > 0 && t.area_effects > 0 && t.deaths > 0, "the generator must reach every effect");
}
