// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp and Object/Update/AIUpdate.cpp (the calls the attack
//     states and commands make of their owner's weapons, body and physics, of other objects' AIs and of the partition
//     manager, as the World answers them)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I9c-5: combat in the World. The AI's host calls that the attack machine makes (choose a weapon, range
//! and approach, aim, pre-fire, fire, locks and clips; distances and angles; other objects' AIs and bodies) answered by the
//! World's own weapon set, body and partition code. Until this batch each was a recorded cut returning a default, so an
//! attack order entered the attack state, found no weapon (`chooseBestWeaponForTarget` false) and failed back to idle.
//!
//! **The answers:** a read of the current weapon (range, approach, too close) asks a copy of it, as the partition host does
//! (the question needs the World mutably and does not change the weapon); a change (fire, pre-attack frame, shot count, clip)
//! takes the weapon out of its slot and puts it back. An error of the weapon code (`WResult`) is recorded in
//! `cuts.team_live_errors` and answered as upstream's failure (false, nothing done).
//!
//! With them, three calls the gate scenario still reached as cuts: a dozer's bored search (`findObjectToRepair`'s candidates,
//! `findMine`) through the partition manager with upstream's filters, and `addTargeter`, which only a jet's AI overrides (the
//! base's does nothing).
//!
//! `p1w13-sonnet`, integration batch I9c-5, 2026-11-01.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{CommandSourceType as Source, ATTACK_NEW_TARGET};
use crate::common::game_type::{ObjectId, WeaponSlotType, INVALID_ID};
use crate::logic::ai::ai_states::DistanceKind;
use crate::logic::ai::ai_update_types::{CanAttackResultValue, CommandSourceType, ATTACKRESULT_NOT_POSSIBLE};
use crate::logic::weapon::RETURN_TO_BASE_TO_RELOAD;
use crate::logic::weapon_fire::Weapon;
use crate::logic::weapon_set_state::WeaponChoiceCriteria;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_weapons::{self as ow, FireAt, ObjectWeaponHost};
use crate::logic::world::partition::collide::DistanceCalculationType;
use crate::logic::world::partition::filters::PartitionFilter;

/// The AI's command source (`CommandSourceType` as an `int`) as the weapon code's enum.
pub(crate) fn source(src: CommandSourceType) -> Source {
    match src {
        0 => Source::FromPlayer,
        1 => Source::FromScript,
        3 => Source::FromDozer,
        4 => Source::DefaultSwitchWeapon,
        _ => Source::FromAi,
    }
}

fn distance_type(kind: DistanceKind) -> DistanceCalculationType {
    match kind {
        DistanceKind::Center2d => DistanceCalculationType::FromCenter2D,
        DistanceKind::BoundingSphere3d => DistanceCalculationType::FromBoundingSphere3D,
        DistanceKind::BoundingSphere2d => DistanceCalculationType::FromBoundingSphere2D,
    }
}

impl World {
    fn combat_error(&mut self, call: &str, e: impl std::fmt::Debug) {
        self.cuts.team_live_errors.push(format!("{call}: {e:?}"));
    }

    /// An action manager's answer; its error recorded and answered false.
    pub(crate) fn ai_action_answer<E: std::fmt::Debug>(&mut self, call: &str, r: Result<bool, E>) -> bool {
        r.unwrap_or_else(|e| {
            self.combat_error(call, e);
            false
        })
    }

    /// A copy of the current weapon of `o` (`getCurrentWeapon()`), for a question that does not change it.
    fn current_weapon_copy(&self, o: ObjectId) -> Option<Weapon> {
        self.objects.find(o)?.weapon_set.get_cur_weapon().cloned()
    }

    /// The weapon in `slot` of `o`, changed by `f` (taken out of its slot while `f` runs, then put back).
    fn with_slot_weapon<R>(&mut self, o: ObjectId, slot: i32, f: impl FnOnce(&mut Weapon, &mut World) -> R) -> Option<R> {
        let mut w = self.take_weapon(o, slot)?;
        let r = f(&mut w, self);
        self.put_weapon(o, slot, w);
        Some(r)
    }

    pub(crate) fn ai_choose_best_weapon(&mut self, o: ObjectId, target: Option<ObjectId>, criteria: WeaponChoiceCriteria, src: CommandSourceType) -> bool {
        match ow::choose_best_weapon_for_target(self, o, target, criteria, source(src)) {
            Ok(found) => found,
            Err(e) => {
                self.combat_error("Object::chooseBestWeaponForTarget", e);
                false
            }
        }
    }

    pub(crate) fn ai_weapon_within_range_of_object(&mut self, o: ObjectId, victim: ObjectId) -> bool {
        self.current_weapon_copy(o).is_some_and(|w| w.is_within_attack_range(self, o, victim))
    }

    pub(crate) fn ai_weapon_within_range_of_position(&mut self, o: ObjectId, pos: &Coord3D) -> bool {
        self.current_weapon_copy(o).is_some_and(|w| w.is_within_attack_range_pos(self, o, pos))
    }

    /// `computeApproachTarget(obj, victim, pos, 0, result)`'s result (the position itself without a weapon: upstream's callers
    /// ask only with one).
    pub(crate) fn ai_weapon_approach_target(&mut self, o: ObjectId, victim: ObjectId, pos: &Coord3D) -> Coord3D {
        let Some(w) = self.current_weapon_copy(o) else { return *pos };
        let target = (victim != INVALID_ID).then_some(victim);
        w.compute_approach_target(self, o, target, Some(*pos), 0.0).1
    }

    pub(crate) fn ai_weapon_goal_within_range(&mut self, o: ObjectId, goal: &Coord3D, victim: ObjectId, victim_pos: &Coord3D) -> bool {
        let target = (victim != INVALID_ID).then_some(victim);
        self.current_weapon_copy(o).is_some_and(|w| w.is_goal_pos_within_attack_range(self, o, goal, target, victim_pos))
    }

    pub(crate) fn ai_weapon_too_close(&mut self, o: ObjectId, victim: ObjectId) -> bool {
        self.current_weapon_copy(o).is_some_and(|w| w.is_too_close(self, o, victim))
    }

    /// The slot's weapon's `fireWeapon(owner, pos)`: whether it reloaded (the siblings' shared reload after, as
    /// `fireCurrentWeapon`).
    pub(crate) fn ai_weapon_fire_at(&mut self, o: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool {
        let shared = self.weapon_set(o).is_shared_reload_time(self);
        let fired = self.with_slot_weapon(o, slot as i32, |w, world| w.fire_weapon_at_pos(world, o, pos, None, shared));
        match fired {
            Some(Ok(outcome)) => {
                self.weapon_set_mut(o).apply_shared_reload(outcome.shared);
                outcome.reloaded
            }
            Some(Err(e)) => {
                self.combat_error("Weapon::fireWeapon", e);
                false
            }
            None => false,
        }
    }

    pub(crate) fn ai_weapon_set_pre_attack_finished(&mut self, o: ObjectId, slot: WeaponSlotType, frame: u32) {
        self.with_slot_weapon(o, slot as i32, |w, _| w.set_pre_attack_finished_frame(frame));
    }

    pub(crate) fn ai_weapon_set_max_shot_count(&mut self, o: ObjectId, n: i32) {
        let slot = self.weapon_set(o).get_cur_weapon_slot();
        self.with_slot_weapon(o, slot, |w, _| w.set_max_shot_count(n));
    }

    pub(crate) fn ai_pre_fire(&mut self, o: ObjectId, victim: Option<ObjectId>) {
        if let Err(e) = ow::pre_fire_current_weapon(self, o, victim) {
            self.combat_error("Object::preFireCurrentWeapon", e);
        }
    }

    pub(crate) fn ai_fire(&mut self, o: ObjectId, at: FireAt) {
        if let Err(e) = ow::fire_current_weapon(self, o, at) {
            self.combat_error("Object::fireCurrentWeapon", e);
        }
    }

    pub(crate) fn ai_weapon_clip(&mut self, o: ObjectId, slot: WeaponSlotType) -> Option<(bool, i32, i32, i32)> {
        let now = crate::common::state_machine::StateContext::frame(self);
        let w = self.objects.find(o)?.weapon_set.get_weapon_in_weapon_slot(slot as i32)?.clone();
        let t = self.statics.weapon_store.templates().get(w.template())?;
        let (returns, clip) = (t.reload_type == RETURN_TO_BASE_TO_RELOAD, t.clip_size);
        let reload = w.get_clip_reload_time(self, o);
        Some((returns, i32::try_from(w.remaining_ammo(now)).unwrap_or(i32::MAX), clip, reload))
    }

    pub(crate) fn ai_weapon_set_clip_percent_full(&mut self, o: ObjectId, slot: WeaponSlotType, percent: f32, allow_reduction: bool) {
        self.with_slot_weapon(o, slot as i32, |w, world| w.set_clip_percent_full(world, percent, allow_reduction));
    }

    /// `TheActionManager->getCanAttackObject(owner, target, src, ATTACK_NEW_TARGET)`.
    pub(crate) fn ai_action_can_attack(&mut self, o: ObjectId, target: ObjectId, src: CommandSourceType) -> CanAttackResultValue {
        match crate::logic::action_manager::get_can_attack_object(self, Some(o), Some(target), source(src), ATTACK_NEW_TARGET) {
            Ok(r) => r as CanAttackResultValue,
            Err(e) => {
                self.combat_error("ActionManager::getCanAttackObject", e);
                ATTACKRESULT_NOT_POSSIBLE
            }
        }
    }

    pub(crate) fn ai_distance_squared(&mut self, o: ObjectId, other: ObjectId, kind: DistanceKind) -> f32 {
        self.partition.get_distance_squared(self, o, other, distance_type(kind)).map_or(0.0, |(d, _)| d)
    }

    pub(crate) fn ai_distance_squared_pos(&mut self, o: ObjectId, pos: &Coord3D, kind: DistanceKind) -> f32 {
        self.partition.get_distance_squared_pos(self, o, pos, distance_type(kind)).map_or(0.0, |(d, _)| d)
    }

    pub(crate) fn ai_relative_angle(&mut self, o: ObjectId, other: ObjectId) -> f32 {
        self.partition.get_relative_angle_2d(self, o, other).unwrap_or(0.0)
    }

    /// `ThePartitionManager->getClosestObject(pos, range, FROM_CENTER_2D, filters)`: possible to attack (`ATTACK_NEW_TARGET`,
    /// `src`), of `player` when one is given (`PartitionFilterSamePlayer`: the fire state's continue-range retarget), and of the
    /// same map status.
    pub(crate) fn ai_closest_attack_target(&mut self, o: ObjectId, pos: &Coord3D, range: f32, src: CommandSourceType, player: Option<Option<PlayerIndex>>) -> ObjectId {
        let mut filters = vec![PartitionFilter::PossibleToAttack { attack_type: ATTACK_NEW_TARGET, obj: Some(o), command_source: source(src) }];
        if let Some(player) = player {
            filters.push(PartitionFilter::SamePlayer { player });
        }
        filters.push(PartitionFilter::SameMapStatus { obj: o });
        self.with_partition(|pm, w| pm.get_closest_object_at(w, pos, range, DistanceCalculationType::FromCenter2D, &filters).object).unwrap_or(INVALID_ID)
    }

    /// Another object's body's last damage: `out.m_noEffect`, `in.m_sourceID`, `getLastDamageTimestamp()` (`None`: no body).
    pub(crate) fn ai_object_last_damage(&mut self, id: ObjectId) -> Option<(bool, ObjectId, u32)> {
        let (no_effect, src) = self.peek_last_damage(id).map(|d| (d.out.no_effect, d.input.source_id))?;
        let body = self.get_body_module(id)?;
        let when = self.module_call(id, body, ModuleCall::GetLastDamageTimestamp).as_int(0) as u32;
        Some((no_effect, src, when))
    }

    pub(crate) fn ai_clearable_last_attacker(&mut self, o: ObjectId) -> ObjectId {
        let Some(body) = self.get_body_module(o) else { return INVALID_ID };
        self.module_call(o, body, ModuleCall::GetClearableLastAttacker).as_id()
    }

    pub(crate) fn ai_clear_last_attacker(&mut self, o: ObjectId) {
        if let Some(body) = self.get_body_module(o) {
            self.module_call(o, body, ModuleCall::ClearLastAttacker);
        }
    }

    /// Another object's AI's `isIdle()`: its module asked, or (its AI lent: it is the one running) its machine's state.
    pub(crate) fn ai_other_is_idle(&mut self, id: ObjectId) -> bool {
        if self.ai_is_lent(id) {
            use crate::common::state_machine::StateBehavior;
            return self.ai_machine_query(id).is_some_and(|m| m.state(m.get_current_state_id()).is_some_and(|s| s.is_idle()));
        }
        let Some(index) = self.find_object(id).and_then(|o| o.ai) else { return false };
        self.module_call(id, index, ModuleCall::AiIsIdle).as_bool()
    }

    /// `DozerAIUpdate.cpp`'s `findObjectToRepair` candidates: `iterateObjectsInRange(pos, range, FROM_CENTER_2D, {same player
    /// as the dozer, STRUCTURE, same map status})`, fastest order.
    pub(crate) fn ai_repair_candidates(&mut self, dozer: ObjectId, pos: &Coord3D, range: f32) -> Vec<ObjectId> {
        use crate::common::kind_of::{KindOfMaskType, KINDOF_STRUCTURE};
        let filters = [
            PartitionFilter::SamePlayer { player: self.get_controlling_player(dozer) },
            PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_STRUCTURE]), must_be_clear: KindOfMaskType::new() },
            PartitionFilter::SameMapStatus { obj: dozer },
        ];
        let it = self.with_partition(|pm, w| {
            pm.iterate_objects_in_range_at(w, pos, range, DistanceCalculationType::FromCenter2D, &filters, crate::logic::object_iter::IterOrderType::Fastest)
        });
        crate::logic::world::player_host_world::iterator_ids(it)
    }

    /// `DozerAIUpdate.cpp`'s `findMine`: "only clear enemy or neutral mines": `getClosestObject(dozer, range, FROM_CENTER_2D,
    /// {enemies or neutral, possible to attack (ATTACK_NEW_TARGET, CMD_FROM_DOZER), same map status})`.
    pub(crate) fn ai_closest_mine(&mut self, dozer: ObjectId, range: f32) -> ObjectId {
        use crate::logic::world::partition::filters::{ALLOW_ENEMIES, ALLOW_NEUTRAL};
        let filters = [
            PartitionFilter::Relationship { obj: dozer, flags: ALLOW_ENEMIES | ALLOW_NEUTRAL },
            PartitionFilter::PossibleToAttack { attack_type: ATTACK_NEW_TARGET, obj: Some(dozer), command_source: Source::FromDozer },
            PartitionFilter::SameMapStatus { obj: dozer },
        ];
        self.with_partition(|pm, w| pm.get_closest_object(w, dozer, range, DistanceCalculationType::FromCenter2D, &filters).object).unwrap_or(INVALID_ID)
    }

    /// `findObjectByID(target)->getAI()->addTargeter(targeter, add)`: a jet's (the base's does nothing).
    pub(crate) fn ai_add_targeter_live(&mut self, target: ObjectId, targeter: ObjectId, add: bool) {
        use crate::logic::ai::ai_update::AiSubclass;
        if matches!(self.ai_peek(target).map(|a| &a.subclass), Some(AiSubclass::Jet(_))) {
            self.with_ai_lent(target, |w| crate::logic::ai::jet_ai_update::add_targeter(w, targeter, add));
        }
    }
}
