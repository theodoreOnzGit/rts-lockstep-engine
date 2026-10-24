#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `StealthUpdate` (`logic::modules::stealth_update`) against the unchanged C++, `p1w13-sonnet`, batch R26.
//!
//! # Method
//!
//! The C++ side is `StealthUpdate.h` and `StealthUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit as `R26StealthUpdate`
//! (`differential/l9_stealth_r26.inc`, `ref_l9r26_run`; the unit has a stand-in `StealthUpdate` of its own), on the module build's stand-ins:
//! the module's object (id 1) with its kinds, status bits, script bit, player, drawable (selected, hidden), AI (current victim, last command
//! source), physics (a velocity), body (a last damage), contain module (rider change, its rider, its passengers, whether they attack), spawn
//! behaviour (whether the slaves stealth), weapons (their last shots) and container (object 2, garrisonable or not); a rider (object 3) with a
//! second `R26StealthUpdate` from its own block `D`; a disguise target (4); two enemies with an AI (5, 6); a black market (7); three players
//! (default teams, colours, active or not, the local one, the relationships); the radar, audio, FX, UI, control bar, game client and thing factory
//! (the destroyed drawable comes back from the pool, as the game's memory pool gives it); the partition manager's distance; the game logic, whose
//! wake calls are applied as `GameLogic::friend_awakenUpdateModule` applies them; and the REAL `UpdateModule`, `INI`, `Xfer` and random values.
//!
//! The Rust side is the real thing: a `World` with the same objects (two modules from `modules::new_module`, `ProbeModule`s for the AI, the
//! physics, the body, the contain modules, the spawn behaviour and a sub-object upgrade), the teams, players, relationships, colours and
//! player object lists in `cuts`, a thing factory that knows the templates' names (for a load), weapons whose last shot frames are set; every op an
//! `update()` as the scheduler runs it, a module call, a change to the scene, a load.
//!
//! Each case reads two blocks `D` (every field; mostly well formed) and runs 1 to 30 ops. Compared, bit for bit: the reads' throws; both data;
//! the constructors' wake calls; per op the throw status, the result, the effects in order (the drawable's opacity, second pass, look and swap,
//! the indicator colour, the selection, the hidden state, the control bar, the sounds, the FX, the model condition, the radar, the distance, the
//! physics reset, the slaves, the AI wakes, the container's recalculation, the sub-object refresh), the wake calls, the xfer status, bytes and
//! CRC, both modules' state and next call frame, both objects' status bits, the academy counts and whether the object has a drawable.
//!
//! # Results (2026-10-23, `L9R26_CASES=9000`)
//!
//! 9,000 cases (5,281 compared; 2,491 had a data block that threw, as the C++ did; 1,228 excluded, undefined upstream: 355 a `NULL` local player
//! read by the stealth look or a disguise's colour, 317 a body read unchecked by `TAKING_DAMAGE`, 302 a `NULL` controlling player (the black
//! market, the order to the idle enemies, a disguise), 131 a disguise swap with no drawable, 84 with no physics, 39 a rider without a stealth
//! update whose rules are read), 76,953 ops (42,995 updates, 18,242 xfers) and 175,030 outputs: **0 mismatches**; 77,060 effect lines compared,
//! 25,644 ops ended stealthed and 20,268 detected, 1,221 drawable swaps, 22,001 stealth looks and 660 idle enemies woken.
//!
//! On the way: (1) a translation fix: `disguiseAsObject` on the object itself reads its own disguise (the module is out of its slot while it is
//! called); (2) the driver: the unit's stand-in `StealthUpdate` means the real class is compiled renamed, with `getStealth` and `getPhysics` routed
//! to renamed accessors; the shared drawable stand-ins model the memory pool (a destroyed drawable is the next new one), without which the reveal
//! branch reads freed memory; each wake call is applied as the game logic applies it (two modules wake in one op); (3) the test: after a load the
//! game logic's post-process (the R20 lesson), the status table's `USING_ABILITY`, the helpers' wake calls left out.
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 8 of 8 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1961 | a grant forgets its frames | 6,656 |
//! | 1962 | the forbidden status ignored | 2,980 |
//! | 1963 | detection lasts one frame more | 1,607 |
//! | 1964 | `markAsDetected()` one frame longer | 20,645 |
//! | 1965 | the pulse runs at twice the rate | 61,845 |
//! | 1966 | `m_framesGranted` not saved | 5,443 |
//! | 1967 | an ally sees the stealthed object as invisible | 1,336 |
//! | 1968 | the disguise swap does not reset the radar | 802 |
//!
//! (A first plant 1965, the halfway point already reached, was caught, but it moved cases off a path the Rust side marks undefined, and the C++ run
//! of such a case crashed: plants must not change which cases are undefined upstream. It was replaced.)

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_UPGRADE};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::modules::stealth_update::StealthUpdateModuleData;
use rts_lockstep_engine::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNSTEALTHED;
use rts_lockstep_engine::logic::weapon_fire::Weapon;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent, RadarCall, StealthClientOp};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r26.bin");
const XFER_FILE: &str = "l9_r26.bin";
/// The status bits the test sets and dumps, by bit (the driver's `g_r26Status`), with their INI names.
const STATUS: [(ObjectStatusTypes, &str); 8] = [
    (OBJECT_STATUS_IS_FIRING_WEAPON, "IS_FIRING_WEAPON"),
    (OBJECT_STATUS_IS_USING_ABILITY, "USING_ABILITY"),
    (OBJECT_STATUS_CAN_STEALTH, "CAN_STEALTH"),
    (OBJECT_STATUS_STEALTHED, "STEALTHED"),
    (OBJECT_STATUS_DETECTED, "DETECTED"),
    (OBJECT_STATUS_DISGUISED, "DISGUISED"),
    (OBJECT_STATUS_UNDER_CONSTRUCTION, "UNDER_CONSTRUCTION"),
    (OBJECT_STATUS_SOLD, "SOLD"),
];
/// The objects' template names (the driver's `g_r26TmplNames`).
const NAMES: [&str; 7] = ["R26Me", "R26Box", "R26Rider", "R26Tank", "R26Enemy", "R26Enemy", "R26Market"];

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
    *out = Some(ModuleDataKind::StealthUpdate(Arc::new(StealthUpdateModuleData::friend_new_module_data(Some(ini))?)));
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

fn put_data(v: &mut Vec<u8>, d: &StealthUpdateModuleData, fx: &mut FxIds) {
    for m in [&d.hint_detectable_states, &d.required_status, &d.forbidden_status] {
        for i in 0..OBJECT_STATUS_COUNT {
            put_u32(v, u32::from(m.test(i)));
        }
    }
    put_i32(v, fx.id(&d.disguise_fx));
    put_i32(v, fx.id(&d.disguise_reveal_fx));
    for r in [d.stealth_speed, d.friendly_opacity_min, d.friendly_opacity_max, d.reveal_distance_from_target] {
        put_u32(v, r.to_bits());
    }
    for u in [d.disguise_transition_frames, d.disguise_reveal_transition_frames, d.pulse_frames, d.stealth_delay, d.stealth_level, d.black_market_check_frames] {
        put_u32(v, u);
    }
    put_i32(v, d.enemy_detection_eva_event);
    put_i32(v, d.own_detection_eva_event);
    put_u32(
        v,
        u32::from(d.innate_stealth)
            | u32::from(d.order_idle_enemies_to_attack_me_upon_reveal) << 1
            | u32::from(d.team_disguised) << 2
            | u32::from(d.use_rider_stealth) << 3
            | u32::from(d.granted_by_special_power) << 4,
    );
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
    for k in 0..2 {
        let masks: Vec<Vec<u32>> = (0..3).map(|_| (0..OBJECT_STATUS_COUNT).map(|_| u32_(&mut p)).collect()).collect();
        let set: Vec<Vec<usize>> = masks.iter().map(|m| m.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, _)| i).collect()).collect();
        let rest: Vec<u32> = (0..15).map(|_| u32_(&mut p)).collect();
        out.push(format!("data {k}: masks {set:?} {rest:x?}"));
    }
    let (w, l) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor wakes {w} last {l}"));
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
        let mut mods = Vec::new();
        for _ in 0..2 {
            let words: Vec<u32> = (0..13).map(|_| u32_(&mut p)).collect();
            let name = str_(&mut p);
            let next = u32_(&mut p);
            mods.push(format!("{words:x?} {name:?} next {next}"));
        }
        let tail: Vec<u32> = (0..6).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        out.push(format!("op {i} state: {mods:?} tail {tail:x?}"));
    }
    out
}

fn gen_status_list(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(2) {
        t.push(STATUS[g.below(8) as usize].1);
    }
    t.join(" ")
}

fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 70 {
        lines.push(format!("StealthDelay = {}", pick(g, &["0", "33", "100", "500"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("MoveThresholdSpeed = {}", pick(g, &["0", "3", "30"])));
    }
    if g.below(100) < 60 {
        let mut t = Vec::new();
        for _ in 0..1 + g.below(3) {
            t.push(pick(g, &["ATTACKING", "MOVING", "USING_ABILITY", "FIRING_PRIMARY", "FIRING_SECONDARY", "FIRING_TERTIARY", "NO_BLACK_MARKET", "TAKING_DAMAGE", "RIDERS_ATTACKING"]));
        }
        lines.push(format!("StealthForbiddenConditions = {}", t.join(" ")));
    }
    if g.below(100) < 30 {
        lines.push(format!("HintDetectableConditions = {}", gen_status_list(g)));
    }
    if g.below(100) < 20 {
        lines.push(format!("RequiredStatus = {}", gen_status_list(g)));
    }
    if g.below(100) < 20 {
        lines.push(format!("ForbiddenStatus = {}", gen_status_list(g)));
    }
    if g.below(100) < 20 {
        lines.push(format!("FriendlyOpacityMin = {}", pick(g, &["25%", "50%"])));
    }
    if g.below(100) < 10 {
        lines.push(format!("FriendlyOpacityMax = {}", pick(g, &["75%", "100%"])));
    }
    if g.below(100) < 10 {
        lines.push(format!("PulseFrequency = {}", pick(g, &["100", "500"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("DisguisesAsTeam = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("RevealDistanceFromTarget = {}", pick(g, &["0", "10", "30"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("OrderIdleEnemiesToAttackMeUponReveal = {}", pick(g, &["Yes", "Yes", "No"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("DisguiseFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("DisguiseRevealFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("DisguiseTransitionTime = {}", pick(g, &["0", "33", "100", "300"])));
    }
    if g.below(100) < 35 {
        lines.push(format!("DisguiseRevealTransitionTime = {}", pick(g, &["0", "33", "100", "300"])));
    }
    if g.below(100) < 30 {
        lines.push(format!("InnateStealth = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 25 {
        lines.push(format!("UseRiderStealth = {}", pick(g, &["Yes", "No"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("EnemyDetectionEvaEvent = {}", pick(g, &["ENEMYBLACKLOTUSDETECTED", "UnitLost"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("OwnDetectionEvaEvent = {}", pick(g, &["OWNBLACKLOTUSDETECTED", "BEACONDETECTED"])));
    }
    if g.below(100) < 25 {
        lines.push(format!("BlackMarketCheckDelay = {}", pick(g, &["0", "100", "1000"])));
    }
    if g.below(100) < 15 {
        lines.push(format!("GrantedBySpecialPower = {}", pick(g, &["Yes", "No"])));
    }
    if !clean {
        lines.push(match g.below(4) {
            0 => "EnemyDetectionEvaEvent = NOSUCHEVENT".to_string(),
            1 => "StealthForbiddenConditions = SLEEPING".to_string(),
            2 => "RequiredStatus = NOSUCHSTATUS".to_string(),
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
    stealthed: u64,
    detected: u64,
    swaps: u64,
    looks: u64,
    wakes_ai: u64,
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

fn template_of(name: &str, kinds: &[KindOfType]) -> Arc<ThingTemplate> {
    let mut values = ObjectTemplateValues { name: name.as_bytes().to_vec(), ..Default::default() };
    // every object is capturable: its radar priority is visible, so the radar's remove and add are both calls (the stand-in logs both)
    values.kind_of.push(KINDOF_CAPTURABLE);
    values.kind_of.extend_from_slice(kinds);
    Arc::new(ThingTemplate::from_object_values(&values))
}

fn rel(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

/// The slots of the probes and modules of the scene.
struct Slots {
    me_stealth: u16,
    rider_stealth: u16,
    ai: u16,
    physics: u16,
    body: u16,
    contain: u16,
    spawn: u16,
    box_contain: u16,
    weapon_store_ready: bool,
}

fn probe_mut(w: &mut World, id: ObjectId, slot: u16) -> &mut ProbeAnswers {
    let o = w.objects.find_mut(id).unwrap();
    let m = &mut o.modules[slot as usize];
    match m.body.as_mut() {
        Some(UpdateBody::ModuleProbe(p)) => &mut p.answers,
        _ => panic!("a probe in slot {slot}"),
    }
}

fn set_status_bits(w: &mut World, id: ObjectId, bits: u32) {
    let o = w.objects.find_mut(id).unwrap();
    for (b, (s, _)) in STATUS.iter().enumerate() {
        o.status.set(*s, (bits >> b & 1) as i32);
    }
}

fn status_bits(w: &World, id: ObjectId) -> u32 {
    let o = w.objects.find(id).unwrap();
    STATUS.iter().enumerate().fold(0, |acc, (b, (s, _))| acc | u32::from(o.test_status(*s)) << b)
}

/// The scene of the configuration (the driver's `scene`).
fn set_scene(w: &mut World, cfg: &[u32; 20], s: &Slots) {
    let f = cfg[3];
    // the other objects have no drawable (the driver's)
    for id in 2..=7 {
        w.objects.find_mut(id).unwrap().drawable = INVALID_DRAWABLE_ID;
    }
    {
        let o = w.objects.find_mut(1).unwrap();
        if f & 1 == 0 {
            o.drawable = INVALID_DRAWABLE_ID;
        } else if o.drawable == INVALID_DRAWABLE_ID {
            o.drawable = 7;
        }
        o.ai = (f >> 1 & 1 != 0).then_some(s.ai);
        o.physics = (f >> 2 & 1 != 0).then_some(s.physics);
        o.body = (f >> 3 & 1 != 0).then_some(s.body);
        o.contain = (f >> 4 & 1 != 0).then_some(s.contain);
        o.contained_by = if f >> 8 & 1 != 0 { 2 } else { INVALID_ID };
        let p = f >> 9 & 3;
        o.team = if p < 3 { p + 1 } else { 0 };
        if f >> 11 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.script_status = if f >> 12 & 1 != 0 { OBJECT_STATUS_SCRIPT_UNSTEALTHED } else { 0 };
    }
    let mut kinds = Vec::new();
    if f >> 6 & 1 != 0 {
        kinds.push(KINDOF_MINE);
    }
    if f >> 7 & 1 != 0 {
        kinds.push(KINDOF_SPAWNS_ARE_THE_WEAPONS);
    }
    w.objects.find_mut(1).unwrap().thing.friend_set_template(template_of(NAMES[0], &kinds));
    probe_mut(w, 1, s.spawn).has = if f >> 5 & 1 != 0 { 1 << 3 } else { 0 };
    probe_mut(w, 1, s.spawn).slaves_stealthed = cfg[12] & 1 != 0;
    {
        let c = probe_mut(w, 1, s.contain);
        c.contained = if f >> 15 & 1 != 0 { vec![3] } else { Vec::new() };
        c.rider = if f >> 16 & 1 != 0 { 3 } else { INVALID_ID };
        c.rider_change = cfg[10] & 1 != 0;
        c.passenger_may_fire = cfg[10] >> 1 & 1 != 0;
        c.yes = cfg[10] >> 2 & 1 != 0;
        c.stealth_contain = true;
    }
    {
        let c = probe_mut(w, 2, s.box_contain);
        c.garrisonable = cfg[11] & 1 != 0;
        c.stealth_contain = true;
    }
    set_status_bits(w, 1, cfg[4]);
    let v = cfg[5] & 7;
    {
        let a = probe_mut(w, 1, s.ai);
        a.current_victim = if v != 0 { v as ObjectId } else { INVALID_ID };
        a.last_command_source = (cfg[5] >> 3 & 7) as i32;
    }
    probe_mut(w, 1, s.physics).velocity = Coord3D { x: f32::from_bits(cfg[6]), y: 0.0, z: 0.0 };
    probe_mut(w, 1, s.body).last_damage = Some((cfg[7], cfg[8] as i32));
    if s.weapon_store_ready {
        let store = w.statics.clone();
        let frame = w.frame();
        let o = w.objects.find_mut(1).unwrap();
        for k in 0..3usize {
            o.weapon_set.slots_mut()[k] = (cfg[9] >> k & 1 != 0).then(|| {
                let mut wp = Weapon::new_in_store(&store.weapon_store, frame, 0, k as i32);
                wp.friend_set_last_shot_frame(cfg[9] >> (8 + 8 * k) & 255);
                wp
            });
        }
    }
    for t in 1..=3 {
        for j in 0..3 {
            w.cuts.team_relationships.insert((t, 101 + j), rel(cfg[13] >> (2 * j) & 3));
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            w.cuts.player_relationships.insert((i as i32, 101 + j), rel(cfg[13] >> (8 + 2 * (3 * i + j)) & 3));
        }
    }
    let local = cfg[14] & 3;
    w.players.friend_set_local_index((local < 3).then_some(local as i32));
    w.cuts.inactive_players.clear();
    for i in 0..3 {
        if cfg[14] >> (4 + i) & 1 != 0 {
            w.cuts.inactive_players.insert(i);
        }
    }
    w.cuts.globals.time_of_day_is_night = cfg[14] >> 8 & 1 != 0;
    {
        let market_kinds: Vec<KindOfType> = if cfg[15] >> 4 & 1 != 0 { vec![KINDOF_FS_BLACK_MARKET] } else { Vec::new() };
        w.objects.find_mut(7).unwrap().thing.friend_set_template(template_of(NAMES[6], &market_kinds));
        let o = w.objects.find_mut(7).unwrap();
        if cfg[15] >> 1 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (cfg[15] >> 2 & 1) as i32);
        o.status.set(OBJECT_STATUS_SOLD, (cfg[15] >> 3 & 1) as i32);
    }
    let p = f >> 9 & 3;
    for i in 0..3u32 {
        let mut list = Vec::new();
        if i == 1 {
            list.push(5);
        }
        if i == 2 {
            list.push(6);
        }
        if cfg[15] & 1 != 0 && p == i {
            list.push(7);
        }
        w.cuts.player_objects.insert(i as i32, list);
    }
    let tp = cfg[16] & 3;
    w.objects.find_mut(4).unwrap().team = if tp < 3 { tp + 1 } else { 0 };
    for k in 0..2u32 {
        let id = 5 + k as ObjectId;
        w.set_position(id, &Coord3D { x: (cfg[17] >> (16 * k) & 255) as f32, y: 0.0, z: 0.0 });
        w.objects.find_mut(id).unwrap().vision_range = (cfg[17] >> (16 * k + 8) & 255) as f32;
    }
    set_status_bits(w, 3, cfg[18] >> 8 & 0xff);
    w.objects.find_mut(3).unwrap().stealth = (cfg[18] & 1 != 0).then_some(s.rider_stealth);
    w.cuts.distance_squared_center_2d.clear();
    w.cuts.distance_squared_center_2d.insert((1, if v != 0 { v as ObjectId } else { 1 }), f32::from_bits(cfg[19]));
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(ctx: &Ctx, data: [ModuleDataKind; 2], cfg_in: &[u32; 20], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut cfg = *cfg_in;
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    for d in &data {
        let ModuleDataKind::StealthUpdate(d) = d else { unreachable!() };
        put_data(&mut v, d, &mut fx);
    }
    let mut w = World::new();
    w.names = ctx.names.clone();
    // the thing factory knows the templates' names (a load finds the disguise by name); the weapon store has one template
    let mut factory = ThingFactory::new();
    for n in ["R26Me", "R26Box", "R26Rider", "R26Tank", "R26Enemy", "R26Market"] {
        factory.new_template(n.as_bytes(), 0).expect("a template");
    }
    let mut weapon_store = rts_lockstep_engine::logic::weapon::WeaponStore::new();
    let mut scratch = rts_lockstep_engine::common::name_key_generator::NameKeyGenerator::new();
    scratch.reset();
    weapon_store.new_weapon_template(&mut scratch, b"R26Gun").unwrap();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center.clone(), thing_factory: factory, weapon_store, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_effect_trace = true;
    w.cuts.body.probe_trace = true;
    w.set_frame(cfg[0]);
    w.random.init_random(cfg[2]);
    w.random.init_game_logic_random(cfg[1]);
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
    }
    for i in 0..3 {
        w.cuts.player_default_teams.insert(i, 101 + i as u32);
        w.cuts.player_colors.insert(i, 0x100 + i);
        w.cuts.player_night_colors.insert(i, 0x200 + i);
    }
    let probe = |name: &[u8], answers: ProbeAnswers| {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default())));
        m
    };
    let [d1, d2] = data;
    let stealth = |w: &World, d: &ModuleDataKind| rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"StealthUpdate", b"ModuleTag_Stealth", d).unwrap().unwrap();
    // object 1: the module, the AI, the physics, the body, the contain module, the spawn behaviour, a sub-object upgrade
    let mut ai = probe(b"R26Ai", ProbeAnswers::default());
    ai.is_ai = true;
    let physics = probe(b"PhysicsBehavior", ProbeAnswers::default());
    let mut body = probe(b"R26Body", ProbeAnswers::default());
    body.interfaces = MODULEINTERFACE_BODY;
    let mut contain = probe(b"R26Contain", ProbeAnswers::default());
    contain.interfaces = MODULEINTERFACE_CONTAIN;
    let spawn = probe(b"R26Spawn", ProbeAnswers::default());
    let mut upgrade = probe(b"R26Upgrade", ProbeAnswers { sub_objects: true, trace_refresh: true, ..Default::default() });
    upgrade.interfaces = MODULEINTERFACE_UPGRADE;
    // the status bits before the constructor, which adds CAN_STEALTH
    let mut initial = ObjectStatusMaskType::default();
    for (b, (s, _)) in STATUS.iter().enumerate() {
        initial.set(*s, (cfg[4] >> b & 1) as i32);
    }
    let m = stealth(&w, &d1);
    let me = w.new_object(NewObject { template: template_of(NAMES[0], &[]), status: initial, team: None, modules: vec![m, ai, physics, body, contain, spawn, upgrade] }).unwrap();
    assert_eq!(me, 1);
    let mut box_contain = probe(b"R26Contain", ProbeAnswers::default());
    box_contain.interfaces = MODULEINTERFACE_CONTAIN;
    assert_eq!(w.new_object(NewObject { template: template_of(NAMES[1], &[]), status: ObjectStatusMaskType::default(), team: None, modules: vec![box_contain] }).unwrap(), 2);
    let mut rider_initial = ObjectStatusMaskType::default();
    for (b, (s, _)) in STATUS.iter().enumerate() {
        rider_initial.set(*s, (cfg[18] >> (8 + b) & 1) as i32);
    }
    let rm = stealth(&w, &d2);
    assert_eq!(w.new_object(NewObject { template: template_of(NAMES[2], &[]), status: rider_initial, team: Some(2), modules: vec![rm] }).unwrap(), 3);
    assert_eq!(w.new_object(NewObject { template: template_of(NAMES[3], &[]), status: ObjectStatusMaskType::default(), team: None, modules: vec![] }).unwrap(), 4);
    for k in 0..2u32 {
        let mut ai = probe(b"R26Ai", ProbeAnswers::default());
        ai.is_ai = true;
        assert_eq!(w.new_object(NewObject { template: template_of(NAMES[4], &[]), status: ObjectStatusMaskType::default(), team: Some(2 + k), modules: vec![ai] }).unwrap(), 5 + k as ObjectId);
    }
    assert_eq!(w.new_object(NewObject { template: template_of(NAMES[6], &[]), status: ObjectStatusMaskType::default(), team: None, modules: vec![] }).unwrap(), 7);
    w.set_position(1, &Coord3D { x: 10.0, y: 0.0, z: 0.0 });
    w.objects.find_mut(1).unwrap().on_radar = true;
    let slot = |w: &World, id: ObjectId, name: &[u8]| w.find_module(id, name).unwrap();
    let slots = Slots {
        me_stealth: slot(&w, 1, b"StealthUpdate"),
        rider_stealth: slot(&w, 3, b"StealthUpdate"),
        ai: slot(&w, 1, b"R26Ai"),
        physics: slot(&w, 1, b"PhysicsBehavior"),
        body: slot(&w, 1, b"R26Body"),
        contain: slot(&w, 1, b"R26Contain"),
        spawn: slot(&w, 1, b"R26Spawn"),
        box_contain: slot(&w, 2, b"R26Contain"),
        weapon_store_ready: true,
    };
    let u = w.update_handle_of_module(ModuleHandle { object: 1, index: slots.me_stealth }).expect("an update module");
    let ur = w.update_handle_of_module(ModuleHandle { object: 3, index: slots.rider_stealth }).expect("an update module");
    // the constructors' wake calls
    let (mut cw, mut cl) = (0u32, 0u32);
    // (the two stealth modules' calls: the line names the module's slot, the same on both objects; the helpers' slots are others)
    let stealth_slot = |l: &str| -> Option<u32> {
        let (i, f) = l.split_once(' ').unwrap();
        let i: i32 = i.parse().unwrap();
        (i == i32::from(slots.me_stealth) || i == i32::from(slots.rider_stealth)).then(|| f.parse().unwrap())
    };
    for t in w.cuts.trace.take().unwrap() {
        if let Some(f) = t.strip_prefix("awaken ").and_then(stealth_slot) {
            cw += 1;
            cl = f;
        }
    }
    put_u32(&mut v, cw);
    put_u32(&mut v, cl);
    // the constructors' status bits are the scene's from now on
    cfg[4] = cfg[4] & !0xff | status_bits(&w, 1);
    cfg[18] = cfg[18] & !0xff00 | status_bits(&w, 3) << 8;
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 6 && (3..20).contains(&o[1]) {
            cfg[o[1] as usize] = o[2];
        }
        if o[0] == 11 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &slots);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        let target = |a: u32| if (1..=7).contains(&a) { a as ObjectId } else { INVALID_ID };
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1 => {
                res = w.with_body(ur, |b, w| b.update(w, ur)).expect("the module is in its slot").0;
            }
            2 => {
                w.module_call(1, slots.me_stealth, if o[1] == 0 { ModuleCall::StealthMarkAsDetected } else { ModuleCall::StealthMarkAsDetectedFor(o[1]) });
            }
            3 => {
                w.module_call(1, slots.me_stealth, ModuleCall::StealthReceiveGrant { active: o[1] & 1 != 0, frames: o[2] });
            }
            4 => {
                w.module_call(1, slots.me_stealth, ModuleCall::StealthDisguiseAsObject(target(o[1])));
            }
            5 => {
                w.module_call(3, slots.rider_stealth, ModuleCall::StealthDisguiseAsObject(target(o[1])));
            }
            7..=9 => {
                let r = run_xfer(&mut w, 1, slots.me_stealth as usize, o[0] - 7, &saved);
                // `throw SC_INVALID_DATA` is caught by the driver's `catch (...)`: -1
                xst = if r.0 == XferStatus::ErrorUnknown as i32 { -1 } else { r.0 };
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
            10 => {
                // `loadPostProcess()` on the module with the scheduling half of its slot
                let mut body = w.objects.find_mut(1).unwrap().behaviors[u.index as usize].body.take().unwrap();
                let mut update = w.objects.find(1).unwrap().behaviors[u.index as usize].update.clone();
                if let UpdateBody::StealthUpdate(m) = &mut body {
                    m.load_post_process_with(&mut update);
                }
                w.objects.find_mut(1).unwrap().behaviors[u.index as usize].body = Some(body);
                w.objects.find_mut(1).unwrap().behaviors[u.index as usize].update = update;
            }
            6 | 11 => {}
            _ => unreachable!(),
        }
        cfg[4] = cfg[4] & !0xff | status_bits(&w, 1);
        cfg[18] = cfg[18] & !0xff00 | status_bits(&w, 3) << 8;
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let (selected, hidden) = (cfg[3] >> 13 & 1 != 0, cfg[3] >> 14 & 1 != 0);
        let (mut ev_iter, mut cut_iter) = (events.into_iter(), w.cuts.log.take().unwrap().into_iter());
        let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                if let Some(CutCall::PartitionGetDistanceSquaredCenter2D { object, other }) = cut_iter.next() {
                    log.push(format!("distsq {object} {other}"));
                }
            } else if t.starts_with("evt ") {
                match ev_iter.next() {
                    Some(ObjectClientEvent::Stealth { object_id, op }) => match op {
                        StealthClientOp::EffectiveOpacity { opacity, explicit } => log.push(format!("opacity {:08x} {:08x}", opacity.to_bits(), explicit.to_bits())),
                        StealthClientOp::SecondMaterialPassOpacity(x) => log.push(format!("secondpass {object_id} {:08x}", x.to_bits())),
                        StealthClientOp::StealthLook(l) => {
                            tally.looks += 1;
                            log.push(format!("look {l}"))
                        }
                        StealthClientOp::ReplaceDrawable { template } => {
                            tally.swaps += 1;
                            log.push(format!("destroydraw {object_id}"));
                            log.push(format!("newdraw {}", String::from_utf8_lossy(&template)));
                            log.push(format!("bind {object_id}"));
                        }
                        StealthClientOp::IndicatorColor(c) => log.push(format!("indicator {object_id} {c}")),
                        StealthClientOp::SelectIfWasSelected => {
                            if selected {
                                log.push(format!("uiselect {object_id}"));
                            }
                        }
                        StealthClientOp::HideIfWasHidden => {
                            if hidden {
                                log.push(format!("hidden {object_id} 1"));
                            }
                        }
                        StealthClientOp::MarkUiDirtyIfSelected => {
                            if selected {
                                log.push("markuidirty".to_string());
                            }
                        }
                    },
                    Some(ObjectClientEvent::AudioAddEvent { name, object_id, .. }) => log.push(format!("audio {} - -1 {object_id}", String::from_utf8_lossy(&name))),
                    Some(ObjectClientEvent::FxListDoFxPos { fx: name, position }) => {
                        let id = fx.id(&Some(name));
                        log.push(format!("fxpos {id} {},{},{}", position.x.to_bits(), position.y.to_bits(), position.z.to_bits()));
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(a), .. }) => log.push(format!("mc {}", a[0])),
                    Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(a), .. }) => log.push(format!("mcclear {}", a[0])),
                    Some(ObjectClientEvent::Radar { call: RadarCall::AddObject, object_id, .. }) => log.push(format!("radaradd {object_id}")),
                    Some(ObjectClientEvent::Radar { call: RadarCall::RemoveObject, object_id, .. }) => log.push(format!("radarremove {object_id}")),
                    _ => {}
                }
            } else if let Some(f) = t.strip_prefix("awaken ").and_then(stealth_slot) {
                wakes += 1;
                last = f;
            } else if let Some(l) = t.strip_prefix("bt ") {
                if ["resetphysics ", "aiwake ", "revealslaves ", "recalcapparent ", "refreshsub "].iter().any(|p| l.starts_with(p)) {
                    if l.starts_with("aiwake ") {
                        tally.wakes_ai += 1;
                    }
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
        for (id, h) in [(1, u), (3, ur)] {
            let o = w.objects.find(id).unwrap();
            let slot = &o.behaviors[h.index as usize];
            let Some(UpdateBody::StealthUpdate(m)) = slot.body.as_ref() else { panic!("the module is back in its slot") };
            for s in m.state_words() {
                put_u32(&mut v, s);
            }
            put_str(&mut v, &m.disguise_template_name());
            put_u32(&mut v, slot.update.friend_get_next_call_frame());
        }
        let (s1, s3) = (status_bits(&w, 1), status_bits(&w, 3));
        if s1 >> 3 & 1 != 0 {
            tally.stealthed += 1;
        }
        if s1 >> 4 & 1 != 0 {
            tally.detected += 1;
        }
        put_u32(&mut v, s1);
        put_u32(&mut v, s3);
        for p in 0..3 {
            put_u32(&mut v, w.players.get_nth(p).map_or(0, |p| p.academy_stats.vehicles_disguised()));
        }
        put_u32(&mut v, u32::from(w.has_drawable(1)));
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R26_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn stealth_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0026_57EA);
    let mut read = Check::new("R26 data read (throw, message)");
    let mut run = Check::new("R26 ops (data, effects, wakes, xfers, state)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut shown = 0;
    let mut reasons = std::collections::BTreeMap::new();
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let clean2 = g.below(10) < 9;
        let text2 = gen_text(&mut g, clean2);
        let name = format!("Data\\INI\\R26{}.ini", case % 5).into_bytes();
        let name2 = format!("Data\\INI\\R26r{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let mut outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut data2 = None;
        if outcome.status == 0 {
            let (r2, d2) = rust_read(&name2, &text2);
            outcome = match rust_outcome(r2, 0) {
                Ok(o) => o,
                Err(e) => {
                    assert!(e.is_undefined(), "{e}");
                    excluded += 1;
                    continue;
                }
            };
            data2 = d2;
        }
        let gen_me = |g: &mut Gen| {
            u32::from(g.below(6) != 0)
                | u32::from(g.below(4) != 0) << 1
                | u32::from(g.below(8) != 0) << 2
                | u32::from(g.below(3) != 0) << 3
                | u32::from(g.below(3) == 0) << 4
                | u32::from(g.below(5) == 0) << 5
                | u32::from(g.below(10) == 0) << 6
                | u32::from(g.below(6) == 0) << 7
                | u32::from(g.below(4) == 0) << 8
                | (if g.below(6) == 0 { 3 } else { g.below(3) }) << 9
                | u32::from(g.below(12) == 0) << 11
                | u32::from(g.below(10) == 0) << 12
                | u32::from(g.below(3) == 0) << 13
                | u32::from(g.below(3) == 0) << 14
                | u32::from(g.below(2) == 0) << 15
                | u32::from(g.below(2) == 0) << 16
        };
        let gen_status = |g: &mut Gen| (0..8).fold(0, |acc, b| acc | u32::from(g.below(if b == 2 { 2 } else { 6 }) == 0) << b);
        let gen_rel = |g: &mut Gen| (0..13).fold(0u32, |acc, k| acc | g.below(3) << (if k < 3 { 2 * k } else { 8 + 2 * (k - 3) }));
        let frame0 = g.below(200);
        let gen_weapons = |g: &mut Gen, frame: u32| (0..3).fold(0, |acc, k| acc | g.below(2) << k | (frame.saturating_sub(g.below(3)) & 255) << (8 + 8 * k));
        let mut cfg = [
            frame0,
            g.u32(),
            g.u32(),
            gen_me(&mut g),
            gen_status(&mut g),
            g.below(8) | g.below(5) << 3,
            (*pick(&mut g, &[&0.0f32, &1.0, &10.0, &40.0])).to_bits(),
            frame0.saturating_sub(g.below(3)) | if g.below(4) == 0 { 0xffff_ffff } else { 0 },
            *pick(&mut g, &[&0u32, &10, &3]),
            gen_weapons(&mut g, frame0),
            g.below(8),
            g.below(2),
            g.below(2),
            gen_rel(&mut g),
            (if g.below(5) == 0 { 3 } else { g.below(3) }) | u32::from(g.below(8) == 0) << 4 | u32::from(g.below(8) == 0) << 5 | u32::from(g.below(8) == 0) << 6 | g.below(2) << 8,
            g.below(32),
            if g.below(5) == 0 { 3 } else { g.below(3) },
            g.below(60) | g.below(80) << 8 | g.below(60) << 16 | g.below(80) << 24,
            g.below(2) | gen_status(&mut g) << 8,
            (*pick(&mut g, &[&0.0f32, &50.0, &400.0, &2500.0])).to_bits(),
        ];
        if cfg[7] == 0xffff_ffff | frame0.saturating_sub(0) {
            cfg[7] = 0xffff_ffff;
        }
        let n_ops = 1 + g.below(30) as usize;
        let mut frame = cfg[0];
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(33) {
                0..=10 => [0, 0, 0, 0, 0, 0],
                11 => [1, 0, 0, 0, 0, 0],
                12..=13 => [2, *pick(&mut g, &[&0u32, &0, &5, &100]), 0, 0, 0, 0],
                14 => [3, g.below(2), *pick(&mut g, &[&0u32, &3, &30]), 0, 0, 0],
                15..=17 => [4, *pick(&mut g, &[&0u32, &4, &4, &3, &1]), 0, 0, 0, 0],
                18 => [5, *pick(&mut g, &[&0u32, &4]), 0, 0, 0, 0],
                30..=32 => [0, 0, 0, 0, 0, 0],
                19..=20 => {
                    let a = 3 + g.below(17);
                    let b = match a {
                        3 => gen_me(&mut g),
                        4 | 18 => cfg[a as usize] ^ 1 << g.below(16),
                        13 => gen_rel(&mut g),
                        _ => cfg[a as usize] ^ 1 << g.below(5),
                    };
                    [6, a, b, 0, 0, 0]
                }
                21..=22 => {
                    frame += 1 + g.below(40);
                    [11, frame, 0, 0, 0, 0]
                }
                23..=24 => [7, 0, 0, 0, 0, 0],
                25..=26 => [8, 0, 0, 0, 0, 0],
                27 => [10, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(&ctx, [data.expect("read ok"), data2.expect("read ok")], &cfg, &ops, &mut tally);
            if let Some((_, e)) = w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
                *reasons.entry(format!("{e}")).or_insert(0u64) += 1;
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cfile2 = CString::new(name2.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_ini_add_file(cfile2.as_ptr(), text2.as_ptr() as *const _, text2.len() as i32);
            cpp::ref_l9r26_run(
                cfile.as_ptr(),
                cfile2.as_ptr(),
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
        let what = || format!("case {case}: cfg {cfg:x?}, files {:?} {:?}, ops {ops:?}", show(&text), show(&text2));
        if status != 0 && std::env::var("L9R26_SHOW_THROWS").is_ok() {
            eprintln!("throw {status} {:?} {:?} {:?}", show(&msg[..code.min(400) as usize]), show(&text), show(&text2));
        }
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} files {:?} {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), show(&text2), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "R26: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} xfers, \
         {} effect lines, {} ops stealthed, {} detected, {} drawable swaps, {} looks, {} AI wakes",
        tally.updates, tally.xfers, tally.effects, tally.stealthed, tally.detected, tally.swaps, tally.looks, tally.wakes_ai
    );
    println!("R26 excluded: {reasons:?}");
    read.finish();
    run.finish();
    assert!(tally.stealthed * 4 > cases as u64 && tally.detected * 8 > cases as u64 && tally.swaps * 8 > cases as u64 && tally.wakes_ai * 20 > cases as u64, "the generator must reach every path");
}
