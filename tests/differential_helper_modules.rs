// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the nine layer-9 helper and die modules of p1l9-helpers against upstream's own text (commit
//! 0a05454d), each compiled unchanged: `ObjectHelper.cpp` (the base: `sleepUntil`, xfer), `ObjectSMCHelper.cpp`,
//! `ObjectRepulsorHelper.cpp`, `ObjectWeaponStatusHelper.cpp`, `StatusDamageHelper.cpp`, `TempWeaponBonusHelper.cpp` and
//! `DestroyDie.cpp`, `KeepObjectDie.cpp`, `UpgradeDie.cpp`. See `differential/l9_helpers.inc` (it is `#include`d at the end of
//! `wrap_module.cpp`, in the module harness's setup, on the REAL `UpdateModule`, `DieModule`, `UpgradeCenter` and `INI`).
//!
//! # What is compared against what
//!
//! A program is a kind (the nine modules in turn) and 1 to 24 ops. The C++ object is a stand-in whose status mask is real and whose
//! calls (`setStatus`, `clearStatus`, `clearSpecialModelConditionStates`, `adjustModelConditionForWeaponStatus`,
//! `setWeaponBonusCondition`, `clearWeaponBonusCondition`, the drawable's tint, `destroyObject`, `removeUpgrade`) are lines of a log. The
//! Rust modules run on a [`Sim`] that implements [`HelperHost`] and [`DieHost`] and writes the same lines. The wake calls of
//! `setWakeFrame` go to the recorder of the harness's `GameLogic` on the C++ side and to `awaken_update_module` on the Rust side.
//! The dies read their data (`DeathTypes`, `VeterancyLevels`, `ExemptStatus`, `RequiredStatus`, `UpgradeToRemove`) from INI text through
//! the real INI, and `UpgradeDie` looks its upgrade up in a real `UpgradeCenter` loaded from the same `Upgrade UpN` blocks on both sides.
//!
//! **After every op** both sides write: the CATCH_ALL status, the wake record (calls, last frame, whether it named this object and this
//! module), the last sleep time `update()` returned, the log, the object's 45-bit status mask, the producer's upgrade mask, the status of
//! an xfer and its bytes or CRC, the module's `m_nextCallFrameAndPhase`, `m_indexInLogic`, `getUpdatePhase()` and the number of bits
//! in `getDisabledTypesToProcess()`, and the module's own words (`m_statusToHeal`/`m_frameToHeal`, `m_currentBonus`/`m_frameToRemove`).
//! Ops: `update()`, `doStatusDamage` / `doTempWeaponBonus`, set the frame, destroy the object / give it a drawable, `sleepUntil`, xfer save,
//! xfer load of the last save, XferCRC, `friend_setNextCallFrame`, set or clear a status bit; for the dies `onDie` (death type, veterancy,
//! status), give the producer an upgrade mask or none, the three xfers. Every byte of every dump is compared.
//!
//! # Defects found in the drafts (p1l9-helpers, Haiku), all fixed in the same change
//!
//! The drafts were pure functions returning "effects"; they were rebuilt as modules on the `UpdateModule` / `DieModule` bases, host
//! traits and xfer, and the comparison ran against upstream before any plant:
//!
//! * `getDisabledTypesToProcess()` was missing: `StatusDamageHelper` and `TempWeaponBonusHelper` return `DISABLEDMASK_ALL` (they run while
//!   the object is disabled); the other helpers inherit `DISABLEDMASK_NONE`.
//! * The constructors: `ObjectHelper` sleeps forever, and `StatusDamageHelper`, `TempWeaponBonusHelper` and `ObjectWeaponStatusHelper`
//!   wake a second time (forever, forever, `UPDATE_SLEEP_NONE`); the drafts had one wake or none.
//! * `xfer` was missing from `StatusDamageHelper` and `TempWeaponBonusHelper` (version, the base, the enum with `xferUser`, the frame).
//! * `doStatusDamage` and `doTempWeaponBonus` did not make the `setWakeFrame` call (the sleep was a return value); `sleepUntil` was a pure
//!   function that could not call it.
//!
//! # Results
//!
//! Run 2026-10-21 (`cargo test --release --features differential --test differential_helper_modules -- --nocapture`, default
//! 12,000 cases, 1,044 to 1,500 per module): **133,142 ops, 4,206,471 words of op dumps, 10,658 data dumps and 4,500 data reads
//! compared, 0 mismatches.** 29,119 effect lines were written, 24,024 xfers (save, load, CRC) ran, 29,831 wake calls were recorded,
//! 189 upgrade removals and 2,650 destroys happened, and 1,342 die data blocks threw (the same status and message on both sides). Inputs
//! upstream leaves undefined are not generated (status indices stay below `OBJECT_STATUS_COUNT`).
//!
//! # Planted bugs
//!
//! Each was applied to the Rust alone as a runtime switch (`L9_PLANT=<n>`, `logic::modules::plant`) and the test run at 4,000 cases;
//! `words` is the number of differing 4-byte words of the dump (0 in the clean run; 1,393,197 words are compared at 4,000 cases).
//! 24 switches; **24 caught** (numbers 50, 52 and 54 are unassigned).
//!
//! | # | planted bug | words |
//! |---|---|---|
//! | 30 | `sleepUntil(NEVER)` is a delay of `NEVER - now`, not forever | 3,613 |
//! | 31 | `sleepUntil` ignores a destroyed object | 5,497 |
//! | 32 | `ObjectHelper`'s constructor does not sleep forever | 47,523 |
//! | 33 | `ObjectSMCHelper::update` does not clear the special model conditions | 63,091 |
//! | 34 | `ObjectRepulsorHelper::update` clears the next status | 1,214 |
//! | 35 | `ObjectWeaponStatusHelper` runs in the normal phase | 8,196 |
//! | 36 | `ObjectWeaponStatusHelper`'s constructor does not start it active | 9,028 |
//! | 37 | `ObjectWeaponStatusHelper::update` sleeps forever | 4,430 |
//! | 38 | `doStatusDamage` does not clear a different status first | 44,395 |
//! | 39 | `doStatusDamage`: `m_frameToHeal` one frame late | 3,207 |
//! | 40 | `StatusDamageHelper` does not run while disabled | 6,358 |
//! | 41 | `doStatusDamage` truncates the duration instead of flooring it | 1,161 |
//! | 42 | `StatusDamageHelper::clearStatusCondition` keeps `m_frameToHeal` | 1,763 |
//! | 43 | `doTempWeaponBonus` does not tint | 13,230 |
//! | 44 | `clearTempWeaponBonus` clears the tint of an object with no drawable | 60,961 |
//! | 45 | `doTempWeaponBonus`: `m_frameToRemove` is the duration alone | 1,265 |
//! | 46 | `TempWeaponBonusHelper::xfer` omits the frame | 42,965 |
//! | 47 | `DestroyDie` destroys even when the die mux does not apply | 33,764 |
//! | 48 | `KeepObjectDie` destroys the object | 25,756 |
//! | 49 | `UpgradeDie` removes the upgrade though the producer lacks it | 11,576 |
//! | 51 | `UpgradeDie` ignores the die mux | 881 |
//! | 53 | `StatusDamageHelper::xfer` omits the status | 44,875 |
//! | 55 | `TempWeaponBonusHelper`'s constructor omits its own wake | 6,084 |
//! | 56 | `StatusDamageHelper`'s constructor omits its own wake | 6,358 |
//!
//! Plant 51 has the lowest count: `UpgradeDie`'s mux fields are sparse on purpose, so that the producer loses an upgrade 189 times over 1,047 programs.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, ObjectStatusTypes, OBJECT_STATUS_COUNT, OBJECT_STATUS_DESTROYED};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::die_module::DieModuleData;
use rts_lockstep_engine::logic::module::update_module::{UpdateModuleData, UPDATE_SLEEP_NONE};
use rts_lockstep_engine::logic::modules::destroy_die::DestroyDie;
use rts_lockstep_engine::logic::modules::keep_object_die::KeepObjectDie;
use rts_lockstep_engine::logic::modules::object_helper::{HelperHost, ObjectHelper};
use rts_lockstep_engine::logic::modules::object_repulsor_helper::ObjectRepulsorHelper;
use rts_lockstep_engine::logic::modules::object_smc_helper::ObjectSMCHelper;
use rts_lockstep_engine::logic::modules::object_weapon_status_helper::ObjectWeaponStatusHelper;
use rts_lockstep_engine::logic::modules::status_damage_helper::StatusDamageHelper;
use rts_lockstep_engine::logic::modules::temp_weapon_bonus_helper::TempWeaponBonusHelper;
use rts_lockstep_engine::logic::modules::upgrade_die::{UpgradeDie, UpgradeDieModuleData};
use rts_lockstep_engine::logic::modules::DieHost;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9helpers.bin");
const XFER_FILE: &str = "l9helpers.bin";

/// The kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 8] =
    ["ObjectSMCHelper", "ObjectRepulsorHelper", "ObjectWeaponStatusHelper", "StatusDamageHelper", "TempWeaponBonusHelper", "DestroyDie", "KeepObjectDie", "UpgradeDie"];

const LOG_SET_STATUS: i32 = 1;
const LOG_CLEAR_STATUS: i32 = 2;
const LOG_CLEAR_SMC: i32 = 3;
const LOG_ADJUST_MC: i32 = 4;
const LOG_SET_BONUS: i32 = 5;
const LOG_CLEAR_BONUS: i32 = 6;
const LOG_TINT_ON: i32 = 7;
const LOG_TINT_OFF: i32 = 8;
const LOG_DESTROY: i32 = 9;
const LOG_REMOVE_UPGRADE: i32 = 10;

/// The object's side of the world, as the C++ stand-ins answer it.
struct Sim {
    frame: u32,
    status: ObjectStatusMaskType,
    drawable: bool,
    level: i32,
    awakes: i32,
    last_wake: u32,
    log: Vec<(i32, u32)>,
    /// `findObjectByID` answers the producer object: present or not; its upgrade mask persists either way
    producer_present: bool,
    producer_mask: UpgradeMaskType,
    center: up_common::Ctx,
    destroyed_by_hook: bool,
}

impl Sim {
    fn upgrade_number(name: &[u8]) -> u32 {
        let s: String = name.iter().skip_while(|c| !c.is_ascii_digit()).map(|&c| c as char).collect();
        s.parse().unwrap_or(0)
    }
}

impl HelperHost for Sim {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn awaken_update_module(&mut self, frame: u32) {
        self.awakes += 1;
        self.last_wake = frame;
    }
    fn is_destroyed(&self) -> bool {
        self.status.test(OBJECT_STATUS_DESTROYED)
    }
    fn set_status(&mut self, s: ObjectStatusTypes) {
        self.log.push((LOG_SET_STATUS, s as u32));
        self.status.set(s, 1);
    }
    fn clear_status(&mut self, s: ObjectStatusTypes) {
        self.log.push((LOG_CLEAR_STATUS, s as u32));
        self.status.set(s, 0);
    }
    fn clear_special_model_condition_states(&mut self) {
        self.log.push((LOG_CLEAR_SMC, 0));
    }
    fn adjust_model_condition_for_weapon_status(&mut self) {
        self.log.push((LOG_ADJUST_MC, 0));
    }
    fn set_weapon_bonus_condition(&mut self, b: i32) {
        self.log.push((LOG_SET_BONUS, b as u32));
    }
    fn clear_weapon_bonus_condition(&mut self, b: i32) {
        self.log.push((LOG_CLEAR_BONUS, b as u32));
    }
    fn has_drawable(&self) -> bool {
        self.drawable
    }
    fn set_tint_status(&mut self, t: i32) {
        self.log.push((LOG_TINT_ON, t as u32));
    }
    fn clear_tint_status(&mut self, t: i32) {
        self.log.push((LOG_TINT_OFF, t as u32));
    }
}

impl DieHost for Sim {
    fn veterancy_level(&self) -> i32 {
        self.level
    }
    fn status_bits(&self) -> ObjectStatusMaskType {
        self.status
    }
    fn destroy_object(&mut self) {
        self.log.push((LOG_DESTROY, 0));
        self.destroyed_by_hook = true;
    }
    fn producer_exists(&self) -> bool {
        self.producer_present
    }
    fn find_upgrade(&mut self, name: &[u8]) -> bool {
        let c = &mut self.center;
        matches!(c.center.find_upgrade(&mut c.names, name), Ok(Some(_)))
    }
    fn producer_has_upgrade(&mut self, name: &[u8]) -> bool {
        self.template_mask(name).is_some_and(|t| self.producer_mask.test_for_all(&t))
    }
    fn producer_remove_upgrade(&mut self, name: &[u8]) {
        self.log.push((LOG_REMOVE_UPGRADE, Sim::upgrade_number(name)));
        if let Some(t) = self.template_mask(name) {
            self.producer_mask.clear_mask(&t);
        }
    }
}

impl Sim {
    fn template_mask(&self, name: &[u8]) -> Option<UpgradeMaskType> {
        // read-only lookup by the key the center assigned (names is only read through a clone of the generator's lookup)
        let mut names = self.center.names.clone();
        self.center.center.find_upgrade(&mut names, name).ok().flatten().map(|t| t.upgrade_mask())
    }
}

// -- the dump, in ref_l9help_run's format --

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
}
fn upgrade_mask_words(m: &UpgradeMaskType) -> [u64; 4] {
    let w = m.words();
    [w[0], w[1], 0, 0]
}

enum Module {
    Smc(ObjectSMCHelper),
    Repulsor(ObjectRepulsorHelper),
    WeaponStatus(ObjectWeaponStatusHelper),
    StatusDamage(StatusDamageHelper),
    TempBonus(TempWeaponBonusHelper),
    Destroy(DestroyDie),
    Keep(KeepObjectDie),
    Upgrade(UpgradeDie),
}

impl Module {
    fn helper(&mut self) -> Option<&mut ObjectHelper> {
        Some(match self {
            Module::Smc(m) => m.helper_mut(),
            Module::Repulsor(m) => m.helper_mut(),
            Module::WeaponStatus(m) => m.helper_mut(),
            Module::StatusDamage(m) => m.helper_mut(),
            Module::TempBonus(m) => m.helper_mut(),
            _ => return None,
        })
    }
    fn snapshot(&mut self) -> &mut dyn Snapshot {
        match self {
            Module::Smc(m) => m,
            Module::Repulsor(m) => m,
            Module::WeaponStatus(m) => m,
            Module::StatusDamage(m) => m,
            Module::TempBonus(m) => m,
            Module::Destroy(m) => m,
            Module::Keep(m) => m,
            Module::Upgrade(m) => m,
        }
    }
}

/// An xfer of the module: `how` 0 save (the bytes out), 1 load of `saved`, 2 CRC. Returns (status, bytes, crc).
fn run_xfer(m: &mut Module, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    let s = m.snapshot();
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => match x.xfer_snapshot(s) {
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
                Ok(()) => match x.xfer_snapshot(s) {
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
                Ok(()) => match x.xfer_snapshot(s) {
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

#[derive(Default)]
struct Slot {
    kind: usize,
    die: Option<DieModuleData>,
    upgrade_die: Option<UpgradeDieModuleData>,
}

fn read_block(ini: &mut Ini<'_>, s: &mut Slot) -> IniResult<()> {
    if s.kind == 7 {
        s.upgrade_die = Some(UpgradeDieModuleData::friend_new_module_data(Some(ini))?);
    } else {
        s.die = Some(DieModuleData::friend_new_module_data(Some(ini))?);
    }
    Ok(())
}

fn put_die_mux(v: &mut Vec<u8>, m: &rts_lockstep_engine::logic::module::die_module::DieMuxData) {
    put_u32(v, m.death_types);
    put_u32(v, m.veterancy_levels);
    put_words(v, m.exempt_status.words());
    put_words(v, m.required_status.words());
}

/// The Rust run: the dump, and the tallies of what the program reached.
#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    removals: u64,
    destroys: u64,
    wakes: u64,
}

#[allow(clippy::too_many_arguments)]
fn rust_run(kind: usize, slot: &Slot, ctx: up_common::Ctx, ops: &[[u32; 6]], t: &mut Tally) -> (Vec<u8>, Vec<u8>) {
    let mut v = Vec::new();
    let mut pre = Vec::new();
    let mut sim = Sim {
        frame: 0,
        status: ObjectStatusMaskType::default(),
        drawable: false,
        level: 0,
        awakes: 0,
        last_wake: 0,
        log: Vec::new(),
        producer_present: false,
        producer_mask: UpgradeMaskType::default(),
        center: ctx,
        destroyed_by_hook: false,
    };
    let data = Arc::new(UpdateModuleData::new());
    let mut m = match kind {
        0 => Module::Smc(ObjectSMCHelper::new(0, Some(data), &mut sim).unwrap()),
        1 => Module::Repulsor(ObjectRepulsorHelper::new(0, Some(data), &mut sim).unwrap()),
        2 => Module::WeaponStatus(ObjectWeaponStatusHelper::new(0, Some(data), &mut sim).unwrap()),
        3 => Module::StatusDamage(StatusDamageHelper::new(0, Some(data), &mut sim).unwrap()),
        4 => Module::TempBonus(TempWeaponBonusHelper::new(0, Some(data), &mut sim).unwrap()),
        5 => {
            let d = slot.die.clone().unwrap();
            put_die_mux(&mut pre, &d.die_mux_data);
            Module::Destroy(DestroyDie::new(0, Some(Arc::new(d))).unwrap())
        }
        6 => {
            let d = slot.die.clone().unwrap();
            put_die_mux(&mut pre, &d.die_mux_data);
            Module::Keep(KeepObjectDie::new(0, Some(Arc::new(d))).unwrap())
        }
        _ => {
            let d = slot.upgrade_die.clone().unwrap();
            put_die_mux(&mut pre, &d.die.die_mux_data);
            put_u32(&mut pre, d.upgrade_name.len() as u32);
            pre.extend_from_slice(&d.upgrade_name);
            Module::Upgrade(UpgradeDie::new(0, Some(Arc::new(d))).unwrap())
        }
    };
    let mut last_sleep = UPDATE_SLEEP_NONE.0;
    let mut saved: Vec<u8> = Vec::new();
    let helper_kind = kind < 5;
    for o in ops {
        sim.log.clear();
        let mut xst = -2;
        let mut blen = 0usize;
        let mut crc = 0u32;
        let mut bytes: Vec<u8> = Vec::new();
        if helper_kind {
            match o[0] {
                0 => {
                    last_sleep = match &mut m {
                        Module::Smc(h) => h.update(&mut sim),
                        Module::Repulsor(h) => h.update(&mut sim),
                        Module::WeaponStatus(h) => h.update(&mut sim),
                        Module::StatusDamage(h) => h.update(&mut sim),
                        Module::TempBonus(h) => h.update(&mut sim),
                        _ => unreachable!(),
                    }
                    .0
                }
                1 => match &mut m {
                    Module::StatusDamage(h) => h.do_status_damage(&mut sim, o[1] as i32, f32::from_bits(o[2])),
                    Module::TempBonus(h) => h.do_temp_weapon_bonus(&mut sim, o[1] as i32, o[2]),
                    _ => {}
                },
                2 => sim.frame = o[1],
                3 => {
                    sim.status.set(OBJECT_STATUS_DESTROYED, i32::from(o[1] & 1 != 0));
                    sim.drawable = o[1] & 2 != 0;
                }
                4 => {
                    let h: &ObjectHelper = m.helper().unwrap();
                    h.sleep_until(&mut sim, o[1]);
                }
                5 => {
                    let (st, b, _) = run_xfer(&mut m, 0, &[]);
                    xst = st;
                    blen = b.len();
                    saved = b.clone();
                    bytes = b;
                }
                6 => xst = run_xfer(&mut m, 1, &saved).0,
                7 => {
                    let (st, _, c) = run_xfer(&mut m, 2, &[]);
                    xst = st;
                    crc = c;
                }
                8 => m.helper().unwrap().update_module_mut().friend_set_next_call_frame(o[1]),
                _ => {
                    if o[1] < OBJECT_STATUS_COUNT as u32 {
                        sim.status.set(o[1] as i32, i32::from(o[2] & 1 != 0));
                    }
                }
            }
        } else {
            match o[0] {
                0 => {
                    sim.level = o[2] as i32;
                    sim.status = ObjectStatusMaskType::from_words([u64::from(o[3]) | u64::from(o[4]) << 32, 0, 0, 0]);
                    let d = o[1] as i32;
                    match &mut m {
                        Module::Destroy(h) => h.on_die(&mut sim, d),
                        Module::Keep(h) => h.on_die(&mut sim, d),
                        Module::Upgrade(h) => h.on_die(&mut sim, d),
                        _ => unreachable!(),
                    }
                }
                1 => {
                    sim.producer_present = o[1] & 1 != 0;
                    if sim.producer_present {
                        let mut mk = UpgradeMaskType::default();
                        for i in 0..128 {
                            if o[2 + i / 32] >> (i % 32) & 1 != 0 {
                                mk.set(i as i32, 1);
                            }
                        }
                        sim.producer_mask = mk;
                    }
                }
                2 => {
                    let (st, b, _) = run_xfer(&mut m, 0, &[]);
                    xst = st;
                    blen = b.len();
                    saved = b.clone();
                    bytes = b;
                }
                3 => xst = run_xfer(&mut m, 1, &saved).0,
                4 => {
                    let (st, _, c) = run_xfer(&mut m, 2, &[]);
                    xst = st;
                    crc = c;
                }
                _ => {}
            }
        }
        t.effects += sim.log.len() as u64;
        t.removals += sim.log.iter().filter(|l| l.0 == LOG_REMOVE_UPGRADE).count() as u64;
        t.destroys += sim.log.iter().filter(|l| l.0 == LOG_DESTROY).count() as u64;
        t.xfers += u64::from(xst != -2);
        put_i32(&mut v, 0);
        put_i32(&mut v, sim.awakes);
        put_u32(&mut v, sim.last_wake);
        let named = if helper_kind { u32::from(sim.awakes > 0) } else { 0 };
        put_u32(&mut v, named);
        put_u32(&mut v, if helper_kind { named } else { 1 });
        put_u32(&mut v, last_sleep);
        put_u32(&mut v, sim.log.len() as u32);
        for (k, a) in &sim.log {
            put_i32(&mut v, *k);
            put_u32(&mut v, *a);
        }
        put_words(&mut v, sim.status.words());
        put_words(&mut v, upgrade_mask_words(&sim.producer_mask));
        put_i32(&mut v, xst);
        put_u32(&mut v, blen as u32);
        for b in &bytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let disabled = match &m {
            Module::StatusDamage(h) => Some(h.get_disabled_types_to_process().count()),
            Module::TempBonus(h) => Some(h.get_disabled_types_to_process().count()),
            _ => None,
        };
        if let Some(h) = m.helper() {
            let um = h.update_module();
            put_u32(&mut v, um.friend_get_priority());
            put_i32(&mut v, um.friend_get_index_in_logic());
            put_u32(&mut v, um.get_update_phase() as u32);
            put_u32(&mut v, disabled.unwrap_or_else(|| um.get_disabled_types_to_process().count()) as u32);
        }
        match &m {
            Module::StatusDamage(h) => {
                put_i32(&mut v, h.status_to_heal);
                put_u32(&mut v, h.frame_to_heal);
            }
            Module::TempBonus(h) => {
                put_i32(&mut v, h.current_bonus);
                put_u32(&mut v, h.frame_to_remove);
            }
            _ => {}
        }
        put_u32(&mut v, u32::from(sim.destroyed_by_hook));
    }
    t.wakes += sim.awakes as u64;
    (pre, v)
}

// -- the generators --

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

/// The block `D` of a die: the mux fields (each present with some probability) and, for `UpgradeDie`, `UpgradeToRemove`; now and then a
/// bogus token or an unknown field, which both sides must refuse alike.
fn gen_die_text(g: &mut Gen, kind: usize, n_up: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    // UpgradeDie only does something when the mux applies: its fields are sparser, so that it does
    let thin = if kind == 7 { 3 } else { 1 };
    if g.below(100) * thin < 70 {
        lines.push(format!("DeathTypes = {}", death_tokens(g)));
    }
    if g.below(100) * thin < 50 {
        lines.push(format!("VeterancyLevels = {}", level_tokens(g)));
    }
    if g.below(100) * thin < 40 {
        lines.push(format!("ExemptStatus = {}", status_tokens(g)));
    }
    if g.below(100) * thin < 40 {
        lines.push(format!("RequiredStatus = {}", status_tokens(g)));
    }
    if kind == 7 && g.below(100) < 90 {
        lines.push(format!(
            "UpgradeToRemove = {}",
            match g.below(10) {
                0 => "Nope".to_string(),
                1 => "upgrade_unknown".to_string(),
                _ => format!("Up{}", g.below(n_up as u32 + 1)),
            }
        ));
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

fn pick_frame(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1..=4 => g.below(5000),
        5 => u32::MAX - g.below(100),
        6 => 0x3fff_ffff - g.below(50) + g.below(100),
        _ => g.u32(),
    }
}

fn gen_duration(g: &mut Gen) -> f32 {
    match g.below(8) {
        0..=2 => g.uniform(-5.0, 120.0),
        3 => g.below(300) as f32,
        4 => g.below(300) as f32 + 0.5,
        5 => g.mixed(),
        6 => g.special(),
        _ => g.uniform(-1.0e10, 1.0e10),
    }
}

fn gen_ops(g: &mut Gen, kind: usize, n_up: usize) -> Vec<[u32; 6]> {
    let n = 1 + g.below(24) as usize;
    (0..n)
        .map(|i| {
            if kind < 5 {
                let op = match g.below(32) {
                    0..=5 => 0,
                    6..=12 => 1,
                    13..=15 => 2,
                    16..=17 => 3,
                    18..=21 => 4,
                    22..=23 => 5,
                    24..=25 => 6,
                    26 => 7,
                    27 => 8,
                    _ => 9,
                };
                match op {
                    1 if kind == 3 => [1, g.below(OBJECT_STATUS_COUNT as u32), gen_duration(g).to_bits(), 0, 0, 0],
                    1 if kind == 4 => {
                        let dur = match g.below(5) {
                            0 => 0,
                            1 => u32::MAX - g.below(100),
                            2 => g.u32(),
                            _ => g.below(300),
                        };
                        [1, (g.below(40) as i32 - 3) as u32, dur, 0, 0, 0]
                    }
                    1 => [1, 0, 0, 0, 0, 0],
                    2 => [2, pick_frame(g), 0, 0, 0, 0],
                    3 => [3, g.below(4), 0, 0, 0, 0],
                    4 => [4, if g.below(6) == 0 { 0 } else { pick_frame(g) }, 0, 0, 0, 0],
                    8 => [8, pick_frame(g), 0, 0, 0, 0],
                    9 => [9, g.below(OBJECT_STATUS_COUNT as u32 + 2), g.below(2), 0, 0, 0],
                    _ => [op, 0, 0, 0, 0, 0],
                }
            } else {
                if i == 0 && g.below(10) < 6 {
                    // most programs start with a producer that has some upgrades
                    let mut m = [0u32; 4];
                    for b in 0..n_up + 2 {
                        if g.below(3) != 0 {
                            m[b / 32] |= 1 << (b % 32);
                        }
                    }
                    return [1, 1, m[0], m[1], m[2], m[3]];
                }
                match g.below(12) {
                    0..=5 => {
                        let status = g.u64() & g.u64() & ((1u64 << 45) - 1);
                        [0, 1 + g.below(20), 1 + g.below(3), status as u32, (status >> 32) as u32, 0]
                    }
                    6..=8 => {
                        let mut m = [0u32; 4];
                        for b in 0..n_up + 2 {
                            if g.below(2) == 0 {
                                m[b / 32] |= 1 << (b % 32);
                            }
                        }
                        [1, g.below(4), m[0], m[1], m[2], m[3]]
                    }
                    9 => [2, 0, 0, 0, 0, 0],
                    10 => [3, 0, 0, 0, 0, 0],
                    _ => [4, 0, 0, 0, 0, 0],
                }
            }
        })
        .collect()
}

#[test]
fn helper_and_die_modules_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("L9HELP_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(12_000);
    let mut g = Gen::new(0x1091_7E1B);
    let table = blocks();
    let mut read = Check::new("die data read (throw, message)");
    let mut data = Check::new("die data (mux, upgrade name)");
    let mut run = Check::new("helper and die ops (words)");
    let (mut read_failed, mut ops_run, mut compared_cases) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 8];
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 8;
        let n_up = 1 + g.below(6) as usize;
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
        let mut ctx = Ctx::new(true, true);
        Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        cpp_reset(true, true);
        assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);
        let mut slot = Slot { kind, ..Default::default() };
        let name = format!("Data\\INI\\Hm{}.ini", case % 5).into_bytes();
        let clean = g.below(10) < 7;
        let text = if kind >= 5 { gen_die_text(&mut g, kind, n_up, clean) } else { Vec::new() };
        let mut read_outcome = None;
        if kind >= 5 {
            let blocks = [BlockParse { token: "D", parse: read_block }];
            let r = Ini::load(&name, &text, IniLoadType::Overwrite, None, &blocks, &mut slot);
            read_outcome = Some(rust_outcome(r, 0).expect("no undefined input in the die generator"));
        }
        let ops = gen_ops(&mut g, kind, n_up);
        let rust = if read_outcome.as_ref().is_none_or(|o| o.status == 0) { Some(rust_run(kind, &slot, ctx, &ops, &mut tally)) } else { None };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 20];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9help_run(
                kind as i32,
                cfile.as_ptr(),
                cxfer.as_ptr(),
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
        let what = || format!("case {case} ({}): file {:?}, ops {ops:?}", CLASSES[kind], show(&text));
        if let Some(ro) = &read_outcome {
            let detail = match status {
                0 | 4 => Vec::new(),
                1 => msg[..code as usize].to_vec(),
                2 | 3 => code.to_le_bytes().to_vec(),
                _ => panic!("C++ threw something unexpected (status {status})"),
            };
            read.eq(ro, &Outcome { status, detail, crc: 0 }, what);
        } else {
            assert_eq!(status, 0);
        }
        let Some((pre, body)) = rust else {
            if std::env::var("L9HELP_SHOW").is_ok() && read_failed < 6 {
                eprintln!("read failed: {:?} {:?}", read_outcome.as_ref().map(|o| (o.status, show(&o.detail))), show(&text));
            }
            read_failed += 1;
            continue;
        };
        data.eq(&pre[..], &buf[..pre.len().min(buf.len())], what);
        let cbody = &buf[pre.len().min(buf.len())..];
        if body != cbody && shown < 4 {
            shown += 1;
            let i = body.chunks(4).zip(cbody.chunks(4)).position(|(a, b)| a != b).unwrap_or(0);
            eprintln!("--- case {case} ({}) file {:?} ops {ops:?}\n  word {i}: rust {:?} c++ {:?} (lengths {} {})", CLASSES[kind], show(&text), body.chunks(4).nth(i), cbody.chunks(4).nth(i), body.len(), cbody.len());
        }
        run.eq(&body.len(), &cbody.len(), what);
        for (a, b) in body.chunks(4).zip(cbody.chunks(4)) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
        compared_cases += 1;
    }
    println!(
        "helper and die modules: {cases} cases ({per_kind:?} compared per kind), {read_failed} die data blocks threw, {ops_run} ops, {} effect lines, \
         {} xfers, {} wake calls, {} upgrade removals, {} destroys",
        tally.effects, tally.xfers, tally.wakes, tally.removals, tally.destroys
    );
    read.finish();
    data.finish();
    run.finish();
    assert!(
        compared_cases * 2 > cases as u64 && tally.effects > cases as u64 && tally.xfers > cases as u64 / 2 && tally.removals * 100 > cases as u64 && tally.destroys > 20,
        "the generator must reach every path"
    );
}
