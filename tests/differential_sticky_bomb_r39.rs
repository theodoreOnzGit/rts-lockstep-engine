#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StickyBombUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `StickyBombUpdate` (`logic::modules::sticky_bomb_update`) against the unchanged C++, `p1w13-sonnet`, batch R39.
//!
//! # Method
//!
//! The C++ side is the file (one mechanical edit: a function-local `static` name key made a plain local, so that the per-case reset of the
//! name key generator cannot leave it stale; the class compiled under another name, because RebuildHoleBehavior's driver already declares
//! it in the unit), in `wrap_module.cpp`'s unit (`differential/l9_sticky_bomb_r39.inc`, `ref_l9r39_run`), on the module build's stand-ins:
//! six objects (ids 1-6; the kinds `IMMOBILE`, `BOOBY_TRAP` and `STRUCTURE`, effectively dead, maybe a body at health 500 of 1000 whose
//! `attemptDamage` is a line, maybe an AI with the test's goal, a sphere geometry, a position, the `BOOBY_TRAPPED` status bit, which the module
//! sets and clears), the bomb (object 1: maybe a player, maybe a `LifetimeUpdate` stand-in with the test's die frame, a producer), the
//! template's sounds `StickyBombCreated` and `UnitBombPing`, the audio manager, the terrain's eight cells, the scan (the objects and numerics
//! the test lists), the weapon store (`WEAP_A`, `WEAP_B` with the test's damage numbers), the FX lists (`FX_A`, `FX_B`), `destroyObject`
//! (a line), the REAL `Object::kill`, `UpdateModule`, `INI` and `Xfer`, and the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`; `ProbeModule` bodies, AIs with
//! the goal and a probe named `LifetimeUpdate` with `lifetime_die_frame`; templates with the two sounds), `cuts.terrain_ground`,
//! `cuts.range_scan` and `range_scan_numeric`, a weapon store with the two templates, `cuts.body.stub_object_calls` (the destruction is a
//! line); the real `World::attempt_damage`, `kill` and `set_status`.
//!
//! Each case reads a block `D` (mostly well formed), makes the bomb, gives it its producer, and runs 1 to 80 ops: an `update()`, a change to
//! the frame or the scene, `initStickyBomb`, `detonate`, one of the four accessors, `onObjectCreated` or an xfer. Compared, bit for bit: the
//! read's throw; the data; the constructor's effects and wake calls; per op the throw status, the result, the effects in order (the ground
//! reads, the sounds, the scan, the damage, the FX, the destruction), the wake calls, the xfer status, bytes and CRC, the module's state, the
//! bomb's position and producer, and per object `BOOBY_TRAPPED` and the health.
//!
//! # Results (2026-10-24, `L9R39_CASES=9000`)
//!
//! 9,000 cases (6,755 compared; 1,828 data blocks threw, as the C++ did; 417 excluded, undefined upstream: 252 geometry-based detonations
//! of a bomb with no player, 165 detonations of a bomb with no body), 271,926 ops (95,129 updates, 27,105 inits, 13,436 detonations, 34,109
//! xfers) and 557,362 outputs: **0 mismatches**; 124,549 effect lines compared: 27,032 pings, 29,411 created sounds, 25,656 damage lines
//! (the victims' and the bomb's own death), 1,533 FX and 5,324 destructions (a dead target).
//!
//! On the way: the first runs differed only in the saved `UpdateModule` base after a load and a wake: the test did not run the game logic's
//! post-process after a load, as R37's does (`World::load_post_process`). Fixed in the test.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2091 | the created sound on the bomb, not at a position alone | 29,411 |
//! | 2092 | the countdown's pings 29 frames apart | 110,614 |
//! | 2093 | no `OffsetZ` above a mobile target when stuck | 58,933 |
//! | 2094 | an immobile target followed, not the ground | 70,586 |
//! | 2095 | no ping on the ping frame itself | 5,106 |
//! | 2096 | the primary radius not grown by the target's size | 4,335 |
//! | 2097 | every victim takes the primary damage | 2,358 |
//! | 2098 | `BOOBY_TRAPPED` not cleared on detonating | 47,643 |
//! | 2099 | the next ping frame not saved | 39,667 |
//!
//! Two first drafts were replaced: 2091 (the producer left unset) changed which object the bomb took as its shooter, so the Rust side
//! skipped undefined paths the C++ then crashed on; 2097 (`<` for `<=`) is never reached by these numerics.

mod support;
mod up_common;

use rts_lockstep_engine::common::audio_event_name::AudioEventName;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_BOOBY_TRAPPED;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::sticky_bomb_update::StickyBombUpdateModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r39.bin");
const XFER_FILE: &str = "l9_r39.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r39Kinds`).
const KINDS: [KindOfType; 3] = [KINDOF_IMMOBILE, KINDOF_BOOBY_TRAP, KINDOF_STRUCTURE];
/// The spheres' radii, by index (the driver's `g_r39Radius`).
const RADIUS: [f32; 4] = [1.0, 5.0, 20.0, 0.5];
/// The two weapons' damage, death and damage status types (the driver's).
const WEAPON_TYPES: [(i32, i32, i32); 2] = [(3, 2, 0), (7, 5, 4)];

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
    *out = Some(ModuleDataKind::StickyBombUpdate(Arc::new(StickyBombUpdateModuleData::friend_new_module_data(Some(ini))?)));
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
    let bone = str_(&mut p);
    let head: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {bone:?} {head:x?} constructor {log:?} wakes {w} last {l}"));
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
        let state: Vec<u32> = (0..3 + 4 + 12).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 30 {
        lines.push(format!("AttachToTargetBone = {}", pick(g, &["BONE01", "Turret"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("OffsetZ = {}", pick(g, &["10", "2.5", "0", "-1"])));
    }
    if g.below(100) < 75 {
        lines.push(format!("GeometryBasedDamageWeapon = {}", pick(g, &["WEAP_A", "WEAP_B", "WEAP_A", "None", "NoSuchWeapon"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("GeometryBasedDamageFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "OffsetZ = x", "OffsetZ"]).to_string());
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
    inits: u64,
    detonations: u64,
    xfers: u64,
    effects: u64,
    pings: u64,
    created: u64,
    damage: u64,
    fx: u64,
    destroys: u64,
    trapped: u64,
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

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R39Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    let mut t = ThingTemplate::from_object_values(&values);
    t.friend_set_per_unit_sound(b"StickyBombCreated", AudioEventName { event_name: b"R39Created".to_vec() });
    t.friend_set_per_unit_sound(b"UnitBombPing", AudioEventName { event_name: b"R39Ping".to_vec() });
    Arc::new(t)
}

/// The slots of each object's body and AI, and the bomb's lifetime stand-in.
struct Slots {
    body: [u16; 6],
    ai: [u16; 6],
    life: u16,
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 24], s: &Slots) {
    let tp = cfg[1];
    w.objects.find_mut(1).unwrap().team = if (1..=3).contains(&tp) { tp } else { 0 };
    {
        let o = w.objects.find_mut(1).unwrap();
        let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.life as usize].body.as_mut() else { unreachable!() };
        p.answers.lifetime_die_frame = (cfg[2] != 0).then_some(cfg[3]);
    }
    for k in 0..6usize {
        let f = cfg[4 + k];
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.thing.friend_set_template(template_of(f & 7));
        if f >> 3 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.body = (f >> 4 & 1 != 0).then_some(s.body[k]);
        o.ai = (f >> 5 & 1 != 0).then_some(s.ai[k]);
        let Some(UpdateBody::ModuleProbe(p)) = o.modules[s.ai[k] as usize].body.as_mut() else { unreachable!() };
        p.answers.goal_object = cfg[10] as ObjectId;
    }
    let (mut scan, mut nums) = (Vec::new(), Vec::new());
    for k in 0..6u32 {
        let id = cfg[18] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 6 {
            scan.push(id as ObjectId);
            nums.push(5.0 * (k * k) as f32);
        }
    }
    w.cuts.range_scan = scan;
    w.cuts.range_scan_numeric = nums;
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    let ModuleDataKind::StickyBombUpdate(d) = &data else { unreachable!() };
    put_str(&mut v, &d.attach_to_bone);
    put_u32(&mut v, d.offset_z.to_bits());
    let weapon = match d.geometry_based_damage_weapon.as_deref() {
        Some(n) if n.eq_ignore_ascii_case(b"WEAP_A") => 1,
        Some(n) if n.eq_ignore_ascii_case(b"WEAP_B") => 2,
        _ => 0,
    };
    put_i32(&mut v, weapon);
    put_i32(&mut v, fx.id(&d.geometry_based_damage_fx));
    let mut w = World::new();
    w.names = ctx.names.clone();
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    for (k, n) in ["WEAP_A", "WEAP_B"].iter().enumerate() {
        let i = weapon_store.new_weapon_template(&mut w.names, n.as_bytes()).unwrap().unwrap();
        let t = &mut weapon_store.templates_mut()[i];
        t.primary_damage = (cfg[20 + k] & 0xFFFF) as f32;
        t.secondary_damage = (cfg[20 + k] >> 16) as f32;
        t.primary_damage_radius = (cfg[22 + k] & 0xFFFF) as f32;
        t.secondary_damage_radius = (cfg[22 + k] >> 16) as f32;
        (t.damage_type, t.death_type, t.damage_status_type) = WEAPON_TYPES[k];
    }
    w.statics = Arc::new(GameStatics { weapon_store, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.stub_object_calls = true;
    for c in 0..8 {
        w.cuts.terrain_ground[c] = (cfg[19] >> (4 * c) & 15) as f32 * 0.5;
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
        let mut p = ProbeModule::new(answers, max, Default::default());
        p.health = max * 0.5;
        m.body = Some(UpdateBody::ModuleProbe(p));
        m
    };
    let mut slots = Slots { body: [0; 6], ai: [0; 6], life: 0 };
    for k in 0..6usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"StickyBombUpdate", b"ModuleTag_Me", &data).unwrap().unwrap());
            modules.push(probe(b"LifetimeUpdate", ProbeAnswers::default(), 0.0, 0));
        }
        modules.push(probe(b"R39Body", ProbeAnswers::default(), 1000.0, MODULEINTERFACE_BODY));
        let mut ai = probe(b"R39Ai", ProbeAnswers::default(), 0.0, 0);
        ai.is_ai = true;
        modules.push(ai);
        let mut status = rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::default();
        if cfg[4 + k] >> 8 & 1 != 0 {
            status.set(OBJECT_STATUS_BOOBY_TRAPPED, 1);
        }
        let id = w.new_object(NewObject { template: template_of(0), status, team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        slots.body[k] = w.find_module(id, b"R39Body").unwrap();
        slots.ai[k] = w.find_module(id, b"R39Ai").unwrap();
        let r = RADIUS[(cfg[4 + k] >> 6 & 3) as usize];
        let o = w.objects.find_mut(id).unwrap();
        o.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, r, r, r);
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[11 + k]), y: 10.0 * k as f32, z: k as f32 });
        if k == 0 {
            slots.life = w.find_module(id, b"LifetimeUpdate").unwrap();
        }
    }
    let idx = w.find_module(1, b"StickyBombUpdate").unwrap();
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
                    Some(CutCall::TerrainGetGroundHeight { x, y }) => log.push(format!("ground {:08x} {:08x}", b(x), b(y))),
                    Some(CutCall::PartitionIterateObjectsInRangeAt { position: p, range, dc }) => {
                        log.push(format!("scan {:08x} {:08x} {:08x} {:08x} {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                    }
                    Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                        tally.destroys += 1;
                        log.push(format!("destroy {object}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                        if position.is_some() {
                            tally.created += 1;
                        } else {
                            tally.pings += 1;
                        }
                        let pos = match position {
                            Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)));
                    }
                    Some(ObjectClientEvent::FxListDoFxPosRadius { fx: name, position: p, radius }) => {
                        tally.fx += 1;
                        log.push(format!("fxpos {} {},{},{} r{:08x}", fx.id(&Some(name)), b(p.x), b(p.y), b(p.z), b(radius)));
                    }
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if t.starts_with("damage ") {
                tally.damage += 1;
                log.push(t.clone());
            }
        }
        (log, wakes, last)
    };
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
    // the producer is set after the object is made
    w.objects.find_mut(1).unwrap().producer_id = cfg[17] as ObjectId;
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=18).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
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
                tally.inits += 1;
                let specific_pos = (o[3] != 0).then(|| Coord3D { x: f32::from_bits(o[3]), y: 3.0, z: 99.0 });
                w.module_call(1, idx, ModuleCall::StickyBombInit { target: o[1] as ObjectId, bomber: o[2] as ObjectId, specific_pos });
            }
            4 => {
                tally.detonations += 1;
                w.module_call(1, idx, ModuleCall::Detonate);
            }
            5 => res = w.module_call(1, idx, ModuleCall::StickyBombGetTargetObject).as_id() as u32,
            6 => {
                w.module_call(1, idx, ModuleCall::StickyBombSetTargetObject(o[1] as ObjectId));
            }
            7 => res = u32::from(w.module_call(1, idx, ModuleCall::StickyBombIsTimedBomb).as_bool()),
            8 => {
                res = match w.module_call(1, idx, ModuleCall::StickyBombGetDetonationFrame) {
                    ModuleReply::Int(f) => f as u32,
                    _ => panic!("the module answers"),
                }
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
            12 => {
                w.module_call(1, idx, ModuleCall::OnObjectCreated);
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
        let Some(UpdateBody::StickyBombUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        for s in m.state_words() {
            put_u32(&mut v, s);
        }
        let p = ob.get_position();
        put_u32(&mut v, p.x.to_bits());
        put_u32(&mut v, p.y.to_bits());
        put_u32(&mut v, p.z.to_bits());
        put_u32(&mut v, ob.producer_id as u32);
        for k in 0..6usize {
            let o = w.objects.find(k as ObjectId + 1).unwrap();
            let trapped = o.test_status(OBJECT_STATUS_BOOBY_TRAPPED);
            tally.trapped += u64::from(trapped);
            put_u32(&mut v, u32::from(trapped));
            let Some(UpdateBody::ModuleProbe(pb)) = o.modules[slots.body[k] as usize].body.as_ref() else { unreachable!() };
            put_u32(&mut v, pb.health.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R39_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

/// An object id: none (0), one of objects 2-6 (mostly the targets 3-6), the bomb (rarely), or no object (7).
fn gen_id(g: &mut Gen) -> u32 {
    match g.below(14) {
        0 => 0,
        1 => 7,
        2 => 2,
        _ => 3 + g.below(4),
    }
}

/// An object's scene word (`k` its index; the start-only bits 6-8 are kept as `keep` has them).
fn gen_object(g: &mut Gen, k: usize, keep: u32) -> u32 {
    let kinds = (0..3).fold(0, |acc, b| acc | u32::from(g.below(if b == 1 && k == 0 { 2 } else { 3 }) == 0) << b);
    let dead = u32::from(g.below(if k == 0 { 40 } else { 12 }) == 0) << 3;
    let body = u32::from(g.below(if k == 0 { 30 } else { 8 }) != 0) << 4;
    let ai = u32::from(g.below(3) != 0) << 5;
    kinds | dead | body | ai | (keep & (7 << 6))
}

#[test]
fn sticky_bomb_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0039_5B0B);
    let mut read = Check::new("R39 data read (throw, message)");
    let mut run = Check::new("R39 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R39{}.ini", case % 5).into_bytes();
        let mut cfg = [0u32; 24];
        cfg[0] = g.below(200);
        cfg[1] = if g.below(8) == 0 { 0 } else { 1 + g.below(3) };
        cfg[2] = u32::from(g.below(2) == 0);
        cfg[3] = if g.below(10) == 0 { g.below(100) } else { cfg[0] + g.below(400) };
        for k in 0..6 {
            let start = (g.below(4) << 6) | u32::from(k != 0 && g.below(8) == 0) << 8;
            cfg[4 + k] = gen_object(&mut g, k, start);
        }
        cfg[10] = gen_id(&mut g);
        for k in 0..6 {
            cfg[11 + k] = (g.below(200) as f32 - 100.0).to_bits();
        }
        cfg[17] = if g.below(6) == 0 { gen_id(&mut g) } else { 2 };
        let gen_scan = |g: &mut Gen| (0..6).fold(0, |acc, k| acc | (1 + g.below(6)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(6)))) - 1);
        cfg[18] = gen_scan(&mut g);
        cfg[19] = g.below(u32::MAX);
        for w in 0..2 {
            cfg[20 + w] = (g.below(300) << 16) | g.below(300);
            cfg[22 + w] = (g.below(60) << 16) | g.below(60);
        }
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let n_ops = 1 + g.below(80) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=13 => [0, 0, 0, 0],
                14..=18 => {
                    frame += 1 + g.below(40);
                    [1, frame, 0, 0]
                }
                19..=22 => {
                    let a = 1 + g.below(18);
                    let b = match a {
                        1 => if g.below(8) == 0 { 0 } else { 1 + g.below(3) },
                        2 => g.below(2),
                        3 => frame + g.below(300),
                        4..=9 => gen_object(&mut g, a as usize - 4, cfg[a as usize]),
                        11..=16 => (g.below(200) as f32 - 100.0).to_bits(),
                        18 => gen_scan(&mut g),
                        _ => gen_id(&mut g),
                    };
                    [2, a, b, 0]
                }
                23..=26 => [3, gen_id(&mut g), gen_id(&mut g), if g.below(4) == 0 { (g.below(50) as f32).to_bits() } else { 0 }],
                27..=28 => [4, 0, 0, 0],
                29 => [5, 0, 0, 0],
                30 => [6, gen_id(&mut g), 0, 0],
                31 => [7, 0, 0, 0],
                32 => [8, 0, 0, 0],
                33..=34 => [9, 0, 0, 0],
                35..=36 => [10, 0, 0, 0],
                37 => [11, 0, 0, 0],
                _ => [12, 0, 0, 0],
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
            cpp::ref_l9r39_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R39: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, \
         {} inits, {} detonations, {} xfers), {} effect lines ({} pings, {} created sounds, {} damage lines, {} FX, {} destructions; {} trapped \
         object-ops; the compared cases')",
        tally.updates, tally.inits, tally.detonations, tally.xfers, tally.effects, tally.pings, tally.created, tally.damage, tally.fx, tally.destroys, tally.trapped
    );
    println!("R39 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(
        tally.pings > cases as u64 && tally.created > cases as u64 && tally.damage > cases as u64 && tally.fx > cases as u64 / 10 && tally.destroys > cases as u64 / 20,
        "the generator must reach every effect"
    );
}
