// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AttackStateMachine; inWeaponRangeObject, wantToSquishTarget,
//     outOfWeaponRangePosition, cannotPossiblyAttackObject; AIAttackAimAtTargetState; AIAttackState)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (setTurretTargetObject, setTurretTargetPosition,
//     getTurretTurnRate, getWhichTurretForCurWeapon)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (those classes)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's attack state and its machine (`p1w13-sonnet`, layer 10, batch A10a).
//!
//! `AIAttackState` (the AI's `AI_ATTACK_POSITION`, `AI_ATTACK_OBJECT`, `AI_FORCE_ATTACK_OBJECT`, `AI_ATTACK_AND_FOLLOW_OBJECT`)
//! makes an `AttackStateMachine` on entry and runs it every update: **aim** (the default; turn toward the victim or position, or
//! let the turret aim; success when within the aim delta), **fire** ([`crate::logic::ai::fire_state`]), and the chase states
//! (pursue and approach, batch A10b; a portable structure's chase is a continue state, an immobile owner's a failure state), with
//! the conditions out of weapon range, want to squish and cannot attack (objects) or out of range (positions).
//!
//! # The shape
//!
//! The attack machine is a [`StateMachine`] of [`AiState`] too (its states are variants beside the AI's own), held by the attack
//! state's variant. While it runs, what its states ask of the attack state (`NotifyWeaponFiredInterface`: the original victim's
//! position, whether it attacks an object, the new victim) and of the AI's machine (the mood matrix's view of it) is **lent to the
//! host** as an [`AttackFrame`] ([`AiStateHost::attack_stack`], the innermost last); a new victim reaches the AI's machine's goal
//! when the attack machine's update returns.
//!
//! What the owner, its weapons and the world are asked comes through the host; [`crate::logic::ai::fire_state::FireHost`] is
//! implemented for every [`AiStateHost`] (`ai_fire_host`).
//!
//! # Upstream facts kept
//!
//! - The machine's owner kinds (immobile; a portable structure that can attack) choose its chase states when the machine is made
//!   (on entry, and on loading a state saved with one); this port reads them when the AI's machine is made (a template's kinds
//!   do not change).
//! - `m_victimTeam` and `m_lockedWeaponOnEnter` are not saved, and nothing calls the state's `loadPostProcess`: a loaded attack
//!   state keeps the values of its last entry.
//! - `onExit` deletes the machine, which exits its state (`EXIT_RESET`).

use crate::common::base_type::Coord3D;
use crate::common::game_common::{
    AbleToAttackType, Relationship, WhichTurretType, ATTACK_CONTINUED_TARGET, ATTACK_CONTINUED_TARGET_FORCED, MAX_TURRETS, TURRET_INVALID,
};
use crate::common::game_type::{ObjectId, WeaponSlotType, INVALID_ID};
use crate::common::kind_of::{KINDOF_DONT_AUTO_CRUSH_INFANTRY, KINDOF_IMMOBILE, KINDOF_PROJECTILE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE};
use crate::common::model_state::MODELCONDITION_ATTACKING;
use crate::common::object_status_types::{
    OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_IGNORING_STEALTH, OBJECT_STATUS_IS_AIMING_WEAPON, OBJECT_STATUS_IS_ATTACKING,
    OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::state_machine::{
    SmResult, StateConditionInfo, StateExitType, StateId, StateMachine, StateReturn, UtilityState, EXIT_MACHINE_WITH_FAILURE,
};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_paths;
use crate::logic::ai::ai_states::{ai, with_ai, AiCondition, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiMachineQuery;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::fire_state::FireAttacker;
use crate::logic::modules::plant;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;
use crate::logic::weapon_set_state::PREFER_MOST_DAMAGE;

type Sm<H> = StateMachine<AiState<H>>;

/// `AttackStateMachine::CHASE_TARGET`.
pub const CHASE_TARGET: StateId = 0;
/// `AttackStateMachine::APPROACH_TARGET`.
pub const APPROACH_TARGET: StateId = 1;
/// `AttackStateMachine::AIM_AT_TARGET`.
pub const AIM_AT_TARGET: StateId = 2;
/// `AttackStateMachine::FIRE_WEAPON`.
pub const FIRE_WEAPON: StateId = 3;
/// The untranslated chase states (batch A10b) are `AiState::Untranslated` of this plus their id.
pub const ATTACK_PLACEHOLDER_BASE: StateId = 1000;
/// `REL_THRESH`: "about 2 degrees".
const REL_THRESH: f32 = 0.035;

/// The owner's kinds that choose the attack machine's chase states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttackOwnerKinds {
    /// `isKindOf(KINDOF_IMMOBILE)`.
    pub immobile: bool,
    /// `isKindOf(KINDOF_PORTABLE_STRUCTURE) && isKindOf(KINDOF_CAN_ATTACK)`.
    pub portable_that_attacks: bool,
}

/// The AI's machine as the mood matrix sees it (its answers do not change while the attack machine runs, but the goal object,
/// which a new victim sets).
#[derive(Clone, Debug, PartialEq)]
pub struct OuterView {
    pub current_state_id: StateId,
    pub idle: bool,
    pub attack: bool,
    pub busy: bool,
    pub goal_object_id: ObjectId,
    pub goal_position: Coord3D,
}

impl AiMachineQuery for OuterView {
    fn current_state_id(&self) -> StateId {
        self.current_state_id
    }
    fn is_in_idle_state(&self) -> bool {
        self.idle
    }
    fn is_in_attack_state(&self) -> bool {
        self.attack
    }
    fn is_in_busy_state(&self) -> bool {
        self.busy
    }
    fn goal_object_id(&self) -> ObjectId {
        self.goal_object_id
    }
    fn goal_position(&self) -> Coord3D {
        self.goal_position
    }
}

/// What the attack machine's states ask of their attack state and of the AI's machine, lent while the attack machine runs.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackFrame {
    pub outer: OuterView,
    /// `m_originalVictimPos`.
    pub original_victim_pos: Coord3D,
    /// `m_isAttackingObject`.
    pub attacking_object: bool,
    /// A victim chosen by `notifyNewVictimChosen`, for the AI's machine's goal.
    pub new_victim: Option<ObjectId>,
}

/// The AI's machine's view (`AiMachineQuery`) for the attack machine.
fn outer_view<H: AiStateHost>(sm: &Sm<H>) -> OuterView {
    OuterView {
        current_state_id: sm.get_current_state_id(),
        idle: sm.is_in_idle_state(),
        attack: sm.is_in_attack_state(),
        busy: sm.is_in_busy_state(),
        goal_object_id: sm.goal_object_id(),
        goal_position: *sm.goal_position(),
    }
}

/// The innermost lent attack frame.
pub fn frame<H: AiStateHost>(h: &mut H) -> &mut AttackFrame {
    h.attack_stack().last_mut().expect("an attack machine is running")
}

/// Runs `f` on the attack machine of the attack state `id` (taken out of the state), with the attack frame lent to the host; then
/// a new victim becomes the AI's machine's goal.
fn run_inner<H: AiStateHost, R>(sm: &mut Sm<H>, h: &mut H, id: StateId, f: impl FnOnce(&mut Sm<H>, &mut H) -> SmResult<R>) -> SmResult<R> {
    let (mut inner, original_victim_pos, attacking_object) = match sm.state_mut(id) {
        Some(AiState::Attack { machine, original_victim_pos, attacking_object, .. }) => {
            (machine.take().expect("the attack machine"), *original_victim_pos, *attacking_object)
        }
        _ => unreachable!("the attack state"),
    };
    let outer = outer_view(sm);
    h.attack_stack().push(AttackFrame { outer, original_victim_pos, attacking_object, new_victim: None });
    let r = f(&mut inner, h);
    let lent = h.attack_stack().pop().expect("the attack frame");
    if let Some(AiState::Attack { machine, .. }) = sm.state_mut(id) {
        *machine = Some(inner);
    }
    if let Some(v) = lent.new_victim.filter(|_| !plant(3305)) {
        let o = h.find_object(v);
        sm.set_goal_object(o.as_ref());
    }
    r
}

/// `AttackStateMachine::AttackStateMachine(obj, att, name, follow, attackingObject, forceAttacking)`: "order matters: first state
/// is the default state" (aim, so that things that cannot move can shoot things in range).
pub fn define_attack_states<H: AiStateHost>(inner: &mut Sm<H>, attacking_object: bool, force_attacking: bool, kinds: AttackOwnerKinds) -> SmResult<()> {
    let objects_normal = [
        StateConditionInfo { test: AiCondition::OutOfWeaponRangeObject, to_state_id: CHASE_TARGET },
        StateConditionInfo { test: AiCondition::WantToSquishTarget, to_state_id: CHASE_TARGET },
        StateConditionInfo { test: AiCondition::CannotPossiblyAttackObject(ATTACK_CONTINUED_TARGET), to_state_id: EXIT_MACHINE_WITH_FAILURE },
    ];
    let objects_forced = [
        StateConditionInfo { test: AiCondition::OutOfWeaponRangeObject, to_state_id: CHASE_TARGET },
        StateConditionInfo {
            test: AiCondition::CannotPossiblyAttackObject(ATTACK_CONTINUED_TARGET_FORCED),
            to_state_id: EXIT_MACHINE_WITH_FAILURE,
        },
        StateConditionInfo { test: AiCondition::WantToSquishTarget, to_state_id: CHASE_TARGET },
    ];
    let positions = [StateConditionInfo { test: AiCondition::OutOfWeaponRangePosition, to_state_id: CHASE_TARGET }];
    let conditions: &[StateConditionInfo<AiCondition>] =
        if !attacking_object { &positions } else if force_attacking && !plant(3307) { &objects_forced } else { &objects_normal };
    let aim = AiState::AttackAim { attacking_object, force_attacking, can_turn_in_place: false, set_locomotor: false };
    inner.define_state(AIM_AT_TARGET, aim, FIRE_WEAPON, EXIT_MACHINE_WITH_FAILURE, conditions)?;
    inner.define_state(FIRE_WEAPON, AiState::AttackFire, AIM_AT_TARGET, AIM_AT_TARGET, conditions)?;
    let placeholder = |id: StateId| AiState::Untranslated { id: ATTACK_PLACEHOLDER_BASE + id };
    if !kinds.immobile {
        if kinds.portable_that_attacks {
            // "we're a rider on a mobile object, so we can't control our motion. just make bogus states that always fall back
            // into "aim"."
            let chase = [StateConditionInfo { test: AiCondition::InWeaponRangeObject, to_state_id: AIM_AT_TARGET }];
            inner.define_state(CHASE_TARGET, AiState::Utility(UtilityState::Continue), EXIT_MACHINE_WITH_FAILURE, EXIT_MACHINE_WITH_FAILURE, &chase)?;
        } else if attacking_object {
            // (AIAttackPursueTargetState, then AIAttackApproachTargetState: batch A10b)
            inner.define_state(CHASE_TARGET, placeholder(CHASE_TARGET), APPROACH_TARGET, APPROACH_TARGET, &[])?;
            inner.define_state(APPROACH_TARGET, placeholder(APPROACH_TARGET), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
        } else {
            // (AIAttackApproachTargetState twice: batch A10b)
            inner.define_state(CHASE_TARGET, placeholder(CHASE_TARGET), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
            inner.define_state(APPROACH_TARGET, placeholder(APPROACH_TARGET), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
        }
    } else {
        // "This state always instantly fails, so when immobile things transition here, we bail."
        inner.define_state(CHASE_TARGET, AiState::Utility(UtilityState::Failure), EXIT_MACHINE_WITH_FAILURE, EXIT_MACHINE_WITH_FAILURE, &[])?;
    }
    Ok(())
}

/// A new attack machine for the attack state's flags.
fn new_attack_machine<H: AiStateHost>(attacking_object: bool, force_attacking: bool, kinds: AttackOwnerKinds) -> Sm<H> {
    let mut inner = StateMachine::new();
    define_attack_states(&mut inner, attacking_object, force_attacking, kinds).expect("the attack machine's definitions");
    inner
}

// ---- the conditions

/// The condition `cond` of the attack machine `sm`.
pub fn test<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, cond: AiCondition) -> bool {
    match cond {
        AiCondition::OutOfWeaponRangeObject => crate::logic::ai::fire_state::out_of_weapon_range_object(sm, h),
        AiCondition::InWeaponRangeObject => !crate::logic::ai::fire_state::out_of_weapon_range_object(sm, h),
        AiCondition::WantToSquishTarget => want_to_squish_target(sm, h),
        AiCondition::OutOfWeaponRangePosition => out_of_weapon_range_position(sm, h),
        AiCondition::CannotPossiblyAttackObject(t) => cannot_possibly_attack_object(sm, h, t),
    }
}

/// `wantToSquishTarget`: a computer player's turreted attacker that can crush the victim, when the AI crushes infantry.
fn want_to_squish_target<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> bool {
    let Some(victim) = sm.goal_object(h).map(|o| o.id) else { return false };
    if h.object_contained_by(victim) != INVALID_ID {
        return false; // "can't crush guys in buildings or vehicles."
    }
    // "I can only decide to crush-attack if I am attacking with a turreted weapon."
    if get_which_turret_for_cur_weapon(h) != TURRET_INVALID
        && h.ai_data_crushes_infantry()
        && h.controlling_player_type() == Some(crate::common::game_common::PlayerType::Computer)
        && h.can_crush_or_squish(victim)
        && (!h.is_kind_of(KINDOF_DONT_AUTO_CRUSH_INFANTRY) || plant(3306))
    {
        return true;
    }
    false
}

/// `outOfWeaponRangePosition`: a view blocked from the ground, or the position out of the weapon's range.
fn out_of_weapon_range_position<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> bool {
    let pos = *sm.goal_position();
    if h.current_weapon_slot().is_none() {
        return false;
    }
    let mut on_ground = with_ai(h, |a, h| a.is_doing_ground_movement(h));
    if h.is_kind_of(KINDOF_IMMOBILE) {
        on_ground = true;
    }
    // "brutal special case for stinger soldiers, who generally don't have locomotors, but are still on the ground."
    if h.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS) {
        on_ground = true;
    }
    let container = h.contained_by();
    if container != INVALID_ID && (h.object_is_kind_of(container, KINDOF_STRUCTURE) || !h.object_is_airborne_target(container)) {
        // "Contained objects on the ground -- garrisoned buildings for example!"
        on_ground = true;
    }
    let mut view_blocked = false;
    if on_ground {
        view_blocked = h.pf_is_attack_view_blocked(INVALID_ID, &pos);
    }
    if view_blocked {
        return true;
    }
    !h.weapon_is_within_attack_range_of_position(&pos)
}

/// `cannotPossiblyAttackObject(attackType)`: the owner cannot attack at all, or not this victim (now or after moving).
fn cannot_possibly_attack_object<H: AiStateHost>(sm: &Sm<H>, h: &mut H, t: AbleToAttackType) -> bool {
    let Some(victim) = sm.goal_object(h).map(|o| o.id) else { return false };
    if !h.is_able_to_attack() {
        return true;
    }
    let src = ai(h).last_command_source;
    let result = h.able_to_attack_specific_object(t, victim, src);
    result != ATTACKRESULT_POSSIBLE && (result != ATTACKRESULT_POSSIBLE_AFTER_MOVING || plant(3309))
}

// ---- AIUpdateInterface's turret members

/// `getWhichTurretForCurWeapon()`: the first turret that holds the owner's current weapon.
pub fn get_which_turret_for_cur_weapon<H: AiStateHost>(h: &mut H) -> WhichTurretType {
    for i in 0..MAX_TURRETS as usize {
        if ai(h).has_turret_ai[i] && h.turret_holds_current_weapon(i) {
            return i as WhichTurretType;
        }
    }
    TURRET_INVALID
}

/// `setTurretTargetObject(tur, o, forceAttacking)`.
pub fn set_turret_target_object<H: AiStateHost>(h: &mut H, tur: WhichTurretType, o: Option<ObjectId>, force_attacking: bool) {
    if tur >= 0 && ai(h).has_turret_ai[tur as usize] {
        h.turret_set_target_object(tur as usize, o, force_attacking);
    }
}

/// `setTurretTargetPosition(tur, pos)`.
pub fn set_turret_target_position<H: AiStateHost>(h: &mut H, tur: WhichTurretType, pos: Option<&Coord3D>) {
    if tur >= 0 && ai(h).has_turret_ai[tur as usize] {
        h.turret_set_target_position(tur as usize, pos);
    }
}

/// `getTurretTurnRate(tur)`.
pub fn get_turret_turn_rate<H: AiStateHost>(h: &mut H, tur: WhichTurretType) -> f32 {
    if tur != TURRET_INVALID && ai(h).has_turret_ai[tur as usize] { h.turret_turn_rate(tur as usize) } else { 0.0 }
}

// ---- AIAttackState

/// `chooseWeapon()`: the best weapon for the victim (none attacking a position), then the weapon status's model conditions.
fn choose_weapon<H: AiStateHost>(sm: &Sm<H>, h: &mut H, attacking_object: bool) -> bool {
    let victim = sm.goal_object(h).map(|o| o.id);
    if attacking_object && victim.is_none() {
        return false;
    }
    let src = ai(h).last_command_source;
    let found = h.choose_best_weapon_for_target(victim, PREFER_MOST_DAMAGE, src);
    // "Check if we need to update because of the weapon choice switch."
    h.adjust_model_condition_for_weapon_status();
    found
}

/// The attack state's (follow, attacking object, force attacking, owner kinds).
fn attack_flags<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> (bool, bool, bool, AttackOwnerKinds) {
    match sm.state(id) {
        Some(AiState::Attack { follow, attacking_object, force_attacking, kinds, .. }) => (*follow, *attacking_object, *force_attacking, *kinds),
        _ => unreachable!("the attack state"),
    }
}

/// `AIAttackState::onEnter()`.
pub fn attack_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    // "if we are in sleep mode, we will not attack"
    let adjust = with_ai(h, |a, h| a.get_mood_matrix_action_adjustment(h, &*sm, MM_ACTION_ATTACK));
    if adjust & MAA_ACTION_OK == 0 {
        return Ok(StateReturn::SUCCESS);
    }
    // (m_attackParameters: none for the AI's machine's attack states)
    // "Don't allow units under construction to attack!"
    if h.status_test(OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(StateReturn::FAILURE);
    }
    // "if all of our weapons are out of ammo, can't attack."
    if h.is_out_of_ammo() && !h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(StateReturn::FAILURE);
    }
    let (_, attacking_object, force_attacking, kinds) = attack_flags(sm, id);
    // "create new state machine for attack behavior"
    let mut inner = new_attack_machine::<H>(attacking_object, force_attacking, kinds);
    // "tell the attack machine who the victim of the attack is"
    let original;
    if attacking_object {
        let victim = sm.goal_object(h);
        let Some(v) = victim.filter(|v| !h.object_is_effectively_dead(v.id)) else {
            set_machine(sm, id, Some(inner));
            // (notifyVictimIsDead: AIUpdateInterface's does nothing)
            return Ok(StateReturn::FAILURE); // "we have nothing to attack!"
        };
        let team = h.object_team(v.id);
        if let Some(AiState::Attack { victim_team, .. }) = sm.state_mut(id) {
            *victim_team = team;
        }
        inner.set_goal_object(Some(&v));
        original = v.position;
    } else {
        let p = *sm.goal_position();
        inner.set_goal_position(Some(&p));
        original = p;
    }
    if let Some(AiState::Attack { original_victim_pos, .. }) = sm.state_mut(id) {
        *original_victim_pos = original;
    }
    set_machine(sm, id, Some(inner));
    // "Something can happen to make none of our weapons work."
    if !choose_weapon(sm, h, attacking_object) {
        return Ok(StateReturn::FAILURE);
    }
    let cur_weapon = h.current_weapon_slot();
    if cur_weapon.is_some() {
        h.weapon_set_max_shot_count(NO_MAX_SHOTS_LIMIT);
        // "icky special case for ignoring stealth units we might be targeting, that are currently stealthed."
        if h.weapon_continue_attack_range() > 0.0 && !plant(3302) {
            h.set_status_bits(&[OBJECT_STATUS_IGNORING_STEALTH], true);
        }
    }
    let locked = if h.is_cur_weapon_locked() { cur_weapon } else { None };
    if let Some(AiState::Attack { locked_weapon_on_enter, .. }) = sm.state_mut(id) {
        *locked_weapon_on_enter = locked;
    }
    let ret = run_inner(sm, h, id, |inner, h| inner.init_default_state(h))?;
    if ret == StateReturn::CONTINUE {
        h.set_status_bits(&[OBJECT_STATUS_IS_ATTACKING], true);
        h.set_model_condition_state(MODELCONDITION_ATTACKING);
    }
    Ok(ret)
}

fn set_machine<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, m: Option<Sm<H>>) {
    if let Some(AiState::Attack { machine, .. }) = sm.state_mut(id) {
        *machine = m;
    }
}

/// `AIAttackState::update()`.
pub fn attack_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    // "if all of our weapons are out of ammo, can't attack."
    if h.is_out_of_ammo() && !h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(StateReturn::FAILURE);
    }
    let (_, attacking_object, _, _) = attack_flags(sm, id);
    if attacking_object {
        let Some(victim) = sm.goal_object(h).filter(|v| !h.object_is_effectively_dead(v.id)) else {
            // (notifyVictimIsDead: nothing)
            return Ok(StateReturn::SUCCESS); // "my, that was easy"
        };
        with_ai(h, |a, h| a.set_current_victim(h, victim.id));
        let victim_team = match sm.state(id) {
            Some(AiState::Attack { victim_team, .. }) => *victim_team,
            _ => unreachable!("the attack state"),
        };
        if h.object_team(victim.id) != victim_team {
            // "If, while I have been attacking my victim, it has lost its ability to attack (a recently de-garrisoned building) I
            // should bail here..."
            if !h.object_test_status(victim.id, OBJECT_STATUS_CAN_ATTACK)
                && h.object_has_contain(victim.id)
                && h.contain_is_garrisonable()
                && h.contain_count() == 0
                && h.object_relationship(victim.id) == Relationship::Neutral
            {
                return Ok(stop_attacking(sm, h, victim.id));
            }
            // "The team of the victim has changed since we began attacking it."
            if h.object_relationship(victim.id) != Relationship::Enemies && !plant(3300) {
                return Ok(stop_attacking(sm, h, victim.id));
            }
        }
        let inner_goal = match sm.state(id) {
            Some(AiState::Attack { machine: Some(inner), .. }) => inner.goal_object_id(),
            _ => unreachable!("the attack machine"),
        };
        if victim.id != inner_goal {
            // "Our parent machine has changed the goal. We need to reset to the new goal."
            if let Some(AiState::Attack { machine: Some(inner), .. }) = sm.state_mut(id) {
                inner.set_goal_object(Some(&victim));
            }
        }
    }
    // "re-evaluate our weapon choice every frame, so the sub-states don't have to."
    if !choose_weapon(sm, h, attacking_object) {
        return Ok(StateReturn::FAILURE);
    }
    let cur_weapon = h.current_weapon_slot();
    // "if we entered with a locked weapon (ie, a special weapon), then we will only keep attacking as long as that weapon remains
    // the cur weapon..."
    let locked = match sm.state(id) {
        Some(AiState::Attack { locked_weapon_on_enter, .. }) => *locked_weapon_on_enter,
        _ => unreachable!("the attack state"),
    };
    if locked.is_some() && locked != cur_weapon && !plant(3303) {
        return Ok(StateReturn::FAILURE);
    }
    // "we've shot as many times as we are allowed to"
    if cur_weapon.is_none() || h.weapon_max_shot_count() <= 0 {
        return Ok(StateReturn::FAILURE);
    }
    // "Note the use of CONVERT_SLEEP_TO_CONTINUE; even if the sub-machine sleeps, we still need to be called every frame."
    let r = run_inner(sm, h, id, |inner, h| inner.update_state_machine(h))?;
    Ok(if r.is_sleep() { StateReturn::CONTINUE } else { r })
}

/// The victim is no longer attackable: the AI's goal object none (and the team's target, if it was it), failure.
fn stop_attacking<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, victim: ObjectId) -> StateReturn {
    ai_paths::friend_set_goal_object(sm, h, INVALID_ID);
    if victim == h.team_target_object() {
        h.set_team_target_object(INVALID_ID);
    }
    // (notifyVictimIsDead: "well, not "dead", but longer attackable")
    StateReturn::FAILURE
}

/// `AIAttackState::onExit(status)`: the machine deleted (its state exits), the attack's statuses, model condition and leech
/// ranges cleared, the victim, the turrets' targets and the goal object none.
pub fn attack_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    let has_machine = matches!(sm.state(id), Some(AiState::Attack { machine: Some(_), .. }));
    if has_machine {
        run_inner(sm, h, id, |inner, h| {
            let m = std::mem::take(inner);
            m.destroy(h)
        })?;
        set_machine(sm, id, None);
    }
    h.set_status_bits(&[OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_IS_AIMING_WEAPON, OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_IGNORING_STEALTH], false);
    h.clear_model_condition_state(MODELCONDITION_ATTACKING);
    h.clear_leech_range_mode_for_all_weapons();
    with_ai(h, |a, h| a.set_current_victim(h, INVALID_ID));
    for i in 0..MAX_TURRETS {
        set_turret_target_object(h, i as WhichTurretType, None, false);
    }
    ai_paths::friend_set_goal_object(sm, h, INVALID_ID);
    Ok(())
}

/// `AIAttackState::xfer` (version 1): whether it has a machine, the original victim's position, the machine (made for loading
/// when there is none). Not saved: the flags (given at creation), the victim's team, the locked weapon.
pub fn attack_xfer<H: AiStateHost>(
    machine: &mut Option<Sm<H>>,
    original_victim_pos: &mut Coord3D,
    attacking_object: bool,
    force_attacking: bool,
    kinds: AttackOwnerKinds,
    xfer: &mut dyn Xfer,
) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    xfer.xfer_coord3d(original_victim_pos)?;
    if has_machine && machine.is_none() {
        // "create new state machine for attack behavior"
        *machine = Some(new_attack_machine(attacking_object, force_attacking, kinds));
    }
    if has_machine {
        let m = machine.as_mut().expect("the attack machine");
        // AttackStateMachine::xfer: its version, then the machine's
        let cv: XferVersion = 1;
        let mut v = cv;
        xfer.xfer_version(&mut v, cv)?;
        Snapshot::xfer(m, xfer)?;
    }
    Ok(())
}

// ---- AIAttackAimAtTargetState

fn aim_flags<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> (bool, bool, bool, bool) {
    match sm.state(id) {
        Some(AiState::AttackAim { attacking_object, force_attacking, can_turn_in_place, set_locomotor }) => {
            (*attacking_object, *force_attacking, *can_turn_in_place, *set_locomotor)
        }
        _ => unreachable!("the aim state"),
    }
}

fn set_aim<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, turn: Option<bool>, set_loco: Option<bool>) {
    if let Some(AiState::AttackAim { can_turn_in_place, set_locomotor, .. }) = sm.state_mut(id) {
        if let Some(t) = turn {
            *can_turn_in_place = t;
        }
        if let Some(s) = set_loco {
            *set_locomotor = s;
        }
    }
}

/// `AIAttackAimAtTargetState::onEnter()`.
pub fn aim_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let (attacking_object, force_attacking, _, _) = aim_flags(sm, id);
    let weapon = h.current_weapon_slot();
    let victim = sm.goal_object(h).map(|o| o.id);
    let target_pos = *sm.goal_position();
    let can_turn = {
        let a = ai(h);
        a.cur_locomotor.map(|c| (a.locomotor_set.get(c).expect("the locomotor").get_min_speed() == 0.0) != plant(3308)).unwrap_or(false)
    };
    set_aim(sm, id, Some(can_turn), None);
    // "If the object is garrisoning a building, then we want to force the object to move to the best fire point"
    let container = h.contained_by();
    let enclosing = container != INVALID_ID && weapon.is_some() && h.object_has_contain(container) && h.contain_is_enclosing_container_for_me();
    let in_firing_range = if enclosing {
        match victim {
            Some(v) => h.contain_attempt_best_fire_point_at_object(v),
            None => h.contain_attempt_best_fire_point_at_position(&target_pos),
        }
    } else if let (Some(v), Some(_)) = (victim, weapon) {
        h.weapon_is_within_attack_range_of_object(v)
    } else if weapon.is_some() {
        // "See, I can be attacking the ground. Perfectly valid."
        h.weapon_is_within_attack_range_of_position(&target_pos)
    } else {
        return Ok(StateReturn::FAILURE); // "can't happen."
    };
    // "add ourself as a targeter BEFORE calling isTemporarilyPreventingAimSuccess()."
    let me = h.object_id();
    if let Some(v) = victim {
        // (the host adds to the victim's AI, when it has one)
        h.ai_add_targeter(v, me, true);
    }
    if ai(h).data.turrets_linked {
        // "Order all turrets to attack."
        for i in 0..MAX_TURRETS {
            if attacking_object {
                set_turret_target_object(h, i as WhichTurretType, victim, force_attacking);
            } else {
                set_turret_target_position(h, i as WhichTurretType, Some(&target_pos));
            }
        }
    } else {
        let tur = get_which_turret_for_cur_weapon(h);
        if tur != TURRET_INVALID {
            // "Order specific turret to attack."
            if attacking_object {
                set_turret_target_object(h, tur, victim, force_attacking);
            } else {
                set_turret_target_position(h, tur, Some(&target_pos));
            }
        } else {
            // (isTemporarilyPreventingAimSuccess: AIUpdateInterface's false)
            let preventing = false;
            // "Contact weapons don't aim. They just go boom."
            if weapon.is_some() && h.weapon_is_contact() && in_firing_range && !preventing {
                return Ok(StateReturn::SUCCESS);
            }
        }
    }
    set_aim(sm, id, None, Some(false));
    h.set_status_bits(&[OBJECT_STATUS_IS_AIMING_WEAPON], true);
    Ok(StateReturn::CONTINUE)
}

/// `AIAttackAimAtTargetState::update()`: "Orient the machine's owner to face towards the given target".
pub fn aim_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let (attacking_object, force_attacking, can_turn_in_place, _) = aim_flags(sm, id);
    if !h.has_any_weapon() {
        return Ok(StateReturn::FAILURE);
    }
    let victim = sm.goal_object(h);
    if attacking_object && victim.as_ref().is_none_or(|v| h.object_is_effectively_dead(v.id)) {
        return Ok(StateReturn::FAILURE); // "can't aim at dead things"
    }
    let goal_pos = *sm.goal_position();
    let tur = get_which_turret_for_cur_weapon(h);
    if tur != TURRET_INVALID {
        if attacking_object {
            set_turret_target_object(h, tur, victim.as_ref().map(|v| v.id), force_attacking);
        } else {
            set_turret_target_position(h, tur, Some(&goal_pos));
        }
        // "if we have a turret, but it is incapable of turning, turn ourself."
        if get_turret_turn_rate(h, tur) != 0.0 {
            // "We always say Continue, so the Turret can do its own Aiming state."
            return Ok(StateReturn::CONTINUE);
        }
        // "else fall thru!"
    }
    let rel_angle = match (&victim, attacking_object) {
        (Some(v), true) => h.relative_angle_2d_to_object(v.id),
        _ => h.relative_angle_2d_to_position(&goal_pos),
    };
    let mut aim_delta = if h.current_weapon_slot().is_some() { h.weapon_aim_delta() } else { 0.0 };
    if aim_delta < REL_THRESH && !plant(3304) {
        aim_delta = REL_THRESH;
    }
    if can_turn_in_place {
        if rel_angle.abs() > aim_delta {
            let desired = h.orientation() + rel_angle;
            ai(h).set_locomotor_goal_orientation(desired);
            set_aim(sm, id, None, Some(true));
        }
    } else {
        let target = if attacking_object { victim.as_ref().expect("the victim").position } else { goal_pos };
        ai(h).set_locomotor_goal_position_explicit(&target);
    }
    if rel_angle.abs() < aim_delta {
        // "add ourself as a targeter BEFORE calling isTemporarilyPreventingAimSuccess()."
        let me = h.object_id();
        if let Some(v) = victim.as_ref() {
            h.ai_add_targeter(v.id, me, true);
        }
        // (isTemporarilyPreventingAimSuccess: false)
        return Ok(StateReturn::SUCCESS);
    }
    if h.is_disabled_by_type(crate::common::disabled_types::DISABLED_HELD) {
        // "We are contained by something (transport, building)."
        let in_range = if h.current_weapon_slot().is_none() {
            false
        } else if attacking_object {
            h.weapon_is_within_attack_range_of_object(victim.as_ref().expect("the victim").id)
        } else {
            h.weapon_is_within_attack_range_of_position(&goal_pos)
        };
        if !in_range {
            // "We're no longer in range, so exit with failure so we can automatically reacquire a closer target if possible."
            return Ok(StateReturn::FAILURE);
        }
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIAttackAimAtTargetState::onExit(status)`: the locomotor goal none if it set one; not aiming.
pub fn aim_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let (_, _, can_turn_in_place, set_locomotor) = aim_flags(sm, id);
    // "Tell the ai we are done moving, if we set the locomotor goal."
    if can_turn_in_place && set_locomotor && !plant(3301) {
        ai(h).set_locomotor_goal_none();
    }
    h.set_status_bits(&[OBJECT_STATUS_IS_AIMING_WEAPON], false);
}

/// `AIAttackAimAtTargetState::xfer` (version 1): `m_canTurnInPlace`, `m_setLocomotor`.
pub fn aim_xfer(can_turn_in_place: &mut bool, set_locomotor: &mut bool, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_bool(can_turn_in_place)?;
    xfer.xfer_bool(set_locomotor)?;
    Ok(())
}

// ---- the attack state as the fire state's attacker

impl<H: AiStateHost> FireAttacker for AiState<H> {
    /// `AIAttackState::notifyFired()`: "nothing".
    fn notify_fired(_sm: &mut StateMachine<Self>, _h: &mut H) -> SmResult<()> {
        Ok(())
    }
    /// `AIAttackState::notifyNewVictimChosen(victim)`: the AI's machine's goal (when the attack machine returns) and the attack
    /// machine's. "do NOT update m_originalVictimPos here."
    fn notify_new_victim_chosen(sm: &mut StateMachine<Self>, h: &mut H, victim: ObjectId) -> SmResult<()> {
        frame(h).new_victim = Some(victim);
        frame(h).outer.goal_object_id = victim;
        let o = h.find_object(victim);
        sm.set_goal_object(o.as_ref());
        Ok(())
    }
    /// `isWeaponSlotOkToFire(wslot)`: `true`.
    fn is_weapon_slot_ok_to_fire(_sm: &StateMachine<Self>, _h: &mut H, _slot: WeaponSlotType) -> bool {
        true
    }
    /// `isAttackingObject()`.
    fn is_attacking_object(_sm: &StateMachine<Self>, h: &mut H) -> bool {
        frame(h).attacking_object
    }
    /// `getOriginalVictimPos()`.
    fn original_victim_pos(_sm: &StateMachine<Self>, h: &mut H) -> Option<Coord3D> {
        Some(frame(h).original_victim_pos)
    }
}

/// The attack machine's states' `onExit` for the fire state.
pub fn fire_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, status: StateExitType) -> SmResult<()> {
    crate::logic::ai::fire_state::fire_on_exit(sm, h, status)
}
