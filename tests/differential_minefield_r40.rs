#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/MinefieldBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `MinefieldBehavior` (`logic::modules::minefield_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R40.
//!
//! # Method
//!
//! The C++ side is the file (one mechanical edit: a function-local `static` name key made a plain local, so that the per-case reset of the
//! name key generator cannot leave it stale), in `wrap_module.cpp`'s unit (`differential/l9_minefield_r40.inc`, `ref_l9r40_run`), on the
//! module build's stand-ins: six objects (ids 1-6; a team of three whose relationships the test gives, the kinds `INFANTRY`, `DOZER` and
//! `STRUCTURE`, effectively dead, maybe a body (its damage, healing and, on the mine, `internalChangeHealth` lines), maybe an AI (clearing
//! mines or not, with the test's goal), a position), the mine (object 1: a drawable, a geometry, a producer, maybe the REAL
//! `AutoHealBehavior`), the terrain's eight cells, `setLayer`, the weapon store, the object creation lists and `destroyObject` (lines), the
//! REAL `Object::getRelationship`, `GeometryInfo::clipPointToFootprint`, `UpdateModule`, `INI` and `Xfer`, and the game logic, whose wake
//! calls are applied as `GameLogic::friend_awakenUpdateModule` applies them and counted per module.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module and the real `AutoHealBehavior` from `modules::new_module`;
//! `ProbeModule` bodies that never die, the mine's with `trace_internal_change`, and AIs), three teams and `cuts.team_relationships`, the
//! terrain tables, the weapon and object creation list stores, `cuts.body.stub_object_calls` and `cuts.crate_stub_object_calls` (the
//! destruction and `setLayer` are lines); the real `World::attempt_damage`, `get_relationship` and `set_status`.
//!
//! Each case reads a block `D` (mostly well formed), makes the mine, gives it its producer, and runs 1 to 100 ops: an `update()`, a change
//! to the frame or the scene, a collision, damage or healing (the body first, then the module), a death, `setScootParms`, `disarm` or an
//! xfer. Compared, bit for bit: the read's throw; the data; the constructor's effects and wake calls; per op the throw status, the result,
//! the effects in order, both modules' wake calls, the xfer status, bytes and CRC, the module's state (the detonators included), the mine's
//! position and two status bits, and every body's health.
//!
//! # Results (2026-10-24, `L9R40_CASES=9000`)
//!
//! 9,000 cases (7,005 compared; 1,774 data blocks threw, as the C++ did; 221 excluded, undefined upstream: a mine without a body whose
//! body is read), 350,191 ops (87,396 updates, 61,722 collisions, 43,729 damages and healings, 61,365 xfers) and 714,392 outputs: **0
//! mismatches**; 300,310 effect lines compared: 13,961 weapons fired, 12,060 creation lists, 21,642 destructions, 30,570 rubble states, 32,717
//! scoot steps, 3,512 health fixes (`internalChangeHealth`) and 3,587 wakes of the `AutoHealBehavior` stopped when the creator dies.
//!
//! On the way: the first runs differed (1) in the constructors' wake calls, which the driver's hook did not count before it knew the
//! modules' pointers (fixed in the driver), and (2) where the test gave a team relationship the 2-bit code 3, which is no `Relationship`:
//! the C++ takes it as is (no detonation mask bit), the Rust test read it as neutral. The generator now makes only 0-2.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2101 | an immune not keeping the mine awake | 4,488 |
//! | 2102 | an immunity expiring a frame early | 1,311 |
//! | 2103 | no 0.1 health floor on a detonation | 6,234 |
//! | 2104 | `DetonatedBy` ignored | 63,505 |
//! | 2105 | the repeat-detonation move threshold ignored | 15,509 |
//! | 2106 | the detonation point not clipped to the footprint | 3,506 |
//! | 2107 | a draining field's own damage detonating mines | 2 |
//! | 2108 | the draining flag not saved | 26,770 |
//! | 2109 | healing ceiled, not floored | 80,688 |
//!
//! 2107's path (draining, then damage from the mine itself) is reached only by the drain's own damage; two outputs caught it.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK_FROM_AI};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use rts_lockstep_engine::logic::modules::auto_heal_behavior::AutoHealBehaviorModuleData;
use rts_lockstep_engine::logic::modules::minefield_behavior::MinefieldBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r40.bin");
const XFER_FILE: &str = "l9_r40.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r40Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_INFANTRY, KINDOF_DOZER, KINDOF_STRUCTURE];
/// The mine's body maximum, by index (the driver's `g_r40Max`).
const MAX: [f32; 4] = [100.0, 30.0, 1000.0, 7.0];
/// The geometry's radii, by index (the driver's).
const RADII: [f32; 4] = [1.0, 4.0, 10.0, 0.5];

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
    *out = Some(ModuleDataKind::MinefieldBehavior(Arc::new(MinefieldBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
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
    let head: Vec<u32> = (0..11).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let wakes: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
    out.push(format!("data {head:x?} constructor {log:?} wakes {wakes:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let wakes: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let ns = u32_(&mut p);
        let state: Vec<u32> = (0..ns).map(|_| u32_(&mut p)).collect();
        let rest: Vec<u32> = (0..5 + 6).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} {rest:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 90 {
        lines.push(format!("DetonationWeapon = {}", pick(g, &["WEAP_A", "WEAP_B", "WEAP_A", "WEAP_B", "None", "NoSuchWeapon"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("DetonatedBy = {}", pick(g, &["ENEMIES", "ENEMIES NEUTRAL", "ALLIES ENEMIES NEUTRAL", "NEUTRAL"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("StopsRegenAfterCreatorDies = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("Regenerates = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("WorkersDetonate = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("CreatorDeathCheckRate = {}", pick(g, &["0", "100", "500", "1000"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("ScootFromStartingPointTime = {}", pick(g, &["0", "100", "300", "1000"])));
    }
    if g.below(100) < 70 {
        lines.push(format!("NumVirtualMines = {}", pick(g, &["1", "3", "5", "10", "0"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("RepeatDetonateMoveThresh = {}", pick(g, &["1", "5", "0", "20"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("DegenPercentPerSecondAfterCreatorDies = {}", pick(g, &["10%", "50%", "0%", "100%"])));
    }
    if g.below(100) < 80 {
        lines.push(format!("CreationList = {}", pick(g, &["OCL_A", "OCL_B", "OCL_A", "OCL_B", "None", "OCL_None"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "NumVirtualMines = x", "DetonatedBy = FRIENDS", "Regenerates"]).to_string());
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
    collides: u64,
    damages: u64,
    xfers: u64,
    effects: u64,
    weapons: u64,
    ocls: u64,
    destroys: u64,
    rubble: u64,
    scoot_moves: u64,
    changes: u64,
    stops: u64,
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            // `throw SC_INVALID_DATA` (a SaveCode, not an XferStatus): the C++ runXfer's catch-all answers -1
            Err(rts_lockstep_engine::common::xfer::XferStatus::ListNotEmpty) => -1,
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

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R40Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

/// The slots of each object's body and AI.
struct Slots {
    body: [u16; 6],
    ai: [u16; 6],
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 24], s: &Slots) {
    for k in 0..6usize {
        let f = cfg[2 + k];
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.team = f & 3;
        o.thing.friend_set_template(template_of(f >> 2 & 7));
        if f >> 5 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.body = (f >> 6 & 1 != 0).then_some(s.body[k]);
        o.ai = (f >> 7 & 1 != 0).then_some(s.ai[k]);
        let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[k] as usize].body.as_mut() else { unreachable!() };
        p.answers.clearing_mines = f >> 8 & 1 != 0;
        p.answers.goal_object = cfg[9] as ObjectId;
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[8] >> (2 * (3 * a + b)) & 3));
        }
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let ModuleDataKind::MinefieldBehavior(d) = &data else { unreachable!() };
    let code = |n: &Option<Vec<u8>>, a: &[u8], b: &[u8]| match n.as_deref() {
        Some(x) if x.eq_ignore_ascii_case(a) => 1,
        Some(x) if x.eq_ignore_ascii_case(b) => 2,
        _ => 0,
    };
    put_i32(&mut v, code(&d.detonation_weapon, b"WEAP_A", b"WEAP_B"));
    put_u32(&mut v, d.detonated_by);
    put_u32(&mut v, u32::from(d.stops_regen_after_creator_dies));
    put_u32(&mut v, u32::from(d.regenerates));
    put_u32(&mut v, u32::from(d.workers_detonate));
    put_u32(&mut v, d.creator_death_check_rate);
    put_u32(&mut v, d.scoot_from_starting_point_time);
    put_u32(&mut v, d.num_virtual_mines);
    put_u32(&mut v, d.repeat_detonate_move_thresh.to_bits());
    put_u32(&mut v, d.health_percent_to_drain_per_second.to_bits());
    put_i32(&mut v, code(&d.ocl, b"OCL_A", b"OCL_B"));
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        ocls.ocls.insert(w.names.name_to_key(n), Default::default());
    }
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for n in ["WEAP_A", "WEAP_B"] {
        weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap();
    }
    w.statics = Arc::new(GameStatics { object_creation_list_store: ocls, weapon_store, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.crate_stub_object_calls = true;
    for c in 0..8 {
        w.cuts.terrain_layer[c] = (cfg[19] >> (4 * c) & 15) as i32;
        w.cuts.terrain_layer_height[c] = (cfg[20] >> (4 * c) & 15) as f32 * 0.5;
        w.cuts.terrain_ground[c] = (cfg[21] >> (4 * c) & 15) as f32 * 0.5;
    }
    w.set_frame(cfg[0]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let probe = |name: &[u8], answers: ProbeAnswers, max: f32, interfaces: i32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        m
    };
    let has_ahb = cfg[11] != 0;
    let mut slots = Slots { body: [0; 6], ai: [0; 6] };
    for k in 0..6usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"MinefieldBehavior", b"ModuleTag_Me", &data).unwrap().unwrap());
            if has_ahb {
                let ahd = ModuleDataKind::AutoHealBehavior(Arc::new(AutoHealBehaviorModuleData::default()));
                modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"AutoHealBehavior", b"ModuleTag_Heal", &ahd).unwrap().unwrap());
            }
        }
        let max = if k == 0 { MAX[(cfg[22] & 3) as usize] } else { 1000.0 };
        let answers = ProbeAnswers { no_death: true, trace_internal_change: k == 0, ..Default::default() };
        modules.push(probe(b"R40Body", answers, max, MODULEINTERFACE_BODY));
        let mut ai = probe(b"R40Ai", ProbeAnswers::default(), 0.0, 0);
        ai.is_ai = true;
        modules.push(ai);
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        slots.body[k] = w.find_module(id, b"R40Body").unwrap();
        slots.ai[k] = w.find_module(id, b"R40Ai").unwrap();
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[12 + k]), y: 10.0 * k as f32 - 20.0, z: 0.0 });
    }
    {
        let gt = cfg[18] & 3;
        let ty = match gt {
            0 => GeometryType::Sphere,
            1 => GeometryType::Cylinder,
            _ => GeometryType::Box,
        };
        let o = w.objects.find_mut(1).unwrap();
        o.geometry_info = GeometryInfo::new(ty, false, 5.0, RADII[(cfg[18] >> 2 & 3) as usize], RADII[(cfg[18] >> 4 & 3) as usize]);
        o.drawable = 7;
    }
    let idx = w.find_module(1, b"MinefieldBehavior").unwrap();
    let ahb_idx = w.find_module(1, b"AutoHealBehavior");
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mut fx_ocl = |name: &[u8]| -> i32 { i32::from(name == b"OCL_B") };
    let effects = |w: &mut World, events_before: usize, tally: &mut Tally, fx_ocl: &mut dyn FnMut(&[u8]) -> i32| -> (Vec<String>, [u32; 4]) {
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes) = (Vec::new(), [0u32; 4]);
        let b = |x: f32| x.to_bits();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next() {
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", b(x), b(y))),
                    Some(CutCall::TerrainGetHighestLayerForDestination { x, y, z }) => {
                        tally.scoot_moves += 1;
                        log.push(format!("highlayer {:08x} {:08x} {:08x}", b(x), b(y), b(z)))
                    }
                    Some(CutCall::ObjectSetLayer { object, layer }) => log.push(format!("setlayer {object} {layer}")),
                    Some(CutCall::WeaponCreateAndFireTemp { weapon, source, position: p }) => {
                        tally.weapons += 1;
                        log.push(format!("weapon {} {source} {},{},{}", String::from_utf8_lossy(&weapon), b(p.x), b(p.y), b(p.z)))
                    }
                    Some(CutCall::OclCreateWithSecondary { secondary, ocl, .. }) => {
                        tally.ocls += 1;
                        log.push(format!("ocl {}", fx_ocl(&ocl)));
                        if secondary != INVALID_ID {
                            log.push(format!("oclsec {secondary}"));
                        }
                    }
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        tally.destroys += 1;
                        log.push(format!("destroy {object}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(f), .. }) => {
                        tally.rubble += 1;
                        log.push(format!("mc {}", f[0]))
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(f), .. }) => log.push(format!("mcclear {}", f[0])),
                    _ => {}
                }
            } else if let Some(rest) = t.strip_prefix("awaken ") {
                let (i, f) = rest.split_once(' ').unwrap();
                let (i, f): (i32, u32) = (i.parse().unwrap(), f.parse().unwrap());
                if i == i32::from(idx) {
                    wakes[0] += 1;
                    wakes[1] = f;
                } else if Some(i) == ahb_idx.map(i32::from) {
                    tally.stops += 1;
                    wakes[2] += 1;
                    wakes[3] = f;
                }
            } else if t.starts_with("damage ") || t.starts_with("heal ") {
                log.push(t.clone());
            } else if let Some(l) = t.strip_prefix("bt internalchange ") {
                tally.changes += 1;
                log.push(format!("internalchange {l}"));
            }
        }
        (log, wakes)
    };
    set_scene(&mut w, &cfg, &slots);
    {
        let (log, wakes) = effects(&mut w, 0, tally, &mut fx_ocl);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        for x in wakes {
            put_u32(&mut v, x);
        }
    }
    // the producer is set after the object is made
    w.objects.find_mut(1).unwrap().producer_id = cfg[10] as ObjectId;
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=10).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
            if o[1] == 10 {
                w.objects.find_mut(1).unwrap().producer_id = cfg[10] as ObjectId;
            }
        }
        if o[0] == 1 {
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
            3 => {
                tally.collides += 1;
                let other = o[1] as ObjectId;
                let loc = w.objects.find(other).map(|x| x.get_position());
                w.module_call(1, idx, ModuleCall::OnCollide { other, loc, normal: None });
            }
            4 => {
                tally.damages += 1;
                let mut info = DamageInfo::default();
                info.input.amount = f32::from_bits(o[1]);
                info.input.damage_type = (o[2] & 255) as i32;
                info.input.death_type = (o[2] >> 8) as i32;
                info.input.source_id = o[3] as ObjectId;
                if let Some(body) = w.get_body_module(1) {
                    w.module_call(1, body, ModuleCall::AttemptDamage(info.clone()));
                }
                w.module_call(1, idx, ModuleCall::OnDamage(info));
            }
            5 => {
                tally.damages += 1;
                let mut info = DamageInfo::default();
                info.input.amount = f32::from_bits(o[1]);
                info.input.damage_type = DAMAGE_HEALING;
                info.input.death_type = DEATH_NONE;
                info.input.source_id = o[3] as ObjectId;
                if let Some(body) = w.get_body_module(1) {
                    w.module_call(1, body, ModuleCall::AttemptHealing(info.clone()));
                }
                w.module_call(1, idx, ModuleCall::OnHealing(info));
            }
            6 => {
                w.module_call(1, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            7 => {
                let start = Coord3D { x: f32::from_bits(o[1]), y: 5.0, z: f32::from_bits(o[3]) };
                let end = Coord3D { x: f32::from_bits(o[2]), y: 7.0, z: 0.0 };
                w.module_call(1, idx, ModuleCall::LandMineSetScootParms { start, end });
            }
            8 => {
                w.module_call(1, idx, ModuleCall::LandMineDisarm);
            }
            9..=11 => {
                let r = run_xfer(&mut w, 1, idx as usize, o[0] - 9, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 9 {
                    saved = xbytes.clone();
                }
                if o[0] == 10 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            1 | 2 => {}
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
        let (log, wakes) = effects(&mut w, events_before, tally, &mut fx_ocl);
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        for x in wakes {
            put_u32(&mut v, x);
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        let Some(UpdateBody::MinefieldBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let words = m.state_words();
        put_u32(&mut v, words.len() as u32);
        for s in &words {
            put_u32(&mut v, *s);
        }
        let p = ob.get_position();
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
        put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_MASKED)));
        put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_NO_ATTACK_FROM_AI)));
        for k in 0..6usize {
            let o = w.objects.find(k as ObjectId + 1).unwrap();
            let Some(UpdateBody::ModuleProbe(pb)) = o.modules[slots.body[k] as usize].body.as_ref() else { unreachable!() };
            put_u32(&mut v, pb.health.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R40_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

/// An object id: none (0), one of objects 2-6, the mine (rarely), or no object (7).
fn gen_id(g: &mut Gen) -> u32 {
    match g.below(14) {
        0 => 0,
        1 => 7,
        2 => 1,
        _ => 2 + g.below(5),
    }
}

/// The teams' relationships: nine 2-bit codes of 0 (enemies), 1 (neutral) or 2 (allies); 3 is no `Relationship`.
fn gen_rel(g: &mut Gen) -> u32 {
    (0..9).fold(0, |acc, i| acc | g.below(3) << (2 * i))
}

/// An object's scene word (`k` its index).
fn gen_object(g: &mut Gen, k: usize) -> u32 {
    let team = if g.below(8) == 0 { 0 } else { 1 + g.below(3) };
    let kinds = (0..3).fold(0, |acc, b| acc | u32::from(g.below(if b == 0 { 2 } else { 4 }) == 0) << b) << 2;
    let dead = u32::from(g.below(if k == 0 { 30 } else { 8 }) == 0) << 5;
    let body = u32::from(g.below(if k == 0 { 40 } else { 6 }) != 0) << 6;
    let ai = u32::from(g.below(3) != 0) << 7;
    let clearing = u32::from(g.below(4) == 0) << 8;
    team | kinds | dead | body | ai | clearing
}

#[test]
fn minefield_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0040_313E);
    let mut read = Check::new("R40 data read (throw, message)");
    let mut run = Check::new("R40 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R40{}.ini", case % 5).into_bytes();
        let mut cfg = [0u32; 24];
        cfg[0] = g.below(200);
        for k in 0..6 {
            cfg[2 + k] = gen_object(&mut g, k);
        }
        cfg[8] = gen_rel(&mut g);
        cfg[9] = gen_id(&mut g);
        cfg[10] = if g.below(5) == 0 { 0 } else { gen_id(&mut g) };
        cfg[11] = u32::from(g.below(2) == 0);
        for k in 0..6 {
            cfg[12 + k] = (g.below(40) as f32 - 20.0).to_bits();
        }
        cfg[18] = g.below(64);
        cfg[19] = (0..8).fold(0, |acc, c| acc | (if g.below(3) == 0 { 2 + g.below(2) } else { 1 }) << (4 * c));
        cfg[20] = g.below(u32::MAX);
        cfg[21] = g.below(u32::MAX);
        cfg[22] = g.below(4);
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let n_ops = 1 + g.below(100) as usize;
        let mut frame = cfg[0];
        let amount = |g: &mut Gen| -> u32 { [1.0f32, 5.0, 10.0, 0.0, 20.0, 2.5, 50.0, 200.0][g.below(8) as usize].to_bits() };
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=9 => [0, 0, 0, 0],
                10..=13 => {
                    let span = if g.below(4) == 0 { 400 } else { 5 };
                    frame += 1 + g.below(span);
                    [1, frame, 0, 0]
                }
                14..=16 => {
                    let a = 2 + g.below(9);
                    let b = match a {
                        2..=7 => gen_object(&mut g, a as usize - 2),
                        8 => gen_rel(&mut g),
                        _ => gen_id(&mut g),
                    };
                    [2, a, b, 0]
                }
                17..=23 => [3, gen_id(&mut g), 0, 0],
                24..=26 => [4, amount(&mut g), g.below(12) | g.below(4) << 8, gen_id(&mut g)],
                27..=28 => [5, amount(&mut g), 0, gen_id(&mut g)],
                29 => [6, 0, 0, 0],
                30..=31 => {
                    let z = if g.below(2) == 0 { 0.0f32 } else { g.below(80) as f32 };
                    [7, (g.below(40) as f32 - 20.0).to_bits(), (g.below(40) as f32 - 20.0).to_bits(), z.to_bits()]
                }
                32 => [8, 0, 0, 0],
                33..=35 => [9, 0, 0, 0],
                36..=37 => [10, 0, 0, 0],
                _ => [11, 0, 0, 0],
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
            cpp::ref_l9r40_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R40: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, \
         {} collisions, {} damages and healings, {} xfers), {} effect lines ({} weapons, {} creation lists, {} destructions, {} rubble, {} scoot \
         steps, {} health fixes, {} heal-stop wakes; the compared cases')",
        tally.updates, tally.collides, tally.damages, tally.xfers, tally.effects, tally.weapons, tally.ocls, tally.destroys, tally.rubble, tally.scoot_moves, tally.changes, tally.stops
    );
    println!("R40 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.weapons > cases as u64 && tally.ocls > cases as u64 / 2 && tally.destroys > cases as u64 / 10 && tally.scoot_moves > cases as u64 && tally.rubble > cases as u64,
        "the generator must reach every effect"
    );
}
