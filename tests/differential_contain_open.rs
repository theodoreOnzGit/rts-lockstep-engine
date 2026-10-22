#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `OpenContain` (`logic::modules::open_contain`) against the unchanged C++, `p1w13-sonnet`, batch R.
//!
//! # Method
//!
//! The C++ side is `OpenContain.cpp`, compiled UNCHANGED into `wrap_module.cpp`'s unit (`differential/l9_open_contain_r.inc`) on the module build's stand-ins, with
//! the REAL `DieMuxData`, `INI`, `RandomValue`, `Matrix3D`, `Xfer`, `Object::kill` and `Object::getRelationship`. A scene of up to four objects is built, each with a
//! team and a player, optionally a drawable (selected, with model conditions), an AI, a physics block, a stealth update and a body, and an `OpenContain` on those that
//! have one (the first reads the INI, the others take their data from the scene). Every call the container makes on the rest of the world is a line of the effect log,
//! in the order it is made.
//!
//! The Rust side is the real thing: a `World` with the same scene (`World::new_object` with the container from `modules::new_module` and `ProbeModule`s for the
//! AI, physics, stealth and body), the cut points of `contain_cuts` answered from the same tables, `cuts.contain.stub_object_calls` set (the Object-level calls that
//! cascade are records only; see `contain_cuts`), and the ops run as `Object` runs them: `World::module_call`. The scene is one list of words (`cfg`), written by
//! `Obj::write` and read by `Obj::read` (the C++ driver reads the same order).
//!
//! Each case reads a `D` block (the fifteen fields; mostly well formed) for the first container and runs 1 to about 60 ops: adding and removing objects, killing
//! and harming them, collisions, deaths, updates, the exits through the door, the orders to the passengers, the queries, the environment (the frame, the selection,
//! the model conditions, the statuses, the places, the booby traps, the AI) and the three xfers with the load post process.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, a result word, the effects in order, the xfer status, bytes and CRC, and the state of
//! every object (the container's lists, counters, fire points, rally point and flags; the place, the container, the statuses).
//!
//! # Deviations the test cannot see
//!
//! The Object-level calls that cascade are stubbed on the Rust side (`onContainedBy`, `onRemovedFrom`, `kill`, `attemptDamage`, `setPosition`, `setLayer`,
//! `setOrientation`, `setTransformMatrix`, `leaveGroup`, `checkAndDetonateBoobyTrap`, `destroyObject`): the C++ side stands in for them with a log line, and each has
//! its own differential test. A container that kills or damages a contained object takes `getBodyModule()->getMaxHealth()` from a body that logs. The partition
//! manager, the pathfinder, the terrain layers and the art's bones are stand-ins on both sides. Upstream destroys the objects of a contain list that a load finds
//! where the list was not empty in the middle of the xfer; the translation (which has no world there) hands them to the world (`DrainXferDestroys`), which the test
//! asks for straight after the xfer: the same lines, in the same op.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::model_state::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::*;
use rts_lockstep_engine::logic::modules::open_contain::{OpenContain, OpenContainModuleData};
use rts_lockstep_engine::logic::world::body_cuts::info_line;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9ct_r.bin");
const XFER_FILE: &str = "l9ct_r.bin";

/// The kinds, statuses and model conditions the scene's masks name (local bit i is `KINDS[i]`, ...), as the driver's `kCtKinds`, `kCtStatus` and `kCtConds`.
const KINDS: [i32; 8] =
    [KINDOF_STEALTH_GARRISON, KINDOF_STRUCTURE, KINDOF_VEHICLE, KINDOF_MONEY_HACKER, KINDOF_INFANTRY, KINDOF_SELECTABLE, KINDOF_PROJECTILE, KINDOF_IMMOBILE];
const KIND_NAMES: [&str; 8] = ["STEALTH_GARRISON", "STRUCTURE", "VEHICLE", "MONEY_HACKER", "INFANTRY", "SELECTABLE", "PROJECTILE", "IMMOBILE"];
const STATUSES: [i32; 3] = [OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED];
const STATUS_NAMES: [&str; 3] = ["IS_ATTACKING", "AIRBORNE_TARGET", "STEALTHED"];
const CONDS: [i32; 6] = [MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_NIGHT, MODELCONDITION_SNOW];
const BONES: [&str; 8] = ["ExitStart", "ExitEnd", "ExitStart01", "ExitEnd01", "ExitStart02", "ExitEnd02", "ExitStart03", "ExitEnd03"];
const FP: usize = 4;

// ---- the scene, as words -------------------------------------------------------------------------------------------------------------------------------------------

fn f(x: u32) -> f32 {
    f32::from_bits(x)
}

struct W(Vec<u32>);
impl W {
    fn u(&mut self, x: u32) {
        self.0.push(x);
    }
    fn s(&mut self, x: i32) {
        self.0.push(x as u32);
    }
    fn f(&mut self, x: f32) {
        self.0.push(x.to_bits());
    }
}

struct R<'a> {
    w: &'a [u32],
    i: usize,
}
impl R<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn s(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
}

/// One object of the scene: the words are written by [`Obj::write`] and read by [`Obj::read`] in the order the C++ driver (`l9ctRunInner`) reads them.
#[derive(Clone, Debug, Default)]
struct Obj {
    snd: [u32; 3],
    kinds: u32,
    status: u32,
    player: i32,
    pos: [f32; 3],
    orient: f32,
    layer: u32,
    radius: f32,
    drawable: bool,
    selected: bool,
    cond: u32,
    has_ai: bool,
    ai_idle: bool,
    enter_target: u32,
    has_phys: bool,
    mass: f32,
    can_fall: bool,
    has_stealth: bool,
    has_body: bool,
    max_health: f32,
    booby: bool,
    eff_dead: bool,
    weapon_bonus: u32,
    contained_by: u32,
    has_contain: bool,
    contain_max: i32,
    enter_snd: u32,
    exit_snd: u32,
    allow_fire: bool,
    in_turret: bool,
    exit_paths: i32,
    damage_pct: f32,
    burned: bool,
    door_open: u32,
    allow_mode: u32,
    allow_mask: u32,
    forbid_mask: u32,
    weapon_pass: bool,
    allow_rel: u32,
    death_mask: u32,
    vet_mask: u32,
    exempt_mask: u32,
    required_mask: u32,
    adj_has: bool,
    adj: [f32; 3],
    fp_count: u32,
    fp: [[f32; 3]; FP],
    bone_has: [bool; 8],
    bone: [[f32; 3]; 8],
    turret_has: [bool; FP],
    turret: [[f32; 3]; FP],
}

impl Obj {
    fn write(&self, w: &mut W) {
        for s in self.snd {
            w.u(s);
        }
        w.u(self.kinds);
        w.u(self.status);
        w.s(self.player);
        for p in self.pos {
            w.f(p);
        }
        w.f(self.orient);
        w.u(self.layer);
        w.f(self.radius);
        w.u(u32::from(self.drawable));
        w.u(u32::from(self.selected));
        w.u(self.cond);
        w.u(u32::from(self.has_ai));
        w.u(u32::from(self.ai_idle));
        w.u(self.enter_target);
        w.u(u32::from(self.has_phys));
        w.f(self.mass);
        w.u(u32::from(self.can_fall));
        w.u(u32::from(self.has_stealth));
        w.u(u32::from(self.has_body));
        w.f(self.max_health);
        w.u(u32::from(self.booby));
        w.u(u32::from(self.eff_dead));
        w.u(self.weapon_bonus);
        w.u(self.contained_by);
        w.u(u32::from(self.has_contain));
        w.s(self.contain_max);
        w.u(self.enter_snd);
        w.u(self.exit_snd);
        w.u(u32::from(self.allow_fire));
        w.u(u32::from(self.in_turret));
        w.s(self.exit_paths);
        w.f(self.damage_pct);
        w.u(u32::from(self.burned));
        w.u(self.door_open);
        w.u(self.allow_mode);
        w.u(self.allow_mask);
        w.u(self.forbid_mask);
        w.u(u32::from(self.weapon_pass));
        w.u(self.allow_rel);
        w.u(self.death_mask);
        w.u(self.vet_mask);
        w.u(self.exempt_mask);
        w.u(self.required_mask);
        w.u(u32::from(self.adj_has));
        for p in self.adj {
            w.f(p);
        }
        w.u(self.fp_count);
        for j in 0..FP {
            for p in self.fp[j] {
                w.f(p);
            }
        }
        for j in 0..8 {
            w.u(u32::from(self.bone_has[j]));
            for p in self.bone[j] {
                w.f(p);
            }
        }
        for j in 0..FP {
            w.u(u32::from(self.turret_has[j]));
            for p in self.turret[j] {
                w.f(p);
            }
        }
    }

    fn read(r: &mut R<'_>) -> Obj {
        let mut o = Obj::default();
        for s in o.snd.iter_mut() {
            *s = r.u();
        }
        o.kinds = r.u();
        o.status = r.u();
        o.player = r.s();
        for p in o.pos.iter_mut() {
            *p = r.f();
        }
        o.orient = r.f();
        o.layer = r.u();
        o.radius = r.f();
        o.drawable = r.u() != 0;
        o.selected = r.u() != 0;
        o.cond = r.u();
        o.has_ai = r.u() != 0;
        o.ai_idle = r.u() != 0;
        o.enter_target = r.u();
        o.has_phys = r.u() != 0;
        o.mass = r.f();
        o.can_fall = r.u() != 0;
        o.has_stealth = r.u() != 0;
        o.has_body = r.u() != 0;
        o.max_health = r.f();
        o.booby = r.u() != 0;
        o.eff_dead = r.u() != 0;
        o.weapon_bonus = r.u();
        o.contained_by = r.u();
        o.has_contain = r.u() != 0;
        o.contain_max = r.s();
        o.enter_snd = r.u();
        o.exit_snd = r.u();
        o.allow_fire = r.u() != 0;
        o.in_turret = r.u() != 0;
        o.exit_paths = r.s();
        o.damage_pct = r.f();
        o.burned = r.u() != 0;
        o.door_open = r.u();
        o.allow_mode = r.u();
        o.allow_mask = r.u();
        o.forbid_mask = r.u();
        o.weapon_pass = r.u() != 0;
        o.allow_rel = r.u();
        o.death_mask = r.u();
        o.vet_mask = r.u();
        o.exempt_mask = r.u();
        o.required_mask = r.u();
        o.adj_has = r.u() != 0;
        for p in o.adj.iter_mut() {
            *p = r.f();
        }
        o.fp_count = r.u();
        for j in 0..FP {
            for p in o.fp[j].iter_mut() {
                *p = r.f();
            }
        }
        for j in 0..8 {
            o.bone_has[j] = r.u() != 0;
            for p in o.bone[j].iter_mut() {
                *p = r.f();
            }
        }
        for j in 0..FP {
            o.turret_has[j] = r.u() != 0;
            for p in o.turret[j].iter_mut() {
                *p = r.f();
            }
        }
        o
    }
}

struct Scene {
    frame: u32,
    logic_seed: u32,
    client_seed: u32,
    objs: Vec<Obj>,
    rel: Vec<Vec<i32>>,
    layer: [i32; 8],
    layer_height: [f32; 8],
}

fn parse_scene(cfg: &[u32]) -> Scene {
    let mut r = R { w: cfg, i: 0 };
    let frame = r.u();
    let (logic_seed, client_seed) = (r.u(), r.u());
    let n = r.u() as usize;
    let objs: Vec<Obj> = (0..n).map(|_| Obj::read(&mut r)).collect();
    let rel = (0..n).map(|_| (0..n).map(|_| r.s()).collect()).collect();
    let mut layer = [0; 8];
    for l in layer.iter_mut() {
        *l = r.s();
    }
    let mut layer_height = [0.0; 8];
    for l in layer_height.iter_mut() {
        *l = r.f();
    }
    Scene { frame, logic_seed, client_seed, objs, rel, layer, layer_height }
}

// ---- the generators ------------------------------------------------------------------------------------------------------------------------------------------------

fn health_str(g: &mut Gen, clean: bool) -> String {
    match g.below(8) {
        0..=5 => format!("{}", *pick(g, &[&1i32, &2, &3, &5, &10, &-1, &0])),
        6 => "0".to_string(),
        _ if clean => "4".to_string(),
        _ => gen_int(g, clean),
    }
}

/// A list of names from `names` (a mask over the local list): at least one, `NONE` sometimes.
fn name_list(g: &mut Gen, names: &[&str], clean: bool) -> String {
    let n = 1 + g.below(3);
    let mut v: Vec<String> = (0..n).map(|_| names[g.below(names.len() as u32) as usize].to_string()).collect();
    if g.below(8) == 0 {
        v.insert(0, "NONE".to_string());
    }
    if !clean && g.below(12) == 0 {
        v.push("NOT_A_NAME".to_string());
    }
    v.join(" ")
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    macro_rules! field {
        ($p:expr, $name:expr, $value:expr) => {
            if g.below(100) < $p {
                let value: String = $value;
                let sep = gen_sep(g);
                lines.push(format!("{}{}{}", $name, sep, value));
            }
        };
    }
    field!(70, "ContainMax", health_str(g, clean));
    field!(50, "EnterSound", format!("snd{}", 1 + g.below(5)));
    field!(50, "ExitSound", format!("snd{}", 1 + g.below(5)));
    field!(40, "DamagePercentToUnits", format!("{}%", *pick(g, &[&0i32, &25, &50, &100, &150])));
    field!(40, "BurnedDeathToUnits", gen_bool(g, clean));
    field!(50, "AllowInsideKindOf", name_list(g, &KIND_NAMES, clean));
    field!(50, "ForbidInsideKindOf", name_list(g, &KIND_NAMES, clean));
    field!(50, "PassengersAllowedToFire", gen_bool(g, clean));
    field!(40, "PassengersInTurret", gen_bool(g, clean));
    field!(50, "NumberOfExitPaths", format!("{}", *pick(g, &[&0i32, &1, &2, &3, &4, &-1])));
    field!(50, "DoorOpenTime", gen_int(g, clean));
    field!(40, "WeaponBonusPassedToPassengers", gen_bool(g, clean));
    field!(40, "AllowAlliesInside", gen_bool(g, clean));
    field!(40, "AllowEnemiesInside", gen_bool(g, clean));
    field!(40, "AllowNeutralInside", gen_bool(g, clean));
    field!(30, "DeathTypes", name_list(g, &["NORMAL", "EXPLODED", "BURNED", "CRUSHED", "POISONED"], clean));
    field!(25, "VeterancyLevels", name_list(g, &["REGULAR", "VETERAN", "ELITE", "HEROIC"], clean));
    field!(25, "ExemptStatus", name_list(g, &STATUS_NAMES, clean));
    field!(25, "RequiredStatus", name_list(g, &STATUS_NAMES, clean));
    if !clean && g.below(12) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn coord(g: &mut Gen) -> f32 {
    *pick(g, &[&0.0f32, &10.0, &50.5, &-20.0, &100.0, &3.5])
}

fn gen_obj(g: &mut Gen, k: usize, n: usize) -> Obj {
    let mut o = Obj::default();
    for s in o.snd.iter_mut() {
        *s = if g.below(3) == 0 { 0 } else { 1 + g.below(6) };
    }
    o.kinds = g.below(256) & g.below(256) | if g.below(3) == 0 { 1 } else { 0 };
    o.status = g.below(8) & g.below(8);
    o.player = if g.below(40) == 0 { -1 } else { g.below(4) as i32 };
    for p in o.pos.iter_mut() {
        *p = coord(g);
    }
    o.orient = *pick(g, &[&0.0f32, &1.5, &3.0, &-1.0]);
    o.layer = g.below(3);
    o.radius = *pick(g, &[&0.0f32, &5.0, &20.0, &10.0]);
    o.drawable = g.below(4) != 0;
    o.selected = g.below(3) == 0;
    o.cond = g.below(64);
    o.has_ai = g.below(10) < 7;
    o.ai_idle = g.below(2) == 0;
    o.enter_target = if g.below(3) == 0 { 0 } else { 1 + g.below(n as u32) };
    o.has_phys = g.below(3) == 0;
    o.mass = *pick(g, &[&1.0f32, &25.0, &100.0, &0.5]);
    o.can_fall = g.below(2) == 0;
    o.has_stealth = g.below(3) == 0;
    o.has_body = g.below(10) < 8;
    o.max_health = *pick(g, &[&100.0f32, &200.0, &50.0, &0.0, &1000.0]);
    o.booby = g.below(12) == 0;
    o.eff_dead = g.below(15) == 0;
    o.weapon_bonus = g.below(8);
    // an object is contained only by an EARLIER one that has a container (so the scene starts without a ring)
    o.contained_by = if k > 0 && g.below(5) == 0 { 1 + g.below(k as u32) } else { 0 };
    o.has_contain = k == 0 || g.below(10) < 6;
    o.contain_max = *pick(g, &[&-1i32, &-1, &1, &2, &3, &0]);
    o.enter_snd = if g.below(3) == 0 { 0 } else { 1 + g.below(5) };
    o.exit_snd = if g.below(3) == 0 { 0 } else { 1 + g.below(5) };
    o.allow_fire = g.below(2) == 0;
    o.in_turret = g.below(5) == 0;
    o.exit_paths = *pick(g, &[&0i32, &1, &1, &2, &3, &12]);
    o.damage_pct = *pick(g, &[&0.0f32, &0.0, &0.25, &0.5, &1.0]);
    o.burned = g.below(2) == 0;
    o.door_open = *pick(g, &[&0u32, &1, &1, &3]);
    o.allow_mode = g.below(2);
    o.allow_mask = g.below(256);
    o.forbid_mask = g.below(256) & g.below(256);
    o.weapon_pass = g.below(2) == 0;
    o.allow_rel = if g.below(3) == 0 { g.below(8) } else { 7 };
    o.death_mask = if g.below(2) == 0 { 0xffff_ffff } else { g.below(32) };
    o.vet_mask = if g.below(2) == 0 { 0xffff_ffff } else { g.below(16) };
    o.exempt_mask = if g.below(4) == 0 { g.below(8) } else { 0 };
    o.required_mask = if g.below(4) == 0 { g.below(8) } else { 0 };
    o.adj_has = g.below(2) == 0;
    for p in o.adj.iter_mut() {
        *p = coord(g);
    }
    o.fp_count = g.below(FP as u32 + 1);
    for j in 0..FP {
        for p in o.fp[j].iter_mut() {
            *p = coord(g);
        }
    }
    for j in 0..8 {
        o.bone_has[j] = g.below(4) != 0;
        for p in o.bone[j].iter_mut() {
            *p = coord(g);
        }
    }
    for j in 0..FP {
        o.turret_has[j] = g.below(3) != 0;
        for p in o.turret[j].iter_mut() {
            *p = coord(g);
        }
    }
    o
}

fn gen_scene(g: &mut Gen, _data: &Option<ModuleDataKind>) -> Vec<u32> {
    let mut w = W(Vec::new());
    let n = 2 + g.below(3) as usize;
    w.u(g.below(300));
    w.u(g.u32());
    w.u(g.u32());
    w.u(n as u32);
    for k in 0..n {
        gen_obj(g, k, n).write(&mut w);
    }
    for i in 0..n {
        for j in 0..n {
            w.s(if i == j { 2 } else { *pick(g, &[&0i32, &1, &2, &2]) });
        }
    }
    for _ in 0..8 {
        w.s(g.below(3) as i32);
    }
    for _ in 0..8 {
        w.f(*pick(g, &[&0.0f32, &5.0, &12.5, &-3.0]));
    }
    w.0
}

/// One op of five words: `[op, container, a, b, c]`.
fn gen_ops(g: &mut Gen, n: u32) -> Vec<[u32; 5]> {
    let count = 1 + g.below(60);
    let mut ops = Vec::new();
    for _ in 0..count {
        let c = g.below(n);
        let a = g.below(n);
        let op = match g.below(100) {
            0..=17 => [0, c, a, 0, 0],
            18..=24 => [1, c, a, g.below(2), 0],
            25..=27 => [2, c, g.below(2), 0, 0],
            28..=30 => [3, c, 0, 0, 0],
            31..=33 => [4, c, g.below(8), g.below(5), g.below(6)],
            34..=38 => [5, c, a, 0, 0],
            39..=41 => [6, c, g.below(8), g.below(5), g.below(6)],
            42..=46 => [7, c, 0, 0, 0],
            47..=49 => [8, c, a, g.below(2), 0],
            50..=55 => [9, c, a, 0, 0],
            56..=58 => [10, c, a, 0, 0],
            59..=61 => [11, c, g.below(2), 0, 0],
            62 => [12, c, 0, 0, 0],
            63 => [13, c, 0, 0, 0],
            64 => [14, c, 0, 0, 0],
            65..=67 => [15, c, (*pick(g, &[&0.0f32, &0.25, &0.5, &1.0, &1.5])).to_bits(), 0, 0],
            68..=70 => [16, c, a, g.below(3), 0],
            71 => [17, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            72 => [18, c, 0, 0, 0],
            73 => [19, c, g.below(2), 0, 0],
            74..=75 => [20, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            76 => [21, c, 0, 0, 0],
            77 => [22, c, 0, 0, 0],
            78 => [23, c, 0, 0, 0],
            79 => [24, c, g.below(2), 0, 0],
            80 => [25, c, 0, 0, 0],
            81..=82 => [26, c, a, 0, 0],
            83..=84 => [27, c, a, 0, 0],
            85 => [28, c, g.below(2), 0, 0],
            86 => [29, c, 0, 0, 0],
            87 => [30, c, 0, 0, 0],
            88 => [31, c, 0, 0, 0],
            89 => [32, c, 0, 0, 0],
            90 => [33, c, 0, 0, 0],
            91 => [34, 0, g.below(300), 0, 0],
            92 => [35, c, g.below(2), 0, 0],
            93 => [36, c, g.below(64), 0, 0],
            94 => [37, c, g.below(8), 0, 0],
            95 => [38, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            96 => [39, c, g.below(2), 0, 0],
            97 => [40, c, g.below(2), 0, 0],
            98 => [41, c, g.below(4), 0, 0],
            _ => match g.below(4) {
                0 => [42, c, a, g.below(2), 0],
                1 => [43, c, a, 0, 0],
                2 => [44, c, 0, 0, 0],
                _ => [45, c, a, 0, 0],
            },
        };
        ops.push(op);
    }
    ops
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let cfg = gen_scene(g, data);
    let n = cfg[3];
    (cfg, gen_ops(g, n))
}

// ---- the Rust run ---------------------------------------------------------------------------------------------------------------------------------------------------

fn fold(h: u32, x: u32) -> u32 {
    h.wrapping_mul(31).wrapping_add(x)
}

/// The words of a container's state, as the driver's `ctPutObject` writes them: the `state_words` of the module, with the fire points folded.
fn put_object(v: &mut Vec<u8>, w: &mut World, id: ObjectId, has_contain: bool) {
    if has_contain {
        let o = w.objects.find(id).unwrap();
        let idx = o.contain.expect("the container");
        let u = o.modules[idx as usize].update_index.expect("an update module");
        let Some(UpdateBody::OpenContain(m)) = o.behaviors[u as usize].body.as_ref() else {
            panic!("the container is in its slot");
        };
        put_u32(v, 1);
        put_words_state(v, m);
    } else {
        put_u32(v, 0);
    }
    let o = w.objects.find(id).unwrap();
    put_u32(v, o.contained_by as u32);
    let pos = o.get_position();
    put_u32(v, pos.x.to_bits());
    put_u32(v, pos.y.to_bits());
    put_u32(v, pos.z.to_bits());
    let (mut w0, mut w1) = (0u32, 0u32);
    for i in 0..OBJECT_STATUS_COUNT.min(64) {
        if o.status.test(i) {
            if i < 32 {
                w0 |= 1 << i;
            } else {
                w1 |= 1 << (i - 32);
            }
        }
    }
    put_u32(v, w0);
    put_u32(v, w1);
    put_u32(v, u32::from(o.is_effectively_dead()));
    let dead = o.ai.and_then(|a| match o.modules[a as usize].body.as_ref() {
        Some(UpdateBody::ModuleProbe(p)) => Some(p.answers.ai_dead),
        _ => None,
    });
    put_u32(v, u32::from(dead.unwrap_or(false)));
    // the physics block's `getAllowToFall()` (2: no physics)
    let fall = o.physics.and_then(|p| match o.modules[p as usize].body.as_ref() {
        Some(UpdateBody::ModuleProbe(p)) => Some(u32::from(p.answers.can_fall)),
        _ => None,
    });
    put_u32(v, fall.unwrap_or(2));
    // the geometry's bounding circle radius, the orientation and the layer the container reads
    put_u32(v, o.geometry_info.get_bounding_circle_radius().to_bits());
    put_u32(v, o.thing.orientation().to_bits());
    put_u32(v, o.layer as u32);
    // the model conditions the client reads give (four words) and whether the object is selected
    let flags = w.reads.drawable_model_condition_flags(id);
    for word in 0..4 {
        let mut m = 0u32;
        for b in 0..32 {
            let i = word * 32 + b;
            if i < MODELCONDITION_COUNT && flags.test(i) {
                m |= 1 << b;
            }
        }
        put_u32(v, m);
    }
    put_u32(v, u32::from(w.reads.local_selection.contains(&id)));
}

/// The state words of the module with the 32 fire point matrices folded into one word (the driver folds the same way).
fn put_words_state(v: &mut Vec<u8>, m: &OpenContain) {
    for x in m.state_words() {
        put_u32(v, x);
    }
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
            b.truncate(8192);
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

fn hex3(p: &Coord3D) -> String {
    format!("{:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())
}

/// The lines of the effect log the trace and the records of one op make, in order.
fn effect_lines(trace: Vec<String>, cuts: Vec<CutCall>, events: Vec<ObjectClientEvent>, tally: &RefCell<Tally>) -> Vec<String> {
    let mut log = Vec::new();
    let (mut cut_iter, mut ev_iter) = (cuts.into_iter(), events.into_iter());
    for t in trace {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Contain(c)) => {
                    tally.borrow_mut().cuts += 1;
                    match c {
                        ContainCall::PartitionRegisterObject { object } => log.push(format!("pregister {object}")),
                        ContainCall::PartitionUnregisterObject { object } => log.push(format!("punregister {object}")),
                        ContainCall::PartitionGetDistanceSquared { object, position } => log.push(format!("cdist {object} {}", hex3(&position))),
                        ContainCall::DrawableSetHidden { object, hidden } => log.push(format!("hidden {object} {}", i32::from(hidden))),
                        ContainCall::PathfindAddObject { object } => log.push(format!("pathadd {object}")),
                        ContainCall::PathfindRemoveObject { object } => log.push(format!("pathremove {object}")),
                        ContainCall::PathfindRemoveUnit { object } => log.push(format!("pathremoveunit {object}")),
                        ContainCall::PathfindUpdatePos { object, position } => log.push(format!("pathupdatepos {object} {}", hex3(&position))),
                        ContainCall::PathfindUpdateGoal { object, position, layer } => log.push(format!("pathupdategoal {object} {} {layer}", hex3(&position))),
                        ContainCall::PathfindAdjustToPossibleDestination { object, before } => log.push(format!("pathadjust {object} {}", hex3(&before))),
                        ContainCall::TerrainGetLayerForDestination { position } => log.push(format!("destlayer {}", hex3(&position))),
                        ContainCall::LeaveGroup { object } => log.push(format!("leavegroup {object}")),
                        ContainCall::CheckAndDetonateBoobyTrap { container, rider, result } => log.push(format!("booby {container} {rider} {}", i32::from(result))),
                        ContainCall::OnContainedBy { rider, container } => log.push(format!("oncontainedby {rider} {container}")),
                        ContainCall::OnRemovedFrom { rider, container } => log.push(format!("onremovedfrom {rider} {container}")),
                        ContainCall::Kill { object, amount } => {
                            // `Object::kill`: unresistable damage of the body's maximum health, `kill` true, to the body (a line of its own)
                            let mut info = DamageInfo::default();
                            info.input.damage_type = DAMAGE_UNRESISTABLE;
                            info.input.death_type = DEATH_NORMAL;
                            info.input.source_id = INVALID_ID;
                            info.input.amount = amount;
                            info.input.kill = true;
                            log.push(format!("bodydamage {object} {}", info_line(&info)));
                        }
                        ContainCall::AttemptDamage { object, info, has_body } => {
                            if has_body {
                                log.push(format!("bodydamage {object} {}", info_line(&info)));
                            }
                        }
                        ContainCall::SetPosition { .. } => {}
                        ContainCall::SetOrientation { object, angle } => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                        ContainCall::SetLayer { object, layer } => log.push(format!("setlayer {object} {layer}")),
                        ContainCall::SetTransformMatrix { object, matrix } => {
                            log.push(format!("settransform {object}"));
                            let mut s = format!("xform {object}");
                            for row in matrix.row.iter() {
                                s.push_str(&format!(" {:08x} {:08x} {:08x} {:08x}", row.x.to_bits(), row.y.to_bits(), row.z.to_bits(), row.w.to_bits()));
                            }
                            log.push(s);
                        }
                        ContainCall::DestroyObject { object } => log.push(format!("destroy {object}")),
                        ContainCall::GetSingleLogicalBonePosition { object, name } => log.push(format!("bone1 {object} {}", String::from_utf8_lossy(&name))),
                        ContainCall::GetMultiLogicalBonePosition { object, prefix, max } => log.push(format!("bonen {object} {} {max}", String::from_utf8_lossy(&prefix))),
                        ContainCall::GetSingleLogicalBonePositionOnTurret { object, name } => log.push(format!("boneturret {object} {}", String::from_utf8_lossy(&name))),
                    }
                }
                Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", x.to_bits(), y.to_bits())),
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)));
                }
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. }) => log.push(format!("clearset {} {}", clear[0], set[0])),
                _ => {}
            }
        } else if let Some(line) = t.strip_prefix("bt ") {
            // a probe's line; the idle order carries its command source, which the C++ stand-in does not print
            let tok: Vec<&str> = line.split(' ').collect();
            if tok[0] == "aiidlefrom" {
                log.push(format!("aiidle {}", tok[1]));
            } else if tok[0] == "pfall" {
                // the physics block's flag is part of the object's state: no line of its own
            } else {
                log.push(line.to_string());
            }
        }
    }
    log
}

#[derive(Default)]
struct Tally {
    cuts: u64,
    adds: u64,
    removes: u64,
    exits: u64,
    kills: u64,
    deaths: u64,
    xfers: u64,
    updates: u64,
    collisions: u64,
    effects: u64,
    /// reads of `list::end()` by `processDamageToContained`, compared as NULL
    end_reads: u64,
}

fn veterancy_of(k: usize) -> rts_lockstep_engine::common::game_common::VeterancyLevel {
    use rts_lockstep_engine::common::game_common::VeterancyLevel as V;
    // never REGULAR: `getVeterancyLevelFlag` shifts by -1 for it (undefined upstream, see `death_flags`)
    [V::Veteran, V::Elite, V::Heroic][k % 3]
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    {
        let ModuleDataKind::OpenContain(d) = &data else { unreachable!("not an OpenContain") };
        put_u32(&mut v, d.contain_max as u32);
        put_str(&mut v, &d.enter_sound.event_name);
        put_str(&mut v, &d.exit_sound.event_name);
        put_u32(&mut v, u32::from(d.passengers_allowed_to_fire));
        put_u32(&mut v, u32::from(d.passengers_in_turret));
        put_u32(&mut v, d.number_of_exit_paths as u32);
        put_u32(&mut v, d.damage_percentage_to_units.to_bits());
        put_u32(&mut v, u32::from(d.is_burned_death_to_units));
        put_u32(&mut v, d.door_open_time);
        let (mut am, mut fm) = (0u32, 0u32);
        for (i, k) in KINDS.iter().enumerate() {
            if d.allow_inside_kind_of.test(*k) {
                am |= 1 << i;
            }
            if d.forbid_inside_kind_of.test(*k) {
                fm |= 1 << i;
            }
        }
        put_u32(&mut v, am);
        put_u32(&mut v, fm);
        put_u32(&mut v, u32::from(d.weapon_bonus_passed_to_passengers));
        put_u32(&mut v, u32::from(d.allow_allies_inside) | u32::from(d.allow_enemies_inside) << 1 | u32::from(d.allow_neutral_inside) << 2);
        put_u32(&mut v, d.die_mux_data.death_types);
        put_u32(&mut v, d.die_mux_data.veterancy_levels);
        let (mut ex, mut rq) = (0u32, 0u32);
        for (i, st) in STATUSES.iter().enumerate() {
            if d.die_mux_data.exempt_status.test(*st) {
                ex |= 1 << i;
            }
            if d.die_mux_data.required_status.test(*st) {
                rq |= 1 << i;
            }
        }
        put_u32(&mut v, ex);
        put_u32(&mut v, rq);
    }
    let sc = parse_scene(cfg);
    let n = sc.objs.len();
    {
        // L9CT_SHOW=<k> prints the scene and the ops of the k-th case that reaches the Rust run (a debugging aid)
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let k = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let want = std::env::var("L9CT_SHOW").ok();
        if want.as_deref() == Some("all") || want.and_then(|s| s.parse::<usize>().ok()) == Some(k) {
            eprintln!("SHOW run {k}");
            for (i, o) in sc.objs.iter().enumerate() {
                eprintln!("SHOW obj {i}: {o:?}");
            }
            eprintln!("SHOW ops {ops:?}");
        }
    }
    put_u32(&mut v, n as u32);

    let mut w = World::new();
    w.names = ctx.names;
    let mut templates = Vec::new();
    for (k, o) in sc.objs.iter().enumerate() {
        let kind_of: Vec<i32> = (0..8).filter(|&i| o.kinds >> i & 1 != 0).map(|i| KINDS[i]).collect();
        let slots = [ThingTemplateAudioType::SoundEnter, ThingTemplateAudioType::SoundExit, ThingTemplateAudioType::SoundFalling];
        let audio = (0..3).filter(|&i| o.snd[i] > 0).map(|i| (slots[i], format!("snd{}", o.snd[i]).into_bytes())).collect();
        let geometry = Some(GeometryInfo::new(GeometryType::Sphere, false, 1.0, o.radius, o.radius));
        templates.push(Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: format!("T{}", k + 1).into_bytes(), kind_of, audio, geometry, ..Default::default() })));
    }
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    // the cut points
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.contain.stub_object_calls = true;
    for i in 0..8 {
        w.cuts.terrain_layer[i] = sc.layer[i];
        w.cuts.terrain_layer_height[i] = sc.layer_height[i];
    }
    for i in 0..n {
        for j in 0..n {
            let rel = match sc.rel[i][j] {
                0 => Relationship::Enemies,
                1 => Relationship::Neutral,
                _ => Relationship::Allies,
            };
            w.cuts.team_relationships.insert((i as u32 + 1, j as u32 + 1), rel);
        }
    }
    for (k, o) in sc.objs.iter().enumerate() {
        if o.player >= 0 {
            w.cuts.team_players.insert(k as u32 + 1, o.player);
        }
    }
    w.set_frame(sc.frame);
    // the scene's tables
    let apply_tables = |w: &mut World, k: usize, o: &Obj| {
        let id = k as i32 + 1;
        w.cuts.contain.booby_trapped.remove(&id);
        if o.booby {
            w.cuts.contain.booby_trapped.insert(id);
        }
        if o.adj_has {
            w.cuts.contain.adjusted_destination.insert(id, Coord3D { x: o.adj[0], y: o.adj[1], z: o.adj[2] });
        }
        let mut fps = Vec::new();
        for j in 0..(o.fp_count as usize).min(FP) {
            let mut m = Matrix3D::identity();
            m.set_translation(&Vector3::new(o.fp[j][0], o.fp[j][1], o.fp[j][2]));
            fps.push(m);
        }
        if !fps.is_empty() {
            w.cuts.contain.fire_points.insert(id, fps);
        }
        for j in 0..8 {
            if o.bone_has[j] {
                w.cuts.contain.single_bones.insert((id, BONES[j].as_bytes().to_vec()), Coord3D { x: o.bone[j][0], y: o.bone[j][1], z: o.bone[j][2] });
            }
        }
        for j in 0..FP {
            if o.turret_has[j] {
                let mut m = Matrix3D::identity();
                m.set_translation(&Vector3::new(o.turret[j][0], o.turret[j][1], o.turret[j][2]));
                w.cuts.contain.turret_bones.insert((id, format!("FIREPOINT{:02}", j + 1).into_bytes()), m);
            }
        }
    };
    // the objects
    for (k, o) in sc.objs.iter().enumerate() {
        let mut modules = Vec::new();
        if o.has_body || o.has_ai || o.has_stealth {
            let mut a = ProbeAnswers { idle: o.ai_idle, enter_target: o.enter_target as i32, ..Default::default() };
            a.can_fall = o.can_fall;
            let mut pm = NewModule::plain(b"CtProbe", b"ModuleTag_Probe");
            pm.interfaces = if o.has_body { MODULEINTERFACE_BODY } else { 0 };
            pm.is_ai = o.has_ai;
            pm.is_stealth = o.has_stealth;
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, if o.has_body { o.max_health } else { 0.0 }, Default::default())));
            modules.push(pm);
        }
        if o.has_phys {
            let a = ProbeAnswers { mass: o.mass, can_fall: o.can_fall, ..Default::default() };
            let mut pm = NewModule::plain(b"PhysicsBehavior", b"ModuleTag_Physics");
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, 0.0, Default::default())));
            modules.push(pm);
        }
        if o.has_contain {
            let cd = if k == 0 {
                data.clone()
            } else {
                let mut d = OpenContainModuleData { contain_max: o.contain_max, passengers_allowed_to_fire: o.allow_fire, passengers_in_turret: o.in_turret, number_of_exit_paths: o.exit_paths, damage_percentage_to_units: o.damage_pct, is_burned_death_to_units: o.burned, door_open_time: o.door_open, weapon_bonus_passed_to_passengers: o.weapon_pass, allow_allies_inside: o.allow_rel & 1 != 0, allow_enemies_inside: o.allow_rel & 2 != 0, allow_neutral_inside: o.allow_rel & 4 != 0, ..Default::default() };
                if o.enter_snd > 0 {
                    d.enter_sound.event_name = format!("snd{}", o.enter_snd).into_bytes();
                }
                if o.exit_snd > 0 {
                    d.exit_sound.event_name = format!("snd{}", o.exit_snd).into_bytes();
                }
                if o.allow_mode != 0 {
                    d.allow_inside_kind_of = KindOfMaskType::new();
                    for i in 0..8 {
                        if o.allow_mask >> i & 1 != 0 {
                            d.allow_inside_kind_of.set(KINDS[i], 1);
                        }
                    }
                }
                for i in 0..8 {
                    if o.forbid_mask >> i & 1 != 0 {
                        d.forbid_inside_kind_of.set(KINDS[i], 1);
                    }
                }
                d.die_mux_data.death_types = o.death_mask;
                d.die_mux_data.veterancy_levels = o.vet_mask;
                for i in 0..3 {
                    if o.exempt_mask >> i & 1 != 0 {
                        d.die_mux_data.exempt_status.set(STATUSES[i], 1);
                    }
                    if o.required_mask >> i & 1 != 0 {
                        d.die_mux_data.required_status.set(STATUSES[i], 1);
                    }
                }
                ModuleDataKind::OpenContain(Arc::new(d))
            };
            let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"OpenContain", b"ModuleTag_Contain", &cd).unwrap().unwrap();
            modules.push(m);
        }
        let mut status = Default::default();
        for (b, st) in STATUSES.iter().enumerate() {
            if o.status >> b & 1 != 0 {
                rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::set(&mut status, *st, 1);
            }
        }
        let id = w.new_object(NewObject { template: templates[k].clone(), status, team: Some(k as u32 + 1), modules }).unwrap();
        assert_eq!(id, k as i32 + 1);
        w.contain_set_position(id, &Coord3D { x: o.pos[0], y: o.pos[1], z: o.pos[2] });
        w.set_orientation(id, o.orient);
        w.friend_bind_to_drawable(id, if o.drawable { 7 + k as i32 } else { INVALID_DRAWABLE_ID });
        let ob = w.objects.find_mut(id).unwrap();
        ob.experience.level = veterancy_of(k);
        ob.layer = o.layer as i32;
        ob.weapon_bonus_condition = o.weapon_bonus;
        ob.contained_by = INVALID_ID;
        apply_tables(&mut w, k, o);
        if o.selected && o.drawable {
            w.reads.local_selection.push(id);
        }
        let mut flags = ModelConditionFlags::new();
        for i in 0..6 {
            if o.cond >> i & 1 != 0 {
                flags.set(CONDS[i], 1);
            }
        }
        w.reads.drawable_model_condition.insert(id, flags);
    }
    // the objects that start contained: their container lists them
    // the random generators start after the objects are made (the object creation may draw)
    w.random.init_random(sc.client_seed);
    w.random.init_game_logic_random(sc.logic_seed);
    let ids: Vec<ObjectId> = (1..=n as i32).collect();
    for (k, o) in sc.objs.iter().enumerate() {
        w.objects.find_mut(k as i32 + 1).unwrap().contained_by = o.contained_by as i32;
        w.set_effectively_dead(k as i32 + 1, o.eff_dead);
        if o.contained_by != 0 {
            let cont = o.contained_by as i32;
            if let Some(c) = w.get_contain(cont) {
                w.module_call(cont, c, ModuleCall::AddToContainList(k as i32 + 1));
            }
        }
    }
    let _ = &ids;
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    let mut end_reads = 0u64;
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut result) = (-2i32, Vec::new(), 0u32, 0u32);
        let mut load_status = 0;
        if phase > 0 {
            let o = &ops[phase - 1];
            let id = (o[1] % n as u32) as i32 + 1;
            let rider = (o[2] % n as u32) as i32 + 1;
            let m = w.get_contain(id);
            let mut info = DamageInfo::default();
            if o[0] == 4 || o[0] == 6 {
                info.input.source_id = (o[3] % 5) as i32;
                info.input.damage_type = (o[2] % 8) as i32;
                info.input.death_type = (o[4] % 5 + 1) as i32;
                info.input.amount = 10.0;
            }
            let fold3 = |p: &Coord3D| p.x.to_bits().wrapping_mul(31 * 31).wrapping_add(p.y.to_bits().wrapping_mul(31)).wrapping_add(p.z.to_bits());
            if let Some(m) = m.filter(|_| o[0] < 34 || (42..=45).contains(&o[0])) {
                match o[0] {
                    0 => {
                        w.module_call(id, m, ModuleCall::AddToContain(rider));
                        tally.borrow_mut().adds += 1;
                    }
                    1 => {
                        w.module_call(id, m, ModuleCall::RemoveFromContainExposing { obj: rider, expose_stealth_units: o[3] != 0 });
                        tally.borrow_mut().removes += 1;
                    }
                    2 => {
                        w.module_call(id, m, ModuleCall::RemoveAllContained { expose_stealth_units: o[2] != 0 });
                    }
                    3 => {
                        w.module_call(id, m, ModuleCall::KillAllContained);
                        tally.borrow_mut().kills += 1;
                    }
                    4 => {
                        if let ModuleReply::Damage(d) = w.module_call(id, m, ModuleCall::HarmAndForceExitAllContained(info)) {
                            result = d.input.amount.to_bits();
                        }
                    }
                    5 => {
                        w.module_call(id, m, ModuleCall::OnCollide { other: rider, loc: None, normal: None });
                        tally.borrow_mut().collisions += 1;
                    }
                    6 => {
                        // whether the die mux of the container's data takes this death (the module's own check, asked again)
                        {
                            let ob = w.objects.find(id).unwrap();
                            let u = ob.modules[m as usize].update_index.expect("an update module");
                            if let Some(UpdateBody::OpenContain(c)) = ob.behaviors[u as usize].body.as_ref() {
                                result = u32::from(c.module_data().die_mux_data.is_die_applicable(info.input.death_type, ob.get_veterancy_level() as i32, &ob.status));
                            }
                        }
                        w.module_call(id, m, ModuleCall::OnDie(info));
                        tally.borrow_mut().deaths += 1;
                    }
                    7 => {
                        let u = w.update_handle_of_module(ModuleHandle { object: id, index: m }).expect("an update module");
                        result = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                        tally.borrow_mut().updates += 1;
                    }
                    8 => result = u32::from(w.module_call(id, m, ModuleCall::IsValidContainerFor { obj: rider, check_capacity: o[3] != 0 }).as_bool()),
                    9 => {
                        w.module_call(id, m, ModuleCall::ExitObjectViaDoor { obj: rider, door: 0 });
                        tally.borrow_mut().exits += 1;
                    }
                    10 => {
                        w.module_call(id, m, ModuleCall::ExitObjectInAHurry(rider));
                        tally.borrow_mut().exits += 1;
                    }
                    11 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToExit { command_source: 2, instantly: o[2] != 0 });
                    }
                    12 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToIdleFrom { command_source: 2 });
                    }
                    13 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToHackInternetFrom { command_source: 2 });
                    }
                    14 => {
                        w.module_call(id, m, ModuleCall::MarkAllPassengersDetected);
                    }
                    15 => {
                        w.module_call(id, m, ModuleCall::ProcessDamageToContained(f(o[2])));
                    }
                    16 => {
                        w.module_call(id, m, ModuleCall::OnObjectWantsToEnterOrExit { obj: rider, wants: (o[3] % 3) as i32 });
                    }
                    17 => {
                        w.module_call(id, m, ModuleCall::SetRallyPoint(Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) }));
                    }
                    18 => {
                        result = match w.module_call(id, m, ModuleCall::GetRallyPoint) {
                            ModuleReply::Coord(p) => fold3(&p),
                            _ => 0xdead_0000,
                        };
                    }
                    19 => {
                        if let ModuleReply::Coord(p) = w.module_call(id, m, ModuleCall::GetNaturalRallyPoint) {
                            result = fold3(&p);
                        }
                    }
                    20 => result = w.module_call(id, m, ModuleCall::GetClosestRider(Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) })).as_id() as u32,
                    21 => result = u32::from(w.module_call(id, m, ModuleCall::IsAnyRiderAttacking).as_bool()),
                    22 => {
                        w.module_call(id, m, ModuleCall::ContainReactToTransformChange);
                    }
                    23 => {
                        w.module_call(id, m, ModuleCall::OnSelling);
                    }
                    24 => {
                        w.module_call(id, m, ModuleCall::SetPassengerAllowedToFire(o[2] != 0));
                    }
                    25 => result = u32::from(w.module_call(id, m, ModuleCall::IsPassengerAllowedToFire(INVALID_ID)).as_bool()),
                    26 => {
                        let int = |r: ModuleReply| if let ModuleReply::Int(i) = r { i as u32 } else { 0xdead_0001 };
                        let boo = |r: ModuleReply| u32::from(r.as_bool());
                        let mut h = w.module_call(id, m, ModuleCall::GetContainedItemsMass).as_real(f32::NAN).to_bits();
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetStealthUnitsContained)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetPlayerWhoEntered)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetContainCount)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsWeaponBonusPassedToPassengers)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetWeaponBonusPassedToPassengers)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetContainMax)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsEnclosingContainerFor(rider))));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsDisplayedOnControlBar)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetExtraSlotsInUse)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsKickOutOnCapture)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::HasObjectsWantingToEnterOrExit)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsExitBusy)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsImmuneToClearBuildingAttacks)));
                        result = h;
                    }
                    27 => {
                        w.module_call(id, m, ModuleCall::ScatterToNearbyPosition(rider));
                    }
                    28 => {
                        w.module_call(id, m, ModuleCall::EnableLoadSounds(o[2] != 0));
                    }
                    29 => {
                        w.module_call(id, m, ModuleCall::OnDelete);
                    }
                    30 => {
                        let (st, bytes, _) = run_xfer(&mut w, id, m as usize, 0, &[]);
                        xst = st;
                        saved = bytes.clone();
                        xbytes = bytes;
                        tally.borrow_mut().xfers += 1;
                    }
                    31 => {
                        let (st, bytes, _) = run_xfer(&mut w, id, m as usize, 1, &saved);
                        xst = st;
                        xbytes = bytes;
                        tally.borrow_mut().xfers += 1;
                        // the objects a load found in a list that was not empty are destroyed in the middle of the xfer upstream
                        w.module_call(id, m, ModuleCall::DrainXferDestroys);
                    }
                    32 => {
                        let (st, bytes, c) = run_xfer(&mut w, id, m as usize, 2, &[]);
                        xst = st;
                        xbytes = bytes;
                        crc = c;
                        tally.borrow_mut().xfers += 1;
                    }
                    33 => {
                        let before = w.cuts.module_errors.len();
                        w.module_call(id, m, ModuleCall::LoadPostProcess);
                        // `throw SC_INVALID_DATA` is not upstream-undefined
                        let thrown = w.cuts.module_errors[before..].iter().any(|(_, e)| !e.is_undefined());
                        if thrown {
                            load_status = rts_lockstep_engine::common::xfer::XferStatus::ListNotEmpty as i32;
                            w.cuts.module_errors.truncate(before);
                        }
                        xst = load_status;
                    }
                    42 => {
                        w.module_call(id, m, ModuleCall::OnContaining { obj: rider, was_selected: o[3] != 0 });
                    }
                    43 => {
                        w.module_call(id, m, ModuleCall::OnRemoving(rider));
                    }
                    44 => {
                        w.module_call(id, m, ModuleCall::RedeployOccupants);
                    }
                    45 => {
                        w.module_call(id, m, ModuleCall::PutObjAtNextFirePoint(rider));
                    }
                    _ => {}
                }
            }
            // the environment
            match o[0] {
                34 => w.set_frame(o[2]),
                35 => {
                    let ob = w.objects.find(id).unwrap();
                    let has_draw = ob.drawable != INVALID_DRAWABLE_ID;
                    w.reads.local_selection.retain(|&x| x != id);
                    if o[2] != 0 && has_draw {
                        w.reads.local_selection.push(id);
                    }
                }
                36 => {
                    let mut flags = ModelConditionFlags::new();
                    for i in 0..6 {
                        if o[2] >> i & 1 != 0 {
                            flags.set(CONDS[i], 1);
                        }
                    }
                    w.reads.drawable_model_condition.insert(id, flags);
                }
                37 => {
                    let ob = w.objects.find_mut(id).unwrap();
                    ob.status = Default::default();
                    for (b, st) in STATUSES.iter().enumerate() {
                        if o[2] >> b & 1 != 0 {
                            ob.status.set(*st, 1);
                        }
                    }
                }
                38 => w.contain_set_position(id, &Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) }),
                39 => w.set_effectively_dead(id, o[2] != 0),
                40 => {
                    if o[2] != 0 {
                        w.cuts.contain.booby_trapped.insert(id);
                    } else {
                        w.cuts.contain.booby_trapped.remove(&id);
                    }
                }
                41 => {
                    let ob = w.objects.find_mut(id).unwrap();
                    if let Some(ai) = ob.ai {
                        if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[ai as usize].body.as_mut() {
                            p.answers.idle = o[2] & 1 != 0;
                            p.answers.enter_target = if o[2] & 2 != 0 { rider } else { 0 };
                        }
                    }
                }
                _ => {}
            }
        }
        let new_errors = w.cuts.module_errors[errors_before..].to_vec();
        // the end-iterator read of `processDamageToContained` is compared as the shipped game reads it (NULL: the loop ends), the C++ edited to say so
        end_reads += new_errors.iter().filter(|(_, e)| e.is_end_iterator_dereferenced()).count() as u64;
        if new_errors.iter().any(|(_, e)| e.is_undefined() && !e.is_end_iterator_dereferenced()) {
            excluded = true;
            break;
        }
        // the effects of the op, in order
        let trace = w.cuts.trace.take().unwrap_or_default();
        let cuts = w.cuts.log.take().unwrap_or_default();
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        events_before = w.events.events().len();
        let _ = events_before;
        let log = effect_lines(trace, cuts, events, tally);
        tally.borrow_mut().effects += log.len() as u64;
        put_i32(&mut v, 0);
        put_u32(&mut v, result);
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
        for k in 0..n {
            put_object(&mut v, &mut w, k as i32 + 1, sc.objs[k].has_contain);
        }
    }
    tally.borrow_mut().end_reads += end_reads;
    RunOut { dump: v, keys: Vec::new(), excluded }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let max = r.u32() as i32;
    let (en, ex) = (r.str(), r.str());
    let data: Vec<u32> = (0..14).map(|_| r.u32()).collect();
    out.push(format!("data {max} {en} {ex} {data:08x?}"));
    let n_obj = r.u32();
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let result = r.u32();
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let xfer = r.xfer();
        out.push(format!("op {i}: status {st} result {result:08x} log {log:?} {xfer}"));
        for k in 0..n_obj {
            let has = r.u32();
            let mut state: Vec<u32> = Vec::new();
            if has != 0 {
                let size = r.u32();
                let len = r.u32();
                state.push(size);
                state.push(len);
                for _ in 0..len {
                    state.push(r.u32());
                }
                // the entered mask, the two sound frames, the stealth count, the door countdown, the four condition words, the fire point hash, the three indexes,
                // the art flag, the rally point (3 + exists), then the enter-exit map, the exit path and two flags
                for _ in 0..(5 + 4 + 1 + 3 + 1 + 4) {
                    state.push(r.u32());
                }
                let map = r.u32();
                state.push(map);
                for _ in 0..map * 2 {
                    state.push(r.u32());
                }
                for _ in 0..3 {
                    state.push(r.u32());
                }
            }
            let rest: Vec<u32> = (0..17).map(|_| r.u32()).collect();
            out.push(format!("op {i} object {k}: contain {state:08x?} container {} at {:08x} {:08x} {:08x} status {:08x} {:08x} dead {} aidead {} fall {} radius {:08x} orient {:08x} layer {} conds {:08x?} selected {}", rest[0], rest[1], rest[2], rest[3], rest[4], rest[5], rest[6], rest[7], rest[8], rest[9], rest[10], rest[11], &rest[12..16], rest[16]));
        }
    }
    out
}

#[test]
fn open_contain_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "contain module R1",
        seed: 0x00C0_0A17,
        cases_var: "L9CT_CASES",
        ini_stem: "Ct",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9ct_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "contain module R1: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} adds, {} removes, {} exits, {} kill-alls, {} deaths, {} collisions, {} updates, {} xfers, {} effect lines, {} cut points, {} end-iterator reads",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.adds, t.removes, t.exits, t.kills, t.deaths, t.collisions, t.updates, t.xfers, t.effects, t.cuts, t.end_reads
    );
    assert!(c.cases > 0);
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::OpenContain(Arc::new(OpenContainModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}
