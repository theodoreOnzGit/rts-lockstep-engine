// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 5's module base classes
//! (`common::module`, `common::module_factory`, `logic::module::*`) against
//! upstream `Module.cpp`, `BehaviorModule.cpp`, `UpdateModule.cpp`,
//! `UpgradeModule.cpp`, `DieModule.cpp`, `CreateModule.cpp`, `DamageModule.cpp`,
//! `CollideModule.cpp`, `DestroyModule.cpp`, `BodyModule.cpp` and the body of
//! `ModuleFactory::init` (commit 0a05454d), compiled as
//! `differential/wrap_module.cpp` describes, with the real `INI`, the real
//! `UpgradeCenter` (the Upgrade build, through `ref_up_reset` / `ref_up_load`),
//! `NameKeyGenerator` and `XferSave`/`XferLoad`/`XferCRC` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: module
//! data blocks (every field in its several forms, upgrade names that exist,
//! differ only in case, do not exist, or are `None`; bad and missing values,
//! unknown fields, a missing `End`, comments) over a small set of upgrades
//! defined on both sides by `Upgrade` blocks.
//!
//! 1. `registrations`: the 205 `addModule` names of `ModuleFactory::init`
//!    in order, preprocessed from the unchanged text with `ALLOW_SURRENDER`
//!    and `ALLOW_DEMORALIZE` defined, and again as the shipped game (199),
//!    against `module_factory::REGISTRATIONS`.
//! 2. `constants`: the `MODULEINTERFACE_*` bits, `ModuleType`, the interface
//!    masks of the base classes, `UpdateSleepTime` and `SleepyUpdatePhase`.
//! 3. `module data`: 6,000 blocks read by each data class's
//!    `friend_newModuleData` (or `buildFieldParse` where the class has no
//!    standard macro) for the nine data kinds: what `INI::load` throws (and
//!    its whole message) and, for `UpgradeModuleData` and `DieModuleData`,
//!    every field.
//! 4. `UpgradeMux`: 6,000 modules, each a random `UpgradeModuleData` over 0 to
//!    8 upgrades, driven through a random sequence of `wouldUpgrade`,
//!    `attemptUpgrade`, `testUpgradeConditions`, `resetUpgrade`,
//!    `forceRefreshUpgrade`, `isAlreadyUpgraded` and `isTriggeredBy`
//!    on random 128-bit keys. Compared after **every** op: what it threw,
//!    its result, `isAlreadyUpgraded()`, the effects in the order they
//!    happened (the effect list, each `removeUpgrade`, `upgradeImplementation`)
//!    and the whole `UpgradeMuxData` (every name list, both masks, the
//!    effect list, the flag); and at the end the keys `nameToKey` gives to a
//!    list of names (`findUpgrade` assigns a key to every name it looks up).
//! 5. `UpdateModule`: 6,000 sequences of `friend_setNextCallFrame`,
//!    `friend_getPriority` and its three siblings, `getWakeFrame`,
//!    `frameToSleepTime`, `setWakeFrame` (the call it makes to
//!    `TheGameLogic->friend_awakenUpdateModule`), the index in the logic list,
//!    xfer save, load (of a word with any phase) and CRC, in each of the four
//!    phases.
//! 6. `DieMuxData`: 6,000 modules over random death types, levels and status
//!    masks: `isDieApplicable` through the data and through `DieModule`.
//! 7. The base classes' constructors (a null data is `INI_INVALID_DATA`),
//!    xfer chains (save, load, CRC), `CreateModule`'s and `BodyModule`'s state
//!    and defaults, `CollideModule`'s defaults, `BodyDamageType` and
//!    `IS_CONDITION_WORSE/BETTER`.
//!
//! **Not compared** (their `.cpp` files need the player, the UI, the script
//! engine and the thing factory; see `differential/wrap_module.cpp`; tested by
//! Rust unit tests against the upstream text instead): `SpecialPowerModule`,
//! `SpecialPowerModuleData`, `SpecialPowerUpdateModule`, the
//! `ModuleFactory` lookups themselves (the 205 classes are not available:
//! only the registration names are), and `ContainModuleInterface::
//! getContainerPipsToShow`.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (a name
//! that is empty or `None` and names no upgrade, a field line in a block of
//! the base `ModuleData`, which reads a null parse table) are **not** given to
//! the C++; their count is printed. `DEATH_NORMAL` and `LEVEL_REGULAR`
//! (`1UL << -1`) are not given either (see `logic::module::die_module`).
//!
//! Each comparison also runs a planted bug, which must be caught.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined};
use rts_lockstep_engine::common::module::*;
use rts_lockstep_engine::common::module_factory::{REGISTRATIONS, NUM_REGISTRATIONS};
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_NAMES};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};
use rts_lockstep_engine::logic::module::body_module::*;
use rts_lockstep_engine::logic::module::collide_module::{CollideModule, CollideModuleData};
use rts_lockstep_engine::logic::module::create_module::{CreateModule, CreateModuleData};
use rts_lockstep_engine::logic::module::damage_module::{DamageModule, DamageModuleData};
use rts_lockstep_engine::logic::module::destroy_module::DestroyModule;
use rts_lockstep_engine::logic::module::die_module::{DieModule, DieModuleData, DieMuxData};
use rts_lockstep_engine::logic::module::update_module::*;
use rts_lockstep_engine::logic::module::upgrade_module::{UpgradeEffects, UpgradeModule, UpgradeModuleData, UpgradeMuxData};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

/// The C++ side has process-global state (the name keys, the stores, `strtok`,
/// `XferLoad`'s static buffer), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rts-lockstep-module-{}-{tag}.bin", std::process::id()))
}

fn status_of(r: Result<(), XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

// ---------------------------------------------------------------------------
// registrations
// ---------------------------------------------------------------------------

fn cpp_registrations(which: i32) -> Vec<Vec<u8>> {
    let mut buf = vec![0u8; 1 << 16];
    let n = unsafe { cpp::ref_mod_registrations(which, buf.as_mut_ptr(), buf.len() as i32) } as usize;
    assert!(n > 0 && n <= buf.len());
    let rd = |at: &mut usize| -> u32 {
        let v = u32::from_le_bytes(buf[*at..*at + 4].try_into().unwrap());
        *at += 4;
        v
    };
    let mut at = 0;
    let count = rd(&mut at);
    let mut out = Vec::new();
    for _ in 0..count {
        let len = rd(&mut at) as usize;
        out.push(buf[at..at + len].to_vec());
        at += len;
    }
    assert_eq!(at, n);
    out
}

#[test]
fn registrations_match_the_unchanged_init_body() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = Check::new("ModuleFactory::init names (205, in order)");
    let mut shipped = Check::new("ModuleFactory::init names (shipped, 199)");
    let mut swap_bug = Check::new("ModuleFactory::init names (order)");
    let mut drop_bug = Check::new("ModuleFactory::init names (shipped)");
    let rust_all: Vec<Vec<u8>> = REGISTRATIONS.iter().map(|r| r.name.as_bytes().to_vec()).collect();
    let rust_shipped: Vec<Vec<u8>> = REGISTRATIONS.iter().filter(|r| r.in_shipped_build).map(|r| r.name.as_bytes().to_vec()).collect();
    let c_all = cpp_registrations(0);
    let c_shipped = cpp_registrations(1);
    assert_eq!((c_all.len(), c_shipped.len(), NUM_REGISTRATIONS), (205, 199, 205));
    all.eq(rust_all.clone(), c_all.clone(), || "all".into());
    shipped.eq(rust_shipped.clone(), c_shipped.clone(), || "shipped".into());
    // name by name too, so that a miss names the registration
    for (i, (r, c)) in rust_all.iter().zip(&c_all).enumerate() {
        all.eq(r.clone(), c.clone(), || format!("registration {i}"));
    }
    let mut swapped = rust_all.clone();
    swapped.swap(3, 4);
    swap_bug.eq(swapped, c_all.clone(), || "swapped".into());
    let mut dropped = rust_shipped.clone();
    dropped.pop();
    drop_bug.eq(dropped, c_shipped.clone(), || "dropped".into());
    // the not-shipped ones are exactly the ifdef'd ones: in the 205 and not in the 199
    let not_shipped: Vec<&Vec<u8>> = c_all.iter().filter(|n| !c_shipped.contains(n)).collect();
    println!("registrations: 205 / 199; compiled only with ALLOW_*: {:?}", not_shipped.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>());
    assert_eq!(not_shipped.len(), 6);
    all.finish();
    shipped.finish();
    swap_bug.finish_planted();
    drop_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// constants
// ---------------------------------------------------------------------------

#[test]
fn constants_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let rust: Vec<i32> = vec![
        MODULEINTERFACE_UPDATE,
        MODULEINTERFACE_DIE,
        MODULEINTERFACE_DAMAGE,
        MODULEINTERFACE_CREATE,
        MODULEINTERFACE_COLLIDE,
        MODULEINTERFACE_BODY,
        MODULEINTERFACE_CONTAIN,
        MODULEINTERFACE_UPGRADE,
        MODULEINTERFACE_SPECIAL_POWER,
        MODULEINTERFACE_DESTROY,
        MODULEINTERFACE_DRAW,
        MODULEINTERFACE_CLIENT_UPDATE,
        ModuleType::Behavior as i32,
        ModuleType::Draw as i32,
        ModuleType::ClientUpdate as i32,
        NUM_MODULE_TYPES,
        FIRST_DRAWABLE_MODULE_TYPE as i32,
        LAST_DRAWABLE_MODULE_TYPE as i32,
        NUM_DRAWABLE_MODULE_TYPES,
        UpdateModule::get_interface_mask(),
        UpgradeModule::get_interface_mask(),
        DieModule::get_interface_mask(),
        DamageModule::get_interface_mask(),
        CreateModule::get_interface_mask(),
        CollideModule::get_interface_mask(),
        BodyModule::get_interface_mask(),
        DestroyModule::get_interface_mask(),
        BehaviorModule::get_interface_mask(),
        BehaviorModule::get_module_type() as i32,
        UPDATE_SLEEP_INVALID.0 as i32,
        UPDATE_SLEEP_NONE.0 as i32,
        UPDATE_SLEEP_FOREVER.0 as i32,
        SleepyUpdatePhase::Initial as i32,
        SleepyUpdatePhase::Physics as i32,
        SleepyUpdatePhase::Normal as i32,
        SleepyUpdatePhase::Final as i32,
        FOREVER as i32,
    ];
    let mut c = Check::new("module constants (interfaces, types, phases)");
    let mut bug = Check::new("module constants");
    for (i, r) in rust.iter().enumerate() {
        let v = unsafe { cpp::ref_mod_const(i as i32) };
        c.eq(*r, v, || format!("constant {i}"));
        bug.eq(*r ^ 1, v, || format!("constant {i}"));
    }
    assert_eq!(unsafe { cpp::ref_mod_const(rust.len() as i32) }, -12345, "the list is complete");

    // BodyDamageType, MaxHealthChangeType and IS_CONDITION_*
    let mut out = [0i32; 64];
    let n = unsafe { cpp::ref_mod_body_enums(out.as_mut_ptr()) } as usize;
    let mut want = vec![BODY_PRISTINE, BODY_DAMAGED, BODY_REALLYDAMAGED, BODY_RUBBLE, BODYDAMAGETYPE_COUNT];
    want.extend([SAME_CURRENTHEALTH, PRESERVE_RATIO, ADD_CURRENT_HEALTH_TOO, FULLY_HEAL]);
    for a in 0..4 {
        for b in 0..4 {
            want.push(is_condition_worse(a, b) as i32);
            want.push(is_condition_better(a, b) as i32);
        }
    }
    let mut body = Check::new("BodyDamageType, IS_CONDITION_WORSE/BETTER");
    let mut body_bug = Check::new("IS_CONDITION_WORSE/BETTER");
    body.eq(want.clone(), out[..n].to_vec(), || "body enums".into());
    let mut swapped = want;
    swapped.swap(11, 12); // IS_CONDITION_WORSE(0, 1) and IS_CONDITION_BETTER(0, 1)
    body_bug.eq(swapped, out[..n].to_vec(), || "body enums".into());
    c.finish();
    bug.finish_planted();
    body.finish();
    body_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// module data
// ---------------------------------------------------------------------------

/// The ids of the effect lists, by first appearance, as the C++ numbers them.
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

/// `putMux` of the wrapper. `bug`: 0 none; 1 the masks swapped; 2 the trigger names and the activation names swapped.
fn put_mux(v: &mut Vec<u8>, m: &UpgradeMuxData, fx: &mut FxIds, bug: u32) {
    if bug == 2 {
        put_names(v, &m.activation_upgrade_names);
        put_names(v, &m.trigger_upgrade_names);
    } else {
        put_names(v, &m.trigger_upgrade_names);
        put_names(v, &m.activation_upgrade_names);
    }
    put_names(v, &m.conflicting_upgrade_names);
    put_names(v, &m.removal_upgrade_names);
    put_i32(v, fx.id(&m.fx_list_upgrade));
    if bug == 1 {
        put_words(v, m.conflicting_mask().words());
        put_words(v, m.activation_mask().words());
    } else {
        put_words(v, m.activation_mask().words());
        put_words(v, m.conflicting_mask().words());
    }
    v.push(u8::from(m.requires_all_triggers));
}

fn put_die_mux(v: &mut Vec<u8>, m: &DieMuxData, bug: u32) {
    put_u32(v, if bug == 1 { m.veterancy_levels } else { m.death_types });
    put_u32(v, if bug == 1 { m.death_types } else { m.veterancy_levels });
    put_words(v, m.exempt_status.words());
    put_words(v, m.required_status.words());
}

#[derive(Default)]
struct Slot {
    kind: i32,
    upgrade: Option<UpgradeModuleData>,
    die: Option<DieModuleData>,
}

fn read_block(ini: &mut Ini<'_>, s: &mut Slot) -> IniResult<()> {
    match s.kind {
        0 => {
            ModuleData::friend_new_module_data(Some(ini))?;
        }
        1 => {
            UpdateModuleData::friend_new_module_data(Some(ini))?;
        }
        2 => s.upgrade = Some(UpgradeModuleData::friend_new_module_data(Some(ini))?),
        3 => s.die = Some(DieModuleData::friend_new_module_data(Some(ini))?),
        4 => {
            DamageModuleData::friend_new_module_data(Some(ini))?;
        }
        5 => {
            CollideModuleData::friend_new_module_data(Some(ini))?;
        }
        6 => {
            ini.init_from_ini_multi_proc(&mut BehaviorModuleData::new(), BehaviorModuleData::build_field_parse)?;
        }
        7 => {
            ini.init_from_ini_multi_proc(&mut BodyModuleData::new(), BodyModuleData::build_field_parse)?;
        }
        _ => {
            ini.init_from_ini_multi_proc(&mut CreateModuleData::new(), CreateModuleData::build_field_parse)?;
        }
    }
    Ok(())
}

fn rust_read(kind: i32, name: &[u8], text: &[u8]) -> (IniResult<()>, Slot) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut slot = Slot { kind, ..Default::default() };
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut slot);
    (r, slot)
}

fn rust_data_dump(slot: &Slot, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    if let Some(u) = &slot.upgrade {
        put_mux(&mut v, &u.mux_data(), &mut FxIds::default(), bug);
    }
    if let Some(d) = &slot.die {
        put_die_mux(&mut v, &d.die_mux_data, bug);
    }
    v
}

fn detail_of(status: i32, msg: &[u8], code: u32) -> Vec<u8> {
    match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    }
}

fn cpp_data(kind: i32, name: &[u8], text: &[u8]) -> (Outcome, Vec<u8>) {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let mut buf = vec![0u8; 1 << 16];
    let (mut code, mut len) = (0u32, 0i32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
        cpp::ref_mod_data(kind, cname.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
    buf.truncate(len as usize);
    (Outcome { status, detail: detail_of(status, &msg, code), crc: 0 }, buf)
}

// -- generators --

const KIND_NAMES: [&str; 9] = ["Module", "Update", "Upgrade", "Die", "Damage", "Collide", "Behavior", "Body", "Create"];

fn case_pick(g: &mut Gen, s: &str) -> String {
    case_mix(g, s)
}

/// A block: header `D`, the lines, an end token (mostly `End`, rarely missing or damaged).
fn assemble_block(g: &mut Gen, lines: &[String], clean: bool) -> Vec<u8> {
    let mut s = String::from("D\n");
    for l in lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push_str(gen_comment(g));
        s.push('\n');
    }
    if clean {
        s.push_str("End\n");
    } else {
        match g.below(40) {
            0 => {}                                  // no End at all
            1 => s.push_str("Ends\n"),               // a field named Ends
            2 => s.push_str("; just a comment\n"),   // still no End
            _ => {
                s.push_str(&gen_end(g));
                s.push('\n');
            }
        }
    }
    s.into_bytes()
}

fn gen_junk_line(g: &mut Gen) -> String {
    let sep = gen_sep(g);
    match g.below(4) {
        0 => format!("Bogus{sep}1"),
        1 => format!("{}{sep}", pick(g, &["TriggeredBy", "Name", "Count"])),
        2 => "=".to_string(),
        _ => format!("Unknown{sep}x y z"),
    }
}

/// Names for the upgrade lists: the upgrades of the case (mostly), other-case and unknown spellings,
/// `None` (any case) and a random token.
fn gen_upgrade_list(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let n = 1 + g.below(3);
    let mut out = Vec::new();
    for _ in 0..n {
        let name = if clean && !upgrades.is_empty() {
            upgrades[g.below(upgrades.len() as u32) as usize].clone()
        } else {
            match g.below(100) {
                0..=59 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].clone(),
                60..=74 if !upgrades.is_empty() => {
                    let i = g.below(upgrades.len() as u32) as usize;
                    case_pick(g, &upgrades[i])
                }
                75..=87 => format!("Unknown{}", g.below(4)),
                88..=94 => case_pick(g, "None"),
                _ => format!("Up{}", g.below(10)),
            }
        };
        out.push(name);
    }
    out.join(pick(g, &[" ", "  ", " \t "]))
}

fn gen_bool_value(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

fn gen_upgrade_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            // (field tokens are case-sensitive: a changed case is an unknown field)
            let token = if clean || g.below(30) != 0 { name.to_string() } else { case_mix(g, name) };
            lines.push(format!("{token}{sep}{value}"));
        }
    };
    let v = gen_upgrade_list(g, upgrades, clean);
    field(g, 75, "TriggeredBy", v);
    let v = gen_upgrade_list(g, upgrades, clean);
    field(g, 40, "ConflictsWith", v);
    let v = gen_upgrade_list(g, upgrades, clean);
    field(g, 35, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None", "none", "fx_up"]).to_string();
    field(g, 35, "FXListUpgrade", v);
    let v = gen_bool_value(g, clean);
    field(g, 40, "RequiresAllTriggers", v);
    if !clean && g.below(25) == 0 {
        lines.push(gen_junk_line(g));
    }
    if g.below(10) == 0 {
        lines.reverse();
    }
    assemble_block(g, &lines, clean)
}

fn gen_death_tokens(g: &mut Gen) -> String {
    const NAMES: [&str; 20] = [
        "NONE", "CRUSHED", "BURNED", "EXPLODED", "POISONED", "TOPPLED", "FLOODED", "SUICIDED", "LASERED", "DETONATED", "SPLATTED",
        "POISONED_BETA", "EXTRA_2", "EXTRA_3", "EXTRA_4", "EXTRA_5", "EXTRA_6", "EXTRA_7", "EXTRA_8", "POISONED_GAMMA",
    ];
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        t.push(match g.below(10) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=6 => format!("+{}", NAMES[1 + g.below(19) as usize]),
            _ => format!("-{}", NAMES[1 + g.below(19) as usize]),
        });
    }
    t.join(" ")
}

fn gen_level_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        t.push(match g.below(8) {
            0 => "ALL".to_string(),
            1 => "NONE".to_string(),
            2..=4 => format!("+{}", pick(g, &["VETERAN", "ELITE", "HEROIC"])),
            _ => format!("-{}", pick(g, &["VETERAN", "ELITE", "HEROIC"])),
        });
    }
    t.join(" ")
}

fn gen_status_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        let n = OBJECT_STATUS_NAMES[1 + g.below(44) as usize];
        t.push(match g.below(10) {
            0 => "NONE".to_string(),
            1 => format!("-{n}"),
            2 => format!("+{n}"),
            _ => n.to_string(),
        });
    }
    t.join(" ")
}

fn gen_die_data(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            // (field tokens are case-sensitive: a changed case is an unknown field)
            let token = if clean || g.below(30) != 0 { name.to_string() } else { case_mix(g, name) };
            lines.push(format!("{token}{sep}{value}"));
        }
    };
    let v = gen_death_tokens(g);
    field(g, 70, "DeathTypes", v);
    let v = gen_level_tokens(g);
    field(g, 50, "VeterancyLevels", v);
    let v = gen_status_tokens(g);
    field(g, 40, "ExemptStatus", v);
    let v = gen_status_tokens(g);
    field(g, 40, "RequiredStatus", v);
    if !clean && g.below(12) == 0 {
        let junk = match g.below(4) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "VeterancyLevels = WHAT".to_string(),
            2 => "ExemptStatus = NOT_A_STATUS".to_string(),
            _ => gen_junk_line(g),
        };
        lines.push(junk);
    }
    assemble_block(g, &lines, clean)
}

/// A block for a data class with no fields of its own: nothing, junk, a damaged End.
fn gen_empty_data(g: &mut Gen) -> Vec<u8> {
    let mut lines = Vec::new();
    for _ in 0..g.below(3) {
        lines.push(gen_junk_line(g));
    }
    let clean = lines.is_empty() && g.below(2) == 0;
    assemble_block(g, &lines, clean)
}

#[test]
fn module_data_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC4A1);
    let mut outcome = Check::new("module data INI::load (throw, message)");
    let mut dumps = Check::new("UpgradeMuxData / DieMuxData as read");
    let mut outcome_bug = Check::new("module data INI::load (throw, message)");
    let mut mux_bug = Check::new("UpgradeMuxData as read");
    let mut die_bug = Check::new("DieMuxData as read");
    let mut excluded = Excluded::default();
    let (mut ok, mut failed) = (0u64, 0u64);
    let mut per_kind = [0u64; 9];
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let kind = (case % 9) as i32;
        let clean = g.below(10) < 4;
        let upgrades: Vec<String> = (0..g.below(5)).map(|i| format!("Up{i}")).collect();
        let text = match kind {
            2 => gen_upgrade_data(&mut g, &upgrades, clean),
            3 => gen_die_data(&mut g, clean),
            _ => gen_empty_data(&mut g),
        };
        let name = format!("Data\\INI\\Mod{}.ini", case % 5).into_bytes();
        let (r, slot) = rust_read(kind, &name, &text);
        let rust = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        if rust.status == 0 {
            ok += 1;
        } else {
            failed += 1;
        }
        per_kind[kind as usize] += 1;
        let (c, cdump) = cpp_data(kind, &name, &text);
        let what = || format!("case {case}: {} data, file {:?}", KIND_NAMES[kind as usize], show(&text));
        outcome.eq(&rust, &c, what);
        outcome_bug.eq(&Outcome { status: rust.status ^ 1, detail: rust.detail.clone(), crc: 0 }, &c, what);
        if rust.status == 0 {
            dumps.eq(rust_data_dump(&slot, 0), cdump.clone(), what);
            if kind == 2 {
                mux_bug.eq(rust_data_dump(&slot, 2), cdump.clone(), what);
            }
            if kind == 3 {
                die_bug.eq(rust_data_dump(&slot, 1), cdump, what);
            }
        }
    }
    println!(
        "module data: {CASES} blocks, {ok} read, {failed} threw; per kind {per_kind:?}; excluded (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    assert!(ok > 1200 && failed > 1200, "the generator must reach both outcomes: {ok} read, {failed} threw");
    outcome.finish();
    dumps.finish();
    outcome_bug.finish_planted();
    mux_bug.finish_planted();
    die_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// UpgradeMux
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Impl,
    Fx(Vec<u8>),
    Remove(Vec<u8>),
}

#[derive(Default)]
struct Rec {
    events: Vec<Event>,
}
impl UpgradeEffects for Rec {
    fn upgrade_implementation(&mut self) {
        self.events.push(Event::Impl);
    }
    fn do_fx_obj(&mut self, fx: &[u8]) {
        self.events.push(Event::Fx(fx.to_vec()));
    }
    fn remove_upgrade(&mut self, t: &rts_lockstep_engine::common::upgrade::UpgradeTemplate) {
        self.events.push(Event::Remove(t.upgrade_name().to_vec()));
    }
}

const NAME_POOL_EXTRA: [&str; 4] = ["Nope", "UP0", "up1", "None"];

fn key_of(words: [u32; 4]) -> UpgradeMaskType {
    UpgradeMaskType::from_words([u64::from(words[0]) | u64::from(words[1]) << 32, u64::from(words[2]) | u64::from(words[3]) << 32, 0, 0])
}

/// One op: [op, m0, m1, m2, m3]. The keys mostly grow, as an object that is given upgrades one after another.
fn gen_upgrade_ops(g: &mut Gen, ctx: &Ctx, n_names: usize) -> Vec<[u32; 5]> {
    let masks: Vec<[u64; 4]> = ctx.center.templates().map(|t| t.upgrade_mask().words()).collect();
    let mut acc = [0u64; 4];
    (0..2 + g.below(12))
        .map(|_| {
            let op = match g.below(100) {
                0..=14 => 0,
                15..=44 => 1,
                45..=54 => 2,
                55..=64 => 3,
                65..=69 => 4,
                70..=79 => 5,
                _ => 6,
            };
            if op == 6 {
                return [6, g.below(n_names as u32), 0, 0, 0];
            }
            // another upgrade is given
            if !masks.is_empty() && g.below(10) < 6 {
                let m = masks[g.below(masks.len() as u32) as usize];
                for i in 0..4 {
                    acc[i] |= m[i];
                }
            }
            let mut w = acc;
            match g.below(10) {
                0 => w = [0; 4],
                1 | 2 => {
                    w = [0; 4];
                    for m in &masks {
                        if g.below(5) < 2 {
                            for i in 0..4 {
                                w[i] |= m[i];
                            }
                        }
                    }
                }
                3 => w[g.below(2) as usize] |= 1 << g.below(64),
                _ => {}
            }
            [op, w[0] as u32, (w[0] >> 32) as u32, w[1] as u32, (w[1] >> 32) as u32]
        })
        .collect()
}

/// The Rust twin of `ref_mod_upgrade_run`'s ops. `Err` for an op that is undefined upstream. `bug`: 0 none, 1 the
/// effect order reversed, 2 `isAlreadyUpgraded` read before the op, 3 results inverted for `wouldUpgrade`.
fn rust_upgrade_ops(ctx: &mut Ctx, module: &mut UpgradeModule, ops: &[[u32; 5]], pool: &[String], bug: u32) -> Result<Vec<u8>, IniError> {
    let mut out = Vec::new();
    let mut fx = FxIds::default();
    for o in ops {
        let key = key_of([o[1], o[2], o[3], o[4]]);
        let mut rec = Rec::default();
        let was = module.is_already_upgraded();
        let r: IniResult<bool> = match o[0] {
            0 => module.would_upgrade(&ctx.center, &mut ctx.names, key).map(|b| b ^ (bug == 3)),
            1 => module.attempt_upgrade(&ctx.center, &mut ctx.names, &mut rec, key),
            2 => module.test_upgrade_conditions(&ctx.center, &mut ctx.names, key),
            3 => module.reset_upgrade(&ctx.center, &mut ctx.names, key),
            4 => {
                module.force_refresh_upgrade(&mut rec);
                Ok(false)
            }
            5 => Ok(module.is_already_upgraded()),
            _ => Ok(module.is_triggered_by(pool[o[1] as usize].as_bytes())),
        };
        let (st, res) = match r {
            Ok(b) => (0, b),
            Err(e) => match e.thrown() {
                Some(rts_lockstep_engine::common::ini::Thrown::Status(_)) => (2, false),
                _ => return Err(e),
            },
        };
        put_i32(&mut out, st);
        out.push(u8::from(res));
        out.push(u8::from(if bug == 2 { was } else { module.is_already_upgraded() }));
        if bug == 1 {
            rec.events.reverse();
        }
        put_u32(&mut out, rec.events.len() as u32);
        for e in &rec.events {
            match e {
                Event::Impl => put_str(&mut out, b"impl"),
                Event::Fx(n) => put_str(&mut out, format!("fx{}", fx.id(&Some(n.clone()))).as_bytes()),
                Event::Remove(n) => put_str(&mut out, &[b"remove ".as_slice(), n].concat()),
            }
        }
        put_mux(&mut out, &module.upgrade_module_data().mux_data_snapshot(), &mut fx, 0);
    }
    Ok(out)
}

#[test]
fn upgrade_mux_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC4A2);
    let table = blocks();
    let probe = name_probe();
    let mut data_out = Check::new("UpgradeModuleData read (throw, message)");
    let mut ops_out = Check::new("UpgradeMux ops (result, flag, effects, mux data)");
    let mut keys = Check::new("name keys after the ops");
    let mut order_bug = Check::new("UpgradeMux effect order");
    let mut flag_bug = Check::new("UpgradeMux executed flag");
    let mut result_bug = Check::new("UpgradeMux wouldUpgrade result");
    let mut keys_bug = Check::new("name keys after the ops");
    let mut excluded = Excluded::default();
    let (mut ops_run, mut fired, mut resets, mut read_failed) = (0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        // the upgrades of this case, defined on both sides by Upgrade blocks (the order of the blocks is the mask bits)
        let n_up = g.below(9);
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
        let new_ctx = || {
            let mut ctx = Ctx::new(init_names, init_center);
            Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("the upgrade definitions load");
            ctx
        };
        let mut ctx = new_ctx();
        cpp_reset(init_names, init_center);
        let c = cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false);
        assert_eq!(c.status, 0);

        let clean = g.below(10) < 7;
        let text = gen_upgrade_data(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Mux{}.ini", case % 5).into_bytes();
        let (r, slot) = rust_read(2, &name, &text);
        let read_outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        // the pool of names for isTriggeredBy
        let mut pool: Vec<String> = upgrades.clone();
        pool.extend(NAME_POOL_EXTRA.iter().map(|s| s.to_string()));
        let ops = gen_upgrade_ops(&mut g, &ctx, pool.len());

        let mut rust_dump = None;
        if read_outcome.status == 0 {
            let data = Arc::new(slot.upgrade.expect("read ok"));
            let mut m = UpgradeModule::new(7, Some(data)).unwrap();
            match rust_upgrade_ops(&mut ctx, &mut m, &ops, &pool, 0) {
                Ok(d) => rust_dump = Some(d),
                Err(e) => {
                    assert!(e.is_undefined(), "{e}");
                    excluded.add(&e);
                    continue;
                }
            }
            fired += u64::from(m.is_already_upgraded());
            resets += ops.iter().filter(|o| o[0] == 3).count() as u64;
            ops_run += ops.len() as u64;
        } else {
            read_failed += 1;
        }

        // the C++ side
        let cfile = CString::new(name.clone()).unwrap();
        let cnames: Vec<CString> = pool.iter().map(|s| CString::new(s.as_str()).unwrap()).collect();
        let nptrs: Vec<*const std::os::raw::c_char> = cnames.iter().map(|c| c.as_ptr()).collect();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_mod_upgrade_run(
                cfile.as_ptr(),
                ops.len() as i32,
                flat.as_ptr(),
                nptrs.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut len,
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut code,
            )
        };
        buf.truncate(len as usize);
        let c_read = Outcome { status, detail: detail_of(status, &msg, code), crc: 0 };
        let what = || format!("case {case}: names init {init_names} center {init_center} upgrades {upgrades:?}, file {:?}, ops {ops:?}", show(&text));
        data_out.eq(&read_outcome, &c_read, what);
        let Some(rd) = rust_dump else { continue };
        ops_out.eq(&rd, &buf, what);
        // planted wrong variants of the Rust side, each on a fresh context (the ops change the name keys)
        for (bug, check) in [(1u32, &mut order_bug), (2, &mut flag_bug), (3, &mut result_bug)] {
            let mut ctx2 = new_ctx();
            let (r2, slot2) = rust_read(2, &name, &text);
            r2.unwrap();
            let mut m2 = UpgradeModule::new(7, Some(Arc::new(slot2.upgrade.unwrap()))).unwrap();
            let planted = rust_upgrade_ops(&mut ctx2, &mut m2, &ops, &pool, bug).unwrap();
            check.eq(&planted, &buf, what);
        }
        // the keys nameToKey now gives to the names
        let rk: Vec<u32> = probe.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
        let ck: Vec<u32> = probe.iter().map(|n| cpp_name_key(n)).collect();
        keys.eq(rk.clone(), ck.clone(), what);
        keys_bug.eq(vec![rk[0] ^ 1], vec![ck[0]], what);
    }
    println!(
        "UpgradeMux: {CASES} modules, {read_failed} data blocks threw, {ops_run} ops, {fired} modules fired, {resets} reset ops; excluded (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    assert!(read_failed > 300 && ops_run > 15_000 && fired > 600, "the generator must reach every path");
    data_out.finish();
    ops_out.finish();
    keys.finish();
    order_bug.finish_planted();
    flag_bug.finish_planted();
    result_bug.finish_planted();
    keys_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// UpdateModule
// ---------------------------------------------------------------------------

fn gen_frame(g: &mut Gen) -> u32 {
    match g.below(12) {
        0 => 0,
        1 => 1,
        2 => g.below(10),
        3 => g.below(200),
        4 => 0x3fff_ffff,
        5 => 0x3fff_fffe,
        6 => 0x4000_0000,
        7 => u32::MAX,
        8 => g.u32(),
        _ => g.below(100_000),
    }
}

/// A frame for `frameToSleepTime`'s explicit arguments: never above FOREVER (the result is an `UpdateSleepTime`,
/// an enum whose range ends there).
fn gen_sleep_frame(g: &mut Gen, now: u32) -> u32 {
    match g.below(6) {
        0 | 1 => FOREVER,
        2 => now.saturating_add(g.below(5)).min(FOREVER),
        3 => now.saturating_sub(g.below(5)),
        4 => g.below(FOREVER + 1),
        _ => g.below(300),
    }
}

/// [op, a, b, c, d, e]
fn gen_update_ops(g: &mut Gen) -> Vec<[u32; 6]> {
    (0..1 + g.below(14))
        .map(|_| {
            let op = match g.below(100) {
                0..=19 => 0,
                20..=34 => 1,
                35..=44 => 2,
                45..=59 => 3,
                60..=69 => 4,
                70..=74 => 5,
                75..=79 => 6,
                80..=87 => 7,
                88..=92 => 8,
                _ => 9,
            };
            let a = gen_frame(g);
            match op {
                3 => [3, a, gen_sleep_frame(g, a), gen_sleep_frame(g, a), gen_sleep_frame(g, a), gen_sleep_frame(g, a)],
                4 => [4, a, g.below(FOREVER + 1), 0, 0, 0],
                5 => [5, g.i32() as u32, 0, 0, 0, 0],
                7 => [7, g.u32(), 0, 0, 0, 0],
                9 => [9, g.u32(), [0, 1, 1, 2, 3, 255][g.below(6) as usize], 0, 0, 0],
                _ => [op, a, 0, 0, 0, 0],
            }
        })
        .collect()
}

/// The Rust twin of `ref_mod_update_run`. `bug`: 0 none, 1 the phase bits ignored in `friend_setNextCallFrame`,
/// 2 `getWakeFrame` off by one.
fn rust_update_ops(phase: SleepyUpdatePhase, ops: &[[u32; 6]], bug: u32) -> Vec<u32> {
    let mut m = UpdateModule::new(5, Some(Arc::new(UpdateModuleData::new())), phase).unwrap();
    let mut out = Vec::new();
    let mut awakened = 0u32;
    let mut last_awaken = 0u32;
    for o in ops {
        match o[0] {
            0 => {
                m.friend_set_next_call_frame(o[1]);
                let p = m.friend_get_priority();
                out.push(if bug == 1 { p & !3 } else { p });
            }
            1 => out.extend([m.friend_get_priority(), m.friend_get_next_call_frame(), m.friend_get_next_call_phase() as u32, m.friend_get_index_in_logic() as u32]),
            2 => out.push(m.get_wake_frame(o[1]).0 + u32::from(bug == 2)),
            3 => out.push(UpdateModule::frame_to_sleep_time(o[1], o[2], o[3], o[4], o[5]).0),
            4 => {
                awakened += 1;
                last_awaken = UpdateModule::set_wake_frame(o[1], UpdateSleepTime(o[2]));
                out.extend([awakened, last_awaken, 1, 1]);
            }
            5 => m.friend_set_index_in_logic(o[1] as i32),
            6 => {
                let mut x = XferSave::new();
                x.open(b"rust").unwrap();
                let st = status_of(x.xfer_snapshot(&mut m));
                let _ = x.close();
                out.push(st as u32);
                out.push(x.bytes().len() as u32);
                out.extend(x.bytes().iter().map(|&b| u32::from(b)));
            }
            7 | 9 => {
                let mut bytes = vec![if o[0] == 9 { o[2] as u8 } else { 1 }, 1, 1, 1];
                bytes.extend_from_slice(&o[1].to_le_bytes());
                let mut x = XferLoad::new(bytes);
                x.open(b"rust").unwrap();
                let st = status_of(x.xfer_snapshot(&mut m));
                if o[0] == 9 {
                    out.push(st as u32);
                }
                out.extend([m.friend_get_priority(), m.friend_get_index_in_logic() as u32]);
            }
            _ => {
                let mut x = XferCrc::new_without_nan_guard();
                x.open(b"crc").unwrap();
                let st = status_of(x.xfer_snapshot(&mut m));
                out.extend([st as u32, x.get_crc()]);
            }
        }
    }
    let _ = last_awaken;
    out
}

#[test]
fn update_module_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC4A3);
    let mut ops_out = Check::new("UpdateModule sleepy-update ops");
    let mut phase_bug = Check::new("UpdateModule phase bits");
    let mut wake_bug = Check::new("UpdateModule getWakeFrame");
    let path = tmp_path("update");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    let (mut n_ops, mut loads_failed) = (0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let phase = g.below(4) as i32;
        let ops = gen_update_ops(&mut g);
        n_ops += ops.len() as u64;
        let rust = rust_update_ops(SleepyUpdatePhase::from_low_bits(phase as u32), &ops, 0);
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut out = vec![0u32; 1 << 12];
        let n = unsafe { cpp::ref_mod_update_run(phase, ops.len() as i32, flat.as_ptr(), out.as_mut_ptr(), out.len() as i32, cpath.as_ptr()) } as usize;
        assert!(n <= out.len());
        out.truncate(n);
        let what = || format!("case {case}: phase {phase}, ops {ops:?}");
        ops_out.eq(&rust, &out, what);
        phase_bug.eq(&rust_update_ops(SleepyUpdatePhase::from_low_bits(phase as u32), &ops, 1), &out, what);
        wake_bug.eq(&rust_update_ops(SleepyUpdatePhase::from_low_bits(phase as u32), &ops, 2), &out, what);
        loads_failed += ops.iter().filter(|o| o[0] == 9 && o[2] > 1).count() as u64;
    }
    let _ = std::fs::remove_file(&path);
    println!("UpdateModule: {CASES} sequences, {n_ops} ops, {loads_failed} loads of a too-new version");
    assert!(n_ops > 40_000 && loads_failed > 500);
    ops_out.finish();
    // (the plants must be caught only when the sequence touches what they change)
    phase_bug.finish_planted();
    wake_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// DieMuxData
// ---------------------------------------------------------------------------

#[test]
fn die_mux_data_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC4A4);
    let mut applicable = Check::new("DieMuxData::isDieApplicable (data, module)");
    let mut applicable_bug = Check::new("DieMuxData::isDieApplicable");
    let mut data_out = Check::new("DieModuleData read (throw, message)");
    let (mut yes, mut no, mut read_failed) = (0u64, 0u64, 0u64);
    const CASES: usize = 4_000;
    for case in 0..CASES {
        let clean = g.below(10) < 6;
        let text = gen_die_data(&mut g, clean);
        let name = format!("Data\\INI\\Die{}.ini", case % 5).into_bytes();
        let (r, slot) = rust_read(3, &name, &text);
        let ro = rust_outcome(r, 0).expect("no undefined input in the die generator");
        let queries: Vec<[u32; 4]> = (0..16)
            .map(|_| {
                let status = g.u64() & g.u64() & ((1u64 << 45) - 1);
                [1 + g.below(20), 1 + g.below(3), status as u32, (status >> 32) as u32]
            })
            .collect();
        let flat: Vec<u32> = queries.iter().flatten().copied().collect();
        let cfile = CString::new(name.clone()).unwrap();
        let mut out = vec![0u8; queries.len()];
        let mut msg = vec![0u8; 4096];
        let mut code = 0u32;
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_mod_die_run(cfile.as_ptr(), queries.len() as i32, flat.as_ptr(), out.as_mut_ptr(), msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
        };
        let co = Outcome { status, detail: detail_of(status, &msg, code), crc: 0 };
        let what = || format!("case {case}: file {:?}", show(&text));
        data_out.eq(&ro, &co, what);
        if ro.status != 0 {
            read_failed += 1;
            continue;
        }
        let data = Arc::new(slot.die.expect("read ok"));
        let module = DieModule::new(3, Some(data.clone())).unwrap();
        for (q, c) in queries.iter().zip(&out) {
            let st = ObjectStatusMaskType::from_words([u64::from(q[2]) | u64::from(q[3]) << 32, 0, 0, 0]);
            let a = data.is_die_applicable(q[0] as i32, q[1] as i32, &st);
            let b = module.is_die_applicable(q[0] as i32, q[1] as i32, &st);
            let rust = u8::from(a) | u8::from(b) << 1;
            applicable.eq(rust, *c, || format!("{} query {q:?}", what()));
            applicable_bug.eq(rust ^ 1, *c, || format!("{} query {q:?}", what()));
            if a {
                yes += 1
            } else {
                no += 1
            }
        }
    }
    println!("DieMuxData: {CASES} modules, {read_failed} data blocks threw; {yes} applicable, {no} not");
    assert!(yes > 5_000 && no > 5_000);
    data_out.finish();
    applicable.finish();
    applicable_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// the base classes: constructors, xfer chains, defaults
// ---------------------------------------------------------------------------

const CLASS_NAMES: [&str; 11] =
    ["ObjectModule", "DrawableModule", "BehaviorModule", "UpdateModule", "UpgradeModule", "DieModule", "CreateModule", "BodyModule", "DamageModule", "CollideModule", "DestroyModule"];

/// A Rust module of the base class `cls` (as `ref_mod_construct` numbers them) made on data of its kind.
fn rust_base(cls: i32, null_data: bool) -> IniResult<Module> {
    let behavior_data = || Some(ModuleDataKind::Module(Arc::new(ModuleData::new())));
    let d = |k: ModuleDataKind| if null_data { None } else { Some(k) };
    Ok(match cls {
        0 => Module::Object(ObjectModule::new(1, if null_data { None } else { behavior_data() })?),
        1 => Module::Drawable(DrawableModule::new(if null_data { None } else { behavior_data() })?),
        2 => Module::Behavior(BehaviorModule::new(1, if null_data { None } else { behavior_data() })?),
        3 => Module::Update(UpdateModule::new(1, if null_data { None } else { Some(Arc::new(UpdateModuleData::new())) }, SleepyUpdatePhase::Normal)?),
        4 => Module::Upgrade(UpgradeModule::new(1, if null_data { None } else { Some(Arc::new(UpgradeModuleData::new())) })?),
        5 => Module::Die(DieModule::new(1, if null_data { None } else { Some(Arc::new(DieModuleData::new())) })?),
        6 => Module::Create(CreateModule::new(1, if null_data { None } else { Some(Arc::new(CreateModuleData::new())) })?),
        7 => Module::Body(BodyModule::new(1, if null_data { None } else { Some(Arc::new(BodyModuleData::new())) })?),
        8 => Module::Damage(DamageModule::new(1, if null_data { None } else { Some(Arc::new(DamageModuleData::new())) })?),
        9 => Module::Collide(CollideModule::new(1, if null_data { None } else { Some(Arc::new(CollideModuleData::new())) })?),
        _ => Module::Destroy(DestroyModule::new(1, d(ModuleDataKind::Module(Arc::new(ModuleData::new()))))?),
    })
}

#[test]
fn base_class_constructors_check_their_data() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("base class constructors (null data throws)");
    let mut bug = Check::new("base class constructors");
    for cls in 0..11 {
        for null in [false, true] {
            let r = rust_base(cls, null);
            let rust = match &r {
                Ok(_) => 0,
                Err(e) => {
                    assert_eq!(e.kind, IniErrorKind::InvalidData);
                    2
                }
            };
            let cpp_status = unsafe { cpp::ref_mod_construct(cls, i32::from(null)) };
            c.eq(rust, cpp_status, || format!("{} null data {null}", CLASS_NAMES[cls as usize]));
            bug.eq(rust ^ 2, cpp_status, || format!("{} null data {null}", CLASS_NAMES[cls as usize]));
        }
    }
    c.finish();
    bug.finish_planted();
}

fn mutate(g: &mut Gen, mut file: Vec<u8>) -> Vec<u8> {
    match g.below(8) {
        1 => {
            let p = g.below(file.len() as u32) as usize;
            file[p] = g.below(6) as u8; // a version byte
        }
        2 => {
            let p = g.below(file.len() as u32) as usize;
            file[p] = g.u32() as u8;
        }
        3 => file.truncate(file.len().saturating_sub(1 + g.below(6) as usize)),
        4 => file.extend((0..1 + g.below(5)).map(|_| g.u32() as u8)),
        _ => {}
    }
    file
}

/// What the load into a CreateModule / BodyModule left: the flag, the scalar's bits.
fn state_of(m: &Module) -> u32 {
    match m {
        Module::Create(c) => u32::from(c.should_do_on_build_complete()),
        Module::Body(b) => b.get_damage_scalar().to_bits(),
        _ => 0,
    }
}

#[test]
fn base_class_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC4A5);
    let path = tmp_path("base");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    let mut save = Check::new("base class xfer save (bytes, status)");
    let mut load = Check::new("base class xfer load (status, state)");
    let mut crc = Check::new("base class xfer CRC");
    let mut save_bug = Check::new("base class xfer save");
    let mut load_bug = Check::new("base class xfer load");
    let mut crc_bug = Check::new("base class xfer CRC");
    let (mut load_ok, mut load_failed) = (0u64, 0u64);
    const CASES: usize = 4_400;
    for case in 0..CASES {
        let cls = (case % 11) as i32;
        let prep = i32::from(g.below(2) == 0);
        let scale = [1.0f32, 0.5, 2.0, 0.0, -1.0, 1.0e10, f32::from_bits(g.u32() & 0x7f7f_ffff)][g.below(7) as usize];
        let mut m = rust_base(cls, false).unwrap();
        if let Module::Create(c) = &mut m {
            if prep != 0 {
                c.on_build_complete();
            }
        }
        if let Module::Body(b) = &mut m {
            b.apply_damage_scalar(scale);
        }
        // save
        let mut cbytes = vec![0u8; 64];
        let (mut clen, mut cstate, mut ccrc) = (0i32, 0u32, 0u32);
        let cst = unsafe { cpp::ref_mod_base_xfer(cls, prep, scale, 0, cpath.as_ptr(), cbytes.as_mut_ptr(), 64, &mut clen, &mut ccrc, &mut cstate) };
        cbytes.truncate(clen as usize);
        let mut xs = XferSave::new();
        xs.open(b"rust").unwrap();
        let rst = status_of(xs.xfer_snapshot(&mut m));
        let _ = xs.close();
        let rbytes = xs.bytes().to_vec();
        save.eq((rst, &rbytes), (cst, &cbytes), || format!("case {case}: {} prep {prep} scale {scale}", CLASS_NAMES[cls as usize]));
        let mut planted = rbytes.clone();
        planted[0] ^= 1;
        save_bug.eq((rst, &planted), (cst, &cbytes), || format!("case {case}"));

        // load a saved or damaged file into a fresh module
        let mut file = mutate(&mut g, cbytes.clone());
        if file.is_empty() {
            file.push(1);
        }
        let mut cstate = 0u32;
        let cst = unsafe {
            cpp::ref_mod_base_xfer(cls, 0, 1.0, 1, cpath.as_ptr(), file.as_mut_ptr(), file.len() as i32, &mut 0, &mut ccrc, &mut cstate)
        };
        let mut r = rust_base(cls, false).unwrap();
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"rust").unwrap();
        let rst = status_of(xl.xfer_snapshot(&mut r));
        // (a failed load can leave C++ with a partial read; compare the state only after a good one)
        // (a Bool byte above 1 is stored raw by xferBool and read back as a bool: undefined in C++; the status only)
        let raw_bool = cls == 6 && file.last().is_some_and(|b| *b > 1);
        if rst == 0 && !raw_bool {
            load_ok += 1;
            load.eq((rst, state_of(&r)), (cst, cstate), || format!("case {case}: {} file {file:?}", CLASS_NAMES[cls as usize]));
            load_bug.eq((rst, state_of(&r) ^ 1), (cst, cstate), || format!("case {case}"));
        } else if rst == 0 {
            load_ok += 1;
            load.eq(rst, cst, || format!("case {case}: {} file {file:?}", CLASS_NAMES[cls as usize]));
            load_bug.eq(rst + 1, cst, || format!("case {case}"));
        } else {
            load_failed += 1;
            load.eq((rst, 0), (cst, 0), || format!("case {case}: {} file {file:?}", CLASS_NAMES[cls as usize]));
            load_bug.eq((rst + 1, 0), (cst, 0), || format!("case {case}"));
        }

        // CRC
        let mut ccrc = 7u32;
        let cst = unsafe { cpp::ref_mod_base_xfer(cls, prep, scale, 2, cpath.as_ptr(), std::ptr::null_mut(), 0, &mut 0, &mut ccrc, &mut 0) };
        let mut xc = XferCrc::new_without_nan_guard();
        xc.open(b"crc").unwrap();
        let rst = status_of(xc.xfer_snapshot(&mut m));
        crc.eq((rst, xc.get_crc()), (cst, ccrc), || format!("case {case}: {}", CLASS_NAMES[cls as usize]));
        crc_bug.eq((rst, xc.get_crc() ^ 1), (cst, ccrc), || format!("case {case}"));
    }
    let _ = std::fs::remove_file(&path);
    println!("base class xfer: {CASES} modules over 11 classes; loads {load_ok} ok, {load_failed} failed");
    assert!(load_ok > 2500 && load_failed > 400);
    save.finish();
    load.finish();
    crc.finish();
    save_bug.finish_planted();
    load_bug.finish_planted();
    crc_bug.finish_planted();
}

#[test]
fn body_and_collide_defaults_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut b = BodyModule::new(1, Some(Arc::new(BodyModuleData::new()))).unwrap();
    let r = |x: f32| x.to_bits();
    let mut rust: Vec<u32> = vec![
        r(b.get_max_health()),
        r(b.get_previous_health()),
        b.get_subdual_damage_heal_rate(),
        r(b.get_subdual_damage_heal_amount()),
        u32::from(b.has_any_subdual_damage()),
        r(b.get_current_subdual_damage_amount()),
        r(b.get_initial_health()),
        u32::from(b.get_last_damage_info_is_present()),
        b.get_last_damage_timestamp(),
        b.get_last_healing_timestamp(),
        b.get_clearable_last_attacker() as u32,
        u32::from(b.get_front_crushed()),
        u32::from(b.get_back_crushed()),
        u32::from(b.is_indestructible()),
    ];
    b.set_indestructible(false);
    rust.push(u32::from(b.is_indestructible()));
    rust.push(r(b.get_damage_scalar()));
    b.set_max_health(10.0, FULLY_HEAL);
    b.set_initial_health(50);
    rust.push(r(b.get_max_health()));
    b.set_front_crushed(true);
    rust.push(u32::from(b.get_front_crushed()));
    assert_eq!(INVALID_ID, 0);
    let mut out = [0u32; 32];
    let n = unsafe { cpp::ref_mod_body_defaults(out.as_mut_ptr()) } as usize;
    let mut c = Check::new("BodyModule defaults");
    let mut bug = Check::new("BodyModule defaults");
    c.eq(rust.clone(), out[..n].to_vec(), || "body".into());
    let mut planted = rust;
    planted[13] ^= 1;
    bug.eq(planted, out[..n].to_vec(), || "body".into());

    let col = CollideModule::new(1, Some(Arc::new(CollideModuleData::new()))).unwrap();
    let rust = vec![
        u32::from(col.would_like_to_collide_with()),
        u32::from(col.is_hijacked_vehicle_crate_collide()),
        u32::from(col.is_sabotage_building_crate_collide()),
        u32::from(col.is_car_bomb_crate_collide()),
        u32::from(col.is_railroad()),
        u32::from(col.is_salvage_crate_collide()),
    ];
    let n = unsafe { cpp::ref_mod_collide_defaults(out.as_mut_ptr()) } as usize;
    let mut cc = Check::new("CollideModule defaults");
    let mut cbug = Check::new("CollideModule defaults");
    cc.eq(rust.clone(), out[..n].to_vec(), || "collide".into());
    let mut planted = rust;
    planted[4] = 1;
    cbug.eq(planted, out[..n].to_vec(), || "collide".into());
    c.finish();
    bug.finish_planted();
    cc.finish();
    cbug.finish_planted();
    let _ = NameKeyType::INVALID;
    let _ = Undefined::UninitialisedModuleTag;
}

