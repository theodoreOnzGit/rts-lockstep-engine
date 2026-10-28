// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIAttackThenIdleStateMachine, AIHuntState, AIAttackAreaState)
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
//! The AI's hunt and attack-area states (`p1w13-sonnet`, layer 10, batch A12).
//!
//! `AIHuntState` (`AI_HUNT`) and `AIAttackAreaState` (`AI_ATTACK_AREA`) each run an `AIAttackThenIdleStateMachine` (an attack state,
//! the default; the crate state; an idle state that does not look for targets), made on entry and deleted on exit. Once a second
//! (the first scan at a random frame of that second) they look for the closest enemy (the attack-area state's in its area), make it
//! the machine's goal and attack it from idle; the hunt state picks up crates and shares a team's common target by the attack
//! priorities. The machine runs every update with the AI's machine locked.
//!
//! The machines are [`StateMachine`]s of [`AiState`] held by the states' variants, run as the attack-move state's is
//! ([`run_sub_machine`]).
//!
//! # Upstream facts kept
//!
//! - The hunt state's exit releases a temporary weapon lock; the attack-area state's does not.
//! - The attack-area state fails without an area only at a scan, and succeeds when a scan finds no victim.

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::KINDOF_PROJECTILE;
use crate::common::state_machine::{SmResult, StateContext, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_attack_move::{pick_up_crate, run_sub_machine};
use crate::logic::ai::ai_attack_states::{attack_state, AttackOwnerKinds};
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::modules::plant;
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;

type Sm<H> = StateMachine<AiState<H>>;

/// `ENEMY_SCAN_RATE` (both states').
const ENEMY_SCAN_RATE: u32 = LOGICFRAMES_PER_SECOND as u32;
/// The scans' range ("do NOT do line of sight check - we want to find everything").
const SCAN_RANGE: f32 = 9999.9;

/// `AIAttackThenIdleStateMachine::AIAttackThenIdleStateMachine(owner, name)`: "order matters: first state is the default state."
pub fn new_attack_then_idle_machine<H: AiStateHost>(kinds: AttackOwnerKinds) -> Sm<H> {
    let mut m = StateMachine::new();
    m.define_state(AI_ATTACK_OBJECT, attack_state(false, true, false, kinds), AI_IDLE, AI_IDLE, &[]).expect("the attack-then-idle machine");
    m.define_state(AI_PICK_UP_CRATE, pick_up_crate(), AI_IDLE, AI_IDLE, &[]).expect("the attack-then-idle machine");
    let idle = AiState::Idle { initial_sleep_offset: u16::MAX, should_look_for_targets: false, inited: false };
    m.define_state(AI_IDLE, idle, AI_IDLE, AI_IDLE, &[]).expect("the attack-then-idle machine");
    m
}

/// A new `AIHuntState` (`area` false) or `AIAttackAreaState` (true): no machine, no scan time.
pub fn hunt<H: AiStateHost>(area: bool, kinds: AttackOwnerKinds) -> AiState<H> {
    AiState::Hunt { area, kinds, machine: None, next_enemy_scan_time: 0 }
}

/// `isAttack()`: the machine's attack state.
pub fn is_attack<H: AiStateHost>(machine: &Option<Sm<H>>) -> bool {
    machine.as_ref().is_some_and(|m| m.is_in_attack_state())
}

fn scan_time<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut u32 {
    match sm.state_mut(id) {
        Some(AiState::Hunt { next_enemy_scan_time, .. }) => next_enemy_scan_time,
        _ => unreachable!("the hunt state"),
    }
}

/// `onEnter()`: the machine made; the first scan at a random frame of the next second ("so that everyone doesn't scan on the same
/// frame"); the machine's default state entered (its answer the state's).
pub fn hunt_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match sm.state_mut(id) {
        Some(AiState::Hunt { kinds, machine, .. }) => *machine = Some(new_attack_then_idle_machine(*kinds)),
        _ => unreachable!("the hunt state"),
    }
    let sleep = h.logic_random_value(0, if plant(3330) { ENEMY_SCAN_RATE as i32 - 1 } else { ENEMY_SCAN_RATE as i32 }) as u32;
    let now = StateContext::frame(h);
    *scan_time(sm, id) = now.wrapping_add(sleep);
    run_sub_machine(sm, h, id, |m, h| m.init_default_state(h))
}

/// `onExit(status)`: the machine deleted (its state exits, `EXIT_RESET`); the hunt state's releases a temporary weapon lock.
pub fn hunt_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    run_sub_machine(sm, h, id, |m, h| {
        let dead = std::mem::replace(m, StateMachine::new());
        dead.destroy(h)
    })?;
    let area = match sm.state_mut(id) {
        Some(AiState::Hunt { machine, area, .. }) => {
            *machine = None;
            *area
        }
        _ => unreachable!("the hunt state"),
    };
    if !area && !plant(3331) {
        h.release_weapon_lock(LOCKED_TEMPORARILY as i32);
    }
    Ok(())
}

/// Sets the machine's goal object and, from idle with a victim, its attack state.
fn aim_machine<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, victim: ObjectId) -> SmResult<()> {
    let o = h.find_object(victim);
    run_sub_machine(sm, h, id, |m, h| {
        m.set_goal_object(o.as_ref());
        if m.get_current_state_id() == AI_IDLE && o.is_some() && !plant(3332) {
            m.set_state(h, AI_ATTACK_OBJECT)?;
        }
        Ok(())
    })
}

fn machine_idle<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> bool {
    match sm.state(id) {
        Some(AiState::Hunt { machine: Some(m), .. }) => m.get_current_state_id() == AI_IDLE,
        _ => unreachable!("the hunt machine"),
    }
}

/// `AIHuntState::update()`'s scan: the crate, else the victim (the closest enemy by the attack priorities, else any; a team's
/// common target when it ranks as high). `Some` ends the update.
fn hunt_scan<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, now: u32) -> SmResult<Option<StateReturn>> {
    // "Check to see if we have created a crate we need to pick up."
    let crate_id = with_ai(h, |a, h| a.check_for_crate_to_pickup(h));
    if crate_id != INVALID_ID {
        let o = h.find_object(crate_id);
        run_sub_machine(sm, h, id, |m, h| {
            m.set_goal_object(o.as_ref());
            m.set_state(h, AI_PICK_UP_CRATE).map(|_| ())
        })?;
        return Ok(Some(StateReturn::CONTINUE));
    }
    *scan_time(sm, id) = now.wrapping_add(if plant(3333) { ENEMY_SCAN_RATE + 1 } else { ENEMY_SCAN_RATE });
    let info = ai(h).attack_info;
    // "Check if team auto targets same victim."
    let common = h.team_attack_common_target();
    let team_victim = if common { h.team_target_object() } else { INVALID_ID };
    let mut victim;
    if team_victim != INVALID_ID && info.is_none() {
        victim = team_victim;
    } else {
        victim = h.find_closest_enemy(SCAN_RANGE, AI_CAN_ATTACK, info, None);
        if victim == INVALID_ID && h.units_should_hunt() == Some(true) && !plant(3334) {
            // "If we are doing an all hunt, try hunting without the attack priority info."
            victim = h.find_closest_enemy(SCAN_RANGE, AI_CAN_ATTACK, None, None);
        }
        if common {
            // "Check priorities."
            if let (true, Some(info)) = (team_victim != INVALID_ID, info) {
                if victim == INVALID_ID {
                    victim = team_victim;
                }
                let team_victim_priority = h.attack_priority(info, team_victim);
                let victim_priority = if victim != INVALID_ID { h.attack_priority(info, victim) } else { 0 };
                if if plant(3335) { team_victim_priority > victim_priority } else { team_victim_priority >= victim_priority } {
                    victim = team_victim;
                }
            }
            h.set_team_target_object(victim);
        }
    }
    aim_machine(sm, h, id, victim)?;
    // "If we are not doing an all hunt, then exit hunt state - no more victims."
    if h.units_should_hunt() == Some(false) && machine_idle(sm, id) && victim == INVALID_ID {
        return Ok(Some(StateReturn::SUCCESS)); // "we killed everything :) jba."
    }
    Ok(None)
}

/// `AIAttackAreaState::update()`'s scan: the closest enemy in the area (none: done).
fn area_scan<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, now: u32) -> SmResult<Option<StateReturn>> {
    *scan_time(sm, id) = now.wrapping_add(ENEMY_SCAN_RATE);
    let Some(area) = ai(h).area_to_guard else { return Ok(Some(StateReturn::FAILURE)) };
    let info = ai(h).attack_info;
    let victim = h.find_closest_enemy(SCAN_RANGE, AI_CAN_ATTACK, info, Some(area));
    aim_machine(sm, h, id, victim)?;
    if victim == INVALID_ID && !plant(3336) {
        return Ok(Some(StateReturn::SUCCESS));
    }
    Ok(None)
}

/// `update()`: a scan once a second ("look around for better victims every so often"; none when out of ammo: failure), then the
/// machine's update with the AI's machine locked ("The idle state in the sub machine can sometimes acquire targets. It is important
/// to not switch out of this state via a sub machine call."); a sleep is a continue.
pub fn hunt_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    let (area, next) = match sm.state_mut(id) {
        Some(AiState::Hunt { area, next_enemy_scan_time, .. }) => (*area, *next_enemy_scan_time),
        _ => unreachable!("the hunt state"),
    };
    if now >= next {
        // "if all of our weapons are out of ammo, can't hunt."
        if h.is_out_of_ammo() && !h.is_kind_of(KINDOF_PROJECTILE) {
            return Ok(StateReturn::FAILURE);
        }
        let end = if area { area_scan(sm, h, id, now)? } else { hunt_scan(sm, h, id, now)? };
        if let Some(r) = end {
            return Ok(r);
        }
    }
    if !plant(3337) {
        sm.lock();
    }
    let ret = run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h));
    sm.unlock();
    Ok(ret?.convert_sleep_to_continue())
}

/// `xfer` (version 1, both states'): whether there is a machine (one made to load into), the machine (its own version, then the
/// machine's), the scan time.
pub fn hunt_xfer<H: AiStateHost>(kinds: AttackOwnerKinds, machine: &mut Option<Sm<H>>, next_enemy_scan_time: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    if has_machine && machine.is_none() {
        *machine = Some(new_attack_then_idle_machine(kinds));
    }
    if let Some(m) = machine.as_mut().filter(|_| has_machine) {
        // AIAttackThenIdleStateMachine::xfer: its version, then the machine's
        let cv: XferVersion = 1;
        let mut v = cv;
        xfer.xfer_version(&mut v, cv)?;
        Snapshot::xfer(m, xfer)?;
    }
    xfer.xfer_unsigned_int(next_enemy_scan_time)
}
