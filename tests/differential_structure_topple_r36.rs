#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureToppleUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `StructureToppleUpdate` (`logic::modules::structure_topple_update`) against the unchanged C++, `p1w13-sonnet`, batch R36.
//!
//! # Method
//!
//! The C++ side is `StructureToppleUpdate.cpp` (its crushing loop's `j` declared before the loop by `build.rs`, as MSVC's old `for` scope let it
//! be read after it), compiled into `wrap_module.cpp`'s unit (`differential/l9_structure_topple_r36.inc`, `ref_l9r36_run`), on the module
//! build's stand-ins: the building (id 1: maybe a body whose last damage is none or the test's and whose `updateBodyParticleSystems` is a line,
//! maybe a drawable whose model condition calls are lines, maybe an AI whose `markAsDead` is a line, maybe a `BoneFXUpdate` whose
//! `stopAllBoneFX` is a line; a veterancy, status bits, a name, a position, an orientation, the transform `Thing` keeps, a box geometry), the
//! attacker (id 2), the script's topple directions, the terrain's eight cells, the FX lists, the object creation lists, the weapon store
//! (`createAndFireTempWeapon` a line), the game logic, whose wake calls are applied as `GameLogic::friend_awakenUpdateModule` applies them, and
//! the REAL `UpdateModule`, `DieMuxData`, `INI`, `RandomValue` (the logic's and the client's) and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` body, AI and
//! `BoneFXUpdate`), the real `Thing` transform, an FX-free object creation list store with `OCL_A` and `OCL_B`, a weapon store with `WEAP_A`
//! and `WEAP_B`, `cuts.script_topple_directions`, `cuts.terrain_ground`; every op an `update()` as the scheduler runs it, a death, a change to
//! the scene or the frame, or an xfer.
//!
//! Not compared (upstream undefined, and the 64-bit reference build differs from the shipped game): a death of `DEATH_NORMAL` and a building of
//! `LEVEL_REGULAR`, whose die mux tests shift by -1 (as in R29).
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 400 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's effects and wake calls; per op the throw status, the result, the effects in order (the deselection, the AI, the FX, the object
//! creation lists, the crushing weapon and the ground it reads, the model conditions, the body's particle systems, the bone FX), the wake calls,
//! the xfer status, bytes and CRC, the state, the building's transform, position and orientation.
//!
//! # Results (2026-10-24, `L9R36_CASES=9000`)
//!
//! 9,000 cases (5,332 compared; 2,319 data blocks threw, as the C++ did; 1,349 excluded, undefined upstream: 1,016 a building with no body,
//! read unchecked for its last damage, 333 the end of the fall with no drawable), 1,006,130 ops (701,507 updates, 29,360 deaths, 100,233 xfers)
//! and 2,022,924 outputs: **0 mismatches**; 272,573 effect lines compared (the compared cases'): 28,636 crushing weapons fired, 68,265 FX and
//! 52,210 object creation lists.
//!
//! On the way: the first C++ runs crashed in `doToppleDoneStuff`: R30's driver had renamed `BoneFXUpdate` by macro for the rest of the unit,
//! so the stand-in pointer the collapse driver's `findUpdateModule` gives was taken for R30's real class; the renames now end with R30's
//! driver. Then the C++ toppled nothing at `LEVEL_REGULAR` (the die mux's shift by -1, as R29 found), which the generator now leaves out.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 10 caught. (Measured with an orientation of 1.5708 among the generated ones,
//! since 1.57 for clippy's `approx_constant`; the counts are those runs'.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2061 | the burst point at 95% of the radius, not 90% | 1,007,188 |
//! | 2062 | an acceleration factor of 0.03, not 0.02 | 525,857 |
//! | 2063 | the structural integrity never decays | 405,569 |
//! | 2064 | an angle FX also at an angle equal to the current one | **survived** |
//! | 2065 | the crushing rows 20 apart, not 25 | 64,985 |
//! | 2066 | the last crushed location the reach, not the loop's `j` | 66,961 |
//! | 2067 | the first burst delay from the logic's random values | 867,426 |
//! | 2068 | the rubble not set upright before the turn | 161,810 |
//! | 2069 | the script's topple direction ignored | 196,138 |
//! | 2070 | an angle FX at every angle passed so far (no lower bound) | 10,406 |
//!
//! 2064 survives because it changes the test only at an FX angle exactly equal to the accumulated angle, which these runs never reach (the
//! angle starts at 0.001 and grows by computed velocities; the FX angles are whole and half degrees). It is left as a survivor, not replaced;
//! 2070 shows the angle test's lower bound is covered.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
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
use rts_lockstep_engine::logic::modules::structure_topple_update::{StructureToppleUpdateModuleData, ST_PHASE_COUNT};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r36.bin");
const XFER_FILE: &str = "l9_r36.bin";

/// The ground heights a cell can have, by nibble (the driver's `g_r36Grounds`).
const GROUNDS: [f32; 8] = [0.0, 2.0, 5.5, -3.0, 12.0, 1.25, 30.0, -0.5];

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
    *out = Some(ModuleDataKind::StructureToppleUpdate(Arc::new(StructureToppleUpdateModuleData::friend_new_module_data(Some(ini))?)));
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

fn put_data(v: &mut Vec<u8>, d: &StructureToppleUpdateModuleData, fx: &mut FxIds) {
    for x in [d.min_topple_delay, d.max_topple_delay, d.min_topple_burst_delay, d.max_topple_burst_delay] {
        put_i32(v, x);
    }
    put_u32(v, d.structural_integrity.to_bits());
    put_u32(v, d.structural_decay.to_bits());
    let mut w = [0u32; 2];
    for t in 0..rts_lockstep_engine::logic::damage::DAMAGE_NUM_TYPES.min(64) {
        if d.damage_fx_types.test(t as i32) {
            w[t / 32] |= 1 << (t % 32);
        }
    }
    put_u32(v, w[0]);
    put_u32(v, w[1]);
    for f in [&d.topple_start_fx_list, &d.topple_delay_fx_list, &d.topple_fx_list, &d.topple_done_fx_list, &d.crushing_fx_list] {
        put_i32(v, fx.id(f));
    }
    put_str(v, &d.crushing_weapon_name);
    for ph in 0..ST_PHASE_COUNT {
        put_u32(v, d.ocls[ph].len() as u32);
        for o in &d.ocls[ph] {
            put_i32(v, ocl_code(o));
        }
        put_u32(v, d.ocl_count[ph]);
    }
    put_u32(v, d.angle_fx.len() as u32);
    for a in &d.angle_fx {
        put_u32(v, a.angle.to_bits());
        put_i32(v, fx.id(&a.fx_list));
    }
    // `fxbones`: no INI field fills it
    put_u32(v, 0);
}

/// The words of the state the dump holds after the xfer: the module's 13, the transform's 12, the position and the orientation.
const STATE_WORDS: usize = 13 + 12 + 4;

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
    let head: Vec<u32> = (0..13).map(|_| u32_(&mut p)).collect();
    let weapon = str_(&mut p);
    let mut phases = Vec::new();
    for _ in 0..ST_PHASE_COUNT {
        let n = u32_(&mut p);
        let codes: Vec<u32> = (0..n).map(|_| u32_(&mut p)).collect();
        phases.push((codes, u32_(&mut p)));
    }
    let na = u32_(&mut p);
    let angles: Vec<(u32, u32)> = (0..na).map(|_| (u32_(&mut p), u32_(&mut p))).collect();
    let bones = u32_(&mut p);
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} {weapon:?} {phases:?} {angles:x?} {bones} constructor {log:?} wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..STATE_WORDS).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let duration = |g: &mut Gen| pick(g, &["0", "33", "100", "300", "1000"]).to_string();
    for f in ["MinToppleDelay", "MaxToppleDelay", "MinToppleBurstDelay", "MaxToppleBurstDelay"] {
        if g.below(100) < 70 {
            let d = duration(g);
            lines.push(format!("{f} = {d}"));
        }
    }
    if g.below(100) < 70 {
        lines.push(format!("StructuralIntegrity = {}", pick(g, &["0.1", "0.5", "0.9", "0", "1.5"])));
    }
    if g.below(100) < 70 {
        lines.push(format!("StructuralDecay = {}", pick(g, &["0", "0.5", "0.95", "1"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("DamageFXTypes = {}", pick(g, &["ALL", "NONE", "EXPLOSION", "ALL -EXPLOSION"])));
    }
    for f in ["TopplingFX", "ToppleDelayFX", "ToppleStartFX", "ToppleDoneFX", "CrushingFX"] {
        if g.below(100) < 50 {
            let n = pick(g, &["FX_A", "FX_B", "None"]);
            lines.push(format!("{f} = {n}"));
        }
    }
    if g.below(100) < 70 {
        lines.push(format!("CrushingWeaponName = {}", pick(g, &["WEAP_A", "WEAP_B", "NoSuchWeapon"])));
    }
    for _ in 0..g.below(4) {
        let phase = pick(g, &["INITIAL", "DELAY", "FINAL"]);
        let n = 1 + g.below(3);
        let names: Vec<&str> = (0..n).map(|_| *pick(g, &[&"OCL_A", &"OCL_B", &"None"])).collect();
        lines.push(format!("OCL = {phase} {}", names.join(" ")));
    }
    for _ in 0..g.below(4) {
        let angle = pick(g, &["10", "45", "60", "80", "89.5"]);
        let fx = pick(g, &["FX_A", "FX_B", "None"]);
        lines.push(format!("AngleFX = {angle} {fx}"));
    }
    if g.below(100) < 15 {
        lines.push(format!("DeathTypes = {}", pick(g, &["ALL", "NONE +NORMAL", "ALL -CRUSHED"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "OCL = MIDDLE OCL_A", "AngleFX = x FX_A", "StructuralDecay = y", "MinToppleDelay"]).to_string());
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
    updates: u64,
    deaths: u64,
    xfers: u64,
    effects: u64,
    weapons: u64,
    fx: u64,
    ocls: u64,
    falls_done: u64,
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
            b.truncate(512);
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

fn veterancy(i: u32) -> VeterancyLevel {
    match i {
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        3 => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
}

/// The slots of the probes of object 1 (`None`: the object was made without it).
struct Slots {
    body: Option<u16>,
    ai: Option<u16>,
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 20], s: &Slots) {
    let f = cfg[3];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.drawable = if f & 1 != 0 { 7 } else { INVALID_DRAWABLE_ID };
        o.ai = if f >> 1 & 1 != 0 { s.ai } else { None };
        o.body = if f >> 2 & 1 != 0 { s.body } else { None };
        o.name = if f >> 5 & 1 != 0 { b"Tower1".to_vec() } else { Vec::new() };
        o.experience.level = veterancy(cfg[4]);
        o.status = Default::default();
        for b in 0..8 {
            if cfg[5] >> b & 1 != 0 {
                o.status.set(b, 1);
            }
        }
    }
    if let Some(b) = s.body {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(1).unwrap().modules[b as usize].body.as_mut() {
            p.answers.last_damage = (f >> 4 & 1 != 0).then_some((0, cfg[14] as i32));
        }
    }
    w.set_position(2, &Coord3D { x: f32::from_bits(cfg[12]), y: f32::from_bits(cfg[13]), z: 0.0 });
    w.cuts.script_topple_directions.clear();
    if cfg[15] != 0 {
        let a = (cfg[15] % 8) as f32 * (2.0 * rts_lockstep_engine::common::base_type::PI / 8.0);
        w.cuts.script_topple_directions.push((b"Tower1".to_vec(), Coord3D { x: 3.0 * a.cos(), y: 3.0 * a.sin(), z: 0.0 }));
    }
    for c in 0..8 {
        w.cuts.terrain_ground[c] = GROUNDS[(cfg[16] >> (4 * c) & 7) as usize];
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 20], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::StructureToppleUpdate(d) = &data else { unreachable!() };
    put_data(&mut v, d, &mut fx);
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    let mut scratch = rts_lockstep_engine::common::name_key_generator::NameKeyGenerator::new();
    scratch.reset();
    for n in ["WEAP_A", "WEAP_B"] {
        weapon_store.new_weapon_template(&mut scratch, n.as_bytes()).unwrap();
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), object_creation_list_store: ocls, weapon_store, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"StructureToppleUpdate", b"ModuleTag_Topple", &data).unwrap().unwrap();
    let probe = |name: &[u8]| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 100.0, Default::default())));
        pm
    };
    let mut modules = vec![m];
    let mut b = probe(b"R36Body");
    b.interfaces = MODULEINTERFACE_BODY;
    modules.push(b);
    let mut a = probe(b"R36Ai");
    a.is_ai = true;
    modules.push(a);
    if cfg[3] >> 3 & 1 != 0 {
        modules.push(probe(b"BoneFXUpdate"));
    }
    let geom = GeometryInfo::new(GeometryType::Box, false, f32::from_bits(cfg[9]), f32::from_bits(cfg[10]), f32::from_bits(cfg[11]));
    let values = ObjectTemplateValues { geometry: Some(geom), ..Default::default() };
    let tt = Arc::new(ThingTemplate::from_object_values(&values));
    // the pose before the constructor (the driver poses the building first)
    assert_eq!(w.new_object(NewObject { template: tt.clone(), status: Default::default(), team: None, modules }).unwrap(), 1);
    assert_eq!(w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules: vec![] }).unwrap(), 2);
    let slots = Slots { body: w.find_module(1, b"R36Body"), ai: w.find_module(1, b"R36Ai") };
    let idx = w.find_module(1, b"StructureToppleUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let effects = |w: &mut World, events_before: usize, fx: &mut FxIds, tally: &mut Tally| -> (Vec<String>, u32, u32) {
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
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::GameLogicDeselectObject { object, player_mask, affect_client }) => log.push(format!("deselect {object} {player_mask} {}", i32::from(affect_client))),
                    Some(CutCall::OclCreateAtPrimary { ocl, primary_pos: p, angle, .. }) => {
                        tally.ocls += 1;
                        log.push(format!("oclat {} {:08x},{:08x},{:08x} null {:08x}", ocl_code(&ocl) - 1, b(p.x), b(p.y), b(p.z), b(angle)))
                    }
                    Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) => {
                        tally.weapons += 1;
                        log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(a), .. }) => log.push(format!("mc {}", a[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(a), .. }) => log.push(format!("mcclear {}", a[0])),
                    Some(ObjectClientEvent::FxListDoFxPos { fx: name, position: p }) => {
                        tally.fx += 1;
                        log.push(format!("fxpos {} {},{},{}", fx.id(&Some(name)), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => {
                        tally.fx += 1;
                        log.push(format!("fx {}", fx.id(&Some(name))))
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
        (log, wakes, last)
    };
    // the pose (position, orientation; the transform from them) and the scene
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[6]), y: f32::from_bits(cfg[7]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[8]));
    set_scene(&mut w, &cfg, &slots);
    {
        let (log, cw, cl) = effects(&mut w, 0, &mut fx, tally);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, cw);
        put_u32(&mut v, cl);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 3 && ((3..=5).contains(&o[1]) || (12..=17).contains(&o[1])) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
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
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as i32;
                info.input.source_id = o[2] as ObjectId;
                w.module_call(1, idx, ModuleCall::OnDie(info));
                tally.deaths += 1;
            }
            4..=6 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 4, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 4 {
                    saved = xbytes.clone();
                }
                if o[0] == 5 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            2 | 3 => {}
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
        let (log, wakes, last) = effects(&mut w, events_before, &mut fx, tally);
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
        let Some(UpdateBody::StructureToppleUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let words = m.state_words();
        if o[0] == 0 && words[3] == 4 && res != 0 {
            tally.falls_done += 1;
        }
        for s in words {
            put_u32(&mut v, s);
        }
        let mx = *ob.thing.transform_matrix();
        for r in 0..3 {
            for c in [mx[r].x, mx[r].y, mx[r].z, mx[r].w] {
                put_u32(&mut v, c.to_bits());
            }
        }
        let pos = ob.get_position();
        for c in [pos.x, pos.y, pos.z, ob.thing.orientation()] {
            put_u32(&mut v, c.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R36_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn structure_topple_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0036_7091);
    let mut read = Check::new("R36 data read (throw, message)");
    let mut run = Check::new("R36 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let gen_flags = |g: &mut Gen, bone: u32| -> u32 {
        u32::from(g.below(8) != 0) | u32::from(g.below(2) == 0) << 1 | u32::from(g.below(15) != 0) << 2 | bone << 3 | u32::from(g.below(2) == 0) << 4 | u32::from(g.below(3) == 0) << 5
    };
    let gen_cells = |g: &mut Gen| (0..8).fold(0, |acc, k| acc | g.below(8) << (4 * k));
    let coords = [0.0f32, 15.0, -22.5, 40.0, 8.25, -60.0];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R36{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 20];
        cfg[0] = g.below(60);
        cfg[1] = g.u32();
        cfg[2] = g.u32();
        let bone = u32::from(g.below(2) == 0);
        cfg[3] = gen_flags(&mut g, bone);
        // (not `LEVEL_REGULAR`: `getVeterancyLevelFlag`'s shift by -1, as for `DEATH_NORMAL` below; see `common::game_common::veterancy_bit`)
        cfg[4] = 1 + g.below(3);
        cfg[5] = if g.below(4) == 0 { g.below(256) } else { 0 };
        cfg[6] = real(&mut g, &coords);
        cfg[7] = real(&mut g, &coords);
        cfg[8] = real(&mut g, &[0.0, 0.5, -1.2, 3.0, 1.57]);
        cfg[9] = real(&mut g, &[20.0, 50.0, 120.0, 0.0]);
        cfg[10] = real(&mut g, &[10.0, 30.0, 80.0]);
        cfg[11] = real(&mut g, &[10.0, 20.0, 45.0]);
        cfg[12] = real(&mut g, &coords);
        cfg[13] = real(&mut g, &coords);
        cfg[14] = g.below(12);
        cfg[15] = if g.below(4) == 0 { 1 + g.below(8) } else { 0 };
        cfg[16] = gen_cells(&mut g);
        let n_ops = 1 + g.below(400) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=27 => {
                    // mostly a frame on, then an update
                    [0, 0, 0, 0]
                }
                28..=31 => {
                    frame += 1 + g.below(3);
                    [2, frame, 0, 0]
                }
                32 => [1, 1 + g.below(11), if g.below(3) == 0 { 0 } else { 2 }, 0],
                33..=35 => {
                    let a = *pick(&mut g, &[&3u32, &4, &5, &12, &13, &14, &15, &16]);
                    let b = match a {
                        3 => gen_flags(&mut g, bone),
                        4 => 1 + g.below(3),
                        5 => g.below(256),
                        12 | 13 => real(&mut g, &coords),
                        14 => g.below(12),
                        15 => g.below(9),
                        _ => gen_cells(&mut g),
                    };
                    [3, a, b, 0]
                }
                36..=37 => [4, 0, 0, 0],
                38 => [5, 0, 0, 0],
                _ => [6, 0, 0, 0],
            })
            .collect();
        // a death early, usually
        let mut ops = ops;
        if g.below(5) != 0 && !ops.is_empty() {
            let k = g.below(ops.len().min(10) as u32) as usize;
            ops[k] = [1, 1 + g.below(11), if g.below(3) == 0 { 0 } else { 2 }, 0];
        }
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
            cpp::ref_l9r36_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R36: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} deaths, {} xfers), \
         {} effect lines ({} crushing weapons, {} FX, {} object creation lists; {} updates after the fall; the compared cases')",
        tally.updates, tally.deaths, tally.xfers, tally.effects, tally.weapons, tally.fx, tally.ocls, tally.falls_done
    );
    println!("R36 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.weapons > cases as u64 && tally.fx > cases as u64 && tally.ocls > cases as u64 / 2 && tally.falls_done * 10 > cases as u64, "the generator must reach every effect");
}
