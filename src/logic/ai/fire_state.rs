// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (outOfWeaponRangeObject, 1103-1157; AIAttackFireWeaponState,
//     5161-5340)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (NotifyWeaponFiredInterface, AIAttackFireWeaponState)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIAttackFireWeaponState` and the `outOfWeaponRangeObject` condition, which the turret's machine and the attack machine share
//! (`p1w13-sonnet`, layer 10, batch T1).
//!
//! - **The state** fires the owner's current weapon once: on entering, a passive mood refuses the attack (failure), a team that
//!   attacks a common target takes the victim as its target if it has none, the owner is marked firing and pre-fires; each update,
//!   a dead victim (when attacking an object), no weapon, a weapon not ready (waiting while it pre-attacks), or a slot the attacker
//!   will not fire from fails; else the firing condition is set and the weapon fires at the victim (then the goal follows the AI's
//!   victim, and a weapon with a continue range picks the closest attackable object of the dead victim's player around the original
//!   victim's position) or at the goal position (every slot's weapon for linked turrets), and the attacker is told. On exit the
//!   firing flags clear and a weapon still pre-attacking is cancelled.
//! - **The condition**: out of range when the owner's current weapon cannot reach the goal object, or, for a non-contact weapon of
//!   an owner on the ground against a victim not well above it, the view is blocked (a weapon with leech range is never out).
//!
//! The attacker (`NotifyWeaponFiredInterface`) is the machine's state type ([`FireAttacker`]); what the state asks of the owner,
//! its weapons and the world is [`FireHost`].
//!
//! # Upstream facts kept
//!
//! - `onEnter` reads the owner's AI unchecked (its mood adjustment), and `update` reads it unchecked after firing at an object.
//! - The linked turrets' branch fires each slot's weapon at the goal position and notifies the firing tracker even when a weapon
//!   did not reload.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{CommandSourceType, ATTACK_NEW_TARGET};
use crate::common::game_type::{ObjectId, WeaponSlotType, INVALID_ID};
use crate::common::kind_of::{KindOfType, KINDOF_IMMOBILE, KINDOF_MINE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE};
use crate::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_IGNORING_STEALTH, OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_MASKED,
};
use crate::common::state_machine::{SmResult, StateBehavior, StateContext, StateExitType, StateMachine, StateReturn};
use crate::logic::ai::ai_update_types::{MAA_ACTION_OK, MM_ACTION_ATTACK};
use crate::logic::modules::plant;
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::ids::PlayerIndex;

/// The weapon slots in order (`PRIMARY_WEAPON` .. `WEAPONSLOT_COUNT`).
pub const SLOTS: [WeaponSlotType; 3] = [WeaponSlotType::Primary, WeaponSlotType::Secondary, WeaponSlotType::Tertiary];

/// What the fire state and the range condition ask of the machine's owner (the context's [`FireHost::owner`]), its AI, its weapons
/// and the world. Weapons are named by their object and slot.
pub trait FireHost: StateContext {
    /// The machine's owner (`getMachineOwner()`).
    fn owner(&mut self) -> ObjectId;
    /// `obj->getAI()->getMoodMatrixActionAdjustment(action)` (the owner's AI, read unchecked).
    fn ai_mood_action_adjustment(&mut self, obj: ObjectId, action: i32) -> u32;
    /// `getAI()->areTurretsLinked()` (unchecked).
    fn ai_are_turrets_linked(&mut self, obj: ObjectId) -> bool;
    /// `getAI()->getCurrentVictim()` (unchecked).
    fn ai_current_victim(&mut self, obj: ObjectId) -> Option<ObjectId>;
    /// `getAI()->getLastCommandSource()`; none without an AI.
    fn ai_last_command_source(&mut self, obj: ObjectId) -> Option<CommandSourceType>;
    /// `getAI()->isDoingGroundMovement()`; none without an AI.
    fn ai_is_doing_ground_movement(&mut self, obj: ObjectId) -> Option<bool>;
    fn is_kind_of(&mut self, obj: ObjectId, kind: KindOfType) -> bool;
    fn test_status(&mut self, obj: ObjectId, bit: ObjectStatusTypes) -> bool;
    /// `setStatus(MAKE_OBJECT_STATUS_MASK..(bits), set)` / `clearStatus(..)`: one call, its bits.
    fn set_status(&mut self, obj: ObjectId, bits: &[ObjectStatusTypes], set: bool);
    fn is_effectively_dead(&mut self, obj: ObjectId) -> bool;
    fn is_destroyed(&mut self, obj: ObjectId) -> bool;
    fn contained_by(&mut self, obj: ObjectId) -> Option<ObjectId>;
    fn is_airborne_target(&mut self, obj: ObjectId) -> bool;
    fn is_significantly_above_terrain(&mut self, obj: ObjectId) -> bool;
    fn position(&mut self, obj: ObjectId) -> Coord3D;
    fn controlling_player(&mut self, obj: ObjectId) -> Option<PlayerIndex>;
    /// `getTeam()->getPrototype()->getTemplateInfo()->m_attackCommonTarget`.
    fn team_attacks_common_target(&mut self, obj: ObjectId) -> bool;
    /// `getTeam()->getTeamTargetObject()`, `setTeamTargetObject(..)`.
    fn team_target_object(&mut self, obj: ObjectId) -> Option<ObjectId>;
    fn set_team_target_object(&mut self, obj: ObjectId, target: ObjectId);
    /// `getCurrentWeapon(&slot)`: the slot; none without a weapon.
    fn current_weapon(&mut self, obj: ObjectId) -> Option<WeaponSlotType>;
    /// `getWeaponInWeaponSlot(slot) != NULL`.
    fn has_weapon_in_slot(&mut self, obj: ObjectId, slot: WeaponSlotType) -> bool;
    fn weapon_status(&mut self, obj: ObjectId, slot: WeaponSlotType) -> WeaponStatus;
    fn weapon_is_contact(&mut self, obj: ObjectId, slot: WeaponSlotType) -> bool;
    fn weapon_has_leech_range(&mut self, obj: ObjectId, slot: WeaponSlotType) -> bool;
    /// `isWithinAttackRange(obj, victim)`.
    fn weapon_within_range_of_object(&mut self, obj: ObjectId, slot: WeaponSlotType, victim: ObjectId) -> bool;
    /// `isWithinAttackRange(obj, &pos)`.
    fn weapon_within_range_of_position(&mut self, obj: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool;
    fn weapon_continue_attack_range(&mut self, obj: ObjectId, slot: WeaponSlotType) -> f32;
    fn weapon_attack_range(&mut self, obj: ObjectId, slot: WeaponSlotType) -> f32;
    fn weapon_set_pre_attack_finished_frame(&mut self, obj: ObjectId, slot: WeaponSlotType, frame: u32);
    /// `weapon->fireWeapon(obj, &pos)`: whether it reloaded.
    fn weapon_fire_at_position(&mut self, obj: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool;
    fn release_weapon_lock(&mut self, obj: ObjectId, lock: i32);
    fn notify_firing_tracker_shot_fired(&mut self, obj: ObjectId, slot: WeaponSlotType, victim: ObjectId);
    fn pre_fire_current_weapon(&mut self, obj: ObjectId, victim: Option<ObjectId>);
    fn set_firing_condition_for_current_weapon(&mut self, obj: ObjectId);
    fn fire_current_weapon_at_object(&mut self, obj: ObjectId, victim: Option<ObjectId>);
    fn fire_current_weapon_at_position(&mut self, obj: ObjectId, pos: &Coord3D);
    /// `TheAI->pathfinder()->isAttackViewBlockedByObstacle(obj, objPos, victim, victimPos)`.
    fn is_attack_view_blocked(&mut self, obj: ObjectId, obj_pos: &Coord3D, victim: ObjectId, victim_pos: &Coord3D) -> bool;
    /// `ThePartitionManager->getClosestObject(pos, range, FROM_CENTER_2D, {possible to attack as a new target by obj from
    /// command source, of player, of obj's map status})`.
    fn closest_attackable_of_player(&mut self, pos: &Coord3D, range: f32, obj: ObjectId, source: CommandSourceType, player: Option<PlayerIndex>) -> Option<ObjectId>;
}

/// `NotifyWeaponFiredInterface`: what the fire state tells and asks its attacker, implemented by the machine's state type (the
/// turret for the turret's machine, the attack state for the attack machine).
pub trait FireAttacker: StateBehavior {
    fn notify_fired(sm: &mut StateMachine<Self>, cx: &mut Self::Ctx) -> SmResult<()>;
    fn notify_new_victim_chosen(sm: &mut StateMachine<Self>, cx: &mut Self::Ctx, victim: ObjectId) -> SmResult<()>;
    fn is_weapon_slot_ok_to_fire(sm: &StateMachine<Self>, cx: &mut Self::Ctx, slot: WeaponSlotType) -> bool;
    fn is_attacking_object(sm: &StateMachine<Self>, cx: &mut Self::Ctx) -> bool;
    fn original_victim_pos(sm: &StateMachine<Self>, cx: &mut Self::Ctx) -> Option<Coord3D>;
}

/// The machine's goal object, when it resolves (`getMachineGoalObject()`).
fn goal<S: StateBehavior>(sm: &StateMachine<S>, cx: &S::Ctx) -> Option<ObjectId> {
    sm.goal_object(cx).map(|o| o.id)
}

/// `outOfWeaponRangeObject(thisState, userData)`: see the module's notes.
pub fn out_of_weapon_range_object<S: StateBehavior>(sm: &StateMachine<S>, cx: &mut S::Ctx) -> bool
where
    S::Ctx: FireHost,
{
    let obj = cx.owner();
    let victim = goal(sm, cx);
    let weapon = cx.current_weapon(obj);
    if let (Some(victim), Some(slot)) = (victim, weapon) {
        let mut view_blocked = false;
        let mut on_ground = cx.ai_is_doing_ground_movement(obj).unwrap_or(true);
        if cx.is_kind_of(obj, KINDOF_IMMOBILE) {
            on_ground = true;
        }
        // "brutal special case for stinger soldiers, who generally don't have locomotors, but are still on the ground."
        if cx.is_kind_of(obj, KINDOF_SPAWNS_ARE_THE_WEAPONS) {
            on_ground = true;
        }
        if let Some(c) = cx.contained_by(obj) {
            if cx.is_kind_of(c, KINDOF_STRUCTURE) || !cx.is_airborne_target(c) {
                on_ground = true; // "Contained objects on the ground -- garrisoned buildings for example!"
            }
        }
        // "at tiny ranges, isAttackViewBlockedByObstacle() can return false positives, so just skip it for contact weapons"
        if !cx.weapon_is_contact(obj, slot) && on_ground && !cx.is_significantly_above_terrain(victim) {
            let (op, vp) = (cx.position(obj), cx.position(victim));
            view_blocked = cx.is_attack_view_blocked(obj, &op, victim, &vp);
        }
        // "A weapon with leech range temporarily has unlimited range and is locked onto its target."
        if !cx.weapon_has_leech_range(obj, slot) && view_blocked && !plant(3170) {
            return true;
        }
        if !cx.weapon_has_leech_range(obj, slot) && !cx.weapon_within_range_of_object(obj, slot, victim) {
            return true;
        }
    }
    false
}

/// `AIAttackFireWeaponState::onEnter()`: see the module's notes.
pub fn fire_on_enter<S: FireAttacker>(sm: &mut StateMachine<S>, cx: &mut S::Ctx) -> SmResult<StateReturn>
where
    S::Ctx: FireHost,
{
    let obj = cx.owner();
    // "Passive stuff will approach but not attack, so we check here (after approach is complete)"
    let adjust = cx.ai_mood_action_adjustment(obj, MM_ACTION_ATTACK);
    if adjust & MAA_ACTION_OK == 0 {
        return Ok(StateReturn::FAILURE);
    }
    let victim = goal(sm, cx);
    if let Some(v) = victim {
        if cx.team_attacks_common_target(obj) && cx.team_target_object(obj).is_none() {
            cx.set_team_target_object(obj, v);
        }
    }
    cx.set_status(obj, &[OBJECT_STATUS_IS_FIRING_WEAPON], true);
    cx.pre_fire_current_weapon(obj, victim);
    Ok(StateReturn::CONTINUE)
}

/// `AIAttackFireWeaponState::update()`: see the module's notes.
pub fn fire_update<S: FireAttacker>(sm: &mut StateMachine<S>, cx: &mut S::Ctx) -> SmResult<StateReturn>
where
    S::Ctx: FireHost,
{
    let obj = cx.owner();
    let mut victim = goal(sm, cx);
    if S::is_attacking_object(sm, cx) {
        // "if our target is dead, go ahead and stop."
        match victim {
            Some(v) if !cx.is_effectively_dead(v) => {}
            _ => return Ok(StateReturn::FAILURE),
        }
    }
    let Some(slot) = cx.current_weapon(obj) else { return Ok(StateReturn::FAILURE) };
    let status = cx.weapon_status(obj, slot);
    if status == WeaponStatus::PreAttack {
        return Ok(StateReturn::CONTINUE);
    } else if status != WeaponStatus::ReadyToFire {
        return Ok(StateReturn::FAILURE);
    }
    // "the weird case where we have multi turrets, and turret 'a' wants to fire, but someone has changed the current weapon to be
    // one not on him."
    if !S::is_weapon_slot_ok_to_fire(sm, cx, slot) {
        return Ok(StateReturn::FAILURE);
    }
    // "must adjust the state BEFORE calling fireWeapon, for FX to work correctly..."
    cx.set_firing_condition_for_current_weapon(obj);
    if S::is_attacking_object(sm, cx) {
        cx.fire_current_weapon_at_object(obj, victim);
        // (Patch 1.01: the goal follows an attack transferred to another object)
        let current = cx.ai_current_victim(obj);
        if victim != current && !plant(3171) {
            let r = current.and_then(|c| cx.find_object(c));
            sm.set_goal_object(r.as_ref());
        }
        cx.set_status(obj, &[OBJECT_STATUS_IGNORING_STEALTH], false); // "clear this, just in case."
        let continue_range = cx.weapon_continue_attack_range(obj, slot);
        if let Some(v) = victim {
            if continue_range > 0.0
                && (cx.is_destroyed(v) || cx.is_effectively_dead(v) || (cx.is_kind_of(v, KINDOF_MINE) && cx.test_status(v, OBJECT_STATUS_MASKED)))
            {
                if let Some(original) = S::original_victim_pos(sm, cx) {
                    // "note that it is important to use getLastCommandSource here"
                    let source = cx.ai_last_command_source(obj).unwrap_or(CommandSourceType::FromAi);
                    let player = cx.controlling_player(v);
                    // "note that we look around originalVictimPos, *not* the current victim's pos."
                    victim = cx.closest_attackable_of_player(&original, continue_range, obj, source, player);
                    if let Some(nv) = victim {
                        let r = cx.find_object(nv);
                        sm.set_goal_object(r.as_ref());
                        S::notify_new_victim_chosen(sm, cx, nv)?;
                    }
                }
            }
        }
    } else {
        let goal_pos = *sm.goal_position();
        if cx.ai_are_turrets_linked(obj) {
            // "it doesn;t matter which weapon slot is locked, current or whatever"
            for s in SLOTS {
                if cx.has_weapon_in_slot(obj, s) {
                    if cx.weapon_fire_at_position(obj, s, &goal_pos) {
                        cx.release_weapon_lock(obj, LOCKED_TEMPORARILY); // "unlock, 'cause we're loaded"
                    }
                    cx.notify_firing_tracker_shot_fired(obj, s, INVALID_ID);
                }
            }
        } else {
            cx.fire_current_weapon_at_position(obj, &goal_pos);
        }
        cx.set_status(obj, &[OBJECT_STATUS_IGNORING_STEALTH], false); // "clear this, just in case."
    }
    S::notify_fired(sm, cx)?;
    Ok(StateReturn::SUCCESS)
}

/// `AIAttackFireWeaponState::onExit(status)`: see the module's notes.
pub fn fire_on_exit<S: StateBehavior>(_sm: &mut StateMachine<S>, cx: &mut S::Ctx, _status: StateExitType) -> SmResult<()>
where
    S::Ctx: FireHost,
{
    let obj = cx.owner();
    cx.set_status(obj, &[OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_IGNORING_STEALTH], false);
    // "this can occur if we start a preattack (eg, bayonet) and the target moves out range before we can actually "fire"..."
    if let Some(slot) = cx.current_weapon(obj) {
        if cx.weapon_status(obj, slot) == WeaponStatus::PreAttack && !plant(3172) {
            cx.weapon_set_pre_attack_finished_frame(obj, slot, 0);
        }
    }
    Ok(())
}

/// (`ATTACK_NEW_TARGET`, the filter's attack type, kept for the host's doc.)
pub const CONTINUE_ATTACK_FILTER: i32 = ATTACK_NEW_TARGET;
