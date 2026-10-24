#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `OverchargeBehavior` (`logic::modules::overcharge_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R24.
//!
//! # Method
//!
//! The C++ side is `OverchargeBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_overcharge_r24.inc`,
//! `ref_l9r24_run`) on the module build's stand-ins: the module's object (id 1) with a body whose `attemptDamage` logs and takes the health, up to
//! two power plant updates whose `extendRods` logs, a controlling player of three (or none) whose power bonus calls log, the local player; the
//! in-game UI's `message` and the radar's `createEvent` as lines; the game logic, whose wake calls are counted; and the REAL `UpdateModule`, `INI`
//! and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the same object (the module from `modules::new_module`, `ProbeModule` body and power plants),
//! three players, the team's player as the controlling player; every op an `update()` as the scheduler runs it, a module call or a change to the
//! scene, a load followed by the game logic's post-process.
//!
//! Each case reads a block `D` (`HealthPercentToDrainPerSecond`, `NotAllowedWhenHealthBelowPercent`), mostly well formed, and runs 1 to 30 ops.
//! Compared, bit for bit: the read's throw; the data; the constructor's wake calls; per op the throw status, the result, the effects in order
//! (`extendrods`, `addpower`, `removepower`, `damage`, `message`, `radarevent`), the wake calls, the xfer status, bytes and CRC, the flag and the
//! body's health.
//!
//! # Results (2026-10-23, `L9R24_CASES=9000`)
//!
//! 9,000 cases (7,181 compared, none excluded; 1,819 data files threw, as the C++ did), 111,804 ops (41,940 updates, 13,938 xfers) and 126,166
//! outputs: **0 mismatches**; 40,791 effect lines compared, among them 10,854 drains, 830 exhaustions (the message and the radar event) and
//! 8,974 power bonuses added.
//!
//! On the way: the world puts its standard helpers before the module, so the power plants' slots are renumbered to the driver's (2 and 3); the
//! exhaustion message is the local player's, so half the cases make the local player the owner (91 in a first 2,000-case run).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 6 of 6 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1941 | `enable` acts when the state does not change | 2,427 |
//! | 1942 | a disabled object's capture moves the bonus | 134 |
//! | 1943 | no drain | 29,449 |
//! | 1944 | exhaustion turns the overcharge on, not off | 11,228 |
//! | 1945 | no exhaustion message | 830 |
//! | 1946 | the bonus not re-added after a load | 980 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::overcharge_behavior::OverchargeBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r24.bin");
const XFER_FILE: &str = "l9_r24.bin";

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
    *out = Some(ModuleDataKind::OverchargeBehavior(Arc::new(
        OverchargeBehaviorModuleData::friend_new_module_data(Some(ini))?,
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
    let (a, c) = (u32_(&mut p), u32_(&mut p));
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {a:#x} {c:#x} constructor wakes {w} last {l}"));
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
        let (active, health) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} active {active} health {health:#x}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 85 {
        lines.push(format!(
            "HealthPercentToDrainPerSecond = {}",
            pick(g, &["0%", "5%", "10%", "25%", "100%"])
        ));
    }
    if g.below(100) < 85 {
        lines.push(format!(
            "NotAllowedWhenHealthBelowPercent = {}",
            pick(g, &["0%", "10%", "50%", "75%", "90%"])
        ));
    }
    if !clean {
        lines.push(match g.below(2) {
            0 => "HealthPercentToDrainPerSecond = x".to_string(),
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
    drains: u64,
    exhausted: u64,
    bonuses: u64,
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

fn probe(name: &[u8], answers: ProbeAnswers, max: f32) -> NewModule {
    let mut m = NewModule::plain(name, name);
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        answers,
        max,
        Default::default(),
    )));
    m
}

fn body_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeModule {
    match w.objects.find_mut(id).unwrap().modules[slot as usize]
        .body
        .as_mut()
    {
        Some(UpdateBody::ModuleProbe(p)) => p,
        _ => unreachable!("a probe"),
    }
}

fn set_player(w: &mut World, p: u32) {
    if p == 0 {
        w.cuts.team_players.remove(&1);
    } else {
        w.cuts.team_players.insert(1, ((p - 1) % 3) as i32);
    }
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    ctx: &Ctx,
    data: ModuleDataKind,
    cfg: &[u32; 5],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let ModuleDataKind::OverchargeBehavior(d) = &data else {
        unreachable!()
    };
    put_u32(&mut v, d.health_percent_to_drain_per_second.to_bits());
    put_u32(&mut v, d.not_allowed_when_health_below_percent.to_bits());
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center.clone(),
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.teams.insert_team(Team::new(1, 0)).unwrap();
    w.players.set_player_count(3);
    set_player(&mut w, cfg[0]);
    w.players
        .friend_set_local_index((cfg[1] != 0).then(|| ((cfg[1] - 1) % 3) as i32));
    let plain = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"OverchargeBehavior",
        b"ModuleTag_Overcharge",
        &data,
    )
    .unwrap()
    .unwrap();
    let mut body = probe(b"R24Body", ProbeAnswers::default(), f32::from_bits(cfg[4]));
    body.interfaces = MODULEINTERFACE_BODY;
    let mut modules = vec![m, body];
    for _ in 0..cfg[2] % 3 {
        modules.push(probe(
            b"R24Plant",
            ProbeAnswers {
                has: 1 << 20,
                ..Default::default()
            },
            0.0,
        ));
    }
    let me = w
        .new_object(NewObject {
            template: plain,
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules,
        })
        .unwrap();
    assert_eq!(me, 1);
    let idx = w.find_module(me, b"OverchargeBehavior").unwrap();
    // the world puts its standard helpers first: the power plants follow the module and the body, as slots 2 and 3 of the driver's numbering
    let plant_base = i32::from(idx) + 2;
    let body_slot = w.objects.find(me).unwrap().body.unwrap();
    body_mut(&mut w, me, body_slot).health = f32::from_bits(cfg[3]);
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
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
    w.set_position(
        me,
        &Coord3D {
            x: 10.0,
            y: -2.5,
            z: 1.0,
        },
    );
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        match o[0] {
            11 => body_mut(&mut w, me, body_slot).health = f32::from_bits(o[1]),
            12 => w
                .players
                .friend_set_local_index((o[1] != 0).then(|| ((o[1] - 1) % 3) as i32)),
            13 => set_player(&mut w, o[1]),
            14 => w.set_frame(o[1]),
            15 => {
                w.objects.find_mut(me).unwrap().disabled.set(
                    rts_lockstep_engine::common::disabled_types::DISABLED_HACKED,
                    i32::from(o[1] & 1 != 0),
                );
            }
            _ => {}
        }
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let player = |p: u32| if p == 0 { -1 } else { ((p - 1) % 3) as i32 };
        match o[0] {
            0 => {
                res = w
                    .with_body(u, |b, w| b.update(w, u))
                    .expect("the module is in its slot")
                    .0;
                tally.updates += 1;
            }
            1 => {
                w.module_call(me, idx, ModuleCall::OverchargeToggle);
            }
            2 => {
                w.module_call(me, idx, ModuleCall::OverchargeEnable(o[1] & 1 != 0));
            }
            3 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::IsOverchargeActive)
                        .as_bool(),
                )
            }
            4 => {
                w.module_call(me, idx, ModuleCall::OnDelete);
            }
            5 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::OnCapture {
                        old_owner: player(o[1]),
                        new_owner: player(o[2]),
                    },
                );
            }
            6 => {
                w.module_call(me, idx, ModuleCall::OnDamage(DamageInfo::default()));
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
                    // a load sets `m_indexInLogic` to -1 ("restored in gamelogic::postprocess"): the game logic's post-process (the sleepy list), as
                    // after a real load; the modules' own `loadPostProcess` is op 10
                    w.load_post_process();
                }
                tally.xfers += 1;
            }
            10 => {
                w.module_call(me, idx, ModuleCall::LoadPostProcess);
            }
            11..=15 => {}
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
                    CutCall::PlayerAddPowerBonus { player, .. } => {
                        tally.bonuses += 1;
                        log.push(format!("addpower {player}"));
                    }
                    CutCall::PlayerRemovePowerBonus { player, .. } => {
                        log.push(format!("removepower {player}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::UiMessage { label }) => {
                        tally.exhausted += 1;
                        log.push(format!("message {}", String::from_utf8_lossy(&label)));
                    }
                    Some(ObjectClientEvent::RadarCreateEvent {
                        position: p,
                        event_type,
                    }) => log.push(format!(
                        "radarevent {:08x} {:08x} {:08x} {event_type} {:08x}",
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        4.0f32.to_bits()
                    )),
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ") {
                let (i, f) = f.split_once(' ').unwrap();
                if i.parse::<i32>().unwrap() == i32::from(idx) {
                    wakes += 1;
                    last = f.parse().unwrap();
                }
            } else if let Some(r) = t.strip_prefix("extendrods ") {
                let (slot, extend) = r.split_once(' ').unwrap();
                log.push(format!(
                    "extendrods {} {extend}",
                    slot.parse::<i32>().unwrap() - plant_base + 2
                ));
            } else if t.starts_with("damage ") {
                tally.drains += 1;
                log.push(t);
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
        let active =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::OverchargeBehavior(m)) => Some(m.state_words()[0]),
                    _ => None,
                });
        put_u32(&mut v, active.expect("the module is back in its slot"));
        let health = body_mut(&mut w, me, body_slot).health;
        put_u32(&mut v, health.to_bits());
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R24_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn overcharge_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0024_0C4A);
    let mut read = Check::new("R24 data read (throw, message)");
    let mut run = Check::new("R24 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R24{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let owner = if g.below(8) == 0 { 0 } else { 1 + g.below(3) };
        // half the cases are the local player's own plant (the exhaustion message is the local player's)
        let local = if g.below(2) == 0 { owner } else { g.below(4) };
        let cfg = [
            owner,
            local,
            g.below(3),
            (*pick(&mut g, &[&100.0f32, &80.0, &60.0, &30.0])).to_bits(),
            100.0f32.to_bits(),
        ];
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = 0u32;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(24) {
                0..=8 => [0, 0, 0, 0, 0, 0],
                9..=10 => [1, 0, 0, 0, 0, 0],
                11 => [2, g.below(2), 0, 0, 0, 0],
                12 => [3, 0, 0, 0, 0, 0],
                13 => [4, 0, 0, 0, 0, 0],
                14 => [5, g.below(4), g.below(4), 0, 0, 0],
                15 => [6, 0, 0, 0, 0, 0],
                16 => [7, 0, 0, 0, 0, 0],
                17 => [8, 0, 0, 0, 0, 0],
                18 => [9, 0, 0, 0, 0, 0],
                19 => [10, 0, 0, 0, 0, 0],
                20 => [
                    11,
                    (*pick(&mut g, &[&100.0f32, &55.0, &20.0, &5.0])).to_bits(),
                    0,
                    0,
                    0,
                    0,
                ],
                21 => [12, g.below(4), 0, 0, 0, 0],
                22 => [13, g.below(4), 0, 0, 0, 0],
                _ => {
                    if g.below(2) == 0 {
                        frame += 1 + g.below(30);
                        [14, frame, 0, 0, 0, 0]
                    } else {
                        [15, g.below(2), 0, 0, 0, 0]
                    }
                }
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
            cpp::ref_l9r24_run(
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
        "R24: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} xfers, \
         {} effect lines, {} drains, {} exhausted, {} power bonuses added",
        tally.updates, tally.xfers, tally.effects, tally.drains, tally.exhausted, tally.bonuses
    );
    read.finish();
    run.finish();
    assert!(
        tally.drains * 2 > cases as u64
            && tally.exhausted * 20 > cases as u64
            && tally.bonuses * 2 > cases as u64,
        "the generator must reach every path"
    );
}
