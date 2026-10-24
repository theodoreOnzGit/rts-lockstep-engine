#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `StructureCollapseUpdate` (`logic::modules::structure_collapse_update`) against the unchanged C++, `p1w13-sonnet`, batch R29.
//!
//! # Method
//!
//! The C++ side is `StructureCollapseUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_collapse_r29.inc`,
//! `ref_l9r29_run`), on the module build's stand-ins: the building (id 1) with maybe a body (its `updateBodyParticleSystems` a line), a drawable
//! (its model condition calls and the translation of its instance matrix lines), an AI (`markAsDead` a line), a `BoneFXUpdate` (`stopAllBoneFX` a
//! line), a veterancy, status bits, a position and an orientation, and a template of the test's height; the FX list store (`FX_A`, `FX_B`), the
//! object creation list store (`OCL_A`, `OCL_B`), the global data's gravity, `deselectObject` (a line); the REAL `UpdateModule`, `DieMuxData`,
//! `INI`, random values (the logic's and the client's) and `Xfer`; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same object (the module from `modules::new_module`, `ProbeModule`s for the body, the AI and
//! the `BoneFXUpdate`, a template with a box geometry of that height), the same two random seeds, an OCL store with the two lists,
//! `cuts.globals.gravity`; every op an `update()` as the scheduler runs it, an `onDie` of a death of the op's type, or a change to the scene.
//!
//! Each case reads a block `D` (every field; mostly well formed) and runs 1 to 60 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's wake calls; per op the throw status, the result, the effects in order (the deselection, the AI, the FX and OCL of every phase with
//! their position, the shudder's translations, the model conditions, the bone FX, the body's particle systems), the wake calls, the xfer status,
//! bytes and CRC, and the state.
//!
//! Not compared (upstream undefined, and the 64-bit reference build differs from the shipped game): a death of `DEATH_NORMAL` and a building of
//! `LEVEL_REGULAR`, whose die mux tests shift by -1 (`logic::damage::death_bit`, `common::game_common::veterancy_bit` pin the shipped answer).
//!
//! # Results (2026-10-24, `L9R29_CASES=9000`)
//!
//! 9,000 cases (6,205 compared; 1,826 data blocks threw, as the C++ did; 969 excluded, undefined upstream: 641 a collapse with no drawable, 179
//! `MaxBurstDelay` read uninitialised, 149 a building down with no body), 183,290 ops (~~109,259~~ **91,528** updates, ~~18,119~~ **14,942** deaths,
//! ~~46,015~~ **38,655** xfers) and 195,700 outputs: **0 mismatches**; ~~94,505~~ **80,744** effect lines compared, among them ~~13,291~~ **10,438** FX
//! lists and OCLs of the phases, ~~1,195~~ **1,038** bone FX stops and ~~2,116~~ **2,057** body particle updates; ~~74,745~~ **56,377** ops ended
//! waiting, ~~27,746~~ **22,606** collapsing and ~~17,284~~ **14,738** done.
//! **CORRECTED 2026-10-24** (the same day): the first record's counts of updates, xfers and effects also counted the cases later excluded as
//! undefined upstream, whose outputs are not compared; the counts below are the compared cases' only (the output and mismatch counts were right).
//!
//! On the way: the shudder's two client random numbers are the arguments of one call, whose order C++ leaves unspecified; the reference build
//! (clang) draws them left to right, and the first translation had them the other way round.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1991 | the AI not marked dead | 6,018 |
//! | 1992 | the collapse delay is always the minimum | 82,046 |
//! | 1993 | the damping ignored | 20,852 |
//! | 1994 | the bone FX not stopped | 1,038 |
//! | 1995 | big bursts and delays swapped | 5,455 |
//! | 1996 | the next burst counted from now | 6,918 |
//! | 1997 | the height not saved | 23,020 |
//! | 1998 | the shudder's `x` and `y` swapped | 15,045 |
//! | 1999 | the `BURST` phase plays at the death | 45,725 |

mod support;
mod up_common;

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
use rts_lockstep_engine::logic::modules::structure_collapse_update::{
    StructureCollapseUpdateModuleData, SC_PHASE_COUNT,
};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r29.bin");
const XFER_FILE: &str = "l9_r29.bin";

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
    *out = Some(ModuleDataKind::StructureCollapseUpdate(Arc::new(
        StructureCollapseUpdateModuleData::friend_new_module_data(Some(ini))?,
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

fn put_data(v: &mut Vec<u8>, d: &StructureCollapseUpdateModuleData, fx: &mut FxIds) {
    for x in [
        d.min_collapse_delay,
        d.max_collapse_delay,
        d.min_burst_delay,
        d.big_burst_frequency,
    ] {
        put_i32(v, x);
    }
    put_u32(v, d.collapse_damping.to_bits());
    put_u32(v, d.max_shudder.to_bits());
    for ph in 0..SC_PHASE_COUNT {
        put_u32(v, d.fxs[ph].len() as u32);
        for f in &d.fxs[ph] {
            put_i32(v, fx.id(f));
        }
        put_u32(v, d.ocls[ph].len() as u32);
        for o in &d.ocls[ph] {
            put_i32(v, ocl_code(o));
        }
        put_u32(v, d.fx_count[ph]);
        put_u32(v, d.ocl_count[ph]);
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
    let head: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    let mut lists = Vec::new();
    for _ in 0..SC_PHASE_COUNT {
        let n = u32_(&mut p);
        let fx: Vec<u32> = (0..n).map(|_| u32_(&mut p)).collect();
        let n = u32_(&mut p);
        let ocl: Vec<u32> = (0..n).map(|_| u32_(&mut p)).collect();
        let counts = (u32_(&mut p), u32_(&mut p));
        lists.push((fx, ocl, counts));
    }
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!(
        "data {head:x?} {lists:?} constructor wakes {w} last {l}"
    ));
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
        let state: Vec<u32> = (0..5).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:x?}"));
    }
    out
}

const DEATH_NAMES: [&str; 20] = [
    "NONE",
    "CRUSHED",
    "BURNED",
    "EXPLODED",
    "POISONED",
    "TOPPLED",
    "FLOODED",
    "SUICIDED",
    "LASERED",
    "DETONATED",
    "SPLATTED",
    "POISONED_BETA",
    "EXTRA_2",
    "EXTRA_3",
    "EXTRA_4",
    "EXTRA_5",
    "EXTRA_6",
    "EXTRA_7",
    "EXTRA_8",
    "POISONED_GAMMA",
];

fn death_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        t.push(match g.below(10) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=6 => format!("+{}", DEATH_NAMES[1 + g.below(19) as usize]),
            _ => format!("-{}", DEATH_NAMES[1 + g.below(19) as usize]),
        });
    }
    t.join(" ")
}

fn level_tokens(g: &mut Gen) -> String {
    let names = ["VETERAN", "ELITE", "HEROIC"];
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        t.push(match g.below(8) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=4 => format!("+{}", names[g.below(3) as usize]),
            _ => format!("-{}", names[g.below(3) as usize]),
        });
    }
    t.join(" ")
}

/// The first eight status names (the object's status bits are the first eight: the die mux can meet them).
fn status_tokens(g: &mut Gen) -> String {
    let names = rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NAMES;
    let mode = g.below(5);
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = names[1 + g.below(7) as usize];
        t.push(match mode {
            0 => format!("-{n}"),
            1 => format!("+{n}"),
            _ => n.to_string(),
        });
    }
    if mode == 4 && g.below(3) == 0 {
        return "NONE".to_string();
    }
    t.join(" ")
}

fn field(lines: &mut Vec<String>, g: &mut Gen, p: u32, name: &str, value: String) {
    if g.below(100) < p {
        lines.push(format!("{name} = {value}"));
    }
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    for name in ["MinCollapseDelay", "MaxCollapseDelay", "MinBurstDelay"] {
        let v = pick(g, &["0", "100", "500", "1500"]).to_string();
        field(&mut lines, g, 60, name, v);
    }
    // the maximum burst delay is read uninitialised when the INI does not give it: mostly given
    let v = pick(g, &["0", "200", "600", "2000"]).to_string();
    field(&mut lines, g, 92, "MaxBurstDelay", v);
    let v = pick(g, &["0", "0.5", "0.9", "1"]).to_string();
    field(&mut lines, g, 60, "CollapseDamping", v);
    let v = pick(g, &["0", "1.5", "4"]).to_string();
    field(&mut lines, g, 60, "MaxShudder", v);
    let v = pick(g, &["0", "1", "3", "8"]).to_string();
    field(&mut lines, g, 60, "BigBurstFrequency", v);
    for ph in ["INITIAL", "DELAY", "BURST", "FINAL"] {
        for (name, names) in [
            ("FXList", &["FX_A", "FX_B", "None"][..]),
            ("OCL", &["OCL_A", "OCL_B", "None", "OCL_C"][..]),
        ] {
            if g.below(100) < 40 {
                let n = 1 + g.below(3);
                let list: Vec<&str> = (0..n).map(|_| pick(g, names)).collect();
                lines.push(format!("{name} = {ph} {}", list.join(" ")));
            }
        }
    }
    let v = death_tokens(g);
    field(&mut lines, g, 20, "DeathTypes", v);
    let v = level_tokens(g);
    field(&mut lines, g, 12, "VeterancyLevels", v);
    let v = status_tokens(g);
    field(&mut lines, g, 10, "ExemptStatus", v);
    let v = status_tokens(g);
    field(&mut lines, g, 10, "RequiredStatus", v);
    if !clean {
        lines.push(
            pick(
                g,
                &[
                    "NoSuchField = 1",
                    "FXList = NOPHASE FX_A",
                    "OCL = BURST",
                    "MaxShudder = x",
                    "DeathTypes = +BOGUS",
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
    effects: u64,
    updates: u64,
    xfers: u64,
    dies: u64,
    waiting: u64,
    collapsing: u64,
    done: u64,
    phase_lines: u64,
    bone_fx_stops: u64,
    body_particles: u64,
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

/// The scene of the configuration (the driver's `scene`); the modules are the case's, the drawable comes and goes.
fn set_scene(w: &mut World, cfg: &[u32; 12]) {
    let o = w.objects.find_mut(1).unwrap();
    o.drawable = if cfg[3] & 1 != 0 {
        7
    } else {
        INVALID_DRAWABLE_ID
    };
    o.experience.level = veterancy(cfg[4]);
    o.status = Default::default();
    for b in 0..8 {
        if cfg[5] >> b & 1 != 0 {
            o.status.set(b, 1);
        }
    }
    let pos = rts_lockstep_engine::common::base_type::Coord3D {
        x: f32::from_bits(cfg[6]),
        y: f32::from_bits(cfg[7]),
        z: f32::from_bits(cfg[8]),
    };
    w.set_position(1, &pos);
    w.set_orientation(1, f32::from_bits(cfg[9]));
    w.cuts.globals.gravity = f32::from_bits(cfg[11]);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    cfg_in: &[u32; 12],
    ops: &[[u32; 4]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::StructureCollapseUpdate(d) = &data else {
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
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"StructureCollapseUpdate",
        b"ModuleTag_Collapse",
        &data,
    )
    .unwrap()
    .unwrap();
    let probe = |name: &[u8]| {
        let mut pm = NewModule::plain(name, b"ModuleTag_Probe");
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            ProbeAnswers::default(),
            100.0,
            Default::default(),
        )));
        pm
    };
    let mut modules = vec![m];
    if cfg[3] >> 2 & 1 != 0 {
        let mut b = probe(b"R29Body");
        b.interfaces = MODULEINTERFACE_BODY;
        modules.push(b);
    }
    if cfg[3] >> 1 & 1 != 0 {
        let mut a = probe(b"R29Ai");
        a.is_ai = true;
        modules.push(a);
    }
    if cfg[3] >> 3 & 1 != 0 {
        modules.push(probe(b"BoneFXUpdate"));
    }
    let values = ObjectTemplateValues {
        geometry: Some(GeometryInfo::new(
            GeometryType::Box,
            false,
            f32::from_bits(cfg[10]),
            10.0,
            10.0,
        )),
        ..Default::default()
    };
    assert_eq!(
        w.new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&values)),
            status: Default::default(),
            team: None,
            modules
        })
        .unwrap(),
        1
    );
    let idx = w.find_module(1, b"StructureCollapseUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: 1,
            index: idx,
        })
        .expect("an update module");
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
    set_scene(&mut w, &cfg);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 3 && (3..12).contains(&o[1]) && o[1] != 10 {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 2 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w
                    .with_body(u, |b, w| b.update(w, u))
                    .expect("the module is in its slot")
                    .0;
                tally.updates += 1;
            }
            1 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as i32;
                w.module_call(1, idx, ModuleCall::OnDie(info));
                tally.dies += 1;
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
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) =
            (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        let b = |x: f32| x.to_bits();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::GameLogicDeselectObject {
                        object,
                        player_mask,
                        affect_client,
                    }) => log.push(format!(
                        "deselect {object} {player_mask} {}",
                        i32::from(affect_client)
                    )),
                    Some(CutCall::OclCreateAtPrimary {
                        ocl,
                        primary_pos: p,
                        angle,
                        ..
                    }) => {
                        tally.phase_lines += 1;
                        log.push(format!(
                            "oclat {} {:08x},{:08x},{:08x} null {:08x}",
                            ocl_code(&ocl) - 1,
                            b(p.x),
                            b(p.y),
                            b(p.z),
                            b(angle)
                        ))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Set(a),
                        ..
                    }) => log.push(format!("mc {}", a[0])),
                    Some(ObjectClientEvent::DrawableModelCondition {
                        op: ModelConditionOp::Clear(a),
                        ..
                    }) => log.push(format!("mcclear {}", a[0])),
                    Some(ObjectClientEvent::FxListDoFxPos {
                        fx: name,
                        position: p,
                    }) => {
                        tally.phase_lines += 1;
                        log.push(format!(
                            "fxpos {} {},{},{}",
                            fx.id(&Some(name)),
                            b(p.x),
                            b(p.y),
                            b(p.z)
                        ))
                    }
                    Some(ObjectClientEvent::DrawableSetInstanceTranslation {
                        translation: p,
                        ..
                    }) => log.push(format!(
                        "instance {:08x} {:08x} {:08x}",
                        b(p.x),
                        b(p.y),
                        b(p.z)
                    )),
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("bt ") {
                tally.bone_fx_stops += u64::from(l.starts_with("stopbonefx "));
                tally.body_particles += u64::from(l.starts_with("bodyps "));
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
        let Some(UpdateBody::StructureCollapseUpdate(m)) = slot.body.as_ref() else {
            panic!("the module is back in its slot")
        };
        let state = m.state_words();
        match state[2] {
            1 => tally.waiting += 1,
            2 => tally.collapsing += 1,
            3 => tally.done += 1,
            _ => {}
        }
        for s in state {
            put_u32(&mut v, s);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R29_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn structure_collapse_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0029_C011);
    let mut read = Check::new("R29 data read (throw, message)");
    let mut run = Check::new("R29 ops (data, effects, wakes, xfers, state)");
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
        let name = format!("Data\\INI\\R29{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let flags = u32::from(g.below(8) != 0)
            | u32::from(g.below(2) == 0) << 1
            | u32::from(g.below(8) != 0) << 2
            | g.below(2) << 3;
        let cfg = [
            g.below(200),
            g.u32(),
            g.u32(),
            flags,
            // (not `LEVEL_REGULAR`: `getVeterancyLevelFlag`'s shift by -1, as for the death type below; see `common::game_common::veterancy_bit`)
            1 + g.below(3),
            g.below(256) & g.below(256) & g.below(256),
            real(&mut g, &[0.0, 10.0, -35.5]),
            real(&mut g, &[0.0, 7.0, 120.25]),
            real(&mut g, &[0.0, 2.5]),
            real(&mut g, &[0.0, 0.5, -1.2, 2.9]),
            real(&mut g, &[0.0, 5.0, 20.0, 60.0]),
            real(&mut g, &[-1.0, -0.5, -2.0]),
        ];
        let n_ops = 1 + g.below(60) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(24) {
                0..=11 => [0, 0, 0, 0],
                // (not `DEATH_NORMAL`: `getDeathTypeFlag`'s shift by -1 is undefined in C++, and the 64-bit reference build differs from the shipped game; see
                // `logic::damage::death_bit`)
                12..=13 => [1, *pick(&mut g, &[&1u32, &2, &3, &5, &9]), 0, 0],
                14..=17 => {
                    frame += 1 + g.below(400);
                    [2, frame, 0, 0]
                }
                18 => {
                    let a = *pick(&mut g, &[&3u32, &4, &5, &6, &9, &11]);
                    let b = match a {
                        3 => (flags & !1) | u32::from(g.below(8) != 0),
                        4 => 1 + g.below(3),
                        5 => g.below(256) & g.below(256),
                        6 => real(&mut g, &[0.0, 10.0, -35.5]),
                        9 => real(&mut g, &[0.0, 0.5, -1.2]),
                        _ => real(&mut g, &[-1.0, -0.5, -2.0]),
                    };
                    [3, a, b, 0]
                }
                19..=20 => [4, 0, 0, 0],
                21..=22 => [5, 0, 0, 0],
                _ => [6, 0, 0, 0],
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
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r29_run(
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
            if std::env::var("R29_FULL").is_ok() {
                eprintln!("RUST {dr:#?}\nCPP {dc:#?}");
            }
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R29: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} deaths, \
         {} xfers, {} effect lines ({} phase FX and OCLs, {} bone FX stops, {} body particle updates), {} ops waiting, {} collapsing, {} done",
        tally.updates, tally.dies, tally.xfers, tally.effects, tally.phase_lines, tally.bone_fx_stops, tally.body_particles, tally.waiting, tally.collapsing, tally.done
    );
    println!("R29 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.waiting > cases as u64
            && tally.collapsing > cases as u64
            && tally.done > cases as u64
            && tally.bone_fx_stops * 20 > cases as u64,
        "the generator must reach every state"
    );
}
