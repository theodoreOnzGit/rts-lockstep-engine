// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp (`findClosestEnemy`, `findClosestAlly`, `findClosestRepulsor`,
//     `getAdjustedVisionRangeForObject`, as the World runs them)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I9c-3: `TheAI`'s queries in the World ([`AiQueryHost`] for World), live with `cuts.live_ai` (without it
//! they stay the cut tables, as before): the closest enemy, ally and repulsor through the World's partition manager, and an
//! object's vision range adjusted for its owner, guard ring and mood.
//!
//! **The answers:** a template is its id, an attack priority list (`AttackPriorityInfo*`) is its index in the script engine's
//! (0 the default), the contained objects are those whose `contained_by` is the container (the query takes the highest
//! priority among them, so their order does not matter), the mood matrix value is the live AI's. `getLargestWeaponRange()`
//! needs the World mutably (a weapon's range query may); the read-only query reads the value [`World::ai_adjusted_vision_range`]
//! computed just before (`largest_weapon_range_for`).
//!
//! `p1w13-sonnet`, integration batch I9c-3, 2026-11-01.

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::logic::ai::ai_system::{self, AiQueryHost};
use crate::logic::world::game_logic::World;

impl AiQueryHost for World {
    fn template_of(&self, id: ObjectId) -> i32 {
        self.find_object(id).map_or(-1, |o| i32::from(o.template().get_template_id()))
    }
    fn attack_priority(&self, info: i32, template: i32) -> i32 {
        let infos = &self.script_engine.attack_priority_info;
        let i = usize::try_from(info).ok().filter(|&i| i < infos.len()).unwrap_or(0);
        infos.get(i).map_or(0, |a| a.get_priority(Some(template)))
    }
    fn is_default_attack_info(&self, info: i32) -> bool {
        info == 0
    }
    fn contained_templates(&self, id: ObjectId) -> Vec<i32> {
        self.objects.ids_in_order().into_iter().filter(|&o| self.find_object(o).is_some_and(|x| x.contained_by == id)).map(|o| self.template_of(o)).collect()
    }
    fn vision_range(&self, id: ObjectId) -> f32 {
        self.get_vision_range(id)
    }
    fn has_ai(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some_and(|o| o.ai.is_some())
    }
    fn mood_matrix_value(&self, id: ObjectId) -> u32 {
        match (self.ai_peek(id), self.ai_machine_query(id)) {
            (Some(a), m) => self.with_ai_self_ref(id, |w| a.get_mood_matrix_value(w, m)),
            _ => 0,
        }
    }
    fn is_contained(&self, id: ObjectId) -> bool {
        World::is_contained(self, id)
    }
    fn largest_weapon_range(&self, id: ObjectId) -> f32 {
        match self.cuts.ai_host.largest_weapon_range {
            Some((o, r)) if o == id => r,
            _ => -1.0,
        }
    }
}

impl World {
    /// `f` with `id` as the AI hosts' `this`, read-only (the queries' mood matrix asks the AI host about its own object).
    fn with_ai_self_ref<T>(&self, id: ObjectId, f: impl FnOnce(&World) -> T) -> T {
        if self.ai_self == id {
            return f(self);
        }
        // (a read-only World cannot change `ai_self`: the query's object is `this` in every caller, which the debug check
        // pins)
        debug_assert_eq!(self.ai_self, id, "the AI query's object is the running AI's");
        f(self)
    }

    /// `TheAI->findClosestEnemy(me, range, qualifiers, info)` (`INVALID_ID`: none).
    pub fn ai_find_closest_enemy_live(&mut self, me: ObjectId, range: f32, qualifiers: u32, info: Option<usize>) -> ObjectId {
        let data = self.statics.clone();
        let info = info.map(|i| i as i32);
        self.with_ai_self(me, |w| w.with_partition(|pm, w| ai_system::find_closest_enemy(pm, w, &data.ai_data, me, range, qualifiers, info, None)))
            .unwrap_or(INVALID_ID)
    }

    /// `TheAI->findClosestAlly(me, range, qualifiers)` (`INVALID_ID`: none).
    pub fn ai_find_closest_ally_live(&mut self, me: ObjectId, range: f32, qualifiers: u32) -> ObjectId {
        self.with_ai_self(me, |w| w.with_partition(|pm, w| ai_system::find_closest_ally(pm, w, me, range, qualifiers))).unwrap_or(INVALID_ID)
    }

    /// `TheAI->findClosestRepulsor(me, range)` (`INVALID_ID`: none).
    pub fn ai_find_closest_repulsor_live(&mut self, me: ObjectId, range: f32) -> ObjectId {
        let data = self.statics.clone();
        self.with_ai_self(me, |w| w.with_partition(|pm, w| ai_system::find_closest_repulsor(pm, w, &data.ai_data, me, range))).unwrap_or(INVALID_ID)
    }

    /// `TheAI->getAdjustedVisionRangeForObject(object, factors)`, its largest weapon range computed first (see the module's
    /// notes).
    pub fn ai_adjusted_vision_range(&mut self, object: ObjectId, factors: i32) -> f32 {
        let r = crate::logic::world::object_weapons::get_largest_weapon_range(self, object);
        self.cuts.ai_host.largest_weapon_range = Some((object, r));
        let data = self.statics.clone();
        let v = self.with_ai_self(object, |w| ai_system::get_adjusted_vision_range_for_object(w, &data.ai_data, object, factors));
        self.cuts.ai_host.largest_weapon_range = None;
        v
    }
}

#[cfg(test)]
mod tests {
    use crate::common::base_type::Coord3D;
    use crate::common::game_type::INVALID_ID;
    use crate::logic::world::player_host_world::tests::{make, world};

    /// `TheAI`'s queries through the World's partition manager. (Written first without the player's relationship to itself, the
    /// runners were `NEUTRAL` to each other and no ally was found: the fixture runs neither `Player::initFromDict` nor
    /// `PlayerList::newGame`, which ally a player with itself.) Two runners of one team 30 apart: each is the other's closest
    /// ally within 100, neither within 10; there is no enemy (one player). A runner's adjusted vision range is its vision range
    /// (the fixture's runner has none: 0) with no factors.
    #[test]
    fn the_ais_queries_run_on_the_world() {
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        // (the player allied to itself, as `Player::initFromDict` and `PlayerList::newGame` leave it; the fixture runs neither)
        crate::logic::world::player_ops::player_set_player_relationship(&mut w, 1, Some(1), crate::common::game_common::Relationship::Allies).unwrap();
        let a = make(&mut w, b"Runner", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let b = make(&mut w, b"Runner", team, Coord3D { x: 130.0, y: 100.0, z: 0.0 });
        assert_eq!(w.ai_find_closest_ally_live(a, 100.0, 0), b);
        assert_eq!(w.ai_find_closest_ally_live(b, 100.0, 0), a);
        assert_eq!(w.ai_find_closest_ally_live(a, 10.0, 0), INVALID_ID);
        assert_eq!(w.ai_find_closest_enemy_live(a, 1000.0, 0, None), INVALID_ID);
        assert_eq!(w.ai_adjusted_vision_range(a, 0), w.get_vision_range(a));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
