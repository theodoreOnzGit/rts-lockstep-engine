// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Object.h  (the inline weapon-set forwarders)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//     (reloadAllAmmo, isOutOfAmmo, hasAnyWeapon, hasAnyDamageWeapon, getMostPercentReadyToFireAnyWeapon,
//      hasWeaponToDealDamageType, getLargestWeaponRange, setFiringConditionForCurrentWeapon, getCurrentWeapon,
//      findWaypointFollowingCapableWeapon, getAmmoPipShowingInfo, getAbleToAttackSpecificObject,
//      getAbleToUseWeaponAgainstTarget, chooseBestWeaponForTarget, fireCurrentWeapon, notifyFiringTrackerShotFired,
//      preFireCurrentWeapon, getLastShotFiredFrame, adjustModelConditionForWeaponStatus,
//      clearLeechRangeModeForAllWeapons)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The weapon half of `Object` (layer 8): the 25 forwarders to `m_weaponSet` and `Weapon`, the two functions
//! that keep an object's model condition in step with its weapons, and (below, behind [`ObjectBoneHost`]) the bone
//! positions and `canProduceUpgrade`, the other `Object` functions that ask the drawable or the control bar.
//!
//! # The shape
//!
//! Every function is generic over an [`ObjectWeaponHost`]: a [`WeaponHost`] that also owns the objects' weapon
//! sets and answers the few facts about an object that only `Object` has (the firing tracker, the drawable, the
//! last weapon condition). The full-game adapter (layer 12) implements it for `World` with the thing factory and
//! the weapon store; `tests/differential_weapon_fire.rs` implements it on its scenario host, so the functions run
//! against upstream's own text of the same functions.
//!
//! A function that changes the set (`choose`, `reload`, a lock) **takes the set out of its object**
//! ([`ObjectWeaponHost::take_weapon_set`]) for the call and puts it back, so that the host (the world) stays
//! borrowable while the set is `&mut`; nothing such a call asks of the host reaches the source's own set. A
//! function that **fires** takes only the weapon out of its slot ([`ObjectWeaponHost::take_weapon`]): firing
//! reaches the world, and what it reaches (a temporary weapon, `isReloadTimeShared`) uses the source's live set.
//! The first version took the whole set; the differential test found the difference at program 7910 of 30,000.
//!
//! # Verified
//!
//! `build.rs` cuts these functions out of `Object.cpp` (and the inline ones of `Object.h` are written out by hand
//! in the stand-in object) and compiles them unchanged on the weapon test's stand-ins;
//! `tests/differential_weapon_fire.rs` ops 37 to 47 run both (the unchanged text of `Object.cpp` against these functions on the
//! same scenario host). Results are in its header.

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, WEAPONSLOT_COUNT};
use crate::common::model_state::{
    ModelConditionFlags, MODELCONDITION_BETWEEN_FIRING_SHOTS_A, MODELCONDITION_BETWEEN_FIRING_SHOTS_B, MODELCONDITION_BETWEEN_FIRING_SHOTS_C,
    MODELCONDITION_FIRING_A, MODELCONDITION_FIRING_B, MODELCONDITION_FIRING_C, MODELCONDITION_PREATTACK_A, MODELCONDITION_PREATTACK_B,
    MODELCONDITION_PREATTACK_C, MODELCONDITION_RELOADING_A, MODELCONDITION_RELOADING_B, MODELCONDITION_RELOADING_C,
    MODELCONDITION_USING_WEAPON_A, MODELCONDITION_USING_WEAPON_B, MODELCONDITION_USING_WEAPON_C,
};
use crate::common::object_status_types::{OBJECT_STATUS_IS_AIMING_WEAPON, OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_IS_FIRING_WEAPON};
use crate::logic::weapon::WeaponError;
use crate::logic::weapon_fire::{Weapon, SLOTS};
use crate::logic::weapon_host::{AbleToAttackType, CanAttackResult, CommandSourceType, WeaponHost};
use crate::logic::weapon_set_state::{
    WeaponChoiceCriteria, WeaponLockType, WeaponSet, WeaponSetConditionType, LOCKED_TEMPORARILY, WSF_BETWEEN, WSF_FIRING, WSF_INVALID, WSF_NONE,
    WSF_PREATTACK, WSF_RELOADING,
};
use crate::logic::weapon_types::WeaponStatus;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON_SLOT: i32 = crate::logic::weapon_fire::PRIMARY_WEAPON;

type WResult<T> = Result<T, WeaponError>;

/// What the weapon half of `Object` needs beyond a [`WeaponHost`]. See the module docs.
pub trait ObjectWeaponHost: WeaponHost {
    /// `m_weaponSet` of `o`.
    fn weapon_set(&self, o: ObjectId) -> &WeaponSet;
    /// Takes `m_weaponSet` out of `o`, leaving an empty set in its place.
    fn take_weapon_set(&mut self, o: ObjectId) -> WeaponSet;
    /// Puts the set taken by [`take_weapon_set`](Self::take_weapon_set) back.
    fn put_weapon_set(&mut self, o: ObjectId, set: WeaponSet);
    /// `m_weaponSet` of `o`, to change.
    fn weapon_set_mut(&mut self, o: ObjectId) -> &mut WeaponSet;
    /// Takes the weapon out of slot `slot` of `o`'s set (the rest of the set stays where it is).
    fn take_weapon(&mut self, o: ObjectId, slot: i32) -> Option<Weapon>;
    /// Puts the weapon taken by [`take_weapon`](Self::take_weapon) back into its slot.
    fn put_weapon(&mut self, o: ObjectId, slot: i32, weapon: Weapon);
    /// `m_lastWeaponCondition[slot]`.
    fn last_weapon_condition(&self, o: ObjectId, slot: usize) -> i8;
    /// `m_lastWeaponCondition[slot] = v`.
    fn set_last_weapon_condition(&mut self, o: ObjectId, slot: usize, v: i8);
    /// `m_firingTracker->shotFired(weapon, victimID)`, the weapon by its slot in the object's set (the tracker reads the live weapon:
    /// batch R78); false (and nothing done) if the object has no firing tracker.
    fn firing_tracker_shot_fired(&mut self, o: ObjectId, slot: i32, victim: ObjectId) -> bool;
    /// `friend_setUndetectedDefector(undetected)`.
    fn friend_set_undetected_defector(&mut self, o: ObjectId, undetected: bool);
    /// `m_drawable->updateDrawableClipStatus(ammoInClip, maxClipSize, slot)`.
    fn drawable_update_clip_status(&mut self, o: ObjectId, ammo_in_clip: u32, max_clip_size: i32, slot: i32);
    /// `m_drawable->clearAndSetModelConditionFlags(clear, set)`.
    fn drawable_clear_and_set_model_condition_flags(&mut self, o: ObjectId, clear: &ModelConditionFlags, set: &ModelConditionFlags);
    /// `m_drawable->setAnimationLoopDuration(frames)`.
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32);
}

/// `s_allWeaponFireFlags[slot]` (`Object.cpp`): every model condition a weapon slot can set.
pub fn all_weapon_fire_flags(slot: usize) -> ModelConditionFlags {
    const FLAGS: [[i32; 5]; SLOTS] = [
        [
            MODELCONDITION_FIRING_A,
            MODELCONDITION_BETWEEN_FIRING_SHOTS_A,
            MODELCONDITION_RELOADING_A,
            MODELCONDITION_PREATTACK_A,
            MODELCONDITION_USING_WEAPON_A,
        ],
        [
            MODELCONDITION_FIRING_B,
            MODELCONDITION_BETWEEN_FIRING_SHOTS_B,
            MODELCONDITION_RELOADING_B,
            MODELCONDITION_PREATTACK_B,
            MODELCONDITION_USING_WEAPON_B,
        ],
        [
            MODELCONDITION_FIRING_C,
            MODELCONDITION_BETWEEN_FIRING_SHOTS_C,
            MODELCONDITION_RELOADING_C,
            MODELCONDITION_PREATTACK_C,
            MODELCONDITION_USING_WEAPON_C,
        ],
    ];
    let mut f = ModelConditionFlags::new();
    for &b in &FLAGS[slot] {
        f.set(b, 1);
    }
    f
}

/// Runs `f` on the set of `o` taken out of it, then puts it back (also on an error).
fn with_set<H: ObjectWeaponHost, T>(host: &mut H, o: ObjectId, f: impl FnOnce(&mut H, &mut WeaponSet) -> T) -> T {
    let mut set = host.take_weapon_set(o);
    let r = f(host, &mut set);
    host.put_weapon_set(o, set);
    r
}

/// `Object::reloadAllAmmo(now)`.
pub fn reload_all_ammo<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, now: bool) {
    with_set(host, o, |host, set| set.reload_all_ammo(host, o, now));
}

/// `Object::isOutOfAmmo()`.
pub fn is_out_of_ammo<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> bool {
    host.weapon_set(o).is_out_of_ammo(host.frame())
}

/// `Object::hasAnyWeapon()`.
pub fn has_any_weapon<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> bool {
    host.weapon_set(o).has_any_weapon()
}

/// `Object::hasAnyDamageWeapon()`: "a should be used for real weapons that directly inflict damage... not deploy,
/// hack, etc." (the first check of upstream is the whole answer).
pub fn has_any_damage_weapon<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> bool {
    // First check to see if we have any weapons -- if not return false.
    host.weapon_set(o).has_any_damage_weapon()
}

/// `Object::getMostPercentReadyToFireAnyWeapon()`.
pub fn get_most_percent_ready_to_fire_any_weapon<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> u32 {
    host.weapon_set(o).get_most_percent_ready_to_fire_any_weapon(host.frame())
}

/// `Object::hasWeaponToDealDamageType(typeToDeal)`.
pub fn has_weapon_to_deal_damage_type<H: ObjectWeaponHost>(host: &H, o: ObjectId, type_to_deal: i32) -> bool {
    host.weapon_set(o).has_weapon_to_deal_damage_type(type_to_deal)
}

/// `Object::getLargestWeaponRange()`.
pub fn get_largest_weapon_range<H: ObjectWeaponHost>(host: &mut H, o: ObjectId) -> f32 {
    let mut ret_val = -1.0f32;
    for i in 0..WEAPONSLOT_COUNT {
        // (a copy, put back after: the range query may need the host mutably, and the weapon's status is a cell the query may update)
        let Some(weapon) = host.weapon_set(o).get_weapon_in_weapon_slot(i).cloned() else { continue };
        let tmp_val = weapon.get_attack_range(host, o);
        host.put_weapon(o, i, weapon);
        if tmp_val > ret_val {
            ret_val = tmp_val;
        }
    }
    ret_val
}

/// `Object::setFiringConditionForCurrentWeapon()`.
pub fn set_firing_condition_for_current_weapon<H: ObjectWeaponHost>(host: &mut H, o: ObjectId) {
    if host.has_drawable(o) {
        let wslot = host.weapon_set(o).get_cur_weapon_slot();
        let c = WeaponSet::get_model_condition_for_weapon_slot(wslot, WSF_FIRING);
        host.drawable_clear_and_set_model_condition_flags(o, &all_weapon_fire_flags(wslot as usize), &c);
    }
}

/// What `Object::getCurrentWeapon(wslot)` answers: the weapon (as whether there is one in the current slot) and the
/// slot written through `wslot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentWeapon {
    /// `*wslot = getCurWeaponSlot()`, written only when the object has any weapon.
    pub slot: Option<i32>,
    /// The returned `Weapon*` is not `NULL`: the weapon in the current slot (the set may have weapons in other
    /// slots only).
    pub has_weapon: bool,
}

/// `Object::getCurrentWeapon(wslot)` (both the const and the non-const).
pub fn get_current_weapon<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> CurrentWeapon {
    let set = host.weapon_set(o);
    if !set.has_any_weapon() {
        return CurrentWeapon { slot: None, has_weapon: false };
    }
    CurrentWeapon { slot: Some(set.get_cur_weapon_slot()), has_weapon: set.get_cur_weapon().is_some() }
}

/// `Object::findWaypointFollowingCapableWeapon()`: the slot of the weapon.
pub fn find_waypoint_following_capable_weapon<H: ObjectWeaponHost>(host: &mut H, o: ObjectId) -> Option<i32> {
    with_set(host, o, |host, set| set.find_waypoint_following_capable_weapon(host).map(|w| w.weapon_slot()))
}

/// `Object::getAmmoPipShowingInfo(numTotal, numFull)`.
pub fn get_ammo_pip_showing_info<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> Option<(i32, i32)> {
    // @todo srj -- may need to cache this inside weaponset.
    let w = host.weapon_set(o).find_ammo_pip_showing_weapon(host)?;
    Some((host.store().templates()[w.template()].clip_size, w.remaining_ammo(host.frame()) as i32))
}

/// `Object::getAbleToAttackSpecificObject(t, target, commandSource, specificSlot)`: "NO LONGER internally calls
/// isAbleToAttack(), since that isn't an incredibly fast call".
pub fn get_able_to_attack_specific_object<H: ObjectWeaponHost>(
    host: &mut H,
    o: ObjectId,
    t: AbleToAttackType,
    target: ObjectId,
    command_source: CommandSourceType,
    specific_slot: i32,
) -> WResult<CanAttackResult> {
    // (a copy of the set, put back after: the question may need the host mutably, and the weapons' status cells may be updated)
    let set = host.weapon_set(o).clone();
    let r = set.get_able_to_attack_specific_object(host, t, o, target, command_source, specific_slot);
    *host.weapon_set_mut(o) = set;
    r
}

/// `Object::getAbleToUseWeaponAgainstTarget(attackType, victim, pos, commandSource, specificSlot)`.
pub fn get_able_to_use_weapon_against_target<H: ObjectWeaponHost>(
    host: &mut H,
    o: ObjectId,
    attack_type: AbleToAttackType,
    victim: Option<ObjectId>,
    pos: Option<crate::common::base_type::Coord3D>,
    command_source: CommandSourceType,
    specific_slot: i32,
) -> WResult<CanAttackResult> {
    // (a copy of the set, put back after: the question may need the host mutably, and the weapons' status cells may be updated)
    let set = host.weapon_set(o).clone();
    let r = set.get_able_to_use_weapon_against_target(host, attack_type, o, victim, pos, command_source, specific_slot);
    *host.weapon_set_mut(o) = set;
    r
}

/// `Object::chooseBestWeaponForTarget(target, criteria, cmdSource)`.
pub fn choose_best_weapon_for_target<H: ObjectWeaponHost>(
    host: &mut H,
    o: ObjectId,
    target: Option<ObjectId>,
    criteria: WeaponChoiceCriteria,
    cmd_source: CommandSourceType,
) -> WResult<bool> {
    with_set(host, o, |host, set| set.choose_best_weapon_for_target(host, o, target, criteria, cmd_source))
}

/// What `fireCurrentWeapon` fires at: an object (`None` is "victim may have already been destroyed") or a position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FireAt {
    /// `fireCurrentWeapon(Object* target)`.
    Object(Option<ObjectId>),
    /// `fireCurrentWeapon(const Coord3D* pos)`.
    Position(Option<crate::common::base_type::Coord3D>),
}

/// `Object::fireCurrentWeapon(target)` and `fireCurrentWeapon(pos)`.
///
/// The weapon is taken out of its slot while it fires, not the whole set: firing reaches the world (damage, a
/// temporary weapon for the historic bonus) and what it reaches asks the source's set whether its reload time is
/// shared, and sets the other weapons' reload (`apply_shared_reload`), which must find the live set.
pub fn fire_current_weapon<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, at: FireAt) -> WResult<()> {
    // victim may have already been destroyed
    let (target, pos) = match at {
        FireAt::Object(None) | FireAt::Position(None) => return Ok(()),
        FireAt::Object(Some(t)) => (Some(t), None),
        FireAt::Position(Some(p)) => (None, Some(p)),
    };
    let now = host.frame();
    let shared = host.weapon_set(o).is_shared_reload_time(host);
    let slot = host.weapon_set(o).get_cur_weapon_slot();
    let Some(mut weapon) = host.take_weapon(o, slot) else { return Ok(()) };
    if weapon.get_status(now) != WeaponStatus::ReadyToFire {
        host.put_weapon(o, slot, weapon);
        return Ok(());
    }
    let fired = match (target, pos) {
        (Some(t), _) => weapon.fire_weapon(host, o, t, None, shared),
        (None, Some(p)) => weapon.fire_weapon_at_pos(host, o, &p, None, shared),
        (None, None) => unreachable!("one of the two is given"),
    };
    let outcome = match fired {
        Ok(outcome) => outcome,
        Err(e) => {
            host.put_weapon(o, slot, weapon);
            return Err(e);
        }
    };
    host.put_weapon(o, slot, weapon);
    // the siblings' shared reload is the last thing `fireWeapon` does
    host.weapon_set_mut(o).apply_shared_reload(outcome.shared);
    // (batch R78: the tracker is told after `fireWeapon` returns, with the weapon back in its set, as upstream)
    host.firing_tracker_shot_fired(o, slot, target.unwrap_or(crate::common::game_type::INVALID_ID));
    if outcome.reloaded {
        host.weapon_set_mut(o).release_weapon_lock(LOCKED_TEMPORARILY); // release any temporary locks.
    }
    host.friend_set_undetected_defector(o, false); // My secret is out
    Ok(())
}

/// `Object::notifyFiringTrackerShotFired(weaponFired, victimID)`.
pub fn notify_firing_tracker_shot_fired<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, slot: i32, victim_id: ObjectId) {
    host.firing_tracker_shot_fired(o, slot, victim_id);
}

/// `Object::preFireCurrentWeapon(victim)`: "if we are going to be capable of firing our weapon NEXT frame, set
/// the pre-attack up now".
pub fn pre_fire_current_weapon<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, victim: Option<ObjectId>) -> WResult<()> {
    let now = host.frame();
    let slot = host.weapon_set(o).get_cur_weapon_slot();
    let Some(mut weapon) = host.take_weapon(o, slot) else { return Ok(()) };
    let fire = now.wrapping_add(1) >= weapon.possible_next_shot_frame();
    let result = if fire { weapon.pre_fire_weapon(host, o, victim) } else { Ok(()) };
    host.put_weapon(o, slot, weapon);
    result?;
    if fire {
        host.friend_set_undetected_defector(o, false); // My secret is out
    }
    Ok(())
}

/// `Object::getLastShotFiredFrame()`: "using the firing tracker, return the frame a shot was last fired on".
pub fn get_last_shot_fired_frame<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> u32 {
    let mut recent = 0;
    for i in 0..WEAPONSLOT_COUNT {
        if let Some(w) = host.weapon_set(o).get_weapon_in_weapon_slot(i) {
            let when = w.last_shot_frame();
            if when > recent {
                recent = when;
            }
        }
    }
    recent
}

/// `Object::setWeaponLock(weaponSlot, lockType)`.
pub fn set_weapon_lock<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, weapon_slot: i32, lock_type: WeaponLockType) -> bool {
    with_set(host, o, |_, set| set.set_weapon_lock(weapon_slot, lock_type))
}

/// `Object::releaseWeaponLock(lockType)`.
pub fn release_weapon_lock<H: ObjectWeaponHost>(host: &mut H, o: ObjectId, lock_type: WeaponLockType) {
    with_set(host, o, |_, set| set.release_weapon_lock(lock_type));
}

/// `Object::isCurWeaponLocked()`.
pub fn is_cur_weapon_locked<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> bool {
    host.weapon_set(o).is_cur_weapon_locked()
}

/// `Object::getWeaponInWeaponSlotCommandSourceMask(wSlot)`.
pub fn get_weapon_in_weapon_slot_command_source_mask<H: ObjectWeaponHost>(host: &H, o: ObjectId, w_slot: usize) -> u32 {
    host.weapon_set(o).get_nth_command_source_mask(host, w_slot)
}

/// `Object::isReloadTimeShared()`.
pub fn is_reload_time_shared<H: ObjectWeaponHost>(host: &H, o: ObjectId) -> bool {
    host.weapon_set(o).is_shared_reload_time(host)
}

/// `Object::clearLeechRangeModeForAllWeapons()`.
pub fn clear_leech_range_mode_for_all_weapons<H: ObjectWeaponHost>(host: &mut H, o: ObjectId) {
    with_set(host, o, |_, set| set.clear_leech_range_mode_for_all_weapons());
}

/// `Object::adjustModelConditionForWeaponStatus()`: "a weapon cannot be in charge of maintaining condition flags as
/// it is all event driven. I will maintain my ModelCondition myself if it should change. Firing is set by firing
/// logic, so I don't include it here. It is only the states that expire on timers that noone watches that I am
/// concerned with."
pub fn adjust_model_condition_for_weapon_status<H: ObjectWeaponHost>(host: &mut H, o: ObjectId) {
    let now = host.frame();

    for i in 0..WEAPONSLOT_COUNT {
        // (the facts of the weapon, read while the set is in the host)
        let Some((last_shot_frame, status, remaining_ammo, clip_size, weapon_slot, pre_attack_done)) =
            host.weapon_set(o).get_weapon_in_weapon_slot(i).map(|w| {
                (
                    w.last_shot_frame(),
                    w.get_status(now),
                    w.remaining_ammo(now),
                    host.store().templates()[w.template()].clip_size,
                    w.weapon_slot(),
                    w.pre_attack_finished_frame(),
                )
            })
        else {
            host.set_last_weapon_condition(o, i as usize, WSF_NONE as i8);
            continue;
        };

        // (upstream starts at WSF_INVALID; every branch below assigns)
        let mut condition_to_set: WeaponSetConditionType;
        if i != host.weapon_set(o).get_cur_weapon_slot() {
            // if this isn't the current weapon, then we never set ANYTHING for it.
            condition_to_set = WSF_NONE;
        } else if last_shot_frame == now {
            // yep, this overrides any weapon-status condition!
            condition_to_set = WSF_FIRING;
        } else if !host.test_status(o, OBJECT_STATUS_IS_ATTACKING) {
            // srj sez: not 100% sure about this one, but the problem is: say we were attacking, then issue a move
            // command. if we didn't do this here, we might still have a 'firing' pose, because his weapon might
            // be in 'reloading' mode. since we're not attacking, however, we really don't care, so we just force
            // the issue here. (This might still need tweaking for the pursue state.)
            condition_to_set = WSF_NONE;
        } else {
            let new_status = status;

            const S_WSF_LOOKUP: [WeaponSetConditionType; 5] = [
                WSF_NONE,      // READY_TO_FIRE,
                WSF_NONE,      // OUT_OF_AMMO,
                WSF_BETWEEN,   // BETWEEN_FIRING_SHOTS,
                WSF_RELOADING, // RELOADING_CLIP,
                WSF_PREATTACK, // PRE_ATTACK,
            ];
            condition_to_set = S_WSF_LOOKUP[new_status as usize];

            // special case this: say we are firing in bursts: pow-pow-pow-pause, etc. then we might have a frame
            // where we have reloaded and are ready-to-fire, but haven't fired yet this frame. in that case, use
            // 'between' so we still have a firing pose, 'cuz if we use 'none' we will 'pop' back to idle for a
            // frame. (srj) additional note: only do if aiming or firing, since we could also be in this state if
            // we are approaching or pursuing a target! (srj)
            if new_status == WeaponStatus::ReadyToFire
                && condition_to_set == WSF_NONE
                && host.test_status(o, OBJECT_STATUS_IS_ATTACKING)
                && (host.test_status(o, OBJECT_STATUS_IS_AIMING_WEAPON) || host.test_status(o, OBJECT_STATUS_IS_FIRING_WEAPON))
            {
                condition_to_set = WSF_BETWEEN;
            }
        }

        if host.has_drawable(o) {
            host.drawable_update_clip_status(o, remaining_ammo, clip_size, weapon_slot);
            if condition_to_set != WSF_INVALID && condition_to_set != host.last_weapon_condition(o, i as usize) as i32 {
                host.set_last_weapon_condition(o, i as usize, condition_to_set as i8);
                let c = WeaponSet::get_model_condition_for_weapon_slot(i, condition_to_set);
                host.drawable_clear_and_set_model_condition_flags(o, &all_weapon_fire_flags(i as usize), &c);
                if condition_to_set == WSF_PREATTACK {
                    // in the preattack state, adjust the speed of the preattack anim to match the actual time it will take
                    let pre_attack_done: u32 = pre_attack_done;
                    if pre_attack_done > now {
                        host.drawable_set_animation_loop_duration(o, pre_attack_done - now);
                    }
                }
            }
        }
    }
}

// ---- bone positions and canProduceUpgrade -----------------------------------------------------------------------------

/// What the bone-position functions and `canProduceUpgrade` ask of the drawable, the AI and the control bar.
pub trait ObjectBoneHost: WeaponHost {
    /// `m_drawable->getPristineBonePositions(prefix, startIndex, ..., maxBones)`: the bones found (at most
    /// `max_bones`), each as its position and transform. Only asked of an object with a drawable.
    fn pristine_bone_positions(&self, o: ObjectId, bone_name_prefix: &[u8], start_index: i32, max_bones: i32) -> Vec<(Coord3D, Matrix3D)>;
    /// `convertBonePosToWorldPos(bonePos, boneTransform, worldPos, worldTransform)` of the drawable and of `Thing`:
    /// a world value is made for each of the inputs that is given (an output pointer upstream is the input's).
    fn convert_bone_pos_to_world_pos(&self, o: ObjectId, bone_pos: Option<&Coord3D>, bone_transform: Option<&Matrix3D>) -> (Option<Coord3D>, Option<Matrix3D>);
    /// `getAI() != NULL`.
    fn has_ai(&self, o: ObjectId) -> bool;
    /// `getAI()->getTurretRotAndPitch(turret, &rot, &pitch)`: `None` when it returns false (and writes neither).
    fn ai_turret_rot_and_pitch(&self, o: ObjectId, turret: i32) -> Option<(f32, f32)>;
    /// `TheControlBar->findCommandSet(name)`: its buttons' upgrade templates (by id; `None` for no button or a
    /// button that is not an upgrade), `None` when there is no such set.
    fn command_set_upgrades(&self, name: &[u8]) -> Option<Vec<Option<u32>>>;
    /// `m_commandSetStringOverride`.
    fn command_set_string_override(&self, o: ObjectId) -> Vec<u8>;
    /// `getTemplate()->friend_getCommandSetString()`.
    fn template_command_set_string(&self, o: ObjectId) -> Vec<u8>;
}

/// What `Object::getSingleLogicalBonePosition` and `...OnTurret` give back through their pointers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneQuery {
    /// The returned `Bool`.
    pub found: bool,
    /// `*position`, when the pointer was given and written.
    pub position: Option<Coord3D>,
    /// `*transform`, when the pointer was given and written.
    pub transform: Option<Matrix3D>,
}

/// `Object::getSingleLogicalBonePosition(boneName, position, transform)`; `want_position` and `want_transform` say
/// which pointers are non-null.
pub fn get_single_logical_bone_position<H: ObjectBoneHost>(host: &H, o: ObjectId, bone_name: &[u8], want_position: bool, want_transform: bool) -> BoneQuery {
    if host.has_drawable(o) {
        let bones = host.pristine_bone_positions(o, bone_name, 0, 1);
        if bones.len() == 1 {
            let (p, t) = bones[0];
            let pos = want_position.then_some(p);
            let tr = want_transform.then_some(t);
            let (wp, wt) = host.convert_bone_pos_to_world_pos(o, pos.as_ref(), tr.as_ref());
            return BoneQuery { found: true, position: wp, transform: wt };
        }
    }
    BoneQuery {
        found: false,
        position: want_position.then(|| host.position(o)),
        transform: want_transform.then(|| host.transform_matrix(o)),
    }
}

/// `Object::getSingleLogicalBonePositionOnTurret(whichTurret, boneName, position, transform)`. Upstream reads the
/// turret position and rotation it asked for even when the call that fills them returned false: that read of an
/// uninitialised value is [`WeaponError::UninitialisedRead`].
pub fn get_single_logical_bone_position_on_turret<H: ObjectBoneHost>(
    host: &mut H,
    o: ObjectId,
    which_turret: i32,
    bone_name: &[u8],
    want_position: bool,
    want_transform: bool,
) -> WResult<BoneQuery> {
    let none = BoneQuery { found: false, position: None, transform: None };
    if !host.has_drawable(o) || !host.has_ai(o) {
        return Ok(none);
    }

    // We need to find the TurretBone's pristine position.
    let launch = host.projectile_launch_offset(o, PRIMARY_WEAPON_SLOT, 1, which_turret);
    // And the required bone's pristine position
    let bones = host.pristine_bone_positions(o, bone_name, 0, 1);
    if bones.len() != 1 {
        return Ok(none);
    }
    let bone_position = bones[0].0;
    let turret_position = launch.ok_or(WeaponError::UninitialisedRead)?.turret_rot_pos;
    // Then we mojo the Logic position of the required bone like Missile firing does. Using the logic twist of the turret
    let (turret_rotation, _) = host.ai_turret_rot_and_pitch(o, which_turret).ok_or(WeaponError::UninitialisedRead)?;

    let mut bone_offset = Matrix3D::identity(); // This will be from the turret to the requested bone
    bone_offset.translate_v(&Vector3::new(bone_position.x, bone_position.y, bone_position.z));

    let mut turn_adjustment = Matrix3D::identity(); // this is the turret twist to be applied to the final answer
    turn_adjustment.translate(turret_position.x, turret_position.y, turret_position.z);
    turn_adjustment.in_place_pre_rotate_z(turret_rotation);
    turn_adjustment.translate(-turret_position.x, -turret_position.y, -turret_position.z);

    let mut bone_logic_transform = Matrix3D::identity();
    bone_logic_transform.mul(&turn_adjustment, &bone_offset);

    let world_transform = host.convert_bone_pos_to_world_pos(o, None, Some(&bone_logic_transform)).1.ok_or(WeaponError::UninitialisedRead)?;

    let tmp = world_transform.get_translation();
    let world_pos = Coord3D { x: tmp.x, y: tmp.y, z: tmp.z };

    Ok(BoneQuery { found: true, position: want_position.then_some(world_pos), transform: want_transform.then_some(world_transform) })
}

/// `Object::getMultiLogicalBonePosition(prefix, maxBones, positions, transforms, convertToWorld)`: the bones found
/// (positions and transforms only where the pointers were given).
pub fn get_multi_logical_bone_position<H: ObjectBoneHost>(
    host: &H,
    o: ObjectId,
    bone_name_prefix: &[u8],
    max_bones: i32,
    want_positions: bool,
    want_transforms: bool,
    convert_to_world: bool,
) -> Vec<(Option<Coord3D>, Option<Matrix3D>)> {
    if !host.has_drawable(o) {
        return Vec::new();
    }
    let bones = host.pristine_bone_positions(o, bone_name_prefix, 1, max_bones);
    let mut out: Vec<(Option<Coord3D>, Option<Matrix3D>)> = bones.iter().map(|&(p, t)| (want_positions.then_some(p), want_transforms.then_some(t))).collect();
    if !out.is_empty() && convert_to_world {
        for e in out.iter_mut() {
            *e = host.convert_bone_pos_to_world_pos(o, e.0.as_ref(), e.1.as_ref());
        }
    }
    out
}

/// `Object::getCommandSetString()`.
pub fn get_command_set_string<H: ObjectBoneHost>(host: &H, o: ObjectId) -> Vec<u8> {
    let over = host.command_set_string_override(o);
    if !over.is_empty() {
        return over;
    }
    host.template_command_set_string(o)
}

/// `Object::canProduceUpgrade(upgrade)`: "we need to have the button to make the upgrade. CommandSets are a weird
/// Logic/Client hybrid." A name that names no command set dereferences a null `CommandSet*` upstream:
/// [`WeaponError::NullObject`].
pub fn can_produce_upgrade<H: ObjectBoneHost>(host: &H, o: ObjectId, upgrade: u32) -> WResult<bool> {
    let name = get_command_set_string(host, o);
    let set = host.command_set_upgrades(&name).ok_or(WeaponError::NullObject)?;
    for button_upgrade in set {
        if button_upgrade == Some(upgrade) {
            return Ok(true); // getUpgradeTemplate only returns something if it is actually an upgrade
        }
    }
    Ok(false) // Cheatin' punk.
}
