// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGuard.cpp (ExitConditions, AIGuardMachine and its states)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIGuard.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIGuardState, AIGuardRetaliateState)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGuardRetaliate.cpp (GuardRetaliateExitConditions, AIGuardRetaliateMachine and its
//     states)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIGuardRetaliate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's guard state and its guard machine (`p1w13-sonnet`, layer 10, batch A18).
//!
//! `AIGuardState` (`AI_GUARD`) runs an `AIGuardMachine` guarding a position, an object or an area: from its inner state (attack
//! the nemesis within the guard range, or enter it for an enter guard) to the outer (chase it, for a time, within the vision
//! range), a crate, the return (scanning for inner targets on the way), the idle (scanning, following a guarded object that
//! moved), and attacking an aggressor whenever one that may be fought back has attacked.
//!
//! The inner, outer and attack-aggressor states run an `AIAttackState` (or the inner an `AIEnterState`) of their own on the
//! guard machine, with their `ExitConditions`. Here that state is held by the guard state ([`AiState::GuardAttack`]'s
//! `embedded`) and put in the guard state's place in the machine while it runs ([`with_embedded`]), the exit conditions lent to it.
//! The machine's members ([`GuardData`]) are the guard state's, lent on the attack frame while the machine runs.
//!
//! `AIGuardRetaliateState` (`AI_GUARD_RETALIATE`, batch A19) runs an `AIGuardRetaliateMachine`: the same states (the same ids) from
//! the attack-aggressor state, about the AI's machine's goal position and against its goal object, without an area, a guarded
//! object or a mode; its idle state ends the machine when it finds no target, its search rejects buildings, its radii differ and its
//! exit conditions also give up when the owner is beyond the guard range of the centre. It does not lock the AI's machine. It is
//! the same [`AiState::Guard`] with [`GuardData::retaliate`] (and [`ExitConditions::retaliate`]).
//!
//! `AITunnelNetworkGuardState` (`AI_GUARD_TUNNEL_NETWORK`, batch A20) is the same [`AiState::Guard`] with [`GuardData::tunnel`]: its
//! machine and states are in [`ai_tn_guard`](crate::logic::ai::ai_tn_guard), those shared with this machine's dispatched there.
//!
//! # Upstream facts kept
//!
//! - `getGuardTargetType()` answers `m_guardTargetType[1]`; `clearGuardTargetType()` moves `[0]` to `[1]`.
//! - The return state's `xfer` saves its scan time only (not the move's); the idle state's `m_guardeePos` is not saved and no
//!   constructor sets it (the pool's zeros); the embedded attack and enter states are not saved.
//! - `AIGuardState::onEnter` returns before `recordGuardAbilityUsed`.
//! - The guard machine's crate state's exit does not call the crate state's (nor the move's) exit.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{Relationship, ATTACK_NEW_TARGET};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::KINDOF_PROJECTILE;
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateContext, StateExitType, StateId, StateMachine, StateReturn,
};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_attack_move::{self as attack_move, run_sub_machine};
use crate::logic::ai::ai_attack_states::{self as attack_states, attack_state, AttackOwnerKinds};
use crate::logic::ai::ai_move_states::{self as move_states, mv, MoveToData};
use crate::logic::ai::ai_states::{ai, with_ai, AiCondition, AiState, AiStateHost};
use crate::logic::ai::ai_tn_guard as tn_guard;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::damage::DAMAGE_HEALING;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `AI_GUARD_INNER` .. `AI_GUARD_ATTACK_AGGRESSOR`.
pub const AI_GUARD_INNER: StateId = 5000;
pub const AI_GUARD_IDLE: StateId = 5001;
pub const AI_GUARD_OUTER: StateId = 5002;
pub const AI_GUARD_RETURN: StateId = 5003;
pub const AI_GUARD_GET_CRATE: StateId = 5004;
pub const AI_GUARD_ATTACK_AGGRESSOR: StateId = 5005;

/// `ExitConditions::ATTACK_ExitIfOutsideRadius`, `ATTACK_ExitIfExpiredDuration`, `ATTACK_ExitIfNoUnitFound`.
pub const ATTACK_EXIT_IF_OUTSIDE_RADIUS: i32 = 0x01;
pub const ATTACK_EXIT_IF_EXPIRED_DURATION: i32 = 0x02;
pub const ATTACK_EXIT_IF_NO_UNIT_FOUND: i32 = 0x04;

/// `ExitConditions`: when an attack state with them gives up (its machine's goal gone, the time up, or the goal outside the
/// radius about the centre).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExitConditions {
    pub conditions_to_consider: i32,
    pub center: Coord3D,
    pub radius_sqr: f32,
    pub attack_give_up_frame: u32,
    /// `GuardRetaliateExitConditions` (batch A19): the owner, too, must stay within the guard range of the centre.
    pub retaliate: bool,
    /// `TunnelNetworkExitConditions` (batch A20): only the give-up time counts (not even the goal).
    pub tunnel: bool,
}

impl ExitConditions {
    /// `ExitConditions::shouldExit(machine)`.
    pub fn should_exit<H: AiStateHost>(&self, sm: &Sm<H>, h: &mut H) -> bool {
        if self.tunnel {
            return StateContext::frame(h) >= self.attack_give_up_frame;
        }
        let Some(goal) = sm.goal_object(h) else { return self.conditions_to_consider & ATTACK_EXIT_IF_NO_UNIT_FOUND != 0 };
        if self.conditions_to_consider & ATTACK_EXIT_IF_EXPIRED_DURATION != 0 && StateContext::frame(h) >= self.attack_give_up_frame {
            return true;
        }
        if self.conditions_to_consider & ATTACK_EXIT_IF_OUTSIDE_RADIUS != 0 && self.retaliate {
            let p = goal.position;
            let me = h.position();
            let guard_range_sqr = {
                let r = std_guard_range(h);
                r * r
            };
            let (dx, dy) = (p.x - self.center.x, p.y - self.center.y);
            if dx * dx + dy * dy > self.radius_sqr {
                return true; // "The aggressor is too far away now... give up retaliation."
            }
            let (mx, my) = (me.x - self.center.x, me.y - self.center.y);
            if mx * mx + my * my > guard_range_sqr && !plant(3410) {
                return true; // "Never go beyond standard guard radius."
            }
            return false;
        }
        if self.conditions_to_consider & ATTACK_EXIT_IF_OUTSIDE_RADIUS != 0 {
            // "BGC - when we search for a target we don't account for Z"
            let p = goal.position;
            let (dx, dy) = (p.x - self.center.x, p.y - self.center.y);
            let dz = if plant(3400) { p.z - self.center.z } else { 0.0 };
            if dx * dx + dy * dy + dz * dz > self.radius_sqr {
                return true;
            }
        }
        false
    }
}

/// The guard machine's members: `m_targetToGuard`, `m_areaToGuard` (a polygon trigger by index), `m_positionToGuard`,
/// `m_nemesisToAttack`, `m_guardMode`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardData {
    pub target_to_guard: ObjectId,
    pub area_to_guard: Option<i32>,
    pub position_to_guard: Coord3D,
    pub nemesis_to_attack: ObjectId,
    pub guard_mode: GuardMode,
    /// The machine is an `AIGuardRetaliateMachine` (batch A19).
    pub retaliate: bool,
    /// The machine is an `AITNGuardMachine` (batch A20).
    pub tunnel: bool,
}

impl Default for GuardData {
    /// `AIGuardMachine::AIGuardMachine(owner)`'s.
    fn default() -> Self {
        GuardData {
            target_to_guard: INVALID_ID,
            area_to_guard: None,
            position_to_guard: Coord3D::default(),
            nemesis_to_attack: INVALID_ID,
            guard_mode: GUARDMODE_NORMAL,
            retaliate: false,
            tunnel: false,
        }
    }
}

/// The lent guard machine's members.
pub(crate) fn gd<H: AiStateHost>(h: &mut H) -> &mut GuardData {
    &mut h.attack_stack().last_mut().expect("the guard machine's frame").guard
}

/// `AIGuardMachine::AIGuardMachine(owner)`: "So I'm making AI_GUARD_INNER the first state."
pub fn new_guard_machine<H: AiStateHost>(kinds: AttackOwnerKinds) -> Sm<H> {
    let mut m = StateMachine::new();
    let aggressors = [StateConditionInfo { test: AiCondition::GuardAttackedMe, to_state_id: AI_GUARD_ATTACK_AGGRESSOR }];
    let attack = |which| AiState::GuardAttack { which, kinds, exit: ExitConditions::default(), embedded: Vec::new() };
    let r = (|| -> SmResult<()> {
        m.define_state(AI_GUARD_INNER, attack(AI_GUARD_INNER), AI_GUARD_OUTER, AI_GUARD_OUTER, &aggressors)?;
        m.define_state(AI_GUARD_RETURN, AiState::GuardReturn { mv: MoveToData::default(), next_return_scan_time: 0 }, AI_GUARD_IDLE, AI_GUARD_INNER, &aggressors)?;
        // (the idle state's members: the pool's zeros)
        let idle = AiState::GuardIdle { next_enemy_scan_time: 0, guardee_pos: Coord3D::default(), guard_idle: true };
        m.define_state(AI_GUARD_IDLE, idle, AI_GUARD_INNER, AI_GUARD_RETURN, &aggressors)?;
        m.define_state(AI_GUARD_OUTER, attack(AI_GUARD_OUTER), AI_GUARD_GET_CRATE, AI_GUARD_GET_CRATE, &[])?;
        m.define_state(AI_GUARD_GET_CRATE, AiState::GuardCrate { mv: MoveToData::default(), delay_counter: 0 }, AI_GUARD_RETURN, AI_GUARD_RETURN, &[])?;
        m.define_state(AI_GUARD_ATTACK_AGGRESSOR, attack(AI_GUARD_ATTACK_AGGRESSOR), AI_GUARD_INNER, AI_GUARD_INNER, &[])?;
        Ok(())
    })();
    r.expect("the guard machine");
    m
}

/// `AIGuardRetaliateMachine::AIGuardRetaliateMachine(owner)`: from the attack-aggressor state; the idle state's failure ends the
/// machine.
pub fn new_guard_retaliate_machine<H: AiStateHost>(kinds: AttackOwnerKinds) -> Sm<H> {
    let mut m = StateMachine::new();
    let aggressors = [StateConditionInfo { test: AiCondition::GuardAttackedMe, to_state_id: AI_GUARD_ATTACK_AGGRESSOR }];
    let attack = |which| AiState::GuardAttack { which, kinds, exit: ExitConditions::default(), embedded: Vec::new() };
    let r = (|| -> SmResult<()> {
        m.define_state(AI_GUARD_ATTACK_AGGRESSOR, attack(AI_GUARD_ATTACK_AGGRESSOR), AI_GUARD_RETURN, AI_GUARD_RETURN, &[])?;
        m.define_state(AI_GUARD_RETURN, AiState::GuardReturn { mv: MoveToData::default(), next_return_scan_time: 0 }, AI_GUARD_IDLE, AI_GUARD_INNER, &aggressors)?;
        let idle = AiState::GuardIdle { next_enemy_scan_time: 0, guardee_pos: Coord3D::default(), guard_idle: false };
        m.define_state(AI_GUARD_IDLE, idle, AI_GUARD_INNER, crate::common::state_machine::EXIT_MACHINE_WITH_SUCCESS, &aggressors)?;
        m.define_state(AI_GUARD_INNER, attack(AI_GUARD_INNER), AI_GUARD_OUTER, AI_GUARD_OUTER, &[])?;
        m.define_state(AI_GUARD_OUTER, attack(AI_GUARD_OUTER), AI_GUARD_GET_CRATE, AI_GUARD_GET_CRATE, &[])?;
        m.define_state(AI_GUARD_GET_CRATE, AiState::GuardCrate { mv: MoveToData::default(), delay_counter: 0 }, AI_GUARD_RETURN, AI_GUARD_RETURN, &[])?;
        Ok(())
    })();
    r.expect("the guard-retaliate machine");
    m
}

/// `hasAttackedMeAndICanReturnFire`: the body's clearable last attacker (cleared) is an enemy alive that the owner may attack.
pub fn has_attacked_me_and_i_can_return_fire<H: AiStateHost>(h: &mut H) -> bool {
    let attacker = h.clearable_last_attacker();
    if attacker == INVALID_ID {
        return false;
    }
    // "K. It appears we have a valid aggressor."
    let target = if h.object_exists(attacker) { attacker } else { INVALID_ID };
    if !plant(3401) {
        h.clear_last_attacker();
    }
    if target == INVALID_ID {
        return false;
    }
    if h.object_relationship(target) != Relationship::Enemies {
        return false;
    }
    if h.object_is_effectively_dead(target) {
        return false;
    }
    if !h.is_able_to_attack() {
        return false;
    }
    let result = h.able_to_attack_specific_object(ATTACK_NEW_TARGET, target, CMD_FROM_AI);
    result == ATTACKRESULT_POSSIBLE || result == ATTACKRESULT_POSSIBLE_AFTER_MOVING
}

/// `AIGuardMachine::getStdGuardRange(obj)`.
pub(crate) fn std_guard_range<H: AiStateHost>(h: &mut H) -> f32 {
    h.adjusted_vision_range_for_object(AI_VISIONFACTOR_OWNERTYPE | AI_VISIONFACTOR_MOOD | AI_VISIONFACTOR_GUARDINNER)
}

/// The guarded object's position, else the guarded position (`findTargetToGuardByID()`).
fn guard_position<H: AiStateHost>(h: &mut H) -> (ObjectId, Coord3D) {
    let d = *gd(h);
    if h.object_exists(d.target_to_guard) {
        (d.target_to_guard, h.object_position(d.target_to_guard))
    } else {
        (INVALID_ID, d.position_to_guard)
    }
}

/// `AIGuardMachine::lookForInnerTarget()`: the team's common target, else the closest the guard (enter, hijack or attack) would
/// take about the guarded spot or in the area; it becomes the nemesis.
pub fn look_for_inner_target<H: AiStateHost>(h: &mut H) -> bool {
    if !h.is_able_to_attack() {
        return false; // "my, that was easy"
    }
    // "Check if team auto targets same victim."
    if h.team_attack_common_target() {
        let victim = h.team_target_object();
        if victim != INVALID_ID {
            gd(h).nemesis_to_attack = victim;
            return true;
        }
    }
    let (_, mut pos) = guard_position(h);
    let area = gd(h).area_to_guard;
    let retaliate = gd(h).retaliate;
    let mut filters = 0u32;
    if h.template_is_enter_guard() {
        filters |= GUARD_FILTER_POSSIBLE_TO_ENTER;
        filters |= if h.template_is_hijack_guard() { GUARD_FILTER_ENEMIES | GUARD_FILTER_POSSIBLE_TO_HIJACK } else { GUARD_FILTER_NEUTRAL };
    } else {
        filters |= GUARD_FILTER_ENEMIES | GUARD_FILTER_POSSIBLE_TO_ATTACK;
        if retaliate && !plant(3411) {
            // "Different than guard... we won't allow acquiring of structures (unless base defenses)"
            filters |= GUARD_FILTER_REJECT_BUILDINGS;
        }
    }
    if retaliate {
        filters |= GUARD_FILTER_SAME_MAP_STATUS;
        let vision_range = std_guard_range(h);
        let pos = gd(h).position_to_guard;
        let target = h.closest_guard_target(&pos, vision_range, filters, None);
        if target != INVALID_ID {
            gd(h).nemesis_to_attack = target;
            return true;
        }
        return false;
    }
    filters |= GUARD_FILTER_SAME_MAP_STATUS;
    let mut vision_range = std_guard_range(h);
    if let Some(a) = area {
        let check_frame = h.frame_objects_changed_trigger_areas().wrapping_add(h.ai_data_guard_enemy_scan_rate());
        if StateContext::frame(h) > check_frame {
            return false;
        }
        filters |= GUARD_FILTER_POLYGON;
        vision_range = h.polygon_radius(a);
        pos = h.polygon_center(a);
    }
    if gd(h).guard_mode == GUARDMODE_GUARD_FLYING_UNITS_ONLY && !plant(3402) {
        // "only consider flying targets"
        filters |= GUARD_FILTER_IS_FLYING;
    }
    let target = h.closest_guard_target(&pos, vision_range, filters, area);
    if target != INVALID_ID {
        gd(h).nemesis_to_attack = target;
        return true;
    }
    false
}

/// The guard's search filters (`lookForInnerTarget`'s): relationship enemies, possible to attack, same map status, the polygon,
/// flying, relationship neutral, possible to enter, possible to hijack.
pub const GUARD_FILTER_ENEMIES: u32 = 1;
pub const GUARD_FILTER_POSSIBLE_TO_ATTACK: u32 = 2;
pub const GUARD_FILTER_SAME_MAP_STATUS: u32 = 4;
pub const GUARD_FILTER_POLYGON: u32 = 8;
pub const GUARD_FILTER_IS_FLYING: u32 = 16;
pub const GUARD_FILTER_NEUTRAL: u32 = 32;
pub const GUARD_FILTER_POSSIBLE_TO_ENTER: u32 = 64;
pub const GUARD_FILTER_POSSIBLE_TO_HIJACK: u32 = 128;
/// (batch A19: the guard-retaliate search's `PartitionFilterRejectBuildings`)
pub const GUARD_FILTER_REJECT_BUILDINGS: u32 = 256;

// ---- the embedded attack and enter states

/// Runs `f` with the guard state `id`'s embedded state in its place (its exit conditions lent), then puts the guard state back.
pub(crate) fn with_embedded<H: AiStateHost, R>(sm: &mut Sm<H>, h: &mut H, id: StateId, f: impl FnOnce(&mut Sm<H>, &mut H) -> SmResult<R>) -> SmResult<R> {
    let mut embedded = match sm.state_mut(id) {
        Some(AiState::GuardAttack { embedded, exit, .. }) => {
            let mut e = embedded.pop().expect("an embedded state");
            if let AiState::Attack { exit: x, .. } = &mut e {
                *x = Some(*exit);
            }
            e
        }
        _ => unreachable!("a guard attack state"),
    };
    std::mem::swap(sm.state_mut(id).expect("the guard state"), &mut embedded);
    let r = f(sm, h);
    std::mem::swap(sm.state_mut(id).expect("the guard state"), &mut embedded);
    if let Some(AiState::GuardAttack { embedded: e, .. }) = sm.state_mut(id) {
        e.push(embedded);
    }
    r
}

fn has_embedded<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> Option<bool> {
    match sm.state(id) {
        Some(AiState::GuardAttack { embedded, .. }) => embedded.first().map(|e| matches!(e, AiState::Attack { .. })),
        _ => unreachable!("a guard attack state"),
    }
}

pub(crate) fn set_exit<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, f: impl FnOnce(&mut ExitConditions)) {
    if let Some(AiState::GuardAttack { exit, .. }) = sm.state_mut(id) {
        f(exit);
    }
}

/// The embedded state made (an attack state with the exit conditions, or an enter state), the machine's goal the nemesis, and
/// entered: a continue keeps it.
pub(crate) fn enter_embedded<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, state: AiState<H>, nemesis: ObjectId) -> SmResult<StateReturn> {
    if let Some(AiState::GuardAttack { embedded, .. }) = sm.state_mut(id) {
        embedded.clear();
        embedded.push(state);
    }
    let o = h.find_object(nemesis);
    sm.set_goal_object(o.as_ref());
    let enter = matches!(sm.state(id), Some(AiState::GuardAttack { embedded, .. }) if matches!(embedded.first(), Some(AiState::Enter { .. })));
    let r = with_embedded(sm, h, id, |sm, h| {
        if enter {
            crate::logic::ai::ai_enter_state::enter_on_enter(sm, h, id)
        } else {
            attack_states::attack_on_enter(sm, h, id)
        }
    })?;
    // "if we had no one to attack, we were successful, so go to the next state." (the embedded state is kept either way)
    Ok(if r == StateReturn::CONTINUE { StateReturn::CONTINUE } else { StateReturn::SUCCESS })
}

/// The embedded state's update (none: success).
pub(crate) fn update_embedded<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match has_embedded(sm, id) {
        None => Ok(StateReturn::SUCCESS),
        Some(true) => with_embedded(sm, h, id, |sm, h| attack_states::attack_update(sm, h, id)),
        Some(false) => with_embedded(sm, h, id, |sm, h| crate::logic::ai::ai_enter_state::enter_update(sm, h, id)),
    }
}

/// The embedded state's exit, then it is deleted.
pub(crate) fn exit_embedded<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    match has_embedded(sm, id) {
        None => {}
        Some(true) => with_embedded(sm, h, id, |sm, h| attack_states::attack_on_exit(sm, h, id))?,
        Some(false) => with_embedded(sm, h, id, |sm, h| {
            crate::logic::ai::ai_enter_state::enter_on_exit(sm, h, id);
            Ok(())
        })?,
    }
    if let Some(AiState::GuardAttack { embedded, .. }) = sm.state_mut(id) {
        embedded.clear();
    }
    Ok(())
}

// ---- the guard machine's attack states (inner, outer, attack aggressor)

/// The guard-retaliate machine's inner, outer and attack-aggressor states' `onEnter`.
fn retaliate_attack_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId, kinds: AttackOwnerKinds) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let set = |sm: &mut Sm<H>, center: Coord3D, radius_sqr: f32, give_up: u32, conditions: i32| {
        set_exit(sm, id, |x| {
            x.center = center;
            x.radius_sqr = radius_sqr;
            x.attack_give_up_frame = give_up;
            x.conditions_to_consider = conditions;
            x.retaliate = true;
        });
    };
    match which {
        AI_GUARD_INNER => {
            if h.template_is_enter_guard() {
                let nemesis = gd(h).nemesis_to_attack;
                if !h.object_exists(nemesis) {
                    return Ok(StateReturn::SUCCESS); // "Unexpected NULL nemesis in AIGuardRetaliateInnerState."
                }
                let enter = AiState::Enter { mv: MoveToData::default(), entry_to_clear: INVALID_ID };
                return enter_embedded(sm, h, id, enter, nemesis);
            }
            let pos = gd(h).position_to_guard;
            let nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                return Ok(StateReturn::SUCCESS);
            }
            let r = (if plant(3412) { 1.0 } else { 1.5 }) * std_guard_range(h);
            let give_up = match sm.state(id) {
                Some(AiState::GuardAttack { exit, .. }) => exit.attack_give_up_frame,
                _ => 0,
            };
            set(sm, pos, r * r, give_up, ATTACK_EXIT_IF_OUTSIDE_RADIUS | ATTACK_EXIT_IF_NO_UNIT_FOUND);
            enter_embedded(sm, h, id, attack_state(false, true, false, kinds), nemesis)
        }
        AI_GUARD_OUTER => {
            let pos = gd(h).position_to_guard;
            let nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                return Ok(StateReturn::SUCCESS);
            }
            let range = h.adjusted_vision_range_for_object(AI_VISIONFACTOR_OWNERTYPE | AI_VISIONFACTOR_MOOD);
            let r = 0.67 * (range + std_guard_range(h));
            let give_up = now.wrapping_add(h.ai_data_guard_chase_unit_frames());
            set(sm, pos, r * r, give_up, ATTACK_EXIT_IF_EXPIRED_DURATION | ATTACK_EXIT_IF_OUTSIDE_RADIUS | ATTACK_EXIT_IF_NO_UNIT_FOUND);
            enter_embedded(sm, h, id, attack_state(false, true, false, kinds), nemesis)
        }
        _ => {
            // AI_GUARD_ATTACK_AGGRESSOR: the nemesis, else the last damage's source (not a healing), made the nemesis if an enemy
            let mut nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                nemesis = INVALID_ID;
                let (damage_type, source) = h.last_damage_type_and_source();
                if source != INVALID_ID && damage_type != DAMAGE_HEALING && h.object_exists(source) {
                    nemesis = source;
                    if h.object_relationship(source) == Relationship::Enemies && !plant(3413) {
                        gd(h).nemesis_to_attack = source;
                    }
                }
            }
            if nemesis == INVALID_ID {
                return Ok(StateReturn::SUCCESS); // "Unexpected NULL nemesis in AIGuardRetaliateAttackAggressorState."
            }
            let pos = gd(h).position_to_guard;
            let range = h.adjusted_vision_range_for_object(AI_VISIONFACTOR_OWNERTYPE | AI_VISIONFACTOR_MOOD);
            let give_up = now.wrapping_add(h.ai_data_guard_chase_unit_frames());
            let r = range + std_guard_range(h);
            set(sm, pos, r * r, give_up, ATTACK_EXIT_IF_EXPIRED_DURATION | ATTACK_EXIT_IF_OUTSIDE_RADIUS | ATTACK_EXIT_IF_NO_UNIT_FOUND);
            enter_embedded(sm, h, id, attack_state(false, true, false, kinds), nemesis)
        }
    }
}

/// The inner, outer and attack-aggressor states' `onEnter`.
pub fn guard_attack_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId, kinds: AttackOwnerKinds) -> SmResult<StateReturn> {
    if gd(h).retaliate {
        return retaliate_attack_on_enter(sm, h, id, which, kinds);
    }
    if gd(h).tunnel {
        return tn_guard::tn_attack_on_enter(sm, h, id, which, kinds);
    }
    let now = StateContext::frame(h);
    match which {
        AI_GUARD_INNER => {
            // "See if we try to enter the target"
            if h.template_is_enter_guard() {
                let nemesis = gd(h).nemesis_to_attack;
                if !h.object_exists(nemesis) {
                    return Ok(StateReturn::SUCCESS); // "Unexpected NULL nemesis in AIGuardInnerState."
                }
                let enter = AiState::Enter { mv: MoveToData::default(), entry_to_clear: INVALID_ID };
                return enter_embedded(sm, h, id, enter, nemesis);
            }
            // "Or try to destroy the target"
            let (_, pos) = guard_position(h);
            let nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                return Ok(StateReturn::SUCCESS);
            }
            let range = std_guard_range(h);
            set_exit(sm, id, |x| {
                x.center = pos;
                x.radius_sqr = range * range;
                x.conditions_to_consider = ATTACK_EXIT_IF_OUTSIDE_RADIUS | ATTACK_EXIT_IF_NO_UNIT_FOUND;
            });
            enter_embedded(sm, h, id, attack_state(false, true, false, kinds), nemesis)
        }
        AI_GUARD_OUTER => {
            if gd(h).guard_mode == GUARDMODE_GUARD_WITHOUT_PURSUIT {
                // "'patrol' mode does not follow targets outside the guard area."
                return Ok(StateReturn::SUCCESS);
            }
            let (_, mut pos) = guard_position(h);
            let nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                return Ok(StateReturn::SUCCESS);
            }
            let mut range = h.adjusted_vision_range_for_object(AI_VISIONFACTOR_OWNERTYPE | AI_VISIONFACTOR_MOOD);
            if let Some(a) = gd(h).area_to_guard {
                let r = h.polygon_radius(a);
                if range < r {
                    range = r;
                }
                pos = h.polygon_center(a);
            }
            let give_up = now.wrapping_add(h.ai_data_guard_chase_unit_frames());
            set_exit(sm, id, |x| {
                x.center = pos;
                x.radius_sqr = range * range;
                x.attack_give_up_frame = give_up;
                x.conditions_to_consider = ATTACK_EXIT_IF_EXPIRED_DURATION | ATTACK_EXIT_IF_OUTSIDE_RADIUS | ATTACK_EXIT_IF_NO_UNIT_FOUND;
            });
            enter_embedded(sm, h, id, attack_state(false, true, false, kinds), nemesis)
        }
        _ => {
            // AI_GUARD_ATTACK_AGGRESSOR: the last damage's source becomes the nemesis
            let (_, source) = h.last_damage_type_and_source();
            if source != INVALID_ID {
                gd(h).nemesis_to_attack = source;
            }
            let nemesis = gd(h).nemesis_to_attack;
            if !h.object_exists(nemesis) {
                return Ok(StateReturn::SUCCESS);
            }
            let (_, pos) = guard_position(h);
            // "Don't allow guarding units to leave their guard radius!"
            let range = std_guard_range(h);
            let give_up = now.wrapping_add(h.ai_data_guard_chase_unit_frames());
            set_exit(sm, id, |x| {
                x.center = pos;
                x.radius_sqr = range * range;
                x.attack_give_up_frame = give_up;
                x.conditions_to_consider = ATTACK_EXIT_IF_EXPIRED_DURATION | ATTACK_EXIT_IF_NO_UNIT_FOUND | ATTACK_EXIT_IF_OUTSIDE_RADIUS;
            });
            enter_embedded(sm, h, id, attack_state(!plant(3403), true, false, kinds), nemesis)
        }
    }
}

/// The inner, outer and attack-aggressor states' `update`: the centre follows a guarded object; the outer state's give-up time
/// renewed while its goal is within the guard range of the centre; the embedded state's update.
pub fn guard_attack_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<StateReturn> {
    if gd(h).tunnel {
        return tn_guard::tn_attack_update(sm, h, id, which);
    }
    if which != AI_GUARD_INNER && has_embedded(sm, id).is_none() {
        return Ok(StateReturn::SUCCESS);
    }
    if which == AI_GUARD_INNER && has_embedded(sm, id) != Some(true) {
        return update_embedded(sm, h, id);
    }
    // "if the position has moved (IE we're guarding an object), move with it."
    let (target, pos) = guard_position(h);
    if target != INVALID_ID {
        set_exit(sm, id, |x| x.center = pos);
    }
    if which == AI_GUARD_OUTER {
        if let Some(goal) = sm.goal_object(h) {
            let center = match sm.state(id) {
                Some(AiState::GuardAttack { exit, .. }) => exit.center,
                _ => unreachable!("a guard attack state"),
            };
            let (dx, dy, dz) = (center.x - goal.position.x, center.y - goal.position.y, center.z - goal.position.z);
            let range = std_guard_range(h);
            if dx * dx + dy * dy + dz * dz <= range * range {
                // "reset the counter"
                let give_up = StateContext::frame(h).wrapping_add(h.ai_data_guard_chase_unit_frames());
                set_exit(sm, id, |x| x.attack_give_up_frame = give_up);
            }
        }
    }
    update_embedded(sm, h, id)
}

/// The inner, outer and attack-aggressor states' `onExit`: the embedded state's exit and deletion; the inner and aggressor
/// states clear the team's target.
pub fn guard_attack_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<()> {
    if gd(h).tunnel {
        return tn_guard::tn_attack_on_exit(sm, h, id, which);
    }
    exit_embedded(sm, h, id)?;
    if which != AI_GUARD_OUTER && h.has_team() && !plant(3404) {
        h.set_team_target_object(INVALID_ID); // "clear the target."
    }
    Ok(())
}

/// The inner, outer and attack-aggressor states' `isAttack`: their embedded attack state's (the guard-retaliate and tunnel-network
/// guard machines' states do not override it: false).
pub fn guard_attack_is_attack<H: AiStateHost>(embedded: &[AiState<H>], exit: &ExitConditions) -> bool {
    !exit.retaliate && !exit.tunnel && embedded.first().is_some_and(|e| e.is_attack())
}

// ---- AIGuardReturnState

/// `AIGuardReturnState::onEnter()`: the first scan at a random time; to the guarded object, position or area's centre (adjusted
/// on the ground).
pub fn return_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let rate = h.ai_data_guard_enemy_return_scan_rate();
    let next = now.wrapping_add(h.logic_random_value(0, rate as i32) as u32);
    let (_, mut goal) = guard_position(h);
    if let Some(a) = gd(h).area_to_guard {
        goal = h.polygon_center(a);
    }
    if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
        h.pf_adjust_destination(&mut goal);
    }
    if let Some(AiState::GuardReturn { next_return_scan_time, .. }) = sm.state_mut(id) {
        *next_return_scan_time = next;
    }
    mv(sm, id).goal_position = goal;
    mv(sm, id).adjust_destinations = !plant(3405);
    move_states::internal_on_enter(sm, h, id)
}

/// `AIGuardReturnState::update()`: a scan for an inner target now and then ("early termination because we found a target"),
/// else the move.
pub fn return_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let next = match sm.state(id) {
        Some(AiState::GuardReturn { next_return_scan_time, .. }) => *next_return_scan_time,
        _ => unreachable!("the guard return state"),
    };
    if now >= next {
        let rate = h.ai_data_guard_enemy_return_scan_rate();
        if let Some(AiState::GuardReturn { next_return_scan_time, .. }) = sm.state_mut(id) {
            *next_return_scan_time = now.wrapping_add(rate);
        }
        if look_for_inner_target(h) {
            return Ok(StateReturn::FAILURE);
        }
    }
    // "Just let the return movement finish."
    move_states::internal_update(sm, h, id)
}

/// `AIGuardReturnState::xfer` (version 1): the scan time only.
pub fn return_xfer(next_return_scan_time: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(next_return_scan_time)
}

// ---- AIGuardIdleState

/// `AIGuardIdleState::onEnter()`: the first scan at a random time ("so that everyone doesn't scan on the same frame").
pub fn idle_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if gd(h).tunnel {
        return tn_guard::tn_idle_on_enter(sm, h, id);
    }
    let now = StateContext::frame(h);
    let rate = h.ai_data_guard_enemy_scan_rate();
    let next = now.wrapping_add(h.logic_random_value(0, rate as i32) as u32);
    if let Some(AiState::GuardIdle { next_enemy_scan_time, .. }) = sm.state_mut(id) {
        *next_enemy_scan_time = next;
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIGuardIdleState::update()`: asleep until the scan; then a crate, an inner target (success), or the guarded object moved more
/// than two cells (failure, to the return).
pub fn idle_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if gd(h).tunnel {
        return tn_guard::tn_idle_update(sm, h, id);
    }
    let now = StateContext::frame(h);
    let (next, guardee) = match sm.state(id) {
        Some(AiState::GuardIdle { next_enemy_scan_time, guardee_pos, .. }) => (*next_enemy_scan_time, *guardee_pos),
        _ => unreachable!("the guard idle state"),
    };
    if now < next {
        return Ok(StateReturn::sleep(next - now));
    }
    let next = now.wrapping_add(h.ai_data_guard_enemy_scan_rate());
    if let Some(AiState::GuardIdle { next_enemy_scan_time, .. }) = sm.state_mut(id) {
        *next_enemy_scan_time = next;
    }
    // "Check to see if we have created a crate we need to pick up."
    if ai(h).crate_created != INVALID_ID {
        sm.set_state(h, AI_GUARD_GET_CRATE)?;
        // (`m_nextEnemyScanTime`, read after the transitions: they may have entered this state again; batch A20)
        let next = if plant(3409) { next } else { idle_next_enemy_scan_time(sm, id) };
        return Ok(StateReturn::sleep(next.wrapping_sub(now)));
    }
    // "if anyone is in the inner area, return success."
    if look_for_inner_target(h) {
        return Ok(StateReturn::SUCCESS);
    }
    if gd(h).retaliate {
        // (the guard-retaliate machine's idle state: none, the machine ends)
        return Ok(if plant(3414) { StateReturn::sleep(next.wrapping_sub(now)) } else { StateReturn::FAILURE });
    }
    // "See if the object we are guarding moved."
    let (target, pos) = guard_position(h);
    if target != INVALID_ID {
        let limit = 4.0 * PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F;
        let dx = guardee.x - pos.x;
        let dy = guardee.y - pos.y;
        if dx * dx > limit || (dy * dy > limit && !plant(3406)) {
            if let Some(AiState::GuardIdle { guardee_pos, .. }) = sm.state_mut(id) {
                *guardee_pos = pos;
            }
            return Ok(StateReturn::FAILURE);
        }
    }
    Ok(StateReturn::sleep(next.wrapping_sub(now)))
}

/// The idle state's `m_nextEnemyScanTime`.
pub(crate) fn idle_next_enemy_scan_time<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> u32 {
    match sm.state(id) {
        Some(AiState::GuardIdle { next_enemy_scan_time, .. }) => *next_enemy_scan_time,
        _ => unreachable!("a guard idle state"),
    }
}

/// `AIGuardIdleState::xfer` (version 1): the scan time.
pub fn idle_xfer(next_enemy_scan_time: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(next_enemy_scan_time)
}

// ---- AIGuardPickUpCrateState

/// `AIGuardPickUpCrateState::onEnter()`: a crate to pick up becomes the goal, then the crate state's; none: done.
pub fn crate_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let crate_id = with_ai(h, |a, h| a.check_for_crate_to_pickup(h));
    if crate_id != INVALID_ID {
        let o = h.find_object(crate_id);
        sm.set_goal_object(o.as_ref());
        return attack_move::crate_on_enter(sm, h, id);
    }
    Ok(StateReturn::SUCCESS) // "no crate, so we're done."
}

// ---- AIGuardState

/// `AIGuardState::onEnter()`: the guard machine made, given what the AI guards (`getGuardTargetType()`) and its mode; its default
/// state entered (a failure ends it), then its return state.
pub fn guard_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let a = ai(h);
    let mut data = GuardData::default();
    match a.guard_target_type[1] {
        GUARDTARGET_LOCATION => data.position_to_guard = a.location_to_guard,
        GUARDTARGET_OBJECT => {
            let o = a.object_to_guard;
            data.target_to_guard = o;
        }
        GUARDTARGET_AREA => data.area_to_guard = a.area_to_guard,
        _ => {}
    }
    data.guard_mode = a.guard_mode;
    if data.target_to_guard != INVALID_ID && !h.object_exists(data.target_to_guard) {
        data.target_to_guard = INVALID_ID; // (setTargetToGuard(findObjectByID(..)): none)
    }
    let kinds = match sm.state_mut(id) {
        Some(AiState::Guard { machine, data: d, kinds }) => {
            *machine = Some(new_guard_machine(*kinds));
            *d = data;
            *kinds
        }
        _ => unreachable!("the guard state"),
    };
    let _ = kinds;
    run_sub_machine(sm, h, id, |m, h| {
        if m.init_default_state(h)? == StateReturn::FAILURE && !plant(3407) {
            return Ok(StateReturn::FAILURE);
        }
        m.set_state(h, AI_GUARD_RETURN)
    })
}

/// `AIGuardState::onExit(status)`: the guard machine deleted (its state exits, `EXIT_RESET`); the guard target type cleared.
pub fn guard_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    run_sub_machine(sm, h, id, |m, h| std::mem::take(m).destroy(h))?;
    if let Some(AiState::Guard { machine, .. }) = sm.state_mut(id) {
        *machine = None;
    }
    let a = ai(h);
    a.guard_target_type[1] = a.guard_target_type[0];
    a.guard_target_type[0] = GUARDTARGET_NONE;
    Ok(())
}

/// `AIGuardState::update()`: the guard machine's update with the AI's machine locked ("We don't want to switch out of guard
/// during the update"); none: failure.
pub fn guard_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if !matches!(sm.state(id), Some(AiState::Guard { machine: Some(_), .. })) {
        return Ok(StateReturn::FAILURE); // "We actually already exited."
    }
    // (getJetAIUpdate(): none here; the out-of-ammo check is a jet's)
    let _ = KINDOF_PROJECTILE;
    sm.lock();
    let ret = run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h));
    sm.unlock();
    ret
}

/// `AIGuardState::xfer` (version 1): whether there is a machine (one made to load into), the machine (`AIGuardMachine::xfer`,
/// version 2: the machine's, the guarded object, the nemesis, the guarded position, the area's trigger name).
pub fn guard_xfer<H: AiStateHost>(machine: &mut Option<Sm<H>>, data: &mut GuardData, kinds: AttackOwnerKinds, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    if has_machine && machine.is_none() {
        *machine = Some(new_guard_machine(kinds));
        *data = GuardData::default();
    }
    if let Some(m) = machine.as_mut().filter(|_| has_machine) {
        let cv: XferVersion = 2;
        let mut v = cv;
        xfer.xfer_version(&mut v, cv)?;
        if v >= 2 {
            Snapshot::xfer(m, xfer)?;
        }
        xfer.xfer_object_id(&mut data.target_to_guard)?;
        xfer.xfer_object_id(&mut data.nemesis_to_attack)?;
        xfer.xfer_coord3d(&mut data.position_to_guard)?;
        let mut name = data.area_to_guard.map(|a| format!("poly{a}").into_bytes()).unwrap_or_default();
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() == XferMode::Load && !name.is_empty() {
            // (TheTerrainLogic->getTriggerAreaByName: the stand-ins' "poly<index>")
            data.area_to_guard = std::str::from_utf8(&name).ok().and_then(|n| n.strip_prefix("poly")).and_then(|n| n.parse().ok());
        }
    }
    Ok(())
}

// ---- AIGuardRetaliateState (batch A19)

/// `AIGuardRetaliateState::onEnter()`: the machine made about the AI's machine's goal position, its nemesis the AI's machine's
/// goal object; its default state entered (its answer the state's).
pub fn retaliate_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut data = GuardData { retaliate: true, ..GuardData::default() };
    data.position_to_guard = *sm.goal_position();
    if let Some(goal) = sm.goal_object(h) {
        data.nemesis_to_attack = goal.id;
    }
    match sm.state_mut(id) {
        Some(AiState::Guard { machine, data: d, kinds }) => {
            *machine = Some(new_guard_retaliate_machine(*kinds));
            *d = data;
        }
        _ => unreachable!("the guard-retaliate state"),
    }
    run_sub_machine(sm, h, id, |m, h| m.init_default_state(h))
}

/// `AIGuardRetaliateState::update()`: the machine's update (the AI's machine not locked); none: failure.
pub fn retaliate_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if !matches!(sm.state(id), Some(AiState::Guard { machine: Some(_), .. })) {
        return Ok(StateReturn::FAILURE); // "We actually already exited."
    }
    run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h))
}

/// `AIGuardRetaliateState::xfer` (version 1): whether there is a machine (one made to load into), the machine
/// (`AIGuardRetaliateMachine::xfer`, version 2: the machine's, the nemesis, the guarded position).
pub fn retaliate_xfer<H: AiStateHost>(machine: &mut Option<Sm<H>>, data: &mut GuardData, kinds: AttackOwnerKinds, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    if has_machine && machine.is_none() {
        *machine = Some(new_guard_retaliate_machine(kinds));
        *data = GuardData { retaliate: true, ..GuardData::default() };
    }
    if let Some(m) = machine.as_mut().filter(|_| has_machine) {
        let cv: XferVersion = 2;
        let mut v = cv;
        xfer.xfer_version(&mut v, cv)?;
        if v >= 2 {
            Snapshot::xfer(m, xfer)?;
        }
        xfer.xfer_object_id(&mut data.nemesis_to_attack)?;
        xfer.xfer_coord3d(&mut data.position_to_guard)?;
    }
    Ok(())
}

/// The guard machine's inner, outer and aggressor states' `xfer` (version 1: nothing).
pub fn version_only_xfer(xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)
}

#[allow(dead_code)]
fn _exit_type(_: StateExitType) {}
