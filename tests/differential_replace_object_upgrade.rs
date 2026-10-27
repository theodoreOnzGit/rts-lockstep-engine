// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ReplaceObjectUpgrade.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ReplaceObjectUpgrade` (`logic::modules::replace_object_upgrade`) with `World::thing_factory_find_template` and
//! `World::thing_factory_new_object` (`logic::world::thing_factory_world`) against the unchanged C++, `p1w11-sonnet`. The pattern is
//! `differential_upgrade_modules_c.rs`'s; read its header first.
//!
//! # Method
//!
//! The C++ side is `ReplaceObjectUpgrade.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_replace_object.inc`) on the
//! module build's stand-ins: a thing factory that knows the templates `RO_A`, `RO_B`, `RO_C` (each defined or not, each with up to four behaviour
//! modules, each with a create interface or not) and whose `newObject` makes a stand-in `Object` with those modules, the team asked for (`NULL` is
//! team 1, the neutral default team) and the controlling player that team has; a pathfinder, `GameLogic::destroyObject` and
//! `Player::onStructureConstructionComplete` that log; the REAL `UpgradeCenter`, `INI` and name keys.
//!
//! The Rust side is the real thing: a `World` whose `statics` hold the upgrade center and a `ThingFactory` read from `Object` INI blocks (the
//! modules `TestCreate`, which has a create interface, and `TestPlain`, registered for the test; they are not translated, so the world makes them
//! stubs with the interfaces their registration recorded), three teams with the players the case gives them, the object under test made by
//! `World::new_object` with the module from `modules::new_module`, and every op sent through `World::module_call`; the module destroys the object
//! with `World::destroy_object` and makes the replacement with `World::thing_factory_new_object`.
//!
//! Each case defines 0 to 8 upgrades, reads a `D` block (the `UpgradeMux` fields and `ReplaceObject`, mostly well formed), and when the read succeeds
//! runs 1 to 12 ops: `wouldUpgrade`, `attemptUpgrade`, `resetUpgrade`, `forceRefreshUpgrade`, `isAlreadyUpgraded`, changes of the object's and the
//! players' completed upgrades and of the object's place.
//!
//! Compared, bit for bit: the read's throw; the data (the mux and the template name); per op the throw status, the result, `isAlreadyUpgraded`, the
//! effects in order (`pathremove`, `destroy`, `newobject`, `settransform`, `pathadd`, `buildcomplete <object> <position>`, `structdone`, FX and
//! removals), and every object afterwards (id, destroyed, which template it was made from, team, controlling player, the 12 words of its matrix);
//! after the ops, the key `nameToKey` gives to a probe list of names.
//!
//! The `replace ...` lines `ReplaceObjectUpgrade` adds to `world.cuts.trace` are the order of its own steps (the way `upgrade_body` records
//! `remove`); the cut calls are the other effects. `destroy` is logged by the C++ stand-in every time `destroyObject` is called, as the Rust
//! module's trace line is (upstream's `destroyObject` ignores an already destroyed object after that).
//!
//! # Results (2026-10-21, 9000 cases, `L9RO_CASES` overrides)
//!
//! 8657 cases compared (9000 generated: 239 data blocks threw on both sides, 104 inputs excluded as undefined upstream), 8896 data reads,
//! 177,582 op/object dumps (56,497 ops, 1834 upgrades fired) and 8657 key probes compared, 0 mismatches. Effects: 834 `buildcomplete`, 829 each
//! of `destroy`, `newobject`, `settransform`, `pathadd` and `pathremove`, 630 `structdone`, 408 FX, 1067 removals.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000.
//!
//! | plants | what is broken |
//! |---|---|
//! | 681-682 | the pathfinder removal skipped / `destroyObject(me)` skipped |
//! | 683 | the replacement does not get the matrix |
//! | 684 | no `onBuildComplete` to the replacement's create modules |
//! | 685-686 | `onStructureConstructionComplete` skipped / told `isRebuild` TRUE |
//! | 687 | the replacement made on the neutral team instead of the object's |
//! | 688 | the template name upper-cased when read |
//!
//! All 8 are caught.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::damage_fx::DamageFxStore;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{
    Module, ModuleData, ModuleDataKind, ModuleType, ObjectId, MODULEINTERFACE_CREATE,
};
use rts_lockstep_engine::common::module_factory::{ModuleFactory, ModuleTemplate};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingParseEnv, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::ArmorStore;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::replace_object_upgrade::ReplaceObjectUpgradeModuleData;
use rts_lockstep_engine::logic::weapon_set::WeaponTemplateTable;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// -- the dump, in ref_l9ro_run's format --

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
fn mask_from(w: &[u32]) -> UpgradeMaskType {
    let mut m = UpgradeMaskType::default();
    for i in 0..128 {
        if w[i / 32] >> (i % 32) & 1 != 0 {
            m.set(i as i32, 1);
        }
    }
    m
}

// -- the thing factory --

struct Text;
impl GameTextFetch for Text {
    fn fetch(&self, _label: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
}
fn never(_: ObjectId, _: Option<ModuleDataKind>) -> Result<Module, IniError> {
    Err(IniError::new(IniErrorKind::Bug))
}
fn base_data(ini: Option<&mut Ini<'_>>) -> Result<ModuleDataKind, IniError> {
    Ok(ModuleDataKind::Module(Arc::new(ModuleData::friend_new_module_data(ini)?)))
}

struct FactoryCtx {
    factory: ThingFactory,
    names: NameKeyGenerator,
    modules: ModuleFactory,
    armors: ArmorStore,
    dfx: DamageFxStore,
    weapons: WeaponTemplateTable,
    sciences: ScienceStore,
}

fn object_block(ini: &mut Ini<'_>, c: &mut FactoryCtx) -> IniResult<()> {
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.dfx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &Text,
        locomotors: &rts_lockstep_engine::logic::locomotor::LocomotorStore::new(),
    };
    c.factory.parse_object_block(ini, &mut env, 0)
}

/// The templates RO_A, RO_B, RO_C the case defines (`cfg` words: bit 31 defined, bits 4-6 the number of modules, bits 0-3 the create flags), as a
/// factory read from INI text. The modules are `TestCreate` (a create interface) and `TestPlain` (none), registered for the test.
fn build_factory(templates: &[u32; 3]) -> ThingFactory {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut modules = ModuleFactory::new();
    for (name, mask) in [("TestCreate", MODULEINTERFACE_CREATE), ("TestPlain", 0)] {
        let t = ModuleTemplate { create_proc: Some(never), create_data_proc: Some(base_data), which_interfaces: Some(mask) };
        modules.add_module_internal(&mut names, t, ModuleType::Behavior, name).unwrap();
    }
    let mut c = FactoryCtx {
        factory: ThingFactory::new(),
        names,
        modules,
        armors: ArmorStore::new(),
        dfx: DamageFxStore::new(),
        weapons: WeaponTemplateTable::new(),
        sciences: ScienceStore::new(),
    };
    let mut text = String::new();
    for (i, w) in templates.iter().enumerate() {
        if w >> 31 & 1 == 0 {
            continue;
        }
        text.push_str(&format!("Object RO_{}\n", ["A", "B", "C"][i]));
        for k in 0..(w >> 4 & 7).min(4) {
            let class = if w >> k & 1 != 0 { "TestCreate" } else { "TestPlain" };
            text.push_str(&format!(" Behavior = {class} ModuleTag_{k}\n End\n"));
        }
        text.push_str("End\n");
    }
    static BLOCKS: [BlockParse<FactoryCtx>; 1] = [BlockParse { token: "Object", parse: object_block }];
    Ini::load(b"Objects.ini", text.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("objects load");
    c.factory
}

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ReplaceObjectUpgrade(Arc::new(ReplaceObjectUpgradeModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::ReplaceObjectUpgrade(d) = data else { unreachable!("not a ReplaceObjectUpgrade") };
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_str(v, &d.replace_object_name);
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
    out.push(format!("replace object {:?}", str_(&mut p)));
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
        let n_obj = u32_(&mut p);
        let mut objs = Vec::new();
        for _ in 0..n_obj {
            let id = u32_(&mut p) as i32;
            let destroyed = u32_(&mut p);
            let tmpl = u32_(&mut p);
            let team = u32_(&mut p) as i32;
            let player = u32_(&mut p) as i32;
            let m: Vec<u32> = (0..12).map(|_| u32_(&mut p)).collect();
            objs.push(format!("object {id} destroyed {destroyed} template {tmpl} team {team} player {player} matrix {m:x?}"));
        }
        out.push(format!("op {i}: status {st} res {res} upgraded {up} log {log:?}"));
        out.extend(objs.into_iter().map(|o| format!("op {i}: {o}")));
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

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
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
    let v = pick(g, &["RO_A", "RO_B", "RO_C", "RO_A", "RO_B", "ro_a", "None", "RO_D", ""]).to_string();
    field(g, 90, "ReplaceObject", v);
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

/// The line the driver logs for a cut call.
fn cut_line(c: &CutCall) -> Option<String> {
    Some(match c {
        CutCall::PathfinderRemoveObjectFromPathfindMap { object } => format!("pathremove {object}"),
        CutCall::PathfinderAddObjectToPathfindMap { object } => format!("pathadd {object}"),
        CutCall::PlayerOnStructureConstructionComplete { player, builder, structure, is_rebuild } => {
            format!("structdone {player} {builder} {structure} {}", i32::from(*is_rebuild))
        }
        _ => return None,
    })
}

fn translation(x: f32, y: f32, z: f32) -> Matrix3D {
    Matrix3D::from_array([1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z])
}

/// Runs the ops on a fresh world and returns the dump (after the data dump), the key probes and the world.
#[allow(clippy::too_many_arguments)]
fn rust_run(ctx: Ctx, data: ModuleDataKind, player_mask: [u32; 4], obj_mask: [u32; 4], cfg: &mut [u32; 8], ops: &[[u32; 5]], tally: &mut Tally) -> (Vec<u8>, Vec<u32>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut w = World::new();
    let factory = build_factory(&[cfg[2], cfg[3], cfg[4]]);
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, thing_factory: factory, ..Default::default() });
    w.names = ctx.names;
    w.cuts.neutral_default_team = 1;
    for t in 1..=3 {
        w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team::new(t, 0)).unwrap();
        if cfg[4 + t as usize] != 0 {
            w.cuts.team_players.insert(t, cfg[4 + t as usize] as i32 - 1);
        }
    }
    for p in 0..3 {
        w.cuts.player_upgrades.insert(p, mask_from(&player_mask));
    }
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"ReplaceObjectUpgrade", b"ModuleTag_Up", &data).unwrap().unwrap();
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let me = w
        .new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default())),
            status,
            team: Some(cfg[1] as _),
            modules: vec![m],
        })
        .unwrap();
    cfg[0] = me as u32;
    {
        let o = w.objects.find_mut(me).unwrap();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&obj_mask);
    }
    let idx = w.find_module(me, b"ReplaceObjectUpgrade").unwrap();
    let mut new_ids: BTreeMap<i32, i32> = BTreeMap::new(); // a replacement's id -> the position of its first module
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let res = match o[0] {
            0 => w.module_call(me, idx, ModuleCall::WouldUpgrade(mask)).as_bool(),
            1 => {
                let r = w.module_call(me, idx, ModuleCall::AttemptUpgrade(mask)).as_bool();
                tally.fired += u64::from(r);
                r
            }
            2 => w.module_call(me, idx, ModuleCall::ResetUpgrade(mask)).as_bool(),
            3 => {
                w.module_call(me, idx, ModuleCall::ForceRefreshUpgrade);
                false
            }
            4 => w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded).as_bool(),
            5 => {
                w.objects.find_mut(me).unwrap().object_upgrades_completed = mask;
                false
            }
            6 => {
                for p in 0..3 {
                    w.cuts.player_upgrades.insert(p, mask);
                }
                false
            }
            _ => {
                let f = |x: u32| f32::from_bits(x);
                w.set_transform_matrix(me, &translation(f(o[1]), f(o[2]), f(o[3])));
                false
            }
        };
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        v.push(u8::from(res && !threw));
        v.push(u8::from(w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded).as_bool()));
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
        let mut last_new = 0i32;
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
            } else if t == "replace destroy" {
                log.push(format!("destroy {me}"));
            } else if let Some(n) = t.strip_prefix("replace newobject ") {
                last_new = n.parse().unwrap();
                // the position of the replacement's first behaviour module: the driver counts from it
                let first = ["TestCreate", "TestPlain"].iter().filter_map(|c| w.find_module(last_new as ObjectId, c.as_bytes())).min().map_or(0, i32::from);
                new_ids.insert(last_new, first);
                log.push(format!("newobject {n}"));
            } else if t == "replace settransform" {
                log.push(format!("settransform {last_new}"));
            } else if let Some(i) = t.strip_prefix("replace buildcomplete ") {
                log.push(format!("buildcomplete {last_new} {}", i.parse::<i32>().unwrap() - new_ids[&last_new]));
            }
        }
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        // the objects, in id order
        let mut ids = w.objects.ids_in_order();
        ids.sort();
        put_u32(&mut v, ids.len() as u32);
        for id in ids {
            let ob = w.objects.find(id).unwrap();
            put_i32(&mut v, id);
            put_i32(&mut v, i32::from(ob.is_destroyed()));
            let t = match ob.template().get_name() {
                b"RO_A" => 1,
                b"RO_B" => 2,
                b"RO_C" => 3,
                _ => 0,
            };
            put_i32(&mut v, t);
            put_i32(&mut v, ob.team as i32);
            put_i32(&mut v, w.get_controlling_player(id).unwrap_or(-1));
            for x in ob.thing.transform_matrix().to_array() {
                put_u32(&mut v, x.to_bits());
            }
        }
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w)
}

const PROBE: [&str; 9] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SCIENCE_Pathfinder", "Sci_X", "zz_fresh"];

fn cases() -> usize {
    std::env::var("L9RO_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn replace_object_upgrade_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_D0D5);
    let table = blocks();
    let mut read = Check::new("ReplaceObjectUpgrade data read (throw, message)");
    let mut run = Check::new("ReplaceObjectUpgrade ops (data, results, effects, objects)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut compared = 0u64;
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
        let text = gen_data(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Um{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let player_mask = gen_mask(&mut g, n_up, 0);
        let obj_mask = gen_mask(&mut g, n_up, 0);
        // [the object's id (the Rust side's), its team 1..3, RO_A, RO_B, RO_C, the player of team 1, 2, 3 (0 none, else the index + 1)]
        let mut cfg = [0u32; 8];
        cfg[1] = 1 + g.below(3);
        for t in 0..3 {
            let defined = u32::from(g.below(4) != 0) << 31;
            let n_mod = g.below(5);
            cfg[2 + t] = defined | (n_mod << 4) | (g.below(16) & ((1 << n_mod) - 1));
            cfg[5 + t] = if g.below(4) == 0 { 0 } else { 1 + g.below(3) };
        }
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| {
                let op = match g.below(30) {
                    0..=2 => 0,
                    3..=14 => 1,
                    15..=17 => 2,
                    18..=19 => 3,
                    20..=21 => 4,
                    22..=24 => 5,
                    25 => 6,
                    _ => 7,
                };
                if op == 7 {
                    [7, (g.below(2000) as f32 - 1000.0).to_bits(), (g.below(2000) as f32 - 1000.0).to_bits(), g.uniform(-50.0, 50.0).to_bits(), 0]
                } else {
                    let m = gen_mask(&mut g, n_up, 1);
                    [op, m[0], m[1], m[2], m[3]]
                }
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w) = rust_run(ctx, data.expect("read ok"), player_mask, obj_mask, &mut cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some((rd, rk, w))
        } else {
            cfg[0] = 1;
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
            cpp::ref_l9ro_run(
                cfile.as_ptr(),
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
        let what = || format!("case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["pathremove", "destroy", "newobject", "settransform", "pathadd", "buildcomplete", "structdone", "remove ", "fx"] {
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
        compared += 1;
        let _ = w;
    }
    println!(
        "ReplaceObjectUpgrade: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
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
