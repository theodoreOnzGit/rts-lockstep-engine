#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PropagandaTowerBehavior` (`logic::modules::propaganda_tower_behavior`) against the unchanged C++, `p1w13-sonnet`, batch R37.
//!
//! # Method
//!
//! The C++ side is `PropagandaTowerBehavior.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit (`differential/l9_propaganda_r37.inc`,
//! `ref_l9r37_run`), on the module build's stand-ins: five objects (ids 1-5; kinds, status bits, effectively dead, the disabled bits, whether
//! the weapon set has a damage weapon, maybe a body with health 100 of 200 whose `attemptHealing` is a line, maybe a container, the weapon
//! bonus bits); the tower (object 1) and its player of three (the neutral player the first); the local player; the players' and the tower's
//! completed upgrades; the upgrade center with `Upgrade_A` (a player upgrade) and `Upgrade_B` (an object upgrade), loaded on both sides;
//! the scan (the objects the test lists, taken as those that pass the filters); the FX lists; the REAL
//! `Object::attemptHealingFromSoleBenefactor`, `UpdateModule`, `INI` and `Xfer`; the game logic, whose wake calls are applied as
//! `GameLogic::friend_awakenUpdateModule` applies them.
//!
//! The Rust side is the real thing: a `World` with the same objects (the module from `modules::new_module`, `ProbeModule` bodies), three teams
//! and players, the upgrade center, `cuts.player_upgrades`, `cuts.range_scan`, the weapon sets' damage-weapon flag
//! (`WeaponSet::friend_set_has_damage_weapon`); every op an `update()` as the scheduler runs it, a death, a deletion, a capture, a change to
//! the scene or the frame, or an xfer.
//!
//! Each case reads a block `D` (mostly well formed) and runs 1 to 120 ops. Compared, bit for bit: the read's throw; the data; the
//! constructor's and `onObjectCreated`'s effects and wake calls; per op the throw status, the result, the effects in order (the scan, the
//! pulse FX, the heals), the wake calls, the xfer status, bytes and CRC, the module's state (the last scan frame, whether the upgrade was
//! found, the tracked list from its head), and every object's weapon bonus bits and health.
//!
//! # Results (2026-10-24, `L9R37_CASES=9000`)
//!
//! 9,000 cases (5,612 compared; 1,777 data blocks threw, as the C++ did; 1,611 excluded, undefined upstream: 945 a tower whose
//! `UpgradeRequired` names no upgrade, 666 one with no controlling player, both dereferenced by `effectLogic`), 328,209 ops (164,436 updates,
//! 41,192 xfers) and 667,642 outputs: **0 mismatches**; 169,576 effect lines compared (the compared cases'): 34,273 scans, 14,687 pulses and
//! 120,616 heals; 150,157 object-ops ended enthusiastic and 80,166 subliminal.
//!
//! On the way: the only first-run difference was the load into a non-empty list, which throws a `SaveCode` (`SC_INVALID_DATA`), not an
//! `XferStatus`: the C++ `runXfer` answers -1 for it, as for the rebuild hole's, and the test maps `ListNotEmpty` to it.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 9 of 9 caught. (Each changes a value, not which calls are made: a plant that let
//! the Rust side skip a removal the C++ makes could take it off an undefined path the C++ then runs into.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2071 | the upgraded heal percent when not upgraded, and the reverse | 232,987 |
//! | 2072 | the tracked list in scan order, not reversed | 84,200 |
//! | 2073 | the tower affects itself without `AffectsSelf` | 81,186 |
//! | 2074 | the scan radius doubled | 27,338 |
//! | 2075 | the heal per second divided by 15, not 30 | 216,424 |
//! | 2076 | subliminal without the upgrade | 48,019 |
//! | 2077 | a tower captured by the neutral player kept awake | 4,014 |
//! | 2078 | the pulse shown while the tower is stealthed and undetected | 2,434 |
//! | 2079 | the upgraded pulse FX when not upgraded, and the reverse | 19,350 |

mod support;
mod up_common;

use rts_lockstep_engine::common::disabled_types::{DISABLED_EMP, DISABLED_HELD};
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::propaganda_tower_behavior::PropagandaTowerBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r37.bin");
const XFER_FILE: &str = "l9_r37.bin";

/// The kinds the test gives an object, by bit (the driver's `g_r37Kinds`).
const KINDS: [KindOfType; 6] = [KINDOF_SCORE, KINDOF_SCORE_CREATE, KINDOF_SCORE_DESTROY, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_VEHICLE, KINDOF_PORTABLE_STRUCTURE];
/// The status bits, by bit (the driver's `g_r37Status`).
const STATUS: [ObjectStatusTypes; 4] = [OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED];
const UPGRADES: [&str; 2] = ["Upgrade_A", "Upgrade_B"];

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
    *out = Some(ModuleDataKind::PropagandaTowerBehavior(Arc::new(PropagandaTowerBehaviorModuleData::friend_new_module_data(Some(ini))?)));
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
    let head: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
    let upgrade = str_(&mut p);
    let affects_self = u32_(&mut p);
    let n = u32_(&mut p);
    let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("data {head:x?} {upgrade:?} {affects_self} constructor {log:?} wakes {w} last {l}"));
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
        let (scan, found, count) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
        let list: Vec<u32> = (0..count).map(|_| u32_(&mut p)).collect();
        let objects: Vec<u32> = (0..10).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state {scan} {found} {list:?} {objects:x?}"));
    }
    out
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 80 {
        lines.push(format!("Radius = {}", pick(g, &["50", "150", "0", "1"])));
    }
    if g.below(100) < 80 {
        lines.push(format!("DelayBetweenUpdates = {}", pick(g, &["0", "33", "100", "1000", "3000"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("HealPercentEachSecond = {}", pick(g, &["1%", "5%", "0%", "50%"])));
    }
    if g.below(100) < 60 {
        lines.push(format!("UpgradedHealPercentEachSecond = {}", pick(g, &["2%", "10%", "100%"])));
    }
    if g.below(100) < 85 {
        lines.push(format!("PulseFX = {}", pick(g, &["FX_A", "FX_B", "FX_A", "None"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("UpgradedPulseFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 96 {
        lines.push(format!("UpgradeRequired = {}", pick(g, &["Upgrade_A", "Upgrade_A", "Upgrade_B", "Upgrade_B", "Upgrade_A", "Upgrade_None"])));
    }
    if g.below(100) < 50 {
        lines.push(format!("AffectsSelf = {}", pick(g, &["Yes", "No"])));
    }
    if !clean {
        lines.push(pick(g, &["NoSuchField = 1", "Radius = x", "HealPercentEachSecond = q%", "AffectsSelf"]).to_string());
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
    xfers: u64,
    effects: u64,
    scans: u64,
    pulses: u64,
    heals: u64,
    enthusiastic: u64,
    subliminal: u64,
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
    let mut values = ObjectTemplateValues { name: b"R37Thing".to_vec(), ..Default::default() };
    for (b, k) in KINDS.iter().enumerate() {
        if kinds >> b & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    Arc::new(ThingTemplate::from_object_values(&values))
}

/// The upgrade masks of `Upgrade_A` (bit 0) and `Upgrade_B` (bit 1) in `bits`.
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

/// The scene of the configuration (the driver's `scene`); `bodies` the body slots.
fn set_scene(w: &mut World, cfg: &[u32; 24], bodies: &[u16; 5]) {
    let tp = cfg[1];
    w.objects.find_mut(1).unwrap().team = if (1..=3).contains(&tp) { tp } else { 0 };
    w.players.friend_set_local_index((cfg[2] < 3).then_some(cfg[2] as i32));
    for p in 0..3 {
        let m = upgrade_mask(w, cfg[3] >> (2 * p) & 3);
        w.cuts.player_upgrades.insert(p, m);
    }
    let m = upgrade_mask(w, cfg[4] & 3);
    w.objects.find_mut(1).unwrap().object_upgrades_completed = m;
    for k in 0..5usize {
        let f = cfg[5 + k];
        let o = w.objects.find_mut(k as ObjectId + 1).unwrap();
        o.thing.friend_set_template(template_of(f & 63));
        o.status = Default::default();
        for (b, s) in STATUS.iter().enumerate() {
            if f >> (6 + b) & 1 != 0 {
                o.status.set(*s, 1);
            }
        }
        if f >> 10 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.disabled = Default::default();
        o.disabled.set(DISABLED_HELD, (f >> 11 & 1) as i32);
        o.disabled.set(DISABLED_EMP, (f >> 12 & 1) as i32);
        o.weapon_set.friend_set_has_damage_weapon(f >> 13 & 1 != 0);
        o.body = (f >> 14 & 1 != 0).then_some(bodies[k]);
        let c = f >> 15 & 7;
        o.contained_by = if (1..=5).contains(&c) { c as ObjectId } else { INVALID_ID };
    }
    let mut scan = Vec::new();
    for k in 0..5 {
        let id = cfg[10] >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 5 {
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
    let ModuleDataKind::PropagandaTowerBehavior(d) = &data else { unreachable!() };
    put_u32(&mut v, d.scan_radius.to_bits());
    put_u32(&mut v, d.scan_delay_in_frames);
    put_u32(&mut v, d.auto_heal_percent_per_second.to_bits());
    put_u32(&mut v, d.upgraded_auto_heal_percent_per_second.to_bits());
    put_i32(&mut v, fx.id(&d.pulse_fx));
    put_i32(&mut v, fx.id(&d.upgraded_pulse_fx));
    put_str(&mut v, &d.upgrade_required);
    put_u32(&mut v, u32::from(d.affects_self));
    let mut w = World::new();
    w.names = ctx.names.clone();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    let body = || {
        let mut b = NewModule::plain(b"R37Body", b"ModuleTag_Body");
        b.interfaces = MODULEINTERFACE_BODY;
        b.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 200.0, Default::default())));
        b
    };
    // the scene's tower before the constructor (its player for onObjectCreated's lookup is not read)
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"PropagandaTowerBehavior", b"ModuleTag_Tower", &data).unwrap().unwrap();
    let mut bodies = [0u16; 5];
    for (k, slot) in bodies.iter_mut().enumerate() {
        let mut modules = Vec::new();
        if k == 0 {
            modules.push(m.clone());
        }
        modules.push(body());
        let id = w.new_object(NewObject { template: template_of(0), status: Default::default(), team: None, modules }).unwrap();
        assert_eq!(id, k as ObjectId + 1);
        *slot = w.find_module(id, b"R37Body").unwrap();
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[*slot as usize].body.as_mut() {
            p.health = 100.0;
        }
    }
    let idx = w.find_module(1, b"PropagandaTowerBehavior").unwrap();
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
                if let Some(CutCall::PartitionIterateAlliesAliveSameMapNotStructuresAt { position: p, range, .. }) = cut_iter.next() {
                    tally.scans += 1;
                    // the four filters, FROM_CENTER_2D
                    log.push(format!("scan {:08x} {:08x} {:08x} {:08x} 4 0", b(p.x), b(p.y), b(p.z), b(range)));
                }
            } else if t.starts_with("evt ") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) = ev_iter.next() {
                    tally.pulses += 1;
                    log.push(format!("fx {}", fx.id(&Some(name))));
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(mine) {
                wakes += 1;
                last = f;
            } else if t.starts_with("heal ") {
                tally.heals += 1;
                log.push(t.clone());
            }
        }
        (log, wakes, last)
    };
    set_scene(&mut w, &cfg, &bodies);
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
        if o[0] == 2 && (1..=10).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &bodies);
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
                w.module_call(1, idx, ModuleCall::OnDie(DamageInfo::default()));
            }
            4 => {
                w.module_call(1, idx, ModuleCall::OnDelete);
            }
            5 => {
                let old_owner = w.get_controlling_player(1).unwrap_or(-1);
                let new_owner = if o[1] < 3 { o[1] as i32 } else { -1 };
                w.module_call(1, idx, ModuleCall::OnCapture { old_owner, new_owner });
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
        let Some(UpdateBody::PropagandaTowerBehavior(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        let words = m.state_words();
        for s in &words {
            put_u32(&mut v, *s);
        }
        for (k, slot) in bodies.iter().enumerate() {
            let o = w.objects.find(k as ObjectId + 1).unwrap();
            put_u32(&mut v, o.weapon_bonus_condition);
            tally.enthusiastic += u64::from(o.weapon_bonus_condition >> 8 & 1);
            tally.subliminal += u64::from(o.weapon_bonus_condition >> 15 & 1);
            let health = match o.modules[*slot as usize].body.as_ref() {
                Some(UpdateBody::ModuleProbe(p)) => p.health,
                _ => 0.0,
            };
            put_u32(&mut v, health.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R37_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn propaganda_tower_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0037_9A9A);
    let mut read = Check::new("R37 data read (throw, message)");
    let mut run = Check::new("R37 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut reasons = std::collections::BTreeMap::new();
    let mut shown = 0;
    let gen_object = |g: &mut Gen, k: usize| -> u32 {
        let odds = [2, 4, 5, 5, 4, 6];
        let kinds = (0..6).fold(0, |acc, b| acc | u32::from(g.below(odds[b]) == 0) << b);
        let status = (0..4).fold(0, |acc, b| acc | u32::from(g.below(if b < 2 { 15 } else { 4 }) == 0) << (6 + b));
        let dead = u32::from(g.below(15) == 0) << 10;
        let disabled = u32::from(g.below(10) == 0) << 11 | u32::from(g.below(15) == 0) << 12;
        let weapon = u32::from(g.below(3) != 0) << 13;
        let body = u32::from(g.below(6) != 0) << 14;
        let container = if g.below(6) == 0 { (1 + g.below(5)) << 15 } else { 0 };
        let _ = k;
        kinds | status | dead | disabled | weapon | body | container
    };
    let gen_scan = |g: &mut Gen| (0..5).fold(0, |acc, k| acc | (1 + g.below(5)) << (4 * k)) & ((1u32 << (4 * (1 + g.below(5)))) - 1);
    for case in 0..cases {
        let mut ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let defs = "Upgrade Upgrade_A\n Type = PLAYER\nEnd\nUpgrade Upgrade_B\n Type = OBJECT\nEnd\n";
        let table = blocks();
        Ini::load(b"Data\\INI\\Upgrade.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        assert_eq!(cpp_load(b"Data\\INI\\Upgrade.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R37{}.ini", case % 5).into_bytes();
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
        cfg[0] = g.below(60);
        cfg[1] = if g.below(12) == 0 { 0 } else { 1 + g.below(3) };
        cfg[2] = g.below(4);
        cfg[3] = g.below(64);
        cfg[4] = g.below(4);
        for k in 0..5 {
            cfg[5 + k] = gen_object(&mut g, k);
        }
        cfg[10] = gen_scan(&mut g);
        let n_ops = 1 + g.below(120) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 4]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=19 => [0, 0, 0, 0],
                20..=25 => {
                    frame += 1 + g.below(60);
                    [1, frame, 0, 0]
                }
                26..=31 => {
                    let a = 1 + g.below(10);
                    let b = match a {
                        1 => if g.below(10) == 0 { 0 } else { 1 + g.below(3) },
                        2 => g.below(4),
                        3 => g.below(64),
                        4 => g.below(4),
                        10 => gen_scan(&mut g),
                        _ => gen_object(&mut g, a as usize - 5),
                    };
                    [2, a, b, 0]
                }
                32 => [3, 0, 0, 0],
                33 => [4, 0, 0, 0],
                34 => [5, g.below(4), 0, 0],
                35..=36 => [6, 0, 0, 0],
                37..=38 => [7, 0, 0, 0],
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
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9r37_run(cfile.as_ptr(), cxfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
        "R37: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops ({} updates, {} xfers), \
         {} effect lines ({} scans, {} pulses, {} heals; {} enthusiastic and {} subliminal object-ops; the compared cases')",
        tally.updates, tally.xfers, tally.effects, tally.scans, tally.pulses, tally.heals, tally.enthusiastic, tally.subliminal
    );
    println!("R37 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.scans > cases as u64 && tally.pulses > cases as u64 && tally.heals > cases as u64 && tally.subliminal > cases as u64 / 4, "the generator must reach every effect");
}
