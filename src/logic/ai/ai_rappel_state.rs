// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (findEnemyInContainer, killEnemiesInContainer, AIRappelState)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (that class)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's rappel state (`p1w13-sonnet`, layer 10, batch A15).
//!
//! `AIRappelState` (`AI_RAPPEL_INTO`) slides the owner down a rope at a rate capped by gravity, to the goal building's roof (a
//! live structure) or the ground. On a building it kills up to two enemies inside (both killed: it dies too), then enters, or,
//! with no room, is put on the ground beside it and walks to a free position.
//!
//! # Upstream facts kept
//!
//! - The physics is used unchecked (`getPhysics()->resetDynamicPhysics()`, the velocity scrubs): not generated without it.
//! - The rate is negative: `-min(desired speed, |gravity| * 30 * 2.5)`.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND, PI};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_CAN_RAPPEL, KINDOF_STRUCTURE};
use crate::common::model_state::MODELCONDITION_RAPPELLING;
use crate::common::state_machine::{SmResult, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands;
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `MAX_TO_KILL`: "if there are enemies... kill up to two. if we kill two, then we die ourselves, otherwise we enter the bldg."
const MAX_TO_KILL: i32 = 2;

/// The rappel state's members: `m_rappelRate`, `m_destZ`, `m_targetIsBldg`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RappelData {
    pub rappel_rate: f32,
    pub dest_z: f32,
    pub target_is_bldg: bool,
}

fn data<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut RappelData {
    match sm.state_mut(id) {
        Some(AiState::Rappel(d)) => d,
        _ => unreachable!("the rappel state"),
    }
}

/// `findEnemyInContainer(killer, bldg)`: the first live one the owner considers an enemy ("order matters").
fn find_enemy_in_container<H: AiStateHost>(h: &mut H, bldg: ObjectId) -> ObjectId {
    if !h.object_has_contain(bldg) {
        return INVALID_ID;
    }
    for it in h.contain_items(bldg) {
        if h.object_is_effectively_dead(it) {
            // ("why is there a dead thing in this container?")
            continue;
        }
        if h.object_relationship(it) == Relationship::Enemies {
            return it;
        }
    }
    INVALID_ID
}

/// `killEnemiesInContainer(killer, bldg, maxToKill)`: each removed from its container, scored and killed.
fn kill_enemies_in_container<H: AiStateHost>(h: &mut H, bldg: ObjectId, max_to_kill: i32) -> i32 {
    let mut num_killed = 0;
    while num_killed < max_to_kill {
        let enemy = find_enemy_in_container(h, bldg);
        if enemy == INVALID_ID {
            break;
        }
        let container = h.object_contained_by(enemy);
        if container != INVALID_ID && h.object_has_contain(container) {
            h.contain_remove_from_contain(container, enemy);
        }
        h.score_the_kill(enemy);
        h.object_kill(enemy);
        num_killed += 1;
    }
    num_killed
}

/// `onEnter()`: only for those who rappel; rappelling, the dynamic physics reset; the destination the building's roof (a live
/// structure) or the highest healthy layer's height (the owner put on that layer); the rate.
pub fn rappel_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if !h.is_kind_of(KINDOF_CAN_RAPPEL) {
        return Ok(StateReturn::FAILURE);
    }
    h.set_model_condition_state(MODELCONDITION_RAPPELLING);
    // "don't do this, or we'll be unable to be forced out of this state. instead, just manipulate physics directly."
    h.physics_reset_dynamic();
    let bldg = sm.goal_object(h).map(|o| o.id);
    let target_is_bldg = match bldg {
        Some(b) => !h.object_is_effectively_dead(b) && h.object_is_kind_of(b, KINDOF_STRUCTURE),
        None => false,
    };
    let pos = h.position();
    // "ignore dead bridges."
    let layer_at_dest = h.highest_layer_for_destination(&pos);
    let mut dest_z = h.layer_height(pos.x, pos.y, layer_at_dest);
    if target_is_bldg {
        dest_z += h.object_max_height_above_position(bldg.expect("the building"));
    } else {
        h.object_set_layer(layer_at_dest);
    }
    let max_rappel_rate = h.gravity().abs() * LOGICFRAMES_PER_SECOND as f32 * if plant(3370) { 2.0 } else { 2.5 };
    let rate = -ai(h).desired_speed.min(max_rappel_rate);
    *data(sm, id) = RappelData { rappel_rate: rate, dest_z, target_is_bldg };
    Ok(StateReturn::CONTINUE)
}

/// `update()`: down the rope (no 2-D speed, the fall clamped to the rate) to the destination; there, on a building, the kills and
/// the entry (or, no room, beside it and away).
pub fn rappel_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut result = StateReturn::CONTINUE;
    let pos = h.position();
    let bldg = sm.goal_object(h).map(|o| o.id);
    let mut d = *data(sm, id);
    if d.target_is_bldg && bldg.is_none_or(|b| h.object_is_effectively_dead(b)) {
        // "if bldg is destroyed, just head for the ground"
        d.target_is_bldg = false;
        d.dest_z = h.ground_height(pos.x, pos.y);
    }
    // "nuke 2d speed..."
    h.scrub_velocity_2d(0.0);
    // "and clamp z speed to rappel rate (gravity will have accelerated us)"
    h.physics_scrub_velocity_z(d.rappel_rate);
    if !d.target_is_bldg && !plant(3371) {
        // "if heading for ground, do this every frame"
        let layer = h.layer();
        d.dest_z = h.layer_height(pos.x, pos.y, layer);
    }
    *data(sm, id) = d;
    if pos.z <= d.dest_z {
        h.set_position(&Coord3D { z: d.dest_z, ..pos });
        if d.target_is_bldg {
            let bldg = bldg.expect("the building");
            let num_killed = kill_enemies_in_container(h, bldg, if plant(3372) { 1 } else { MAX_TO_KILL });
            if num_killed > 0 {
                h.do_fx_combat_drop_kill(bldg);
            }
            if num_killed == MAX_TO_KILL {
                h.kill_self();
            } else if h.object_has_contain(bldg) && h.contain_is_valid_container_for_me(bldg) {
                h.contain_add_to_contain(bldg);
            } else {
                // "this can legitimately happen if you drop into a full (or nearly full) building."
                let exit_angle = h.object_orientation(bldg);
                let offset = h.bounding_circle_radius().min(h.object_bounding_circle_radius(bldg));
                // "Downish."
                let angle = h.logic_random_value_real(PI, 2.0 * PI);
                let mut start = h.object_position(bldg);
                start.x += offset * angle.cos();
                start.y += offset * angle.sin();
                start.z = h.ground_height(start.x, start.y);
                h.set_position(&start);
                h.set_orientation(exit_angle);
                // (options: startAngle 1.5 PI, "Down."; maxRadius 200; the stand-in search logs the radii only)
                if let Some(end) = h.find_position_around_no_source(&start, 0.0, 200.0) {
                    let path = [end];
                    ai_commands::ai_follow_path(sm, h, &path, bldg, CMD_FROM_AI)?;
                }
            }
        }
        result = StateReturn::SUCCESS;
    }
    Ok(result)
}

/// `onExit(status)`: no longer rappelling; the desired speed as fast as possible.
pub fn rappel_on_exit<H: AiStateHost>(h: &mut H) {
    h.clear_model_condition_state(MODELCONDITION_RAPPELLING);
    if !plant(3373) {
        ai(h).desired_speed = crate::logic::ai::ai_update_types::FAST_AS_POSSIBLE;
    }
}

/// `AIRappelState::xfer` (version 1): the rate, the destination height, whether it is a building.
pub fn rappel_xfer(d: &mut RappelData, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_real(&mut d.rappel_rate)?;
    xfer.xfer_real(&mut d.dest_z)?;
    xfer.xfer_bool(&mut d.target_is_bldg)
}
