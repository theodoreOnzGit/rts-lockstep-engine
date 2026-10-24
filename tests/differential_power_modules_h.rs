#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/{SpecialPowerModule, CleanupAreaPower, BaikonurLaunchPower,
//   SpyVisionSpecialPower, DefectorSpecialPower}.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The special power base and four powers (`logic::modules::{special_power_body, special_power_module_impl, cleanup_area_power,
//! baikonur_launch_power, spy_vision_special_power, defector_special_power}`) against the unchanged C++, `p1w11-sonnet`, special power batch H.
//!
//! # Method
//!
//! The C++ side is the five `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_powers_h.inc`) on the module build's
//! stand-ins: three players (the shared special power timers, the academy stats, the relationship to the object's team, a default team), an Eva that
//! logs, an in-game UI that logs the superweapons, a script engine that logs, an audio manager that logs, the update modules the powers look for
//! (`CleanupHazardUpdate` and a behaviour with a `SpecialPowerUpdateInterface`; `SpyVisionUpdate` is the REAL class, compiled for update batch I), a contain with a count, a thing factory with `RO_A`
//! (the view and the detonation object) and the REAL `SpecialPowerStore`, `INI` and `AudioEventRTS` parse.
//!
//! The Rust side is the real thing: a `World` with the loaded special power store and a thing factory read from INI text, five teams, three players
//! (the timers are `Player::special_power_timers`), two objects (the one with the module from `modules::new_module` beside scripted `ProbeModule`s
//! for `CleanupHazardUpdate`, the update interface behaviour and the contain, and the real `SpyVisionUpdate`; and a second one that is the target of `doSpecialPowerAtObject`), every op a `World::module_call` or an
//! xfer through `World::xfer_module_slot`.
//!
//! Each case defines four special powers with random fields, reads a `D` block (mostly well formed), and when the read succeeds runs 1 to 16 ops
//! (the four `doSpecialPower...` calls, `pauseCountdown`, `onSpecialPowerCreation`, `setReadyFrame`, `markSpecialPowerTriggered`,
//! `startPowerRecharge`, a frame change, the object disabled or its player changed, an xfer save, load and CRC, `loadPostProcess`, the local player and
//! the relationships changed).
//!
//! Compared, bit for bit: the read's throw; the data; the constructor's effects; per op the throw status, `isReady`, `getPercentReady`,
//! `getReadyFrame`, `isScriptOnly`, the effects in order (`eva`, `script`, `academy`, `audio`, `addsuper`, `defect`, `cleanup`, `spied`, `intent`, `mc`),
//! the xfer status, bytes and CRC, the module's four state words, the real spy vision update's deactivate frame and whether it is on, and every object the module made (id, team, position, shroud clearing range).
//!
//! # Deviations the test cannot see
//!
//! `SpecialPowerTemplate` is kept as a name and found in the store when the power is used (see `modules::special_power_body`): a name the store does not
//! have is a `NULL` pointer upstream and every function that follows it dereferences it, so those cases (and the ones where the field is missing)
//! are never sent to the C++: the Rust records an `is_undefined()` error and the case is counted as excluded. The same goes for a power used by an
//! object with no controlling player. The local player is never null (`setLocalPlayer(NULL)` selects the neutral player), so the dereference of a null local player in the Eva code is not reachable.
//! `~SpecialPowerModule` (`removeSuperweapon`) is not sent when the object goes (no destruction hook; see the module docs), so the driver does not log it.
//!
//! # Results
//!
//! (2026-10-21, 9000 cases, `L9PH_CASES` overrides.) 7,805 data reads and 61,698 dump lines (42,234 ops: 20,664 `doSpecialPower...` calls, 4,504 xfers)
//! compared, 0 mismatches; 1,195 inputs excluded as undefined upstream; 2,678 data blocks threw on both sides. Compared per kind: 1,044 base,
//! 1,024 `CleanupAreaPower`, 1,018 `BaikonurLaunchPower`, 1,034 `SpyVisionSpecialPower`, 1,007 `DefectorSpecialPower`. Effects: 7,543 `academy`,
//! 7,444 `script`, 10,981 `audio`, 4,524 `eva`, 3,114 `intent`, 525 `defect`, 635 `cleanup`, 532 `spied`, 466 `addsuper`, 510 `mc`; up to 10 objects made
//! by one case (view objects and detonation objects).
//!
//! `SpyVisionUpdate` was a logging stand-in when this batch was first written (a `spyvision <duration>` line); it is now the real class on both sides
//! (update batch I), so the duration the power computes is seen as the update's deactivate frame and the activation as the `spied` lines.
//!
//! # Batch R21: `CashBountyPower` and `CashHackSpecialPower` (2026-10-23, `p1w13-sonnet`)
//!
//! The driver also compiles `CashBountyPower.cpp` and `CashHackSpecialPower.cpp` unchanged (kinds 5 and 6; `min` defined around the second, as
//! `Lib/BaseType.h:101` does; the module build's `Coord3D` gained `BaseType.h`'s `add`, the `Player` stand-in its cash bounty, verbatim), gives
//! the players money (the cash hack's money calls and floating texts are lines), calls `onObjectCreated` after the constructor as `Object`'s does
//! (`Object.cpp:485`; only `CashBountyPower` has one), and dumps each player's cash bounty after every op. New ops: a player's sciences (19),
//! `onObjectCreated` (20), a player's money (21) and cash bounty (22). Their own test (`cash_powers_r21_match_upstream`, seed `0x0021_CA54`,
//! `L9R21_CASES`) adds `RequiredScience` to half the powers; the five kinds above keep their stream (re-run: the same 7,805 reads and 61,698 lines).
//!
//! Results, 9,000 cases: 8,145 reads and 67,621 dump lines compared (2,778 `CashBountyPower`, 2,669 `CashHackSpecialPower`), **0 mismatches**;
//! 855 excluded as undefined upstream, 2,698 data blocks threw on both sides; 1,312 thefts (`withdraw`), 2,624 floating texts, 8,426 dump
//! lines with a cash bounty set.
//!
//! A translation fix on the way: `CashBountyPower.h` defines `doSpecialPower` as `{ return; }`; the first translation ran the base's (220
//! mismatches in a 2,000-case run, every one an `academy`/`script`/`audio` line the C++ did not have). Not generated: a science name
//! `UpgradeMoneyAmount` does not declare (an `INI_INVALID_DATA` throw upstream at read; the Rust keeps the name, see
//! `modules::cash_hack_special_power`).
//!
//! | plant | module | what is broken | mismatches |
//! |---|---|---|---|
//! | 1891 | CashBountyPower | `onObjectCreated`'s science test inverted | 11,981 |
//! | 1892 | CashBountyPower | the bounty set even when it is not more (caught once op 22 gave players another bounty) | 54 |
//! | 1893 | CashBountyPower | `doSpecialPower` runs the base's | 734 |
//! | 1901 | CashHackSpecialPower | the science upgrades ignored | 73 |
//! | 1902 | CashHackSpecialPower | the theft not capped by the victim's money | 1,218 |
//! | 1903 | CashHackSpecialPower | the victim's floating text at the thief's height | 1,312 |
//!
//! # Batch R22: `OCLSpecialPower` (2026-10-23, `p1w13-sonnet`)
//!
//! `OCLSpecialPower.cpp` is compiled unchanged at the end of `wrap_module.cpp`'s unit (`differential/l9_ocl_power_r22.inc`), after the stand-ins it
//! reaches, and run as kind 7 through that file's hooks: the lists `OCL_A`/`OCL_B` (any other name, and `None`, is `NULL`), both `createInternal`
//! overloads as lines (`oclat` with an angle, `oclown` with `createOwner`), the map edges as offsets the test sets (ops 23 and 24) and
//! `findPositionAround` as the contain driver's answer (op 25). Its own test (`ocl_power_r22_matches_upstream`, seed `0x0022_0C15`,
//! `L9R22_CASES`).
//!
//! Results, 9,000 cases: 8,107 reads and 66,643 dump lines compared (5,367 cases run), **0 mismatches**; 893 excluded as undefined upstream,
//! 2,740 data blocks threw on both sides; 3,770 `oclat`, 2,024 `oclown` and 2,777 `findpos` lines.
//!
//! | plant | what is broken | mismatches |
//! |---|---|---|
//! | 1911 | the science upgrades ignored | 228 |
//! | 1912 | `doSpecialPower`'s `createOwner` true | 2,024 |
//! | 1913 | the target never moved to a clear cell | 2,796 |
//! | 1914 | `CREATE_ABOVE_LOCATION` at the target's height | 557 |
//!
//! # Upstream facts the test exposed
//!
//! - `SpecialPowerModule.h` declares TWO `getReadyFrame`: the interface's `const` one (the shared timer, or `m_availableOnFrame` plus the paused frames)
//!   and a non-`const` one that returns `m_availableOnFrame` ("used by player to keep recharge timers in sync"). The driver calls the const one, which is
//!   what `ModuleCall::GetReadyFrame` means.
//! - `CleanupAreaPower.cpp`, `SpyVisionSpecialPower.cpp` and `SpecialPowerModule.cpp` cache a name key in a function-local `static`
//!   (`static NameKeyType key_X = NAMEKEY("X")`). Harmless in the game, where the generator is never reset; here every case resets it, and a stale
//!   key names another string. `build.rs` makes the three declarations plain locals (`mod_gen/*_nokey.cpp`, each asserted to apply once).
//! - `isModuleForPower(m_specialPowerTemplate)` in `initiateIntentToDoSpecialPower` compares the module's template with itself: always true.
//! - Not covered by the differential: the `DeletionUpdate` branch of `createViewObject` (the stand-in template `RO_A` has no modules); it is pinned by
//!   `logic::modules::special_power_body::tests::the_view_object_gets_the_view_duration_as_its_lifetime`.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 801 | CleanupAreaPower | the update gets a range of 0 |
//! | 811 | BaikonurLaunchPower | the door does not open |
//! | 821 | SpyVisionSpecialPower | the duration is not clamped to the maximum |
//! | 831 | DefectorSpecialPower | the target does not defect |
//! | 841 | base | the academy is not told |
//! | 843 | base | the Eva announcement counts only allies (neutral as enemy) |
//! | 844 | base | the view object clears no shroud |
//! | 846 | base | the sound at the target is not played |
//! | 847 | base | a public timer is shown for any object, not only structures |
//! | 848 | base | `onSpecialPowerCreation` ignores `StartsPaused` |
//! | 849 | base | an object under construction starts its recharge |
//! | 852 | base | `doSpecialPowerAtObject` triggers at the power's own position |
//! | 855 | SpyVisionSpecialPower | the contained units add no duration |
//!
//! All 13 are caught (3000 cases each; the first four and the nine of the base are mismatches, none by a crash).

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::damage_fx::DamageFxStore;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_STRUCTURE;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_CONTAIN};
use rts_lockstep_engine::common::module_factory::ModuleFactory;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore, ScienceType};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{
    ObjectTemplateValues, ThingParseEnv, ThingTemplate,
};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::ArmorStore;
use rts_lockstep_engine::logic::module::special_power_module::{
    SpecialPowerModule, SpecialPowerModuleData,
};
use rts_lockstep_engine::logic::modules::baikonur_launch_power::BaikonurLaunchPowerModuleData;
use rts_lockstep_engine::logic::modules::cash_bounty_power::CashBountyPowerModuleData;
use rts_lockstep_engine::logic::modules::cash_hack_special_power::CashHackSpecialPowerModuleData;
use rts_lockstep_engine::logic::modules::cleanup_area_power::CleanupAreaPowerModuleData;
use rts_lockstep_engine::logic::modules::defector_special_power::DefectorSpecialPowerModuleData;
use rts_lockstep_engine::logic::modules::ocl_special_power::OCLSpecialPowerModuleData;
use rts_lockstep_engine::logic::modules::special_power_body::SpecialPowerBehavior;
use rts_lockstep_engine::logic::modules::special_power_module_impl::SpecialPowerModuleImpl;
use rts_lockstep_engine::logic::modules::spy_vision_special_power::SpyVisionSpecialPowerModuleData;
use rts_lockstep_engine::logic::modules::spy_vision_update::SpyVisionUpdateModuleData;
use rts_lockstep_engine::logic::weapon_set::WeaponTemplateTable;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{
    EvaSuperweapon, EvaWhose, ModelConditionOp, ObjectClientEvent,
};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9power_h.bin");
const XFER_FILE: &str = "l9power_h.bin";

/// The class names of the kinds, in the order of the driver's `kind` (the base is never registered with the module factory).
const CLASSES: [&str; 8] = [
    "SpecialPowerModule",
    "CleanupAreaPower",
    "BaikonurLaunchPower",
    "SpyVisionSpecialPower",
    "DefectorSpecialPower",
    "CashBountyPower",
    "CashHackSpecialPower",
    "OCLSpecialPower",
];
/// The power types the cases give the special powers (`Enum = `).
const TYPES: [&str; 13] = [
    "SPECIAL_PARTICLE_UPLINK_CANNON",
    "SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_SCUD_STORM",
    "SPECIAL_GPS_SCRAMBLER",
    "SPECIAL_SNEAK_ATTACK",
    "SPECIAL_DEFECTOR",
    "SPECIAL_SPY_SATELLITE",
    "NUKE_SPECIAL_NEUTRON_MISSILE",
    "SLTH_SPECIAL_GPS_SCRAMBLER",
    "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
    "LAZR_SPECIAL_PARTICLE_UPLINK_CANNON",
    "SUPW_SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_INVALID",
];
const NO_PLAYER: u32 = 0xFFFF_FFFF;
/// The team of both objects, and the default teams of the players (`101 + index`).
const OBJECT_TEAM: u32 = 100;

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

// -- the thing factory (RO_A: the view object and the detonation object) --

struct Text;
impl GameTextFetch for Text {
    fn fetch(&self, _label: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
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
    };
    c.factory.parse_object_block(ini, &mut env, 0)
}
fn build_factory(ro_a: bool) -> ThingFactory {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut c = FactoryCtx {
        factory: ThingFactory::new(),
        names,
        modules: ModuleFactory::new(),
        armors: ArmorStore::new(),
        dfx: DamageFxStore::new(),
        weapons: WeaponTemplateTable::new(),
        sciences: ScienceStore::new(),
    };
    let text = if ro_a { "Object RO_A\nEnd\n" } else { "" };
    static BLOCKS: [BlockParse<FactoryCtx>; 1] = [BlockParse {
        token: "Object",
        parse: object_block,
    }];
    Ini::load(
        b"Objects.ini",
        text.as_bytes(),
        IniLoadType::Overwrite,
        None,
        &BLOCKS,
        &mut c,
    )
    .expect("objects load");
    c.factory
}

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    out.1 = Some(match out.0 {
        0 => ModuleDataKind::SpecialPower(Arc::new(
            SpecialPowerModuleData::friend_new_module_data_deferred(Some(ini))?,
        )),
        1 => ModuleDataKind::CleanupAreaPower(Arc::new(
            CleanupAreaPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        2 => ModuleDataKind::BaikonurLaunchPower(Arc::new(
            BaikonurLaunchPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        3 => ModuleDataKind::SpyVisionSpecialPower(Arc::new(
            SpyVisionSpecialPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        5 => ModuleDataKind::CashBountyPower(Arc::new(
            CashBountyPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        6 => ModuleDataKind::CashHackSpecialPower(Arc::new(
            CashHackSpecialPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        7 => ModuleDataKind::OCLSpecialPower(Arc::new(
            OCLSpecialPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
        _ => ModuleDataKind::DefectorSpecialPower(Arc::new(
            DefectorSpecialPowerModuleData::friend_new_module_data(Some(ini))?,
        )),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: read_block,
    }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

fn base_of(data: &ModuleDataKind) -> &SpecialPowerModuleData {
    match data {
        ModuleDataKind::SpecialPower(d) => d,
        ModuleDataKind::CleanupAreaPower(d) => &d.special_power,
        ModuleDataKind::BaikonurLaunchPower(d) => &d.special_power,
        ModuleDataKind::SpyVisionSpecialPower(d) => &d.special_power,
        ModuleDataKind::DefectorSpecialPower(d) => &d.special_power,
        ModuleDataKind::CashBountyPower(d) => &d.special_power,
        ModuleDataKind::CashHackSpecialPower(d) => &d.special_power,
        ModuleDataKind::OCLSpecialPower(d) => &d.special_power,
        _ => unreachable!("not a special power module"),
    }
}

/// The data dump of `ref_l9sp_run`: the base, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, store: &SpecialPowerStore) {
    let b = base_of(data);
    // upstream resolved the name while it read the data; here it is resolved when the power runs
    let id = b
        .special_power_template_name
        .as_deref()
        .and_then(|n| store.find_special_power_template(n))
        .map_or(0, |t| t.id());
    put_u32(v, id);
    put_u32(v, u32::from(b.update_module_starts_attack));
    put_u32(v, u32::from(b.starts_paused));
    put_u32(v, u32::from(b.scripted_special_power_only));
    put_str(v, &b.initiate_sound.event_name);
    match data {
        ModuleDataKind::CleanupAreaPower(d) => put_u32(v, d.cleanup_move_range.to_bits()),
        ModuleDataKind::BaikonurLaunchPower(d) => put_str(v, &d.detonation_object),
        ModuleDataKind::SpyVisionSpecialPower(d) => {
            put_u32(v, d.base_duration_in_frames);
            put_u32(v, d.bonus_duration_per_captured_in_frames);
            put_u32(v, d.max_duration_in_frames);
        }
        ModuleDataKind::DefectorSpecialPower(d) => put_u32(v, d.fat_cursor_radius.to_bits()),
        ModuleDataKind::CashBountyPower(d) => put_u32(v, d.default_bounty.to_bits()),
        ModuleDataKind::CashHackSpecialPower(d) => {
            put_u32(v, d.upgrades.len() as u32);
            for u in &d.upgrades {
                put_str(v, &u.science);
                put_i32(v, u.amount_to_steal);
            }
            put_i32(v, d.default_amount_to_steal);
        }
        ModuleDataKind::OCLSpecialPower(d) => {
            put_u32(v, d.upgrade_ocl.len() as u32);
            for u in &d.upgrade_ocl {
                put_str(v, &u.science);
                put_u32(v, ocl_code(u.ocl.as_deref()));
            }
            put_u32(v, ocl_code(d.default_ocl.as_deref()));
            put_i32(v, d.create_loc);
            put_u32(v, u32::from(d.is_ocl_adjust_position_to_passable));
            put_str(v, &d.reference_thing_name);
        }
        _ => {}
    }
}

/// The driver's code of a list: 1 `OCL_A`, 2 `OCL_B` (case matters), 0 for `NULL` and any other name (batch R22).
fn ocl_code(ocl: Option<&[u8]>) -> u32 {
    match ocl {
        Some(b"OCL_A") => 1,
        Some(b"OCL_B") => 2,
        _ => 0,
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
    let (tid, starts_attack, paused, scripted) =
        (u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let sound = str_(&mut p);
    out.push(format!("data template {tid} startsattack {starts_attack} paused {paused} scripted {scripted} sound {sound:?}"));
    match kind {
        1 => out.push(format!("cleanup range {:#x}", u32_(&mut p))),
        2 => out.push(format!("detonation {:?}", str_(&mut p))),
        3 => {
            let (a, c, d) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
            out.push(format!("spy {a} {c} {d}"));
        }
        4 => out.push(format!("fat cursor {:#x}", u32_(&mut p))),
        5 => out.push(format!("bounty {:#x}", u32_(&mut p))),
        6 => {
            let n = u32_(&mut p);
            let ups: Vec<String> = (0..n)
                .map(|_| format!("{}={}", str_(&mut p), u32_(&mut p) as i32))
                .collect();
            let def = u32_(&mut p) as i32;
            out.push(format!("cash hack {ups:?} default {def}"));
        }
        7 => {
            let n = u32_(&mut p);
            let ups: Vec<String> = (0..n)
                .map(|_| format!("{}={}", str_(&mut p), u32_(&mut p)))
                .collect();
            let (def, loc, adjust) = (u32_(&mut p), u32_(&mut p) as i32, u32_(&mut p));
            let reference = str_(&mut p);
            out.push(format!("ocl power {ups:?} default {def} location {loc} adjust {adjust} reference {reference:?}"));
        }
        _ => {}
    }
    let n = u32_(&mut p);
    let ctor: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
    out.push(format!("constructor {ctor:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let (ready, percent, frame, script_only) =
            (u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p));
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
        let spy: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        let bounties: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        let n_objs = u32_(&mut p);
        let objs: Vec<String> = (0..n_objs)
            .map(|_| {
                let (id, team) = (u32_(&mut p) as i32, u32_(&mut p) as i32);
                let (x, y, z, r) = (u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p));
                format!("{id}:{team}:{x:x},{y:x},{z:x}:{r:x}")
            })
            .collect();
        out.push(format!(
            "op {i}: status {st} ready {ready} percent {percent:#x} frame {frame} scriptonly {script_only} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:x?} spy {spy:?} bounties {bounties:x?} objects {objs:?}"
        ));
    }
    out
}

// -- generators --

fn gen_ms(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=2 => "0".to_string(),
        3..=6 => pick(g, &["100", "500", "1000", "3000", "10000", "33", "34"]).to_string(),
        7..=8 => pick(g, &["30000", "120000", "300000"]).to_string(),
        _ if clean => "1000".to_string(),
        9 => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => format!("{}", g.below(100_000)),
    }
}

/// The text of the four special powers (`SP_0`..`SP_3`).
fn gen_powers(g: &mut Gen) -> String {
    let mut s = String::new();
    for k in 0..4 {
        s.push_str(&format!("SpecialPower SP_{k}\n"));
        if g.below(100) < 80 {
            s.push_str(&format!(" Enum = {}\n", pick(g, &TYPES)));
        }
        if g.below(100) < 70 {
            s.push_str(&format!(" ReloadTime = {}\n", gen_ms(g, true)));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(" PublicTimer = {}\n", pick(g, &["Yes", "No"])));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(
                " SharedSyncedTimer = {}\n",
                pick(g, &["Yes", "No", "Yes"])
            ));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(" ViewObjectDuration = {}\n", gen_ms(g, true)));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(
                " ViewObjectRange = {}\n",
                pick(g, &["0", "50", "150.5", "300"])
            ));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(" DetectionTime = {}\n", gen_ms(g, true)));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(
                " InitiateSound = {}\n",
                pick(g, &["SND_A", "SND_B", "None"])
            ));
        }
        if g.below(100) < 50 {
            s.push_str(&format!(
                " InitiateAtLocationSound = {}\n",
                pick(g, &["SND_C", "SND_D", "None"])
            ));
        }
        s.push_str("End\n");
    }
    s
}

/// The block `D`: the base fields (each present with some probability) and the class's own.
fn gen_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    // the template: mostly a defined one (a missing or unknown one is undefined upstream: excluded)
    match g.below(100) {
        0..=91 => lines.push(format!("SpecialPowerTemplate = SP_{}", g.below(4))),
        92..=94 => lines.push("SpecialPowerTemplate = None".to_string()),
        95..=97 => lines.push("SpecialPowerTemplate = SP_9".to_string()),
        _ => {}
    }
    if g.below(100) < 40 {
        lines.push(format!("UpdateModuleStartsAttack = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 25 {
        lines.push(format!("StartsPaused = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 40 {
        lines.push(format!(
            "InitiateSound = {}",
            pick(g, &["SND_M", "SND_N", "None"])
        ));
    }
    if g.below(100) < 15 {
        lines.push(format!("ScriptedSpecialPowerOnly = {}", gen_bool(g, clean)));
    }
    match kind {
        1 => {
            if g.below(100) < 80 {
                lines.push(format!(
                    "MaxMoveDistanceFromLocation = {}",
                    gen_real(g, clean)
                ));
            }
        }
        2 => {
            if g.below(100) < 85 {
                lines.push(format!(
                    "DetonationObject = {}",
                    pick(g, &["RO_A", "RO_A", "RO_B", "ro_a", ""])
                ));
            }
        }
        3 => {
            for f in ["BaseDuration", "BonusDurationPerCaptured", "MaxDuration"] {
                if g.below(100) < 85 {
                    lines.push(format!("{f} = {}", gen_ms(g, clean)));
                }
            }
        }
        4 if g.below(100) < 80 => lines.push(format!("FatCursorRadius = {}", gen_real(g, clean))),
        5 if g.below(100) < 85 => lines.push(format!(
            "Bounty = {}",
            pick(g, &["0%", "10%", "25%", "50%", "12.5%"])
        )),
        6 => {
            // (only declared sciences: an unknown one throws upstream when read, and is looked up only when used here; see the module docs)
            for _ in 0..g.below(3) {
                let (sci, amount) = (
                    pick(g, &SCIENCES).to_string(),
                    pick(g, &["0", "200", "750", "5000"]).to_string(),
                );
                lines.push(format!("UpgradeMoneyAmount = {sci} {amount}"));
            }
            if g.below(100) < 85 {
                lines.push(format!(
                    "MoneyAmount = {}",
                    pick(g, &["0", "100", "500", "1000", "-5"])
                ));
            }
        }
        7 => {
            let ocl = |g: &mut Gen| {
                pick(g, &["OCL_A", "OCL_B", "OCL_A", "None", "OCL_C", "ocl_a"]).to_string()
            };
            for _ in 0..g.below(3) {
                let (sci, list) = (pick(g, &SCIENCES).to_string(), ocl(g));
                lines.push(format!("UpgradeOCL = {sci} {list}"));
            }
            if g.below(100) < 85 {
                let list = ocl(g);
                lines.push(format!("OCL = {list}"));
            }
            if g.below(100) < 85 {
                lines.push(format!("CreateLocation = {}", pick(g, &rts_lockstep_engine::logic::modules::ocl_special_power::OCL_CREATE_LOC_TYPE_NAMES)));
            }
            if g.below(100) < 50 {
                lines.push(format!(
                    "OCLAdjustPositionToPassable = {}",
                    gen_bool(g, true)
                ));
            }
            if g.below(100) < 30 {
                lines.push(format!("ReferenceObject = {}", pick(g, &["RO_A", "Thing"])));
            }
        }
        _ => {}
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "StartsPaused = Perhaps".to_string(),
            1 => "UpdateModuleStartsAttack = 7".to_string(),
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
    powers: u64,
    made: u64,
}

fn run_xfer(
    w: &mut World,
    me: ObjectId,
    idx: usize,
    how: u32,
    saved: &[u8],
) -> (i32, Vec<u8>, u32) {
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

fn relationship(v: u32) -> Relationship {
    match v & 3 {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

fn set_relationships(w: &mut World, word: u32) {
    for p in 0..3 {
        w.cuts
            .player_relationships
            .insert((p, OBJECT_TEAM), relationship(word >> (2 * p)));
    }
}

/// Player `a`'s relationship to the default team of player `b` is bits `2 * (3 * a + b)` of `word`.
fn set_matrix(w: &mut World, word: u32) {
    for a in 0..3 {
        for b in 0..3 {
            w.cuts
                .player_relationships
                .insert((a, 101 + b as u32), relationship(word >> (2 * (3 * a + b))));
        }
    }
}

/// A kind-of mask as the driver prints it in a log line: four words, hex, the highest first.
fn kind_words(m: &rts_lockstep_engine::common::kind_of::KindOfMaskType) -> String {
    let mut w = [0u32; 4];
    for i in 0..rts_lockstep_engine::common::kind_of::KINDOF_COUNT {
        if m.test(i) {
            w[(i / 32) as usize] |= 1 << (i % 32);
        }
    }
    format!("{:08x}{:08x}{:08x}{:08x}", w[3], w[2], w[1], w[0])
}

fn set_local(w: &mut World, local: Option<i32>) {
    w.players.set_local_player(local);
}

fn set_disabled_and_player(w: &mut World, ids: [ObjectId; 2], disabled: bool, player: Option<u32>) {
    for id in ids {
        let o = w.objects.find_mut(id).unwrap();
        o.disabled.set(
            rts_lockstep_engine::common::disabled_types::DISABLED_HACKED,
            i32::from(disabled),
        );
    }
    match player {
        Some(p) => {
            w.cuts.team_players.insert(OBJECT_TEAM, p as i32);
        }
        None => {
            w.cuts.team_players.remove(&OBJECT_TEAM);
        }
    }
}

/// The `Eva` message number of the stand-in's enumeration (differential/shim_module/GameClient/Eva.h).
fn eva_number(weapon: EvaSuperweapon, whose: EvaWhose) -> i32 {
    let whose = match whose {
        EvaWhose::Own => 0,
        EvaWhose::Ally => 1,
        EvaWhose::Enemy => 2,
    };
    match weapon {
        EvaSuperweapon::ParticleCannon => whose * 3,
        EvaSuperweapon::Nuke => whose * 3 + 1,
        EvaSuperweapon::ScudStorm => whose * 3 + 2,
        EvaSuperweapon::GpsScrambler => 9 + whose,
        EvaSuperweapon::SneakAttack => 12 + whose,
    }
}

/// The effect lines of one stretch of the trace, in the driver's format: each `cut` line takes the next cut call, each `evt` line the next client
/// event, and the module's own lines (`defect`, `cleanup`, `intent`) are the driver's.
fn effects(trace: Vec<String>, cuts: Vec<CutCall>, events: Vec<ObjectClientEvent>) -> Vec<String> {
    let mut log = Vec::new();
    let mut cut_iter = cuts.into_iter();
    let mut event_iter = events.into_iter();
    for t in trace {
        if t.starts_with("cut ") {
            match cut_iter.next().expect("a cut line has a cut call") {
                CutCall::ScriptNotifyOfTriggeredSpecialPower {
                    player,
                    name,
                    source,
                } => log.push(format!(
                    "script {player} {} {source}",
                    String::from_utf8_lossy(&name)
                )),
                CutCall::PlayerAcademyRecordSpecialPowerUsed { player, template } => {
                    log.push(format!("academy {player} {template}"))
                }
                CutCall::PlayerSetUnitsVisionSpied {
                    player,
                    setting,
                    which_units,
                    by_whom,
                } => log.push(format!(
                    "spied {player} {} {by_whom} {}",
                    i32::from(setting),
                    kind_words(&which_units)
                )),
                // batch R21: the cash hack (the stand-ins' lines, l9_crates_f.inc, l9_sabotage_r13.inc, l9_updates_l.inc)
                CutCall::PlayerMoneyWithdraw { player, amount } => {
                    log.push(format!("withdraw {player} {amount} 1"))
                }
                CutCall::PlayerMoneyDeposit { player, amount } => {
                    log.push(format!("deposit {player} {amount} 1"))
                }
                CutCall::PlayerScoreAddMoneyEarned { player, amount } => {
                    log.push(format!("earned {player} {amount}"))
                }
                // batch R22: the OCL power's lists (a name that is no list makes nothing) and the clear cell it asks for
                CutCall::OclCreateAt {
                    ocl,
                    primary_pos: a,
                    secondary_pos: b,
                    angle,
                    ..
                } if ocl_code(Some(&ocl)) != 0 => log.push(format!(
                    "oclat {} {:08x},{:08x},{:08x} {:08x},{:08x},{:08x} {:08x}",
                    ocl_code(Some(&ocl)) - 1,
                    a.x.to_bits(),
                    a.y.to_bits(),
                    a.z.to_bits(),
                    b.x.to_bits(),
                    b.y.to_bits(),
                    b.z.to_bits(),
                    angle.to_bits()
                )),
                CutCall::OclCreateOwner {
                    ocl,
                    primary_pos: a,
                    secondary_pos: b,
                    create_owner,
                    ..
                } if ocl_code(Some(&ocl)) != 0 => log.push(format!(
                    "oclown {} {:08x},{:08x},{:08x} {:08x},{:08x},{:08x} {} 0",
                    ocl_code(Some(&ocl)) - 1,
                    a.x.to_bits(),
                    a.y.to_bits(),
                    a.z.to_bits(),
                    b.x.to_bits(),
                    b.y.to_bits(),
                    b.z.to_bits(),
                    i32::from(create_owner)
                )),
                CutCall::Contain(ContainCall::PartitionFindPositionAround {
                    position: p,
                    min_radius,
                    max_radius,
                    flags,
                }) => log.push(format!(
                    "findpos {:08x} {:08x} {:08x} {:08x} {:08x} {flags}",
                    p.x.to_bits(),
                    p.y.to_bits(),
                    p.z.to_bits(),
                    min_radius.to_bits(),
                    max_radius.to_bits()
                )),
                CutCall::InGameUiAddFloatingTextNamed {
                    text,
                    amount,
                    position: p,
                    color,
                } => log.push(format!(
                    "float {} {amount} {},{},{} {:08x}",
                    String::from_utf8_lossy(&text),
                    p.x.to_bits(),
                    p.y.to_bits(),
                    p.z.to_bits(),
                    color as u32
                )),
                _ => {}
            }
        } else if t.starts_with("evt ObjectCreated") || t.starts_with("evt ObjectDestroyed") {
            // (the client's own events, not `Object`'s)
        } else if t.starts_with("evt ") {
            match event_iter.next().expect("an evt line has an event") {
                ObjectClientEvent::EvaSuperweaponLaunched { weapon, whose } => {
                    log.push(format!("eva {}", eva_number(weapon, whose)))
                }
                ObjectClientEvent::UiAddSuperweapon {
                    player_index,
                    power_name,
                    object_id,
                    template,
                } => log.push(format!(
                    "addsuper {player_index} {} {object_id} {template}",
                    String::from_utf8_lossy(&power_name)
                )),
                ObjectClientEvent::AudioAddEvent {
                    name,
                    object_id,
                    position,
                    player_index,
                } => {
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!(
                        "audio {} {pos} {} {object_id}",
                        String::from_utf8_lossy(&name),
                        player_index.unwrap_or(-1)
                    ));
                }
                ObjectClientEvent::DrawableModelCondition {
                    op: ModelConditionOp::Set(v),
                    ..
                } => log.push(format!("mc {}", v.first().copied().unwrap_or(-1))),
                _ => {}
            }
        } else if t.starts_with("defect ") || t.starts_with("cleanup ") || t.starts_with("intent ")
        {
            log.push(t);
        }
    }
    log
}

/// The module on object `me` at slot `idx`, as its state words.
fn state_words(w: &World, me: ObjectId, idx: u16) -> Vec<u32> {
    let o = w.objects.find(me).unwrap();
    let power: &SpecialPowerModule = match o.modules[idx as usize]
        .body
        .as_ref()
        .expect("the module is back in its slot")
    {
        UpdateBody::SpecialPowerModule(m) => m.power(),
        UpdateBody::CleanupAreaPower(m) => m.power(),
        UpdateBody::BaikonurLaunchPower(m) => m.power(),
        UpdateBody::SpyVisionSpecialPower(m) => m.power(),
        UpdateBody::DefectorSpecialPower(m) => m.power(),
        UpdateBody::CashBountyPower(m) => m.power(),
        UpdateBody::CashHackSpecialPower(m) => m.power(),
        UpdateBody::OCLSpecialPower(m) => m.power(),
        _ => unreachable!("not a special power module"),
    };
    vec![
        power.get_available_on_frame(),
        power.paused_count() as u32,
        power.paused_on_frame(),
        power.paused_percent().to_bits(),
    ]
}

fn probe(answers: ProbeAnswers) -> UpdateBody {
    UpdateBody::ModuleProbe(ProbeModule::new(answers, 0.0, Default::default()))
}

fn power_module(kind: usize, id: ObjectId, data: &ModuleDataKind) -> NewModule {
    if kind == 0 {
        let ModuleDataKind::SpecialPower(d) = data else {
            unreachable!()
        };
        let mut nm = NewModule::plain(CLASSES[0].as_bytes(), b"ModuleTag_Power");
        nm.interfaces = SpecialPowerModule::get_interface_mask();
        nm.body = Some(UpdateBody::SpecialPowerModule(
            SpecialPowerModuleImpl::new(id, Some(d.clone())).unwrap(),
        ));
        nm
    } else {
        rts_lockstep_engine::logic::modules::new_module(
            id,
            CLASSES[kind].as_bytes(),
            b"ModuleTag_Power",
            data,
        )
        .unwrap()
        .unwrap()
    }
}

/// Runs the ops on a fresh world and returns the dump (after the data dump) and the world.
#[allow(clippy::too_many_arguments)]
fn rust_run(
    kind: usize,
    names: NameKeyGenerator,
    store: SpecialPowerStore,
    data: ModuleDataKind,
    cfg: &[u32; 11],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data, &store);

    let mut w = World::new();
    // batch R21: the declared sciences, for the cash hack's `UpgradeMoneyAmount` (their keys are the ones the context gave them)
    let mut names = names;
    let mut sciences = ScienceStore::new();
    {
        struct SciCtx<'a> {
            store: &'a mut ScienceStore,
            names: &'a mut NameKeyGenerator,
        }
        fn science_block(ini: &mut Ini<'_>, c: &mut SciCtx<'_>) -> IniResult<()> {
            c.store.parse_science_definition(ini, c.names, &Text)
        }
        let defs: String = SCIENCES
            .iter()
            .map(|s| format!("Science {s}\nEnd\n"))
            .collect();
        let blocks = [BlockParse {
            token: "Science",
            parse: science_block,
        }];
        let mut c = SciCtx {
            store: &mut sciences,
            names: &mut names,
        };
        Ini::load(
            b"Data\\INI\\Science.ini",
            defs.as_bytes(),
            IniLoadType::Overwrite,
            None,
            &blocks,
            &mut c,
        )
        .expect("sciences load");
    }
    w.names = names;
    w.statics = Arc::new(GameStatics {
        special_power_store: store,
        thing_factory: build_factory(cfg[2] >> 8 & 1 != 0),
        science_store: sciences,
        ..Default::default()
    });
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    if cfg[2] >> 9 & 1 != 0 {
        w.cuts.special_power_view_object_name = b"RO_A".to_vec();
    }
    for team in [OBJECT_TEAM, 101, 102, 103] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    w.players.set_player_count(3);
    for i in 0..3 {
        w.players.get_nth_mut(i).unwrap().default_team = Some(101 + i as u32);
        // batch R21: the money (the driver's `m_count`; the cash hack only counts it, its calls are cuts)
        let mut events = Vec::new();
        let p = w.players.get_nth_mut(i).unwrap();
        p.money.init();
        p.money.set_player_index(i);
        p.money.deposit(cfg[8 + i as usize], false, &mut events);
    }
    set_local(&mut w, (cfg[1] < 3).then_some(cfg[1] as i32));
    set_relationships(&mut w, cfg[4]);
    set_matrix(&mut w, cfg[7]);
    if cfg[0] != NO_PLAYER {
        w.cuts.team_players.insert(OBJECT_TEAM, cfg[0] as i32);
    }
    w.set_frame(cfg[6]);

    let first_id = w.next_object_id();
    let mut modules = vec![power_module(kind, first_id, &data)];
    if cfg[2] >> 6 & 1 != 0 {
        let mut b = NewModule::plain(b"Behavior", b"ModuleTag_Spu");
        b.body = Some(probe(ProbeAnswers {
            spu_present: true,
            spu_pass: true,
            spu_take: cfg[2] >> 7 & 1 != 0,
            ..Default::default()
        }));
        modules.push(b);
    }
    if cfg[2] >> 4 & 1 != 0 {
        // the real SpyVisionUpdate, with the default data (batch I tests its fields)
        let spy = ModuleDataKind::SpyVisionUpdate(Arc::new(SpyVisionUpdateModuleData::default()));
        modules.push(
            rts_lockstep_engine::logic::modules::new_module(
                first_id,
                b"SpyVisionUpdate",
                b"ModuleTag_Spy",
                &spy,
            )
            .unwrap()
            .unwrap(),
        );
    }
    if cfg[2] >> 5 & 1 != 0 {
        let mut b = NewModule::plain(b"CleanupHazardUpdate", b"ModuleTag_Cleanup");
        b.body = Some(probe(ProbeAnswers::default()));
        modules.push(b);
    }
    if cfg[2] >> 3 & 1 != 0 {
        let mut b = NewModule::plain(b"OpenContain", b"ModuleTag_Contain");
        b.interfaces = MODULEINTERFACE_CONTAIN;
        b.body = Some(probe(ProbeAnswers {
            contained: (0..cfg[3] as i32).map(|i| 1000 + i).collect(),
            ..Default::default()
        }));
        modules.push(b);
    }
    let mut values = ObjectTemplateValues::default();
    if cfg[2] >> 2 & 1 != 0 {
        values.kind_of.push(KINDOF_STRUCTURE);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let mut status = ObjectStatusMaskType::default();
    if cfg[2] >> 1 & 1 != 0 {
        status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    }
    let events_before = w.events.events().len();
    let me = w
        .new_object(NewObject {
            template: template.clone(),
            status,
            team: Some(OBJECT_TEAM),
            modules,
        })
        .unwrap();
    assert_eq!(me, first_id);
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let other = w
        .new_object(NewObject {
            template,
            status,
            team: Some(OBJECT_TEAM),
            modules: Vec::new(),
        })
        .unwrap();
    assert_eq!(other, 2);
    w.set_position(
        me,
        &Coord3D {
            x: 11.0,
            y: 22.0,
            z: 5.0,
        },
    );
    w.set_position(
        other,
        &Coord3D {
            x: -3.0,
            y: 8.0,
            z: 1.5,
        },
    );
    let idx = w.find_module(me, CLASSES[kind].as_bytes()).unwrap();

    // the constructor's effects (the events and cut calls so far; the second object has none)
    let new_events = |w: &World, before: usize| -> Vec<ObjectClientEvent> {
        w.events.events()[before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect()
    };
    let ctor = effects(
        w.cuts.trace.take().unwrap(),
        w.cuts.log.take().unwrap(),
        new_events(&w, events_before),
    );
    tally.effects += ctor.len() as u64;
    put_u32(&mut v, ctor.len() as u32);
    for l in &ctor {
        put_str(&mut v, l.as_bytes());
    }
    // after the constructor: disabled, and the drawable (binding it makes the client set the whole condition state; not the module's)
    set_disabled_and_player(
        &mut w,
        [me, other],
        cfg[2] & 1 != 0,
        (cfg[0] != NO_PLAYER).then_some(cfg[0]),
    );
    // (every object the world makes has a drawable; the case may take it away)
    w.friend_bind_to_drawable(
        me,
        if cfg[2] >> 10 & 1 != 0 {
            7
        } else {
            rts_lockstep_engine::common::game_type::INVALID_DRAWABLE_ID
        },
    );

    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        let loc = |a: u32, b: u32, c: u32| Coord3D {
            x: f32::from_bits(a),
            y: f32::from_bits(b),
            z: f32::from_bits(c),
        };
        match o[0] {
            0 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::DoSpecialPower {
                        command_options: o[1],
                    },
                );
                tally.powers += 1;
            }
            1 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::DoSpecialPowerAtObject {
                        target: other,
                        command_options: o[1],
                    },
                );
                tally.powers += 1;
            }
            2 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::DoSpecialPowerAtLocation {
                        loc: loc(o[1], o[2], o[3]),
                        angle: 0.0,
                        command_options: o[4],
                    },
                );
                tally.powers += 1;
            }
            3 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::DoSpecialPowerUsingWaypoints {
                        waypoint: o[1],
                        command_options: o[2],
                    },
                );
                tally.powers += 1;
            }
            4 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::PauseCountdown {
                        disabling: o[1] & 1 != 0,
                    },
                );
            }
            5 => {
                w.module_call(me, idx, ModuleCall::OnSpecialPowerCreation);
            }
            6 => {
                w.module_call(me, idx, ModuleCall::SetReadyFrame { frame: o[1] });
            }
            7 => {
                let location = (o[1] != 0).then(|| loc(o[2], o[3], o[4]));
                w.module_call(me, idx, ModuleCall::MarkSpecialPowerTriggered { location });
            }
            8 => {
                w.module_call(me, idx, ModuleCall::StartPowerRecharge);
            }
            9 => {}
            10 => w.set_frame(o[1]),
            11 => set_disabled_and_player(
                &mut w,
                [me, other],
                o[1] & 1 != 0,
                (o[2] != 0).then(|| o[2] - 1),
            ),
            12..=14 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 12, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 12 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            15 => {
                w.module_call(me, idx, ModuleCall::LoadPostProcess);
            }
            16 => set_local(&mut w, (o[1] != 0).then(|| o[1] as i32 - 1)),
            17 => set_relationships(&mut w, o[1]),
            18 => set_matrix(&mut w, o[1]),
            // batch R21
            19 => {
                let p = w.players.get_nth_mut((o[1] % 3) as i32).unwrap();
                if o[2] != 0 {
                    p.sciences.push(ScienceType(o[3] as i32));
                } else {
                    p.sciences.clear();
                }
            }
            20 => {
                w.module_call(me, idx, ModuleCall::OnObjectCreated);
            }
            22 => {
                w.players
                    .get_nth_mut((o[1] % 3) as i32)
                    .unwrap()
                    .cash_bounty_percent = f32::from_bits(o[2])
            }
            // batch R22: the map edges and the clear cell
            23 => w.cuts.edge_point_offset = loc(o[1], o[2], o[3]),
            24 => w.cuts.farthest_edge_point_offset = loc(o[1], o[2], o[3]),
            25 => w.cuts.contain.find_position = (o[1] != 0).then(|| loc(o[2], o[3], o[4])),
            21 => {
                let mut events = Vec::new();
                let p = w.players.get_nth_mut((o[1] % 3) as i32).unwrap();
                p.money.init();
                p.money.deposit(o[2], false, &mut events);
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        // the effects of the op itself (the queries below log nothing)
        let log = effects(
            w.cuts.trace.take().unwrap(),
            w.cuts.log.take().unwrap(),
            new_events(&w, events_before),
        );
        if o[0] == 16 || o[0] == 17 {
            assert!(log.is_empty());
        }
        let ready = matches!(
            w.module_call(me, idx, ModuleCall::IsReady),
            ModuleReply::Bool(true)
        );
        let percent = match w.module_call(me, idx, ModuleCall::GetPercentReady) {
            ModuleReply::Real(r) => r.to_bits(),
            r => panic!("unexpected reply {r:?}"),
        };
        let frame = match w.module_call(me, idx, ModuleCall::GetReadyFrame) {
            ModuleReply::Int(f) => f as u32,
            r => panic!("unexpected reply {r:?}"),
        };
        let script_only = matches!(
            w.module_call(me, idx, ModuleCall::IsScriptOnly),
            ModuleReply::Bool(true)
        );
        put_u32(&mut v, u32::from(ready));
        put_u32(&mut v, percent);
        put_u32(&mut v, frame);
        put_u32(&mut v, u32::from(script_only));
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
        for s in state_words(&w, me, idx) {
            put_u32(&mut v, s);
        }
        // the object's real SpyVisionUpdate, if it has one: its deactivate frame and whether it is on
        let spy =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::SpyVisionUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        put_u32(&mut v, u32::from(spy.is_some()));
        put_u32(&mut v, spy.as_ref().map_or(0, |s| s[0]));
        put_u32(&mut v, spy.as_ref().map_or(0, |s| s[1]));
        for p in 0..3 {
            put_u32(
                &mut v,
                w.players.get_nth(p).unwrap().cash_bounty_percent.to_bits(),
            );
        }
        // the objects the module made (the ids after the two it was given, in creation order)
        let made: Vec<ObjectId> = (3..)
            .take_while(|id| w.objects.find(*id).is_some())
            .collect();
        tally.made = tally.made.max(made.len() as u64);
        put_u32(&mut v, made.len() as u32);
        for id in made {
            let ob = w.objects.find(id).unwrap();
            let pos = ob.get_position();
            put_i32(&mut v, id);
            put_i32(&mut v, w.get_team(id).map_or(0, |t| t as i32));
            put_u32(&mut v, pos.x.to_bits());
            put_u32(&mut v, pos.y.to_bits());
            put_u32(&mut v, pos.z.to_bits());
            put_u32(&mut v, ob.shroud_clearing_range.to_bits());
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9PH_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

fn f32_bits(g: &mut Gen, range: f32) -> u32 {
    g.uniform(-range, range).to_bits()
}

fn gen_options(g: &mut Gen) -> u32 {
    if g.below(3) == 0 {
        g.below(16)
    } else {
        0
    }
}

fn gen_frame(g: &mut Gen) -> u32 {
    match g.below(10) {
        0..=5 => g.below(400),
        6..=7 => g.below(10_000),
        8 => g.below(2_000_000),
        _ => 0,
    }
}

#[test]
fn power_modules_h_match_upstream() {
    run_suite(
        &[0, 1, 2, 3, 4],
        0x1091_4A55,
        cases(),
        "special power modules H",
    );
}

/// Batch R21: `CashBountyPower` and `CashHackSpecialPower`, on the same driver (their own seed and case count, so the five kinds above keep their
/// random stream).
#[test]
fn cash_powers_r21_match_upstream() {
    let cases = std::env::var("L9R21_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000);
    run_suite(&[5, 6], 0x0021_CA54, cases, "cash powers R21");
}

/// Batch R22: `OCLSpecialPower` on the same driver (its own seed and case count).
#[test]
fn ocl_power_r22_matches_upstream() {
    let cases = std::env::var("L9R22_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000);
    run_suite(&[7], 0x0022_0C15, cases, "OCL power R22");
}

/// The cases of `kinds` (in turn), from the seed: each a set of special powers, a data block, a scene and ops, run on both sides and compared.
fn run_suite(kinds: &[usize], seed: u64, cases: usize, label: &str) {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(seed);
    let table = blocks();
    let mut read = Check::new(&format!("{label} data read (throw, message)"));
    let mut run = Check::new(&format!("{label} ops (data, effects, state, objects)"));
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 8];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = kinds[case % kinds.len()];
        let mut defs = gen_powers(&mut g);
        if kind >= 5 {
            // batch R21: some powers require a science (the cash bounty's `onObjectCreated` tests it)
            let mut out = String::new();
            for part in defs.split_inclusive("End\n") {
                if g.below(2) == 0 {
                    out.push_str(&part.replace(
                        "End\n",
                        &format!(" RequiredScience = {}\nEnd\n", pick(&mut g, &SCIENCES)),
                    ));
                } else {
                    out.push_str(part);
                }
            }
            defs = out;
        }
        let mut ctx = Ctx::new(true, true);
        Ini::load(
            b"Data\\INI\\Sp.ini",
            defs.as_bytes(),
            IniLoadType::Overwrite,
            None,
            &table,
            &mut ctx,
        )
        .expect("special powers load");
        cpp_reset(true, true);
        assert_eq!(
            cpp_load(
                b"Data\\INI\\Sp.ini",
                defs.as_bytes(),
                IniLoadType::Overwrite,
                false
            )
            .status,
            0
        );
        let clean = g.below(10) < 7;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\Sh{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's player, the local player, flags, the contain count, the relationships, the first new id, the frame]
        let mut cfg = [0u32; 11];
        cfg[0] = if g.below(12) == 0 {
            NO_PLAYER
        } else {
            g.below(3)
        };
        cfg[1] = g.below(3); // (a local player always exists once the list is set up: `setLocalPlayer(NULL)` selects the neutral player)
        let dis = u32::from(g.below(5) == 0);
        let constructing = u32::from(g.below(7) == 0) << 1;
        let structure = u32::from(g.below(10) < 6) << 2;
        let contain = u32::from(g.below(100) < if kind == 3 { 75 } else { 30 }) << 3;
        let spy = u32::from(g.below(100) < if kind == 3 { 70 } else { 20 }) << 4;
        let cleanup = u32::from(g.below(100) < if kind == 1 { 75 } else { 20 }) << 5;
        let spu = u32::from(g.below(10) < 4) << 6;
        let take = u32::from(g.below(2) == 0) << 7;
        let ro_a = u32::from(g.below(10) < 7) << 8;
        let view_name = u32::from(g.below(10) < 7) << 9;
        let drawable = u32::from(g.below(10) < 6) << 10;
        cfg[2] = dis
            | constructing
            | structure
            | contain
            | spy
            | cleanup
            | spu
            | take
            | ro_a
            | view_name
            | drawable;
        cfg[3] = if g.below(10) == 0 {
            g.below(40)
        } else {
            g.below(6)
        };
        cfg[4] = (g.below(3)) | (g.below(3) << 2) | (g.below(3) << 4);
        cfg[5] = 3;
        cfg[6] = gen_frame(&mut g);
        cfg[7] = (0..9).fold(0u32, |acc, i| acc | (g.below(3) << (2 * i)));
        let n_ops = 1 + g.below(16) as usize;
        let mut ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(60) {
                0..=7 => [0, gen_options(&mut g), 0, 0, 0, 0],
                8..=12 => [1, gen_options(&mut g), 0, 0, 0, 0],
                13..=19 => [
                    2,
                    f32_bits(&mut g, 200.0),
                    f32_bits(&mut g, 200.0),
                    f32_bits(&mut g, 20.0),
                    gen_options(&mut g),
                    0,
                ],
                20..=22 => [3, g.below(3), gen_options(&mut g), 0, 0, 0],
                23..=26 => [4, g.below(2), 0, 0, 0, 0],
                27..=28 => [5, 0, 0, 0, 0, 0],
                29..=31 => [6, gen_frame(&mut g), 0, 0, 0, 0],
                32..=34 => [
                    7,
                    g.below(2),
                    f32_bits(&mut g, 200.0),
                    f32_bits(&mut g, 200.0),
                    f32_bits(&mut g, 20.0),
                    0,
                ],
                35..=37 => [8, 0, 0, 0, 0, 0],
                38..=39 => [9, 0, 0, 0, 0, 0],
                40..=43 => [10, gen_frame(&mut g), 0, 0, 0, 0],
                44..=46 => [11, g.below(2), g.below(4), 0, 0, 0],
                47..=48 => [12, 0, 0, 0, 0, 0],
                49..=50 => [13, 0, 0, 0, 0, 0],
                51 => [14, 0, 0, 0, 0, 0],
                52..=53 => [15, 0, 0, 0, 0, 0],
                54..=55 => [16, 1 + g.below(3), 0, 0, 0, 0],
                56..=57 => [
                    18,
                    (0..9).fold(0u32, |acc, i| acc | (g.below(3) << (2 * i))),
                    0,
                    0,
                    0,
                    0,
                ],
                _ => [
                    17,
                    g.below(3) | (g.below(3) << 2) | (g.below(3) << 4),
                    0,
                    0,
                    0,
                    0,
                ],
            })
            .collect();
        if kind == 7 {
            // batch R22: the edges, the clear cell and the players' sciences; the power mostly runs
            for o in &mut ops {
                match g.below(10) {
                    0 => {
                        *o = [
                            23,
                            f32_bits(&mut g, 100.0),
                            f32_bits(&mut g, 100.0),
                            f32_bits(&mut g, 10.0),
                            0,
                            0,
                        ]
                    }
                    1 => {
                        *o = [
                            24,
                            f32_bits(&mut g, 100.0),
                            f32_bits(&mut g, 100.0),
                            f32_bits(&mut g, 10.0),
                            0,
                            0,
                        ]
                    }
                    2 => {
                        *o = [
                            25,
                            g.below(2),
                            f32_bits(&mut g, 200.0),
                            f32_bits(&mut g, 200.0),
                            f32_bits(&mut g, 20.0),
                            0,
                        ]
                    }
                    3 => {
                        *o = [
                            19,
                            g.below(3),
                            1,
                            ctx.sciences[g.below(SCIENCES.len() as u32) as usize].0,
                            0,
                            0,
                        ]
                    }
                    4 => *o = [0, gen_options(&mut g), 0, 0, 0, 0],
                    5 => *o = [1, gen_options(&mut g), 0, 0, 0, 0],
                    6 => {
                        *o = [
                            2,
                            f32_bits(&mut g, 200.0),
                            f32_bits(&mut g, 200.0),
                            f32_bits(&mut g, 20.0),
                            gen_options(&mut g),
                            0,
                        ]
                    }
                    _ => {}
                }
            }
        }
        if (5..7).contains(&kind) {
            // batch R21: the players' money, and ops on their sciences, money and `onObjectCreated`; the cash hack mostly hacks
            for word in &mut cfg[8..11] {
                *word = *pick(&mut g, &[&0u32, &50, &500, &100_000]);
            }
            for o in &mut ops {
                match g.below(9) {
                    0..=1 => {
                        *o = [
                            19,
                            g.below(3),
                            1,
                            ctx.sciences[g.below(SCIENCES.len() as u32) as usize].0,
                            0,
                            0,
                        ]
                    }
                    2 => *o = [19, g.below(3), 0, 0, 0, 0],
                    3 => *o = [20, 0, 0, 0, 0, 0],
                    4 => {
                        *o = [
                            21,
                            g.below(3),
                            *pick(&mut g, &[&0u32, &30, &800, &100_000]),
                            0,
                            0,
                            0,
                        ]
                    }
                    5 if kind == 6 => *o = [1, gen_options(&mut g), 0, 0, 0, 0],
                    6 if kind == 5 => {
                        *o = [
                            22,
                            g.below(3),
                            (*pick(&mut g, &[&0.0f32, &0.1, &0.3])).to_bits(),
                            0,
                            0,
                            0,
                        ]
                    }
                    _ => {}
                }
            }
        }

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(
                kind,
                ctx.names.clone(),
                ctx.store.clone(),
                data.expect("read ok"),
                &cfg,
                &ops,
                &mut tally,
            );
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        if std::env::var_os("L9PH_TRACE").is_some() {
            eprintln!(
                "case {case} ({}): cfg {cfg:x?}, file {:?}, defs {defs:?}, ops {ops:?}",
                CLASSES[kind],
                show(&text)
            );
        }
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
            cpp::ref_l9sp_run(
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
        let what = || {
            format!(
                "case {case} ({}): cfg {cfg:x?}, file {:?}, defs {defs:?}, ops {ops:?}",
                CLASSES[kind],
                show(&text)
            )
        };
        read.eq(
            &outcome,
            &Outcome {
                status,
                detail,
                crc: 0,
            },
            what,
        );
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in [
                "\"eva ",
                "\"script ",
                "\"academy ",
                "\"audio ",
                "\"addsuper ",
                "\"defect ",
                "\"cleanup ",
                "\"spied ",
                "\"intent ",
                "\"mc ",
                "\"withdraw ",
                "\"float ",
                "\"oclat ",
                "\"oclown ",
                "\"findpos ",
            ] {
                *counts
                    .entry(key.trim_matches('"').trim().to_string())
                    .or_default() += l.matches(key).count() as u64;
            }
            if l.contains(" bounties [") && !l.contains(" bounties [0, 0, 0]") {
                *counts.entry("bounty set".to_string()).or_default() += 1;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?} file {:?} defs {defs:?}\n  rust {:?}\n  c++  {:?}", CLASSES[kind], show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
    }
    println!(
        "{label}: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} powers done, {} xfers, {} effect lines, at most {} objects made",
        tally.powers, tally.xfers, tally.effects, tally.made
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<10} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4,
        "the generator must reach every path"
    );
    if kinds.contains(&7) {
        let n = |k: &str| counts.get(k).copied().unwrap_or(0);
        assert!(
            n("oclat") * 4 > cases as u64
                && n("oclown") * 20 > cases as u64
                && n("findpos") * 20 > cases as u64,
            "the generator must reach every list"
        );
    }
    if kinds.contains(&5) {
        let n = |k: &str| counts.get(k).copied().unwrap_or(0);
        assert!(
            n("withdraw") * 20 > cases as u64 && n("bounty set") * 20 > cases as u64,
            "the generator must reach the theft and the bounty"
        );
    }
}
