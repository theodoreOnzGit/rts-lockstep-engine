// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/{DamDie, SpecialPowerCompletionDie, EjectPilotDie}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! Three layer-9 die modules (`logic::modules::{dam_die, special_power_completion_die, eject_pilot_die}`, over `modules::die_body`) against the
//! unchanged C++, `p1w11-sonnet`, die batch D.
//!
//! # Method
//!
//! The C++ side is the three `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_dies_d.inc`) on the module build's
//! stand-ins (`differential/shim_module`): a game logic whose object list holds the object under test, up to four others and a damage dealer (newest
//! first), each with disabled bits and `KINDOF_WAVEGUIDE` or not; an `Object::clearDisabled` that logs; a script engine that logs the completed special
//! power; the object creation lists `OCL_A` and `OCL_B` (`l9_upgrades_b.inc`: `ocl N`, and `oclsec <id>` when a damage dealer is passed); a template
//! with the per-unit sounds the case gives it; an audio manager that logs the events (name, position, player index); the REAL `DieModule`, `INI` and,
//! for `SpecialPowerCompletionDie`, the REAL `SpecialPowerStore` of `wrap_upgrade.cpp` behind `INI::parseSpecialPowerTemplate` (the lookup of the
//! stand-in store in `wrap_ini.cpp` is bound to it while the data is read).
//!
//! The Rust side is the real thing: a `World` whose statics hold the special power store and a `ThingFactory` read from an `Object` block (the dying
//! object's template, with its `UnitSpecificSounds`), the object under test made by `World::new_object` with the module from `modules::new_module`,
//! the others made the same way, and every op sent through `World::module_call` (`OnDie`, `SetCreator`) or run on the module's body (the xfers).
//!
//! Each case defines 0 to 3 special powers, reads a `D` block (the die mux fields and the module's own, mostly well formed), and when the read
//! succeeds runs 1 to 14 ops: `onDie` (a death type, a veterancy level, status bits, a damage source that is the dealer, nobody or no object),
//! `setCreator`, an xfer save, load of the last save and CRC, the object's position, its controlling player, and the disabled bits of another object.
//!
//! Compared, bit for bit: the read's throw; the data (the die mux, the special power template's id, the object creation lists and the time); per
//! op the throw status, the effects in order (`cleardisabled`, `script`, `ocl`, `oclsec`, `audio`) and the xfer status, bytes and CRC. (The disabled
//! bits themselves are `Object::clearDisabled`'s, which the C++ stand-in only logs.)
//!
//! `onDie` is sent death types 1 to 20 and veterancy levels 1 to 3 only: `getDeathTypeFlag` and `getVeterancyLevelFlag` shift `1UL` by `dt - 1`, which
//! is -1 for `DEATH_NONE` and `LEVEL_REGULAR`; MSVC's 32-bit `unsigned long` makes that bit 31 (what `common::game_common` models, and what the
//! Rust gives), and the C++ here is built for a 64-bit `unsigned long` (clang, LP64), where the same expression is bit 63. The comparison would
//! measure the compiler's `long`, not the translation.
//!
//! Inputs for which upstream is undefined (`SpecialPowerCompletionDie` with a creator set on an object with no controlling player or with a name the
//! store does not know; `EjectPilotDie` ejecting from an object with no controlling player) are never sent to the C++: the Rust records an
//! `is_undefined()` error and the case is counted as excluded.
//!
//! # Deviation from upstream that the test cannot see
//!
//! The special power template of `SpecialPowerCompletionDie` and the lists of `EjectPilotDie` are kept as NAMES and looked up when they are used (a
//! module's data has no store to look in when it is read); upstream looks them up in the INI parser. See `UnpauseSpecialPowerUpgrade`.
//!
//! # Results (2026-10-21, 9000 cases, `L9DD_CASES` overrides)
//!
//! 8511 data reads and 57,059 op dumps compared (41,897 ops: 18,965 `onDie`, 7,861 xfers), 0 mismatches; 489 inputs excluded as undefined upstream,
//! 2,780 data blocks threw on both sides. Effects: 7,717 `cleardisabled`, 2,266 `audio`, 579 `ocl 1`, 554 `ocl 0`, 285 `oclsec`, 199 `script`.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000. All 12 are caught.
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 701 | DamDie | `clearDisabled` called for every object, not only the water waves |
//! | 702 | DamDie | no `clearDisabled` |
//! | 703 | DamDie | the die mux not applied |
//! | 711 | SpecialPowerCompletionDie | `setCreator` takes every creator, not the first: caught by a C++ crash (see below) |
//! | 712 | SpecialPowerCompletionDie | the script engine told even when no creator was set |
//! | 713 | SpecialPowerCompletionDie | the player index off by one |
//! | 715 | SpecialPowerCompletionDie | the die mux not applied |
//! | 721 | EjectPilotDie | the air and ground lists swapped: caught by a C++ crash (see below) |
//! | 722 | EjectPilotDie | no `SoundEject` event |
//! | 723 | EjectPilotDie | the voice event without the player index |
//! | 724 | EjectPilotDie | the damage dealer never passed |
//! | 725 | EjectPilotDie | the die mux not applied |
//!
//! Plants 711 and 721 are detected by the C++ reference segfaulting (exit 139): with the plant the Rust no longer reports a case as undefined
//! upstream (an ejection from an object with no controlling player; a script notification), so the case reaches the C++ that upstream cannot run.
//! That is a detection, not a clean mismatch; it is recorded as it happened (as plant 619 of `differential_upgrade_modules_b.rs`).
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::damage_fx::DamageFxStore;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_WAVEGUIDE;
use rts_lockstep_engine::common::module::{Module, ModuleData, ModuleDataKind, ModuleType, ObjectId, MODULEINTERFACE_DIE};
use rts_lockstep_engine::common::module_factory::{ModuleFactory, ModuleTemplate};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingParseEnv, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::ArmorStore;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::die_module::DieMuxData;
use rts_lockstep_engine::logic::modules::dam_die::DamDieModuleData;
use rts_lockstep_engine::logic::modules::eject_pilot_die::EjectPilotDieModuleData;
use rts_lockstep_engine::logic::modules::special_power_completion_die::SpecialPowerCompletionDieModuleData;
use rts_lockstep_engine::logic::weapon_set::WeaponTemplateTable;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9dies_d.bin");
const XFER_FILE: &str = "l9dies_d.bin";

/// The class names of the kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 3] = ["DamDie", "SpecialPowerCompletionDie", "EjectPilotDie"];

// -- the dump, in ref_l9dd_run's format --

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
fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
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

// -- the thing factory (the dying object's template: its per-unit sounds) --

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

/// A template named `DYING` (a water wave if `wave`) with the per-unit sounds the case names (`VoiceEject = V<n>`, `SoundEject = S<n>`; 0 none), as INI text.
fn build_factory(voice: u32, sound: u32, wave: bool) -> ThingFactory {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut modules = ModuleFactory::new();
    let t = ModuleTemplate { create_proc: Some(never), create_data_proc: Some(base_data), which_interfaces: Some(MODULEINTERFACE_DIE) };
    modules.add_module_internal(&mut names, t, ModuleType::Behavior, "TestDie").unwrap();
    let mut c = FactoryCtx {
        factory: ThingFactory::new(),
        names,
        modules,
        armors: ArmorStore::new(),
        dfx: DamageFxStore::new(),
        weapons: WeaponTemplateTable::new(),
        sciences: ScienceStore::new(),
    };
    let mut text = String::from("Object DYING\n");
    if wave {
        text.push_str(" KindOf = WAVEGUIDE\n");
    }
    if voice != 0 || sound != 0 {
        text.push_str(" UnitSpecificSounds\n");
        if voice != 0 {
            text.push_str(&format!("  VoiceEject = V{voice}\n"));
        }
        if sound != 0 {
            text.push_str(&format!("  SoundEject = S{sound}\n"));
        }
        text.push_str(" End\n");
    }
    text.push_str("End\n");
    static BLOCKS: [BlockParse<FactoryCtx>; 1] = [BlockParse { token: "Object", parse: object_block }];
    Ini::load(b"Objects.ini", text.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("objects load");
    c.factory
}

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    let kind = out.0;
    out.1 = Some(match kind {
        0 => ModuleDataKind::DamDie(Arc::new(DamDieModuleData::friend_new_module_data(Some(ini))?)),
        1 => ModuleDataKind::SpecialPowerCompletionDie(Arc::new(SpecialPowerCompletionDieModuleData::friend_new_module_data(Some(ini))?)),
        _ => ModuleDataKind::EjectPilotDie(Arc::new(EjectPilotDieModuleData::friend_new_module_data(Some(ini))?)),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9dd_run`: the die mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, store: &rts_lockstep_engine::common::special_power::SpecialPowerStore) {
    match data {
        ModuleDataKind::DamDie(d) => put_die_mux(v, &d.die.die_mux_data),
        ModuleDataKind::SpecialPowerCompletionDie(d) => {
            put_die_mux(v, &d.die.die_mux_data);
            // upstream resolved the name while it read the data; here it is resolved when the die runs
            put_u32(v, d.special_power_template.as_deref().and_then(|n| store.find_special_power_template(n)).map_or(0, |t| t.id()));
        }
        ModuleDataKind::EjectPilotDie(d) => {
            put_die_mux(v, &d.die.die_mux_data);
            // the fixture knows OCL_A and OCL_B (the case matters): the list a name finds
            let which = |n: &Option<Vec<u8>>| match n.as_deref() {
                Some(b"OCL_A") => 1,
                Some(b"OCL_B") => 2,
                _ => 0,
            };
            put_i32(v, which(&d.ocl_in_air));
            put_i32(v, which(&d.ocl_on_ground));
            put_u32(v, d.invulnerable_time);
        }
        _ => unreachable!("not a die module"),
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
    let (dt, vl) = (u32_(&mut p), u32_(&mut p));
    let ex: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let rq: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    out.push(format!("die mux {dt:#x} {vl:#x} exempt {ex:x?} required {rq:x?}"));
    match kind {
        1 => out.push(format!("special power template {}", u32_(&mut p))),
        2 => {
            let (a, g, t) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
            out.push(format!("eject air {a} ground {g} invulnerable {t}"));
        }
        _ => {}
    }
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        out.push(format!("op {i}: status {st} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
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

/// The block `D` of a die: the mux fields (each present with some probability, sparse, so that the mux applies often) and the module's own
/// fields; now and then a bogus token or an unknown field, which both sides must refuse alike.
fn gen_die_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 35 {
        lines.push(format!("DeathTypes = {}", death_tokens(g)));
    }
    if g.below(100) < 25 {
        lines.push(format!("VeterancyLevels = {}", level_tokens(g)));
    }
    if g.below(100) < 15 {
        lines.push(format!("ExemptStatus = {}", status_tokens(g)));
    }
    if g.below(100) < 15 {
        lines.push(format!("RequiredStatus = {}", status_tokens(g)));
    }
    match kind {
        1 => {
            if g.below(100) < 92 {
                lines.push(format!("SpecialPowerTemplate = {}", pick(g, &["SP_0", "SP_1", "SP_2", "None", "none", "sp_0", "SP_9", "SP_1"])));
            }
        }
        2 => {
            if g.below(100) < 85 {
                lines.push(format!("AirCreationList = {}", pick(g, &["OCL_A", "OCL_B", "ocl_a", "None", "OCL_C"])));
            }
            if g.below(100) < 85 {
                lines.push(format!("GroundCreationList = {}", pick(g, &["OCL_A", "OCL_B", "ocl_b", "None", "OCL_C"])));
            }
            if g.below(100) < 50 {
                lines.push(format!("InvulnerableTime = {}", g.below(5000)));
            }
        }
        _ => {}
    }
    if !clean {
        lines.push(match g.below(4) {
            0 => "DeathTypes = +BOGUS".to_string(),
            1 => "VeterancyLevels = WHAT".to_string(),
            2 => "ExemptStatus = NOT_A_STATUS".to_string(),
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

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    fired: u64,
}

fn run_xfer(body: &mut dyn Snapshot, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => match x.xfer_snapshot(body) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
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
                Ok(()) => match x.xfer_snapshot(body) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => match x.xfer_snapshot(body) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

/// The line the driver logs for a cut call or client event the dies cause.
fn cut_line(c: &CutCall) -> Vec<String> {
    match c {
        CutCall::ScriptNotifyOfCompletedSpecialPower { player, name, creator } => vec![format!("script {player} {} {creator}", String::from_utf8_lossy(name))],
        CutCall::OclCreateWithSecondary { secondary, ocl, .. } => {
            let mut v = Vec::new();
            match ocl.as_slice() {
                b"OCL_A" => v.push("ocl 0".to_string()),
                b"OCL_B" => v.push("ocl 1".to_string()),
                // an unknown name is no list: nothing is made
                _ => return v,
            }
            if *secondary != INVALID_ID {
                v.push(format!("oclsec {secondary}"));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// Runs the ops on a fresh world and returns the dump (after the data dump) and the world.
#[allow(clippy::too_many_arguments)]
fn rust_run(kind: usize, mut ctx: Ctx, data: ModuleDataKind, cfg: &mut [u32; 9], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data, &ctx.store);

    let mut w = World::new();
    let factory = build_factory(cfg[7], cfg[8], cfg[3] & 1 != 0);
    let dying_template = Arc::new(factory.find_template(b"DYING").unwrap().get_final_override().data.clone());
    let mut ocls = rts_lockstep_engine::logic::object_creation_list::ObjectCreationListStore::new();
    for n in [&b"OCL_A"[..], b"OCL_B"] {
        // the fixture's two lists (the case matters)
        ocls.ocls.insert(ctx.names.name_to_key(n), Default::default());
    }
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, thing_factory: factory, object_creation_list_store: ocls, ..Default::default() });
    w.names = ctx.names;
    w.cuts.neutral_default_team = 1;
    w.cuts.globals.gravity = f32::from_bits(cfg[6]);
    w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team::new(1, 0)).unwrap();
    if (cfg[1] as i32) >= 0 {
        w.cuts.team_players.insert(1, cfg[1] as i32);
    }
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASSES[kind].as_bytes(), b"ModuleTag_Die", &data).unwrap().unwrap();
    let n_others = cfg[2].min(4);
    let total = 1 + n_others + u32::from(cfg[5] != 0);
    let mut ids = Vec::new();
    for i in 0..total {
        let (template, modules) = if i == 0 {
            (dying_template.clone(), vec![m.clone()])
        } else {
            let mut values = ObjectTemplateValues::default();
            if cfg[3] >> i & 1 != 0 {
                values.kind_of.push(KINDOF_WAVEGUIDE);
            }
            (Arc::new(ThingTemplate::from_object_values(&values)), Vec::new())
        };
        let id = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: Some(1), modules }).unwrap();
        if (1..=4).contains(&i) {
            let bits = cfg[4] >> (4 * (i - 1)) & 15;
            let o = w.objects.find_mut(id).unwrap();
            for b in 0..4 {
                o.disabled.set(b, ((bits >> b) & 1) as i32);
            }
        }
        ids.push(id);
    }
    assert_eq!(ids[0], 1, "the first object of a fresh world has the id 1");
    cfg[0] = ids[0] as u32;
    let me = ids[0];
    let idx = w.find_module(me, CLASSES[kind].as_bytes()).unwrap();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
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
                tally.fired += 1;
            }
            1 => {
                w.module_call(me, idx, ModuleCall::SetCreator { creator: o[1] as _ });
            }
            2..=4 => {
                let how = o[0] - 2;
                let body = w.objects.find_mut(me).unwrap().modules[idx as usize].body.as_mut().expect("the module's body");
                let r = run_xfer(body, how, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 2 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            5 => {
                let pos = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
                w.set_position(me, &pos);
            }
            6 => {
                if o[1] == 0 {
                    w.cuts.team_players.remove(&1);
                } else {
                    w.cuts.team_players.insert(1, o[1] as i32 - 1);
                }
            }
            _ => {
                if o[1] >= 1 && o[1] <= n_others {
                    let id = ids[o[1] as usize];
                    let ob = w.objects.find_mut(id).unwrap();
                    for b in 0..4 {
                        ob.disabled.set(b, ((o[2] >> b) & 1) as i32);
                    }
                }
            }
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        // the effects, in order: the trace has each cut call, client event and module line
        let audio: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(e @ ObjectClientEvent::AudioAddEvent { .. }) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let mut audio_iter = audio.into_iter();
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                let c = cut_iter.next().expect("a cut line has a cut call");
                log.extend(cut_line(c));
            } else if t.starts_with("evt AudioAddEvent") {
                if let Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) = audio_iter.next() {
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio {} {pos} {} {}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1), { object_id }));
                }
            } else if let Some(r) = t.strip_prefix("dam cleardisabled ") {
                log.push(format!("cleardisabled {r}"));
            }
        }
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
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9DD_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn die_modules_d_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_DDD5);
    let table = blocks();
    let mut read = Check::new("die module D data read (throw, message)");
    let mut run = Check::new("die module D ops (data, effects, xfers, disabled bits)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 3];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 3;
        let n_sp = g.below(4) as usize;
        let defs: String = (0..n_sp).map(|i| format!("SpecialPower SP_{i}\nEnd\n")).collect();
        let init_names = g.below(3) != 0;
        let mut ctx = Ctx::new(init_names, true);
        Ini::load(b"Data\\INI\\Sp.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("special powers load");
        cpp_reset(init_names, true);
        assert_eq!(cpp_load(b"Data\\INI\\Sp.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

        let clean = g.below(10) < 7;
        let text = gen_die_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\Ud{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's id (the Rust side's), its player or -1, the number of other objects, which have KINDOF_WAVEGUIDE, their disabled bits,
        //  has a damage dealer, gravity, VoiceEject number, SoundEject number]
        let mut cfg = [0u32; 9];
        cfg[1] = if g.below(6) == 0 { u32::MAX } else { g.below(3) };
        cfg[2] = g.below(5);
        cfg[3] = g.below(64);
        cfg[4] = g.u32() & 0xffff;
        cfg[5] = u32::from(g.below(2) == 0);
        cfg[6] = pick_gravity(&mut g).to_bits();
        cfg[7] = if g.below(3) == 0 { 0 } else { 1 + g.below(3) };
        cfg[8] = if g.below(3) == 0 { 0 } else { 1 + g.below(3) };
        let n_ops = 1 + g.below(14) as usize;
        let total_objs = 1 + cfg[2].min(4) + cfg[5];
        // the objects get the ids 1.. in the order they are made: the one under test, the others, the damage dealer
        let dealer_id = 2 + cfg[2].min(4);
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(30) {
                0..=11 => {
                    // onDie: the death type, a veterancy level, status words, a damage source: the dealer, nobody, or an id that is no object
                    let source = match g.below(4) {
                        0 => 0,
                        1 => 9000 + g.below(10),
                        _ if cfg[5] != 0 => dealer_id,
                        _ => 9000,
                    };
                    [0, 1 + g.below(20), 1 + g.below(3), g.u32() & 0x3fff, g.u32() & 0xff, source]
                }
                12..=14 => [1, g.below(4), 0, 0, 0, 0],
                15..=16 => [2, 0, 0, 0, 0, 0],
                17..=18 => [3, 0, 0, 0, 0, 0],
                19 => [4, 0, 0, 0, 0, 0],
                20..=22 => [5, f32_bits(&mut g, 100.0), f32_bits(&mut g, 100.0), pick_z(&mut g), 0, 0],
                23..=24 => [6, g.below(4), 0, 0, 0, 0],
                _ => [7, g.below(total_objs.max(1)), g.below(16), 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(kind, ctx, data.expect("read ok"), &mut cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some((rd, w))
        } else {
            cfg[0] = 1;
            None
        };
        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9dd_run(
                kind as i32,
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
        let what = || format!("case {case} ({}): cfg {cfg:x?}, file {:?}, ops {ops:?}", CLASSES[kind], show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, w)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in ["script", "ocl 0", "ocl 1", "oclsec", "audio", "cleardisabled"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
        let _ = w;
    }
    println!(
        "die modules D: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} onDie calls, {} xfers, {} effect lines",
        tally.fired, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.effects * 4 > cases as u64, "the generator must reach every path");
}

fn pick_gravity(g: &mut Gen) -> f32 {
    [-0.5f32, -1.0, -0.25, -2.0][g.below(4) as usize]
}
fn f32_bits(g: &mut Gen, range: f32) -> u32 {
    g.uniform(-range, range).to_bits()
}
fn pick_z(g: &mut Gen) -> u32 {
    [0.0f32, 1.0, 4.0, 4.5, 5.0, 40.0, -3.0][g.below(7) as usize].to_bits()
}
