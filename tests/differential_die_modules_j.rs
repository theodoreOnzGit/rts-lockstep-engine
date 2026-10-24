#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/FXListDie.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FXListDie` (`logic::modules::fx_list_die`, over `modules::die_body` and `modules::upgrade_mux_user`) against the unchanged C++, `p1w11-sonnet`,
//! die batch J.
//!
//! # Method
//!
//! The C++ side is `FXListDie.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dies_j.inc`) on the module build's stand-ins:
//! an `Object` with a controlling player, completed upgrades and a position, three players with completed upgrades, a damage dealer found by its id,
//! `FXList::doFXObj` (the list, then through `g_fxObjHook` the two objects) and `FXList::doFXPos` (the list and the position), and the REAL `DieModule`,
//! `UpgradeMux`, `UpgradeCenter`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the loaded upgrade center, two objects (the one with the module from `modules::new_module`, and the
//! damage dealer), every op a `World::module_call` or an xfer through `World::xfer_module_slot`. The constructor's effects (`giveSelfUpgrade()` when
//! the module starts active) are compared too.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the four die mux fields, `StartsActive`, `DeathFX`, `OrientToObject` and the upgrade mux's five;
//! mostly well formed) and, when the read succeeds, runs 1 to 14 ops.
//!
//! Compared, bit for bit: the read's throw; the data (both muxes, the list, the two flags); the constructor's effects; per op the throw status, the
//! answer of an upgrade call, `isAlreadyUpgraded`, the effects in order (`fx<id>`, `fxobj <primary> <secondary>`, `fxpos <id> <position>`, `remove <name>`),
//! the xfer status, bytes and CRC; and the name keys after the ops.
//!
//! # Deviations the test cannot see
//!
//! An upgrade name that is not an upgrade, in a constructor that starts active (a removal name) or an `onDie` (a mux name), is `INI_INVALID_DATA` thrown upstream:
//! the constructor's throw cannot be compared (the module is never made), so such cases are never sent to the C++ and counted as excluded; an `onDie`'s
//! throw is a status and is compared. An empty or `None` name that finds no upgrade is a `NULL` template upstream (`Undefined`): excluded as well.
//!
//! # Results
//!
//! (2026-10-21, 9000 cases, `L9FJ_CASES` overrides.) 8,826 data reads, 139,328 dump lines (63,161 ops: 16,706 `onDie` calls, 881 upgrades fired, 5,858 xfers)
//! and 8,463 name-key checks compared, 0 mismatches; 174 inputs excluded (undefined upstream, or a constructor that throws); 363 data blocks threw on
//! both sides. Effects: 6,610 `fx`, 3,111 `fxobj`, 388 `fxpos`, 1,414 `remove`.
//!
//! # Upstream facts kept
//!
//! - `StartsActive` defaults to TRUE ("Patch 1.02 -- Craptacular HACK -- should default to FALSE but only ONE case sets it false out of 847!"), so the
//!   constructor's `giveSelfUpgrade()` runs for nearly every `FXListDie` and the module cannot be upgraded again.
//! - `FXListDie::xfer` does not save the mux's executed flag.
//! - The object is given its completed upgrades after the module is made: the constructor's removals (`RemovesUpgrades`) would take them away, in
//!   the C++ driver's stand-in `Object::removeUpgrade` and in `World::remove_upgrade` alike, and a new object has none.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 871 | the mux never marks the upgrade executed (`upgrade_mux_user`) |
//! | 881 | `onDie` plays whether or not the upgrade is active |
//! | 882 | the object's own conflicting upgrades are ignored |
//! | 883 | the player's conflicting upgrades are ignored |
//! | 884 | `OrientToObject` is inverted |
//! | 885 | the damage dealer is never passed |
//! | 886 | `isDieApplicable` is ignored |
//! | 887 | the constructor does not give the upgrade to a module that starts active |
//!
//! All 8 are caught (3000 cases each). Plant 871 is a mismatch from the first case (`isAlreadyUpgraded` after the constructor) and then ends in a crash of
//! the test process (exit 139), in the C++ (`UpgradeMuxData::getUpgradeActivationMasks` under `FXListDie::onDie`, seen in gdb): with the plant the
//! Rust `onDie` returns early (the upgrade never became active), so it no longer sees an input as undefined upstream (an empty or `None` upgrade name:
//! a `NULL` template) and sends it to the C++, which cannot run it. That is a detection, not a clean mismatch; it is recorded as it happened, and
//! the same plant is a clean mismatch in `differential_update_modules_i.rs`.

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::fx_list_die::FXListDieModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9fxdie_j.bin");
const XFER_FILE: &str = "l9fxdie_j.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
/// The team of both objects.
const OBJECT_TEAM: u32 = 100;

#[derive(Default)]
struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    fn id(&mut self, name: &Option<Vec<u8>>) -> i32 {
        match name {
            None => 0,
            Some(n) => {
                let next = self.0.len() as i32 + 1;
                *self.0.entry(n.clone()).or_insert(next)
            }
        }
    }
}

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
fn put_names(v: &mut Vec<u8>, names: &[Vec<u8>]) {
    put_u32(v, names.len() as u32);
    for n in names {
        put_str(v, n);
    }
}
fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
}
fn put_mux(v: &mut Vec<u8>, m: &UpgradeMuxData, fx: &mut FxIds) {
    put_names(v, &m.trigger_upgrade_names);
    put_names(v, &m.activation_upgrade_names);
    put_names(v, &m.conflicting_upgrade_names);
    put_names(v, &m.removal_upgrade_names);
    put_i32(v, fx.id(&m.fx_list_upgrade));
    put_words(v, m.activation_mask().words());
    put_words(v, m.conflicting_mask().words());
    v.push(u8::from(m.requires_all_triggers));
}
fn put_die_mux(v: &mut Vec<u8>, m: &DieMuxData) {
    put_u32(v, m.death_types);
    put_u32(v, m.veterancy_levels);
    put_words(v, m.exempt_status.words());
    put_words(v, m.required_status.words());
}

fn status_from(w: &[u32]) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    for i in 0..OBJECT_STATUS_COUNT {
        if w[(i / 32) as usize] >> (i % 32) & 1 != 0 {
            m.set(i, 1);
        }
    }
    m
}

fn mask_from(w: &[u32]) -> UpgradeMaskType {
    let mut m = UpgradeMaskType::default();
    for i in 0..128 {
        if w[i / 32] >> (i % 32) & 1 != 0 {
            m.set(i as i32, 1);
        }
    }
    m
}

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FXListDie(Arc::new(FXListDieModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9fj_run`: the die mux, the upgrade mux, the list, the two flags.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::FXListDie(d) = data else { unreachable!("not an FXListDie") };
    put_die_mux(v, &d.die.die_mux_data);
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_i32(v, fx.id(&d.default_death_fx));
    put_u32(v, u32::from(d.orient_to_object));
    put_u32(v, u32::from(d.initially_active));
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
    let u64_ = |p: &mut usize| -> u64 {
        let v = u64::from_le_bytes(b[*p..*p + 8].try_into().unwrap());
        *p += 8;
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
    let (dt, vl) = (u32_(&mut p), u32_(&mut p));
    let ex: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let rq: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    out.push(format!("die mux {dt:#x} {vl:#x} exempt {ex:x?} required {rq:x?}"));
    for what in ["trigger", "activation", "conflicting", "removal"] {
        let n = u32_(&mut p);
        let names: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        out.push(format!("mux {what} {names:?}"));
    }
    let fx = u32_(&mut p) as i32;
    let act: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let con: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let all = b[p];
    p += 1;
    out.push(format!("mux fx {fx} act {act:x?} con {con:x?} all {all}"));
    let (death_fx, orient, active) = (u32_(&mut p) as i32, u32_(&mut p), u32_(&mut p));
    out.push(format!("data death fx {death_fx} orient {orient} active {active}"));
    let n = u32_(&mut p);
    let ctor: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    out.push(format!("constructor {ctor:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let up = u32_(&mut p);
        let nlog = u32_(&mut p);
        let log: Vec<String> = (0..nlog).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        out.push(format!("op {i}: status {st} result {res} upgraded {up} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
    }
    out
}

// -- generators --

const DEATH_NAMES: [&str; 20] = [
    "NONE", "CRUSHED", "BURNED", "EXPLODED", "POISONED", "TOPPLED", "FLOODED", "SUICIDED", "LASERED", "DETONATED", "SPLATTED", "POISONED_BETA",
    "EXTRA_2", "EXTRA_3", "EXTRA_4", "EXTRA_5", "EXTRA_6", "EXTRA_7", "EXTRA_8", "POISONED_GAMMA",
];

fn death_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        t.push(match g.below(10) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=6 => format!("+{}", DEATH_NAMES[1 + g.below(19) as usize]),
            _ => format!("-{}", DEATH_NAMES[1 + g.below(19) as usize]),
        });
    }
    t.join(" ")
}

fn level_tokens(g: &mut Gen) -> String {
    let names = ["VETERAN", "ELITE", "HEROIC"];
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        t.push(match g.below(8) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=4 => format!("+{}", names[g.below(3) as usize]),
            _ => format!("-{}", names[g.below(3) as usize]),
        });
    }
    t.join(" ")
}

fn status_tokens(g: &mut Gen) -> String {
    // a list is all plain names or all signed ones (a mix is refused by the INI reader; the junk lines of an unclean file cover refusals)
    let names = rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_NAMES;
    let mode = g.below(5);
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        let n = names[1 + g.below(44) as usize];
        t.push(match mode {
            0 => format!("-{n}"),
            1 => format!("+{n}"),
            _ => n.to_string(),
        });
    }
    if mode == 4 && g.below(3) == 0 {
        return "NONE".to_string();
    }
    t.join(" ")
}

fn gen_name(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    if clean && !upgrades.is_empty() {
        return upgrades[g.below(upgrades.len() as u32) as usize].clone();
    }
    match g.below(100) {
        0..=64 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].clone(),
        65..=74 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].to_lowercase(),
        75..=76 => format!("Unknown{}", g.below(3)),
        77 => "Up9".to_string(),
        78..=79 => pick(g, &["None", "none", "NONE"]).to_string(),
        _ if !upgrades.is_empty() => upgrades[0].clone(),
        _ => format!("Unknown{}", g.below(3)),
    }
}

fn gen_list(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let n = 1 + g.below(3);
    (0..n).map(|_| gen_name(g, upgrades, clean)).collect::<Vec<_>>().join(" ")
}


fn gen_text(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = death_tokens(g);
    field(g, 30, "DeathTypes", v);
    let v = level_tokens(g);
    field(g, 20, "VeterancyLevels", v);
    let v = status_tokens(g);
    field(g, 12, "ExemptStatus", v);
    let v = status_tokens(g);
    field(g, 12, "RequiredStatus", v);
    // (inactive often: it starts active unless told otherwise, and an active module cannot be upgraded again)
    let v = if g.below(100) < 55 { "No".to_string() } else { gen_bool(g, clean) };
    field(g, 85, "StartsActive", v);
    let v = pick(g, &["FX_Die", "FX_Other", "None", "fx_die"]).to_string();
    field(g, 80, "DeathFX", v);
    let v = gen_bool(g, clean);
    field(g, 40, "OrientToObject", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 75, "TriggeredBy", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 20, "ConflictsWith", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 20, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None"]).to_string();
    field(g, 25, "FXListUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 20, "RequiresAllTriggers", v);
    if !clean && g.below(25) == 0 {
        lines.push(match g.below(3) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "VeterancyLevels = WHAT".to_string(),
            _ => "NoSuchField = 1".to_string(),
        });
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(30) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn gen_mask(g: &mut Gen, n_up: usize, at_least: u32) -> [u32; 4] {
    let mut w = [0u32; 4];
    if n_up > 0 {
        // a third of the masks name most of the upgrades, so that a TriggeredBy list is met more often than by chance
        let extra = if g.below(3) == 0 { g.below(n_up as u32 + 1) } else { g.below(3) };
        for _ in 0..at_least + extra {
            let b = g.below(n_up as u32) as usize;
            w[b / 32] |= 1 << (b % 32);
        }
    }
    w
}

fn f32_bits(g: &mut Gen, range: f32) -> u32 {
    g.uniform(-range, range).to_bits()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    fired: u64,
    died: u64,
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


const PROBE: [&str; 9] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SCIENCE_Pathfinder", "zz_fresh", "FXListDie"];

/// The effect lines of one stretch of the trace, in the driver's format.
fn effects(trace: Vec<String>, events: Vec<ObjectClientEvent>, fx: &mut FxIds) -> Vec<String> {
    let mut log = Vec::new();
    let mut ev = events.into_iter();
    for t in trace {
        if t.starts_with("evt ObjectCreated") || t.starts_with("evt ObjectDestroyed") {
            // (the client's own events, not `Object`'s)
        } else if t.starts_with("evt ") {
            match ev.next() {
                Some(ObjectClientEvent::FxListDoFxObj { fx: name, object_id }) => {
                    log.push(format!("fx{}", fx.id(&Some(name))));
                    log.push(format!("fxobj {object_id} -1"));
                }
                Some(ObjectClientEvent::FxListDoFxObjSecondary { fx: name, object_id, secondary }) => {
                    log.push(format!("fx{}", fx.id(&Some(name))));
                    log.push(format!("fxobj {object_id} {}", if secondary == INVALID_ID { -1 } else { secondary }));
                }
                Some(ObjectClientEvent::FxListDoFxPos { fx: name, position }) => {
                    log.push(format!("fxpos {} {},{},{}", fx.id(&Some(name)), position.x.to_bits(), position.y.to_bits(), position.z.to_bits()));
                }
                _ => {}
            }
        } else if let Some(n) = t.strip_prefix("remove ") {
            log.push(format!("remove {n}"));
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump (after the data dump), the name keys afterwards and the world; `None` when the constructor
/// recorded an error (never sent to the C++).
fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32; 8], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, Vec<u32>, World, bool) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut w = World::new();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.teams.insert_team(Team::new(OBJECT_TEAM, 0)).unwrap();
    if cfg[0] != NO_PLAYER {
        w.cuts.team_players.insert(OBJECT_TEAM, cfg[0] as i32);
    }
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"FXListDie", b"ModuleTag_Die", &data).unwrap().unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    // made under construction (as batches C and I do): `initObject` runs the upgrade pass of a finished object, which resolves the mux's names; the C++
    // driver makes the object with no such pass
    let mut status = ObjectStatusMaskType::default();
    status.set(rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let events_before = w.events.events().len();
    let me = w.new_object(NewObject { template: template.clone(), status, team: Some(OBJECT_TEAM), modules: vec![m] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    {
        let o = w.objects.find_mut(me).unwrap();
        o.status.set(rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&cfg[4..8]);
    }
    let constructor_events = new_events(&w, events_before);
    let constructor_threw = !w.cuts.module_errors.is_empty();
    if cfg[1] != 0 {
        let dealer = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: Some(OBJECT_TEAM), modules: Vec::new() }).unwrap();
        assert_eq!(dealer, 2);
    }
    w.set_position(me, &Coord3D { x: 11.0, y: 22.0, z: 5.0 });
    let idx = w.find_module(me, b"FXListDie").unwrap();
    let ctor = effects(w.cuts.trace.take().unwrap(), constructor_events, &mut fx);
    tally.effects += ctor.len() as u64;
    put_u32(&mut v, ctor.len() as u32);
    for l in &ctor {
        put_str(&mut v, l.as_bytes());
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let mask = mask_from(&o[1..5]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                let mut info = DamageInfo::default();
                info.input.death_type = o[1] as _;
                info.input.source_id = o[5] as _;
                {
                    let ob = w.objects.find_mut(me).unwrap();
                    ob.experience.level = match o[2] % 4 {
                        0 => rts_lockstep_engine::common::game_common::VeterancyLevel::Regular,
                        1 => rts_lockstep_engine::common::game_common::VeterancyLevel::Veteran,
                        2 => rts_lockstep_engine::common::game_common::VeterancyLevel::Elite,
                        _ => rts_lockstep_engine::common::game_common::VeterancyLevel::Heroic,
                    };
                    ob.status = status_from(&o[3..5]);
                }
                w.module_call(me, idx, ModuleCall::OnDie(info));
                tally.died += 1;
            }
            1 => {
                res = u32::from(w.module_call(me, idx, ModuleCall::AttemptUpgrade(mask)).as_bool());
                tally.fired += u64::from(res);
            }
            2 => res = u32::from(w.module_call(me, idx, ModuleCall::WouldUpgrade(mask)).as_bool()),
            3 => res = u32::from(w.module_call(me, idx, ModuleCall::ResetUpgrade(mask)).as_bool()),
            4 => res = u32::from(w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded).as_bool()),
            5 => {
                w.module_call(me, idx, ModuleCall::ForceRefreshUpgrade);
            }
            6 => w.objects.find_mut(me).unwrap().object_upgrades_completed = mask,
            7 => {
                for p in 0..3 {
                    w.cuts.player_upgrades.insert(p, mask);
                }
            }
            8 => {
                if o[1] == 0 {
                    w.cuts.team_players.remove(&OBJECT_TEAM);
                } else {
                    w.cuts.team_players.insert(OBJECT_TEAM, o[1] as i32 - 1);
                }
            }
            9..=11 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 9, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 9 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            12 => w.set_position(me, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, if threw { 0 } else { res });
        put_u32(&mut v, u32::from(w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded).as_bool()));
        let log = effects(w.cuts.trace.take().unwrap(), new_events(&w, events_before), &mut fx);
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
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w, constructor_threw)
}

/// The `Object` client events from event number `before` on.
fn new_events(w: &World, before: usize) -> Vec<ObjectClientEvent> {
    w.events.events()[before..]
        .iter()
        .filter_map(|e| match e {
            rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) => Some(e.clone()),
            _ => None,
        })
        .collect()
}

fn cases() -> usize {
    std::env::var("L9FJ_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn fx_list_die_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_FA51);
    let table = blocks();
    let mut read = Check::new("die module J data read (throw, message)");
    let mut run = Check::new("die module J ops (data, constructor, effects, xfers)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
        let mut ctx = Ctx::new(init_names, init_center);
        Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        cpp_reset(init_names, init_center);
        assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

        let clean = g.below(10) < 7;
        let text = gen_text(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Uj{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's player, has a damage dealer, unused, unused, the object's completed upgrades (four words)]
        let mut cfg = [0u32; 8];
        cfg[0] = if g.below(6) == 0 { NO_PLAYER } else { g.below(3) };
        cfg[1] = u32::from(g.below(3) != 0);
        let m0 = gen_mask(&mut g, n_up, 0);
        cfg[4..8].copy_from_slice(&m0);
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(54) {
                0..=13 => {
                    // onDie: the death type, a veterancy level, status words, a damage source: the dealer (object 2), nobody, or an id that is no object
                    let source = match g.below(5) {
                        0 => 0,
                        1 => 9000 + g.below(10),
                        _ => 2,
                    };
                    [0, 1 + g.below(20), 1 + g.below(3), g.u32() & 0x3fff, g.u32() & 0xff, source]
                }
                14..=25 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [1, m[0], m[1], m[2], m[3], 0]
                }
                26..=28 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [2, m[0], m[1], m[2], m[3], 0]
                }
                29..=31 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [3, m[0], m[1], m[2], m[3], 0]
                }
                32 => [4, 0, 0, 0, 0, 0],
                33..=34 => [5, 0, 0, 0, 0, 0],
                35..=38 => {
                    let m = gen_mask(&mut g, n_up, 0);
                    [6, m[0], m[1], m[2], m[3], 0]
                }
                39..=42 => {
                    let m = gen_mask(&mut g, n_up, 0);
                    [7, m[0], m[1], m[2], m[3], 0]
                }
                43..=44 => [8, g.below(4), 0, 0, 0, 0],
                45..=46 => [9, 0, 0, 0, 0, 0],
                47..=48 => [10, 0, 0, 0, 0, 0],
                49 => [11, 0, 0, 0, 0, 0],
                _ => [12, f32_bits(&mut g, 100.0), f32_bits(&mut g, 100.0), f32_bits(&mut g, 20.0), 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour (or a constructor that throws) is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w, ctor_threw) = rust_run(ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if ctor_threw || w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some((rd, rk, w))
        } else {
            None
        };
        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9fj_run(
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
            _ => panic!("C++ threw something unexpected (status {status}) -- a constructor that threw upstream but not here"),
        };
        let what = || format!("case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, _w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["fx", "fxobj", "fxpos", "remove "] {
                *counts.entry(key.trim().to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        let ck: Vec<u32> = PROBE
            .iter()
            .map(|n| {
                let c = CString::new(*n).unwrap();
                unsafe { cpp::ref_up_name_key(c.as_ptr()) }
            })
            .collect();
        keys.eq(rk, ck, what);
        ops_run += ops.len() as u64;
    }
    println!(
        "die module J: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream or a constructor that throws), {ops_run} ops, \
         {} onDie calls, {} upgrades fired, {} xfers, {} effect lines",
        tally.died, tally.fired, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    keys.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.fired * 20 > cases as u64, "the generator must reach every path");
}
