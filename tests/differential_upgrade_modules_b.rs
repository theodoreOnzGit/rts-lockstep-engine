// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/{StatusBitsUpgrade, LocomotorSetUpgrade, PassengersFireUpgrade,
//   StealthUpgrade, SubObjectsUpgrade, ObjectCreationUpgrade}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! Six more layer-9 upgrade modules (`logic::modules::{status_bits_upgrade, locomotor_set_upgrade, passengers_fire_upgrade, stealth_upgrade,
//! sub_objects_upgrade, object_creation_upgrade}`) against the unchanged C++, `p1w10-sonnet`, upgrade batch B. The pattern is
//! `differential_upgrade_modules.rs`'s; read its header first.
//!
//! # Method
//!
//! The C++ side is the six `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_upgrades_b.inc`) on the module
//! build's stand-ins (`differential/shim_module`): an `Object` with the AI, contain and spawn behaviour the case gives it, a drawable that logs
//! the sub-objects it is told to show and hide, the thing's `KINDOF_SPAWNS_ARE_THE_WEAPONS`, and the object creation lists `OCL_A` and `OCL_B`
//! (any other name and `None` are `NULL`, as `INI::parseObjectCreationList` answers); the contain module is the real `ContainModuleInterface` with
//! every pure virtual answering "no" (`setPassengerAllowedToFire` logs); the REAL `UpgradeCenter`, `INI` and `NameKeyGenerator`.
//!
//! Each case defines 0 to 8 upgrades on both sides, picks a module kind, generates a `D` block of the `UpgradeMux` fields and the module's own
//! (mostly well-formed, sometimes with unknown names, bad separators, junk lines or no `End`), reads it on both sides, and when the read succeeds
//! gives the module an object with (or without) a controlling player, random completed upgrades on each, and runs 1 to 12 ops: `wouldUpgrade`,
//! `attemptUpgrade`, `resetUpgrade`, `forceRefreshUpgrade`, `isAlreadyUpgraded`, changes of the object's and the player's completed upgrades, and a
//! reset of the object's status bits.
//!
//! The Rust side is the real thing: a `World` whose `statics.upgrade_center` and `names` are the loaded store and generator, one object made by
//! `World::new_object` with the module from `modules::new_module` (and an AI, a contain and a spawn behaviour that are scripted `ProbeModule`s when the
//! case has them), and every op sent through `World::module_call`.
//!
//! Compared, bit for bit: the read's throw (status and message); the data (the mux, the module's fields); per op the throw status, the result,
//! `isAlreadyUpgraded`, the effects in the order they happened (`locoupg`, `pfire`, `stealthslaves`, `subshow`, `subupdate`, `ocl`, FX and removals),
//! and the object's status bits (`setStatus` and `clearStatus` are `World::set_status`, the verified `Object::setStatus`); after the ops, the key
//! `nameToKey` gives to a probe list of names.
//!
//! Inputs for which upstream is undefined (`SubObjectsUpgrade` on an object with no controlling player; a mux list naming `None`) are never sent
//! to the C++: the Rust returns an `is_undefined()` error and the case is counted as excluded.
//!
//! # Results (2026-10-21, 9000 cases, `L9UPB_CASES` overrides)
//!
//! 8854 data reads, 110,657 op dumps (results, effects, state) and 8372 key probes compared, 0 mismatches (146 inputs excluded as undefined
//! upstream; 1556 `attemptUpgrade` calls fired; effects: 934 removals, 604 `subshow`, 397 FX, 203 `locoupg`, 198 `pfire`, 174 `subupdate`, 111
//! `stealthslaves`, 66 `ocl`). The module also dumps `isSubObjectsUpgrade()` (a `protected` virtual, reached through a derived probe class).
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000. Every plant but the two noted is caught.
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 601 | StatusBits | the status cleared before it is set |
//! | 602-603 | StatusBits | the clear / the set skipped |
//! | 604 | StatusBits | a non-empty default `StatusToSet` |
//! | 605 | StatusBits | the set called with `false` |
//! | 606 | StatusBits | `StatusToClear` parsed into `StatusToSet` |
//! | 607-608 | LocomotorSet | the AI guard forced on / forced off |
//! | 609-610 | PassengersFire | the contain guard forced on / forced off |
//! | 611-612 | Stealth | `CAN_STEALTH` not set / `STEALTHED` set instead |
//! | 613-616 | Stealth | the `KINDOF_SPAWNS_ARE_THE_WEAPONS` guard forced, the spawn-behaviour guard forced, the grant skipped, `&&` for `||` |
//! | 617-618 | SubObjects | the object's / the player's conflicting-upgrade early return dropped |
//! | 619 | SubObjects | conflicting mask taken as the activation mask: caught by a crash (see below) |
//! | 620-622 | SubObjects | the drawable guard forced on, the show and hide lists swapped (two ways) |
//! | 623-624 | SubObjects | `updateSubObjects` forced / suppressed |
//! | 625 | SubObjects | `isSubObjectsUpgrade()` false (observable only since the probe above) |
//! | 626-627 | SubObjects | `ShowSubObjects` replaces instead of appends / `HideSubObjects` parsed into the show list |
//! | 628 | (unused id) | no plant placed |
//! | 629 | ObjectCreation | an empty OCL name emitted as a cut call when none is named: EQUIVALENT, an unknown name is no list and the cut contract treats it as a no-op |
//! | 630 | ObjectCreation | `upgradeImplementation` does nothing |
//! | 631 | ObjectCreation | the OCL name upper-cased (the lookup is case sensitive) |
//!
//! Plant 619 is detected by the C++ reference segfaulting (exit 139): with the plant the Rust no longer reports the input as undefined, so
//! the case reaches the C++ side that upstream cannot run. That is a detection, not a clean mismatch; it is recorded as it happened.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_SPAWNS_ARE_THE_WEAPONS;
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::object_creation_upgrade::ObjectCreationUpgradeModuleData;
use rts_lockstep_engine::logic::modules::status_bits_upgrade::StatusBitsUpgradeModuleData;
use rts_lockstep_engine::logic::modules::sub_objects_upgrade::SubObjectsUpgradeModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The class names of the kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 6] = ["StatusBitsUpgrade", "LocomotorSetUpgrade", "PassengersFireUpgrade", "StealthUpgrade", "SubObjectsUpgrade", "ObjectCreationUpgrade"];

// -- the dump, in ref_l9upb_run's format --

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
fn put_u64(v: &mut Vec<u8>, x: u64) {
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
/// A status mask as the indices of its set bits.
fn put_status(v: &mut Vec<u8>, m: &ObjectStatusMaskType) {
    let bits: Vec<i32> = (0..OBJECT_STATUS_COUNT).filter(|&i| m.test(i)).collect();
    put_u32(v, bits.len() as u32);
    for b in bits {
        put_i32(v, b);
    }
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

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    let kind = out.0;
    out.1 = Some(match kind {
        0 => ModuleDataKind::StatusBitsUpgrade(Arc::new(StatusBitsUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        1 => ModuleDataKind::LocomotorSetUpgrade(Arc::new(
            rts_lockstep_engine::logic::modules::locomotor_set_upgrade::LocomotorSetUpgradeModuleData::friend_new_module_data(Some(ini))?,
        )),
        2 => ModuleDataKind::PassengersFireUpgrade(Arc::new(
            rts_lockstep_engine::logic::modules::passengers_fire_upgrade::PassengersFireUpgradeModuleData::friend_new_module_data(Some(ini))?,
        )),
        3 => ModuleDataKind::StealthUpgrade(Arc::new(rts_lockstep_engine::logic::modules::stealth_upgrade::StealthUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        4 => ModuleDataKind::SubObjectsUpgrade(Arc::new(SubObjectsUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        _ => ModuleDataKind::ObjectCreationUpgrade(Arc::new(ObjectCreationUpgradeModuleData::friend_new_module_data(Some(ini))?)),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9upb_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    match data {
        ModuleDataKind::StatusBitsUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_status(v, &d.status_to_set);
            put_status(v, &d.status_to_clear);
        }
        ModuleDataKind::LocomotorSetUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::PassengersFireUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::StealthUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::SubObjectsUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_names(v, &d.show_sub_object_names);
            put_names(v, &d.hide_sub_object_names);
        }
        ModuleDataKind::ObjectCreationUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            // the fixture knows OCL_A and OCL_B (the case matters): the list a name finds
            put_i32(v, match d.ocl.as_deref() {
                Some(b"OCL_A") => 1,
                Some(b"OCL_B") => 2,
                _ => 0,
            });
        }
        _ => unreachable!("not an upgrade module"),
    }
}

/// A dump as text, for the comparison.
fn decode(kind: usize, b: &[u8], n_ops: usize) -> Vec<String> {
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
    let status_ = |p: &mut usize| -> Vec<i32> {
        let n = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap()) as usize;
        *p += 4;
        (0..n)
            .map(|_| {
                let v = i32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
                *p += 4;
                v
            })
            .collect()
    };
    if b.is_empty() {
        return out;
    }
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
    match kind {
        0 => {
            let set = status_(&mut p);
            let clear = status_(&mut p);
            out.push(format!("statuses set {set:?} clear {clear:?}"));
        }
        4 => {
            let n = u32_(&mut p);
            let show: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
            let n = u32_(&mut p);
            let hide: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
            out.push(format!("subobjects show {show:?} hide {hide:?}"));
            out.push(format!("isSubObjectsUpgrade {}", b[p]));
            p += 1;
        }
        5 => out.push(format!("ocl {}", u32_(&mut p) as i32)),
        _ => {}
    }
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = b[p];
        let up = b[p + 1];
        p += 2;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let status = status_(&mut p);
        out.push(format!("op {i}: status {st} res {res} upgraded {up} log {log:?} object status {status:?}"));
    }
    out
}

// -- generators --

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

const STATUS_NAMES: [&str; 12] = [
    "DESTROYED",
    "CAN_ATTACK",
    "UNDER_CONSTRUCTION",
    "UNSELECTABLE",
    "NO_COLLISIONS",
    "NO_ATTACK",
    "AIRBORNE_TARGET",
    "PARACHUTING",
    "CAN_STEALTH",
    "STEALTHED",
    "IS_FIRING_WEAPON",
    "REPULSOR",
];

fn gen_status_list(g: &mut Gen, clean: bool) -> String {
    match g.below(8) {
        0 => "NONE".to_string(),
        1 if !clean => pick(g, &["Bogus", "+STEALTHED", "STEALTHED +CAN_STEALTH", "", "-CAN_STEALTH", "OBJECT_STATUS_STEALTHED"]).to_string(),
        2 => format!("+{}", pick(g, &STATUS_NAMES)),
        3 => format!("-{}", pick(g, &STATUS_NAMES)),
        _ => (0..1 + g.below(3)).map(|_| pick(g, &STATUS_NAMES).to_string()).collect::<Vec<_>>().join(" "),
    }
}

/// The module's own fields.
fn own_fields(g: &mut Gen, kind: usize, clean: bool) -> Vec<(u32, &'static str, String)> {
    match kind {
        0 => {
            let mut v = vec![(80, "StatusToSet", gen_status_list(g, clean))];
            v.push((70, "StatusToClear", gen_status_list(g, clean)));
            v
        }
        4 => {
            let names = |g: &mut Gen| (0..1 + g.below(3)).map(|_| pick(g, &["Door", "Wing", "Gun_01", "Antenna"]).to_string()).collect::<Vec<_>>().join(" ");
            let show = names(g);
            let hide = names(g);
            vec![(70, "ShowSubObjects", show), (70, "HideSubObjects", hide), (25, "ShowSubObjects", names(g))]
        }
        5 => vec![(85, "UpgradeObject", pick(g, &["OCL_A", "OCL_B", "ocl_a", "None", "none", "OCL_C", ""]).to_string())],
        _ => vec![],
    }
}

fn gen_data(g: &mut Gen, kind: usize, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_list(g, upgrades, clean);
    field(g, 85, "TriggeredBy", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 30, "ConflictsWith", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 30, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None"]).to_string();
    field(g, 35, "FXListUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 30, "RequiresAllTriggers", v);
    for (p, name, value) in own_fields(g, kind, clean) {
        field(g, p, name, value);
    }
    if !clean && g.below(25) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
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

// -- the Rust side --

#[derive(Default)]
struct Tally {
    fired: u64,
    effects: u64,
}

fn probe(has: u32) -> UpdateBody {
    UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { has, ..Default::default() }, 0.0, UpgradeMaskType::default()))
}

/// Runs the ops on a fresh world and returns the dump (after the data dump).
#[allow(clippy::too_many_arguments)]
fn rust_run(
    kind: usize,
    ctx: Ctx,
    data: ModuleDataKind,
    has_player: bool,
    player_mask: [u32; 4],
    obj_mask: [u32; 4],
    cfg: [u32; 5],
    ops: &[[u32; 5]],
    tally: &mut Tally,
) -> (Vec<u8>, Vec<u32>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut w = World::new();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, ..Default::default() });
    w.names = ctx.names;
    w.cuts.neutral_default_team = 1;
    w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team::new(1, 0)).unwrap();
    if has_player {
        w.cuts.team_players.insert(1, 0);
        w.cuts.player_upgrades.insert(0, mask_from(&player_mask));
    }
    w.cuts.trace = Some(Vec::new());
    let m = rts_lockstep_engine::logic::modules::new_module(0, CLASSES[kind].as_bytes(), b"ModuleTag_Up", &data).unwrap().unwrap();
    let mut modules = vec![m];
    if cfg[1] != 0 {
        let mut ai = NewModule::plain(b"AIUpdate", b"ModuleTag_Ai");
        ai.is_ai = true;
        ai.body = Some(probe(0));
        modules.push(ai);
    }
    if cfg[2] != 0 {
        let mut contain = NewModule::plain(b"OpenContain", b"ModuleTag_Contain");
        contain.interfaces = MODULEINTERFACE_CONTAIN;
        contain.body = Some(probe(0));
        modules.push(contain);
    }
    if cfg[3] != 0 {
        let mut spawn = NewModule::plain(b"SpawnBehavior", b"ModuleTag_Spawn");
        spawn.body = Some(probe(1 << 3));
        modules.push(spawn);
    }
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let mut values = ObjectTemplateValues::default();
    if cfg[4] != 0 {
        values.kind_of.push(KINDOF_SPAWNS_ARE_THE_WEAPONS);
    }
    let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&values)), status, team: None, modules }).unwrap();
    {
        let o = w.objects.find_mut(id).unwrap();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&obj_mask);
        o.drawable = if cfg[0] != 0 { 1 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
    }
    let idx = w.find_module(id, CLASSES[kind].as_bytes()).unwrap();
    if kind == 4 {
        v.push(u8::from(w.module_call(id, idx, ModuleCall::IsSubObjectsUpgrade).as_bool()));
    }
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        let res = match o[0] {
            0 => w.module_call(id, idx, ModuleCall::WouldUpgrade(mask)).as_bool(),
            1 => {
                let r = w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask)).as_bool();
                tally.fired += u64::from(r);
                r
            }
            2 => w.module_call(id, idx, ModuleCall::ResetUpgrade(mask)).as_bool(),
            3 => {
                w.module_call(id, idx, ModuleCall::ForceRefreshUpgrade);
                false
            }
            4 => w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool(),
            5 => {
                w.objects.find_mut(id).unwrap().object_upgrades_completed = mask;
                false
            }
            6 => {
                if has_player {
                    w.cuts.player_upgrades.insert(0, mask);
                }
                false
            }
            _ => {
                let bits = u64::from(o[1]) | (u64::from(o[2]) << 32);
                let ob = w.objects.find_mut(id).unwrap();
                ob.status = ObjectStatusMaskType::default();
                for b in 0..64.min(OBJECT_STATUS_COUNT) {
                    if bits >> b & 1 != 0 {
                        ob.status.set(b, 1);
                    }
                }
                false
            }
        };
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        v.push(u8::from(res && !threw));
        v.push(u8::from(w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool()));
        // the effects, in order: the trace has each removal, cut call, client event and trace line of a module
        let fxs: Vec<Vec<u8>> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(
                    rts_lockstep_engine::logic::world::object_events::ObjectClientEvent::FxListDoFxObj { fx, .. },
                ) => Some(fx.clone()),
                _ => None,
            })
            .collect();
        let mut fx_iter = fxs.into_iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("evt FxListDoFxObj") {
                log.push(format!("fx{}", fx.id(&fx_iter.next())));
            } else if let Some(n) = t.strip_prefix("remove ") {
                log.push(format!("remove {n}"));
            } else if t.starts_with("cut AiSetLocomotorUpgrade") {
                log.push("locoupg 1".into());
            } else if t.starts_with("cut ContainSetPassengerAllowedToFire") {
                log.push("pfire 1".into());
            } else if t.starts_with("cut SpawnGiveSlavesStealthUpgrade") {
                log.push("stealthslaves 1".into());
            } else if let Some(rest) = t.strip_prefix("evt DrawableShowSubObject") {
                // `{ object_id: N, name: [bytes], show: true }`
                let name: Vec<u8> = rest
                    .split("name: [")
                    .nth(1)
                    .and_then(|s| s.split(']').next())
                    .map(|s| s.split(", ").filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect())
                    .unwrap_or_default();
                let show = rest.contains("show: true");
                log.push(format!("subshow {} {}", String::from_utf8_lossy(&name), i32::from(show)));
            } else if t.starts_with("evt DrawableUpdateSubObjects") {
                log.push("subupdate".into());
            } else if let Some(rest) = t.strip_prefix("cut OclCreate") {
                let name: Vec<u8> = rest
                    .split("ocl: [")
                    .nth(1)
                    .and_then(|s| s.split(']').next())
                    .map(|s| s.split(", ").filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect())
                    .unwrap_or_default();
                // the list is found by name when it is used: the fixture has OCL_A and OCL_B, any other name finds no list (nothing is made)
                match name.as_slice() {
                    b"OCL_A" => log.push("ocl 0".into()),
                    b"OCL_B" => log.push("ocl 1".into()),
                    _ => {}
                }
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        let ob = w.objects.find(id).unwrap();
        put_status(&mut v, &ob.status);
    }
    let _ = put_u64;
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w)
}

const PROBE: [&str; 9] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SCIENCE_Pathfinder", "Sci_X", "zz_fresh"];

fn cases() -> usize {
    std::env::var("L9UPB_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn upgrade_modules_b_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_B0B5);
    let table = blocks();
    let mut read = Check::new("upgrade module B data read (throw, message)");
    let mut run = Check::new("upgrade module B ops (data, results, effects, state)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 6];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 6;
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
        let text = gen_data(&mut g, kind, &upgrades, clean);
        let name = format!("Data\\INI\\Um{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let has_player = g.below(5) != 0;
        let player_mask = gen_mask(&mut g, n_up, 0);
        let obj_mask = gen_mask(&mut g, n_up, 0);
        let cfg = [u32::from(g.below(4) != 0), u32::from(g.below(3) != 0), u32::from(g.below(3) != 0), u32::from(g.below(3) != 0), u32::from(g.below(2) != 0)];
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| {
                let op = match g.below(24) {
                    0..=2 => 0,
                    3..=10 => 1,
                    11..=12 => 2,
                    13 => 3,
                    14 => 4,
                    15..=16 => 5,
                    17 => 6,
                    _ => 7,
                };
                if op == 7 {
                    // the status bits to start from
                    [7, g.u32(), g.u32() & 0xff, 0, 0]
                } else {
                    let m = gen_mask(&mut g, n_up, 1);
                    [op, m[0], m[1], m[2], m[3]]
                }
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w) = rust_run(kind, ctx, data.expect("read ok"), has_player, player_mask, obj_mask, cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some((rd, rk, w))
        } else {
            None
        };

        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 20];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9upb_run(
                kind as i32,
                cfile.as_ptr(),
                i32::from(has_player),
                player_mask.as_ptr(),
                obj_mask.as_ptr(),
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
        let what = || format!("case {case} ({}): upgrades {upgrades:?} player {has_player} {player_mask:?} obj {obj_mask:?} cfg {cfg:?}, file {:?}, ops {ops:?}", CLASSES[kind], show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["locoupg", "pfire", "stealthslaves", "subshow", "subupdate", "ocl 0", "ocl 1", "remove", "fx"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) player {has_player} cfg {cfg:?} file {:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
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
        per_kind[kind] += 1;
        let _ = w;
    }
    println!(
        "upgrade modules B: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} upgrades fired, {} effect lines",
        tally.fired, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    keys.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.fired * 6 > cases as u64 && tally.effects * 4 > cases as u64, "the generator must reach every path");
}
