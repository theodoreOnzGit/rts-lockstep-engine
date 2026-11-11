// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (`m_turretAI[]`: the AI's turrets, made by its
//     constructor, and the members that forward to them)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp (`getRelativeAngle2D`, 3548-3594)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (`reactToTurretChange`: its rotation, 1744-1760)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I9b-6: the turrets' AIs (`TurretAI`) of the World's AI modules, and the World as their host
//! ([`TurretHost`]).
//!
//! **Where a turret lives.** Upstream an `AIUpdateInterface` owns its `m_turretAI[2]`; here the World keeps them by owner
//! ([`TurretAis`]), because the AI reaches them only through its host (`AiStateHost::turret_*`), and the AI module is checked
//! out of its object while it runs. [`AiModuleBody::world_construct`](crate::logic::world::ai_module_world::AiModuleBody)
//! makes them after the AI (see the notes there on the order). A turret runs checked out of the table
//! ([`World::with_turret`]), with its owner as [`World::ai_self`]; while its machine runs its fields are on
//! `ai_stacks.turret` ([`TurretHost::turret_stack`]), where [`World::turret_peek`] reads them.
//!
//! **What a turret asks of its AI** (the mood check, the goal object, the next mood target, the sync flag) is answered from
//! the AI module in its object, or, while the AI is lent to the World (its machine's run, and the turret updates, for which
//! `AIUpdateInterface::update` lends the AI and its machine since this batch), from the top of `ai_stacks`. An AI lent deeper
//! than the top (an AI whose run reaches another's) is not found: its answers are the defaults (none so far).
//!
//! **Cut points:** the rotation sound (`TurretMoveLoop`) is the client's audio: its handle comes from the audio cut table.
//! The model condition and the container's reaction to the turning are the object's (live).
//!
//! `p1w13-sonnet`, integration batch I9b-6, 2026-11-01.

use std::collections::BTreeMap;

use crate::common::base_type::Coord3D;
use crate::common::game_common::{CommandSourceType, WhichTurretType, TURRET_INVALID};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::model_state::ModelConditionFlagType;
use crate::common::state_machine::StateMachine;
use crate::common::trig;
use crate::logic::ai::ai_states::AiState;
use crate::logic::ai::ai_update::{AiSubclass, AiUpdate};
use crate::logic::ai::turret::{TurretAi, TurretData, TurretHost};
use crate::logic::weapon_host::CanAttackResult;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::TeamId;
use crate::logic::world::module_call::ModuleHandle;
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::update_body::UpdateBody;

/// The turrets' AIs of the World's AI modules, by owner (`m_turretAI[MAX_TURRETS]`), and the turrets running now (checked
/// out of the table), the innermost last. A copy of the World copies no turret (their machines' states have no `Clone`), as
/// it copies no running AI.
#[derive(Default)]
pub struct TurretAis {
    pub map: BTreeMap<ObjectId, [Option<TurretAi<World>>; 2]>,
    pub running: Vec<(ObjectId, usize)>,
}

impl Clone for TurretAis {
    fn clone(&self) -> Self {
        TurretAis::default()
    }
}

impl std::fmt::Debug for TurretAis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TurretAis {{ owners: {}, running: {} }}", self.map.len(), self.running.len())
    }
}

impl World {
    /// `ThePartitionManager->getRelativeAngle2D(obj, pos)`: the signed angle from the object's 2-D direction to the position
    /// (0 at the object's own position); the cosine clamped, `ACos` the logic's table (`common::trig`).
    pub fn partition_relative_angle_2d(&self, obj: ObjectId, pos: &Coord3D) -> f32 {
        let Some(o) = self.objects.find(obj) else { return 0.0 };
        let p = o.get_position();
        let mut v = Coord3D { x: pos.x - p.x, y: pos.y - p.y, z: 0.0 };
        let dist = petir::Real::sqrt(v.x * v.x + v.y * v.y);
        if dist == 0.0 {
            return 0.0;
        }
        // (`getUnitDirectionVector2D()`: the cache's value, `(cos a, sin a)` of the orientation)
        let a = o.thing.orientation();
        let dir = Coord3D { x: trig::cos(a), y: trig::sin(a), z: 0.0 };
        let dist_inv = 1.0 / dist;
        v.x *= dist_inv;
        v.y *= dist_inv;
        let c = (dir.x * v.x + dir.y * v.y).clamp(-1.0, 1.0);
        let mut value = trig::acos(c);
        // "Determine sign by checking Z component of dir cross v"
        if dir.x * v.y - dir.y * v.x < 0.0 {
            value = -value;
        }
        value
    }

    /// Runs `f` on turret `which` of `id`, checked out of the table, with `id` as the AI's `this`; `None` when it has none
    /// (or it is running already: a re-entry, recorded in `team_live_errors`). A turret whose owner left the table while it
    /// ran is dropped.
    pub fn with_turret<R>(&mut self, id: ObjectId, which: usize, f: impl FnOnce(&mut TurretAi<World>, &mut World) -> R) -> Option<R> {
        let mut t = self.turret_ais.map.get_mut(&id).and_then(|a| a.get_mut(which)).and_then(Option::take)?;
        self.turret_ais.running.push((id, which));
        let r = self.with_ai_self(id, |w| f(&mut t, w));
        self.turret_ais.running.pop();
        if let Some(slot) = self.turret_ais.map.get_mut(&id).and_then(|a| a.get_mut(which)) {
            *slot = Some(t);
        }
        Some(r)
    }

    /// The fields of turret `which` of `id` as they are now: in the table, or lent while its machine runs (the top of
    /// `ai_stacks.turret`, when that turret is the innermost running one).
    pub fn turret_peek(&self, id: ObjectId, which: usize) -> Option<&TurretData> {
        if let Some(t) = self.turret_ais.map.get(&id).and_then(|a| a.get(which)).and_then(Option::as_ref) {
            return Some(&t.data);
        }
        if self.turret_ais.running.last() == Some(&(id, which)) {
            return self.ai_stacks.turret.last().filter(|d| d.which_turret == which as WhichTurretType);
        }
        None
    }

    /// The angle and pitch of turret `which` of `id` (`getTurretRotAndPitch`); `None` without one.
    pub fn ai_turret_rot_and_pitch(&self, id: ObjectId, which: i32) -> Option<(f32, f32)> {
        let which = usize::try_from(which).ok().filter(|&w| w < 2)?;
        self.turret_peek(id, which).map(|d| (d.angle, d.pitch))
    }

    /// `obj->getPhysics()`, read in place (GitHub #41: the client's draw modules read its turning, motive force and
    /// speed with [`PhysicsBehavior`]'s `peek_*`); `None` without physics, or while its module is out (lent).
    pub fn physics_peek(&self, obj: ObjectId) -> Option<&crate::logic::modules::physics_behavior::PhysicsBehavior> {
        let index = self.find_object(obj).and_then(|o| o.physics)?;
        match self.peek_module_body(ModuleHandle { object: obj, index }) {
            Some(UpdateBody::PhysicsBehavior(p)) => Some(p),
            _ => None,
        }
    }

    /// `obj->getAIUpdateInterface()->getCurLocomotorSpeed()` read in place (GitHub #41): the current locomotor's
    /// `getMaxSpeedForCondition` with the body's damage state ([`World::peek_damage_state`], the body module read in
    /// place) and the global data's movement-penalty state; 0 without a current locomotor, `None` without an AI.
    /// Nothing is written: the same fields `AIUpdateInterface::getCurLocomotorSpeed` reads.
    pub fn peek_cur_locomotor_speed(&self, obj: ObjectId) -> Option<f32> {
        let ai = self.ai_peek(obj)?;
        Some(ai.get_cur_locomotor().map_or(0.0, |l| l.get_max_speed_for_condition(self, self.peek_damage_state(obj))))
    }

    /// Whether `obj`'s AI is lent to the World now (on top of `ai_stacks`: `obj` is `this` and its module holds no AI).
    pub(crate) fn ai_is_lent(&self, obj: ObjectId) -> bool {
        obj == self.ai_self && !self.ai_stacks.ai.is_empty() && !self.ai_module_holds_ai(obj)
    }

    fn ai_module_holds_ai(&self, obj: ObjectId) -> bool {
        let Some(index) = self.find_object(obj).and_then(|o| o.ai) else { return false };
        matches!(self.peek_module_body(ModuleHandle { object: obj, index }), Some(UpdateBody::AiUpdate(m)) if m.module.as_ref().is_some_and(|m| m.ai.is_some()))
    }

    /// `obj->getAI()`'s state: in its module, or lent.
    pub fn ai_peek(&self, obj: ObjectId) -> Option<&AiUpdate> {
        let index = self.find_object(obj).and_then(|o| o.ai)?;
        if let Some(UpdateBody::AiUpdate(m)) = self.peek_module_body(ModuleHandle { object: obj, index }) {
            if let Some(a) = m.module.as_ref().and_then(|m| m.ai.as_ref()) {
                return Some(a);
            }
        }
        if self.ai_is_lent(obj) {
            return self.ai_stacks.ai.last();
        }
        None
    }

    /// [`World::ai_peek`], mutably: in its module, or lent (batch I44).
    pub(crate) fn ai_peek_mut(&mut self, obj: ObjectId) -> Option<&mut AiUpdate> {
        let index = self.find_object(obj).and_then(|o| o.ai)?;
        if self.ai_module_holds_ai(obj) {
            return match self.peek_module_body_mut(ModuleHandle { object: obj, index }) {
                Some(UpdateBody::AiUpdate(m)) => m.module.as_mut().and_then(|m| m.ai.as_mut()),
                _ => None,
            };
        }
        if self.ai_is_lent(obj) {
            return self.ai_stacks.ai.last_mut();
        }
        None
    }

    /// `obj->getAI()`'s machine: in its module, or lent (the top of `ai_stacks.machine`).
    pub(crate) fn ai_machine_query(&self, obj: ObjectId) -> Option<&StateMachine<AiState<World>>> {
        self.ai_machine_peek(obj)
    }

    fn ai_machine_peek(&self, obj: ObjectId) -> Option<&StateMachine<AiState<World>>> {
        let index = self.find_object(obj).and_then(|o| o.ai)?;
        if let Some(UpdateBody::AiUpdate(m)) = self.peek_module_body(ModuleHandle { object: obj, index }) {
            if let Some(sm) = m.module.as_ref().and_then(|m| m.machine.as_ref()) {
                return Some(sm);
            }
        }
        if self.ai_is_lent(obj) {
            return self.ai_stacks.machine.last();
        }
        None
    }

    /// Runs `f` with `obj`'s AI lent to the World (its state on `ai_stacks.ai`, its machine on `ai_stacks.machine`, `obj` as
    /// `this`), as the AI's own run lends them; `None` without an AI (or one that cannot be reached: see the module's notes).
    /// Already lent: run in place.
    pub fn with_ai_lent<R>(&mut self, obj: ObjectId, f: impl FnOnce(&mut World) -> R) -> Option<R> {
        if self.ai_is_lent(obj) {
            return Some(f(self));
        }
        let index = self.find_object(obj).and_then(|o| o.ai)?;
        let h = ModuleHandle { object: obj, index };
        let mut body = self.take_module_body(h)?;
        let lent = match &mut body {
            UpdateBody::AiUpdate(m) => m.module.as_mut().and_then(|m| Some((m.ai.take()?, m.machine.take()?))),
            _ => None,
        };
        let Some((ai, machine)) = lent else {
            self.put_module_body(h, body);
            return None;
        };
        self.put_module_body(h, body);
        self.ai_stacks.ai.push(ai);
        self.ai_stacks.machine.push(machine);
        let r = self.with_ai_self(obj, f);
        let machine = self.ai_stacks.machine.pop().expect("the lent machine");
        let ai = self.ai_stacks.ai.pop().expect("the lent AI");
        match self.take_module_body(h) {
            Some(UpdateBody::AiUpdate(mut m)) => {
                if let Some(module) = m.module.as_mut() {
                    module.ai = Some(ai);
                    module.machine = Some(machine);
                }
                self.put_module_body(h, UpdateBody::AiUpdate(m));
            }
            Some(other) => self.put_module_body(h, other),
            None => {}
        }
        Some(r)
    }

    /// `m_turretAI[i] = newInstance(TurretAI)(getObject(), data, i)` for each turret of the AI's data, with `id` as `this`. A
    /// turret whose data names no weapon slot (upstream's debug crash) or whose machine cannot enter its default state is
    /// recorded in `team_live_errors` and not made.
    pub(crate) fn make_turrets(&mut self, id: ObjectId, data: &crate::logic::ai::ai_update_data::AiUpdateModuleData) {
        let mut made: [Option<TurretAi<World>>; 2] = [None, None];
        for (i, td) in data.turret_data.iter().enumerate() {
            let Some(td) = td else { continue };
            match self.with_ai_self(id, |w| TurretAi::new(w, td.clone(), i as WhichTurretType)) {
                Ok(Ok(t)) => made[i] = Some(t),
                Ok(Err(e)) => self.cuts.team_live_errors.push(format!("TurretAI({i}): {e:?}")),
                Err(_) => self.cuts.team_live_errors.push(format!("TurretAI({i}): no controlled weapon slots")),
            }
        }
        if made.iter().any(Option::is_some) {
            self.turret_ais.map.insert(id, made);
        }
    }
}

impl TurretHost for World {
    fn turret_stack(&mut self) -> &mut Vec<TurretData> {
        &mut self.ai_stacks.turret
    }
    fn set_model_condition(&mut self, obj: ObjectId, flag: ModelConditionFlagType, set: bool) {
        if set {
            self.set_model_condition_state(obj, flag);
        } else {
            self.clear_model_condition_state(obj, flag);
        }
    }
    fn react_to_turret_change(&mut self, obj: ObjectId, turret: WhichTurretType, old_angle: f32, old_pitch: f32) {
        World::react_to_turret_change(self, obj, turret, old_angle, old_pitch);
    }
    fn has_ai(&self, obj: ObjectId) -> bool {
        self.find_object(obj).is_some_and(|o| o.ai.is_some())
    }
    fn ai_next_mood_check_time(&self, obj: ObjectId) -> u32 {
        self.ai_peek(obj).map_or(0, AiUpdate::get_next_mood_check_time)
    }
    fn ai_reset_next_mood_check_time(&mut self, obj: ObjectId) {
        self.with_ai_lent(obj, |w| crate::logic::ai::ai_states::with_ai(w, |a, w| a.reset_next_mood_check_time(w)));
    }
    fn ai_turret_sync(&self, obj: ObjectId) -> WhichTurretType {
        self.ai_peek(obj).map_or(TURRET_INVALID, |a| a.turret_sync_flag)
    }
    fn ai_set_turret_sync(&mut self, obj: ObjectId, turret: WhichTurretType) {
        self.with_ai_lent(obj, |w| crate::logic::ai::ai_states::with_ai(w, |a, _| a.turret_sync_flag = turret));
    }
    fn ai_goal_object(&self, obj: ObjectId) -> Option<ObjectId> {
        let id = self.ai_machine_peek(obj)?.goal_object_id();
        (id != INVALID_ID && self.objects.find(id).is_some()).then_some(id)
    }
    fn ai_next_mood_target(&mut self, obj: ObjectId, called_by_ai: bool, called_during_idle: bool) -> Option<ObjectId> {
        let id = self.with_ai_lent(obj, |w| {
            let sm = w.ai_stacks.machine.pop().expect("the lent machine");
            let r = crate::logic::ai::ai_states::with_ai(w, |a, w| a.get_next_mood_target(w, &sm, called_by_ai, called_during_idle));
            w.ai_stacks.machine.push(sm);
            r
        })?;
        (id != INVALID_ID).then_some(id)
    }
    fn ai_add_targeter(&mut self, target: ObjectId, attacker: ObjectId, add: bool) {
        // (only `JetAIUpdate` overrides the base's, which does nothing)
        if matches!(self.ai_peek(target).map(|a| &a.subclass), Some(AiSubclass::Jet(_))) {
            self.with_ai_lent(target, |w| crate::logic::ai::jet_ai_update::add_targeter(w, attacker, add));
        }
    }
    fn ai_is_temporarily_preventing_aim_success(&mut self, obj: ObjectId) -> bool {
        // (the base's is false; `JetAIUpdate` overrides it)
        if matches!(self.ai_peek(obj).map(|a| &a.subclass), Some(AiSubclass::Jet(_))) {
            return self.with_ai_lent(obj, crate::logic::ai::jet_ai_update::is_temporarily_preventing_aim_success).unwrap_or(false);
        }
        false
    }
    fn choose_best_weapon_for_target(&mut self, obj: ObjectId, target: ObjectId) {
        let _ = crate::logic::world::object_weapons::choose_best_weapon_for_target(
            self,
            obj,
            Some(target),
            crate::logic::weapon_set_state::PREFER_MOST_DAMAGE,
            CommandSourceType::FromAi,
        );
    }
    fn is_able_to_attack(&mut self, obj: ObjectId) -> bool {
        World::is_able_to_attack(self, obj)
    }
    fn able_to_attack_specific_object(&mut self, obj: ObjectId, attack_type: i32, target: ObjectId, source: Option<CommandSourceType>) -> CanAttackResult {
        // (the source is the owner's AI's: the owner has one)
        let source = source.unwrap_or(CommandSourceType::FromAi);
        // (an error is upstream's undefined behaviour: recorded, and the attack not possible)
        match crate::logic::world::object_weapons::get_able_to_attack_specific_object(self, obj, attack_type, target, source, -1) {
            Ok(r) => r,
            Err(e) => {
                self.cuts.team_live_errors.push(format!("getAbleToAttackSpecificObject: {e:?}"));
                CanAttackResult::NotPossible
            }
        }
    }
    fn team(&self, obj: ObjectId) -> Option<TeamId> {
        self.get_team(obj)
    }
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D) {
        crate::logic::weapon_host::WeaponHost::bridge_attack_points(self, bridge)
    }
    fn distance_squared_bounding_sphere_3d(&self, obj: ObjectId, pos: &Coord3D) -> f32 {
        self.partition_distance_squared_to_pos(obj, pos, DistanceCalculationType::FromBoundingSphere3D)
    }
    fn relative_angle_2d(&self, obj: ObjectId, pos: &Coord3D) -> f32 {
        self.partition_relative_angle_2d(obj, pos)
    }
    fn vector_to_center_3d(&self, obj: ObjectId, pos: &Coord3D) -> Coord3D {
        self.partition_vector_to_pos(obj, pos, DistanceCalculationType::FromCenter3D)
    }
    fn max_height_above_position(&self, obj: ObjectId) -> f32 {
        self.find_object(obj).map_or(0.0, |o| o.geometry_info.get_max_height_above_position())
    }
    fn random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }
    fn audio_is_playing(&mut self, handle: u32) -> bool {
        self.audio_is_currently_playing(handle)
    }
    fn audio_add_turret_sound(&mut self, obj: ObjectId) -> u32 {
        // `*getTemplate()->getPerUnitSound("TurretMoveLoop")`, played on the object (the client's audio: a cut point)
        let name = self.find_object(obj).map(|o| o.template().get_per_unit_sound(b"TurretMoveLoop").event_name.clone()).unwrap_or_default();
        self.audio_add_event(name, obj)
    }
    fn audio_remove(&mut self, handle: u32) {
        self.audio_remove_event(handle);
    }
}
