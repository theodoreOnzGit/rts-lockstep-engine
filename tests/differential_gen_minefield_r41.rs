#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GenerateMinefieldBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GenerateMinefieldBehavior` (`logic::modules::generate_minefield_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R41.
//!
//! # Method
//!
//! The C++ side is the file, unchanged, in `wrap_module.cpp`'s unit (`differential/l9_gen_minefield_r41.inc`, `ref_l9r41_run`), on the
//! module build's stand-ins: the generator (object 1: maybe a player of three, whose default team is the player's, a geometry, a position
//! and an orientation, its completed object upgrades), objects 2-6 (structures or not), the thing factory (`MINE_A`, `MINE_B`, with the
//! test's geometry; `newObject` makes an object whose one behaviour is a `LandMineInterface` whose `setScootParms` is a line), the
//! terrain's eight cells (ground, layer, water, cliff), the potential collisions (the test's list), the FX lists, the REAL `UpgradeCenter`,
//! `UpgradeMux`, `GeometryInfo`, `RandomValue`, `UpdateModule`, `INI` and `Xfer`, and the game logic.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`), a thing factory with the two
//! templates (`ThingFactory::friend_find_template_mut`, `ThingTemplate::friend_set_geometry_info`), `cuts.crate_stub_object_calls` with
//! `crate_new_object_modules` a probe with `trace_land_mine` (the new mine is a record, its orientation a line), the terrain and partition
//! tables, the upgrade center; the real `World::set_position`, `GeometryInfo` and `RandomValues`.
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 30 ops: an `update()`, a change to the frame or the scene, a death, an
//! upgrade, `setMinefieldTarget` or an xfer. Compared, bit for bit: the read's throw; the data; the constructor's effects and wake calls;
//! per op the throw status, the result, the effects in order (the terrain and partition calls, the new mines, their orientations and
//! scoots, the destructions, the FX), the wake calls, the xfer status, bytes and CRC, the module's state, and every mine made so far.
//!
//! # Results (2026-10-24, `L9R41_CASES=9000`)
//!
//! 9,000 cases (6,914 compared; 1,838 data blocks threw, as the C++ did; 248 excluded, undefined upstream: mines placed for an object with
//! no controlling player), 107,849 ops (28,920 updates, 17,747 upgrades, 7,199 deaths, 25,257 xfers) and 229,526 outputs: **0 mismatches**;
//! 4,047,350 effect lines compared: 359,600 mines made (each scooted), 533,802 structure tests, 32,518 destructions of replaced mines and
//! 1,385 FX.
//!
//! On the way (test harness only): the logic random seed is now the case's on both sides (the C++ generator carried over between cases);
//! the Rust effect decoder kept only object events while every event has a trace line (a new mine's drawable shifted the FX line); the
//! driver lacked its `destroyObject` hook; and the C++ save buffer (512 bytes, then 8,192) cut off the largest fields' mine lists. It is
//! 65,536 bytes on both sides.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2111 | a mine placed on a cliff | 18,331 |
//! | 2112 | a mine placed under a structure | 13,605 |
//! | 2113 | the scoot ending where it starts | 2,237 |
//! | 2114 | the rectangle's sides not skipping their first point | 10,226 |
//! | 2115 | no retries for a scattered mine too close to another | 4,311 |
//! | 2116 | smart-border rings a mine radius apart, not a diameter | 4,935 |
//! | 2117 | the replaced mines not destroyed | 265 |
//! | 2118 | the upgraded flag not saved | 11,567 |
//! | 2119 | the jitter in x only | 9,365 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::generate_minefield_behavior::GenerateMinefieldBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r41.bin");
const XFER_FILE: &str = "l9_r41.bin";

/// The radii, by index (the driver's).
const RADII: [f32; 4] = [5.0, 20.0, 1.0, 0.0];
const UPGRADES: [&str; 2] = ["Upgrade_A", "Upgrade_ChinaEMPMines"];

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
    *out = Some(ModuleDataKind::GenerateMinefieldBehavior(Arc::new(GenerateMinefieldBehaviorModuleData::friend_new_module_data(Some(ini))?)));
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
    let names: Vec<String> = (0..3).map(|_| str_(&mut p)).collect();
    let head: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {names:?} {head:x?} constructor {log:?} wakes {w} last {l}"));
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
        let ns = u32_(&mut p);
        let state: Vec<u32> = (0..ns).map(|_| u32_(&mut p)).collect();
        let nm = u32_(&mut p);
        let mines: Vec<u32> = (0..nm * 6).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?} mines {mines:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 92 {
        lines.push(format!("MineName = {}", pick(g, &["MINE_A", "MINE_A", "MINE_B", "NoSuchMine"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("UpgradedMineName = {}", pick(g, &["MINE_B", "MINE_A", "NoSuchMine"])));
    }
    if g.below(100) < 10 {
        lines.push("UpgradedTriggeredBy = Upgrade_ChinaEMPMines".to_string());
    }
    if g.below(100) < 60 {
        lines.push(format!("GenerationFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("DistanceAroundObject = {}", pick(g, &["10", "40", "25", "0", "60"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("MinesPerSquareFoot = {}", pick(g, &["0.01", "0.003", "0.02", "0"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("GenerateOnlyOnDeath = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("BorderOnly = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("SmartBorder = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("SmartBorderSkipInterior = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("AlwaysCircular = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("Upgradable = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("RandomJitter = {}", pick(g, &["0%", "20%", "50%", "100%"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("SkipIfThisMuchUnderStructure = {}", pick(g, &["33%", "0%", "100%", "50%"])));
    }
    if g.below(100) < 85 {
        lines.push(format!("TriggeredBy = {}", pick(g, &["Upgrade_A", "Upgrade_A", "Upgrade_ChinaEMPMines"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "MinesPerSquareFoot = x", "BorderOnly = maybe", "RandomJitter"]).to_string());
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
    upgrades: u64,
    deaths: u64,
    xfers: u64,
    effects: u64,
    mines: u64,
    skipped: u64,
    scoots: u64,
    destroys: u64,
    fx: u64,
    replaced: u64,
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
            b.truncate(65536);
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

fn template_of(structure: bool) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R41Thing".to_vec(), ..Default::default() };
    if structure {
        values.kind_of.push(KINDOF_STRUCTURE);
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn geom_of(g: u32) -> GeometryInfo {
    let ty = match g & 3 {
        0 => GeometryType::Sphere,
        1 => GeometryType::Cylinder,
        _ => GeometryType::Box,
    };
    GeometryInfo::new(ty, false, 5.0, RADII[(g >> 2 & 3) as usize], RADII[(g >> 4 & 3) as usize])
}

/// The upgrade masks of `Upgrade_A` (bit 0) and `Upgrade_ChinaEMPMines` (bit 1) in `bits`.
fn upgrade_mask(w: &mut World, bits: u32) -> UpgradeMaskType {
    let statics = w.statics.clone();
    let mut m = UpgradeMaskType::default();
    for (k, name) in UPGRADES.iter().enumerate() {
        if bits >> k & 1 != 0 {
            if let Some(u) = statics.upgrade_center.find_upgrade(&mut w.names, name.as_bytes()).unwrap() {
                m.set_mask(&u.upgrade_mask());
            }
        }
    }
    m
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 24]) {
    let tp = cfg[1];
    w.objects.find_mut(1).unwrap().team = if (1..=3).contains(&tp) { tp } else { 0 };
    for k in 1..6usize {
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.thing.friend_set_template(template_of(cfg[1 + k] & 1 != 0));
    }
    let m = upgrade_mask(w, cfg[8] & 3);
    w.objects.find_mut(1).unwrap().object_upgrades_completed = m;
    let mut scan = Vec::new();
    for k in 0..6 {
        let id = cfg[7] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 6 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::GenerateMinefieldBehavior(d) = &data else { unreachable!() };
    put_str(&mut v, &d.mine_name);
    put_str(&mut v, &d.mine_name_upgraded);
    put_str(&mut v, &d.mine_upgrade_trigger);
    put_i32(&mut v, fx.id(&d.gen_fx));
    put_u32(&mut v, d.distance_around_object.to_bits());
    put_u32(&mut v, d.mines_per_square_foot.to_bits());
    put_u32(&mut v, d.random_jitter.to_bits());
    put_u32(&mut v, d.skip_if_this_much_under_structure.to_bits());
    put_u32(
        &mut v,
        u32::from(d.on_death)
            | u32::from(d.border_only) << 1
            | u32::from(d.always_circular) << 2
            | u32::from(d.upgradable) << 3
            | u32::from(d.smart_border) << 4
            | u32::from(d.smart_border_skip_interior) << 5,
    );
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut factory = ThingFactory::new();
    let mut template_names: BTreeMap<u16, &str> = BTreeMap::new();
    for name in ["MINE_A", "MINE_B"] {
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let t = factory.friend_find_template_mut(name.as_bytes()).expect("the new template");
        t.data.friend_set_geometry_info(geom_of(cfg[11]));
        template_names.insert(t.data.get_template_id(), name);
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), thing_factory: factory, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.crate_stub_object_calls = true;
    {
        let mut m = NewModule::plain(b"R41Mine", b"ModuleTag_Mine");
        let answers = ProbeAnswers { trace_land_mine: true, ..Default::default() };
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        w.cuts.crate_new_object_modules = vec![m];
    }
    for c in 0..8 {
        w.cuts.terrain_layer[c] = (cfg[15] >> (4 * c) & 15) as i32;
        w.cuts.terrain_ground[c] = (cfg[16] >> (4 * c) & 15) as f32 * 0.5;
        w.cuts.terrain_has_water[c] = cfg[17] >> c & 1 != 0;
        w.cuts.terrain_cliff[c] = cfg[17] >> (8 + c) & 1 != 0;
    }
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[9]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    for k in 0..6usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"GenerateMinefieldBehavior", b"ModuleTag_Me", &data).unwrap().unwrap());
        }
        let id = w.new_object(NewObject { template: template_of(false), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
    }
    w.objects.find_mut(1).unwrap().geometry_info = geom_of(cfg[10]);
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[12]), y: f32::from_bits(cfg[13]), z: 0.0 });
    w.set_orientation(1, f32::from_bits(cfg[14]));
    let idx = w.find_module(1, b"GenerateMinefieldBehavior").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let effects = |w: &mut World, events_before: usize, fx: &mut FxIds, tally: &mut Tally| -> (Vec<String>, u32, u32) {
        // every event, in order: the trace has a line for each (a new mine's drawable is one that is not an object event)
        let events: Vec<Option<ObjectClientEvent>> = w.events.events()[events_before..]
            .iter()
            .map(|e| match e {
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
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::TerrainGetHighestLayerForDestination { x, y, z }) => log.push(format!("highlayer {:08x} {:08x} {:08x}", b(x), b(y), b(z))),
                    Some(CutCall::TerrainIsUnderwater { x, y }) => log.push(format!("underwater {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::TerrainIsCliffCell { x, y }) => log.push(format!("cliff {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::PartitionIteratePotentialCollisions { position: p, geom_type, major_radius, minor_radius, angle }) => {
                        tally.skipped += 1;
                        log.push(format!(
                            "potcol {:08x} {:08x} {:08x} {geom_type} {:08x} {:08x} {:08x}",
                            b(p.x),
                            b(p.y),
                            b(p.z),
                            b(major_radius),
                            b(minor_radius),
                            b(angle)
                        ))
                    }
                    Some(CutCall::ThingFactoryNewObject { template, team }) => {
                        tally.mines += 1;
                        log.push(format!("newobject {} {}", template_names.get(&template).copied().unwrap_or("?"), team.unwrap_or(0)))
                    }
                    Some(CutCall::ObjectSetOrientation { object, angle }) => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        tally.destroys += 1;
                        log.push(format!("destroy {object}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                if let Some(Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. })) = ev_iter.next() {
                    tally.fx += 1;
                    log.push(format!("fx {}", fx.id(&Some(name))));
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("bt scoot ") {
                tally.scoots += 1;
                log.push(format!("scoot {l}"));
            }
        }
        (log, wakes, last)
    };
    set_scene(&mut w, &cfg);
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
        if o[0] == 2 && (1..=8).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
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
                let before = w.objects.find(1).is_some_and(|_| true);
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                let _ = before;
                tally.updates += 1;
            }
            3 => {
                tally.deaths += 1;
                w.module_call(1, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            4 => {
                tally.upgrades += 1;
                let m = upgrade_mask(&mut w, o[1] & 3);
                res = u32::from(w.module_call(1, idx, ModuleCall::AttemptUpgrade(m)).as_bool());
            }
            5 => {
                let pos = (o[1] != 0).then(|| Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: 0.0 });
                w.module_call(1, idx, ModuleCall::GenerateMinefieldSetTarget(pos));
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
        let Some(UpdateBody::GenerateMinefieldBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let words = m.state_words();
        if words[3] != 0 {
            tally.replaced += 1;
        }
        put_u32(&mut v, words.len() as u32);
        for s in &words {
            put_u32(&mut v, *s);
        }
        let mut mines: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id >= 7).collect();
        mines.sort_unstable();
        put_u32(&mut v, mines.len() as u32);
        for id in mines {
            let mo = w.objects.find(id).unwrap();
            let p = mo.get_position();
            put_u32(&mut v, id as u32);
            put_u32(&mut v, p.x.to_bits());
            put_u32(&mut v, p.y.to_bits());
            put_u32(&mut v, p.z.to_bits());
            put_u32(&mut v, mo.producer_id as u32);
            put_u32(&mut v, mo.team);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R41_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn generate_minefield_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0041_63E7);
    let mut read = Check::new("R41 data read (throw, message)");
    let mut run = Check::new("R41 ops (data, effects, wakes, xfers, state, mines)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let mut ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let defs = "Upgrade Upgrade_A\n Type = OBJECT\nEnd\nUpgrade Upgrade_ChinaEMPMines\n Type = OBJECT\nEnd\n";
        let table = blocks();
        Ini::load(b"Data\\INI\\Upgrade.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        assert_eq!(cpp_load(b"Data\\INI\\Upgrade.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R41{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 24];
        cfg[0] = g.below(100);
        cfg[1] = if g.below(12) == 0 { 0 } else { 1 + g.below(3) };
        for c in &mut cfg[2..=6] {
            *c = u32::from(g.below(3) == 0);
        }
        let gen_scan = |g: &mut Gen| (0..6).fold(0, |acc, k| acc | (1 + g.below(6)) << (4 * k)) & ((1u32 << (4 * g.below(4))) - 1);
        cfg[7] = gen_scan(&mut g);
        cfg[8] = g.below(4);
        cfg[9] = g.below(u32::MAX);
        cfg[10] = g.below(4) | g.below(3) << 2 | g.below(3) << 4;
        cfg[11] = if g.below(2) == 0 { 2 << 2 | 2 << 4 } else { g.below(4) | (if g.below(3) == 0 { 0 } else { 2 }) << 2 | 2 << 4 };
        cfg[12] = (g.below(100) as f32 - 50.0).to_bits();
        cfg[13] = (g.below(100) as f32 - 50.0).to_bits();
        cfg[14] = [0.0f32, 0.5, 1.5707964, -2.0, 3.0][g.below(5) as usize].to_bits();
        cfg[15] = (0..8).fold(0, |acc, c| acc | (if g.below(5) == 0 { 2 } else { 1 }) << (4 * c));
        cfg[16] = g.below(u32::MAX);
        cfg[17] = (0..16).fold(0, |acc, b| acc | u32::from(g.below(6) == 0) << b);
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(30) {
                0..=7 => [0, 0, 0, 0],
                8..=10 => {
                    frame += 1 + g.below(10);
                    [1, frame, 0, 0]
                }
                11..=13 => {
                    let a = 1 + g.below(8);
                    let b = match a {
                        1 => if g.below(12) == 0 { 0 } else { 1 + g.below(3) },
                        2..=6 => u32::from(g.below(3) == 0),
                        7 => gen_scan(&mut g),
                        _ => g.below(4),
                    };
                    [2, a, b, 0]
                }
                14..=15 => [3, 0, 0, 0],
                16..=20 => [4, 1 + g.below(3), 0, 0],
                21..=22 => [5, u32::from(g.below(3) != 0), (g.below(60) as f32 - 30.0).to_bits(), (g.below(60) as f32 - 30.0).to_bits()],
                23..=25 => [6, 0, 0, 0],
                26..=27 => [7, 0, 0, 0],
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
        let mut buf = vec![0u8; 1 << 24];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r41_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        if dr != dc && shown < 4 {
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
        "R41: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, \
         {} upgrades, {} deaths, {} xfers), {} effect lines ({} mines made, {} structure tests, {} scoots, {} destructions, {} FX; {} upgraded op-states; \
         the compared cases')",
        tally.updates, tally.upgrades, tally.deaths, tally.xfers, tally.effects, tally.mines, tally.skipped, tally.scoots, tally.destroys, tally.fx, tally.replaced
    );
    println!("R41 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.mines > cases as u64 * 3 && tally.scoots > cases as u64 && tally.fx > cases as u64 / 10 && tally.destroys > cases as u64 / 50, "the generator must reach every effect");
}
