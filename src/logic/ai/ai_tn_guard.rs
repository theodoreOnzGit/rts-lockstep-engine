// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AITNGuard.cpp (TunnelNetworkExitConditions, AITNGuardMachine and its states)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AITNGuard.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AITunnelNetworkGuardState)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's tunnel-network guard state and its machine (`p1w13-sonnet`, layer 10, batch A20).
//!
//! `AITunnelNetworkGuardState` (`AI_GUARD_TUNNEL_NETWORK`) runs an `AITNGuardMachine`: a unit guarding its player's tunnel network
//! returns into the nearest tunnel (an `AIEnterState`), idles inside it scanning for a target (the team's common target, the
//! network's current nemesis, a tunnel's goal, a tunnel's recent attacker), leaves through the tunnel nearest the target, attacks
//! it (inner), chases it (outer, for a time), picks up a crate, and returns; it attacks an aggressor whenever one that may be
//! fought back has attacked. Its states' ids are the guard machine's (`AI_TN_GUARD_INNER` .. are `AI_GUARD_INNER` ..), and its
//! inner, outer and attack-aggressor states are the guard machine's [`AiState::GuardAttack`] (their embedded attack state run the
//! same way, [`ai_guard`]), with [`ExitConditions::tunnel`] (and [`GuardData::tunnel`]): only the give-up time ends their attacks.
//!
//! # Upstream facts kept
//!
//! - The inner state's `m_scanForEnemy` is never set (the pool's zero): its one scan (`TunnelNetworkScan`) is never made.
//! - The attack-aggressor state's update asks its attack state's `getMachine()` — the tunnel-network guard machine, in the
//!   attack-aggressor state, never `AttackStateMachine::FIRE_WEAPON` — so it never renews the network's nemesis.
//! - The inner and outer states do not clear the team's target on exit; the attack-aggressor state does.
//! - No state overrides `isAttack` or `isGuardIdle`: the tunnel-network guard state is an attack only without a current state.
//! - The tunnel-network guard state's update checks the ammunition of every owner, not only a jet's.
//! - The inner state's update offers its nemesis to the network without checking there is one (when the team's victim is
//!   another); the attack-aggressor state's enter and update check.
//! - The outer state's `loadPostProcess` declares a function (`AITNGuardOuterState onEnter();`) rather than calling `onEnter`.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{Relationship, ATTACK_NEW_TARGET, ATTACK_TUNNEL_NETWORK_GUARD};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::KINDOF_PROJECTILE;
use crate::common::state_machine::{SmResult, StateConditionInfo, StateContext, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_attack_move::run_sub_machine;
use crate::logic::ai::ai_attack_states::{self as attack_states, ai_friend_set_goal_object, attack_state, AttackOwnerKinds};
use crate::logic::ai::ai_enter_state as enter_state;
use crate::logic::ai::ai_guard::{
    enter_embedded, idle_next_enemy_scan_time, exit_embedded, gd, set_exit, update_embedded, with_embedded, ExitConditions, GuardData, AI_GUARD_ATTACK_AGGRESSOR,
    AI_GUARD_GET_CRATE, AI_GUARD_IDLE, AI_GUARD_INNER, AI_GUARD_OUTER, AI_GUARD_RETURN,
};
use crate::logic::ai::ai_move_states::MoveToData;
use crate::logic::ai::ai_states::{ai, AiCondition, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `AttackStateMachine::FIRE_WEAPON`.
const ATTACK_MACHINE_FIRE_WEAPON: StateId = 3;

/// `AITNGuardMachine::AITNGuardMachine(owner)`: from the return state; the return and inner states attack aggressors.
pub fn new_tn_guard_machine<H: AiStateHost>(kinds: AttackOwnerKinds) -> Sm<H> {
    let mut m = StateMachine::new();
    let aggressors = [StateConditionInfo { test: AiCondition::TnGuardAttackedMe, to_state_id: AI_GUARD_ATTACK_AGGRESSOR }];
    let attack = |which| AiState::GuardAttack { which, kinds, exit: ExitConditions { tunnel: true, ..ExitConditions::default() }, embedded: Vec::new() };
    let r = (|| -> SmResult<()> {
        let ret = AiState::TnGuardReturn { mv: MoveToData::default(), entry_to_clear: INVALID_ID, next_return_scan_time: 0 };
        m.define_state(AI_GUARD_RETURN, ret, AI_GUARD_IDLE, AI_GUARD_INNER, &aggressors)?;
        // (the idle state's members: the pool's zeros; it is not a guard idle state)
        let idle = AiState::GuardIdle { next_enemy_scan_time: 0, guardee_pos: Coord3D::default(), guard_idle: false };
        m.define_state(AI_GUARD_IDLE, idle, AI_GUARD_INNER, AI_GUARD_RETURN, &[])?;
        m.define_state(AI_GUARD_INNER, attack(AI_GUARD_INNER), AI_GUARD_OUTER, AI_GUARD_OUTER, &aggressors)?;
        m.define_state(AI_GUARD_OUTER, attack(AI_GUARD_OUTER), AI_GUARD_GET_CRATE, AI_GUARD_GET_CRATE, &[])?;
        m.define_state(AI_GUARD_GET_CRATE, AiState::GuardCrate { mv: MoveToData::default(), delay_counter: 0 }, AI_GUARD_RETURN, AI_GUARD_RETURN, &[])?;
        m.define_state(AI_GUARD_ATTACK_AGGRESSOR, attack(AI_GUARD_ATTACK_AGGRESSOR), AI_GUARD_RETURN, AI_GUARD_RETURN, &[])?;
        Ok(())
    })();
    r.expect("the tunnel-network guard machine");
    m
}

/// AITNGuard.cpp's `hasAttackedMeAndICanReturnFire`: the guard's, without asking whether the owner is able to attack at all.
pub fn tn_has_attacked_me_and_i_can_return_fire<H: AiStateHost>(h: &mut H) -> bool {
    let attacker = h.clearable_last_attacker();
    if attacker == INVALID_ID {
        return false;
    }
    let target = if h.object_exists(attacker) { attacker } else { INVALID_ID };
    h.clear_last_attacker();
    if target == INVALID_ID {
        return false;
    }
    if h.object_relationship(target) != Relationship::Enemies {
        return false;
    }
    if h.object_is_effectively_dead(target) && !plant(3420) {
        return false;
    }
    let result = h.able_to_attack_specific_object(ATTACK_NEW_TARGET, target, CMD_FROM_AI);
    result == ATTACKRESULT_POSSIBLE || result == ATTACKRESULT_POSSIBLE_AFTER_MOVING
}

/// `findBestTunnel(ownerPlayer, pos)`: the player's tunnel (that exists) nearest `pos` in 2D, the first of equals; none without a
/// controlling player.
fn find_best_tunnel<H: AiStateHost>(h: &mut H, pos: &Coord3D) -> ObjectId {
    if !h.tunnel_system() {
        return INVALID_ID; // "should never happen, but hey."
    }
    let mut best = INVALID_ID;
    let mut best_dist_sqr = 0.0f32;
    for t in h.tunnel_container_list() {
        if h.object_exists(t) {
            let p = h.object_position(t);
            let dx = p.x - pos.x;
            let dy = p.y - pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if best == INVALID_ID || (dist_sqr < best_dist_sqr && !plant(3421)) {
                best_dist_sqr = dist_sqr;
                best = t;
            }
        }
    }
    best
}

/// `AITNGuardMachine::lookForInnerTarget()`: the team's common target, else the network's current nemesis, else (the player's
/// tunnels in order) a tunnel's AI's goal that is an enemy, or a tunnel's attacker (damage with effect within the scan rate) that
/// is an enemy the owner may attack as a tunnel-network guard (it becomes the team's target and is offered to the network).
pub fn tn_look_for_inner_target<H: AiStateHost>(h: &mut H) -> bool {
    // "Check if team auto targets same victim."
    if h.team_attack_common_target() {
        let victim = h.team_target_object();
        if victim != INVALID_ID {
            gd(h).nemesis_to_attack = victim;
            return true;
        }
    }
    if !h.tunnel_system() {
        return false; // "should never happen, but hey."
    }
    let cur = h.tunnel_cur_nemesis();
    if cur != INVALID_ID {
        gd(h).nemesis_to_attack = cur;
        return true;
    }
    for t in h.tunnel_container_list() {
        if !h.object_exists(t) {
            continue;
        }
        if let Some(victim) = h.object_ai_goal_object(t) {
            // (getRelationship(NULL): NEUTRAL)
            if victim != INVALID_ID && h.object_relationship(victim) == Relationship::Enemies {
                gd(h).nemesis_to_attack = victim;
                return true;
            }
        }
        if let Some((no_effect, source, stamp)) = h.object_last_damage(t) {
            if no_effect && !plant(3422) {
                continue;
            }
            if stamp.wrapping_add(h.ai_data_guard_enemy_scan_rate()) > StateContext::frame(h) {
                if h.object_exists(source) {
                    if h.object_relationship(source) != Relationship::Enemies {
                        continue;
                    }
                    let result = h.able_to_attack_specific_object(ATTACK_TUNNEL_NETWORK_GUARD, source, CMD_FROM_AI);
                    if result == ATTACKRESULT_POSSIBLE || result == ATTACKRESULT_POSSIBLE_AFTER_MOVING {
                        gd(h).nemesis_to_attack = source;
                        h.set_team_target_object(source);
                        h.tunnel_update_nemesis(source);
                        return true;
                    }
                }
            }
        }
    }
    false
}

// ---- the inner, outer and attack-aggressor states

/// The inner, outer and attack-aggressor states' `onEnter`: the nemesis (the aggressor's: the last damage's source), attacked
/// until the chase time is up.
pub fn tn_attack_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId, kinds: AttackOwnerKinds) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let mut follow = false;
    match which {
        AI_GUARD_INNER => {}
        AI_GUARD_OUTER => {
            if gd(h).guard_mode == GUARDMODE_GUARD_WITHOUT_PURSUIT {
                // "'patrol' mode does not follow targets outside the guard area."
                return Ok(StateReturn::SUCCESS);
            }
        }
        _ => {
            // AI_TN_GUARD_ATTACK_AGGRESSOR
            let (_, source) = h.last_damage_type_and_source();
            if source != INVALID_ID {
                gd(h).nemesis_to_attack = source;
            }
            follow = !plant(3423);
        }
    }
    let nemesis = gd(h).nemesis_to_attack;
    if !h.object_exists(nemesis) {
        return Ok(StateReturn::SUCCESS); // "Unexpected NULL nemesis in AITNGuard..State."
    }
    if which == AI_GUARD_ATTACK_AGGRESSOR && h.tunnel_system() {
        h.tunnel_update_nemesis(nemesis);
    }
    let give_up = now.wrapping_add(h.ai_data_guard_chase_unit_frames());
    set_exit(sm, id, |x| x.attack_give_up_frame = give_up);
    enter_embedded(sm, h, id, attack_state(follow, true, false, kinds), nemesis)
}

/// The inner, outer and attack-aggressor states' `update`.
pub fn tn_attack_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<StateReturn> {
    let chase = |h: &mut H| StateContext::frame(h).wrapping_add(h.ai_data_guard_chase_unit_frames());
    match which {
        AI_GUARD_INNER => {
            let nemesis = gd(h).nemesis_to_attack;
            let nemesis = if h.object_exists(nemesis) { nemesis } else { INVALID_ID };
            let tunnels = h.tunnel_system();
            // "killed him."
            let team_victim = h.team_target_object();
            if nemesis == INVALID_ID {
                if team_victim != INVALID_ID {
                    gd(h).nemesis_to_attack = team_victim;
                    let give_up = chase(h);
                    set_exit(sm, id, |x| x.attack_give_up_frame = give_up);
                    return Ok(StateReturn::CONTINUE);
                }
                // "Check tunnel."
                if tunnels {
                    let n = h.tunnel_cur_nemesis();
                    if n != INVALID_ID && !plant(3424) {
                        gd(h).nemesis_to_attack = n;
                        let give_up = chase(h);
                        set_exit(sm, id, |x| x.attack_give_up_frame = give_up);
                        return Ok(StateReturn::CONTINUE);
                    }
                }
                // (m_scanForEnemy: never set, the scan never made)
            } else if nemesis != team_victim && team_victim != INVALID_ID {
                // (the network unchecked upstream; the scenes keep a controlling player while the state runs)
                h.tunnel_update_nemesis(nemesis);
                gd(h).nemesis_to_attack = team_victim;
            }
            update_embedded(sm, h, id)
        }
        AI_GUARD_OUTER => {
            if sm.goal_object(h).is_none() {
                let mut goal = INVALID_ID;
                let nemesis = gd(h).nemesis_to_attack;
                if h.object_exists(nemesis) {
                    goal = nemesis;
                }
                // "Check if team auto targets same victim."
                if goal == INVALID_ID && h.team_attack_common_target() {
                    let team_victim = h.team_target_object();
                    if team_victim != INVALID_ID {
                        goal = team_victim;
                    }
                    let o = h.find_object(goal);
                    sm.set_goal_object(o.as_ref());
                    // (the attack state entered again, not left first)
                    return with_embedded(sm, h, id, |sm, h| attack_states::attack_on_enter(sm, h, id));
                }
            }
            update_embedded(sm, h, id)
        }
        _ => {
            // (the attack state's machine is this one, in this state: never FIRE_WEAPON)
            if sm.get_current_state_id() == ATTACK_MACHINE_FIRE_WEAPON {
                let nemesis = gd(h).nemesis_to_attack;
                let nemesis = if h.object_exists(nemesis) { nemesis } else { INVALID_ID };
                if h.tunnel_system() {
                    h.tunnel_update_nemesis(nemesis);
                }
            }
            update_embedded(sm, h, id)
        }
    }
}

/// The inner, outer and attack-aggressor states' `onExit`: the embedded state's exit and deletion; the attack-aggressor state
/// clears the team's target.
pub fn tn_attack_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<()> {
    exit_embedded(sm, h, id)?;
    if which == AI_GUARD_ATTACK_AGGRESSOR && h.has_team() && !plant(3425) {
        h.set_team_target_object(INVALID_ID); // "clear the target."
    }
    Ok(())
}

// ---- AITNGuardReturnState

fn set_next_return_scan<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, next: u32) {
    if let Some(AiState::TnGuardReturn { next_return_scan_time, .. }) = sm.state_mut(id) {
        *next_return_scan_time = next;
    }
}

/// `AITNGuardReturnState::onEnter()`: done inside a tunnel; the team's victim fails it (to the inner state); else the tunnel
/// nearest the owner is the machine's and the AI's goal, entered (none: failure).
pub fn tn_return_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let rate = h.ai_data_guard_enemy_return_scan_rate();
    let next = now.wrapping_add(h.logic_random_value(0, rate as i32) as u32);
    set_next_return_scan(sm, id, next);
    if h.contained_by() != INVALID_ID {
        return Ok(StateReturn::SUCCESS); // "we are already inside our tunnel."
    }
    if h.has_team() {
        let team_victim = h.team_target_object();
        if team_victim != INVALID_ID {
            // "We have a team target. Go attack it rather than returning to the tunel."
            gd(h).nemesis_to_attack = team_victim;
            return Ok(StateReturn::FAILURE);
        }
    }
    // "Find tunnel network to enter."
    let pos = h.position();
    let best = find_best_tunnel(h, &pos);
    if best == INVALID_ID {
        return Ok(StateReturn::FAILURE);
    }
    let o = h.find_object(best);
    sm.set_goal_object(o.as_ref());
    ai_friend_set_goal_object(sm, h, best);
    enter_state::enter_on_enter(sm, h, id)
}

/// `AITNGuardReturnState::update()`: the team's victim, else the network's nemesis, fails it (to the inner state); else the enter
/// state's update, anything but a continue a success.
pub fn tn_return_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.has_team() {
        let team_victim = h.team_target_object();
        if team_victim != INVALID_ID {
            gd(h).nemesis_to_attack = team_victim;
            return Ok(StateReturn::FAILURE); // "Fail to return goes to inner attack state."
        }
    }
    // "Check tunnel for target."
    if h.tunnel_system() {
        let nemesis = h.tunnel_cur_nemesis();
        if nemesis != INVALID_ID && !plant(3426) {
            gd(h).nemesis_to_attack = nemesis;
            return Ok(StateReturn::FAILURE);
        }
    }
    let ret = enter_state::enter_update(sm, h, id)?;
    Ok(if ret == StateReturn::CONTINUE { StateReturn::CONTINUE } else { StateReturn::SUCCESS })
}

/// `AITNGuardReturnState::xfer` (version 1): the enter state's, then the scan time.
pub fn tn_return_xfer(mv: &mut MoveToData, entry_to_clear: &mut ObjectId, next_return_scan_time: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    enter_state::enter_xfer(mv, entry_to_clear, xfer)?;
    xfer.xfer_unsigned_int(next_return_scan_time)
}

// ---- AITNGuardIdleState

fn set_next_enemy_scan<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, next: u32) {
    if let Some(AiState::GuardIdle { next_enemy_scan_time, .. }) = sm.state_mut(id) {
        *next_enemy_scan_time = next;
    }
}

/// `AITNGuardIdleState::onEnter()`: the first scan at a random time; the AI's goal cleared.
pub fn tn_idle_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let rate = h.ai_data_guard_enemy_scan_rate();
    let next = now.wrapping_add(h.logic_random_value(0, rate as i32) as u32);
    set_next_enemy_scan(sm, id, next);
    ai_friend_set_goal_object(sm, h, INVALID_ID);
    Ok(StateReturn::CONTINUE)
}

/// `AITNGuardIdleState::update()`: asleep until the scan; then a crate, an inner target (inside a tunnel: out through the tunnel
/// nearest it, in a hurry, when its exit is free; else success, to the inner state), or outside a tunnel with one to go to
/// (failure, to the return).
pub fn tn_idle_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let next = match sm.state(id) {
        Some(AiState::GuardIdle { next_enemy_scan_time, .. }) => *next_enemy_scan_time,
        _ => unreachable!("the tunnel-network guard idle state"),
    };
    if now < next {
        return Ok(StateReturn::sleep(next - now));
    }
    let next = now.wrapping_add(h.ai_data_guard_enemy_scan_rate());
    set_next_enemy_scan(sm, id, next);
    ai_friend_set_goal_object(sm, h, INVALID_ID);
    // "Check to see if we have created a crate we need to pick up."
    if ai(h).crate_created != INVALID_ID {
        sm.set_state(h, AI_GUARD_GET_CRATE)?;
        // (the member, read after the transitions: they may have entered this state again)
        let next = if plant(3430) { next } else { idle_next_enemy_scan_time(sm, id) };
        return Ok(StateReturn::sleep(next.wrapping_sub(now)));
    }
    // "if anyone is in the inner area, return success."
    if tn_look_for_inner_target(h) {
        let nemesis = gd(h).nemesis_to_attack;
        if !h.object_exists(nemesis) {
            return Ok(StateReturn::sleep(0)); // "Unexpected NULL nemesis in AITNGuardAttackAggressorState."
        }
        if h.contained_by() != INVALID_ID && !plant(3427) {
            let pos = h.object_position(nemesis);
            // (some tunnel: the scenes' container is one of the network's)
            let best = find_best_tunnel(h, &pos);
            if !h.object_has_contain(best) || !h.contain_has_exit_interface(best) {
                return Ok(StateReturn::FAILURE);
            }
            if h.exit_is_busy(best) {
                return Ok(StateReturn::sleep(0)); // "Just wait a sec."
            }
            h.exit_object_in_a_hurry(best);
            return Ok(StateReturn::sleep(0));
        }
        return Ok(StateReturn::SUCCESS);
    }
    let pos = h.position();
    if h.contained_by() == INVALID_ID && find_best_tunnel(h, &pos) != INVALID_ID {
        return Ok(StateReturn::FAILURE); // "go to AITNGuardReturnState, & enter a tunnel."
    }
    Ok(StateReturn::sleep(next.wrapping_sub(now)))
}

// ---- AITunnelNetworkGuardState

/// `AITunnelNetworkGuardState::onEnter()`: the machine made about the AI's guard location, in its guard mode; its default state
/// entered (a failure ends it), then its return state.
pub fn tn_guard_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let a = ai(h);
    let (location, mode) = (a.location_to_guard, a.guard_mode);
    match sm.state_mut(id) {
        Some(AiState::Guard { machine, data, kinds }) => {
            *machine = Some(new_tn_guard_machine(*kinds));
            *data = GuardData { tunnel: true, ..GuardData::default() };
            data.position_to_guard = location;
            data.guard_mode = mode;
        }
        _ => unreachable!("the tunnel-network guard state"),
    }
    run_sub_machine(sm, h, id, |m, h| {
        if m.init_default_state(h)? == StateReturn::FAILURE && !plant(3428) {
            return Ok(StateReturn::FAILURE);
        }
        m.set_state(h, AI_GUARD_RETURN)
    })
}

/// `AITunnelNetworkGuardState::update()`: none: failure; out of ammunition (not a projectile): failure; else the machine's update
/// with the AI's machine locked.
pub fn tn_guard_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if !matches!(sm.state(id), Some(AiState::Guard { machine: Some(_), .. })) {
        return Ok(StateReturn::FAILURE); // "We actually already exited."
    }
    // "if all of our weapons are out of ammo, can't attack."
    if h.is_out_of_ammo() && !h.is_kind_of(KINDOF_PROJECTILE) && !plant(3429) {
        return Ok(StateReturn::FAILURE);
    }
    sm.lock();
    let ret = run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h));
    sm.unlock();
    ret
}

/// `AITunnelNetworkGuardState::xfer` (version 1): whether there is a machine (one made to load into), the machine
/// (`AITNGuardMachine::xfer`, version 2: the machine's, the nemesis, the guarded position).
pub fn tn_guard_xfer<H: AiStateHost>(machine: &mut Option<Sm<H>>, data: &mut GuardData, kinds: AttackOwnerKinds, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    if has_machine && machine.is_none() {
        *machine = Some(new_tn_guard_machine(kinds));
        *data = GuardData { tunnel: true, ..GuardData::default() };
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
