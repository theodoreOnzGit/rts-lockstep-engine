#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HordeUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `HordeUpdate` (`logic::modules::horde_update`) against the unchanged C++, `p1w13-sonnet`, batch R25.
//!
//! # Method
//!
//! The C++ side is `HordeUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_horde_r25.inc`, `ref_l9r25_run`) on the
//! module build's stand-ins: the module's object (id 1: kinds, maybe an AI whose `evaluateMoraleBonus` logs, maybe a drawable whose terrain decal
//! calls log and which keeps its decal type, the weapon bonus conditions, off the map or not, effectively dead or not, a major radius) and three
//! more (kinds, maybe a horde interface whose `isTrueHordeMember` answers from the test, maybe sharing the object's template, off the map or not,
//! their team's relationship to the object's); the height-die driver's object range scan, filtered by the module's own
//! `PartitionFilterHordeMember` as the partition manager does; the ActiveBody driver's distances; the REAL `Object::getRelationship`; the game
//! logic (the frame, the draw-icon flag, the REAL logic random values) whose wake calls are counted; and the REAL `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` AI and horde
//! interfaces), the team relationships, the scan `cuts.range_scan` (the module filters it), the distances `cuts.distance_squared_center_2d`;
//! every op an `update()` as the scheduler runs it, a module call or a change to the scene, a load followed by the game logic's post-process.
//!
//! Each case reads a block `D` (every field), mostly well formed, and runs 1 to 30 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's wake calls (its random first wake); per op the throw status, the sleep time, the effects in order (`scan`, `distsq`, `morale`,
//! `decal`, `decalsize`, `decalfade`, `subshow`, `subupdate`), the wake calls, the xfer status, bytes and CRC, the state and the drawable's decal.
//!
//! # Results (2026-10-23, `L9R25_CASES=9000`)
//!
//! 9,000 cases (7,198 compared, none excluded; 1,802 data files threw, as the C++ did), 111,045 ops (46,266 updates, 23,077 xfers) and 125,441
//! outputs: **0 mismatches**; 73,077 effect lines compared, 41,074 ops ended in a horde, 2,800 rub-off distances and 8,483 decals.
//!
//! On the way: (1) a translation fix: the object's own scan includes itself, and `getHUI` on itself is this module, lent out of its slot while it
//! updates, so its `isTrueHordeMember` is the live one (just cleared), not the slot's copy (13 mismatches in 28,860 outputs before); (2) a driver
//! fix: `UpdateRate = 0` draws `GameLogicRandomValue(1, 0) = 0`, a wake at frame 0, which upstream's `registerObject` makes frame 1 at frame 0 (the
//! Rust world already did; the driver now does too); (3) the generator gives more counts above the scan and more true members, for the rub-off.
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1951 | `ExactMatch` ignored by the filter | 2,043 |
//! | 1952 | the horde test runs every update, not every `UpdateRate` | 29,412 |
//! | 1953 | no rub-off | 5,248 |
//! | 1954 | a vehicle's decal set even when unchanged | 2,127 |
//! | 1955 | `m_hasFlag` not saved | 9,213 |

mod support;
mod up_common;

use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::horde_update::HordeUpdateModuleData;
use rts_lockstep_engine::logic::weapon_bonus::condition;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{PRIVATE_EFFECTIVELY_DEAD, PRIVATE_OFF_MAP};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r25.bin");
const XFER_FILE: &str = "l9_r25.bin";
/// The kinds the objects are given, by bit (the driver's `g_r25Kinds`), with their INI names.
const KINDS: [(KindOfType, &str); 4] = [
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_PORTABLE_STRUCTURE, "PORTABLE_STRUCTURE"),
    (KINDOF_AIRCRAFT, "AIRCRAFT"),
];

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
    *out = Some(ModuleDataKind::HordeUpdate(Arc::new(
        HordeUpdateModuleData::friend_new_module_data(Some(ini))?,
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

fn put_data(v: &mut Vec<u8>, d: &HordeUpdateModuleData) {
    put_u32(v, d.update_rate);
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(d.kindof.test(i)));
    }
    put_i32(v, d.min_count);
    put_u32(v, d.min_dist.to_bits());
    put_u32(v, d.rub_off_radius.to_bits());
    put_u32(v, u32::from(d.allies_only));
    put_u32(v, u32::from(d.exact_match));
    put_i32(v, d.action);
    put_u32(v, d.flag_sub_obj_names.len() as u32);
    for n in &d.flag_sub_obj_names {
        put_str(v, n);
    }
    put_u32(v, u32::from(d.allowed_nationalism));
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
    let rate = u32_(&mut p);
    let kinds: Vec<u32> = (0..KINDOF_COUNT).map(|_| u32_(&mut p)).collect();
    let set: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0)
        .map(|(i, _)| i)
        .collect();
    let f: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let names: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let nat = u32_(&mut p);
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data rate {rate} kinds {set:?} {f:x?} names {names:?} nat {nat} constructor wakes {w} last {l}"));
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
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 70 {
        lines.push(format!(
            "UpdateRate = {}",
            pick(g, &["0", "33", "100", "500", "1000"])
        ));
    }
    if g.below(100) < 50 {
        let mut t = Vec::new();
        for _ in 0..1 + g.below(2) {
            t.push(KINDS[g.below(2) as usize].1);
        }
        lines.push(format!("KindOf = {}", t.join(" ")));
    }
    if g.below(100) < 85 {
        lines.push(format!(
            "Count = {}",
            pick(g, &["0", "1", "2", "3", "4", "4", "5"])
        ));
    }
    if g.below(100) < 70 {
        lines.push(format!("Radius = {}", pick(g, &["0", "30", "60.5"])));
    }
    if g.below(100) < 60 {
        lines.push(format!(
            "RubOffRadius = {}",
            pick(g, &["0", "10", "20", "50"])
        ));
    }
    if g.below(100) < 50 {
        lines.push(format!("AlliesOnly = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 40 {
        lines.push(format!("ExactMatch = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 20 {
        lines.push("Action = HORDE".to_string());
    }
    if g.below(100) < 40 {
        lines.push(format!(
            "FlagSubObjectNames = {}",
            pick(g, &["Flag01", "Flag01 Flag02", "FLAG"])
        ));
    }
    if g.below(100) < 40 {
        lines.push(format!("AllowedNationalism = {}", pick(g, &["Yes", "No"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "Action = MOB".to_string(),
            1 => "Count = x".to_string(),
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

#[derive(Default)]
struct Tally {
    effects: u64,
    updates: u64,
    xfers: u64,
    in_horde: u64,
    rub_offs: u64,
    decals: u64,
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

fn template_of(bits: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn relationship(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn set_private(w: &mut World, id: ObjectId, bit: u8, on: bool) {
    let o = w.objects.find_mut(id).unwrap();
    if on {
        o.private_status |= bit;
    } else {
        o.private_status &= !bit;
    }
}

/// The scene of the flags (the driver's, op by op).
#[allow(clippy::too_many_arguments)]
fn set_scene(
    w: &mut World,
    flags: &[u32; 4],
    drawable: i32,
    ai: Option<u16>,
    horde_slots: &[Option<u16>; 4],
) {
    let f0 = flags[0];
    let my_template = template_of(f0 & 15);
    w.objects
        .find_mut(1)
        .unwrap()
        .thing
        .friend_set_template(my_template.clone());
    w.objects.find_mut(1).unwrap().ai = if f0 >> 4 & 1 != 0 { ai } else { None };
    w.objects.find_mut(1).unwrap().drawable = if f0 >> 5 & 1 != 0 {
        drawable
    } else {
        rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID
    };
    set_private(w, 1, PRIVATE_OFF_MAP, f0 >> 6 & 1 != 0);
    set_private(w, 1, PRIVATE_EFFECTIVELY_DEAD, f0 >> 7 & 1 != 0);
    w.objects.find_mut(1).unwrap().weapon_bonus_condition =
        (f0 >> 8 & 1) << condition::NATIONALISM | (f0 >> 9 & 1) << condition::FANATICISM;
    for k in 1..4usize {
        let f = flags[k];
        let id = k as ObjectId + 1;
        let t = if f >> 7 & 1 != 0 {
            my_template.clone()
        } else {
            template_of(f & 15)
        };
        w.objects.find_mut(id).unwrap().thing.friend_set_template(t);
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules
            [horde_slots[k].unwrap() as usize]
            .body
            .as_mut()
        {
            p.answers.horde = (f >> 4 & 1 != 0).then_some(f >> 5 & 1 != 0);
        }
        set_private(w, id, PRIVATE_OFF_MAP, f >> 6 & 1 != 0);
        w.cuts
            .team_relationships
            .insert((1, id as u32), relationship(f >> 8 & 3));
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    cfg: &[u32; 12],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let ModuleDataKind::HordeUpdate(d) = &data else {
        unreachable!()
    };
    put_data(&mut v, d);
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.globals.draw_icon_ui = cfg[7] != 0;
    w.set_frame(cfg[0]);
    w.random.init_game_logic_random(cfg[1]);
    for t in 1..=4 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
    }
    let probe = |answers: ProbeAnswers| {
        let mut m = NewModule::plain(b"R25Probe", b"ModuleTag_Probe");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
            answers,
            0.0,
            Default::default(),
        )));
        m
    };
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"HordeUpdate",
        b"ModuleTag_Horde",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut ai = probe(ProbeAnswers::default());
    ai.is_ai = true;
    // object 1's kinds before the constructor (it reads none, but the template is the scene's)
    let me = w
        .new_object(NewObject {
            template: template_of(cfg[2] & 15),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m, ai],
        })
        .unwrap();
    assert_eq!(me, 1);
    for k in 2..=4 {
        let id = w
            .new_object(NewObject {
                template: template_of(0),
                status: ObjectStatusMaskType::default(),
                team: Some(k),
                modules: vec![probe(ProbeAnswers::default())],
            })
            .unwrap();
        assert_eq!(id, k as ObjectId);
    }
    let idx = w.find_module(me, b"HordeUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let ai_slot = w.objects.find(me).unwrap().ai;
    let mut horde_slots = [None; 4];
    for (k, slot) in horde_slots.iter_mut().enumerate().skip(1) {
        *slot = w.find_module(k as ObjectId + 1, b"R25Probe");
    }
    // the constructor's wake calls
    let (mut cw, mut cl) = (0u32, 0u32);
    for t in w.cuts.trace.take().unwrap() {
        if let Some(f) = t.strip_prefix("awaken ") {
            let (i, f) = f.split_once(' ').unwrap();
            if i.parse::<i32>().unwrap() == i32::from(idx) {
                cw += 1;
                cl = f.parse().unwrap();
            }
        }
    }
    put_u32(&mut v, cw);
    put_u32(&mut v, cl);
    w.friend_bind_to_drawable(me, 7);
    let drawable = w.objects.find(me).unwrap().drawable;
    let mut geom = w.objects.find(me).unwrap().geometry_info.clone();
    geom.set_major_radius(f32::from_bits(cfg[6]));
    w.objects.find_mut(me).unwrap().geometry_info = geom;
    let mut flags = [cfg[2], cfg[3], cfg[4], cfg[5]];
    let mut scan = cfg[8];
    for k in 0..3 {
        w.cuts
            .distance_squared_center_2d
            .insert((me, k as ObjectId + 2), f32::from_bits(cfg[9 + k]));
    }
    let mut saved: Vec<u8> = Vec::new();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    for o in ops {
        match o[0] {
            1 => flags[((o[1] - 1) % 4) as usize] = o[2],
            2 => scan = o[1],
            3 => w.set_frame(o[1]),
            4 => {
                w.cuts
                    .distance_squared_center_2d
                    .insert((me, ((o[1] - 1) % 4) as ObjectId + 1), f32::from_bits(o[2]));
            }
            6 => w.cuts.globals.draw_icon_ui = o[1] != 0,
            _ => {}
        }
        set_scene(&mut w, &flags, drawable, ai_slot, &horde_slots);
        w.cuts.range_scan = (0..4)
            .map(|k| scan >> (4 * k) & 15)
            .take_while(|&id| id != 0)
            .filter(|&id| id <= 4)
            .map(|id| id as ObjectId)
            .collect();
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
            5 => {
                w.module_call(me, idx, ModuleCall::OnDrawableBoundToObject);
            }
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                if o[0] == 8 {
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            1..=4 | 6 => {}
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
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
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::PartitionIterateObjectsInRangeOf { object, range, dc } => {
                        log.push(format!("scan {object} {:08x} {dc}", range.to_bits()))
                    }
                    CutCall::PartitionGetDistanceSquaredCenter2D { object, other } => {
                        tally.rub_offs += 1;
                        log.push(format!("distsq {object} {other}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::DrawableSetTerrainDecal { decal, .. }) => {
                        tally.decals += 1;
                        log.push(format!("decal {decal}"))
                    }
                    Some(ObjectClientEvent::DrawableSetTerrainDecalSize { x, y, .. }) => {
                        log.push(format!("decalsize {:08x} {:08x}", x.to_bits(), y.to_bits()))
                    }
                    Some(ObjectClientEvent::DrawableTerrainDecalFadeTarget {
                        target,
                        rate,
                        ..
                    }) => log.push(format!(
                        "decalfade {:08x} {:08x}",
                        target.to_bits(),
                        rate.to_bits()
                    )),
                    Some(ObjectClientEvent::DrawableShowSubObject { name, show, .. }) => {
                        log.push(format!(
                            "subshow {} {}",
                            String::from_utf8_lossy(&name),
                            i32::from(show)
                        ))
                    }
                    Some(ObjectClientEvent::DrawableUpdateSubObjects { .. }) => {
                        log.push("subupdate".to_string())
                    }
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if let Some(l) = t.strip_prefix("bt ") {
                if l.starts_with("morale ") {
                    log.push(l.to_string());
                }
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
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::HordeUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        let state = state.expect("the module is back in its slot");
        if state[1] != 0 {
            tally.in_horde += 1;
        }
        for s in state {
            put_u32(&mut v, s);
        }
        put_i32(&mut v, w.objects.find(me).unwrap().terrain_decal_type);
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R25_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn horde_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0025_4D0E);
    let mut read = Check::new("R25 data read (throw, message)");
    let mut run = Check::new("R25 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R25{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // object 1: mostly infantry or a vehicle, with an AI and a drawable; the others mostly horde members, allies
        let gen_me = |g: &mut Gen| {
            *pick(g, &[&1u32, &1, &2, &2, &3, &5, &0])
                | u32::from(g.below(6) != 0) << 4
                | u32::from(g.below(6) != 0) << 5
                | u32::from(g.below(10) == 0) << 6
                | u32::from(g.below(10) == 0) << 7
                | g.below(4) << 8
        };
        let gen_other = |g: &mut Gen| {
            g.below(4)
                | u32::from(g.below(5) != 0) << 4
                | u32::from(g.below(4) != 0) << 5
                | u32::from(g.below(10) == 0) << 6
                | u32::from(g.below(4) == 0) << 7
                | (if g.below(3) == 0 { g.below(3) } else { 2 }) << 8
        };
        let gen_scan = |g: &mut Gen| (0..4).fold(0, |acc, k| acc | g.below(5) << (4 * k));
        let gen_dist = |g: &mut Gen| (*pick(g, &[&0.0f32, &100.0, &400.0, &2500.0])).to_bits();
        let cfg = [
            g.below(200),
            g.u32(),
            gen_me(&mut g),
            gen_other(&mut g),
            gen_other(&mut g),
            gen_other(&mut g),
            (*pick(&mut g, &[&1.0f32, &10.0, &12.5])).to_bits(),
            u32::from(g.below(6) != 0),
            gen_scan(&mut g),
            gen_dist(&mut g),
            gen_dist(&mut g),
            gen_dist(&mut g),
        ];
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(24) {
                0..=9 => [0, 0, 0, 0, 0, 0],
                10..=11 => {
                    let k = 1 + g.below(4);
                    [
                        1,
                        k,
                        if k == 1 {
                            gen_me(&mut g)
                        } else {
                            gen_other(&mut g)
                        },
                        0,
                        0,
                        0,
                    ]
                }
                12..=13 => [2, gen_scan(&mut g), 0, 0, 0, 0],
                14..=15 => {
                    frame += 1 + g.below(60);
                    [3, frame, 0, 0, 0, 0]
                }
                16 => [4, 2 + g.below(3), gen_dist(&mut g), 0, 0, 0],
                17 => [5, 0, 0, 0, 0, 0],
                18 => [6, u32::from(g.below(4) != 0), 0, 0, 0, 0],
                19..=20 => [7, 0, 0, 0, 0, 0],
                21..=22 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r25_run(
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
        "R25: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} xfers, \
         {} effect lines, {} ops in a horde, {} rub-off distances, {} decals",
        tally.updates, tally.xfers, tally.effects, tally.in_horde, tally.rub_offs, tally.decals
    );
    read.finish();
    run.finish();
    assert!(
        tally.in_horde * 2 > cases as u64
            && tally.rub_offs * 4 > cases as u64
            && tally.decals * 4 > cases as u64,
        "the generator must reach every path"
    );
}
