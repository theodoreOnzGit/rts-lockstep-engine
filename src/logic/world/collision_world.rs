// SPDX-License-Identifier: GPL-3.0-only
//
// Part of the integration of AIUpdate.cpp's `processCollision` (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590) into the World;
// the collision functions themselves are `logic::ai::ai_collision`. Licensed under the GNU General Public License version 3
// (GPL-3.0-only for this translation), with the additional terms in NOTICE.md. Translated to Rust and modified; not the
// original program.
//! Integration batch I9c-4: the AI's collisions in the World ([`CollisionHost`] for World), live with `cuts.live_ai`.
//!
//! `PhysicsBehavior`'s collision asks its object's AI `processCollision(physics, other)` (`ModuleCall::AiProcessCollision`).
//! The two parties are [`World::ai_self`] ([`Party::Me`]) and [`World::collision_other`] ([`Party::Other`], set for the call).
//! Each party's AI is reached lent to the World ([`World::with_ai_lent`]): the asking AI's module is checked out while it
//! answers, and lends its AI and machine for the call as its turrets' run does. A command to the asking AI waits until the
//! call returns (see `party_command`).
//!
//! `p1w13-sonnet`, integration batch I9c-4, 2026-11-01.

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KindOfType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::state_machine::{StateId, StateMachine};
use crate::logic::ai::ai_collision::{CollisionHost, Party};
use crate::logic::ai::ai_states::AiState;
use crate::logic::ai::ai_update::{AiUpdate, LocoGoalType};
use crate::logic::ai::ai_update_types::CMD_FROM_AI;
use crate::logic::ai::ai_group::NO_FORMATION_ID;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_damage::CrushSquishTestType;

impl World {
    fn party(&self, p: Party) -> ObjectId {
        match p {
            Party::Me => self.ai_self,
            Party::Other => self.collision_other,
        }
    }

    /// `f` on the party's AI and its machine, lent to the World and taken off the stacks for the call; `None` without one.
    fn with_party_ai<R>(&mut self, p: Party, f: impl FnOnce(&mut AiUpdate, &StateMachine<AiState<World>>, &mut World) -> R) -> Option<R> {
        let id = self.party(p);
        let me = self.ai_self;
        let other = self.collision_other;
        self.with_ai_lent(id, |w| {
            let sm = w.ai_stacks.machine.pop().expect("the lent machine");
            let mut a = w.ai_stacks.ai.pop().expect("the lent AI");
            // (the parties as they were: `with_ai_lent` made this party `this`)
            let prev = (w.ai_self, w.collision_other);
            w.ai_self = id;
            w.collision_other = if id == me { other } else { me };
            let r = f(&mut a, &sm, w);
            (w.ai_self, w.collision_other) = prev;
            w.ai_stacks.ai.push(a);
            w.ai_stacks.machine.push(sm);
            r
        })
    }

    /// `processCollision(physics, other)` of `me`'s AI (batch I9c-4); false when `me` has no AI.
    pub(crate) fn ai_process_collision(&mut self, me: ObjectId, other: ObjectId) -> bool {
        let prev = std::mem::replace(&mut self.collision_other, other);
        let r = self.with_ai_self(me, crate::logic::ai::ai_collision::process_collision);
        self.collision_other = prev;
        r
    }
}

impl CollisionHost for World {
    fn object_id(&mut self, p: Party) -> ObjectId {
        self.party(p)
    }
    fn position(&mut self, p: Party) -> Coord3D {
        let id = self.party(p);
        self.find_object(id).map(|o| o.get_position()).unwrap_or_default()
    }
    fn unit_direction_2d(&mut self, p: Party) -> Coord3D {
        let id = self.party(p);
        self.find_object_mut(id).map_or_else(Coord3D::default, |o| o.thing.unit_direction_vector_2d())
    }
    fn is_kind_of(&mut self, p: Party, k: KindOfType) -> bool {
        let id = self.party(p);
        self.find_object(id).is_some_and(|o| o.is_kind_of(k))
    }
    fn test_status(&mut self, p: Party, s: ObjectStatusTypes) -> bool {
        let id = self.party(p);
        self.find_object(id).is_some_and(|o| o.test_status(s))
    }
    fn formation_id(&mut self, p: Party) -> u32 {
        let id = self.party(p);
        self.find_object(id).map_or(NO_FORMATION_ID, |o| o.formation_id as u32)
    }
    fn physics_velocity(&mut self, p: Party) -> Option<Coord3D> {
        let id = self.party(p);
        let ph = self.find_object(id)?.physics?;
        self.module_call(id, ph, ModuleCall::PhysicsGetVelocity).as_coord_opt()
    }
    fn can_crush_or_squish(&mut self, p: Party, victim: Party, test: CrushSquishTestType) -> bool {
        let (a, b) = (self.party(p), self.party(victim));
        World::can_crush_or_squish(self, a, Some(b), test)
    }
    fn relative_angle_2d(&mut self, p: Party, pos: &Coord3D) -> f32 {
        let id = self.party(p);
        self.partition_relative_angle_2d(id, pos)
    }
    fn frame(&mut self) -> u32 {
        crate::common::state_machine::StateContext::frame(self)
    }
    fn has_ai(&mut self, p: Party) -> bool {
        let id = self.party(p);
        self.find_object(id).is_some_and(|o| o.ai.is_some())
    }
    fn ignore_collisions_until(&mut self, p: Party) -> u32 {
        self.with_party_ai(p, |a, _, _| a.ignore_collisions_until).unwrap_or(0)
    }
    fn can_path_through_units(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, _, _| a.can_path_through_units).unwrap_or(false)
    }
    fn locomotor_goal_type(&mut self, p: Party) -> LocoGoalType {
        self.with_party_ai(p, |a, _, _| a.locomotor_goal_type).unwrap_or(LocoGoalType::None)
    }
    fn blocked_frames(&mut self, p: Party) -> i32 {
        self.with_party_ai(p, |a, _, _| a.blocked_frames).unwrap_or(0)
    }
    fn set_blocked_frames(&mut self, p: Party, v: i32) {
        self.with_party_ai(p, |a, _, _| a.blocked_frames = v);
    }
    fn cur_max_blocked_speed(&mut self, p: Party) -> f32 {
        self.with_party_ai(p, |a, _, _| a.cur_max_blocked_speed).unwrap_or(0.0)
    }
    fn set_cur_max_blocked_speed(&mut self, p: Party, v: f32) {
        self.with_party_ai(p, |a, _, _| a.cur_max_blocked_speed = v);
    }
    fn set_is_blocked(&mut self, p: Party, v: bool) {
        self.with_party_ai(p, |a, _, _| a.is_blocked = v);
    }
    fn set_is_blocked_and_stuck(&mut self, p: Party, v: bool) {
        self.with_party_ai(p, |a, _, _| a.is_blocked_and_stuck = v);
    }
    fn machine_goal_position(&mut self, p: Party) -> Coord3D {
        self.with_party_ai(p, |_, sm, _| *sm.goal_position()).unwrap_or_default()
    }
    fn pathfind_goal_cell(&mut self, p: Party) -> (i32, i32) {
        self.with_party_ai(p, |a, _, _| (a.pathfind_goal_cell.x, a.pathfind_goal_cell.y)).unwrap_or((0, 0))
    }
    fn is_moving(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, sm, w| a.is_moving(sm, w)).unwrap_or(false)
    }
    fn is_doing_ground_movement(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, _, w| a.is_doing_ground_movement(w)).unwrap_or(false)
    }
    fn is_waiting_for_path(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, _, _| a.is_waiting_for_path()).unwrap_or(false)
    }
    fn is_moving_away_from(&mut self, p: Party, from: Party) -> bool {
        let from = self.party(from);
        self.with_party_ai(p, |a, _, _| a.is_moving_away_from(from)).unwrap_or(false)
    }
    fn is_busy(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, sm, _| a.is_busy(sm)).unwrap_or(false)
    }
    fn is_idle(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, sm, w| a.is_idle(sm, w)).unwrap_or(false)
    }
    fn is_ai_in_dead_state(&mut self, p: Party) -> bool {
        self.with_party_ai(p, |a, _, _| a.is_ai_in_dead_state()).unwrap_or(false)
    }
    fn current_state_id(&mut self, p: Party) -> StateId {
        self.with_party_ai(p, |_, sm, _| sm.get_current_state_id()).unwrap_or_default()
    }
    fn cur_locomotor_moving_backwards(&mut self, p: Party) -> Option<bool> {
        self.with_party_ai(p, |a, _, _| a.get_cur_locomotor().map(|l| l.is_moving_backwards())).flatten()
    }
    fn cur_locomotor_wander_width_factor(&mut self, p: Party) -> Option<f32> {
        self.with_party_ai(p, |a, _, _| a.get_cur_locomotor().map(|l| l.get_wander_width_factor())).flatten()
    }
    fn compute_point_on_path(&mut self, p: Party) -> Option<Coord3D> {
        let id = self.party(p);
        let pos = self.find_object(id)?.get_position();
        self.with_party_ai(p, |a, _, w| {
            let surfaces = a.locomotor_set.get_valid_surfaces();
            let mut path = a.path.take()?;
            let r = path.compute_point_on_path(w, id, surfaces, &pos).pos_on_path;
            a.path = Some(path);
            Some(r)
        })
        .flatten()
    }
    fn ai_move_away_from_unit(&mut self, p: Party, from: Party) {
        let from = self.party(from);
        let id = self.party(p);
        let mut parms = crate::logic::ai::ai_commands::AiCommandParms::new(crate::logic::ai::ai_commands::AICMD_MOVE_AWAY_FROM_UNIT, CMD_FROM_AI);
        parms.obj = from;
        self.party_command(id, parms);
    }
    fn adjust_to_possible_destination(&mut self, p: Party, pos: &mut Coord3D) {
        let id = self.party(p);
        let _ = self.pathfinder_adjust_to_possible_destination(id, pos);
    }
    fn ai_move_to_position(&mut self, p: Party, pos: &Coord3D) {
        let id = self.party(p);
        let mut parms = crate::logic::ai::ai_commands::AiCommandParms::new(crate::logic::ai::ai_commands::AICMD_MOVE_TO_POSITION, CMD_FROM_AI);
        parms.pos = *pos;
        self.party_command(id, parms);
    }
}

impl World {
    /// `aiDoCommand(&parms)` on a party's AI: the other party's through its module (the whole AI, its subclass's machines
    /// included); the asking AI's, whose module is the one checked out, waits until `processCollision` returns and runs then
    /// through that module (`collision_self_commands`). Upstream runs it in place; the commands are the collision's last acts.
    fn party_command(&mut self, id: ObjectId, parms: crate::logic::ai::ai_commands::AiCommandParms) {
        if id == self.ai_self {
            self.collision_self_commands.push(parms);
        } else {
            self.ai_ask(id, ModuleCall::AiDoCommandParms(parms));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::common::base_type::Coord3D;
    use crate::logic::world::module_call::ModuleCall;
    use crate::logic::world::pathfinder_host_world::PfOp;
    use crate::logic::world::player_host_world::tests::{make, world};

    /// Two runners driving head-on along one line (from x 50 and x 350 at y 200, each to the other's start). Their physics
    /// collide and ask their AIs (`processCollision`); after 400 frames each has passed the middle, with no live error.
    #[test]
    fn colliding_runners_get_past_each_other() {
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        w.cuts.live_pathfinder = true;
        let a = make(&mut w, b"Runner", team, Coord3D { x: 50.0, y: 200.0, z: 0.0 });
        let b = make(&mut w, b"Runner", team, Coord3D { x: 350.0, y: 200.0, z: 0.0 });
        assert!(w.pathfinder_op(PfOp::NewMap));
        for (r, x) in [(a, 350.0), (b, 50.0)] {
            let index = w.find_object(r).unwrap().ai.unwrap();
            w.module_call(r, index, ModuleCall::AiMoveToPosition { pos: Coord3D { x, y: 200.0, z: 0.0 }, command_source: 0 });
        }
        w.cuts.trace = Some(Vec::new());
        for _ in 0..400 {
            w.logic_update().unwrap();
        }
        let collisions = w.cuts.trace.as_ref().unwrap().iter().filter(|t| t.as_str() == "mod AiProcessCollision").count();
        let (pa, pb) = (w.find_object(a).unwrap().get_position(), w.find_object(b).unwrap().get_position());
        assert!(pa.x > 200.0 && pb.x < 200.0, "a at {pa:?}, b at {pb:?} ({collisions} collisions asked)");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        eprintln!("collisions asked of the AIs: {collisions}");
    }
}
