// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponSet.h  (WeaponSet, WeaponChoiceCriteria, WeaponLockType,
//     CanAttackResult, WeaponSetConditionType)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp  (class WeaponSet)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   WeaponSet.cpp: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponSet`: an object's current weapons (one per slot), which of them is current and whether that is locked, and
//! the questions the AI and the UI ask of the set ("can this object attack that one", "which weapon is best against
//! it"). The template side (`WeaponTemplateSet`, what a thing template says a set is) is `logic::weapon_set`; the
//! weapons are `logic::weapon_fire::Weapon`.
//!
//! # The shape
//!
//! The set owns its [`Weapon`]s. Every question that reaches an object, a player, the partition manager or a
//! module goes through [`WeaponHost`], called where upstream makes the call. The object that owns the set takes
//! it out of itself for a call that changes the set (`std::mem::take`), so that the host can be `World` while the
//! set is `&mut`; while a set is out, the host cannot see it. Nothing those calls ask of the host reaches the
//! source's own set. **Firing is different**: firing reaches the world (damage, a temporary weapon) and what it
//! reaches asks the source's set whether the reload time is shared and sets the other weapons' reload, so
//! `logic::world::object_weapons` takes only the firing *weapon* out of its slot. The first version took the whole
//! set, and the differential test found the difference at program 7910 of 30,000.
//!
//! # Upstream defects kept
//!
//! - `getAbleToUseWeaponAgainstTarget` loops `slot` over `WEAPONSLOT_COUNT - 1` slots but always tests
//!   `m_weapons[m_curWeapon]`: the same weapon twice, and no other.
//! - `xfer` writes `m_hasDamageWeapon` twice and never writes `m_hasPitchLimit`, so a loaded set has no pitch
//!   limit until its next `updateWeaponSet`; `crc` writes nothing.
//! - `updateWeaponSet` loads each new weapon's ammo while `m_curWeaponTemplateSet` is still the *old* set, so a
//!   shared reload time is the old set's.
//! - `getNthCommandSourceMask` of a set with no template set is `NULL` (0).
//!
//! # Where upstream is undefined
//!
//! `chooseBestWeaponForTarget` with a weapon in a slot and no template set dereferences the null
//! `m_curWeaponTemplateSet`: [`WeaponError::NullObject`]; so does a load that finds a weapon but no template set.
//!
//! # Verified
//!
//! Against upstream's own `WeaponSet.cpp` (the class `WeaponSet`) by `tests/differential_weapon_fire.rs`.

use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{
    KINDOFMASK_NONE,
    KINDOF_DISGUISER, KINDOF_HERO, KINDOF_IMMOBILE, KINDOF_MINE, KINDOF_SMALL_MISSILE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_UNATTACKABLE,
    KINDOF_BALLISTIC_MISSILE, KINDOF_DEMOTRAP, KINDOF_INFANTRY, KINDOF_PARACHUTE, KINDOF_PROJECTILE, KINDOF_VEHICLE,
};
use crate::common::model_state::{
    ModelConditionFlags, MODELCONDITION_BETWEEN_FIRING_SHOTS_A, MODELCONDITION_BETWEEN_FIRING_SHOTS_B, MODELCONDITION_BETWEEN_FIRING_SHOTS_C,
    MODELCONDITION_FIRING_A, MODELCONDITION_FIRING_B, MODELCONDITION_FIRING_C, MODELCONDITION_INVALID, MODELCONDITION_PREATTACK_A,
    MODELCONDITION_PREATTACK_B, MODELCONDITION_PREATTACK_C, MODELCONDITION_RELOADING_A, MODELCONDITION_RELOADING_B, MODELCONDITION_RELOADING_C,
    MODELCONDITION_USING_WEAPON_A, MODELCONDITION_USING_WEAPON_B, MODELCONDITION_USING_WEAPON_C,
};
use crate::common::object_status_types::{
    OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_IGNORING_STEALTH, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK_FROM_AI,
    OBJECT_STATUS_STEALTHED,
};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageTypeFlags, DAMAGE_KILLPILOT, DAMAGE_UNRESISTABLE};
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_TARGETABLE;
use crate::logic::weapon::{
    WeaponError, WEAPON_ANTI_AIRBORNE_INFANTRY, WEAPON_ANTI_AIRBORNE_VEHICLE, WEAPON_ANTI_BALLISTIC_MISSILE, WEAPON_ANTI_GROUND, WEAPON_ANTI_MINE,
    WEAPON_ANTI_PARACHUTE, WEAPON_ANTI_PROJECTILE, WEAPON_ANTI_SMALL_MISSILE,
};
use crate::logic::weapon_fire::{float_to_unsigned, SharedReload, Slots, Weapon, WeaponSnapshot, WeaponXferError, PRIMARY_WEAPON, SLOTS};
use crate::logic::weapon_host::{
    is_forced_attack, AbleToAttackType, CanAttackResult, CommandSourceType, WeaponHost, WeaponTemplateSetRef, WeaponXferLookups,
    ATTACK_TUNNEL_NETWORK_GUARD,
};
use crate::logic::weapon_types::WeaponStatus;

type WResult<T> = Result<T, WeaponError>;

/// `enum WeaponSetConditionType`.
pub type WeaponSetConditionType = i32;
/// `WSF_INVALID`.
pub const WSF_INVALID: WeaponSetConditionType = -1;
/// `WSF_NONE`.
pub const WSF_NONE: WeaponSetConditionType = 0;
/// `WSF_FIRING`.
pub const WSF_FIRING: WeaponSetConditionType = 1;
/// `WSF_BETWEEN`.
pub const WSF_BETWEEN: WeaponSetConditionType = 2;
/// `WSF_RELOADING`.
pub const WSF_RELOADING: WeaponSetConditionType = 3;
/// `WSF_PREATTACK`.
pub const WSF_PREATTACK: WeaponSetConditionType = 4;
/// `WSF_COUNT`.
pub const WSF_COUNT: WeaponSetConditionType = 5;

/// `enum WeaponChoiceCriteria`.
pub type WeaponChoiceCriteria = i32;
/// `PREFER_MOST_DAMAGE`: choose the weapon that will do the most damage.
pub const PREFER_MOST_DAMAGE: WeaponChoiceCriteria = 0;
/// `PREFER_LONGEST_RANGE`: choose the weapon with the longest range (that will do nonzero damage).
pub const PREFER_LONGEST_RANGE: WeaponChoiceCriteria = 1;

/// `enum WeaponLockType`.
pub type WeaponLockType = i32;
/// `NOT_LOCKED`.
pub const NOT_LOCKED: WeaponLockType = 0;
/// `LOCKED_TEMPORARILY`: locked until the clip is empty, or the current "attack" state exits.
pub const LOCKED_TEMPORARILY: WeaponLockType = 1;
/// `LOCKED_PERMANENTLY`: locked until explicitly unlocked or the lock is changed to another weapon.
pub const LOCKED_PERMANENTLY: WeaponLockType = 2;

/// `getVictimAntiMask(victim)`.
fn get_victim_anti_mask<H: WeaponHost>(host: &H, victim: ObjectId) -> u32 {
    if host.is_kind_of(victim, KINDOF_SMALL_MISSILE) {
        // all missiles are also projectiles!
        WEAPON_ANTI_SMALL_MISSILE
    } else if host.is_kind_of(victim, KINDOF_BALLISTIC_MISSILE) {
        WEAPON_ANTI_BALLISTIC_MISSILE
    } else if host.is_kind_of(victim, KINDOF_PROJECTILE) {
        WEAPON_ANTI_PROJECTILE
    } else if host.is_kind_of(victim, KINDOF_MINE) || host.is_kind_of(victim, KINDOF_DEMOTRAP) {
        WEAPON_ANTI_MINE | WEAPON_ANTI_GROUND
    } else if host.is_airborne_target(victim) {
        if host.is_kind_of(victim, KINDOF_VEHICLE) {
            WEAPON_ANTI_AIRBORNE_VEHICLE
        } else if host.is_kind_of(victim, KINDOF_INFANTRY) {
            WEAPON_ANTI_AIRBORNE_INFANTRY
        } else if host.is_kind_of(victim, KINDOF_PARACHUTE) {
            WEAPON_ANTI_PARACHUTE
        } else {
            // "Object %s is being targetted as airborne, but is not infantry, nor vehicle. Is this legit?"
            0
        }
    } else {
        WEAPON_ANTI_GROUND
    }
}

/// `class WeaponSet`. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponSet {
    /// `m_curWeaponTemplateSet`.
    cur_weapon_template_set: Option<WeaponTemplateSetRef>,
    /// `m_weapons`.
    weapons: Slots,
    /// `m_curWeapon`.
    cur_weapon: i32,
    /// `m_curWeaponLockedStatus`.
    cur_weapon_locked_status: WeaponLockType,
    /// `m_filledWeaponSlotMask`.
    filled_weapon_slot_mask: u32,
    /// `m_totalAntiMask`: anti mask of all current weapons.
    total_anti_mask: i32,
    /// `m_totalDamageTypeMask`: damage type mask of all current weapons.
    total_damage_type_mask: DamageTypeFlags,
    /// `m_hasPitchLimit`.
    has_pitch_limit: bool,
    /// `m_hasDamageWeapon`.
    has_damage_weapon: bool,
}

impl Default for WeaponSet {
    fn default() -> Self {
        WeaponSet::new()
    }
}

impl WeaponSet {
    /// `WeaponSet::WeaponSet()`.
    pub fn new() -> WeaponSet {
        WeaponSet {
            cur_weapon: PRIMARY_WEAPON,
            cur_weapon_locked_status: NOT_LOCKED,
            cur_weapon_template_set: None,
            filled_weapon_slot_mask: 0,
            total_anti_mask: 0,
            total_damage_type_mask: DamageTypeFlags::new(),
            has_pitch_limit: false,
            has_damage_weapon: false,
            weapons: [None, None, None],
        }
    }

    // ---- the inline members of WeaponSet.h ----------------------------------------------------------------------------

    /// `hasAnyWeapon()`.
    pub fn has_any_weapon(&self) -> bool {
        self.filled_weapon_slot_mask != 0
    }
    /// `hasAnyDamageWeapon()`.
    pub fn has_any_damage_weapon(&self) -> bool {
        self.has_damage_weapon
    }
    /// batch R37: sets what [`WeaponSet::has_any_damage_weapon`] answers, for the tests whose objects carry no weapons (the weapon layer is
    /// layer 12's).
    pub fn friend_set_has_damage_weapon(&mut self, has: bool) {
        self.has_damage_weapon = has;
    }
    /// `hasWeaponToDealDamageType(typeToDeal)`.
    pub fn has_weapon_to_deal_damage_type(&self, type_to_deal: i32) -> bool {
        self.total_damage_type_mask.test(type_to_deal)
    }
    /// `m_totalDamageTypeMask.count()`.
    pub fn total_damage_type_count(&self) -> i32 {
        self.total_damage_type_mask.count()
    }
    /// `hasSingleDamageType(typeToDeal)`.
    pub fn has_single_damage_type(&self, type_to_deal: i32) -> bool {
        self.total_damage_type_mask.test(type_to_deal) && self.total_damage_type_mask.count() == 1
    }
    /// `isCurWeaponLocked()`.
    pub fn is_cur_weapon_locked(&self) -> bool {
        self.cur_weapon_locked_status != NOT_LOCKED
    }
    /// `getCurWeapon()`: the weapon in the current slot (`None` for `NULL`).
    pub fn get_cur_weapon(&self) -> Option<&Weapon> {
        self.weapons.get(self.cur_weapon as usize).and_then(|w| w.as_ref())
    }
    /// `getCurWeapon()`, to change.
    pub fn get_cur_weapon_mut(&mut self) -> Option<&mut Weapon> {
        self.weapons.get_mut(self.cur_weapon as usize).and_then(|w| w.as_mut())
    }
    /// `getCurWeaponSlot()`.
    pub fn get_cur_weapon_slot(&self) -> i32 {
        self.cur_weapon
    }
    /// `m_curWeaponLockedStatus`.
    pub fn cur_weapon_locked_status(&self) -> WeaponLockType {
        self.cur_weapon_locked_status
    }
    /// `m_curWeaponTemplateSet`.
    pub fn cur_weapon_template_set(&self) -> Option<WeaponTemplateSetRef> {
        self.cur_weapon_template_set
    }
    /// `m_filledWeaponSlotMask`.
    pub fn filled_weapon_slot_mask(&self) -> u32 {
        self.filled_weapon_slot_mask
    }
    /// `m_totalAntiMask`.
    pub fn total_anti_mask(&self) -> i32 {
        self.total_anti_mask
    }
    /// `m_hasPitchLimit`.
    pub fn has_pitch_limit(&self) -> bool {
        self.has_pitch_limit
    }
    /// `getWeaponInWeaponSlot(wslot)`.
    pub fn get_weapon_in_weapon_slot(&self, wslot: i32) -> Option<&Weapon> {
        self.weapons.get(wslot as usize).and_then(|w| w.as_ref())
    }
    /// `getWeaponInWeaponSlot(wslot)`, to change.
    pub fn get_weapon_in_weapon_slot_mut(&mut self, wslot: i32) -> Option<&mut Weapon> {
        self.weapons.get_mut(wslot as usize).and_then(|w| w.as_mut())
    }
    /// The weapons, for the owner that hands them out.
    pub fn slots(&self) -> &Slots {
        &self.weapons
    }
    /// The weapons, to change.
    pub fn slots_mut(&mut self) -> &mut Slots {
        &mut self.weapons
    }

    /// `getNthCommandSourceMask(n)`: "m_curWeaponTemplateSet ? ... : NULL".
    pub fn get_nth_command_source_mask<H: WeaponHost>(&self, host: &H, n: usize) -> u32 {
        match self.cur_weapon_template_set {
            Some(r) => host.weapon_template_set(r).get_nth_command_source_mask(n),
            None => 0,
        }
    }

    /// `isSharedReloadTime()`.
    pub fn is_shared_reload_time<H: WeaponHost>(&self, host: &H) -> bool {
        match self.cur_weapon_template_set {
            Some(r) => host.weapon_template_set(r).is_shared_reload_time(),
            None => false,
        }
    }

    /// The effect of a shared reload on this set's weapons (what a weapon fired from outside the set returns).
    pub fn apply_shared_reload(&mut self, effect: Option<SharedReload>) {
        self.apply(effect);
    }

    /// The effect of a shared reload on this set's weapons.
    fn apply(&mut self, effect: Option<SharedReload>) {
        if let Some(e) = effect {
            e.apply_to_slots(&mut self.weapons);
        }
    }

    // ---- snapshot -----------------------------------------------------------------------------------------------------

    /// `WeaponSet::crc`: nothing.
    pub fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `WeaponSet::xfer` (version 1). `lookups` are the thing factory, the thing templates and the weapon store.
    pub fn xfer<L: WeaponXferLookups>(&mut self, lookups: &mut L, xfer: &mut dyn Xfer) -> Result<(), WeaponXferError> {
        // version
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            let mut tt_name: Vec<u8> = Vec::new();
            let mut ws_flags = crate::logic::weapon_set_flags::WeaponSetFlags::new();

            xfer.xfer_ascii_string(&mut tt_name)?;
            ws_flags.xfer(xfer)?;

            if tt_name.is_empty() {
                self.cur_weapon_template_set = None;
            } else {
                let tt = lookups.find_thing_template(&tt_name).ok_or(WeaponXferError::InvalidData)?; // throw INI_INVALID_DATA
                self.cur_weapon_template_set =
                    Some(lookups.find_weapon_template_set_by_flags(tt, &ws_flags).ok_or(WeaponXferError::InvalidData)?);
            }
        } else if xfer.get_xfer_mode() == XferMode::Save {
            let mut tt_name: Vec<u8> = Vec::new(); // leave 'em empty in case we're null
            let mut ws_flags = crate::logic::weapon_set_flags::WeaponSetFlags::new();
            if let Some(r) = self.cur_weapon_template_set {
                let set = lookups.weapon_template_set(r);
                let tt = set.thing_template().ok_or(WeaponXferError::InvalidData)?; // throw INI_INVALID_DATA
                ws_flags = *set.weapon_set_flags();
                tt_name = lookups.thing_template_name(tt);
            }
            xfer.xfer_ascii_string(&mut tt_name)?;
            ws_flags.xfer(xfer)?;
        }

        for i in 0..SLOTS {
            let mut has_weapon_in_slot = self.weapons[i].is_some();
            xfer.xfer_bool(&mut has_weapon_in_slot)?;
            if has_weapon_in_slot {
                if xfer.get_xfer_mode() == XferMode::Load && self.weapons[i].is_none() {
                    // (a weapon but no template set is a null dereference upstream)
                    let r = self.cur_weapon_template_set.ok_or(WeaponXferError::Xfer(XferStatus::ErrorUnknown))?;
                    let mut key = lookups.weapon_template_set(r).get_nth(i);
                    if key.is_none() {
                        // "xfer backwards compatibility code - old save file???"
                        key = lookups.weapon_template_set(r).get_nth(0);
                    }
                    let template = key.and_then(|k| lookups.weapon_template_index(k)).ok_or(WeaponXferError::Xfer(XferStatus::ErrorUnknown))?;
                    self.weapons[i] = Some(Weapon::for_load(template, i as i32));
                }
                // xferSnapshot(m_weapons[i])
                let weapon = self.weapons[i].take().ok_or(WeaponXferError::Xfer(XferStatus::ErrorUnknown))?;
                let current_name = lookups.weapon_template_name(weapon.template());
                let templates = if xfer.get_xfer_mode() == XferMode::Load { lookups.weapon_template_names() } else { Vec::new() };
                let mut snap = WeaponSnapshot { weapon, current_name, templates, error: None };
                let r = xfer.xfer_snapshot(&mut snap);
                self.weapons[i] = Some(snap.weapon);
                if let Some(e) = snap.error {
                    return Err(e);
                }
                r?;
            }
        }
        let mut cur = self.cur_weapon.to_le_bytes();
        xfer.xfer_user(&mut cur)?;
        self.cur_weapon = i32::from_le_bytes(cur);
        let mut locked = self.cur_weapon_locked_status.to_le_bytes();
        xfer.xfer_user(&mut locked)?;
        self.cur_weapon_locked_status = i32::from_le_bytes(locked);
        xfer.xfer_unsigned_int(&mut self.filled_weapon_slot_mask)?;
        xfer.xfer_int(&mut self.total_anti_mask)?;
        xfer.xfer_bool(&mut self.has_damage_weapon)?;
        xfer.xfer_bool(&mut self.has_damage_weapon)?;

        self.total_damage_type_mask.xfer(xfer)?; // BitSet has built in xfer
        Ok(())
    }

    /// `WeaponSet::loadPostProcess()` is empty; the weapons' (`Weapon::loadPostProcess`) are run by the owner, which
    /// is the snapshot machinery upstream: forget a projectile stream that no longer exists.
    pub fn load_post_process_weapons<H: WeaponHost>(&mut self, host: &H) {
        for w in self.weapons.iter_mut().flatten() {
            w.load_post_process(host);
        }
    }

    // ---- the weapon set ----------------------------------------------------------------------------------------------------

    /// `updateWeaponSet(obj)`: switch to the weapon set that matches the object's weapon-set flags, replacing the
    /// weapons if it is not the one in use.
    pub fn update_weapon_set<H: WeaponHost>(&mut self, host: &mut H, obj: ObjectId) {
        let set = host.find_weapon_template_set(obj);
        if let Some(set) = set {
            if Some(set) != self.cur_weapon_template_set {
                if !host.weapon_template_set(set).is_weapon_lock_shared_across_sets() {
                    // "changing WeaponSet while Weapon is Locked... implicit unlock occurring!"
                    self.release_weapon_lock(LOCKED_PERMANENTLY); // release all locks. sorry!
                    self.cur_weapon = PRIMARY_WEAPON;
                }
                self.filled_weapon_slot_mask = 0;
                self.total_anti_mask = 0;
                self.total_damage_type_mask.clear();
                self.has_pitch_limit = false;
                self.has_damage_weapon = false;
                for i in (PRIMARY_WEAPON as usize..SLOTS).rev() {
                    if self.weapons[i].is_some() {
                        self.weapons[i] = None;
                    }

                    if let Some(key) = host.weapon_template_set(set).get_nth(i) {
                        // (a template the store does not have cannot be in a set: the INI parser took it from there)
                        let Some(template) = host.weapon_template_index(key) else { continue };
                        let mut weapon = Weapon::new(host, template, i as i32);
                        // m_curWeaponTemplateSet is still the old set here, so is its shared reload time
                        let shared = self.is_shared_reload_time(host);
                        let effect = weapon.load_ammo_now(host, obj, shared); // start 'em all with full clips
                        let (anti, damage_type, pitch_limited, damage_weapon) = (
                            host.store().templates()[template].anti_mask,
                            host.store().templates()[template].damage_type,
                            weapon.is_pitch_limited(),
                            weapon.is_damage_weapon(host.store()),
                        );
                        self.weapons[i] = Some(weapon);
                        self.apply(effect);
                        self.filled_weapon_slot_mask |= 1 << i;
                        self.total_anti_mask |= anti as i32;
                        self.total_damage_type_mask.set(damage_type, 1);
                        if pitch_limited {
                            self.has_pitch_limit = true;
                        }
                        if damage_weapon {
                            self.has_damage_weapon = true;
                        }

                        // no, do NOT do this; always start with the cur weapon being primary, even if there is no
                        // primary weapon. this is by design, to allow us to have units that have only "spell"
                        // weapons and no "normal" weapons. (srj)
                    }
                }
                self.cur_weapon_template_set = Some(set);
            }
        }
    }

    /// `WeaponSet::getModelConditionForWeaponSlot(wslot, a)`.
    pub fn get_model_condition_for_weapon_slot(wslot: i32, a: WeaponSetConditionType) -> ModelConditionFlags {
        const NOTHING: [i32; SLOTS] = [MODELCONDITION_INVALID, MODELCONDITION_INVALID, MODELCONDITION_INVALID];
        const FIRING: [i32; SLOTS] = [MODELCONDITION_FIRING_A, MODELCONDITION_FIRING_B, MODELCONDITION_FIRING_C];
        const BETWEENING: [i32; SLOTS] =
            [MODELCONDITION_BETWEEN_FIRING_SHOTS_A, MODELCONDITION_BETWEEN_FIRING_SHOTS_B, MODELCONDITION_BETWEEN_FIRING_SHOTS_C];
        const RELOADING: [i32; SLOTS] = [MODELCONDITION_RELOADING_A, MODELCONDITION_RELOADING_B, MODELCONDITION_RELOADING_C];
        const PRE_ATTACK: [i32; SLOTS] = [MODELCONDITION_PREATTACK_A, MODELCONDITION_PREATTACK_B, MODELCONDITION_PREATTACK_C];
        const LOOKUP: [[i32; SLOTS]; WSF_COUNT as usize] = [NOTHING, FIRING, BETWEENING, RELOADING, PRE_ATTACK];

        let mut flags = ModelConditionFlags::new(); // defaults to all clear

        let f = LOOKUP[a as usize][wslot as usize];
        if f != MODELCONDITION_INVALID {
            flags.set(f, 1);
        }

        const USING: [i32; SLOTS] = [MODELCONDITION_USING_WEAPON_A, MODELCONDITION_USING_WEAPON_B, MODELCONDITION_USING_WEAPON_C];
        if a != WSF_NONE {
            flags.set(USING[wslot as usize], 1);
        }

        flags
    }

    /// `reloadAllAmmo(obj, now)`.
    pub fn reload_all_ammo<H: WeaponHost>(&mut self, host: &H, obj: ObjectId, now: bool) {
        for i in 0..SLOTS {
            let shared = self.is_shared_reload_time(host);
            if let Some(weapon) = self.weapons[i].as_mut() {
                let effect = if now { weapon.load_ammo_now(host, obj, shared) } else { weapon.reload_ammo(host, obj, shared) };
                self.apply(effect);
            }
        }
    }

    /// `isOutOfAmmo()`.
    pub fn is_out_of_ammo(&self, now: u32) -> bool {
        for weapon in self.weapons.iter().flatten() {
            if weapon.get_status(now) != WeaponStatus::OutOfAmmo {
                return false;
            }
        }
        true
    }

    /// `findAmmoPipShowingWeapon()`.
    pub fn find_ammo_pip_showing_weapon<H: WeaponHost>(&self, host: &H) -> Option<&Weapon> {
        self.weapons.iter().flatten().find(|w| host.store().templates()[w.template()].is_shows_ammo_pips)
    }

    /// `findWaypointFollowingCapableWeapon()`: the last slot first.
    pub fn find_waypoint_following_capable_weapon<H: WeaponHost>(&mut self, host: &H) -> Option<&mut Weapon> {
        let i = (PRIMARY_WEAPON as usize..SLOTS)
            .rev()
            .find(|&i| self.weapons[i].as_ref().is_some_and(|w| host.store().templates()[w.template()].capable_of_following_waypoint))?;
        self.weapons[i].as_mut()
    }

    /// `weaponSetOnWeaponBonusChange(source)`.
    pub fn weapon_set_on_weapon_bonus_change<H: WeaponHost>(&mut self, host: &mut H, source: ObjectId) {
        for i in 0..SLOTS {
            let shared = self.is_shared_reload_time(host);
            if let Some(weapon) = self.weapons[i].as_mut() {
                let effect = weapon.on_weapon_bonus_change(host, source, shared);
                self.apply(effect);
            }
        }
    }

    /// `getMostPercentReadyToFireAnyWeapon()`.
    pub fn get_most_percent_ready_to_fire_any_weapon(&self, now: u32) -> u32 {
        let mut most_ready: u32 = 0;
        for weapon in self.weapons.iter().flatten() {
            let percentage = float_to_unsigned(weapon.get_percent_ready_to_fire(now) * 100.0);
            if percentage > most_ready {
                most_ready = percentage;
            }
            if most_ready >= 100 {
                return most_ready;
            }
        }
        most_ready
    }

    /// `isAnyWithinTargetPitch(obj, victim)`.
    fn is_any_within_target_pitch<H: WeaponHost>(&self, host: &H, obj: ObjectId, victim: ObjectId) -> bool {
        if !self.has_pitch_limit {
            return true;
        }

        for weapon in self.weapons.iter().flatten() {
            if weapon.is_within_target_pitch(host, obj, victim) {
                return true;
            }
        }
        false
    }

    /// `getAbleToAttackSpecificObject(attackType, source, victim, commandSource, specificSlot)`.
    pub fn get_able_to_attack_specific_object<H: WeaponHost>(
        &self,
        host: &H,
        attack_type: AbleToAttackType,
        source: ObjectId,
        victim: ObjectId,
        command_source: CommandSourceType,
        specific_slot: i32,
    ) -> WResult<CanAttackResult> {
        // basic sanity checks
        if host.is_effectively_dead(source)
            || host.is_effectively_dead(victim)
            || host.is_destroyed(source)
            || host.is_destroyed(victim)
            || victim == source
        {
            return Ok(CanAttackResult::NotPossible);
        }

        let same_owner_force_attack = host.controlling_player(source) == host.controlling_player(victim) && is_forced_attack(attack_type);

        // ----- examine VICTIM to determine legality of attack

        // an "OBJECT_STATUS_MASKED" object is not really a valid target for attacking, this masked status is an
        // "override" that can be applied to objects that are otherwise totally valid targets when we really really
        // really don't want anything in the world being able to attack stuff
        if host.test_status(victim, OBJECT_STATUS_MASKED) {
            return Ok(CanAttackResult::NotPossible);
        }

        // and this is the KINDOF version, for things that aren't really game objects, but need to have "life" so
        // they can have lifetimeupdates and vision
        if host.is_kind_of(victim, KINDOF_UNATTACKABLE) {
            return Ok(CanAttackResult::NotPossible);
        }

        // this object is not currently auto-acquireable
        if host.test_status(victim, OBJECT_STATUS_NO_ATTACK_FROM_AI) && command_source == CommandSourceType::FromAi {
            return Ok(CanAttackResult::NotPossible);
        }

        let mut allow_stealth_to_prevent_attacks = true;
        if host.test_status(source, OBJECT_STATUS_IGNORING_STEALTH) || same_owner_force_attack {
            allow_stealth_to_prevent_attacks = false;
        }
        if is_forced_attack(attack_type) && host.is_kind_of(victim, KINDOF_DISGUISER) {
            // force-attack allows you to attack disguised things, which also happen to be stealthed. since we
            // normally disallow attacking stealthed things (even via force-fire), we check for disguised and
            // explicitly ignore stealth in that case
            if host.test_status(victim, OBJECT_STATUS_DISGUISED) {
                allow_stealth_to_prevent_attacks = false;
            }
        }

        // if an object is stealthed and hasn't been detected yet, then it is not a valid target to fire on
        if allow_stealth_to_prevent_attacks && host.test_status(victim, OBJECT_STATUS_STEALTHED) && !host.test_status(victim, OBJECT_STATUS_DETECTED) {
            if !host.is_kind_of(victim, KINDOF_DISGUISER) {
                return Ok(CanAttackResult::NotPossible);
            } else {
                // exception case -- don't return false if we are a bomb truck disguised as an enemy vehicle
                if let Some(disguised_player_index) = host.stealth_disguised_player_index(victim) {
                    let our_player = host.controlling_player(source);
                    if let Some(rel) = host.player_relationship_to_player(our_player, disguised_player_index) {
                        if rel != Relationship::Enemies {
                            // our stealthed & undetected object is disguised as a unit not perceived to be our enemy
                            return Ok(CanAttackResult::NotPossible);
                        }
                    }
                }
            }
        }

        // (fog no longer prevents attacking at all; it prevents targeting in findClosestEnemy)

        // at least one of the following must be true:
        // -- victim is our enemy
        // -- victim is a Bridge Tower (they are always neutral)
        // -- victim is a non-allied Mine
        // -- attack mode is force attack
        // if none of the above is true, we can't attack. sorry!
        let r = host.relationship(source, victim);
        if r != Relationship::Enemies && !is_forced_attack(attack_type) && !(host.is_kind_of(victim, KINDOF_MINE) && r != Relationship::Allies) {
            // only reject this if the command is from a player. If scripts or AI order attacks, they don't care
            // about relationships (and fixes broken scripts).
            if command_source == CommandSourceType::FromPlayer && (!host.test_script_status_bit(victim, u32::from(OBJECT_STATUS_SCRIPT_TARGETABLE)) || r == Relationship::Allies) {
                // unless the object has a map property that sets it to be targetable (and not allied), then give up
                return Ok(CanAttackResult::NotPossible);
            }
        }

        // if the victim is contained within an enclosing container, it cannot be attacked directly
        if let Some(victims_container) = host.contained_by(victim) {
            if host.contain_is_enclosing_container_for(victims_container, victim) {
                return Ok(CanAttackResult::NotPossible);
            }
        }

        // check if the container has an apparent controller, that controller must be an enemy (unless this is a
        // force-attack)
        if !is_forced_attack(attack_type) && host.has_contain(victim) {
            let victim_apparent_controller = host.contain_apparent_controlling_player(victim, host.controlling_player(source));
            if let Some(controller) = victim_apparent_controller {
                if host.team_relationship_to_player_index(source, controller) != Relationship::Enemies {
                    // only reject this if the command is from a player. If scripts or AI order attacks, they don't
                    // care about relationships (and fixes broken scripts).
                    if command_source == CommandSourceType::FromPlayer
                        && (!host.test_script_status_bit(victim, u32::from(OBJECT_STATUS_SCRIPT_TARGETABLE)) || r == Relationship::Allies)
                    {
                        // unless the object has a map property that sets it to be targetable (and not allied), then
                        // give up
                        return Ok(CanAttackResult::NotPossible);
                    }
                }
            }
        }

        // check if the shot itself is valid!
        let pos = host.position(victim);
        self.get_able_to_use_weapon_against_target(host, attack_type, source, Some(victim), Some(pos), command_source, specific_slot)
    }

    /// `getAbleToUseWeaponAgainstTarget(attackType, source, victim, pos, commandSource, specificSlot)`: when all
    /// conditions have been validated to the point where we have decided we wish to attack the object (faction
    /// checks, etc), is the attack itself able to be performed?
    #[allow(clippy::too_many_arguments)]
    pub fn get_able_to_use_weapon_against_target<H: WeaponHost>(
        &self,
        host: &H,
        attack_type: AbleToAttackType,
        source: ObjectId,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
        specific_slot: i32,
    ) -> WResult<CanAttackResult> {
        // first determine if we are attacking an object or the ground and get the appropriate weapon anti mask
        let target_anti_mask: u32;
        let mut pos = pos;
        if let Some(v) = victim {
            // attacking a specific object -- get the victim's anti weapon mask!
            target_anti_mask = get_victim_anti_mask(host, v);

            // make sure that the pos matches the victim!
            pos = Some(host.position(v));
        } else {
            // attacking the ground so this is obvious
            target_anti_mask = WEAPON_ANTI_GROUND;
        }

        // Special Case test for turreted weapons on buildings... since they cannot move, they must be within range
        // of target. (Kris, srj: see the comments upstream.)
        let mut within_attack_range = false;
        let contained_by = host.contained_by(source);
        let mut has_a_weapon_in_range = false;
        let mut has_a_weapon = false;
        for _slot in 0..(SLOTS - 1) {
            let weapon_to_test_for_range = self.weapons.get(self.cur_weapon as usize).and_then(|w| w.as_ref());
            if let Some(weapon_to_test_for_range) = weapon_to_test_for_range {
                has_a_weapon = true;
                if (self.total_anti_mask & target_anti_mask as i32) == 0 {
                    // we don't care to check for this weapon
                    continue;
                }

                let contain = contained_by.filter(|&c| host.has_contain(c));
                let garrison = contain.is_some_and(|c| host.contain_is_garrisonable(c) && host.contain_is_enclosing_container_for(c, source));
                if garrison {
                    // non enclosing garrison containers do not use firepoints. Lorenzen, 6/11/03
                    // for contained things, we need to fake-move objects to the best garrison point in order to get
                    // precise range checks
                    let target_pos = pos.ok_or(WeaponError::NullObject)?;
                    if let Some(goal_pos) = contain.and_then(|c| host.contain_calc_best_garrison_position(c, &target_pos)) {
                        within_attack_range = weapon_to_test_for_range.is_source_object_with_goal_position_within_attack_range(
                            host,
                            source,
                            &goal_pos,
                            victim,
                            Some(&target_pos),
                        );
                    }
                } else if let Some(v) = victim {
                    within_attack_range = weapon_to_test_for_range.is_within_attack_range(host, source, v);
                } else {
                    within_attack_range = weapon_to_test_for_range.is_within_attack_range_pos(host, source, &pos.ok_or(WeaponError::NullObject)?);
                }
                if within_attack_range {
                    has_a_weapon_in_range = true;
                    break;
                }
            }
        }

        if (host.is_kind_of(source, KINDOF_IMMOBILE) || host.is_kind_of(source, KINDOF_SPAWNS_ARE_THE_WEAPONS) || contained_by.is_some())
            && has_a_weapon
            && !has_a_weapon_in_range
            && attack_type != ATTACK_TUNNEL_NETWORK_GUARD
        {
            return Ok(CanAttackResult::InvalidShot);
        }

        let mut ok_result = if within_attack_range { CanAttackResult::Possible } else { CanAttackResult::PossibleAfterMoving };

        // ----- things we examine about SOURCE to determine legality of attack

        if self.has_any_damage_weapon() {
            if (self.total_anti_mask & target_anti_mask as i32) == 0 {
                return Ok(CanAttackResult::InvalidShot);
            }

            // if we don't have a victim, we are force attacking a position. Because of the weapon mask check
            // preceding this, the attack is possible!
            let Some(v) = victim else { return Ok(ok_result) };

            if !self.is_any_within_target_pitch(host, source, v) {
                return Ok(CanAttackResult::InvalidShot);
            }

            let (first, last);
            if self.is_cur_weapon_locked() {
                first = self.cur_weapon;
                last = self.cur_weapon;
            } else if specific_slot != -1 {
                first = specific_slot;
                last = specific_slot;
            } else {
                first = SLOTS as i32 - 1;
                last = PRIMARY_WEAPON;
            }

            // check each weapon, until we find one that can do damage (or fail)
            let mut i = first;
            while i >= last {
                if let Some(weapon) = self.weapons.get(i as usize).and_then(|w| w.as_ref()) {
                    if weapon.estimate_weapon_damage(host, source, Some(v), None)? != 0.0 {
                        // Kris: Aug 22, 2003: surgical fix so Jarmen Kell doesn't get a targeting cursor on enemy
                        // vehicles unless he is in snipe mode.
                        if host.store().templates()[weapon.template()].damage_type == DAMAGE_KILLPILOT
                            && host.is_kind_of(source, KINDOF_HERO)
                            && self.cur_weapon == PRIMARY_WEAPON
                            && specific_slot == -1
                        {
                            i -= 1;
                            continue;
                        }

                        return Ok(ok_result);
                    }
                }
                i -= 1;
            }
        }

        // do a check to see if we have an occupied container (garrisoned building, transport that allows
        // passengers to fire)
        if host.has_contain(source) && host.contain_is_passenger_allowed_to_fire(source) {
            // loop through each member and if just one of them can attack the specific target, then we are good to go!
            if let Some(items) = host.contain_items(source) {
                for garrisoned_member in items {
                    if host.is_able_to_attack(garrisoned_member) {
                        let result = host.member_able_to_use_weapon_against_target(garrisoned_member, attack_type, victim, pos, command_source)?;
                        if result == CanAttackResult::Possible || result == CanAttackResult::PossibleAfterMoving {
                            return Ok(result);
                        }
                    }
                }
            }
        }

        // do a check to see if we have a hive object that has slaved objects
        if let Some(slaves) = host.spawn_slaves_can_use_weapon_against_target(source, attack_type, victim, pos, command_source) {
            if slaves == CanAttackResult::Possible {
                // srj sez: this bit is intended to fix the situation where you have a stinger site selected and get
                // the "attack if I move" cursor against an enemy. since the stinger site can't move or fire, you
                // shouldn't really EVER get this cursor for it. and since we just verified above that our slaves (the
                // soldiers) can attack correctly, just nork it.
                if host.is_kind_of(source, KINDOF_IMMOBILE)
                    && host.is_kind_of(source, KINDOF_SPAWNS_ARE_THE_WEAPONS)
                    && ok_result == CanAttackResult::PossibleAfterMoving
                {
                    ok_result = CanAttackResult::Possible;
                }

                return Ok(ok_result);
            }
        }

        // oh well... guess not!
        Ok(CanAttackResult::InvalidShot)
    }

    /// `chooseBestWeaponForTarget(obj, victim, criteria, cmdSource)`: select the best weapon for the given target and
    /// make it the current weapon. If there is no weapon that can damage the target, false is returned (and the
    /// current weapon becomes the primary one). This does take weapon attack range into account.
    pub fn choose_best_weapon_for_target<H: WeaponHost>(
        &mut self,
        host: &H,
        obj: ObjectId,
        victim: Option<ObjectId>,
        criteria: WeaponChoiceCriteria,
        cmd_source: CommandSourceType,
    ) -> WResult<bool> {
        if self.is_cur_weapon_locked() {
            return Ok(true); // I have been forced into choosing a specific weapon, so it is right until someone says otherwise
        }

        let Some(victim) = victim else {
            // weapon lock is checked first for specific attack-ground powers. Otherwise, we will reproduce the old
            // behavior and make only Primary attack the ground.
            self.cur_weapon = PRIMARY_WEAPON;
            return Ok(true);
        };

        let now = host.frame();
        let mut found = false; // a Ready weapon has been found
        let mut found_backup = false; // an unready, but valid weapon has been found

        let mut longest_range = 0.0f32;
        let mut best_damage = 0.0f32;
        let mut longest_range_backup = 0.0f32;
        let mut best_damage_backup = 0.0f32;

        let mut current_decision = PRIMARY_WEAPON;
        let mut current_decision_backup = PRIMARY_WEAPON;

        // go backwards, so that in the event of ties, the primary weapon is preferred
        for i in (PRIMARY_WEAPON..SLOTS as i32).rev() {
            // first: eliminate the weapons that cannot be used.

            // no weapon in this slot
            let Some(weapon) = self.weapons[i as usize].as_ref() else { continue };

            // weapon not allowed to be specified via this command source
            let set = self.cur_weapon_template_set.ok_or(WeaponError::NullObject)?;
            let ok_srcs = host.weapon_template_set(set).get_nth_command_source_mask(i as usize);
            // (`CMD_DEFAULT_SWITCH_WEAPON` is the enum value 4, tested here as a mask: it is the `FROM_AI` bit)
            if (ok_srcs & (1u32.wrapping_shl(cmd_source as u32))) == 0 && (ok_srcs & CommandSourceType::DefaultSwitchWeapon as u32) == 0 {
                continue;
            }

            // weapon out of ammo
            let t = &host.store().templates()[weapon.template()];
            if weapon.get_status(now) == WeaponStatus::OutOfAmmo && !t.get_auto_reloads_clip() {
                continue;
            }

            // weapon not allowed to target this kind of thing
            if (t.anti_mask & get_victim_anti_mask(host, victim)) == 0 {
                continue;
            }

            if !weapon.is_within_target_pitch(host, obj, victim) {
                continue;
            }

            let mut damage = weapon.estimate_weapon_damage(host, obj, Some(victim), None)?;
            let mut attack_range = weapon.get_attack_range(host, obj);
            let mut weapon_is_ready = weapon.get_status(now) == WeaponStatus::ReadyToFire;

            if host.ai_is_weapon_slot_on_turret_and_aiming_at_target(obj, i, victim) {
                weapon_is_ready = false;
            }

            // weapon would do no damage against this target. exception: 'unresistable' weapons are allowed to do
            // zero damage.
            if damage <= 0.0 && t.damage_type != DAMAGE_UNRESISTABLE {
                continue;
            }

            // now that we've eliminated the impossible ones, let's decide which one we will prefer.

            // if a weapon matches the magic "preferred" bits, it's ALWAYS preferred over other weapons, regardless
            // of actual damage/range. (eg, the Comanche cannon, which is always preferred against infantry, rather
            // than its antitank missiles.) we accomplish this cheesily, by magnifying the damage/range, and by
            // declaring the weapon "ready" in a more liberal manner.
            let preferred_against = *host.weapon_template_set(set).get_nth_preferred_against_mask(i as usize);
            if preferred_against.any() && host.is_kind_of_multi(victim, &preferred_against, &KINDOFMASK_NONE) {
                const HUGE_DAMAGE: f32 = 1e10; // wow, that's a lot of damage
                const HUGE_RANGE: f32 = 1e10;
                damage = HUGE_DAMAGE;
                attack_range = HUGE_RANGE;
                // preferred weapons are also kept if they are merely reloading. (if out of ammo, we can punt.)
                weapon_is_ready = weapon.get_status(now) != WeaponStatus::OutOfAmmo;
            }

            match criteria {
                PREFER_MOST_DAMAGE => {
                    if !weapon_is_ready {
                        // if this weapon is not ready, the best it can do is qualify as the best backup choice
                        if damage >= best_damage_backup {
                            best_damage_backup = damage;
                            current_decision_backup = i;
                            found_backup = true;
                        }
                    } else if damage >= best_damage {
                        best_damage = damage;
                        current_decision = i;
                        found = true;
                    }
                }
                PREFER_LONGEST_RANGE => {
                    if !weapon_is_ready {
                        if attack_range > longest_range_backup {
                            longest_range_backup = attack_range;
                            current_decision_backup = i;
                            found_backup = true;
                        }
                    } else if attack_range > longest_range {
                        longest_range = attack_range;
                        current_decision = i;
                        found = true;
                    }
                }
                _ => {}
            }
        }

        if found {
            // if we found a good best one, then just use it
            self.cur_weapon = current_decision;
        } else if found_backup {
            // no ready weapon was found, so return the most suitable unready one
            self.cur_weapon = current_decision_backup;
            found = true;
        } else {
            // no weapon at all was found, so we go back to primary
            self.cur_weapon = PRIMARY_WEAPON;
        }

        Ok(found)
    }

    /// `setWeaponLock(weaponSlot, lockType)`: "a special type of command demands that you use this (normally
    /// unchooseable) weapon until told otherwise".
    pub fn set_weapon_lock(&mut self, weapon_slot: i32, lock_type: WeaponLockType) -> bool {
        if lock_type == NOT_LOCKED {
            // "calling setWeaponLock with NOT_LOCKED, so I am doing nothing... did you mean to use releaseWeaponLock()?"
            return false;
        }

        // verify the asked for weapon exists, choose it, and then lock it as chosen until unlocked
        if self.weapons.get(weapon_slot as usize).is_some_and(|w| w.is_some()) {
            // upstream has two branches with the same body: permanent, or temporary over a non-permanent lock
            if lock_type == LOCKED_PERMANENTLY || (lock_type == LOCKED_TEMPORARILY && self.cur_weapon_locked_status != LOCKED_PERMANENTLY) {
                self.cur_weapon = weapon_slot;
                self.cur_weapon_locked_status = lock_type;
            }

            return true;
        }

        // "setWeaponLock: weapon %d not found (missing an upgrade?)"
        false
    }

    /// `releaseWeaponLock(lockType)`: either we have successfully fired a full clip of our special attack, or we have
    /// switched weaponsets entirely, or any Player issued command besides special attack has been given.
    pub fn release_weapon_lock(&mut self, lock_type: WeaponLockType) {
        if self.cur_weapon_locked_status == NOT_LOCKED {
            return; // nothing to do
        }

        if lock_type == LOCKED_PERMANENTLY {
            // all locks released
            self.cur_weapon_locked_status = NOT_LOCKED;
        } else if lock_type == LOCKED_TEMPORARILY {
            // only unlocked if the current lock is temporary
            if self.cur_weapon_locked_status == LOCKED_TEMPORARILY {
                self.cur_weapon_locked_status = NOT_LOCKED;
            }
        }
        // else: "calling releaseWeaponLock with NOT_LOCKED makes no sense"
    }

    /// `clearLeechRangeModeForAllWeapons()`: when an `AIAttackState` is over, it needs to clean up any weapons that
    /// might be in leech range mode or else those weapons will have unlimited range!
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        for weapon in self.weapons.iter_mut().flatten() {
            // just clear the leech range active flag
            weapon.set_leech_range_active(false);
        }
    }
}
