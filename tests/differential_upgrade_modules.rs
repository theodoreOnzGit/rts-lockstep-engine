// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/{WeaponSetUpgrade, WeaponBonusUpgrade, ArmorUpgrade,
//   ModelConditionUpgrade, ExperienceScalarUpgrade, ActiveShroudUpgrade, MaxHealthUpgrade, GrantScienceUpgrade}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The eight layer-9 upgrade modules of `p1l9-upgrades` (`logic::modules::{weapon_set_upgrade, weapon_bonus_upgrade, armor_upgrade,
//! model_condition_upgrade, experience_scalar_upgrade, active_shroud_upgrade, max_health_upgrade, grant_science_upgrade}`) against the
//! unchanged C++, `p1w10-sonnet`. The pattern is `differential_command_set_upgrade.rs`'s; read its header first.
//!
//! # Method
//!
//! The C++ side is the eight `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_upgrades.inc`) on the
//! module build's stand-ins (`differential/shim_module`: an `Object` with a body, a drawable, an experience tracker, weapon set flags, a
//! weapon bonus condition and a shroud range; a `Player` that logs the science it is granted; a `Drawable` that logs its model condition
//! and terrain decal) and the REAL `UpgradeCenter`, `INI` and `NameKeyGenerator`.
//!
//! Each case defines 0 to 8 upgrades on both sides (`Upgrade UpN` blocks, the same load; `Upgrade_AmericaChemicalSuits` is among them a
//! quarter of the time, the name `ArmorUpgrade` tests), picks a module kind, generates a `D` block of the `UpgradeMux` fields and the
//! module's own (mostly well-formed, sometimes with unknown names, bad separators, junk lines or no `End`), and reads it on both sides.
//! When the read succeeds it gives the module an object with (or without) a controlling player, random completed upgrades on each, a
//! body and a drawable or neither, and runs 1 to 12 ops: `wouldUpgrade`, `attemptUpgrade`, `resetUpgrade`, `forceRefreshUpgrade`,
//! `isAlreadyUpgraded`, changes of the object's and the player's completed upgrades, and a reset of the state the modules write (weapon set
//! flags and bonus condition, armor flags, experience scalar, shroud range, max health) to values of the op.
//!
//! The Rust side is the real thing: a `World` whose `statics.upgrade_center` and `names` are the loaded store and generator, one object
//! (team 1, whose player is 0 when there is one) made by `World::new_object` with the module from `modules::new_module` (and a scripted
//! body module when the case has a body), and every op sent through `World::module_call`.
//!
//! Compared, bit for bit: the read's throw (status and message); the data (the mux, its masks, the module's fields); per op the throw
//! status, the result, `isAlreadyUpgraded`, the effects in the order they happened (`wset`, `mc N`, `decal N`, `partition`,
//! `maxhealth B T`, `science N`, FX and removals), and the state the module wrote: the weapon set flags, the weapon bonus condition,
//! the body's armor flags, the experience scalar, the shroud range, the body's max health and the cached science; after the ops, the key
//! `nameToKey` gives to a probe list of names.
//!
//! The effects the Rust `World` reports are read from `world.cuts.trace` (the cut calls and client events, in order). Two of the
//! C++ stand-in's lines have no Rust call to read: `weaponBonusCondition` and the armor flag (the body module call) are compared as
//! state, so each op starts them cleared. `partition` is read from the `PartitionQueueUndoShroudReveal` cut that
//! `handlePartitionCellMaintenance` makes on an object that has looked, so the Rust object is made to have looked before each op.
//!
//! Inputs for which upstream is undefined (`ArmorUpgrade` for the chemical suit on an object with no drawable; a mux list naming
//! `None`) are never sent to the C++: the Rust returns an `is_undefined()` error, and the case is counted as excluded.
//!
//! # Results (2026-10-21, upstream `0a05454d`)
//!
//! `cargo test --release --features differential --test differential_upgrade_modules -- --nocapture`, 12,000 cases (1,019 to 1,450
//! compared per module): **11,872 data reads, 141,744 op dumps and 10,823 name-key probes compared, 0 mismatches.** 70,309 ops ran,
//! 2,016 upgrades fired, 3,079 effect lines were written, 1,049 data blocks threw (the same status and message on both sides) and 128
//! cases were excluded as undefined upstream. The comparison found no defect in the unverified drafts of `p1l9-upgrades`.
//!
//! # Planted bugs
//!
//! Each was applied to the Rust alone as a runtime switch (`L9_PLANT=<n>`, `logic::modules::plant`) and the test run at 3,000 cases;
//! `mismatches` is the number of differing op dumps (0 in the clean run). 22 switches; **21 caught, 1 equivalent**.
//!
//! | # | planted bug | mismatches |
//! |---|---|---|
//! | 1 | `WeaponSetUpgrade` sets `WEAPONSET_VETERAN`, not `WEAPONSET_PLAYER_UPGRADE` | 146 |
//! | 2 | `WeaponSetUpgrade` sets nothing | 146 |
//! | 3 | `WeaponBonusUpgrade` sets the next condition | 213 |
//! | 4 | `WeaponBonusUpgrade` sets nothing | 213 |
//! | 5 | `ArmorUpgrade` never sets the armor flag | 179 |
//! | 6 | `ArmorUpgrade` inverts the chemical-suit test | 80 |
//! | 7 | `ArmorUpgrade` sets the wrong terrain decal | 7 |
//! | 9 | `ModelConditionUpgrade` sets `MODELCONDITION_INVALID` too | 6 |
//! | 10 | `ModelConditionUpgrade` sets the next flag | 45 |
//! | 11 | `ExperienceScalarUpgrade` subtracts | 108 |
//! | 12 | `ExperienceScalarUpgrade` writes nothing | 108 |
//! | 13 | `ActiveShroudUpgrade` skips `handlePartitionCellMaintenance` | 80 |
//! | 14 | `ActiveShroudUpgrade` skips `setShroudRange` | 190 |
//! | 15 | `ActiveShroudUpgrade` doubles the range | 178 |
//! | 16 | `MaxHealthUpgrade` always passes `SAME_CURRENTHEALTH` | 17 |
//! | 17 | `MaxHealthUpgrade` subtracts | 78 |
//! | 18 | `MaxHealthUpgrade` skips `setMaxHealth` | 78 |
//! | 20 | `GrantScienceUpgrade` looks the science up on every attempt, not once | 0, **equivalent**: the lookup is the name key of a fixed name |
//! | 21 | `GrantScienceUpgrade` never grants | 67 |
//! | 23 | `ExperienceScalarUpgrade`'s `AddXPScalar` defaults to 1 | 79 |
//! | 24 | `MaxHealthUpgrade`'s `ChangeType` defaults to `PRESERVE_RATIO` | 140 |
//! | 25 | `ActiveShroudUpgrade`'s `NewShroudRange` defaults to 1 | 83 |
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use rts_lockstep_engine::common::thing_template::ThingTemplate;
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::active_shroud_upgrade::ActiveShroudUpgradeModuleData;
use rts_lockstep_engine::logic::modules::armor_upgrade::ArmorUpgradeModuleData;
use rts_lockstep_engine::logic::modules::experience_scalar_upgrade::ExperienceScalarUpgradeModuleData;
use rts_lockstep_engine::logic::modules::grant_science_upgrade::GrantScienceUpgradeModuleData;
use rts_lockstep_engine::logic::modules::max_health_upgrade::MaxHealthUpgradeModuleData;
use rts_lockstep_engine::logic::modules::model_condition_upgrade::ModelConditionUpgradeModuleData;
use rts_lockstep_engine::logic::modules::weapon_bonus_upgrade::WeaponBonusUpgradeModuleData;
use rts_lockstep_engine::logic::modules::weapon_set_upgrade::WeaponSetUpgradeModuleData;
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
const CLASSES: [&str; 8] = [
    "WeaponSetUpgrade",
    "WeaponBonusUpgrade",
    "ArmorUpgrade",
    "ModelConditionUpgrade",
    "ExperienceScalarUpgrade",
    "ActiveShroudUpgrade",
    "MaxHealthUpgrade",
    "GrantScienceUpgrade",
];

/// The upgrade name `ArmorUpgrade::upgradeImplementation` tests with `isTriggeredBy`.
const CHEMSUIT: &str = "Upgrade_AmericaChemicalSuits";

// -- the dump, in ref_l9up_run's format --

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
        0 => ModuleDataKind::WeaponSetUpgrade(Arc::new(WeaponSetUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        1 => ModuleDataKind::WeaponBonusUpgrade(Arc::new(WeaponBonusUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        2 => ModuleDataKind::ArmorUpgrade(Arc::new(ArmorUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        3 => ModuleDataKind::ModelConditionUpgrade(Arc::new(ModelConditionUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        4 => ModuleDataKind::ExperienceScalarUpgrade(Arc::new(ExperienceScalarUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        5 => ModuleDataKind::ActiveShroudUpgrade(Arc::new(ActiveShroudUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        6 => ModuleDataKind::MaxHealthUpgrade(Arc::new(MaxHealthUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        _ => ModuleDataKind::GrantScienceUpgrade(Arc::new(GrantScienceUpgradeModuleData::friend_new_module_data(Some(ini))?)),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9up_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    match data {
        ModuleDataKind::WeaponSetUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::WeaponBonusUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::ArmorUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::ModelConditionUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_i32(v, d.condition_flag);
        }
        ModuleDataKind::ExperienceScalarUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_u32(v, d.add_xp_scalar.to_bits());
        }
        ModuleDataKind::ActiveShroudUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_u32(v, d.new_shroud_range.to_bits());
        }
        ModuleDataKind::MaxHealthUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_u32(v, d.add_max_health.to_bits());
            put_i32(v, d.max_health_change_type);
        }
        ModuleDataKind::GrantScienceUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_str(v, &d.grant_science_name);
        }
        _ => unreachable!("not an upgrade module"),
    }
}


/// A dump as text, for the comparison: the mux and the module's fields, then per op its line.
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
        3 => out.push(format!("flag {}", u32_(&mut p) as i32)),
        4 | 5 => out.push(format!("real {:08x}", u32_(&mut p))),
        6 => {
            let a = u32_(&mut p);
            let t = u32_(&mut p) as i32;
            out.push(format!("addmax {a:08x} type {t}"));
        }
        7 => out.push(format!("science {:?}", str_(&mut p))),
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
        let (w0, w1) = (u64_(&mut p), u64_(&mut p));
        let (wbc, armor, xp, shroud, maxh) = (u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p));
        let sci = u32_(&mut p) as i32;
        out.push(format!(
            "op {i}: status {st} res {res} upgraded {up} log {log:?} wset {w0:x},{w1:x} wbc {wbc:x} armor {armor:x} xp {xp:08x} shroud {shroud:08x} maxh {maxh:08x} sci {sci}"
        ));
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

/// The module's own fields.
fn own_fields(g: &mut Gen, kind: usize, clean: bool) -> Vec<(u32, &'static str, String)> {
    match kind {
        3 => vec![(85, "ConditionFlag", pick(g, &["DAMAGED", "NIGHT", "SNOW", "GARRISONED", "ENEMYNEAR", "DOOR_1_OPENING", "WEAPONSET_PLAYER_UPGRADE", "None", "Bogus"]).to_string())],
        4 => vec![(85, "AddXPScalar", gen_real(g, clean))],
        5 => vec![(85, "NewShroudRange", gen_real(g, clean))],
        6 => vec![
            (85, "AddMaxHealth", gen_real(g, clean)),
            (70, "ChangeType", pick(g, &["SAME_CURRENTHEALTH", "PRESERVE_RATIO", "ADD_CURRENT_HEALTH_TOO", "FULLY_HEAL", "Bogus"]).to_string()),
        ],
        7 => vec![(85, "GrantScience", pick(g, &["SCIENCE_Pathfinder", "SCIENCE_Paladin", "Sci_X", "", "None"]).to_string())],
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

/// Runs the ops on a fresh world and returns the dump (after the data dump).
#[allow(clippy::too_many_arguments)]
fn rust_run(
    kind: usize,
    ctx: Ctx,
    data: ModuleDataKind,
    has_player: bool,
    player_mask: [u32; 4],
    obj_mask: [u32; 4],
    cfg: [u32; 2],
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
    if cfg[0] != 0 {
        let mut body = NewModule::plain(b"ActiveBody", b"ModuleTag_Body");
        body.interfaces = rts_lockstep_engine::common::module::MODULEINTERFACE_BODY;
        body.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, UpgradeMaskType::default())));
        modules.push(body);
    }
    let mut status = rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let id = w
        .new_object(NewObject { template: Arc::new(ThingTemplate::new(0)), status, team: None, modules })
        .unwrap();
    {
        let o = w.objects.find_mut(id).unwrap();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&obj_mask);
        // the object as the case wants it: with a drawable, or (whatever `new_object` made) without
        o.drawable = if cfg[1] != 0 { 1 } else { rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID };
    }
    let idx = w.find_module(id, CLASSES[kind].as_bytes()).unwrap();
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        // an object that has looked: the handlePartitionCellMaintenance of ActiveShroudUpgrade then undoes it, which is a cut call
        w.objects.find_mut(id).unwrap().partition_last_look.how_far = 1.0;
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
                let ob = w.objects.find_mut(id).unwrap();
                ob.cur_weapon_set_flags = Default::default();
                ob.weapon_bonus_condition = 0;
                ob.experience.scalar = f32::from_bits(o[1]);
                ob.shroud_range = f32::from_bits(o[2]);
                if let Some(bi) = ob.body {
                    if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[bi as usize].body.as_mut() {
                        p.armor_flags = 0;
                        p.max_health = f32::from_bits(o[3]);
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
            } else if t.starts_with("cut WeaponSetUpdate") {
                log.push("wset".into());
            } else if t.starts_with("cut PartitionQueueUndoShroudReveal") {
                log.push("partition".into());
            } else if let Some(rest) = t.strip_prefix("evt DrawableModelCondition") {
                // `{ object_id: N, op: Set([x]) }`
                if let Some(x) = rest.split("Set([").nth(1).and_then(|s| s.split("])").next()) {
                    log.push(format!("mc {x}"));
                }
            } else if let Some(rest) = t.strip_prefix("evt DrawableSetTerrainDecal") {
                // `{ object_id: N, decal: 7 }`
                if let Some(x) = rest.split("decal: ").nth(1).and_then(|s| s.split(' ').next()) {
                    log.push(format!("decal {x}"));
                }
            } else if let Some(rest) = t.strip_prefix("setMaxHealth ") {
                log.push(format!("maxhealth {rest}"));
            } else if let Some(rest) = t.strip_prefix("cut PlayerGrantScience") {
                // `{ player: 0, science: N }`
                if let Some(x) = rest.split("science: ").nth(1).and_then(|s| s.split(' ').next()) {
                    log.push(format!("science {x}"));
                }
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        // the state the module wrote
        let ob = w.objects.find(id).unwrap();
        let mut lo = 0u64;
        // (a WeaponSetFlags has WEAPONSET_COUNT = 17 bits)
        for b in 0..17 {
            if ob.cur_weapon_set_flags.test(b) {
                lo |= 1 << b;
            }
        }
        let hi = 0u64;
        put_u64(&mut v, lo);
        put_u64(&mut v, hi);
        put_u32(&mut v, ob.weapon_bonus_condition);
        let (armor, max_health) = match ob.body.and_then(|bi| ob.modules[bi as usize].body.as_ref()) {
            Some(UpdateBody::ModuleProbe(p)) => (p.armor_flags, p.max_health.to_bits()),
            _ => (0, 0),
        };
        put_u32(&mut v, armor);
        put_u32(&mut v, ob.experience.scalar.to_bits());
        put_u32(&mut v, ob.shroud_range.to_bits());
        put_u32(&mut v, max_health);
        let science = match ob.modules[idx as usize].body.as_ref() {
            Some(UpdateBody::GrantScienceUpgrade(m)) => m.cached_science().0,
            _ => 0,
        };
        put_i32(&mut v, science);
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w)
}

const PROBE: [&str; 9] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SCIENCE_Pathfinder", "Sci_X", "zz_fresh"];

#[test]
fn upgrade_modules_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("L9UP_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(12_000);
    let mut g = Gen::new(0x1091_50A5);
    let table = blocks();
    let mut read = Check::new("upgrade module data read (throw, message)");
    let mut run = Check::new("upgrade module ops (data, results, effects, state)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 8];
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 8;
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
        let mut upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        if kind == 2 && g.below(4) == 0 && n_up < 100 {
            // ArmorUpgrade tests isTriggeredBy( "Upgrade_AmericaChemicalSuits" ): the upgrade has to exist to be named
            upgrades.push(CHEMSUIT.to_string());
        }
        let n_up = upgrades.len();
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
        let has_player = g.below(4) != 0;
        let player_mask = gen_mask(&mut g, n_up, 0);
        let obj_mask = gen_mask(&mut g, n_up, 0);
        let cfg = [u32::from(g.below(4) != 0), u32::from(g.below(4) != 0)];
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
                    // the state to start from: an experience scalar, a shroud range, a max health
                    let r = |g: &mut Gen| g.uniform(-50.0, 500.0).to_bits();
                    [7, r(&mut g), r(&mut g), r(&mut g), 0]
                } else {
                    let m = gen_mask(&mut g, n_up, 1);
                    [op, m[0], m[1], m[2], m[3]]
                }
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour (a mux list naming `None`, a chemical suit
        // upgrade on an object with no drawable) is counted and never sent to the C++
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
            cpp::ref_l9up_run(
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
        let _ = &what;
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
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
        "upgrade modules: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} upgrades fired, {} effect lines",
        tally.fired, tally.effects
    );
    read.finish();
    run.finish();
    keys.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.fired * 6 > cases as u64 && tally.effects * 4 > cases as u64, "the generator must reach every path");
}
