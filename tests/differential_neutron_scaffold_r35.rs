#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/NeutonBlastBehavior.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `NeutronBlastBehavior` (`logic::modules::neutron_blast_behavior`) and `BridgeScaffoldBehavior` (`logic::modules::bridge_scaffold_behavior`)
//! against the unchanged C++, `p1w13-sonnet`, batch R35.
//!
//! # Method
//!
//! The C++ side is the two files, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_neutron_scaffold_r35.inc`,
//! `ref_l9r35_run(kind, ...)`), on the module build's stand-ins: five objects (ids 1-5; kinds, the airborne status bit, maybe a contain module
//! whose `killAllContained` is a line, maybe an AI whose `aiIdle` is a line, maybe a drawable whose `setTerrainDecal` is a line, maybe a team;
//! a body whose `attemptDamage` is a line; a position); the teams' relationships; the neutral player's default team (9); the scan (the objects
//! the test lists, taken as those that pass the filters; the call a line); `setDisabled`, `deselectObject`, `setTeam` and `destroyObject`
//! (lines); the REAL `Object::kill`, `UpdateModule`, `INI` and `Xfer`; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`; `ProbeModule` bodies, contain
//! modules with `trace_kill_contained` and AIs, which the scene plugs in or out), three teams and `cuts.team_relationships`,
//! `cuts.neutral_default_team`, `cuts.range_scan`, `cuts.body.stub_object_calls` (the disable, team and destroy are lines); every op an
//! `update()` as the scheduler runs it, a death, one of the scaffold interface's calls, a change to the scene or the frame, or an xfer.
//!
//! Each case picks a class (a third `BridgeScaffoldBehavior`), reads a block `D` (mostly well formed) and runs 1 to 80 ops (the scaffold up to
//! 300, for its legs to end). Compared, bit for bit: the read's throw; the data; the constructor's effects and wake calls; per op the throw
//! status, the result, the effects in order (the scan, the kills' damage, the passengers killed, the disable, the AI's idle, the deselection,
//! the decal, the team, the destruction), the wake calls, the xfer status, bytes and CRC, the scaffold's state and object 1's position.
//!
//! # Results (2026-10-24, `L9R35_CASES=9000`)
//!
//! 9,000 cases (7,019 compared; 1,834 data blocks threw, as the C++ did; 147 excluded, undefined upstream: a scaffold updated in a motion
//! outside the enum), 532,950 ops (234,781 updates, 48,579 deaths, 65,821 xfers) and 1,079,938 outputs: **0 mismatches**; 252,796 effect
//! lines compared (the compared cases'): 41,550 kills, 32,923 containers' passengers killed, 32,027 vehicles left unmanned; 151,220 scaffold
//! moves, 32,455 arrivals at rest and 1,813 scaffolds destroyed.
//!
//! On the way: the first C++ run crashed: the scene gave some objects no body, and `Object::kill` reads the body unchecked (upstream would
//! crash the same way on a bodiless infantry); every object now has one.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2051 | an airborne target hit when the air is not | 2,714 |
//! | 2052 | `AffectAllies = No` sparing enemies, not allies | 4,210 |
//! | 2053 | a drone left unmanned | 6,106 |
//! | 2054 | a combat bike left unmanned, not killed | 6,258 |
//! | 2055 | the terrain decal not cleared | 11,237 |
//! | 2056 | a minimum speed of 10%, not 8% | 9,240 |
//! | 2057 | the slowing over half the leg, not a quarter | 115,071 |
//! | 2058 | the overshoot test `< 0`, not `<= 0` | 6,404 |
//! | 2059 | reversing a still scaffold builds across | 85,135 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::update_module::UpdateModuleData;
use rts_lockstep_engine::logic::modules::neutron_blast_behavior::NeutronBlastBehaviorModuleData;
use rts_lockstep_engine::logic::world::body_cuts::BodyCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r35.bin");
const XFER_FILE: &str = "l9_r35.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r35Kinds`).
const KINDS: [KindOfType; 5] = [KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_DRONE, KINDOF_CLIFF_JUMPER, KINDOF_AIRCRAFT];
/// The neutral player's default team.
const NEUTRAL_TEAM: u32 = 9;

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
fn read_neutron(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::NeutronBlastBehavior(Arc::new(NeutronBlastBehaviorModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}
fn read_scaffold(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::Update(Arc::new(UpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(kind: u32, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let neutron = [BlockParse { token: "D", parse: read_neutron }];
    let scaffold = [BlockParse { token: "D", parse: read_scaffold }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, if kind == 0 { &neutron } else { &scaffold }, &mut out);
    (r, out)
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
    let head: Vec<u32> = (0..if kind == 0 { 3 } else { 0 }).map(|_| u32_(&mut p)).collect();
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} constructor {log:?} wakes {w} last {l}"));
    let n_state = if kind == 0 { 3 } else { 15 + 3 };
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
        let state: Vec<u32> = (0..n_state).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {state:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, kind: u32, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if kind == 0 {
        if g.below(100) < 80 {
            lines.push(format!("BlastRadius = {}", pick(g, &["10", "50", "150", "0"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("AffectAirborne = {}", pick(g, &["Yes", "No"])));
        }
        if g.below(100) < 50 {
            lines.push(format!("AffectAllies = {}", pick(g, &["Yes", "No"])));
        }
        if !clean {
            lines.push(pick(g, &["NoSuchField = 1", "BlastRadius = x", "AffectAllies = maybe", "BlastRadius"]).to_string());
        }
    } else if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "Enabled = Yes"]).to_string());
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
    kills: u64,
    killalls: u64,
    unmanned: u64,
    arrivals: u64,
    destroys: u64,
    moves: u64,
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

fn rel(v: u32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn template_of(kinds: u32) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: b"R35Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The slots of the probes on objects 2-5 (and 1): the contain module and the AI.
#[derive(Clone, Copy)]
struct Slots {
    contain: [Option<u16>; 5],
    ai: [Option<u16>; 5],
}

/// The scene of the configuration (the driver's `scene`, kind 0).
fn set_scene(w: &mut World, cfg: &[u32; 24], s: &Slots) {
    for k in 0..5usize {
        let id = k as ObjectId + 1;
        let f = cfg[2 + k];
        {
            let o = w.objects.find_mut(id).unwrap();
            o.thing.friend_set_template(template_of(f & 31));
            o.status = Default::default();
            o.status.set(OBJECT_STATUS_AIRBORNE_TARGET, (f >> 5 & 1) as i32);
            o.contain = if f >> 6 & 1 != 0 { s.contain[k] } else { None };
            o.ai = if f >> 7 & 1 != 0 { s.ai[k] } else { None };
            o.drawable = if f >> 8 & 1 != 0 { 7 + k as i32 } else { INVALID_DRAWABLE_ID };
            o.team = f >> 9 & 3;
        }
        w.set_position(id, &Coord3D { x: f32::from_bits(cfg[9 + k]), y: 0.0, z: 0.0 });
    }
    for a in 0..3u32 {
        for b in 0..3u32 {
            w.cuts.team_relationships.insert((a + 1, b + 1), rel(cfg[7] >> (2 * (3 * a + b)) & 3));
        }
    }
    let mut scan = Vec::new();
    for k in 0..5 {
        let id = cfg[8] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 5 {
            scan.push(id as ObjectId);
        }
    }
    w.cuts.range_scan = scan;
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, events_before: usize, tally: &mut Tally) -> Vec<String> {
    let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
    let mut log = Vec::new();
    let b = |x: f32| x.to_bits();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::PartitionIterateAliveSameMapInRangeAt { position: p, range, dc, .. }) => {
                    // the two filters: PartitionFilterAlive, PartitionFilterSameMapStatus
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 2 {dc}", b(p.x), b(p.y), b(p.z), b(range)))
                }
                Some(CutCall::Body(BodyCall::SetDisabled { object, disabled_type })) => {
                    tally.unmanned += 1;
                    log.push(format!("setdisabled {object} {disabled_type}"))
                }
                Some(CutCall::Body(BodyCall::SetTeam { team, .. })) => log.push(format!("setteam {team}")),
                Some(CutCall::Body(BodyCall::DestroyObject { object })) => {
                    tally.destroys += 1;
                    log.push(format!("destroy {object}"))
                }
                Some(CutCall::GameLogicDeselectObject { object, player_mask, affect_client }) => {
                    log.push(format!("deselect {object} {player_mask} {}", i32::from(affect_client)))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            if let Some(ObjectClientEvent::DrawableSetTerrainDecal { decal, .. }) = ev_iter.next() {
                log.push(format!("decal {decal}"));
            }
        } else if t.starts_with("damage ") {
            tally.kills += 1;
            log.push(t.clone());
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("killall ") {
                tally.killalls += 1;
                log.push(l.to_string());
            } else if l.starts_with("aiidle ") {
                log.push(l.to_string());
            }
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, kind: u32, data: ModuleDataKind, cfg_in: &[u32; 24], ops: &[[u32; 4]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    if let ModuleDataKind::NeutronBlastBehavior(d) = &data {
        put_u32(&mut v, d.blast_radius.to_bits());
        put_u32(&mut v, u32::from(d.is_affect_airborne));
        put_u32(&mut v, u32::from(d.affect_allies));
    }
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.neutral_default_team = NEUTRAL_TEAM;
    w.set_frame(cfg[0]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let name: &[u8] = if kind == 0 { b"NeutronBlastBehavior" } else { b"BridgeScaffoldBehavior" };
    let probe = |name: &[u8], answers: ProbeAnswers, interfaces: i32| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let mut slots = Slots { contain: [None; 5], ai: [None; 5] };
    for k in 0..5usize {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), name, b"ModuleTag_Me", &data).unwrap().unwrap());
        }
        if kind == 0 {
            modules.push(probe(b"R35Body", ProbeAnswers::default(), MODULEINTERFACE_BODY));
            modules.push(probe(b"R35Contain", ProbeAnswers { trace_kill_contained: true, ..Default::default() }, MODULEINTERFACE_CONTAIN));
            let mut ai = probe(b"R35Ai", ProbeAnswers::default(), 0);
            ai.is_ai = true;
            modules.push(ai);
        }
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        slots.contain[k] = w.find_module(id, b"R35Contain");
        slots.ai[k] = w.find_module(id, b"R35Ai");
    }
    let place_me = |w: &mut World, cfg: &[u32; 24]| {
        w.set_position(1, &Coord3D { x: f32::from_bits(cfg[11]), y: f32::from_bits(cfg[12]), z: f32::from_bits(cfg[13]) });
    };
    if kind == 0 {
        set_scene(&mut w, &cfg, &slots);
    } else {
        place_me(&mut w, &cfg);
    }
    let idx = w.find_module(1, name).unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: idx }).expect("an update module");
    let mine = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        (i.parse::<i32>().unwrap() == i32::from(idx)).then(|| f.parse().unwrap())
    };
    let wake_count = |trace: &[String]| {
        let (mut n, mut last) = (0u32, 0u32);
        for t in trace {
            if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                n += 1;
                last = f;
            }
        }
        (n, last)
    };
    {
        let trace = w.cuts.trace.clone().unwrap();
        let log = effects(&mut w, 0, tally);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (cw, cl) = wake_count(&trace);
        put_u32(&mut v, cw);
        put_u32(&mut v, cl);
    }
    let mut saved: Vec<u8> = Vec::new();
    let (cfg_op, frame_op, xfer_op) = if kind == 0 { (3, 2, 4) } else { (11, 1, 8) };
    for o in ops {
        if o[0] == cfg_op && (2..=22).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == frame_op {
            w.set_frame(o[1]);
        }
        if kind == 0 {
            set_scene(&mut w, &cfg, &slots);
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let pos_before = w.objects.find(1).unwrap().get_position();
        let call = |w: &mut World, c: ModuleCall| w.module_call(1, idx, c);
        if (xfer_op..=xfer_op + 2).contains(&o[0]) {
            let how = o[0] - xfer_op;
            let r = run_xfer(&mut w, 1, idx as usize, how, &saved);
            xst = r.0;
            xbytes = r.1;
            crc = r.2;
            if how == 0 {
                saved = xbytes.clone();
            }
            if how == 1 {
                // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process, as after a real load
                w.load_post_process();
            }
            tally.xfers += 1;
        } else if o[0] == 0 {
            res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
            tally.updates += 1;
        } else if kind == 0 && o[0] == 1 {
            call(&mut w, ModuleCall::OnDie(DamageInfo::default()));
            tally.deaths += 1;
        } else if kind == 1 {
            let p = |k: usize| Coord3D { x: f32::from_bits(cfg[2 + 3 * k]), y: f32::from_bits(cfg[3 + 3 * k]), z: f32::from_bits(cfg[4 + 3 * k]) };
            match o[0] {
                2 => {
                    call(&mut w, ModuleCall::ScaffoldSetPositions { create_pos: p(0), rise_to_pos: p(1), build_pos: p(2) });
                }
                3 => {
                    call(&mut w, ModuleCall::ScaffoldSetMotion(o[1] as i32));
                }
                4 => {
                    let ModuleReply::Int(m) = call(&mut w, ModuleCall::ScaffoldGetCurrentMotion) else { panic!("a motion") };
                    res = m as u32;
                }
                5 => {
                    call(&mut w, ModuleCall::ScaffoldReverseMotion);
                }
                6 => {
                    call(&mut w, ModuleCall::ScaffoldSetLateralSpeed(f32::from_bits(o[1])));
                }
                7 => {
                    call(&mut w, ModuleCall::ScaffoldSetVerticalSpeed(f32::from_bits(o[1])));
                }
                12 => res = u32::from(w.bridge_scaffold_interface((o[1] % 5) as ObjectId + 1).is_some()),
                13 => place_me(&mut w, &cfg),
                _ => {}
            }
        }
        if o[0] == 0 && w.objects.find(1).unwrap().get_position() != pos_before {
            tally.moves += 1;
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
        let trace = w.cuts.trace.clone().unwrap();
        let log = effects(&mut w, events_before, tally);
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let (wakes, last) = wake_count(&trace);
        put_u32(&mut v, wakes);
        put_u32(&mut v, last);
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for x in &xbytes {
            put_u32(&mut v, u32::from(*x));
        }
        put_u32(&mut v, crc);
        let ob = w.objects.find(1).unwrap();
        if let Some(UpdateBody::BridgeScaffoldBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() {
            let words = m.state_words();
            if o[0] == 0 && words[0] == 0 && words[12..15] == [pos_before.x.to_bits(), pos_before.y.to_bits(), pos_before.z.to_bits()] {
                // (an update that found itself still on its target)
            } else if o[0] == 0 && words[0] == 0 {
                tally.arrivals += 1;
            }
            for s in words {
                put_u32(&mut v, s);
            }
        }
        let p = ob.get_position();
        for x in [p.x, p.y, p.z] {
            put_u32(&mut v, x.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R35_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn neutron_blast_and_bridge_scaffold_behavior_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0035_B1A5);
    let mut read = Check::new("R35 data read (throw, message)");
    let mut run = Check::new("R35 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let real = |g: &mut Gen, xs: &[f32]| xs[g.below(xs.len() as u32) as usize].to_bits();
    let gen_object = |g: &mut Gen| -> u32 {
        let odds = [3, 2, 6, 5, 4];
        let kinds = (0..5).fold(0, |acc, b| acc | u32::from(g.below(odds[b]) == 0) << b);
        kinds | u32::from(g.below(4) == 0) << 5 | u32::from(g.below(3) == 0) << 6 | u32::from(g.below(2) == 0) << 7 | u32::from(g.below(2) == 0) << 8 | g.below(4) << 9
    };
    let gen_rel = |g: &mut Gen| (0..9).fold(0, |acc, k| acc | g.below(3) << (2 * k));
    let gen_scan = |g: &mut Gen| (0..5).fold(0, |acc, k| acc | (1 + g.below(5)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(5)))) - 1);
    let coords = [0.0f32, 10.0, -5.0, 30.0, 2.5, -40.0];
    let speeds = [1.0f32, 0.5, 3.0, 0.0001, 7.5];
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let kind = u32::from(g.below(3) == 0);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\R35{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; 24];
        cfg[0] = g.below(60);
        if kind == 0 {
            for k in 0..5 {
                cfg[2 + k] = gen_object(&mut g);
            }
            cfg[7] = gen_rel(&mut g);
            cfg[8] = gen_scan(&mut g);
            for c in &mut cfg[9..14] {
                *c = real(&mut g, &coords);
            }
        } else {
            for c in &mut cfg[2..14] {
                *c = real(&mut g, &coords);
            }
        }
        let n_ops = if kind == 1 { 1 + g.below(300) as usize } else { 1 + g.below(80) as usize };
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| {
                if kind == 0 {
                    match g.below(24) {
                        0..=5 => [0, 0, 0, 0],
                        6..=11 => [1, 0, 0, 0],
                        12..=13 => {
                            frame += 1 + g.below(20);
                            [2, frame, 0, 0]
                        }
                        14..=18 => {
                            let a = 2 + g.below(12);
                            let b = match a {
                                2..=6 => gen_object(&mut g),
                                7 => gen_rel(&mut g),
                                8 => gen_scan(&mut g),
                                _ => real(&mut g, &coords),
                            };
                            [3, a, b, 0]
                        }
                        19..=20 => [4, 0, 0, 0],
                        21..=22 => [5, 0, 0, 0],
                        _ => [6, 0, 0, 0],
                    }
                } else {
                    match g.below(40) {
                        0..=21 => [0, 0, 0, 0],
                        22 => {
                            frame += 1 + g.below(20);
                            [1, frame, 0, 0]
                        }
                        23..=24 => [2, 0, 0, 0],
                        25..=27 => [3, if g.below(150) == 0 { 5 + g.below(3) } else { g.below(5) }, 0, 0],
                        28 => [4, 0, 0, 0],
                        29..=30 => [5, 0, 0, 0],
                        31 => [6, real(&mut g, &speeds), 0, 0],
                        32 => [7, real(&mut g, &speeds), 0, 0],
                        33 => [8, 0, 0, 0],
                        34 => [9, 0, 0, 0],
                        35 => [10, 0, 0, 0],
                        36..=37 => [11, 2 + g.below(12), real(&mut g, &coords), 0],
                        38 => [12, g.below(5), 0, 0],
                        _ => [13, 0, 0, 0],
                    }
                }
            })
            .collect();
        let rust = if outcome.status == 0 {
            let tally_before = tally.clone();
            let (rd, w) = rust_run(&ctx, kind, data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r35_run(
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
        let what = || format!("case {case}: kind {kind} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} kind {kind} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R35: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} deaths, {} xfers), \
         {} effect lines ({} kills, {} passengers killed, {} vehicles unmanned; {} scaffold moves, {} arrivals at rest, {} destroyed; the compared cases')",
        tally.updates, tally.deaths, tally.xfers, tally.effects, tally.kills, tally.killalls, tally.unmanned, tally.moves, tally.arrivals, tally.destroys
    );
    println!("R35 excluded: {reasons:?}");
    read.finish();
    run.finish();
    let _ = INVALID_ID;
    assert!(
        tally.kills > cases as u64 && tally.killalls > cases as u64 / 2 && tally.unmanned > cases as u64 / 2 && tally.destroys * 20 > cases as u64 && tally.arrivals * 10 > cases as u64,
        "the generator must reach every effect"
    );
}
