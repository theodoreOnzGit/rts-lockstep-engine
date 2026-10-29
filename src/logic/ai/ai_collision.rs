// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (AIUpdateInterface::hasHigherPathPriority,
//     calculateMaxBlockedSpeed, blockedBy, needToRotate, processCollision)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's collisions with another AI's object (`p1w13-sonnet`, layer 10, batch A22).
//!
//! `PhysicsBehavior`'s collision asks the AI `processCollision(physics, other)`: whether to apply the push (normally not), and,
//! while moving, whether the other object blocks it (`blockedBy`), how fast it may then go (`calculateMaxBlockedSpeed`), whether it
//! is stuck (`needToRotate`), who gives way (`hasHigherPathPriority`), with the other AI told to move away, or both to move to a
//! possible position when they stand on each other.
//!
//! Two AIs take part, so each function names its party: [`Party::Me`] (the AI asked) or [`Party::Other`]; a member called on the
//! other AI (`aiOther->blockedBy(getObject())`) is the same function with the parties swapped ([`Party::swap`]). The host
//! ([`CollisionHost`]) answers for either party: the object's facts, the AI's members these functions read and write, the AI's
//! queries (`isMoving`, `isIdle`, ...; their own batches'), and the commands (`aiMoveAwayFromUnit`, `aiMoveToPosition`).
//!
//! # Upstream facts kept
//!
//! - `blockedBy` asks `aiOther->isDoingGroundMovement()` before checking `aiOther` for null (`processCollision` only calls it with
//!   an AI on the other side).
//! - `processCollision`'s `otherMoving` checks `aiOther` again after it returned for a null one.
//! - `INFANTRY_MOVES_THROUGH_INFANTRY` and `MOVE_AROUND` are not defined; `DO_UNIT_TIMINGS` neither.
//! - The tie on the same spot uses `PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.0001f`; the dead-infantry push, `0.25f`.

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KindOfType, KINDOF_DOZER, KINDOF_INFANTRY, KINDOF_VEHICLE};
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_IS_USING_ABILITY};
use crate::common::state_machine::StateId;
use crate::logic::ai::ai_update::LocoGoalType;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;
use crate::logic::world::object_damage::CrushSquishTestType;

/// One of the two AIs of a collision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Party {
    /// The AI asked (`this`).
    Me,
    /// The other object's AI (`aiOther`).
    Other,
}

impl Party {
    /// The other party.
    pub fn swap(self) -> Party {
        match self {
            Party::Me => Party::Other,
            Party::Other => Party::Me,
        }
    }
}

/// What the collision functions ask of either party.
pub trait CollisionHost {
    // ---- the object
    fn object_id(&mut self, p: Party) -> ObjectId;
    fn position(&mut self, p: Party) -> Coord3D;
    /// `getUnitDirectionVector2D()`.
    fn unit_direction_2d(&mut self, p: Party) -> Coord3D;
    fn is_kind_of(&mut self, p: Party, k: KindOfType) -> bool;
    fn test_status(&mut self, p: Party, s: ObjectStatusTypes) -> bool;
    /// `getFormationID()` (`NO_FORMATION_ID`: 0).
    fn formation_id(&mut self, p: Party) -> u32;
    /// `getPhysics()->getVelocity()` (`None`: no physics).
    fn physics_velocity(&mut self, p: Party) -> Option<Coord3D>;
    /// `p->canCrushOrSquish(victim's object, test)`.
    fn can_crush_or_squish(&mut self, p: Party, victim: Party, test: CrushSquishTestType) -> bool;
    /// `ThePartitionManager->getRelativeAngle2D(p's object, pos)`.
    fn relative_angle_2d(&mut self, p: Party, pos: &Coord3D) -> f32;
    /// `TheGameLogic->getFrame()`.
    fn frame(&mut self) -> u32;
    // ---- the AI's members
    /// `getObject()->getAI() != NULL` of the other object (only asked of [`Party::Other`]).
    fn has_ai(&mut self, p: Party) -> bool;
    fn ignore_collisions_until(&mut self, p: Party) -> u32;
    fn can_path_through_units(&mut self, p: Party) -> bool;
    fn locomotor_goal_type(&mut self, p: Party) -> LocoGoalType;
    fn blocked_frames(&mut self, p: Party) -> i32;
    fn set_blocked_frames(&mut self, p: Party, v: i32);
    fn cur_max_blocked_speed(&mut self, p: Party) -> f32;
    fn set_cur_max_blocked_speed(&mut self, p: Party, v: f32);
    fn set_is_blocked(&mut self, p: Party, v: bool);
    fn set_is_blocked_and_stuck(&mut self, p: Party, v: bool);
    /// `getStateMachine()->getGoalPosition()`; `*getPathfindGoalCell()` as `(x, y)`.
    fn machine_goal_position(&mut self, p: Party) -> Coord3D;
    fn pathfind_goal_cell(&mut self, p: Party) -> (i32, i32);
    // ---- the AI's queries (their own batches')
    fn is_moving(&mut self, p: Party) -> bool;
    fn is_doing_ground_movement(&mut self, p: Party) -> bool;
    fn is_waiting_for_path(&mut self, p: Party) -> bool;
    fn is_moving_away_from(&mut self, p: Party, from: Party) -> bool;
    fn is_busy(&mut self, p: Party) -> bool;
    fn is_idle(&mut self, p: Party) -> bool;
    fn is_ai_in_dead_state(&mut self, p: Party) -> bool;
    fn current_state_id(&mut self, p: Party) -> StateId;
    /// `getCurLocomotor()`'s `isMovingBackwards()` and `getWanderWidthFactor()` (`None`: no current locomotor).
    fn cur_locomotor_moving_backwards(&mut self, p: Party) -> Option<bool>;
    fn cur_locomotor_wander_width_factor(&mut self, p: Party) -> Option<f32>;
    /// `getPath()->computePointOnPath(getObject(), m_locomotorSet, *getObject()->getPosition(), info)`'s `posOnPath` (`None`: no
    /// path).
    fn compute_point_on_path(&mut self, p: Party) -> Option<Coord3D>;
    // ---- the commands
    /// `aiMoveAwayFromUnit(from's object, CMD_FROM_AI)`.
    fn ai_move_away_from_unit(&mut self, p: Party, from: Party);
    /// `TheAI->pathfinder()->adjustToPossibleDestination(p's object, p's locomotor set, pos)`, then `aiMoveToPosition(pos,
    /// CMD_FROM_AI)`.
    fn adjust_to_possible_destination(&mut self, p: Party, pos: &mut Coord3D);
    fn ai_move_to_position(&mut self, p: Party, pos: &Coord3D);
}

const PI: f32 = std::f32::consts::PI;

/// `p->hasHigherPathPriority(q's AI)`: dozers first, vehicles over infantry; crossing: the lower id; else the one ahead (a tie:
/// the lower id).
pub fn has_higher_path_priority<H: CollisionHost>(h: &mut H, p: Party) -> bool {
    let q = p.swap();
    // "Dozers have highest priority."
    let (pd, qd) = (h.is_kind_of(p, KINDOF_DOZER), h.is_kind_of(q, KINDOF_DOZER));
    if pd && !qd {
        return true;
    }
    if !pd && qd {
        return false;
    }
    // "Vehicles always have higher priority than infantry."
    if h.is_kind_of(p, KINDOF_VEHICLE) && h.is_kind_of(q, KINDOF_INFANTRY) {
        return true;
    }
    if h.is_kind_of(p, KINDOF_INFANTRY) && h.is_kind_of(q, KINDOF_VEHICLE) {
        return false;
    }
    // "The paths aren't of the same group, so see which unit is in front."
    let our_dir = h.unit_direction_2d(p);
    let other_dir = h.unit_direction_2d(q);
    let (pid, qid) = (h.object_id(p), h.object_id(q));
    if our_dir.x * other_dir.x + our_dir.y * other_dir.y <= 0.0 {
        return pid < qid;
    }
    let combined = Coord2D { x: our_dir.x + other_dir.x, y: our_dir.y + other_dir.y };
    let (pp, qp) = (h.position(p), h.position(q));
    let to_other = Coord2D { x: qp.x - pp.x, y: qp.y - pp.y };
    let dot = combined.x * to_other.x + combined.y * to_other.y;
    if dot > 0.0 {
        return false; // "other is ahead of us along our directional vector."
    }
    if dot < 0.0 && !plant(3510) {
        return true; // "We are ahead of other."
    }
    // "Exactly equal. Use object id's to break the tie."
    pid < qid
}

/// `p->calculateMaxBlockedSpeed(q's object)`: "max speed we can have and not run into unit that is blocking us".
pub fn calculate_max_blocked_speed<H: CollisionHost>(h: &mut H, p: Party) -> f32 {
    let q = p.swap();
    let our_dir = h.unit_direction_2d(p);
    let other_dir = h.unit_direction_2d(q);
    let (pp, qp) = (h.position(p), h.position(q));
    let mut to_other = Coord2D { x: qp.x - pp.x, y: qp.y - pp.y };
    to_other.normalize();
    let dot = to_other.x * other_dir.x + to_other.y * other_dir.y;
    if dot < 0.0 {
        return 0.0; // "They are running into us."
    }
    let speed_factor = dot;
    let cur_max = h.cur_max_blocked_speed(p);
    let Some(mut other_vel) = h.physics_velocity(q) else {
        return cur_max;
    };
    other_vel.z = 0.0;
    // "Calculate how fast other is moving away from us..."
    let away_speed = other_vel.length() * speed_factor;
    // "Now calculate the amount we are moving relative to towards them..."
    let dot = to_other.x * our_dir.x + to_other.y * our_dir.y;
    if dot <= 0.0 {
        // "Unexpected - we are moving away. Shouldn't be blocked..."
        return cur_max;
    }
    let mut max_speed = away_speed / dot;
    let (qf, pf) = (h.formation_id(q), h.formation_id(p));
    if qf != NO_FORMATION_ID && pf == qf && !plant(3511) {
        max_speed *= 0.55; // "don't let formations crowd each other."
    }
    if max_speed > cur_max {
        return cur_max;
    }
    max_speed
}

/// `NO_FORMATION_ID`.
pub const NO_FORMATION_ID: u32 = 0;

/// `p->blockedBy(q's object)`: whether `p` is blocked from moving by the other object.
// (upstream's `a > b || a < -b` comparisons kept: a range's `contains` would answer otherwise for a NaN angle)
#[allow(clippy::manual_range_contains)]
pub fn blocked_by<H: CollisionHost>(h: &mut H, p: Party) -> bool {
    let q = p.swap();
    let goal_pos = h.machine_goal_position(p);
    let pos = h.position(p);
    let goal_cell = h.pathfind_goal_cell(p);
    // "If we are near our final goal, don't get stuck."
    if goal_cell.0 > 0 && goal_cell.1 > 0 {
        let dx = (goal_pos.x - pos.x).abs();
        let dy = (goal_pos.y - pos.y).abs();
        if dx < PATHFIND_CELL_SIZE_F && dy < PATHFIND_CELL_SIZE_F {
            return false; // "If we're approaching our goal, ignore obstacles."
        }
    }
    if h.can_crush_or_squish(p, q, CrushSquishTestType::CrushOrSquish) {
        return false; // "just run over them."
    }
    // (aiOther's ground movement asked before aiOther is checked: always one here)
    if !h.is_doing_ground_movement(q) {
        return false; // "Can't be blocked if the other is airborne."
    }
    if h.cur_locomotor_moving_backwards(p) == Some(true) && !plant(3512) {
        return false; // "don't collide."
    }
    let other_moving = h.locomotor_goal_type(q) != LocoGoalType::None;
    let other_pos = h.position(q);
    let mut dx = pos.x - other_pos.x;
    let mut dy = pos.y - other_pos.y;
    let cur_d_sqr = dx * dx + dy * dy;
    if h.is_kind_of(p, KINDOF_INFANTRY) && h.is_kind_of(q, KINDOF_INFANTRY) {
        // "Infantry doesn't tend to impede other infantry..." "If we are crossing, just pass through."
        let our_dir = h.unit_direction_2d(p);
        let their_dir = h.unit_direction_2d(q);
        let dot = our_dir.x * their_dir.x + our_dir.y * their_dir.y;
        if dot <= 0.25 {
            return false; // "we are not moving in the same direction."
        }
    }
    if cur_d_sqr < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.0001 {
        // "Somehow 2 units ended up on the same grid." "Lowest path priority wins."
        return has_higher_path_priority(h, p);
    }
    // "we've been blocked for a while. If we're crossing, just move through."
    let our_dir = h.unit_direction_2d(p);
    let their_dir = h.unit_direction_2d(q);
    let dot = our_dir.x * their_dir.x + our_dir.y * their_dir.y;
    if h.blocked_frames(p) > LOGICFRAMES_PER_SECOND && dot <= 0.0 {
        return false; // "we are not moving in the same direction."
    }
    let collision_angle = h.relative_angle_2d(p, &other_pos);
    let other_angle = h.relative_angle_2d(q, &pos);
    let mut angle_limit = PI / 4.0; // "45 degrees."
    if collision_angle > PI / 2.0 || collision_angle < -PI / 2.0 {
        return false; // "we're moving away."
    }
    if !other_moving {
        angle_limit *= 0.75;
    }
    if collision_angle > angle_limit || collision_angle < -angle_limit {
        if dot <= 0.0 {
            return false; // "we are not moving in the same direction."
        }
        if other_moving && (other_angle > angle_limit || other_angle < -angle_limit) {
            // "See if we're running into each other."
            let our_dir = h.unit_direction_2d(p);
            let their_dir = h.unit_direction_2d(q);
            dx += our_dir.x - their_dir.x;
            dy += our_dir.y - their_dir.y;
            if cur_d_sqr > dx * dx + dy * dy {
                if has_higher_path_priority(h, p) {
                    return false; // "Lowest path priority wins."
                }
            } else {
                return false; // "moving away, so no need for corrective action."
            }
        } else {
            return false; // "Off angle, and they're not moving, so we aren't moving into each other."
        }
    }
    !h.is_ai_in_dead_state(q)
}

/// `p->needToRotate()`: waiting for a path (a new one will want it), not a wanderer, and the path's point more than 6 degrees off.
pub fn need_to_rotate<H: CollisionHost>(h: &mut H, p: Party) -> bool {
    if h.is_waiting_for_path(p) {
        return true; // "new path will probably require rotation."
    }
    if h.cur_locomotor_wander_width_factor(p).is_some_and(|w| w > 0.0) {
        return false; // "wanderers don't need to rotate."
    }
    let mut delta_angle = 0.0f32;
    if let Some(pos_on_path) = h.compute_point_on_path(p) {
        delta_angle = h.relative_angle_2d(p, &pos_on_path);
    }
    delta_angle.abs() > PI / 30.0 && !plant(3513)
}

/// `AIUpdateInterface::processCollision(physics, other)`: whether the physics' collision should apply its push.
pub fn process_collision<H: CollisionHost>(h: &mut H) -> bool {
    let (me, other) = (Party::Me, Party::Other);
    if h.ignore_collisions_until(me) > h.frame() {
        return false;
    }
    if h.can_path_through_units(me) {
        return false;
    }
    if !h.has_ai(other) {
        return false;
    }
    let self_moving = h.is_moving(me);
    let other_moving = h.is_moving(other);
    if !h.is_doing_ground_movement(me) {
        return false;
    }
    if !h.is_doing_ground_movement(other) {
        return false;
    }
    if self_moving {
        if blocked_by(h, me) {
            if h.is_kind_of(me, KINDOF_INFANTRY) && h.current_state_id(me) == AI_PANIC as StateId {
                return true; // "just bounce off of other humans."
            }
            h.set_is_blocked(me, true); // "we are blocked."
            if other_moving && h.is_waiting_for_path(other) {
                return false; // "let them get their path;"
            }
            let max_speed = calculate_max_blocked_speed(h, me);
            if max_speed < h.cur_max_blocked_speed(me) {
                h.set_cur_max_blocked_speed(me, max_speed);
            }
            if !h.is_moving_away_from(other, me)
                && h.is_kind_of(other, KINDOF_INFANTRY)
                && !h.is_kind_of(me, KINDOF_INFANTRY)
            {
                // "Prevent busy units from being told to move out of the way!"
                if h.test_status(other, OBJECT_STATUS_IS_USING_ABILITY) || (h.has_ai(other) && h.is_busy(other)) {
                    return false;
                }
                if !plant(3514) {
                    h.ai_move_away_from_unit(other, me);
                }
                return false;
            }
            if h.blocked_frames(me) == 0 {
                h.set_blocked_frames(me, 1);
            }
            if !need_to_rotate(h, me) {
                // "If we are already pointing in the right direction, we may be stuck."
                if !other_moving {
                    h.set_is_blocked_and_stuck(me, true);
                    return false;
                }
                // "See if other is blocked by us."
                if blocked_by(h, other) && !need_to_rotate(h, other) && !has_higher_path_priority(h, me) {
                    // "Deadlocked." "get out of his way."
                    h.ai_move_away_from_unit(me, other);
                }
                // (else "Just wait.")
            } else {
                // "We are rotating, so don't accumulate blocked frames."
                h.set_blocked_frames(me, 1);
            }
        }
    } else {
        if h.is_ai_in_dead_state(me) {
            // "Dead infantry get pushed around by crushers."
            if h.is_kind_of(me, KINDOF_INFANTRY) && h.can_crush_or_squish(other, me, CrushSquishTestType::SquishOnly) {
                return true;
            }
        }
        let other_pos = h.position(other);
        let my_pos = h.position(me);
        let dx = my_pos.x - other_pos.x;
        let dy = my_pos.y - other_pos.y;
        let cur_d_sqr = dx * dx + dy * dy;
        if !other_moving && cur_d_sqr < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.25 {
            if h.current_state_id(me) == AI_BUSY as StateId {
                return false;
            }
            if h.test_status(me, OBJECT_STATUS_IS_USING_ABILITY) {
                return false; // "we are doing a special ability. Shouldn't move at this time."
            }
            if h.is_idle(me) {
                let mut safe = my_pos;
                h.adjust_to_possible_destination(me, &mut safe);
                h.ai_move_to_position(me, &safe);
            }
            if h.is_idle(other) && !plant(3515) {
                let mut pos = other_pos;
                h.adjust_to_possible_destination(other, &mut pos);
                h.ai_move_to_position(other, &pos);
            }
        }
    }
    false
}
