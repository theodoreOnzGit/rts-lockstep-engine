// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h  (class Weapon, WeaponStore's firing members)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp
//     (WeaponTemplate::estimateWeaponTemplateDamage, shouldProjectileCollideWith, fireWeaponTemplate,
//      trimOldHistoricDamage, dealDamageInternal; WeaponStore::handleProjectileDetonation,
//      createAndFireTempWeapon, update; class Weapon)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Weapon.cpp: Steven Johnson, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The `Weapon` instance (ammo, reload, status, range checks, firing, `xfer`) and the firing half of
//! `WeaponTemplate` and `WeaponStore`: `fireWeaponTemplate`, `dealDamageInternal` with its historic bonus,
//! `estimateWeaponTemplateDamage`, `shouldProjectileCollideWith`, `WeaponStore::update`,
//! `createAndFireTempWeapon` and `handleProjectileDetonation`. Wave 7, slice B (`p1w8-sonnet`); the template and
//! store data are slice A (`logic::weapon`).
//!
//! # The shape
//!
//! Plain functions over a generic [`WeaponHost`] (the same shape as `FiringTracker`'s host): the host answers every
//! question about objects, players, the partition manager, the terrain and the client, and each method is called
//! where upstream makes the call, in upstream's order. A [`Weapon`] holds the index of its template in the store
//! (templates are never removed from the store: an override never joins it, see `logic::weapon`).
//!
//! # Siblings: the weapons of the same set
//!
//! `reloadWithBonus`, `onWeaponBonusChange` and `privateFireWeapon` of a unit whose weapon set `isReloadTimeShared`
//! set the next-shot frame and the status of **every weapon of the source's set**, `this` among them (they have
//! just set the same two values on `this`). A weapon cannot reach the set it lives in, and it asks the set (not
//! the host) whether the reload time is shared, since during `WeaponSet::updateWeaponSet` that is still the
//! *old* template set's answer. So those methods take `shared: bool` (the caller's answer to
//! `isReloadTimeShared()`) and **return** the assignment they would make as a [`SharedReload`]: it is the last
//! thing each of them does, so applying it afterwards is the same order of effects. The `WeaponSet` applies it to
//! its other slots ([`SharedReload::apply_to_slots`]); a temporary weapon (`createAndFireTempWeapon`), which is in
//! no set, applies it to the source's own set through [`WeaponHost::apply_shared_reload`]. A temporary weapon's
//! reload therefore overwrites the reload state of the source's real weapons when the set shares its reload time;
//! that is upstream's behaviour.
//!
//! # Where upstream is undefined
//!
//! - A **null source** is dereferenced (`createAndFireTempWeapon` from `dealDamageInternal` after the source has
//!   been removed; `dealDamageInternal`'s `KILLS_SELF` path): [`WeaponError::NullObject`].
//! - `estimateWeaponTemplateDamage` tests `victimObj->isKindOf(KINDOF_STRUCTURE)` before it tests the damage
//!   type, so a position-only estimate (`victimObj == NULL`) dereferences a null pointer: the same error.
//! - `getBarrelCount` of an object with no drawable: the same error.
//! - `newProjectileFired` with a projectile stream name that names no template: `TheThingFactory->newObject(NULL, ..)`
//!   throws `ERROR_BAD_ARG` ([`WeaponError::NullTemplate`]); a missing laser template is, as upstream, a quiet return.
//! - Implicit `float` to `UnsignedInt` conversions (`m_whenWeCanFireAgain = m_whenLastReloadStarted +
//!   reloadTime`, with `reloadTime` a `Real`) are done as the x86-64 and the x86 shipped code do them: truncate to
//!   64 bits and keep the low 32 ([`float_to_unsigned`]), which is defined for every value that fits 64 bits.
//!
//! # Upstream defects kept
//!
//! - `setClipPercentFull` sets `m_status = m_ammoInClip ? OUT_OF_AMMO : READY_TO_FIRE`: the test is inverted, so a
//!   weapon given ammo is `OUT_OF_AMMO` until its next `getStatus()` (which recomputes it from the frame).
//! - `Weapon::getStatus()` is `const` and writes `m_status` (`mutable`): a [`Cell`].
//! - `privateFireWeapon`'s `DAMAGE_DISARM` path decrements `m_ammoInClip` without testing it: an empty clip wraps.
//! - `forceFireWeapon` passes `NULL` for `extraBonusFlags` (0).
//! - `getAbleToUseWeaponAgainstTarget` (in `weapon_set_state`) loops over the slots but always tests
//!   `m_weapons[m_curWeapon]`.
//! - `DAMAGE_DISARM`'s test is `!found && MINE || BOOBY_TRAP || DEMOTRAP`: a found mine that is also a booby
//!   trap is destroyed as well.
//! - `fireWeaponTemplate`'s scatter test is `radius > 0 || infantryDist > 0 && victim && infantry`.
//!
//! # Verified
//!
//! Against upstream's own text, all of `Weapon.cpp` and the class `Weapon` of `Weapon.h`, compiled on stand-ins for
//! the objects, players, partition manager, terrain, drawable and AI, by `tests/differential_weapon_fire.rs`
//! (methodology, the 21 planted bugs and the results are in its header). Measured 2026-10-20 against upstream commit
//! 0a05454d: 30,000 random programs, 24,082,803 trace lines (every effect and the state of every weapon after every
//! operation), **0 mismatches**, 1,363 programs excluded as undefined upstream. The first runs found, and the test
//! now pins: an unsigned comparison in `getPreAttackDelay` (`m_ammoInClip < clipSize` with a wrapped ammo count), the
//! `newObject(NULL)` throw, and the self-assignment guard of `operator=`.

use std::cell::Cell;

use petir::Real;

use crate::common::base_type::{
    max, real_to_int_ceil, real_to_int_floor, sqr, Coord3D, Region3D, PI,
};
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::{
    KINDOF_BALLISTIC_MISSILE, KINDOF_BOOBY_TRAP, KINDOF_BRIDGE, KINDOF_DEMOTRAP,
    KINDOF_FS_AIRFIELD, KINDOF_INFANTRY, KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_SHRUBBERY,
    KINDOF_SMALL_MISSILE, KINDOF_STRUCTURE,
};
use crate::common::object_status_types::{
    OBJECT_STATUS_BURNED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED,
};
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult};
use crate::logic::damage::{
    DamageInfo, DamageInfoInput, DamageType, DeathType, DAMAGE_DEPLOY, DAMAGE_DISARM, DAMAGE_FLAME,
    DAMAGE_HACK, DAMAGE_PARTICLE_BEAM, DAMAGE_SNIPER, DAMAGE_SURRENDER, HUGE_DAMAGE_AMOUNT,
};
use crate::logic::weapon::{
    WeaponError, WeaponStore, NO_MAX_SHOTS_LIMIT, PATHFIND_CELL_SIZE_F, PREFIRE_PER_ATTACK,
    PREFIRE_PER_CLIP, WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_SELF,
    WEAPON_COLLIDE_ALLIES, WEAPON_COLLIDE_BALLISTIC_MISSILES, WEAPON_COLLIDE_CONTROLLED_STRUCTURES,
    WEAPON_COLLIDE_ENEMIES, WEAPON_COLLIDE_PROJECTILE, WEAPON_COLLIDE_SHRUBBERY,
    WEAPON_COLLIDE_SMALL_MISSILES, WEAPON_COLLIDE_STRUCTURES, WEAPON_COLLIDE_WALLS,
    WEAPON_DOESNT_AFFECT_AIRBORNE, WEAPON_DOESNT_AFFECT_SIMILAR, WEAPON_KILLS_SELF,
};
use crate::logic::weapon_bonus::WeaponBonus;
use crate::logic::weapon_host::{DistanceCalculationType, WeaponFx, WeaponHost};
use crate::logic::weapon_types::{WeaponBonusConditionFlags, WeaponStatus};
use crate::logic::world::partition::DistanceCalculationType as Dct;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath::WWMATH_EPSILON;

/// `WEAPONSLOT_COUNT` as an array length.
pub const SLOTS: usize = crate::common::game_type::WEAPONSLOT_COUNT as usize;

/// `PRIMARY_WEAPON`.
pub const PRIMARY_WEAPON: i32 = 0;

/// The three weapon slots of a set.
pub type Slots = [Option<Weapon>; SLOTS];

/// `DEATH_BURNED`.
pub const DEATH_BURNED: DeathType = 3;

/// `ATTACK_RANGE_CALC_TYPE`: `ATTACK_RANGE_IS_2D` is defined, so the bounding spheres in 2D.
pub const ATTACK_RANGE_CALC_TYPE: DistanceCalculationType = Dct::FromBoundingSphere2D;
/// `DAMAGE_RANGE_CALC_TYPE`: "damage is ALWAYS 3d".
pub const DAMAGE_RANGE_CALC_TYPE: DistanceCalculationType = Dct::FromBoundingSphere3D;

/// `TURRET_INVALID`.
pub const TURRET_INVALID: i32 = -1;
/// `LOCOMOTORSET_SUPERSONIC`.
pub const LOCOMOTORSET_SUPERSONIC: i32 = 6;

type WResult<T> = Result<T, WeaponError>;

/// Why a weapon's `xfer` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeaponXferError {
    /// The `Xfer` failed.
    Xfer(crate::common::xfer::XferStatus),
    /// `throw INI_INVALID_DATA`: a saved weapon names a template the store does not have.
    InvalidData,
}

impl From<crate::common::xfer::XferStatus> for WeaponXferError {
    fn from(s: crate::common::xfer::XferStatus) -> Self {
        WeaponXferError::Xfer(s)
    }
}

/// The implicit conversion of a `Real` to an `UnsignedInt`: truncate to 64 bits and keep the low 32, which is what
/// the shipped x86 code (`__ftol2`) and x86-64 (`cvttss2si rax`) both do.
pub fn float_to_unsigned(x: f32) -> u32 {
    (x as i64) as u32
}

/// `atan2` of two `Real`s: the C `double` function, stored in a `Real`.
fn atan2_d(y: f32, x: f32) -> f32 {
    Real::atan2(f64::from(y), f64::from(x)) as f32
}

/// `fabs` of a `Real`.
fn fabs(x: f32) -> f32 {
    x.abs()
}

/// The assignment a shared-reload weapon makes on every weapon of its set (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedReload {
    /// `weapon->setPossibleNextShotFrame(...)`.
    pub next_shot_frame: u32,
    /// `weapon->setStatus(...)`.
    pub status: WeaponStatus,
}

impl SharedReload {
    /// The loop over the weapons of a set: every weapon present gets the frame and the status.
    pub fn apply_to_slots(self, slots: &mut Slots) {
        for weapon in slots.iter_mut().flatten() {
            weapon.set_possible_next_shot_frame(self.next_shot_frame);
            weapon.set_status(self.status);
        }
    }

    /// The same loop over the set of `source`, which the host owns.
    pub fn apply_via_host<H: WeaponHost>(self, host: &mut H, source: ObjectId) {
        host.apply_shared_reload(source, self.next_shot_frame, self.status);
    }
}

/// What firing a weapon came to: whether the clip was reloaded after the shot, and the shared-reload assignment
/// to make on the weapons of the set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireOutcome {
    /// "Return true if we auto-reloaded our clip after firing."
    pub reloaded: bool,
    /// What to assign to the set's weapons.
    pub shared: Option<SharedReload>,
}

/// `class Weapon`: one weapon of an object. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    /// `m_template`: the index in the store.
    template: usize,
    /// `m_wslot`.
    wslot: i32,
    /// `m_status` (mutable upstream).
    status: Cell<WeaponStatus>,
    /// `m_ammoInClip`.
    ammo_in_clip: u32,
    /// `m_whenWeCanFireAgain`.
    when_we_can_fire_again: u32,
    /// `m_whenPreAttackFinished`.
    when_pre_attack_finished: u32,
    /// `m_whenLastReloadStarted`.
    when_last_reload_started: u32,
    /// `m_lastFireFrame`.
    last_fire_frame: u32,
    /// `m_suspendFXFrame`.
    suspend_fx_frame: u32,
    /// `m_projectileStreamID`.
    projectile_stream_id: ObjectId,
    /// `m_maxShotCount`.
    max_shot_count: i32,
    /// `m_curBarrel`.
    cur_barrel: i32,
    /// `m_numShotsForCurBarrel`.
    num_shots_for_cur_barrel: i32,
    /// `m_scatterTargetsUnused`.
    scatter_targets_unused: Vec<i32>,
    /// `m_pitchLimited`.
    pitch_limited: bool,
    /// `m_leechWeaponRangeActive`.
    leech_weapon_range_active: bool,
}

/// The `WeaponTemplate` of `store` at `i`.
fn tpl(store: &WeaponStore, i: usize) -> &crate::logic::weapon::WeaponTemplate {
    &store.templates()[i]
}

impl Weapon {
    /// `Weapon::Weapon(tmpl, wslot)` ("weapons start empty; you must reload before use").
    pub fn new<H: WeaponHost>(host: &H, template: usize, wslot: i32) -> Weapon {
        Weapon::new_in_store(host.store(), host.frame(), template, wslot)
    }

    /// `Weapon::Weapon(tmpl, wslot)` with what it reads of the host given directly: the store that holds `template` and the frame now.
    pub fn new_in_store(store: &WeaponStore, frame: u32, template: usize, wslot: i32) -> Weapon {
        let t = tpl(store, template);
        Weapon {
            template,
            wslot,
            status: Cell::new(WeaponStatus::OutOfAmmo),
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            suspend_fx_frame: frame.wrapping_add(t.suspend_fx_delay),
            projectile_stream_id: INVALID_ID,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            cur_barrel: 0,
            num_shots_for_cur_barrel: t.shots_per_barrel,
            scatter_targets_unused: Vec::new(),
            pitch_limited: t.min_target_pitch > -PI || t.max_target_pitch < PI,
            leech_weapon_range_active: false,
        }
    }

    /// `Weapon::Weapon(const Weapon& that)`: "weapons lose all ammo when copied".
    pub fn copy_from(that: &Weapon, store: &WeaponStore) -> Weapon {
        let t = tpl(store, that.template);
        Weapon {
            template: that.template,
            wslot: that.wslot,
            status: Cell::new(WeaponStatus::OutOfAmmo),
            ammo_in_clip: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            when_we_can_fire_again: 0,
            projectile_stream_id: INVALID_ID,
            leech_weapon_range_active: false,
            pitch_limited: t.min_target_pitch > -PI || t.max_target_pitch < PI,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            cur_barrel: 0,
            num_shots_for_cur_barrel: t.shots_per_barrel,
            last_fire_frame: 0,
            suspend_fx_frame: that.suspend_fx_frame,
            scatter_targets_unused: Vec::new(),
        }
    }

    /// `Weapon::operator=(const Weapon& that)`: the same as the copy constructor, onto an existing weapon, which
    /// keeps its scatter targets (the operator does not touch `m_scatterTargetsUnused`).
    pub fn assign_from(&mut self, that: &Weapon, store: &WeaponStore) {
        let t = tpl(store, that.template);
        self.template = that.template;
        self.wslot = that.wslot;
        self.status = Cell::new(WeaponStatus::OutOfAmmo);
        self.ammo_in_clip = 0;
        self.when_pre_attack_finished = 0;
        self.when_last_reload_started = 0;
        self.when_we_can_fire_again = 0;
        self.leech_weapon_range_active = false;
        self.pitch_limited = t.min_target_pitch > -PI || t.max_target_pitch < PI;
        self.max_shot_count = NO_MAX_SHOTS_LIMIT;
        self.cur_barrel = 0;
        self.last_fire_frame = 0;
        self.suspend_fx_frame = that.suspend_fx_frame;
        self.num_shots_for_cur_barrel = t.shots_per_barrel;
        self.projectile_stream_id = INVALID_ID;
    }

    // ---- accessors (the inline members of Weapon.h) ---------------------------------------------------------------

    /// `getTemplate()`: the index in the store.
    pub fn template(&self) -> usize {
        self.template
    }
    /// `getWeaponSlot()`.
    pub fn weapon_slot(&self) -> i32 {
        self.wslot
    }
    /// `getPossibleNextShotFrame()`.
    pub fn possible_next_shot_frame(&self) -> u32 {
        self.when_we_can_fire_again
    }
    /// `getPreAttackFinishedFrame()`.
    pub fn pre_attack_finished_frame(&self) -> u32 {
        self.when_pre_attack_finished
    }
    /// `getLastReloadStartedFrame()`.
    pub fn last_reload_started_frame(&self) -> u32 {
        self.when_last_reload_started
    }
    /// `setPossibleNextShotFrame(frame)`.
    pub fn set_possible_next_shot_frame(&mut self, frame: u32) {
        self.when_we_can_fire_again = frame;
    }
    /// `setPreAttackFinishedFrame(frame)`.
    pub fn set_pre_attack_finished_frame(&mut self, frame: u32) {
        self.when_pre_attack_finished = frame;
    }
    /// `setLastReloadStartedFrame(frame)`.
    pub fn set_last_reload_started_frame(&mut self, frame: u32) {
        self.when_last_reload_started = frame;
    }
    /// `setStatus(status)` (private upstream, "should not be used outside this class").
    pub fn set_status(&self, status: WeaponStatus) {
        self.status.set(status);
    }
    /// `getLastShotFrame()`.
    pub fn last_shot_frame(&self) -> u32 {
        self.last_fire_frame
    }
    /// `m_lastFireFrame` set directly (batch R26): the state a reader of `getLastShotFrame()` sees, for a test of such a reader (`StealthUpdate`).
    pub fn friend_set_last_shot_frame(&mut self, frame: u32) {
        self.last_fire_frame = frame;
    }
    /// `getSuspendFXFrame()`.
    pub fn suspend_fx_frame(&self) -> u32 {
        self.suspend_fx_frame
    }
    /// `isPitchLimited()`.
    pub fn is_pitch_limited(&self) -> bool {
        self.pitch_limited
    }
    /// `setLeechRangeActive(active)`.
    pub fn set_leech_range_active(&mut self, active: bool) {
        self.leech_weapon_range_active = active;
    }
    /// `hasLeechRange()`.
    pub fn has_leech_range(&self) -> bool {
        self.leech_weapon_range_active
    }
    /// `setMaxShotCount(maxShots)`.
    pub fn set_max_shot_count(&mut self, max_shots: i32) {
        self.max_shot_count = max_shots;
    }
    /// `getMaxShotCount()`.
    pub fn max_shot_count(&self) -> i32 {
        self.max_shot_count
    }
    /// `m_curBarrel` (for the tests and the xfer).
    pub fn cur_barrel(&self) -> i32 {
        self.cur_barrel
    }
    /// `m_numShotsForCurBarrel`.
    pub fn num_shots_for_cur_barrel(&self) -> i32 {
        self.num_shots_for_cur_barrel
    }
    /// `m_scatterTargetsUnused`.
    pub fn scatter_targets_unused(&self) -> &[i32] {
        &self.scatter_targets_unused
    }
    /// `m_projectileStreamID`.
    pub fn projectile_stream_id(&self) -> ObjectId {
        self.projectile_stream_id
    }
    /// `m_ammoInClip`, without the `getStatus()` of `getRemainingAmmo`.
    pub fn raw_ammo_in_clip(&self) -> u32 {
        self.ammo_in_clip
    }
    /// `m_status`, without recomputing it.
    pub fn raw_status(&self) -> WeaponStatus {
        self.status.get()
    }
    /// `isLaser()`.
    pub fn is_laser(&self, store: &WeaponStore) -> bool {
        !tpl(store, self.template).laser_name.is_empty()
    }
    /// `getRemainingAmmo()`: "if we are reloading, then m_ammoInClip is a lie. It will say full."
    pub fn remaining_ammo(&self, now: u32) -> u32 {
        if self.get_status(now) == WeaponStatus::ReloadingClip {
            0
        } else {
            self.ammo_in_clip
        }
    }

    /// `getStatus()`: recomputes `m_status` once the weapon can fire again.
    pub fn get_status(&self, now: u32) -> WeaponStatus {
        if now < self.when_pre_attack_finished {
            return WeaponStatus::PreAttack;
        }
        if now >= self.when_we_can_fire_again {
            self.status.set(if self.ammo_in_clip > 0 {
                WeaponStatus::ReadyToFire
            } else {
                WeaponStatus::OutOfAmmo
            });
        }
        self.status.get()
    }

    // ---- bonus, ammo and reload ------------------------------------------------------------------------------------

    /// `computeBonus(source, extraBonusFlags, bonus)`.
    pub fn compute_bonus<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        extra_bonus_flags: WeaponBonusConditionFlags,
    ) -> WeaponBonus {
        let mut bonus = WeaponBonus::new();
        bonus.clear();
        let mut flags = host.weapon_bonus_condition(source);
        flags |= extra_bonus_flags;

        if let Some(container) = host.contained_by(source) {
            // we may be able to add in our container's flags
            if let Some(passed) = host.container_passed_weapon_bonus(container) {
                flags |= passed;
            }
        }

        if let Some(set) = host.global_weapon_bonus_set() {
            set.append_bonuses(flags, &mut bonus);
        }
        if let Some(extra) = &tpl(host.store(), self.template).extra_bonus {
            extra.append_bonuses(flags, &mut bonus);
        }
        bonus
    }

    /// `loadAmmoNow(source)`.
    pub fn load_ammo_now<H: WeaponHost>(
        &mut self,
        host: &H,
        source: ObjectId,
        shared: bool,
    ) -> Option<SharedReload> {
        let bonus = self.compute_bonus(host, source, 0);
        self.reload_with_bonus(host, &bonus, true, shared)
    }

    /// `reloadAmmo(source)`.
    pub fn reload_ammo<H: WeaponHost>(
        &mut self,
        host: &H,
        source: ObjectId,
        shared: bool,
    ) -> Option<SharedReload> {
        let bonus = self.compute_bonus(host, source, 0);
        self.reload_with_bonus(host, &bonus, false, shared)
    }

    /// `getClipReloadTime(source)`.
    pub fn get_clip_reload_time<H: WeaponHost>(&self, host: &H, source: ObjectId) -> i32 {
        let bonus = self.compute_bonus(host, source, 0);
        tpl(host.store(), self.template).get_clip_reload_time(&bonus)
    }

    /// `setClipPercentFull(percent, allowReduction)`.
    pub fn set_clip_percent_full<H: WeaponHost>(
        &mut self,
        host: &H,
        percent: f32,
        allow_reduction: bool,
    ) {
        let t = tpl(host.store(), self.template);
        if t.clip_size == 0 {
            return;
        }

        let ammo = real_to_int_floor(t.clip_size as f32 * percent);
        // `ammo > m_ammoInClip` compares an `Int` with an `UnsignedInt`: both as unsigned
        if (ammo as u32) > self.ammo_in_clip
            || (allow_reduction && (ammo as u32) < self.ammo_in_clip)
        {
            self.ammo_in_clip = ammo as u32;
            // upstream defect, kept: the test is inverted
            self.status.set(if self.ammo_in_clip != 0 {
                WeaponStatus::OutOfAmmo
            } else {
                WeaponStatus::ReadyToFire
            });
            self.when_last_reload_started = host.frame();
            self.when_we_can_fire_again = self.when_last_reload_started;
            self.rebuild_scatter_targets(host.store());
        }
    }

    /// `rebuildScatterTargets()`.
    pub fn rebuild_scatter_targets(&mut self, store: &WeaponStore) {
        self.scatter_targets_unused.clear();
        let count = tpl(store, self.template).scatter_targets.len() as i32;
        if count != 0 {
            // when I reload, I need to rebuild the list of ScatterTargets to shoot at
            for target_index in 0..count {
                self.scatter_targets_unused.push(target_index);
            }
        }
    }

    /// `reloadWithBonus(source, bonus, loadInstantly)`.
    pub fn reload_with_bonus<H: WeaponHost>(
        &mut self,
        host: &H,
        bonus: &WeaponBonus,
        load_instantly: bool,
        shared: bool,
    ) -> Option<SharedReload> {
        let (clip_size, clip_reload_time) = {
            let t = tpl(host.store(), self.template);
            (t.clip_size, t.get_clip_reload_time(bonus))
        };
        if clip_size > 0 && self.ammo_in_clip == clip_size as u32 && !shared {
            return None; // don't restart our reload delay
        }

        self.ammo_in_clip = clip_size as u32;
        if self.ammo_in_clip == 0 {
            // `m_ammoInClip <= 0` on an `UnsignedInt`: 0 == unlimited (or effectively so)
            self.ammo_in_clip = 0x7fff_ffff;
        }

        self.status.set(WeaponStatus::ReloadingClip);
        let reload_time: f32 = if load_instantly {
            0.0
        } else {
            clip_reload_time as f32
        };
        self.when_last_reload_started = host.frame();
        // `UnsignedInt + Real` is a `float` sum, converted back to `UnsignedInt`
        self.when_we_can_fire_again =
            float_to_unsigned(self.when_last_reload_started as f32 + reload_time);

        // if we are sharing reload times, go through the other weapons in the weapon set: set their
        // m_whenWeCanFireAgain to this one's and their m_status to this one's status
        let effect = if shared {
            Some(SharedReload {
                next_shot_frame: self.when_we_can_fire_again,
                status: WeaponStatus::ReloadingClip,
            })
        } else {
            None
        };

        self.rebuild_scatter_targets(host.store());
        effect
    }

    /// `onWeaponBonusChange(source)`: "we are concerned with our reload times being off if our ROF just changed".
    pub fn on_weapon_bonus_change<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        shared: bool,
    ) -> Option<SharedReload> {
        let bonus = self.compute_bonus(host, source, 0);

        let mut new_delay = 0;
        let mut need_update = false;

        let now = host.frame();
        if self.get_status(now) == WeaponStatus::ReloadingClip {
            new_delay = tpl(host.store(), self.template).get_clip_reload_time(&bonus);
            need_update = true;
        } else if self.get_status(now) == WeaponStatus::BetweenFiringShots {
            new_delay = host.delay_between_shots(self.template, &bonus);
            need_update = true;
        }

        if need_update {
            self.when_last_reload_started = host.frame();
            self.when_we_can_fire_again =
                self.when_last_reload_started.wrapping_add(new_delay as u32);

            if shared {
                return Some(SharedReload {
                    next_shot_frame: self.when_we_can_fire_again,
                    status: WeaponStatus::ReloadingClip,
                });
            }
        }
        None
    }

    // ---- ranges -------------------------------------------------------------------------------------------------------

    /// `getAttackRange(source)`.
    pub fn get_attack_range<H: WeaponHost>(&self, host: &H, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(host, source, 0);
        tpl(host.store(), self.template).get_attack_range(&bonus)
    }

    /// `getAttackDistance(source, victimObj, victimPos)`: the largest distance between the centres for the victim to
    /// be in range.
    pub fn get_attack_distance<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim: Option<ObjectId>,
    ) -> f32 {
        let mut range = self.get_attack_range(host, source);

        if let Some(v) = victim {
            // ATTACK_RANGE_IS_2D
            range += host.geometry(source).get_bounding_circle_radius();
            range += host.geometry(v).get_bounding_circle_radius();
        }

        range
    }

    /// `getPrimaryDamageRadius(source)`.
    pub fn get_primary_damage_radius<H: WeaponHost>(&self, host: &H, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(host, source, 0);
        tpl(host.store(), self.template).get_primary_damage_radius(&bonus)
    }

    /// The `RATIONALIZE_ATTACK_RANGE` test of "too close": `distSqr < minAttackRangeSqr`.
    fn is_inside_min_range(dist_sqr: f32, min_attack_range_sqr: f32) -> bool {
        dist_sqr < min_attack_range_sqr
    }

    /// `isSourceObjectWithGoalPositionWithinAttackRange(source, goalPos, target, targetPos)`.
    pub fn is_source_object_with_goal_position_within_attack_range<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        goal_pos: &Coord3D,
        target: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        let dist_sqr = if let Some(t) = target {
            host.goal_distance_squared_to_object(source, goal_pos, t, ATTACK_RANGE_CALC_TYPE)
        } else if let Some(p) = target_pos {
            host.goal_distance_squared_to_pos(source, goal_pos, p, ATTACK_RANGE_CALC_TYPE)
        } else {
            return false;
        };

        let attack_range_sqr = sqr(self.get_attack_range(host, source));
        let min_attack_range_sqr = sqr(tpl(host.store(), self.template).get_minimum_attack_range());
        if Self::is_inside_min_range(dist_sqr, min_attack_range_sqr) {
            return false;
        }
        dist_sqr <= attack_range_sqr
    }

    /// `isWithinAttackRange(source, pos)`.
    pub fn is_within_attack_range_pos<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        pos: &Coord3D,
    ) -> bool {
        let dist_sqr = host.distance_squared_to_pos(source, pos, ATTACK_RANGE_CALC_TYPE);
        let attack_range_sqr = sqr(self.get_attack_range(host, source));
        let min_attack_range_sqr = sqr(tpl(host.store(), self.template).get_minimum_attack_range());
        if Self::is_inside_min_range(dist_sqr, min_attack_range_sqr) {
            return false;
        }
        dist_sqr <= attack_range_sqr
    }

    /// `isWithinAttackRange(source, target)`.
    pub fn is_within_attack_range<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        target: ObjectId,
    ) -> bool {
        let attack_range_sqr = sqr(self.get_attack_range(host, source));

        let mut dist_sqr;
        if !host.is_kind_of(target, KINDOF_BRIDGE) {
            dist_sqr = host.distance_squared_to_object(source, target, ATTACK_RANGE_CALC_TYPE);
        } else {
            // special case - bridges have two attackable points at either end
            let (p1, p2) = host.bridge_attack_points(target);
            dist_sqr = host.distance_squared_to_pos(source, &p1, ATTACK_RANGE_CALC_TYPE);
            if dist_sqr > attack_range_sqr {
                // try the other one
                dist_sqr = host.distance_squared_to_pos(source, &p2, ATTACK_RANGE_CALC_TYPE);
            }
        }

        let min_attack_range_sqr = sqr(tpl(host.store(), self.template).get_minimum_attack_range());
        if Self::is_inside_min_range(dist_sqr, min_attack_range_sqr) {
            // too close. can't attack.
            return false;
        }

        if dist_sqr <= attack_range_sqr {
            // only compare contact weapons with structures. If you do the collision check against vehicles, the
            // attacker may get close enough to the vehicle to get crushed before it fires its weapon. jba.
            if tpl(host.store(), self.template).is_contact_weapon()
                && host.is_kind_of(target, KINDOF_STRUCTURE)
            {
                // we're close enough to fire off ranged weapons, but for contact weapons we want a more detailed
                // check to see if we're actually colliding with the target
                for them in host.potential_collisions(source) {
                    if target == them {
                        return true;
                    }
                }
                return false;
            }
            return true;
        }
        false
    }

    /// `isTooClose(source, target)`.
    pub fn is_too_close<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        target: ObjectId,
    ) -> bool {
        let min_attack_range = tpl(host.store(), self.template).get_minimum_attack_range();
        if min_attack_range == 0.0 {
            return false;
        }

        let dist_sqr = host.distance_squared_to_object(source, target, ATTACK_RANGE_CALC_TYPE);
        dist_sqr < sqr(min_attack_range)
    }

    /// `isTooClose(source, pos)`.
    pub fn is_too_close_pos<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        pos: &Coord3D,
    ) -> bool {
        let min_attack_range = tpl(host.store(), self.template).get_minimum_attack_range();
        if min_attack_range == 0.0 {
            return false;
        }

        let dist_sqr = host.distance_squared_to_pos(source, pos, ATTACK_RANGE_CALC_TYPE);
        dist_sqr < sqr(min_attack_range)
    }

    /// `isGoalPosWithinAttackRange(source, goalPos, target, targetPos)`.
    pub fn is_goal_pos_within_attack_range<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        goal_pos: &Coord3D,
        target: Option<ObjectId>,
        target_pos: &Coord3D,
    ) -> bool {
        // undersize by 1/4 of a pathfind cell, so that the goal is not teetering on the edge of firing range. jba.
        let attack_range_sqr =
            sqr(self.get_attack_range(host, source) - (PATHFIND_CELL_SIZE_F * 0.25));

        let dist_sqr;
        if let Some(t) = target {
            if host.is_kind_of(t, KINDOF_BRIDGE) {
                // special case - bridges have two attackable points at either end
                let (p1, p2) = host.bridge_attack_points(t);
                let mut d = host.goal_distance_squared_to_pos(
                    source,
                    goal_pos,
                    &p1,
                    ATTACK_RANGE_CALC_TYPE,
                );
                if d > attack_range_sqr {
                    // try the other one
                    d = host.goal_distance_squared_to_pos(
                        source,
                        goal_pos,
                        &p2,
                        ATTACK_RANGE_CALC_TYPE,
                    );
                }
                dist_sqr = d;
            } else {
                dist_sqr = host.goal_distance_squared_to_object(
                    source,
                    goal_pos,
                    t,
                    ATTACK_RANGE_CALC_TYPE,
                );
            }
        } else {
            dist_sqr = host.goal_distance_squared_to_pos(
                source,
                goal_pos,
                target_pos,
                ATTACK_RANGE_CALC_TYPE,
            );
        }

        // oversize by 1/4 of a pathfind cell, so that the goal is not teetering on the edge of firing range. jba.
        let min_attack_range_sqr = sqr(tpl(host.store(), self.template).get_minimum_attack_range()
            + (PATHFIND_CELL_SIZE_F * 0.25));
        if Self::is_inside_min_range(dist_sqr, min_attack_range_sqr) {
            return false;
        }
        dist_sqr <= attack_range_sqr
    }

    /// `computeApproachTarget(source, target, pos, angleOffset, approachTargetPos)`: a position from which `source`
    /// can attack the target, and whether it is already close enough (the position is then its own).
    pub fn compute_approach_target<H: WeaponHost>(
        &self,
        host: &mut H,
        source: ObjectId,
        target: Option<ObjectId>,
        pos: Option<Coord3D>,
        angle_offset: f32,
    ) -> (bool, Coord3D) {
        // compute unit direction vector from us to our victim
        let target_pos;
        let mut dir;
        if let Some(t) = target {
            target_pos = host.position(t);
            dir = host.vector_to_object(t, source, ATTACK_RANGE_CALC_TYPE);
        } else if let Some(p) = pos {
            target_pos = p;
            dir = host.vector_to_pos(source, &p, ATTACK_RANGE_CALC_TYPE);
            // flip the vector to get from source to pos
            dir.x = -dir.x;
            dir.y = -dir.y;
            dir.z = -dir.z;
        } else {
            let mut zero = Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            zero.zero();
            return (false, zero);
        }

        let dist = dir.length();
        let min_attack_range = tpl(host.store(), self.template).get_minimum_attack_range();
        if min_attack_range > PATHFIND_CELL_SIZE_F && dist < min_attack_range {
            // we are too close, so move away from the target
            // recompute dir, because if the bounding spheres touch, it will be 0
            let src_pos = host.position(source);
            dir.x = src_pos.x - target_pos.x;
            dir.y = src_pos.y - target_pos.y;
            // ATTACK_RANGE_IS_2D
            dir.z = 0.0;
            dir.normalize();

            // if we're airborne and too close, just head for the opposite side
            if host.is_above_terrain(source) {
                // don't do a 180 degree turn
                let angle = atan2_d(-dir.y, -dir.x);
                let mut rel_angle = host.orientation(source) - angle;
                if rel_angle > 2.0 * PI {
                    rel_angle -= 2.0 * PI;
                }
                if rel_angle < -2.0 * PI {
                    rel_angle += 2.0 * PI;
                }
                if fabs(rel_angle) < PI / 2.0 {
                    dir.x = -dir.x;
                    dir.y = -dir.y;
                    dir.z = -dir.z;
                }
            }

            if angle_offset != 0.0 {
                let angle = atan2_d(dir.y, dir.x);
                dir.x = trig::cos(angle + angle_offset);
                dir.y = trig::sin(angle + angle_offset);
            }

            // select a spot along the line between us, halfway between the min & max range
            let mut attack_range = (self.get_attack_range(host, source) + min_attack_range) / 2.0;
            // ATTACK_RANGE_IS_2D
            if let Some(t) = target {
                attack_range += host.geometry(t).get_bounding_circle_radius();
            }
            attack_range += host.geometry(source).get_bounding_circle_radius();
            let mut approach = Coord3D {
                x: attack_range * dir.x + target_pos.x,
                y: attack_range * dir.y + target_pos.y,
                z: attack_range * dir.z + target_pos.z,
            };
            // @todo - make sure we can get to the approach position
            clip_to_terrain_extent(&host.terrain_extent(), &mut approach);
            return (false, approach);
        }

        const FUDGE: f32 = 0.001;
        if dist < FUDGE {
            // we're close enough!
            (true, host.position(source))
        } else {
            if tpl(host.store(), self.template).is_contact_weapon() {
                // weapon is basically a contact weapon, like a car bomb: the approach target logic has been modified
                // to let it approach the object, so just return the target position. jba.
                return (false, target_pos);
            }

            dir.x /= dist;
            dir.y /= dist;
            dir.z /= dist;

            if angle_offset != 0.0 {
                let angle = atan2_d(dir.y, dir.x);
                dir.x = trig::cos(angle + angle_offset);
                dir.y = trig::sin(angle + angle_offset);
            }

            // select a spot along the line between us, in range of our weapon
            const ATTACK_RANGE_APPROACH_FUDGE: f32 = 0.9;
            let attack_range = self.get_attack_range(host, source) * ATTACK_RANGE_APPROACH_FUDGE;
            let mut approach = Coord3D {
                x: attack_range * dir.x + target_pos.x,
                y: attack_range * dir.y + target_pos.y,
                z: attack_range * dir.z + target_pos.z,
            };

            if host.ai_is_aircraft_that_adjusts_destination(source) {
                // adjust the target so that we are not stacked atop another aircraft
                host.adjust_target_destination(source, target, pos, self.template, &mut approach);
            }

            (false, approach)
        }
    }

    /// `getPercentReadyToFire()`.
    pub fn get_percent_ready_to_fire(&self, now: u32) -> f32 {
        match self.get_status(now) {
            WeaponStatus::OutOfAmmo | WeaponStatus::PreAttack => 0.0,
            WeaponStatus::ReadyToFire => 1.0,
            WeaponStatus::BetweenFiringShots | WeaponStatus::ReloadingClip => {
                let next_shot = self.possible_next_shot_frame();
                if now >= next_shot {
                    return 1.0;
                }

                let total_time = next_shot.wrapping_sub(self.when_last_reload_started);
                if total_time == 0 {
                    return 1.0;
                }

                let time_left = next_shot.wrapping_sub(now);
                let time_so_far = total_time.wrapping_sub(time_left);
                if time_so_far >= total_time {
                    1.0
                } else {
                    time_so_far as f32 / total_time as f32
                }
            }
        }
    }

    /// `estimateWeaponDamage(source, victim, victimPos)`.
    pub fn estimate_weapon_damage<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim: Option<ObjectId>,
        victim_pos: Option<Coord3D>,
    ) -> WResult<f32> {
        // if the weapon is just reloading, it's ok. if it's out of ammo (and won't autoreload), then we aren't
        // gonna do any damage.
        if self.get_status(host.frame()) == WeaponStatus::OutOfAmmo
            && !tpl(host.store(), self.template).get_auto_reloads_clip()
        {
            return Ok(0.0);
        }

        let bonus = self.compute_bonus(host, source, 0);

        estimate_weapon_template_damage(
            host,
            self.template,
            Some(source),
            victim,
            victim_pos,
            &bonus,
        )
    }

    /// `newProjectileFired(sourceObj, projectile, victimObj, victimPos)`: "I just made this projectile and may need
    /// to keep track of it".
    pub fn new_projectile_fired<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        victim_pos: Option<Coord3D>,
    ) -> Result<(), WeaponError> {
        // if I have a stream, I need to tell it about this new guy
        let stream_name = tpl(host.store(), self.template)
            .projectile_stream_name
            .clone();
        if stream_name.is_empty() {
            return Ok(()); // nope, no streak logic to do
        }

        let mut projectile_stream = if host.object_exists(self.projectile_stream_id) {
            Some(self.projectile_stream_id)
        } else {
            None
        };
        if projectile_stream.is_none() {
            self.projectile_stream_id = INVALID_ID; // reset, since it might have been "valid" but deleted out from under us
            let pst = host.find_thing_template(&stream_name);
            // a name that was not found: `newObject(NULL, ..)` throws `ERROR_BAD_ARG` upstream
            let pst = pst.ok_or(WeaponError::NullTemplate)?;
            let created = host.new_object_for_player_of(pst, source);
            projectile_stream = Some(created);
            self.projectile_stream_id = created;
        }

        // check for projectile stream update
        if let Some(stream) = projectile_stream {
            host.projectile_stream_add(stream, source, projectile, victim, victim_pos);
        }
        Ok(())
    }

    /// `createLaser(sourceObj, victimObj, victimPos)`.
    pub fn create_laser<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        victim: Option<ObjectId>,
        victim_pos: &Coord3D,
    ) {
        let (laser_name, bone_name) = {
            let t = tpl(host.store(), self.template);
            (t.laser_name.clone(), t.laser_bone_name.clone())
        };
        let Some(pst) = host.find_thing_template(&laser_name) else {
            return; // "could not find template for its laser"
        };
        let laser = host.new_object_for_player_of(pst, source);

        // give it a good basis in reality to ensure it can draw when on screen
        let src_pos = host.position(source);
        host.set_position(laser, &src_pos);

        // check for laser update
        let mut pos = *victim_pos;
        if let Some(v) = victim {
            if !host.is_kind_of(v, KINDOF_PROJECTILE) && !host.is_airborne_target(v) {
                // targets are positioned on the ground, so raise the beam up so we're not shooting their feet;
                // projectiles are a different story, target their exact position
                pos.z += 10.0;
            }
        }
        host.laser_init(laser, source, victim, &pos, &bone_name);
    }

    // ---- firing -------------------------------------------------------------------------------------------------------

    /// `privateFireWeapon(...)`: the one firing routine. `shared` is the answer to `source->isReloadTimeShared()`.
    #[allow(clippy::too_many_arguments)]
    fn private_fire_weapon<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        mut victim: Option<ObjectId>,
        mut victim_pos: Option<Coord3D>,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        extra_bonus_flags: WeaponBonusConditionFlags,
        mut projectile_id: Option<&mut ObjectId>,
        inflict_damage: bool,
        shared: bool,
    ) -> WResult<FireOutcome> {
        if let Some(p) = projectile_id.as_deref_mut() {
            *p = INVALID_ID;
        }

        // if we are a networked weapon, tell everyone nearby they might want to get in on this shot
        let (request_assist_range, leech, damage_type) = {
            let t = tpl(host.store(), self.template);
            (t.request_assist_range, t.leech_range_weapon, t.damage_type)
        };
        if request_assist_range != 0.0 {
            if let Some(v) = victim {
                self.process_request_assistance(host, source, v);
            }
        }

        // for weapon templates that have the leech range weapon flag set, it essentially grants the weapon unlimited
        // range for the remainder of the attack
        if leech {
            self.set_leech_range_active(true);
        }

        // special case damage type overrides requiring special handling
        match damage_type {
            DAMAGE_DEPLOY => {
                host.ai_begin_assault(source, victim);
            }

            DAMAGE_DISARM => {
                if let Some(v) = victim {
                    let mut found = false;
                    let veterancy = host.veterancy_level(source);
                    let fire_fx =
                        tpl(host.store(), self.template).fire_fxs[veterancy as usize].clone();
                    if host.has_land_mine_interface(v) {
                        host.weapon_fx(WeaponFx::Disarm {
                            fx: fire_fx.clone(),
                            at: v,
                        });
                        host.disarm_land_mine(v);
                        found = true;
                    }

                    // it's a mine, but doesn't have LandMineInterface...
                    if (!found && host.is_kind_of(v, KINDOF_MINE))
                        || host.is_kind_of(v, KINDOF_BOOBY_TRAP)
                        || host.is_kind_of(v, KINDOF_DEMOTRAP)
                    {
                        host.weapon_fx(WeaponFx::Disarm { fx: fire_fx, at: v });
                        host.destroy_object(v); // douse this thing before somebody gets hurt!
                        found = true;
                    }

                    if found {
                        host.record_mine_cleared(source);
                    }
                }

                self.max_shot_count = self.max_shot_count.wrapping_sub(1);
                self.ammo_in_clip = self.ammo_in_clip.wrapping_sub(1); // so we can use the delay between shots on the mine clearing weapon
                if self.ammo_in_clip == 0
                    && tpl(host.store(), self.template).get_auto_reloads_clip()
                {
                    let effect = self.reload_ammo(host, source, shared);
                    return Ok(FireOutcome {
                        reloaded: true,
                        shared: effect,
                    }); // reloaded
                } else {
                    return Ok(FireOutcome {
                        reloaded: false,
                        shared: None,
                    }); // did not reload
                }
            }

            DAMAGE_HACK => {
                // we're using a hacker unit to hack a target. Hacking has various effects and instead of inflicting
                // damage, we are waiting for a period of time until the hack takes effect.
            }

            _ => {}
        }

        let bonus = self.compute_bonus(host, source, extra_bonus_flags);

        if self.get_status(host.frame()) != WeaponStatus::ReadyToFire {
            return Ok(FireOutcome {
                reloaded: false,
                shared: None,
            });
        }

        let now = host.frame();
        let mut reloaded = false;
        let mut effect: Option<SharedReload> = None;
        if self.ammo_in_clip > 0 {
            let barrel_count = host
                .barrel_count(source, self.wslot)
                .ok_or(WeaponError::NullObject)?;
            if self.cur_barrel >= barrel_count {
                self.cur_barrel = 0;
                self.num_shots_for_cur_barrel = tpl(host.store(), self.template).shots_per_barrel;
            }

            if !self.scatter_targets_unused.is_empty() {
                // if I have a set scatter pattern, I need to offset the target by a random pick from that pattern
                if let Some(v) = victim {
                    victim_pos = Some(host.position(v));
                    victim = None;
                }
                let mut target_pos = victim_pos.ok_or(WeaponError::NullObject)?; // need to copy, as this pointer is actually inside somebody potentially
                let random_pick = host.random_int(0, self.scatter_targets_unused.len() as i32 - 1);
                let target_index = self.scatter_targets_unused[random_pick as usize];

                let scatter_target_scalar = tpl(host.store(), self.template).scatter_target_scalar; // essentially a radius, but operates only on this scatterTarget table
                let mut scatter_offset =
                    tpl(host.store(), self.template).scatter_targets[target_index as usize];

                scatter_offset.x *= scatter_target_scalar;
                scatter_offset.y *= scatter_target_scalar;

                target_pos.x += scatter_offset.x;
                target_pos.y += scatter_offset.y;

                target_pos.z = host.ground_height(target_pos.x, target_pos.y);

                // to erase from a vector, put the last on the one you used and pop the back
                let last = *self.scatter_targets_unused.last().unwrap_or(&0);
                self.scatter_targets_unused[random_pick as usize] = last;
                self.scatter_targets_unused.pop();
                fire_weapon_template(
                    host,
                    self.template,
                    Some(source),
                    self.wslot,
                    self.cur_barrel,
                    victim,
                    Some(target_pos),
                    &bonus,
                    is_projectile_detonation,
                    ignore_ranges,
                    self,
                    projectile_id.as_deref_mut(),
                    inflict_damage,
                )?;
            } else {
                fire_weapon_template(
                    host,
                    self.template,
                    Some(source),
                    self.wslot,
                    self.cur_barrel,
                    victim,
                    victim_pos,
                    &bonus,
                    is_projectile_detonation,
                    ignore_ranges,
                    self,
                    projectile_id,
                    inflict_damage,
                )?;
            }

            self.last_fire_frame = now;
            self.ammo_in_clip = self.ammo_in_clip.wrapping_sub(1);
            self.max_shot_count = self.max_shot_count.wrapping_sub(1);
            self.num_shots_for_cur_barrel = self.num_shots_for_cur_barrel.wrapping_sub(1);
            if self.num_shots_for_cur_barrel <= 0 {
                self.cur_barrel = self.cur_barrel.wrapping_add(1);
                self.num_shots_for_cur_barrel = tpl(host.store(), self.template).shots_per_barrel;
            }

            if self.ammo_in_clip == 0 {
                if tpl(host.store(), self.template).get_auto_reloads_clip() {
                    effect = self.reload_ammo(host, source, shared);
                    reloaded = true;
                } else {
                    self.status.set(WeaponStatus::OutOfAmmo);
                    self.when_we_can_fire_again = 0x7fff_ffff;
                }
            } else {
                self.status.set(WeaponStatus::BetweenFiringShots);
                let delay = host.delay_between_shots(self.template, &bonus);
                self.when_last_reload_started = now;
                self.when_we_can_fire_again = now.wrapping_add(delay as u32);

                // if we are sharing reload times, go through the other weapons in the weapon set: set their
                // m_whenWeCanFireAgain to this one's delay and their m_status to this one's status
                if shared {
                    effect = Some(SharedReload {
                        next_shot_frame: self.when_we_can_fire_again,
                        status: WeaponStatus::BetweenFiringShots,
                    });
                }
            }
        }

        Ok(FireOutcome {
            reloaded,
            shared: effect,
        })
    }

    /// `preFireWeapon(source, victim)`.
    pub fn pre_fire_weapon<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        victim: Option<ObjectId>,
    ) -> WResult<()> {
        let delay = self.get_pre_attack_delay(host, source, victim)?;
        if delay > 0 {
            self.set_status(WeaponStatus::PreAttack);
            self.set_pre_attack_finished_frame(host.frame().wrapping_add(delay as u32));
            if tpl(host.store(), self.template).leech_range_weapon {
                self.set_leech_range_active(true);
            }
        }
        Ok(())
    }

    /// `fireWeapon(source, target, projectileID)`: true if the clip was reloaded after the shot.
    pub fn fire_weapon<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        target: ObjectId,
        projectile_id: Option<&mut ObjectId>,
        shared: bool,
    ) -> WResult<FireOutcome> {
        self.private_fire_weapon(
            host,
            source,
            Some(target),
            None,
            false,
            false,
            0,
            projectile_id,
            true,
            shared,
        )
    }

    /// `fireWeapon(source, pos, projectileID)`: true if the clip was reloaded after the shot.
    pub fn fire_weapon_at_pos<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        pos: &Coord3D,
        projectile_id: Option<&mut ObjectId>,
        shared: bool,
    ) -> WResult<FireOutcome> {
        self.private_fire_weapon(
            host,
            source,
            None,
            Some(*pos),
            false,
            false,
            0,
            projectile_id,
            true,
            shared,
        )
    }

    /// `fireProjectileDetonationWeapon(source, target, extraBonusFlags, inflictDamage)`.
    pub fn fire_projectile_detonation_weapon<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        target: ObjectId,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
        shared: bool,
    ) -> WResult<Option<SharedReload>> {
        Ok(self
            .private_fire_weapon(
                host,
                source,
                Some(target),
                None,
                true,
                false,
                extra_bonus_flags,
                None,
                inflict_damage,
                shared,
            )?
            .shared)
    }

    /// `fireProjectileDetonationWeapon(source, pos, extraBonusFlags, inflictDamage)`.
    pub fn fire_projectile_detonation_weapon_at_pos<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        pos: &Coord3D,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
        shared: bool,
    ) -> WResult<Option<SharedReload>> {
        Ok(self
            .private_fire_weapon(
                host,
                source,
                None,
                Some(*pos),
                true,
                false,
                extra_bonus_flags,
                None,
                inflict_damage,
                shared,
            )?
            .shared)
    }

    /// `forceFireWeapon(source, pos)`: fire at a position, ignoring the ranges, and give back the projectile if one
    /// was made (a script gains control of it). Upstream passes `NULL` for the bonus flags.
    pub fn force_fire_weapon<H: WeaponHost>(
        &mut self,
        host: &mut H,
        source: ObjectId,
        pos: &Coord3D,
        shared: bool,
    ) -> WResult<(Option<ObjectId>, FireOutcome)> {
        let mut projectile_id = INVALID_ID;
        let outcome = self.private_fire_weapon(
            host,
            source,
            None,
            Some(*pos),
            false,
            true,
            0,
            Some(&mut projectile_id),
            true,
            shared,
        )?;
        Ok((
            if host.object_exists(projectile_id) {
                Some(projectile_id)
            } else {
                None
            },
            outcome,
        ))
    }

    /// `isWithinTargetPitch(source, victim)`.
    pub fn is_within_target_pitch<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim: ObjectId,
    ) -> bool {
        let t = tpl(host.store(), self.template);
        if t.is_contact_weapon() || !self.is_pitch_limited() {
            return true;
        }

        let src = host.position(source);
        let dst = host.position(victim);

        const ACCCEPTABLE_DZ: f32 = 10.0;
        if fabs(dst.z - src.z) < ACCCEPTABLE_DZ {
            return true; // always good enough if dz is small, regardless of pitch
        }

        let (min_pitch, max_pitch) =
            host.geometry(source)
                .calc_pitches(&src, &host.geometry(victim), &dst);

        // if there's any intersection between the two pitch ranges, we're good to go
        if (min_pitch >= t.min_target_pitch && min_pitch <= t.max_target_pitch)
            || (max_pitch >= t.min_target_pitch && max_pitch <= t.max_target_pitch)
            || (min_pitch <= t.min_target_pitch && max_pitch >= t.max_target_pitch)
        {
            return true;
        }

        false
    }

    /// `isDamageWeapon()`.
    pub fn is_damage_weapon(&self, store: &WeaponStore) -> bool {
        let t = tpl(store, self.template);
        // these damage types are special attacks that don't do damage directly, even if they can indirectly
        match t.damage_type {
            DAMAGE_DEPLOY => return true,
            DAMAGE_DISARM => return true, // hmm, can only "damage" mines, but still...
            DAMAGE_HACK => return false,
            _ => {}
        }

        // use no bonus
        let who_cares = WeaponBonus::new();
        t.get_primary_damage(&who_cares) > 0.0 || t.get_secondary_damage(&who_cares) > 0.0
    }

    /// `getPreAttackDelay(source, victim)`.
    pub fn get_pre_attack_delay<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim: Option<ObjectId>,
    ) -> WResult<i32> {
        // look for a reason to return zero and have no delay
        let t = tpl(host.store(), self.template);
        let prefire_type = t.prefire_type;
        if prefire_type == PREFIRE_PER_CLIP {
            // `UnsignedInt < Int` compares as unsigned (clip_size > 0 here)
            if t.clip_size > 0 && self.ammo_in_clip < t.clip_size as u32 {
                return Ok(0); // I only delay once a clip, and this is not the first shot
            }
        } else if prefire_type == PREFIRE_PER_ATTACK
            && host.num_consecutive_shots_fired_at_target(source, victim) > 0
        {
            return Ok(0); // I only delay once an attack, and I have already shot this guy
        }
        // else it is per shot, so it always applies

        let bonus = self.compute_bonus(host, source, 0);
        t.get_pre_attack_delay(&bonus)
    }

    /// `processRequestAssistance(requestingObject, victimObject)`: "iterate through our player's objects, and tell
    /// everyone like us within our assistance range who is free to do so to assist us on this shot".
    pub fn process_request_assistance<H: WeaponHost>(
        &self,
        host: &mut H,
        requesting: ObjectId,
        victim: ObjectId,
    ) {
        if host.controlling_player(requesting).is_none() {
            return;
        }

        let request_range = tpl(host.store(), self.template).request_assist_range;
        let request_distance_squared = request_range * request_range;

        for request_of in host.controlling_player_objects(requesting) {
            // makeAssistanceRequest
            // don't ask ourselves (can't believe I forgot this one)
            if request_of == requesting {
                continue;
            }

            // only request of our kind of people
            if !host.is_equivalent_template(request_of, requesting) {
                continue;
            }

            // who are close enough
            let dist_sq =
                host.distance_squared_to_object(request_of, requesting, Dct::FromCenter2D);
            if dist_sq > request_distance_squared {
                continue;
            }

            // and respond to requests
            let Some(free) = host.assisted_targeting_is_free(request_of) else {
                continue;
            };

            // and say yes
            if !free {
                continue;
            }

            host.assisted_targeting_assist_attack(request_of, requesting, victim);
        }
    }

    /// `getFiringLineOfSightOrigin(source, origin)`: "we have to simplify this" (the launch position is not used).
    fn firing_line_of_sight_origin<H: WeaponHost>(
        host: &H,
        source: ObjectId,
        origin: &mut Coord3D,
    ) {
        origin.z += host.geometry(source).get_max_height_above_position();
    }

    /// `isClearFiringLineOfSightTerrain(source, victim)`.
    pub fn is_clear_firing_line_of_sight_terrain<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim: ObjectId,
    ) -> bool {
        let mut origin = host.position(source);
        Self::firing_line_of_sight_origin(host, source, &mut origin);
        let victim_pos = host
            .geometry(victim)
            .get_center_position(&host.position(victim));
        host.is_clear_line_of_sight_terrain(&origin, &victim_pos)
    }

    /// `isClearFiringLineOfSightTerrain(source, victimPos)`.
    pub fn is_clear_firing_line_of_sight_terrain_pos<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        victim_pos: &Coord3D,
    ) -> bool {
        let mut origin = host.position(source);
        Self::firing_line_of_sight_origin(host, source, &mut origin);
        host.is_clear_line_of_sight_terrain(&origin, victim_pos)
    }

    /// `isClearGoalFiringLineOfSightTerrain(source, goalPos, victim)`: whether, were `source` at `goalPos`, it
    /// would have a clear line of sight.
    pub fn is_clear_goal_firing_line_of_sight_terrain<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        goal_pos: &Coord3D,
        victim: ObjectId,
    ) -> bool {
        let mut origin = *goal_pos;
        Self::firing_line_of_sight_origin(host, source, &mut origin);
        let victim_pos = host
            .geometry(victim)
            .get_center_position(&host.position(victim));
        host.is_clear_line_of_sight_terrain(&origin, &victim_pos)
    }

    /// `isClearGoalFiringLineOfSightTerrain(source, goalPos, victimPos)`.
    pub fn is_clear_goal_firing_line_of_sight_terrain_pos<H: WeaponHost>(
        &self,
        host: &H,
        source: ObjectId,
        goal_pos: &Coord3D,
        victim_pos: &Coord3D,
    ) -> bool {
        let mut origin = *goal_pos;
        Self::firing_line_of_sight_origin(host, source, &mut origin);
        host.is_clear_line_of_sight_terrain(&origin, victim_pos)
    }

    /// `transferNextShotStatsFrom(weapon)`: key weapon stats for Jarmen Kell to and from the bike for the sniper
    /// attack (patch 1.01).
    pub fn transfer_next_shot_stats_from(&mut self, weapon: &Weapon, now: u32) {
        self.when_we_can_fire_again = weapon.possible_next_shot_frame();
        self.when_last_reload_started = weapon.last_reload_started_frame();
        self.status.set(weapon.get_status(now));
    }

    // ---- snapshot ------------------------------------------------------------------------------------------------------

    /// `Weapon::crc(xfer)`. (Not the status, and not the suspend-FX frame.) `template_name` is
    /// `m_template->getName()`.
    pub fn crc_with_name(&mut self, template_name: &[u8], xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut tmpl_name = template_name.to_vec();
        xfer.xfer_ascii_string(&mut tmpl_name)?;

        // slot
        let mut slot = self.wslot.to_le_bytes();
        xfer.xfer_user(&mut slot)?;

        // ammo
        xfer.xfer_unsigned_int(&mut self.ammo_in_clip)?;
        // when can fire again
        xfer.xfer_unsigned_int(&mut self.when_we_can_fire_again)?;
        // when pre attack finished
        xfer.xfer_unsigned_int(&mut self.when_pre_attack_finished)?;
        // when last reload started
        xfer.xfer_unsigned_int(&mut self.when_last_reload_started)?;
        // last fire frame
        xfer.xfer_unsigned_int(&mut self.last_fire_frame)?;
        // projectile stream object
        xfer.xfer_object_id(&mut self.projectile_stream_id)?;
        // laser object (defunct)
        let mut laser_id_unused = INVALID_ID;
        xfer.xfer_object_id(&mut laser_id_unused)?;
        // max shot count
        xfer.xfer_int(&mut self.max_shot_count)?;
        // current barrel
        xfer.xfer_int(&mut self.cur_barrel)?;
        // num shots for current barrel
        xfer.xfer_int(&mut self.num_shots_for_cur_barrel)?;

        // scatter targets unused
        let mut scatter_count = self.scatter_targets_unused.len() as u16;
        xfer.xfer_unsigned_short(&mut scatter_count)?;
        for it in &self.scatter_targets_unused {
            let mut int_data = *it;
            xfer.xfer_int(&mut int_data)?;
        }

        // pitch limited
        xfer.xfer_bool(&mut self.pitch_limited)?;
        // leech weapon range active
        xfer.xfer_bool(&mut self.leech_weapon_range_active)
    }

    /// `Weapon::xfer(xfer)` (version 3). `current_name` is `m_template->getName()`. On a load the saved template
    /// name is looked up in `templates` (name and store position of every weapon template: `findWeaponTemplate`),
    /// and a name it does not have is `INI_INVALID_DATA`.
    pub fn xfer_with_names(
        &mut self,
        current_name: &[u8],
        templates: &[(Vec<u8>, usize)],
        xfer: &mut dyn Xfer,
    ) -> Result<(), WeaponXferError> {
        // version
        let current_version = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if version >= 2 {
            let mut tmpl_name = current_name.to_vec();
            xfer.xfer_ascii_string(&mut tmpl_name)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                // `findWeaponTemplate`: "None" (any case) is `NULL`, otherwise the name key is the name
                let found = if tmpl_name.eq_ignore_ascii_case(b"None") {
                    None
                } else {
                    templates.iter().find(|(n, _)| *n == tmpl_name)
                };
                match found {
                    Some((_, i)) => self.template = *i,
                    None => return Err(WeaponXferError::InvalidData), // throw INI_INVALID_DATA
                }
            }
        }

        // slot
        let mut slot = self.wslot.to_le_bytes();
        xfer.xfer_user(&mut slot)?;
        self.wslot = i32::from_le_bytes(slot);

        // status
        let mut status = (self.status.get() as i32).to_le_bytes();
        xfer.xfer_user(&mut status)?;
        self.status
            .set(weapon_status_from_i32(i32::from_le_bytes(status)));

        // ammo
        xfer.xfer_unsigned_int(&mut self.ammo_in_clip)?;
        // when can fire again
        xfer.xfer_unsigned_int(&mut self.when_we_can_fire_again)?;
        // when pre attack finished
        xfer.xfer_unsigned_int(&mut self.when_pre_attack_finished)?;
        // when last reload started
        xfer.xfer_unsigned_int(&mut self.when_last_reload_started)?;
        // last fire frame
        xfer.xfer_unsigned_int(&mut self.last_fire_frame)?;

        // suspendFXFrame, this affects client only
        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.suspend_fx_frame)?;
        } else {
            self.suspend_fx_frame = 0;
        }

        // projectile stream object
        xfer.xfer_object_id(&mut self.projectile_stream_id)?;
        // laser object
        let mut laser_id_unused = INVALID_ID;
        xfer.xfer_object_id(&mut laser_id_unused)?;
        // max shot count
        xfer.xfer_int(&mut self.max_shot_count)?;
        // current barrel
        xfer.xfer_int(&mut self.cur_barrel)?;
        // num shots for current barrel
        xfer.xfer_int(&mut self.num_shots_for_cur_barrel)?;

        // scatter targets unused
        let mut scatter_count = self.scatter_targets_unused.len() as u16;
        xfer.xfer_unsigned_short(&mut scatter_count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for it in &self.scatter_targets_unused {
                let mut int_data = *it;
                xfer.xfer_int(&mut int_data)?;
            }
        } else {
            // sanity, the scatter targets must be empty
            self.scatter_targets_unused.clear();

            for _ in 0..scatter_count {
                let mut int_data = 0;
                xfer.xfer_int(&mut int_data)?;
                self.scatter_targets_unused.push(int_data);
            }
        }

        // pitch limited
        xfer.xfer_bool(&mut self.pitch_limited)?;
        // leech weapon range active
        xfer.xfer_bool(&mut self.leech_weapon_range_active)?;
        Ok(())
    }

    /// A weapon as `WeaponSet::xfer` allocates it on a load (`allocateNewWeapon(wt, slot)`) before the stream
    /// overwrites it: every field `xfer` moves is overwritten (the suspend-FX frame is, for version 3 and set to 0
    /// below it), so the frame the constructor reads does not matter.
    pub fn for_load(template: usize, wslot: i32) -> Weapon {
        Weapon {
            template,
            wslot,
            status: Cell::new(WeaponStatus::OutOfAmmo),
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            suspend_fx_frame: 0,
            projectile_stream_id: INVALID_ID,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            cur_barrel: 0,
            num_shots_for_cur_barrel: 0,
            scatter_targets_unused: Vec::new(),
            pitch_limited: false,
            leech_weapon_range_active: false,
        }
    }

    /// `Weapon::loadPostProcess()`: forget a projectile stream that no longer exists.
    pub fn load_post_process<H: WeaponHost>(&mut self, host: &H) {
        if self.projectile_stream_id != INVALID_ID && !host.object_exists(self.projectile_stream_id)
        {
            self.projectile_stream_id = INVALID_ID;
        }
    }

    /// `Weapon::calcProjectileLaunchPosition(launcher, wslot, specificBarrelToUse, worldTransform, worldPos)`.
    pub fn calc_projectile_launch_position<H: WeaponHost>(
        host: &H,
        launcher: ObjectId,
        wslot: i32,
        specific_barrel_to_use: i32,
    ) -> (Matrix3D, Coord3D) {
        if let Some(container) = host.contained_by(launcher) {
            // if we are in an enclosing container, our launch position is our actual position
            if host.contain_is_enclosing_container_for(container, launcher) {
                let world_transform = host.transform_matrix(launcher);
                let tmp = world_transform.get_translation();
                return (
                    world_transform,
                    Coord3D {
                        x: tmp.x,
                        y: tmp.y,
                        z: tmp.z,
                    },
                );
            }
        }

        let (tur, turret_angle, turret_pitch) =
            match host.ai_which_turret_for_weapon_slot(launcher, wslot) {
                Some(t) => t,
                None => (TURRET_INVALID, 0.0, 0.0),
            };

        let mut attach_transform = Matrix3D::default();
        attach_transform.make_identity();
        let mut turret_rot_pos = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut turret_pitch_pos = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        match host.projectile_launch_offset(launcher, wslot, specific_barrel_to_use, tur) {
            Some(lo) => {
                attach_transform = lo.attach_transform;
                turret_rot_pos = lo.turret_rot_pos;
                turret_pitch_pos = lo.turret_pitch_pos;
            }
            None => {
                // "ProjectileLaunchPos %d %d not found!"
                attach_transform.make_identity();
                turret_rot_pos.zero();
                turret_pitch_pos.zero();
            }
        }
        if tur != TURRET_INVALID {
            // the attach transform is the pristine front and center position of the fire point; we can't read from
            // the client, so we need to reproduce the actual point that takes turn and pitch into account
            let mut turn_adjustment = Matrix3D::default();
            turn_adjustment.make_identity();
            let mut pitch_adjustment = Matrix3D::default();
            pitch_adjustment.make_identity();

            // to rotate about a point, move that point to 0,0, rotate, then move it back; pre rotate will keep the
            // first twist from screwing the angle of the second pitch
            pitch_adjustment.translate(turret_pitch_pos.x, turret_pitch_pos.y, turret_pitch_pos.z);
            pitch_adjustment.in_place_pre_rotate_y(-turret_pitch);
            pitch_adjustment.translate(
                -turret_pitch_pos.x,
                -turret_pitch_pos.y,
                -turret_pitch_pos.z,
            );

            turn_adjustment.translate(turret_rot_pos.x, turret_rot_pos.y, turret_rot_pos.z);
            turn_adjustment.in_place_pre_rotate_z(turret_angle);
            turn_adjustment.translate(-turret_rot_pos.x, -turret_rot_pos.y, -turret_rot_pos.z);

            let tmp = attach_transform;
            attach_transform.mul(&turn_adjustment, &pitch_adjustment);
            attach_transform.post_mul(&tmp);
        }

        let world_transform = host.convert_bone_transform_to_world(launcher, &attach_transform);

        let tmp = world_transform.get_translation();
        (
            world_transform,
            Coord3D {
                x: tmp.x,
                y: tmp.y,
                z: tmp.z,
            },
        )
    }

    /// `Weapon::positionProjectileForLaunch(projectile, launcher, wslot, specificBarrelToUse)`. A launcher that is
    /// gone (`None`) destroys the projectile.
    pub fn position_projectile_for_launch<H: WeaponHost>(
        host: &mut H,
        projectile: ObjectId,
        launcher: Option<ObjectId>,
        wslot: i32,
        specific_barrel_to_use: i32,
    ) {
        // if our launch vehicle is gone, destroy ourselves
        let Some(launcher) = launcher else {
            host.destroy_object(projectile);
            return;
        };

        let (world_transform, world_pos) =
            Weapon::calc_projectile_launch_position(host, launcher, wslot, specific_barrel_to_use);

        host.unhide_drawable(projectile);
        host.set_transform_matrix(projectile, &world_transform);
        host.set_position(projectile, &world_pos);
        host.set_experience_sink(projectile, launcher);

        host.transfer_velocity_and_ignore_collisions(launcher, projectile);
    }
}

/// A weapon as `xferSnapshot` sees it: the weapon, checked out, with the names it needs (so no host is held).
/// The error of a failed `xfer` (`throw INI_INVALID_DATA` is not an `XferStatus`) is left in `error`.
#[derive(Clone, Debug)]
pub struct WeaponSnapshot {
    /// The weapon.
    pub weapon: Weapon,
    /// `m_template->getName()`.
    pub current_name: Vec<u8>,
    /// Every weapon template's name and store position (only needed, and only filled, on a load).
    pub templates: Vec<(Vec<u8>, usize)>,
    /// Why the `xfer` failed, if it did.
    pub error: Option<WeaponXferError>,
}

impl Snapshot for WeaponSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let name = self.current_name.clone();
        self.weapon.crc_with_name(&name, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let name = self.current_name.clone();
        let templates = std::mem::take(&mut self.templates);
        let r = self.weapon.xfer_with_names(&name, &templates, xfer);
        self.templates = templates;
        match r {
            Ok(()) => Ok(()),
            Err(WeaponXferError::Xfer(s)) => Err(s),
            Err(e) => {
                self.error = Some(e);
                Err(crate::common::xfer::XferStatus::ErrorUnknown)
            }
        }
    }

    /// `Weapon::loadPostProcess()` needs the game logic: [`Weapon::load_post_process`], called by the owner.
    fn load_post_process(&mut self) {}
}

/// `WeaponStatus` from the `int` that `xferUser` moved (any other value is kept as upstream keeps it: an enum
/// of that value; here the nearest defined state is not invented, an unknown value reads as `ReadyToFire`).
fn weapon_status_from_i32(v: i32) -> WeaponStatus {
    match v {
        0 => WeaponStatus::ReadyToFire,
        1 => WeaponStatus::OutOfAmmo,
        2 => WeaponStatus::BetweenFiringShots,
        3 => WeaponStatus::ReloadingClip,
        4 => WeaponStatus::PreAttack,
        _ => WeaponStatus::ReadyToFire,
    }
}

/// `clipToTerrainExtent(approachTargetPos)`.
fn clip_to_terrain_extent(bounds: &Region3D, approach_target_pos: &mut Coord3D) {
    if approach_target_pos.x < bounds.lo.x + PATHFIND_CELL_SIZE_F {
        approach_target_pos.x = bounds.lo.x + PATHFIND_CELL_SIZE_F;
    }
    if approach_target_pos.y < bounds.lo.y + PATHFIND_CELL_SIZE_F {
        approach_target_pos.y = bounds.lo.y + PATHFIND_CELL_SIZE_F;
    }
    if approach_target_pos.x > bounds.hi.x - PATHFIND_CELL_SIZE_F {
        approach_target_pos.x = bounds.hi.x - PATHFIND_CELL_SIZE_F;
    }
    if approach_target_pos.y > bounds.hi.y - PATHFIND_CELL_SIZE_F {
        approach_target_pos.y = bounds.hi.y - PATHFIND_CELL_SIZE_F;
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// WeaponTemplate: the firing half
// ---------------------------------------------------------------------------------------------------------------------

/// `WeaponTemplate::estimateWeaponTemplateDamage(sourceObj, victimObj, victimPos, bonus)`.
pub fn estimate_weapon_template_damage<H: WeaponHost>(
    host: &H,
    template: usize,
    source: Option<ObjectId>,
    victim: Option<ObjectId>,
    victim_pos: Option<Coord3D>,
    bonus: &WeaponBonus,
) -> WResult<f32> {
    let t = tpl(host.store(), template);
    let Some(source) = source else { return Ok(0.0) };
    if victim.is_none() && victim_pos.is_none() {
        return Ok(0.0);
    }

    let damage_type: DamageType = t.damage_type;
    let death_type: DeathType = t.death_type;

    if let Some(v) = victim {
        if host.is_kind_of(v, KINDOF_SHRUBBERY) {
            if death_type == DEATH_BURNED {
                // this is just a nonzero value, to ensure we can target shrubbery with flame weapons, regardless...
                return Ok(1.0);
            } else {
                return Ok(0.0);
            }
        }
    }

    // hmm.. must be shooting a firebase or such, if there is noone home to take the bullet, return 0!
    // (upstream tests the victim before it tests the damage type, and the victim may be NULL)
    let victim = victim.ok_or(WeaponError::NullObject)?;
    if host.is_kind_of(victim, KINDOF_STRUCTURE) && damage_type == DAMAGE_SNIPER {
        if let Some(c) = host.contain_facts(victim) {
            if c.count == 0 {
                return Ok(0.0);
            }
        }
    }

    if damage_type == DAMAGE_SURRENDER || t.allow_attack_garrisoned_bldgs {
        if let Some(c) = host.contain_facts(victim) {
            if c.count > 0 && c.garrisonable && !c.immune_to_clear_building_attacks {
                // this is just a nonzero value, to ensure we can target garrisoned things with surrender weapons,
                // regardless...
                return Ok(1.0);
            }
        }
    }

    if damage_type == crate::logic::damage::DAMAGE_DISARM {
        if host.is_kind_of(victim, KINDOF_MINE)
            || host.is_kind_of(victim, KINDOF_BOOBY_TRAP)
            || host.is_kind_of(victim, KINDOF_DEMOTRAP)
        {
            // this is just a nonzero value, to ensure we can target mines with disarm weapons, regardless...
            return Ok(1.0);
        }
        return Ok(0.0);
    }
    if damage_type == DAMAGE_DEPLOY && !host.is_airborne_target(victim) {
        return Ok(1.0);
    }

    // @todo Kris need to examine the DAMAGE_HACK type for damage estimation purposes.

    let damage_amount = t.get_primary_damage(bonus);
    let damage_info = DamageInfoInput {
        damage_type,
        death_type,
        source_id: source,
        amount: damage_amount,
        ..Default::default()
    };
    Ok(host.estimate_damage(victim, &damage_info))
}

/// `WeaponTemplate::shouldProjectileCollideWith(projectileLauncher, projectile, thingWeCollidedWith,
/// intendedVictimID)`. `intended_victim` is `INVALID_ID` for a position-shot.
pub fn should_projectile_collide_with<H: WeaponHost>(
    host: &H,
    template: usize,
    projectile_launcher: Option<ObjectId>,
    projectile: Option<ObjectId>,
    thing_we_collided_with: Option<ObjectId>,
    intended_victim: ObjectId,
) -> bool {
    let t = tpl(host.store(), template);
    let (Some(projectile), Some(thing)) = (projectile, thing_we_collided_with) else {
        return false;
    };

    // if it's our intended victim, we want to collide with it, regardless of any other considerations
    if intended_victim == thing {
        return true;
    }

    if let Some(launcher) = projectile_launcher {
        // don't hit your own launcher, ever
        if launcher == thing {
            return false;
        }

        // if our launcher is inside something, and that something is 'thingWeCollidedWith' we won't collide
        let launcher_contained_by = host.contained_by(launcher);
        if launcher_contained_by == Some(thing) {
            return false;
        }
    }

    // never bother burning already-burned things. (srj)
    if (t.damage_type == DAMAGE_FLAME || t.damage_type == DAMAGE_PARTICLE_BEAM)
        && host.test_status(thing, OBJECT_STATUS_BURNED)
    {
        return false;
    }

    // horrible special case for airplanes sitting on airfields: the projectile might "collide" with the airfield's
    // (invisible) collision geometry when a resting plane is targeted. we don't want this.
    if host.is_kind_of(thing, KINDOF_FS_AIRFIELD) {
        // so, if we are an airfield, and our intended victim has a reserved space with us, it "belongs" to us and
        // collisions intended for it should not detonate as a result of colliding with us
        if host.parking_place_has_reserved_space(thing, intended_victim) {
            return false;
        }
    }

    // if something has a Sneaky Target offset, it is momentarily immune to being hit... (srj)
    if host.ai_sneaky_targeting_offset(thing).is_some() {
        return false;
    }

    let mut required_mask: u32 = 0;

    let r = host.relationship(projectile, thing);
    if r == Relationship::Allies {
        required_mask = WEAPON_COLLIDE_ALLIES;
    } else if r == Relationship::Enemies {
        required_mask = WEAPON_COLLIDE_ENEMIES;
    }

    if host.is_kind_of(thing, KINDOF_STRUCTURE) {
        if host.controlling_player(thing) == host.controlling_player(projectile) {
            required_mask |= WEAPON_COLLIDE_CONTROLLED_STRUCTURES;
        } else {
            required_mask |= WEAPON_COLLIDE_STRUCTURES;
        }
    }
    if host.is_kind_of(thing, KINDOF_SHRUBBERY) {
        required_mask |= WEAPON_COLLIDE_SHRUBBERY;
    }
    if host.is_kind_of(thing, KINDOF_PROJECTILE) {
        required_mask |= WEAPON_COLLIDE_PROJECTILE;
    }
    if host.fence_width(thing) > 0.0 {
        required_mask |= WEAPON_COLLIDE_WALLS;
    }
    if host.is_kind_of(thing, KINDOF_SMALL_MISSILE) {
        required_mask |= WEAPON_COLLIDE_SMALL_MISSILES; // all missiles are also projectiles!
    }
    if host.is_kind_of(thing, KINDOF_BALLISTIC_MISSILE) {
        required_mask |= WEAPON_COLLIDE_BALLISTIC_MISSILES; // all missiles are also projectiles!
    }

    // if any in requiredMask are present in collidemask, do the collision. (srj)
    (t.collide_mask & required_mask) != 0
}

/// `WeaponTemplate::fireWeaponTemplate(...)`: fire the weapon, return the frame the damage will be dealt (0 when a
/// projectile was made, the current frame when the damage was dealt at once, or nothing happened).
#[allow(clippy::too_many_arguments)]
pub fn fire_weapon_template<H: WeaponHost>(
    host: &mut H,
    template: usize,
    source: Option<ObjectId>,
    wslot: i32,
    specific_barrel_to_use: i32,
    mut victim: Option<ObjectId>,
    victim_pos: Option<Coord3D>,
    bonus: &WeaponBonus,
    is_projectile_detonation: bool,
    ignore_ranges: bool,
    firing_weapon: &mut Weapon,
    projectile_id: Option<&mut ObjectId>,
    inflict_damage: bool,
) -> WResult<u32> {
    let Some(source) = source else { return Ok(0) };
    if victim.is_none() && victim_pos.is_none() {
        return Ok(0);
    }

    let source_id = source;
    let source_pos = host.position(source);

    let dist_sqr;
    let mut victim_id: ObjectId;
    let mut vpos: Coord3D;
    if let Some(v) = victim {
        vpos = host.position(v);
        victim_id = v;

        if let Some(sneaky_offset) = host.ai_sneaky_targeting_offset(v) {
            vpos.x += sneaky_offset.x;
            vpos.y += sneaky_offset.y;
            vpos.z += sneaky_offset.z;

            // for a sneaky offset, we always target a position rather than an object
            victim = None;
            victim_id = INVALID_ID;
            dist_sqr = host.distance_squared_to_pos(source, &vpos, ATTACK_RANGE_CALC_TYPE);
        } else if host.is_kind_of(v, KINDOF_BRIDGE) {
            // bridges are kind of oddball - they have 2 target points at either end
            let (p1, p2) = host.bridge_attack_points(v);
            let mut d = host.distance_squared_to_pos(source, &p1, ATTACK_RANGE_CALC_TYPE);
            vpos = p1;
            let dist_sqr2 = host.distance_squared_to_pos(source, &p2, ATTACK_RANGE_CALC_TYPE);
            if d > dist_sqr2 {
                // try the other one
                d = dist_sqr2;
                vpos = p2;
            }
            dist_sqr = d;
        } else {
            dist_sqr = host.distance_squared_to_object(source, v, ATTACK_RANGE_CALC_TYPE);
        }
    } else {
        victim_id = INVALID_ID;
        vpos = victim_pos.ok_or(WeaponError::NullObject)?;
        dist_sqr = host.distance_squared_to_pos(source, &vpos, ATTACK_RANGE_CALC_TYPE);
    }

    // only perform this check if the weapon isn't a leech range weapon (which can have unlimited range!)
    if !ignore_ranges && !tpl(host.store(), template).leech_range_weapon {
        let attack_range_sqr = sqr(tpl(host.store(), template).get_attack_range(bonus));
        if dist_sqr > attack_range_sqr {
            return Ok(0);
        }
    }

    if !ignore_ranges {
        let min_attack_range_sqr = sqr(tpl(host.store(), template).get_minimum_attack_range());
        if dist_sqr < min_attack_range_sqr && !is_projectile_detonation {
            return Ok(0);
        }
    }

    // call this even if FXList is null, because this also handles stuff like Gun Barrel Recoil
    if host.has_drawable(source) {
        let target_pos = match victim {
            Some(v) => host.geometry(v).get_center_position(&host.position(v)),
            None => vpos,
        };
        let t = tpl(host.store(), template);
        let re_angle = t.weapon_recoil;
        let re_dir = if re_angle != 0.0 {
            atan2_d(vpos.y - source_pos.y, vpos.x - source_pos.x)
        } else {
            0.0
        };
        let v = host.veterancy_level(source) as usize;
        let mut fx = if is_projectile_detonation {
            t.projectile_detonate_fxs[v].clone()
        } else {
            t.fire_fxs[v].clone()
        };

        if host.frame() < firing_weapon.suspend_fx_frame() {
            fx = None;
        }

        let hidden = !host.is_locally_controlled(source) // if user watching is not controller and
            && host.test_status(source, OBJECT_STATUS_STEALTHED) // if unit is stealthed (like a Pathfinder)
            && !host.test_status(source, OBJECT_STATUS_DETECTED) // but not detected...
            && !host.test_status(source, OBJECT_STATUS_DISGUISED) // and not disguised...
            && !host.is_kind_of(source, KINDOF_MINE) // and not a mine (which always do the FX, even if hidden)...
            && !t.play_fx_when_stealthed; // and not a weapon marked to playwhenstealthed
        if !hidden {
            let radius = t.get_primary_damage_radius(bonus);
            let contact_weapon = t.is_contact_weapon();
            let speed = t.weapon_speed;
            host.weapon_fx(WeaponFx::Fire {
                source,
                weapon_slot: wslot,
                barrel: specific_barrel_to_use,
                fx,
                speed,
                recoil_amount: re_angle,
                recoil_dir: re_dir,
                target: target_pos,
                radius,
                contact_weapon,
            });
        }
    }

    // now do the FireOCL if there is one
    {
        let v = host.veterancy_level(source) as usize;
        let t = tpl(host.store(), template);
        let ocl_to_use = if is_projectile_detonation {
            t.projectile_detonation_ocls[v]
        } else {
            t.fire_ocls[v]
        };
        if let Some(ocl) = ocl_to_use {
            host.create_ocl(ocl, source);
        }
    }

    let mut projectile_destination = vpos; // need to copy, as this pointer is to their actual position
    let mut scatter_radius = 0.0f32;
    let (t_scatter_radius, t_infantry_inaccuracy_dist) = {
        let t = tpl(host.store(), template);
        (t.scatter_radius, t.infantry_inaccuracy_dist)
    };
    if t_scatter_radius > 0.0
        || (t_infantry_inaccuracy_dist > 0.0
            && victim.is_some()
            && host.is_kind_of(victim.unwrap_or(INVALID_ID), KINDOF_INFANTRY))
    {
        // this weapon scatters, so clear the victimObj, as we are no longer shooting it directly, and find a random
        // point within the radius to shoot at as victimPos
        scatter_radius = t_scatter_radius;

        // if it's an object, aim at the center, not the ground part (srj)
        let mut target_layer: PathfindLayer = LAYER_GROUND;
        if let Some(v) = victim {
            if host.is_kind_of(v, KINDOF_STRUCTURE) {
                projectile_destination = host.geometry(v).get_center_position(&host.position(v));
            }
            if t_infantry_inaccuracy_dist > 0.0 && host.is_kind_of(v, KINDOF_INFANTRY) {
                // if we are firing a weapon that is considered inaccurate against infantry, then add it to the
                // scatter radius!
                scatter_radius += t_infantry_inaccuracy_dist;
            }
            target_layer = host.layer(v);
        }
        // victimObj = NULL; // his position is already in victimPos, if he existed

        // randomize the scatter radius (sometimes it can be more accurate than others)
        scatter_radius = host.random_real(0.0, scatter_radius);
        let scatter_angle_radian = host.random_real(0.0, 2.0 * PI);

        let mut firing_offset = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        firing_offset.zero();
        firing_offset.x = scatter_radius * trig::cos(scatter_angle_radian);
        firing_offset.y = scatter_radius * trig::sin(scatter_angle_radian);

        projectile_destination.x += firing_offset.x;
        projectile_destination.y += firing_offset.y;

        // what's suddenly become crucially important is to FIRE at the ground at this location!!! If we aim for the
        // center point of our target and miss, the shot will go much farther than we expect! srj sez: we should
        // actually fire at the layer the victim is on, if possible, in case it is on a bridge...
        projectile_destination.z = host.layer_height(
            projectile_destination.x,
            projectile_destination.y,
            target_layer,
        );
    }

    let (has_projectile, damage_dealt_at_self_position, weapon_speed) = {
        let t = tpl(host.store(), template);
        (
            t.projectile_tmpl,
            t.damage_dealt_at_self_position,
            t.weapon_speed,
        )
    };
    match has_projectile {
        Some(projectile_template) if !is_projectile_detonation => {
            // must be a projectile
            fire_projectile(
                host,
                template,
                projectile_template,
                source,
                wslot,
                specific_barrel_to_use,
                victim,
                vpos,
                projectile_destination,
                scatter_radius,
                firing_weapon,
                projectile_id,
            )?;
            Ok(0)
        }
        _ => {
            // see if we need to be called back at a later point to deal the damage
            let v = Coord3D {
                x: vpos.x - source_pos.x,
                y: vpos.y - source_pos.y,
                z: vpos.z - source_pos.z,
            };
            // don't round the result; we WANT a fractional-frame-delay in this case
            let delay_in_frames = v.length() / weapon_speed;

            let mut damage_id = if damage_dealt_at_self_position {
                INVALID_ID
            } else {
                victim_id
            };

            if firing_weapon.is_laser(host.store()) {
                let (primary_radius, secondary_radius) = {
                    let t = tpl(host.store(), template);
                    (
                        t.get_primary_damage_radius(bonus),
                        t.get_secondary_damage_radius(bonus),
                    )
                };
                if scatter_radius <= primary_radius || scatter_radius <= secondary_radius {
                    // the laser is close enough to damage the object, so just hit it directly; some victim objects
                    // adjust the laser's position to prevent it from hitting the ground
                    if let Some(vo) = victim {
                        projectile_destination = host.position(vo);
                    }
                    firing_weapon.create_laser(host, source, victim, &projectile_destination);
                } else {
                    // we are missing our intended target, so now we want to aim at the ground at the projectile offset
                    damage_id = INVALID_ID;
                    firing_weapon.create_laser(host, source, None, &projectile_destination);
                }
                if inflict_damage {
                    deal_damage_internal(
                        host,
                        template,
                        source_id,
                        damage_id,
                        projectile_destination,
                        bonus,
                        is_projectile_detonation,
                    )?;
                }
                return Ok(host.frame());
            }

            let damage_pos = if damage_dealt_at_self_position {
                source_pos
            } else {
                vpos
            };
            if delay_in_frames < 1.0 {
                // go ahead and do it now
                if inflict_damage {
                    deal_damage_internal(
                        host,
                        template,
                        source_id,
                        damage_id,
                        damage_pos,
                        bonus,
                        is_projectile_detonation,
                    )?;
                }
                Ok(host.frame())
            } else {
                let mut when: u32 = 0;
                if inflict_damage {
                    let delay_in_whole_frames = real_to_int_ceil(delay_in_frames) as u32;
                    when = host.frame().wrapping_add(delay_in_whole_frames);
                    let key = tpl(host.store(), template).name_key;
                    host.store_mut().set_delayed_damage(
                        key,
                        &damage_pos,
                        when,
                        source_id,
                        damage_id,
                        bonus,
                    );
                }
                Ok(when)
            }
        }
    }
}

/// The projectile half of `fireWeaponTemplate` (the `else // must be a projectile` branch).
#[allow(clippy::too_many_arguments)]
fn fire_projectile<H: WeaponHost>(
    host: &mut H,
    template: usize,
    projectile_template: u16,
    source: ObjectId,
    wslot: i32,
    specific_barrel_to_use: i32,
    victim: Option<ObjectId>,
    victim_pos: Coord3D,
    projectile_destination: Coord3D,
    scatter_radius: f32,
    firing_weapon: &mut Weapon,
    projectile_id: Option<&mut ObjectId>,
) -> Result<(), WeaponError> {
    // need to know so missiles don't collide with firer
    let projectile = host.new_object_for_player_of(projectile_template, source);
    host.set_producer(projectile, source);

    // if the player has battle plans (America Strategy Center), then apply those bonuses to this object if
    // applicable; internally it validates certain kinds of objects. When projectiles are created, weapon bonuses
    // such as damage may get applied.
    if host.controlling_player_battle_plans(source) > 0 {
        host.apply_battle_plan_bonuses(source, projectile);
    }

    // store the project ID in the object as the last projectile fired!
    if let Some(p) = projectile_id {
        *p = projectile;
    }

    // notify special power tracking
    if host.has_special_power_completion_die(source) {
        host.special_power_completion_die_notify_script_engine(source);
        if host.has_special_power_completion_die(projectile) {
            host.special_power_completion_die_set_creator(projectile, INVALID_ID);
        }
    } else if host.has_special_power_completion_die(projectile) {
        host.special_power_completion_die_set_creator(projectile, source);
    }

    // the actual logic weapon needs to know this was created
    firing_weapon.new_projectile_fired(host, source, projectile, victim, Some(victim_pos))?;

    if host.has_projectile_update_interface(projectile) {
        let v = host.veterancy_level(source) as usize;
        let exhaust = tpl(host.store(), template).projectile_exhausts[v].clone();
        if scatter_radius > 0.0 {
            // with a scatter radius, don't follow the victim (overriding the intent)
            host.projectile_launch(
                projectile,
                None,
                &projectile_destination,
                source,
                wslot,
                specific_barrel_to_use,
                template,
                &exhaust,
            );
        } else {
            host.projectile_launch(
                projectile,
                victim,
                &projectile_destination,
                source,
                wslot,
                specific_barrel_to_use,
                template,
                &exhaust,
            );
        }
    } else {
        // actually, this is ok, for things like Firestorm.... (srj)
        host.set_position(projectile, &projectile_destination);
    }

    // if we're launching a missile at a unit with valid countermeasures, then communicate it
    if host.is_kind_of(projectile, KINDOF_SMALL_MISSILE) {
        if let Some(v) = victim {
            if host.has_countermeasures(v) {
                // only allow jets not currently supersonic to launch countermeasures
                if let Some(locomotor_set) = host.ai_cur_locomotor_set_type(v) {
                    if locomotor_set != LOCOMOTORSET_SUPERSONIC {
                        // this function will determine now whether or not the fired projectile will be diverted
                        // to an available decoy flare
                        host.report_missile_for_countermeasures(v, projectile);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `WeaponTemplate::trimOldHistoricDamage()`.
pub fn trim_old_historic_damage<H: WeaponHost>(host: &mut H, template: usize) {
    let expiration_date = host.frame().wrapping_sub(host.historic_damage_limit());
    let t = &mut host.store_mut().templates_mut()[template];
    let mut drop = 0;
    while drop < t.historic_damage.len() {
        if t.historic_damage[drop].frame <= expiration_date {
            drop += 1;
            continue;
        } else {
            // since they are in strict chronological order, stop as soon as we get to a nonexpired one
            break;
        }
    }
    t.historic_damage.drain(..drop);
}

/// `is2DDistSquaredLessThan(a, b, distSqr)`.
fn is_2d_dist_squared_less_than(a: &Coord3D, b: &Coord3D, dist_sqr: f32) -> bool {
    let da = sqr(a.x - b.x) + sqr(a.y - b.y);
    da <= dist_sqr
}

/// `WeaponTemplate::dealDamageInternal(sourceID, victimID, pos, bonus, isProjectileDetonation)`.
pub fn deal_damage_internal<H: WeaponHost>(
    host: &mut H,
    template: usize,
    source_id: ObjectId,
    victim_id: ObjectId,
    pos: Coord3D,
    bonus: &WeaponBonus,
    is_projectile_detonation: bool,
) -> WResult<()> {
    if source_id == 0 {
        return Ok(()); // must have a source
    }

    // (victimID == 0 && pos == NULL cannot be asked of a position that is always given)

    let source: Option<ObjectId> = if host.object_exists(source_id) {
        Some(source_id)
    } else {
        None
    }; // might be null...
    let mut pos = pos;

    // @todo We need to rewrite the historic stuff ... (CBD)

    trim_old_historic_damage(host, template);

    let (
        historic_bonus_count,
        historic_bonus_weapon,
        name_key,
        historic_bonus_radius,
        historic_bonus_time,
    ) = {
        let t = tpl(host.store(), template);
        (
            t.historic_bonus_count,
            t.historic_bonus_weapon,
            t.name_key,
            t.historic_bonus_radius,
            t.historic_bonus_time,
        )
    };
    if historic_bonus_count > 0 && historic_bonus_weapon != Some(name_key) {
        let rad_sqr = historic_bonus_radius * historic_bonus_radius;
        let mut count = 0;
        let frame_now = host.frame();
        let oldest_that_will_count = frame_now.wrapping_sub(historic_bonus_time); // anything before this frame is "more than two seconds ago" eg
        for it in &tpl(host.store(), template).historic_damage {
            if it.frame >= oldest_that_will_count
                && is_2d_dist_squared_less_than(&pos, &it.location, rad_sqr)
            {
                // this one is close enough in time and distance, so count it; this is tracked by template since it
                // applies across units, so don't try to clear historicDamage on success in here
                count += 1;
            }
        }

        if count >= historic_bonus_count - 1 {
            // minus 1 since we include ourselves implicitly
            let historic_template = historic_bonus_weapon.and_then(|key| {
                host.store()
                    .templates()
                    .iter()
                    .position(|t| t.name_key == key)
            });
            create_and_fire_temp_weapon_at_pos(host, historic_template, source, &pos)?;

            // @todo E3 hack! Clear the list for now to make sure we don't have multiple firestorms; remove this when
            // the branches merge back into one. What is causing the multiple firestorms, who is to say ... this is a
            // plug, not a fix!
            host.store_mut().templates_mut()[template]
                .historic_damage
                .clear();
        } else {
            // add AFTER checking for historic stuff
            host.store_mut().templates_mut()[template]
                .historic_damage
                .push(crate::logic::weapon::HistoricWeaponDamageInfo {
                    frame: frame_now,
                    location: pos,
                });
        }
    }

    // if there's a specific victim, use it's pos (overriding the value passed in)
    let primary_victim: Option<ObjectId> = if victim_id != 0 && host.object_exists(victim_id) {
        Some(victim_id)
    } else {
        None
    }; // might be null...
    if let Some(pv) = primary_victim {
        pos = host.position(pv);
    }

    let (
        damage_type,
        death_type,
        damage_status_type,
        has_projectile,
        primary_radius,
        secondary_radius,
        primary_damage,
        secondary_damage,
        affects,
    ) = {
        let t = tpl(host.store(), template);
        (
            t.damage_type,
            t.death_type,
            t.damage_status_type,
            t.projectile_tmpl.is_some(),
            t.get_primary_damage_radius(bonus),
            t.get_secondary_damage_radius(bonus),
            t.get_primary_damage(bonus),
            t.get_secondary_damage(bonus),
            t.affects_mask,
        )
    };
    if !has_projectile || is_projectile_detonation {
        let primary_radius_sqr = sqr(primary_radius);
        let radius = max(primary_radius, secondary_radius);
        let mut list: Vec<(ObjectId, f32)> = Vec::new();
        let have_iter = radius > 0.0;
        if have_iter {
            list = host.objects_in_range(&pos, radius, DAMAGE_RANGE_CALC_TYPE);
        } else {
            // check against victimID rather than primaryVictim, since we may have targeted a legitimate victim that
            // got killed before the damage was dealt... (srj)
            if let Some(pv) = primary_victim {
                list.push((pv, 0.0));
            }

            if affects & WEAPON_KILLS_SELF != 0 {
                let mut damage_info = DamageInfo::default();
                damage_info.input.damage_type = damage_type;
                damage_info.input.death_type = death_type;
                damage_info.input.source_id = source_id;
                damage_info.input.source_player_mask = 0;
                damage_info.input.damage_status_type = damage_status_type;
                damage_info.input.amount = HUGE_DAMAGE_AMOUNT;
                let s = source.ok_or(WeaponError::NullObject)?;
                host.attempt_damage(s, &mut damage_info);
                return Ok(());
            }
        }

        for (cur_victim, cur_victim_dist_sqr) in list {
            let mut kill_self = false;
            if let Some(src) = source {
                // anytime something is designated as the "primary victim" (ie, the direct target of the weapon), we
                // ignore all the "affects" flags
                if Some(cur_victim) != primary_victim {
                    if (affects & WEAPON_KILLS_SELF) != 0 && src == cur_victim {
                        kill_self = true;
                    } else {
                        // should object ever be allowed to damage themselves? methinks not... exception: a few
                        // weapons allow this (eg, for suicide bombers).
                        if (affects & WEAPON_AFFECTS_SELF) == 0 {
                            // remember that source is a missile for some units, and they don't want to injure them'selves' either
                            if src == cur_victim || host.producer_id(src) == cur_victim {
                                continue;
                            }
                        }

                        if affects & WEAPON_DOESNT_AFFECT_SIMILAR != 0 {
                            // this means we probably are affecting allies, but don't want to kill nearby members that
                            // are the same type as us. A good example are a group of terrorists blowing themselves up.
                            if host.is_equivalent_template(src, cur_victim)
                                && host.relationship(src, cur_victim) == Relationship::Allies
                            {
                                continue;
                            }
                        }

                        if (affects & WEAPON_DOESNT_AFFECT_AIRBORNE) != 0
                            && host.is_significantly_above_terrain(cur_victim)
                        {
                            continue;
                        }

                        // the idea here is: if its our ally(/enemies), AND it's not the direct target, AND the weapon
                        // doesn't do radius-damage to allies(/enemies)... skip it
                        let r = host.relationship(cur_victim, src);
                        let required_mask = if r == Relationship::Allies {
                            WEAPON_AFFECTS_ALLIES
                        } else if r == Relationship::Enemies {
                            WEAPON_AFFECTS_ENEMIES
                        } else {
                            // r == NEUTRAL
                            crate::logic::weapon::WEAPON_AFFECTS_NEUTRALS
                        };

                        if (affects & required_mask) == 0 {
                            // skip if we aren't affected by this weapon
                            continue;
                        }
                    }
                }
            }

            let mut damage_info = DamageInfo::default();
            damage_info.input.damage_type = damage_type;
            damage_info.input.death_type = death_type;
            damage_info.input.source_id = source_id;
            damage_info.input.source_player_mask = 0;
            damage_info.input.damage_status_type = damage_status_type;

            let mut damage_direction = Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            damage_direction.zero();
            if let Some(src) = source {
                damage_direction = host.position(cur_victim);
                let sp = host.position(src);
                damage_direction.sub(&sp);
            }

            let (allowed_angle, shock_wave_amount, shock_wave_radius, shock_wave_taper_off) = {
                let t = tpl(host.store(), template);
                (
                    t.radius_damage_angle,
                    t.shock_wave_amount,
                    t.shock_wave_radius,
                    t.shock_wave_taper_off,
                )
            };
            if allowed_angle < PI {
                let Some(src) = source else { continue }; // we are directional damage, but can't figure out our direction. Just bail.

                // people can only be hit in a cone oriented as the firer is oriented
                let mut source_vector = host.transform_matrix(src).get_x_vector();
                let mut damage_vector =
                    Vector3::new(damage_direction.x, damage_direction.y, damage_direction.z);
                source_vector.normalize();
                damage_vector.normalize();

                // these are now normalized, so the dot product is actually the Cos of the angle they form; a smaller
                // Cos would mean a more obtuse angle
                if Vector3::dot_product(&source_vector, &damage_vector) < trig::cos(allowed_angle) {
                    continue; // too far to the side, can't hurt them
                }
            }

            // grab the vector between the source object causing the damage and the victim in order that we can
            // simulate a shockwave pushing objects around
            damage_info.input.shock_wave_amount = shock_wave_amount;
            if damage_info.input.shock_wave_amount > 0.0 {
                // calculate the vector of the shockwave
                let mut shock_wave_vector = damage_direction;

                // guard against zero vector. Make vector straight up if that is the case
                if fabs(shock_wave_vector.x) < WWMATH_EPSILON
                    && fabs(shock_wave_vector.y) < WWMATH_EPSILON
                    && fabs(shock_wave_vector.z) < WWMATH_EPSILON
                {
                    shock_wave_vector.z = 1.0;
                }

                // populate the damage information with the shockwave information
                damage_info.input.shock_wave_vector = shock_wave_vector;
                damage_info.input.shock_wave_radius = shock_wave_radius;
                damage_info.input.shock_wave_taper_off = shock_wave_taper_off;
            }

            if let Some(src) = source {
                if let Some(mask) = host.controlling_player_mask(src) {
                    damage_info.input.source_player_mask = mask;
                }
            }
            // note, don't bother with damage multipliers here... that's handled internally by the attemptDamage() method
            damage_info.input.amount = if cur_victim_dist_sqr <= primary_radius_sqr {
                primary_damage
            } else {
                secondary_damage
            };

            if kill_self {
                // deal enough damage to kill yourself: blindly inflicting a very high value of the intended damage type
                damage_info.input.amount = HUGE_DAMAGE_AMOUNT;
            }

            // if the damage-dealer is a projectile, designate the damage as done by its launcher, not the projectile;
            // this is much more useful for the AI...
            if let Some(src) = source {
                if host.is_kind_of(src, KINDOF_PROJECTILE) {
                    if let Some(launcher) = host.projectile_launcher_id(src) {
                        damage_info.input.source_id = launcher;
                    }
                }
            }

            host.attempt_damage(cur_victim, &mut damage_info);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// WeaponStore: the firing half
// ---------------------------------------------------------------------------------------------------------------------

/// `WeaponStore::handleProjectileDetonation(wt, source, pos, extraBonusFlags, inflictDamage)`.
pub fn handle_projectile_detonation<H: WeaponHost>(
    host: &mut H,
    template: usize,
    source: ObjectId,
    pos: &Coord3D,
    extra_bonus_flags: WeaponBonusConditionFlags,
    inflict_damage: bool,
) -> WResult<()> {
    let mut w = Weapon::new(host, template, PRIMARY_WEAPON);
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w.load_ammo_now(host, source, shared) {
        e.apply_via_host(host, source);
    }
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w.fire_projectile_detonation_weapon_at_pos(
        host,
        source,
        pos,
        extra_bonus_flags,
        inflict_damage,
        shared,
    )? {
        e.apply_via_host(host, source);
    }
    Ok(())
}

/// `WeaponStore::createAndFireTempWeapon(wt, source, pos)`. A `NULL` template does nothing; a `NULL` source is
/// dereferenced by `loadAmmoNow` (undefined upstream).
pub fn create_and_fire_temp_weapon_at_pos<H: WeaponHost>(
    host: &mut H,
    template: Option<usize>,
    source: Option<ObjectId>,
    pos: &Coord3D,
) -> WResult<()> {
    let Some(template) = template else {
        return Ok(());
    };
    let source = source.ok_or(WeaponError::NullObject)?;
    let mut w = Weapon::new(host, template, PRIMARY_WEAPON);
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w.load_ammo_now(host, source, shared) {
        e.apply_via_host(host, source);
    }
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w
        .fire_weapon_at_pos(host, source, pos, None, shared)?
        .shared
    {
        e.apply_via_host(host, source);
    }
    Ok(())
}

/// `WeaponStore::createAndFireTempWeapon(wt, source, target)`.
pub fn create_and_fire_temp_weapon<H: WeaponHost>(
    host: &mut H,
    template: Option<usize>,
    source: Option<ObjectId>,
    target: ObjectId,
) -> WResult<()> {
    let Some(template) = template else {
        return Ok(());
    };
    let source = source.ok_or(WeaponError::NullObject)?;
    let mut w = Weapon::new(host, template, PRIMARY_WEAPON);
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w.load_ammo_now(host, source, shared) {
        e.apply_via_host(host, source);
    }
    let shared = host.is_reload_time_shared(source);
    if let Some(e) = w.fire_weapon(host, source, target, None, shared)?.shared {
        e.apply_via_host(host, source);
    }
    Ok(())
}

/// `WeaponStore::update()`: deal the delayed damage that is due, in the order it was set; damage dealt may set more
/// (they join the end of the list and are visited in the same pass).
pub fn weapon_store_update<H: WeaponHost>(host: &mut H) -> WResult<()> {
    let mut i = 0;
    while i < host.store().delayed_damage().len() {
        let cur_frame = host.frame();
        let ddi = host.store().delayed_damage()[i].clone();
        if cur_frame >= ddi.delay_damage_frame {
            // we never do projectile-detonation-damage via this code path
            let is_projectile_detonation = false;
            let template = host
                .store()
                .templates()
                .iter()
                .position(|t| t.name_key == ddi.delayed_weapon);
            if let Some(template) = template {
                deal_damage_internal(
                    host,
                    template,
                    ddi.delay_source_id,
                    ddi.delay_intended_victim_id,
                    ddi.delay_damage_pos,
                    &ddi.bonus,
                    is_projectile_detonation,
                )?;
            }
            host.store_mut().remove_delayed_damage(i);
        } else {
            i += 1;
        }
    }
    Ok(())
}
