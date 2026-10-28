// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AttackStateMachine; inWeaponRangeObject, wantToSquishTarget,
//     outOfWeaponRangePosition, cannotPossiblyAttackObject; AIAttackAimAtTargetState; AIAttackState; batch A10b: canPursue,
//     AIAttackApproachTargetState, AIAttackPursueTargetState)
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
//! The AI's attack state and its machine (`p1w13-sonnet`, layer 10, batches A10a and A10b).
//!
//! `AIAttackState` (the AI's `AI_ATTACK_POSITION`, `AI_ATTACK_OBJECT`, `AI_FORCE_ATTACK_OBJECT`, `AI_ATTACK_AND_FOLLOW_OBJECT`)
//! makes an `AttackStateMachine` on entry and runs it every update: **aim** (the default; turn toward the victim or position, or
//! let the turret aim; success when within the aim delta), **fire** ([`crate::logic::ai::fire_state`]), and the chase states
//! (pursue a fleeing victim with a turret, then approach a spot to fire from, batch A10b; a portable structure's chase is a
//! continue state, an immobile owner's a failure state), with
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
    OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_IGNORING_STEALTH, OBJECT_STATUS_IS_AIMING_WEAPON, OBJECT_STATUS_IS_ATTACKING,
    OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::state_machine::{
    SmResult, StateConditionInfo, StateExitType, StateId, StateMachine, StateReturn, UtilityState, EXIT_MACHINE_WITH_FAILURE,
};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_move_states::{self as move_states, mv, set_precise_z, MoveToData};
use crate::logic::ai::ai_paths;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::common::state_machine::{ObjectRef, StateContext};
use crate::common::kind_of::KINDOF_AIRCRAFT;
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
    /// (the bottom frame, batch A11) the AI's machine's goal set by `ai->friend_setGoalObject` from a machine inside it.
    pub ai_goal_object: Option<ObjectId>,
    /// (batch A17) the docking machine's `m_approachPosition`, while it runs.
    pub dock_approach: i32,
    /// (batch A18) the guard machine's members, while it runs.
    pub guard: crate::logic::ai::ai_guard::GuardData,
}

/// `ai->friend_setGoalObject(obj)`: the AI's machine's goal. `sm` is the AI's machine when no frame is lent; else (a machine inside
/// one of its states) the goal is set when that machine returns.
pub fn ai_friend_set_goal_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId) {
    if plant(3327) {
        return ai_paths::friend_set_goal_object(sm, h, obj);
    }
    match h.attack_stack().first_mut() {
        None => ai_paths::friend_set_goal_object(sm, h, obj),
        Some(bottom) => {
            bottom.ai_goal_object = Some(obj);
            bottom.outer.goal_object_id = obj;
        }
    }
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

/// The AI's machine as the mood matrix sees it: the bottom lent frame's (a machine inside one of its states runs), or `sm`'s.
pub fn top_view<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> OuterView {
    match h.attack_stack().first() {
        Some(f) => f.outer.clone(),
        None => outer_view(sm),
    }
}

/// A new `AIAttackState(machine, follow, attackingObject, forceAttacking, NULL)`.
pub fn attack_state<H: AiStateHost>(follow: bool, attacking_object: bool, force_attacking: bool, kinds: AttackOwnerKinds) -> AiState<H> {
    AiState::Attack {
        follow,
        attacking_object,
        force_attacking,
        kinds,
        machine: None,
        original_victim_pos: Coord3D::default(),
        victim_team: None,
        locked_weapon_on_enter: None,
        exit: None,
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
    let outer = top_view(sm, h);
    h.attack_stack().push(AttackFrame { outer, original_victim_pos, attacking_object, new_victim: None, ai_goal_object: None, dock_approach: -1, guard: Default::default() });
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
pub fn define_attack_states<H: AiStateHost>(
    inner: &mut Sm<H>,
    follow: bool,
    attacking_object: bool,
    force_attacking: bool,
    kinds: AttackOwnerKinds,
) -> SmResult<()> {
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
    // the chase states' constructors: the initial approach; the approach's other members the pool's zeros
    let data = ChaseData { follow, attacking_object, initial_approach: true, force_attacking, ..ChaseData::default() };
    let approach = || AiState::AttackApproach { mv: MoveToData::default(), chase: data.clone() };
    let pursue = || AiState::AttackPursue { mv: MoveToData::default(), chase: data.clone() };
    if !kinds.immobile {
        if kinds.portable_that_attacks {
            // "we're a rider on a mobile object, so we can't control our motion. just make bogus states that always fall back
            // into "aim"."
            let chase = [StateConditionInfo { test: AiCondition::InWeaponRangeObject, to_state_id: AIM_AT_TARGET }];
            inner.define_state(CHASE_TARGET, AiState::Utility(UtilityState::Continue), EXIT_MACHINE_WITH_FAILURE, EXIT_MACHINE_WITH_FAILURE, &chase)?;
        } else if attacking_object {
            // "This state will pursue a target that is moving away from it. If it is not moving away, it will drop into the
            // AIAttackApproachTarget state."
            inner.define_state(CHASE_TARGET, pursue(), APPROACH_TARGET, APPROACH_TARGET, &[])?;
            // "This state will succeed when we have a useful weapon within range of victim, and fail if the victim is dead"
            inner.define_state(APPROACH_TARGET, approach(), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
        } else {
            inner.define_state(CHASE_TARGET, approach(), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
            inner.define_state(APPROACH_TARGET, approach(), AIM_AT_TARGET, EXIT_MACHINE_WITH_FAILURE, &[])?;
        }
    } else {
        // "This state always instantly fails, so when immobile things transition here, we bail."
        inner.define_state(CHASE_TARGET, AiState::Utility(UtilityState::Failure), EXIT_MACHINE_WITH_FAILURE, EXIT_MACHINE_WITH_FAILURE, &[])?;
    }
    Ok(())
}

/// A new attack machine for the attack state's flags.
fn new_attack_machine<H: AiStateHost>(follow: bool, attacking_object: bool, force_attacking: bool, kinds: AttackOwnerKinds) -> Sm<H> {
    let mut inner = StateMachine::new();
    define_attack_states(&mut inner, follow, attacking_object, force_attacking, kinds).expect("the attack machine's definitions");
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
        AiCondition::DockAbleToAdvance => crate::logic::ai::ai_dock::able_to_advance(sm, h),
        AiCondition::GuardAttackedMe => crate::logic::ai::ai_guard::has_attacked_me_and_i_can_return_fire(h),
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
    let view = top_view(sm, h);
    let adjust = with_ai(h, |a, h| a.get_mood_matrix_action_adjustment(h, &view, MM_ACTION_ATTACK));
    if adjust & MAA_ACTION_OK == 0 {
        return Ok(StateReturn::SUCCESS);
    }
    // "if we've met the conditions specified by m_attackParameters, we consider ourselves 'successful.'" (batch A18: a guard
    // machine attack state's)
    if let Some(Some(exit)) = match sm.state(id) {
        Some(AiState::Attack { exit, .. }) => Some(*exit),
        _ => None,
    } {
        if exit.should_exit(sm, h) && !plant(3408) {
            return Ok(StateReturn::SUCCESS);
        }
    }
    // "Don't allow units under construction to attack!"
    if h.status_test(OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(StateReturn::FAILURE);
    }
    // "if all of our weapons are out of ammo, can't attack."
    if h.is_out_of_ammo() && !h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(StateReturn::FAILURE);
    }
    let (follow, attacking_object, force_attacking, kinds) = attack_flags(sm, id);
    // "create new state machine for attack behavior"
    let mut inner = new_attack_machine::<H>(follow, attacking_object, force_attacking, kinds);
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
    // "if we've met the conditions specified by m_attackParameters, we consider ourselves 'successful.'"
    if let Some(Some(exit)) = match sm.state(id) {
        Some(AiState::Attack { exit, .. }) => Some(*exit),
        _ => None,
    } {
        if exit.should_exit(sm, h) {
            return Ok(StateReturn::SUCCESS);
        }
    }
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
    ai_friend_set_goal_object(sm, h, INVALID_ID);
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
    ai_friend_set_goal_object(sm, h, INVALID_ID);
    Ok(())
}

/// `AIAttackState::xfer` (version 1): whether it has a machine, the original victim's position, the machine (made for loading
/// when there is none). Not saved: the flags (given at creation), the victim's team, the locked weapon.
pub fn attack_xfer<H: AiStateHost>(
    machine: &mut Option<Sm<H>>,
    original_victim_pos: &mut Coord3D,
    follow: bool,
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
        *machine = Some(new_attack_machine(follow, attacking_object, force_attacking, kinds));
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

// ---- batch A10b: AIAttackApproachTargetState and AIAttackPursueTargetState (to be appended to ai_attack_states.rs)

/// `MIN_RECOMPUTE_TIME` of both chase states.
const MIN_RECOMPUTE_TIME: u32 = 10;

/// The chase states' members (`AIAttackApproachTargetState`'s and `AIAttackPursueTargetState`'s).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChaseData {
    /// `m_prevVictimPos`: "Where we think our victim is".
    pub prev_victim_pos: Coord3D,
    /// `m_approachTimestamp`: "When we last computed an approach goal".
    pub approach_timestamp: u32,
    /// `m_follow`.
    pub follow: bool,
    /// `m_isAttackingObject`.
    pub attacking_object: bool,
    /// `m_stopIfInRange`.
    pub stop_if_in_range: bool,
    /// `m_isInitialApproach`: "we can attack other units along the way".
    pub initial_approach: bool,
    /// `m_isForceAttacking` (not saved).
    pub force_attacking: bool,
}

/// `xfer` of both (version 1): the base's, the victim's position, the timestamp, the flags but force-attacking.
pub fn chase_xfer(mv: &mut MoveToData, c: &mut ChaseData, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_coord3d(&mut c.prev_victim_pos)?;
    xfer.xfer_unsigned_int(&mut c.approach_timestamp)?;
    xfer.xfer_bool(&mut c.follow)?;
    xfer.xfer_bool(&mut c.attacking_object)?;
    xfer.xfer_bool(&mut c.stop_if_in_range)?;
    xfer.xfer_bool(&mut c.initial_approach)?;
    Ok(())
}

fn chase<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut ChaseData {
    match sm.state_mut(id) {
        Some(AiState::AttackApproach { chase, .. }) | Some(AiState::AttackPursue { chase, .. }) => chase,
        _ => unreachable!("a chase state"),
    }
}

/// `canPursue(source, weapon, victim)`: "Requires that 1. We are faster than the other unit. 2. The other unit is moving. 3. The
/// other unit is moving away from us." (a turret, and physics on the victim; a computer that crushes always pursues).
fn can_pursue<H: AiStateHost>(h: &mut H, victim: ObjectId) -> bool {
    if !h.object_has_physics(victim) {
        return false;
    }
    // "Have to have a turret to pursue."
    if get_which_turret_for_cur_weapon(h) == TURRET_INVALID {
        return false;
    }
    if h.ai_data_crushes_infantry()
        && h.controlling_player_type() == Some(crate::common::game_common::PlayerType::Computer)
        && h.can_crush_or_squish(victim)
    {
        return true; // "Always pursue if we can squish."
    }
    if h.weapon_is_too_close(victim) {
        return false; // "Don't chase it if we are already too close."
    }
    let our_max_speed = with_ai(h, |a, h| a.get_cur_locomotor_speed(h));
    let victim_speed = h.object_forward_speed_2d(victim);
    if victim_speed >= our_max_speed {
        return false; // "we can't catch them."
    }
    if victim_speed < our_max_speed / 10.0 && !plant(3311) {
        return false; // "They aren't moving very fast, so don't chase."
    }
    let (vp, sp) = (h.find_object(victim).expect("the victim").position, h.position());
    let (dx, dy) = (vp.x - sp.x, vp.y - sp.y);
    let dir = h.object_unit_direction_vector_2d(victim);
    if (dx * dir.x + dy * dir.y < 0.0) != plant(3312) {
        return false; // "they are moving towards us."
    }
    true
}

/// `getMachineGoalObject()` of the attack machine.
fn goal_id<H: AiStateHost>(sm: &Sm<H>, h: &H) -> Option<ObjectId> {
    sm.goal_object(h).map(|o| o.id)
}

/// `AIAttackApproachTargetState::computePath()`: "Compute a valid spot to fire our weapon from" (false: none, or pursue instead).
pub fn approach_compute_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    let mut force_repath = false;
    // "if we're immobile we can't possibly approach the target"
    if !h.is_mobile() {
        return false;
    }
    if ai(h).is_blocked_and_stuck {
        force_repath = true;
    }
    if mv(sm, id).waiting_for_path {
        return true;
    }
    if !force_repath && ai(h).path.is_none() && !ai(h).waiting_for_path {
        force_repath = true;
    }
    // "force minimum time between recomputation"
    let now = StateContext::frame(h);
    if !force_repath && now.wrapping_sub(chase(sm, id).approach_timestamp) < MIN_RECOMPUTE_TIME {
        return true;
    }
    chase(sm, id).approach_timestamp = now;
    // "if we have a goal object, move to it, otherwise move to goal position"
    if let Some(victim) = sm.goal_object(h) {
        let me = h.position();
        // "if our victim's position hasn't changed, don't re-path"
        if !force_repath && move_states::is_same_position(&me, &chase(sm, id).prev_victim_pos, &victim.position) {
            return true;
        }
        if h.current_weapon_slot().is_none() {
            return false;
        }
        // "remember where we think our victim is, so if it moves, we can re-path"
        chase(sm, id).prev_victim_pos = victim.position;
        if can_pursue(h, victim.id) {
            return false; // "break out, and do the pursuit state."
        }
        mv(sm, id).adjust_destinations = true;
        if h.weapon_is_contact() {
            // "Weapon is basically a contact weapon, so let the attacker pathfind into the target."
            ai(h).ignore_obstacle(victim.id);
            mv(sm, id).adjust_destinations = false; // "We want to run into the target."
            if !plant(3316) {
                ai(h).set_path_extra_distance(10.0 * PATHFIND_CELL_SIZE_F); // "We don't want it to slow down."
            }
        }
        let prev = chase(sm, id).prev_victim_pos;
        mv(sm, id).goal_position = prev;
        mv(sm, id).waiting_for_path = true;
        let pos = h.object_center_position(victim.id, &victim.position);
        ai_paths::request_attack_path(h, victim.id, &pos);
        chase(sm, id).stop_if_in_range = false; // "we have calculated a position to shoot from, so go there."
        true
    } else {
        // "goal position."
        mv(sm, id).adjust_destinations = true;
        chase(sm, id).stop_if_in_range = false; // "Attack position is used by missiles, and they hit the position."
        let goal = *sm.goal_position();
        mv(sm, id).goal_position = goal;
        if !force_repath {
            return true; // "fixed positions don't move."
        }
        // "must use computeAttackPath so that min ranges are considered."
        mv(sm, id).waiting_for_path = true;
        ai_paths::request_attack_path(h, INVALID_ID, &goal);
        true
    }
}

/// `AIAttackPursueTargetState::computePath()`: the victim's position, if it can still be pursued.
pub fn pursue_compute_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    let force_repath = false;
    // "if we're immobile we can't possibly approach the target"
    if !h.is_mobile() {
        return false;
    }
    if ai(h).is_blocked_and_stuck {
        return false;
    }
    if mv(sm, id).waiting_for_path {
        return true;
    }
    let mut force_repath = force_repath;
    if !force_repath && ai(h).path.is_none() && !ai(h).waiting_for_path {
        force_repath = true;
    }
    // "force minimum time between recomputation"
    let now = StateContext::frame(h);
    if !force_repath && now.wrapping_sub(chase(sm, id).approach_timestamp) < MIN_RECOMPUTE_TIME {
        return true;
    }
    chase(sm, id).approach_timestamp = now;
    // "if we have a goal object, move to it, otherwise fail & continue to AIAttackApproachTargetState"
    let Some(victim) = sm.goal_object(h) else { return false };
    let me = h.position();
    // "if our victim's position hasn't changed, don't re-path"
    if !force_repath && move_states::is_same_position(&me, &chase(sm, id).prev_victim_pos, &victim.position) {
        return true;
    }
    if h.current_weapon_slot().is_none() {
        return false;
    }
    if !can_pursue(h, victim.id) {
        return false;
    }
    // "remember where we think our victim is, so if it moves, we can re-path"
    chase(sm, id).prev_victim_pos = victim.position;
    mv(sm, id).adjust_destinations = true;
    mv(sm, id).goal_position = victim.position;
    mv(sm, id).waiting_for_path = true;
    ai_paths::request_path(h, &victim.position, false);
    chase(sm, id).stop_if_in_range = false; // "we have calculated a position to shoot from, so go there."
    true
}

/// The view from the owner to the victim is blocked (asked only for ground movement against a victim not well above the terrain).
fn victim_view_blocked<H: AiStateHost>(h: &mut H, victim: &ObjectRef) -> bool {
    if with_ai(h, |a, h| a.is_doing_ground_movement(h)) && !h.object_is_significantly_above_terrain(victim.id) {
        return h.pf_is_attack_view_blocked(victim.id, &victim.position);
    }
    false
}

/// `AIAttackApproachTargetState::onEnter()`.
pub fn approach_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    // "urg. hacky. if we are a projectile, turn on precise z-pos."
    if h.is_kind_of(KINDOF_PROJECTILE) {
        set_precise_z(h, true);
    }
    if sm.is_goal_object_destroyed(h) {
        return Ok(StateReturn::SUCCESS); // "Already killed victim."
    }
    {
        let c = chase(sm, id);
        c.prev_victim_pos = Coord3D::default();
        c.approach_timestamp = if plant(3310) { 0 } else { MIN_RECOMPUTE_TIME.wrapping_neg() };
    }
    // "See if we're close enough."
    let victim = sm.goal_object(h);
    if let Some(v) = &victim {
        if h.current_weapon_slot().is_none() {
            return Ok(StateReturn::FAILURE);
        }
        if h.weapon_is_within_attack_range_of_object(v.id) && !victim_view_blocked(h, v) {
            return Ok(StateReturn::SUCCESS);
        }
        // "If we are a player, and we got to this state via an ai command (ie we auto-acquired), we don't want to chase the unit."
        // "If we are retaliating... don't fail out!"
        if frame(h).outer.current_state_id != AI_GUARD_RETALIATE {
            let player = h.controlling_player_type().expect("upstream reads the controlling player unchecked");
            if player == crate::common::game_common::PlayerType::Human {
                if ai(h).last_command_source == CMD_FROM_AI && !ai(h).allowed_to_chase && !h.weapon_is_contact() {
                    return Ok(StateReturn::FAILURE);
                }
            } else {
                // "Computer player. Don't chase aircraft, unless we're hunting."
                let hunt = frame(h).outer.current_state_id == AI_HUNT;
                if !hunt && h.object_is_kind_of(v.id, KINDOF_AIRCRAFT) && h.object_is_airborne_target(v.id) {
                    return Ok(StateReturn::FAILURE);
                }
            }
        }
        if can_pursue(h, v.id) {
            return Ok(StateReturn::SUCCESS); // "break out, and do the pursuit state."
        }
    } else {
        // "Attacking a position. For a varitey of reasons, we need to destroy any existing path or we spin."
        ai(h).destroy_path();
    }
    // "If we have a turret, start aiming."
    let (attacking_object, force_attacking) = { let c = chase(sm, id); (c.attacking_object, c.force_attacking) };
    let tur = get_which_turret_for_cur_weapon(h);
    if tur != TURRET_INVALID {
        if attacking_object {
            set_turret_target_object(h, tur, victim.as_ref().map(|v| v.id), force_attacking);
        } else {
            let p = *sm.goal_position();
            set_turret_target_position(h, tur, Some(&p));
        }
    }
    // "find a good spot to shoot from"
    if !approach_compute_path(sm, h, id) {
        return Ok(StateReturn::FAILURE);
    }
    mv(sm, id).adjust_destinations = false;
    let ret = move_states::internal_on_enter(sm, h, id)?;
    mv(sm, id).adjust_destinations = true;
    Ok(ret)
}

/// `AIAttackApproachTargetState::updateInternal()`.
fn approach_update_internal<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if sm.is_goal_object_destroyed(h) {
        // (notifyVictimIsDead: nothing)
        with_ai(h, |a, h| a.set_current_victim(h, INVALID_ID));
        return Ok(StateReturn::FAILURE);
    }
    let is_attack_path = ai(h).is_attack_path;
    chase(sm, id).stop_if_in_range = !is_attack_path;
    let has_weapon = h.current_weapon_slot().is_some();
    if let Some(victim) = sm.goal_object(h) {
        if h.controlling_player_type().expect("upstream reads the controlling player unchecked") == crate::common::game_common::PlayerType::Computer {
            let hunt = frame(h).outer.current_state_id == AI_HUNT;
            // "Computer player. Don't chase aircraft unless hunting."
            if !hunt && h.object_is_kind_of(victim.id, KINDOF_AIRCRAFT) && h.object_is_airborne_target(victim.id) {
                return Ok(StateReturn::FAILURE);
            }
        }
        if h.object_test_status(victim.id, OBJECT_STATUS_STEALTHED)
            && !h.object_test_status(victim.id, OBJECT_STATUS_DETECTED)
            && !h.object_test_status(victim.id, OBJECT_STATUS_DISGUISED)
        {
            return Ok(StateReturn::FAILURE); // "If obj is stealthed, can no longer approach."
        }
        with_ai(h, |a, h| a.set_current_victim(h, victim.id));
        // "Attacking an object."
        if has_weapon && h.weapon_is_contact() && h.weapon_is_within_attack_range_of_object(victim.id) {
            return Ok(StateReturn::SUCCESS);
        }
        if chase(sm, id).stop_if_in_range && has_weapon && h.weapon_is_within_attack_range_of_object(victim.id) && !victim_view_blocked(h, &victim) {
            return Ok(StateReturn::SUCCESS);
        }
        // "find a good spot to shoot from"
        if !approach_compute_path(sm, h, id) {
            return Ok(StateReturn::SUCCESS);
        }
        let code = move_states::internal_update(sm, h, id)?;
        if code != StateReturn::CONTINUE {
            return Ok(StateReturn::SUCCESS); // "Always return state success, as state failure exits the attack."
        }
        Ok(code)
    } else {
        // "Attacking a position."
        let goal = mv(sm, id).goal_position;
        if chase(sm, id).stop_if_in_range && has_weapon && h.weapon_is_within_attack_range_of_position(&goal) {
            let mut view_blocked = false;
            if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
                view_blocked = h.pf_is_attack_view_blocked(INVALID_ID, &goal);
            }
            if !view_blocked {
                return Ok(StateReturn::SUCCESS);
            }
        }
        if !approach_compute_path(sm, h, id) {
            return Ok(StateReturn::FAILURE);
        }
        move_states::internal_update(sm, h, id)
    }
}

/// While the initial approach lasts, a turreted attacker's turret takes the next mood target ("we can attack other units along
/// the way").
fn initial_approach_targets<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    if !chase(sm, id).initial_approach || plant(3317) {
        return;
    }
    let tur = get_which_turret_for_cur_weapon(h);
    if tur != TURRET_INVALID {
        let outer = frame(h).outer.clone();
        let temporary_target = with_ai(h, |a, h| a.get_next_mood_target(h, &outer, true, false));
        if temporary_target != INVALID_ID {
            let force = chase(sm, id).force_attacking;
            set_turret_target_object(h, tur, Some(temporary_target), force);
        }
    }
}

/// `AIAttackApproachTargetState::update()`: following a mobile victim, it continues ("It could move (and so can we)").
pub fn approach_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut code = approach_update_internal(sm, h, id)?;
    let (follow, attacking_object) = { let c = chase(sm, id); (c.follow, c.attacking_object) };
    if follow && attacking_object {
        // "Basically, if the object is alive, we continue, in case the target moves."
        if let Some(v) = goal_id(sm, h) {
            if h.is_mobile() && !h.object_template_is_kind_of(v, KINDOF_IMMOBILE) && !plant(3313) {
                if code != StateReturn::CONTINUE {
                    chase(sm, id).initial_approach = false;
                }
                code = StateReturn::CONTINUE;
            }
        }
    }
    initial_approach_targets(sm, h, id);
    Ok(code)
}

/// `AIAttackApproachTargetState::onExit(status)`: no ignored obstacle, a projectile's precise z off, a ground mover close to its
/// goal put exactly on it; the initial approach over.
pub fn approach_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    move_states::internal_on_exit(sm, h, id);
    ai(h).ignore_obstacle(INVALID_ID);
    // "urg. hacky. if we are a projectile, reset precise z-pos."
    if h.is_kind_of(KINDOF_PROJECTILE) {
        set_precise_z(h, false);
    }
    if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
        let goal = mv(sm, id).goal_position;
        let p = h.position();
        let (dx, dy) = (goal.x - p.x, goal.y - p.y);
        if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.125 && !plant(3314) {
            // "We are doing accurate ground movement, so make sure we end exactly at the goal."
            h.set_position(&goal);
        }
    }
    chase(sm, id).initial_approach = false;
}

/// `AIAttackPursueTargetState::onEnter()`: success (to the approach) unless a turreted attacker of a moving victim can pursue it.
pub fn pursue_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(StateReturn::SUCCESS); // "Projectiles go directly to AIAttackApproachTargetState."
    }
    if sm.is_goal_object_destroyed(h) {
        return Ok(StateReturn::SUCCESS); // "Already killed victim."
    }
    let (attacking_object, force_attacking) = { let c = chase(sm, id); (c.attacking_object, c.force_attacking) };
    if !attacking_object {
        return Ok(StateReturn::SUCCESS); // "only pursue objects - positions don't move."
    }
    mv(sm, id).adjust_destinations = false;
    // "If we are retaliating... don't succeed out!"
    if frame(h).outer.current_state_id != AI_GUARD_RETALIATE
        && h.controlling_player_type().expect("upstream reads the controlling player unchecked") == crate::common::game_common::PlayerType::Human
        && ai(h).last_command_source == CMD_FROM_AI
    {
        return Ok(StateReturn::SUCCESS);
    }
    {
        let c = chase(sm, id);
        c.prev_victim_pos = Coord3D::default();
        c.approach_timestamp = MIN_RECOMPUTE_TIME.wrapping_neg();
    }
    // "See if we're close enough."
    let Some(victim) = goal_id(sm, h) else { return Ok(StateReturn::SUCCESS) }; // "gotta have a victim."
    if h.current_weapon_slot().is_none() {
        return Ok(StateReturn::FAILURE);
    }
    if !can_pursue(h, victim) {
        return Ok(StateReturn::SUCCESS);
    }
    // "If we have a turret, start aiming."
    let tur = get_which_turret_for_cur_weapon(h);
    if tur == TURRET_INVALID {
        return Ok(StateReturn::SUCCESS); // "we only pursue with turrets, as non-turreted weapons can't fire on the run."
    }
    set_turret_target_object(h, tur, Some(victim), force_attacking);
    // "find a good spot to shoot from"
    if !pursue_compute_path(sm, h, id) {
        return Ok(StateReturn::SUCCESS);
    }
    move_states::internal_on_enter(sm, h, id)
}

/// `AIAttackPursueTargetState::updateInternal()`: the chase, matching the victim's speed once in range.
fn pursue_update_internal<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if sm.is_goal_object_destroyed(h) {
        with_ai(h, |a, h| a.set_current_victim(h, INVALID_ID));
        return Ok(StateReturn::FAILURE);
    }
    chase(sm, id).stop_if_in_range = false;
    let Some(victim) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    if h.object_test_status(victim.id, OBJECT_STATUS_STEALTHED)
        && !h.object_test_status(victim.id, OBJECT_STATUS_DETECTED)
        && !h.object_test_status(victim.id, OBJECT_STATUS_DISGUISED)
    {
        return Ok(StateReturn::FAILURE); // "If obj is stealthed, can no longer pursue."
    }
    with_ai(h, |a, h| a.set_current_victim(h, victim.id));
    // "find a good spot to shoot from"
    if !pursue_compute_path(sm, h, id) {
        return Ok(StateReturn::FAILURE);
    }
    let code = move_states::internal_update(sm, h, id)?;
    if code != StateReturn::CONTINUE {
        return Ok(StateReturn::SUCCESS); // "Always return state success, as state failure exits the attack."
    }
    if h.current_weapon_slot().is_none() {
        return Ok(StateReturn::FAILURE);
    }
    let tur = get_which_turret_for_cur_weapon(h);
    if tur == TURRET_INVALID {
        return Ok(StateReturn::SUCCESS); // "We currently only pursue with a turret weapon."
    }
    let view_blocked = victim_view_blocked(h, &victim);
    if !view_blocked && h.object_has_physics(victim.id) && h.weapon_is_within_attack_range_of_object(victim.id) {
        let force = chase(sm, id).force_attacking;
        set_turret_target_object(h, tur, Some(victim.id), force);
        // "match speeds;"
        chase(sm, id).initial_approach = false;
        let mut victim_speed = h.object_forward_speed_2d(victim.id);
        let me = h.position();
        if h.weapon_is_goal_pos_within_attack_range(&me, victim.id, &victim.position) {
            victim_speed *= if plant(3315) { 1.0 } else { 0.95 };
        }
        if h.can_crush_or_squish(victim.id) {
            victim_speed = FAST_AS_POSSIBLE;
        }
        ai(h).desired_speed = victim_speed;
    } else {
        ai(h).desired_speed = FAST_AS_POSSIBLE;
    }
    Ok(code)
}

/// `AIAttackPursueTargetState::update()`.
pub fn pursue_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let code = pursue_update_internal(sm, h, id)?;
    initial_approach_targets(sm, h, id);
    Ok(code)
}

/// `AIAttackPursueTargetState::onExit(status)`.
pub fn pursue_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    move_states::internal_on_exit(sm, h, id);
    chase(sm, id).initial_approach = false;
}
