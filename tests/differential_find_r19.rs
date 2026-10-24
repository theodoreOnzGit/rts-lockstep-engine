#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoFindHealingUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PilotFindVehicleUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `AutoFindHealingUpdate` and `PilotFindVehicleUpdate` (`logic::modules::{auto_find_healing_update, pilot_find_vehicle_update}`) against the
//! unchanged C++, `p1w13-sonnet`, batch R19.
//!
//! # Method
//!
//! The C++ side is the two `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_find_r19.inc`, `ref_l9r19_run`; the
//! module build gained stand-ins `GameLogic/ObjectIter.h` and `GameLogic/WeaponSet.h`, forwarders to the ones it had) on the module build's
//! stand-ins: the module's object (id 1) with a controlling player, human or computer, whose `getAiBaseCenter` answers from the test; maybe an
//! AI, idle or not; maybe a body; up to two collide modules whose `wouldLikeToCollideWith` answers from the test; three more objects with kinds
//! (`HEAL_PAD`, `VEHICLE`) and maybe a body; the partition manager's range scan (the objects the test lists) and `getDistanceSquared` (a table);
//! the AI's orders as lines; and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` AI, bodies and
//! collide modules), the scan `cuts.range_scan`, the distances `cuts.distance_squared_center_2d`, the base `cuts.ai_base_center`, every op an
//! `update()` as the scheduler runs it or a change to the scene.
//!
//! Each case picks the module, reads a block `D` (`ScanRate`, `ScanRange`, and `NeverHeal` and `AlwaysHeal` or `MinHealth`), mostly well
//! formed, and runs 1 to 20 ops. Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time, the effects in
//! order (`scan`, `distsq`, `wouldcollide`, `basecenter`, `aiheal`, `aienter`, `aimove`), the xfer status, bytes and CRC, the module's field.
//!
//! # Results (2026-10-23, `L9R19_CASES=9000`)
//!
//! 9,000 cases (3,586 `AutoFindHealingUpdate`, 3,566 `PilotFindVehicleUpdate` compared, none excluded; 1,848 data files threw, as the C++
//! did), 75,104 ops (33,682 updates, 18,807 xfers) and 89,408 op outputs: **0 mismatches**; 17,143 effect lines compared, among them 982
//! `aiGetHealed`, 1,246 `aiEnter` and 624 moves to the base.
//!
//! Harness correction on the way, none in the translation: the module's field read 0 after every update on the Rust side (4,108 mismatches in a
//! 2,000-case run), because the modules' `PartialEq`, copied from a module with no state, ignored the field, so `update_behavior::world_update`
//! took the update as having changed nothing and put back the copy it had lent the world. Both now compare the field, as
//! `DefaultProductionExitUpdate` does its rally point.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1851 | the healing unit at exactly `NeverHeal` of its maximum not sent | 304 |
//! | 1852 | the last of equally near heal pads taken, not the first | 64 |
//! | 1853 | the scan countdown not restarted | 14,024 |
//! | 1854 | any object taken for a heal pad | 1,381 |
//! | 1861 | the pilot's `MinHealth` test skipped | 1,958 |
//! | 1862 | the base flag not cleared on entering a vehicle | 124 |
//! | 1863 | a human player's pilot not put to sleep for ever | 5,468 |
//! | 1864 | `m_didMoveToBase` not saved | 3,975 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{
    ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_COLLIDE,
};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::auto_find_healing_update::AutoFindHealingUpdateModuleData;
use rts_lockstep_engine::logic::modules::pilot_find_vehicle_update::PilotFindVehicleUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleHandle;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r19.bin");
const XFER_FILE: &str = "l9_r19.bin";
const CLASSES: [&str; 2] = ["AutoFindHealingUpdate", "PilotFindVehicleUpdate"];
/// The kinds the objects are given, by bit (the driver's `g_r19Kinds`).
const KINDS: [KindOfType; 2] = [KINDOF_HEAL_PAD, KINDOF_VEHICLE];

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
fn read_heal(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::AutoFindHealingUpdate(Arc::new(
        AutoFindHealingUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}
fn read_pilot(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::PilotFindVehicleUpdate(Arc::new(
        PilotFindVehicleUpdateModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: if kind == 0 { read_heal } else { read_pilot },
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r19_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    match data {
        ModuleDataKind::AutoFindHealingUpdate(d) => {
            put_u32(v, d.scan_frames);
            put_u32(v, d.scan_range.to_bits());
            put_u32(v, d.never_heal.to_bits());
            put_u32(v, d.always_heal.to_bits());
        }
        ModuleDataKind::PilotFindVehicleUpdate(d) => {
            put_u32(v, d.scan_frames);
            put_u32(v, d.scan_range.to_bits());
            put_u32(v, d.min_health.to_bits());
        }
        _ => unreachable!("not an R19 module"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: u32, b: &[u8], n_ops: usize) -> Vec<String> {
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
    let f: Vec<u32> = (0..if kind == 0 { 4 } else { 3 })
        .map(|_| u32_(&mut p))
        .collect();
    out.push(format!("data {f:x?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let field = u32_(&mut p);
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} field {field}"));
    }
    out
}

// -- generators --

/// The block `D`, each field present with some probability; now and then a bogus value or an unknown field.
fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 80 {
        lines.push(format!(
            "ScanRate = {}",
            pick(g, &["0", "33", "34", "100", "1000"])
        ));
    }
    if g.below(100) < 80 {
        lines.push(format!(
            "ScanRange = {}",
            pick(g, &["0", "50", "150.5", "-1"])
        ));
    }
    let ratio = |g: &mut Gen| pick(g, &["0", "0.25", "0.5", "0.95", "1", "1.5"]).to_string();
    if kind == 0 {
        if g.below(100) < 60 {
            let r = ratio(g);
            lines.push(format!("NeverHeal = {r}"));
        }
        if g.below(100) < 50 {
            let r = ratio(g);
            lines.push(format!("AlwaysHeal = {r}"));
        }
    } else if g.below(100) < 60 {
        let r = ratio(g);
        lines.push(format!("MinHealth = {r}"));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "ScanRate = x".to_string(),
            1 => "ScanRange = ".to_string(),
            _ => "NoSuchField = 1".to_string(),
        });
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

/// A health, mostly on the thresholds the data names (a maximum of 100).
fn gen_health(g: &mut Gen) -> u32 {
    (*pick(
        g,
        &[&0.0f32, &25.0, &50.0, &94.0, &95.0, &96.0, &100.0, &49.5],
    ))
    .to_bits()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    updates: u64,
    xfers: u64,
    heals: u64,
    enters: u64,
    moves: u64,
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

/// The kinds of object `id` (bits of [`KINDS`]).
fn set_kinds(w: &mut World, id: ObjectId, bits: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, k) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects
        .find_mut(id)
        .unwrap()
        .thing
        .friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The scan: up to four nibbles of ids, 0 ends.
fn scan_list(scan: u32) -> Vec<ObjectId> {
    let mut v = Vec::new();
    for k in 0..4 {
        let id = scan >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 4 {
            v.push(id as ObjectId);
        }
    }
    v
}

fn probe(name: &[u8], answers: ProbeAnswers, max: f32) -> NewModule {
    let mut m = NewModule::plain(name, name);
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        answers,
        max,
        Default::default(),
    )));
    m
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeModule {
    match w.objects.find_mut(id).unwrap().modules[slot as usize]
        .body
        .as_mut()
    {
        Some(UpdateBody::ModuleProbe(p)) => p,
        _ => unreachable!("a probe"),
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    kind: u32,
    data: ModuleDataKind,
    cfg: &[u32; 15],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    w.cuts.team_players.insert(1, 0);
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let class = CLASSES[kind as usize];
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        class.as_bytes(),
        b"ModuleTag_Find",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut ai = probe(b"R19Ai", ProbeAnswers::default(), 0.0);
    ai.is_ai = true;
    let mut body = probe(b"R19Body", ProbeAnswers::default(), f32::from_bits(cfg[2]));
    body.interfaces = MODULEINTERFACE_BODY;
    let mut modules = vec![m, ai, body];
    let n_collide = (cfg[11] & 3) % 3;
    for k in 0..n_collide {
        let mask = cfg[11] >> (4 + 4 * k) & 15;
        let list: Vec<ObjectId> = (0..4)
            .filter(|j| mask >> j & 1 != 0)
            .map(|j| j + 1)
            .collect();
        let mut c = probe(
            b"R19Collide",
            ProbeAnswers {
                collide_with: Some(list),
                ..Default::default()
            },
            0.0,
        );
        c.interfaces = MODULEINTERFACE_COLLIDE;
        modules.push(c);
    }
    let me = w
        .new_object(NewObject {
            template: plain.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules,
        })
        .unwrap();
    assert_eq!(me, 1);
    for k in 2..=4 {
        let mut b = probe(b"R19Body", ProbeAnswers::default(), 100.0);
        b.interfaces = MODULEINTERFACE_BODY;
        let id = w
            .new_object(NewObject {
                template: plain.clone(),
                status: ObjectStatusMaskType::default(),
                team: None,
                modules: vec![b],
            })
            .unwrap();
        assert_eq!(id, k);
    }
    let ai_slot = w.objects.find(me).unwrap().ai;
    let body_slots: Vec<Option<u16>> = (1..=4).map(|k| w.objects.find(k).unwrap().body).collect();
    probe_mut(&mut w, me, body_slots[0].unwrap()).health = f32::from_bits(cfg[1]);
    for k in 2..=4usize {
        probe_mut(&mut w, k as ObjectId, body_slots[k - 1].unwrap()).health =
            f32::from_bits(cfg[1 + k]);
    }
    for k in 0..4 {
        w.cuts
            .distance_squared_center_2d
            .insert((me, k as ObjectId + 1), f32::from_bits(cfg[6 + k]));
    }
    let base = Coord3D {
        x: f32::from_bits(cfg[12]),
        y: f32::from_bits(cfg[13]),
        z: f32::from_bits(cfg[14]),
    };
    w.set_position(
        me,
        &Coord3D {
            x: 10.0,
            y: -2.5,
            z: 1.0,
        },
    );
    let idx = w.find_module(me, class.as_bytes()).unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let (mut flags, mut scan) = (cfg[0], cfg[10]);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            1 => flags = o[1],
            2 => scan = o[1],
            3 => {
                let k = ((o[1] - 1) % 4) as usize;
                probe_mut(&mut w, k as ObjectId + 1, body_slots[k].unwrap()).health =
                    f32::from_bits(o[2]);
            }
            4 => {
                w.cuts
                    .distance_squared_center_2d
                    .insert((me, ((o[1] - 1) % 4) as ObjectId + 1), f32::from_bits(o[2]));
            }
            _ => {}
        }
        w.players.get_nth_mut(0).unwrap().player_type = if flags & 1 != 0 {
            PlayerType::Computer
        } else {
            PlayerType::Human
        };
        w.objects.find_mut(me).unwrap().ai = if flags & 2 != 0 { ai_slot } else { None };
        probe_mut(&mut w, me, ai_slot.unwrap()).answers.idle = flags & 4 != 0;
        w.objects.find_mut(me).unwrap().body = if flags & 8 != 0 { body_slots[0] } else { None };
        if flags & 16 != 0 {
            w.cuts.ai_base_center.insert(0, base);
        } else {
            w.cuts.ai_base_center.remove(&0);
        }
        for k in 2..=4usize {
            w.objects.find_mut(k as ObjectId).unwrap().body = if flags >> (6 + k) & 1 != 0 {
                body_slots[k - 1]
            } else {
                None
            };
            set_kinds(&mut w, k as ObjectId, flags >> (12 + 2 * (k - 2)) & 3);
        }
        w.cuts.range_scan = scan_list(scan);
        let errors_before = w.cuts.module_errors.len();
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
            1..=4 => {}
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                let scan_line = |w: &World, object: ObjectId, range: f32, n: u32| {
                    let p = w.objects.find(object).unwrap().get_position();
                    format!(
                        "scan {:08x} {:08x} {:08x} {:08x} {n}",
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        range.to_bits()
                    )
                };
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::PartitionIterateObjectsInRange { object, range } => {
                        log.push(scan_line(&w, *object, *range, 0))
                    }
                    // the four filters: the kind, alive, the player, the same map status
                    CutCall::PartitionIterateVehiclesNearToFar { object, range } => {
                        log.push(scan_line(&w, *object, *range, 4))
                    }
                    CutCall::PartitionGetDistanceSquaredCenter2D { object, other } => {
                        log.push(format!("distsq {object} {other}"))
                    }
                    CutCall::PlayerGetAiBaseCenter { player } => {
                        log.push(format!("basecenter {player}"))
                    }
                    _ => {}
                }
            } else if let Some(l) = t.strip_prefix("bt ") {
                if l.starts_with("aiheal ") {
                    tally.heals += 1;
                } else if l.starts_with("aienter ") {
                    tally.enters += 1;
                } else if l.starts_with("aimove ") {
                    tally.moves += 1;
                } else if !l.starts_with("wouldcollide ") {
                    continue;
                }
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
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let field =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::AutoFindHealingUpdate(m)) => Some(m.state_words()[0]),
                    Some(UpdateBody::PilotFindVehicleUpdate(m)) => Some(m.state_words()[0]),
                    _ => None,
                });
        put_u32(&mut v, field.expect("the module is back in its slot"));
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R19_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn find_healing_and_vehicle_updates_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0019_F1D0);
    let mut read = Check::new("R19 data read (throw, message)");
    let mut run = Check::new("R19 ops (data, effects, xfers, field)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut compared = [0u64; 2];
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = g.below(2);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R19{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a computer player's unit with an AI and a body
        let mut cfg = [0u32; 15];
        cfg[0] = (u32::from(g.below(5) != 0))
            | u32::from(g.below(8) != 0) << 1
            | u32::from(g.below(3) != 0) << 2
            | u32::from(g.below(8) != 0) << 3
            | u32::from(g.below(2) != 0) << 4
            | (g.u32() | g.u32()) & 0x700
            | g.below(64) << 12;
        cfg[1] = gen_health(&mut g);
        cfg[2] = 100.0f32.to_bits();
        for word in &mut cfg[3..6] {
            *word = gen_health(&mut g);
        }
        for word in &mut cfg[6..10] {
            *word = (*pick(&mut g, &[&0.0f32, &25.0, &100.0, &100.0, &2500.0])).to_bits();
        }
        cfg[10] = (0..4).fold(0, |acc, k| acc | g.below(5) << (4 * k));
        cfg[11] = g.below(3) | g.below(16) << 4 | g.below(16) << 8;
        cfg[12] = 40.0f32.to_bits();
        cfg[13] = (-7.5f32).to_bits();
        cfg[14] = 0.0f32.to_bits();
        let n_ops = 1 + g.below(20) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(20) {
                0..=8 => [0, 0, 0, 0, 0, 0],
                9..=10 => [1, (cfg[0] & !0x1f) | g.below(32), 0, 0, 0, 0],
                11 => [
                    2,
                    (0..4).fold(0, |acc, k| acc | g.below(5) << (4 * k)),
                    0,
                    0,
                    0,
                    0,
                ],
                12..=13 => [3, 1 + g.below(4), gen_health(&mut g), 0, 0, 0],
                14 => [
                    4,
                    1 + g.below(4),
                    (*pick(&mut g, &[&0.0f32, &25.0, &100.0])).to_bits(),
                    0,
                    0,
                    0,
                ],
                15..=16 => [7, 0, 0, 0, 0, 0],
                17..=18 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, kind, data.expect("read ok"), &cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
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
            cpp::ref_l9r19_run(
                kind as i32,
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
                "case {case}: kind {kind} cfg {cfg:x?}, file {:?}, ops {ops:?}",
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
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} kind {kind} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared[kind as usize] += 1;
    }
    println!(
        "R19: {cases} cases ({} AutoFindHealingUpdate, {} PilotFindVehicleUpdate compared), {read_failed} data blocks threw, {excluded} excluded \
         (undefined upstream), {ops_run} ops, {} updates, {} xfers, {} effect lines, {} heals, {} enters, {} moves to base",
        compared[0], compared[1], tally.updates, tally.xfers, tally.effects, tally.heals, tally.enters, tally.moves
    );
    read.finish();
    run.finish();
    assert!(
        tally.heals * 10 > cases as u64
            && tally.enters * 10 > cases as u64
            && tally.moves * 20 > cases as u64,
        "the generator must reach every order"
    );
}
