// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/{CostModifierUpgrade, PowerPlantUpgrade, RadarUpgrade,
//   UnpauseSpecialPowerUpgrade}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! Four more layer-9 upgrade modules (`logic::modules::{cost_modifier_upgrade, power_plant_upgrade, radar_upgrade,
//! unpause_special_power_upgrade}`) against the unchanged C++, `p1w11-sonnet`, upgrade batch C. The pattern is
//! `differential_upgrade_modules_b.rs`'s; read its header first.
//!
//! # Method
//!
//! The C++ side is the four `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_upgrades_c.inc`) on the module
//! build's stand-ins (`differential/shim_module`): three players that log what the modules do to them (`addcost`, `removecost`, `addpower`,
//! `removepower`, `addradar`, `removeradar`, each tagged with the player's index), an `Object` with a disabled flag and up to four behaviour
//! modules (each with a `PowerPlantUpdateInterface` that logs `extendRods`, or a `SpecialPowerModuleInterface` whose template is one of the real
//! store's and whose `pauseCountdown` logs, or neither), a radar update the object has or has not (found by `findUpdateModule`, whose
//! `extendRadar` logs), the REAL `UpgradeCenter`, `INI` and name keys, and for `UnpauseSpecialPowerUpgrade` the REAL `SpecialPowerStore` of
//! `wrap_upgrade.cpp` behind `INI::parseSpecialPowerTemplate` (the lookup of the stand-in store in `wrap_ini.cpp` is bound to it while the data is
//! read).
//!
//! Each case defines 0 to 8 upgrades and 0 to 3 special powers (`SP_0`..`SP_2`) on both sides, picks a module kind, generates a `D` block of the
//! `UpgradeMux` fields and the module's own (mostly well-formed, sometimes with unknown names, bad separators, junk lines or no `End`), reads it on
//! both sides, and when the read succeeds gives the module an object (disabled or not, with a controlling player of three or none, with random
//! behaviour modules) and runs 1 to 14 ops: `wouldUpgrade`, `attemptUpgrade`, `resetUpgrade`, `forceRefreshUpgrade`, `isAlreadyUpgraded`, changes
//! of the object's and the players' completed upgrades, of the disabled flag and of the controlling player, `onDelete`, `onCapture(old, new)` (either
//! owner may be `NULL`) and `loadPostProcess`.
//!
//! The Rust side is the real thing: a `World` whose `statics` hold the loaded upgrade center and special power store, one object made by
//! `World::new_object` with the module from `modules::new_module` and the behaviour modules as scripted `ProbeModule`s, and every op sent through
//! `World::module_call` (`OnDelete` and `OnCapture` as the upgrade call path of `modules::upgrade_body` answers them, and `LoadPostProcess` through `World::modules_load_post_process`).
//!
//! Compared, bit for bit: the read's throw (status and message); the data (the mux and the module's fields: the kind-of list and the percentage
//! bits, the disable-proof flag, the id of the template the name finds); per op the throw status, the result, `isAlreadyUpgraded`, the effects in the
//! order they happened; after the ops, the key `nameToKey` gives to a probe list of names (which includes `RadarUpdate`, whose key
//! `RadarUpgrade` assigns).
//!
//! Inputs for which upstream is undefined (`CostModifierUpgrade` and `RadarUpgrade` on an object with no controlling player: `player->add...` is
//! dereferenced without a test) are never sent to the C++: the Rust returns an `is_undefined()` error and the case is counted as excluded.
//!
//! # Deviation from upstream that the test cannot see
//!
//! `UnpauseSpecialPowerUpgrade` keeps the NAME of its template and looks it up when the upgrade runs (the data's `NewModuleDataProc` has no
//! store); upstream looks it up in `INI::parseSpecialPowerTemplate`. Both sides here define the special powers before the data is read, so
//! the two agree; they could differ only if the store changed between the read and the use.
//!
//! # Results (2026-10-21, 9000 cases, `L9UPC_CASES` overrides)
//!
//! 8712 data reads, 118,456 op dumps (results, effects) and 8354 key probes compared, 0 mismatches (288 inputs excluded as undefined upstream;
//! 358 data blocks threw on both sides; 1641 `attemptUpgrade` calls fired; effects: 917 removals, 573 `addpower`, 542 `addcost`, 507 `addradar`,
//! 373 FX, 313 `extendrods`, 290 `extendradar`, 200 `removecost`, 191 `pause`, 183 `removepower`, 146 `removeradar`).
//!
//! Re-run 2026-10-21 after batch E (same numbers, 0 mismatches): `RadarUpgrade`'s `findUpdateModule("RadarUpdate")` now finds the REAL `RadarUpdate`
//! (batch E compiles `RadarUpdate.cpp`, so the old stand-in `extendRadar` that logged is gone), on an object with the stand-in drawable; its
//! `extendRadar()` gives the drawable `MODELCONDITION_RADAR_EXTENDING`, which the driver shows as the line `extendradar`.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000. All 29 are caught (the first run of 655 survived:
//! it skipped the first module of the object, which is a helper; it now skips the module after the one under test, and is caught).
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 640-641 | CostModifier | `onDelete` runs for a module that was not upgraded / does not clear `m_upgradeExecuted` |
//! | 642-644 | CostModifier | `onCapture` skips the removal for the old owner / the addition for the new one / clearing the flag for the old owner |
//! | 645-646 | CostModifier | `Percentage` read as a plain real / `EffectKindOf` not read |
//! | 647 | CostModifier | `upgradeImplementation` adds nothing |
//! | 650 | PowerPlant | `onDelete` runs for a module that was not upgraded |
//! | 651 | PowerPlant | `onCapture` of a disabled object does not return early |
//! | 652 | PowerPlant | `onCapture`'s old and new owner swapped |
//! | 653-655 | PowerPlant | no power bonus on upgrade / `extendRods(FALSE)` / the module after this one skipped |
//! | 656-657 | PowerPlant | the world half of `loadPostProcess` skipped / run for a module that is not upgraded |
//! | 658 | PowerPlant | `onDelete` leaves the module "upgraded" |
//! | 660-661 | Radar | `onDelete` / `onCapture` of a disabled object do not return early |
//! | 662 | Radar | `addRadar` told the opposite of `DisableProof` |
//! | 663-664 | Radar | `extendRadar` not sent / `NAMEKEY("RadarUpdate")` not taken (seen only by the key probes) |
//! | 665-666 | Radar | `onDelete` leaves the module "upgraded" / `onCapture`'s owners swapped |
//! | 670-673 | UnpauseSpecialPower | every special power module matches / a `NULL` template never matches / `pauseCountdown(TRUE)` / the loop stops at the first match |
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::disabled_types::DISABLED_HACKED;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KindOfMaskType, KINDOF_COUNT};
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::cost_modifier_upgrade::CostModifierUpgradeModuleData;
use rts_lockstep_engine::logic::modules::power_plant_upgrade::PowerPlantUpgradeModuleData;
use rts_lockstep_engine::logic::modules::radar_upgrade::RadarUpgradeModuleData;
use rts_lockstep_engine::logic::modules::unpause_special_power_upgrade::UnpauseSpecialPowerUpgradeModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
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
const CLASSES: [&str; 4] = ["CostModifierUpgrade", "PowerPlantUpgrade", "RadarUpgrade", "UnpauseSpecialPowerUpgrade"];

// -- the dump, in ref_l9upc_run's format --

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
/// A kind-of mask as the driver prints it: the indices of its set bits, each followed by a comma.
fn kind_str(m: &KindOfMaskType) -> String {
    (0..KINDOF_COUNT).filter(|&i| m.test(i)).map(|i| format!("{i},")).collect()
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
        0 => ModuleDataKind::CostModifierUpgrade(Arc::new(CostModifierUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        1 => ModuleDataKind::PowerPlantUpgrade(Arc::new(PowerPlantUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        2 => ModuleDataKind::RadarUpgrade(Arc::new(RadarUpgradeModuleData::friend_new_module_data(Some(ini))?)),
        _ => ModuleDataKind::UnpauseSpecialPowerUpgrade(Arc::new(UnpauseSpecialPowerUpgradeModuleData::friend_new_module_data(Some(ini))?)),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9upc_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds, store: &SpecialPowerStore) {
    match data {
        ModuleDataKind::CostModifierUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            put_str(v, kind_str(&d.kind_of).as_bytes());
            put_u32(v, d.percentage.to_bits());
        }
        ModuleDataKind::PowerPlantUpgrade(d) => put_mux(v, &d.upgrade.mux_data(), fx),
        ModuleDataKind::RadarUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            v.push(u8::from(d.is_disable_proof));
        }
        ModuleDataKind::UnpauseSpecialPowerUpgrade(d) => {
            put_mux(v, &d.upgrade.mux_data(), fx);
            // upstream resolved the name while it read the data; here it is resolved when the upgrade runs
            put_u32(v, d.special_power.as_deref().and_then(|n| store.find_special_power_template(n)).map_or(0, |t| t.id()));
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
            let kinds = str_(&mut p);
            let bits = u32_(&mut p);
            out.push(format!("cost kinds [{kinds}] percent {bits:#x}"));
        }
        2 => {
            out.push(format!("disable proof {}", b[p]));
            p += 1;
        }
        3 => out.push(format!("special power template {}", u32_(&mut p))),
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
        out.push(format!("op {i}: status {st} res {res} upgraded {up} log {log:?}"));
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

const KIND_NAMES: [&str; 8] = ["STRUCTURE", "INFANTRY", "VEHICLE", "AIRCRAFT", "SELECTABLE", "CAN_ATTACK", "SCORE", "HUGE_VEHICLE"];

fn gen_kind_list(g: &mut Gen, clean: bool) -> String {
    match g.below(8) {
        0 => "NONE".to_string(),
        1 if !clean => pick(g, &["Bogus", "+STRUCTURE", "STRUCTURE +VEHICLE", "", "-INFANTRY", "KINDOF_VEHICLE"]).to_string(),
        2 => format!("+{}", pick(g, &KIND_NAMES)),
        3 => format!("-{}", pick(g, &KIND_NAMES)),
        _ => (0..1 + g.below(3)).map(|_| pick(g, &KIND_NAMES).to_string()).collect::<Vec<_>>().join(" "),
    }
}

fn gen_percent(g: &mut Gen, clean: bool) -> String {
    match g.below(10) {
        0..=3 => format!("{}%", g.below(200) as i32 - 100),
        4 => format!("{}%", g.uniform(-100.0, 100.0)),
        5 => format!("{}", g.below(100)),
        6 => pick(g, &["0%", "-0%", "100%", "-100%", "12.5%", "33.3333%"]).to_string(),
        _ if clean => "25%".to_string(),
        7 => pick(g, &["x", "", "%", "5%%", "1e2%"]).to_string(),
        _ => format!("{}", g.mixed()),
    }
}

/// The module's own fields.
fn own_fields(g: &mut Gen, kind: usize, clean: bool) -> Vec<(u32, &'static str, String)> {
    match kind {
        0 => vec![(85, "EffectKindOf", gen_kind_list(g, clean)), (85, "Percentage", gen_percent(g, clean))],
        2 => vec![(70, "DisableProof", gen_bool(g, clean))],
        3 => vec![(88, "SpecialPowerTemplate", pick(g, &["SP_0", "SP_1", "SP_2", "None", "none", "sp_0", "SP_9", "SP_1"]).to_string())],
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

fn probe(has: u32, power_template: u32) -> UpdateBody {
    UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { has, power_template, ..Default::default() }, 0.0, UpgradeMaskType::default()))
}

/// The line the driver logs for a cut call.
fn cut_line(c: &CutCall) -> Option<String> {
    Some(match c {
        CutCall::PlayerAddKindOfProductionCostChange { player, kind_of, percent } => format!("addcost {player} [{}] {}", kind_str(kind_of), percent.to_bits()),
        CutCall::PlayerRemoveKindOfProductionCostChange { player, kind_of, percent } => format!("removecost {player} [{}] {}", kind_str(kind_of), percent.to_bits()),
        CutCall::PlayerAddPowerBonus { player, .. } => format!("addpower {player}"),
        CutCall::PlayerRemovePowerBonus { player, .. } => format!("removepower {player}"),
        CutCall::PlayerAddRadar { player, disable_proof } => format!("addradar {player} {}", i32::from(*disable_proof)),
        CutCall::PlayerRemoveRadar { player, disable_proof } => format!("removeradar {player} {}", i32::from(*disable_proof)),
        _ => return None,
    })
}

/// Runs the ops on a fresh world and returns the dump (after the data dump).
#[allow(clippy::too_many_arguments)]
fn rust_run(
    kind: usize,
    ctx: Ctx,
    data: ModuleDataKind,
    ctrl: i32,
    player_mask: [u32; 4],
    obj_mask: [u32; 4],
    cfg: [u32; 11],
    ops: &[[u32; 5]],
    tally: &mut Tally,
) -> (Vec<u8>, Vec<u32>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx, &ctx.store);

    // the template the driver finds for SP_<i>: the id in the store, 0 (NULL) for a name the store does not have (or 99)
    let template_of = |i: u32| -> u32 { ctx.store.find_special_power_template(format!("SP_{i}").as_bytes()).map_or(0, |t| t.id()) };
    let templates: Vec<u32> = (0..4).map(|i| if cfg[7 + i] == 99 { 0 } else { template_of(cfg[7 + i]) }).collect();

    let mut w = World::new();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.neutral_default_team = 1;
    w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team::new(1, 0)).unwrap();
    if ctrl >= 0 {
        w.cuts.team_players.insert(1, ctrl);
    }
    for p in 0..3 {
        w.cuts.player_upgrades.insert(p, mask_from(&player_mask));
    }
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let m = rts_lockstep_engine::logic::modules::new_module(0, CLASSES[kind].as_bytes(), b"ModuleTag_Up", &data).unwrap().unwrap();
    let mut modules = vec![m];
    for i in 0..cfg[1].min(4) as usize {
        let mut b = NewModule::plain(b"Behavior", format!("ModuleTag_B{i}").as_bytes());
        match cfg[2 + i] {
            // a power plant update interface (ProbeAnswers::has bit 20)
            1 => b.body = Some(probe(1 << 20, 0)),
            // a special power interface with its template
            2 => {
                b.interfaces = MODULEINTERFACE_SPECIAL_POWER;
                b.body = Some(probe(0, templates[i]));
            }
            _ => b.body = Some(probe(0, 0)),
        }
        modules.push(b);
    }
    if cfg[6] != 0 && kind == 2 {
        let mut radar = NewModule::plain(b"RadarUpdate", b"ModuleTag_Radar");
        radar.body = Some(probe(0, 0));
        modules.push(radar);
    }
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default())), status, team: None, modules }).unwrap();
    {
        let o = w.objects.find_mut(id).unwrap();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&obj_mask);
        o.disabled.set(DISABLED_HACKED, i32::from(cfg[0] != 0));
    }
    let idx = w.find_module(id, CLASSES[kind].as_bytes()).unwrap();
    // the position of the first behaviour module (the object's helpers come first, then the module under test): the driver counts from it
    let base = w.find_module(id, b"Behavior").map_or(0, i32::from);
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
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
                for p in 0..3 {
                    w.cuts.player_upgrades.insert(p, mask);
                }
                false
            }
            7 => {
                w.objects.find_mut(id).unwrap().disabled.set(DISABLED_HACKED, (o[1] & 1) as i32);
                false
            }
            8 => {
                w.module_call(id, idx, ModuleCall::OnDelete);
                false
            }
            9 => {
                w.module_call(id, idx, ModuleCall::OnCapture { old_owner: o[1] as i32 - 1, new_owner: o[2] as i32 - 1 });
                false
            }
            10 => {
                w.modules_load_post_process(id);
                false
            }
            _ => {
                if o[1] == 0 {
                    w.cuts.team_players.remove(&1);
                } else {
                    w.cuts.team_players.insert(1, o[1] as i32 - 1);
                }
                false
            }
        };
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        v.push(u8::from(res && !threw));
        v.push(u8::from(w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool()));
        // the effects, in order: the trace has each removal, cut call, client event and probe line of a module
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
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("evt FxListDoFxObj") {
                log.push(format!("fx{}", fx.id(&fx_iter.next())));
            } else if let Some(n) = t.strip_prefix("remove ") {
                log.push(format!("remove {n}"));
            } else if t.starts_with("cut ") {
                let c = cut_iter.next().expect("a cut line has a cut call");
                if let Some(l) = cut_line(c) {
                    log.push(l);
                }
            } else if let Some(r) = t.strip_prefix("extendrods ") {
                // `<module position> <extend>`: the driver counts from the first behaviour module, the world from the object's first helper
                let (i, e) = r.split_once(' ').unwrap();
                log.push(format!("extendrods {} {e}", i.parse::<i32>().unwrap() - base));
            } else if let Some(r) = t.strip_prefix("pause ") {
                let (i, e) = r.split_once(' ').unwrap();
                log.push(format!("pause {} {e}", i.parse::<i32>().unwrap() - base));
            } else if t == "extendradar" {
                log.push("extendradar".into());
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w)
}

const PROBE: [&str; 11] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SCIENCE_Pathfinder", "Sci_X", "zz_fresh", "RadarUpdate", "radarupdate"];

fn cases() -> usize {
    std::env::var("L9UPC_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn upgrade_modules_c_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_C0C5);
    let table = blocks();
    let mut read = Check::new("upgrade module C data read (throw, message)");
    let mut run = Check::new("upgrade module C ops (data, results, effects)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 4];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 4;
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
        let n_sp = g.below(4) as usize;
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let mut defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
        defs.push_str(&(0..n_sp).map(|i| format!("SpecialPower SP_{i}\nEnd\n")).collect::<String>());
        let mut ctx = Ctx::new(init_names, init_center);
        Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        cpp_reset(init_names, init_center);
        assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

        let clean = g.below(10) < 7;
        let text = gen_data(&mut g, kind, &upgrades, clean);
        let name = format!("Data\\INI\\Um{}.ini", case % 5).into_bytes();
        // the special power store must be the one the data is read against, on both sides
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let ctrl: i32 = if g.below(5) == 0 { -1 } else { g.below(3) as i32 };
        let player_mask = gen_mask(&mut g, n_up, 0);
        let obj_mask = gen_mask(&mut g, n_up, 0);
        // [disabled, number of behaviour modules, their kinds (0 none, 1 power plant, 2 special power), has a radar update, their templates]
        let mut cfg = [0u32; 11];
        cfg[0] = u32::from(g.below(4) == 0);
        cfg[1] = g.below(5);
        for i in 0..4 {
            cfg[2 + i] = g.below(3);
            cfg[7 + i] = if g.below(6) == 0 { 99 } else { g.below(3) };
        }
        cfg[6] = u32::from(g.below(3) != 0);
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| {
                let op = match g.below(48) {
                    0..=3 => 0,
                    4..=19 => 1,
                    20..=22 => 2,
                    23..=24 => 3,
                    25..=26 => 4,
                    27..=28 => 5,
                    29 => 6,
                    30..=31 => 7,
                    32..=34 => 8,
                    35..=40 => 9,
                    41..=43 => 10,
                    _ => 11,
                };
                match op {
                    7 | 11 => [op, g.below(if op == 7 { 2 } else { 4 }), 0, 0, 0],
                    9 => [op, g.below(4), g.below(4), 0, 0],
                    8 | 10 => [op, 0, 0, 0, 0],
                    _ => {
                        let m = gen_mask(&mut g, n_up, 1);
                        [op, m[0], m[1], m[2], m[3]]
                    }
                }
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w) = rust_run(kind, ctx, data.expect("read ok"), ctrl, player_mask, obj_mask, cfg, &ops, &mut tally);
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
            cpp::ref_l9upc_run(
                kind as i32,
                cfile.as_ptr(),
                ctrl,
                [player_mask[0], player_mask[1], player_mask[2], player_mask[3]].as_ptr(),
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
        let what = || format!("case {case} ({}): upgrades {upgrades:?} sp {n_sp} ctrl {ctrl} {player_mask:?} obj {obj_mask:?} cfg {cfg:?}, file {:?}, ops {ops:?}", CLASSES[kind], show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["addcost", "removecost", "addpower", "removepower", "addradar", "removeradar", "extendrods", "extendradar", "pause", "remove ", "fx"] {
                *counts.entry(key.trim().to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) ctrl {ctrl} cfg {cfg:?} file {:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
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
        "upgrade modules C: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
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
